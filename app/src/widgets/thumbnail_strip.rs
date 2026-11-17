//! 缩略图条：水平虚拟滚动 + 按需解码。
//!
//! 只为可见区间构建 widget，前后用 `Space` 撑出完整的内容宽度；滚动偏移由
//! `on_scroll` 回报给 `LibraryState`，解码请求在 Update 里排队。

use std::path::PathBuf;

use iced::widget::{button, container, image, row, scrollable, space, text};
use iced::{ContentFit, Element, Fill};

use crate::message::Message;
use crate::state::library_state::{LibraryState, THUMB_ITEM_WIDTH};

pub const THUMB_SCROLL_ID: &str = "thumbnail-strip";
pub const STRIP_HEIGHT: f32 = 88.0;

/// 卡片 76×56（规范 4.3），四边留 4 px，内容区刚好 68×48。
const CARD_WIDTH: f32 = 76.0;
const CARD_HEIGHT: f32 = 56.0;
const PREVIEW_WIDTH: f32 = 68.0;
const PREVIEW_HEIGHT: f32 = 48.0;

pub fn view<'a>(
    library: &'a LibraryState,
    files: &'a [PathBuf],
    index: Option<usize>,
) -> Element<'a, Message> {
    let total = files.len();
    let range = library.visible_range(total);
    let mut cards: Vec<Element<'a, Message>> = Vec::with_capacity(range.len() + 2);

    // 用占位宽度保持滚动条的内容总长，避免每帧创建成千上万个卡片。
    cards.push(space().width(range.start as f32 * THUMB_ITEM_WIDTH).into());

    for position in range.clone() {
        let path = &files[position];
        let selected = index == Some(position);

        let preview: Element<'a, Message> = match library.thumbs.handle(path) {
            Some(handle) => image(handle.clone())
                .width(PREVIEW_WIDTH)
                .height(PREVIEW_HEIGHT)
                .content_fit(ContentFit::Contain)
                .into(),
            None => container(text("…").size(20))
                .width(PREVIEW_WIDTH)
                .height(PREVIEW_HEIGHT)
                .align_x(iced::Center)
                .align_y(iced::Center)
                .into(),
        };

        // 卡片 76×56、圆角 8、选中 2px accent 环 + 投影（规范 4.3）；
        // 只显示缩略图，不显示文件名（标题栏与元数据面板已给出文件名）。
        let card = button(preview)
            .on_press(Message::Select(position))
            .padding(4)
            .width(CARD_WIDTH)
            .height(CARD_HEIGHT)
            .style(move |theme, status| crate::theme::thumb_card(theme, status, selected));

        cards.push(
            container(card)
                .width(THUMB_ITEM_WIDTH)
                .height(STRIP_HEIGHT)
                .align_x(iced::Center)
                .align_y(iced::Center)
                .into(),
        );
    }

    cards.push(
        space()
            .width((total - range.end) as f32 * THUMB_ITEM_WIDTH)
            .into(),
    );

    let scroller = scrollable(row(cards).height(STRIP_HEIGHT))
        .id(THUMB_SCROLL_ID)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new().width(8),
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
