<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useViewerStore } from "../stores/viewer";
import { thumbnailUrl } from "../services/ipc";

const store = useViewerStore();
const scroller = ref<HTMLDivElement | null>(null);
const scrollLeft = ref(0);
const viewportWidth = ref(0);
/** 卡片 76 × 56 + 间距 9；左右内边距 16（设计 4.3）。 */
const ITEM_WIDTH = 85;
const PAD = 16;
const OVERSCAN = 4;

let observer: ResizeObserver | null = null;

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

function onScroll(): void {
  scrollLeft.value = scroller.value?.scrollLeft ?? 0;
}

function measure(): void {
  viewportWidth.value = scroller.value?.clientWidth ?? 0;
}

function centerCurrent(behavior: ScrollBehavior = "smooth"): void {
  const index = store.currentIndex;
  const element = scroller.value;
  if (index < 0 || !element) return;
  const target = PAD + index * ITEM_WIDTH - element.clientWidth / 2 + 38;
  element.scrollTo({ left: Math.max(0, target), behavior });
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
    if (scroller.value) scroller.value.scrollLeft = 0;
    await nextTick();
    centerCurrent("auto");
  },
);

onMounted(() => {
  observer = new ResizeObserver(measure);
  if (scroller.value) observer.observe(scroller.value);
  measure();
  nextTick(() => centerCurrent("auto"));
});

onBeforeUnmount(() => observer?.disconnect());
</script>

<template>
  <section ref="scroller" class="thumbnail-strip" aria-label="缩略图" @scroll.passive="onScroll">
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
          :src="thumbnailUrl(entry.id, 128, 0)"
          :alt="entry.name"
          loading="lazy"
          decoding="async"
          draggable="false"
        />
      </button>
    </div>
  </section>
</template>
