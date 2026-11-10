//! 持久化设置：上次浏览位置、排序方式与将来的偏好项。
//!
//! 窗口大小/位置/最大化交给 eframe 的 `persist_window`；这里只保存应用级状态。
//! 两者共用 eframe 的存储文件（Windows：`%APPDATA%\image-view\data\app.ron`）。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::folder::SortOrder;

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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            last_dir: None,
            last_file: None,
            sort: SortOrder::default(),
        }
    }
}

impl Settings {
    /// 读取设置；存储缺失、内容损坏或版本不兼容时回退默认值，且不阻塞启动。
    pub fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        let Some(storage) = storage else {
            return Self::default();
        };
        // 记录缺失或反序列化失败时 `get_value` 返回 `None`。
        let Some(settings) = eframe::get_value::<Self>(storage, STORAGE_KEY) else {
            log::debug!("没有可用的设置记录，使用默认值");
            return Self::default();
        };

        if settings.version != CURRENT_VERSION {
            log::warn!(
                "设置版本 {} 与当前版本 {CURRENT_VERSION} 不兼容，回退默认值",
                settings.version
            );
            return Self::default();
        }

        settings
    }

    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, self);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use eframe::Storage;

    use super::*;

    /// 最小化的内存存储，用来验证读写与损坏回退。
    #[derive(Default)]
    struct MemoryStorage {
        values: HashMap<String, String>,
    }

    impl eframe::Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.values.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.values.insert(key.to_owned(), value);
        }

        fn remove_string(&mut self, key: &str) {
            self.values.remove(key);
        }

        fn flush(&mut self) {}
    }

    fn sample() -> Settings {
        Settings {
            version: CURRENT_VERSION,
            last_dir: Some(PathBuf::from(r"D:\photos")),
            last_file: Some(PathBuf::from(r"D:\photos\a.jpg")),
            sort: SortOrder::ModifiedNewest,
        }
    }

    #[test]
    fn settings_round_trip() {
        let mut storage = MemoryStorage::default();
        sample().save(&mut storage);

        assert_eq!(Settings::load(Some(&storage)), sample());
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
