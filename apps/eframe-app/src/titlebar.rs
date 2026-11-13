//! Linux 自绘标题栏与窗口边缘缩放把手。
//!
//! Linux 上窗口关闭了系统装饰（见 `main.rs`）：GNOME/Mutter 等合成器不提供
//! xdg-decoration 服务端装饰，winit 的 Adwaita 客户端装饰在分数缩放下又会把
//! 三个窗口按钮糊成一片。标题栏、窗口按钮与缩放把手因此改由 egui 绘制：走
//! GL 管线、严格按 `pixels_per_point` 缩放，任何 DPI 下都保持清晰；窗口
//! 移动、缩放、最小化等交互通过 `ViewportCommand` 转发给窗口系统。
//!
//! 窗口同时开启了透明背景（见 `main.rs`）：窗口化时四角按 [`CORNER_RADIUS`]
//! 做圆角、圆角处不绘制而透出桌面；最大化与全屏贴边时保持直角。

use eframe::egui::{
    self, Color32, CornerRadius, CursorIcon, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};

use crate::theme::{self, Palette};
use crate::visuals;

/// 标题栏高度（逻辑点）。Q9：32 → 36，仅高度变化，交互与圆角逻辑不动；
/// 数值统一取自 `visuals::TITLEBAR_H`，避免多处硬编码。
const HEIGHT: f32 = visuals::TITLEBAR_H;
/// 单个窗口按钮的命中区域宽度。
const CELL_W: f32 = 24.0;
/// 相邻窗口按钮之间的间距。
const GAP: f32 = 6.0;
/// 最右侧按钮到窗口右缘的距离。
const RIGHT_MARGIN: f32 = 10.0;
/// 按钮圆形背景半径。
const BUTTON_R: f32 = 10.0;
/// 边缘缩放把手的厚度。
const GRIP: f32 = 6.0;
/// 窗口四角的圆角半径。仅 Linux 自绘装饰、窗口化时应用：标题栏画上两角、
/// 状态栏画下两角，圆角处不绘制、透出桌面；最大化与全屏贴边时保持直角。
pub const CORNER_RADIUS: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Button {
    Minimize,
    Maximize,
    Close,
}

/// 在窗口顶部绘制标题栏：标题居中，右侧依次为最小化、最大化、关闭按钮；
/// 空白区域可拖动移动窗口，双击切换最大化。全屏时由调用方跳过。
pub fn show(ui: &mut egui::Ui, title: &str) {
    let ctx = ui.ctx().clone();
    let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
    let palette = theme::palette(&ctx);
    // 最大化时窗口贴边，圆角会露出桌面缝隙，保持直角。
    let radius = if maximized { 0 } else { CORNER_RADIUS as u8 };
    let frame = egui::Frame::NONE
        .fill(palette.chrome)
        .corner_radius(egui::CornerRadius {
            nw: radius,
            ne: radius,
            sw: 0,
            se: 0,
        });

    let panel = egui::Panel::top("titlebar")
        .exact_size(HEIGHT)
        .frame(frame)
        .show(ui, |ui| {
            let rect = ui.max_rect();

            let close_rect = Rect::from_min_size(
                egui::pos2(rect.right() - RIGHT_MARGIN - CELL_W, rect.top()),
                egui::vec2(CELL_W, rect.height()),
            );
            let maximize_rect = Rect::from_min_size(
                egui::pos2(close_rect.left() - GAP - CELL_W, rect.top()),
                egui::vec2(CELL_W, rect.height()),
            );
            let minimize_rect = Rect::from_min_size(
                egui::pos2(maximize_rect.left() - GAP - CELL_W, rect.top()),
                egui::vec2(CELL_W, rect.height()),
            );
            // 拖拽区不覆盖按钮命中区域，避免交互重叠。
            let drag_rect =
                Rect::from_min_max(rect.min, egui::pos2(minimize_rect.left() - 4.0, rect.max.y));

            let drag = ui.interact(
                drag_rect,
                ui.scope_id().with("titlebar_drag"),
                Sense::click_and_drag(),
            );
            // 只响应左键：右键拖动在部分平台有别的含义。
            if drag.drag_started() && ui.input(|input| input.pointer.primary_down()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if drag.double_clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            }

            paint_title(ui, &palette, rect, drag_rect, title);

            for (kind, cell) in [
                (Button::Minimize, minimize_rect),
                (Button::Maximize, maximize_rect),
                (Button::Close, close_rect),
            ] {
                let response = ui.interact(
                    cell,
                    ui.scope_id().with(("window_button", kind)),
                    Sense::click(),
                );
                let hovered = response.hovered();
                if hovered {
                    let pressed = response.is_pointer_button_down_on();
                    let background = match kind {
                        Button::Close if !pressed => palette.danger,
                        _ if pressed => palette.press,
                        _ => palette.hover,
                    };
                    ui.painter()
                        .circle_filled(cell.center(), BUTTON_R, background);
                }
                let icon_color = if kind == Button::Close && hovered {
                    Color32::WHITE
                } else {
                    palette.icon
                };
                paint_icon(ui, kind, cell.center(), icon_color, maximized);

                if response.clicked() {
                    let command = match kind {
                        Button::Minimize => egui::ViewportCommand::Minimized(true),
                        Button::Maximize => egui::ViewportCommand::Maximized(!maximized),
                        Button::Close => egui::ViewportCommand::Close,
                    };
                    ctx.send_viewport_cmd(command);
                }
            }
        });

    let rect = panel.response.rect;
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, palette.border),
    );
}

/// 系统装饰关闭后合成器不再提供缩放边框；在窗口四边与四角叠加透明把手，
/// 悬停时显示对应方向的光标，按下拖动转交 `ViewportCommand::BeginResize`。
/// 最大化与全屏时禁用。
pub fn resize_grips(ctx: &egui::Context, maximized: bool, fullscreen: bool) {
    if maximized || fullscreen {
        return;
    }
    let screen = ctx.viewport_rect();
    let corner = egui::vec2(GRIP, GRIP);
    // 边在前、角在后：同层重叠时后创建的部件优先，角把手覆盖边把手端点。
    let grips = [
        (
            Rect::from_min_size(screen.min, egui::vec2(GRIP, screen.height())),
            CursorIcon::ResizeHorizontal,
            egui::viewport::ResizeDirection::West,
        ),
        (
            Rect::from_min_size(
                egui::pos2(screen.right() - GRIP, screen.top()),
                egui::vec2(GRIP, screen.height()),
            ),
            CursorIcon::ResizeHorizontal,
            egui::viewport::ResizeDirection::East,
        ),
        (
            Rect::from_min_size(screen.min, egui::vec2(screen.width(), GRIP)),
            CursorIcon::ResizeVertical,
            egui::viewport::ResizeDirection::North,
        ),
        (
            Rect::from_min_size(
                egui::pos2(screen.left(), screen.bottom() - GRIP),
                egui::vec2(screen.width(), GRIP),
            ),
            CursorIcon::ResizeVertical,
            egui::viewport::ResizeDirection::South,
        ),
        (
            Rect::from_min_size(screen.min, corner),
            CursorIcon::ResizeNwSe,
            egui::viewport::ResizeDirection::NorthWest,
        ),
        (
            Rect::from_min_size(egui::pos2(screen.right() - GRIP, screen.top()), corner),
            CursorIcon::ResizeNeSw,
            egui::viewport::ResizeDirection::NorthEast,
        ),
        (
            Rect::from_min_size(egui::pos2(screen.left(), screen.bottom() - GRIP), corner),
            CursorIcon::ResizeNwSe,
            egui::viewport::ResizeDirection::SouthWest,
        ),
        (
            Rect::from_min_size(
                egui::pos2(screen.right() - GRIP, screen.bottom() - GRIP),
                corner,
            ),
            CursorIcon::ResizeNeSw,
            egui::viewport::ResizeDirection::SouthEast,
        ),
    ];

    for (index, (rect, cursor, direction)) in grips.into_iter().enumerate() {
        let response = egui::Area::new(egui::Id::unique(("resize_grip", index)))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                ui.allocate_exact_size(rect.size(), Sense::click_and_drag())
                    .1
            })
            .inner;
        if response.hovered() {
            ctx.set_cursor_icon(cursor);
        }
        if response.drag_started() && ctx.input(|input| input.pointer.primary_down()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
        }
    }
}

fn paint_title(ui: &egui::Ui, palette: &Palette, rect: Rect, drag_rect: Rect, title: &str) {
    if title.is_empty() {
        return;
    }
    let painter = ui.painter();
    let galley =
        painter.layout_no_wrap(title.to_owned(), FontId::proportional(13.0), palette.text2);
    // 居中显示，但不滑入右侧按钮区域；超长时右端被裁切。
    let x = (rect.center().x - galley.size().x / 2.0).max(drag_rect.left() + 8.0);
    let y = rect.center().y - galley.size().y / 2.0;
    painter
        .with_clip_rect(drag_rect)
        .galley(egui::pos2(x, y), galley, palette.text2);
}

fn paint_icon(ui: &egui::Ui, kind: Button, center: Pos2, color: Color32, maximized: bool) {
    let painter = ui.painter();
    let (x, y) = (center.x, center.y);
    match kind {
        Button::Minimize => {
            painter.line_segment(
                [egui::pos2(x - 4.5, y), egui::pos2(x + 4.5, y)],
                Stroke::new(1.5, color),
            );
        }
        Button::Maximize if !maximized => {
            painter.rect_stroke(
                Rect::from_center_size(center, Vec2::splat(9.0)),
                CornerRadius::same(0),
                Stroke::new(1.3, color),
                StrokeKind::Middle,
            );
        }
        // 已最大化：画还原图标，前景窗口完整描边，
        // 背景窗口只画未被前景遮挡的四段边线。
        Button::Maximize => {
            let stroke = Stroke::new(1.3, color);
            painter.rect_stroke(
                Rect::from_min_max(egui::pos2(x - 5.5, y - 1.5), egui::pos2(x + 1.5, y + 5.5)),
                CornerRadius::same(0),
                stroke,
                StrokeKind::Middle,
            );
            painter.line_segment(
                [egui::pos2(x - 0.5, y - 5.5), egui::pos2(x + 5.0, y - 5.5)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(x + 5.0, y - 5.5), egui::pos2(x + 5.0, y + 0.5)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(x + 1.5, y + 0.5), egui::pos2(x + 5.0, y + 0.5)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(x - 0.5, y - 5.5), egui::pos2(x - 0.5, y - 1.5)],
                stroke,
            );
        }
        Button::Close => {
            let stroke = Stroke::new(1.5, color);
            painter.line_segment(
                [egui::pos2(x - 4.5, y - 4.5), egui::pos2(x + 4.5, y + 4.5)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(x - 4.5, y + 4.5), egui::pos2(x + 4.5, y - 4.5)],
                stroke,
            );
        }
    }
}
