<script lang="ts" setup>
defineProps<{title: string}>();
</script>

<template>
    <div class="inline-flex items-center">
        <h1
            class="ml-1 text-2xl font-bold text-gray-700 dark:text-white"
            v-text="title"
        />
    </div>
</template>
