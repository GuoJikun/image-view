//! 主题色板与 egui 样式。
//!
//! 设计稿只使用一个强调色，其余靠中性灰的明度层次和 1 px 细线分隔；
//! 画布在浅色/深色主题下都保持深色，保证图片始终有最大对比度。

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Theme, ThemePreference,
    Visuals,
};

use crate::i18n::{self, Key};

/// 用户可选的主题。跟随系统时由 egui 读取操作系统的浅色/深色偏好。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ThemeSetting {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeSetting {
    pub fn label(self) -> &'static str {
        match self {
            Self::System => i18n::t(Key::ThemeSystem),
            Self::Light => i18n::t(Key::ThemeLight),
            Self::Dark => i18n::t(Key::ThemeDark),
        }
    }

    fn preference(self) -> ThemePreference {
        match self {
            Self::System => ThemePreference::System,
            Self::Light => ThemePreference::Light,
            Self::Dark => ThemePreference::Dark,
        }
    }
}

/// 一套设计稿色板。
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub chrome: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text2: Color32,
    pub text3: Color32,
    pub icon: Color32,
    pub control: Color32,
    pub control_border: Color32,
    pub track: Color32,
    pub accent: Color32,
    pub canvas: Color32,
    pub strip: Color32,
    pub status: Color32,
    pub danger: Color32,
    pub danger_soft: Color32,
    pub danger_text: Color32,
    pub success: Color32,
    pub tile: Color32,
    pub tile_border: Color32,
    pub hover: Color32,
    pub press: Color32,
}

impl Palette {
    pub fn light() -> Self {
        Self {
            chrome: Color32::from_rgb(246, 246, 247),
            border: Color32::from_rgb(228, 228, 231),
            text: Color32::from_rgb(29, 29, 31),
            text2: Color32::from_rgb(110, 110, 115),
            text3: Color32::from_rgb(154, 154, 160),
            icon: Color32::from_rgb(72, 72, 76),
            control: Color32::WHITE,
            control_border: Color32::from_black_alpha(26),
            track: Color32::from_rgb(233, 233, 235),
            accent: Color32::from_rgb(10, 124, 255),
            canvas: Color32::from_rgb(28, 28, 30),
            strip: Color32::from_rgb(239, 239, 241),
            status: Color32::from_rgb(246, 246, 247),
            danger: Color32::from_rgb(229, 72, 77),
            danger_soft: Color32::from_rgb(253, 236, 236),
            danger_text: Color32::from_rgb(140, 47, 50),
            success: Color32::from_rgb(47, 143, 78),
            tile: Color32::WHITE,
            tile_border: Color32::from_black_alpha(18),
            hover: Color32::from_black_alpha(14),
            press: Color32::from_black_alpha(26),
        }
    }

    pub fn dark() -> Self {
        Self {
            chrome: Color32::from_rgb(42, 42, 45),
            border: Color32::from_rgb(58, 58, 61),
            text: Color32::from_rgb(245, 245, 247),
            text2: Color32::from_rgb(161, 161, 166),
            text3: Color32::from_rgb(124, 124, 130),
            icon: Color32::from_rgb(211, 211, 217),
            control: Color32::from_rgb(58, 58, 61),
            control_border: Color32::from_white_alpha(26),
            track: Color32::from_rgb(58, 58, 61),
            accent: Color32::from_rgb(10, 132, 255),
            canvas: Color32::from_rgb(16, 16, 18),
            strip: Color32::from_rgb(35, 35, 38),
            status: Color32::from_rgb(42, 42, 45),
            danger: Color32::from_rgb(255, 105, 97),
            danger_soft: Color32::from_rgb(58, 37, 38),
            danger_text: Color32::from_rgb(255, 180, 176),
            success: Color32::from_rgb(110, 200, 140),
            tile: Color32::from_rgb(51, 51, 56),
            tile_border: Color32::from_white_alpha(18),
            hover: Color32::from_white_alpha(18),
            press: Color32::from_white_alpha(33),
        }
    }

    pub fn of(theme: Theme) -> Self {
        match theme {
            Theme::Dark => Self::dark(),
            Theme::Light => Self::light(),
        }
    }
}

/// 当前生效主题的色板。
pub fn palette(ctx: &egui::Context) -> Palette {
    Palette::of(ctx.theme())
}

/// 进度条空槽等“空背景”的颜色。
///
/// egui 的 `ProgressBar` 用 `extreme_bg_color` 画轨道，而该值默认等于窗口底色，
/// 深色主题下轨道会完全看不见；这里按主题选一个与面板有对比度的中性色。
pub fn progress_track(ctx: &egui::Context) -> Color32 {
    let palette = palette(ctx);
    match ctx.theme() {
        Theme::Dark => palette.strip,
        Theme::Light => palette.track,
    }
}

/// 应用主题偏好，并为浅色/深色两套 egui 样式安装设计稿配色。
pub fn apply(ctx: &egui::Context, setting: ThemeSetting) {
    ctx.set_theme(setting.preference());
    install(ctx, Theme::Light, Palette::light());
    install(ctx, Theme::Dark, Palette::dark());
}

fn install(ctx: &egui::Context, theme: Theme, palette: Palette) {
    let base = match theme {
        Theme::Dark => Visuals::dark(),
        Theme::Light => Visuals::light(),
    };

    let mut visuals = Visuals {
        panel_fill: palette.chrome,
        window_fill: palette.control,
        window_stroke: Stroke::new(1.0, palette.border),
        window_corner_radius: CornerRadius::same(10),
        menu_corner_radius: CornerRadius::same(8),
        extreme_bg_color: palette.control,
        faint_bg_color: palette.strip,
        override_text_color: None,
        hyperlink_color: palette.accent,
        error_fg_color: palette.danger,
        warn_fg_color: Color32::from_rgb(230, 160, 50),
        ..base
    };
    visuals.selection.bg_fill = palette.accent;
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.noninteractive.bg_fill = palette.chrome;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    visuals.widgets.hovered.weak_bg_fill = palette.hover;
    visuals.widgets.active.weak_bg_fill = palette.press;
    visuals.widgets.open.weak_bg_fill = palette.hover;
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = CornerRadius::same(6);
    }

    ctx.set_visuals_of(theme, visuals);

    let mut style = (*ctx.style_of(theme)).clone();
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(13.0, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(12.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(11.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(16.0, FontFamily::Proportional),
    );
    ctx.set_style_of(theme, style);
}
