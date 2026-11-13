<script setup lang="ts">
import { computed } from "vue";
import { useViewerStore } from "../stores/viewer";

const store = useViewerStore();

const positionText = computed(() => {
  const position = store.snapshot?.position;
  return position ? `${position[0]} / ${position[1]}` : "—";
});

const sizeText = computed(() => {
  const image = store.preview;
  if (!image) return "—";
  const source = `${image.sourceWidth} × ${image.sourceHeight}`;
  if (!image.downscaled) return source;
  return `${image.width} × ${image.height}（原图 ${source}）`;
});
</script>

<template>
  <footer class="status-bar">
    <span class="status-name" :title="store.preview?.path">{{ store.preview?.fileName ?? "没有打开图片" }}</span>
    <span class="status-sep">·</span>
    <span>{{ sizeText }}</span>
    <span class="status-sep">·</span>
    <span>{{ store.zoomPercent }}%</span>
    <span class="status-sep">·</span>
    <span>{{ positionText }}</span>

    <span v-if="store.preview?.downscaled" class="status-badge">已降采样</span>
    <span v-if="store.preview?.colorConverted" class="status-badge">已色彩转换</span>
    <span v-if="store.preview && store.preview.sourceDepth === 16" class="status-badge">16-bit</span>

    <span class="status-spacer" />
    <span v-if="store.loading" class="status-loading"><span class="spinner spinner-small" />加载中…</span>
    <span v-else class="status-mode">{{ store.view.mode === "fit" ? "适配窗口" : store.view.mode === "actual" ? "实际大小" : "自由缩放" }}</span>
  </footer>
</template>
