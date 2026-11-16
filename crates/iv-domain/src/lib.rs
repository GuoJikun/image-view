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

impl Language {
    /// 解析成界面语言标签：`"zh"` 或 `"en"`。
    ///
    /// `system` 是系统语言标签（BCP-47，如 `zh-CN`）。跟随系统时只看主语言子标签，
    /// 与前端 `navigator.language` 的判断一致；读不到或非中文一律回退英文。
    pub fn resolve_tag(self, system: Option<&str>) -> &'static str {
        match self {
            Self::Zh => "zh",
            Self::En => "en",
            Self::System => match system {
                Some(tag) if is_chinese_tag(tag) => "zh",
                _ => "en",
            },
        }
    }
}

/// 语言标签的主子标签是否为中文（大小写不敏感）。
///
/// 按字节比较前两位，避免 `zh-CN` 这类标签切片时的 UTF-8 边界问题。
fn is_chinese_tag(tag: &str) -> bool {
    tag.as_bytes()
        .get(..2)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"zh"))
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

    #[test]
    fn explicit_language_ignores_system_locale() {
        assert_eq!(Language::Zh.resolve_tag(Some("en-US")), "zh");
        assert_eq!(Language::En.resolve_tag(Some("zh-CN")), "en");
        assert_eq!(Language::En.resolve_tag(None), "en");
    }

    #[test]
    fn system_language_detects_chinese_variants() {
        for tag in ["zh", "zh-CN", "zh-Hans-CN", "ZH-tw", "zh_TW", "zh-Hant-HK"] {
            assert_eq!(
                Language::System.resolve_tag(Some(tag)),
                "zh",
                "{tag} 应判定为中文"
            );
        }
    }

    #[test]
    fn system_language_falls_back_to_english() {
        // 读不到语言、"C"/"POSIX" 这类占位标签，以及非中文语言都要回退英文。
        for tag in [
            None,
            Some(""),
            Some("C"),
            Some("POSIX"),
            Some("en-GB"),
            Some("ja-JP"),
        ] {
            assert_eq!(
                Language::System.resolve_tag(tag),
                "en",
                "{tag:?} 应回退英文"
            );
        }
        // 以 "z" 开头但不是中文的标签不能误判。
        assert_eq!(Language::System.resolve_tag(Some("zu-ZA")), "en");
    }
}
