//! ImageViewer 基础设施：设置与窗口状态持久化、日志。
//!
//! 不依赖 GUI；`settings` 定义含窗口几何在内的持久化数据结构，`window_state` 提供
//! 恢复判定与几何采集的纯逻辑。存储位置与窗口 API 由 GUI/宿主适配层提供。

pub mod logging;
pub mod settings;
pub mod storage;
pub mod window_state;
