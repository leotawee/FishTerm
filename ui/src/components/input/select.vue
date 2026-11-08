<script lang="ts" setup>
import {computed, ref} from 'vue';

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
const SBox = ref(null);
// Variable
let isClickedOutside = false;

// Function
function clickSelectTextBox() {
    if (!isClickedOutside) SLabel_isOpen.value = !SLabel_isOpen.value;
    else isClickedOutside = false;
}

function SelectItem(data: SelectData) {
    SLabel_isOpen.value = false;

    emit('onChange', data.value);
    emit('update:modelValue', data.value);
}

function isSelected(value: string | number) {
    return value === props.modelValue;
}

// Plugins
onClickOutside(SBox, () => {
    if (SLabel_isOpen.value === true) {
        isClickedOutside = true;
        SLabel_isOpen.value = false;
    }
});
</script>

<template>
    <div class="col-span-12 mb-2">
        <label
            class="block text-md font-medium text-black dark:text-white mb-2"
            v-text="name"
        />

        <div class="relative">
            <span class="inline-block w-full rounded-md shadow-sm">
                <button
                    type="button"
                    @click="clickSelectTextBox"
                    aria-haspopup="listbox"
                    :aria-expanded="SLabel_isOpen"
                    aria-labelledby="listbox-label"
                    class="cursor-pointer dark:border-slate-600 dark:bg-slate-800 relative w-full rounded-md border border-gray-300 bg-white pl-3 pr-10 py-2 text-left focus:outline-none focus:shadow-outline-blue transition ease-in-out duration-150 sm:text-sm sm:leading-5"
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
                enter-active-class="transition duration-150 transform"
                enter-from-class="opacity-0"
                enter-to-class="opacity-100"
                leave-active-class="transition duration-150 transform"
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
                        aria-labelledby="listbox-label"
                        class="bg-white dark:bg-slate-800 max-h-56 rounded-md py-1 text-base leading-6 shadow-xs overflow-auto focus:outline-none sm:text-sm sm:leading-5"
                    >
                        <li
                            tabindex="0"
                            @click="SelectItem(d)"
                            :id="`listbox-item-${idx}`"
                            role="option"
                            :aria-selected="isSelected(d.value)"
                            v-for="(d, idx) in data"
                            :key="idx"
                            class="select-none relative py-2 pl-3 pr-9 cursor-pointer text-indigo-600 hover:bg-indigo-600 hover:text-white dark:text-white"
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
