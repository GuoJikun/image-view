<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useGalleryStore } from "../stores/gallery";
import { useCompareStore } from "../stores/compare";
import { useUiStore } from "../stores/ui";
import { openPath, thumbnailUrl } from "../services/ipc";
import { t } from "../i18n";
import AppScrollbar from "./AppScrollbar.vue";

const gallery = useGalleryStore();
const compare = useCompareStore();
const ui = useUiStore();

const CARD_WIDTH = 140;
const CARD_HEIGHT = 116;
const GAP = 12;
const PAD = 16;
const OVERSCAN = 2;

const scroller = ref<InstanceType<typeof AppScrollbar> | null>(null);
const scrollTop = ref(0);
const viewportHeight = ref(0);
const viewportWidth = ref(0);
const queryInput = ref("");

const columns = computed(() =>
  Math.max(1, Math.floor((viewportWidth.value - PAD * 2 + GAP) / (CARD_WIDTH + GAP))),
);
const rowHeight = CARD_HEIGHT + GAP;
const totalRows = computed(() => Math.ceil(gallery.entries.length / columns.value));
const startRow = computed(() => Math.max(0, Math.floor(scrollTop.value / rowHeight) - OVERSCAN));
const visibleRows = computed(
  () => Math.ceil(viewportHeight.value / rowHeight) + OVERSCAN * 2 + 1,
);
const visible = computed(() => {
  const start = startRow.value * columns.value;
  const count = visibleRows.value * columns.value;
  return gallery.entries.slice(start, start + count).map((entry, offset) => ({
    entry,
    index: start + offset,
  }));
});
const trackHeight = computed(() => PAD * 2 + totalRows.value * rowHeight - GAP);

function onScroll(offset: number): void {
  scrollTop.value = offset;
  // 触底加载下一页。
  if (offset + viewportHeight.value >= trackHeight.value - 200) void gallery.loadMore();
}

function onResize(size: { client: number; scroll: number; width?: number }): void {
  viewportHeight.value = size.client;
}

function measure(): void {
  viewportHeight.value = scroller.value?.wrap?.clientHeight ?? 0;
  viewportWidth.value = scroller.value?.wrap?.clientWidth ?? 0;
}

async function submitSearch(): Promise<void> {
  gallery.filter.query = queryInput.value;
  await gallery.search();
  scroller.value?.scrollTo(0, "auto");
}

function openEntry(id: string): void {
  // 从图库打开：定位到当前目录列表中的同一文件，进入查看器。
  const entry = gallery.entries.find((item) => item.id === id);
  if (!entry) return;
  void openPath(entry.path).then(() => {
    ui.activeView = "viewer";
  });
}

async function startCompare(): Promise<void> {
  const chosen = gallery.entries.filter((entry) => gallery.selectedIds.includes(entry.id));
  if (chosen.length < 2 || chosen.length > 4) return;
  if (await compare.enter(chosen)) ui.activeView = "compare";
}

watch(
  () => gallery.entries.length,
  async () => {
    await nextTick();
    scroller.value?.update();
  },
);

onMounted(() => {
  measure();
  queryInput.value = gallery.filter.query;
});
</script>

<template>
  <div class="gallery-view">
    <div class="gallery-toolbar">
      <form class="gallery-search" @submit.prevent="submitSearch">
        <input
          v-model="queryInput"
          type="search"
          :placeholder="t('gallery.search')"
          :aria-label="t('gallery.search')"
        />
        <button type="submit">{{ t("gallery.filter") }}</button>
      </form>
      <select
        v-model="gallery.filter.format"
        :aria-label="t('gallery.filter')"
        @change="submitSearch"
      >
        <option value="">{{ t("gallery.allFormats") }}</option>
        <option v-for="fmt in ['PNG', 'JPEG', 'GIF', 'WEBP', 'HEIC', 'AVIF']" :key="fmt" :value="fmt.toLowerCase()">
          {{ fmt }}
        </option>
      </select>
      <select v-model="gallery.filter.sort" :aria-label="t('gallery.filter')" @change="submitSearch">
        <option value="name">{{ t("gallery.sortName") }}</option>
        <option value="modified">{{ t("gallery.sortModified") }}</option>
        <option value="size">{{ t("gallery.sortSize") }}</option>
        <option value="format">{{ t("gallery.sortFormat") }}</option>
      </select>
      <span class="gallery-progress">
        <template v-if="!gallery.indexReady">{{ t("gallery.indexing", { indexed: gallery.indexed, total: gallery.total }) }}</template>
        <template v-else>{{ gallery.total }}</template>
      </span>
      <button
        type="button"
        class="gallery-compare"
        :disabled="gallery.selectedIds.length < 2 || gallery.selectedIds.length > 4"
        :title="t('gallery.compareHint')"
        @click="startCompare"
      >
        {{ t("gallery.compare") }} ({{ gallery.selectedIds.length }})
      </button>
    </div>

    <div v-if="gallery.degraded" class="gallery-notice">{{ t("gallery.degraded") }}</div>

    <AppScrollbar
      ref="scroller"
      class="gallery-scroller"
      direction="vertical"
      @scroll="onScroll"
      @resize="onResize"
    >
      <div class="gallery-grid" :style="{ height: `${trackHeight}px` }">
        <button
          v-for="{ entry, index } in visible"
          :key="entry.id"
          type="button"
          class="gallery-card"
          :class="{ selected: gallery.selectedIds.includes(entry.id) }"
          :style="{
            left: `${PAD + (index % columns) * (CARD_WIDTH + GAP)}px`,
            top: `${PAD + Math.floor(index / columns) * rowHeight}px`,
            width: `${CARD_WIDTH}px`,
          }"
          :title="entry.name"
          @click="gallery.toggleSelected(entry.id, true)"
          @dblclick="openEntry(entry.id)"
        >
          <img
            :src="thumbnailUrl(entry.id, 160, entry.revision)"
            :alt="entry.name"
            loading="lazy"
            decoding="async"
            draggable="false"
          />
          <span class="gallery-card-name">{{ entry.name }}</span>
          <span v-if="entry.tags.length" class="gallery-card-tags">{{ entry.tags.join(" · ") }}</span>
        </button>
      </div>
    </AppScrollbar>

    <div v-if="gallery.entries.length === 0 && gallery.indexReady" class="gallery-empty">
      {{ t("gallery.empty") }}
    </div>
  </div>
</template>
