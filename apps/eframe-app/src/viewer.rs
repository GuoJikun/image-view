//! 视图变换（缩放/平移）与画布绘制。

use eframe::egui::{self, Color32, CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};
pub use iv_image_render::{ViewMode, ViewState};

use crate::i18n::{self, Key};
use crate::icons::{self, Icon};
use crate::theme::Palette;

/// 画布输出：原画布响应，以及空状态主按钮是否被点击。
pub struct CanvasOutput {
    pub response: egui::Response,
    pub open_clicked: bool,
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

    painter.rect_filled(rect, egui::CornerRadius::ZERO, palette.canvas);
    view.last_view = rect.size();

    let Some(texture) = texture else {
        let open_clicked = empty_state(ui, rect, palette);
        return CanvasOutput {
            response,
            open_clicked,
        };
    };

    let pixels_per_point = ui.ctx().pixels_per_point();

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
                pixels_per_point,
            );
        }
    }

    if response.dragged() {
        let display_size =
            ViewState::displayed_size(image_size, view.rotation) * view.scale(pixels_per_point);
        if display_size.x > rect.width() + 0.5 || display_size.y > rect.height() + 0.5 {
            view.mode = ViewMode::Manual;
            view.pan += response.drag_delta();
            view.clamp_pan(image_size, rect.size(), pixels_per_point);
        }
    }

    if response.double_clicked() {
        view.toggle_actual_size(image_size, rect.size(), pixels_per_point);
    }

    if view.mode == ViewMode::Fit {
        view.update_fit(rect.size(), image_size, pixels_per_point);
    } else {
        view.clamp_pan(image_size, rect.size(), pixels_per_point);
    }

    let scale = view.scale(pixels_per_point);
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
        draw_checkerboard(painter, image_rect, rect);
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
    }
}

/// 空状态：中心图标 + 标题 + 说明 + 强调色主按钮。
fn empty_state(ui: &mut egui::Ui, rect: Rect, palette: &Palette) -> bool {
    let painter = ui.painter();
    let center = rect.center();

    let icon_rect = Rect::from_center_size(center - Vec2::new(0.0, 42.0), Vec2::splat(60.0));
    painter.rect(
        icon_rect,
        CornerRadius::same(15),
        Color32::from_white_alpha(15),
        Stroke::new(1.0, Color32::from_white_alpha(31)),
        StrokeKind::Inside,
    );
    icons::paint(
        painter,
        Rect::from_center_size(icon_rect.center(), Vec2::splat(26.0)),
        Icon::Sidebar,
        Color32::from_white_alpha(128),
    );

    painter.text(
        center + Vec2::new(0.0, 8.0),
        egui::Align2::CENTER_CENTER,
        i18n::t(Key::EmptyTitle),
        FontId::proportional(15.0),
        Color32::from_white_alpha(217),
    );
    painter.text(
        center + Vec2::new(0.0, 30.0),
        egui::Align2::CENTER_CENTER,
        i18n::t(Key::EmptyHint),
        FontId::proportional(12.0),
        Color32::from_white_alpha(115),
    );

    let button = Rect::from_center_size(center + Vec2::new(0.0, 62.0), Vec2::new(124.0, 32.0));
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
    ui.painter()
        .rect_filled(button, CornerRadius::same(7), fill);
    ui.painter().text(
        button.center(),
        egui::Align2::CENTER_CENTER,
        i18n::t(Key::EmptyOpen),
        FontId::proportional(12.5),
        Color32::WHITE,
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

fn draw_checkerboard(painter: &egui::Painter, image_rect: Rect, clip: Rect) {
    if image_rect.width() <= 0.0 || image_rect.height() <= 0.0 {
        return;
    }

    let visible = image_rect.intersect(clip);
    if visible.width() <= 0.0 || visible.height() <= 0.0 {
        return;
    }

    painter.rect_filled(
        visible,
        egui::CornerRadius::ZERO,
        Color32::from_rgb(36, 36, 38),
    );

    const CELL: f32 = 24.0;
    let start_x = ((visible.min.x - image_rect.min.x) / CELL).floor() as i64;
    let end_x = ((visible.max.x - image_rect.min.x) / CELL).ceil() as i64;
    let start_y = ((visible.min.y - image_rect.min.y) / CELL).floor() as i64;
    let end_y = ((visible.max.y - image_rect.min.y) / CELL).ceil() as i64;
    let light = Color32::from_rgb(48, 48, 52);

    for y in start_y..end_y {
        for x in start_x..end_x {
            if (x + y) % 2 == 0 {
                continue;
            }
            let min = egui::pos2(
                image_rect.min.x + x as f32 * CELL,
                image_rect.min.y + y as f32 * CELL,
            );
            // 必须裁剪到 `visible`，否则最后一个格子会溢出图片边界，在图片外侧露出浅色块。
            let cell_rect = Rect::from_min_size(min, egui::vec2(CELL, CELL)).intersect(visible);
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
