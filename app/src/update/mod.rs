//! Update：把 Message 转换为 Application 用例、视图变换与 Task。
//!
//! 这里不执行解码、目录遍历或数据库查询；所有耗时工作由 Core 的后台线程完成，
//! 结果通过 `Message::Poll` 回到状态。

pub mod library;
pub mod menu;
pub mod task;
pub mod viewer;

use iced::Task;

use crate::message::Message;
use crate::state::AppState;

pub fn update(state: &mut AppState, message: Message) -> Task<Message> {
    match message {
        // ---- 查看器 ----
        Message::ZoomAt { cursor, factor } => {
            viewer::zoom_at(state, cursor, factor);
            Task::none()
        }
        Message::ZoomBy(factor) => {
            viewer::zoom_by(state, factor);
            Task::none()
        }
        Message::PanBy(delta) => {
            viewer::pan_by(state, delta);
            Task::none()
        }
        Message::Fit => {
            state.viewer.fit();
            Task::none()
        }
        Message::ToggleActualSize => {
            viewer::toggle_actual_size(state);
            Task::none()
        }
        Message::CanvasResized(size) => {
            viewer::canvas_resized(state, size);
            Task::none()
        }
        // ---- 图库与导航 ----
        Message::Navigate(delta) => library::navigate(state, delta),
        Message::Select(index) => library::select(state, index),
        Message::ThumbScrolled {
            offset_x,
            viewport_w,
        } => {
            library::thumb_scrolled(state, offset_x, viewport_w);
            Task::none()
        }
        Message::ToggleThumbnails => {
            library::toggle_thumbnails(state);
            Task::none()
        }
        Message::MenuGroupClicked(index) => {
            // 悬停切换过来的分组，第一次点击只「落定」，再点才收起。
            if state.menu.open == Some(index) && !state.menu.hover_opened {
                state.menu.close();
            } else if state.menu.open == Some(index) {
                state.menu.hover_opened = false;
            } else {
                state.menu.open = Some(index);
                state.menu.sub = None;
                state.menu.hover = None;
                state.menu.hover_opened = false;
            }
            Task::none()
        }
        Message::MenuGroupHovered(index) => {
            // 标题只做底色反馈，光标保持箭头（见 `view/menu.rs`）。
            state.menu.bar_hover = Some(index);
            // 只有菜单已展开时，悬停其它分组才切换（与 macOS 菜单栏行为一致）。
            if state.menu.is_open() && state.menu.open != Some(index) {
                state.menu.open = Some(index);
                state.menu.sub = None;
                state.menu.hover = None;
                state.menu.hover_opened = true;
            }
            Task::none()
        }
        Message::MenuGroupLeft(index) => {
            // 离开事件可能晚于下一个标题的进入事件，只清自己这一项。
            if state.menu.bar_hover == Some(index) {
                state.menu.bar_hover = None;
            }
            Task::none()
        }
        Message::MenuOpened { group, sub } => {
            state.menu.open = Some(group);
            state.menu.sub = sub;
            state.menu.hover = None;
            state.menu.hover_opened = false;
            Task::none()
        }
        Message::MenuSubmenuToggled(index) => {
            state.menu.sub = if state.menu.sub == Some(index) {
                None
            } else {
                Some(index)
            };
            state.menu.hover = None;
            Task::none()
        }
        Message::MenuRowHovered(hover) => {
            // 悬停到父面板的其它行时收起子菜单（与原生菜单一致）。
            if hover.child.is_none() && state.menu.sub != Some(hover.entry) {
                state.menu.sub = None;
            }
            state.menu.hover = Some(hover);
            Task::none()
        }
        Message::MenuRowLeft(hover) => {
            // 离开事件可能晚于下一行的进入事件到达，因此只清自己这一行。
            if state.menu.hover == Some(hover) {
                state.menu.hover = None;
            }
            Task::none()
        }
        Message::MenuClose => {
            state.menu.close();
            Task::none()
        }
        Message::MenuAction(action) => {
            let task = menu::apply(state, action);
            state.menu.close();
            task
        }
        Message::CloseDialog => {
            state.dialog = None;
            Task::none()
        }
        Message::Noop => Task::none(),
        Message::ToggleSlideshow => {
            task::toggle_slideshow(state);
            Task::none()
        }
        Message::Reveal => {
            library::reveal(state);
            Task::none()
        }
        Message::CopyPath => library::copy_path(state),

        // ---- 异步结果与系统事件 ----
        Message::OpenDialog => task::open_dialog(state.text()),
        Message::DialogPicked(path) => match path {
            Some(path) => library::open_path(state, path),
            None => Task::none(),
        },
        Message::Event(event) => task::handle_event(state, event),
        Message::WindowReady(id) => {
            state.window_id = Some(id);
            Task::none()
        }
        Message::ApplyWindowMode(mode) => task::apply_window_mode(state, mode),
        Message::DragWindow => match state.window_id {
            Some(id) => iced::window::drag(id),
            None => Task::none(),
        },
        Message::ToggleMaximize => match state.window_id {
            Some(id) => iced::window::toggle_maximize(id),
            None => Task::none(),
        },
        Message::MinimizeWindow => match state.window_id {
            Some(id) => iced::window::minimize(id, true),
            None => Task::none(),
        },
        Message::CloseWindow => match state.window_id {
            Some(id) => iced::window::close(id),
            None => Task::none(),
        },
        Message::CaptionHovered(caption) => {
            state.caption_hover = Some(caption);
            Task::none()
        }
        Message::CaptionLeft(caption) => {
            // 只清自己：离开旧按钮的退出事件可能晚于进入新按钮的进入事件。
            if state.caption_hover == Some(caption) {
                state.caption_hover = None;
            }
            Task::none()
        }
        Message::Poll => task::poll(state),
        Message::Dismiss => {
            state.notice = None;
            state.viewer.error = None;
            state.dialog = None;
            state.menu.close();
            Task::none()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Caption;

    fn state() -> AppState {
        let dir = std::env::temp_dir().join(format!("iv-caption-{}", std::process::id()));
        AppState::for_test(dir)
    }

    /// caption 相关的消息都返回空任务，测试里显式丢弃。
    fn send(state: &mut AppState, message: Message) {
        let _ = update(state, message);
    }

    #[test]
    fn hovering_caption_tracks_and_clears_the_touched_button() {
        let mut state = state();

        send(&mut state, Message::CaptionHovered(Caption::Close));
        assert_eq!(state.caption_hover, Some(Caption::Close));

        // 离开另一颗按钮不能把当前悬停清掉（退出事件可能晚于进入事件）。
        send(&mut state, Message::CaptionLeft(Caption::Minimize));
        assert_eq!(state.caption_hover, Some(Caption::Close));

        send(&mut state, Message::CaptionLeft(Caption::Close));
        assert_eq!(state.caption_hover, None);
    }

    #[test]
    fn caption_hover_resets_when_cursor_leaves_or_window_unfocuses() {
        let mut state = state();

        send(&mut state, Message::CaptionHovered(Caption::Close));
        send(
            &mut state,
            Message::Event(iced::Event::Mouse(iced::mouse::Event::CursorLeft)),
        );
        assert_eq!(state.caption_hover, None, "指针移出窗口要复位");

        send(&mut state, Message::CaptionHovered(Caption::Close));
        send(
            &mut state,
            Message::Event(iced::Event::Window(iced::window::Event::Unfocused)),
        );
        assert_eq!(state.caption_hover, None, "窗口失焦要复位");
    }
}
