<script setup lang="ts">
/** 快捷键帮助弹窗：两列「键帽 + 说明」，三端一致（macOS 使用 ⌘ ⌥ ⇧ 符号）。 */
import { computed, onMounted, ref } from "vue";
import { isMac, modSymbol } from "../platform";
import { t } from "../i18n";

const emit = defineEmits<{ (event: "close"): void }>();

interface ShortcutRow {
  keys: string[];
  label: string;
}

const closeButton = ref<HTMLButtonElement | null>(null);

const rows = computed<ShortcutRow[]>(() => [
  { keys: [modSymbol, "O"], label: t("shortcut.open") },
  { keys: ["←", "→"], label: t("shortcut.navigate") },
  { keys: ["+", "−"], label: t("shortcut.zoom") },
  { keys: ["0"], label: t("shortcut.fit") },
  { keys: ["1"], label: t("shortcut.actual") },
  { keys: ["R", isMac ? "⇧R" : "Shift+R"], label: t("shortcut.rotate") },
  { keys: ["H"], label: t("shortcut.flipH") },
  { keys: ["V"], label: t("shortcut.flipV") },
  { keys: ["Space"], label: t("shortcut.slideshow") },
  { keys: isMac ? ["⌃", "⌘", "F"] : ["F11"], label: t("shortcut.fullscreen") },
  { keys: [modSymbol, isMac ? "⇧" : "Shift", "C"], label: t("shortcut.copyPath") },
  { keys: [isMac ? "⌫" : "Delete"], label: t("shortcut.trash") },
  { keys: ["Esc"], label: t("shortcut.dismiss") },
  { keys: [], label: t("shortcut.pan") },
  { keys: [], label: t("shortcut.wheel") },
]);

onMounted(() => closeButton.value?.focus());
</script>

<template>
  <div class="dialog-backdrop" @pointerdown.self="emit('close')">
    <section class="dialog help-dialog" role="dialog" aria-modal="true" aria-labelledby="help-title">
      <header class="dialog-header">
        <h2 id="help-title">{{ t("dialog.shortcuts") }}</h2>
      </header>
      <div class="dialog-body">
        <dl class="shortcut-grid">
          <template v-for="row in rows" :key="row.label">
            <dt class="shortcut-keys">
              <kbd v-for="key in row.keys" :key="key" class="keycap">{{ key }}</kbd>
            </dt>
            <dd>{{ row.label }}</dd>
          </template>
        </dl>
      </div>
      <footer class="dialog-actions">
        <button ref="closeButton" type="button" class="btn btn-primary" @click="emit('close')">
          {{ t("dialog.close") }}
        </button>
      </footer>
    </section>
  </div>
</template>
