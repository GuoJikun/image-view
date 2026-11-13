//! 核心错误 → 界面文案，以及状态栏辅助文案。
//!
//! 与 eframe 应用保持一致：核心 crate 只返回结构化错误，本地化留在 GUI 层。

use iv_image_decode::LoadError;
use iv_metadata::ColorInfo;
use iv_platform::FileOpError;

pub fn load_error_text(error: &LoadError) -> String {
    match error {
        LoadError::Io(error) => format!("读取文件失败：{error}"),
        LoadError::Decode(error) => format!("图片已损坏或格式不受支持：{error}"),
        LoadError::TooLarge { w, h } => {
            format!("图片过大（{w}×{h}），超过安全上限")
        }
        LoadError::OutOfMemory => "解码超出内存上限".to_owned(),
        LoadError::PreloadSkipped { w, h } => format!("已跳过大图预加载（{w}×{h}）"),
        #[cfg(feature = "avif")]
        LoadError::Avif(detail) => format!("AVIF 解码失败：{detail}"),
        #[cfg(feature = "heic")]
        LoadError::Heic(detail) => format!("HEIC 解码失败：{detail}"),
        #[cfg(feature = "svg")]
        LoadError::Svg(detail) => format!("SVG 解析失败：{detail}"),
        LoadError::MissingDecoder { format, extension } => {
            format!("当前构建未包含 {format} 解码器，请使用启用该特性的版本打开 .{extension}")
        }
        #[cfg(feature = "svg")]
        LoadError::SvgzUnsupported => {
            "不支持 Gzip 压缩的 SVGZ，请先解压为 .svg".to_owned()
        }
    }
}

pub fn file_op_error_text(error: &FileOpError) -> String {
    match error {
        FileOpError::Trash(error) => format!("移入回收站失败：{error}"),
        FileOpError::Reveal(error) => format!("无法在文件管理器中定位：{error}"),
        FileOpError::Open(error) => format!("无法打开：{error}"),
    }
}

/// 状态栏与元数据面板的色彩转换文案。
pub fn color_label(info: &ColorInfo) -> Option<String> {
    if !info.converted {
        return None;
    }
    match info.source_label() {
        Some(name) => Some(format!("{name} → sRGB")),
        None => Some("已转换到 sRGB".to_owned()),
    }
}