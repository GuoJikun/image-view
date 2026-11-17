//! 核心错误 → 界面文案，以及状态栏辅助文案。
//!
//! 核心 crate 只返回结构化错误，本地化留在 GUI 层：这里按 [`Text`] 组装句子，
//! 不在 Core 里出现任何中英文字面量。

use iv_image_decode::LoadError;
use iv_metadata::ColorInfo;
use iv_platform::FileOpError;

use crate::i18n::Text;

pub fn load_error_text(error: &LoadError, t: &Text) -> String {
    match error {
        LoadError::Io(error) => t.detail(t.error_read_file, &error.to_string()),
        LoadError::Decode(error) => t.detail(t.error_decode, &error.to_string()),
        LoadError::TooLarge { w, h } => t.image_too_large(*w, *h),
        LoadError::OutOfMemory => t.error_out_of_memory.to_owned(),
        LoadError::PreloadSkipped { w, h } => t.preload_skipped(*w, *h),
        #[cfg(feature = "avif")]
        LoadError::Avif(detail) => t.detail(t.error_avif, detail),
        #[cfg(feature = "heic")]
        LoadError::Heic(detail) => t.detail(t.error_heic, detail),
        #[cfg(feature = "svg")]
        LoadError::Svg(detail) => t.detail(t.error_svg, detail),
        LoadError::MissingDecoder { format, extension } => t.missing_decoder(format, extension),
        #[cfg(feature = "svg")]
        LoadError::SvgzUnsupported => t.error_svgz.to_owned(),
    }
}

pub fn file_op_error_text(error: &FileOpError, t: &Text) -> String {
    match error {
        FileOpError::Trash(error) => t.detail(t.error_trash, &error.to_string()),
        FileOpError::Reveal(error) => t.detail(t.error_reveal, &error.to_string()),
        FileOpError::Open(error) => t.detail(t.error_open, &error.to_string()),
    }
}

/// 状态栏与元数据面板的色彩转换文案。
///
/// 色彩空间名（`Display P3` 等）是专有名词，不随界面语言翻译。
pub fn color_label(info: &ColorInfo, t: &Text) -> Option<String> {
    if !info.converted {
        return None;
    }
    Some(match info.source_label() {
        Some(name) => format!("{name} → sRGB"),
        None => format!("{} sRGB", t.color_converted),
    })
}
