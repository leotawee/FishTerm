<script lang="ts" setup>
import {nextTick, onMounted, reactive, ref, watch} from 'vue';
import {startupWarnings, type HostView} from './api';

import Home from './views/Home.vue';
import SSH from './views/SSH.vue';

import Dialog from './components/Dialog.vue';
import Button from './components/Button.vue';
import Info from './components/dialog/info.vue';

const tabIndex = ref('home');
const terminal: Record<string, {title: string; host: HostView}> = reactive({});

const showModal = ref(false);
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

onMounted(async () => {
    warnings.value = await startupWarnings().catch(() => []);
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
        <nav class="flex mx-auto bg-gray-200 z-50 w-full dark:bg-slate-800 justify-between">
            <!-- Tabs scroll sideways instead of wrapping onto a second row on narrow windows. -->
            <div class="flex min-w-0 overflow-x-auto">
                <button
                    class="ml-2 space-x-8 w-[250px] shrink-0 text-left"
                    data-tab="home"
                    aria-label="Go to Home"
                    @click="tabIndex = 'home'"
                >
                    <div
                        :class="`bg-white py-1 ${
                            tabIndex === 'home' ? 'pt-2 mt-1' : 'mt-2'
                        } rounded-t-lg hover:bg-slate-100 dark:bg-white dark:hover:bg-slate-300`"
                    >
                        <p class="ml-2 inline-flex items-center"> Home </p>
                    </div>
                </button>

                <div
                    class="ml-2 space-x-8 w-[250px] shrink-0 text-left cursor-pointer"
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
                        :class="`bg-white hover:bg-slate-100 ${
                            tabIndex === key ? 'pt-2 mt-1' : 'mt-2'
                        } py-1 rounded-t-lg dark:bg-white dark:hover:bg-slate-300`"
                    >
                        <div class="flex justify-between items-center">
                            <p class="ml-2 min-w-0 flex-1 truncate">
                                {{ terminal[key]?.title }}
                            </p>
                            <button
                                class="flex items-center rounded-full hover:bg-gray-200 px-2 mr-3 py-1"
                                aria-label="Close terminal"
                                @click.stop="requestCloseTerminal(key)"
                                @keydown.enter.stop
                            >
                                <Icon icon="fas fa-close" />
                            </button>
                        </div>
                    </div>
                </div>
            </div>
            <div class="flex items-center shrink-0 ml-3 mr-5">
                <button @click="showModal = true">
                    <Icon
                        icon="fas fa-info-circle"
                        class="text-black dark:text-white"
                    />
                </button>
            </div>
        </nav>

        <main class="dark:bg-[#121212] flex-1 min-h-0">
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
        :show="warnings.length > 0"
        label="Startup warning"
        @close="warnings = []"
    >
        <div class="space-y-2 text-black dark:text-white">
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
        <div class="space-y-2 text-black dark:text-white">
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
</template>
