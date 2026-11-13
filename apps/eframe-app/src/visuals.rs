//! 尺寸、控件状态、动效时长与高 DPI 取整的单一真值来源。
//!
//! 本模块只依赖 `theme::Palette`（色板）与 egui 基础类型，不参与任何布局切分，
//! 也不缓存颜色或文本；所有绘制模块在每帧绘制前从这里取常量与辅助函数。

use eframe::egui::{self, Color32, Rect, Stroke};

use crate::theme::Palette;

// ---------------------------------------------------------------------------
// 分区尺寸（单位：逻辑点 pt）
// ---------------------------------------------------------------------------

pub const TOPBAR_H: f32 = 56.0;
pub const STATUS_H: f32 = 28.0;
pub const STRIP_H: f32 = 96.0;
pub const TITLEBAR_H: f32 = 36.0;

pub const WINDOW_MARGIN: f32 = 16.0;
pub const GROUP_GAP: f32 = 12.0;
pub const IN_GROUP_GAP: f32 = 2.0;
pub const MORE_GAP: f32 = 16.0;

pub const BTN_SIZE: f32 = 32.0;
pub const BTN_RADIUS: f32 = 8.0;
pub const CAPSULE_H: f32 = 36.0;
pub const CAPSULE_RADIUS: f32 = 10.0;
pub const CAPSULE_PAD_V: f32 = 2.0;
pub const CAPSULE_PAD_H: f32 = 4.0;

pub const ZOOM_W: f32 = 132.0;
pub const ZOOM_H: f32 = 36.0;
pub const ZOOM_RADIUS: f32 = 10.0;
pub const ZOOM_VALUE_W: f32 = 56.0;
pub const ZOOM_VALUE_H: f32 = 30.0;
pub const ZOOM_VALUE_RADIUS: f32 = 7.0;
pub const ZOOM_BTN_W: f32 = 38.0;
pub const ZOOM_BTN_H: f32 = 36.0;

pub const SORT_MIN_W: f32 = 64.0;
pub const SORT_H: f32 = 36.0;

pub const ICON_GRID: f32 = 20.0;
pub const ICON_FRAME: f32 = 18.0;
pub const ICON_STROKE: f32 = 1.6;

pub const THUMB_W: f32 = 80.0;
pub const THUMB_H: f32 = 60.0;
pub const THUMB_RADIUS: f32 = 8.0;
pub const THUMB_GAP: f32 = 12.0;
pub const THUMB_STEP: f32 = THUMB_W + THUMB_GAP;
pub const THUMB_PAD: f32 = 2.0;
pub const THUMB_PAD_H: f32 = 12.0;
pub const THUMB_PAD_V: f32 = 18.0;
pub const RING_EXPAND: f32 = 3.0;
pub const RING_RADIUS: f32 = 11.0;
pub const RING_W: f32 = 2.0;

pub const MENU_MIN_W: f32 = 232.0;
pub const MENU_RADIUS: f32 = 12.0;
pub const MENU_ITEM_H: f32 = 32.0;
pub const MENU_ITEM_RADIUS: f32 = 6.0;
pub const MENU_GROUP_GAP: f32 = 8.0;
pub const MENU_PAD: f32 = 8.0;
pub const MENU_ICON_SLOT: f32 = 20.0;
pub const MENU_ICON_GAP: f32 = 10.0;

pub const CHECK_SIZE: f32 = 16.0;
pub const CHECK_RADIUS: f32 = 4.0;
pub const RADIO_RADIUS: f32 = 8.0;
pub const RADIO_DOT: f32 = 6.0;

pub const PRIMARY_BTN_MIN_W: f32 = 96.0;
pub const PRIMARY_BTN_H: f32 = 36.0;
pub const PRIMARY_BTN_RADIUS: f32 = 9.0;
pub const EMPTY_BTN_W: f32 = 132.0;
pub const EMPTY_BTN_H: f32 = 36.0;
pub const EMPTY_ICON_BOX: f32 = 96.0;
pub const EMPTY_ICON_BOX_RADIUS: f32 = 24.0;
pub const EMPTY_ICON_SIZE: f32 = 40.0;

pub const TOAST_H: f32 = 36.0;
pub const TOAST_MIN_W: f32 = 200.0;
pub const TOAST_MAX_W: f32 = 320.0;
pub const TOAST_RADIUS: f32 = 10.0;
pub const CARD_MIN_H: f32 = 44.0;
pub const CARD_MAX_H: f32 = 96.0;
pub const CARD_RADIUS: f32 = 10.0;
pub const CARD_MARGIN: f32 = 12.0;
pub const TITLE_CAP_H: f32 = 32.0;
pub const TITLE_CAP_MIN_W: f32 = 160.0;
pub const TITLE_CAP_RADIUS: f32 = 16.0;
pub const TITLE_CAP_TOP: f32 = 16.0;

pub const STRUCTURE_LINE: f32 = 1.0;
pub const CHECKER_CELL: f32 = 16.0;
pub const NARROW_SORT_W: f32 = 720.0;
pub const NARROW_COLLAPSE_W: f32 = 560.0;

// 字号组（pt）
pub const FS_MIN: f32 = 10.5;
pub const FS_STATUS: f32 = 11.5;
pub const FS_BODY: f32 = 12.5;
pub const FS_BUTTON: f32 = 13.0;
pub const FS_TITLE: f32 = 17.0;

// ---------------------------------------------------------------------------
// 控件六态
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlState {
    Default,
    Hovered,
    Pressed,
    Selected,
    Disabled,
    Focused,
}

/// 六态解析的控件种类词表；`resolve_state` 用它决定圆角与描边策略。
/// 部分种类由菜单/浮层/窗口按语义选用，当前实现未全部实例化，按设计保留。
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetKind {
    IconButton,
    ZoomSegment,
    SortButton,
    ThumbCard,
    MenuItem,
    Checkbox,
    Radio,
    PrimaryButton,
    Capsule,
}

#[derive(Clone, Copy, Debug)]
pub struct StateStyle {
    pub bg: Color32,
    pub stroke: Option<Stroke>,
    pub fg: Color32,
    pub radius: u8,
    pub focus_ring: bool,
}

fn kind_radius(kind: WidgetKind) -> u8 {
    match kind {
        WidgetKind::IconButton | WidgetKind::SortButton => BTN_RADIUS as u8,
        WidgetKind::ZoomSegment => ZOOM_VALUE_RADIUS as u8,
        WidgetKind::ThumbCard => THUMB_RADIUS as u8,
        WidgetKind::MenuItem => MENU_ITEM_RADIUS as u8,
        WidgetKind::Checkbox => CHECK_RADIUS as u8,
        WidgetKind::Radio => RADIO_RADIUS as u8,
        WidgetKind::PrimaryButton => PRIMARY_BTN_RADIUS as u8,
        WidgetKind::Capsule => CAPSULE_RADIUS as u8,
    }
}

/// 六态矩阵的唯一出口。
///
/// 关键约束：开启/选中态使用「强调浅底 + 强调色图标/描边」，
/// hover 态使用「中性叠色」，二者底色必须不同（PRD §5.7 / B1）。
pub fn resolve_state(p: &Palette, state: ControlState, kind: WidgetKind) -> StateStyle {
    let radius = kind_radius(kind);
    match state {
        ControlState::Disabled => StateStyle {
            bg: Color32::TRANSPARENT,
            stroke: None,
            fg: with_alpha(p.text3, 102),
            radius,
            focus_ring: false,
        },
        ControlState::Hovered => StateStyle {
            bg: p.hover,
            stroke: match kind {
                WidgetKind::ThumbCard | WidgetKind::MenuItem => {
                    Some(Stroke::new(STRUCTURE_LINE, p.control_border))
                }
                _ => None,
            },
            fg: p.text,
            radius,
            focus_ring: false,
        },
        ControlState::Pressed => StateStyle {
            bg: p.press,
            stroke: None,
            fg: p.text,
            radius,
            focus_ring: false,
        },
        ControlState::Selected => StateStyle {
            bg: p.accent_soft,
            stroke: Some(Stroke::new(STRUCTURE_LINE, with_alpha(p.accent, 115))),
            fg: p.accent,
            radius,
            focus_ring: false,
        },
        ControlState::Focused => StateStyle {
            bg: Color32::TRANSPARENT,
            stroke: None,
            fg: p.icon,
            radius,
            focus_ring: true,
        },
        ControlState::Default => StateStyle {
            bg: Color32::TRANSPARENT,
            stroke: None,
            fg: p.icon,
            radius,
            focus_ring: false,
        },
    }
}

/// 保留 RGB，仅替换 alpha（用于「强调色 @45%」等叠色描边）。
pub fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

// ---------------------------------------------------------------------------
// 动效
// ---------------------------------------------------------------------------

/// 全部动效时长的唯一常量表；所有过渡动效 ≤ `MAX_MS`。
///
/// `*_HOLD_MS` 是「状态保持时长」，不是过渡动效，因此不受 600 ms 约束。
/// 装饰性动效与状态过渡时长按设计集中登记，本期默认关闭/由 egui 原生插值，
/// 故部分字段只作为常量表的一部分保留，不在此处逐一读取。
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct Motion {
    pub hover_ms: u32,
    pub press_ms: u32,
    pub select_ms: u32,
    pub menu_ms: u32,
    pub toast_in_ms: u32,
    pub toast_hold_ms: u32,
    pub toast_out_ms: u32,
    pub title_in_ms: u32,
    pub title_hold_ms: u32,
    pub title_out_ms: u32,
    pub error_in_ms: u32,
    pub fullscreen_chrome_ms: u32,
    pub theme_crossfade_ms: u32,
    pub skeleton_ms: u32,
}

impl Motion {
    /// 过渡动效上限；供后续动画实现校验，本期由 egui 原生插值保证不超时。
    #[allow(dead_code)]
    pub const MAX_MS: u32 = 600;

    /// 装饰性动效（主题交叉淡入、占位骨架微光）默认不实现；常量仅用于降级路径。
    pub const fn reduced() -> Self {
        Self {
            hover_ms: 0,
            press_ms: 0,
            select_ms: 0,
            menu_ms: 0,
            toast_in_ms: 0,
            toast_hold_ms: 4000,
            toast_out_ms: 0,
            title_in_ms: 0,
            title_hold_ms: 3000,
            title_out_ms: 0,
            error_in_ms: 0,
            fullscreen_chrome_ms: 0,
            theme_crossfade_ms: 0,
            skeleton_ms: 0,
        }
    }

    pub const fn toast_total_ms(&self) -> u32 {
        self.toast_in_ms + self.toast_hold_ms + self.toast_out_ms
    }

    pub const fn title_total_ms(&self) -> u32 {
        self.title_in_ms + self.title_hold_ms + self.title_out_ms
    }
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            hover_ms: 120,
            press_ms: 0,
            select_ms: 160,
            menu_ms: 140,
            // Z1：按零回退「总时长不变」取 160/3520/320 = 4000 ms。
            toast_in_ms: 160,
            toast_hold_ms: 3520,
            toast_out_ms: 320,
            title_in_ms: 200,
            title_hold_ms: 2400,
            title_out_ms: 400,
            error_in_ms: 180,
            fullscreen_chrome_ms: 600,
            // 装饰动效默认关闭：保留 0 表示不启用。
            theme_crossfade_ms: 0,
            skeleton_ms: 0,
        }
    }
}

/// 跨帧稳定的临时状态 / 浮层 ID。
///
/// egui 0.37 将 `Id::new` 标记为 deprecated，但 `ctx.data` 的键与 `Area` 的
/// 身份必须是确定性的，不能用每次调用都变化的 `Id::unique`；这里集中保留
/// 唯一的确定性构造点。
#[allow(deprecated)]
pub fn stable_id(name: &'static str) -> egui::Id {
    egui::Id::new(name)
}

fn reduced_motion_id() -> egui::Id {
    stable_id("iv_reduced_motion")
}

fn fullscreen_id() -> egui::Id {
    stable_id("iv_fullscreen_active")
}

fn keyboard_nav_id() -> egui::Id {
    stable_id("iv_keyboard_nav")
}

/// 设置「减少动态效果」标志（Q6：本期不新增设置项，默认 false，仅保留降级入口）。
pub fn set_reduced_motion(ctx: &egui::Context, reduced: bool) {
    ctx.data_mut(|data| data.insert_temp(reduced_motion_id(), reduced));
}

/// 当前生效的动效常量表。
pub fn motion(ctx: &egui::Context) -> Motion {
    let reduced = ctx.data(|data| data.get_temp::<bool>(reduced_motion_id()).unwrap_or(false));
    if reduced {
        Motion::reduced()
    } else {
        Motion::default()
    }
}

/// 三段 alpha（淡入 / 停留 / 淡出）；返回 `None` 表示动效已结束、应完全移除绘制与命中区。
pub fn three_phase_alpha(elapsed_ms: f32, in_ms: f32, hold_ms: f32, out_ms: f32) -> Option<f32> {
    let total = in_ms + hold_ms + out_ms;
    if elapsed_ms >= total {
        return None;
    }
    if in_ms > 0.0 && elapsed_ms < in_ms {
        let t = (elapsed_ms / in_ms).clamp(0.0, 1.0);
        // ease-out：快速起、缓收尾。
        return Some(1.0 - (1.0 - t) * (1.0 - t));
    }
    let after_in = elapsed_ms - in_ms;
    if after_in < hold_ms {
        return Some(1.0);
    }
    if out_ms > 0.0 {
        let t = ((after_in - hold_ms) / out_ms).clamp(0.0, 1.0);
        // ease-in：慢起、快收尾，直至 alpha=0。
        return Some((1.0 - t * t).max(0.0));
    }
    None
}

/// 错误卡片只有一次性淡入，淡入完成后常驻（Q7）。
pub fn error_alpha(elapsed_ms: f32, in_ms: f32) -> f32 {
    if in_ms <= 0.0 {
        return 1.0;
    }
    let t = (elapsed_ms / in_ms).clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

// ---------------------------------------------------------------------------
// 每帧状态（全屏 / 键盘导航），全部走 ctx.data，不写进 ToolbarState / Settings
// ---------------------------------------------------------------------------

pub fn set_fullscreen(ctx: &egui::Context, fullscreen: bool) {
    ctx.data_mut(|data| data.insert_temp(fullscreen_id(), fullscreen));
}

pub fn fullscreen_active(ctx: &egui::Context) -> bool {
    ctx.data(|data| data.get_temp::<bool>(fullscreen_id()).unwrap_or(false))
}

/// 每帧根据输入更新「最近输入是否为键盘」标志：按下鼠标即清除，
/// 保证鼠标点击不出现焦点环（B2 / R-5）。
pub fn update_keyboard_nav(ctx: &egui::Context) {
    let used_keyboard = ctx.input(|input| {
        input
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::Key { pressed: true, .. }))
    });
    let pointer = ctx.input(|input| input.pointer.any_pressed() || input.pointer.any_click());
    ctx.data_mut(|data| {
        let current = data.get_temp::<bool>(keyboard_nav_id()).unwrap_or(false);
        let next = if pointer {
            false
        } else if used_keyboard {
            true
        } else {
            current
        };
        data.insert_temp(keyboard_nav_id(), next);
    });
}

pub fn keyboard_nav(ctx: &egui::Context) -> bool {
    ctx.data(|data| data.get_temp::<bool>(keyboard_nav_id()).unwrap_or(false))
}

/// 全屏时框架底色替换为 `mask`（@72%），并把前景换成亮色，保证文字 100% 可读。
/// 其余主题 token 保持不变；每帧调用，不跨帧缓存。
pub fn chrome_palette(ctx: &egui::Context, p: Palette) -> Palette {
    if !fullscreen_active(ctx) {
        return p;
    }
    Palette {
        chrome: p.mask,
        strip: p.mask,
        status: p.mask,
        text: p.on_accent,
        text2: Color32::from_white_alpha(204),
        text3: Color32::from_white_alpha(150),
        icon: p.on_accent,
        hover: Color32::from_white_alpha(26),
        press: Color32::from_white_alpha(46),
        border: Color32::from_white_alpha(31),
        structure_stroke: Color32::from_white_alpha(31),
        control: Color32::from_white_alpha(20),
        control_border: Color32::from_white_alpha(46),
        tile: Color32::from_white_alpha(20),
        tile_border: Color32::from_white_alpha(46),
        track: Color32::from_white_alpha(20),
        ..p
    }
}

// ---------------------------------------------------------------------------
// 高 DPI 取整（PRD §5.10）
// ---------------------------------------------------------------------------

/// `round(v × ppp) / ppp`，用于所有绘制几何；命中区不得调用。
pub fn snap(v: f32, ppp: f32) -> f32 {
    if ppp <= 0.0 {
        return v;
    }
    (v * ppp).round() / ppp
}

pub fn snap_rect(rect: Rect, ppp: f32) -> Rect {
    Rect::from_min_max(
        egui::pos2(snap(rect.min.x, ppp), snap(rect.min.y, ppp)),
        egui::pos2(snap(rect.max.x, ppp), snap(rect.max.y, ppp)),
    )
}

/// `CornerRadius` 以逻辑点为单位；这里把半径取整到物理像素对齐的逻辑值。
pub fn snap_radius(radius: f32, ppp: f32) -> u8 {
    snap(radius, ppp).round().max(0.0) as u8
}

/// 1 pt 结构线的中心：整数 ppp 落在物理像素中心，分数 ppp 落在整数边界。
pub fn snap_line_center(x: f32, ppp: f32) -> f32 {
    if ppp <= 0.0 {
        return x;
    }
    if (ppp - ppp.round()).abs() < 1e-4 {
        ((x * ppp).round() + 0.5) / ppp
    } else {
        (x * ppp).round() / ppp
    }
}

/// 图标线宽：`max(1 物理像素, 1.6pt × ppp)`，结果换算回逻辑点。
pub fn snap_stroke(pt: f32, ppp: f32) -> f32 {
    if ppp <= 0.0 {
        return pt;
    }
    (pt * ppp).max(1.0) / ppp
}

// ---------------------------------------------------------------------------
// 画布内浮层几何（全部相对 canvas_rect，含错误卡片与文件名胶囊互斥避让）
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct OverlayGeometry {
    pub error: Rect,
    pub toast: Rect,
    pub title: Rect,
}

pub fn overlay_geometry(
    canvas_rect: Rect,
    error_height: f32,
    has_error: bool,
    toast_width: f32,
    title_width: f32,
) -> OverlayGeometry {
    let margin = CARD_MARGIN;
    let error = Rect::from_min_size(
        egui::pos2(canvas_rect.min.x + margin, canvas_rect.min.y + margin),
        egui::vec2(
            (canvas_rect.width() - 2.0 * margin).max(0.0),
            error_height.clamp(CARD_MIN_H, CARD_MAX_H),
        ),
    );

    // C-7：错误卡片存在时，文件名胶囊整体下移（错误态优先）。
    let title_top = if has_error {
        error.max.y + GROUP_GAP
    } else {
        canvas_rect.min.y + TITLE_CAP_TOP
    };
    let title_w = title_width.max(TITLE_CAP_MIN_W);
    let title = Rect::from_min_size(
        egui::pos2(canvas_rect.center().x - title_w / 2.0, title_top),
        egui::vec2(title_w, TITLE_CAP_H),
    );

    // C-4：画布底边即缩略图条/状态栏顶边，故无需感知条显隐。
    let toast_w = toast_width.clamp(TOAST_MIN_W, TOAST_MAX_W);
    let toast = Rect::from_min_size(
        egui::pos2(
            canvas_rect.max.x - margin - toast_w,
            canvas_rect.max.y - margin - TOAST_H,
        ),
        egui::vec2(toast_w, TOAST_H),
    );

    OverlayGeometry {
        error,
        toast,
        title,
    }
}

// ---------------------------------------------------------------------------
// QA 回归测试：UI 重设计（暗房 · Depth Frame）
// ---------------------------------------------------------------------------

#[cfg(test)]
mod qa_redesign_tests {
    use super::*;
    use crate::theme::Palette;

    const PPPS: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

    /// egui 0.37 的 `FullOutput` 若直接 drop，会因未应用的字体纹理 delta 而 panic。
    fn run_ui(ctx: &egui::Context, input: egui::RawInput, body: impl FnMut(&mut egui::Ui)) {
        ctx.run_ui(input, body).drop_without_applying_deltas();
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn headless_input() -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        }
    }

    #[test]
    fn snap_aligns_every_value_to_a_whole_physical_pixel() {
        for ppp in PPPS {
            for value in [0.0, 1.0, 8.0, 10.5, 12.0, 56.0, 92.0, 132.0, 232.0] {
                let snapped = snap(value, ppp);
                let physical = snapped * ppp;
                assert!(
                    close(physical, physical.round()),
                    "ppp={ppp} v={value} -> {snapped}pt = {physical}px"
                );
            }
        }
        assert!(close(snap(8.4, 1.25), 8.8), "10.5px -> 11px -> 8.8pt");
        assert!(close(snap(10.4, 1.5), 16.0 / 1.5), "15.6px -> 16px");
        assert_eq!(snap(7.0, 0.0), 7.0, "ppp<=0 时原值返回");
    }

    #[test]
    fn snap_rect_aligns_all_four_edges() {
        let rect = Rect::from_min_max(egui::pos2(3.3, 7.7), egui::pos2(135.4, 103.1));
        for ppp in PPPS {
            let snapped = snap_rect(rect, ppp);
            for value in [snapped.min.x, snapped.min.y, snapped.max.x, snapped.max.y] {
                assert!(
                    close(value * ppp, (value * ppp).round()),
                    "ppp={ppp} edge={value}"
                );
            }
        }
    }

    #[test]
    fn snap_radius_is_the_rounded_snap_result() {
        for ppp in PPPS {
            for radius in [0.0_f32, 4.0, 8.0, 10.0, 12.0, 16.0, 24.0] {
                assert_eq!(
                    snap_radius(radius, ppp),
                    snap(radius, ppp).round().max(0.0) as u8,
                    "ppp={ppp} radius={radius}"
                );
            }
        }
    }

    #[test]
    fn snap_line_center_uses_half_pixel_for_integer_ppp() {
        for x in [0.0_f32, 12.5, 56.0, 100.3] {
            assert!(close(snap_line_center(x, 1.0), x.round() + 0.5));
            assert!(close(
                snap_line_center(x, 2.0),
                ((x * 2.0).round() + 0.5) / 2.0
            ));
            assert!(close(snap_line_center(x, 1.25), (x * 1.25).round() / 1.25));
            assert_eq!(snap_line_center(x, 0.0), x);
        }
    }

    #[test]
    fn snap_stroke_is_never_thinner_than_one_physical_pixel() {
        for ppp in PPPS {
            let width = snap_stroke(ICON_STROKE, ppp);
            assert!(width * ppp >= 1.0 - 1e-4, "ppp={ppp} width={width}");
        }
        assert_eq!(snap_stroke(1.6, 0.0), 1.6);
    }

    #[test]
    fn resolve_state_selected_background_differs_from_hover_in_both_themes() {
        for palette in [Palette::light(), Palette::dark()] {
            for kind in [
                WidgetKind::IconButton,
                WidgetKind::MenuItem,
                WidgetKind::Checkbox,
                WidgetKind::Radio,
                WidgetKind::ThumbCard,
                WidgetKind::PrimaryButton,
            ] {
                let hovered = resolve_state(&palette, ControlState::Hovered, kind);
                let selected = resolve_state(&palette, ControlState::Selected, kind);
                assert_ne!(
                    hovered.bg, selected.bg,
                    "B1: {kind:?} 开启态与 hover 底色必须不同"
                );
                assert_eq!(selected.bg, palette.accent_soft);
                assert_eq!(hovered.bg, palette.hover);
                assert_eq!(selected.fg, palette.accent);
            }
        }
    }

    #[test]
    fn resolve_state_six_states_are_pairwise_distinct() {
        let palette = Palette::light();
        let states = [
            ControlState::Default,
            ControlState::Hovered,
            ControlState::Pressed,
            ControlState::Selected,
            ControlState::Disabled,
            ControlState::Focused,
        ];
        let fingerprint = |style: StateStyle| {
            format!(
                "{:?}|{:?}|{:?}|{}|{}",
                style.bg, style.stroke, style.fg, style.radius, style.focus_ring
            )
        };
        let styles: Vec<String> = states
            .iter()
            .map(|state| fingerprint(resolve_state(&palette, *state, WidgetKind::IconButton)))
            .collect();
        for (index, first) in styles.iter().enumerate() {
            for second in styles.iter().skip(index + 1) {
                assert_ne!(first, second, "六态不得有两态完全一致：{first}");
            }
        }
        assert!(resolve_state(&palette, ControlState::Focused, WidgetKind::IconButton).focus_ring);
        assert_eq!(
            resolve_state(&palette, ControlState::Default, WidgetKind::IconButton).fg,
            palette.icon,
            "默认态禁止强调色图标"
        );
    }

    #[test]
    fn three_phase_alpha_covers_fade_in_hold_fade_out_then_ends() {
        let motion = Motion::default();
        let (fade_in, hold, fade_out) = (
            motion.toast_in_ms as f32,
            motion.toast_hold_ms as f32,
            motion.toast_out_ms as f32,
        );
        assert_eq!(three_phase_alpha(0.0, fade_in, hold, fade_out), Some(0.0));
        let during_in = three_phase_alpha(fade_in / 2.0, fade_in, hold, fade_out).unwrap();
        assert!(during_in > 0.0 && during_in < 1.0, "淡入中：{during_in}");
        assert_eq!(
            three_phase_alpha(fade_in, fade_in, hold, fade_out),
            Some(1.0)
        );
        assert_eq!(
            three_phase_alpha(fade_in + hold / 2.0, fade_in, hold, fade_out),
            Some(1.0)
        );
        assert_eq!(
            three_phase_alpha(fade_in + hold, fade_in, hold, fade_out),
            Some(1.0)
        );
        let during_out =
            three_phase_alpha(fade_in + hold + fade_out / 2.0, fade_in, hold, fade_out).unwrap();
        assert!(during_out > 0.0 && during_out < 1.0, "淡出中：{during_out}");
        assert_eq!(
            three_phase_alpha(fade_in + hold + fade_out, fade_in, hold, fade_out),
            None,
            "总时长结束必须返回 None（不注册命中区）"
        );
        assert_eq!(
            three_phase_alpha(fade_in + hold + fade_out + 1.0, fade_in, hold, fade_out),
            None
        );
        assert_eq!(three_phase_alpha(-10.0, fade_in, hold, fade_out), Some(0.0));
        assert_eq!(
            three_phase_alpha(fade_in + hold, fade_in, hold, 0.0),
            None,
            "无淡出段时停留结束即终止"
        );
    }

    #[test]
    fn error_alpha_fades_in_once_then_stays_opaque() {
        assert!(close(error_alpha(0.0, 180.0), 0.0));
        assert!(close(error_alpha(90.0, 180.0), 0.75));
        assert!(close(error_alpha(180.0, 180.0), 1.0));
        assert!(close(error_alpha(10_000.0, 180.0), 1.0), "错误卡片常驻");
        assert!(close(error_alpha(0.0, 0.0), 1.0), "减少动效时直接终态");
    }

    #[test]
    fn motion_totals_match_the_four_and_three_second_budget() {
        let motion = Motion::default();
        assert_eq!(
            (
                motion.toast_in_ms,
                motion.toast_hold_ms,
                motion.toast_out_ms
            ),
            (160, 3520, 320)
        );
        assert_eq!(
            (
                motion.title_in_ms,
                motion.title_hold_ms,
                motion.title_out_ms
            ),
            (200, 2400, 400)
        );
        assert_eq!(motion.toast_total_ms(), 4000, "Z1 口径：toast 4.0s");
        assert_eq!(motion.title_total_ms(), 3000, "文件名胶囊 3.0s");
        assert_eq!(motion.error_in_ms, 180);
        assert_eq!(Motion::MAX_MS, 600);
        for ms in [
            motion.hover_ms,
            motion.press_ms,
            motion.select_ms,
            motion.menu_ms,
            motion.toast_in_ms,
            motion.toast_out_ms,
            motion.title_in_ms,
            motion.title_out_ms,
            motion.error_in_ms,
            motion.fullscreen_chrome_ms,
            motion.theme_crossfade_ms,
            motion.skeleton_ms,
        ] {
            assert!(ms <= Motion::MAX_MS, "过渡动效 {ms}ms 超过 600ms");
        }
    }

    #[test]
    fn reduced_motion_zeroes_transitions_without_changing_function_durations() {
        let reduced = Motion::reduced();
        for ms in [
            reduced.hover_ms,
            reduced.press_ms,
            reduced.select_ms,
            reduced.menu_ms,
            reduced.toast_in_ms,
            reduced.toast_out_ms,
            reduced.title_in_ms,
            reduced.title_out_ms,
            reduced.error_in_ms,
            reduced.fullscreen_chrome_ms,
            reduced.theme_crossfade_ms,
            reduced.skeleton_ms,
        ] {
            assert_eq!(ms, 0, "Q6：减少动效时全部过渡置 0");
        }
        assert_eq!(reduced.toast_total_ms(), 4000);
        assert_eq!(reduced.title_total_ms(), 3000);
    }

    #[test]
    fn overlay_geometry_avoids_error_card_and_title_capsule_overlap() {
        let canvas = Rect::from_min_max(egui::pos2(0.0, 100.0), egui::pos2(900.0, 700.0));
        let with_error = overlay_geometry(canvas, 60.0, true, 240.0, 200.0);
        assert!(
            !with_error.error.intersects(with_error.title),
            "错误卡片与文件名胶囊不得重叠"
        );
        assert!(with_error.title.min.y >= with_error.error.max.y + GROUP_GAP - 1e-4);
        let without_error = overlay_geometry(canvas, 60.0, false, 240.0, 200.0);
        assert!(close(
            without_error.title.min.y,
            canvas.min.y + TITLE_CAP_TOP
        ));
        assert!(close(without_error.error.min.x, canvas.min.x + CARD_MARGIN));
        assert!(close(
            without_error.error.width(),
            canvas.width() - 2.0 * CARD_MARGIN
        ));
    }

    #[test]
    fn overlay_geometry_clamps_sizes_and_anchors_toast_bottom_right() {
        let canvas = Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(810.0, 620.0));
        for (width, expected) in [(50.0, TOAST_MIN_W), (240.0, 240.0), (999.0, TOAST_MAX_W)] {
            let geometry = overlay_geometry(canvas, 44.0, false, width, 200.0);
            assert!(close(geometry.toast.width(), expected));
            assert!(close(geometry.toast.max.x, canvas.max.x - CARD_MARGIN));
            assert!(close(geometry.toast.max.y, canvas.max.y - CARD_MARGIN));
            assert!(close(geometry.toast.height(), TOAST_H));
        }
        assert!(close(
            overlay_geometry(canvas, 1.0, false, 200.0, 160.0)
                .error
                .height(),
            CARD_MIN_H
        ));
        assert!(close(
            overlay_geometry(canvas, 500.0, false, 200.0, 160.0)
                .error
                .height(),
            CARD_MAX_H
        ));
        assert!(close(
            overlay_geometry(canvas, 44.0, false, 200.0, 10.0)
                .title
                .width(),
            TITLE_CAP_MIN_W
        ));
    }

    #[test]
    fn size_constants_match_prd_section_5() {
        let expected = [
            (TOPBAR_H, 56.0, "顶栏"),
            (STRIP_H, 96.0, "缩略图条"),
            (STATUS_H, 28.0, "状态栏"),
            (TITLEBAR_H, 36.0, "Linux 标题栏"),
            (BTN_SIZE, 32.0, "图标按钮"),
            (CAPSULE_H, 36.0, "胶囊高"),
            (CAPSULE_RADIUS, 10.0, "胶囊圆角"),
            (ZOOM_W, 132.0, "缩放分段宽"),
            (ZOOM_H, 36.0, "缩放分段高"),
            (ZOOM_VALUE_W, 56.0, "值块宽"),
            (ZOOM_VALUE_H, 30.0, "值块高"),
            (THUMB_W, 80.0, "卡片宽"),
            (THUMB_H, 60.0, "卡片高"),
            (THUMB_GAP, 12.0, "卡片间隙"),
            (THUMB_STEP, 92.0, "卡片步进"),
            (MENU_MIN_W, 232.0, "菜单最小宽"),
            (MENU_RADIUS, 12.0, "菜单圆角"),
            (MENU_ITEM_H, 32.0, "菜单项高"),
            (CHECK_SIZE, 16.0, "复选框"),
            (EMPTY_ICON_BOX, 96.0, "空状态容器"),
            (TOAST_H, 36.0, "toast 高"),
            (TITLE_CAP_H, 32.0, "文件名胶囊高"),
            (ICON_GRID, 20.0, "图标网格"),
            (ICON_FRAME, 18.0, "图标绘制框"),
            (ICON_STROKE, 1.6, "图标线宽"),
        ];
        for (actual, want, label) in expected {
            assert!(close(actual, want), "{label}: 期望 {want}，实际 {actual}");
        }
        assert!(close(THUMB_STEP, THUMB_W + THUMB_GAP));
        assert!(close(GROUP_GAP, 12.0));
        assert!(close(IN_GROUP_GAP, 2.0));
        assert!(close(MORE_GAP, 16.0));
        assert!(close(WINDOW_MARGIN, 16.0));
    }

    #[test]
    fn stable_ids_are_deterministic_and_distinct() {
        assert_eq!(stable_id("canvas_overlays"), stable_id("canvas_overlays"));
        assert_ne!(
            stable_id("canvas_overlays"),
            stable_id("iv_fullscreen_active")
        );
    }

    #[test]
    fn fullscreen_chrome_palette_uses_the_72_percent_mask() {
        let ctx = egui::Context::default();
        let palette = Palette::light();
        assert_eq!(palette.mask.a(), 184, "@72% ≈ 184/255");
        run_ui(&ctx, headless_input(), |ui| {
            set_fullscreen(ui.ctx(), false);
            assert_eq!(chrome_palette(ui.ctx(), palette).chrome, palette.chrome);
            set_fullscreen(ui.ctx(), true);
            let fullscreen = chrome_palette(ui.ctx(), palette);
            assert_eq!(fullscreen.chrome, palette.mask);
            assert_eq!(fullscreen.strip, palette.mask);
            assert_eq!(fullscreen.status, palette.mask);
            assert_eq!(fullscreen.text, palette.on_accent);
        });
    }

    #[test]
    fn motion_respects_the_reduced_motion_flag() {
        let ctx = egui::Context::default();
        run_ui(&ctx, headless_input(), |ui| {
            set_reduced_motion(ui.ctx(), false);
            assert_eq!(motion(ui.ctx()).error_in_ms, 180);
            set_reduced_motion(ui.ctx(), true);
            assert_eq!(motion(ui.ctx()).error_in_ms, 0);
            assert_eq!(motion(ui.ctx()).toast_total_ms(), 4000);
        });
    }
}
