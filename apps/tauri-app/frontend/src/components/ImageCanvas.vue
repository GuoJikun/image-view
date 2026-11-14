<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useViewerStore } from "../stores/viewer";
import { useUiStore } from "../stores/ui";
import { modSymbol } from "../platform";
import { t } from "../i18n";
import Icon from "./Icon.vue";

const store = useViewerStore();
const ui = useUiStore();
const host = ref<HTMLDivElement | null>(null);
const canvas = ref<HTMLCanvasElement | null>(null);
const dragging = ref(false);

let image: HTMLImageElement | null = null;
let resizeObserver: ResizeObserver | null = null;
let frame = 0;
let checker: CanvasPattern | null = null;
let lastPointer = { x: 0, y: 0 };

const cursor = computed(() => (dragging.value ? "grabbing" : "grab"));

function scheduleDraw(): void {
  if (frame !== 0) return;
  frame = requestAnimationFrame(() => {
    frame = 0;
    draw();
  });
}

function ensureChecker(context: CanvasRenderingContext2D): CanvasPattern | null {
  if (checker) return checker;
  const tile = document.createElement("canvas");
  tile.width = 16;
  tile.height = 16;
  const tileContext = tile.getContext("2d");
  if (!tileContext) return null;
  tileContext.fillStyle = "#232326";
  tileContext.fillRect(0, 0, 16, 16);
  tileContext.fillStyle = "#2A2A2D";
  tileContext.fillRect(0, 0, 8, 8);
  tileContext.fillRect(8, 8, 8, 8);
  checker = context.createPattern(tile, "repeat");
  return checker;
}

function draw(): void {
  const element = canvas.value;
  if (!element) return;
  const context = element.getContext("2d");
  if (!context) return;

  const dpr = window.devicePixelRatio || 1;
  context.setTransform(dpr, 0, 0, dpr, 0, 0);
  context.clearRect(0, 0, store.viewport.width, store.viewport.height);

  const current = store.preview;
  if (!image || !current) return;

  const scale = store.effectiveZoom / dpr;
  const width = current.width * scale;
  const height = current.height * scale;
  const centerX = store.viewport.width / 2 + store.view.panX;
  const centerY = store.viewport.height / 2 + store.view.panY;

  context.save();
  context.translate(centerX, centerY);
  context.rotate((store.view.rotation * Math.PI) / 2);
  context.scale(store.view.flipX ? -1 : 1, store.view.flipY ? -1 : 1);

  const pattern = ensureChecker(context);
  if (pattern) {
    context.fillStyle = pattern;
    context.fillRect(-width / 2, -height / 2, width, height);
  }

  context.imageSmoothingEnabled = store.effectiveZoom < 1;
  context.imageSmoothingQuality = "high";
  context.drawImage(image, -width / 2, -height / 2, width, height);
  context.restore();
}

function resize(): void {
  const container = host.value;
  const element = canvas.value;
  if (!container || !element) return;

  const rect = container.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  element.width = Math.max(1, Math.round(rect.width * dpr));
  element.height = Math.max(1, Math.round(rect.height * dpr));
  element.style.width = `${rect.width}px`;
  element.style.height = `${rect.height}px`;
  store.setViewport(rect.width, rect.height, dpr);
  scheduleDraw();
}

function load(url: string | null): void {
  image = null;
  if (!url) {
    scheduleDraw();
    return;
  }

  const next = new Image();
  next.decoding = "async";
  next.onload = () => {
    if (store.imageUrl !== url) return;
    image = next;
    scheduleDraw();
  };
  next.onerror = () => {
    if (store.imageUrl !== url) return;
    store.error = { code: "protocol", message: "预览资源加载失败，请重试。" };
  };
  next.src = url;
  scheduleDraw();
}

function pointerDown(event: PointerEvent): void {
  if (event.button !== 0 || !store.hasImage) return;
  dragging.value = true;
  lastPointer = { x: event.clientX, y: event.clientY };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function pointerMove(event: PointerEvent): void {
  if (!dragging.value) return;
  const deltaX = event.clientX - lastPointer.x;
  const deltaY = event.clientY - lastPointer.y;
  lastPointer = { x: event.clientX, y: event.clientY };
  store.panBy(deltaX, deltaY);
}

function pointerUp(event: PointerEvent): void {
  dragging.value = false;
  const target = event.currentTarget as HTMLElement;
  if (target.hasPointerCapture(event.pointerId)) {
    target.releasePointerCapture(event.pointerId);
  }
}

function wheel(event: WheelEvent): void {
  if (!store.hasImage) return;
  const container = host.value;
  if (!container) return;
  const rect = container.getBoundingClientRect();
  const cursorX = event.clientX - rect.left - rect.width / 2;
  const cursorY = event.clientY - rect.top - rect.height / 2;
  const factor = Math.exp(-event.deltaY * 0.0015);
  store.zoomAt(factor, cursorX, cursorY);
}

watch(() => store.imageUrl, load, { immediate: true });
watch(
  () => [
    store.effectiveZoom,
    store.view.panX,
    store.view.panY,
    store.view.rotation,
    store.view.flipX,
    store.view.flipY,
  ],
  scheduleDraw,
);

onMounted(() => {
  resizeObserver = new ResizeObserver(resize);
  if (host.value) resizeObserver.observe(host.value);
  window.addEventListener("resize", resize);
  resize();
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  window.removeEventListener("resize", resize);
  if (frame !== 0) cancelAnimationFrame(frame);
});
</script>

<template>
  <div
    ref="host"
    class="canvas-host"
    :style="{ cursor }"
    @pointerdown="pointerDown"
    @pointermove="pointerMove"
    @pointerup="pointerUp"
    @pointercancel="pointerUp"
    @dblclick="store.toggleFitActual()"
    @wheel.prevent="wheel"
  >
    <canvas ref="canvas" class="image-canvas" />

    <div v-if="store.loading" class="canvas-overlay">
      <span class="spinner" aria-hidden="true" />
      <span>{{ t("status.decoding") }}</span>
    </div>
    <div v-else-if="!store.hasImage" class="canvas-overlay canvas-empty">
      <Icon name="photo" :size="60" class="empty-icon" />
      <h2 class="empty-title">{{ t("empty.title") }}</h2>
      <p class="empty-hint">
        {{ t("empty.hintPrefix") }}<kbd>{{ modSymbol }}</kbd><kbd>O</kbd>{{ t("empty.hintSuffix") }}
      </p>
      <button type="button" class="btn btn-primary empty-action" @click="store.openDialog()">
        {{ t("empty.open") }}
      </button>
    </div>

    <div v-if="ui.dragActive" class="drop-overlay" aria-hidden="true">
      <Icon name="folder" :size="28" />
      <span>{{ t("empty.drag") }}</span>
    </div>
  </div>
</template>
