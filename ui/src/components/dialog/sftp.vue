<script lang="ts" setup>
// SFTP file browser for a live SSH session. Local paths always come from the
// native dialogs; remote paths come from the listing itself.
import {computed, onMounted, ref} from 'vue';
import {save, open} from '@tauri-apps/plugin-dialog';
import {errorOf, sftpDownload, sftpLs, sftpUpload, type SftpEntry, type SftpProgress} from '../../api';

import Button from '../Button.vue';

const props = defineProps<{sessionId: string}>();
const emit = defineEmits<{close: []}>();

const cwd = ref('');
const entries = ref<SftpEntry[]>([]);
const loading = ref(false);
const error = ref('');
// One row per active transfer; done rows linger until the dialog closes.
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
        entries.value = await sftpLs(props.sessionId, path);
        // Entries carry full paths; the cwd is the shared prefix (dirname of the first).
        const first = entries.value[0]?.path ?? '';
        const cut = first.lastIndexOf('/');
        cwd.value = cut > 0 ? first.slice(0, cut) : path;
    } catch (e) {
        error.value = errorOf(e).message;
    } finally {
        loading.value = false;
    }
}

function track(name: string): [(p: SftpProgress) => void, number] {
    transfers.value.push({name, bytes: 0, total: 1});
    const i = transfers.value.length - 1;
    return [
        p => {
            const t = transfers.value[i];
            if (t) {
                t.bytes = p.bytes;
                t.total = p.total || 1;
            }
        },
        i,
    ];
}

async function download(e: SftpEntry) {
    error.value = '';
    const dest = await save({defaultPath: e.name});
    if (!dest) return;
    const [onProgress] = track(e.name);
    try {
        await sftpDownload(props.sessionId, e.path, dest, onProgress);
    } catch (err) {
        error.value = errorOf(err).message;
    }
}

async function upload() {
    error.value = '';
    const src = await open({multiple: false});
    if (!src || Array.isArray(src)) return;
    const name = src.split(/[\\/]/).pop() || 'file';
    // Remote join: cwd never ends with '/' (it is the home or a canonical dir).
    const remote = `${cwd.value}/${name}`;
    const [onProgress] = track(name);
    try {
        await sftpUpload(props.sessionId, src, remote, onProgress);
        await ls(cwd.value);
    } catch (err) {
        error.value = errorOf(err).message;
    }
}

onMounted(() => ls(''));
</script>

<template>
    <div class="text-left">
        <div class="flex items-center justify-between mb-2">
            <h1 class="text-xl font-bold text-[var(--c-text)]">Files</h1>
            <div class="flex space-x-2">
                <Button
                    type="button"
                    variant="ghost"
                    @click="upload"
                >
                    <Icon icon="fas fa-arrow-up" />
                    Upload
                </Button>
                <Button
                    type="button"
                    variant="ghost"
                    @click="emit('close')"
                >
                    Close
                </Button>
            </div>
        </div>

        <!-- Breadcrumb -->
        <div class="flex items-center flex-wrap gap-1 text-sm text-[var(--c-text)] mb-2 min-h-6">
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
                                class="text-left hover:underline"
                                :title="`Download ${e.name}`"
                                @click="download(e)"
                            >
                                <Icon
                                    icon="fas fa-file"
                                    class="text-[var(--c-muted)]"
                                /> {{ e.name }}
                            </button>
                        </td>
                        <td
                            class="px-2 py-1 text-right text-[var(--c-muted)] whitespace-nowrap"
                        >
                            {{ e.is_dir ? 'dir' : fmtSize(e.size) }}
                        </td>
                    </tr>
                </tbody>
            </table>
        </div>
        <p class="text-xs text-[var(--c-muted)] mt-2">
            Click a folder to open it, a file to download it. Uploads go to the folder you are in.
        </p>
    </div>
</template>
