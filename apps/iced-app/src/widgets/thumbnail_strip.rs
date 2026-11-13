//! 缩略图条：水平虚拟滚动 + 按需解码。
//!
//! 只为可见区间构建 widget，前后用 `Space` 撑出完整的内容宽度；滚动偏移由
//! `on_scroll` 回报给 `LibraryState`，解码请求在 Update 里排队。

use std::path::PathBuf;

use iced::widget::{button, column, container, image, row, scrollable, space, text};
use iced::{ContentFit, Element, Fill};

use crate::message::Message;
use crate::state::library_state::{LibraryState, THUMB_ITEM_WIDTH};

pub const THUMB_SCROLL_ID: &str = "thumbnail-strip";
pub const STRIP_HEIGHT: f32 = 88.0;

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
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();

        let preview: Element<'a, Message> = match library.thumbs.handle(path) {
            Some(handle) => image(handle.clone())
                .width(THUMB_ITEM_WIDTH - 16.0)
                .height(STRIP_HEIGHT - 32.0)
                .content_fit(ContentFit::Contain)
                .into(),
            None => container(text("…").size(20))
                .width(THUMB_ITEM_WIDTH - 16.0)
                .height(STRIP_HEIGHT - 32.0)
                .align_x(iced::Center)
                .align_y(iced::Center)
                .into(),
        };

        let card = button(
            column![preview, text(name).size(10)]
                .spacing(4)
                .align_x(iced::Center),
        )
        .on_press(Message::Select(position))
        .padding(4)
        .width(THUMB_ITEM_WIDTH - 8.0)
        .height(STRIP_HEIGHT - 8.0)
        .style(move |theme, status| {
            if selected {
                button::primary(theme, status)
            } else {
                button::text(theme, status)
            }
        });

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

    scrollable(row(cards).height(STRIP_HEIGHT))
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
        })
        .into()
}
