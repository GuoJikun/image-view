//! Iced 侧样式：画布固定深色，工具栏/状态栏跟随主题。

use iced::widget::container;
use iced::{Background, Color, Theme};

/// 画布与 eframe 应用保持一致：两套主题下都用深色，保证图片对比度。
pub const CANVAS_BACKGROUND: Color = Color::from_rgb(0.086, 0.094, 0.110);

pub fn canvas(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(CANVAS_BACKGROUND)),
        ..Default::default()
    }
}

pub fn chrome(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.weak.color)),
        text_color: Some(palette.background.weak.text),
        ..Default::default()
    }
}

pub fn panel(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.base.color)),
        text_color: Some(palette.background.base.text),
        ..Default::default()
    }
}
