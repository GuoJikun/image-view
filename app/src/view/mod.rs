//! View：把 `AppState` 映射为 Iced 元素树。只读状态，不发起用例。

pub mod dialog;
pub mod library;
pub mod menu;
#[cfg(not(target_os = "macos"))]
pub mod titlebar;
pub mod viewer;

#[cfg(target_os = "macos")]
use iced::widget::container;
use iced::widget::{column, row, stack};
use iced::{Element, Fill};

use crate::message::Message;
use crate::state::AppState;

pub fn view(state: &AppState) -> Element<'_, Message> {
    let mut sections: Vec<Element<'_, Message>> = Vec::new();

    // macOS 没有可用的原生菜单 API（Iced 0.14 无系统菜单），菜单栏单独占顶部一行；
    // Windows / Linux 把同一个菜单栏并进自绘标题栏（规范 4.5）。
    #[cfg(target_os = "macos")]
    sections.push(
        container(menu::bar(state))
            .style(crate::theme::chrome)
            .padding([0, 12])
            .width(Fill)
            .into(),
    );

    // macOS 用系统原生标题栏；Windows / Linux 自绘（规范二/五）。全屏时隐藏。
    #[cfg(not(target_os = "macos"))]
    if !state.fullscreen {
        sections.push(titlebar::view(state));
    }

    sections.push(viewer::toolbar(state));

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

    let base = column(sections).width(Fill).height(Fill);

    // 下拉菜单与弹窗都叠在内容之上，顺序即层级：菜单遮罩 → 菜单面板 → 弹窗。
    let mut layers: Vec<Element<'_, Message>> = vec![base.into()];
    if let Some(mut overlay) = menu::overlay(state) {
        layers.append(&mut overlay);
    }
    if let Some(dialog) = dialog::view(state) {
        layers.push(dialog);
    }

    stack(layers).width(Fill).height(Fill).into()
}
