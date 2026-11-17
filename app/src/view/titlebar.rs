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
    let t = crate::view::viewer::theme_of(state);
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
        caption_button(Icon::Minimize, Message::MinimizeWindow, false, &t),
        caption_button(Icon::Maximize, Message::ToggleMaximize, false, &t),
        caption_button(Icon::Close, Message::CloseWindow, true, &t),
    ]
    .spacing(0);

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

fn caption_button(
    icon: Icon,
    message: Message,
    danger: bool,
    theme_ref: &iced::Theme,
) -> Element<'static, Message> {
    // 按钮 46×28、图标 14：用 padding 把内容区收回到 14×14，否则 `button`
    // 会把固定尺寸的 SVG 拉伸到按钮大小（与工具栏图标同一问题）。
    const ICON: f32 = 14.0;
    // 图标颜色来自主题令牌；SVG 是自绘的，按钮的 `text_color` 管不到它。
    let icon_element = icons::view(icon, ICON, Palette::from(theme_ref).icon);
    button(icon_element)
        .width(46)
        .height(28)
        .padding([(28.0 - ICON) / 2.0, (46.0 - ICON) / 2.0])
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
