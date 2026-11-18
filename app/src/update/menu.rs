//! 菜单动作 → Core 用例、设置变更与宿主能力。
//!
//! 菜单与工具栏、键盘走同一批消息处理函数；这里只做「动作 id → 具体用例」的分派，
//! 不重复实现业务。

use iced::Task;

use crate::menu::Action;
use crate::message::Message;
use crate::state::AppState;
use crate::state::app_state::Dialog;
use crate::update::{library, task, viewer};

pub fn apply(state: &mut AppState, action: Action) -> Task<Message> {
    match action {
        Action::Open => task::open_dialog(state.text()),
        Action::Reveal => {
            library::reveal(state);
            Task::none()
        }
        Action::CopyPath => library::copy_path(state),
        Action::Trash => library::delete_current(state),
        Action::ToggleThumbnails => {
            library::toggle_thumbnails(state);
            Task::none()
        }
        Action::ToggleSlideshow => {
            task::toggle_slideshow(state);
            Task::none()
        }
        Action::Sort(order) => library::set_sort(state, order),
        Action::ZoomIn => {
            viewer::zoom_by(state, 1.25);
            Task::none()
        }
        Action::ZoomOut => {
            viewer::zoom_by(state, 0.8);
            Task::none()
        }
        Action::Fit => {
            state.viewer.fit();
            Task::none()
        }
        Action::ActualSize => {
            viewer::actual_size(state);
            Task::none()
        }
        Action::RotateCw => {
            state.viewer.rotate(1);
            Task::none()
        }
        Action::RotateCcw => {
            state.viewer.rotate(-1);
            Task::none()
        }
        Action::FlipH => {
            state.viewer.flip_horizontal();
            Task::none()
        }
        Action::FlipV => {
            state.viewer.flip_vertical();
            Task::none()
        }
        Action::Theme(setting) => {
            state.settings.theme = setting;
            library::persist(state);
            Task::none()
        }
        Action::Language(setting) => {
            state.settings.language = setting;
            library::persist(state);
            Task::none()
        }
        Action::ToggleColorBadge => {
            state.settings.show_color_status = !state.settings.show_color_status;
            library::persist(state);
            Task::none()
        }
        Action::ToggleMetadata => {
            state.library.meta.show = !state.library.meta.show;
            Task::none()
        }
        Action::ShowShortcuts => {
            state.dialog = Some(Dialog::Shortcuts);
            Task::none()
        }
        Action::OpenLogs => open_logs(state),
    }
}

/// 在系统文件管理器里打开日志目录。
fn open_logs(state: &mut AppState) -> Task<Message> {
    let dir = iv_infra::logging::log_dir();
    if let Err(error) = iv_platform::open_folder(&dir) {
        let notice = crate::adapters::errors::file_op_error_text(&error, state.text());
        state.notice = Some(notice);
    }
    Task::none()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 工具栏图标按钮发出的 `MenuAction` 与菜单项、快捷键走同一处分派。
    #[test]
    fn rotate_and_flip_actions_reach_the_viewer() {
        let mut state = AppState::for_test(
            std::env::temp_dir().join(format!("image-view-menu-action-{}", std::process::id())),
        );
        let _ = apply(&mut state, Action::RotateCw);
        assert_eq!(state.viewer.view.rotation, 1);
        let _ = apply(&mut state, Action::RotateCcw);
        assert_eq!(state.viewer.view.rotation, 0);
        let _ = apply(&mut state, Action::FlipH);
        assert!(state.viewer.view.flip_x);
        let _ = apply(&mut state, Action::FlipV);
        assert!(state.viewer.view.flip_y);
    }
}
