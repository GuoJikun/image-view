//! 图片与目录相关 Command：短请求/响应，不在此处做像素处理。

use std::path::Path;

use tauri::State;

use crate::dto::{CommandError, DirectorySnapshot, PreviewRequestDto};
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

#[tauri::command]
pub fn startup_path(state: State<'_, AppState>) -> Option<String> {
    state.startup_path()
}

#[tauri::command]
pub fn close_image(state: State<'_, AppState>) {
    state.close();
}
