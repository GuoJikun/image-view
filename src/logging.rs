//! 日志与崩溃转储。
//!
//! - 常规日志写入 `%LOCALAPPDATA%\image-view\logs\image-view.log`
//!   （可用 `IMAGE_VIEW_LOG_DIR` 覆盖），单文件超过 4 MiB 时轮转为
//!   `.1`、`.2`、`.3`，最多保留 3 份历史。
//! - panic 时额外写一份 `crash-*.log`，包含版本、系统、位置与 backtrace，
//!   最多保留 10 份，方便用户反馈问题。
//! - release 构建使用 `windows_subsystem = "windows"`，没有控制台，文件是
//!   唯一的诊断入口；因此日志初始化失败时不应阻塞启动。

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// 覆盖日志目录的环境变量（测试或便携场景）。
pub const LOG_DIR_ENV: &str = "IMAGE_VIEW_LOG_DIR";

/// 当前日志文件名。
pub const LOG_FILE_NAME: &str = "image-view.log";

/// 单份日志的轮转阈值。
const MAX_LOG_BYTES: u64 = 4 * 1024 * 1024;

/// 保留的历史日志份数（`.1` ~ `.3`）。
const LOG_ARCHIVES: usize = 3;

/// 保留的崩溃转储份数。
const MAX_CRASH_FILES: usize = 10;

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static CRASH_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 日志目录；优先使用 `IMAGE_VIEW_LOG_DIR`，否则按平台约定。
pub fn log_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os(LOG_DIR_ENV).filter(|value| !value.is_empty()) {
        return PathBuf::from(dir);
    }
    platform_log_dir()
}

/// 当前日志文件路径；日志文件创建失败时为 `None`。
pub fn log_path() -> Option<&'static Path> {
    LOG_PATH.get().map(PathBuf::as_path)
}

#[cfg(target_os = "windows")]
fn platform_log_dir() -> PathBuf {
    env_base("LOCALAPPDATA")
        .map(|base| base.join("image-view").join("logs"))
        .unwrap_or_else(fallback_log_dir)
}

#[cfg(target_os = "macos")]
fn platform_log_dir() -> PathBuf {
    env_base("HOME")
        .map(|home| home.join("Library").join("Logs").join("image-view"))
        .unwrap_or_else(fallback_log_dir)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_log_dir() -> PathBuf {
    env_base("XDG_STATE_HOME")
        .map(|base| base.join("image-view").join("logs"))
        .or_else(|| env_base("HOME").map(|home| home.join(".local/state/image-view/logs")))
        .unwrap_or_else(fallback_log_dir)
}

#[cfg(not(any(unix, target_os = "windows")))]
fn platform_log_dir() -> PathBuf {
    fallback_log_dir()
}

fn env_base(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn fallback_log_dir() -> PathBuf {
    std::env::temp_dir().join("image-view-logs")
}

/// 初始化文件 + stderr 双写日志。失败时退回 stderr，不阻塞启动。
pub fn init() {
    let dir = log_dir();
    let path = dir.join(LOG_FILE_NAME);

    match open_log(&dir, &path) {
        Ok(file) => {
            let _ = LOG_PATH.set(path.clone());
            let writer = TeeWriter {
                file: Mutex::new(file),
            };
            let _ = env_logger::Builder::from_env(
                env_logger::Env::default().default_filter_or("warn,image_view=info"),
            )
            .target(env_logger::Target::Pipe(Box::new(writer)))
            .format_timestamp_millis()
            .try_init();
        }
        Err(error) => {
            env_logger::Builder::from_env(
                env_logger::Env::default().default_filter_or("warn,image_view=info"),
            )
            .format_timestamp_millis()
            .init();
            log::error!("无法创建日志文件 {}：{error}", path.display());
        }
    }

    log_startup();
}

/// 安装 panic hook：先写常规日志与独立崩溃文件，再交给默认 hook。
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let report = panic_report(info);
        log::error!("{report}");
        if let Err(error) = write_crash_report(&report) {
            log::error!("无法写入崩溃日志：{error}");
        }
        previous(info);
    }));
}

fn log_startup() {
    log::info!(
        "image-view {} 启动（{} {}），日志：{}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        log_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<仅 stderr>".to_owned())
    );
}

/// 同时写文件和 stderr。文件层失败不向调用方传播，避免日志本身导致崩溃。
struct TeeWriter {
    file: Mutex<File>,
}

impl Write for TeeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.write_all(buf);
        }
        let _ = io::stderr().write_all(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
        let _ = io::stderr().flush();
        Ok(())
    }
}

/// 创建日志目录、按需轮转并打开追加写入的文件。
fn open_log(dir: &Path, path: &Path) -> io::Result<File> {
    fs::create_dir_all(dir)?;
    rotate(path, MAX_LOG_BYTES, LOG_ARCHIVES);
    OpenOptions::new().create(true).append(true).open(path)
}

/// 超过 `max_bytes` 时把 `log` 轮转为 `log.1`，旧档依次后移。
fn rotate(path: &Path, max_bytes: u64, archives: usize) {
    let len = fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    if len < max_bytes || archives == 0 {
        return;
    }

    for index in (1..archives).rev() {
        let target = archive_path(path, index + 1);
        // Windows 的 rename 不会覆盖已存在的目标，先删掉最旧的一份。
        let _ = fs::remove_file(&target);
        let _ = fs::rename(archive_path(path, index), target);
    }
    let first = archive_path(path, 1);
    let _ = fs::remove_file(&first);
    let _ = fs::rename(path, first);
}

fn archive_path(path: &Path, index: usize) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{index}"));
    path.with_file_name(name)
}

fn panic_report(info: &std::panic::PanicHookInfo<'_>) -> String {
    let message = info
        .payload()
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "<非字符串 panic>".to_owned());
    let location = info
        .location()
        .map(|location| {
            format!(
                "{}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            )
        })
        .unwrap_or_else(|| "<未知位置>".to_owned());

    format!(
        "image-view {} 发生崩溃\n\
         时间(UTC): {}\n\
         位置: {location}\n\
         消息: {message}\n\
         系统: {} {}\n\
         \nbacktrace:\n{}",
        env!("CARGO_PKG_VERSION"),
        utc_timestamp(SystemTime::now()),
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::backtrace::Backtrace::force_capture()
    )
}

fn write_crash_report(report: &str) -> io::Result<PathBuf> {
    let dir = log_dir();
    fs::create_dir_all(&dir)?;
    prune_crashes(&dir, MAX_CRASH_FILES.saturating_sub(1));

    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let counter = CRASH_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = dir.join(format!(
        "crash-{millis}-{}-{counter}.log",
        std::process::id()
    ));

    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)?;
    file.write_all(report.as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(path)
}

/// 只保留最新的 `keep` 份崩溃转储；文件名以毫秒时间戳开头，可按名称排序。
fn prune_crashes(dir: &Path, keep: usize) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("crash-") && name.ends_with(".log"))
        })
        .collect();

    if files.len() <= keep {
        return;
    }

    files.sort();
    let remove_count = files.len() - keep;
    for path in files.into_iter().take(remove_count) {
        let _ = fs::remove_file(path);
    }
}

/// `YYYYMMDD-HHMMSS`（UTC），不引入日期库。
fn utc_timestamp(now: SystemTime) -> String {
    let seconds = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let days = (seconds / 86_400) as i64;
    let remainder = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = remainder / 3_600;
    let minute = (remainder % 3_600) / 60;
    let second = remainder % 60;

    format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}")
}

/// Howard Hinnant 的 `civil_from_days`：1970-01-01 起的天数 → 年月日。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    } as u32;

    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "image-view-logging-test-{}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn utc_timestamp_matches_known_values() {
        assert_eq!(
            utc_timestamp(UNIX_EPOCH),
            "19700101-000000",
            "Unix 纪元应为 1970-01-01"
        );
        assert_eq!(
            utc_timestamp(UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000)),
            "20231114-221320",
            "1700000000 应换算为 2023-11-14 22:13:20 UTC"
        );
    }

    #[test]
    fn rotate_moves_log_and_shifts_archives() {
        let dir = temp_dir("rotate");
        let path = dir.join(LOG_FILE_NAME);
        fs::write(&path, b"new").expect("write current log");
        fs::write(archive_path(&path, 1), b"older").expect("write archive 1");
        fs::write(archive_path(&path, 2), b"oldest").expect("write archive 2");

        rotate(&path, 1, 3);

        assert!(!path.exists(), "轮转后当前日志应被移走");
        assert_eq!(
            fs::read(archive_path(&path, 1)).expect("archive 1"),
            b"new",
            "当前日志应成为 .1"
        );
        assert_eq!(
            fs::read(archive_path(&path, 2)).expect("archive 2"),
            b"older",
            "旧 .1 应后移为 .2"
        );
        assert_eq!(
            fs::read(archive_path(&path, 3)).expect("archive 3"),
            b"oldest",
            "旧 .2 应后移为 .3"
        );

        // 第二次轮转：目标档已存在，Windows 上必须能覆盖最旧一份。
        fs::write(&path, b"newer").expect("write current log again");
        rotate(&path, 1, 3);
        assert_eq!(
            fs::read(archive_path(&path, 1)).expect("archive 1 after second rotate"),
            b"newer"
        );
        assert_eq!(
            fs::read(archive_path(&path, 2)).expect("archive 2 after second rotate"),
            b"new"
        );
        assert_eq!(
            fs::read(archive_path(&path, 3)).expect("archive 3 after second rotate"),
            b"older"
        );
        assert!(
            !archive_path(&path, 4).exists(),
            "只保留 3 份历史，最旧的应被丢弃"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rotate_skips_small_log() {
        let dir = temp_dir("rotate-small");
        let path = dir.join(LOG_FILE_NAME);
        fs::write(&path, b"small").expect("write log");

        rotate(&path, 1024, 3);

        assert!(path.exists());
        assert!(!archive_path(&path, 1).exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn prune_crashes_keeps_newest_by_name() {
        let dir = temp_dir("prune");
        for index in 0..15 {
            fs::write(dir.join(format!("crash-{index:04}-1-0.log")), b"report")
                .expect("write crash file");
        }
        fs::write(dir.join("image-view.log"), b"normal").expect("write normal log");

        prune_crashes(&dir, 10);

        let crashes: Vec<_> = fs::read_dir(&dir)
            .expect("read dir")
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("crash-"))
            .collect();
        assert_eq!(crashes.len(), 10, "应只保留 10 份崩溃转储");
        assert!(dir.join("crash-0014-1-0.log").exists(), "最新一份应保留");
        assert!(!dir.join("crash-0000-1-0.log").exists(), "最旧一份应删除");
        assert!(dir.join("image-view.log").exists(), "普通日志不应被清理");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_log_creates_directory_and_appends() {
        let dir = temp_dir("open").join("nested").join("logs");
        let path = dir.join(LOG_FILE_NAME);

        let mut file = open_log(&dir, &path).expect("open log");
        file.write_all(b"hello\n").expect("write log");
        drop(file);
        let mut file = open_log(&dir, &path).expect("reopen log");
        file.write_all(b"world\n").expect("append log");
        drop(file);

        assert_eq!(
            fs::read_to_string(&path).expect("read log"),
            "hello\nworld\n",
            "日志应以追加模式打开"
        );

        let _ = fs::remove_dir_all(dir.parent().unwrap().parent().unwrap());
    }
}
