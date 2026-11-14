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

/// 原生菜单文案：`t(中文, English)`；英文文案与前端 `i18n.ts` 保持一致。
#[derive(Clone, Copy)]
struct MenuText {
    en: bool,
}

impl MenuText {
    fn t(self, zh: &'static str, english: &'static str) -> &'static str {
        if self.en { english } else { zh }
    }
}

/// macOS 系统菜单栏。Windows / Linux 使用前端自绘菜单，不调用本函数。
///
/// 只在 macOS 上安装；仍然在其它平台编译，以便 CI 校验 API 用法。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn build_app_menu(
    app: &tauri::AppHandle,
    en: bool,
) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};

    let t = MenuText { en };

    let about = SubmenuBuilder::new(app, t.t("图片查看器", "Image Viewer"))
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

    let file = SubmenuBuilder::new(app, t.t("文件", "File"))
        .item(
            &MenuItemBuilder::with_id("open", t.t("打开图片…", "Open Image…"))
                .accelerator("CmdOrCtrl+O")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id(
                "reveal",
                t.t("在文件管理器中显示", "Reveal in File Manager"),
            )
            .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("copy-path", t.t("复制路径", "Copy Path"))
                .accelerator("CmdOrCtrl+Shift+C")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("trash", t.t("移到废纸篓", "Move to Trash"))
                .accelerator("Backspace")
                .build(app)?,
        )
        .build()?;

    let sort = SubmenuBuilder::new(app, t.t("排序", "Sort"))
        .text("sort-name", t.t("按名称", "By Name"))
        .text("sort-modified", t.t("按修改时间", "By Date Modified"))
        .build()?;

    let theme = SubmenuBuilder::new(app, t.t("主题", "Theme"))
        .text("theme-system", t.t("跟随系统", "System"))
        .text("theme-light", t.t("浅色", "Light"))
        .text("theme-dark", t.t("深色", "Dark"))
        .build()?;

    let language = SubmenuBuilder::new(app, t.t("语言", "Language"))
        .text("lang-system", t.t("跟随系统", "System"))
        .text("lang-zh", "简体中文")
        .text("lang-en", "English")
        .build()?;

    let display = SubmenuBuilder::new(app, t.t("显示", "View"))
        .item(
            &MenuItemBuilder::with_id("toggle-thumbnails", t.t("缩略图条", "Thumbnails"))
                .accelerator("CmdOrCtrl+T")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("slideshow", t.t("幻灯片", "Slideshow"))
                .accelerator("Space")
                .build(app)?,
        )
        .separator()
        .item(&sort)
        .separator()
        .item(
            &MenuItemBuilder::with_id("zoom-in", t.t("放大", "Zoom In"))
                .accelerator("CmdOrCtrl+=")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("zoom-out", t.t("缩小", "Zoom Out"))
                .accelerator("CmdOrCtrl+-")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("fit", t.t("适配窗口", "Fit to Window"))
                .accelerator("CmdOrCtrl+0")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("actual", t.t("实际大小", "Actual Size"))
                .accelerator("CmdOrCtrl+1")
                .build(app)?,
        )
        .separator()
        .item(&theme)
        .item(&language)
        .separator()
        .item(
            &MenuItemBuilder::with_id(
                "color-badge",
                t.t("显示色彩转换标记", "Show Color Conversion Badge"),
            )
            .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("metadata", t.t("元数据面板", "Metadata Panel"))
                .build(app)?,
        )
        .build()?;

    let help = SubmenuBuilder::new(app, t.t("帮助", "Help"))
        .item(
            &MenuItemBuilder::with_id("shortcuts", t.t("快捷键…", "Keyboard Shortcuts…"))
                .accelerator("CmdOrCtrl+/")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("update", t.t("检查更新…", "Check for Updates…"))
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id(
                "auto-update",
                t.t("启动时检查更新", "Check for Updates at Launch"),
            )
            .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("logs", t.t("打开日志文件夹", "Open Logs Folder"))
                .build(app)?,
        )
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
        let en = locale.starts_with("en");
        if let Ok(menu) = build_app_menu(&app, en) {
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
        .register_asynchronous_uri_scheme_protocol("iviewer", protocol::handle)
        .setup(move |app| {
            app.manage(AppState::new(app.handle(), startup_path.clone()));
            app.manage(commands::update::UpdateState::new());

            if let Some(window) = app.get_webview_window("main") {
                // macOS 保留系统原生标题栏与红绿灯；Win/Linux 使用 28 px 自绘标题栏。
                #[cfg(not(target_os = "macos"))]
                let _ = window.set_decorations(false);

                #[cfg(target_os = "macos")]
                {
                    let _ = window;
                    if let Ok(menu) = build_app_menu(app.handle(), false) {
                        let _ = app.handle().set_menu(menu);
                    }
                }
            }

            log::info!(
                target: "image_view_tauri",
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
