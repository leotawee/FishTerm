<script lang="ts">
import {defineComponent, ref, watch} from 'vue';

import Button from './Button.vue';

const props = {
    show: {
        type: Boolean,
        default: false,
    },
};

export default defineComponent({
    components: {Button},
    name: 'ModalDialog',
    props,
    emits: ['close'],
    setup(props, context) {
        const showModal = ref(false);

        watch(
            () => props.show,
            show => {
                showModal.value = show;
            },
        );

        function closeModal() {
            context.emit('close');
            showModal.value = false;
        }

        return {
            closeModal,
            showModal,
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
                            class="max-h-[calc(100vh-2rem)] overflow-y-auto relative bg-white dark:bg-[#121212] rounded-lg text-left shadow-xl p-8 w-[80%] lg:w-1/2"
                            role="dialog"
                            aria-modal="true"
                            v-if="showModal"
                            aria-labelledby="modal-headline"
                        >
                            <button
                                class="absolute right-3 top-3"
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
