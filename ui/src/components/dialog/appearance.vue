<script lang="ts" setup>
import {ref} from 'vue';

import Select from '../input/select.vue';
import {getTermThemeName, getThemeMode, setTermTheme, setThemeMode, type ThemeMode} from '../../appearance';

const mode = ref<ThemeMode>(getThemeMode());
const termName = ref(getTermThemeName());

const modeOptions = [
    {name: 'Follow system', value: 'system', icon: 'fas fa-desktop'},
    {name: 'Light (default)', value: 'light', icon: 'fas fa-sun'},
    {name: 'Dark (default)', value: 'dark', icon: 'fas fa-moon'},
    {name: 'Catppuccin Mocha', value: 'catppuccin_mocha', icon: 'fas fa-palette'},
    {name: 'Catppuccin Latte', value: 'catppuccin_latte', icon: 'fas fa-palette'},
];
const termOptions = [
    {name: 'Slate (default)', value: 'slate', icon: 'fas fa-terminal'},
    {name: 'Black', value: 'black', icon: 'fas fa-terminal'},
    {name: 'Catppuccin Mocha', value: 'catppuccin_mocha', icon: 'fas fa-terminal'},
    {name: 'Catppuccin Latte', value: 'catppuccin_latte', icon: 'fas fa-terminal'},
    {name: 'Solarized Dark', value: 'solarized', icon: 'fas fa-terminal'},
    {name: 'Dracula', value: 'dracula', icon: 'fas fa-terminal'},
    {name: 'Paper (light)', value: 'paper', icon: 'fas fa-terminal'},
];

function onMode(value: string | number) {
    mode.value = value as ThemeMode;
    setThemeMode(mode.value);
}

function onTerm(value: string | number) {
    termName.value = String(value);
    setTermTheme(termName.value);
}
</script>

<template>
    <div class="space-y-3 text-left">
        <h1 class="text-xl font-bold text-[var(--c-text)]">Appearance</h1>
        <Select
            v-model="mode"
            name="Theme"
            :data="modeOptions"
            @onChange="onMode"
        />
        <Select
            v-model="termName"
            name="Terminal color theme"
            :data="termOptions"
            @onChange="onTerm"
        />
        <p class="text-xs text-[var(--c-muted)]">
            Terminal themes apply to open tabs immediately. Preferences are saved on this machine.
        </p>
    </div>
</template>
