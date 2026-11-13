//! ImageViewer 领域值对象。
//!
//! 本 crate 不依赖文件系统、GUI 或任何平台 API；只保存跨层共享的纯类型。
//! 图片格式识别与排序规则属于领域知识：文件层负责遍历，渲染层负责展示。

use std::path::Path;

use serde::{Deserialize, Serialize};

/// 与解码器特性保持一致的扩展名列表。
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff", "ico", "qoi", "tga", "pnm", "hdr",
    "exr", "dds", "avif", "heic", "heif", "svg",
];

/// 扩展名是否属于支持的图片格式（大小写不敏感）。
pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            SUPPORTED_EXTENSIONS
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(ext))
        })
        .unwrap_or(false)
}

/// 目录内图片的排序方式。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortOrder {
    /// 文件名自然排序（大小写不敏感）。
    #[default]
    Name,
    /// 修改时间从新到旧，时间相同时按文件名。
    ModifiedNewest,
}

/// 用户可选的语言。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    /// 跟随系统；非中文系统回退英文。
    #[default]
    System,
    Zh,
    En,
}

/// 用户可选的主题。跟随系统时由 GUI 读取操作系统的浅色/深色偏好。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeSetting {
    #[default]
    System,
    Light,
    Dark,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_matching_is_case_insensitive() {
        assert!(is_supported_image(Path::new("a.PNG")));
        assert!(is_supported_image(Path::new("b.JpEg")));
        assert!(is_supported_image(Path::new("c.AVIF")));
        assert!(is_supported_image(Path::new("d.HEIC")));
        assert!(is_supported_image(Path::new("e.heif")));
        assert!(is_supported_image(Path::new("g.SVG")));
        assert!(!is_supported_image(Path::new("h.svgz")));
        assert!(!is_supported_image(Path::new("f.txt")));
        assert!(!is_supported_image(Path::new("no-extension")));
    }
}
