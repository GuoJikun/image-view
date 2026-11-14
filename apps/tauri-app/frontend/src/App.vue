<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, watchEffect } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useViewerStore } from "./stores/viewer";
import { useUiStore } from "./stores/ui";
import { useActions } from "./actions";
import { inTauri, platform } from "./platform";
import { t } from "./i18n";
import * as ipc from "./services/ipc";
import AppToolbar from "./components/AppToolbar.vue";
import ErrorBanner from "./components/ErrorBanner.vue";
import HelpDialog from "./components/HelpDialog.vue";
import ImageCanvas from "./components/ImageCanvas.vue";
import MetadataPanel from "./components/MetadataPanel.vue";
import StatusBar from "./components/StatusBar.vue";
import ThumbnailStrip from "./components/ThumbnailStrip.vue";
import TitleBar from "./components/TitleBar.vue";
import UpdateDialog from "./components/UpdateDialog.vue";

const store = useViewerStore();
const ui = useUiStore();
const { run, syncNativeMenu } = useActions();

const stripVisible = computed(
  () => store.showThumbnails && (store.snapshot?.entries.length ?? 0) > 0,
);

const windowTitle = computed(() =>
  store.preview ? `${store.preview.fileName} — ${t("app.name")}` : t("app.name"),
);

// 原生标题栏（macOS）与文档标题同步文件名。
watchEffect(() => {
  document.title = windowTitle.value;
  if (platform === "macos" && inTauri) {
    void getCurrentWindow()
      .setTitle(windowTitle.value)
      .catch(() => undefined);
  }
});

let unlistenDragDrop: (() => void) | null = null;
let unlistenMenu: (() => void) | null = null;
let unlistenResize: (() => void) | null = null;

function isTextInput(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return Boolean(target.closest("input, textarea, select, [contenteditable='true']"));
}

function isButton(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && Boolean(target.closest("button, [role='menuitem']"));
}

async function onKeyDown(event: KeyboardEvent): Promise<void> {
  if (isTextInput(event.target)) return;

  if (event.key === "Escape") {
    if (ui.helpOpen || ui.updateOpen) {
      event.preventDefault();
      ui.helpOpen = false;
      ui.updateOpen = false;
      return;
    }
    store.clearError();
    return;
  }

  if (ui.helpOpen || ui.updateOpen) return;

  const mod = event.ctrlKey || event.metaKey;
  const key = event.key;

  if (mod && !event.altKey) {
    const lower = key.toLowerCase();
    if (lower === "o") {
      event.preventDefault();
      void run("open");
      return;
    }
    if (lower === "c" && event.shiftKey) {
      event.preventDefault();
      void run("copy-path");
      return;
    }
    if (event.shiftKey) return;
    if (key === "=" || key === "+") {
      event.preventDefault();
      void run("zoom-in");
      return;
    }
    if (key === "-") {
      event.preventDefault();
      void run("zoom-out");
      return;
    }
    if (key === "0") {
      event.preventDefault();
      void run("fit");
      return;
    }
    if (key === "1") {
      event.preventDefault();
      void run("actual");
      return;
    }
    if (key === "/") {
      event.preventDefault();
      void run("shortcuts");
    }
    return;
  }

  if (event.altKey) return;

  switch (key) {
    case "ArrowLeft":
      event.preventDefault();
      void store.step(-1);
      break;
    case "ArrowRight":
      event.preventDefault();
      void store.step(1);
      break;
    case "+":
    case "=":
      event.preventDefault();
      store.zoomBy(1.25);
      break;
    case "-":
      event.preventDefault();
      store.zoomBy(0.8);
      break;
    case "0":
      event.preventDefault();
      store.setFit();
      break;
    case "1":
      event.preventDefault();
      store.setActual();
      break;
    case "f":
    case "F":
      store.toggleFitActual();
      break;
    case "r":
      store.rotate(1);
      break;
    case "R":
      store.rotate(-1);
      break;
    case "h":
    case "H":
      store.toggleFlipX();
      break;
    case "v":
    case "V":
      store.toggleFlipY();
      break;
    case " ":
      if (event.repeat || isButton(event.target)) return;
      event.preventDefault();
      store.toggleSlideshow();
      break;
    case "Delete":
    case "Backspace":
      event.preventDefault();
      void run("trash");
      break;
    case "F11":
      event.preventDefault();
      void ui.toggleFullscreen();
      break;
    default:
      break;
  }
}

onMounted(async () => {
  window.addEventListener("keydown", onKeyDown);
  await store.initialize();

  if (!inTauri) return;

  // macOS 原生菜单按当前语言重建（启动时同步一次，切换语言时再同步）。
  void syncNativeMenu();

  try {
    unlistenDragDrop = await getCurrentWebview().onDragDropEvent((event) => {
      const payload = event.payload;
      if (payload.type === "enter" || payload.type === "over") {
        ui.dragActive = true;
      } else if (payload.type === "leave") {
        ui.dragActive = false;
      } else if (payload.type === "drop") {
        ui.dragActive = false;
        const first = payload.paths[0];
        if (first) void store.openPath(first);
      }
    });
  } catch {
    // 拖放不可用时仍可用对话框/命令行打开。
  }

  try {
    unlistenMenu = await ipc.onMenuAction((id) => void run(id));
  } catch {
    // macOS 原生菜单不可用时忽略。
  }

  try {
    unlistenResize = await getCurrentWindow().onResized(() => {
      void getCurrentWindow()
        .isFullscreen()
        .then((value) => ui.syncFullscreen(value))
        .catch(() => undefined);
    });
  } catch {
    // 忽略窗口事件不可用。
  }
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeyDown);
  unlistenDragDrop?.();
  unlistenMenu?.();
  unlistenResize?.();
  store.stopSlideshow();
});
</script>

<template>
  <div class="app-shell" :class="{ 'is-macos': platform === 'macos', 'is-fullscreen': ui.fullscreen }">
    <TitleBar v-if="platform !== 'macos'" />
    <AppToolbar />
    <ErrorBanner
      v-if="store.error"
      :error="store.error"
      @close="store.clearError()"
      @retry="store.requestCurrent()"
    />
    <main class="workspace">
      <ImageCanvas />
      <MetadataPanel v-if="store.showMetadata" />
    </main>
    <ThumbnailStrip v-if="stripVisible" />
    <StatusBar />

    <HelpDialog v-if="ui.helpOpen" @close="ui.helpOpen = false" />
    <UpdateDialog v-if="ui.updateOpen" @close="ui.updateOpen = false" />
  </div>
</template>
