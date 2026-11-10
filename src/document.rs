//! 当前显示的图片及其纹理。

use std::path::PathBuf;

use eframe::egui;

use crate::loader::Decoded;

pub struct LoadedImage {
    pub path: PathBuf,
    pub texture: egui::TextureHandle,
    pub display_size: [usize; 2],
    pub source_size: [usize; 2],
    pub downscaled: bool,
}

impl LoadedImage {
    pub fn from_decoded(ctx: &egui::Context, path: PathBuf, decoded: Decoded) -> Self {
        let color_image = egui::ColorImage::from_rgba_unmultiplied(decoded.size, &decoded.rgba);
        let texture = ctx.load_texture(
            format!("image:{}", path.display()),
            color_image,
            egui::TextureOptions::LINEAR,
        );

        Self {
            path,
            texture,
            display_size: decoded.size,
            source_size: decoded.source_size,
            downscaled: decoded.size != decoded.source_size,
        }
    }

    pub fn texture_size(&self) -> egui::Vec2 {
        egui::vec2(self.display_size[0] as f32, self.display_size[1] as f32)
    }

    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.display().to_string())
    }

    /// 窗口标题：仅在内容变化时由调用方发送。
    pub fn window_title(&self, zoom_percent: f32) -> String {
        format_title(
            &self.file_name(),
            self.source_size,
            zoom_percent,
            self.downscaled,
        )
    }

    pub fn dimensions_text(&self) -> String {
        if self.downscaled {
            format!(
                "{}×{}（显示 {}×{}）",
                self.source_size[0],
                self.source_size[1],
                self.display_size[0],
                self.display_size[1]
            )
        } else {
            format!("{}×{}", self.source_size[0], self.source_size[1])
        }
    }
}

pub fn format_title(
    file_name: &str,
    source_size: [usize; 2],
    zoom_percent: f32,
    downscaled: bool,
) -> String {
    let mut title = format!(
        "{file_name} — {}×{} ({zoom_percent:.0}%)",
        source_size[0], source_size[1]
    );
    if downscaled {
        title.push_str(" · 已降采样");
    }
    title.push_str(" — 图片查看器");
    title
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_contains_name_size_and_zoom() {
        assert_eq!(
            format_title("a.jpg", [6000, 4000], 67.4, false),
            "a.jpg — 6000×4000 (67%) — 图片查看器"
        );
    }

    #[test]
    fn title_marks_downscaled_images() {
        let title = format_title("big.png", [20000, 10000], 25.0, true);
        assert!(title.contains("已降采样"));
        assert!(title.contains("20000×10000"));
    }
}
