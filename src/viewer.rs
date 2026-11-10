//! 视图变换（缩放/平移）与画布绘制。

use eframe::egui::{self, Color32, Rect, Sense, Vec2};

pub const MIN_ZOOM: f32 = 0.01; // 1%
pub const MAX_ZOOM: f32 = 32.0; // 3200%

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewMode {
    Fit,
    Manual,
}

#[derive(Clone, Copy, Debug)]
pub struct ViewState {
    /// 物理像素 / 纹理像素。1.0 表示 100%，与 DPI 无关。
    pub zoom: f32,
    /// 图像中心相对画布中心的偏移（逻辑点）。
    pub pan: Vec2,
    pub mode: ViewMode,
    /// 上一帧画布尺寸，供键盘缩放的锚点使用。
    pub last_view: Vec2,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            mode: ViewMode::Fit,
            last_view: Vec2::ZERO,
        }
    }
}

impl ViewState {
    /// 逻辑点 / 纹理像素。
    pub fn scale(&self, pixels_per_point: f32) -> f32 {
        self.zoom / pixels_per_point
    }

    pub fn reset_fit(&mut self) {
        self.mode = ViewMode::Fit;
        self.pan = Vec2::ZERO;
    }

    pub fn fit_zoom(view: Vec2, image: Vec2, pixels_per_point: f32) -> f32 {
        if image.x <= 0.0 || image.y <= 0.0 {
            return 1.0;
        }
        let fit = (view.x * pixels_per_point / image.x).min(view.y * pixels_per_point / image.y);
        fit.clamp(MIN_ZOOM, 1.0)
    }

    /// Fit 模式每帧重算，保证窗口 resize 后仍然完整可见。
    pub fn update_fit(&mut self, view: Vec2, image: Vec2, pixels_per_point: f32) {
        if self.mode == ViewMode::Fit {
            self.zoom = Self::fit_zoom(view, image, pixels_per_point);
            self.pan = Vec2::ZERO;
        }
    }

    /// 以画布内某点为锚点缩放（`cursor` 相对画布中心）。
    pub fn zoom_at(
        &mut self,
        cursor: Vec2,
        image: Vec2,
        view: Vec2,
        factor: f32,
        pixels_per_point: f32,
    ) {
        if !factor.is_finite() || factor <= 0.0 || image.x <= 0.0 || image.y <= 0.0 {
            return;
        }

        let old_scale = self.scale(pixels_per_point);
        if old_scale <= 0.0 {
            return;
        }

        let image_point = (cursor - self.pan) / old_scale;
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let new_scale = self.scale(pixels_per_point);
        self.pan = cursor - image_point * new_scale;
        self.mode = ViewMode::Manual;
        self.clamp_pan(image, view, pixels_per_point);
    }

    pub fn clamp_pan(&mut self, image: Vec2, view: Vec2, pixels_per_point: f32) {
        let display_size = image * self.scale(pixels_per_point);
        let limit = ((display_size - view) / 2.0).max(Vec2::ZERO);
        self.pan.x = self.pan.x.clamp(-limit.x, limit.x);
        self.pan.y = self.pan.y.clamp(-limit.y, limit.y);
    }

    /// 双击：Fit ⇄ 100%（居中）。
    pub fn toggle_actual_size(&mut self, image: Vec2, view: Vec2, pixels_per_point: f32) {
        if self.mode == ViewMode::Fit {
            self.actual_size(image, view, pixels_per_point);
        } else {
            self.reset_fit();
        }
    }

    pub fn actual_size(&mut self, image: Vec2, view: Vec2, pixels_per_point: f32) {
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
        self.mode = ViewMode::Manual;
        self.clamp_pan(image, view, pixels_per_point);
    }

    pub fn zoom_by(&mut self, image: Vec2, view: Vec2, factor: f32, pixels_per_point: f32) {
        self.zoom_at(Vec2::ZERO, image, view, factor, pixels_per_point);
    }
}

/// 绘制画布并处理缩放/平移输入。
pub fn canvas(
    ui: &mut egui::Ui,
    view: &mut ViewState,
    texture: Option<&egui::TextureHandle>,
    image_size: Vec2,
) -> egui::Response {
    let rect = ui.max_rect();
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    let painter = ui.painter();

    painter.rect_filled(rect, egui::CornerRadius::ZERO, Color32::from_gray(18));
    view.last_view = rect.size();

    let Some(texture) = texture else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "拖入图片，或按 Ctrl+O 打开",
            egui::FontId::proportional(18.0),
            Color32::from_gray(140),
        );
        return response;
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
        let display_size = image_size * view.scale(pixels_per_point);
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
    let image_rect = Rect::from_center_size(rect.center() + view.pan, image_size * scale);
    let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));

    // 图片完全盖住画布时，棋盘格不可见，直接跳过以省掉上千个矩形。
    let covers_view = image_rect.min.x <= rect.min.x
        && image_rect.min.y <= rect.min.y
        && image_rect.max.x >= rect.max.x
        && image_rect.max.y >= rect.max.y;
    if !covers_view {
        draw_checkerboard(painter, image_rect, rect);
    }
    painter.image(texture.id(), image_rect, uv, Color32::WHITE);

    if response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if response.hovered() {
        let display_size = image_size * scale;
        if display_size.x > rect.width() + 0.5 || display_size.y > rect.height() + 0.5 {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    }

    response
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

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-3, "{a} != {b}");
    }

    #[test]
    fn fit_zoom_uses_the_tighter_dimension() {
        approx(
            ViewState::fit_zoom(vec2(1000.0, 800.0), vec2(4000.0, 3000.0), 1.0),
            0.25,
        );
    }

    #[test]
    fn fit_does_not_upscale_small_images() {
        approx(
            ViewState::fit_zoom(vec2(1000.0, 800.0), vec2(200.0, 100.0), 1.0),
            1.0,
        );
    }

    #[test]
    fn zoom_at_keeps_the_point_under_the_cursor() {
        let image = vec2(4000.0, 3000.0);
        let view = vec2(1000.0, 800.0);
        let ppp = 2.0;
        let mut state = ViewState {
            mode: ViewMode::Manual,
            zoom: 0.5,
            pan: vec2(30.0, -20.0),
            ..Default::default()
        };

        let cursor = vec2(123.0, -45.0);
        let before = (cursor - state.pan) / state.scale(ppp);
        state.zoom_at(cursor, image, view, 1.7, ppp);
        let after = (cursor - state.pan) / state.scale(ppp);
        approx(before.x, after.x);
        approx(before.y, after.y);
    }

    #[test]
    fn zoom_is_clamped() {
        let image = vec2(100.0, 100.0);
        let view = vec2(1000.0, 1000.0);
        let mut state = ViewState {
            mode: ViewMode::Manual,
            ..Default::default()
        };
        for _ in 0..100 {
            state.zoom_at(Vec2::ZERO, image, view, 2.0, 1.0);
        }
        approx(state.zoom, MAX_ZOOM);
        for _ in 0..200 {
            state.zoom_at(Vec2::ZERO, image, view, 0.5, 1.0);
        }
        approx(state.zoom, MIN_ZOOM);
    }

    #[test]
    fn pan_is_clamped_when_image_is_smaller_than_view() {
        let image = vec2(400.0, 300.0);
        let view = vec2(1000.0, 800.0);
        let mut state = ViewState {
            mode: ViewMode::Manual,
            zoom: 0.5,
            pan: vec2(500.0, 500.0),
            ..Default::default()
        };
        state.clamp_pan(image, view, 1.0);
        approx(state.pan.x, 0.0);
        approx(state.pan.y, 0.0);
    }

    #[test]
    fn pan_is_clamped_when_image_is_larger_than_view() {
        let image = vec2(4000.0, 3000.0);
        let view = vec2(1000.0, 800.0);
        let mut state = ViewState {
            mode: ViewMode::Manual,
            zoom: 1.0,
            pan: vec2(9000.0, -9000.0),
            ..Default::default()
        };
        state.clamp_pan(image, view, 1.0);
        approx(state.pan.x, 1500.0);
        approx(state.pan.y, -1100.0);
    }

    #[test]
    fn scale_respects_dpi() {
        let state = ViewState {
            zoom: 1.0,
            ..Default::default()
        };
        approx(state.scale(1.0), 1.0);
        approx(state.scale(1.5), 1.0 / 1.5);
        approx(state.scale(2.0), 0.5);
    }

    use eframe::egui::vec2;
}
