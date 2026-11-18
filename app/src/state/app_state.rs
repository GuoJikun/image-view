//! 顶层应用状态：导航用例、查看器、图库侧状态与持久化设置。

use std::path::PathBuf;
use std::time::Instant;

use iced::Task;
use iv_application::FolderNav;
use iv_infra::settings::Settings;
use iv_infra::window_state::WindowState;

use crate::adapters::storage::FileStorage;
use crate::i18n::{self, Text};
use crate::menu::MenuState;
use crate::message::{Caption, CloseQueryPart, Message};
use crate::state::library_state::LibraryState;
use crate::state::viewer_state::ViewerState;

/// 幻灯片自动前进间隔。
pub const SLIDESHOW_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3);

/// 关窗前待凑齐的窗口状态。
///
/// Iced 0.14 没有最大化 / 最小化事件，只能在关闭时查询；位置、尺寸、全屏与两个
/// 标志各查一次，全部到齐后再统一采集几何并落盘，避免多次写 `settings.ron`。
#[derive(Debug, Default)]
pub struct CloseQuery {
    pub maximized: Option<bool>,
    pub minimized: Option<bool>,
    pub fullscreen: Option<bool>,
    pub size: Option<iced::Size>,
    pub position: Option<Option<iced::Point>>,
}

impl CloseQuery {
    pub fn absorb(&mut self, part: CloseQueryPart) {
        match part {
            CloseQueryPart::Maximized(value) => self.maximized = Some(value),
            CloseQueryPart::Minimized(value) => self.minimized = Some(value),
            CloseQueryPart::Fullscreen(value) => self.fullscreen = Some(value),
            CloseQueryPart::Size(size) => self.size = Some(size),
            CloseQueryPart::Position(point) => self.position = Some(point),
        }
    }

    pub fn complete(&self) -> bool {
        self.maximized.is_some()
            && self.minimized.is_some()
            && self.fullscreen.is_some()
            && self.size.is_some()
            && self.position.is_some()
    }
}

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
    /// 自绘标题栏里鼠标停留的 caption 按钮；图标配色随它变化。
    pub caption_hover: Option<Caption>,
    pub slideshow: bool,
    pub last_advance: Instant,
    pub scale_factor: f32,
    /// DPI 是否已由 `WindowReady` 查询过；采集位置需要逻辑→物理换算。
    pub scale_known: bool,
    /// 启动时保存的窗口几何，等 DPI 到手后应用一次。
    pub pending_window: Option<WindowState>,
    /// 恢复几何后仍会回抛 `Moved`；用几次轮询的宽限期忽略这些事件。
    pub restore_grace: u8,
    /// 最大化状态：Iced 没有对应事件，由显式动作与关闭查询维护。
    pub maximized: bool,
    pub minimized: bool,
    /// 关闭流程进行中；五项查询全部返回后才采集并关窗。
    pub closing: Option<CloseQuery>,
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

        let window = crate::startup_window();
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
            caption_hover: None,
            slideshow: false,
            last_advance: Instant::now(),
            scale_factor: 1.0,
            scale_known: false,
            pending_window: window.clone(),
            restore_grace: 0,
            maximized: window.as_ref().is_some_and(|window| window.maximized),
            minimized: false,
            closing: None,
            window_id: None,
            drag_hover: false,
            fullscreen: window.as_ref().is_some_and(|window| window.fullscreen),
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
            caption_hover: None,
            slideshow: false,
            last_advance: Instant::now(),
            scale_factor: 1.0,
            scale_known: true,
            pending_window: None,
            restore_grace: 0,
            maximized: false,
            minimized: false,
            closing: None,
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
