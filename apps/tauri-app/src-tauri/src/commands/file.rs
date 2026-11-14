//! 文件系统相关 Command；只接受已授权的 ImageId，不接受任意路径。

use tauri::State;

use crate::dto::CommandError;
use crate::state::AppState;

#[tauri::command]
pub fn reveal_image(state: State<'_, AppState>, image_id: String) -> Result<(), CommandError> {
    state.reveal(&image_id)
}

/// 在系统文件管理器中打开日志目录（帮助菜单 → 打开日志文件夹）。
#[tauri::command]
pub fn open_log_dir() -> Result<(), CommandError> {
    iv_platform::open_folder(&iv_infra::logging::log_dir())
        .map_err(|error| CommandError::io(error.to_string()))
}
