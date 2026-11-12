<script lang="ts" setup>
// variant="ghost" is for secondary actions (e.g. Settings next to Connect) so the
// filled indigo stays reserved for the primary action of the screen.
withDefaults(defineProps<{variant?: 'primary' | 'ghost'}>(), {variant: 'primary'});
</script>

<template>
    <button
        :class="variant === 'ghost'
            ? 'bg-transparent border border-slate-300 dark:border-slate-600 text-slate-600 dark:text-slate-300 hover:bg-slate-100 hover:text-slate-800 dark:hover:bg-slate-700 dark:hover:text-white'
            : 'bg-indigo-600 dark:bg-indigo-600 hover:bg-indigo-700 active:bg-indigo-700 text-white'"
        class="flex sm:inline-flex justify-center items-center space-x-2 focus-visible:ring-2 focus-visible:ring-indigo-300 font-semibold text-center rounded-md outline-none transition-[transform,background-color,border-color,color] duration-100 ease-out active:scale-[0.98] px-5 py-1"
    >
        <slot />
    </button>
</template>
