//! 缩略图条：水平虚拟滚动 + 按需解码。
//!
//! 只为可见区间构建 widget，前后用 `Space` 撑出完整的内容宽度；滚动偏移由
//! `on_scroll` 回报给 `LibraryState`，解码请求在 Update 里排队。
//!
//! 卡片几何逐项对齐 `main`（Vue 版）的 `.thumb-item`：76 × 56、圆角 8、
//! 1 px 描边、图片 `cover` 内缩 1 px 并裁圆角；选中时卡片尺寸不变，
//! 2 px `accent` 环画在卡片外 2 px 处（等价 `outline: 2px … offset: 2px`）。

use std::path::PathBuf;

use iced::widget::{button, container, image, row, scrollable, space};
use iced::{ContentFit, Element, Fill};

use crate::message::Message;
use crate::state::library_state::{
    LibraryState, THUMB_ITEM_WIDTH, THUMB_RING_OVERHANG, THUMB_TRACK_PAD,
};
use crate::theme::RADIUS_TILE;

pub const THUMB_SCROLL_ID: &str = "thumbnail-strip";
pub const STRIP_HEIGHT: f32 = 88.0;

/// 卡片 76 × 56（规范 4.3 / `main` 的 `.thumb-item`）。
const CARD_WIDTH: f32 = 76.0;
const CARD_HEIGHT: f32 = 56.0;
/// 卡片 1 px 描边：`button` 的 `border` 不参与布局，必须用 padding 把图片挤进去，
/// 否则满铺的图片会盖住描边（`main` 用 `border-box` 天然留出这 1 px）。
const CARD_BORDER: f32 = 1.0;
/// 图片圆角 = 卡片圆角 − 描边，等价于 `main` 的 `overflow: hidden` 裁出的内圆角。
const IMAGE_RADIUS: f32 = RADIUS_TILE - CARD_BORDER;
/// 选中环容器：卡片 + 两侧各 4 px（2 px 空隙 + 2 px 环）。
const RING_WIDTH: f32 = CARD_WIDTH + THUMB_RING_OVERHANG * 2.0;
const RING_HEIGHT: f32 = CARD_HEIGHT + THUMB_RING_OVERHANG * 2.0;

pub fn view<'a>(
    library: &'a LibraryState,
    files: &'a [PathBuf],
    index: Option<usize>,
) -> Element<'a, Message> {
    let total = files.len();
    let range = library.visible_range(total);
    let mut cards: Vec<Element<'a, Message>> = Vec::with_capacity(range.len() + 2);

    // 用占位宽度保持滚动条的内容总长，避免每帧创建成千上万个卡片。
    cards.push(
        space()
            .width(THUMB_TRACK_PAD + range.start as f32 * THUMB_ITEM_WIDTH)
            .into(),
    );

    for position in range.clone() {
        let path = &files[position];
        let selected = index == Some(position);

        // 图片落在卡片内容区（74 × 54）并按 7 px 圆角裁切；未解码时留一张空白卡片
        // （`main` 的 `<img>` 未加载时同样是白底，不显示占位字符）。
        let preview: Element<'a, Message> = match library.thumbs.handle(path) {
            Some(handle) => image(handle.clone())
                .width(Fill)
                .height(Fill)
                .content_fit(ContentFit::Cover)
                .border_radius(IMAGE_RADIUS)
                .into(),
            None => space().width(Fill).height(Fill).into(),
        };

        let card = button(preview)
            .on_press(Message::Select(position))
            .padding(CARD_BORDER)
            .width(CARD_WIDTH)
            .height(CARD_HEIGHT)
            .style(move |theme, status| crate::theme::thumb_card(theme, status, selected));

        // 选中环画在 84 × 64 的外层容器上：4 px 内边距让卡片回到 76 × 56，
        // 环占据外沿 2 px，与卡片之间留 2 px 空隙。未选中时外层照样占位，
        // 因此切换选中不会改变卡片尺寸或相邻卡片位置。
        cards.push(
            container(
                container(card)
                    .width(RING_WIDTH)
                    .height(RING_HEIGHT)
                    .padding(THUMB_RING_OVERHANG)
                    .style(move |theme| crate::theme::thumb_ring(theme, selected)),
            )
            // 槽位 85 px、环靠左：卡片落在槽位 + 4 px（选中环外扩量），
            // 与 `main` 的 `left = PAD + index * ITEM_WIDTH` 等价，
            // 槽位余下的 9 px 就是卡片间距。
            .width(THUMB_ITEM_WIDTH)
            .height(STRIP_HEIGHT)
            .align_x(iced::Left)
            .align_y(iced::Center)
            .into(),
        );
    }

    cards.push(
        space()
            .width(THUMB_TRACK_PAD + (total - range.end) as f32 * THUMB_ITEM_WIDTH)
            .into(),
    );

    // 滚动条不占位：常驻的 8 px 滚动条会吃掉条带高度、把 56 px 卡片裁成 52 px
    // （与 `main` 的悬浮滚动条也不一致）。隐藏后条带恢复 88 px，卡片上下各 16 px；
    // 滚动仍可用 Shift + 滚轮，翻页时由 `center_thumbnails` 自动定位。
    let scroller = scrollable(row(cards).height(STRIP_HEIGHT))
        .id(THUMB_SCROLL_ID)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::hidden(),
        ))
        .width(Fill)
        .height(STRIP_HEIGHT)
        .on_scroll(|viewport| {
            let offset = viewport.absolute_offset();
            Message::ThumbScrolled {
                offset_x: offset.x,
                viewport_w: viewport.bounds().width,
            }
        });

    // 条底用 strip 令牌，上缘 1px 发丝线与画布分隔（规范 4.3 / 体验清单 9）。
    container(scroller)
        .style(crate::theme::strip)
        .width(Fill)
        .height(STRIP_HEIGHT)
        .into()
}
