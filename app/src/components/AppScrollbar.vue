<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, useId } from "vue";

/**
 * 仿 ElementPlus ElScrollbar 的 overlay 滚动条。
 * 滚动能力仍交给原生 overflow（滚轮 / 触控板 / 触屏 / 惯性全部免费），
 * 这里只接管视觉、thumb 拖拽、轨道点击、自动隐藏与键盘。
 */
const props = withDefaults(
  defineProps<{
    direction?: "horizontal" | "vertical";
    /** thumb 沿主轴的最小长度，保证内容极长时仍可拖拽。 */
    minSize?: number;
    /** 滚动结束后自动隐藏的延迟（ms）。 */
    hideDelay?: number;
    /** 常驻显示，不做自动隐藏。 */
    alwaysVisible?: boolean;
  }>(),
  {
    direction: "horizontal",
    minSize: 24,
    hideDelay: 900,
    alwaysVisible: false,
  },
);

const emit = defineEmits<{
  scroll: [offset: number];
  resize: [size: { client: number; scroll: number }];
}>();

const root = ref<HTMLDivElement | null>(null);
/** 真正的滚动容器；bar 必须在它外面，否则会跟着内容滚走。 */
const wrap = ref<HTMLDivElement | null>(null);
const view = ref<HTMLDivElement | null>(null);
const bar = ref<HTMLDivElement | null>(null);
const thumb = ref<HTMLDivElement | null>(null);

const wrapId = useId();

const horizontal = computed(() => props.direction === "horizontal");

/** thumb 沿主轴的像素长度；只在尺寸变化时更新。 */
const thumbSize = ref(0);
/** 内容是否真的溢出主轴方向。 */
const overflowing = ref(false);
const hovering = ref(false);
const dragging = ref(false);
const scrolling = ref(false);
const focused = ref(false);

const visible = computed(
  () =>
    overflowing.value &&
    (props.alwaysVisible ||
      hovering.value ||
      dragging.value ||
      scrolling.value ||
      focused.value),
);

const thumbStyle = computed(() =>
  horizontal.value ? { width: `${thumbSize.value}px` } : { height: `${thumbSize.value}px` },
);

/** 尺寸缓存：滚动时每帧只读 scrollLeft，避免重复触发布局计算。 */
let metrics = { client: 0, scroll: 0, maxScroll: 0 };

let frame = 0;
let hideTimer = 0;

function clamp(value: number, min: number, max: number): number {
  return value < min ? min : value > max ? max : value;
}

function pointerPos(event: PointerEvent): number {
  return horizontal.value ? event.clientX : event.clientY;
}

function currentOffset(): number {
  const element = wrap.value;
  if (!element) return 0;
  return horizontal.value ? element.scrollLeft : element.scrollTop;
}

/** 重新测量视口 / 内容尺寸并重算 thumb 长度。 */
function updateSize(): void {
  const element = wrap.value;
  if (!element) return;
  const client = horizontal.value ? element.clientWidth : element.clientHeight;
  const scroll = horizontal.value ? element.scrollWidth : element.scrollHeight;
  metrics = { client, scroll, maxScroll: Math.max(0, scroll - client) };
  overflowing.value = metrics.maxScroll > 0.5;
  thumbSize.value = overflowing.value
    ? Math.min(client, Math.max(props.minSize, (client * client) / scroll))
    : 0;

  const handle = thumb.value;
  if (handle) handle.setAttribute("aria-valuemax", String(Math.round(metrics.maxScroll)));

  emit("resize", { client, scroll });
  updateOffset();
}

/** 把 scrollLeft 映射成 thumb 的位移，直接写 DOM 避免每帧重渲染。 */
function updateOffset(): void {
  const element = wrap.value;
  const handle = thumb.value;
  if (!element || !handle) return;
  const travel = metrics.client - thumbSize.value;
  const offset =
    metrics.maxScroll > 0 && travel > 0
      ? (currentOffset() / metrics.maxScroll) * travel
      : 0;
  handle.style.transform = horizontal.value
    ? `translateX(${offset}px)`
    : `translateY(${offset}px)`;

  const now = String(Math.round(currentOffset()));
  if (handle.getAttribute("aria-valuenow") !== now) {
    handle.setAttribute("aria-valuenow", now);
  }
}

/** 同帧内的多次滚动合并成一次同步。 */
function scheduleSync(): void {
  if (frame) return;
  frame = requestAnimationFrame(() => {
    frame = 0;
    updateOffset();
  });
}

function markScrolling(): void {
  if (!props.alwaysVisible) scrolling.value = true;
  if (hideTimer) clearTimeout(hideTimer);
  hideTimer = window.setTimeout(() => {
    hideTimer = 0;
    scrolling.value = false;
  }, props.hideDelay);
}

function onScroll(): void {
  const element = wrap.value;
  if (!element) return;
  markScrolling();
  if (frame) return;
  frame = requestAnimationFrame(() => {
    frame = 0;
    updateOffset();
    emit("scroll", horizontal.value ? element.scrollLeft : element.scrollTop);
  });
}

/** 鼠标竖滚轮在横向容器里转成横向滚动；触控板的 deltaX 交给原生。 */
function onWheel(event: WheelEvent): void {
  if (event.ctrlKey || metrics.maxScroll <= 0) return;
  if (event.deltaX !== 0 || event.deltaY === 0) return;
  const delta = event.deltaMode === 1 ? event.deltaY * 16 : event.deltaY;
  event.preventDefault();
  scrollTo(clamp(currentOffset() + delta, 0, metrics.maxScroll));
}

function scrollTo(offset: number, behavior: ScrollBehavior = "auto"): void {
  const element = wrap.value;
  if (!element) return;
  if (horizontal.value) element.scrollTo({ left: offset, behavior });
  else element.scrollTo({ top: offset, behavior });
}

let drag: { pointerId: number; startPos: number; startScroll: number; ratio: number } | null =
  null;

function onThumbPointerDown(event: PointerEvent): void {
  if (event.pointerType === "mouse" && event.button !== 0) return;
  const handle = thumb.value;
  if (!handle) return;
  const travel = metrics.client - thumbSize.value;
  drag = {
    pointerId: event.pointerId,
    startPos: pointerPos(event),
    startScroll: currentOffset(),
    /* thumb 被 minSize 夹住后，1/ratio 不再等于真实行程比，必须按可移动距离换算。 */
    ratio: travel > 0 ? metrics.maxScroll / travel : 0,
  };
  dragging.value = true;
  handle.setPointerCapture(event.pointerId);
  event.preventDefault();
}

function onThumbPointerMove(event: PointerEvent): void {
  if (!drag || event.pointerId !== drag.pointerId) return;
  const element = wrap.value;
  if (!element) return;
  const next = drag.startScroll + (pointerPos(event) - drag.startPos) * drag.ratio;
  if (horizontal.value) element.scrollLeft = next;
  else element.scrollTop = next;
}

function endDrag(event: PointerEvent): void {
  if (!drag || event.pointerId !== drag.pointerId) return;
  drag = null;
  dragging.value = false;
  const handle = thumb.value;
  if (handle?.hasPointerCapture(event.pointerId)) {
    handle.releasePointerCapture(event.pointerId);
  }
  scheduleSync();
}

/** 点击（或按住）轨道空白处直接跳转，thumb 中心对齐指针。 */
function onBarPointerDown(event: PointerEvent): void {
  if (event.target !== bar.value) return;
  if (event.pointerType === "mouse" && event.button !== 0) return;
  const track = bar.value;
  const travel = metrics.client - thumbSize.value;
  if (!track || travel <= 0 || metrics.maxScroll <= 0) return;
  const rect = track.getBoundingClientRect();
  const clickPos = pointerPos(event) - (horizontal.value ? rect.left : rect.top);
  const ratio = clamp((clickPos - thumbSize.value / 2) / travel, 0, 1);
  scrollTo(ratio * metrics.maxScroll, "smooth");
  event.preventDefault();
}

function onThumbKeydown(event: KeyboardEvent): void {
  const step = 80;
  const page = Math.max(40, metrics.client * 0.9);
  const current = currentOffset();
  let next: number | null = null;
  switch (event.key) {
    case "ArrowLeft":
    case "ArrowUp":
      next = current - step;
      break;
    case "ArrowRight":
    case "ArrowDown":
      next = current + step;
      break;
    case "PageUp":
      next = current - page;
      break;
    case "PageDown":
      next = current + page;
      break;
    case "Home":
      next = 0;
      break;
    case "End":
      next = metrics.maxScroll;
      break;
    default:
      return;
  }
  event.preventDefault();
  scrollTo(clamp(next, 0, metrics.maxScroll));
}

let observer: ResizeObserver | null = null;

onMounted(() => {
  observer = new ResizeObserver(updateSize);
  if (wrap.value) observer.observe(wrap.value);
  if (view.value) observer.observe(view.value);
  updateSize();
});

onBeforeUnmount(() => {
  observer?.disconnect();
  observer = null;
  if (frame) cancelAnimationFrame(frame);
  if (hideTimer) clearTimeout(hideTimer);
  frame = 0;
  hideTimer = 0;
});

defineExpose({ root, wrap, view, scrollTo, update: updateSize });
</script>

<template>
  <!-- 外层不滚动，只作为 bar 的定位基准与 hover 判定区域。 -->
  <div
    ref="root"
    class="app-scrollbar"
    :class="[
      `app-scrollbar--${direction}`,
      { 'is-visible': visible, 'is-dragging': dragging },
    ]"
    @pointerenter="hovering = true"
    @pointerleave="hovering = false"
    @wheel="onWheel"
  >
    <div :id="wrapId" ref="wrap" class="app-scrollbar__wrap" @scroll.passive="onScroll">
      <div ref="view" class="app-scrollbar__view">
        <slot />
      </div>
    </div>
    <div ref="bar" class="app-scrollbar__bar" @pointerdown="onBarPointerDown">
      <div
        ref="thumb"
        class="app-scrollbar__thumb"
        :style="thumbStyle"
        role="scrollbar"
        tabindex="0"
        aria-valuemin="0"
        aria-valuemax="0"
        :aria-controls="wrapId"
        :aria-orientation="direction"
        @pointerdown.stop="onThumbPointerDown"
        @pointermove="onThumbPointerMove"
        @pointerup="endDrag"
        @pointercancel="endDrag"
        @lostpointercapture="endDrag"
        @focus="focused = true"
        @blur="focused = false"
        @keydown="onThumbKeydown"
      />
    </div>
  </div>
</template>
