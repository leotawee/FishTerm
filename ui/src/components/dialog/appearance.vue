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
    {name: 'Dracula', value: 'dracula', icon: 'fas fa-palette'},
    {name: 'Nord', value: 'nord', icon: 'fas fa-palette'},
    {name: 'Tokyo Night', value: 'tokyo_night', icon: 'fas fa-palette'},
    {name: 'Gruvbox Dark', value: 'gruvbox_dark', icon: 'fas fa-palette'},
    {name: 'One Dark', value: 'one_dark', icon: 'fas fa-palette'},
    {name: 'Rosé Pine', value: 'rose_pine', icon: 'fas fa-palette'},
];
const termOptions = [
    {name: 'Slate (default)', value: 'slate', icon: 'fas fa-terminal'},
    {name: 'Black', value: 'black', icon: 'fas fa-terminal'},
    {name: 'Catppuccin Mocha', value: 'catppuccin_mocha', icon: 'fas fa-terminal'},
    {name: 'Catppuccin Latte', value: 'catppuccin_latte', icon: 'fas fa-terminal'},
    {name: 'Catppuccin Frappé', value: 'catppuccin_frappe', icon: 'fas fa-terminal'},
    {name: 'Catppuccin Macchiato', value: 'catppuccin_macchiato', icon: 'fas fa-terminal'},
    {name: 'Dracula', value: 'dracula', icon: 'fas fa-terminal'},
    {name: 'Nord', value: 'nord', icon: 'fas fa-terminal'},
    {name: 'Tokyo Night', value: 'tokyo_night', icon: 'fas fa-terminal'},
    {name: 'Tokyo Night Light', value: 'tokyo_night_light', icon: 'fas fa-terminal'},
    {name: 'Gruvbox Dark', value: 'gruvbox_dark', icon: 'fas fa-terminal'},
    {name: 'Gruvbox Light', value: 'gruvbox_light', icon: 'fas fa-terminal'},
    {name: 'One Dark', value: 'one_dark', icon: 'fas fa-terminal'},
    {name: 'Rosé Pine', value: 'rose_pine', icon: 'fas fa-terminal'},
    {name: 'Everforest Dark', value: 'everforest_dark', icon: 'fas fa-terminal'},
    {name: 'Monokai Pro', value: 'monokai_pro', icon: 'fas fa-terminal'},
    {name: 'Snazzy', value: 'snazzy', icon: 'fas fa-terminal'},
    {name: 'Night Owl', value: 'night_owl', icon: 'fas fa-terminal'},
    {name: 'Tomorrow Night', value: 'tomorrow_night', icon: 'fas fa-terminal'},
    {name: 'Solarized Dark', value: 'solarized_dark', icon: 'fas fa-terminal'},
    {name: 'Solarized Light', value: 'solarized_light', icon: 'fas fa-terminal'},
    {name: 'GitHub Dark', value: 'github_dark', icon: 'fas fa-terminal'},
    {name: 'GitHub Light', value: 'github_light', icon: 'fas fa-terminal'},
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
