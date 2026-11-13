<script setup lang="ts">
defineProps<{
  label: string;
  title?: string;
  active?: boolean;
  disabled?: boolean;
}>();

const emit = defineEmits<{ (event: "click"): void }>();
</script>

<template>
  <button
    type="button"
    class="toolbar-button"
    :class="{ active }"
    :title="title ?? label"
    :aria-label="label"
    :disabled="disabled"
    @click="emit('click')"
  >
    <span class="toolbar-icon"><slot /></span>
  </button>
</template>
