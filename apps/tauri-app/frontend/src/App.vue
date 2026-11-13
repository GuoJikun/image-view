<script setup lang="ts">
import { onBeforeUnmount, onMounted } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useViewerStore } from "./stores/viewer";
import { isTauri } from "./services/ipc";
import AppToolbar from "./components/AppToolbar.vue";
import ErrorBanner from "./components/ErrorBanner.vue";
import ImageCanvas from "./components/ImageCanvas.vue";
import MetadataPanel from "./components/MetadataPanel.vue";
import StatusBar from "./components/StatusBar.vue";
import ThumbnailStrip from "./components/ThumbnailStrip.vue";

const store = useViewerStore();
let unlistenDragDrop: (() => void) | null = null;

function isTextInput(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return Boolean(target.closest("input, textarea, select, [contenteditable='true']"));
}

function onKeyDown(event: KeyboardEvent): void {
  if (isTextInput(event.target)) return;

  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "o") {
    event.preventDefault();
    void store.openDialog();
    return;
  }
  if (event.ctrlKey || event.metaKey || event.altKey) return;

  switch (event.key) {
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
      store.setFit();
      break;
    case "1":
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
    case "Escape":
      store.clearError();
      break;
    default:
      break;
  }
}

onMounted(async () => {
  window.addEventListener("keydown", onKeyDown);
  await store.initialize();

  if (isTauri()) {
    try {
      unlistenDragDrop = await getCurrentWebview().onDragDropEvent((event) => {
        if (event.payload.type === "drop" && event.payload.paths.length > 0) {
          void store.openPath(event.payload.paths[0]);
        }
      });
    } catch {
      // 拖放不可用时仍可用对话框/命令行打开。
    }
  }
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeyDown);
  unlistenDragDrop?.();
});
</script>

<template>
  <div class="app-shell">
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
    <ThumbnailStrip v-if="store.showThumbnails" />
    <StatusBar />
  </div>
</template>
