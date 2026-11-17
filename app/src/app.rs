//! Iced Application 装配点：boot / update / view / subscription / title / theme。
//!
//! 这里不实现业务；所有消息都转给 `update` 模块，所有 Core 能力保持不变，
//! 因此替换 GUI 时无需改动 Domain 与 Application。

use iced::{Element, Subscription, Task, Theme};
use iv_domain::ThemeSetting;

use crate::message::Message;
use crate::state::AppState;
use crate::update::task::POLL_INTERVAL;

pub fn boot() -> (AppState, Task<Message>) {
    AppState::boot(crate::startup_path())
}

pub fn update(state: &mut AppState, message: Message) -> Task<Message> {
    crate::update::update(state, message)
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    crate::view::view(state)
}

pub fn subscription(_state: &AppState) -> Subscription<Message> {
    Subscription::batch([
        // widget 未捕获的键盘、拖放、窗口缩放事件。
        iced::event::listen().map(Message::Event),
        // 窗口 Id 用于全屏切换。
        iced::window::open_events().map(Message::WindowReady),
        // 后台解码线程与缩略图线程的结果以固定周期收拢，消息量有上界。
        iced::time::every(POLL_INTERVAL).map(|_| Message::Poll),
    ])
}

pub fn title(state: &AppState) -> String {
    match state.current_path() {
        Some(path) => format!(
            "{} — image-view (Iced POC)",
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string())
        ),
        None => "image-view (Iced POC)".to_owned(),
    }
}

pub fn theme(state: &AppState) -> Theme {
    match state.settings.theme {
        ThemeSetting::Light => Theme::Light,
        ThemeSetting::Dark => Theme::Dark,
        // POC 阶段「跟随系统」使用深色：Iced 0.14 的 `Theme` 没有系统主题
        // 探测入口，而本应用的画布固定深色，深色外壳视觉更统一。
        ThemeSetting::System => Theme::Dark,
    }
}
