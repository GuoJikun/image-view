//! GUI 适配器：把 Core 的结果翻译为 Iced 可用的资源、文案与持久化。
//!
//! 这一层是唯一允许出现 `iced` 类型的适配代码；Domain / Application 保持不变。

pub mod errors;
pub mod preview;
pub mod storage;
