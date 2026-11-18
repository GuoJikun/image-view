<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, watch, watchEffect } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useViewerStore } from "./stores/viewer";
import { useUiStore } from "./stores/ui";
import { useActions } from "./actions";
import { usePreferences } from "./preferences";
import { inTauri, platform } from "./platform";
import { t } from "./i18n";
import * as ipc from "./services/ipc";
import { colorTarget, onColorTargetChange } from "./services/color";
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
const { run, syncNativeMenu, menuState } = useActions();
const { load: loadPreferences, ready: preferencesReady } = usePreferences();

// macOS 原生菜单的勾选状态跟随界面：任何开关变化都推给后端，
// 语言变化顺带重建菜单（后端按语言是否变化决定重建还是只改勾选）。
// 偏好读取完成前不推，否则会先用默认值重建一次菜单（启动瞬间菜单栏语言来回变）。
watch([menuState, preferencesReady], () => {
  if (preferencesReady.value) void syncNativeMenu();
});

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
let stopColorWatch: (() => void) | null = null;

function isTextInput(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return Boolean(target.closest("input, textarea, select, [contenteditable='true']"));
}

function isButton(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && Boolean(target.closest("button, [role='menuitem']"));
}

// WebView 自带的右键菜单（重新加载、另存为图片、检查元素）对看图没有意义，整窗禁用。
// 以后若加入输入框且需要系统粘贴菜单，在这里按 event.target 放行。
function onContextMenu(event: MouseEvent): void {
  event.preventDefault();
}

async function onKeyDown(event: KeyboardEvent): Promise<void> {
  if (isTextInput(event.target)) return;

  if (event.key === "Escape") {
    if (ui.helpOpen) {
      event.preventDefault();
      ui.helpOpen = false;
      return;
    }
    // 更新弹窗只能通过「关闭」按钮退出，避免检查或下载过程中被误关。
    if (ui.updateOpen) {
      event.preventDefault();
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
      void run("rotate-cw");
      break;
    case "R":
      void run("rotate-ccw");
      break;
    case "h":
    case "H":
      void run("flip-h");
      break;
    case "v":
    case "V":
      void run("flip-v");
      break;
    case " ":
      if (event.repeat || isButton(event.target)) return;
      event.preventDefault();
      // 有动画时优先控制动画，没有动画时才控制幻灯片。
      store.togglePlayback();
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
  window.addEventListener("contextmenu", onContextMenu);
  // 目标色彩空间必须在首次解码前告诉后端，否则第一张图会按 sRGB 编码。
  // 探测本身是同步的，模块加载时就已完成，这里只负责回传。
  if (inTauri) await ipc.setColorTarget(colorTarget);
  await store.initialize();
  // 偏好来自 settings.ron，读取后覆盖默认值；随后的菜单同步才能拿到真实勾选状态。
  await loadPreferences();

  if (!inTauri) return;

  // 窗口拖到另一块屏、HDR 开关都会改变显示能力：重新回传并重取当前图片。
  // 画布空间在首次 getContext 时已固定，P3 收益要下次启动才生效，但颜色始终正确
  // （预览 PNG 带 iCCP 标签，画布空间与标签不一致时浏览器会自行换算）。
  stopColorWatch = onColorTargetChange((next) => {
    void ipc.setColorTarget(next).then((changed) => {
      if (changed) void store.requestCurrent();
    });
  });

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

  // 启动时自动检查更新（静默；发现新版本只提示，不自动下载）。
  if (ui.checkUpdatesOnLaunch) {
    void ipc
      .checkUpdate()
      .then((result) => {
        if (result) ui.notify(`${t("update.available")} — ${result.version}`);
      })
      .catch(() => {
        // 静默失败；手动检查时才展示错误。
      });
  }
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeyDown);
  window.removeEventListener("contextmenu", onContextMenu);
  unlistenDragDrop?.();
  unlistenMenu?.();
  unlistenResize?.();
  stopColorWatch?.();
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
    <Transition name="strip">
      <ThumbnailStrip v-if="stripVisible" />
    </Transition>
    <StatusBar />

    <HelpDialog v-if="ui.helpOpen" @close="ui.helpOpen = false" />
    <UpdateDialog v-if="ui.updateOpen" @close="ui.updateOpen = false" />
  </div>
</template>
