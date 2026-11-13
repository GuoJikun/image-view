//! 主题色板与 egui 样式。
//!
//! 设计稿只使用一个强调色，其余靠中性灰的明度层次和 1 pt 细线分隔；
//! 画布在浅色/深色主题下都保持深色，保证图片始终有最大对比度。
//!
//! `Palette` 只扩展、不重命名：现有字段被 5 个绘制模块引用，重赋值即可
//! 让「画布 S0 → 框架 S1 → 控件 S2 → 浮层 S3」四级阶梯在浅深两套主题下同构。

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Theme, ThemePreference,
    Visuals,
};

use iv_domain::ThemeSetting;

use crate::i18n::{self, Key};
use crate::visuals;

/// 主题菜单中的显示名。
pub fn theme_label(setting: ThemeSetting) -> &'static str {
    match setting {
        ThemeSetting::System => i18n::t(Key::ThemeSystem),
        ThemeSetting::Light => i18n::t(Key::ThemeLight),
        ThemeSetting::Dark => i18n::t(Key::ThemeDark),
    }
}

fn preference(setting: ThemeSetting) -> ThemePreference {
    match setting {
        ThemeSetting::System => ThemePreference::System,
        ThemeSetting::Light => ThemePreference::Light,
        ThemeSetting::Dark => ThemePreference::Dark,
    }
}

/// 一套设计稿色板。
///
/// 前 21 个字段为版式 A 保留字段（名称不变），其后为 S0~S3 补齐的 token。
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    // ---- S1 框架（chrome = strip = status）----
    pub chrome: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text2: Color32,
    pub text3: Color32,
    pub icon: Color32,
    // ---- S2 控件面 ----
    pub control: Color32,
    pub control_border: Color32,
    pub track: Color32,
    pub accent: Color32,
    // ---- S0 画布 ----
    pub canvas: Color32,
    pub strip: Color32,
    pub status: Color32,
    // ---- 语义色 ----
    pub danger: Color32,
    pub danger_soft: Color32,
    pub danger_text: Color32,
    pub success: Color32,
    pub tile: Color32,
    pub tile_border: Color32,
    pub hover: Color32,
    pub press: Color32,
    // ---- 新增：S0 画布抬升面 / 画布内文字 ----
    pub canvas_raised: Color32,
    pub canvas_text: Color32,
    pub canvas_text2: Color32,
    // ---- 新增：S3 浮层 ----
    pub overlay: Color32,
    pub shadow: [Color32; 4],
    pub mask: Color32,
    // ---- 新增：强调/焦点/语义补充 ----
    pub accent_soft: Color32,
    pub on_accent: Color32,
    pub focus_ring: Color32,
    pub success_soft: Color32,
    pub warning: Color32,
    // ---- 新增：棋盘格与结构边界 ----
    pub checker_a: Color32,
    pub checker_b: Color32,
    pub structure_stroke: Color32,
}

impl Palette {
    pub fn light() -> Self {
        let accent = Color32::from_rgb(47, 107, 255); // #2F6BFF（Q3）
        Self {
            // S0
            canvas: Color32::from_rgb(23, 24, 27), // #17181B
            canvas_raised: Color32::from_rgb(35, 38, 44), // #23262C
            canvas_text: Color32::from_rgb(243, 244, 246),
            canvas_text2: Color32::from_rgb(198, 204, 214),
            checker_a: Color32::from_rgb(27, 29, 33), // #1B1D21
            checker_b: Color32::from_rgb(38, 41, 46), // #26292E
            // S1（三层同值，解决 P-1）
            chrome: Color32::from_rgb(243, 244, 246), // #F3F4F6
            strip: Color32::from_rgb(243, 244, 246),
            status: Color32::from_rgb(243, 244, 246),
            border: Color32::from_rgb(221, 224, 229), // #DDE0E5
            structure_stroke: Color32::from_rgb(221, 224, 229),
            // S2
            control: Color32::WHITE,
            tile: Color32::WHITE,
            control_border: Color32::from_rgb(201, 205, 212), // #C9CDD4
            tile_border: Color32::from_rgb(201, 205, 212),
            track: Color32::from_rgb(233, 235, 238), // #E9EBEE
            // S3
            overlay: Color32::WHITE,
            shadow: [
                Color32::from_rgba_unmultiplied(11, 12, 14, 10),
                Color32::from_rgba_unmultiplied(11, 12, 14, 15),
                Color32::from_rgba_unmultiplied(11, 12, 14, 20),
                Color32::from_rgba_unmultiplied(11, 12, 14, 26),
            ],
            mask: Color32::from_rgba_unmultiplied(11, 12, 14, 184), // @72%
            // 文字 / 图标
            text: Color32::from_rgb(27, 29, 33),     // #1B1D21
            text2: Color32::from_rgb(92, 98, 108),   // #5C626C
            text3: Color32::from_rgb(139, 145, 155), // #8B919B
            icon: Color32::from_rgb(69, 74, 83),     // #454A53
            // 中性叠色 6% / 12%
            hover: Color32::from_rgba_unmultiplied(27, 29, 33, 15),
            press: Color32::from_rgba_unmultiplied(27, 29, 33, 31),
            // 强调
            accent,
            accent_soft: Color32::from_rgb(232, 238, 255), // #E8EEFF
            on_accent: Color32::WHITE,
            focus_ring: Color32::from_rgba_unmultiplied(47, 107, 255, 115), // @45%
            // 语义
            danger: Color32::from_rgb(217, 58, 58), // #D93A3A
            danger_soft: Color32::from_rgb(253, 236, 236), // #FDECEC
            danger_text: Color32::from_rgb(155, 38, 38), // #9B2626
            success: Color32::from_rgb(31, 138, 76), // #1F8A4C
            success_soft: Color32::from_rgb(231, 246, 236), // #E7F6EC
            warning: Color32::from_rgb(194, 132, 43), // #C2842B
        }
    }

    pub fn dark() -> Self {
        Self {
            // S0
            canvas: Color32::from_rgb(12, 13, 15), // #0C0D0F
            canvas_raised: Color32::from_rgb(26, 29, 34), // #1A1D22
            canvas_text: Color32::WHITE,
            canvas_text2: Color32::from_rgb(154, 163, 178),
            checker_a: Color32::from_rgb(20, 22, 25), // #141619
            checker_b: Color32::from_rgb(31, 34, 39), // #1F2227
            // S1（与 S0 明度差 ≥ 25，解决 P-1）
            chrome: Color32::from_rgb(43, 46, 52), // #2B2E34
            strip: Color32::from_rgb(43, 46, 52),
            status: Color32::from_rgb(43, 46, 52),
            border: Color32::from_rgb(69, 74, 83), // #454A53
            structure_stroke: Color32::from_rgb(69, 74, 83),
            // S2
            control: Color32::from_rgb(53, 57, 65), // #353941
            tile: Color32::from_rgb(53, 57, 65),
            control_border: Color32::from_rgb(85, 91, 101), // #555B65
            tile_border: Color32::from_rgb(85, 91, 101),
            track: Color32::from_rgb(36, 39, 44), // #24272C
            // S3（比 S2 再亮一档）
            overlay: Color32::from_rgb(58, 63, 70), // #3A3F46
            shadow: [
                Color32::from_rgba_unmultiplied(0, 0, 0, 31),
                Color32::from_rgba_unmultiplied(0, 0, 0, 41),
                Color32::from_rgba_unmultiplied(0, 0, 0, 51),
                Color32::from_rgba_unmultiplied(0, 0, 0, 61),
            ],
            mask: Color32::from_rgba_unmultiplied(11, 12, 14, 184), // @72%
            // 文字 / 图标
            text: Color32::from_rgb(242, 243, 245),  // #F2F3F5
            text2: Color32::from_rgb(167, 173, 182), // #A7ADB6
            text3: Color32::from_rgb(122, 129, 139), // #7A818B
            icon: Color32::from_rgb(194, 199, 207),  // #C2C7CF
            // 中性叠色 7% / 13%
            hover: Color32::from_white_alpha(18),
            press: Color32::from_white_alpha(33),
            // 强调
            accent: Color32::from_rgb(91, 140, 255), // #5B8CFF（Q3）
            accent_soft: Color32::from_rgb(29, 42, 69), // #1D2A45
            on_accent: Color32::WHITE,
            focus_ring: Color32::from_rgba_unmultiplied(91, 140, 255, 115), // @45%
            // 语义
            danger: Color32::from_rgb(255, 107, 107), // #FF6B6B
            danger_soft: Color32::from_rgb(58, 34, 36), // #3A2224
            danger_text: Color32::from_rgb(255, 180, 180), // #FFB4B4
            success: Color32::from_rgb(99, 197, 138), // #63C58A
            success_soft: Color32::from_rgb(28, 52, 39), // #1C3427
            warning: Color32::from_rgb(230, 179, 92), // #E6B35C
        }
    }

    pub fn of(theme: Theme) -> Self {
        match theme {
            Theme::Dark => Self::dark(),
            Theme::Light => Self::light(),
        }
    }
}

/// 当前生效主题的色板（每帧取，禁止跨帧缓存）。
pub fn palette(ctx: &egui::Context) -> Palette {
    Palette::of(ctx.theme())
}

/// 进度条空槽等“空背景”的颜色。
///
/// egui 的 `ProgressBar` 用 `extreme_bg_color` 画轨道，而该值默认等于窗口底色，
/// 深色主题下轨道会完全看不见；这里按主题选一个与面板有对比度的中性色。
pub fn progress_track(ctx: &egui::Context) -> Color32 {
    // `track` 即「下沉面」token：浅/深两套主题都与 S3 浮层底有足够对比。
    palette(ctx).track
}

/// 应用主题偏好，并为浅色/深色两套 egui 样式安装设计稿配色。
pub fn apply(ctx: &egui::Context, setting: ThemeSetting) {
    ctx.set_theme(preference(setting));
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
        // S3 浮层底：菜单/toast/错误卡片/帮助/更新窗口共用。
        window_fill: palette.overlay,
        window_stroke: Stroke::new(1.0, palette.control_border),
        window_corner_radius: CornerRadius::same(visuals::MENU_RADIUS as u8),
        menu_corner_radius: CornerRadius::same(visuals::MENU_RADIUS as u8),
        extreme_bg_color: palette.track,
        faint_bg_color: palette.strip,
        override_text_color: None,
        hyperlink_color: palette.accent,
        error_fg_color: palette.danger,
        warn_fg_color: palette.warning,
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
        widget.corner_radius = CornerRadius::same(visuals::BTN_RADIUS as u8);
    }

    ctx.set_visuals_of(theme, visuals);

    let mut style = (*ctx.style_of(theme)).clone();
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(visuals::FS_BODY, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(visuals::FS_BUTTON, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(visuals::FS_MIN, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(visuals::FS_TITLE, FontFamily::Proportional),
    );
    ctx.set_style_of(theme, style);
}

// ---------------------------------------------------------------------------
// QA 回归测试：色板（PRD §5.3 / DESIGN §8.1）
// ---------------------------------------------------------------------------

#[cfg(test)]
mod qa_redesign_tests {
    use super::*;

    fn luma(color: Color32) -> f32 {
        0.299 * color.r() as f32 + 0.587 * color.g() as f32 + 0.114 * color.b() as f32
    }

    #[test]
    fn light_and_dark_follow_the_s0_s1_s2_s3_elevation_ladder() {
        for palette in [Palette::light(), Palette::dark()] {
            assert!(luma(palette.canvas) < luma(palette.chrome), "S0 应比 S1 暗");
            assert!(
                luma(palette.chrome) < luma(palette.control),
                "S1 应比 S2 暗"
            );
            assert!(luma(palette.control) <= luma(palette.overlay), "S2 <= S3");
            assert!(
                luma(palette.canvas) < luma(palette.canvas_raised),
                "画布抬升面应亮于画布"
            );
            assert!(
                luma(palette.track) < luma(palette.control),
                "下沉面应暗于控件面"
            );
        }
    }

    #[test]
    fn dark_chrome_is_at_least_25_levels_above_canvas() {
        let palette = Palette::dark();
        assert!(
            luma(palette.chrome) - luma(palette.canvas) >= 25.0,
            "A2：深色 S1 与 S0 明度差必须 ≥25/255"
        );
    }

    #[test]
    fn accent_colors_match_the_q3_user_decision() {
        assert_eq!(Palette::light().accent, Color32::from_rgb(47, 107, 255));
        assert_eq!(Palette::dark().accent, Color32::from_rgb(91, 140, 255));
        assert_eq!(
            Palette::light().accent_soft,
            Color32::from_rgb(232, 238, 255)
        );
        assert_eq!(Palette::dark().accent_soft, Color32::from_rgb(29, 42, 69));
        assert_eq!(Palette::light().on_accent, Color32::WHITE);
        assert_eq!(Palette::dark().on_accent, Color32::WHITE);
    }

    #[test]
    fn chrome_strip_status_share_one_surface_color() {
        for palette in [Palette::light(), Palette::dark()] {
            assert_eq!(palette.chrome, palette.strip, "A1：三层 chrome 必须同值");
            assert_eq!(palette.chrome, palette.status);
            assert_eq!(palette.control, palette.tile);
            assert_eq!(palette.control_border, palette.tile_border);
            assert_eq!(palette.border, palette.structure_stroke);
        }
    }

    #[test]
    fn mask_is_72_percent_and_shadow_has_four_graded_layers() {
        for palette in [Palette::light(), Palette::dark()] {
            assert_eq!(palette.mask.a(), 184, "@72% 应为 184/255");
            assert_eq!(palette.shadow.len(), 4);
            assert!(
                palette
                    .shadow
                    .windows(2)
                    .all(|pair| pair[0].a() <= pair[1].a())
            );
        }
    }

    #[test]
    fn palette_of_theme_matches_the_two_constructors() {
        assert_eq!(
            format!("{:?}", Palette::of(Theme::Light)),
            format!("{:?}", Palette::light())
        );
        assert_eq!(
            format!("{:?}", Palette::of(Theme::Dark)),
            format!("{:?}", Palette::dark())
        );
    }
}
