//! 任务取消：推进预览代号，让所有在途解码结果过期。

use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub fn cancel_preview(state: State<'_, AppState>) -> u64 {
    state.cancel_preview()
}
