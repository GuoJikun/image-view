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
