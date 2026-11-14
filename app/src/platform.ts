/**
 * 平台判定与快捷键文案。
 *
 * 三端共用同一套控件；只有窗口外壳（标题栏来源、窗口按钮造型）与快捷键符号不同。
 * 这里不依赖 Tauri 插件，浏览器里跑 Vite 时也能正确渲染。
 */

const ua = navigator.userAgent;

export type PlatformName = "macos" | "windows" | "linux";

export const platform: PlatformName = ua.includes("Mac")
  ? "macos"
  : ua.includes("Windows")
    ? "windows"
    : "linux";

export const isMac = platform === "macos";
export const isWindows = platform === "windows";
export const isLinux = platform === "linux";

/** 是否运行在 Tauri WebView 内（纯浏览器打开时只渲染空状态）。 */
export const inTauri = "__TAURI_INTERNALS__" in window;

const MODIFIERS: Record<string, string> = {
  mod: isMac ? "⌘" : "Ctrl",
  shift: isMac ? "⇧" : "Shift",
  alt: isMac ? "⌥" : "Alt",
  ctrl: isMac ? "⌃" : "Ctrl",
};

/**
 * 生成平台化快捷键文案：`accel("mod", "shift", "C")`
 * → macOS `⌘⇧C`，Windows / Linux `Ctrl+Shift+C`。
 */
export function accel(...parts: string[]): string {
  const mapped = parts.map((part) => MODIFIERS[part] ?? part);
  return isMac ? mapped.join("") : mapped.join("+");
}

/** Tooltip 文案：`打开图片 (Ctrl+O)`。 */
export function tip(label: string, shortcut?: string): string {
  return shortcut ? `${label} (${shortcut})` : label;
}

/** 状态栏 / 对话框里的键帽符号（主修饰键）。 */
export const modSymbol = isMac ? "⌘" : "Ctrl";

/** 窗口按钮的悬停色（仅自绘标题栏使用）。 */
export const dangerHover = "#C42B1C";
