//! 文件系统相关 Command；只接受已授权的 ImageId，不接受任意路径。

use tauri::State;

use crate::dto::CommandError;
use crate::state::AppState;

#[tauri::command]
pub fn reveal_image(state: State<'_, AppState>, image_id: String) -> Result<(), CommandError> {
    state.reveal(&image_id)
}
