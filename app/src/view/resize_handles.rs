//! 无边框窗口的手动缩放手柄（Windows / Linux）。
//!
//! 自绘标题栏要求 `decorations=false`，而 winit 在 Windows 上用 `WM_NCCALCSIZE`
//! 抹掉非客户区，系统自带的 `WS_THICKFRAME` 缩放边框也就没了——`resizable: true`
//! 只保留尺寸约束，用户拖边缘不会有任何反应。Iced 0.14 为此暴露了
//! `window::drag_resize`，这里在窗口外圈叠一圈透明 `mouse_area`，按下即触发对应
//! 方向的缩放。
//!
//! 手柄从标题栏下方开始：顶部 28px 是菜单 / 拖动区 / caption 按钮，交还给他们，
//! 避免抢走它们的点击和光标。因此提供左右边、底边、底部两角五个方向；顶部缩放
//! 走 caption 的最大化按钮，四边缩放足以改变窗口大小。

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{container, mouse_area, space, stack};
use iced::{Element, Fill, Length, Padding, mouse};

use crate::message::Message;

/// 边缘手柄厚度（px）。
const EDGE: f32 = 6.0;
/// 角落手柄边长（px），略大于边厚以便抓握。
const CORNER: f32 = 12.0;
/// 标题栏高度，手柄整体下移这么多，避开菜单 / caption。
const TITLEBAR: f32 = 28.0;

/// 方向对应的缩放光标。
fn interaction_for(direction: iced::window::Direction) -> mouse::Interaction {
    use iced::window::Direction;
    match direction {
        Direction::East | Direction::West => mouse::Interaction::ResizingHorizontally,
        Direction::North | Direction::South => mouse::Interaction::ResizingVertically,
        Direction::NorthEast | Direction::SouthWest => mouse::Interaction::ResizingDiagonallyUp,
        Direction::NorthWest | Direction::SouthEast => mouse::Interaction::ResizingDiagonallyDown,
    }
}

fn handle(
    direction: iced::window::Direction,
    width: impl Into<Length>,
    height: impl Into<Length>,
    align_x: Horizontal,
    align_y: Vertical,
) -> Element<'static, Message> {
    // 尺寸必须设在 mouse_area 内部的内容上：若只给外层 container 设尺寸，
    // mouse_area 裹着的 space() 仍是 0×0，没有可点击区域。
    let bar = mouse_area(space().width(width).height(height))
        .interaction(interaction_for(direction))
        .on_press(Message::DragResize(direction));
    // 外层填满整个手柄区域，align 才能把 bar 贴到对应边 / 角。container 本身不吃
    // 鼠标事件，只有内部的 mouse_area 捕获，其余区域照常穿透到底下的内容。
    container(bar)
        .width(Fill)
        .height(Fill)
        .align_x(align_x)
        .align_y(align_y)
        .into()
}

/// 叠加在内容之上的透明缩放手柄层。调用方保证仅在非 macOS、非全屏、非最大化时使用。
pub fn view() -> Element<'static, Message> {
    use iced::window::Direction;

    // stack 后加的在上层：角落放在边之后，让角落区域显示对角光标并优先响应。
    let west = handle(
        Direction::West,
        EDGE,
        Fill,
        Horizontal::Left,
        Vertical::Center,
    );
    let east = handle(
        Direction::East,
        EDGE,
        Fill,
        Horizontal::Right,
        Vertical::Center,
    );
    let south = handle(
        Direction::South,
        Fill,
        EDGE,
        Horizontal::Center,
        Vertical::Bottom,
    );
    let southwest = handle(
        Direction::SouthWest,
        CORNER,
        CORNER,
        Horizontal::Left,
        Vertical::Bottom,
    );
    let southeast = handle(
        Direction::SouthEast,
        CORNER,
        CORNER,
        Horizontal::Right,
        Vertical::Bottom,
    );

    container(
        stack([west, east, south, southwest, southeast])
            .width(Fill)
            .height(Fill),
    )
    .width(Fill)
    .height(Fill)
    .padding(Padding {
        top: TITLEBAR,
        ..Padding::ZERO
    })
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::window::Direction;

    #[test]
    fn edge_directions_use_matching_cursors() {
        assert_eq!(
            interaction_for(Direction::West),
            mouse::Interaction::ResizingHorizontally
        );
        assert_eq!(
            interaction_for(Direction::South),
            mouse::Interaction::ResizingVertically
        );
        assert_eq!(
            interaction_for(Direction::SouthEast),
            mouse::Interaction::ResizingDiagonallyDown
        );
    }
}
