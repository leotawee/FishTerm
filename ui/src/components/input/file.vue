<script lang="ts" setup>
import {ref, watch, onMounted} from 'vue';
import {open} from '@tauri-apps/plugin-dialog';

const props = defineProps(['name', 'text', 'path']);
const emit = defineEmits(['updatePath']);

const previewFile = ref('');

// WHY: upstream read Electron's `File.path`; Tauri webviews don't expose paths, so the
// native picker returns the absolute path and the Rust core reads the file itself.
async function pick() {
    const selected = await open({multiple: false, directory: false, title: props.name});
    if (typeof selected === 'string') emit('updatePath', selected);
}

watch(
    () => props.path,
    (n: string) => {
        previewFile.value = n || 'Please upload file';
    },
);

onMounted(() => {
    previewFile.value = props.path || 'Please upload file';
});
</script>

<template>
    <div class="col-span-12 mb-2">
        <label
            class="block text-md font-medium text-gray-700 dark:text-white mb-2"
            v-text="props.name"
        />
        <div class="flex items-center justify-center w-full">
            <button
                type="button"
                @click="pick"
                class="flex flex-col w-full h-32 border-4 border-blue-200 dark:border-slate-500 dark:hover:bg-slate-900 border-dashed hover:bg-gray-100 hover:border-gray-300"
            >
                <div class="flex flex-col items-center justify-center pt-7 w-full">
                    <p
                        class="pt-1 text-sm tracking-wider text-gray-400 group-hover:text-gray-600"
                        v-text="props.text"
                    />
                    <p
                        class="pt-1 text-sm tracking-wider text-gray-400 group-hover:text-gray-600 break-all px-2"
                        v-text="previewFile"
                    />
                </div>
            </button>
        </div>
    </div>
</template>
