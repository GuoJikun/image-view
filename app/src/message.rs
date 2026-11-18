//! Iced 消息：只描述 UI 事件和异步结果，不进入 Domain。
//!
//! Update 层把消息转换为 `iv-application` 用例或视图变换；Core 从不依赖
//! 这里定义的类型。

use std::path::PathBuf;

use crate::menu::{Action, Hover};
use emath::Vec2;

/// 自绘标题栏的 caption 按钮；悬停时要单独着色，因此需要能被状态记住。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caption {
    Minimize,
    Maximize,
    Close,
}

/// 关窗前异步查询的一项窗口状态。
///
/// Iced 0.14 没有「最大化 / 最小化」事件，只能在关闭时查询一次，
/// 与位置、尺寸、全屏一起凑齐后再落盘，避免多次写 `settings.ron`。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CloseQueryPart {
    Maximized(bool),
    Minimized(bool),
    Fullscreen(bool),
    Size(iced::Size),
    Position(Option<iced::Point>),
}

#[derive(Debug, Clone)]
pub enum Message {
    /// 全局事件（键盘、窗口、拖放）；widget 已捕获的事件不会到达这里。
    Event(iced::Event),
    WindowReady(iced::window::Id),
    /// 窗口 DPI；恢复保存的几何与采集位置都需要它做逻辑/物理像素换算。
    WindowScale(f32),

    OpenDialog,
    DialogPicked(Option<PathBuf>),

    Navigate(isize),
    Select(usize),

    ZoomAt {
        /// 相对画布中心的逻辑点偏移。
        cursor: Vec2,
        factor: f32,
    },
    ZoomBy(f32),
    PanBy(Vec2),
    Fit,
    ToggleActualSize,

    CanvasResized(iced::Size),
    ThumbScrolled {
        offset_x: f32,
        viewport_w: f32,
    },

    ToggleThumbnails,
    ToggleSlideshow,

    // ---- 菜单栏 ----
    /// 点击分组：展开 / 收起。
    MenuGroupClicked(usize),
    /// 鼠标移到另一个分组上：菜单展开时切换分组。
    MenuGroupHovered(usize),
    /// 鼠标离开分组标题。
    MenuGroupLeft(usize),
    /// 直接展开某个分组（工具栏的排序按钮用）。
    /// 展开 / 收起子菜单。
    MenuSubmenuToggled(usize),
    /// 进入某一行：设置悬停高亮。
    MenuRowHovered(Hover),
    /// 离开某一行：只清掉自己那一行的高亮，避免把刚进入的新行清掉。
    MenuRowLeft(Hover),
    MenuClose,
    MenuAction(Action),
    /// 工具栏排序按钮：展开 / 收起按钮下方的下拉面板。
    SortMenuToggled,

    // ---- 弹窗 ----
    CloseDialog,
    /// 无副作用的占位消息：用于「弹窗内点击不穿透到遮罩」。
    Noop,

    Reveal,
    CopyPath,
    ApplyWindowMode(iced::window::Mode),

    /// 自绘标题栏（Win/Linux）：拖动 / 双击最大化 / caption 按钮。
    DragWindow,
    ToggleMaximize,
    MinimizeWindow,
    CloseWindow,
    /// 关窗查询的一项结果；全部到齐后统一采集几何并落盘。
    CloseQuery(CloseQueryPart),
    /// 进入 caption 按钮：关闭按钮的图标据此转白。
    CaptionHovered(Caption),
    /// 离开 caption 按钮：只清掉自己那一颗，避免误清刚进入的按钮。
    CaptionLeft(Caption),

    /// 定时轮询解码线程与缩略图线程的结果。
    Poll,
    Dismiss,
}
