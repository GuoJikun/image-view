#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cache;
mod color;
mod document;
mod fileops;
mod folder;
mod icons;
mod loader;
mod logging;
mod settings;
mod theme;
mod thumbs;
mod ui;
mod viewer;

use std::ffi::OsStr;
use std::path::PathBuf;

use eframe::egui;

fn main() -> eframe::Result {
    logging::init();
    logging::install_panic_hook();

    let open_path = match std::env::args_os().nth(1) {
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

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("图片查看器")
            .with_app_id("image-view")
            .with_inner_size([1100.0, 760.0])
            .with_min_inner_size([480.0, 320.0])
            .with_drag_and_drop(true)
            .with_icon(load_icon()),
        // 窗口尺寸/位置/最大化由 eframe 持久化；应用状态见 `settings` 模块。
        persist_window: true,
        ..Default::default()
    };

    let result = eframe::run_native(
        "image-view",
        options,
        Box::new(move |cc| Ok(Box::new(app::ImageViewerApp::new(cc, open_path)))),
    );
    if let Err(error) = &result {
        log::error!("图形界面启动失败：{error}");
    }
    result
}

fn load_icon() -> egui::viewport::IconData {
    let bytes = include_bytes!("../assets/icon.png");
    match image::load_from_memory(bytes) {
        Ok(image) => {
            let image = image.to_rgba8();
            let (width, height) = image.dimensions();
            egui::viewport::IconData {
                rgba: image.into_raw(),
                width,
                height,
            }
        }
        Err(error) => {
            log::warn!("无法解码窗口图标：{error}");
            egui::viewport::IconData {
                rgba: vec![255, 255, 255, 255],
                width: 1,
                height: 1,
            }
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
