/**
 * 菜单、工具栏、键盘共用的一套动作分发。
 * 动作 id 与 macOS 原生菜单（Rust 侧）保持一致。
 */
import { useViewerStore } from "./stores/viewer";
import { useUiStore, type ThemeSetting } from "./stores/ui";
import { setLocale, resolvedLocale, t, type LocaleSetting } from "./i18n";
import * as ipc from "./services/ipc";

function fallbackCopy(text: string): boolean {
  const area = document.createElement("textarea");
  area.value = text;
  area.setAttribute("readonly", "true");
  area.style.position = "fixed";
  area.style.opacity = "0";
  document.body.appendChild(area);
  area.select();
  const ok = document.execCommand("copy");
  area.remove();
  return ok;
}

export function useActions() {
  const store = useViewerStore();
  const ui = useUiStore();

  async function copyPath(): Promise<void> {
    const path = store.preview?.path;
    if (!path) return;
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(path);
      } else if (!fallbackCopy(path)) {
        throw new Error("copy unavailable");
      }
      ui.notify(t("notice.copied"));
    } catch {
      ui.notify(fallbackCopy(path) ? t("notice.copied") : t("notice.copyFailed"));
    }
  }

  async function moveToTrash(): Promise<void> {
    if (!store.hasImage) return;
    if (await store.deleteCurrent()) ui.notify(t("notice.trashed"));
  }

  async function openLogs(): Promise<void> {
    try {
      await ipc.openLogDir();
    } catch (caught) {
      store.error = ipc.normalizeError(caught);
    }
  }

  function setTheme(value: ThemeSetting): void {
    ui.setTheme(value);
  }

  /** macOS 原生菜单跟随语言；其它平台静默忽略。 */
  async function syncNativeMenu(): Promise<void> {
    try {
      await ipc.syncNativeMenu(resolvedLocale.value);
    } catch {
      // 浏览器预览或非 macOS 平台没有系统菜单。
    }
  }

  function setLanguage(value: LocaleSetting): void {
    setLocale(value);
    void syncNativeMenu();
  }

  /** 统一的动作入口；未知 id 静默忽略。 */
  async function run(id: string): Promise<void> {
    switch (id) {
      case "open":
        await store.openDialog();
        break;
      case "reveal":
        await store.revealCurrent();
        break;
      case "copy-path":
        await copyPath();
        break;
      case "trash":
        await moveToTrash();
        break;
      case "toggle-thumbnails":
        store.showThumbnails = !store.showThumbnails;
        break;
      case "slideshow":
        store.toggleSlideshow();
        break;
      case "sort-name":
        await store.changeSort("name");
        break;
      case "sort-modified":
        await store.changeSort("modified");
        break;
      case "zoom-in":
        store.zoomBy(1.25);
        break;
      case "zoom-out":
        store.zoomBy(0.8);
        break;
      case "fit":
        store.setFit();
        break;
      case "actual":
        store.setActual();
        break;
      case "theme-system":
        setTheme("system");
        break;
      case "theme-light":
        setTheme("light");
        break;
      case "theme-dark":
        setTheme("dark");
        break;
      case "lang-system":
        setLanguage("system");
        break;
      case "lang-zh":
        setLanguage("zh");
        break;
      case "lang-en":
        setLanguage("en");
        break;
      case "color-badge":
        ui.showColorBadge = !ui.showColorBadge;
        break;
      case "metadata":
        store.showMetadata = !store.showMetadata;
        break;
      case "shortcuts":
        ui.helpOpen = true;
        break;
      case "update":
        ui.updateOpen = true;
        break;
      case "auto-update":
        ui.checkUpdatesOnLaunch = !ui.checkUpdatesOnLaunch;
        break;
      case "logs":
        await openLogs();
        break;
      default:
        break;
    }
  }

  return { run, copyPath, moveToTrash, syncNativeMenu };
}
