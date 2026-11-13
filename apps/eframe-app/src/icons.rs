//! 工具栏 / 菜单 / 浮层 / 空状态图标。
//!
//! 统一 20×20 设计网格、18 pt 绘制框、1.6 pt 线宽、全线稿（三类实心例外），
//! 由 egui 路径 tessellator 提供圆头与圆角连接；颜色由调用方按六态传入，
//! 图标本身不内置颜色、不随主题改变形状、不依赖任何位图或字体图标。

use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};

use crate::visuals;

/// 设计约定的完整图标词表。部分变体（Info / Close / Retry / Rotate / RadioDot）
/// 由菜单、错误卡片、窗口与空状态按语义选用，当前实现未全部实例化；
/// 按 PRD §5.4 要求保留枚举，不删除、不重命名，避免下游调用方连锁改动。
#[allow(dead_code)]
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
    Thumbnails,
    Sort,
    Ellipsis,
    Image,
    Check,
    RadioDot,
    Warning,
    Trash,
    External,
    Copy,
    Info,
    Close,
    Retry,
    Rotate,
}

/// 在 `rect` 内绘制图标；`rect` 通常为 18×18 pt 绘制框。
///
/// 对外签名保持不变：所有调用方仍只传 `painter / rect / Icon / color`。
pub fn paint(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let size = rect.width().min(rect.height());
    if size <= 0.0 {
        return;
    }

    let ppp = painter.ctx().pixels_per_point();
    let scale = size / visuals::ICON_GRID;
    let origin = rect.center() - Vec2::splat(size / 2.0);
    let point = |x: f32, y: f32| origin + Vec2::new(x * scale, y * scale);
    // 线宽随绘制框等比缩放；18 pt 时恰为 1.6 pt，并至少保证 1 物理像素。
    let thickness = visuals::snap_stroke(visuals::ICON_STROKE, ppp)
        * (size / visuals::ICON_FRAME).clamp(0.6, 2.5);
    let stroke = Stroke::new(thickness, color);
    let thin = Stroke::new((thickness * 0.9).max(0.6), color);

    match icon {
        Icon::Folder => {
            line(
                painter,
                &[
                    (2.6, 5.2),
                    (2.6, 15.2),
                    (3.0, 15.6),
                    (17.0, 15.6),
                    (17.4, 15.2),
                    (17.4, 8.0),
                    (17.0, 7.6),
                    (9.2, 7.6),
                    (7.6, 5.2),
                    (3.0, 5.2),
                ],
                stroke,
                true,
                point,
            );
        }
        Icon::ChevronLeft => line(
            painter,
            &[(12.4, 4.8), (7.2, 10.0), (12.4, 15.2)],
            stroke,
            false,
            point,
        ),
        Icon::ChevronRight => line(
            painter,
            &[(7.6, 4.8), (12.8, 10.0), (7.6, 15.2)],
            stroke,
            false,
            point,
        ),
        Icon::ChevronDown => line(
            painter,
            &[(5.4, 8.0), (10.0, 12.6), (14.6, 8.0)],
            stroke,
            false,
            point,
        ),
        Icon::Minus => line(painter, &[(4.4, 10.0), (15.6, 10.0)], stroke, false, point),
        Icon::Plus => {
            line(painter, &[(4.4, 10.0), (15.6, 10.0)], stroke, false, point);
            line(painter, &[(10.0, 4.4), (10.0, 15.6)], stroke, false, point);
        }
        // 播放：1.6 pt 线稿三角（不再是实心多边形）。
        Icon::Play => line(
            painter,
            &[(7.2, 4.6), (15.4, 10.0), (7.2, 15.4), (7.2, 4.6)],
            stroke,
            true,
            point,
        ),
        // 暂停：两根圆头竖线（不再是实心矩形）。
        Icon::Pause => {
            line(painter, &[(7.6, 4.8), (7.6, 15.2)], stroke, false, point);
            line(painter, &[(12.4, 4.8), (12.4, 15.2)], stroke, false, point);
        }
        // 缩略图条开关：底部为缩略图带，语义不再复用于空状态。
        Icon::Thumbnails => {
            rounded_rect(
                painter,
                Rect::from_min_max(point(2.6, 3.6), point(17.4, 16.4)),
                2.0 * scale,
                stroke,
            );
            line(painter, &[(2.8, 12.4), (17.2, 12.4)], thin, false, point);
            line(painter, &[(8.0, 12.4), (8.0, 16.2)], thin, false, point);
            line(painter, &[(12.6, 12.4), (12.6, 16.2)], thin, false, point);
        }
        Icon::Sort => {
            line(painter, &[(4.2, 6.0), (15.8, 6.0)], stroke, false, point);
            line(painter, &[(4.2, 10.0), (12.6, 10.0)], stroke, false, point);
            line(painter, &[(4.2, 14.0), (9.4, 14.0)], stroke, false, point);
        }
        // 例外 1：三个实心圆点（直径 2.4 pt，间距 4.5 pt）。
        Icon::Ellipsis => {
            for x in [5.5, 10.0, 14.5] {
                painter.circle_filled(point(x, 10.0), 1.2 * scale, color);
            }
        }
        Icon::Image => {
            rounded_rect(
                painter,
                Rect::from_min_max(point(2.6, 3.8), point(17.4, 16.2)),
                2.2 * scale,
                stroke,
            );
            painter.circle_filled(point(7.0, 7.6), 1.3 * scale, color);
            line(
                painter,
                &[
                    (3.4, 14.0),
                    (8.0, 9.8),
                    (11.6, 13.0),
                    (13.8, 11.2),
                    (16.6, 13.8),
                ],
                thin,
                false,
                point,
            );
        }
        Icon::Check => line(
            painter,
            &[(4.2, 10.2), (8.4, 14.4), (15.8, 5.8)],
            stroke,
            false,
            point,
        ),
        // 例外 2：6 pt 实心圆点（单选项）。
        Icon::RadioDot => {
            painter.circle_filled(point(10.0, 10.0), 3.0 * scale, color);
        }
        Icon::Warning => {
            line(
                painter,
                &[(10.0, 2.8), (18.0, 16.4), (2.0, 16.4), (10.0, 2.8)],
                stroke,
                true,
                point,
            );
            line(painter, &[(10.0, 7.6), (10.0, 11.4)], thin, false, point);
            painter.circle_filled(point(10.0, 13.8), 0.7 * scale, color);
        }
        Icon::Trash => {
            line(painter, &[(3.4, 5.6), (16.6, 5.6)], stroke, false, point);
            line(
                painter,
                &[(7.8, 5.6), (7.8, 3.6), (12.2, 3.6), (12.2, 5.6)],
                thin,
                false,
                point,
            );
            line(
                painter,
                &[(5.0, 5.8), (5.6, 16.6), (14.4, 16.6), (15.0, 5.8)],
                stroke,
                false,
                point,
            );
            line(painter, &[(8.4, 8.6), (8.4, 14.0)], thin, false, point);
            line(painter, &[(11.6, 8.6), (11.6, 14.0)], thin, false, point);
        }
        Icon::External => {
            line(
                painter,
                &[(11.4, 3.6), (16.4, 3.6), (16.4, 8.6)],
                stroke,
                false,
                point,
            );
            line(painter, &[(16.4, 3.6), (9.4, 10.6)], stroke, false, point);
            line(
                painter,
                &[
                    (15.4, 11.4),
                    (15.4, 16.4),
                    (3.6, 16.4),
                    (3.6, 4.6),
                    (8.6, 4.6),
                ],
                stroke,
                false,
                point,
            );
        }
        Icon::Copy => {
            rounded_rect(
                painter,
                Rect::from_min_max(point(6.6, 6.6), point(16.6, 16.6)),
                1.8 * scale,
                stroke,
            );
            line(
                painter,
                &[(13.4, 3.6), (3.6, 3.6), (3.6, 13.4), (6.6, 13.4)],
                thin,
                false,
                point,
            );
        }
        Icon::Info => {
            painter.circle_stroke(point(10.0, 10.0), 7.4 * scale, stroke);
            line(painter, &[(10.0, 9.0), (10.0, 14.0)], thin, false, point);
            painter.circle_filled(point(10.0, 6.4), 0.8 * scale, color);
        }
        Icon::Close => {
            line(painter, &[(5.4, 5.4), (14.6, 14.6)], stroke, false, point);
            line(painter, &[(14.6, 5.4), (5.4, 14.6)], stroke, false, point);
        }
        Icon::Retry => {
            line(
                painter,
                &[(16.2, 8.2), (16.2, 3.8), (11.8, 3.8)],
                stroke,
                false,
                point,
            );
            line(
                painter,
                &[
                    (16.2, 4.0),
                    (15.0, 8.0),
                    (12.4, 3.9),
                    (8.4, 3.6),
                    (4.8, 6.2),
                    (3.8, 10.0),
                    (5.2, 13.8),
                    (8.6, 16.2),
                    (12.6, 16.0),
                    (15.6, 13.6),
                ],
                stroke,
                false,
                point,
            );
        }
        Icon::Rotate => {
            line(
                painter,
                &[(3.8, 8.2), (3.8, 3.8), (8.2, 3.8)],
                stroke,
                false,
                point,
            );
            line(
                painter,
                &[
                    (3.8, 4.0),
                    (5.0, 8.0),
                    (7.6, 3.9),
                    (11.6, 3.6),
                    (15.2, 6.2),
                    (16.2, 10.0),
                    (14.8, 13.8),
                    (11.4, 16.2),
                    (7.4, 16.0),
                    (4.4, 13.6),
                ],
                stroke,
                false,
                point,
            );
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

// ---------------------------------------------------------------------------
// QA 回归测试：图标词表（PRD §5.8 / A10）
// ---------------------------------------------------------------------------

#[cfg(test)]
mod qa_redesign_tests {
    use super::*;
    use eframe::egui;

    /// egui 0.37 的 `FullOutput` 若直接 drop，会因未应用的字体纹理 delta 而 panic。
    fn run_ui(ctx: &egui::Context, input: egui::RawInput, body: impl FnMut(&mut egui::Ui)) {
        ctx.run_ui(input, body).drop_without_applying_deltas();
    }

    const ALL_ICONS: [Icon; 22] = [
        Icon::Folder,
        Icon::ChevronLeft,
        Icon::ChevronRight,
        Icon::ChevronDown,
        Icon::Minus,
        Icon::Plus,
        Icon::Play,
        Icon::Pause,
        Icon::Thumbnails,
        Icon::Sort,
        Icon::Ellipsis,
        Icon::Image,
        Icon::Check,
        Icon::RadioDot,
        Icon::Warning,
        Icon::Trash,
        Icon::External,
        Icon::Copy,
        Icon::Info,
        Icon::Close,
        Icon::Retry,
        Icon::Rotate,
    ];

    #[test]
    fn every_icon_variant_paints_without_panicking() {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(400.0, 300.0),
            )),
            ..Default::default()
        };
        run_ui(&ctx, input, |ui| {
            let painter = ui.ctx().layer_painter(egui::LayerId::background());
            let frame = Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(18.0, 18.0));
            for icon in ALL_ICONS {
                paint(&painter, frame, icon, Color32::WHITE);
            }
            // 退化尺寸：不得 panic。
            paint(
                &painter,
                Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(0.0, 0.0)),
                Icon::Folder,
                Color32::WHITE,
            );
        });
    }

    #[test]
    fn icon_geometry_constants_follow_the_20_grid_18_frame_1_6_stroke_spec() {
        assert_eq!(
            (
                visuals::ICON_GRID,
                visuals::ICON_FRAME,
                visuals::ICON_STROKE
            ),
            (20.0, 18.0, 1.6)
        );
        assert_eq!(ALL_ICONS.len(), 22);
    }
}
