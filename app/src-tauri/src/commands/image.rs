//! 图片与目录相关 Command：短请求/响应，不在此处做像素处理。

use std::path::Path;

use tauri::State;

use iv_image_decode::ColorTarget;

use crate::dto::{CommandError, DirectorySnapshot, PreviewRequestDto, UiPreferencesDto};
use crate::state::AppState;

#[tauri::command]
pub fn open_path(
    state: State<'_, AppState>,
    path: String,
) -> Result<DirectorySnapshot, CommandError> {
    state.open_path(Path::new(&path))
}

/// 原生文件对话框。阻塞调用放到 blocking 线程池，避免占用异步运行时。
#[tauri::command]
pub async fn pick_image() -> Option<String> {
    tauri::async_runtime::spawn_blocking(|| {
        let extensions = iv_domain::SUPPORTED_EXTENSIONS.to_vec();
        rfd::FileDialog::new()
            .set_title("打开图片")
            .add_filter("图片", &extensions)
            .pick_file()
            .map(|path| path.to_string_lossy().into_owned())
    })
    .await
    .ok()
    .flatten()
}

#[tauri::command]
pub fn navigate(state: State<'_, AppState>, delta: i32) -> Option<DirectorySnapshot> {
    state.navigate(delta as isize)
}

#[tauri::command]
pub fn select_image(state: State<'_, AppState>, index: usize) -> Option<DirectorySnapshot> {
    state.select(index)
}

#[tauri::command]
pub fn current_snapshot(state: State<'_, AppState>) -> DirectorySnapshot {
    state.snapshot()
}

#[tauri::command]
pub fn request_preview(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<PreviewRequestDto, CommandError> {
    state.request_preview(&image_id)
}

#[tauri::command]
pub fn set_sort(
    state: State<'_, AppState>,
    order: String,
) -> Result<DirectorySnapshot, CommandError> {
    let order = crate::state::parse_sort(&order)?;
    Ok(state.set_sort(order))
}

/// 设置显示目标色彩空间（`srgb` / `display-p3`）。
///
/// 未知标识按 sRGB 处理：这是前端探测结果的回传，不该因为拼错而阻断打开图片。
/// 返回 `true` 表示目标真的变了，缓存已失效，前端需要重新请求当前图片。
#[tauri::command]
pub fn set_color_target(state: State<'_, AppState>, target: String) -> bool {
    let target = ColorTarget::from_id(&target).unwrap_or_else(|| {
        log::debug!(
            target: "image_view::preview",
            "未知的色彩目标 {target}，按 sRGB 处理"
        );
        ColorTarget::Srgb
    });
    state.set_color_target(target)
}

#[tauri::command]
pub fn startup_path(state: State<'_, AppState>) -> Option<String> {
    state.startup_path()
}

/// 启动时读取界面偏好；变更通过 `set_ui_preferences` 整体写回。
#[tauri::command]
pub fn ui_preferences(state: State<'_, AppState>) -> UiPreferencesDto {
    state.ui_preferences()
}

#[tauri::command]
pub fn set_ui_preferences(
    state: State<'_, AppState>,
    preferences: UiPreferencesDto,
) -> Result<(), CommandError> {
    state.set_ui_preferences(&preferences)?;
    log::debug!(
        target: "image_view::settings",
        "界面偏好已更新：主题={} 语言={} 缩略图条={} 色彩标记={} 启动检查更新={}",
        preferences.theme,
        preferences.language,
        preferences.show_thumbnails,
        preferences.show_color_status,
        preferences.check_updates_on_startup
    );
    Ok(())
}

#[tauri::command]
pub fn close_image(state: State<'_, AppState>) {
    state.close();
}

/// 把当前图片移到系统回收站，并返回删除后的目录快照。
#[tauri::command]
pub fn delete_image(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<DirectorySnapshot, CommandError> {
    state.delete_current(&image_id)
}
