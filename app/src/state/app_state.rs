//! 顶层应用状态：导航用例、查看器、图库侧状态与持久化设置。

use std::path::PathBuf;
use std::time::Instant;

use iced::Task;
use iv_application::FolderNav;
use iv_infra::settings::Settings;

use crate::adapters::storage::FileStorage;
use crate::i18n::{self, Text};
use crate::menu::MenuState;
use crate::message::Message;
use crate::state::library_state::LibraryState;
use crate::state::viewer_state::ViewerState;

/// 幻灯片自动前进间隔。
pub const SLIDESHOW_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3);

/// 当前打开的模态弹窗。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialog {
    Shortcuts,
}

pub struct AppState {
    /// Application 层用例状态（当前目录、列表、下标、排序）。
    pub nav: FolderNav,
    pub viewer: ViewerState,
    pub library: LibraryState,
    pub settings: Settings,
    pub storage: FileStorage,

    /// 系统语言标签；启动时读一次，供「跟随系统」解析语言。
    pub system_locale: Option<String>,
    /// 文件操作/系统能力的用户可读提示，与解码错误分开。
    pub notice: Option<String>,
    pub menu: MenuState,
    pub dialog: Option<Dialog>,
    pub slideshow: bool,
    pub last_advance: Instant,
    pub scale_factor: f32,
    pub window_id: Option<iced::window::Id>,
    pub fullscreen: bool,
    /// 文件正被拖入窗口：画布叠加强调色提示「松开以打开」。
    pub drag_hover: bool,
}

impl AppState {
    /// 启动：优先打开命令行路径，否则恢复上次会话。
    pub fn boot(open_path: Option<PathBuf>) -> (Self, Task<Message>) {
        let storage = FileStorage::open();
        let settings = Settings::load(Some(&storage));

        let mut nav = FolderNav {
            sort: settings.sort,
            ..FolderNav::default()
        };

        let startup = match open_path {
            Some(path) => Some(path),
            None => settings
                .last_dir
                .as_deref()
                .and_then(|dir| nav.restore(dir, settings.last_file.as_deref(), settings.sort)),
        };

        let mut library = LibraryState::default();
        library.thumbs.show = settings.show_thumbnails;

        let system_locale = iv_platform::system_locale();

        let mut state = Self {
            nav,
            viewer: ViewerState::new(),
            library,
            settings: settings.clone(),
            storage,
            system_locale,
            notice: None,
            menu: MenuState::default(),
            dialog: None,
            slideshow: false,
            last_advance: Instant::now(),
            scale_factor: 1.0,
            window_id: None,
            drag_hover: false,
            fullscreen: false,
        };

        let task = match startup {
            Some(path) => crate::update::library::open_path(&mut state, path),
            None => {
                crate::update::library::request_visible_thumbnails(&mut state);
                Task::none()
            }
        };

        (state, task)
    }

    /// 当前图片路径；用于标题栏、状态栏与文件操作。
    pub fn current_path(&self) -> Option<PathBuf> {
        self.nav.current().map(std::path::Path::to_path_buf)
    }

    /// 显示比例（0.01~32），Fit 模式按画布尺寸实时计算。
    pub fn zoom(&self) -> Option<f32> {
        self.viewer.effective_zoom(self.scale_factor)
    }

    /// 当前界面文案表；语言偏好与系统标签决定中/英文。
    pub fn text(&self) -> &'static Text {
        i18n::for_language(self.settings.language, self.system_locale.as_deref())
    }
}

#[cfg(test)]
impl AppState {
    /// 测试用：不读取用户配置，把设置写到 `config_dir` 下。
    pub fn for_test(config_dir: PathBuf) -> Self {
        Self {
            nav: FolderNav::default(),
            viewer: ViewerState::new(),
            library: LibraryState::default(),
            settings: Settings::default(),
            storage: FileStorage::open_at(config_dir.join("settings.ron")),
            // 测试固定系统语言，避免用例结果随执行环境变化。
            system_locale: Some("zh-CN".to_owned()),
            notice: None,
            menu: MenuState::default(),
            dialog: None,
            slideshow: false,
            last_advance: Instant::now(),
            scale_factor: 1.0,
            window_id: None,
            drag_hover: false,
            fullscreen: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slideshow_interval_is_three_seconds() {
        assert_eq!(SLIDESHOW_INTERVAL.as_secs(), 3);
    }
}
