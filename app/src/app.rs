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
    let (state, task) = AppState::boot(crate::startup_path());
    // 系统主题是异步查询（Iced 0.14 只有 Task 入口，没有同步 API）。
    // 与启动时的打开/缩略图任务并行，返回前 `system_theme` 保持 None。
    let theme_task = iced::system::theme().map(Message::SystemTheme);
    (state, Task::batch([task, theme_task]))
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
        // 用户在系统设置里切换明暗时实时更新，与 `main` 的 prefers-color-scheme 一致。
        // 始终订阅（不止「跟随系统」），这样手动主题期间改了系统主题、再切回跟随
        // 时拿到的仍是最新值；系统主题切换本身很罕见，额外一次重绘可忽略。
        iced::system::theme_changes().map(Message::SystemTheme),
    ])
}

pub fn title(state: &AppState) -> String {
    let app_name = state.text().app_name;
    match state.current_path() {
        Some(path) => format!(
            "{} — {app_name}",
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string())
        ),
        None => app_name.to_owned(),
    }
}

/// 把 Iced 报告的系统明暗映射成实际主题。
///
/// `Mode::None` 表示平台未给出明确偏好（如部分 Linux 桌面），按 CSS
/// `prefers-color-scheme` 的默认值回退到浅色，与 `main` 一致。
pub fn theme_for_system(mode: iced::theme::Mode) -> Theme {
    match mode {
        iced::theme::Mode::Dark => Theme::Dark,
        iced::theme::Mode::Light | iced::theme::Mode::None => Theme::Light,
    }
}

pub fn theme(state: &AppState) -> Theme {
    match state.settings.theme {
        ThemeSetting::Light => Theme::Light,
        ThemeSetting::Dark => Theme::Dark,
        // 查询完成前 system_theme 为 None，先按浅色显示，异步结果到达后自动纠正。
        ThemeSetting::System => theme_for_system(state.system_theme),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_theme_maps_light_and_dark() {
        assert_eq!(theme_for_system(iced::theme::Mode::Light), Theme::Light);
        assert_eq!(theme_for_system(iced::theme::Mode::Dark), Theme::Dark);
    }

    #[test]
    fn system_theme_none_falls_back_to_light() {
        // 与 CSS prefers-color-scheme 默认值一致：平台未报告时按浅色。
        assert_eq!(theme_for_system(iced::theme::Mode::None), Theme::Light);
    }

    #[test]
    fn view_theme_matches_app_theme_under_system_light() {
        // 回归：view 层曾有一份平行实现把 System 当 Dark，导致浅色系统下
        // Iced 画浅色背景、菜单文字却取深色色板（白底浅字）。两处必须永远一致。
        let mut state = AppState::for_test(std::env::temp_dir());
        state.settings.theme = ThemeSetting::System;
        state.system_theme = iced::theme::Mode::Light;
        assert_eq!(theme(&state), Theme::Light);
        assert_eq!(crate::view::viewer::theme_of(&state), theme(&state));
    }
}
