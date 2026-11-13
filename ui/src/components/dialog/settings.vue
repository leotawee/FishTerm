<script lang="ts" setup>
import {
    hostClearSecret,
    hostSave,
    hostSetSecret,
    hostDelete,
    errorOf,
    type HostView,
    type HostInput,
    type Os,
    type PortForward,
} from '../../api';
import {osIcon} from '../../osIcons';

import Title from '../Title.vue';
import Text from '../input/text.vue';
import Select from '../input/select.vue';
import File from '../input/file.vue';
import Button from '../Button.vue';

import Dialog from '../Dialog.vue';

import {computed, onMounted, reactive, ref, useId} from 'vue';

const props = defineProps<{host: HostView | null; actionType: 'add' | 'edit'}>();
const emit = defineEmits(['closePage', 'changed']);

// Password (type 0) or key file path (type 1). Saved passwords are never loaded back.
const previewPasswordData = ref('');
const previousPasswordType = ref(0);
// Entered secret kept per login type, so switching type never loses typed text and a
// saved key path never leaks into the password field.
const secretSlots = reactive<Record<number, string>>({0: '', 1: ''});
// Login type whose value currently lives in previewPasswordData.
const activeSecretType = ref(0);
const passphrase = ref('');

const previewConnectionData = ref('');
const previousConnectionType = ref(0);
// Host value kept per connection type, so switching type doesn't wipe what was typed.
const connectionSlots = reactive<Record<number, string>>({0: '', 1: ''});

const showModal = ref(false);
const error = ref('');
// Hide the forget button after clearing; `props.host` stays stale until the list reloads.
const secretCleared = ref(false);

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
        os: 'generic' as Os,
        group: '',
        forwards: [] as PortForward[],
    },
});

// Suggested sections; the field is free text, so anything else works too.
const GROUP_PRESETS = ['Homelab', 'Network', 'Cloud', 'Work', 'IoT'];
const groupPresetsId = useId();

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

const selectOsType = ref(
    (
        [
            {name: 'Generic', value: 'generic'},
            {name: 'Linux (other)', value: 'linux'},
            {name: 'Debian', value: 'debian'},
            {name: 'Ubuntu', value: 'ubuntu'},
            {name: 'Raspberry Pi OS', value: 'raspberry_pi_os'},
            {name: 'Red Hat', value: 'redhat'},
            {name: 'CentOS', value: 'centos'},
            {name: 'Fedora', value: 'fedora'},
            {name: 'Amazon Linux', value: 'amazon'},
            {name: 'openSUSE', value: 'opensuse'},
            {name: 'SUSE', value: 'suse'},
            {name: 'Arch Linux', value: 'arch_linux'},
            {name: 'FreeBSD', value: 'freebsd'},
            {name: 'macOS', value: 'apple'},
            {name: 'Windows', value: 'windows'},
        ] as Array<{name: string; value: Os}>
    ).map(o => ({...o, icon: osIcon(o.value)})),
);

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
        os: config.data.os,
        group: config.data.group.trim(),
        forwards: config.data.forwards,
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

async function forgetSecret() {
    if (!props.host) return;
    try {
        await hostClearSecret(props.host.id);
        secretCleared.value = true;
    } catch (err) {
        error.value = errorOf(err).message;
    }
}

function updatePrivateKey(path: string) {
    previewPasswordData.value = path;
}

function onUpdatePasswordSelect(value: number) {
    // Stash the secret into the type being left, then load the target type's slot.
    // previousPasswordType stays mount-time so hasSavedSecret keeps its meaning.
    secretSlots[activeSecretType.value] = previewPasswordData.value;
    previewPasswordData.value = secretSlots[value] ?? '';
    activeSecretType.value = value;
    passphrase.value = '';
}

function onUpdateConnectionSelect(value: number) {
    connectionSlots[previousConnectionType.value] = previewConnectionData.value;
    previewConnectionData.value = connectionSlots[value] ?? '';
    previousConnectionType.value = value;
}

onMounted(() => {
    const h = props.host;
    if (props.actionType !== 'edit' || !h) return;
    config.data.title = h.name;
    config.data.connection.type = h.transport === 'cloudflare_access' ? 1 : 0;
    config.data.connection.port = h.port;
    config.data.connection.username = h.username;
    config.data.connection.password.type = h.auth.kind === 'key' ? 1 : 0;
    config.data.os = h.os;
    config.data.group = h.group;
    config.data.forwards = h.forwards.map(f => ({...f}));

    previousConnectionType.value = config.data.connection.type;
    previewConnectionData.value = h.host;
    connectionSlots[config.data.connection.type] = h.host;

    previousPasswordType.value = config.data.connection.password.type;
    activeSecretType.value = config.data.connection.password.type;
    previewPasswordData.value = h.auth.kind === 'key' ? h.auth.path : '';
    secretSlots[config.data.connection.password.type] = previewPasswordData.value;
});
</script>

<template>
    <Title title="Settings" />
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
                type="number"
                placeholder="22"
                min="1"
                max="65535"
                :required="true"
            />
        </div>
        <div v-if="config.data.connection.type === 1">
            <Text
                v-model="previewConnectionData"
                name="Cloudflare hostname"
                placeholder="ssh.example.com"
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
            v-model="config.data.os"
            name="Operating System"
            :data="selectOsType"
        />
        <Text
            v-model="config.data.group"
            name="Group"
            placeholder="Optional — e.g. Homelab"
            :list="groupPresetsId"
            maxlength="32"
        />
        <datalist :id="groupPresetsId">
            <option
                v-for="g in GROUP_PRESETS"
                :key="g"
                :value="g"
            />
        </datalist>

        <!-- Local port forwards (ssh -L), active while a session is open -->
        <div class="space-y-2">
            <p class="block text-base font-medium text-gray-700 dark:text-white mb-0">
                Port forwards (optional)
            </p>
            <p class="text-xs text-[var(--c-muted)] -mt-1">
                Forwards <code>127.0.0.1:local → remote_host:remote_port</code> while the session is open.
            </p>
            <div
                v-for="(f, i) in config.data.forwards"
                :key="i"
                class="flex items-center gap-2"
            >
                <input
                    v-model.number="f.local_port"
                    type="number"
                    min="1"
                    max="65535"
                    placeholder="8080"
                    required
                    class="w-24 border-[1px] rounded-md px-2 py-2 text-sm border-[var(--c-border)] bg-[var(--c-card)] text-[var(--c-text)] focus:outline-none focus:ring-2 focus:ring-[var(--c-accent)]"
                />
                <span class="text-[var(--c-muted)]">→</span>
                <input
                    v-model="f.remote_host"
                    placeholder="localhost"
                    maxlength="253"
                    required
                    class="flex-1 border-[1px] rounded-md px-2 py-2 text-sm border-[var(--c-border)] bg-[var(--c-card)] text-[var(--c-text)] focus:outline-none focus:ring-2 focus:ring-[var(--c-accent)]"
                />
                <input
                    v-model.number="f.remote_port"
                    type="number"
                    min="1"
                    max="65535"
                    placeholder="80"
                    required
                    class="w-24 border-[1px] rounded-md px-2 py-2 text-sm border-[var(--c-border)] bg-[var(--c-card)] text-[var(--c-text)] focus:outline-none focus:ring-2 focus:ring-[var(--c-accent)]"
                />
                <Button
                    type="button"
                    variant="ghost"
                    class="!px-2"
                    :aria-label="`Remove forward ${i + 1}`"
                    @click="config.data.forwards.splice(i, 1)"
                >
                    <Icon icon="fas fa-trash" />
                </Button>
            </div>
            <Button
                v-if="config.data.forwards.length < 16"
                type="button"
                variant="ghost"
                @click="config.data.forwards.push({local_port: 0, remote_host: 'localhost', remote_port: 80})"
            >
                <Icon icon="fas fa-plus" />
                Add forward
            </Button>
        </div>
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
                    hasSavedSecret
                        ? 'Saved in keychain. Leave blank to keep it.'
                        : 'Only if the key is encrypted'
                "
            />
        </template>

        <!-- Forget a saved secret without deleting the host -->
        <Button
            v-if="hasSavedSecret && !secretCleared"
            class="w-full !bg-gray-500"
            type="button"
            @click="forgetSecret"
        >
            Forget saved password
        </Button>

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
            :class="`!bg-red-500 hover:!bg-red-600`"
            class="mr-2 mt-3 mb-2 w-full"
            type="button"
        >
            Delete terminal
        </Button>
        <Dialog
            :show="showModal"
            label="Delete this config?"
            @close="showModal = false"
        >
            <div>
                <p class="text-[var(--c-text)]">Delete this config? This cannot be undone.</p>
                <div class="space-y-1">
                    <Button
                        v-if="props.actionType !== 'add'"
                        :class="`!bg-red-500 hover:!bg-red-600`"
                        class="mr-2 mt-3 mb-2 w-full"
                        type="button"
                        @click="deleteConfig"
                    >
                        Delete it!
                    </Button>
                    <Button
                        :class="`!bg-slate-200 !text-slate-700 hover:!bg-slate-300 dark:!bg-slate-700 dark:!text-white dark:hover:!bg-slate-600`"
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
