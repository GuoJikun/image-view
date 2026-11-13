//! 图库侧视图区：底部缩略图条与右侧元数据面板。

use iced::Element;

use crate::message::Message;
use crate::state::AppState;
use crate::widgets::{metadata_panel, thumbnail_strip};

pub fn thumbnails(state: &AppState) -> Option<Element<'_, Message>> {
    if !state.library.thumbs.show {
        return None;
    }
    Some(thumbnail_strip::view(
        &state.library,
        &state.nav.files,
        state.nav.index,
    ))
}

pub fn metadata(state: &AppState) -> Option<Element<'_, Message>> {
    if !state.library.meta.show {
        return None;
    }
    Some(metadata_panel::view(&state.library.meta))
}
