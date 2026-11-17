//! Update：把 Message 转换为 Application 用例、视图变换与 Task。
//!
//! 这里不执行解码、目录遍历或数据库查询；所有耗时工作由 Core 的后台线程完成，
//! 结果通过 `Message::Poll` 回到状态。

pub mod library;
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
        Message::ActualSize => {
            viewer::actual_size(state);
            Task::none()
        }
        Message::ToggleActualSize => {
            viewer::toggle_actual_size(state);
            Task::none()
        }
        Message::Rotate(steps) => {
            state.viewer.rotate(steps);
            Task::none()
        }
        Message::CanvasResized(size) => {
            viewer::canvas_resized(state, size);
            Task::none()
        }
        // ---- 图库与导航 ----
        Message::Navigate(delta) => library::navigate(state, delta),
        Message::Select(index) => library::select(state, index),
        Message::FirstImage => library::first(state),
        Message::LastImage => library::last(state),
        Message::SetSort(order) => library::set_sort(state, order),
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
        Message::ToggleMetadata => {
            state.library.meta.show = !state.library.meta.show;
            Task::none()
        }
        Message::ToggleMenu => {
            state.menu_open = !state.menu_open;
            Task::none()
        }
        Message::ToggleSlideshow => {
            task::toggle_slideshow(state);
            Task::none()
        }
        Message::DeleteCurrent => library::delete_current(state),
        Message::Reveal => {
            library::reveal(state);
            Task::none()
        }
        Message::CopyPath => library::copy_path(state),

        // ---- 异步结果与系统事件 ----
        Message::OpenDialog => task::open_dialog(),
        Message::DialogPicked(path) => match path {
            Some(path) => library::open_path(state, path),
            None => Task::none(),
        },
        Message::Event(event) => task::handle_event(state, event),
        Message::WindowReady(id) => {
            state.window_id = Some(id);
            Task::none()
        }
        Message::ToggleFullscreen => task::toggle_fullscreen(state),
        Message::ApplyWindowMode(mode) => task::apply_window_mode(state, mode),
        Message::Poll => task::poll(state),
        Message::Dismiss => {
            state.notice = None;
            state.viewer.error = None;
            state.menu_open = false;
            Task::none()
        }
    }
}
