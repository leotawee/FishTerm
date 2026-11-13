<script lang="ts" setup>
import {nextTick, onBeforeUnmount, onMounted, ref, watch} from 'vue';
import {Terminal} from '@xterm/xterm';
import {FitAddon} from '@xterm/addon-fit';
import {WebLinksAddon} from '@xterm/addon-web-links';
import {ask} from '@tauri-apps/plugin-dialog';
import {writeText} from '@tauri-apps/plugin-clipboard-manager';
import {getTermTheme} from '../appearance';

import Button from '../components/Button.vue';
import Dialog from '../components/Dialog.vue';
import SftpDialog from '../components/dialog/sftp.vue';
import {
    sessionOpen,
    sessionWrite,
    sessionResize,
    sessionClose,
    sessionStats,
    hostkeyDecide,
    urlOpen,
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
    theme: {...getTermTheme()},
    fontFamily: 'Fira Code, monospace',
});

// Re-theme live when the Appearance dialog changes the terminal color scheme.
const applyTermTheme = () => {
    term.options.theme = {...getTermTheme()};
};
addEventListener('appearance-changed', applyTermTheme);
onBeforeUnmount(() => removeEventListener('appearance-changed', applyTermTheme));
const fitAddon = new FitAddon();

// OSC 52: the remote writes OUR clipboard (yank inside vim/micro over SSH).
// Write-only by design — the query form ('?') is swallowed, never answered.
// shortcut: Allow is remembered per session only; per-host memory if abuse shows up.
let osc52Allowed: boolean | null = null;

function decodeOsc52(b64: string): string {
    const bin = atob(b64.replace(/-/g, '+').replace(/_/g, '/'));
    return new TextDecoder().decode(Uint8Array.from(bin, c => c.charCodeAt(0)));
}

term.parser.registerOscHandler(52, (data: string) => {
    const b64 = data.slice(data.indexOf(';') + 1);
    if (!b64 || b64 === '?' || b64.length > 100_000) return true;
    try {
        const text = decodeOsc52(b64);
        if (osc52Allowed === null) {
            ask('The remote host wants to set your clipboard. Allow?', {
                title: 'Clipboard access',
                okLabel: 'Allow',
                cancelLabel: 'Deny',
            })
                .then(ok => {
                    osc52Allowed = ok;
                    if (ok) void writeText(text).catch(() => {});
                })
                .catch(() => {});
        } else if (osc52Allowed) {
            void writeText(text).catch(() => {});
        }
    } catch {
        // malformed base64 — swallow it, keep the session alive
    }
    return true;
});

const termDiv = ref<HTMLDivElement | null>(null);
const isShow = ref(false);

// Modal loading. Steps: 0 connecting, 3 auth failed, 4 timeout, 5 other error, 6 host key changed.
const isLoading = ref(true);
const pageStep = ref(0);
const errorText = ref('');
const prompt = ref<(HostKeyPrompt & {id: string}) | null>(null);

// Cloudflare Access sign-in progress, shown under the connecting status.
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

// Elapsed connect time (m:ss), shown next to the status line; restarted by Retry.
const elapsedText = ref('0:00');
let startedAt = 0;
let elapsedTimer = 0;
function startElapsed() {
    stopElapsed();
    startedAt = Date.now();
    elapsedText.value = '0:00';
    elapsedTimer = window.setInterval(() => {
        const s = Math.floor((Date.now() - startedAt) / 1000);
        elapsedText.value = `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
    }, 1000);
}
function stopElapsed() {
    if (elapsedTimer) clearInterval(elapsedTimer);
    elapsedTimer = 0;
}

// Footer monitor. First sample has no delta yet, so rates stay `--` for ~3s.
const stats = ref<SessionStats>({cpu: null, mem: null, disk: null, rx_kbps: null, tx_kbps: null});
let poller = 0;
let polling = false;
const pct = (v: number | null) => (v === null ? '--%' : `${Math.round(v)}%`);
const net = (v: number | null) => (v === null ? '--' : `${Math.round(v)}K/s`);
// Hover hint for metrics that have no sample yet (first ~3s or non-Linux host).
const NO_DATA_TITLE = 'No data for this host (non-Linux or still sampling)';

// Color encodes load level; the matching word goes into the tooltip so the
// signal is never color-only.
function levelClass(v: number | null, warn: number, crit: number) {
    if (v === null) return '';
    if (v >= crit) return 'text-red-500 dark:text-red-400';
    if (v >= warn) return 'text-amber-500 dark:text-amber-400';
    return '';
}

function levelWord(v: number | null, warn: number, crit: number) {
    if (v === null) return NO_DATA_TITLE;
    if (v >= crit) return 'critical';
    if (v >= warn) return 'high';
    return 'normal';
}
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
const showSftp = ref(false);
// One-line notice per failing forward rule; cleared when the session closes.
const forwardErrors = ref<string[]>([]);

function fail(err: AppError) {
    stopElapsed();
    isShow.value = false;
    isLoading.value = false;
    prompt.value = null;
    cfStatus.value = '';
    pageStep.value =
        err.code === 'auth_failed' ? 3 : err.code === 'timeout' ? 4 : err.code === 'host_key_changed' ? 6 : 5;
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
    } else if (msg.type === 'forward-error') {
        const line = `Port ${msg.local_port}: the server refused to open the tunnel.`;
        if (!forwardErrors.value.includes(line)) forwardErrors.value.push(line);
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
    // Custom handler: xterm's default uses window.open, which Tauri's webview blocks.
    term.loadAddon(new WebLinksAddon((_e, uri) => void urlOpen(uri).catch(() => {})));
    if (termDiv.value) {
        term.open(termDiv.value);
        // Cmd+C copies the visible selection (Cmd+V paste is handled by xterm itself).
        // Cmd+Shift+S opens the SFTP browser for the live session.
        termDiv.value?.addEventListener(
            'keydown',
            e => {
                if (e.metaKey && !e.shiftKey && e.key === 'c' && term.hasSelection()) {
                    e.preventDefault();
                    e.stopPropagation();
                    void writeText(term.getSelection()).catch(() => {});
                } else if (e.metaKey && e.shiftKey && e.key.toLowerCase() === 's' && sessionId) {
                    e.preventDefault();
                    e.stopPropagation();
                    showSftp.value = true;
                }
            },
            true,
        );
        observer.observe(termDiv.value);
    }
    fitAddon.fit();
    term.onData(data => {
        if (sessionId) sessionWrite(sessionId, data).catch(() => {});
    });

    await connect();
});

// One connect attempt; doubles as the Retry path (resets the modal state first).
async function connect() {
    isLoading.value = true;
    pageStep.value = 0;
    errorText.value = '';
    cfStatus.value = '';
    forwardErrors.value = [];
    startElapsed();
    try {
        sent = `${term.cols}x${term.rows}`;
        sessionId = await sessionOpen(props.host.id, term.cols, term.rows, onEvent);
        cfStatus.value = '';
        stopElapsed();
        isShow.value = true;
        startStats();
        await nextTick();
        refit();
        term.focus();
    } catch (e) {
        fail(errorOf(e));
    }
}

watch(
    () => props.active,
    active => {
        if (active) nextTick(() => (refit(), term.focus()));
    },
);

onBeforeUnmount(() => {
    unmounting = true;
    stopStats();
    stopElapsed();
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
                class="absolute inset-0 flex items-center justify-center mx-auto z-20 bg-slate-900/70"
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
                                class="inline mr-2 w-8 h-8 text-gray-200 animate-spin dark:text-gray-600 fill-[var(--c-accent)]"
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
                            <h1 class="text-[var(--c-text)]"
                                >Connecting to {{ props.host.username }}@{{ props.host.host }}...</h1
                            >
                            <p
                                class="text-[var(--c-text)] text-xs mt-1"
                                v-text="elapsedText"
                            />
                            <h4
                                v-if="cfStatus"
                                class="text-[var(--c-text)] text-sm mt-2"
                            >
                                {{ CF_STATUS_TEXT[cfStatus] || cfStatus
                                }}<template v-if="cfStatus === 'waiting'">
                                    ({{ cfAttempt }}/10)</template
                                >
                            </h4>
                            <div v-if="isLoading && !prompt" class="mt-3">
                                <Button
                                    :class="`!bg-slate-200 hover:!bg-slate-300 !text-slate-700 dark:!bg-slate-700 dark:hover:!bg-slate-600 dark:!text-white`"
                                    class="w-full"
                                    type="button"
                                    @click="emit('close')"
                                >
                                    Cancel
                                </Button>
                            </div>
                        </div>
                        <template v-if="pageStep >= 3">
                            <template v-if="pageStep !== 6">
                                <div v-if="pageStep === 3">
                                    <h1 class="text-[var(--c-text)]">Authentication failed</h1>
                                    <h4 class="text-[var(--c-text)] text-sm"
                                        >Please check your username, password or key.</h4
                                    >
                                </div>
                                <div v-if="pageStep === 4">
                                    <h1 class="text-[var(--c-text)]">Timeout from server</h1>
                                    <h4 class="text-[var(--c-text)] text-sm"
                                        >Please check that the host is on and reachable.</h4
                                    >
                                </div>
                                <div v-if="pageStep === 5">
                                    <h1 class="text-[var(--c-text)]">Connection failed</h1>
                                    <h4 class="text-[var(--c-text)] text-sm"
                                        >Check the host, port and your network, then retry.</h4
                                    >
                                </div>
                                <h4
                                    v-if="errorText"
                                    class="text-[var(--c-text)] text-xs break-words line-clamp-6"
                                    role="alert"
                                    v-text="errorText"
                                />
                                <div class="flex space-x-2">
                                    <Button
                                        class="w-full"
                                        type="button"
                                        @click="emit('close')"
                                    >
                                        Close
                                    </Button>
                                    <Button
                                        class="w-full"
                                        type="button"
                                        @click="connect"
                                    >
                                        Retry
                                    </Button>
                                </div>
                            </template>
                            <div
                                v-else
                                class="w-full bg-red-50 dark:bg-red-950/40 border border-red-300 dark:border-red-800 rounded-md p-4 text-left"
                            >
                                <h1 class="text-[var(--c-text)] font-bold">Host key changed</h1>
                                <p
                                    class="text-[var(--c-text)] text-sm mt-2 break-words"
                                    v-text="errorText"
                                />
                                <div class="flex space-x-2 mt-3">
                                    <Button
                                        :class="`!bg-slate-200 hover:!bg-slate-300 !text-slate-700 dark:!bg-slate-700 dark:hover:!bg-slate-600 dark:!text-white`"
                                        class="w-full"
                                        type="button"
                                        @click="emit('close')"
                                    >
                                        Close
                                    </Button>
                                    <Button
                                        class="w-full"
                                        type="button"
                                        @click="connect"
                                    >
                                        Retry
                                    </Button>
                                </div>
                            </div>
                        </template>
                    </div>
                </div>
            </div>
        </div>
        <div
            v-if="forwardErrors.length"
            class="px-3 py-1 text-xs text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-950/40"
        >
            <p
                v-for="line in forwardErrors"
                :key="line"
                role="alert"
                v-text="line"
            />
        </div>
        <div
            class="py-1 sticky bottom-0 space-x-3 flex bg-[var(--c-card)] z-50 w-full text-[var(--c-text)] justify-end pr-2"
        >
            <Button
                v-if="isShow"
                variant="ghost"
                class="!mr-auto"
                title="Browse remote files (⌘⇧S)"
                @click="showSftp = true"
            >
                <Icon icon="fas fa-folder-open" />
                Files
            </Button>
            <div
                class="flex items-center space-x-1"
                aria-label="CPU usage"
                :title="stats.cpu === null ? NO_DATA_TITLE : `${Math.round(stats.cpu)}% — ${levelWord(stats.cpu, 70, 90)}`"
            >
                <Icon icon="fas fa-microchip" />
                <p :class="levelClass(stats.cpu, 70, 90)" v-text="pct(stats.cpu)" />
            </div>
            <div
                class="flex items-center space-x-1"
                aria-label="Memory usage"
                :title="stats.mem === null ? NO_DATA_TITLE : `${Math.round(stats.mem)}% — ${levelWord(stats.mem, 80, 95)}`"
            >
                <Icon icon="fas fa-memory" />
                <p :class="levelClass(stats.mem, 80, 95)" v-text="pct(stats.mem)" />
            </div>
            <div
                class="flex items-center space-x-1"
                aria-label="Disk usage"
                :title="stats.disk === null ? NO_DATA_TITLE : `${stats.disk}% — ${levelWord(stats.disk, 80, 90)}`"
            >
                <Icon icon="fas fa-hard-drive" />
                <p :class="levelClass(stats.disk, 80, 90)" v-text="pct(stats.disk)" />
            </div>
            <div
                class="flex items-center space-x-1"
                aria-label="Upload speed"
                :title="stats.tx_kbps === null ? NO_DATA_TITLE : undefined"
            >
                <Icon icon="fas fa-upload" />
                <p v-text="net(stats.tx_kbps)" />
            </div>
            <div
                class="flex items-center space-x-1"
                aria-label="Download speed"
                :title="stats.rx_kbps === null ? NO_DATA_TITLE : undefined"
            >
                <Icon icon="fas fa-download" />
                <p v-text="net(stats.rx_kbps)" />
            </div>
        </div>
    </div>

    <Dialog
        :show="prompt !== null"
        label="Unknown host key"
        @close="decide(false)"
    >
        <div
            v-if="prompt"
            class="space-y-3 text-[var(--c-text)]"
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
                    :class="`!bg-slate-200 hover:!bg-slate-300 !text-slate-700 dark:!bg-slate-700 dark:hover:!bg-slate-600 dark:!text-white`"
                    class="mr-2 mt-3 mb-2 w-full"
                    type="button"
                    @click="decide(false)"
                >
                    Cancel
                </Button>
            </div>
        </div>
    </Dialog>

    <Dialog
        :show="showSftp"
        label="Remote files"
        @close="showSftp = false"
    >
        <SftpDialog
            v-if="sessionId"
            :sessionId="sessionId"
            @close="showSftp = false"
        />
    </Dialog>
</template>

<style>
/* WHY: xterm wheel-scrolls via JS anyway; this only hides the native scrollbar (upstream look). */
.xterm .xterm-viewport {
    overflow-y: hidden !important;
}
</style>
