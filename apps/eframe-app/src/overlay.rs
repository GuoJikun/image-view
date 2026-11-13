//! 画布内浮层（错误卡片 / toast / 全屏文件名胶囊）与菜单共用件。
//!
//! 关键约定：三类画布浮层一律用 `egui::Area`（`Order::Middle`）叠加在
//! `viewer::CanvasOutput::canvas_rect` 之上，**不参与任何布局切分**；
//! 菜单 / 右键 / 窗口交给 egui 原生 Foreground 层。所有颜色与文案每帧从
//! `theme::palette` / `i18n::t` 取得，不跨帧缓存。

use std::time::Instant;

use eframe::egui::{
    self, Align2, Area, Color32, CornerRadius, FontId, Order, Rect, Response, Sense, Shape, Stroke,
    StrokeKind, Vec2,
};
use iv_gui_contract::Action;

use crate::i18n::{self, Key};
use crate::icons::{self, Icon};
use crate::theme::Palette;
use crate::visuals::{self, ControlState, Motion, WidgetKind};

// ---------------------------------------------------------------------------
// 浮层状态
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct OverlayState {
    pub canvas_rect: Rect,
    pub palette: Palette,
    pub error: Option<ErrorCardState>,
    pub toast: Option<ToastState>,
    pub title: Option<TitleCapsuleState>,
    pub motion: Motion,
}

#[derive(Clone, Debug)]
pub struct ErrorCardState {
    pub text: String,
    pub can_retry: bool,
    pub since: Instant,
}

#[derive(Clone, Debug)]
pub struct ToastState {
    pub text: String,
    pub since: Instant,
}

#[derive(Clone, Debug)]
pub struct TitleCapsuleState {
    pub text: String,
    pub alpha: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorAction {
    None,
    Dismiss,
    Retry,
}

// ---------------------------------------------------------------------------
// 统一绘制入口
// ---------------------------------------------------------------------------

/// 在所有 Panel 与 CentralPanel 之后调用；返回错误卡片上的用户动作。
///
/// 浮层拆成两个 Area（egui 0.37 下 `Area::interactable(false)` 会使整个 layer
/// 点击穿透，子 `ui.interact` 收不到点击，因此交互控件必须放在
/// `interactable(true)` 的 Area 内）：
///
/// 1. **绘制型浮层**（`canvas_overlays`，`interactable(false)`）：承载 toast 与
///    全屏文件名胶囊。二者均无交互控件；alpha ≤ 0 时不绘制、不注册任何 interact，
///    画布点击 / 缩放 / 拖拽完全不受影响。
/// 2. **交互型错误卡片**（`error_card_overlay`，`interactable(true)`）：仅当错误
///    存在且已淡入（`alpha > 0`）时创建，Area 的 rect 严格收缩到卡片包围盒
///    （`fixed_pos` + `default_size`），不覆盖画布其余区域；卡片之外的画布仍可
///    缩放 / 平移，只有关闭 / 重试两个按钮注册了 interact（B7）。
pub fn draw_all(ctx: &egui::Context, state: OverlayState) -> ErrorAction {
    let canvas_rect = state.canvas_rect;
    if canvas_rect.width() <= 0.0 || canvas_rect.height() <= 0.0 {
        return ErrorAction::None;
    }

    let palette = state.palette;
    let motion = state.motion;
    let now = Instant::now();
    let ppp = ctx.pixels_per_point();

    // 绘制型浮层（interactable=false）：toast 与全屏文件名胶囊。
    // 同时在此统一测算三类浮层尺寸并产出几何，供下方交互型错误卡片 Area 复用，
    // 保证像素一致、避免重复测量。
    let geometry = Area::new(visuals::stable_id("canvas_overlays"))
        .order(Order::Middle)
        .fixed_pos(canvas_rect.min)
        .interactable(false)
        .fade_in(false)
        .show(ctx, |ui| {
            let painter = ui.painter().clone();

            let card_w = (canvas_rect.width() - 2.0 * visuals::CARD_MARGIN).max(0.0);
            let error_h = match &state.error {
                Some(error) => error_card_height(&painter, &palette, error, card_w),
                None => visuals::CARD_MIN_H,
            };
            let toast_w = match &state.toast {
                Some(toast) => toast_width(&painter, &palette, &toast.text),
                None => visuals::TOAST_MIN_W,
            };
            let title_w = match &state.title {
                Some(title) => title_width(&painter, &title.text),
                None => visuals::TITLE_CAP_MIN_W,
            };
            let geometry = visuals::overlay_geometry(
                canvas_rect,
                error_h,
                state.error.is_some(),
                toast_w,
                title_w,
            );

            if let Some(title) = &state.title
                && title.alpha > 0.0
            {
                title_capsule(
                    ui,
                    visuals::snap_rect(geometry.title, ppp),
                    title,
                    &palette,
                    ppp,
                );
            }

            if let Some(toast) = &state.toast {
                let elapsed = now.saturating_duration_since(toast.since).as_secs_f32() * 1000.0;
                let alpha = visuals::three_phase_alpha(
                    elapsed,
                    motion.toast_in_ms as f32,
                    motion.toast_hold_ms as f32,
                    motion.toast_out_ms as f32,
                );
                // alpha ≤ 0 时不注册任何 interact / 不绘制（B10）。
                if let Some(alpha) = alpha.filter(|value| *value > 0.0) {
                    toast_overlay(
                        ui,
                        visuals::snap_rect(geometry.toast, ppp),
                        toast,
                        &palette,
                        alpha,
                        ppp,
                    );
                }
            }

            geometry
        })
        .inner;

    // 交互型错误卡片（interactable=true）：仅当错误存在且已淡入（alpha > 0）时创建，
    // 否则完全不创建该 Area —— 画布点击 / 缩放 / 拖拽不受任何影响。
    if let Some(error) = &state.error {
        let elapsed = now.saturating_duration_since(error.since).as_secs_f32() * 1000.0;
        let alpha = visuals::error_alpha(elapsed, motion.error_in_ms as f32);
        if alpha > 0.0 {
            let card_rect = visuals::snap_rect(geometry.error, ppp);
            return Area::new(visuals::stable_id("error_card_overlay"))
                .order(Order::Middle)
                .fixed_pos(card_rect.min)
                .default_size(card_rect.size())
                .interactable(true)
                .fade_in(false)
                .show(ctx, |ui| {
                    error_card(ui, card_rect, error, &palette, alpha, ppp)
                })
                .inner;
        }
    }

    ErrorAction::None
}

// ---------------------------------------------------------------------------
// 四层柔和阴影
// ---------------------------------------------------------------------------

pub fn shadow(painter: &egui::Painter, rect: Rect, p: &Palette) {
    let ppp = painter.ctx().pixels_per_point();
    for (index, expand) in [16.0_f32, 11.0, 6.0, 2.0].into_iter().enumerate() {
        let outer = visuals::snap_rect(rect.expand(expand), ppp);
        let radius = visuals::snap_radius(visuals::CARD_RADIUS + expand / 2.0, ppp);
        painter.rect_filled(outer, CornerRadius::same(radius), p.shadow[index]);
    }
}

// ---------------------------------------------------------------------------
// 错误卡片
// ---------------------------------------------------------------------------

fn button_width(painter: &egui::Painter, label: &str, p: &Palette) -> f32 {
    let width = painter
        .layout_no_wrap(
            label.to_owned(),
            FontId::proportional(visuals::FS_BODY),
            p.text2,
        )
        .size()
        .x;
    (width + 16.0).max(44.0)
}

fn reserved_buttons_width(painter: &egui::Painter, p: &Palette, can_retry: bool) -> f32 {
    let close = button_width(painter, i18n::t(Key::Close), p);
    let retry = if can_retry {
        button_width(painter, i18n::t(Key::Retry), p) + 6.0
    } else {
        0.0
    };
    close + retry
}

fn error_card_text_wrap(card_w: f32, p: &Palette, can_retry: bool, painter: &egui::Painter) -> f32 {
    let reserved = reserved_buttons_width(painter, p, can_retry);
    (card_w - 42.0 - 12.0 - reserved).max(80.0)
}

fn error_card_height(
    painter: &egui::Painter,
    p: &Palette,
    state: &ErrorCardState,
    card_w: f32,
) -> f32 {
    let wrap = error_card_text_wrap(card_w, p, state.can_retry, painter);
    let galley = painter.layout(
        state.text.clone(),
        FontId::proportional(visuals::FS_BODY),
        p.danger_text,
        wrap,
    );
    (galley.size().y + 24.0).clamp(visuals::CARD_MIN_H, visuals::CARD_MAX_H)
}

fn error_card(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &ErrorCardState,
    p: &Palette,
    alpha: f32,
    ppp: f32,
) -> ErrorAction {
    let mut painter = ui.painter().clone();
    painter.set_opacity(alpha);

    shadow(&painter, rect, p);
    painter.rect(
        rect,
        CornerRadius::same(visuals::snap_radius(visuals::CARD_RADIUS, ppp)),
        p.danger_soft,
        Stroke::new(
            visuals::STRUCTURE_LINE,
            visuals::with_alpha(p.danger, 89), // @35%
        ),
        StrokeKind::Inside,
    );

    // 左侧 3 pt 危险竖条（高 = 卡片高 − 16）。
    let bar = Rect::from_min_size(
        egui::pos2(rect.left() + 8.0, rect.min.y + 8.0),
        egui::vec2(3.0, (rect.height() - 16.0).max(4.0)),
    );
    painter.rect_filled(bar, CornerRadius::same(2), p.danger);

    let icon_center = egui::pos2(rect.left() + 26.0, rect.center().y);
    icons::paint(
        &painter,
        Rect::from_center_size(icon_center, Vec2::splat(visuals::ICON_FRAME)),
        Icon::Warning,
        p.danger,
    );

    let close_label = i18n::t(Key::Close);
    let retry_label = i18n::t(Key::Retry);
    let wrap = error_card_text_wrap(rect.width(), p, state.can_retry, &painter);
    let galley = painter.layout(
        state.text.clone(),
        FontId::proportional(visuals::FS_BODY),
        p.danger_text,
        wrap,
    );
    painter.galley(
        egui::pos2(
            rect.left() + 42.0,
            rect.min.y + (rect.height() - galley.size().y) / 2.0,
        ),
        galley,
        p.danger_text,
    );

    // 右侧按钮：28 高、圆角 6。仅 can_retry 时提供「重试」。
    let close_w = button_width(&painter, close_label, p);
    let retry_w = button_width(&painter, retry_label, p);
    let button_y = rect.min.y + (rect.height() - 28.0) / 2.0;
    let close_rect = Rect::from_min_size(
        egui::pos2(rect.right() - 12.0 - close_w, button_y),
        egui::vec2(close_w, 28.0),
    );
    let retry_rect = Rect::from_min_size(
        egui::pos2(close_rect.left() - 6.0 - retry_w, button_y),
        egui::vec2(retry_w, 28.0),
    );

    let mut action = ErrorAction::None;

    let close = ui.interact(
        close_rect,
        ui.scope_id().with("error_card_close"),
        Sense::click(),
    );
    draw_text_button(
        &mut painter,
        close_rect,
        close_label,
        p.text2,
        p,
        &close,
        ppp,
    );
    if close.clicked() {
        action = ErrorAction::Dismiss;
    }

    if state.can_retry {
        let retry = ui.interact(
            retry_rect,
            ui.scope_id().with("error_card_retry"),
            Sense::click(),
        );
        draw_text_button(
            &mut painter,
            retry_rect,
            retry_label,
            p.danger,
            p,
            &retry,
            ppp,
        );
        if retry.clicked() {
            action = ErrorAction::Retry;
        }
    }

    action
}

/// 错误卡片上的纯文字按钮：hover/press 用中性叠色，圆角 6。
fn draw_text_button(
    painter: &mut egui::Painter,
    rect: Rect,
    label: &str,
    color: Color32,
    p: &Palette,
    response: &Response,
    ppp: f32,
) {
    let state = if response.is_pointer_button_down_on() {
        ControlState::Pressed
    } else if response.hovered() {
        ControlState::Hovered
    } else {
        ControlState::Default
    };
    let style = visuals::resolve_state(p, state, WidgetKind::MenuItem);
    if style.bg.a() > 0 {
        painter.rect_filled(
            rect,
            CornerRadius::same(visuals::snap_radius(6.0, ppp)),
            style.bg,
        );
    }
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(visuals::FS_BODY),
        color,
    );
}

// ---------------------------------------------------------------------------
// toast
// ---------------------------------------------------------------------------

fn toast_width(painter: &egui::Painter, p: &Palette, text: &str) -> f32 {
    let text_w = painter
        .layout_no_wrap(
            text.to_owned(),
            FontId::proportional(visuals::FS_BODY),
            p.text,
        )
        .size()
        .x;
    (text_w + 10.0 + 3.0 + 10.0 + 20.0 + 8.0 + 12.0)
        .clamp(visuals::TOAST_MIN_W, visuals::TOAST_MAX_W)
}

fn toast_overlay(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &ToastState,
    p: &Palette,
    alpha: f32,
    ppp: f32,
) {
    // toast 无交互控件：只需绘制，不注册任何 interact（B10）。
    let mut painter = ui.painter().clone();
    painter.set_opacity(alpha);

    shadow(&painter, rect, p);
    painter.rect(
        rect,
        CornerRadius::same(visuals::snap_radius(visuals::TOAST_RADIUS, ppp)),
        p.overlay,
        Stroke::new(visuals::STRUCTURE_LINE, p.structure_stroke),
        StrokeKind::Inside,
    );

    // 左侧 3 pt 成功竖条（高 20）。
    painter.rect_filled(
        Rect::from_min_size(
            egui::pos2(rect.left() + 10.0, rect.center().y - 10.0),
            egui::vec2(3.0, 20.0),
        ),
        CornerRadius::same(2),
        p.success,
    );

    // 成功浅底徽标 + 对勾图标。
    let badge = Rect::from_center_size(
        egui::pos2(rect.left() + 10.0 + 3.0 + 10.0 + 10.0, rect.center().y),
        Vec2::splat(20.0),
    );
    painter.rect_filled(badge, CornerRadius::same(10), p.success_soft);
    icons::paint(&painter, badge, Icon::Check, p.success);

    painter.text(
        egui::pos2(badge.right() + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        &state.text,
        FontId::proportional(visuals::FS_BODY),
        p.text,
    );
}

// ---------------------------------------------------------------------------
// 全屏文件名胶囊
// ---------------------------------------------------------------------------

fn title_width(painter: &egui::Painter, text: &str) -> f32 {
    let text_w = painter
        .layout_no_wrap(
            text.to_owned(),
            FontId::proportional(visuals::FS_BODY),
            Color32::WHITE,
        )
        .size()
        .x;
    (text_w + 28.0).max(visuals::TITLE_CAP_MIN_W)
}

fn title_capsule(ui: &mut egui::Ui, rect: Rect, state: &TitleCapsuleState, p: &Palette, ppp: f32) {
    // 胶囊无交互控件：不注册 interact。
    let mut painter = ui.painter().clone();
    painter.set_opacity(state.alpha.clamp(0.0, 1.0));

    painter.rect(
        rect,
        CornerRadius::same(visuals::snap_radius(visuals::TITLE_CAP_RADIUS, ppp)),
        p.mask,
        Stroke::new(visuals::STRUCTURE_LINE, Color32::from_white_alpha(31)), // @12%
        StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        &state.text,
        FontId::proportional(visuals::FS_BODY),
        p.on_accent,
    );
}

// ---------------------------------------------------------------------------
// 菜单共用件（S3 / 232 / 12 / 32 / 自绘复选与单选）
// ---------------------------------------------------------------------------

fn menu_row(ui: &mut egui::Ui, enabled: bool) -> (Rect, Response) {
    let width = ui
        .available_width()
        .max(visuals::MENU_MIN_W - 2.0 * visuals::MENU_PAD);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    ui.allocate_exact_size(egui::vec2(width, visuals::MENU_ITEM_H), sense)
}

fn draw_row_bg(ui: &egui::Ui, rect: Rect, response: &Response, p: &Palette, enabled: bool) {
    let state = if !enabled {
        ControlState::Disabled
    } else if response.is_pointer_button_down_on() {
        ControlState::Pressed
    } else if response.hovered() {
        ControlState::Hovered
    } else {
        ControlState::Default
    };
    let style = visuals::resolve_state(p, state, WidgetKind::MenuItem);
    if style.bg.a() > 0 {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(style.radius), style.bg);
    }
    if let Some(stroke) = style.stroke {
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(style.radius),
            stroke,
            StrokeKind::Inside,
        );
    }
}

fn disabled_text(p: &Palette) -> Color32 {
    visuals::resolve_state(p, ControlState::Disabled, WidgetKind::MenuItem).fg
}

/// 通用菜单项：20 pt 图标槽 + 10 pt 间距，项高 32，危险项图标/文字用危险色。
pub fn menu_item(
    ui: &mut egui::Ui,
    p: &Palette,
    icon: Option<Icon>,
    label: &str,
    danger: bool,
    enabled: bool,
) -> Response {
    let (rect, response) = menu_row(ui, enabled);
    if ui.is_rect_visible(rect) {
        draw_row_bg(ui, rect, &response, p, enabled);
        let fg = if !enabled {
            disabled_text(p)
        } else if danger {
            p.danger
        } else {
            p.text
        };
        let icon_fg = if !enabled {
            disabled_text(p)
        } else if danger {
            p.danger
        } else {
            p.icon
        };
        let mut x = rect.left() + visuals::MENU_PAD;
        if let Some(icon) = icon {
            let icon_rect = Rect::from_center_size(
                egui::pos2(x + visuals::ICON_FRAME / 2.0, rect.center().y),
                Vec2::splat(visuals::ICON_FRAME),
            );
            icons::paint(ui.painter(), icon_rect, icon, icon_fg);
            x += visuals::MENU_ICON_SLOT + visuals::MENU_ICON_GAP;
        }
        ui.painter().text(
            egui::pos2(x, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(visuals::FS_BODY),
            fg,
        );
    }
    response
}

/// 自绘复选框：16×16 圆角 4；选中 = 强调实心 + 1.6 pt 白色对勾。
pub fn menu_checkbox(
    ui: &mut egui::Ui,
    p: &Palette,
    checked: bool,
    label: &str,
    enabled: bool,
) -> bool {
    let (rect, response) = menu_row(ui, enabled);
    if ui.is_rect_visible(rect) {
        draw_row_bg(ui, rect, &response, p, enabled);
        let box_rect = Rect::from_center_size(
            egui::pos2(
                rect.left() + visuals::MENU_PAD + visuals::CHECK_SIZE / 2.0,
                rect.center().y,
            ),
            Vec2::splat(visuals::CHECK_SIZE),
        );
        let corner = CornerRadius::same(visuals::CHECK_RADIUS as u8);
        let painter = ui.painter();
        if checked {
            let fill = if enabled {
                p.accent
            } else {
                visuals::with_alpha(p.accent, 102)
            };
            painter.rect_filled(box_rect, corner, fill);
            let tick = box_rect.shrink(3.5);
            painter.add(Shape::line(
                vec![
                    egui::pos2(tick.left(), tick.center().y),
                    egui::pos2(tick.center().x, tick.bottom()),
                    egui::pos2(tick.right(), tick.top()),
                ],
                Stroke::new(1.6, p.on_accent),
            ));
        } else {
            painter.rect_stroke(
                box_rect,
                corner,
                Stroke::new(visuals::STRUCTURE_LINE, p.control_border),
                StrokeKind::Inside,
            );
        }
        let color = if enabled { p.text } else { disabled_text(p) };
        painter.text(
            egui::pos2(box_rect.right() + visuals::MENU_ICON_GAP, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(visuals::FS_BODY),
            color,
        );
    }
    response.clicked()
}

/// 自绘单选：16×16 圆角 8；选中 = 强调色环 + 6 pt 实心圆点。
pub fn menu_radio(
    ui: &mut egui::Ui,
    p: &Palette,
    selected: bool,
    label: &str,
    enabled: bool,
) -> bool {
    let (rect, response) = menu_row(ui, enabled);
    if ui.is_rect_visible(rect) {
        draw_row_bg(ui, rect, &response, p, enabled);
        let box_rect = Rect::from_center_size(
            egui::pos2(
                rect.left() + visuals::MENU_PAD + visuals::CHECK_SIZE / 2.0,
                rect.center().y,
            ),
            Vec2::splat(visuals::CHECK_SIZE),
        );
        let corner = CornerRadius::same(visuals::RADIO_RADIUS as u8);
        let painter = ui.painter();
        if selected {
            painter.rect_filled(box_rect, corner, p.accent);
            painter.circle_filled(box_rect.center(), visuals::RADIO_DOT / 2.0, p.on_accent);
        } else {
            painter.rect_stroke(
                box_rect,
                corner,
                Stroke::new(visuals::STRUCTURE_LINE, p.control_border),
                StrokeKind::Inside,
            );
        }
        let color = if enabled { p.text } else { disabled_text(p) };
        painter.text(
            egui::pos2(box_rect.right() + visuals::MENU_ICON_GAP, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(visuals::FS_BODY),
            color,
        );
    }
    response.clicked()
}

/// 跨组：8 pt 间距 + 1 pt 结构边界线；不新增任何文案键。
pub fn menu_group_gap(ui: &mut egui::Ui, p: &Palette) {
    let width = ui
        .available_width()
        .max(visuals::MENU_MIN_W - 2.0 * visuals::MENU_PAD);
    ui.add_space(visuals::MENU_GROUP_GAP);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 1.0), Sense::hover());
    let y = visuals::snap_line_center(rect.center().y, ui.ctx().pixels_per_point());
    ui.painter().hline(
        rect.x_range(),
        y,
        Stroke::new(visuals::STRUCTURE_LINE, p.structure_stroke),
    );
    ui.add_space(visuals::MENU_GROUP_GAP);
}

/// 右键菜单内容（复制路径 / 定位 / 删除），与溢出菜单共用同一套项样式。
pub fn context_menu_content(ui: &mut egui::Ui, p: &Palette, action: &mut Option<Action>) {
    ui.set_min_width(visuals::MENU_MIN_W);

    if menu_item(
        ui,
        p,
        Some(Icon::Copy),
        i18n::t(Key::CopyPathMenu),
        false,
        true,
    )
    .clicked()
    {
        *action = Some(Action::CopyPath);
        ui.close();
    }
    if menu_item(
        ui,
        p,
        Some(Icon::External),
        i18n::t(Key::RevealInExplorer),
        false,
        true,
    )
    .clicked()
    {
        *action = Some(Action::RevealInExplorer);
        ui.close();
    }

    menu_group_gap(ui, p);

    if menu_item(
        ui,
        p,
        Some(Icon::Trash),
        i18n::t(Key::MoveToTrashMenu),
        true,
        true,
    )
    .clicked()
    {
        *action = Some(Action::Delete);
        ui.close();
    }
}

// ---------------------------------------------------------------------------
// QA 回归测试：画布内浮层可交互性与菜单共用件
// ---------------------------------------------------------------------------

#[cfg(test)]
mod qa_redesign_tests {
    use super::*;
    use std::time::Duration;

    /// egui 0.37 的 `FullOutput` 若直接 drop，会因未应用的字体纹理 delta 而 panic。
    fn run_ui(ctx: &egui::Context, input: egui::RawInput, body: impl FnMut(&mut egui::Ui)) {
        ctx.run_ui(input, body).drop_without_applying_deltas();
    }

    fn raw_input(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(800.0, 600.0),
            )),
            events,
            ..Default::default()
        }
    }

    fn pointer_button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// egui 0.37 语义探针：验证 `Area::interactable(false)` 下子 `ui.interact` 是否可点。
    /// `Area` 首帧是 sizing pass（area.rs 会临时把该 layer 标记为不可交互），
    /// 因此先跑若干稳定帧，再分帧注入 press / release。
    fn child_button_clicked(area_interactable: bool) -> bool {
        let ctx = egui::Context::default();
        let area_id = visuals::stable_id("qa_probe_area");
        let button_id = visuals::stable_id("qa_probe_button");
        let button_rect = Rect::from_min_size(egui::pos2(40.0, 40.0), egui::vec2(60.0, 28.0));
        let pos = button_rect.center();

        let run_button = |ui: &egui::Ui| {
            Area::new(area_id)
                .order(Order::Middle)
                .fixed_pos(egui::pos2(0.0, 0.0))
                .interactable(area_interactable)
                .fade_in(false)
                .show(ui.ctx(), |ui| {
                    ui.interact(button_rect, button_id, Sense::click())
                        .clicked()
                })
                .inner
        };

        for _ in 0..4 {
            run_ui(
                &ctx,
                raw_input(vec![egui::Event::PointerMoved(pos)]),
                |ui| {
                    let _ = run_button(ui);
                },
            );
        }
        run_ui(
            &ctx,
            raw_input(vec![
                egui::Event::PointerMoved(pos),
                pointer_button(pos, true),
            ]),
            |ui| {
                let _ = run_button(ui);
            },
        );

        let clicked = std::cell::Cell::new(false);
        run_ui(
            &ctx,
            raw_input(vec![
                egui::Event::PointerMoved(pos),
                pointer_button(pos, false),
            ]),
            |ui| {
                clicked.set(run_button(ui));
            },
        );
        clicked.get()
    }

    #[test]
    fn egui_area_interactable_false_excludes_child_widgets_from_hit_test() {
        assert!(
            child_button_clicked(true),
            "对照组：interactable(true) 的 Area 内按钮应可点"
        );
        assert!(
            !child_button_clicked(false),
            "egui 0.37 语义：interactable(false) 的 layer 被排除出 hit-test，子 interact 收不到点击"
        );
    }

    /// 产品期望（B7）：错误卡片「关闭」按钮必须能被真实点击命中。
    fn click_error_card_close_button() -> ErrorAction {
        let ctx = egui::Context::default();
        let canvas_rect = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let build_state = || OverlayState {
            canvas_rect,
            palette: crate::theme::Palette::light(),
            error: Some(ErrorCardState {
                text: "QA test error".to_owned(),
                can_retry: true,
                since: Instant::now() - Duration::from_millis(1000),
            }),
            toast: None,
            title: None,
            motion: visuals::Motion::default(),
        };

        // 关闭按钮位于错误卡片右侧内缩 12+20、垂直居中（卡片高 44）。
        let pos = egui::pos2(canvas_rect.max.x - 44.0, canvas_rect.min.y + 34.0);
        for _ in 0..4 {
            run_ui(
                &ctx,
                raw_input(vec![egui::Event::PointerMoved(pos)]),
                |ui| {
                    let _ = draw_all(ui.ctx(), build_state());
                },
            );
        }
        run_ui(
            &ctx,
            raw_input(vec![
                egui::Event::PointerMoved(pos),
                pointer_button(pos, true),
            ]),
            |ui| {
                let _ = draw_all(ui.ctx(), build_state());
            },
        );

        let action = std::cell::Cell::new(ErrorAction::None);
        run_ui(
            &ctx,
            raw_input(vec![
                egui::Event::PointerMoved(pos),
                pointer_button(pos, false),
            ]),
            |ui| {
                action.set(draw_all(ui.ctx(), build_state()));
            },
        );
        action.get()
    }

    #[test]
    fn error_card_close_button_is_reachable_by_a_real_click() {
        assert_eq!(
            click_error_card_close_button(),
            ErrorAction::Dismiss,
            "B7：错误卡片「关闭」按钮必须可达（overlay.rs 的 Area 不应吞掉点击）"
        );
    }

    #[test]
    fn self_drawn_menu_controls_render_in_all_states() {
        let ctx = egui::Context::default();
        let palette = crate::theme::Palette::light();
        run_ui(&ctx, raw_input(vec![]), |ui| {
            let _ = Area::new(visuals::stable_id("qa_menu_probe"))
                .order(Order::Foreground)
                .fixed_pos(egui::pos2(20.0, 20.0))
                .show(ui.ctx(), |ui| {
                    ui.set_min_width(visuals::MENU_MIN_W);
                    for selected in [false, true] {
                        for enabled in [false, true] {
                            let _ = menu_checkbox(ui, &palette, selected, "check", enabled);
                            let _ = menu_radio(ui, &palette, selected, "radio", enabled);
                        }
                    }
                    for danger in [false, true] {
                        let _ = menu_item(ui, &palette, Some(Icon::Trash), "item", danger, true);
                    }
                    menu_group_gap(ui, &palette);
                });
        });
    }

    #[test]
    fn menu_dimension_constants_match_the_s3_spec() {
        assert_eq!(
            (
                visuals::MENU_MIN_W,
                visuals::MENU_RADIUS,
                visuals::MENU_ITEM_H
            ),
            (232.0, 12.0, 32.0)
        );
        assert_eq!(
            (
                visuals::CHECK_SIZE,
                visuals::CHECK_RADIUS,
                visuals::RADIO_RADIUS
            ),
            (16.0, 4.0, 8.0)
        );
    }

    /// 验证「画布区域内的点击能穿透到画布层」：在 root 层注册一个可点击控件，
    /// 再叠加 draw_all 的浮层；若错误卡片 Area 未创建，点击应命中画布控件
    /// （B7 的互补约束：错误不存在 / 淡入未完成时不应拦截画布缩放 / 拖拽）。
    fn canvas_widget_receives_click_when(state_builder: impl Fn() -> OverlayState) -> bool {
        let ctx = egui::Context::default();
        let widget_id = visuals::stable_id("qa_canvas_widget");
        // 该矩形位于错误卡片会出现的画布顶部中心区域（卡片宽度内、顶部）。
        let widget_rect = Rect::from_min_size(egui::pos2(360.0, 12.0), egui::vec2(80.0, 40.0));
        let pos = widget_rect.center();

        let run_frame = |ctx: &egui::Context, events: Vec<egui::Event>| {
            run_ui(ctx, raw_input(events), |ui| {
                let _ = ui.interact(widget_rect, widget_id, Sense::click());
                let _ = draw_all(ui.ctx(), state_builder());
            });
        };

        for _ in 0..4 {
            run_frame(&ctx, vec![egui::Event::PointerMoved(pos)]);
        }
        run_frame(
            &ctx,
            vec![egui::Event::PointerMoved(pos), pointer_button(pos, true)],
        );

        let clicked = std::cell::Cell::new(false);
        run_ui(
            &ctx,
            raw_input(vec![
                egui::Event::PointerMoved(pos),
                pointer_button(pos, false),
            ]),
            |ui| {
                clicked.set(
                    ui.interact(widget_rect, widget_id, Sense::click())
                        .clicked(),
                );
                let _ = draw_all(ui.ctx(), state_builder());
            },
        );
        clicked.get()
    }

    #[test]
    fn canvas_click_passes_through_when_no_error() {
        let canvas_rect = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let build = || OverlayState {
            canvas_rect,
            palette: crate::theme::Palette::light(),
            error: None,
            toast: None,
            title: None,
            motion: visuals::Motion::default(),
        };
        assert!(
            canvas_widget_receives_click_when(build),
            "错误不存在时 error_card_overlay 不应创建，画布点击应穿透到画布控件"
        );
    }

    #[test]
    fn canvas_click_passes_through_when_error_alpha_is_zero() {
        let canvas_rect = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let build = || OverlayState {
            canvas_rect,
            palette: crate::theme::Palette::light(),
            error: Some(ErrorCardState {
                text: "QA".to_owned(),
                can_retry: false,
                // future since → elapsed 饱和到 0 → error_alpha(0)=0 → alpha<=0 → Area 不创建
                since: Instant::now() + Duration::from_secs(10),
            }),
            toast: None,
            title: None,
            motion: visuals::Motion::default(),
        };
        assert!(
            canvas_widget_receives_click_when(build),
            "error.alpha<=0 时 error_card_overlay 不应创建，画布点击应穿透到画布控件"
        );
    }
}
