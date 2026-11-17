//! 视图变换消息 → `iv-image-render::ViewState`。
//!
//! 几何计算完全在 Core 完成，Iced 只提供光标位置、画布尺寸与 DPI。

use emath::Vec2;

use crate::state::AppState;

pub fn zoom_at(state: &mut AppState, cursor: Vec2, factor: f32) {
    let scale_factor = state.scale_factor;
    state.viewer.zoom_at(cursor, factor, scale_factor);
}

pub fn zoom_by(state: &mut AppState, factor: f32) {
    let scale_factor = state.scale_factor;
    state.viewer.zoom_by(factor, scale_factor);
}

pub fn pan_by(state: &mut AppState, delta: Vec2) {
    let scale_factor = state.scale_factor;
    state.viewer.pan_by(delta, scale_factor);
}

pub fn actual_size(state: &mut AppState) {
    let scale_factor = state.scale_factor;
    state.viewer.actual_size(scale_factor);
}

pub fn toggle_actual_size(state: &mut AppState) {
    let scale_factor = state.scale_factor;
    state.viewer.toggle_actual_size(scale_factor);
}

pub fn canvas_resized(state: &mut AppState, size: iced::Size) {
    state.viewer.canvas = Vec2::new(size.width, size.height);
}

/// 只接受有限且为正的 DPI 缩放；窗口事件在部分平台上会短暂给出 0。
fn sanitized_scale_factor(current: f32, candidate: f32) -> f32 {
    if candidate.is_finite() && candidate > 0.0 {
        candidate
    } else {
        current
    }
}

pub fn scale_factor(state: &mut AppState, factor: f32) {
    state.scale_factor = sanitized_scale_factor(state.scale_factor, factor);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_scale_factors_are_ignored() {
        for value in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(sanitized_scale_factor(1.0, value), 1.0);
        }
        assert_eq!(sanitized_scale_factor(1.0, 1.5), 1.5);
    }
}
