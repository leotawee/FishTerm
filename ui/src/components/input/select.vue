<script lang="ts">
// Module scope on purpose: this capture listener is registered at import time — i.e.
// BEFORE the mount-time window capture listener that Dialog.vue registers. Same-target
// capture listeners dispatch in registration order, so this handler sees Escape first
// and `stopImmediatePropagation` keeps Dialog.vue from closing the whole dialog while
// a dropdown is open. (A capture listener on the list container or on document would
// fire after window capture and never see the key.)
type DropdownCloser = () => void;
const openDropdowns = new Set<DropdownCloser>();

function onCaptureKeydown(event: KeyboardEvent) {
    if (event.key !== 'Escape' || openDropdowns.size === 0) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    for (const close of [...openDropdowns]) close();
}

window.addEventListener('keydown', onCaptureKeydown, true);
</script>

<script lang="ts" setup>
import {computed, nextTick, onUnmounted, ref, useId} from 'vue';

import useClickOutside from '../../plugins/useClickOutside';
import {isImageIcon} from '../../osIcons';

const {onClickOutside} = useClickOutside();

const props = defineProps<SelectInfo>();
const emit = defineEmits(['update:modelValue', 'onChange']);

interface SelectInfo {
    modelValue: any;
    name?: string;
    data: SelectData[];
}

interface SelectData {
    name: string;
    value: string | number;
    icon?: string;
}

// Label/trigger association: the visible label points at the trigger button id.
const id = useId();
// <ul> id exposed to the trigger via aria-controls.
const SListboxId = useId();
// Keyboard-active item (distinct styling from the selected item).
const activeIndex = ref(-1);
// Select label
const SLabel_isOpen = ref(false);
// Pure: derived from props instead of mutated during render.
const SLabel_dataPreview = computed(
    () =>
        props.data.find(item => item.value === props.modelValue) ?? {
            name: '',
            value: '',
            icon: '',
        },
);
// Select Box
const SBox = ref<HTMLUListElement | null>(null);
// Variable
let isClickedOutside = false;

// Type-ahead state: accumulated buffer + ~500ms reset timer.
let typeaheadBuffer = '';
let typeaheadTimer: ReturnType<typeof setTimeout> | undefined;

function resetTypeahead() {
    typeaheadBuffer = '';
    if (typeaheadTimer !== undefined) {
        clearTimeout(typeaheadTimer);
        typeaheadTimer = undefined;
    }
}

// Function
function scrollToActive() {
    if (activeIndex.value < 0) return;
    SBox.value
        ?.querySelector(`#listbox-item-${activeIndex.value}`)
        ?.scrollIntoView({block: 'nearest'});
}

function activeIndexOfValue() {
    const idx = props.data.findIndex(item => item.value === props.modelValue);
    return idx >= 0 ? idx : props.data.length > 0 ? 0 : -1;
}

function openDropdown() {
    activeIndex.value = activeIndexOfValue();
    SLabel_isOpen.value = true;
    openDropdowns.add(closeDropdown);
    nextTick(scrollToActive);
}

function closeDropdown() {
    SLabel_isOpen.value = false;
    resetTypeahead();
    openDropdowns.delete(closeDropdown);
}

function clickSelectTextBox() {
    if (!isClickedOutside) {
        if (SLabel_isOpen.value) closeDropdown();
        else openDropdown();
    } else {
        isClickedOutside = false;
    }
}

function SelectItem(data: SelectData) {
    closeDropdown();

    emit('onChange', data.value);
    emit('update:modelValue', data.value);
}

function isSelected(value: string | number) {
    return value === props.modelValue;
}

function moveActive(delta: number) {
    const count = props.data.length;
    if (!count) return;
    if (activeIndex.value < 0) {
        activeIndex.value = 0;
    } else {
        activeIndex.value = (((activeIndex.value + delta) % count) + count) % count;
    }
    scrollToActive();
}

// Type-ahead: case-insensitive match on option names, searched after the active index
// with wrap-around; prefer extending the buffer, otherwise restart from the new char.
function typeahead(char: string) {
    if (typeaheadTimer !== undefined) clearTimeout(typeaheadTimer);
    typeaheadTimer = setTimeout(resetTypeahead, 500);

    const count = props.data.length;
    if (!count) return;
    const start = activeIndex.value >= 0 ? activeIndex.value + 1 : 0;
    for (const needle of [(typeaheadBuffer + char).toLowerCase(), char.toLowerCase()]) {
        for (let step = 0; step < count; step++) {
            const idx = (start + step) % count;
            if (props.data[idx]!.name.toLowerCase().startsWith(needle)) {
                typeaheadBuffer = needle;
                activeIndex.value = idx;
                scrollToActive();
                return;
            }
        }
    }
    typeaheadBuffer = '';
}

function onTriggerKeydown(event: KeyboardEvent) {
    if (event.ctrlKey || event.metaKey || event.altKey) return;

    switch (event.key) {
        case 'ArrowDown':
        case 'ArrowUp':
            event.preventDefault();
            if (!SLabel_isOpen.value) {
                openDropdown();
                return;
            }
            moveActive(event.key === 'ArrowDown' ? 1 : -1);
            return;
        case 'Home':
        case 'End':
            if (!SLabel_isOpen.value) return;
            event.preventDefault();
            activeIndex.value = event.key === 'Home' ? 0 : props.data.length - 1;
            scrollToActive();
            return;
        case 'Enter':
        case ' ': {
            event.preventDefault();
            if (!SLabel_isOpen.value) {
                openDropdown();
                return;
            }
            const item = props.data[activeIndex.value];
            if (item) SelectItem(item);
            return;
        }
        case 'Escape':
            // Normally intercepted earlier by the module-level capture listener;
            // kept as a fallback so Esc only closes the dropdown, never selects.
            if (SLabel_isOpen.value) {
                event.preventDefault();
                closeDropdown();
            }
            return;
        default:
            if (SLabel_isOpen.value && event.key.length === 1) typeahead(event.key);
    }
}

// Plugins
onClickOutside(SBox, () => {
    if (SLabel_isOpen.value === true) {
        isClickedOutside = true;
        closeDropdown();
    }
});

onUnmounted(closeDropdown);
</script>

<template>
    <div class="col-span-12 mb-2">
        <label
            :id="`${id}-label`"
            :for="id"
            class="block text-base font-medium text-black dark:text-white mb-2"
            v-text="name"
        />

        <div class="relative">
            <span class="inline-block w-full rounded-md shadow-sm">
                <button
                    type="button"
                    :id="id"
                    @click="clickSelectTextBox"
                    @keydown="onTriggerKeydown"
                    aria-haspopup="listbox"
                    :aria-expanded="SLabel_isOpen"
                    :aria-controls="SListboxId"
                    :aria-labelledby="`${id}-label`"
                    :aria-activedescendant="
                        SLabel_isOpen && activeIndex >= 0 ? `listbox-item-${activeIndex}` : undefined
                    "
                    class="cursor-pointer dark:border-slate-600 dark:bg-slate-800 relative w-full rounded-md border border-gray-300 bg-white pl-3 pr-10 py-2 text-left focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 transition ease-out duration-150 sm:text-sm sm:leading-5"
                >
                    <div
                        class="flex items-center space-x-3 text-black dark:text-white"
                        v-if="SLabel_dataPreview.name"
                    >
                        <img
                            v-if="isImageIcon(SLabel_dataPreview.icon)"
                            :src="SLabel_dataPreview.icon"
                            class="w-4 h-4"
                            alt=""
                        />
                        <Icon
                            v-else
                            :icon="SLabel_dataPreview.icon"
                            class="text-black dark:text-white"
                        />
                        <span
                            class="block truncate"
                            v-text="SLabel_dataPreview.name"
                        />
                    </div>
                    <div
                        class="flex items-center space-x-3 text-black dark:text-white"
                        v-else
                    >
                        <span
                            class="block truncate"
                            v-text="`Please select an option`"
                        />
                    </div>
                    <span
                        class="absolute inset-y-0 right-0 flex items-center pr-2 pointer-events-none"
                    >
                        <Icon
                            icon="fas fa-chevron-down"
                            class="text-gray-400 dark:text-white"
                        />
                    </span>
                </button>
            </span>
            <transition
                enter-active-class="transition-opacity ease-out duration-150"
                enter-from-class="opacity-0"
                enter-to-class="opacity-100"
                leave-active-class="transition-opacity ease-out duration-150"
                leave-from-class="opacity-100"
                leave-to-class="opacity-0"
            >
                <div
                    v-show="SLabel_isOpen"
                    class="absolute top-full left-0 z-20 mt-1 w-full rounded-md bg-white shadow-lg"
                >
                    <ul
                        tabindex="-1"
                        ref="SBox"
                        role="listbox"
                        :id="SListboxId"
                        :aria-labelledby="`${id}-label`"
                        class="bg-white dark:bg-slate-800 max-h-56 rounded-md py-1 text-base leading-6 overflow-auto focus:outline-none sm:text-sm sm:leading-5"
                    >
                        <li
                            tabindex="-1"
                            @click="SelectItem(d)"
                            :id="`listbox-item-${idx}`"
                            role="option"
                            :aria-selected="isSelected(d.value)"
                            v-for="(d, idx) in data"
                            :key="idx"
                            class="select-none relative py-2 pl-3 pr-9 cursor-pointer text-indigo-600 hover:bg-indigo-600 hover:text-white dark:text-white"
                            :class="{
                                'bg-slate-100 dark:bg-slate-700':
                                    SLabel_isOpen && idx === activeIndex,
                            }"
                        >
                            <div class="flex items-center space-x-3">
                                <img
                                    v-if="d.icon && isImageIcon(d.icon)"
                                    :src="d.icon"
                                    class="w-4 h-4"
                                    alt=""
                                />
                                <Icon
                                    :icon="d.icon"
                                    v-else-if="d.icon"
                                />
                                <span
                                    class="block truncate"
                                    v-text="d.name"
                                />
                            </div>
                            <span
                                class="absolute inset-y-0 right-0 flex items-center pr-4"
                                :class="{
                                    'font-normal': !isSelected(d.value),
                                    'font-semibold': isSelected(d.value),
                                }"
                            >
                                <Icon
                                    icon="fas fa-check-circle"
                                    v-if="isSelected(d.value)"
                                />
                            </span>
                        </li>
                    </ul>
                </div>
            </transition>
        </div>
    </div>
</template>
