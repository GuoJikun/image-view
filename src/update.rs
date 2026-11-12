//! 自动更新：检查发布清单、下载安装包、SHA-256 校验与安装。
//!
//! 网络与磁盘操作都在后台线程执行，结果通过通道回主线程；检查与下载互不阻塞。
//! 更新清单由 `scripts/build-release.ps1` 随安装包生成，默认从 GitHub Release
//! 的 `update.json` 读取；可用 `IMAGE_VIEW_UPDATE_MANIFEST` 指向本地文件或测试地址。

use std::fmt::Display;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::i18n::{self, Key};
use crate::logging;

/// 默认更新清单地址；发布时由 build-release 脚本把 `update.json` 上传为 Release 资产。
pub const MANIFEST_URL: &str =
    "https://github.com/GuoJikun/image-view/releases/latest/download/update.json";

/// 覆盖清单地址（本地文件或测试服务器）。
pub const MANIFEST_ENV: &str = "IMAGE_VIEW_UPDATE_MANIFEST";
/// 覆盖下载目录，便于测试。
pub const DOWNLOAD_DIR_ENV: &str = "IMAGE_VIEW_UPDATE_DIR";

const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const TIMEOUT: Duration = Duration::from_secs(20);
const PROGRESS_STEP: u64 = 256 * 1024;

/// 更新清单。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub notes: String,
}

impl Manifest {
    /// 解析清单；字段缺失或 JSON 损坏时返回错误。
    pub fn parse(bytes: &[u8]) -> Result<Self, UpdateError> {
        serde_json::from_slice(bytes).map_err(|error| UpdateError::Manifest(error.to_string()))
    }

    /// 清单版本是否高于当前版本。
    pub fn newer_than(&self, current: &str) -> Result<bool, UpdateError> {
        let latest = parse_version(&self.version)?;
        let current = parse_version(current)?;
        Ok(latest > current)
    }

    /// 安装包文件名：取 URL 最后一段并去掉查询串；异常时使用固定名称。
    pub fn file_name(&self) -> String {
        let path = self.url.split(['?', '#']).next().unwrap_or(&self.url);
        let name = path
            .rsplit(['/', '\\'])
            .next()
            .filter(|name| !name.is_empty() && *name != "." && *name != "..");
        match name {
            Some(name) => name.to_owned(),
            None => "image-view-update.exe".to_owned(),
        }
    }
}

fn parse_version(text: &str) -> Result<semver::Version, UpdateError> {
    semver::Version::parse(text.trim().trim_start_matches(['v', 'V']))
        .map_err(|error| UpdateError::Version(format!("{text}: {error}")))
}

/// 可展示的更新错误；`message()` 按当前语言生成文案，切换语言后仍然正确。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateError {
    Network(String),
    Status(u16),
    Manifest(String),
    Version(String),
    Hash,
    Io(String),
    Url,
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    Unsupported,
}

impl UpdateError {
    pub fn message(&self) -> String {
        match self {
            Self::Network(detail) => i18n::tf(Key::UpdateErrNetwork, &[detail]),
            Self::Status(code) => i18n::tf(Key::UpdateErrStatus, &[code]),
            Self::Manifest(detail) => i18n::tf(Key::UpdateErrManifest, &[detail]),
            Self::Version(detail) => i18n::tf(Key::UpdateErrVersion, &[detail]),
            Self::Hash => i18n::t(Key::UpdateErrHash).to_owned(),
            Self::Io(detail) => i18n::tf(Key::UpdateErrIo, &[detail]),
            Self::Url => i18n::t(Key::UpdateErrUrl).to_owned(),
            Self::Unsupported => i18n::t(Key::UpdateNotSupported).to_owned(),
        }
    }
}

impl Display for UpdateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message())
    }
}

/// 界面展示的更新状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateState {
    /// 尚未检查。
    Idle,
    Checking,
    UpToDate,
    Available {
        manifest: Manifest,
    },
    Downloading {
        received: u64,
        total: Option<u64>,
    },
    Ready {
        installer: PathBuf,
        manifest: Manifest,
    },
    Error(UpdateError),
}

enum UpdateEvent {
    Checked(Result<Option<Manifest>, UpdateError>),
    Progress { received: u64, total: Option<u64> },
    Downloaded(Result<(PathBuf, Manifest), UpdateError>),
}

/// 更新控制器：后台线程 + 主线程轮询。
pub struct Updater {
    tx: Sender<UpdateEvent>,
    rx: Receiver<UpdateEvent>,
    pub state: UpdateState,
}

impl Default for Updater {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Updater {
    fn drop(&mut self) {
        // 尽力清理上次运行残留的半成品；下载线程仍持有文件时删除会失败，
        // 由下一次启动时的 `new()` 再清理。
        cleanup_partials_in(&download_dir());
    }
}

impl Updater {
    pub fn new() -> Self {
        cleanup_partials_in(&download_dir());
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            state: UpdateState::Idle,
        }
    }

    /// 当前版本（取自 Cargo 包版本）。
    pub fn current_version() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    /// 清单地址：环境变量优先，否则使用发布页默认地址。
    pub fn manifest_url() -> String {
        std::env::var(MANIFEST_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| MANIFEST_URL.to_owned())
    }

    pub fn is_busy(&self) -> bool {
        matches!(
            self.state,
            UpdateState::Checking | UpdateState::Downloading { .. }
        )
    }

    /// 在后台检查更新；已有下载/检查在进行时忽略。
    pub fn check(&mut self, ctx: &eframe::egui::Context) {
        if self.is_busy() {
            return;
        }
        self.state = UpdateState::Checking;
        let url = Self::manifest_url();
        let current = Self::current_version().to_owned();
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        spawn("update-check", move || {
            let result = check_manifest(&url, &current);
            let _ = tx.send(UpdateEvent::Checked(result));
            ctx.request_repaint();
        });
    }

    /// 下载并校验安装包。
    pub fn download(&mut self, ctx: &eframe::egui::Context, manifest: Manifest) {
        if self.is_busy() {
            return;
        }
        self.state = UpdateState::Downloading {
            received: 0,
            total: None,
        };
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        spawn("update-download", move || {
            let dir = download_dir();
            let progress_tx = tx.clone();
            let progress_ctx = ctx.clone();
            let result = download_to(&manifest, &dir, &move |received, total| {
                let _ = progress_tx.send(UpdateEvent::Progress { received, total });
                progress_ctx.request_repaint();
            })
            .map(|path| (path, manifest));
            let _ = tx.send(UpdateEvent::Downloaded(result));
            ctx.request_repaint();
        });
    }

    /// 轮询后台结果；返回 `true` 表示状态发生了变化。
    pub fn poll(&mut self) -> bool {
        let before = self.state.clone();
        while let Ok(event) = self.rx.try_recv() {
            match event {
                UpdateEvent::Checked(Ok(Some(manifest))) => {
                    self.state = UpdateState::Available { manifest };
                }
                UpdateEvent::Checked(Ok(None)) => self.state = UpdateState::UpToDate,
                UpdateEvent::Checked(Err(error)) => self.state = UpdateState::Error(error),
                UpdateEvent::Progress { received, total } => {
                    self.state = UpdateState::Downloading { received, total };
                }
                UpdateEvent::Downloaded(Ok((installer, manifest))) => {
                    self.state = UpdateState::Ready {
                        installer,
                        manifest,
                    };
                }
                UpdateEvent::Downloaded(Err(error)) => self.state = UpdateState::Error(error),
            }
        }
        self.state != before
    }

    /// 记录安装失败等错误，由更新窗口展示。
    pub fn report(&mut self, error: UpdateError) {
        self.state = UpdateState::Error(error);
    }

    /// 启动安装器；调用方需要随后退出应用。
    pub fn install(installer: &Path) -> Result<(), UpdateError> {
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new(installer)
                .arg("/S")
                .spawn()
                .map(|_| ())
                .map_err(|error| UpdateError::Io(error.to_string()))
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = installer;
            Err(UpdateError::Unsupported)
        }
    }
}

fn spawn(name: &str, task: impl FnOnce() + Send + 'static) {
    if let Err(error) = std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(task)
    {
        log::error!("无法启动更新线程：{error}");
    }
}

/// 下载目录：`%LOCALAPPDATA%\image-view\updates`，与日志目录同级。
pub fn download_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os(DOWNLOAD_DIR_ENV).filter(|value| !value.is_empty()) {
        return PathBuf::from(dir);
    }
    logging::log_dir()
        .parent()
        .map(|base| base.join("updates"))
        .unwrap_or_else(|| std::env::temp_dir().join("image-view-updates"))
}

fn check_manifest(url: &str, current: &str) -> Result<Option<Manifest>, UpdateError> {
    let (mut reader, _) = open_source(url)?;
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| UpdateError::Io(error.to_string()))?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(UpdateError::Manifest("manifest too large".to_owned()));
    }

    let manifest = Manifest::parse(&bytes)?;
    if manifest.newer_than(current)? {
        Ok(Some(manifest))
    } else {
        Ok(None)
    }
}

/// 同步下载并校验；供后台线程与单元测试复用。
fn download_to(
    manifest: &Manifest,
    dir: &Path,
    progress: &(dyn Fn(u64, Option<u64>) + Send),
) -> Result<PathBuf, UpdateError> {
    fs::create_dir_all(dir)
        .map_err(|error| UpdateError::Io(format!("{}: {error}", dir.display())))?;

    let (mut reader, total) = open_source(&manifest.url)?;
    let target = dir.join(manifest.file_name());
    let part = target.with_extension("part");

    let mut file = File::create(&part)
        .map_err(|error| UpdateError::Io(format!("{}: {error}", part.display())))?;
    let mut hasher = Sha256::new();
    let mut received = 0u64;
    let mut last_report = 0u64;
    let mut buffer = vec![0u8; 64 * 1024];

    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| UpdateError::Io(error.to_string()))?;
        if count == 0 {
            break;
        }
        file.write_all(&buffer[..count])
            .map_err(|error| UpdateError::Io(error.to_string()))?;
        hasher.update(&buffer[..count]);
        received += count as u64;
        if received - last_report >= PROGRESS_STEP {
            progress(received, total);
            last_report = received;
        }
    }

    file.sync_all()
        .map_err(|error| UpdateError::Io(error.to_string()))?;
    drop(file);

    let digest = hex(&hasher.finalize());
    if !digest.eq_ignore_ascii_case(manifest.sha256.trim()) {
        let _ = fs::remove_file(&part);
        return Err(UpdateError::Hash);
    }

    progress(received, total);
    fs::rename(&part, &target)
        .map_err(|error| UpdateError::Io(format!("{}: {error}", target.display())))?;
    log::info!("更新安装包已下载并通过校验：{}", target.display());
    Ok(target)
}

/// 打开清单或安装包来源；支持 http(s)、`file://` 与本地路径（便于离线测试）。
fn open_source(url: &str) -> Result<(Box<dyn Read>, Option<u64>), UpdateError> {
    if let Some(path) = local_path(url) {
        let file = File::open(&path)
            .map_err(|error| UpdateError::Io(format!("{}: {error}", path.display())))?;
        let total = file.metadata().ok().map(|metadata| metadata.len());
        return Ok((Box::new(file), total));
    }

    if url.starts_with("https://") || url.starts_with("http://") {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            // 不用 timeout_global：大安装包在慢速网络上可能超过总时限。
            // 每次连接/读响应头/读 body 分块各自限时，既能中断卡死连接，又不影响持续下载。
            .timeout_connect(Some(TIMEOUT))
            .timeout_recv_response(Some(TIMEOUT))
            .timeout_per_read(Some(TIMEOUT))
            .build()
            .into();
        let response = agent.get(url).call().map_err(|error| match error {
            ureq::Error::StatusCode(code) => UpdateError::Status(code),
            other => UpdateError::Network(other.to_string()),
        })?;
        let body = response.into_body();
        let total = body.content_length();
        return Ok((Box::new(body.into_reader()), total));
    }

    Err(UpdateError::Url)
}

/// 删除更新目录中未完成的 `.part` 文件；目录不存在时静默跳过。
fn cleanup_partials_in(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_part = path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("part"));
        if is_part && fs::remove_file(&path).is_ok() {
            log::info!("已清理未完成的更新文件：{}", path.display());
        }
    }
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

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn temp_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "image-view-update-test-{}-{tag}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn file_url(path: &Path) -> String {
        format!("file://{}", path.display())
    }

    #[test]
    fn manifest_parses_and_compares_versions() {
        let manifest = Manifest::parse(
            br#"{"version":"1.2.3","url":"https://example.com/a.exe","sha256":"AA","notes":"fix"}"#,
        )
        .expect("parse");

        assert_eq!(manifest.version, "1.2.3");
        assert!(manifest.newer_than("1.2.2").expect("compare"));
        assert!(!manifest.newer_than("1.2.3").expect("compare"));
        assert!(!manifest.newer_than("1.3.0").expect("compare"));

        let prefixed = Manifest {
            version: "v2.0.0".to_owned(),
            ..manifest
        };
        assert!(prefixed.newer_than("1.9.9").expect("compare"));
    }

    #[test]
    fn invalid_manifest_reports_error() {
        assert!(Manifest::parse(b"{").is_err());
        assert!(Manifest::parse(br#"{"version":"1.0.0"}"#).is_err());

        let manifest = Manifest {
            version: "not-a-version".to_owned(),
            url: String::new(),
            sha256: String::new(),
            notes: String::new(),
        };
        assert!(matches!(
            manifest.newer_than("1.0.0"),
            Err(UpdateError::Version(_))
        ));
    }

    #[test]
    fn installer_name_comes_from_url() {
        let base = Manifest {
            version: "1.0.1".to_owned(),
            url: String::new(),
            sha256: String::new(),
            notes: String::new(),
        };

        assert_eq!(
            Manifest {
                url: "https://example.com/download/image-view_1.0.1_x64-setup.exe?token=1"
                    .to_owned(),
                ..base.clone()
            }
            .file_name(),
            "image-view_1.0.1_x64-setup.exe"
        );
        assert_eq!(
            Manifest {
                url: "https://example.com/".to_owned(),
                ..base
            }
            .file_name(),
            "image-view-update.exe"
        );
    }

    #[test]
    fn download_verifies_hash_before_publishing() {
        let dir = temp_dir("hash-ok");
        let source = dir.join("setup.exe");
        fs::write(&source, b"installer-bytes").expect("write source");
        let expected = hex(&Sha256::digest(b"installer-bytes"));

        let manifest = Manifest {
            version: "1.0.1".to_owned(),
            url: file_url(&source),
            sha256: expected,
            notes: String::new(),
        };
        let downloaded =
            download_to(&manifest, &dir.join("downloaded"), &|_, _| {}).expect("download");

        assert_eq!(fs::read(&downloaded).expect("read"), b"installer-bytes");
        assert!(!downloaded.with_extension("part").exists());
        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn download_rejects_bad_hash_without_leaving_files() {
        let dir = temp_dir("hash-bad");
        let source = dir.join("setup.exe");
        fs::write(&source, b"installer-bytes").expect("write source");

        let manifest = Manifest {
            version: "1.0.1".to_owned(),
            url: file_url(&source),
            sha256: "00".repeat(32),
            notes: String::new(),
        };
        let target = dir.join("downloaded").join(manifest.file_name());
        let result = download_to(&manifest, &dir.join("downloaded"), &|_, _| {});

        assert_eq!(result, Err(UpdateError::Hash));
        assert!(!target.exists());
        assert!(!target.with_extension("part").exists());
        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn check_manifest_reads_local_file_and_compares() {
        let dir = temp_dir("manifest");
        let path = dir.join("update.json");
        fs::write(
            &path,
            br#"{"version":"9.9.9","url":"https://example.com/a.exe","sha256":"00"}"#,
        )
        .expect("write manifest");

        let found = check_manifest(&file_url(&path), "1.0.0")
            .expect("check")
            .expect("newer");
        assert_eq!(found.version, "9.9.9");

        let none = check_manifest(&file_url(&path), "9.9.9").expect("check");
        assert!(none.is_none());

        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn unsupported_url_is_rejected() {
        assert_eq!(
            open_source("ftp://example.com/a").err(),
            Some(UpdateError::Url)
        );
    }

    #[test]
    fn cleanup_removes_only_part_files() {
        let dir = temp_dir("cleanup");
        let part = dir.join("image-view_1.0.1_x64-setup.part");
        let installer = dir.join("image-view_1.0.0_x64-setup.exe");
        fs::write(&part, b"partial").expect("write part");
        fs::write(&installer, b"installer").expect("write installer");

        cleanup_partials_in(&dir);

        assert!(!part.exists());
        assert!(installer.exists());
        fs::remove_dir_all(&dir).expect("cleanup");
    }

    fn serve_once(status: &str, body: &'static [u8]) -> String {
        use std::io::{BufRead, BufReader};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let status = status.to_owned();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut request = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).expect("read");
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                request.push_str(&line);
            }
            assert!(
                request.starts_with("GET /update.json HTTP/1.1"),
                "{request}"
            );
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .expect("headers");
            stream.write_all(body).expect("body");
        });
        format!("http://{addr}/update.json")
    }

    #[test]
    fn http_manifest_is_checked_through_ureq() {
        let url = serve_once(
            "200 OK",
            br#"{"version":"9.9.9","url":"https://example.com/a.exe","sha256":"00"}"#,
        );
        let manifest = check_manifest(&url, "1.0.0")
            .expect("check")
            .expect("newer");
        assert_eq!(manifest.version, "9.9.9");
    }

    #[test]
    fn http_status_is_reported() {
        let url = serve_once("404 Not Found", b"missing");
        assert_eq!(check_manifest(&url, "1.0.0"), Err(UpdateError::Status(404)));
    }
}
