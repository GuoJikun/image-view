//! Iced 消息：只描述 UI 事件和异步结果，不进入 Domain。
//!
//! Update 层把消息转换为 `iv-application` 用例或视图变换；Core 从不依赖
//! 这里定义的类型。

use std::path::PathBuf;

use emath::Vec2;
use iv_domain::SortOrder;

#[derive(Debug, Clone)]
pub enum Message {
    /// 全局事件（键盘、窗口、拖放）；widget 已捕获的事件不会到达这里。
    Event(iced::Event),
    WindowReady(iced::window::Id),

    OpenDialog,
    DialogPicked(Option<PathBuf>),

    Navigate(isize),
    Select(usize),
    FirstImage,
    LastImage,

    ZoomAt {
        /// 相对画布中心的逻辑点偏移。
        cursor: Vec2,
        factor: f32,
    },
    ZoomBy(f32),
    PanBy(Vec2),
    Fit,
    ActualSize,
    ToggleActualSize,
    Rotate(i8),

    CanvasResized(iced::Size),
    ThumbScrolled {
        offset_x: f32,
        viewport_w: f32,
    },

    ToggleThumbnails,
    ToggleMetadata,
    ToggleMenu,
    ToggleSlideshow,
    SetSort(SortOrder),

    Reveal,
    DeleteCurrent,
    CopyPath,
    ToggleFullscreen,
    ApplyWindowMode(iced::window::Mode),

    /// 定时轮询解码线程与缩略图线程的结果。
    Poll,
    Dismiss,
}
