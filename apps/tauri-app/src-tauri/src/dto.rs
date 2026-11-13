//! Command/Event 的 DTO 与稳定错误码。
//!
//! 前端只依赖这些轻量结构；像素数据不进入 DTO，而是通过 `iviewer://` 协议按
//! ImageId + revision 读取。

use serde::Serialize;

use iv_image_decode::LoadError;

/// 命令错误：`code` 供前端分支，`message` 保留底层细节用于日志/提示。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

impl CommandError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new("io", message)
    }

    pub fn invalid_path(message: impl Into<String>) -> Self {
        Self::new("invalid_path", message)
    }
}

impl From<LoadError> for CommandError {
    fn from(error: LoadError) -> Self {
        let code = match &error {
            LoadError::Io(_) => "io",
            LoadError::Decode(_) => "decode",
            LoadError::TooLarge { .. } => "too_large",
            LoadError::OutOfMemory => "out_of_memory",
            LoadError::PreloadSkipped { .. } => "preload_skipped",
            #[cfg(feature = "avif")]
            LoadError::Avif(_) => "decode",
            #[cfg(feature = "heic")]
            LoadError::Heic(_) => "decode",
            #[cfg(feature = "svg")]
            LoadError::Svg(_) => "decode",
            LoadError::MissingDecoder { .. } => "missing_decoder",
            #[cfg(feature = "svg")]
            LoadError::SvgzUnsupported => "unsupported",
        };
        Self::new(code, error.to_string())
    }
}

/// 目录内的一张图片。`id` 是后端生成的授权句柄，前端不能凭路径直接读资源。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryEntryDto {
    pub id: String,
    pub path: String,
    pub name: String,
}

/// 一次目录扫描的完整快照。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorySnapshot {
    pub dir: String,
    pub entries: Vec<DirectoryEntryDto>,
    pub index: Option<usize>,
    pub position: Option<(usize, usize)>,
    pub can_prev: bool,
    pub can_next: bool,
    pub current: Option<DirectoryEntryDto>,
    /// `name` 或 `modified`。
    pub sort: String,
}

/// `request_preview` 的返回值；事件会带回同一个 requestId。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRequestDto {
    pub request_id: u64,
    pub image_id: String,
    pub revision: u64,
}

/// 预览解码完成。URL 由前端按 `imageId + revision` 拼接，payload 不含像素。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewReadyDto {
    pub request_id: u64,
    pub image_id: String,
    pub revision: u64,
    pub path: String,
    pub file_name: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub source_width: u32,
    pub source_height: u32,
    pub downscaled: bool,
    pub source_depth: u8,
    pub color_converted: bool,
    pub color_source: Option<String>,
    pub file_size: u64,
    pub modified_at: Option<i64>,
}

/// 预览解码失败；过期请求的失败事件会被前端丢弃。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewFailedDto {
    pub request_id: u64,
    pub image_id: String,
    pub code: &'static str,
    pub message: String,
}

/// 文件扩展名 → 展示用格式名。
pub fn format_label(path: &std::path::Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_uppercase())
        .unwrap_or_else(|| "UNKNOWN".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn format_label_is_uppercase_and_handles_missing_extension() {
        assert_eq!(format_label(Path::new("a.JpEg")), "JPEG");
        assert_eq!(format_label(Path::new("photo.heic")), "HEIC");
        assert_eq!(format_label(Path::new("no-extension")), "UNKNOWN");
    }

    #[test]
    fn command_error_keeps_stable_code() {
        let error = CommandError::from(LoadError::TooLarge { w: 1, h: 2 });
        assert_eq!(error.code, "too_large");
    }
}
