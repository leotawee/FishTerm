<script lang="ts" setup>
import {hostSave, hostSetSecret, hostDelete, errorOf, type HostView, type HostInput} from '../../api';

import Title from '../Title.vue';
import Text from '../input/text.vue';
import Select from '../input/select.vue';
import File from '../input/file.vue';
import Button from '../Button.vue';

import Dialog from '../Dialog.vue';

import {computed, onMounted, reactive, ref} from 'vue';

const props = defineProps<{host: HostView | null; actionType: 'add' | 'edit'}>();
const emit = defineEmits(['closePage', 'changed']);

// Password (type 0) or key file path (type 1). Saved passwords are never loaded back.
const previewPasswordData = ref('');
const previousPasswordType = ref(0);
const previousPasswordData = ref('');
const passphrase = ref('');

const previewConnectionData = ref('');
const previousConnectionType = ref(0);
const previousConnectionData = ref('');

const showModal = ref(false);
const error = ref('');

const config = reactive({
    data: {
        title: '',
        connection: {
            host: '',
            port: 22,
            type: 0,
            username: '',
            password: {
                type: 0,
            },
        },
    },
});

const selectAuthType = ref([
    {
        name: 'Password',
        value: 0,
        icon: 'fas fa-key',
    },
    {
        name: 'Certificate File',
        value: 1,
        icon: 'fas fa-certificate',
    },
]);

const selectConnectionType = ref([
    {
        name: 'SSH Normal',
        value: 0,
        icon: 'fas fa-terminal',
    },
    {
        name: 'Cloudflare Tunnel',
        value: 1,
        icon: 'fab fa-cloudflare',
    },
]);

// A secret already in the keychain still fits only if the auth type is unchanged.
const hasSavedSecret = computed(
    () =>
        props.actionType === 'edit' &&
        !!props.host?.has_secret &&
        config.data.connection.password.type === previousPasswordType.value,
);

async function updateSSH(e: Event) {
    e.preventDefault();
    error.value = '';

    const isKey = config.data.connection.password.type === 1;
    const isTunnel = config.data.connection.type === 1;
    if (isKey && !previewPasswordData.value) {
        error.value = 'Please choose a certificate file.';
        return;
    }

    const input: HostInput = {
        id: props.actionType === 'edit' ? props.host?.id : null,
        name: config.data.title,
        host: previewConnectionData.value.trim(),
        port: isTunnel ? 22 : Number(config.data.connection.port),
        username: config.data.connection.username.trim(),
        transport: isTunnel ? 'cloudflare_access' : 'direct',
        auth: isKey ? {kind: 'key', path: previewPasswordData.value} : {kind: 'password'},
    };

    try {
        const saved = await hostSave(input);
        const secret = isKey ? passphrase.value : previewPasswordData.value;
        if (secret) await hostSetSecret(saved.id, secret);
        emit('changed');
        emit('closePage');
    } catch (err) {
        error.value = errorOf(err).message;
    }
}

async function deleteConfig() {
    if (!props.host) return;
    try {
        await hostDelete(props.host.id);
        showModal.value = false;
        emit('changed');
        emit('closePage');
    } catch (err) {
        showModal.value = false;
        error.value = errorOf(err).message;
    }
}

function updatePrivateKey(path: string) {
    previewPasswordData.value = path;
}

function onUpdatePasswordSelect(value: number) {
    passphrase.value = '';
    if (value != previousPasswordType.value) previewPasswordData.value = '';
    else previewPasswordData.value = previousPasswordData.value;
}

function onUpdateConnectionSelect(value: number) {
    if (value != previousConnectionType.value) previewConnectionData.value = '';
    else previewConnectionData.value = previousConnectionData.value;
}

onMounted(() => {
    const h = props.host;
    if (props.actionType !== 'edit' || !h) return;
    config.data.title = h.name;
    config.data.connection.type = h.transport === 'cloudflare_access' ? 1 : 0;
    config.data.connection.port = h.port;
    config.data.connection.username = h.username;
    config.data.connection.password.type = h.auth.kind === 'key' ? 1 : 0;

    previousConnectionType.value = config.data.connection.type;
    previewConnectionData.value = h.host;
    previousConnectionData.value = h.host;

    previousPasswordType.value = config.data.connection.password.type;
    previewPasswordData.value = h.auth.kind === 'key' ? h.auth.path : '';
    previousPasswordData.value = previewPasswordData.value;
});
</script>

<template>
    <Title
        title="Setting"
        icon="settings"
    />
    <hr class="m-3" />
    <form
        @submit="updateSSH"
        class="space-y-3"
    >
        <Text
            v-model="config.data.title"
            name="Name host"
            placeholder="Ex. My awsome SSH server 😎"
            :required="true"
        />
        <Select
            @onChange="onUpdateConnectionSelect"
            v-model="config.data.connection.type"
            name="Connection Type"
            :data="selectConnectionType"
        />
        <div
            class="grid grid-cols-2 gap-4"
            v-if="config.data.connection.type === 0"
        >
            <Text
                v-model="previewConnectionData"
                name="SSH Host"
                placeholder="localhost"
                :required="true"
            />
            <Text
                v-model.number="config.data.connection.port"
                name="SSH Port"
                placeholder="22"
                :required="true"
            />
        </div>
        <div v-if="config.data.connection.type === 1">
            <Text
                v-model="previewConnectionData"
                name="URL Cloudflare Tunnel"
                placeholder="ssh.example.dev"
                :required="true"
            />
        </div>
        <Text
            v-model="config.data.connection.username"
            name="Username"
            placeholder="root"
            :required="true"
        />
        <Select
            v-model="config.data.connection.password.type"
            @onChange="onUpdatePasswordSelect"
            name="Login type"
            :data="selectAuthType"
        />

        <!-- Login type (Password) -->
        <Text
            v-model="previewPasswordData"
            type="password"
            name="Password"
            :placeholder="hasSavedSecret ? 'Saved in keychain. Leave blank to keep it.' : ''"
            v-if="config.data.connection.password.type === 0"
            :required="!hasSavedSecret"
        />
        <!-- Login type (Private key) -->
        <template v-if="config.data.connection.password.type === 1">
            <File
                :path="previewPasswordData"
                @updatePath="updatePrivateKey"
                name="Certificate"
                text="Certificate File (id_rsa, cert.pem, etc.)"
            />
            <Text
                v-model="passphrase"
                type="password"
                name="Passphrase"
                :placeholder="
                    hasSavedSecret ? 'Saved in keychain. Leave blank to keep it.' : 'Only if the key is encrypted'
                "
            />
        </template>

        <p
            v-if="error"
            class="text-red-500 text-sm"
            v-text="error"
        />

        <Button
            class="mr-2 mt-3 mb-2 w-full"
            type="submit"
        >
            {{ props.actionType === 'add' ? 'Add' : 'Update' }} terminal
        </Button>
        <Button
            v-if="props.actionType !== 'add'"
            @click="showModal = true"
            :class="`!bg-red-500`"
            class="mr-2 mt-3 mb-2 w-full"
            type="button"
        >
            Delete terminal
        </Button>
        <Dialog
            :show="showModal"
            @close="showModal = false"
        >
            <div>
                <p class="text-black dark:text-white">Are you sure delete this config?</p>
                <div class="space-y-1">
                    <Button
                        v-if="props.actionType !== 'add'"
                        :class="`!bg-red-500`"
                        class="mr-2 mt-3 mb-2 w-full"
                        type="button"
                        @click="deleteConfig"
                    >
                        Delete it!
                    </Button>
                    <Button
                        class="mr-2 mt-3 mb-2 w-full"
                        type="button"
                        @click="showModal = false"
                    >
                        Cancel
                    </Button>
                </div>
            </div>
        </Dialog>
    </form>
</template>
