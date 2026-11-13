//! ImageViewer 基础设施：设置持久化与日志。
//!
//! 不依赖 GUI；`settings` 通过 [`storage::Storage`] 端口读写键值，
//! 由 GUI/宿主提供具体实现（eframe 存储适配器在 app 内）。

pub mod logging;
pub mod settings;
pub mod storage;
