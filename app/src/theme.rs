//! Iced 侧样式令牌，对齐 `design/macos-first/README.md` 第三节。
//!
//! 画布恒为深色；chrome / strip / 控件等按浅色 / 深色两套取值。所有 widget 的
//! `style` 闭包从 `Palette::from(theme)` 取色，避免在 view 层散落颜色字面量。

use iced::border::Radius;
use iced::widget::{button, container, svg};
use iced::{Background, Border, Color, Shadow, Theme, Vector};

/// 将 `#RRGGBB` 转为 [`Color`]（不透明）。
const fn rgb(hex: u32) -> Color {
    Color::from_rgb(
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
    )
}

/// 带 alpha 的 `Color`，用于 hover / press / accent-soft / 投影。
const fn rgba(hex: u32, a: f32) -> Color {
    Color { a, ..rgb(hex) }
}

/// 设计令牌集合。`light()` / `dark()` 与规范色板表逐项对应。
#[derive(Clone, Copy)]
pub struct Palette {
    pub chrome: Color,
    pub strip: Color,
    pub hairline: Color,
    pub control: Color,
    pub text: Color,
    pub text2: Color,
    pub text3: Color,
    pub icon: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub danger: Color,
    pub danger_soft: Color,
    pub danger_text: Color,
    pub success: Color,
    pub track: Color,
    pub hover: Color,
    pub press: Color,
}

impl Palette {
    pub fn from(theme: &Theme) -> Self {
        if theme_is_dark(theme) {
            Self::dark()
        } else {
            Self::light()
        }
    }

    fn light() -> Self {
        Self {
            chrome: rgb(0xF6F6F7),
            strip: rgb(0xEFEFF1),
            hairline: rgb(0xE4E4E7),
            control: rgb(0xFFFFFF),
            text: rgb(0x1D1D1F),
            text2: rgb(0x6E6E73),
            text3: rgb(0x98989D),
            icon: rgb(0x48484C),
            accent: rgb(0x0A7CFF),
            accent_soft: rgba(0x0A7CFF, 0.13),
            danger: rgb(0xE5484D),
            danger_soft: rgb(0xFDECEC),
            danger_text: rgb(0xB4232A),
            success: rgb(0x2F8F4E),
            track: rgb(0xE9E9EB),
            hover: rgba(0x000000, 0.055),
            press: rgba(0x000000, 0.10),
        }
    }

    fn dark() -> Self {
        Self {
            chrome: rgb(0x2A2A2D),
            strip: rgb(0x232326),
            hairline: rgb(0x3A3A3D),
            control: rgb(0x3A3A3D),
            text: rgb(0xF5F5F7),
            text2: rgb(0xA1A1A6),
            text3: rgb(0x7C7C82),
            icon: rgb(0xD3D3D9),
            accent: rgb(0x0A84FF),
            accent_soft: rgba(0x0A84FF, 0.20),
            danger: rgb(0xFF6961),
            danger_soft: rgb(0x3A2526),
            danger_text: rgb(0xFF8A82),
            success: rgb(0x6EC88C),
            track: rgb(0x3A3A3D),
            hover: rgba(0xFFFFFF, 0.07),
            press: rgba(0xFFFFFF, 0.13),
        }
    }
}

/// Iced 0.14 没有直接判定明暗的 API，按调色板背景亮度推断。
fn theme_is_dark(theme: &Theme) -> bool {
    let bg = theme.extended_palette().background.base.color;
    bg.r * 0.299 + bg.g * 0.587 + bg.b * 0.114 < 0.5
}

// ---------- 尺寸 / 形状令牌 ----------

/// 画布恒定背景（浅色 `#1C1C1E`，深色 `#101012`；统一取中间偏深值保证对比度）。
pub const CANVAS_BACKGROUND: Color = rgb(0x16171A);
pub const RADIUS_CONTROL: f32 = 6.0;
pub const RADIUS_TILE: f32 = 8.0;
/// 菜单 / 下拉面板圆角（规范 4.5）。
pub const RADIUS_MENU: f32 = 6.0;
/// 弹窗圆角（规范 4.8）。
pub const RADIUS_DIALOG: f32 = 10.0;
pub const HAIRLINE: f32 = 1.0;

fn radius(value: f32) -> Radius {
    Radius::from(value)
}

// ---------- 容器样式 ----------

pub fn canvas(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(CANVAS_BACKGROUND)),
        ..Default::default()
    }
}

pub fn chrome(theme: &Theme) -> container::Style {
    let p = Palette::from(theme);
    container::Style {
        background: Some(Background::Color(p.chrome)),
        text_color: Some(p.text),
        ..Default::default()
    }
}

/// 缩略图条底色，上缘发丝线。
pub fn strip(theme: &Theme) -> container::Style {
    let p = Palette::from(theme);
    container::Style {
        background: Some(Background::Color(p.strip)),
        text_color: Some(p.text2),
        ..Default::default()
    }
}

pub fn panel(theme: &Theme) -> container::Style {
    let p = Palette::from(theme);
    container::Style {
        background: Some(Background::Color(p.control)),
        text_color: Some(p.text),
        border: Border {
            color: p.hairline,
            width: HAIRLINE,
            radius: radius(RADIUS_CONTROL),
        },
        shadow: shadow_popover(theme_is_dark(theme)),
        ..Default::default()
    }
}

/// 错误横幅：danger-soft 底 + 左侧 3px danger 竖条。
pub fn danger_banner(theme: &Theme) -> container::Style {
    let p = Palette::from(theme);
    container::Style {
        background: Some(Background::Color(p.danger_soft)),
        text_color: Some(p.danger_text),
        border: Border {
            color: p.danger,
            width: 3.0,
            radius: radius(0.0),
        },
        ..Default::default()
    }
}

/// 空状态主按钮（accent 填充白字）。
pub fn primary_button(theme: &Theme, status: button::Status) -> button::Style {
    let p = Palette::from(theme);
    let bg = match status {
        button::Status::Hovered => Color {
            a: 0.88,
            ..p.accent
        },
        button::Status::Pressed => Color {
            a: 0.75,
            ..p.accent
        },
        button::Status::Disabled => Color { a: 0.4, ..p.accent },
        button::Status::Active => p.accent,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: Color::WHITE,
        border: Border::default().rounded(RADIUS_CONTROL),
        ..Default::default()
    }
}

/// 工具条图标按钮：默认透明，hover / press 用令牌，激活态用 accent-soft。
pub fn icon_button(theme: &Theme, status: button::Status, active: bool) -> button::Style {
    let p = Palette::from(theme);
    let background = if active {
        Some(Background::Color(p.accent_soft))
    } else {
        match status {
            button::Status::Hovered => Some(Background::Color(p.hover)),
            button::Status::Pressed => Some(Background::Color(p.press)),
            button::Status::Disabled | button::Status::Active => None,
        }
    };
    let text_color = if active {
        p.accent
    } else {
        match status {
            button::Status::Disabled => p.text3,
            _ => p.icon,
        }
    };
    button::Style {
        background,
        text_color,
        border: Border::default().rounded(RADIUS_CONTROL),
        ..Default::default()
    }
}

/// 胶囊缩放段（− / 值 / +）：中段白底 + 投影，两侧透明。
pub fn capsule_segment(theme: &Theme, status: button::Status, middle: bool) -> button::Style {
    let p = Palette::from(theme);
    let background = if middle {
        Some(Background::Color(p.control))
    } else {
        match status {
            button::Status::Hovered => Some(Background::Color(p.hover)),
            button::Status::Pressed => Some(Background::Color(p.press)),
            _ => None,
        }
    };
    button::Style {
        background,
        text_color: if middle { p.text } else { p.icon },
        border: if middle {
            Border {
                color: p.hairline,
                width: HAIRLINE,
                radius: radius(5.0),
            }
        } else {
            Border::default().rounded(4.0)
        },
        shadow: if middle {
            shadow_selected()
        } else {
            Shadow::default()
        },
        ..Default::default()
    }
}

/// 缩略图卡片：选中时外扩 2px accent 环 + 投影。
pub fn thumb_card(theme: &Theme, status: button::Status, selected: bool) -> button::Style {
    let p = Palette::from(theme);
    let background = if selected {
        Some(Background::Color(p.strip))
    } else {
        match status {
            button::Status::Hovered => Some(Background::Color(p.hover)),
            button::Status::Pressed => Some(Background::Color(p.press)),
            _ => None,
        }
    };
    button::Style {
        background,
        text_color: if selected { p.accent } else { p.text2 },
        border: Border {
            color: if selected { p.accent } else { p.hairline },
            width: if selected { 2.0 } else { HAIRLINE },
            radius: radius(RADIUS_TILE),
        },
        shadow: if selected {
            shadow_selected()
        } else {
            Shadow::default()
        },
        ..Default::default()
    }
}

/// 文字次级按钮（⋯ 菜单里的动作项）。
pub fn text_button(theme: &Theme, status: button::Status) -> button::Style {
    let p = Palette::from(theme);
    button::Style {
        background: match status {
            button::Status::Hovered => Some(Background::Color(p.hover)),
            button::Status::Pressed => Some(Background::Color(p.press)),
            _ => None,
        },
        text_color: match status {
            button::Status::Disabled => p.text3,
            _ => p.text,
        },
        border: Border::default().rounded(RADIUS_CONTROL),
        ..Default::default()
    }
}

/// 图标 SVG 的单色过滤，随 token 与状态变化。
pub fn icon_svg(theme: &Theme, color: Color) -> svg::Style {
    let _ = Palette::from(theme);
    svg::Style { color: Some(color) }
}

// ---------- 菜单与弹窗 ----------

/// 菜单栏文本项：展开时 `accent-soft` 底 + `accent` 文字，悬停用 `hover` 底。
///
/// 用 `container` 而不是 `button`：按钮会把悬停光标固定成手型，而菜单栏标题按
/// 原生习惯要保持箭头（光标由 `view/menu.rs` 的 `mouse_area` 声明为 `Idle`）。
pub fn menu_bar_item(theme: &Theme, hovered: bool, open: bool) -> container::Style {
    let p = Palette::from(theme);
    container::Style {
        background: if open {
            Some(Background::Color(p.accent_soft))
        } else if hovered {
            Some(Background::Color(p.hover))
        } else {
            None
        },
        text_color: Some(if open { p.accent } else { p.text }),
        border: Border::default().rounded(4.0),
        ..Default::default()
    }
}

/// 下拉面板：`control` 底 + 1px 发丝线 + popover 投影。
pub fn menu_panel(theme: &Theme) -> container::Style {
    let p = Palette::from(theme);
    container::Style {
        background: Some(Background::Color(p.control)),
        text_color: Some(p.text),
        border: Border {
            color: p.hairline,
            width: HAIRLINE,
            radius: radius(RADIUS_MENU),
        },
        shadow: shadow_popover(theme_is_dark(theme)),
        ..Default::default()
    }
}

/// 菜单行：悬停整行用 `accent` 填充（文字色由调用方按白色处理）。
pub fn menu_row(theme: &Theme, hovered: bool) -> container::Style {
    let p = Palette::from(theme);
    container::Style {
        background: hovered.then_some(Background::Color(p.accent)),
        text_color: Some(if hovered { Color::WHITE } else { p.text }),
        border: Border::default().rounded(4.0),
        ..Default::default()
    }
}

/// 弹窗底板：圆角 10 + popover 投影（规范 4.8）。
pub fn dialog(theme: &Theme) -> container::Style {
    let p = Palette::from(theme);
    container::Style {
        background: Some(Background::Color(p.control)),
        text_color: Some(p.text),
        border: Border {
            color: p.hairline,
            width: HAIRLINE,
            radius: radius(RADIUS_DIALOG),
        },
        shadow: shadow_popover(true),
        ..Default::default()
    }
}

/// 弹窗遮罩。
pub fn dialog_backdrop(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(rgba(0x000000, 0.28))),
        ..Default::default()
    }
}

/// 快捷键键帽（规范 4.8：track 底 + 1px 描边）。
pub fn keycap(theme: &Theme) -> container::Style {
    let p = Palette::from(theme);
    container::Style {
        background: Some(Background::Color(p.track)),
        text_color: Some(p.text2),
        border: Border {
            color: p.hairline,
            width: HAIRLINE,
            radius: radius(4.0),
        },
        ..Default::default()
    }
}

// ---------- 投影 ----------

fn shadow_selected() -> Shadow {
    Shadow {
        color: rgba(0x000000, 0.22),
        offset: Vector::new(0.0, 2.0),
        blur_radius: 8.0,
    }
}

fn shadow_popover(dark: bool) -> Shadow {
    Shadow {
        color: rgba(0x000000, if dark { 0.45 } else { 0.18 }),
        offset: Vector::new(0.0, 8.0),
        blur_radius: 24.0,
    }
}
