//! 视口计算与视图变换。
//!
//! 只依赖 `emath` 的二维数学类型（与 egui 共用 `Vec2`），不依赖窗口、绘制或
//! 纹理；GUI 层负责把这里的变换映射到画布坐标与鼠标输入。

use emath::Vec2;

pub const MIN_ZOOM: f32 = 0.01; // 1%
pub const MAX_ZOOM: f32 = 32.0; // 3200%

/// Fit 模式图片四周留 5% 呼吸空间。
const FIT_PADDING: f32 = 0.9;

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
    /// 顺时针旋转的 90° 次数（0~3），只影响显示，不改动纹理。
    pub rotation: u8,
    /// 上一帧画布尺寸，供键盘缩放的锚点使用。
    pub last_view: Vec2,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            mode: ViewMode::Fit,
            rotation: 0,
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

    /// 旋转 `steps` 个 90°（负数表示逆时针）；旋转后重新居中。
    pub fn rotate(&mut self, steps: i8) {
        self.rotation = (self.rotation as i8 + steps).rem_euclid(4) as u8;
        self.pan = Vec2::ZERO;
    }

    /// 旋转后图像的显示尺寸（奇数个 90° 时长宽互换）。
    pub fn displayed_size(image: Vec2, rotation: u8) -> Vec2 {
        if rotation.is_multiple_of(2) {
            image
        } else {
            Vec2::new(image.y, image.x)
        }
    }

    pub fn fit_zoom(view: Vec2, image: Vec2, pixels_per_point: f32) -> f32 {
        if image.x <= 0.0 || image.y <= 0.0 {
            return 1.0;
        }
        let usable = view * FIT_PADDING;
        let fit =
            (usable.x * pixels_per_point / image.x).min(usable.y * pixels_per_point / image.y);
        fit.clamp(MIN_ZOOM, 1.0)
    }

    /// Fit 模式每帧重算，保证窗口 resize 后仍然完整可见。
    pub fn update_fit(&mut self, view: Vec2, image: Vec2, pixels_per_point: f32) {
        if self.mode == ViewMode::Fit {
            self.zoom = Self::fit_zoom(
                view,
                Self::displayed_size(image, self.rotation),
                pixels_per_point,
            );
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
        self.clamp_pan(
            Self::displayed_size(image, self.rotation),
            view,
            pixels_per_point,
        );
    }

    pub fn clamp_pan(&mut self, image: Vec2, view: Vec2, pixels_per_point: f32) {
        let display_size =
            Self::displayed_size(image, self.rotation) * self.scale(pixels_per_point);
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
        self.clamp_pan(
            Self::displayed_size(image, self.rotation),
            view,
            pixels_per_point,
        );
    }

    pub fn zoom_by(&mut self, image: Vec2, view: Vec2, factor: f32, pixels_per_point: f32) {
        self.zoom_at(Vec2::ZERO, image, view, factor, pixels_per_point);
    }
}

#[cfg(test)]
mod tests {
    use emath::vec2;

    use super::*;

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-3, "{a} != {b}");
    }

    #[test]
    fn fit_zoom_uses_the_tighter_dimension() {
        approx(
            ViewState::fit_zoom(vec2(1000.0, 800.0), vec2(4000.0, 3000.0), 1.0),
            0.225,
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

    #[test]
    fn rotation_swaps_displayed_size() {
        let image = vec2(4000.0, 3000.0);
        assert_eq!(ViewState::displayed_size(image, 0), image);
        assert_eq!(ViewState::displayed_size(image, 1), vec2(3000.0, 4000.0));
        assert_eq!(ViewState::displayed_size(image, 2), image);
        assert_eq!(ViewState::displayed_size(image, 3), vec2(3000.0, 4000.0));
    }

    #[test]
    fn fit_uses_rotated_dimensions() {
        let mut state = ViewState {
            rotation: 1,
            ..Default::default()
        };
        state.update_fit(vec2(1000.0, 800.0), vec2(4000.0, 3000.0), 1.0);
        approx(state.zoom, 0.18);
    }

    #[test]
    fn rotate_wraps_and_centers() {
        let mut state = ViewState {
            pan: vec2(10.0, 20.0),
            ..Default::default()
        };
        state.rotate(-1);
        assert_eq!(state.rotation, 3);
        approx(state.pan.x, 0.0);
        approx(state.pan.y, 0.0);
        state.rotate(1);
        assert_eq!(state.rotation, 0);
    }
}
