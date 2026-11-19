<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref } from "vue";
import { useCompareStore } from "../stores/compare";
import { useUiStore } from "../stores/ui";
import { t } from "../i18n";

const compare = useCompareStore();
const ui = useUiStore();
const canvas = ref<HTMLElement | null>(null);
const dragging = ref(false);
let lastX = 0;
let lastY = 0;

const layoutClass = computed(() =>
  compare.layout === "overlay" ? "compare-overlay" : "compare-side-by-side",
);

function slotStyle(index: number): Record<string, string> {
  if (compare.layout === "overlay") {
    return { left: "0", top: "0", width: "100%", height: "100%" };
  }
  const percent = 100 / compare.count;
  return { left: `${index * percent}%`, top: "0", width: `${percent}%`, height: "100%" };
}

function imageStyle(width: number, height: number): Record<string, string> {
  return {
    width: `${width}px`,
    height: `${height}px`,
    transform: `translate(${compare.panX}px, ${compare.panY}px)`,
  };
}

function onWheel(event: WheelEvent): void {
  event.preventDefault();
  compare.zoomBy(event.deltaY < 0 ? 1.15 : 1 / 1.15);
}

function onPointerDown(event: PointerEvent): void {
  if (event.button !== 0) return;
  dragging.value = true;
  lastX = event.clientX;
  lastY = event.clientY;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function onPointerMove(event: PointerEvent): void {
  if (!dragging.value) return;
  compare.panBy(event.clientX - lastX, event.clientY - lastY);
  lastX = event.clientX;
  lastY = event.clientY;
}

function onPointerUp(event: PointerEvent): void {
  dragging.value = false;
  (event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
}

function resize(): void {
  if (!canvas.value) return;
  compare.setViewport(canvas.value.clientWidth, canvas.value.clientHeight);
}

function back(): void {
  compare.exit();
  ui.activeView = "viewer";
}

onMounted(() => {
  resize();
  window.addEventListener("resize", resize);
});
onBeforeUnmount(() => window.removeEventListener("resize", resize));
</script>

<template>
  <div class="compare-view">
    <div class="compare-toolbar">
      <button type="button" @click="back">{{ t("gallery.back") }}</button>
      <span class="compare-title">{{ t("compare.title", { count: compare.count }) }}</span>
      <span class="compare-zoom">{{ Math.round(compare.zoom * 100) }}%</span>
      <button type="button" @click="compare.resetFit()">Fit</button>
      <label class="compare-quality" :title="t('compare.fullPixelHint')">
        <input
          type="checkbox"
          :checked="compare.fullResolution"
          @change="compare.setFullResolution(($event.target as HTMLInputElement).checked)"
        />
        {{ t("compare.fullPixel") }}
      </label>
      <div class="compare-layout">
        <button
          type="button"
          :class="{ active: compare.layout === 'side-by-side' }"
          @click="compare.setLayout('side-by-side')"
        >
          {{ t("gallery.layoutSideBySide") }}
        </button>
        <button
          type="button"
          :class="{ active: compare.layout === 'overlay' }"
          @click="compare.setLayout('overlay')"
        >
          {{ t("gallery.layoutOverlay") }}
        </button>
      </div>
    </div>
    <div
      ref="canvas"
      class="compare-canvas"
      :class="layoutClass"
      @wheel="onWheel"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @pointercancel="onPointerUp"
    >
      <div v-for="(slot, index) in compare.slots" :key="slot.imageId" class="compare-slot" :style="slotStyle(index)">
        <img
          v-if="slot.width && slot.height"
          :src="compare.slotUrl(slot)"
          :alt="slot.name"
          :style="imageStyle(compare.imageSize(slot).width, compare.imageSize(slot).height)"
          draggable="false"
        />
        <img v-else :src="compare.slotUrl(slot)" :alt="slot.name" class="compare-slot-fallback" draggable="false" />
        <span class="compare-slot-name">
          {{ slot.name }}
          <small v-if="compare.isFullPixel(slot)">{{ t("compare.fullPixelBadge") }}</small>
        </span>
      </div>
      <div v-if="compare.count === 0" class="compare-empty">{{ t("compare.empty") }}</div>
    </div>
  </div>
</template>
