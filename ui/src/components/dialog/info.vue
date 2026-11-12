<script lang="ts" setup>
import iconUrl from '../../../assets/logo.svg?url';

const version = __APP_VERSION__;
</script>

<template>
    <div class="text-center text-black dark:text-white space-y-3">
        <div
            id="logo"
            class="space-y-3"
        >
            <img
                class="mx-auto rounded-2xl w-[256px] h-[256px]"
                :src="iconUrl"
                alt="FishTerm logo"
            />

            <div class="space-y-2">
                <h1 class="text-3xl">Fish Term 🐟</h1>
                <p class="text-sm">{{ version }}</p>
            </div>
        </div>
        <div
            id="desc"
            class="block"
        >
            <p class="text-sm">Developer (Main): @M-307</p>
            <p class="text-sm">Developer (2nd): @Maythiwat, @nSys</p>
            <p class="text-sm">Tester: -</p>
        </div>
    </div>
</template>
