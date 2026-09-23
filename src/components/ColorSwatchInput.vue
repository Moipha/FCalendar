<script setup lang="ts">
const model = defineModel<string>({ required: true });

defineProps<{
  disabled?: boolean;
  ariaLabel?: string;
}>();
</script>

<template>
  <label
    class="relative inline-flex size-9 shrink-0 cursor-pointer overflow-hidden rounded-md border border-border bg-background disabled:cursor-not-allowed disabled:opacity-50"
    :class="{ 'pointer-events-none opacity-50': disabled }"
  >
    <input
      v-model="model"
      type="color"
      class="absolute inset-0 cursor-pointer opacity-0"
      :disabled="disabled"
      :aria-label="ariaLabel"
    />
    <span class="block size-full" :style="{ backgroundColor: model }" />
  </label>
</template>
