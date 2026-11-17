//! 系统事件、异步任务结果与周期内轮询。
//!
//! 解码线程的完成通知通过固定周期的 `Poll` 收拢到 Update；POC 阶段先保证
//! 「任何情况下消息数量有上界」，后续可换成事件驱动的 Subscription。

use std::time::Instant;

use iced::Task;
use iced::keyboard::{self, Key, Modifiers, key::Named};
use iv_domain::SUPPORTED_EXTENSIONS;

use crate::message::Message;
use crate::state::AppState;
use crate::state::app_state::SLIDESHOW_INTERVAL;
use crate::update::{library, viewer};

/// 后台线程的轮询周期。解码线程本身不阻塞 UI，这里只决定结果回到 UI 的延迟。
pub const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

pub fn handle_event(state: &mut AppState, event: iced::Event) -> Task<Message> {
    match event {
        iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
            key_pressed(state, key, modifiers)
        }
        iced::Event::Window(iced::window::Event::FileDropped(path)) => {
            library::open_path(state, path)
        }
        iced::Event::Window(iced::window::Event::Rescaled(factor)) => {
            viewer::scale_factor(state, factor);
            Task::none()
        }
        iced::Event::Window(iced::window::Event::CloseRequested) => {
            library::persist(state);
            Task::none()
        }
        _ => Task::none(),
    }
}

fn key_pressed(state: &mut AppState, key: Key, modifiers: Modifiers) -> Task<Message> {
    if modifiers.command() {
        return match &key {
            Key::Character(text) if text.as_str().eq_ignore_ascii_case("o") => open_dialog(),
            Key::Character(text)
                if text.as_str().eq_ignore_ascii_case("c") && modifiers.shift() =>
            {
                library::copy_path(state)
            }
            _ => Task::none(),
        };
    }

    match key {
        Key::Named(Named::ArrowLeft) => library::navigate(state, -1),
        Key::Named(Named::ArrowRight) => library::navigate(state, 1),
        Key::Named(Named::Home) => library::first(state),
        Key::Named(Named::End) => library::last(state),
        Key::Named(Named::Delete) => library::delete_current(state),
        Key::Named(Named::F11) => toggle_fullscreen(state),
        Key::Named(Named::Escape) => {
            state.menu_open = false;
            state.notice = None;
            Task::none()
        }
        Key::Named(Named::Space) => {
            toggle_slideshow(state);
            Task::none()
        }
        Key::Character(text) => match text.as_str() {
            "+" | "=" => {
                viewer::zoom_by(state, 1.25);
                Task::none()
            }
            "-" | "_" => {
                viewer::zoom_by(state, 0.8);
                Task::none()
            }
            "0" => {
                state.viewer.fit();
                Task::none()
            }
            "1" => {
                viewer::actual_size(state);
                Task::none()
            }
            "f" | "F" => {
                viewer::toggle_actual_size(state);
                Task::none()
            }
            "r" => {
                state.viewer.rotate(1);
                Task::none()
            }
            "R" => {
                state.viewer.rotate(-1);
                Task::none()
            }
            _ => Task::none(),
        },
        _ => Task::none(),
    }
}

pub fn open_dialog() -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .set_title("打开图片")
                .add_filter("图片", SUPPORTED_EXTENSIONS)
                .pick_file()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
        Message::DialogPicked,
    )
}

pub fn toggle_slideshow(state: &mut AppState) {
    state.slideshow = !state.slideshow;
    state.last_advance = Instant::now();
}

pub fn toggle_fullscreen(state: &AppState) -> Task<Message> {
    match state.window_id {
        Some(id) => iced::window::mode(id).map(Message::ApplyWindowMode),
        None => Task::none(),
    }
}

pub fn apply_window_mode(state: &mut AppState, mode: iced::window::Mode) -> Task<Message> {
    let next = if mode == iced::window::Mode::Fullscreen {
        iced::window::Mode::Windowed
    } else {
        iced::window::Mode::Fullscreen
    };
    state.fullscreen = next == iced::window::Mode::Fullscreen;
    match state.window_id {
        Some(id) => iced::window::set_mode::<Message>(id, next),
        None => Task::none(),
    }
}

/// 排空主图与缩略图线程的结果；解码结果只接受当前 generation。
pub fn poll(state: &mut AppState) -> Task<Message> {
    if state.viewer.poll()
        && let Some(image) = state.viewer.image.clone()
    {
        state.library.meta.absorb(&image);
    }

    if state.library.thumbs.poll() {
        library::request_visible_thumbnails(state);
    }

    if state.slideshow
        && !state.viewer.loading()
        && state.last_advance.elapsed() >= SLIDESHOW_INTERVAL
    {
        state.last_advance = Instant::now();
        if state.nav.can_next() {
            return library::navigate(state, 1);
        }
        state.slideshow = false;
    }

    Task::none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_interval_is_bounded() {
        assert!(POLL_INTERVAL.as_millis() >= 16);
        assert!(POLL_INTERVAL.as_millis() <= 250);
    }
}
