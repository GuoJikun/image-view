//! 系统事件、异步任务结果与周期内轮询。
//!
//! 解码线程的完成通知通过固定周期的 `Poll` 收拢到 Update；POC 阶段先保证
//! 「任何情况下消息数量有上界」，后续可换成事件驱动的 Subscription。

use std::time::Instant;

use iced::Task;
use iced::keyboard::{self, Key, Modifiers, key::Named};
use iv_domain::SUPPORTED_EXTENSIONS;

use crate::adapters::errors;
use crate::i18n::Text;
use crate::message::Message;
use crate::state::AppState;
use crate::state::app_state::Dialog;
use crate::state::app_state::SLIDESHOW_INTERVAL;
use crate::update::{library, viewer, window};

/// 后台线程的轮询周期。解码线程本身不阻塞 UI，这里只决定结果回到 UI 的延迟。
pub const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

pub fn handle_event(state: &mut AppState, event: iced::Event) -> Task<Message> {
    match event {
        iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
            key_pressed(state, key, modifiers)
        }
        iced::Event::Window(iced::window::Event::FileHovered(_)) => {
            state.drag_hover = true;
            Task::none()
        }
        iced::Event::Window(iced::window::Event::FilesHoveredLeft) => {
            state.drag_hover = false;
            Task::none()
        }
        iced::Event::Window(iced::window::Event::FileDropped(path)) => {
            state.drag_hover = false;
            library::open_path(state, path)
        }
        iced::Event::Window(iced::window::Event::Rescaled(factor)) => {
            viewer::scale_factor(state, factor);
            state.scale_known = true;
            Task::none()
        }
        iced::Event::Window(iced::window::Event::Moved(point)) => {
            window::moved(state, point);
            Task::none()
        }
        iced::Event::Window(iced::window::Event::Resized(size)) => {
            window::resized(state, size);
            Task::none()
        }
        iced::Event::Window(iced::window::Event::CloseRequested) => window::begin_close(state),
        // 从任务栏还原后最小化标志失效；失焦不等于最小化，不能在这里改。
        iced::Event::Window(iced::window::Event::Focused) => {
            state.minimized = false;
            Task::none()
        }
        // 指针移出窗口或窗口失焦时，caption 按钮收不到 `on_exit`（鼠标直接移出边缘、
        // Alt+Tab 切走都会漏），必须主动清掉，否则关闭按钮会停在「红底 + 白图标」状态。
        iced::Event::Mouse(iced::mouse::Event::CursorLeft) => {
            state.caption_hover = None;
            Task::none()
        }
        iced::Event::Window(iced::window::Event::Unfocused) => {
            state.caption_hover = None;
            Task::none()
        }
        _ => Task::none(),
    }
}

fn key_pressed(state: &mut AppState, key: Key, modifiers: Modifiers) -> Task<Message> {
    // 带修饰键的组合与菜单里标注的快捷键保持一致。
    if modifiers.command() {
        return match &key {
            Key::Character(text)
                if text.as_str().eq_ignore_ascii_case("c") && modifiers.shift() =>
            {
                library::copy_path(state)
            }
            Key::Character(text) if text.as_str().eq_ignore_ascii_case("o") => {
                open_dialog(state.text())
            }
            Key::Character(text) if matches!(text.as_str(), "=" | "+") => {
                viewer::zoom_by(state, 1.25);
                Task::none()
            }
            Key::Character(text) if matches!(text.as_str(), "-" | "_") => {
                viewer::zoom_by(state, 0.8);
                Task::none()
            }
            Key::Character(text) if text.as_str() == "0" => {
                state.viewer.fit();
                Task::none()
            }
            Key::Character(text) if text.as_str() == "1" => {
                viewer::actual_size(state);
                Task::none()
            }
            Key::Character(text) if text.as_str() == "/" => {
                state.menu.close();
                state.dialog = Some(Dialog::Shortcuts);
                Task::none()
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
            // 逐层退出：菜单 / 排序下拉 → 弹窗 → 提示条。
            if state.menu.is_open() || state.menu.sort_open {
                state.menu.close();
            } else if state.dialog.is_some() {
                state.dialog = None;
            } else {
                state.notice = None;
            }
            Task::none()
        }
        Key::Named(Named::Space) => {
            toggle_playback(state);
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
            "h" | "H" => {
                state.viewer.flip_horizontal();
                Task::none()
            }
            "v" | "V" => {
                state.viewer.flip_vertical();
                Task::none()
            }
            _ => Task::none(),
        },
        _ => Task::none(),
    }
}

pub fn open_dialog(t: &Text) -> Task<Message> {
    let title = t.menu_open;
    let filter = t.dialog_images;
    Task::perform(
        async move {
            rfd::AsyncFileDialog::new()
                .set_title(title)
                .add_filter(filter, SUPPORTED_EXTENSIONS)
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

/// Space / 工具栏播放键：有动画时切动画播放，否则切幻灯片。
pub fn toggle_playback(state: &mut AppState) {
    if !state.viewer.toggle_animation() {
        toggle_slideshow(state);
    }
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
    state.set_fullscreen(next == iced::window::Mode::Fullscreen);
    // 全屏时自绘标题栏（含菜单栏）隐藏，菜单必须收起，否则会浮在画布上。
    state.menu.close();
    match state.window_id {
        Some(id) => iced::window::set_mode::<Message>(id, next),
        None => Task::none(),
    }
}

/// 排空主图与缩略图线程的结果；解码结果只接受当前 generation。
pub fn poll(state: &mut AppState) -> Task<Message> {
    // 恢复几何后的移动事件会晚于动作本身到达，用几次轮询的宽限期忽略。
    if state.restore_grace > 0 {
        state.restore_grace -= 1;
    }

    let text = state.text();
    if state.viewer.poll(text)
        && let Some(image) = state.viewer.image.clone()
    {
        state.library.meta.absorb(&image);
    }

    // 动画降级为静态首帧时，用顶部横幅说明原因（与 `main` 一致）。
    if let Some(reason) = state.viewer.degrade.take() {
        state.notice = Some(errors::degrade_text(&reason, state.text()));
    }

    if state.library.thumbs.poll() {
        library::request_visible_thumbnails(state);
    }

    // 动画正在播放时只推进帧，不叠加幻灯片翻页（与 `main` 的 playbackActive 一致）。
    let animation_playing = state
        .viewer
        .animation
        .as_ref()
        .is_some_and(|animation| animation.playing);
    if state.slideshow
        && !animation_playing
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
    fn h_and_v_keys_toggle_flips() {
        let mut state = AppState::for_test(
            std::env::temp_dir().join(format!("image-view-key-flip-{}", std::process::id())),
        );
        let _ = key_pressed(&mut state, Key::Character("h".into()), Modifiers::NONE);
        assert!(state.viewer.view.flip_x);
        let _ = key_pressed(&mut state, Key::Character("H".into()), Modifiers::NONE);
        assert!(!state.viewer.view.flip_x);
        let _ = key_pressed(&mut state, Key::Character("v".into()), Modifiers::NONE);
        assert!(state.viewer.view.flip_y);
    }

    #[test]
    fn poll_interval_is_bounded() {
        assert!(POLL_INTERVAL.as_millis() >= 16);
        assert!(POLL_INTERVAL.as_millis() <= 250);
    }
}
