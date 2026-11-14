/**
 * 前端唯一的 Tauri 边界：组件不直接调用 invoke/listen。
 * 命令与事件的名字、参数和 URL 规则都集中在这里。
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  CommandError,
  DirectorySnapshot,
  PreviewFailed,
  PreviewReady,
  PreviewRequest,
  SortOrder,
} from "../types/ipc";

const isWindows = navigator.userAgent.includes("Windows");

export function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

/** Windows/Android 使用 http://<scheme>.localhost，macOS/Linux 使用 <scheme>://localhost。 */
function resourceUrl(kind: "preview" | "thumbnail", path: string): string {
  return isWindows
    ? `http://iviewer.localhost/${kind}/${path}`
    : `iviewer://localhost/${kind}/${path}`;
}

export function previewUrl(imageId: string, revision: number): string {
  return resourceUrl("preview", `${imageId}/${revision}`);
}

export function thumbnailUrl(imageId: string, size: number, revision: number): string {
  return resourceUrl("thumbnail", `${imageId}/${size}/${revision}`);
}

export async function openPath(path: string): Promise<DirectorySnapshot> {
  return invoke<DirectorySnapshot>("open_path", { path });
}

export async function pickImage(): Promise<string | null> {
  return invoke<string | null>("pick_image");
}

export async function navigate(delta: number): Promise<DirectorySnapshot | null> {
  return invoke<DirectorySnapshot | null>("navigate", { delta });
}

export async function selectImage(index: number): Promise<DirectorySnapshot | null> {
  return invoke<DirectorySnapshot | null>("select_image", { index });
}

export async function currentSnapshot(): Promise<DirectorySnapshot> {
  return invoke<DirectorySnapshot>("current_snapshot");
}

export async function requestPreview(imageId: string): Promise<PreviewRequest> {
  return invoke<PreviewRequest>("request_preview", { imageId });
}

export async function setSort(order: SortOrder): Promise<DirectorySnapshot> {
  return invoke<DirectorySnapshot>("set_sort", { order });
}

export async function startupPath(): Promise<string | null> {
  return invoke<string | null>("startup_path");
}

export async function closeImage(): Promise<void> {
  return invoke<void>("close_image");
}

export async function cancelPreview(): Promise<number> {
  return invoke<number>("cancel_preview");
}

export async function revealImage(imageId: string): Promise<void> {
  return invoke<void>("reveal_image", { imageId });
}

export async function deleteImage(imageId: string): Promise<DirectorySnapshot> {
  return invoke<DirectorySnapshot>("delete_image", { imageId });
}

export async function openLogDir(): Promise<void> {
  return invoke<void>("open_log_dir");
}

/** macOS：让系统菜单栏跟随界面语言。其它平台为空实现。 */
export async function syncNativeMenu(locale: string): Promise<void> {
  return invoke<void>("sync_native_menu", { locale });
}

export function onPreviewReady(
  handler: (payload: PreviewReady) => void,
): Promise<UnlistenFn> {
  return listen<PreviewReady>("image:preview-ready", (event) => handler(event.payload));
}

export function onPreviewFailed(
  handler: (payload: PreviewFailed) => void,
): Promise<UnlistenFn> {
  return listen<PreviewFailed>("image:preview-failed", (event) => handler(event.payload));
}

/** 单实例/文件关联把新文件转交给已运行窗口。 */
export function onOpenFile(handler: (path: string) => void): Promise<UnlistenFn> {
  return listen<string>("app:open-file", (event) => handler(event.payload));
}

/** macOS 原生菜单 → 与自绘菜单共用的动作 id。 */
export function onMenuAction(handler: (id: string) => void): Promise<UnlistenFn> {
  return listen<string>("menu:action", (event) => handler(event.payload));
}

export function normalizeError(error: unknown): CommandError {
  if (typeof error === "string") {
    return { code: "unknown", message: error };
  }
  if (error && typeof error === "object") {
    const candidate = error as Partial<CommandError>;
    return {
      code: candidate.code ?? "unknown",
      message: candidate.message ?? String(error),
    };
  }
  return { code: "unknown", message: String(error) };
}
