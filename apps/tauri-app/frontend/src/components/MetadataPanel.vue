<script setup lang="ts">
import { computed } from "vue";
import { useViewerStore } from "../stores/viewer";

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
  return new Intl.DateTimeFormat("zh-CN", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(timestamp * 1000));
}

const rows = computed(() => {
  const image = store.preview;
  if (!image) return [];
  return [
    ["格式", image.format],
    ["原图尺寸", `${image.sourceWidth} × ${image.sourceHeight}`],
    ["显示尺寸", `${image.width} × ${image.height}`],
    ["位深", `${image.sourceDepth}-bit`],
    ["文件大小", formatBytes(image.fileSize)],
    ["修改时间", formatDate(image.modifiedAt)],
    ["色彩", image.colorConverted ? `已转换到 sRGB${image.colorSource ? `（${image.colorSource}）` : ""}` : "sRGB"],
    ["降采样", image.downscaled ? "是（已按安全上限缩放）" : "否"],
  ];
});
</script>

<template>
  <aside class="metadata-panel" aria-label="图片元数据">
    <header class="panel-header">
      <span>元数据</span>
      <button type="button" class="panel-close" title="隐藏面板" @click="store.showMetadata = false">×</button>
    </header>

    <div v-if="!store.preview" class="panel-empty">打开图片后显示尺寸、格式与色彩信息。</div>

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

      <p v-if="store.preview.downscaled" class="panel-note">
        当前显示的是降采样结果；「100%」相对显示尺寸，而非原图。
      </p>
    </div>
  </aside>
</template>
