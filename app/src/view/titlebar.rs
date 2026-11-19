//! 自绘标题栏（Windows / Linux）：28px，左侧菜单栏、标题（文件名 — 应用名）居中、右侧 caption 按钮。
//!
//! macOS 不编译此模块——那里用系统原生标题栏（`main.rs` 保留 decorations），
//! 菜单栏单独占窗口顶部一行（见 `view/mod.rs`）。

use iced::widget::{button, column, container, mouse_area, row, rule, text};
use iced::{Alignment, Element, Fill, Padding};

use crate::message::{Caption, Message};
use crate::state::AppState;
use crate::theme::{self, Palette};
use crate::widgets::icons::{self, Icon};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let t = crate::view::viewer::theme_of(state);

    // 与系统窗口标题同一份文案（`app::title`）：有文件时是「文件名 — 应用名」，
    // 空状态只有应用名，避免自绘标题栏与任务栏标题不一致。
    let title = text(crate::app::title(state))
        .size(13)
        .width(Fill)
        .align_x(Alignment::Center);

    // 标题栏空白处：单击拖动窗口，双击切换最大化。
    let draggable = mouse_area(title)
        .on_press(Message::DragWindow)
        .on_double_click(Message::ToggleMaximize);

    // 悬停状态决定图标配色（关闭按钮悬停转白），因此每颗按钮各自判断。
    let caption = |kind: Caption, icon: Icon, message: Message| {
        caption_button(kind, icon, message, state.caption_hover == Some(kind), &t)
    };

    let buttons = row![
        caption(Caption::Minimize, Icon::Minimize, Message::MinimizeWindow),
        caption(
            Caption::Maximize,
            if state.maximized {
                Icon::Restore
            } else {
                Icon::Maximize
            },
            Message::ToggleMaximize,
        ),
        caption(Caption::Close, Icon::Close, Message::CloseWindow),
    ]
    .spacing(0);

    // 菜单栏占左侧（规范 4.5），标题吃满剩余宽度并居中，caption 按钮贴右。
    // 右侧不留内边距：Windows 的窗口按钮要贴到窗口边缘（含关闭按钮的矩形高亮），
    // 留 12px 会让人以为关闭按钮没靠边。间距只作用在标题与按钮之间。
    let bar = row![crate::view::menu::bar(state), draggable, buttons]
        .spacing(8)
        .align_y(iced::Center)
        .padding(Padding {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 12.0,
        });

    // main 的 `.titlebar` 有 `border-bottom: 1px solid var(--hairline)`，用来把
    // 标题栏与工具栏隔开。chrome 样式被多处复用，不能就地加边框，故在标题栏
    // 下方补一条同款 1px rule。column 默认 Shrink，必须显式 width(Fill)，
    // 否则 rule 的 Fill 宽度会被解析成 0 而看不见。
    column![
        container(bar).style(theme::chrome).width(Fill).height(28),
        rule::horizontal(1.0).style(theme::hairline_rule),
    ]
    .width(Fill)
    .into()
}

fn caption_button(
    kind: Caption,
    icon: Icon,
    message: Message,
    hovered: bool,
    theme_ref: &iced::Theme,
) -> Element<'static, Message> {
    // 按钮 46×28、图标 14：用 padding 把内容区收回到 14×14，否则 `button`
    // 会把固定尺寸的 SVG 拉伸到按钮大小（与工具栏图标同一问题）。
    const ICON: f32 = 14.0;
    let danger = kind == Caption::Close;
    let palette = Palette::from(theme_ref);
    let icon_color = caption_icon_color(kind, hovered, palette);

    let button = button(icons::view(icon, ICON, icon_color))
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
            button::Style {
                background: bg,
                text_color: p.text,
                border: iced::Border::default(),
                ..Default::default()
            }
        })
        .on_press(message);

    // 悬停要驱动图标配色，而 `button` 的样式触不到内部 SVG，故在外面记一笔；
    // 光标仍由 `button` 决定（`mouse_area` 不声明 interaction，取 max 后是手型）。
    mouse_area(button)
        .on_enter(Message::CaptionHovered(kind))
        .on_exit(Message::CaptionLeft(kind))
        .into()
}

/// caption 图标的颜色。
///
/// 关闭按钮悬停时底色转红，图标必须变白才看得清；最小化 / 最大化悬停底只是浅灰，
/// 保持主题图标色。`button::Style.text_color` 管不到内部 SVG，所以颜色在构建时按
/// 悬停状态决定，悬停状态由 `AppState::caption_hover` 记录。
fn caption_icon_color(caption: Caption, hovered: bool, palette: Palette) -> iced::Color {
    match (caption, hovered) {
        (Caption::Close, true) => iced::Color::WHITE,
        (Caption::Close, false) => palette.text,
        _ => palette.icon,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> Palette {
        Palette::from(&iced::Theme::Light)
    }

    #[test]
    fn close_icon_turns_white_on_hover_only() {
        let p = palette();
        assert_eq!(
            caption_icon_color(Caption::Close, true, p),
            iced::Color::WHITE,
            "关闭按钮悬停：红底上必须是白图标"
        );
        assert_eq!(
            caption_icon_color(Caption::Close, false, p),
            p.text,
            "关闭按钮常态：跟随主题文字色"
        );
    }

    #[test]
    fn other_captions_keep_theme_icon_color() {
        let p = palette();
        for caption in [Caption::Minimize, Caption::Maximize] {
            assert_eq!(caption_icon_color(caption, true, p), p.icon);
            assert_eq!(caption_icon_color(caption, false, p), p.icon);
        }
    }
}
