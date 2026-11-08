<script setup lang="ts">
import {onMounted, ref} from 'vue';
import {hostsList, type HostView} from './api';

const hosts = ref<HostView[]>([]);
const error = ref('');

onMounted(async () => {
    try {
        hosts.value = await hostsList();
    } catch (e) {
        error.value = String(e);
    }
});
</script>

<template>
    <main class="app">
        <h1>MEO FishTerm</h1>
        <p v-if="error" class="error" v-text="error" />
        <p v-else-if="hosts.length === 0">No saved hosts yet.</p>
        <ul v-else>
            <li v-for="h in hosts" :key="h.id">
                <strong v-text="h.name" /> <span v-text="`${h.username}@${h.host}:${h.port}`" />
            </li>
        </ul>
    </main>
</template>

<style>
:root {
    color-scheme: dark;
    font-family: system-ui, sans-serif;
    background: #0f172a;
    color: #e2e8f0;
}
.app { padding: 16px; }
.error { color: #f87171; }
</style>
