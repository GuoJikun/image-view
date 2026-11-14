/**
 * 极简 i18n：只覆盖窗口 chrome（菜单、工具栏、状态栏、空状态、弹窗）。
 * 语言菜单提供「跟随系统 / 简体中文 / English」，切换即时生效。
 */
import { computed, ref } from "vue";

export type LocaleSetting = "system" | "zh" | "en";
export type ResolvedLocale = "zh" | "en";

const STORAGE_KEY = "image-view.locale";

const zh = {
  "app.name": "图片查看器",

  "menu.file": "文件",
  "menu.view": "显示",
  "menu.help": "帮助",
  "menu.label": "主菜单",
  "menu.open": "打开图片…",
  "menu.reveal": "在文件管理器中显示",
  "menu.copyPath": "复制路径",
  "menu.trash": "移到废纸篓",
  "menu.thumbnails": "缩略图条",
  "menu.slideshow": "幻灯片",
  "menu.sort": "排序",
  "menu.sortName": "按名称",
  "menu.sortModified": "按修改时间",
  "menu.zoomIn": "放大",
  "menu.zoomOut": "缩小",
  "menu.fit": "适配窗口",
  "menu.actual": "实际大小",
  "menu.theme": "主题",
  "menu.themeSystem": "跟随系统",
  "menu.themeLight": "浅色",
  "menu.themeDark": "深色",
  "menu.language": "语言",
  "menu.langSystem": "跟随系统",
  "menu.langZh": "简体中文",
  "menu.langEn": "English",
  "menu.colorBadge": "显示色彩转换标记",
  "menu.metadata": "元数据面板",
  "menu.shortcuts": "快捷键…",
  "menu.update": "检查更新…",
  "menu.autoUpdate": "启动时检查更新",
  "menu.logs": "打开日志文件夹",
  "menu.licenses": "第三方许可证",

  "toolbar.open": "打开图片",
  "toolbar.prev": "上一张",
  "toolbar.next": "下一张",
  "toolbar.thumbnails": "缩略图条",
  "toolbar.slideshow": "幻灯片",
  "toolbar.pauseSlideshow": "暂停幻灯片",
  "toolbar.zoomOut": "缩小",
  "toolbar.zoomIn": "放大",
  "toolbar.fit": "适配窗口",

  "empty.title": "还没有打开图片",
  "empty.hintPrefix": "拖入文件，或按 ",
  "empty.hintSuffix": " 选择",
  "empty.open": "打开图片…",
  "empty.drag": "松开以打开",

  "status.noImage": "没有图片",
  "status.downscaled": "已降采样",
  "status.slideshow": "幻灯片中",
  "status.decoding": "正在解码…",

  "error.retry": "重试",
  "error.close": "关闭",

  "notice.copied": "已复制路径",
  "notice.trashed": "已移到废纸篓",
  "notice.copyFailed": "复制失败",

  "dialog.shortcuts": "快捷键",
  "dialog.update": "检查更新",
  "dialog.version": "当前版本",
  "dialog.updateNote":
    "此构建未配置更新源。请从发布页或包管理器获取新版本。",
  "dialog.close": "关闭",

  "metadata.title": "元数据",
  "metadata.hide": "隐藏面板",
  "metadata.empty": "打开图片后显示尺寸、格式与色彩信息。",
  "metadata.format": "格式",
  "metadata.sourceSize": "原图尺寸",
  "metadata.displaySize": "显示尺寸",
  "metadata.depth": "位深",
  "metadata.fileSize": "文件大小",
  "metadata.modified": "修改时间",
  "metadata.color": "色彩",
  "metadata.downscaled": "降采样",
  "metadata.yes": "是",
  "metadata.no": "否",
  "metadata.converted": "已转换到 sRGB",
  "metadata.note":
    "当前显示的是降采样结果；「100%」相对显示尺寸，而非原图。",

  "shortcut.open": "打开图片",
  "shortcut.navigate": "上一张 / 下一张",
  "shortcut.zoom": "放大 / 缩小",
  "shortcut.fit": "适配窗口",
  "shortcut.actual": "实际大小",
  "shortcut.rotate": "顺时针 / 逆时针旋转",
  "shortcut.flipH": "水平翻转",
  "shortcut.flipV": "垂直翻转",
  "shortcut.slideshow": "开始 / 停止幻灯片",
  "shortcut.fullscreen": "全屏",
  "shortcut.trash": "移到废纸篓",
  "shortcut.copyPath": "复制路径",
  "shortcut.dismiss": "关闭提示或对话框",
  "shortcut.pan": "拖动平移",
  "shortcut.wheel": "滚轮以光标为中心缩放",

  "window.minimize": "最小化",
  "window.maximize": "最大化",
  "window.restore": "还原",
  "window.close": "关闭",
} as const;

export type MessageKey = keyof typeof zh;

const en: Record<MessageKey, string> = {
  "app.name": "Image Viewer",

  "menu.file": "File",
  "menu.view": "View",
  "menu.help": "Help",
  "menu.label": "Main menu",
  "menu.open": "Open Image…",
  "menu.reveal": "Reveal in File Manager",
  "menu.copyPath": "Copy Path",
  "menu.trash": "Move to Trash",
  "menu.thumbnails": "Thumbnails",
  "menu.slideshow": "Slideshow",
  "menu.sort": "Sort",
  "menu.sortName": "By Name",
  "menu.sortModified": "By Date Modified",
  "menu.zoomIn": "Zoom In",
  "menu.zoomOut": "Zoom Out",
  "menu.fit": "Fit to Window",
  "menu.actual": "Actual Size",
  "menu.theme": "Theme",
  "menu.themeSystem": "System",
  "menu.themeLight": "Light",
  "menu.themeDark": "Dark",
  "menu.language": "Language",
  "menu.langSystem": "System",
  "menu.langZh": "简体中文",
  "menu.langEn": "English",
  "menu.colorBadge": "Show Color Conversion Badge",
  "menu.metadata": "Metadata Panel",
  "menu.shortcuts": "Keyboard Shortcuts…",
  "menu.update": "Check for Updates…",
  "menu.autoUpdate": "Check for Updates at Launch",
  "menu.logs": "Open Logs Folder",
  "menu.licenses": "Third-Party Licenses",

  "toolbar.open": "Open image",
  "toolbar.prev": "Previous image",
  "toolbar.next": "Next image",
  "toolbar.thumbnails": "Thumbnails",
  "toolbar.slideshow": "Slideshow",
  "toolbar.pauseSlideshow": "Pause slideshow",
  "toolbar.zoomOut": "Zoom out",
  "toolbar.zoomIn": "Zoom in",
  "toolbar.fit": "Fit to window",

  "empty.title": "No image open",
  "empty.hintPrefix": "Drop a file, or press ",
  "empty.hintSuffix": " to choose",
  "empty.open": "Open Image…",
  "empty.drag": "Drop to open",

  "status.noImage": "No image",
  "status.downscaled": "Downsampled",
  "status.slideshow": "Slideshow",
  "status.decoding": "Decoding…",

  "error.retry": "Retry",
  "error.close": "Dismiss",

  "notice.copied": "Path copied",
  "notice.trashed": "Moved to Trash",
  "notice.copyFailed": "Copy failed",

  "dialog.shortcuts": "Keyboard Shortcuts",
  "dialog.update": "Check for Updates",
  "dialog.version": "Current version",
  "dialog.updateNote":
    "This build has no update source configured. Get the latest version from the release page or your package manager.",
  "dialog.close": "Close",

  "metadata.title": "Metadata",
  "metadata.hide": "Hide panel",
  "metadata.empty": "Image details appear here after opening a file.",
  "metadata.format": "Format",
  "metadata.sourceSize": "Source size",
  "metadata.displaySize": "Display size",
  "metadata.depth": "Depth",
  "metadata.fileSize": "File size",
  "metadata.modified": "Modified",
  "metadata.color": "Color",
  "metadata.downscaled": "Downsampled",
  "metadata.yes": "Yes",
  "metadata.no": "No",
  "metadata.converted": "Converted to sRGB",
  "metadata.note":
    "The preview is downsampled; 100% refers to the displayed size, not the source.",

  "shortcut.open": "Open image",
  "shortcut.navigate": "Previous / next image",
  "shortcut.zoom": "Zoom in / out",
  "shortcut.fit": "Fit to window",
  "shortcut.actual": "Actual size",
  "shortcut.rotate": "Rotate clockwise / counter-clockwise",
  "shortcut.flipH": "Flip horizontally",
  "shortcut.flipV": "Flip vertically",
  "shortcut.slideshow": "Start / stop slideshow",
  "shortcut.fullscreen": "Fullscreen",
  "shortcut.trash": "Move to Trash",
  "shortcut.copyPath": "Copy path",
  "shortcut.dismiss": "Dismiss banner or dialog",
  "shortcut.pan": "Drag to pan",
  "shortcut.wheel": "Wheel to zoom at cursor",

  "window.minimize": "Minimize",
  "window.maximize": "Maximize",
  "window.restore": "Restore",
  "window.close": "Close",
};

const messages: Record<ResolvedLocale, Record<MessageKey, string>> = { zh, en };

function readStored(): LocaleSetting {
  try {
    const value = localStorage.getItem(STORAGE_KEY);
    if (value === "zh" || value === "en" || value === "system") return value;
  } catch {
    // 隐私模式或受限 WebView 下退化为跟随系统。
  }
  return "system";
}

export const localeSetting = ref<LocaleSetting>(readStored());

const systemLocale: ResolvedLocale = navigator.language.toLowerCase().startsWith("zh")
  ? "zh"
  : "en";

export const resolvedLocale = computed<ResolvedLocale>(() =>
  localeSetting.value === "system" ? systemLocale : localeSetting.value,
);

export function t(key: MessageKey): string {
  return messages[resolvedLocale.value][key];
}

function applyDocumentLang(): void {
  document.documentElement.lang = resolvedLocale.value === "zh" ? "zh-CN" : "en";
}

export function setLocale(next: LocaleSetting): void {
  localeSetting.value = next;
  try {
    localStorage.setItem(STORAGE_KEY, next);
  } catch {
    // 存储不可用时仅在本次会话生效。
  }
  applyDocumentLang();
}

applyDocumentLang();
