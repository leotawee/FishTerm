<script lang="ts">
import {defineComponent, nextTick, onBeforeUnmount, onMounted, ref, watch} from 'vue';

import Button from './Button.vue';

const props = {
    show: {
        type: Boolean,
        default: false,
    },
    // Accessible name for the dialog (screen readers announce it on open).
    label: {
        type: String,
        default: 'Dialog',
    },
};

export default defineComponent({
    components: {Button},
    name: 'ModalDialog',
    props,
    emits: ['close'],
    setup(props, context) {
        const showModal = ref(false);
        const panel = ref<HTMLDivElement | null>(null);
        let restoreFocus: HTMLElement | null = null;

        watch(
            () => props.show,
            show => {
                showModal.value = show;
                if (show) {
                    // Pull focus out of the underlying view (e.g. the terminal) so
                    // keystrokes land in the dialog while it asks something.
                    restoreFocus = document.activeElement as HTMLElement | null;
                    nextTick(() => panel.value?.focus());
                } else if (restoreFocus && restoreFocus.isConnected) {
                    restoreFocus.focus();
                    restoreFocus = null;
                }
            },
            {immediate: true},
        );

        function closeModal() {
            context.emit('close');
            showModal.value = false;
        }

        function focusables(): HTMLElement[] {
            const root = panel.value;
            if (!root) return [];
            return Array.from(
                root.querySelectorAll<HTMLElement>(
                    'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
                ),
            ).filter(el => !el.hasAttribute('disabled'));
        }

        // Capture phase: fires before xterm's own key handler, so Esc closes the
        // dialog even while the terminal holds focus and `^["` never reaches the shell.
        function onKeydown(e: KeyboardEvent) {
            if (!showModal.value) return;
            if (e.key === 'Escape') {
                e.preventDefault();
                e.stopPropagation();
                closeModal();
                return;
            }
            if (e.key !== 'Tab') return;
            const items = focusables();
            if (items.length === 0) return;
            const first = items[0]!;
            const last = items[items.length - 1]!;
            const active = document.activeElement as HTMLElement | null;
            if (!active || !panel.value?.contains(active)) {
                // Focus lives outside the dialog (e.g. terminal): pull it in.
                e.preventDefault();
                first.focus();
                return;
            }
            if (e.shiftKey && active === first) {
                e.preventDefault();
                last.focus();
            } else if (!e.shiftKey && active === last) {
                e.preventDefault();
                first.focus();
            }
        }

        onMounted(() => window.addEventListener('keydown', onKeydown, true));
        onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown, true));

        return {
            closeModal,
            showModal,
            panel,
        };
    },
});
</script>

<template>
    <teleport to="body">
        <transition
            enter-active-class="transition ease-out duration-200 transform"
            enter-from-class="opacity-0"
            enter-to-class="opacity-100"
            leave-active-class="transition ease-in duration-200 transform"
            leave-from-class="opacity-100"
            leave-to-class="opacity-0"
        >
            <div
                class="modal-backdrop fixed z-[60] bg-black w-full h-full bg-opacity-20 inset-0 max-h-screen"
                v-show="showModal"
            >
                <div
                    class="flex items-center justify-center min-h-screen max-h-screen p-4 text-center"
                    @click.self="closeModal"
                >
                    <transition
                        enter-active-class="transition ease-out duration-200 transform"
                        enter-from-class="opacity-0 translate-y-10 scale-95"
                        enter-to-class="opacity-100 translate-y-0 scale-100"
                        leave-active-class="ease-in duration-200"
                        leave-from-class="opacity-100 translate-y-0 scale-100"
                        leave-to-class="opacity-0 translate-y-10 translate-y-0 scale-95"
                    >
                        <div
                            ref="panel"
                            class="max-h-[calc(100vh-2rem)] overflow-y-auto relative bg-white dark:bg-[#121212] rounded-lg text-left shadow-xl p-8 w-[80%] lg:w-1/2 focus:outline-none"
                            role="dialog"
                            aria-modal="true"
                            v-if="showModal"
                            tabindex="-1"
                            :aria-label="label"
                        >
                            <button
                                type="button"
                                aria-label="Close"
                                class="absolute right-3 top-3 rounded p-1 hover:opacity-70 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
                                @click="closeModal"
                            >
                                <Icon
                                    icon="fas fa-times"
                                    class="text-indigo-600 dark:text-white text-xl"
                                />
                            </button>
                            <slot />
                        </div>
                    </transition>
                </div>
            </div>
        </transition>
    </teleport>
</template>
