//! 单色 SVG 图标集（图标源文件位于 `app/src/icons/`，编译期打包）。
//!
//! 图标来自 Lucide 线性图标集合（ISC 许可，见 `icons/LICENSE`）：24×24 viewBox、
//! 2.0 线宽、圆角端点。工具栏图标 16px 高，2.0 的描边缩到约 1.3px；图标边长必须取
//! 偶数，让按钮 padding 落在整像素，否则半像素偏移叠加 Iced 的线性采样会把横线冲淡。
//! 源文件只声明几何与 `stroke="currentColor"`，不带具体颜色；Iced 用 usvg 光栅化
//! 后由 `svg::Style.color` 给所有不透明像素重新上色，因此同一份路径可随浅 / 深
//! 主题切换颜色。
//!
//! `stroke` 必须留在源文件里：Iced 光栅化时不会像浏览器 CSS 那样补
//! `stroke: currentColor`，缺失描边的路径（`fill="none"`）会渲染成透明。

use iced::widget::svg;
use iced::{Color, Element};

use crate::message::Message;
use crate::theme;

const OPEN: &[u8] = include_bytes!("../icons/folder.svg");
const PREV: &[u8] = include_bytes!("../icons/chevron-left.svg");
const NEXT: &[u8] = include_bytes!("../icons/chevron-right.svg");
const CHEVRON_DOWN: &[u8] = include_bytes!("../icons/chevron-down.svg");
const ROTATE_CW: &[u8] = include_bytes!("../icons/rotate-cw.svg");
const ROTATE_CCW: &[u8] = include_bytes!("../icons/rotate-ccw.svg");
const FLIP_H: &[u8] = include_bytes!("../icons/flip-h.svg");
const FLIP_V: &[u8] = include_bytes!("../icons/flip-v.svg");
const PLAY: &[u8] = include_bytes!("../icons/play.svg");
const PAUSE: &[u8] = include_bytes!("../icons/pause.svg");
const THUMBS: &[u8] = include_bytes!("../icons/sidebar.svg");
const MINUS: &[u8] = include_bytes!("../icons/minus.svg");
const PLUS: &[u8] = include_bytes!("../icons/plus.svg");
const PHOTO: &[u8] = include_bytes!("../icons/photo.svg");
const WARNING: &[u8] = include_bytes!("../icons/warning.svg");
const MINIMIZE: &[u8] = include_bytes!("../icons/minimize.svg");
const MAXIMIZE: &[u8] = include_bytes!("../icons/maximize.svg");
const RESTORE: &[u8] = include_bytes!("../icons/restore.svg");
const CLOSE: &[u8] = include_bytes!("../icons/close.svg");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Open,
    Prev,
    Next,
    RotateCw,
    RotateCcw,
    FlipH,
    FlipV,
    Thumbs,
    Play,
    Pause,
    Chevron,
    ChevronDown,
    Minus,
    Plus,
    Photo,
    Warning,
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl Icon {
    fn bytes(self) -> &'static [u8] {
        match self {
            Icon::Open => OPEN,
            Icon::Prev => PREV,
            Icon::Next => NEXT,
            Icon::Chevron => NEXT,
            Icon::ChevronDown => CHEVRON_DOWN,
            Icon::RotateCw => ROTATE_CW,
            Icon::RotateCcw => ROTATE_CCW,
            Icon::FlipH => FLIP_H,
            Icon::FlipV => FLIP_V,
            Icon::Play => PLAY,
            Icon::Pause => PAUSE,
            Icon::Thumbs => THUMBS,
            Icon::Minus => MINUS,
            Icon::Plus => PLUS,
            Icon::Photo => PHOTO,
            Icon::Warning => WARNING,
            Icon::Minimize => MINIMIZE,
            Icon::Maximize => MAXIMIZE,
            Icon::Restore => RESTORE,
            Icon::Close => CLOSE,
        }
    }
}

/// 生成一个单色图标元素，颜色由主题令牌决定。
pub fn view<'a>(icon: Icon, size: f32, color: Color) -> Element<'a, Message> {
    svg(svg::Handle::from_memory(icon.bytes()))
        .width(size)
        .height(size)
        .style(move |t, _| theme::icon_svg(t, color))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: &[Icon] = &[
        Icon::Open,
        Icon::Prev,
        Icon::Next,
        Icon::RotateCw,
        Icon::RotateCcw,
        Icon::FlipH,
        Icon::FlipV,
        Icon::Thumbs,
        Icon::Play,
        Icon::Pause,
        Icon::Chevron,
        Icon::ChevronDown,
        Icon::Minus,
        Icon::Plus,
        Icon::Photo,
        Icon::Warning,
        Icon::Minimize,
        Icon::Maximize,
        Icon::Restore,
        Icon::Close,
    ];

    /// 图标由 Iced 的 usvg 光栅化；源文件缺少 `stroke` 时路径不可见，
    /// 这里守住源文件的必要属性，避免换图后界面静默丢图标。
    #[test]
    fn every_icon_is_a_stroked_24px_svg() {
        for icon in ALL {
            let source = std::str::from_utf8(icon.bytes())
                .expect("图标源文件应为 UTF-8")
                .replace(['\r', '\n'], " ");
            assert!(source.contains("<svg"), "{icon:?} 不是 SVG");
            assert!(
                source.contains("viewBox=\"0 0 24 24\""),
                "{icon:?} 的 viewBox 应为 24×24"
            );
            assert!(source.contains("fill=\"none\""), "{icon:?} 应为线性图标");
            assert!(
                source.contains("stroke=\"currentColor\""),
                "{icon:?} 缺少 stroke，Iced 会把路径渲染成透明"
            );
            assert!(
                source.contains("stroke-width=\"2\""),
                "{icon:?} 应使用 Lucide 上游的 2px 线宽"
            );
        }
    }

    #[test]
    fn chevrons_share_the_direction_of_their_arrow() {
        assert_ne!(Icon::Prev.bytes(), Icon::Next.bytes());
        assert_eq!(Icon::Chevron.bytes(), Icon::Next.bytes());
    }
}
