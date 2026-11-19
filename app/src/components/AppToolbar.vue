<script setup lang="ts">
/**
 * 工具栏 52 px（三端一致）：左侧「打开 + ‹ ›」，右侧「缩略图 · 幻灯片 · 排序 | 胶囊缩放」。
 * 次级操作全部在菜单栏，工具栏不再有 ⋯ 溢出按钮。
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useViewerStore } from "../stores/viewer";
import { useUiStore } from "../stores/ui";
import { useActions } from "../actions";
import { accel, tip } from "../platform";
import { t } from "../i18n";
import Icon from "./Icon.vue";
import ZoomControl from "./ZoomControl.vue";

const store = useViewerStore();
const ui = useUiStore();
const { run } = useActions();

const sortOpen = ref(false);
const sortWrap = ref<HTMLElement | null>(null);

const sortLabel = computed(() =>
  store.snapshot?.sort === "modified" ? t("menu.sortModified") : t("menu.sortName"),
);

const openTip = computed(() => tip(t("toolbar.open"), accel("mod", "O")));
const thumbTip = computed(() => t("toolbar.thumbnails"));
/** 有动画时按钮控制动画，否则控制幻灯片——与 `Space` 的绑定保持一致。 */
const playbackTip = computed(() => {
  if (store.animation) {
    return tip(
      store.animationPlaying ? t("toolbar.pauseAnimation") : t("toolbar.animation"),
      "Space",
    );
  }
  return store.slideshow ? t("toolbar.pauseSlideshow") : tip(t("toolbar.slideshow"), "Space");
});

/** 旋转与翻转：与「显示」菜单、R / Shift+R / H / V 快捷键同源。 */
const rotateCwTip = computed(() => tip(t("menu.rotateCW"), "R"));
const rotateCcwTip = computed(() => tip(t("menu.rotateCCW"), accel("shift", "R")));
const flipHTip = computed(() => tip(t("menu.flipH"), "H"));
const flipVTip = computed(() => tip(t("menu.flipV"), "V"));

function act(id: string): void {
  void run(id);
}

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

      <span class="vdivider" />

      <button
        type="button"
        class="icon-button"
        :data-tip="rotateCwTip"
        :aria-label="rotateCwTip"
        :disabled="!store.hasImage"
        @click="act('rotate-cw')"
      >
        <Icon name="rotate-cw" />
      </button>
      <button
        type="button"
        class="icon-button"
        :data-tip="rotateCcwTip"
        :aria-label="rotateCcwTip"
        :disabled="!store.hasImage"
        @click="act('rotate-ccw')"
      >
        <Icon name="rotate-ccw" />
      </button>
      <button
        type="button"
        class="icon-button"
        :data-tip="flipHTip"
        :aria-label="flipHTip"
        :disabled="!store.hasImage"
        @click="act('flip-h')"
      >
        <Icon name="flip-h" />
      </button>
      <button
        type="button"
        class="icon-button"
        :data-tip="flipVTip"
        :aria-label="flipVTip"
        :disabled="!store.hasImage"
        @click="act('flip-v')"
      >
        <Icon name="flip-v" />
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
        :class="{ active: store.playbackActive }"
        :data-tip="playbackTip"
        :aria-label="playbackTip"
        :aria-pressed="store.playbackActive"
        :disabled="!store.hasImage"
        @click="store.togglePlayback()"
      >
        <Icon :name="store.playbackActive ? 'pause' : 'play'" />
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
          <Icon name="chevron-down" :size="12" />
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
