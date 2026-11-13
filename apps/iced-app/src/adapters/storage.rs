//! `iv-infra` 存储端口的文件实现。
//!
//! eframe 应用把设置写进 eframe 自带的存储，Iced 应用没有这套机制，因此在
//! GUI 适配层提供一个等价的键值存储：整个文件是一份 RON 哈希表，写操作
//! 立即落盘。设置读写、版本校验与损坏回退仍由 `iv_infra::settings` 负责。

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use iv_infra::storage::{Storage, StorageMut};

/// 覆盖配置目录的环境变量，便于 POC 阶段在临时目录里验证。
pub const CONFIG_DIR_ENV: &str = "IMAGE_VIEW_ICED_CONFIG_DIR";

const FILE_NAME: &str = "settings.ron";

#[derive(Debug)]
pub struct FileStorage {
    path: PathBuf,
    values: HashMap<String, String>,
}

impl FileStorage {
    pub fn open() -> Self {
        let path = config_dir().join(FILE_NAME);
        let values = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| ron::from_str::<HashMap<String, String>>(&raw).ok())
            .unwrap_or_else(|| {
                log::debug!(target: "image_view::storage", "设置文件不可用，使用默认值：{}", path.display());
                HashMap::new()
            });

        Self { path, values }
    }

    /// 测试用：把存储指向临时目录，避免读写用户真实配置。
    #[cfg(test)]
    pub fn open_at(path: PathBuf) -> Self {
        Self {
            path,
            values: HashMap::new(),
        }
    }

    fn flush(&self) {
        let Some(parent) = self.path.parent() else {
            return;
        };
        if let Err(error) = fs::create_dir_all(parent) {
            log::warn!(target: "image_view::storage", "无法创建配置目录：{error}");
            return;
        }
        match ron::ser::to_string(&self.values) {
            Ok(encoded) => {
                // 先写临时文件再替换，避免进程被中断时留下半份设置。
                let temp = self.path.with_extension("tmp");
                if fs::write(&temp, encoded).is_ok()
                    && let Err(error) = fs::rename(&temp, &self.path)
                {
                    log::warn!(target: "image_view::storage", "无法写入设置：{error}");
                    let _ = fs::remove_file(&temp);
                }
            }
            Err(error) => log::warn!(target: "image_view::storage", "无法序列化设置：{error}"),
        }
    }
}

impl Storage for FileStorage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.values.get(key).cloned()
    }
}

impl StorageMut for FileStorage {
    fn set_string(&mut self, key: &str, value: String) {
        self.values.insert(key.to_owned(), value);
        self.flush();
    }

    fn remove_string(&mut self, key: &str) {
        self.values.remove(key);
        self.flush();
    }
}

/// 与平台约定一致的配置目录；不引入额外的目录发现依赖。
fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os(CONFIG_DIR_ENV).filter(|value| !value.is_empty()) {
        return PathBuf::from(dir);
    }

    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    };

    base.unwrap_or_else(std::env::temp_dir)
        .join("image-view-iced")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_remove() {
        let dir = std::env::temp_dir().join(format!(
            "image-view-iced-storage-{}-{}",
            std::process::id(),
            FILE_NAME
        ));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join(FILE_NAME);

        let mut storage = FileStorage {
            path: path.clone(),
            values: HashMap::new(),
        };
        storage.set_string("a", "1".to_owned());
        assert_eq!(storage.get_string("a").as_deref(), Some("1"));

        // 直接从磁盘重新解析，验证写入的是可用的 RON。
        let raw = fs::read_to_string(&path).expect("设置文件应已写入");
        let values: HashMap<String, String> = ron::from_str(&raw).expect("设置应为合法 RON");
        assert_eq!(values.get("a").map(String::as_str), Some("1"));

        storage.remove_string("a");
        assert_eq!(storage.get_string("a"), None);
        let raw = fs::read_to_string(&path).expect("设置文件应仍存在");
        let values: HashMap<String, String> = ron::from_str(&raw).expect("设置应为合法 RON");
        assert!(!values.contains_key("a"));

        let _ = fs::remove_dir_all(&dir);
    }
}
