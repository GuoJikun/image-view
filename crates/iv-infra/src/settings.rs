//! 持久化设置：上次浏览位置、排序方式与界面偏好。
//!
//! GUI 适配器决定存储位置与文件格式；这里只负责数据结构和兼容性检查。
//! 应用把整份 [`Settings`] 以 RON 写入 `settings.ron`，窗口几何是其中的 `window` 字段。

use std::path::PathBuf;

use iv_domain::{Language, SortOrder, ThemeSetting};
use serde::{Deserialize, Serialize};

use crate::storage::{self, Storage, StorageMut};
use crate::window_state::WindowState;

/// 设置 schema 版本。不兼容变更时递增，旧版本记录一律回退默认值。
pub const CURRENT_VERSION: u32 = 1;

/// eframe 存储中的键名。
pub const STORAGE_KEY: &str = "image_view.settings";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    /// 上次浏览的目录。
    pub last_dir: Option<PathBuf>,
    /// 上次浏览的图片，位于 `last_dir` 内。
    pub last_file: Option<PathBuf>,
    pub sort: SortOrder,
    /// 状态栏是否显示“已色彩转换”标记（默认显示，与界面原有表现一致）。
    pub show_color_status: bool,
    /// 主题偏好：跟随系统 / 浅色 / 深色。
    pub theme: ThemeSetting,
    /// 是否显示底部缩略图条（默认显示）。
    pub show_thumbnails: bool,
    /// 界面语言：跟随系统 / 简体中文 / English。
    pub language: Language,
    /// 启动时自动检查更新（默认关闭，保持本地优先）。
    pub check_updates_on_startup: bool,
    /// 主窗口几何：大小、位置与最大化状态，物理像素。
    /// 缺字段的旧记录由 `#[serde(default)]` 补成「未记录」，不触发版本回退。
    pub window: WindowState,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            last_dir: None,
            last_file: None,
            sort: SortOrder::default(),
            show_color_status: true,
            theme: ThemeSetting::default(),
            show_thumbnails: true,
            language: Language::default(),
            check_updates_on_startup: false,
            window: WindowState::default(),
        }
    }
}

impl Settings {
    /// 从 RON 文本解析；损坏或版本不兼容时返回 `None`。
    pub fn from_ron(text: &str) -> Option<Self> {
        let settings: Self = match ron::from_str(text) {
            Ok(settings) => settings,
            Err(error) => {
                log::debug!(
                    target: "image_view::settings",
                    "设置内容损坏，回退默认值：{error}"
                );
                return None;
            }
        };
        if settings.version != CURRENT_VERSION {
            log::warn!(
                target: "image_view::settings",
                "设置版本 {} 与当前版本 {CURRENT_VERSION} 不兼容，回退默认值",
                settings.version
            );
            return None;
        }
        Some(settings)
    }

    /// 序列化为 RON；失败时记录日志并返回 `None`。
    pub fn to_ron(&self) -> Option<String> {
        match ron::ser::to_string(self) {
            Ok(text) => Some(text),
            Err(error) => {
                log::error!(
                    target: "image_view::settings",
                    "无法序列化设置：{error}"
                );
                None
            }
        }
    }

    /// 读取设置；存储缺失、内容损坏或版本不兼容时回退默认值，且不阻塞启动。
    pub fn load(storage: Option<&dyn Storage>) -> Self {
        let Some(storage) = storage else {
            return Self::default();
        };
        let Some(text) = storage.get_string(STORAGE_KEY) else {
            log::debug!(target: "image_view::settings", "没有可用的设置记录，使用默认值");
            return Self::default();
        };
        Self::from_ron(&text).unwrap_or_default()
    }

    pub fn save(&self, storage: &mut dyn StorageMut) {
        storage::set_value(storage, STORAGE_KEY, self);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    /// 最小化的内存存储，用来验证读写与损坏回退。
    #[derive(Default)]
    struct MemoryStorage {
        values: HashMap<String, String>,
    }

    impl Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.values.get(key).cloned()
        }
    }

    impl StorageMut for MemoryStorage {
        fn set_string(&mut self, key: &str, value: String) {
            self.values.insert(key.to_owned(), value);
        }

        fn remove_string(&mut self, key: &str) {
            self.values.remove(key);
        }
    }

    fn sample() -> Settings {
        Settings {
            version: CURRENT_VERSION,
            last_dir: Some(PathBuf::from(r"D:\photos")),
            last_file: Some(PathBuf::from(r"D:\photos\a.jpg")),
            sort: SortOrder::ModifiedNewest,
            show_color_status: true,
            theme: ThemeSetting::Dark,
            show_thumbnails: false,
            language: Language::En,
            check_updates_on_startup: true,
            window: WindowState {
                width: 1180,
                height: 800,
                x: 120,
                y: 80,
                prev_x: 100,
                prev_y: 60,
                maximized: false,
                visible: true,
                decorated: true,
                fullscreen: false,
            },
        }
    }

    #[test]
    fn memory_storage_round_trip() {
        let mut storage = MemoryStorage::default();
        sample().save(&mut storage);

        assert_eq!(Settings::load(Some(&storage)), sample());
    }

    #[test]
    fn ron_round_trip_preserves_session_and_preferences() {
        let text = sample().to_ron().expect("serialize");
        let parsed = Settings::from_ron(&text).expect("parse");
        assert_eq!(parsed, sample());
        assert_eq!(parsed.last_file, Some(PathBuf::from(r"D:\photos\a.jpg")));
        assert_eq!(parsed.sort, SortOrder::ModifiedNewest);
        assert!(!parsed.show_thumbnails);
    }

    #[test]
    fn ron_with_only_version_uses_defaults() {
        let text = format!("(version:{CURRENT_VERSION})");
        let parsed = Settings::from_ron(&text).expect("parse");
        assert_eq!(parsed.last_dir, None);
        assert_eq!(parsed.sort, SortOrder::default());
        assert!(parsed.show_thumbnails);
    }

    #[test]
    fn missing_storage_uses_defaults() {
        assert_eq!(Settings::load(None), Settings::default());
    }

    #[test]
    fn corrupt_record_falls_back_to_defaults() {
        let mut storage = MemoryStorage::default();
        storage.set_string(STORAGE_KEY, "{ not valid ron".to_owned());

        assert_eq!(Settings::load(Some(&storage)), Settings::default());
    }

    #[test]
    fn missing_window_field_uses_default() {
        // 旧版本 settings.ron 没有 window 字段；容器级 #[serde(default)] 补默认值，
        // 且不应触发版本不兼容回退（否则用户其余偏好会被清空）。
        let text = format!("(version:{CURRENT_VERSION})");
        let parsed = Settings::from_ron(&text).expect("parse");
        assert_eq!(parsed.window, WindowState::default());
        assert!(parsed.window.is_unset());
    }

    #[test]
    fn incompatible_version_falls_back_to_defaults() {
        for version in [0, CURRENT_VERSION + 1] {
            let mut storage = MemoryStorage::default();
            let settings = Settings {
                version,
                ..sample()
            };
            settings.save(&mut storage);

            assert_eq!(
                Settings::load(Some(&storage)),
                Settings::default(),
                "版本 {version} 应回退默认值"
            );
        }
    }

    #[test]
    fn missing_fields_use_defaults() {
        let mut storage = MemoryStorage::default();
        // 只写 version：其余字段由 `#[serde(default)]` 补齐。
        storage.set_string(
            STORAGE_KEY,
            format!("(version:{CURRENT_VERSION})").to_owned(),
        );

        assert_eq!(Settings::load(Some(&storage)), Settings::default());
    }
}
