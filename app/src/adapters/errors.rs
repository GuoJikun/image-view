//! 核心错误 → 界面文案，以及状态栏辅助文案。
//!
//! 核心 crate 只返回结构化错误，本地化留在 GUI 层：这里按 [`Text`] 组装句子，
//! 不在 Core 里出现任何中英文字面量。

use iv_image_decode::{DegradeReason, LoadError};
use iv_metadata::{ColorInfo, ColorTarget};
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

/// 动画降级原因 → 用户可读提示（与 `main` 的 `degrade.*` 文案对应）。
pub fn degrade_text(reason: &DegradeReason, t: &Text) -> String {
    match reason {
        DegradeReason::OverBudget { .. } => t.degrade_over_budget.to_owned(),
        DegradeReason::Unsupported16Bit => t.degrade_unsupported.to_owned(),
        DegradeReason::DecodeFailed { message } => t.detail(t.degrade_decode_failed, message),
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

/// 元数据面板的色彩行，与 main 的 `MetadataPanel.vue` 逐字对齐：
/// 未转换时也显示像素被解释成的目标空间；只有转换过才追加「已转换到」和源 profile。
pub fn metadata_color_label(info: &ColorInfo, t: &Text) -> String {
    let target = match info.target {
        ColorTarget::Srgb => "sRGB",
        ColorTarget::DisplayP3 => "Display P3",
    };
    if !info.converted {
        return target.to_owned();
    }
    match info.source_label() {
        Some(name) => format!("{} {target} ({name})", t.color_converted),
        None => format!("{} {target}", t.color_converted),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{EN, ZH};

    #[test]
    fn metadata_color_label_matches_main() {
        let srgb = ColorInfo::default();
        assert_eq!(metadata_color_label(&srgb, &ZH), "sRGB");
        assert_eq!(metadata_color_label(&srgb, &EN), "sRGB");

        let p3 = ColorInfo {
            target: ColorTarget::DisplayP3,
            ..ColorInfo::default()
        };
        assert_eq!(metadata_color_label(&p3, &ZH), "Display P3");

        let converted = ColorInfo {
            converted: true,
            source: Some("  Adobe RGB  ".to_owned()),
            target: ColorTarget::Srgb,
        };
        assert_eq!(
            metadata_color_label(&converted, &ZH),
            "已转换到 sRGB (Adobe RGB)"
        );
        assert_eq!(
            metadata_color_label(&converted, &EN),
            "Converted to sRGB (Adobe RGB)"
        );

        let unnamed = ColorInfo {
            converted: true,
            source: None,
            target: ColorTarget::DisplayP3,
        };
        assert_eq!(
            metadata_color_label(&unnamed, &EN),
            "Converted to Display P3"
        );
    }
}
