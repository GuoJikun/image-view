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

/// 卡片 76×56（规范 4.3），内缩 3 px 让 1 px 发丝线与圆角不被图片方角盖住。
const CARD_WIDTH: f32 = 76.0;
const CARD_HEIGHT: f32 = 56.0;
/// 选中态外扩 2 px：环画在卡片外沿（80×60），内缩 5 px 后图片仍是 70×50。
const SELECTED_WIDTH: f32 = CARD_WIDTH + 4.0;
const SELECTED_HEIGHT: f32 = CARD_HEIGHT + 4.0;
const CARD_INSET: f32 = 3.0;
const SELECTED_INSET: f32 = 5.0;

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
                .width(Fill)
                .height(Fill)
                .content_fit(ContentFit::Cover)
                .into(),
            None => container(text("…").size(20))
                .width(Fill)
                .height(Fill)
                .align_x(iced::Center)
                .align_y(iced::Center)
                .into(),
        };

        // 卡片 76×56、圆角 8、图片满铺（`Cover` 与 `main` 的 `object-fit: cover` 一致）；
        // 只显示缩略图，不显示文件名（标题栏与元数据面板已给出文件名）。
        // 选中态把卡片撑到 80×60：环画在外沿、与图片留 3 px 空隙，图片尺寸保持 70×50
        // 不变，避免选中时缩略图跳动；外层格子宽度固定，相邻卡片不会位移。
        let (width, height, padding) = if selected {
            (SELECTED_WIDTH, SELECTED_HEIGHT, SELECTED_INSET)
        } else {
            (CARD_WIDTH, CARD_HEIGHT, CARD_INSET)
        };
        let card = button(preview)
            .on_press(Message::Select(position))
            .padding(padding)
            .width(width)
            .height(height)
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
