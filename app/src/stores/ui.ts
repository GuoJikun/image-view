/**
 * 窗口 chrome 的状态：主题、语言之外的界面开关、全屏闪现标题、短提示。
 * 图片相关的状态仍在 `stores/viewer.ts`。
 *
 * 主题与「启动时检查更新」的持久化在 `settings.ron`（见 `preferences.ts`）；
 * 这里的 localStorage 只是首帧缓存：在 IPC 返回前先铺上用户上次的选择，避免闪默认配色。
 */
import { defineStore } from "pinia";
import { ref, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { inTauri } from "../platform";
import type { ThemeName } from "../types/ipc";

export type ThemeSetting = ThemeName;

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
  /** 默认显示：与 `Settings::default().show_color_status` 保持一致。 */
  const showColorBadge = ref(true);
  // 默认关闭：与 README「本地优先、默认不联网」及 `Settings::default()` 保持一致。
  // 只有用户手动勾选「启动时检查更新」才会在启动时访问 GitHub Releases。
  const checkUpdatesOnLaunch = ref(readFlag(AUTO_UPDATE_KEY, false));

  const fullscreen = ref(false);
  const showFullscreenTitle = ref(false);
  const dragActive = ref(false);
  /** 当前主视图。图库/对比是只读视图，查看器状态在后台原样保留。 */
  const activeView = ref<"viewer" | "gallery" | "compare">("viewer");
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
    activeView,
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
