<script setup lang="ts">
/**
 * 状态栏 26 px：左侧尺寸 / 位深 / 格式 / 色彩标记，右侧短提示 → 幻灯片 → 序号 → spinner。
 * 缩略图条隐藏时，上缘补 1 px 发丝线与画布分隔（有条时由条自身提供）。
 */
import { computed } from "vue";
import { useViewerStore } from "../stores/viewer";
import { useUiStore } from "../stores/ui";
import { t } from "../i18n";

const store = useViewerStore();
const ui = useUiStore();

const stripVisible = computed(
  () => store.showThumbnails && (store.snapshot?.entries.length ?? 0) > 0,
);

const info = computed<{ text: string; tone?: "warn" }[]>(() => {
  const image = store.preview;
  if (!image) return [];
  const parts: { text: string; tone?: "warn" }[] = [
    { text: `${image.sourceWidth} × ${image.sourceHeight}` },
    { text: `${image.sourceDepth} bit` },
    { text: image.format },
  ];
  if (ui.showColorBadge && image.colorConverted) {
    parts.push({ text: image.colorSource ? `${image.colorSource} → sRGB` : "sRGB" });
  }
  if (image.downscaled) parts.push({ text: t("status.downscaled"), tone: "warn" });
  return parts;
});

const positionText = computed(() => {
  const position = store.snapshot?.position;
  return position ? `${position[0]} / ${position[1]}` : "";
});
</script>

<template>
  <footer class="status-bar" :class="{ solo: !stripVisible }">
    <div class="status-left">
      <template v-if="info.length">
        <template v-for="(part, index) in info" :key="part.text">
          <span v-if="index > 0" class="status-dot">·</span>
          <span :class="{ 'status-warn': part.tone === 'warn' }">{{ part.text }}</span>
        </template>
      </template>
      <span v-else class="status-empty">{{ t("status.noImage") }}</span>
    </div>

    <div class="status-right">
      <span v-if="ui.notice" class="status-notice">{{ ui.notice }}</span>
      <span v-if="store.slideshow" class="status-flag">{{ t("status.slideshow") }}</span>
      <span v-if="positionText" class="status-position">{{ positionText }}</span>
      <span v-if="store.loading" class="spinner spinner-small" aria-hidden="true" />
    </div>
  </footer>
</template>
