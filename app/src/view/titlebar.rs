//! 自绘标题栏（Windows / Linux）：28px，文件名居中，右侧 caption 按钮。
//!
//! macOS 不编译此模块——那里用系统原生标题栏（`main.rs` 保留 decorations）。

use iced::widget::{button, container, mouse_area, row, text};
use iced::{Alignment, Element, Fill};

use crate::message::Message;
use crate::state::AppState;
use crate::theme::{self, Palette};
use crate::widgets::icons::{self, Icon};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let filename = state
        .current_path()
        .and_then(|path| path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "图片查看器".to_owned());

    let title = text(filename)
        .size(13)
        .width(Fill)
        .align_x(Alignment::Center);

    // 标题栏空白处：单击拖动窗口，双击切换最大化。
    let draggable = mouse_area(title)
        .on_press(Message::DragWindow)
        .on_double_click(Message::ToggleMaximize);

    let buttons = row![
        caption_button(Icon::Minimize, Message::MinimizeWindow, false),
        caption_button(Icon::Maximize, Message::ToggleMaximize, false),
        caption_button(Icon::Close, Message::CloseWindow, true),
    ];

    let bar = row![draggable, buttons]
        .spacing(8)
        .align_y(iced::Center)
        .padding([0, 12]);

    container(bar)
        .style(theme::chrome)
        .width(Fill)
        .height(28)
        .into()
}

fn caption_button(icon: Icon, message: Message, danger: bool) -> Element<'static, Message> {
    let icon_element = icons::view(icon, 14.0, iced::Color::from_rgb(0.35, 0.35, 0.38));
    button(icon_element)
        .width(46)
        .height(28)
        .padding(0)
        .style(move |t, status| {
            let p = Palette::from(t);
            let hovering = matches!(status, button::Status::Hovered);
            let bg = if hovering && danger {
                Some(iced::Background::Color(iced::Color::from_rgb(
                    0.769, 0.169, 0.11,
                )))
            } else if hovering {
                Some(iced::Background::Color(p.hover))
            } else {
                None
            };
            let text_color = if hovering && danger {
                iced::Color::WHITE
            } else if danger {
                p.text
            } else {
                p.icon
            };
            button::Style {
                background: bg,
                text_color,
                border: iced::Border::default(),
                ..Default::default()
            }
        })
        .on_press(message)
        .into()
}
