//! 单色 SVG 图标集（内联路径，无外部依赖）。
//!
//! 图标为 24×24 viewBox、`stroke="black"`；通过 `svg::Style.color` 过滤上色，
//! 因此同一份路径可随浅 / 深主题切换颜色。路径取自常见的开源线性图标集合，
//! 统一 1.6 线宽、圆角端点，与 macOS-first 规范的 1.5 线宽接近。

use iced::widget::svg;
use iced::{Color, Element};

use crate::message::Message;
use crate::theme;

/// 图标字节数据（`sun` 之类仅占位）；实际由 [`Icon`] 关联。
// 文件夹形：横向 2..22、纵向 3..20，约 20×17 单位——与相邻箭头图标（纵向 14 单位）
// 视觉量级相当。早期版本只用 3..21 × 6..19（18×13），看上去扁而小。
const OPEN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/></svg>"#;
const PREV: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M15 5 8 12l7 7"/></svg>"#;
const NEXT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m9 5 7 7-7 7"/></svg>"#;
const PLAY: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M8 5.5v13l11-6.5z"/></svg>"#;
const STOP: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="6.5" y="6.5" width="11" height="11" rx="1.5"/></svg>"#;
const INFO: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 8h.01"/></svg>"#;
const THUMBS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="3.5" y="5" width="7" height="6" rx="1"/><rect x="13.5" y="5" width="7" height="6" rx="1"/><rect x="3.5" y="13" width="7" height="6" rx="1"/><rect x="13.5" y="13" width="7" height="6" rx="1"/></svg>"#;
const MORE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black"><circle cx="5.5" cy="12" r="1.6"/><circle cx="12" cy="12" r="1.6"/><circle cx="18.5" cy="12" r="1.6"/></svg>"#;
const MINIMIZE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round"><path d="M5 12h14"/></svg>"#;
const MAXIMIZE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linejoin="round"><rect x="5.5" y="5.5" width="13" height="13" rx="1.5"/></svg>"#;
const CLOSE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18"/></svg>"#;

#[derive(Clone, Copy)]
pub enum Icon {
    Open,
    Prev,
    Next,
    Play,
    Stop,
    Info,
    Thumbs,
    More,
    Minimize,
    Maximize,
    Close,
}

impl Icon {
    fn bytes(self) -> &'static [u8] {
        match self {
            Icon::Open => OPEN,
            Icon::Prev => PREV,
            Icon::Next => NEXT,
            Icon::Play => PLAY,
            Icon::Stop => STOP,
            Icon::Info => INFO,
            Icon::Thumbs => THUMBS,
            Icon::More => MORE,
            Icon::Minimize => MINIMIZE,
            Icon::Maximize => MAXIMIZE,
            Icon::Close => CLOSE,
        }
    }
}

/// 生成一个 17px 的单色图标元素，颜色由主题令牌决定。
pub fn view<'a>(icon: Icon, size: f32, color: Color) -> Element<'a, Message> {
    svg(svg::Handle::from_memory(icon.bytes()))
        .width(size)
        .height(size)
        .style(move |t, _| theme::icon_svg(t, color))
        .into()
}
