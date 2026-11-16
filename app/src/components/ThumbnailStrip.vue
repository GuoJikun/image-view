<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useViewerStore } from "../stores/viewer";
import { thumbnailUrl } from "../services/ipc";
import AppScrollbar from "./AppScrollbar.vue";

const store = useViewerStore();
const strip = ref<InstanceType<typeof AppScrollbar> | null>(null);
const scrollLeft = ref(0);
const viewportWidth = ref(0);
/** 卡片 76 × 56 + 间距 9；左右内边距 16（设计 4.3）。 */
const ITEM_WIDTH = 85;
const PAD = 16;
const OVERSCAN = 4;

const entries = computed(() => store.snapshot?.entries ?? []);
const startIndex = computed(() =>
  Math.max(0, Math.floor(scrollLeft.value / ITEM_WIDTH) - OVERSCAN),
);
const visibleCount = computed(
  () => Math.ceil(viewportWidth.value / ITEM_WIDTH) + OVERSCAN * 2,
);
const visible = computed(() =>
  entries.value
    .slice(startIndex.value, startIndex.value + visibleCount.value)
    .map((entry, offset) => ({ entry, index: startIndex.value + offset })),
);
const trackWidth = computed(() => `${PAD * 2 + entries.value.length * ITEM_WIDTH}px`);

function onScroll(offset: number): void {
  scrollLeft.value = offset;
}

function onResize(size: { client: number; scroll: number }): void {
  viewportWidth.value = size.client;
}

function measure(): void {
  viewportWidth.value = strip.value?.wrap?.clientWidth ?? 0;
}

function centerCurrent(behavior: ScrollBehavior = "smooth"): void {
  const index = store.currentIndex;
  const element = strip.value?.wrap;
  if (index < 0 || !element) return;
  const target = PAD + index * ITEM_WIDTH - element.clientWidth / 2 + 38;
  strip.value?.scrollTo(Math.max(0, target), behavior);
}

watch(
  () => store.currentIndex,
  async () => {
    await nextTick();
    centerCurrent();
  },
);

watch(
  () => store.snapshot?.dir,
  async () => {
    scrollLeft.value = 0;
    strip.value?.scrollTo(0, "auto");
    await nextTick();
    centerCurrent("auto");
  },
);

/* 条目数量变化后内容宽度才落定，补一次测量，避免 thumb 停在旧比例上。 */
watch(
  () => entries.value.length,
  async () => {
    await nextTick();
    strip.value?.update();
  },
);

onMounted(() => {
  measure();
  nextTick(() => centerCurrent("auto"));
});
</script>

<template>
  <AppScrollbar
    ref="strip"
    class="thumbnail-strip"
    aria-label="缩略图"
    @scroll="onScroll"
    @resize="onResize"
  >
    <div class="thumb-track" :style="{ width: trackWidth }">
      <button
        v-for="{ entry, index } in visible"
        :key="entry.id"
        type="button"
        class="thumb-item"
        :class="{ current: index === store.currentIndex }"
        :style="{ left: `${PAD + index * ITEM_WIDTH}px` }"
        :title="entry.name"
        @click="store.select(index)"
      >
        <img
          :src="thumbnailUrl(entry.id, 128, entry.revision)"
          :alt="entry.name"
          loading="lazy"
          decoding="async"
          draggable="false"
        />
      </button>
    </div>
  </AppScrollbar>
</template>
