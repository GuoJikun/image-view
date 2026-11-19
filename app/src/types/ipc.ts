/** 后端 DTO 的 TypeScript 镜像；字段与 Rust 的 camelCase 序列化保持一致。 */

export interface DirectoryEntry {
  id: string;
  path: string;
  name: string;
  revision: number;
}

export interface DirectorySnapshot {
  dir: string;
  entries: DirectoryEntry[];
  index: number | null;
  position: [number, number] | null;
  canPrev: boolean;
  canNext: boolean;
  current: DirectoryEntry | null;
  sort: "name" | "modified";
}

export interface PreviewRequest {
  requestId: number;
  imageId: string;
  revision: number;
}

/**
 * 像素被编码到的目标空间。
 *
 * 只有两档：画布能声明的空间就这么两个（`srgb` / `display-p3`）。
 */
export type ColorTarget = "srgb" | "display-p3";

export interface PreviewReady {
  requestId: number;
  imageId: string;
  revision: number;
  path: string;
  fileName: string;
  format: string;
  width: number;
  height: number;
  sourceWidth: number;
  sourceHeight: number;
  downscaled: boolean;
  sourceDepth: number;
  colorConverted: boolean;
  colorSource: string | null;
  colorTarget: ColorTarget;
  fileSize: number;
  modifiedAt: number | null;
}

export interface PreviewFailed {
  requestId: number;
  imageId: string;
  code: string;
  message: string;
}

/** 动画元信息；`loops` 为 `null` 表示无限循环。 */
export interface AnimationInfo {
  frameCount: number;
  loops: number | null;
  delays: number[];
}

/** 动画帧解码完成事件（`image:animation-ready`）。 */
export interface AnimationReady extends AnimationInfo {
  requestId: number;
  imageId: string;
  revision: number;
}

/** 动画降级事件（`image:degraded`）：本应能播却被降级为静态首帧。 */
export interface Degraded {
  requestId: number;
  imageId: string;
  code: "overBudget" | "unsupported16Bit" | "decodeFailed" | string;
  detail: string;
}

export interface CommandError {
  code: string;
  message: string;
  /** 为 false 时错误横幅隐藏「重试」按钮；默认可重试。 */
  retryable?: boolean;
}

export type FitMode = "fit" | "actual" | "custom";
export type SortOrder = "name" | "modified";

/** 主题偏好；`system` 表示跟随系统（由 CSS `prefers-color-scheme` 取值）。 */
export type ThemeName = "system" | "light" | "dark";

/** 语言偏好；`system` 表示跟随系统语言。 */
export type LocaleName = "system" | "zh" | "en";

/**
 * 菜单可勾选项的当前状态（`sync_native_menu` 入参）。
 *
 * macOS 原生菜单与 Windows / Linux 自绘菜单共用这一份状态，
 * 字段与 Rust 侧 `MenuStateDto` 一一对应。
 */
export interface MenuState {
  thumbnails: boolean;
  slideshow: boolean;
  colorBadge: boolean;
  metadata: boolean;
  autoUpdate: boolean;
  sort: SortOrder;
  theme: ThemeName;
  language: LocaleName;
}

/** 界面偏好（后端 `ui_preferences` / `set_ui_preferences`），持久化在 `settings.ron`。 */
export interface UiPreferences {
  showThumbnails: boolean;
  showColorStatus: boolean;
  theme: ThemeName;
  language: LocaleName;
  checkUpdatesOnStartup: boolean;
}

/** 更新信息（后端 `check_update` 返回）。 */
export interface UpdateInfo {
  version: string;
  body: string;
  date: string | null;
}

/** 下载进度事件。 */
export interface UpdateProgress {
  received: number;
  total: number | null;
}

/** 图库条目（后端 `GalleryEntryDto`）。id + revision 用于缩略图 URL。 */
export interface GalleryEntry {
  id: string;
  path: string;
  name: string;
  revision: number;
  width: number;
  height: number;
  format: string;
  modifiedAt: number | null;
  sizeBytes: number;
  tags: string[];
}

export type GallerySortKey = "name" | "modified" | "size" | "format";

export interface GalleryFilter {
  query: string;
  tags: string[];
  format: string;
  minWidth: number;
  maxWidth: number;
  fromDate: number | null;
  toDate: number | null;
  sort: GallerySortKey;
}

/** 一次图库查询结果。 */
export interface GallerySnapshot {
  dir: string;
  entries: GalleryEntry[];
  total: number;
  offset: number;
  limit: number;
  indexed: number;
  indexReady: boolean;
  degraded: boolean;
  filter: GalleryFilter;
}

/** 全像素对比中单个槽位的预览版本。 */
export interface ComparePreview {
  imageId: string;
  revision: number;
}

/** 对比视图（后端 `CompareViewDto`）。 */
export interface CompareView {
  imageIds: string[];
  layout: "side-by-side" | "overlay";
  /** 与 imageIds 一一对应；缩略图模式或解码失败项为 null。 */
  previews: (ComparePreview | null)[];
}
