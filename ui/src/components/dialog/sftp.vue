<script lang="ts" setup>
// SFTP file browser for a live SSH session. Local paths always come from the
// native dialogs; remote paths come from the listing itself.
import {computed, onMounted, ref} from 'vue';
import {save, open} from '@tauri-apps/plugin-dialog';
import {errorOf, sftpDownload, sftpLs, sftpPreview, sftpUpload, type SftpEntry, type SftpProgress} from '../../api';

import Button from '../Button.vue';

const props = defineProps<{sessionId: string}>();

const cwd = ref('');
const entries = ref<SftpEntry[]>([]);
const loading = ref(false);
const error = ref('');
// One transfer at a time (spec security note: keeps channel windowing predictable).
// shortcut: UI-level lock only; the backend permits parallel channels if a future need shows up.
const busy = ref(false);
// One row per transfer this session; done rows linger until the dialog closes.
const transfers = ref<{name: string; bytes: number; total: number}[]>([]);

function fmtSize(n: number) {
    if (n < 1024) return `${n} B`;
    const units = ['KiB', 'MiB', 'GiB'];
    let v = n;
    let u = 0;
    while (v >= 1024 && u < units.length - 1) {
        v /= 1024;
        u++;
    }
    return `${v.toFixed(1)} ${units[u]}`;
}

function fmtTime(unix: number | null) {
    if (!unix) return '';
    return new Date(unix * 1000).toLocaleString(undefined, {dateStyle: 'short', timeStyle: 'short'});
}

/** `''` is the remote home; render it as `~`. */
const crumbs = computed(() => {
    if (!cwd.value) return [];
    const parts = cwd.value.split('/').filter(Boolean);
    return parts.map((p, i) => ({name: p, path: '/' + parts.slice(0, i + 1).join('/')}));
});

const parentPath = computed(() => {
    const i = cwd.value.lastIndexOf('/');
    return i <= 0 ? '/' : cwd.value.slice(0, i);
});

async function ls(path: string) {
    loading.value = true;
    error.value = '';
    try {
        // Server-canonical cwd; correct even when the directory is empty.
        const listing = await sftpLs(props.sessionId, path);
        entries.value = listing.entries;
        cwd.value = listing.cwd;
    } catch (e) {
        error.value = errorOf(e).message;
    } finally {
        loading.value = false;
    }
}

function addTransfer(name: string): (p: SftpProgress) => void {
    transfers.value.push({name, bytes: 0, total: 1});
    const i = transfers.value.length - 1;
    return p => {
        const t = transfers.value[i];
        if (t) {
            t.bytes = p.bytes;
            t.total = p.total || 1;
        }
    };
}

async function download(e: SftpEntry) {
    if (busy.value) return;
    error.value = '';
    const dest = await save({defaultPath: e.name}).catch(() => null);
    if (!dest) return;
    busy.value = true;
    try {
        await sftpDownload(props.sessionId, e.path, dest, addTransfer(e.name));
    } catch (err) {
        error.value = errorOf(err).message;
    } finally {
        busy.value = false;
    }
}

async function preview(e: SftpEntry) {
    if (busy.value) return;
    error.value = '';
    busy.value = true;
    try {
        await sftpPreview(props.sessionId, e.path, addTransfer(e.name));
    } catch (err) {
        error.value = errorOf(err).message;
    } finally {
        busy.value = false;
    }
}

async function upload() {
    if (busy.value) return;
    error.value = '';
    const src = await open({multiple: false});
    if (!src || Array.isArray(src)) return;
    const name = src.split(/[\\/]/).pop() || 'file';
    const remote = `${cwd.value}/${name}`;
    busy.value = true;
    try {
        await sftpUpload(props.sessionId, src, remote, addTransfer(name));
        await ls(cwd.value);
    } catch (err) {
        error.value = errorOf(err).message;
    } finally {
        busy.value = false;
    }
}

onMounted(() => ls(''));
</script>

<template>
    <div class="text-left">
        <div class="flex items-center justify-between mb-2">
            <!-- Title row keeps clear of the dialog's corner X. -->
            <h1
                class="text-xl font-bold text-[var(--c-text)] pr-10"
                >Files</h1
            >
        </div>

        <!-- Breadcrumb + actions row: Upload sits here, one row below the X. -->
        <div class="flex items-center justify-between gap-2 mb-2">
            <div class="flex items-center flex-wrap gap-1 text-sm text-[var(--c-text)] min-w-0">
            <button
                class="hover:underline text-[var(--c-accent)]"
                @click="ls('')"
                v-text="'~'"
            />
            <template
                v-for="c in crumbs"
                :key="c.path"
            >
                <span class="text-[var(--c-muted)]">/</span>
                <button
                    class="hover:underline text-[var(--c-accent)]"
                    @click="ls(c.path)"
                    v-text="c.name"
                />
            </template>
            </div>
            <Button
                type="button"
                variant="ghost"
                class="shrink-0"
                :disabled="busy"
                title="Upload into the folder you are in"
                @click="upload"
            >
                <Icon icon="fas fa-arrow-up" />
                Upload
            </Button>
        </div>

        <p
            v-if="error"
            class="text-sm text-red-500 mb-2"
            v-text="error"
        />

        <!-- Transfers -->
        <div
            v-if="transfers.length"
            class="space-y-1 mb-3"
        >
            <div
                v-for="(t, i) in transfers"
                :key="i"
                class="text-xs text-[var(--c-text)]"
            >
                <div class="flex justify-between">
                    <span class="truncate">{{ t.bytes === t.total ? '✓' : '⇅' }} {{ t.name }}</span>
                    <span class="text-[var(--c-muted)]">{{ fmtSize(t.bytes) }} / {{ fmtSize(t.total) }}</span>
                </div>
                <div class="h-1 bg-[var(--c-ghost-hover)] rounded mt-0.5">
                    <div
                        class="h-1 bg-[var(--c-accent)] rounded"
                        :style="{width: `${Math.min(100, (t.bytes / t.total) * 100)}%`}"
                    />
                </div>
            </div>
        </div>

        <!-- Listing -->
        <div class="max-h-72 overflow-y-auto scrollbar border border-[var(--c-border)] rounded">
            <p
                v-if="loading"
                class="p-3 text-sm text-[var(--c-muted)]"
            >
                Loading…
            </p>
            <p
                v-else-if="!entries.length"
                class="p-3 text-sm text-[var(--c-muted)]"
            >
                Empty directory.
            </p>
            <table
                v-else
                class="w-full text-sm text-[var(--c-text)]"
            >
                <tbody>
                    <tr>
                        <td class="px-2 py-1">
                            <button
                                class="hover:underline text-[var(--c-accent)]"
                                @click="ls(parentPath)"
                            >
                                <Icon icon="fas fa-level-up-alt" /> ..
                            </button>
                        </td>
                        <td />
                    </tr>
                    <tr
                        v-for="e in entries"
                        :key="e.path"
                        class="hover:bg-[var(--c-ghost-hover)]"
                    >
                        <td class="px-2 py-1">
                            <button
                                v-if="e.is_dir"
                                class="text-left hover:underline text-[var(--c-accent)]"
                                @click="ls(e.path)"
                            >
                                <Icon icon="fas fa-folder" /> {{ e.name }}
                            </button>
                            <button
                                v-else
                                class="text-left hover:underline disabled:opacity-50"
                                :disabled="busy"
                                :title="`Download ${e.name}`"
                                @click="download(e)"
                            >
                                <Icon
                                    icon="fas fa-file"
                                    class="text-[var(--c-muted)]"
                                /> {{ e.name }}
                            </button>
                        </td>
                        <td class="px-2 py-1 text-right whitespace-nowrap">
                            <button
                                v-if="!e.is_dir"
                                class="text-[var(--c-muted)] hover:text-[var(--c-accent)] disabled:opacity-50 px-1"
                                :disabled="busy"
                                :aria-label="`Preview ${e.name}`"
                                title="Preview (Quick Look)"
                                @click="preview(e)"
                            >
                                <Icon icon="fas fa-eye" />
                            </button>
                        </td>
                        <td
                            class="px-2 py-1 text-right text-[var(--c-muted)] whitespace-nowrap"
                            v-text="e.is_dir ? 'dir' : fmtSize(e.size)"
                        />
                        <td
                            class="px-2 py-1 text-right text-[var(--c-muted)] whitespace-nowrap"
                            v-text="fmtTime(e.modified)"
                        />
                    </tr>
                </tbody>
            </table>
        </div>
        <p class="text-xs text-[var(--c-muted)] mt-2">
            Click a folder to open it · click a file to download it ·
            <Icon
                icon="fas fa-eye"
                class="text-[var(--c-muted)]"
            />
            previews a file in macOS Quick Look (copied to a temp folder first) ·
            uploads go to the folder you are in.
        </p>
    </div>
</template>
