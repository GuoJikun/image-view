//! 日志与崩溃转储。
//!
//! - 常规日志优先写入可执行文件所在目录的 `logs\`，即安装目录
//!   （开发模式为 `target\<profile>\logs\`）；该目录不可写时回退到用户数据
//!   目录（Windows `%LOCALAPPDATA%\image-view\logs`、macOS
//!   `~/Library/Logs/image-view`、Linux `$XDG_STATE_HOME/image-view/logs`）。
//!   `IMAGE_VIEW_LOG_DIR` 可完全覆盖。单文件超过 4 MiB 时轮转为
//!   `.1`、`.2`、`.3`，最多保留 3 份历史。
//! - 每行格式为 `[<时间> <级别> <target>] <消息>`，时间按**用户本地时区**
//!   渲染并带 UTC 偏移（`2026-11-16T14:33:37.123+08:00`），与用户看到的系统
//!   时间一致；环境变量 `TZ` 生效，夏令时由系统时区数据库决定。
//! - panic 时额外写一份 `crash-*.log`，包含版本、系统、位置与 backtrace，
//!   最多保留 10 份，方便用户反馈问题。
//! - release 构建使用 `windows_subsystem = "windows"`，没有控制台，文件是
//!   唯一的诊断入口；因此日志初始化失败时不应阻塞启动。

use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, FixedOffset, Local};

/// 覆盖日志目录的环境变量（测试或便携场景）。
pub const LOG_DIR_ENV: &str = "IMAGE_VIEW_LOG_DIR";

/// 当前日志文件名。
pub const LOG_FILE_NAME: &str = "image-view.log";

/// 安装目录下用于存放日志的子目录名。
pub const LOG_DIR_NAME: &str = "logs";

/// 单份日志的轮转阈值。
const MAX_LOG_BYTES: u64 = 4 * 1024 * 1024;

/// 保留的历史日志份数（`.1` ~ `.3`）。
const LOG_ARCHIVES: usize = 3;

/// 保留的崩溃转储份数。
const MAX_CRASH_FILES: usize = 10;

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static CRASH_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 日志目录：`IMAGE_VIEW_LOG_DIR` > 安装目录（可执行文件同级的 `logs`）> 平台用户数据目录。
pub fn log_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os(LOG_DIR_ENV).filter(|value| !value.is_empty()) {
        return PathBuf::from(dir);
    }
    install_log_dir().unwrap_or_else(platform_log_dir)
}

/// 安装目录下的日志目录；目录不可写时为 `None`。
///
/// 只探测一次：启动后目录权限不会变化，而 `log_dir()` 会被日志初始化、
/// 「打开日志文件夹」命令与崩溃转储多次调用。
fn install_log_dir() -> Option<PathBuf> {
    static CACHE: OnceLock<Option<PathBuf>> = OnceLock::new();
    CACHE.get_or_init(resolve_install_log_dir).clone()
}

#[cfg(target_os = "macos")]
fn resolve_install_log_dir() -> Option<PathBuf> {
    // macOS 的安装目录是 `.app` 包内部，写入会破坏代码签名与公证，
    // 系统升级安装时也会被整体替换；因此固定回退到 ~/Library/Logs。
    None
}

#[cfg(not(target_os = "macos"))]
fn resolve_install_log_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    install_log_dir_from(&exe)
}

/// 由可执行文件路径推导安装目录下的日志目录；不可写时为 `None`。
#[cfg(not(target_os = "macos"))]
fn install_log_dir_from(exe: &Path) -> Option<PathBuf> {
    let dir = exe.parent()?.join(LOG_DIR_NAME);
    is_writable(&dir).then_some(dir)
}

/// 创建目录并写入探针文件，确认当前用户确实可写。
///
/// 系统级安装（`/usr/bin`、`Program Files`）与 AppImage 的挂载目录都不可写，
/// 需要据此回退，避免启动时报错或静默丢日志。
fn is_writable(dir: &Path) -> bool {
    if fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".write-probe");
    match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
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
            .format(format_record)
            .try_init();
        }
        Err(error) => {
            env_logger::Builder::from_env(
                env_logger::Env::default().default_filter_or("warn,image_view=info"),
            )
            .format(format_record)
            .init();
            log::error!(
                target: "image_view::logging",
                "无法创建日志文件 {}：{error}",
                path.display()
            );
        }
    }

    log_startup();
}

/// 安装 panic hook：先写常规日志与独立崩溃文件，再交给默认 hook。
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let report = panic_report(info);
        log::error!(target: "image_view::logging", "{report}");
        if let Err(error) = write_crash_report(&report) {
            log::error!(target: "image_view::logging", "无法写入崩溃日志：{error}");
        }
        previous(info);
    }));
}

fn log_startup() {
    log::info!(
        target: "image_view::logging",
        "image-view {} 启动（{} {}），日志：{}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        log_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<仅 stderr>".to_owned())
    );
}

/// `env_logger` 的自定义格式器。
///
/// 默认格式器的时间戳走 `humantime`，恒为 UTC 且不可配置时区，
/// 因此这里自己拼一行：`[<本地时间> <级别> <target>] <消息>`。
fn format_record(buf: &mut env_logger::fmt::Formatter, record: &log::Record<'_>) -> io::Result<()> {
    let stamp = local_stamp(SystemTime::now());
    let message = record.args().to_string();
    let line = format_line(&stamp, record.level(), record.target(), &message);
    buf.write_all(line.as_bytes())
}

/// 拼一行日志；多行消息的后续行缩进 4 空格，与 `env_logger` 默认格式一致。
fn format_line(stamp: &str, level: log::Level, target: &str, message: &str) -> String {
    let mut out = String::with_capacity(stamp.len() + target.len() + message.len() + 16);
    let mut lines = message.split('\n');
    let head = lines.next().unwrap_or_default();
    let _ = writeln!(out, "[{stamp} {level:<5} {target}] {head}");
    for line in lines {
        let _ = writeln!(out, "    {line}");
    }
    out
}

/// 当前本地时间戳，形如 `2026-11-16T14:33:37.123+08:00`。
fn local_stamp(now: SystemTime) -> String {
    format_stamp(&DateTime::<Local>::from(now).fixed_offset())
}

/// 按给定偏移渲染时间戳。偏移决定日期与时钟，夏令时由系统时区换算后落到固定偏移上。
fn format_stamp(dt: &DateTime<FixedOffset>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S%.3f%:z").to_string()
}

/// 崩溃报告里的时间：本地时间 + 偏移，精确到秒。
fn local_display(now: SystemTime) -> String {
    DateTime::<Local>::from(now)
        .format("%Y-%m-%d %H:%M:%S %:z")
        .to_string()
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
         时间(本地): {}\n\
         位置: {location}\n\
         消息: {message}\n\
         系统: {} {}\n\
         \nbacktrace:\n{}",
        env!("CARGO_PKG_VERSION"),
        local_display(SystemTime::now()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "image-view-logging-test-{}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn install_log_dir_prefers_executable_parent() {
        let base = temp_dir("install");
        let exe = base.join("image-view.exe");
        fs::write(&exe, b"MZ").expect("write fake exe");

        let dir = install_log_dir_from(&exe).expect("安装目录可写时应返回 logs 目录");
        assert_eq!(dir, base.join(LOG_DIR_NAME), "日志目录应与可执行文件同级");
        assert!(dir.is_dir(), "应创建 logs 目录");
        assert!(!dir.join(".write-probe").exists(), "可写性探针文件应被清理");

        let _ = fs::remove_dir_all(&base);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn install_log_dir_rejects_unwritable_location() {
        let base = temp_dir("install-blocked");
        let blocker = base.join("blocker");
        fs::write(&blocker, b"not a directory").expect("write blocking file");

        assert!(
            install_log_dir_from(&blocker.join("image-view.exe")).is_none(),
            "父路径不可创建目录时应判定为不可写"
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn stamps_are_rendered_in_the_given_offset() {
        let instant = UNIX_EPOCH + std::time::Duration::from_millis(1_700_000_000_007);

        assert_eq!(
            format_stamp(&in_offset(instant, 8 * 3_600)),
            "2023-11-15T06:13:20.007+08:00",
            "东八区应比 UTC 早 8 小时"
        );
        assert_eq!(
            format_stamp(&in_offset(instant, -5 * 3_600)),
            "2023-11-14T17:13:20.007-05:00",
            "西五区应比 UTC 晚 5 小时"
        );
        assert_eq!(
            format_stamp(&in_offset(instant, 0)),
            "2023-11-14T22:13:20.007+00:00",
            "零偏移应等于 UTC"
        );
    }

    #[test]
    fn stamps_keep_milliseconds_and_handle_the_epoch() {
        assert_eq!(
            format_stamp(&in_offset(UNIX_EPOCH, 0)),
            "1970-01-01T00:00:00.000+00:00",
            "Unix 纪元应渲染为 1970-01-01"
        );
    }

    #[test]
    fn local_stamp_uses_local_offset() {
        let stamp = local_stamp(SystemTime::now());
        // 具体时区依运行环境而定，这里只校验结构与偏移写法。
        assert_eq!(
            stamp.len(),
            29,
            "`YYYY-MM-DDTHH:MM:SS.mmm+HH:MM` 共 29 字符"
        );
        assert_eq!(&stamp[10..11], "T", "日期与时间之间用 `T` 分隔");
        assert!(
            stamp.ends_with(":00") || stamp.ends_with(":30") || stamp.ends_with(":45"),
            "偏移的分钟部分应为 00 / 30 / 45：{stamp}"
        );
    }

    #[test]
    fn log_lines_keep_the_default_layout() {
        assert_eq!(
            format_line(
                "2026-11-16T14:33:37.123+08:00",
                log::Level::Info,
                "image_view",
                "启动"
            ),
            "[2026-11-16T14:33:37.123+08:00 INFO  image_view] 启动\n",
            "级别应左对齐补齐 5 列，消息后换行"
        );
        assert_eq!(
            format_line("S", log::Level::Error, "t", "第一行\n第二行"),
            "[S ERROR t] 第一行\n    第二行\n",
            "多行消息的后续行应缩进 4 空格"
        );
    }

    fn in_offset(instant: SystemTime, seconds_east: i32) -> DateTime<FixedOffset> {
        let offset = FixedOffset::east_opt(seconds_east).expect("合法偏移");
        DateTime::<Utc>::from(instant).with_timezone(&offset)
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
