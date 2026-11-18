#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Iced GUI POC 入口：命令行、日志、窗口与 Application 构建器。
//!
//! 与 eframe 应用共用同一套 Core / Application crate；本文件只负责把宿主
//! 能力（启动参数、日志、图标）接到 Iced。

mod adapters;
mod app;
mod i18n;
mod menu;
mod message;
mod state;
mod theme;
mod update;
mod view;
mod widgets;

use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::OnceLock;

use iv_infra::logging;
use iv_infra::settings::Settings;
use iv_infra::window_state::WindowState;

use crate::adapters::storage::FileStorage;

static STARTUP_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
static STARTUP_WINDOW: OnceLock<Option<WindowState>> = OnceLock::new();

/// `AppState::boot` 读取的启动路径；CLI 在窗口创建前解析完成。
pub fn startup_path() -> Option<PathBuf> {
    STARTUP_PATH.get().cloned().flatten()
}

/// 上次会话保存的主窗口几何；没有记录或记录为空时返回 `None`。
///
/// 需要在创建窗口前读取：有几何时窗口先以隐藏状态创建，等 `WindowReady`
/// 拿到 DPI 后再换算并应用，避免先出现在默认位置再跳动。
pub fn startup_window() -> Option<WindowState> {
    STARTUP_WINDOW.get().cloned().flatten()
}

fn main() -> iced::Result {
    logging::init();
    logging::install_panic_hook();

    let startup = match std::env::args_os().nth(1) {
        Some(arg) if arg == OsStr::new("-h") || arg == OsStr::new("--help") => {
            print_help();
            return Ok(());
        }
        Some(arg) if arg == OsStr::new("-V") || arg == OsStr::new("--version") => {
            println!("image-view {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some(arg) => Some(PathBuf::from(arg)),
        None => None,
    };
    let _ = STARTUP_PATH.set(startup);

    // 窗口几何与其余偏好共用 settings.ron；这里只为「创建窗口前」拿到几何，
    // `AppState::boot` 会再读一次完整设置。
    let saved_window = {
        let storage = FileStorage::open();
        let window = Settings::load(Some(&storage)).window;
        let _ = STARTUP_WINDOW.set((!window.is_unset()).then_some(window));
        startup_window()
    };

    // 标题栏策略（design/macos-first 二/五）：macOS 用系统原生标题栏，
    // Windows / Linux 关装饰后由 view/titlebar.rs 自绘 28px 标题栏。
    #[cfg(not(target_os = "macos"))]
    let decorations = false;
    #[cfg(target_os = "macos")]
    let decorations = true;

    // Windows：无边框窗口不会由系统自动圆角，需显式要求（Win11 Build 22000+ 生效，
    // 更早版本与最大化时系统会忽略，退回方角）。
    #[cfg(target_os = "windows")]
    let platform_specific = iced::window::settings::platform::PlatformSpecific {
        corner_preference: iced::window::settings::platform::CornerPreference::Round,
        ..Default::default()
    };

    let window = iced::window::Settings {
        size: iced::Size::new(1100.0, 760.0),
        min_size: Some(iced::Size::new(720.0, 520.0)),
        icon: load_icon(),
        decorations,
        // 有上次会话的几何时先隐藏创建：等 `WindowReady` 查完 DPI 再应用几何并显示，
        // 避免窗口先出现在默认位置再跳到保存位置。Iced 没有 `set_visible`，
        // 显示由 `window::set_mode(Windowed / Fullscreen)` 完成。
        visible: saved_window.is_none(),
        // 关窗要走自己的流程（采集几何、落盘后再关），因此不能让 Iced 直接关窗。
        exit_on_close_request: false,
        #[cfg(target_os = "windows")]
        platform_specific,
        ..Default::default()
    };

    let result = iced::application(app::boot, app::update, app::view)
        .title(app::title)
        .theme(app::theme)
        .subscription(app::subscription)
        .window(window)
        .settings(iced::Settings {
            id: Some("image-view".to_owned()),
            ..Default::default()
        })
        .run();

    if let Err(error) = &result {
        log::error!("Iced 图形界面启动失败：{error}");
    }
    result
}

fn load_icon() -> Option<iced::window::Icon> {
    let bytes = include_bytes!("../../assets/icon.png");
    let decoded = match image::load_from_memory(bytes) {
        Ok(image) => image.to_rgba8(),
        Err(error) => {
            log::warn!("无法解码窗口图标：{error}");
            return None;
        }
    };
    let (width, height) = decoded.dimensions();
    match iced::window::icon::from_rgba(decoded.into_raw(), width, height) {
        Ok(icon) => Some(icon),
        Err(error) => {
            log::warn!("无法创建窗口图标：{error}");
            None
        }
    }
}

fn print_help() {
    println!(
        "image-view {}
用法:
    image-view [PATH]

参数:
    PATH    要打开的图片文件或目录（目录则打开其中第一张图片）

选项:
    -h, --help       显示帮助
    -V, --version    显示版本",
        env!("CARGO_PKG_VERSION")
    );
}
