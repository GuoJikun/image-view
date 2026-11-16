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

export interface CommandError {
  code: string;
  message: string;
}

export type FitMode = "fit" | "actual" | "custom";
export type SortOrder = "name" | "modified";

/** 启动时读取的界面偏好（后端 `ui_preferences`）。 */
export interface UiPreferences {
  showThumbnails: boolean;
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
