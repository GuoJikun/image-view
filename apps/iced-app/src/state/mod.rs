//! Iced 侧 UI 状态。
//!
//! 只保存 ID、路径、元数据、视图参数与任务状态；像素数据由解码层持有，
//! GUI 只持有轻量的 `image::Handle`。

pub mod app_state;
pub mod library_state;
pub mod viewer_state;

pub use app_state::AppState;
