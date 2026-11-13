//! 当前显示的图片及其纹理。

use std::path::PathBuf;

use eframe::egui;

use iv_image_decode::Decoded;
use iv_metadata::ColorInfo;

use crate::i18n::{self, Key};

#[derive(Clone)]
pub struct LoadedImage {
    pub path: PathBuf,
    pub texture: egui::TextureHandle,
    pub display_size: [usize; 2],
    pub source_size: [usize; 2],
    pub downscaled: bool,
    /// 源文件位深（8 或 16）。
    pub source_depth: u8,
    pub color: ColorInfo,
}

impl LoadedImage {
    pub fn from_decoded(ctx: &egui::Context, path: PathBuf, decoded: Decoded) -> Self {
        let color_image = egui::ColorImage::from_rgba_unmultiplied(decoded.size, &decoded.rgba);
        let texture = ctx.load_texture(
            format!("image:{}", path.display()),
            color_image,
            egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
        );

        Self {
            path,
            texture,
            display_size: decoded.size,
            source_size: decoded.source_size,
            downscaled: decoded.size != decoded.source_size,
            source_depth: decoded.source_depth,
            color: decoded.color,
        }
    }

    pub fn texture_size(&self) -> egui::Vec2 {
        egui::vec2(self.display_size[0] as f32, self.display_size[1] as f32)
    }

    /// 纹理占用的像素数，供缓存估算内存用量。
    pub fn display_pixels(&self) -> usize {
        self.display_size[0].saturating_mul(self.display_size[1])
    }

    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.display().to_string())
    }

    /// 窗口标题：仅在内容变化时由调用方发送。
    pub fn window_title(&self) -> String {
        format_title(&self.file_name(), self.downscaled)
    }

    /// 文件格式（扩展名大写），用于状态栏。
    pub fn format_label(&self) -> String {
        self.path
            .extension()
            .map(|extension| extension.to_string_lossy().to_ascii_uppercase())
            .unwrap_or_default()
    }

    pub fn dimensions_text(&self) -> String {
        let depth = if self.source_depth > 8 {
            " · 16-bit"
        } else {
            ""
        };
        if self.downscaled {
            format!(
                "{}{depth}",
                i18n::tf(
                    Key::DimensionsDownscaled,
                    &[
                        &self.source_size[0],
                        &self.source_size[1],
                        &self.display_size[0],
                        &self.display_size[1],
                    ],
                )
            )
        } else {
            format!("{}×{}{depth}", self.source_size[0], self.source_size[1])
        }
    }

    /// 状态栏色彩转换文案；未转换时返回 `None`。
    pub fn color_label(&self) -> Option<String> {
        crate::errors::color_label(&self.color)
    }
}

pub fn format_title(file_name: &str, downscaled: bool) -> String {
    let mut title = format!("{file_name} — {}", i18n::t(Key::AppName));
    if downscaled {
        title.push_str(" · ");
        title.push_str(i18n::t(Key::Downscaled));
    }
    title
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_contains_name() {
        i18n::set_language(crate::i18n::Language::Zh);
        assert_eq!(format_title("a.jpg", false), "a.jpg — 图片查看器");
    }

    #[test]
    fn title_marks_downscaled_images() {
        i18n::set_language(crate::i18n::Language::Zh);
        let title = format_title("big.png", true);
        assert!(title.contains("已降采样"));
        assert!(title.contains("big.png"));
    }
}
