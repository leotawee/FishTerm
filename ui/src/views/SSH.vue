<script lang="ts" setup>
import {nextTick, onBeforeUnmount, onMounted, ref, watch} from 'vue';
import {Terminal} from '@xterm/xterm';
import {FitAddon} from '@xterm/addon-fit';

import Button from '../components/Button.vue';
import Dialog from '../components/Dialog.vue';
import {
    sessionOpen,
    sessionWrite,
    sessionResize,
    sessionClose,
    sessionStats,
    hostkeyDecide,
    errorOf,
    type AppError,
    type HostKeyPrompt,
    type HostView,
    type SessionEvent,
    type SessionStats,
} from '../api';

const props = defineProps<{host: HostView; active: boolean}>();
const emit = defineEmits<{close: []}>();

const term = new Terminal({
    theme: {
        background: '#0f172a',
    },
    fontFamily: 'Fira Code, monospace',
});
const fitAddon = new FitAddon();

const termDiv = ref<HTMLDivElement | null>(null);
const isShow = ref(false);

// Modal loading. Steps: 0 connecting, 3 auth failed, 4 timeout, 5 other error.
const isLoading = ref(true);
const pageStep = ref(0);
const errorText = ref('');
const prompt = ref<(HostKeyPrompt & {id: string}) | null>(null);

// Cloudflare Access sign-in progress, shown under "Connecting to server ....".
const CF_STATUS_TEXT: Record<string, string> = {
    probe: 'Looking for the Cloudflare Access app...',
    cache: 'Using the saved Cloudflare token...',
    browser: 'A browser window opened — sign in to Cloudflare Access',
    waiting: 'Waiting for your Cloudflare sign-in',
    token: 'Received the Cloudflare token...',
    connecting: 'Opening the Cloudflare tunnel...',
};
const cfStatus = ref('');
const cfAttempt = ref(0);

// Footer monitor. First sample has no delta yet, so rates stay `--` for ~3s.
const stats = ref<SessionStats>({cpu: null, mem: null, disk: null, rx_kbps: null, tx_kbps: null});
let poller = 0;
let polling = false;
const pct = (v: number | null) => (v === null ? '--%' : `${Math.round(v)}%`);
const net = (v: number | null) => (v === null ? '--' : `${Math.round(v)}K/s`);
async function refreshStats() {
    // Skip if the previous exec is still in flight: queued execs would stall the shell loop.
    if (polling || !sessionId || !props.active) return;
    polling = true;
    try {
        const s = await sessionStats(sessionId).catch(() => null);
        if (s) stats.value = s;
    } finally {
        polling = false;
    }
}
function startStats() {
    stopStats();
    poller = window.setInterval(refreshStats, 3000);
    void refreshStats();
}
function stopStats() {
    if (poller) clearInterval(poller);
    poller = 0;
}

let sessionId: string | null = null;
let unmounting = false;

function fail(err: AppError) {
    isShow.value = false;
    isLoading.value = false;
    prompt.value = null;
    cfStatus.value = '';
    pageStep.value = err.code === 'auth_failed' ? 3 : err.code === 'timeout' ? 4 : 5;
    errorText.value = err.message;
}

function onEvent(msg: ArrayBuffer | SessionEvent) {
    if (msg instanceof ArrayBuffer) {
        term.write(new Uint8Array(msg));
    } else if (msg.type === 'hostkey') {
        prompt.value = {id: msg.prompt_id, ...msg.prompt};
    } else if (msg.type === 'cf-login') {
        cfStatus.value = msg.status;
        cfAttempt.value = msg.attempt;
    } else if (msg.type === 'closed') {
        sessionId = null;
        stopStats();
        if (msg.error) fail(msg.error);
        else if (!unmounting) emit('close');
    }
}

async function decide(accept: boolean) {
    const p = prompt.value;
    prompt.value = null;
    if (p) await hostkeyDecide(p.id, accept).catch(() => {});
}

let sent = '';
let frame = 0;
function refit() {
    if (!props.active || frame) return;
    // One fit per animation frame: a window drag fires many size changes.
    frame = requestAnimationFrame(() => {
        frame = 0;
        fitAddon.fit();
        const size = `${term.cols}x${term.rows}`;
        if (sessionId && size !== sent) {
            sent = size;
            sessionResize(sessionId, term.cols, term.rows).catch(() => {});
        }
    });
}
// WHY: observes the terminal box itself, so tab-bar wraps and footer changes refit too.
const observer = new ResizeObserver(refit);

onMounted(async () => {
    // WHY: xterm measures cells at open(); measuring with the fallback font made the
    // first resize overflow by a few rows once Fira Code arrived.
    await document.fonts.load('15px "Fira Code"').catch(() => {});
    term.loadAddon(fitAddon);
    if (termDiv.value) {
        term.open(termDiv.value);
        observer.observe(termDiv.value);
    }
    fitAddon.fit();
    term.onData(data => {
        if (sessionId) sessionWrite(sessionId, data).catch(() => {});
    });

    try {
        sent = `${term.cols}x${term.rows}`;
        sessionId = await sessionOpen(props.host.id, term.cols, term.rows, onEvent);
        cfStatus.value = '';
        isShow.value = true;
        startStats();
        await nextTick();
        refit();
        term.focus();
    } catch (e) {
        fail(errorOf(e));
    }
});

watch(
    () => props.active,
    active => {
        if (active) nextTick(() => (refit(), term.focus()));
    },
);

onBeforeUnmount(() => {
    unmounting = true;
    stopStats();
    observer.disconnect();
    cancelAnimationFrame(frame);
    if (sessionId) sessionClose(sessionId).catch(() => {});
    term.dispose();
});
</script>

<template>
    <div class="flex flex-col h-full">
        <div class="relative flex-1 min-h-0 bg-[#0f172a] z-10">
            <div
                ref="termDiv"
                class="absolute inset-0"
            />
            <div
                class="absolute inset-0 flex items-center justify-center mx-auto z-20 bg-slate-800"
                v-show="!isShow"
            >
                <div
                    class="flex items-center justify-center p-10 bg-white dark:bg-slate-900 rounded-lg max-w-[80%]"
                >
                    <div class="space-y-3 text-center">
                        <div
                            role="status"
                            v-if="isLoading"
                        >
                            <svg
                                class="inline mr-2 w-8 h-8 text-gray-200 animate-spin dark:text-gray-600 fill-blue-600"
                                viewBox="0 0 100 101"
                                fill="none"
                                xmlns="http://www.w3.org/2000/svg"
                            >
                                <path
                                    d="M100 50.5908C100 78.2051 77.6142 100.591 50 100.591C22.3858 100.591 0 78.2051 0 50.5908C0 22.9766 22.3858 0.59082 50 0.59082C77.6142 0.59082 100 22.9766 100 50.5908ZM9.08144 50.5908C9.08144 73.1895 27.4013 91.5094 50 91.5094C72.5987 91.5094 90.9186 73.1895 90.9186 50.5908C90.9186 27.9921 72.5987 9.67226 50 9.67226C27.4013 9.67226 9.08144 27.9921 9.08144 50.5908Z"
                                    fill="currentColor"
                                />
                                <path
                                    d="M93.9676 39.0409C96.393 38.4038 97.8624 35.9116 97.0079 33.5539C95.2932 28.8227 92.871 24.3692 89.8167 20.348C85.8452 15.1192 80.8826 10.7238 75.2124 7.41289C69.5422 4.10194 63.2754 1.94025 56.7698 1.05124C51.7666 0.367541 46.6976 0.446843 41.7345 1.27873C39.2613 1.69328 37.813 4.19778 38.4501 6.62326C39.0873 9.04874 41.5694 10.4717 44.0505 10.1071C47.8511 9.54855 51.7191 9.52689 55.5402 10.0491C60.8642 10.7766 65.9928 12.5457 70.6331 15.2552C75.2735 17.9648 79.3347 21.5619 82.5849 25.841C84.9175 28.9121 86.7997 32.2913 88.1811 35.8758C89.083 38.2158 91.5421 39.6781 93.9676 39.0409Z"
                                    fill="currentFill"
                                />
                            </svg>
                            <span class="sr-only">Loading...</span>
                        </div>
                        <div>
                            <Icon
                                v-if="pageStep >= 3"
                                icon="fas fa-times-circle"
                                class="text-5xl text-red-500"
                            />
                        </div>
                        <div v-if="pageStep === 0">
                            <h1 class="text-black dark:text-white">Connecting to server ....</h1>
                            <h4
                                v-if="cfStatus"
                                class="text-black dark:text-white text-sm mt-2"
                            >
                                {{ CF_STATUS_TEXT[cfStatus] || cfStatus
                                }}<template v-if="cfStatus === 'waiting'">
                                    ({{ cfAttempt }}/10)</template
                                >
                            </h4>
                        </div>
                        <template v-if="pageStep >= 3">
                            <div v-if="pageStep === 3">
                                <h1 class="text-black dark:text-white">Authentication failed</h1>
                                <h4 class="text-black dark:text-white text-sm"
                                    >Please check your username, password or key.</h4
                                >
                            </div>
                            <div v-if="pageStep === 4">
                                <h1 class="text-black dark:text-white">Timeout from server</h1>
                                <h4 class="text-black dark:text-white text-sm"
                                    >Please check that the host is on and reachable.</h4
                                >
                            </div>
                            <div v-if="pageStep === 5">
                                <h1 class="text-black dark:text-white">Connection failed</h1>
                            </div>
                            <h4
                                v-if="errorText"
                                class="text-black dark:text-white text-xs break-words"
                                v-text="errorText"
                            />
                            <Button
                                class="w-full"
                                type="button"
                                @click="emit('close')"
                            >
                                Close
                            </Button>
                        </template>
                    </div>
                </div>
            </div>
        </div>
        <div
            class="py-1 sticky bottom-0 space-x-3 flex bg-gray-200 z-50 w-full dark:bg-slate-800 text-dark dark:text-white justify-end pr-2"
        >
            <div class="flex items-center space-x-1">
                <Icon icon="fas fa-microchip" />
                <p v-text="pct(stats.cpu)" />
            </div>
            <div class="flex items-center space-x-1">
                <Icon icon="fas fa-memory" />
                <p v-text="pct(stats.mem)" />
            </div>
            <div class="flex items-center space-x-1">
                <Icon icon="fas fa-hard-drive" />
                <p v-text="pct(stats.disk)" />
            </div>
            <div class="flex items-center space-x-1">
                <Icon icon="fas fa-upload" />
                <p v-text="net(stats.tx_kbps)" />
            </div>
            <div class="flex items-center space-x-1">
                <Icon icon="fas fa-download" />
                <p v-text="net(stats.rx_kbps)" />
            </div>
        </div>
    </div>

    <Dialog
        :show="prompt !== null"
        @close="decide(false)"
    >
        <div
            v-if="prompt"
            class="space-y-3 text-black dark:text-white"
        >
            <h1 class="text-xl font-bold">Unknown host key</h1>
            <p class="text-sm">
                The authenticity of <b>{{ prompt.host }}:{{ prompt.port }}</b> can't be established.
                Check this fingerprint with the server's admin before trusting it.
            </p>
            <p class="text-sm">{{ prompt.algorithm }}</p>
            <p
                class="font-mono text-sm break-all bg-gray-100 dark:bg-slate-800 rounded-md p-3"
                v-text="prompt.fingerprint"
            />
            <div class="space-y-1">
                <Button
                    class="mr-2 mt-3 mb-2 w-full"
                    type="button"
                    @click="decide(true)"
                >
                    Trust and connect
                </Button>
                <Button
                    :class="`!bg-red-500 !hover:bg-red-600`"
                    class="mr-2 mt-3 mb-2 w-full"
                    type="button"
                    @click="decide(false)"
                >
                    Cancel
                </Button>
            </div>
        </div>
    </Dialog>
</template>

<style>
/* WHY: xterm wheel-scrolls via JS anyway; this only hides the native scrollbar (upstream look). */
.xterm .xterm-viewport {
    overflow-y: hidden !important;
}
</style>
