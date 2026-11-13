//! 视图变换（缩放/平移）与画布绘制。
//!
//! 视图数学、Fit 5% 留白、旋转 UV 全部保持现状；本期只改画布外观：
//! 1 pt 内画框、随主题取色的 16 pt 棋盘格、96×96 主题化空状态插画，
//! 并把 `canvas_rect` 带出供画布内浮层绝对定位。

use eframe::egui::{self, Color32, CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};
pub use iv_image_render::{ViewMode, ViewState};

use crate::i18n::{self, Key};
use crate::icons::{self, Icon};
use crate::theme::Palette;
use crate::visuals;

/// 画布输出：原画布响应、空状态主按钮是否被点击，以及画布矩形（供浮层定位）。
pub struct CanvasOutput {
    pub response: egui::Response,
    pub open_clicked: bool,
    pub canvas_rect: Rect,
}

/// 绘制画布并处理缩放/平移输入。
pub fn canvas(
    ui: &mut egui::Ui,
    view: &mut ViewState,
    texture: Option<&egui::TextureHandle>,
    image_size: Vec2,
    palette: &Palette,
) -> CanvasOutput {
    let rect = ui.max_rect();
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    let painter = ui.painter();
    let ppp = ui.ctx().pixels_per_point();

    painter.rect_filled(rect, egui::CornerRadius::ZERO, palette.canvas);
    view.last_view = rect.size();

    // 四边 1 pt 内画框（浅/深均 #000000 @18%）；绘制在图片之下，
    // 用 StrokeKind::Inside 保证不跨出画布、不遮挡图片内容（A3）。
    painter.rect_stroke(
        visuals::snap_rect(rect, ppp),
        CornerRadius::ZERO,
        Stroke::new(visuals::STRUCTURE_LINE, Color32::from_black_alpha(46)),
        StrokeKind::Inside,
    );

    let Some(texture) = texture else {
        let open_clicked = empty_state(ui, rect, palette);
        return CanvasOutput {
            response,
            open_clicked,
            canvas_rect: rect,
        };
    };

    // 先处理输入，再用最新的视图状态绘制，避免一帧延迟。
    if response.hovered() {
        let (scroll, pinch) = ui
            .ctx()
            .input(|input| (input.smooth_scroll_delta.y, input.zoom_delta()));
        let factor = if (pinch - 1.0).abs() > 1e-4 {
            pinch
        } else {
            (scroll * 0.0015).exp()
        };

        if (factor - 1.0).abs() > 1e-4
            && let Some(position) = response.hover_pos()
        {
            view.zoom_at(
                position - rect.center(),
                image_size,
                rect.size(),
                factor,
                ppp,
            );
        }
    }

    if response.dragged() {
        let display_size = ViewState::displayed_size(image_size, view.rotation) * view.scale(ppp);
        if display_size.x > rect.width() + 0.5 || display_size.y > rect.height() + 0.5 {
            view.mode = ViewMode::Manual;
            view.pan += response.drag_delta();
            view.clamp_pan(image_size, rect.size(), ppp);
        }
    }

    if response.double_clicked() {
        view.toggle_actual_size(image_size, rect.size(), ppp);
    }

    if view.mode == ViewMode::Fit {
        view.update_fit(rect.size(), image_size, ppp);
    } else {
        view.clamp_pan(image_size, rect.size(), ppp);
    }

    let scale = view.scale(ppp);
    let image_rect = Rect::from_center_size(
        rect.center() + view.pan,
        ViewState::displayed_size(image_size, view.rotation) * scale,
    );

    // 图片完全盖住画布时，棋盘格不可见，直接跳过以省掉上千个矩形。
    let covers_view = image_rect.min.x <= rect.min.x
        && image_rect.min.y <= rect.min.y
        && image_rect.max.x >= rect.max.x
        && image_rect.max.y >= rect.max.y;
    if !covers_view {
        draw_checkerboard(painter, image_rect, rect, palette);
    }
    paint_rotated(painter, texture.id(), image_rect, view.rotation);

    if response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if response.hovered() {
        let display_size = ViewState::displayed_size(image_size, view.rotation) * scale;
        if display_size.x > rect.width() + 0.5 || display_size.y > rect.height() + 0.5 {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    }

    CanvasOutput {
        response,
        open_clicked: false,
        canvas_rect: rect,
    }
}

/// 空状态：96×96 画框插画（主题取色）+ 17/12.5 字阶 + 132×36 强调主按钮。
fn empty_state(ui: &mut egui::Ui, rect: Rect, palette: &Palette) -> bool {
    let painter = ui.painter();
    let center = rect.center();
    let ppp = ui.ctx().pixels_per_point();

    let icon_box = Rect::from_center_size(
        center - Vec2::new(0.0, 96.0),
        Vec2::splat(visuals::EMPTY_ICON_BOX),
    );
    painter.rect(
        visuals::snap_rect(icon_box, ppp),
        CornerRadius::same(visuals::snap_radius(visuals::EMPTY_ICON_BOX_RADIUS, ppp)),
        palette.canvas_raised,
        // 明确规格：1 pt 白 @10% 内描边（PRD §5.6），不是散落色值。
        Stroke::new(visuals::STRUCTURE_LINE, Color32::from_white_alpha(26)),
        StrokeKind::Inside,
    );
    icons::paint(
        painter,
        Rect::from_center_size(icon_box.center(), Vec2::splat(visuals::EMPTY_ICON_SIZE)),
        Icon::Image,
        visuals::with_alpha(palette.canvas_text, 140), // 白 @55%，随主题取色
    );

    painter.text(
        center + Vec2::new(0.0, -24.0),
        egui::Align2::CENTER_CENTER,
        i18n::t(Key::EmptyTitle),
        FontId::proportional(visuals::FS_TITLE),
        palette.canvas_text,
    );
    painter.text(
        center + Vec2::new(0.0, 2.0),
        egui::Align2::CENTER_CENTER,
        i18n::t(Key::EmptyHint),
        FontId::proportional(visuals::FS_BODY),
        palette.canvas_text2,
    );

    let button = Rect::from_center_size(
        center + Vec2::new(0.0, 46.0),
        Vec2::new(visuals::EMPTY_BTN_W, visuals::EMPTY_BTN_H),
    );
    let response = ui.interact(
        button,
        ui.scope_id().with("empty_state_open"),
        Sense::click(),
    );
    let fill = if response.is_pointer_button_down_on() {
        palette.accent.linear_multiply(0.9)
    } else if response.hovered() {
        palette.accent.linear_multiply(1.08)
    } else {
        palette.accent
    };
    painter.rect_filled(
        visuals::snap_rect(button, ppp),
        CornerRadius::same(visuals::snap_radius(visuals::PRIMARY_BTN_RADIUS, ppp)),
        fill,
    );
    painter.text(
        button.center(),
        egui::Align2::CENTER_CENTER,
        i18n::t(Key::EmptyOpen),
        FontId::proportional(visuals::FS_BUTTON),
        palette.on_accent,
    );

    response.clicked()
}

/// 用带旋转 UV 的四边形绘制纹理，避免为了旋转重新上传纹理。
fn paint_rotated(painter: &egui::Painter, texture: egui::TextureId, rect: Rect, rotation: u8) {
    let uvs = rotation_uvs(rotation);

    let mut mesh = egui::Mesh {
        texture_id: texture,
        ..Default::default()
    };
    for (pos, (u, v)) in [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ]
    .into_iter()
    .zip(uvs)
    {
        mesh.vertices.push(egui::epaint::Vertex {
            pos,
            uv: egui::pos2(u, v),
            color: Color32::WHITE,
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(egui::Shape::mesh(mesh));
}

/// 四个屏幕角（左上、右上、右下、左下）对应的纹理 UV。
fn rotation_uvs(rotation: u8) -> [(f32, f32); 4] {
    match rotation % 4 {
        0 => [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
        1 => [(0.0, 1.0), (0.0, 0.0), (1.0, 0.0), (1.0, 1.0)],
        2 => [(1.0, 1.0), (0.0, 1.0), (0.0, 0.0), (1.0, 0.0)],
        _ => [(1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0)],
    }
}

fn draw_checkerboard(painter: &egui::Painter, image_rect: Rect, clip: Rect, palette: &Palette) {
    if image_rect.width() <= 0.0 || image_rect.height() <= 0.0 {
        return;
    }

    let visible = image_rect.intersect(clip);
    if visible.width() <= 0.0 || visible.height() <= 0.0 {
        return;
    }

    painter.rect_filled(visible, egui::CornerRadius::ZERO, palette.checker_a);

    let cell = visuals::CHECKER_CELL;
    let start_x = ((visible.min.x - image_rect.min.x) / cell).floor() as i64;
    let end_x = ((visible.max.x - image_rect.min.x) / cell).ceil() as i64;
    let start_y = ((visible.min.y - image_rect.min.y) / cell).floor() as i64;
    let end_y = ((visible.max.y - image_rect.min.y) / cell).ceil() as i64;
    let light = palette.checker_b;

    for y in start_y..end_y {
        for x in start_x..end_x {
            if (x + y) % 2 == 0 {
                continue;
            }
            let min = egui::pos2(
                image_rect.min.x + x as f32 * cell,
                image_rect.min.y + y as f32 * cell,
            );
            // 必须裁剪到 `visible`，否则最后一个格子会溢出图片边界。
            let cell_rect = Rect::from_min_size(min, egui::vec2(cell, cell)).intersect(visible);
            if cell_rect.width() > 0.0 && cell_rect.height() > 0.0 {
                painter.rect_filled(cell_rect, egui::CornerRadius::ZERO, light);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_uvs_turn_clockwise() {
        // 顺时针 90°：屏幕左上角取原图左下角，右上角取原图左上角。
        assert_eq!(rotation_uvs(1)[0], (0.0, 1.0));
        assert_eq!(rotation_uvs(1)[1], (0.0, 0.0));
        assert_eq!(rotation_uvs(1)[2], (1.0, 0.0));
        assert_eq!(rotation_uvs(1)[3], (1.0, 1.0));
        // 180° 与 270° 的角点同理
        assert_eq!(rotation_uvs(2)[0], (1.0, 1.0));
        assert_eq!(rotation_uvs(3)[0], (1.0, 0.0));
        assert_eq!(rotation_uvs(4), rotation_uvs(0));
    }
}
