//! Tauri 2 GUI adapter 入口：只做装配，不承载业务逻辑。
//!
//! 启动顺序与 `docs/03-tauri-vue-rust-architecture.md` §7 一致：
//! 日志 → 状态装配 → 注册 Commands/Events/Protocol → 单实例与文件打开事件。

mod commands;
mod dto;
mod menu;
mod menu_text;
mod preview;
mod protocol;
mod settings_store;
mod state;
mod thumbnails;
mod window_state;

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

/// macOS 系统菜单栏的构建与勾选同步见 [`menu`]；Windows / Linux 使用前端自绘菜单。
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .register_asynchronous_uri_scheme_protocol("iviewer", protocol::handle)
        .on_window_event(|window, event| {
            // 退出前同步落盘会话/偏好；防抖线程可能来不及写最后一次修改。
            // 窗口事件可能早于 setup 的 manage 触发，必须用 try_state 避免启动期 panic。
            if matches!(event, tauri::WindowEvent::CloseRequested { .. })
                && let Some(state) = window.app_handle().try_state::<AppState>()
            {
                state.flush_settings();
            }
            if let Some(store) = window
                .app_handle()
                .try_state::<window_state::WindowStateStore>()
            {
                store.on_window_event(window, event);
            }
        })
        .setup(move |app| {
            app.manage(AppState::new(app.handle(), startup_path.clone()));
            app.manage(commands::update::UpdateState::new());
            app.manage(menu::NativeMenu::new());
            let settings = app.state::<AppState>().settings_store();
            app.manage(window_state::WindowStateStore::new(settings));

            // 原生菜单在 setup 阶段就要建好，此时前端还没跑起来：
            // 语言与勾选状态只能由后端按设置 + 系统语言解析（与前端同一规则）。
            let locale = app.state::<AppState>().locale_tag();
            log::debug!(target: "image_view", "界面语言：{locale}");

            #[cfg(target_os = "macos")]
            {
                let state = app.state::<AppState>().menu_state();
                menu::install(app.handle(), locale, &state);
            }

            if let Some(window) = app.get_webview_window("main") {
                // macOS 保留系统原生标题栏与红绿灯；Win/Linux 使用 28 px 自绘标题栏。
                // 必须先改装饰再恢复几何，否则无边框窗口的 inner size 会偏移。
                #[cfg(not(target_os = "macos"))]
                let _ = window.set_decorations(false);

                // 恢复保存的大小/位置/最大化；主窗口 `visible: false`，由这里显示，
                // 避免启动时先出现在默认位置再跳动。
                app.state::<window_state::WindowStateStore>()
                    .restore(&window);
            }

            log::info!(
                target: "image_view",
                "Tauri GUI 已启动；启动参数：{}",
                startup_path.as_deref().unwrap_or("<无>")
            );
            Ok(())
        })
        .on_menu_event(|app, event| {
            // macOS 原生菜单 → 前端统一动作分发（与自绘菜单同一套 id）。
            let _ = app.emit("menu:action", event.id().0.as_str());
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
            commands::image::set_color_target,
            commands::image::ui_preferences,
            commands::image::set_ui_preferences,
            commands::image::close_image,
            commands::image::delete_image,
            commands::gallery::open_gallery,
            commands::gallery::refresh_gallery,
            commands::gallery::gallery_page,
            commands::gallery::set_gallery_filter,
            commands::gallery::add_gallery_tag,
            commands::gallery::remove_gallery_tag,
            commands::gallery::enter_compare,
            commands::gallery::set_compare_layout,
            commands::gallery::exit_compare,
            commands::file::reveal_image,
            commands::file::open_log_dir,
            commands::task::cancel_preview,
            commands::update::check_update,
            commands::update::download_update,
            commands::update::cancel_update_download,
            commands::update::install_update,
            menu::sync_native_menu,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}
