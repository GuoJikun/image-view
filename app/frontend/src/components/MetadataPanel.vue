<script setup lang="ts">
/** 元数据侧栏（默认关闭，可从「显示」菜单打开）。 */
import { computed } from "vue";
import { useViewerStore } from "../stores/viewer";
import { resolvedLocale, t } from "../i18n";

const store = useViewerStore();

function formatBytes(bytes: number): string {
  if (!bytes) return "—";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let index = 0;
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024;
    index += 1;
  }
  return `${value.toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

function formatDate(timestamp: number | null): string {
  if (!timestamp) return "—";
  return new Intl.DateTimeFormat(resolvedLocale.value === "zh" ? "zh-CN" : "en-US", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(timestamp * 1000));
}

const rows = computed<[string, string][]>(() => {
  const image = store.preview;
  if (!image) return [];
  const color = image.colorConverted
    ? `${t("metadata.converted")}${image.colorSource ? ` (${image.colorSource})` : ""}`
    : "sRGB";
  return [
    [t("metadata.format"), image.format],
    [t("metadata.sourceSize"), `${image.sourceWidth} × ${image.sourceHeight}`],
    [t("metadata.displaySize"), `${image.width} × ${image.height}`],
    [t("metadata.depth"), `${image.sourceDepth} bit`],
    [t("metadata.fileSize"), formatBytes(image.fileSize)],
    [t("metadata.modified"), formatDate(image.modifiedAt)],
    [t("metadata.color"), color],
    [t("metadata.downscaled"), image.downscaled ? t("metadata.yes") : t("metadata.no")],
  ];
});
</script>

<template>
  <aside class="metadata-panel" :aria-label="t('metadata.title')">
    <header class="panel-header">
      <span>{{ t("metadata.title") }}</span>
      <button
        type="button"
        class="panel-close"
        :title="t('metadata.hide')"
        :aria-label="t('metadata.hide')"
        @click="store.showMetadata = false"
      >
        <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true">
          <path d="M7.5 7.5l9 9M16.5 7.5l-9 9" />
        </svg>
      </button>
    </header>

    <div v-if="!store.preview" class="panel-empty">{{ t("metadata.empty") }}</div>

    <div v-else class="panel-body">
      <div class="panel-file">
        <p class="panel-file-name" :title="store.preview.fileName">{{ store.preview.fileName }}</p>
        <p class="panel-file-path" :title="store.preview.path">{{ store.preview.path }}</p>
      </div>

      <dl class="meta-list">
        <template v-for="[label, value] in rows" :key="label">
          <dt>{{ label }}</dt>
          <dd>{{ value }}</dd>
        </template>
      </dl>

      <p v-if="store.preview.downscaled" class="panel-note">{{ t("metadata.note") }}</p>
    </div>
  </aside>
</template>
