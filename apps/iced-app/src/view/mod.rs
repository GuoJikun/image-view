//! View：把 `AppState` 映射为 Iced 元素树。只读状态，不发起用例。

pub mod library;
pub mod settings;
pub mod viewer;

use iced::widget::{column, row};
use iced::{Element, Fill};

use crate::message::Message;
use crate::state::AppState;

pub fn view(state: &AppState) -> Element<'_, Message> {
    let mut sections: Vec<Element<'_, Message>> = Vec::new();
    sections.push(viewer::toolbar(state));

    if let Some(menu) = settings::menu(state) {
        sections.push(menu);
    }
    if let Some(banner) = viewer::banner(state) {
        sections.push(banner);
    }

    let mut middle: Vec<Element<'_, Message>> = vec![viewer::canvas_area(state)];
    if let Some(panel) = library::metadata(state) {
        middle.push(panel);
    }
    sections.push(row(middle).height(Fill).into());

    if let Some(strip) = library::thumbnails(state) {
        sections.push(strip);
    }
    sections.push(viewer::status_bar(state));

    column(sections).width(Fill).height(Fill).into()
}
