//! macOS 系统菜单栏：构建、可勾选项句柄与勾选状态同步。
//!
//! 文案复用前端文案表（见 [`crate::menu_text`]），勾选值由前端推送
//! （见 [`crate::dto::MenuStateDto`]）。菜单项 id 与前端动作 id 相同，
//! 因此 macOS 原生菜单与 Windows / Linux 自绘菜单共用同一套动作分发。
//!
//! 只有语言变化才重建菜单；其余开关变化直接对已建好的 [`CheckMenuItem`] 调
//! `set_checked`，避免每次勾选都替换整个菜单栏。

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use tauri::menu::{
    CheckMenuItem, CheckMenuItemBuilder, Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder,
};
use tauri::{AppHandle, Manager, Wry};

use crate::dto::MenuStateDto;
use crate::menu_text::MenuText;

/// 动作 id → 可勾选菜单项；id 与 [`MenuStateDto::checks`] 的 id 一致。
type CheckMap = HashMap<&'static str, CheckMenuItem<Wry>>;

/// 一次构建出来的菜单里，语言与可勾选项句柄。
struct BuiltMenu {
    locale: String,
    checks: CheckMap,
}

/// 原生菜单状态。语言不变时 `sync` 只更新勾选，不重建菜单。
#[derive(Default)]
pub struct NativeMenu {
    inner: Mutex<Option<BuiltMenu>>,
}

impl NativeMenu {
    pub fn new() -> Self {
        Self::default()
    }

    /// 把菜单同步到给定语言与勾选状态；失败只记录日志，不影响界面。
    pub fn sync(&self, app: &AppHandle, locale: &str, state: &MenuStateDto) {
        let up_to_date = lock(&self.inner)
            .as_ref()
            .is_some_and(|built| built.locale == locale);

        if !up_to_date {
            log::debug!(target: "image_view::menu", "重建原生菜单：{locale}");
            match build(app, locale) {
                Ok((menu, checks)) => {
                    if let Err(error) = app.set_menu(menu) {
                        log::warn!(target: "image_view::menu", "无法安装原生菜单：{error}");
                        return;
                    }
                    *lock(&self.inner) = Some(BuiltMenu {
                        locale: locale.to_owned(),
                        checks,
                    });
                }
                Err(error) => {
                    log::warn!(target: "image_view::menu", "无法构建原生菜单：{error}");
                    return;
                }
            }
        }

        self.apply(state);
    }

    /// 按状态更新勾选。菜单里没有对应 id 时记警告——新增可勾选项漏改 `build` 会走到这里。
    fn apply(&self, state: &MenuStateDto) {
        let checks: Vec<(&'static str, CheckMenuItem<Wry>)> = match &*lock(&self.inner) {
            Some(built) => built
                .checks
                .iter()
                .map(|(id, item)| (*id, item.clone()))
                .collect(),
            None => return,
        };

        for (id, checked) in state.checks() {
            match checks.iter().find(|(item_id, _)| *item_id == id) {
                Some((_, item)) => {
                    if let Err(error) = item.set_checked(checked) {
                        log::warn!(
                            target: "image_view::menu",
                            "无法更新菜单项 {id} 的勾选状态：{error}"
                        );
                    }
                }
                None => log::warn!(target: "image_view::menu", "原生菜单缺少菜单项：{id}"),
            }
        }
    }
}

/// 建一个可勾选项：勾选值先留空，统一由 [`NativeMenu::apply`] 按当前状态写入，
/// 首次构建与后续状态变化因此走同一条路径。
///
/// `spec` 为（动作 id、文案键、快捷键）。
fn check_item(
    app: &AppHandle,
    text: &MenuText,
    spec: (&'static str, &'static str, Option<&'static str>),
    checks: &mut CheckMap,
) -> tauri::Result<CheckMenuItem<Wry>> {
    let (id, key, accelerator) = spec;
    let mut builder = CheckMenuItemBuilder::with_id(id, text.t(key)).checked(false);
    if let Some(accelerator) = accelerator {
        builder = builder.accelerator(accelerator);
    }
    let item = builder.build(app)?;
    checks.insert(id, item.clone());
    Ok(item)
}

/// 构建整套菜单；可勾选项同时登记句柄，供后续 `set_checked` 使用。
fn build(app: &AppHandle, locale: &str) -> tauri::Result<(Menu<Wry>, CheckMap)> {
    let text = MenuText::new(locale);
    let mut checks = CheckMap::new();

    let about = SubmenuBuilder::new(app, text.t("app.name"))
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

    let file = SubmenuBuilder::new(app, text.t("menu.file"))
        .item(
            &MenuItemBuilder::with_id("open", text.t("menu.open"))
                .accelerator("CmdOrCtrl+O")
                .build(app)?,
        )
        .separator()
        .item(&MenuItemBuilder::with_id("reveal", text.t("menu.reveal")).build(app)?)
        .item(
            &MenuItemBuilder::with_id("copy-path", text.t("menu.copyPath"))
                .accelerator("CmdOrCtrl+Shift+C")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("trash", text.t("menu.trash"))
                .accelerator("Backspace")
                .build(app)?,
        )
        .build()?;

    let thumbnails = check_item(
        app,
        &text,
        ("toggle-thumbnails", "menu.thumbnails", Some("CmdOrCtrl+T")),
        &mut checks,
    )?;
    let slideshow = check_item(
        app,
        &text,
        ("slideshow", "menu.slideshow", Some("Space")),
        &mut checks,
    )?;
    let sort_name = check_item(
        app,
        &text,
        ("sort-name", "menu.sortName", None),
        &mut checks,
    )?;
    let sort_modified = check_item(
        app,
        &text,
        ("sort-modified", "menu.sortModified", None),
        &mut checks,
    )?;
    let theme_system = check_item(
        app,
        &text,
        ("theme-system", "menu.themeSystem", None),
        &mut checks,
    )?;
    let theme_light = check_item(
        app,
        &text,
        ("theme-light", "menu.themeLight", None),
        &mut checks,
    )?;
    let theme_dark = check_item(
        app,
        &text,
        ("theme-dark", "menu.themeDark", None),
        &mut checks,
    )?;
    let lang_system = check_item(
        app,
        &text,
        ("lang-system", "menu.langSystem", None),
        &mut checks,
    )?;
    let lang_zh = check_item(app, &text, ("lang-zh", "menu.langZh", None), &mut checks)?;
    let lang_en = check_item(app, &text, ("lang-en", "menu.langEn", None), &mut checks)?;
    let color_badge = check_item(
        app,
        &text,
        ("color-badge", "menu.colorBadge", None),
        &mut checks,
    )?;
    let metadata = check_item(app, &text, ("metadata", "menu.metadata", None), &mut checks)?;
    let auto_update = check_item(
        app,
        &text,
        ("auto-update", "menu.autoUpdate", None),
        &mut checks,
    )?;

    let sort = SubmenuBuilder::new(app, text.t("menu.sort"))
        .item(&sort_name)
        .item(&sort_modified)
        .build()?;

    let theme = SubmenuBuilder::new(app, text.t("menu.theme"))
        .item(&theme_system)
        .item(&theme_light)
        .item(&theme_dark)
        .build()?;

    let language = SubmenuBuilder::new(app, text.t("menu.language"))
        .item(&lang_system)
        .item(&lang_zh)
        .item(&lang_en)
        .build()?;

    let display = SubmenuBuilder::new(app, text.t("menu.view"))
        .item(
            &MenuItemBuilder::with_id("gallery", text.t("gallery.mode"))
                .accelerator("CmdOrCtrl+G")
                .build(app)?,
        )
        .separator()
        .item(&thumbnails)
        .item(&slideshow)
        .separator()
        .item(&sort)
        .separator()
        .item(
            &MenuItemBuilder::with_id("zoom-in", text.t("menu.zoomIn"))
                .accelerator("CmdOrCtrl+=")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("zoom-out", text.t("menu.zoomOut"))
                .accelerator("CmdOrCtrl+-")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("fit", text.t("menu.fit"))
                .accelerator("CmdOrCtrl+0")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("actual", text.t("menu.actual"))
                .accelerator("CmdOrCtrl+1")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("rotate-cw", text.t("menu.rotateCW"))
                .accelerator("R")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("rotate-ccw", text.t("menu.rotateCCW"))
                .accelerator("Shift+R")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("flip-h", text.t("menu.flipH"))
                .accelerator("H")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("flip-v", text.t("menu.flipV"))
                .accelerator("V")
                .build(app)?,
        )
        .separator()
        .item(&theme)
        .item(&language)
        .separator()
        .item(&color_badge)
        .item(&metadata)
        .build()?;

    let help = SubmenuBuilder::new(app, text.t("menu.help"))
        .item(
            &MenuItemBuilder::with_id("shortcuts", text.t("menu.shortcuts"))
                .accelerator("CmdOrCtrl+/")
                .build(app)?,
        )
        .item(&MenuItemBuilder::with_id("update", text.t("menu.update")).build(app)?)
        .item(&auto_update)
        .separator()
        .item(&MenuItemBuilder::with_id("logs", text.t("menu.logs")).build(app)?)
        .build()?;

    let menu = MenuBuilder::new(app)
        .items(&[&about, &file, &display, &help])
        .build()?;
    Ok((menu, checks))
}

/// 启动时构建首帧菜单：此时前端还没跑起来，只能按持久化设置给出勾选状态。
pub fn install(app: &AppHandle, locale: &str, state: &MenuStateDto) {
    app.state::<NativeMenu>().sync(app, locale, state);
}

/// 前端状态变化（语言 / 勾选项）时同步原生菜单；其它平台无系统菜单，空实现。
#[tauri::command]
pub fn sync_native_menu(app: AppHandle, locale: String, state: MenuStateDto) {
    #[cfg(target_os = "macos")]
    install(&app, &locale, &state);
    // 其它平台没有系统菜单，状态由前端自绘菜单直接渲染；这里只留一条调试日志便于对照。
    #[cfg(not(target_os = "macos"))]
    {
        log::debug!(
            target: "image_view::menu",
            "忽略原生菜单同步：locale={locale}，主题={}，勾选缩略图条={}",
            state.theme,
            state.thumbnails
        );
        let _ = app;
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
