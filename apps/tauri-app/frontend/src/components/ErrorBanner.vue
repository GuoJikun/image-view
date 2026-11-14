<script setup lang="ts">
/**
 * 内联错误横幅：工具栏下方，不遮挡画布。
 * 左侧 3 px 竖条 + 警告图标 + 11.5 px 文本 + 无边框文字按钮。
 */
import type { CommandError } from "../types/ipc";
import { t } from "../i18n";
import Icon from "./Icon.vue";

defineProps<{ error: CommandError }>();
const emit = defineEmits<{ (event: "close"): void; (event: "retry"): void }>();
</script>

<template>
  <div class="error-banner" role="alert">
    <Icon name="warning" :size="14" class="error-icon" />
    <span class="error-message">{{ error.message }}</span>
    <button type="button" class="text-button" @click="emit('retry')">{{ t("error.retry") }}</button>
    <button type="button" class="text-button" @click="emit('close')">{{ t("error.close") }}</button>
  </div>
</template>
