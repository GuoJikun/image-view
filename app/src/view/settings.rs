//! 溢出菜单：排序、文件操作与窗口模式。
//!
//! 菜单发出的 `Message` 与工具栏一致，仍然由 Update 层映射到 Core 用例。

use iced::Element;
use iced::widget::{button, column, container, row, text};
use iv_domain::SortOrder;

use crate::message::Message;
use crate::state::AppState;
use crate::theme;

pub fn menu(state: &AppState) -> Option<Element<'_, Message>> {
    if !state.menu_open {
        return None;
    }

    let sort = row![
        text("排序：").size(12),
        button(text("文件名").size(12))
            .style(theme::text_button)
            .on_press(Message::SetSort(SortOrder::Name)),
        button(text("修改时间").size(12))
            .style(theme::text_button)
            .on_press(Message::SetSort(SortOrder::ModifiedNewest)),
        text(format!("当前 {}", sort_label(state.nav.sort))).size(12),
    ]
    .spacing(8)
    .align_y(iced::Center);

    let navigation = row![
        nav_button("首张 (Home)", state.nav.can_prev(), Message::FirstImage),
        nav_button("上一张 (←)", state.nav.can_prev(), Message::Navigate(-1)),
        nav_button("下一张 (→)", state.nav.can_next(), Message::Navigate(1)),
        nav_button("末张 (End)", state.nav.can_next(), Message::LastImage),
    ]
    .spacing(8)
    .align_y(iced::Center);

    let view_actions = row![
        labeled_button("实际大小 (1)", Message::ActualSize),
        labeled_button("顺时针旋转 (R)", Message::Rotate(1)),
        labeled_button("逆时针旋转 (Shift+R)", Message::Rotate(-1)),
    ]
    .spacing(8)
    .align_y(iced::Center);

    let actions = row![
        labeled_button("在资源管理器中显示", Message::Reveal),
        labeled_button("复制路径", Message::CopyPath),
        labeled_button("删除到回收站", Message::DeleteCurrent),
        labeled_button(
            if state.fullscreen {
                "退出全屏 (F11)"
            } else {
                "全屏 (F11)"
            },
            Message::ToggleFullscreen,
        ),
    ]
    .spacing(8)
    .align_y(iced::Center);

    Some(
        container(column![sort, navigation, view_actions, actions].spacing(10))
            .style(theme::panel)
            .padding(12)
            .width(iced::Fill)
            .into(),
    )
}

/// 次级菜单按钮，统一样式令牌。
fn labeled_button(label: &str, message: Message) -> Element<'_, Message> {
    button(text(label).size(12))
        .style(theme::text_button)
        .on_press(message)
        .into()
}

fn nav_button(label: &str, enabled: bool, message: Message) -> Element<'_, Message> {
    let mut btn = button(text(label).size(12)).style(theme::text_button);
    if enabled {
        btn = btn.on_press(message);
    }
    btn.into()
}

fn sort_label(order: SortOrder) -> &'static str {
    match order {
        SortOrder::Name => "文件名",
        SortOrder::ModifiedNewest => "修改时间",
    }
}
