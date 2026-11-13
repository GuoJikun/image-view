//! 键值存储端口与 RON 序列化辅助。
//!
//! `eframe` 的 `Storage` 是同样的键值形态；app 用一个轻量适配器把它接到
//! 这里，核心设置层因此不需要依赖 eframe。

use serde::Serialize;
use serde::de::DeserializeOwned;

/// 宿主提供的只读键值存储。
pub trait Storage {
    fn get_string(&self, key: &str) -> Option<String>;
}

/// 可写入的键值存储。
pub trait StorageMut: Storage {
    fn set_string(&mut self, key: &str, value: String);
    fn remove_string(&mut self, key: &str);
}

/// 读取并反序列化；内容损坏时返回 `None`。
pub fn get_value<T: DeserializeOwned>(storage: &dyn Storage, key: &str) -> Option<T> {
    let value = storage.get_string(key)?;
    match ron::from_str(&value) {
        Ok(value) => Some(value),
        Err(error) => {
            log::debug!(target: "image_view::settings", "Failed to decode RON: {error}");
            None
        }
    }
}

/// 序列化并写入；序列化失败时只记录日志。
pub fn set_value<T: Serialize>(storage: &mut dyn StorageMut, key: &str, value: &T) {
    match ron::ser::to_string(value) {
        Ok(string) => storage.set_string(key, string),
        Err(error) => log::error!(
            target: "image_view::settings",
            "eframe failed to encode data using ron: {error}"
        ),
    }
}
