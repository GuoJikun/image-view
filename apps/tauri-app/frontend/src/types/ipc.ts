/** 后端 DTO 的 TypeScript 镜像；字段与 Rust 的 camelCase 序列化保持一致。 */

export interface DirectoryEntry {
  id: string;
  path: string;
  name: string;
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

export interface CommandError {
  code: string;
  message: string;
}

export type FitMode = "fit" | "actual" | "custom";
export type SortOrder = "name" | "modified";

/** 更新清单（后端 `update.json` 解析结果）。 */
export interface UpdateInfo {
  version: string;
  notes: string;
  url: string;
}

/** 下载进度事件。 */
export interface UpdateProgress {
  received: number;
  total: number | null;
}
