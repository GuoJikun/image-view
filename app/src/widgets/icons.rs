//! 单色 SVG 图标集（内联路径，无外部依赖）。
//!
//! 图标以 `stroke="black"` 描边、通过 `svg::Style.color` 过滤上色，因此同一份路径
//! 可随浅 / 深主题切换颜色。多数图标沿用常见的开源线性图标集合（24×24 viewBox、
//! 1.6 线宽、圆角端点，与 macOS-first 规范的 1.5 线宽接近）；箭头类见 `CHEVRON_*`，
//! 菜单子项与工具栏翻页共用同一份定义。

use iced::widget::svg;
use iced::{Color, Element};

use crate::message::Message;
use crate::theme;

/// 图标字节数据（`sun` 之类仅占位）；实际由 [`Icon`] 关联。
// 文件夹形：横向 2..22、纵向 3..20，约 20×17 单位。
const OPEN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/></svg>"#;
// 箭头类图标：24 viewBox + 2.0 线宽。菜单子项与工具栏翻页共用同一份几何，
// 两处的笔画粗细与折角半径才完全一致——此前两边各写一份（24/1.8 与 16/2.0），
// 折算到实际像素后一细一粗，看起来"不是一个实现"。
const CHEVRON_RIGHT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m10 6 6 6-6 6"/></svg>"#;
const CHEVRON_LEFT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m14 6-6 6 6 6"/></svg>"#;
// 下拉箭头：与左右箭头同一份 24 viewBox / 2.0 线宽，只改折角方向；工具栏排序用 12px 渲染。
const CHEVRON_DOWN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"/></svg>"#;
// 播放 / 停止：2.0 线宽 + 更大的图形。三角原先只占 11×13 单位、1.6 线宽，配 17px
// 渲染后实际 7.8×9.2px、描边 1.1px，比相邻的圆圈 / 四宫格又小又虚；现在占 12.5×15
// 单位、19px 渲染（`viewer::TOOL_MEDIA`）≈ 9.9×11.9px、描边 1.6px。停止键同步放大。
const PLAY: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M8 4.5v15l12.5-7.5z"/></svg>"#;
const STOP: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="5.5" y="5.5" width="13" height="13" rx="1.5"/></svg>"#;
// 缩略图条开关：分栏矩形（左栏竖线），与 `main` 的 `sidebar` 图标同一几何。
const THUMBS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="3.5" y="5" width="17" height="14" rx="2.5"/><path d="M10 5v14"/></svg>"#;
const MINIMIZE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round"><path d="M5 12h14"/></svg>"#;
const MAXIMIZE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linejoin="round"><rect x="5.5" y="5.5" width="13" height="13" rx="1.5"/></svg>"#;
const CLOSE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18"/></svg>"#;
// 旋转 / 翻转：路径与 `main` 的 `Icon.vue` 同源，仍是 24 viewBox / 线性描边；
// 虚线是镜像轴，两个三角分别指向镜像后的方向。
const ROTATE_CW: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M19.5 12a7.5 7.5 0 1 1-2.2-5.3"/><path d="M19.5 4.2v4.5H15"/></svg>"#;
const ROTATE_CCW: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M4.5 12a7.5 7.5 0 1 0 2.2-5.3"/><path d="M4.5 4.2v4.5H9"/></svg>"#;
const FLIP_H: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M12 4.5v15" stroke-dasharray="2 2"/><path d="M9.5 7 4.5 12l5 5z"/><path d="M14.5 7l5 5-5 5z"/></svg>"#;
const FLIP_V: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M4.5 12h15" stroke-dasharray="2 2"/><path d="M7 9.5 12 4.5l5 5z"/><path d="M7 14.5 12 19.5l5-5z"/></svg>"#;

#[derive(Clone, Copy)]
pub enum Icon {
    Open,
    Prev,
    Next,
    RotateCw,
    RotateCcw,
    FlipH,
    FlipV,
    Play,
    Stop,
    Thumbs,
    Minimize,
    Maximize,
    Close,
    Chevron,
    ChevronDown,
}

impl Icon {
    fn bytes(self) -> &'static [u8] {
        match self {
            Icon::Open => OPEN,
            Icon::Prev => CHEVRON_LEFT,
            Icon::Next => CHEVRON_RIGHT,
            Icon::RotateCw => ROTATE_CW,
            Icon::RotateCcw => ROTATE_CCW,
            Icon::FlipH => FLIP_H,
            Icon::FlipV => FLIP_V,
            Icon::Play => PLAY,
            Icon::Stop => STOP,
            Icon::Thumbs => THUMBS,
            Icon::Minimize => MINIMIZE,
            Icon::Maximize => MAXIMIZE,
            Icon::Close => CLOSE,
            Icon::Chevron => CHEVRON_RIGHT,
            Icon::ChevronDown => CHEVRON_DOWN,
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
