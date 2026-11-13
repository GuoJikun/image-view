<script setup lang="ts">
import type { CommandError } from "../types/ipc";

defineProps<{ error: CommandError }>();
const emit = defineEmits<{ (event: "close"): void; (event: "retry"): void }>();
</script>

<template>
  <div class="error-banner" role="alert">
    <span class="error-code">{{ error.code }}</span>
    <span class="error-message">{{ error.message }}</span>
    <button type="button" class="error-action" @click="emit('retry')">重试</button>
    <button type="button" class="error-close" title="关闭" @click="emit('close')">×</button>
  </div>
</template>
