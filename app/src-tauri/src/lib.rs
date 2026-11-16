//! Tauri 2 GUI adapter 入口：只做装配，不承载业务逻辑。
//!
//! 启动顺序与 `docs/03-tauri-vue-rust-architecture.md` §7 一致：
//! 日志 → 状态装配 → 注册 Commands/Events/Protocol → 单实例与文件打开事件。

mod commands;
mod dto;
mod menu_text;
mod preview;
mod protocol;
mod settings_store;
mod state;
mod thumbnails;

use std::path::PathBuf;

use tauri::{Emitter, Manager};

use crate::menu_text::MenuText;
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

/// macOS 系统菜单栏。Windows / Linux 使用前端自绘菜单，不调用本函数。
///
/// 只在 macOS 上安装；仍然在其它平台编译，以便 CI 校验 API 用法。
/// 文案来自前端文案表（见 [`menu_text`]），这里只写键名。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn build_app_menu(
    app: &tauri::AppHandle,
    locale: &str,
) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};

    let t = MenuText::new(locale);

    let about = SubmenuBuilder::new(app, t.t("app.name"))
        .about(None)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let file = SubmenuBuilder::new(app, t.t("menu.file"))
        .item(
            &MenuItemBuilder::with_id("open", t.t("menu.open"))
                .accelerator("CmdOrCtrl+O")
                .build(app)?,
        )
        .separator()
        .item(&MenuItemBuilder::with_id("reveal", t.t("menu.reveal")).build(app)?)
        .item(
            &MenuItemBuilder::with_id("copy-path", t.t("menu.copyPath"))
                .accelerator("CmdOrCtrl+Shift+C")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("trash", t.t("menu.trash"))
                .accelerator("Backspace")
                .build(app)?,
        )
        .build()?;

    let sort = SubmenuBuilder::new(app, t.t("menu.sort"))
        .text("sort-name", t.t("menu.sortName"))
        .text("sort-modified", t.t("menu.sortModified"))
        .build()?;

    let theme = SubmenuBuilder::new(app, t.t("menu.theme"))
        .text("theme-system", t.t("menu.themeSystem"))
        .text("theme-light", t.t("menu.themeLight"))
        .text("theme-dark", t.t("menu.themeDark"))
        .build()?;

    let language = SubmenuBuilder::new(app, t.t("menu.language"))
        .text("lang-system", t.t("menu.langSystem"))
        .text("lang-zh", t.t("menu.langZh"))
        .text("lang-en", t.t("menu.langEn"))
        .build()?;

    let display = SubmenuBuilder::new(app, t.t("menu.view"))
        .item(
            &MenuItemBuilder::with_id("toggle-thumbnails", t.t("menu.thumbnails"))
                .accelerator("CmdOrCtrl+T")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("slideshow", t.t("menu.slideshow"))
                .accelerator("Space")
                .build(app)?,
        )
        .separator()
        .item(&sort)
        .separator()
        .item(
            &MenuItemBuilder::with_id("zoom-in", t.t("menu.zoomIn"))
                .accelerator("CmdOrCtrl+=")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("zoom-out", t.t("menu.zoomOut"))
                .accelerator("CmdOrCtrl+-")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("fit", t.t("menu.fit"))
                .accelerator("CmdOrCtrl+0")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("actual", t.t("menu.actual"))
                .accelerator("CmdOrCtrl+1")
                .build(app)?,
        )
        .separator()
        .item(&theme)
        .item(&language)
        .separator()
        .item(&MenuItemBuilder::with_id("color-badge", t.t("menu.colorBadge")).build(app)?)
        .item(&MenuItemBuilder::with_id("metadata", t.t("menu.metadata")).build(app)?)
        .build()?;

    let help = SubmenuBuilder::new(app, t.t("menu.help"))
        .item(
            &MenuItemBuilder::with_id("shortcuts", t.t("menu.shortcuts"))
                .accelerator("CmdOrCtrl+/")
                .build(app)?,
        )
        .item(&MenuItemBuilder::with_id("update", t.t("menu.update")).build(app)?)
        .item(&MenuItemBuilder::with_id("auto-update", t.t("menu.autoUpdate")).build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("logs", t.t("menu.logs")).build(app)?)
        .build()?;

    MenuBuilder::new(app)
        .items(&[&about, &file, &display, &help])
        .build()
}

/// macOS：按界面语言重建原生菜单；其它平台无系统菜单，空实现。
#[tauri::command]
fn sync_native_menu(app: tauri::AppHandle, locale: String) {
    #[cfg(target_os = "macos")]
    {
        if let Ok(menu) = build_app_menu(&app, &locale) {
            let _ = app.set_menu(menu);
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, locale);
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // 窗口大小/位置/最大化由插件保管；主窗口 `visible: false` 等插件恢复后再显示。
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .register_asynchronous_uri_scheme_protocol("iviewer", protocol::handle)
        .on_window_event(|window, event| {
            // 退出前同步落盘会话/偏好；防抖线程可能来不及写最后一次修改。
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                window.app_handle().state::<AppState>().flush_settings();
            }
        })
        .setup(move |app| {
            app.manage(AppState::new(app.handle(), startup_path.clone()));
            app.manage(commands::update::UpdateState::new());

            // 原生菜单在 setup 阶段就要建好，此时前端还没跑起来，
            // 语言只能由后端按设置 + 系统语言解析（与前端 `resolvedLocale` 同一规则）。
            let locale = app.state::<AppState>().locale_tag();
            log::debug!(target: "image_view", "界面语言：{locale}");

            if let Some(window) = app.get_webview_window("main") {
                // macOS 保留系统原生标题栏与红绿灯；Win/Linux 使用 28 px 自绘标题栏。
                #[cfg(not(target_os = "macos"))]
                let _ = window.set_decorations(false);

                #[cfg(target_os = "macos")]
                {
                    let _ = window;
                    if let Ok(menu) = build_app_menu(app.handle(), locale) {
                        let _ = app.handle().set_menu(menu);
                    }
                }
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
            commands::image::ui_preferences,
            commands::image::set_show_thumbnails,
            commands::image::close_image,
            commands::image::delete_image,
            commands::file::reveal_image,
            commands::file::open_log_dir,
            commands::task::cancel_preview,
            commands::update::check_update,
            commands::update::download_update,
            commands::update::cancel_update_download,
            commands::update::install_update,
            sync_native_menu,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}
