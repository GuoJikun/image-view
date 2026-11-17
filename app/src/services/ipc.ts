/**
 * 前端唯一的 Tauri 边界：组件不直接调用 invoke/listen。
 * 命令与事件的名字、参数和 URL 规则都集中在这里。
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AnimationReady,
  CommandError,
  Degraded,
  DirectorySnapshot,
  MenuState,
  PreviewFailed,
  PreviewReady,
  PreviewRequest,
  SortOrder,
  UiPreferences,
  UpdateInfo,
  UpdateProgress,
} from "../types/ipc";

const isWindows = navigator.userAgent.includes("Windows");

export function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

/** Windows/Android 使用 http://<scheme>.localhost，macOS/Linux 使用 <scheme>://localhost。 */
function resourceUrl(kind: "preview" | "frame" | "thumbnail", path: string): string {
  return isWindows
    ? `http://iviewer.localhost/${kind}/${path}`
    : `iviewer://localhost/${kind}/${path}`;
}

export function previewUrl(imageId: string, revision: number): string {
  return resourceUrl("preview", `${imageId}/${revision}`);
}

/** 动画帧 URL；同一 revision 内帧内容不变，WebView 可长期缓存。 */
export function frameUrl(imageId: string, revision: number, index: number): string {
  return resourceUrl("frame", `${imageId}/${index}/${revision}`);
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

/** 读取界面偏好（唯一持久化位置是后端 `settings.ron`）。 */
export async function uiPreferences(): Promise<UiPreferences> {
  return invoke<UiPreferences>("ui_preferences");
}

/** 整体写回界面偏好；后端 `SettingsStore` 自带防抖落盘。 */
export async function saveUiPreferences(preferences: UiPreferences): Promise<void> {
  return invoke<void>("set_ui_preferences", { preferences });
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

/**
 * macOS：让系统菜单栏跟随界面语言与勾选状态。其它平台为空实现。
 *
 * 语言与上次不同时后端会重建整个菜单，相同则只更新勾选值。
 */
export async function syncNativeMenu(locale: string, state: MenuState): Promise<void> {
  return invoke<void>("sync_native_menu", { locale, state });
}

// ---- 更新（tauri-plugin-updater + 自定义命令） ----

/** 检查更新：返回比当前版本更新的版本信息；无更新返回 null。 */
export async function checkUpdate(): Promise<UpdateInfo | null> {
  return invoke<UpdateInfo | null>("check_update");
}

/** 下载更新；进度通过 onUpdateProgress 事件推送。 */
export async function downloadUpdate(): Promise<void> {
  return invoke<void>("download_update");
}

/** 取消正在进行的下载。 */
export async function cancelUpdateDownload(): Promise<void> {
  return invoke<void>("cancel_update_download");
}

/** 安装已下载的更新（安装器启动后应用退出）。 */
export async function installUpdate(): Promise<void> {
  return invoke<void>("install_update");
}

/** 下载进度事件。 */
export function onUpdateProgress(
  handler: (payload: UpdateProgress) => void,
): Promise<UnlistenFn> {
  return listen<UpdateProgress>("update:progress", (event) => handler(event.payload));
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

/** 动画帧解码完成；首帧静态预览早已由 `preview-ready` 送出。 */
export function onAnimationReady(
  handler: (payload: AnimationReady) => void,
): Promise<UnlistenFn> {
  return listen<AnimationReady>("image:animation-ready", (event) => handler(event.payload));
}

/** 动画降级为静态首帧（超预算 / 16-bit / 解码失败）。 */
export function onDegraded(handler: (payload: Degraded) => void): Promise<UnlistenFn> {
  return listen<Degraded>("image:degraded", (event) => handler(event.payload));
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
