#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Iced GUI POC 入口：命令行、日志、窗口与 Application 构建器。
//!
//! 与 eframe 应用共用同一套 Core / Application crate；本文件只负责把宿主
//! 能力（启动参数、日志、图标）接到 Iced。

mod adapters;
mod app;
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

static STARTUP_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

/// `AppState::boot` 读取的启动路径；CLI 在窗口创建前解析完成。
pub fn startup_path() -> Option<PathBuf> {
    STARTUP_PATH.get().cloned().flatten()
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
            println!("image-view-iced {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some(arg) => Some(PathBuf::from(arg)),
        None => None,
    };
    let _ = STARTUP_PATH.set(startup);

    let window = iced::window::Settings {
        size: iced::Size::new(1100.0, 760.0),
        min_size: Some(iced::Size::new(480.0, 320.0)),
        icon: load_icon(),
        ..Default::default()
    };

    let result = iced::application(app::boot, app::update, app::view)
        .title(app::title)
        .theme(app::theme)
        .subscription(app::subscription)
        .window(window)
        .settings(iced::Settings {
            id: Some("image-view-iced".to_owned()),
            ..Default::default()
        })
        .run();

    if let Err(error) = &result {
        log::error!("Iced 图形界面启动失败：{error}");
    }
    result
}

fn load_icon() -> Option<iced::window::Icon> {
    let bytes = include_bytes!("../../../assets/icon.png");
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
        "image-view-iced {}
用法:
    image-view-iced [PATH]

参数:
    PATH    要打开的图片文件或目录（目录则打开其中第一张图片）

选项:
    -h, --help       显示帮助
    -V, --version    显示版本",
        env!("CARGO_PKG_VERSION")
    );
}
