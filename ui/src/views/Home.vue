<script lang="ts" setup>
// Components
import Container from '../components/Container.vue';
import Card from '../components/Card.vue';
import Title from '../components/Title.vue';
import Button from '../components/Button.vue';
import DialogVue from '../components/Dialog.vue';
// Dialog
import SettingDialog from '../components/dialog/settings.vue';

// Main
import {onMounted, ref} from 'vue';
import {hostsList, errorOf, type HostView} from '../api';
import {isImageIcon, osIcon} from '../osIcons';

const emit = defineEmits<{connect: [host: HostView]}>();

const showModal = ref(false);
const hosts = ref<HostView[]>([]);
const settingHost = ref<HostView | null>(null);
const actionType = ref<'add' | 'edit'>('add');
const loadError = ref('');

async function loadHosts() {
    try {
        hosts.value = await hostsList();
        loadError.value = '';
    } catch (e) {
        loadError.value = errorOf(e).message;
    }
}

function addTerminal() {
    actionType.value = 'add';
    settingHost.value = null;
    showModal.value = true;
}

function openSettings(host: HostView) {
    actionType.value = 'edit';
    settingHost.value = host;
    showModal.value = true;
}

onMounted(loadHosts);
</script>

<template>
    <Container class="flex flex-col h-full">
        <div class="flex justify-between">
            <Title
                class="text-[var(--c-text)]"
                title="SSH list"
            />
            <Button
                class="mr-2 mb-2"
                @click="addTerminal"
            >
                <Icon icon="fas fa-plus" />
                Add terminal
            </Button>
        </div>
        <div
            v-if="loadError"
            class="text-xs text-red-600 dark:text-red-400"
        >
            <p v-text="loadError" />
            <Button
                type="button"
                class="mt-2"
                @click="loadHosts"
            >
                Retry
            </Button>
        </div>
        <p
            v-if="!hosts.length && !loadError"
            class="text-sm text-[var(--c-muted)]"
        >
            No saved hosts yet — click Add to create your first connection.
        </p>
        <div class="space-y-3 overflow-y-auto flex-1 min-h-0 my-7 pr-1 scrollbar">
            <Card
                v-for="h in hosts"
                :key="h.id"
            >
                <template #default>
                    <div class="text-[var(--c-text)] text-7xl flex items-center mx-6">
                        <img
                            v-if="isImageIcon(osIcon(h.os))"
                            :src="osIcon(h.os)"
                            class="w-[72px] h-[72px]"
                            alt=""
                        />
                        <Icon
                            v-else
                            :icon="osIcon(h.os)"
                        />
                    </div>
                    <div class="text-left min-w-0">
                        <h1
                            class="text-lg font-bold text-[var(--c-text)] break-words"
                            v-text="h.name"
                        />
                        <h2
                            class="text-[var(--c-accent)] min-w-0 truncate"
                            v-text="`${h.username}@${h.host}${h.port !== 22 ? `:${h.port}` : ''}`"
                        />
                        <span
                            class="inline-flex mt-1 text-[10px] font-semibold uppercase tracking-wide px-1.5 py-0.5 rounded"
                            :class="h.transport === 'cloudflare_access'
                                ? 'bg-amber-100 text-amber-700 dark:bg-amber-500/10 dark:text-amber-400'
                                : 'bg-slate-100 text-slate-500 dark:bg-slate-800 dark:text-slate-400'"
                            v-text="h.transport === 'cloudflare_access' ? 'Cloudflare' : 'SSH'"
                        />
                        <div class="flex mt-3 pt-0 space-x-2">
                            <Button @click="emit('connect', h)">
                                <Icon icon="fas fa-plug" />
                                <p>Connect</p>
                            </Button>
                            <Button
                                variant="ghost"
                                @click="openSettings(h)"
                            >
                                <Icon icon="fas fa-cogs" />
                                <p>Settings</p>
                            </Button>
                        </div>
                    </div>
                </template>
            </Card>
        </div>
    </Container>
    <DialogVue
        :show="showModal"
        label="Host settings"
        @close="showModal = false"
    >
        <SettingDialog
            :host="settingHost"
            :actionType="actionType"
            @closePage="showModal = false"
            @changed="loadHosts"
        />
    </DialogVue>
</template>
