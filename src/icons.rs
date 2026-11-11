//! 工具栏/状态栏图标，全部用 egui 矢量绘制，随主题与状态着色。

use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Folder,
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    Minus,
    Plus,
    Play,
    Pause,
    Sidebar,
    Ellipsis,
    Warning,
}

/// 在 `rect` 内绘制图标；设计稿按 16×16 网格、1.5 px 线宽绘制。
pub fn paint(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let size = rect.width().min(rect.height());
    if size <= 0.0 {
        return;
    }
    let scale = size / 16.0;
    let origin = rect.center() - Vec2::splat(size / 2.0);
    let point = |x: f32, y: f32| origin + Vec2::new(x * scale, y * scale);
    let stroke = Stroke::new(1.5 * scale, color);
    let thin = Stroke::new(1.4 * scale, color);

    match icon {
        Icon::Folder => {
            line(
                painter,
                &[
                    (2.4, 4.8),
                    (2.4, 11.6),
                    (2.6, 12.0),
                    (3.4, 12.0),
                    (12.6, 12.0),
                    (13.4, 12.0),
                    (13.6, 11.6),
                    (13.6, 6.6),
                    (13.4, 6.4),
                    (8.0, 6.4),
                    (6.6, 4.8),
                    (3.0, 4.8),
                ],
                stroke,
                true,
                point,
            );
        }
        Icon::ChevronLeft => line(
            painter,
            &[(9.9, 3.6), (5.7, 8.0), (9.9, 12.4)],
            stroke,
            false,
            point,
        ),
        Icon::ChevronRight => line(
            painter,
            &[(6.1, 3.6), (10.3, 8.0), (6.1, 12.4)],
            stroke,
            false,
            point,
        ),
        Icon::ChevronDown => line(
            painter,
            &[(4.4, 6.4), (8.0, 9.9), (11.6, 6.4)],
            stroke,
            false,
            point,
        ),
        Icon::Minus => line(painter, &[(3.8, 8.0), (12.2, 8.0)], stroke, false, point),
        Icon::Plus => {
            line(painter, &[(3.8, 8.0), (12.2, 8.0)], stroke, false, point);
            line(painter, &[(8.0, 3.8), (8.0, 12.2)], stroke, false, point);
        }
        Icon::Play => {
            let points = vec![point(5.6, 3.9), point(12.1, 8.0), point(5.6, 12.1)];
            painter.add(Shape::convex_polygon(
                points,
                color,
                Stroke::new(0.8 * scale, color),
            ));
        }
        Icon::Pause => {
            for x in [6.1, 9.5] {
                painter.rect_filled(
                    Rect::from_min_size(point(x, 4.4), Vec2::new(1.5 * scale, 7.2 * scale)),
                    CornerRadius::same((0.75 * scale).round() as u8),
                    color,
                );
            }
        }
        Icon::Sidebar => {
            rounded_rect(
                painter,
                Rect::from_min_max(point(2.2, 3.4), point(13.8, 12.6)),
                1.8 * scale,
                stroke,
            );
            line(painter, &[(7.2, 3.5), (7.2, 12.5)], thin, false, point);
        }
        Icon::Ellipsis => {
            for x in [3.5, 8.0, 12.5] {
                painter.circle_filled(point(x, 8.0), 1.15 * scale, color);
            }
        }
        Icon::Warning => {
            let points = vec![point(8.0, 3.1), point(14.3, 13.0), point(1.7, 13.0)];
            painter.add(Shape::closed_line(points, stroke));
            line(painter, &[(8.0, 7.2), (8.0, 10.1)], thin, false, point);
            painter.circle_filled(point(8.0, 11.6), 0.55 * scale, color);
        }
    }
}

fn line(
    painter: &egui::Painter,
    coords: &[(f32, f32)],
    stroke: Stroke,
    closed: bool,
    point: impl Fn(f32, f32) -> Pos2,
) {
    let points: Vec<Pos2> = coords.iter().map(|(x, y)| point(*x, *y)).collect();
    painter.add(if closed {
        Shape::closed_line(points, stroke)
    } else {
        Shape::line(points, stroke)
    });
}

fn rounded_rect(painter: &egui::Painter, rect: Rect, radius: f32, stroke: Stroke) {
    painter.rect_stroke(
        rect,
        CornerRadius::same(radius.round() as u8),
        stroke,
        StrokeKind::Inside,
    );
}
