/**
 * 窗口 chrome 的状态：主题、语言之外的界面开关、全屏闪现标题、短提示。
 * 图片相关的状态仍在 `stores/viewer.ts`。
 */
import { defineStore } from "pinia";
import { ref, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { inTauri } from "../platform";

export type ThemeSetting = "system" | "light" | "dark";

const THEME_KEY = "image-view.theme";
const AUTO_UPDATE_KEY = "image-view.update-on-launch";

function readSetting<T extends string>(key: string, allowed: readonly T[], fallback: T): T {
  try {
    const value = localStorage.getItem(key);
    if (value && (allowed as readonly string[]).includes(value)) return value as T;
  } catch {
    // 存储不可用时使用默认值。
  }
  return fallback;
}

function readFlag(key: string, fallback: boolean): boolean {
  try {
    const value = localStorage.getItem(key);
    if (value === "1") return true;
    if (value === "0") return false;
  } catch {
    // 同上。
  }
  return fallback;
}

function persist(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // 仅在本次会话生效。
  }
}

export const useUiStore = defineStore("ui", () => {
  const theme = ref<ThemeSetting>(
    readSetting<ThemeSetting>(THEME_KEY, ["system", "light", "dark"], "system"),
  );
  const showColorBadge = ref(true);
  const checkUpdatesOnLaunch = ref(readFlag(AUTO_UPDATE_KEY, true));

  const fullscreen = ref(false);
  const showFullscreenTitle = ref(false);
  const dragActive = ref(false);
  const helpOpen = ref(false);
  const updateOpen = ref(false);
  const notice = ref("");

  let noticeTimer: number | undefined;
  let titleTimer: number | undefined;

  function applyTheme(value: ThemeSetting): void {
    const root = document.documentElement;
    if (value === "system") root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", value);
  }

  applyTheme(theme.value);
  watch(theme, (value) => {
    applyTheme(value);
    persist(THEME_KEY, value);
  });

  watch(checkUpdatesOnLaunch, (value) => persist(AUTO_UPDATE_KEY, value ? "1" : "0"));

  function setTheme(value: ThemeSetting): void {
    theme.value = value;
  }

  /** 状态栏右侧短提示：停留 4 s 后淡出（设计令牌 3.4）。 */
  function notify(message: string): void {
    notice.value = message;
    if (noticeTimer !== undefined) window.clearTimeout(noticeTimer);
    noticeTimer = window.setTimeout(() => {
      notice.value = "";
      noticeTimer = undefined;
    }, 4000);
  }

  /** 全屏时文件名回到工具栏中央，3 s 后淡出。 */
  function flashFullscreenTitle(): void {
    showFullscreenTitle.value = true;
    if (titleTimer !== undefined) window.clearTimeout(titleTimer);
    titleTimer = window.setTimeout(() => {
      showFullscreenTitle.value = false;
      titleTimer = undefined;
    }, 3000);
  }

  async function setFullscreen(next: boolean): Promise<void> {
    fullscreen.value = next;
    if (next) flashFullscreenTitle();
    if (!inTauri) return;
    try {
      await getCurrentWindow().setFullscreen(next);
    } catch {
      // 全屏不可用时仍保持内部状态一致。
    }
  }

  async function toggleFullscreen(): Promise<void> {
    if (inTauri) {
      try {
        const current = await getCurrentWindow().isFullscreen();
        await setFullscreen(!current);
        return;
      } catch {
        // 回退到内部状态。
      }
    }
    await setFullscreen(!fullscreen.value);
  }

  /** 系统快捷键（如 macOS ⌃⌘F）改变全屏时同步界面状态。 */
  function syncFullscreen(next: boolean): void {
    if (fullscreen.value === next) return;
    fullscreen.value = next;
    if (next) flashFullscreenTitle();
  }

  return {
    theme,
    showColorBadge,
    checkUpdatesOnLaunch,
    fullscreen,
    showFullscreenTitle,
    dragActive,
    helpOpen,
    updateOpen,
    notice,
    setTheme,
    notify,
    flashFullscreenTitle,
    setFullscreen,
    toggleFullscreen,
    syncFullscreen,
  };
});
