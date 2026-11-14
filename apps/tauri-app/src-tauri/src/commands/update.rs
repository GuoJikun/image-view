//! 检查更新 / 下载 / 安装：复用 eframe-app 的 `update.json` 清单格式与 SHA-256 校验。
//!
//! 下载在命令线程内同步执行，进度通过 `update:progress` 事件推送；
//! `cancel_update_download` 置取消标记，下载循环逐块检查。

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{Emitter, Manager};

use crate::dto::CommandError;

/// 默认更新清单地址；发布时由 build-release 脚本上传 `update.json`。
pub const MANIFEST_URL: &str =
    "https://github.com/GuoJikun/image-view/releases/latest/download/update.json";

/// 覆盖清单地址（本地文件或测试服务器），便于离线验证。
pub const MANIFEST_ENV: &str = "IMAGE_VIEW_UPDATE_MANIFEST";
/// 覆盖下载目录，便于测试。
pub const DOWNLOAD_DIR_ENV: &str = "IMAGE_VIEW_UPDATE_DIR";

const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const TIMEOUT: Duration = Duration::from_secs(20);
const PROGRESS_STEP: u64 = 256 * 1024;

// ---------------------------------------------------------------------------
// DTO
// ---------------------------------------------------------------------------

/// 更新清单（`update.json` 的反序列化目标）。
#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub notes: String,
}

/// 前端可用的更新信息。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
    pub url: String,
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

/// 下载取消标记与最新下载产物路径。
pub struct UpdateState {
    cancel: AtomicBool,
    /// 正在进行的下载代号；取消后自增，旧回调不再生效。
    generation: AtomicU64,
    /// 已下载并通过校验的安装包路径。
    downloaded: Mutex<Option<PathBuf>>,
}

impl UpdateState {
    pub fn new() -> Self {
        let state = Self {
            cancel: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            downloaded: Mutex::new(None),
        };
        cleanup_partials_in(&download_dir());
        state
    }

    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn reset_cancel(&self) {
        self.cancel.store(false, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Relaxed);
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

/// 检查更新：拉取 `update.json`，返回比当前版本更新的版本信息；无更新返回 `None`。
#[tauri::command]
pub fn check_update() -> Result<Option<UpdateInfo>, CommandError> {
    let url = manifest_url();
    let current = current_version();
    let manifest = fetch_manifest(&url).map_err(to_command_error)?;
    if manifest_newer(&manifest.version, current).map_err(to_command_error)? {
        Ok(Some(UpdateInfo {
            version: manifest.version,
            notes: manifest.notes,
            url: manifest.url,
        }))
    } else {
        Ok(None)
    }
}

/// 下载并校验安装包；进度通过 `update:progress` 事件推送。
/// 返回安装包的本地路径。
#[tauri::command]
pub fn download_update(app: tauri::AppHandle) -> Result<String, CommandError> {
    let state = app.state::<UpdateState>();
    state.reset_cancel();

    let url = manifest_url();
    let manifest = fetch_manifest(&url).map_err(to_command_error)?;

    let dir = download_dir();
    let target = dir.join(file_name_from_url(&manifest.url));

    // 已有通过校验的同版本安装包时直接复用。
    if let Some(cached) = state.downloaded.lock().unwrap().clone()
        && cached == target
        && cached.exists()
    {
        log::info!("复用已下载的安装包：{}", cached.display());
        return Ok(cached.to_string_lossy().into_owned());
    }

    download_and_verify(&app, &manifest, &dir, &state).map_err(to_command_error)?;

    *state.downloaded.lock().unwrap() = Some(target.clone());
    Ok(target.to_string_lossy().into_owned())
}

/// 取消正在进行的下载。
#[tauri::command]
pub fn cancel_update_download(app: tauri::AppHandle) {
    let state = app.state::<UpdateState>();
    state.cancel.store(true, Ordering::Relaxed);
}

/// 启动安装器（NSIS 静默 `/S`）并退出应用。
#[tauri::command]
pub fn install_update(app: tauri::AppHandle, path: String) -> Result<(), CommandError> {
    let installer = PathBuf::from(&path);
    if !installer.exists() {
        return Err(CommandError::new(
            "update_not_found",
            format!("安装包不存在：{path}"),
        ));
    }

    log::info!("启动安装器：{}", installer.display());

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new(&installer)
            .arg("/S")
            .spawn()
            .map_err(|e| CommandError::new("update_install", e.to_string()))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new(&installer)
            .spawn()
            .map_err(|e| CommandError::new("update_install", e.to_string()))?;
    }

    // 给安装器一点启动时间后再退出，避免句柄竞争。
    std::thread::sleep(Duration::from_millis(300));
    app.exit(0);
    Ok(())
}

// ---------------------------------------------------------------------------
// 内部实现
// ---------------------------------------------------------------------------

/// 当前版本（取自 Cargo 包版本）。
fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// 清单地址：环境变量优先。
fn manifest_url() -> String {
    std::env::var(MANIFEST_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| MANIFEST_URL.to_owned())
}

/// 下载目录：`%LOCALAPPDATA%\image-view\updates`。
fn download_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os(DOWNLOAD_DIR_ENV).filter(|v| !v.is_empty()) {
        return PathBuf::from(dir);
    }
    iv_infra::logging::log_dir()
        .parent()
        .map(|base| base.join("updates"))
        .unwrap_or_else(|| std::env::temp_dir().join("image-view-updates"))
}

/// 拉取并解析更新清单。
fn fetch_manifest(url: &str) -> Result<Manifest, UpdateError> {
    let mut bytes = Vec::new();
    open_source(url)?
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| UpdateError::Io(e.to_string()))?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(UpdateError::Manifest("manifest too large".into()));
    }
    serde_json::from_slice(&bytes).map_err(|e| UpdateError::Manifest(e.to_string()))
}

/// 比较版本号（支持 `v` 前缀）。
fn manifest_newer(latest: &str, current: &str) -> Result<bool, UpdateError> {
    let l = semver::Version::parse(latest.trim().trim_start_matches(['v', 'V']))
        .map_err(|e| UpdateError::Version(format!("{latest}: {e}")))?;
    let c = semver::Version::parse(current.trim().trim_start_matches(['v', 'V']))
        .map_err(|e| UpdateError::Version(format!("{current}: {e}")))?;
    Ok(l > c)
}

/// 下载 + SHA-256 校验，完成后重命名 `.part` → 正式文件。
fn download_and_verify(
    app: &tauri::AppHandle,
    manifest: &Manifest,
    dir: &Path,
    state: &UpdateState,
) -> Result<PathBuf, UpdateError> {
    fs::create_dir_all(dir).map_err(|e| UpdateError::Io(e.to_string()))?;

    let target = dir.join(file_name_from_url(&manifest.url));
    let part = target.with_extension("part");

    let reader = open_source(&manifest.url)?;
    let total = content_length(&manifest.url);
    let mut file =
        File::create(&part).map_err(|e| UpdateError::Io(format!("{}: {e}", part.display())))?;
    let mut hasher = Sha256::new();
    let mut received: u64 = 0;
    let mut last_report: u64 = 0;
    let mut buffer = vec![0u8; 64 * 1024];
    let mut reader = reader;

    loop {
        if state.is_cancelled() {
            drop(file);
            let _ = fs::remove_file(&part);
            return Err(UpdateError::Cancelled);
        }
        let n = reader
            .read(&mut buffer)
            .map_err(|e| UpdateError::Io(e.to_string()))?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n])
            .map_err(|e| UpdateError::Io(e.to_string()))?;
        hasher.update(&buffer[..n]);
        received += n as u64;
        if received - last_report >= PROGRESS_STEP {
            let _ = app.emit("update:progress", UpdateProgress { received, total });
            last_report = received;
        }
    }

    file.sync_all()
        .map_err(|e| UpdateError::Io(e.to_string()))?;
    drop(file);

    if state.is_cancelled() {
        let _ = fs::remove_file(&part);
        return Err(UpdateError::Cancelled);
    }

    let digest = hex(&hasher.finalize());
    if !digest.eq_ignore_ascii_case(manifest.sha256.trim()) {
        let _ = fs::remove_file(&part);
        return Err(UpdateError::Hash);
    }

    // 最终进度。
    let _ = app.emit("update:progress", UpdateProgress { received, total });

    fs::rename(&part, &target).map_err(|e| UpdateError::Io(e.to_string()))?;
    log::info!("更新安装包已下载并通过校验：{}", target.display());
    Ok(target)
}

/// 打开 HTTP(S) / `file://` / 本地路径来源。
fn open_source(url: &str) -> Result<Box<dyn Read>, UpdateError> {
    if let Some(path) = local_path(url) {
        let file =
            File::open(&path).map_err(|e| UpdateError::Io(format!("{}: {e}", path.display())))?;
        return Ok(Box::new(file));
    }
    if url.starts_with("https://") || url.starts_with("http://") {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_connect(Some(TIMEOUT))
            .timeout_recv_response(Some(TIMEOUT))
            .timeout_per_read(Some(TIMEOUT))
            .build()
            .into();
        let resp = agent.get(url).call().map_err(|e| match e {
            ureq::Error::StatusCode(code) => UpdateError::Status(code),
            other => UpdateError::Network(other.to_string()),
        })?;
        return Ok(Box::new(resp.into_body().into_reader()));
    }
    Err(UpdateError::Url)
}

/// 尝试获取 Content-Length（用于进度百分比）。
fn content_length(url: &str) -> Option<u64> {
    if let Some(path) = local_path(url) {
        return fs::metadata(&path).ok().map(|m| m.len());
    }
    if url.starts_with("https://") || url.starts_with("http://") {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_connect(Some(TIMEOUT))
            .timeout_recv_response(Some(TIMEOUT))
            .build()
            .into();
        if let Ok(resp) = agent.head(url).call() {
            return resp
                .headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok());
        }
    }
    None
}

/// 从 URL 提取文件名；异常时用固定名称。
fn file_name_from_url(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.rsplit(['/', '\\'])
        .next()
        .filter(|n| !n.is_empty() && *n != "." && *n != "..")
        .map(|s| s.to_owned())
        .unwrap_or_else(|| "image-view-update.exe".to_owned())
}

fn local_path(url: &str) -> Option<PathBuf> {
    if let Some(rest) = url.strip_prefix("file://") {
        let rest = rest.strip_prefix("localhost").unwrap_or(rest);
        #[cfg(windows)]
        let rest = rest.strip_prefix('/').unwrap_or(rest);
        return Some(PathBuf::from(rest));
    }
    if !url.contains("://") && !url.is_empty() {
        return Some(PathBuf::from(url));
    }
    None
}

fn cleanup_partials_in(dir: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("part"))
                && fs::remove_file(&path).is_ok()
            {
                log::info!("已清理未完成的更新文件：{}", path.display());
            }
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02x}");
    }
    out
}

// ---------------------------------------------------------------------------
// 错误
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum UpdateError {
    Network(String),
    Status(u16),
    Manifest(String),
    Version(String),
    Hash,
    Io(String),
    Url,
    Cancelled,
}

fn to_command_error(e: UpdateError) -> CommandError {
    let (code, message) = match &e {
        UpdateError::Network(d) => ("update_network", d.clone()),
        UpdateError::Status(c) => ("update_status", format!("HTTP {c}")),
        UpdateError::Manifest(d) => ("update_manifest", d.clone()),
        UpdateError::Version(d) => ("update_version", d.clone()),
        UpdateError::Hash => ("update_hash", "SHA-256 校验失败".into()),
        UpdateError::Io(d) => ("io", d.clone()),
        UpdateError::Url => ("update_url", "不支持的 URL".into()),
        UpdateError::Cancelled => ("update_cancelled", "下载已取消".into()),
    };
    CommandError::new(code, message)
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_parses() {
        let m: Manifest = serde_json::from_str(
            r#"{"version":"1.2.3","url":"https://example.com/a.exe","sha256":"AA","notes":"fix"}"#,
        )
        .expect("parse");
        assert_eq!(m.version, "1.2.3");
        assert_eq!(m.notes, "fix");
    }

    #[test]
    fn version_compare_handles_prefix() {
        assert!(manifest_newer("1.2.3", "1.2.2").unwrap());
        assert!(!manifest_newer("1.2.3", "1.2.3").unwrap());
        assert!(!manifest_newer("1.2.3", "1.3.0").unwrap());
        assert!(manifest_newer("v2.0.0", "1.9.9").unwrap());
    }

    #[test]
    fn invalid_manifest_rejected() {
        assert!(serde_json::from_slice::<Manifest>(b"{").is_err());
        assert!(serde_json::from_slice::<Manifest>(br#"{"version":"1.0.0"}"#).is_err());
    }

    #[test]
    fn file_name_from_various_urls() {
        assert_eq!(
            file_name_from_url("https://x.com/a/image-view_1.0.1_x64-setup.exe?t=1"),
            "image-view_1.0.1_x64-setup.exe"
        );
        assert_eq!(
            file_name_from_url("https://x.com/"),
            "image-view-update.exe"
        );
    }

    #[test]
    fn local_path_variants() {
        assert!(local_path("ftp://x").is_none());
        assert!(local_path("http://x").is_none());
        assert!(local_path("/tmp/a").is_some());
        assert!(local_path("file:///tmp/a").is_some());
    }

    #[test]
    fn hex_encoding() {
        assert_eq!(hex(&[0xde, 0xad, 0xbe, 0xef]), "deadbeef");
    }
}
