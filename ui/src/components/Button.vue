<script lang="ts" setup>
// variant="ghost" is for secondary actions (e.g. Settings next to Connect) so the
// filled accent stays reserved for the primary action of the screen.
withDefaults(defineProps<{variant?: 'primary' | 'ghost'}>(), {variant: 'primary'});
</script>

<template>
    <button
        :class="variant === 'ghost'
            ? 'bg-transparent border border-[var(--c-border)] text-[var(--c-muted)] hover:bg-[var(--c-ghost-hover)] hover:text-[var(--c-text)]'
            : 'bg-[var(--c-accent)] hover:bg-[var(--c-accent-hover)] active:bg-[var(--c-accent-hover)] text-[var(--c-accent-text)]'"
        class="flex sm:inline-flex justify-center items-center space-x-2 focus-visible:ring-2 focus-visible:ring-[var(--c-accent)] font-semibold text-center rounded-md outline-none transition-[transform,background-color,border-color,color] duration-100 ease-out active:scale-[0.98] px-5 py-1"
    >
        <slot />
    </button>
</template>
