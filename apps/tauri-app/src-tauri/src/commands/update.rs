//! 检查更新 / 下载 / 安装：基于 `tauri-plugin-updater` 的 Rust API，支持取消下载。
//!
//! `check_update` 调用插件 `check()` 获取 `Update` 并暂存；
//! `download_update` 调用 `Update::download()`（进度经 `update:progress` 事件推送，
//! `tokio::select!` 与取消信号竞速实现中断）；
//! `install_update` 调用 `Update::install()` 完成签名校验与安装。

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::Serialize;
use tauri::{Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::watch;

use crate::dto::CommandError;

// ---------------------------------------------------------------------------
// DTO
// ---------------------------------------------------------------------------

/// 前端可用的更新信息。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub body: String,
    pub date: Option<String>,
}

/// 下载进度事件。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub received: u64,
    pub total: Option<u64>,
}

// ---------------------------------------------------------------------------
// 全局下载状态
// ---------------------------------------------------------------------------

/// 更新状态：取消信号、已获取的 `Update`、已下载的字节。
pub struct UpdateState {
    cancel_tx: watch::Sender<bool>,
    update: Mutex<Option<Update>>,
    downloaded: Mutex<Option<Vec<u8>>>,
}

impl UpdateState {
    pub fn new() -> Self {
        let (cancel_tx, _) = watch::channel(false);
        Self {
            cancel_tx,
            update: Mutex::new(None),
            downloaded: Mutex::new(None),
        }
    }

    fn reset_cancel(&self) {
        let _ = self.cancel_tx.send(false);
    }

    fn request_cancel(&self) {
        let _ = self.cancel_tx.send(true);
    }

    fn cancel_rx(&self) -> watch::Receiver<bool> {
        self.cancel_tx.subscribe()
    }
}

impl Default for UpdateState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

/// 检查更新：返回比当前版本更新的版本信息；无更新返回 `None`。
#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<Option<UpdateInfo>, CommandError> {
    let update = app
        .updater()
        .map_err(|e| CommandError::new("updater_init", e.to_string()))?
        .check()
        .await
        .map_err(|e| CommandError::new("update_check", e.to_string()))?;

    match update {
        Some(u) => {
            let info = UpdateInfo {
                version: u.version.clone(),
                body: u.body.clone().unwrap_or_default(),
                date: u.date.map(|d| d.to_string()),
            };
            let state = app.state::<UpdateState>();
            *state.update.lock().unwrap() = Some(u);
            Ok(Some(info))
        }
        None => Ok(None),
    }
}

/// 下载更新；进度通过 `update:progress` 事件推送。支持取消。
#[tauri::command]
pub async fn download_update(app: tauri::AppHandle) -> Result<(), CommandError> {
    let state = app.state::<UpdateState>();
    state.reset_cancel();

    let update = state
        .update
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| CommandError::new("no_update", "没有可用的更新"))?;

    let cancel_rx = state.cancel_rx();
    let progress = Arc::new(AtomicU64::new(0));
    let progress_clone = progress.clone();
    let app_handle = app.clone();

    let on_chunk = move |chunk_len: usize, content_length: Option<u64>| {
        let total =
            progress_clone.fetch_add(chunk_len as u64, Ordering::Relaxed) + chunk_len as u64;
        let _ = app_handle.emit(
            "update:progress",
            UpdateProgress {
                received: total,
                total: content_length,
            },
        );
    };
    let on_finish = || {};

    let bytes = tokio::select! {
        result = update.download(on_chunk, on_finish) => {
            result.map_err(|e| CommandError::new("update_download", e.to_string()))?
        }
        _ = wait_cancel(cancel_rx) => {
            return Err(CommandError::new("update_cancelled", "下载已取消"));
        }
    };

    *state.downloaded.lock().unwrap() = Some(bytes);
    Ok(())
}

/// 取消正在进行的下载。
#[tauri::command]
pub fn cancel_update_download(app: tauri::AppHandle) {
    let state = app.state::<UpdateState>();
    state.request_cancel();
}

/// 宅装已下载的更新。
///
/// - **Windows**：`install` 内部启动 NSIS/MSI 安装器后直接 `process::exit(0)`。
/// - **macOS / Linux**：`install` 返回后需自行退出应用让安装生效。
#[tauri::command]
pub fn install_update(app: tauri::AppHandle) -> Result<(), CommandError> {
    let state = app.state::<UpdateState>();

    let update = state
        .update
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| CommandError::new("no_update", "没有可用的更新"))?;

    let bytes = state
        .downloaded
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| CommandError::new("no_download", "尚未下载更新包"))?;

    update
        .install(&bytes)
        .map_err(|e| CommandError::new("update_install", e.to_string()))?;

    // macOS / Linux 走到这里；Windows 已在 install 内退出。
    std::thread::sleep(Duration::from_millis(300));
    app.exit(0);
    Ok(())
}

// ---------------------------------------------------------------------------
// 内部
// ---------------------------------------------------------------------------

/// 等待取消信号变为 `true`。
async fn wait_cancel(mut rx: watch::Receiver<bool>) {
    let _ = rx.wait_for(|v| *v).await;
}
