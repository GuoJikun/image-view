//! 菜单模型：分组、条目、勾选状态与平台快捷键。
//!
//! 菜单内容与 `main` 分支的 `MenuBar.vue` / 原生菜单一一对应（同样的分组与顺序），
//! 只是绘制方式不同：Iced 0.14 没有系统菜单 API，本分支统一在窗口内自绘。
//! 视图层（[`crate::view::menu`]）只负责把这里的数据映射成 widget。

use iv_domain::{Language, SortOrder, ThemeSetting};

use crate::state::AppState;

/// 菜单动作。与界面文案无关，视图与 Update 共用同一套 id。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Open,
    Reveal,
    CopyPath,
    Trash,
    ToggleThumbnails,
    ToggleSlideshow,
    Sort(SortOrder),
    ZoomIn,
    ZoomOut,
    Fit,
    ActualSize,
    RotateCw,
    RotateCcw,
    FlipH,
    FlipV,
    Theme(ThemeSetting),
    Language(Language),
    ToggleColorBadge,
    ToggleMetadata,
    ShowShortcuts,
    OpenLogs,
}

/// 勾选样式：`Check` 用于开关，`Dot` 用于单选。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mark {
    #[default]
    None,
    Check,
    Dot,
}

/// 一个可点击条目。
#[derive(Clone, Debug)]
pub struct Item {
    pub action: Action,
    pub label: &'static str,
    pub shortcut: Option<String>,
    pub mark: Mark,
    pub checked: bool,
    pub danger: bool,
    pub enabled: bool,
}

/// 面板里的一行：条目、分隔线或子菜单。
#[derive(Clone, Debug)]
pub enum Entry {
    Item(Item),
    Separator,
    Sub {
        label: &'static str,
        items: Vec<Entry>,
    },
}

#[derive(Clone, Debug)]
pub struct Group {
    pub label: &'static str,
    pub entries: Vec<Entry>,
}

/// 「显示」分组的下标，供工具栏的排序按钮直接定位菜单。
#[cfg(test)]
pub const GROUP_VIEW: usize = 1;

/// 「显示」菜单里「排序」子菜单所在的行号（供菜单结构测试断言）。
#[cfg(test)]
pub const SORT_ENTRY: usize = 3;

/// 菜单的展开状态。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuState {
    /// 展开的分组下标。
    pub open: Option<usize>,
    /// 展开的子菜单所在行号。
    pub sub: Option<usize>,
    /// 鼠标停留的行，用于整行强调色高亮。
    pub hover: Option<Hover>,
    /// 鼠标停留在哪个菜单栏标题上（标题保持箭头光标，只做底色反馈）。
    pub bar_hover: Option<usize>,
    /// 菜单是靠悬停切换过来的：此时点击当前分组不应把它关掉
    /// （与 `main` 的 `MenuBar.vue`、macOS 菜单栏一致）。
    pub hover_opened: bool,
    /// 工具栏排序按钮下方的下拉面板是否展开（与菜单栏互斥）。
    pub sort_open: bool,
}

/// 鼠标停留的行：`child` 为子菜单内的行号。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hover {
    pub entry: usize,
    pub child: Option<usize>,
}

impl MenuState {
    pub fn close(&mut self) {
        self.open = None;
        self.sub = None;
        self.hover = None;
        self.hover_opened = false;
        self.sort_open = false;
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }
}

/// 按当前状态构建整份菜单。
/// 排序选项：「显示 › 排序」子菜单与工具栏下拉面板共用同一份条目。
pub fn sort_items(state: &AppState) -> Vec<Item> {
    let text = state.text();
    vec![
        Item {
            action: Action::Sort(SortOrder::Name),
            label: text.menu_sort_name,
            shortcut: None,
            mark: Mark::Dot,
            checked: state.nav.sort == SortOrder::Name,
            danger: false,
            enabled: true,
        },
        Item {
            action: Action::Sort(SortOrder::ModifiedNewest),
            label: text.menu_sort_modified,
            shortcut: None,
            mark: Mark::Dot,
            checked: state.nav.sort == SortOrder::ModifiedNewest,
            danger: false,
            enabled: true,
        },
    ]
}

pub fn groups(state: &AppState) -> Vec<Group> {
    let text = state.text();
    let has_image = state.current_path().is_some();
    let theme = state.settings.theme;
    let language = state.settings.language;

    let file = Group {
        label: text.menu_file,
        entries: vec![
            Entry::Item(item(
                Action::Open,
                text.menu_open,
                Some(accel(&["mod", "O"])),
                Mark::None,
                false,
                true,
            )),
            Entry::Separator,
            Entry::Item(item(
                Action::Reveal,
                text.menu_reveal,
                None,
                Mark::None,
                false,
                has_image,
            )),
            Entry::Item(item(
                Action::CopyPath,
                text.menu_copy_path,
                Some(accel(&["mod", "shift", "C"])),
                Mark::None,
                false,
                has_image,
            )),
            Entry::Separator,
            Entry::Item(Item {
                action: Action::Trash,
                label: text.menu_trash,
                shortcut: Some(delete_key()),
                mark: Mark::None,
                checked: false,
                danger: true,
                enabled: has_image,
            }),
        ],
    };

    let view = Group {
        label: text.menu_view,
        entries: vec![
            Entry::Item(Item {
                action: Action::ToggleThumbnails,
                label: text.menu_thumbnails,
                shortcut: Some(accel(&["mod", "T"])),
                mark: Mark::Check,
                checked: state.library.thumbs.show,
                danger: false,
                enabled: true,
            }),
            Entry::Item(Item {
                action: Action::ToggleSlideshow,
                label: text.menu_slideshow,
                shortcut: Some("Space".to_owned()),
                mark: Mark::Check,
                checked: state.slideshow,
                danger: false,
                enabled: has_image,
            }),
            Entry::Separator,
            Entry::Sub {
                label: text.menu_sort,
                items: sort_items(state).into_iter().map(Entry::Item).collect(),
            },
            Entry::Separator,
            Entry::Item(item_disabled(
                Action::ZoomIn,
                text.menu_zoom_in,
                Some(accel(&["mod", "="])),
                has_image,
            )),
            Entry::Item(item_disabled(
                Action::ZoomOut,
                text.menu_zoom_out,
                Some(accel(&["mod", "-"])),
                has_image,
            )),
            Entry::Item(item_disabled(
                Action::Fit,
                text.menu_fit,
                Some(accel(&["mod", "0"])),
                has_image,
            )),
            Entry::Item(item_disabled(
                Action::ActualSize,
                text.menu_actual,
                Some(accel(&["mod", "1"])),
                has_image,
            )),
            Entry::Separator,
            Entry::Item(item_disabled(
                Action::RotateCw,
                text.menu_rotate_cw,
                Some("R".to_owned()),
                has_image,
            )),
            Entry::Item(item_disabled(
                Action::RotateCcw,
                text.menu_rotate_ccw,
                Some(accel(&["shift", "R"])),
                has_image,
            )),
            Entry::Item(item_disabled(
                Action::FlipH,
                text.menu_flip_h,
                Some("H".to_owned()),
                has_image,
            )),
            Entry::Item(item_disabled(
                Action::FlipV,
                text.menu_flip_v,
                Some("V".to_owned()),
                has_image,
            )),
            Entry::Separator,
            Entry::Sub {
                label: text.menu_theme,
                items: vec![
                    theme_entry(ThemeSetting::System, text.menu_theme_system, theme),
                    theme_entry(ThemeSetting::Light, text.menu_theme_light, theme),
                    theme_entry(ThemeSetting::Dark, text.menu_theme_dark, theme),
                ],
            },
            Entry::Sub {
                label: text.menu_language,
                items: vec![
                    language_entry(Language::System, text.menu_lang_system, language),
                    language_entry(Language::Zh, text.menu_lang_zh, language),
                    language_entry(Language::En, text.menu_lang_en, language),
                ],
            },
            Entry::Separator,
            Entry::Item(Item {
                action: Action::ToggleColorBadge,
                label: text.menu_color_badge,
                shortcut: None,
                mark: Mark::Check,
                checked: state.settings.show_color_status,
                danger: false,
                enabled: true,
            }),
            Entry::Item(Item {
                action: Action::ToggleMetadata,
                label: text.menu_metadata,
                shortcut: None,
                mark: Mark::Check,
                checked: state.library.meta.show,
                danger: false,
                enabled: true,
            }),
        ],
    };

    let help = Group {
        label: text.menu_help,
        entries: vec![
            Entry::Item(item(
                Action::ShowShortcuts,
                text.menu_shortcuts,
                Some(accel(&["mod", "/"])),
                Mark::None,
                false,
                true,
            )),
            Entry::Separator,
            Entry::Item(item(
                Action::OpenLogs,
                text.menu_logs,
                None,
                Mark::None,
                false,
                true,
            )),
        ],
    };

    vec![file, view, help]
}

fn item(
    action: Action,
    label: &'static str,
    shortcut: Option<String>,
    mark: Mark,
    danger: bool,
    enabled: bool,
) -> Item {
    Item {
        action,
        label,
        shortcut,
        mark,
        checked: false,
        danger,
        enabled,
    }
}

fn item_disabled(
    action: Action,
    label: &'static str,
    shortcut: Option<String>,
    enabled: bool,
) -> Item {
    item(action, label, shortcut, Mark::None, false, enabled)
}

fn theme_entry(setting: ThemeSetting, label: &'static str, current: ThemeSetting) -> Entry {
    Entry::Item(Item {
        action: Action::Theme(setting),
        label,
        shortcut: None,
        mark: Mark::Dot,
        checked: current == setting,
        danger: false,
        enabled: true,
    })
}

fn language_entry(setting: Language, label: &'static str, current: Language) -> Entry {
    Entry::Item(Item {
        action: Action::Language(setting),
        label,
        shortcut: None,
        mark: Mark::Dot,
        checked: current == setting,
        danger: false,
        enabled: true,
    })
}

/// 平台修饰键：macOS 用 `⌘ ⇧ ⌥` 符号，其它平台拼 `Ctrl / Shift / Alt`。
pub fn accel(parts: &[&str]) -> String {
    if cfg!(target_os = "macos") {
        parts
            .iter()
            .map(|part| match *part {
                "mod" => "⌘",
                "shift" => "⇧",
                "alt" => "⌥",
                other => other,
            })
            .collect::<Vec<_>>()
            .concat()
    } else {
        parts
            .iter()
            .map(|part| match *part {
                "mod" => "Ctrl",
                "shift" => "Shift",
                "alt" => "Alt",
                other => other,
            })
            .collect::<Vec<_>>()
            .join("+")
    }
}

pub fn delete_key() -> String {
    if cfg!(target_os = "macos") {
        "⌫".to_owned()
    } else {
        "Delete".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Message;

    fn state() -> AppState {
        AppState::for_test(
            std::env::temp_dir().join(format!("image-view-i18n-menu-{}", std::process::id())),
        )
    }

    /// 三组菜单与规范一致，勾选值反映当前设置。
    #[test]
    fn groups_follow_the_designed_structure() {
        let state = state();
        let groups = groups(&state);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].label, "文件");
        assert_eq!(groups[GROUP_VIEW].label, "显示");
        assert_eq!(groups[2].label, "帮助");

        let view = &groups[GROUP_VIEW].entries;
        let Entry::Item(thumbnails) = &view[0] else {
            panic!("第一项应为缩略图条开关");
        };
        assert!(thumbnails.checked, "默认显示缩略图条");
        let expected = accel(&["mod", "T"]);
        assert_eq!(thumbnails.shortcut.as_deref(), Some(expected.as_str()));

        let Entry::Sub { label, items } = &view[SORT_ENTRY] else {
            panic!("排序应为子菜单");
        };
        assert_eq!(*label, "排序");
        assert_eq!(items.len(), 2);

        // 旋转 / 翻转四项与 `main` 的菜单一致，顺序为顺时针、逆时针、水平、垂直。
        let rotation: Vec<_> = view[10..14]
            .iter()
            .map(|entry| match entry {
                Entry::Item(item) => (item.action, item.shortcut.as_deref()),
                _ => panic!("旋转 / 翻转应为条目"),
            })
            .collect();
        assert_eq!(
            rotation,
            vec![
                (Action::RotateCw, Some("R")),
                (Action::RotateCcw, Some(accel(&["shift", "R"]).as_str())),
                (Action::FlipH, Some("H")),
                (Action::FlipV, Some("V")),
            ]
        );
    }

    /// 没有图片时，依赖当前文件的条目应禁用。
    #[test]
    fn image_actions_are_disabled_without_an_image() {
        let state = state();
        let groups = groups(&state);
        let Entry::Item(reveal) = &groups[0].entries[2] else {
            panic!("第二个文件菜单项应为定位");
        };
        assert!(!reveal.enabled, "无图片时不能定位");

        let Entry::Item(zoom) = &groups[GROUP_VIEW].entries[5] else {
            panic!("缩放项应在排序之后");
        };
        assert!(!zoom.enabled, "无图片时不能缩放");
    }

    /// 主题 / 语言的单选标记跟随设置。
    #[test]
    fn theme_and_language_reflect_settings() {
        let mut state = state();
        state.settings.theme = ThemeSetting::Light;
        state.settings.language = Language::En;

        let built = groups(&state);
        let view = &built[GROUP_VIEW].entries;
        let Entry::Sub { items, .. } = &view[15] else {
            panic!("主题应为子菜单");
        };
        let Entry::Item(light) = &items[1] else {
            panic!("主题第二项应为浅色");
        };
        assert!(light.checked);

        let Entry::Sub { items, .. } = &view[16] else {
            panic!("语言应为子菜单");
        };
        let Entry::Item(english) = &items[2] else {
            panic!("语言第三项应为 English");
        };
        assert!(english.checked);

        // 切到英文后菜单文案随之变化。
        assert_eq!(groups(&state)[0].label, "File");
    }

    /// 悬停切换分组后，点击当前分组只「落定」，第二次点击才收起。
    #[test]
    fn clicking_a_hover_opened_group_does_not_close_it() {
        let mut state = state();
        let _ = crate::update::update(&mut state, Message::MenuGroupClicked(0));
        assert_eq!(state.menu.open, Some(0));

        let _ = crate::update::update(&mut state, Message::MenuGroupHovered(GROUP_VIEW));
        assert_eq!(state.menu.open, Some(GROUP_VIEW));

        let _ = crate::update::update(&mut state, Message::MenuGroupClicked(GROUP_VIEW));
        assert_eq!(state.menu.open, Some(GROUP_VIEW), "第一次点击不应收起");

        let _ = crate::update::update(&mut state, Message::MenuGroupClicked(GROUP_VIEW));
        assert!(!state.menu.is_open(), "第二次点击收起");
    }

    /// 工具栏排序下拉与菜单栏互斥，且点空白 / Esc 能收起；条目勾选跟随当前排序。
    #[test]
    fn sort_dropdown_is_exclusive_with_menu_bar() {
        let mut state = state();
        assert!(sort_items(&state)[0].checked, "默认按名称");

        let _ = crate::update::update(&mut state, Message::MenuGroupClicked(0));
        assert_eq!(state.menu.open, Some(0));

        let _ = crate::update::update(&mut state, Message::SortMenuToggled);
        assert!(state.menu.sort_open);
        assert!(!state.menu.is_open(), "展开下拉时收起菜单栏");

        let _ = crate::update::update(&mut state, Message::MenuGroupClicked(0));
        assert_eq!(state.menu.open, Some(0));
        assert!(!state.menu.sort_open, "展开菜单栏时收起下拉");

        let _ = crate::update::update(&mut state, Message::SortMenuToggled);
        assert!(state.menu.sort_open);
        let _ = crate::update::update(&mut state, Message::MenuClose);
        assert!(!state.menu.sort_open, "点空白 / Esc 关闭下拉");
    }

    /// 标题悬停只记录高亮，不展开菜单；离开后清掉自己那一项。
    #[test]
    fn hovering_a_title_only_tracks_highlight() {
        let mut state = state();

        let _ = crate::update::update(&mut state, Message::MenuGroupHovered(0));
        assert_eq!(state.menu.bar_hover, Some(0));
        assert!(!state.menu.is_open(), "菜单未展开时悬停不应弹出");

        let _ = crate::update::update(&mut state, Message::MenuGroupHovered(1));
        let _ = crate::update::update(&mut state, Message::MenuGroupLeft(0));
        assert_eq!(state.menu.bar_hover, Some(1), "离开旧标题不应清掉新标题");

        let _ = crate::update::update(&mut state, Message::MenuGroupLeft(1));
        assert_eq!(state.menu.bar_hover, None);
    }

    #[test]
    fn accelerators_use_platform_symbols() {
        #[cfg(target_os = "macos")]
        assert_eq!(accel(&["mod", "shift", "C"]), "⌘⇧C");
        #[cfg(not(target_os = "macos"))]
        assert_eq!(accel(&["mod", "shift", "C"]), "Ctrl+Shift+C");
    }
}
