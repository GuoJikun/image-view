<script setup lang="ts">
/**
 * 工具栏 52 px（三端一致）：左侧「打开 + ‹ ›」，右侧「缩略图 · 幻灯片 · 排序 | 胶囊缩放」。
 * 次级操作全部在菜单栏，工具栏不再有 ⋯ 溢出按钮。
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useViewerStore } from "../stores/viewer";
import { useUiStore } from "../stores/ui";
import { accel, tip } from "../platform";
import { t } from "../i18n";
import Icon from "./Icon.vue";
import ZoomControl from "./ZoomControl.vue";

const store = useViewerStore();
const ui = useUiStore();

const sortOpen = ref(false);
const sortWrap = ref<HTMLElement | null>(null);

const sortLabel = computed(() =>
  store.snapshot?.sort === "modified" ? t("menu.sortModified") : t("menu.sortName"),
);

const openTip = computed(() => tip(t("toolbar.open"), accel("mod", "O")));
const thumbTip = computed(() => t("toolbar.thumbnails"));
const slideshowTip = computed(() =>
  store.slideshow ? t("toolbar.pauseSlideshow") : tip(t("toolbar.slideshow"), "Space"),
);

function chooseSort(order: "name" | "modified"): void {
  sortOpen.value = false;
  void store.changeSort(order);
}

function onPointerDown(event: PointerEvent): void {
  if (!sortOpen.value) return;
  if (!sortWrap.value?.contains(event.target as Node)) sortOpen.value = false;
}

function onKeyDown(event: KeyboardEvent): void {
  if (event.key === "Escape" && sortOpen.value) {
    event.stopPropagation();
    sortOpen.value = false;
  }
}

onMounted(() => {
  document.addEventListener("pointerdown", onPointerDown);
  window.addEventListener("keydown", onKeyDown, true);
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onPointerDown);
  window.removeEventListener("keydown", onKeyDown, true);
});
</script>

<template>
  <header class="toolbar">
    <div class="bar-left">
      <button
        type="button"
        class="icon-button"
        :data-tip="openTip"
        :aria-label="t('toolbar.open')"
        @click="store.openDialog()"
      >
        <Icon name="folder" />
      </button>

      <span class="vdivider" />

      <button
        type="button"
        class="icon-button"
        :data-tip="tip(t('toolbar.prev'), '←')"
        :aria-label="t('toolbar.prev')"
        :disabled="!store.canPrev"
        @click="store.step(-1)"
      >
        <Icon name="chevron-left" />
      </button>
      <button
        type="button"
        class="icon-button"
        :data-tip="tip(t('toolbar.next'), '→')"
        :aria-label="t('toolbar.next')"
        :disabled="!store.canNext"
        @click="store.step(1)"
      >
        <Icon name="chevron-right" />
      </button>
    </div>

    <Transition name="fade">
      <span v-if="ui.showFullscreenTitle && store.preview" class="toolbar-filename">
        {{ store.preview.fileName }}
      </span>
    </Transition>

    <div class="bar-right">
      <button
        type="button"
        class="icon-button"
        :class="{ active: store.showThumbnails }"
        :data-tip="thumbTip"
        :aria-label="thumbTip"
        :aria-pressed="store.showThumbnails"
        @click="store.showThumbnails = !store.showThumbnails"
      >
        <Icon name="sidebar" />
      </button>

      <button
        type="button"
        class="icon-button"
        :class="{ active: store.slideshow }"
        :data-tip="slideshowTip"
        :aria-label="slideshowTip"
        :aria-pressed="store.slideshow"
        :disabled="!store.hasImage"
        @click="store.toggleSlideshow()"
      >
        <Icon :name="store.slideshow ? 'pause' : 'play'" />
      </button>

      <div ref="sortWrap" class="sort-wrap">
        <button
          type="button"
          class="icon-button text"
          :class="{ open: sortOpen }"
          :data-tip="t('menu.sort')"
          :aria-label="t('menu.sort')"
          aria-haspopup="true"
          :aria-expanded="sortOpen"
          :disabled="!store.snapshot?.entries.length"
          @click="sortOpen = !sortOpen"
        >
          <span>{{ sortLabel }}</span>
          <Icon name="chevron-down" :size="13" />
        </button>

        <div v-if="sortOpen" class="menu-panel sort-menu" role="menu">
          <button
            type="button"
            class="menu-row"
            role="menuitemradio"
            :aria-checked="store.snapshot?.sort !== 'modified'"
            @click="chooseSort('name')"
          >
            <span class="menu-gutter">{{ store.snapshot?.sort !== "modified" ? "•" : "" }}</span>
            <span class="menu-label">{{ t("menu.sortName") }}</span>
          </button>
          <button
            type="button"
            class="menu-row"
            role="menuitemradio"
            :aria-checked="store.snapshot?.sort === 'modified'"
            @click="chooseSort('modified')"
          >
            <span class="menu-gutter">{{ store.snapshot?.sort === "modified" ? "•" : "" }}</span>
            <span class="menu-label">{{ t("menu.sortModified") }}</span>
          </button>
        </div>
      </div>

      <span class="vdivider" />

      <ZoomControl />
    </div>
  </header>
</template>
