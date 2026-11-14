<script setup lang="ts">
/** 检查更新弹窗（460 px）：当前构建未接入更新源，只展示版本信息。 */
import { onMounted, ref } from "vue";
import { t } from "../i18n";
import { APP_VERSION } from "../constants";

const emit = defineEmits<{ (event: "close"): void }>();
const closeButton = ref<HTMLButtonElement | null>(null);

onMounted(() => closeButton.value?.focus());
</script>

<template>
  <div class="dialog-backdrop" @pointerdown.self="emit('close')">
    <section class="dialog update-dialog" role="dialog" aria-modal="true" aria-labelledby="update-title">
      <header class="dialog-header">
        <h2 id="update-title">{{ t("dialog.update") }}</h2>
      </header>
      <div class="dialog-body">
        <p class="update-version">
          <span>{{ t("dialog.version") }}</span>
          <strong>{{ APP_VERSION }}</strong>
        </p>
        <p class="update-note">{{ t("dialog.updateNote") }}</p>
      </div>
      <footer class="dialog-actions">
        <button ref="closeButton" type="button" class="btn btn-primary" @click="emit('close')">
          {{ t("dialog.close") }}
        </button>
      </footer>
    </section>
  </div>
</template>
