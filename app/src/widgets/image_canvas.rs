//! 图片画布：把 `iv-image-render` 的几何结果映射到 Iced Canvas。
//!
//! 画布自身不保存任何业务状态，只把鼠标输入翻译成 `Message`；缩放、平移、
//! 旋转的数值全部来自 `ViewState`。像素资源仍是 `image::Handle`。

use std::time::{Duration, Instant};

use emath::Vec2;
use iced::widget::canvas::{self, Frame, Geometry, Image as CanvasImage};
use iced::widget::image::FilterMethod;
use iced::{Color, Point, Rectangle, Size, Vector, mouse};

use crate::message::Message;
use crate::state::viewer_state::LoadedImage;

/// 传给 Canvas 的只读快照；`view()` 每帧构造一次。
pub struct ImageCanvas<'a> {
    pub image: Option<&'a LoadedImage>,
    /// 逻辑点 / 纹理像素。
    pub zoom: f32,
    /// 图像中心相对画布中心的偏移。
    pub pan: Vec2,
    pub rotation: u8,
    pub background: Color,
}

/// 画布内部的交互状态：拖拽起点与双击计时。
#[derive(Default)]
pub struct CanvasInteraction {
    drag_from: Option<Point>,
    last_click: Option<(Instant, Point)>,
}

impl ImageCanvas<'_> {
    /// 计算本帧的绘制矩形；抽成纯函数便于单测。
    pub fn image_rect(&self, bounds: Size) -> Option<Rectangle> {
        let image = self.image?;
        let size = image.size() * self.zoom;
        let center = Point::new(
            bounds.width / 2.0 + self.pan.x,
            bounds.height / 2.0 + self.pan.y,
        );
        Some(Rectangle::new(
            Point::new(center.x - size.x / 2.0, center.y - size.y / 2.0),
            Size::new(size.x, size.y),
        ))
    }
}

impl canvas::Program<Message> for ImageCanvas<'_> {
    type State = CanvasInteraction;

    fn update(
        &self,
        state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            iced::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let position = cursor.position_in(bounds)?;
                let steps = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / 100.0,
                };
                if steps.abs() < f32::EPSILON {
                    return None;
                }
                let factor = 1.15_f32.powf(steps.clamp(-6.0, 6.0));
                let cursor = Vec2::new(
                    position.x - bounds.width / 2.0,
                    position.y - bounds.height / 2.0,
                );
                Some(canvas::Action::publish(Message::ZoomAt { cursor, factor }).and_capture())
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let position = cursor.position_in(bounds)?;
                let now = Instant::now();
                let is_double_click = state.last_click.is_some_and(|(previous, origin)| {
                    now.duration_since(previous) < Duration::from_millis(400)
                        && (origin.x - position.x).abs() < 6.0
                        && (origin.y - position.y).abs() < 6.0
                });

                if is_double_click {
                    state.last_click = None;
                    state.drag_from = None;
                    return Some(canvas::Action::publish(Message::ToggleActualSize).and_capture());
                }

                state.last_click = Some((now, position));
                state.drag_from = Some(position);
                Some(canvas::Action::capture())
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let position = cursor.position_in(bounds)?;
                let origin = state.drag_from?;
                let delta = Vec2::new(position.x - origin.x, position.y - origin.y);
                if delta.x == 0.0 && delta.y == 0.0 {
                    return None;
                }
                state.drag_from = Some(position);
                Some(canvas::Action::publish(Message::PanBy(delta)).and_capture())
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | iced::Event::Mouse(mouse::Event::CursorLeft) => {
                state.drag_from = None;
                None
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), self.background);

        let Some(image) = self.image else {
            return vec![frame.into_geometry()];
        };
        let Some(rect) = self.image_rect(bounds.size()) else {
            return vec![frame.into_geometry()];
        };

        // 图片后方的暗色底衬，让透明图片与浅色画布分开。
        frame.fill_rectangle(
            Point::new(rect.x + 2.0, rect.y + 2.0),
            rect.size(),
            Color::from_rgba(0.0, 0.0, 0.0, 0.35),
        );

        frame.with_save(|frame| {
            frame.translate(Vector::new(
                rect.x + rect.width / 2.0,
                rect.y + rect.height / 2.0,
            ));
            if self.rotation != 0 {
                frame.rotate(self.rotation as f32 * std::f32::consts::FRAC_PI_2);
            }
            frame.draw_image(
                Rectangle::new(
                    Point::new(-rect.width / 2.0, -rect.height / 2.0),
                    rect.size(),
                ),
                CanvasImage::new(image.handle.clone()).filter_method(FilterMethod::Linear),
            );
        });

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        // 拖动中即使移出画布也保持四向箭头，避免拖到边缘时光标跳回普通箭头。
        if state.drag_from.is_some() {
            return mouse::Interaction::Grabbing;
        }

        // 画布内保持普通箭头：Windows 上 `Grab` / `Grabbing` 只能映射成 `IDC_SIZEALL`
        // （四向箭头），与工具栏按钮的手型混在一起容易被当成错位资源。可拖动这件事
        // 由按下后的光标变化表达，悬停时不预告。
        //
        // 范围外必须交还光标（`None`）：Iced 的 `canvas` 不替 `Program` 判断鼠标范围，
        // 而 `column` / `row` 用 `max()` 合并子控件光标，画布一旦越界返回非 `None`
        // 就会把整个窗口的光标按住。
        if cursor.is_over(bounds) {
            mouse::Interaction::Idle
        } else {
            mouse::Interaction::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::canvas::Program;
    use iced::widget::image::Handle;

    fn fixture() -> LoadedImage {
        LoadedImage {
            path: "sample.png".into(),
            handle: Handle::from_rgba(2, 2, vec![0; 16]),
            pixel_size: [200, 100],
            source_size: [200, 100],
            depth: 8,
            color: Default::default(),
        }
    }

    #[test]
    fn rect_is_centered_and_scaled() {
        let image = fixture();
        let canvas = ImageCanvas {
            image: Some(&image),
            zoom: 0.5,
            pan: Vec2::new(10.0, -20.0),
            rotation: 0,
            background: Color::BLACK,
        };

        let rect = canvas.image_rect(Size::new(800.0, 600.0)).unwrap();
        assert_eq!(rect.width, 100.0);
        assert_eq!(rect.height, 50.0);
        assert_eq!(rect.x, 360.0);
        assert_eq!(rect.y, 255.0);
    }

    #[test]
    fn no_image_produces_no_rect() {
        let canvas = ImageCanvas {
            image: None,
            zoom: 1.0,
            pan: Vec2::ZERO,
            rotation: 0,
            background: Color::BLACK,
        };
        assert!(canvas.image_rect(Size::new(100.0, 100.0)).is_none());
    }

    fn interaction_at(
        canvas: &ImageCanvas<'_>,
        state: &CanvasInteraction,
        x: f32,
        y: f32,
    ) -> mouse::Interaction {
        canvas.mouse_interaction(
            state,
            Rectangle::new(Point::ORIGIN, Size::new(400.0, 300.0)),
            mouse::Cursor::Available(Point::new(x, y)),
        )
    }

    /// 画布内保持普通箭头：四向箭头只在按下拖动时出现。
    #[test]
    fn cursor_inside_bounds_keeps_arrow() {
        let image = fixture();
        let canvas = ImageCanvas {
            image: Some(&image),
            zoom: 1.0,
            pan: Vec2::ZERO,
            rotation: 0,
            background: Color::BLACK,
        };

        assert_eq!(
            interaction_at(&canvas, &CanvasInteraction::default(), 200.0, 150.0),
            mouse::Interaction::Idle
        );
    }

    /// 画布之外必须交还光标，否则画布的交互会盖掉工具栏按钮的手型。
    #[test]
    fn cursor_outside_bounds_yields_no_interaction() {
        let image = fixture();
        let canvas = ImageCanvas {
            image: Some(&image),
            zoom: 1.0,
            pan: Vec2::ZERO,
            rotation: 0,
            background: Color::BLACK,
        };

        assert_eq!(
            interaction_at(&canvas, &CanvasInteraction::default(), 900.0, 150.0),
            mouse::Interaction::None
        );
        assert_eq!(
            interaction_at(&canvas, &CanvasInteraction::default(), -1.0, 150.0),
            mouse::Interaction::None
        );
    }

    /// 拖动中即使移出画布仍保持四向箭头。
    #[test]
    fn dragging_keeps_grabbing_outside_bounds() {
        let image = fixture();
        let canvas = ImageCanvas {
            image: Some(&image),
            zoom: 1.0,
            pan: Vec2::ZERO,
            rotation: 0,
            background: Color::BLACK,
        };
        let state = CanvasInteraction {
            drag_from: Some(Point::new(10.0, 10.0)),
            ..Default::default()
        };

        assert_eq!(
            interaction_at(&canvas, &state, 200.0, 150.0),
            mouse::Interaction::Grabbing
        );
        assert_eq!(
            interaction_at(&canvas, &state, 900.0, 900.0),
            mouse::Interaction::Grabbing
        );
    }
}
