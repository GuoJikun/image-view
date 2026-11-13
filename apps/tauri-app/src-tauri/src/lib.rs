//! Tauri 2 GUI adapter 入口：只做装配，不承载业务逻辑。
//!
//! 启动顺序与 `docs/03-tauri-vue-rust-architecture.md` §7 一致：
//! 日志 → 状态装配 → 注册 Commands/Events/Protocol → 单实例与文件打开事件。

mod commands;
mod dto;
mod preview;
mod protocol;
mod state;
mod thumbnails;

use std::path::PathBuf;

use tauri::{Emitter, Manager};

use crate::state::AppState;

/// 命令行中第一个存在的路径参数（文件或目录）。
fn startup_path_from_env() -> Option<String> {
    std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .find(|path| !path.to_string_lossy().starts_with('-') && path.exists())
        .map(|path| path.to_string_lossy().into_owned())
}

/// 单实例转发过来的参数同样只接受真实存在的路径。
fn forwarded_path(argv: Vec<String>) -> Option<String> {
    argv.into_iter()
        .skip(1)
        .map(PathBuf::from)
        .find(|path| !path.to_string_lossy().starts_with('-') && path.exists())
        .map(|path| path.to_string_lossy().into_owned())
}

pub fn run() {
    iv_infra::logging::init();
    iv_infra::logging::install_panic_hook();

    let startup_path = startup_path_from_env();

    tauri::Builder::default()
        // 必须最先注册：第二个实例把参数转交给已有窗口。
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(path) = forwarded_path(argv) {
                let _ = app.emit("app:open-file", path);
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .register_asynchronous_uri_scheme_protocol("iviewer", protocol::handle)
        .setup(move |app| {
            app.manage(AppState::new(app.handle(), startup_path.clone()));
            log::info!(
                target: "image_view_tauri",
                "Tauri GUI 已启动；启动参数：{}",
                startup_path.as_deref().unwrap_or("<无>")
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::image::open_path,
            commands::image::pick_image,
            commands::image::navigate,
            commands::image::select_image,
            commands::image::current_snapshot,
            commands::image::request_preview,
            commands::image::set_sort,
            commands::image::startup_path,
            commands::image::close_image,
            commands::file::reveal_image,
            commands::task::cancel_preview,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}
