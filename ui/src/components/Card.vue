<template>
    <div class="shadow-lg flex bg-white rounded-lg p-6 leading-normal relative dark:bg-[var(--c-card)]">
        <div class="absolute top-2 right-2">
            <slot name="tags" />
        </div>
        <slot />
    </div>
</template>
