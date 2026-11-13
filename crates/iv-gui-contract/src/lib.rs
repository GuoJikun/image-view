//! GUI 无关的界面契约：工具栏/菜单发出的动作与界面状态 DTO。
//!
//! 这里不依赖 egui；egui 层把控件事件翻译成 [`Action`]，应用层再映射为用例。

use iv_domain::{Language, SortOrder, ThemeSetting};

/// 工具栏与菜单发出的用户动作。
#[derive(Clone, Copy, Debug)]
pub enum Action {
    Open,
    Prev,
    Next,
    Fit,
    ZoomIn,
    ZoomOut,
    ToggleSlideshow,
    ToggleThumbnails,
    Help,
    RevealInExplorer,
    CopyPath,
    Delete,
    OpenLogs,
    CheckUpdate,
    SetSort(SortOrder),
    SetShowColorStatus(bool),
    SetTheme(ThemeSetting),
    SetLanguage(Language),
    SetCheckUpdatesOnStartup(bool),
}

/// 工具栏渲染所需的只读状态。
pub struct ToolbarState {
    pub can_prev: bool,
    pub can_next: bool,
    pub has_image: bool,
    pub slideshow: bool,
    pub show_thumbnails: bool,
    pub sort: SortOrder,
    pub zoom_percent: Option<f32>,
    pub show_color_status: bool,
    pub theme: ThemeSetting,
    pub language: Language,
    pub check_updates_on_startup: bool,
    /// 全屏且系统标题栏隐藏时，在工具栏中央显示的文件名与淡出透明度。
    pub center_title: Option<(String, f32)>,
}
