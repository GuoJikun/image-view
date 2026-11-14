<script setup lang="ts">
/** 胶囊缩放分段控件 118 × 30：− / 百分比（点击 = 适配窗口）/ +。 */
import { computed } from "vue";
import { useViewerStore } from "../stores/viewer";
import { accel, tip } from "../platform";
import { t } from "../i18n";
import Icon from "./Icon.vue";

const store = useViewerStore();

const value = computed(() => (store.hasImage ? `${store.zoomPercent}%` : "—"));
const fitTip = computed(() => tip(t("toolbar.fit"), accel("mod", "0")));
</script>

<template>
  <div class="zoom-capsule" :class="{ empty: !store.hasImage }">
    <button
      type="button"
      class="zoom-step"
      :data-tip="tip(t('toolbar.zoomOut'), accel('mod', '-'))"
      :aria-label="t('toolbar.zoomOut')"
      :disabled="!store.hasImage"
      @click="store.zoomBy(0.8)"
    >
      <Icon name="minus" :size="13" />
    </button>
    <button
      type="button"
      class="zoom-value"
      :data-tip="fitTip"
      :aria-label="fitTip"
      :disabled="!store.hasImage"
      @click="store.setFit()"
    >
      {{ value }}
    </button>
    <button
      type="button"
      class="zoom-step"
      :data-tip="tip(t('toolbar.zoomIn'), accel('mod', '='))"
      :aria-label="t('toolbar.zoomIn')"
      :disabled="!store.hasImage"
      @click="store.zoomBy(1.25)"
    >
      <Icon name="plus" :size="13" />
    </button>
  </div>
</template>
