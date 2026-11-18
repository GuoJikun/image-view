//! 快捷键弹窗（规范 4.8）：两列「按键 + 说明」，遮罩点击关闭。
//!
//! 只列本分支真实支持的快捷键；翻转能力不在本分支，不予展示。

use iced::widget::{button, column, container, mouse_area, row, space, stack, text};
use iced::{Element, Fill, Padding, mouse};

use crate::menu::{accel, delete_key};
use crate::message::Message;
use crate::state::AppState;
use crate::theme;

const PANEL_WIDTH: f32 = 420.0;
const KEYS_WIDTH: f32 = 120.0;

pub fn view(state: &AppState) -> Option<Element<'static, Message>> {
    if state.dialog != Some(crate::state::app_state::Dialog::Shortcuts) {
        return None;
    }
    let t = state.text();

    let rows = [
        (accel(&["mod", "O"]), t.shortcut_open),
        ("← / →".to_owned(), t.shortcut_navigate),
        ("+ / −".to_owned(), t.shortcut_zoom),
        ("0".to_owned(), t.shortcut_fit),
        ("1".to_owned(), t.shortcut_actual),
        (format!("R / {}", accel(&["shift", "R"])), t.shortcut_rotate),
        ("Space".to_owned(), t.shortcut_playback),
        ("F11".to_owned(), t.shortcut_fullscreen),
        (accel(&["mod", "shift", "C"]), t.shortcut_copy_path),
        (delete_key(), t.shortcut_trash),
        ("Esc".to_owned(), t.shortcut_dismiss),
        (String::new(), t.shortcut_pan),
        (String::new(), t.shortcut_wheel),
    ];

    let mut list: Vec<Element<'static, Message>> = Vec::with_capacity(rows.len());
    for (keys, label) in rows {
        let keys: Element<'static, Message> = if keys.is_empty() {
            container(text("")).width(KEYS_WIDTH).into()
        } else {
            container(text(keys).size(12))
                .style(theme::keycap)
                .padding(Padding {
                    top: 2.0,
                    bottom: 2.0,
                    left: 6.0,
                    right: 6.0,
                })
                .into()
        };
        list.push(
            row![
                container(keys).width(KEYS_WIDTH),
                text(label).size(13).width(Fill),
            ]
            .spacing(12)
            .align_y(iced::Center)
            .into(),
        );
    }

    let close = button(text(t.dialog_close).size(12))
        .style(theme::primary_button)
        .padding([6, 16])
        .height(28)
        .on_press(Message::CloseDialog);

    let panel = container(
        column![
            text(t.dialog_shortcuts).size(15),
            column(list).spacing(6),
            row![space().width(Fill), close],
        ]
        .spacing(14),
    )
    .style(theme::dialog)
    .padding(16)
    .width(PANEL_WIDTH);

    // 遮罩：光标固定为箭头，否则被遮住的按钮 / 画布会把各自的光标透上来。
    let backdrop = mouse_area(
        container(space())
            .width(Fill)
            .height(Fill)
            .style(theme::dialog_backdrop),
    )
    .interaction(mouse::Interaction::Idle)
    .on_press(Message::CloseDialog);

    // 面板套一层无副作用的 mouse_area：点在弹窗内部时不再穿透到遮罩。
    let panel = mouse_area(panel)
        .interaction(mouse::Interaction::Idle)
        .on_press(Message::Noop);

    Some(
        stack![
            backdrop,
            container(panel)
                .width(Fill)
                .height(Fill)
                .align_x(iced::Center)
                .align_y(iced::Center)
        ]
        .into(),
    )
}
