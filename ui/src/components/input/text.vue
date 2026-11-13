<script lang="ts" setup>
import {useId} from 'vue';

// Forward unknown attrs (min/max/autocomplete, ...) to the native input instead of the wrapper div.
defineOptions({inheritAttrs: false});

const props = defineProps(['name', 'type', 'placeholder', 'modelValue', 'required']);
const emit = defineEmits(['update:modelValue']);

const id = useId();

function onInput(event: Event) {
    const target = event.target as HTMLInputElement;
    emit('update:modelValue', target.value);
}
</script>

<template>
    <div>
        <label
            :for="id"
            class="block text-base font-medium text-gray-700 dark:text-white mb-2"
            v-text="props.name"
        />
        <input
            :id="id"
            v-bind="$attrs"
            @input="onInput($event)"
            :type="type"
            :value="props.modelValue"
            :placeholder="props.placeholder"
            :required="props.required"
            class="mt-1 border-[1px] block w-full shadow-sm sm:text-sm border-[var(--c-border)] rounded-md px-3 py-3 focus:ring-2 focus:ring-[var(--c-accent)] focus:border-[var(--c-accent)] bg-[var(--c-card)] text-[var(--c-text)] focus:outline-none"
        />
    </div>
</template>
