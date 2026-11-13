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
        button(text("文件名").size(12)).on_press(Message::SetSort(SortOrder::Name)),
        button(text("修改时间").size(12)).on_press(Message::SetSort(SortOrder::ModifiedNewest)),
        text(format!("当前 {}", sort_label(state.nav.sort))).size(12),
    ]
    .spacing(8)
    .align_y(iced::Center);

    let navigation = row![
        button(text("首张 (Home)").size(12))
            .on_press_maybe(state.nav.can_prev().then_some(Message::FirstImage)),
        button(text("上一张 (←)").size(12))
            .on_press_maybe(state.nav.can_prev().then_some(Message::Navigate(-1))),
        button(text("下一张 (→)").size(12))
            .on_press_maybe(state.nav.can_next().then_some(Message::Navigate(1))),
        button(text("末张 (End)").size(12))
            .on_press_maybe(state.nav.can_next().then_some(Message::LastImage)),
    ]
    .spacing(8)
    .align_y(iced::Center);

    let actions = row![
        button(text("在资源管理器中显示").size(12)).on_press(Message::Reveal),
        button(text("复制路径").size(12)).on_press(Message::CopyPath),
        button(text("删除到回收站").size(12)).on_press(Message::DeleteCurrent),
        button(
            text(if state.fullscreen {
                "退出全屏"
            } else {
                "全屏 (F11)"
            })
            .size(12)
        )
        .on_press(Message::ToggleFullscreen),
    ]
    .spacing(8)
    .align_y(iced::Center);

    Some(
        container(column![sort, navigation, actions].spacing(10))
            .style(theme::panel)
            .padding(10)
            .width(iced::Fill)
            .into(),
    )
}

fn sort_label(order: SortOrder) -> &'static str {
    match order {
        SortOrder::Name => "文件名",
        SortOrder::ModifiedNewest => "修改时间",
    }
}
