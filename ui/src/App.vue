<script lang="ts" setup>
import {nextTick, onMounted, reactive, ref, watch} from 'vue';
import {listen} from '@tauri-apps/api/event';
import {startupWarnings, quitApp, type HostView} from './api';

import Home from './views/Home.vue';
import SSH from './views/SSH.vue';

import Dialog from './components/Dialog.vue';
import Button from './components/Button.vue';
import FishTermLogo from './components/FishTermLogo.vue';
import Info from './components/dialog/info.vue';
import AppearanceDialog from './components/dialog/appearance.vue';

const tabIndex = ref('home');
const terminal: Record<string, {title: string; host: HostView}> = reactive({});

const showModal = ref(false);
const showAppearance = ref(false);
const warnings = ref<string[]>([]);

function openTerminal(host: HostView) {
    const id = crypto.randomUUID();
    terminal[id] = {title: `${host.username}@${host.host}`, host};
    tabIndex.value = id;
}

function removeTerminal(key: string) {
    delete terminal[key];
    if (tabIndex.value === key) tabIndex.value = 'home';
}

// Closing a live SSH session is destructive, so the X only arms a confirm.
const closeTarget = ref<string | null>(null);

function requestCloseTerminal(key: string) {
    closeTarget.value = key;
}

function confirmCloseTerminal() {
    const key = closeTarget.value;
    closeTarget.value = null;
    if (key) removeTerminal(key);
}

// Quit guard: the native menu / red close button ask Rust, which forwards here
// only when terminals are live (see lib.rs on_menu_event).
const quitConfirm = ref(false);

onMounted(async () => {
    warnings.value = await startupWarnings().catch(() => []);
    listen('close-active-tab', () => {
        // Browser parity: Cmd+W closes the active tab, confirming live sessions.
        if (tabIndex.value !== 'home') requestCloseTerminal(tabIndex.value);
    });
    listen('quit-requested', () => {
        if (Object.keys(terminal).length > 0) quitConfirm.value = true;
        else quitApp();
    });
});

watch(tabIndex, (n: string) => {
    document.title = terminal[n]?.title || 'Home';
    nextTick(() =>
        document.querySelector(`[data-tab="${n}"]`)?.scrollIntoView({inline: 'nearest'}),
    );
});
</script>

<template>
    <div class="flex flex-col h-screen">
        <nav
            data-tauri-drag-region
            class="nav-acrylic z-50 w-full backdrop-blur-2xl border-b border-[var(--c-border)]"
        >
            <!-- Overlay titlebar strip: traffic lights live here; the strip drags the window. -->
            <div
                data-tauri-drag-region
                class="h-7"
            />
            <div class="flex items-center gap-1.5 px-2 pb-1.5 min-w-0">
                <!-- Product mark: the real FishTerm wordmark, theme-aware. -->
                <div
                    data-tauri-drag-region
                    class="flex items-center shrink-0 mr-1"
                >
                    <FishTermLogo />
                </div>
                <!-- Tabs scroll sideways instead of wrapping onto a second row on narrow windows. -->
                <div class="flex items-center gap-1.5 min-w-0 overflow-x-auto flex-1">
                    <button
                        class="px-3 py-1.5 rounded-md text-sm shrink-0 transition-colors border"
                        :class="tabIndex === 'home'
                            ? 'bg-[var(--c-ghost-hover)] text-[var(--c-text)] font-medium border-[var(--c-border)]'
                            : 'text-[var(--c-muted)] border-[var(--c-border)] hover:text-[var(--c-text)]'"
                        data-tab="home"
                        aria-label="Go to Home"
                        @click="tabIndex = 'home'"
                    >
                        Home
                    </button>
                    <div
                        class="shrink-0"
                        role="button"
                        tabindex="0"
                        v-for="key in Object.keys(terminal)"
                        :key="key"
                        :data-tab="key"
                        :aria-label="`Switch to tab ${terminal[key]?.title}`"
                        @click="tabIndex = key"
                        @keydown.enter="tabIndex = key"
                    >
                        <div
                            class="w-[210px] flex items-center justify-between gap-1 px-3 py-1.5 rounded-md text-sm transition-colors border"
                            :class="tabIndex === key
                                ? 'bg-[var(--c-ghost-hover)] text-[var(--c-text)] font-medium border-[var(--c-border)]'
                                : 'text-[var(--c-muted)] border-[var(--c-border)] hover:text-[var(--c-text)]'"
                        >
                            <p class="min-w-0 flex-1 truncate">
                                {{ terminal[key]?.title }}
                            </p>
                            <button
                                class="shrink-0 rounded p-0.5 hover:bg-[var(--c-ghost-hover)]"
                                aria-label="Close terminal"
                                @click.stop="requestCloseTerminal(key)"
                                @keydown.enter.stop
                            >
                                <Icon
                                    icon="fas fa-close"
                                    class="text-[var(--c-muted)]"
                                />
                            </button>
                        </div>
                    </div>
                </div>
                <div class="flex items-center shrink-0 ml-3 mr-4">
                    <button
                        aria-label="Appearance"
                        class="mr-3"
                        @click="showAppearance = true"
                    >
                        <Icon
                            icon="fas fa-palette"
                            class="text-[var(--c-text)]"
                        />
                    </button>
                    <button
                        aria-label="About FishTerm"
                        @click="showModal = true"
                    >
                        <Icon
                            icon="fas fa-info-circle"
                            class="text-[var(--c-text)]"
                        />
                    </button>
                </div>
            </div>
        </nav>

        <main class="bg-[var(--c-bg)] flex-1 min-h-0">
            <div
                :hidden="tabIndex !== 'home'"
                class="h-full"
            >
                <Home @connect="openTerminal" />
            </div>
            <div
                v-for="(tab, key) in terminal"
                :hidden="tabIndex !== key"
                :key="key"
                class="h-full"
            >
                <SSH
                    :host="tab.host"
                    :active="tabIndex === key"
                    @close="removeTerminal(key)"
                />
            </div>
        </main>
    </div>
    <Dialog
        :show="showModal"
        label="About FishTerm"
        @close="showModal = false"
    >
        <Info />
    </Dialog>
    <Dialog
        :show="showAppearance"
        label="Appearance"
        @close="showAppearance = false"
    >
        <AppearanceDialog />
    </Dialog>
    <Dialog
        :show="warnings.length > 0"
        label="Startup warning"
        @close="warnings = []"
    >
        <div class="space-y-2 text-[var(--c-text)]">
            <h1 class="text-xl font-bold">Warning</h1>
            <p
                v-for="(w, i) in warnings"
                :key="i"
                class="text-sm"
                v-text="w"
            />
        </div>
    </Dialog>
    <Dialog
        :show="closeTarget !== null"
        label="Close terminal?"
        @close="closeTarget = null"
    >
        <div class="space-y-2 text-[var(--c-text)]">
            <h1 class="text-xl font-bold">Close terminal?</h1>
            <p class="text-sm">Close this terminal? The SSH session will end.</p>
            <div class="flex justify-end space-x-2 pt-2">
                <Button
                    type="button"
                    @click="closeTarget = null"
                >
                    Cancel
                </Button>
                <Button
                    type="button"
                    :class="`!bg-red-500 hover:!bg-red-600 active:!bg-red-600`"
                    @click="confirmCloseTerminal"
                >
                    Close terminal
                </Button>
            </div>
        </div>
    </Dialog>
    <Dialog
        :show="quitConfirm"
        label="Quit FishTerm?"
        @close="quitConfirm = false"
    >
        <div class="space-y-2 text-[var(--c-text)]">
            <h1 class="text-xl font-bold">Quit FishTerm?</h1>
            <p class="text-sm">
                There are {{ Object.keys(terminal).length }} open terminals. All SSH sessions will
                end.
            </p>
            <div class="flex justify-end space-x-2 pt-2">
                <Button
                    type="button"
                    @click="quitConfirm = false"
                >
                    Cancel
                </Button>
                <Button
                    type="button"
                    :class="`!bg-red-500 hover:!bg-red-600 active:!bg-red-600`"
                    @click="quitApp"
                >
                    Quit
                </Button>
            </div>
        </div>
    </Dialog>
</template>
