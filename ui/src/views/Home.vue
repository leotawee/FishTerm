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
                class="dark:text-white text-dark"
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
        <p
            v-if="loadError"
            class="text-red-500 text-sm"
            v-text="loadError"
        />
        <p
            v-if="!hosts.length && !loadError"
            class="text-sm text-gray-400 dark:text-gray-500"
        >
            No hosts yet. Click "Add terminal" to create one.
        </p>
        <div class="space-y-3 overflow-y-auto flex-1 min-h-0 my-7 pr-1 scrollbar">
            <Card
                v-for="h in hosts"
                :key="h.id"
            >
                <template #default>
                    <div class="text-slate-800 dark:text-white text-7xl flex items-center mx-6">
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
                    <div class="text-left">
                        <h1
                            class="text-lg font-bold dark:text-white text-dark"
                            v-text="h.name"
                        />
                        <h2
                            class="text-purple-500"
                            v-text="`${h.username}@${h.host}${h.port !== 22 ? `:${h.port}` : ''}`"
                        />
                        <div class="flex mt-3 pt-0 space-x-2">
                            <Button
                                :class="`!bg-green-600 !hover:bg-green-700 !active:bg-green-700 !dark:bg-green-600 !dark:hover:bg-green-700`"
                                @click="emit('connect', h)"
                            >
                                <Icon icon="fas fa-plug" />
                                <p>Connect</p>
                            </Button>
                            <Button @click="openSettings(h)">
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
