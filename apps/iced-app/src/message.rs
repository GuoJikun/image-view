//! Iced 消息：只描述 UI 事件和异步结果，不进入 Domain。

use std::path::PathBuf;

use emath::Vec2;
use iv_domain::SortOrder;

#[derive(Debug, Clone)]
pub enum Message {
    /// 全局事件（键盘、窗口、拖放）。
    Event(iced::Event),

    OpenDialog,
    DialogPicked(Option<PathBuf>),
    OpenPath(PathBuf),

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
    ScaleFactor(f32),

    ToggleThumbnails,
    ToggleMetadata,
    SetSort(SortOrder),

    Reveal,
    DeleteCurrent,
    CopyPath,

    /// 轮询解码线程与缩略图线程的结果。
    Poll,
    Dismiss,
}