//! 核心/平台错误 → 本地化界面文案。
//!
//! 核心 crate 只返回结构化错误，i18n 资源表留在 GUI 层。

use iv_image_decode::LoadError;
use iv_metadata::ColorInfo;
use iv_platform::FileOpError;

use crate::i18n::{self, Key};

/// 解码/读取错误的中英文文案。
pub fn load_error_text(error: &LoadError) -> String {
    match error {
        LoadError::Io(error) => i18n::tf(Key::LoadRead, &[error]),
        LoadError::Decode(error) => i18n::tf(Key::LoadDecode, &[error]),
        LoadError::TooLarge { w, h } => i18n::tf(Key::LoadTooLarge, &[w, h]),
        LoadError::OutOfMemory => i18n::t(Key::LoadOutOfMemory).to_owned(),
        LoadError::PreloadSkipped { w, h } => i18n::tf(Key::LoadPreloadSkipped, &[w, h]),
        #[cfg(feature = "avif")]
        LoadError::Avif(detail) => i18n::tf(Key::LoadAvif, &[detail]),
        #[cfg(feature = "heic")]
        LoadError::Heic(detail) => i18n::tf(Key::LoadHeic, &[detail]),
        #[cfg(feature = "svg")]
        LoadError::Svg(detail) => i18n::tf(Key::LoadSvg, &[detail]),
        LoadError::MissingDecoder { format, extension } => {
            i18n::tf(Key::LoadMissingDecoder, &[format, extension])
        }
        #[cfg(feature = "svg")]
        LoadError::SvgzUnsupported => i18n::t(Key::SvgzUnsupported).to_owned(),
    }
}

/// 回收站/文件管理器错误的本地化文案。
pub fn file_op_error_text(error: &FileOpError) -> String {
    match error {
        FileOpError::Trash(error) => i18n::tf(Key::ErrorTrashFailed, &[error]),
        FileOpError::Reveal(error) => i18n::tf(Key::ErrorOpenFileManager, &[error]),
        FileOpError::Open(error) => i18n::tf(Key::ErrorOpenFile, &[error]),
    }
}

/// 状态栏色彩转换文案；未转换时返回 `None`。
pub fn color_label(info: &ColorInfo) -> Option<String> {
    if !info.converted {
        return None;
    }
    match info.source_label() {
        Some(name) => Some(format!("{name} → sRGB")),
        None => Some(i18n::t(Key::ColorConverted).to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_decoder_is_localized() {
        i18n::set_language(i18n::Language::Zh);
        let error = LoadError::MissingDecoder {
            format: "HEIC",
            extension: "heif".to_owned(),
        };
        let text = load_error_text(&error);
        assert!(text.contains("HEIC"), "实际：{text}");
        assert!(text.contains("启用该特性"), "实际：{text}");
    }
}
