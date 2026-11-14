<script setup lang="ts">
/**
 * Windows / Linux 自绘标题栏（28 px）：菜单栏居左、文件名居中、窗口按钮居右。
 * macOS 使用系统原生标题栏，本组件不渲染。
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { inTauri, isWindows } from "../platform";
import { t } from "../i18n";
import { useViewerStore } from "../stores/viewer";
import MenuBar from "./MenuBar.vue";
import Icon from "./Icon.vue";

const store = useViewerStore();
const maximized = ref(false);

/** 只在 Tauri 内调用：浏览器预览没有 __TAURI_INTERNALS__。 */
function currentWindow() {
  return getCurrentWindow();
}

const title = computed(() =>
  store.preview ? `${store.preview.fileName} — ${t("app.name")}` : t("app.name"),
);

async function syncMaximized(): Promise<void> {
  if (!inTauri) return;
  try {
    maximized.value = await currentWindow().isMaximized();
  } catch {
    // 窗口信息不可用时保持上一次状态。
  }
}

let unlisten: (() => void) | null = null;

onMounted(async () => {
  if (!inTauri) return;
  await syncMaximized();
  try {
    unlisten = await currentWindow().onResized(() => void syncMaximized());
  } catch {
    // 浏览器预览没有窗口事件。
  }
});

onBeforeUnmount(() => unlisten?.());

function minimize(): void {
  if (inTauri) void currentWindow().minimize();
}

function toggleMaximize(): void {
  if (inTauri) void currentWindow().toggleMaximize();
}

function close(): void {
  if (inTauri) void currentWindow().close();
}
</script>

<template>
  <header class="titlebar" data-tauri-drag-region="deep">
    <MenuBar />

    <div class="titlebar-title" :title="title">{{ title }}</div>

    <div v-if="isWindows" class="caption-buttons">
      <button
        type="button"
        class="caption-button"
        :data-tip="t('window.minimize')"
        :aria-label="t('window.minimize')"
        @click="minimize"
      >
        <Icon name="minimize" :size="11" />
      </button>
      <button
        type="button"
        class="caption-button"
        :data-tip="maximized ? t('window.restore') : t('window.maximize')"
        :aria-label="maximized ? t('window.restore') : t('window.maximize')"
        @click="toggleMaximize"
      >
        <Icon :name="maximized ? 'restore' : 'maximize'" :size="11" />
      </button>
      <button
        type="button"
        class="caption-button close"
        :data-tip="t('window.close')"
        :aria-label="t('window.close')"
        @click="close"
      >
        <Icon name="close" :size="11" />
      </button>
    </div>

    <div v-else class="linux-buttons">
      <button
        type="button"
        class="linux-button"
        :data-tip="t('window.minimize')"
        :aria-label="t('window.minimize')"
        @click="minimize"
      >
        <Icon name="minimize" :size="13" />
      </button>
      <button
        type="button"
        class="linux-button"
        :data-tip="maximized ? t('window.restore') : t('window.maximize')"
        :aria-label="maximized ? t('window.restore') : t('window.maximize')"
        @click="toggleMaximize"
      >
        <Icon :name="maximized ? 'restore' : 'maximize'" :size="13" />
      </button>
      <button
        type="button"
        class="linux-button close"
        :data-tip="t('window.close')"
        :aria-label="t('window.close')"
        @click="close"
      >
        <Icon name="close" :size="13" />
      </button>
    </div>
  </header>
</template>
