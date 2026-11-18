//! 主窗口几何：启动恢复、拖动采集与关闭落盘。
//!
//! 数据结构与恢复策略在 `iv_infra::window_state`（物理像素）；Iced 的窗口坐标是
//! 逻辑像素，因此这里负责两者换算，并绕开 Iced 0.14 的几处限制：
//!
//! - 没有「最大化 / 最小化」事件：依靠显式动作跟踪标志，关闭时再查询一次；
//! - 无法枚举显示器：Core 的相交校验拿不到依据，位置直接按保存值恢复；
//! - 没有 `set_visible`：隐藏创建的窗口用 `set_mode(Windowed / Fullscreen)` 显示。
//!
//! 拖动过程中的 `Moved` / `Resized` 只更新内存，关闭时统一写盘，
//! 避免每次移动都重写 `settings.ron`。

use iced::{Point, Size, Task};
use iv_infra::window_state::{self, WindowState};

use crate::message::{CloseQueryPart, Message};
use crate::state::AppState;
use crate::state::app_state::CloseQuery;

/// 恢复几何后系统仍会回抛 `Moved`；用几次轮询的宽限期忽略这些事件。
const RESTORE_GRACE_POLLS: u8 = 4;

/// 窗口创建完成：记下 Id 并查询 DPI；恢复几何要等 DPI 到手。
pub fn ready(state: &mut AppState, id: iced::window::Id) -> Task<Message> {
    state.window_id = Some(id);
    iced::window::scale_factor(id).map(Message::WindowScale)
}

/// DPI 到手后应用保存的几何，最后显示窗口。
pub fn scaled(state: &mut AppState, factor: f32) -> Task<Message> {
    crate::update::viewer::scale_factor(state, factor);
    state.scale_known = true;

    let Some(saved) = state.pending_window.take() else {
        return Task::none();
    };
    let Some(id) = state.window_id else {
        // 窗口 Id 还没到：留到下一次 `WindowReady` 之后的查询再应用。
        state.pending_window = Some(saved);
        return Task::none();
    };

    let scale = safe_scale(state.scale_factor);
    // Core 仍负责尺寸与最大化 / 全屏；位置见 `restore_position`。
    let plan = window_state::plan_restore(Some(&saved), &[]);

    state.maximized = plan.maximized;
    state.minimized = false;
    state.set_fullscreen(plan.fullscreen);
    state.restore_grace = RESTORE_GRACE_POLLS;

    let mut tasks = Vec::new();
    if let (Some(width), Some(height)) = (plan.width, plan.height) {
        tasks.push(iced::window::resize::<Message>(
            id,
            to_logical_size(width, height, scale),
        ));
    }
    let (x, y) = restore_position(&saved);
    tasks.push(iced::window::move_to::<Message>(
        id,
        to_logical_point(x, y, scale),
    ));
    if plan.maximized && !plan.fullscreen {
        tasks.push(iced::window::maximize::<Message>(id, true));
    }
    // `set_mode` 同时负责显示（Iced 创建窗口时总是先隐藏）。
    // 本分支没有托盘，`plan.show == false` 时也显示，避免窗口不可见且无从唤回。
    tasks.push(iced::window::set_mode::<Message>(
        id,
        if plan.fullscreen {
            iced::window::Mode::Fullscreen
        } else {
            iced::window::Mode::Windowed
        },
    ));

    Task::batch(tasks)
}

/// 记住正常态的移动；最大化 / 最小化 / 全屏与恢复期间的移动不采集。
pub fn moved(state: &mut AppState, point: Point) {
    if !can_capture(state) || state.fullscreen {
        return;
    }
    let (x, y) = to_physical_point(point, state.scale_factor);
    window_state::capture_position(
        &mut state.settings.window,
        x,
        y,
        state.maximized,
        state.minimized,
    );
}

/// 记住正常态的尺寸；判定条件与 [`moved`] 一致。
pub fn resized(state: &mut AppState, size: Size) {
    if !can_capture(state) || state.fullscreen {
        return;
    }
    let (width, height) = to_physical_size(size, state.scale_factor);
    window_state::capture_size(
        &mut state.settings.window,
        width,
        height,
        state.maximized,
        state.minimized,
    );
}

/// 恢复等待期与恢复宽限期内的事件都可能带着旧几何，不能写进保存值。
fn can_capture(state: &AppState) -> bool {
    state.scale_known && state.pending_window.is_none() && state.restore_grace == 0
}

/// 自绘标题栏的最大化按钮 / 双击。
///
/// 先翻标志再发命令：最大化会立刻触发一次 `Resized`，标志到位才能跳过它。
pub fn toggle_maximize(state: &mut AppState) -> Task<Message> {
    let Some(id) = state.window_id else {
        return Task::none();
    };
    if state.fullscreen {
        return Task::none();
    }
    state.maximized = !state.maximized;
    iced::window::maximize::<Message>(id, state.maximized)
}

/// 自绘标题栏的最小化按钮；`Focused` 事件负责复位标志。
pub fn minimize(state: &mut AppState) -> Task<Message> {
    let Some(id) = state.window_id else {
        return Task::none();
    };
    state.minimized = true;
    iced::window::minimize::<Message>(id, true)
}

/// 关闭请求：先查询五项窗口状态，凑齐后再采集、落盘并关窗。
pub fn begin_close(state: &mut AppState) -> Task<Message> {
    let Some(id) = state.window_id else {
        // 窗口还没创建（极早的关闭请求）：只落盘，不再等查询。
        crate::update::library::persist(state);
        return Task::none();
    };
    if state.closing.is_some() {
        // 查询在途：系统关闭按钮与自绘标题栏可能各发一次，忽略重复请求。
        return Task::none();
    }
    state.closing = Some(CloseQuery::default());

    Task::batch([
        iced::window::is_maximized(id)
            .map(|value| Message::CloseQuery(CloseQueryPart::Maximized(value))),
        iced::window::is_minimized(id)
            .map(|value| Message::CloseQuery(CloseQueryPart::Minimized(value.unwrap_or(false)))),
        iced::window::mode(id).map(|mode| {
            Message::CloseQuery(CloseQueryPart::Fullscreen(
                mode == iced::window::Mode::Fullscreen,
            ))
        }),
        iced::window::size(id).map(|size| Message::CloseQuery(CloseQueryPart::Size(size))),
        iced::window::position(id)
            .map(|point| Message::CloseQuery(CloseQueryPart::Position(point))),
    ])
}

/// 累积关闭查询结果；五项到齐后采集几何、落盘并关窗。
pub fn close_query(state: &mut AppState, part: CloseQueryPart) -> Task<Message> {
    let Some(query) = state.closing.as_mut() else {
        return Task::none();
    };
    query.absorb(part);
    if !query.complete() {
        return Task::none();
    }
    let query = state.closing.take().unwrap_or_default();

    let scale = safe_scale(state.scale_factor);
    let fullscreen = apply_close_query(&mut state.settings.window, &query, scale);
    state.maximized = query.maximized.unwrap_or(false);
    state.minimized = query.minimized.unwrap_or(false);
    state.set_fullscreen(fullscreen);

    crate::update::library::persist(state);

    match state.window_id {
        Some(id) => iced::window::close(id),
        None => Task::none(),
    }
}

/// 把查询结果写进 [`WindowState`]；返回采集到的全屏标志。
///
/// 最大化 / 最小化 / 全屏时系统报告的尺寸与位置不是「正常态」，
/// 只更新标志，保留上一次正常态的宽高与坐标。
fn apply_close_query(window: &mut WindowState, query: &CloseQuery, scale: f32) -> bool {
    let maximized = query.maximized.unwrap_or(false);
    let minimized = query.minimized.unwrap_or(false);
    let fullscreen = query.fullscreen.unwrap_or(false);

    if !maximized && !minimized && !fullscreen {
        if let Some(size) = query.size {
            let (width, height) = to_physical_size(size, scale);
            window_state::capture_size(window, width, height, false, false);
        }
        if let Some(Some(position)) = query.position {
            let (x, y) = to_physical_point(position, scale);
            window_state::capture_position(window, x, y, false, false);
        }
    }

    // 正常关闭一律记为可见：本分支没有「关到托盘」，下次启动必须能看到窗口。
    window_state::capture_flags(window, maximized, fullscreen, true);
    fullscreen
}

/// 保存的窗口位置（物理像素，窗口左上角）。
///
/// Iced 0.14 没有公开的显示器枚举 API，调用 `plan_restore` 时只能传空列表，
/// 它的相交校验因此会丢弃位置；本分支直接按保存值恢复。
/// 最大化时用 `prev_*`：系统报出的是显示器左上角，不是还原后的位置。
fn restore_position(saved: &WindowState) -> (i32, i32) {
    if saved.maximized {
        (saved.prev_x, saved.prev_y)
    } else {
        (saved.x, saved.y)
    }
}

/// 只接受有限且为正的 DPI；窗口事件在部分平台上会短暂给出 0。
fn safe_scale(scale: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

fn to_logical_size(width: u32, height: u32, scale: f32) -> Size {
    let scale = safe_scale(scale);
    Size::new(width as f32 / scale, height as f32 / scale)
}

fn to_logical_point(x: i32, y: i32, scale: f32) -> Point {
    let scale = safe_scale(scale);
    Point::new(x as f32 / scale, y as f32 / scale)
}

fn to_physical_size(size: Size, scale: f32) -> (u32, u32) {
    let scale = safe_scale(scale);
    (
        (size.width * scale).round().max(0.0) as u32,
        (size.height * scale).round().max(0.0) as u32,
    )
}

fn to_physical_point(point: Point, scale: f32) -> (i32, i32) {
    let scale = safe_scale(scale);
    (
        (point.x * scale).round() as i32,
        (point.y * scale).round() as i32,
    )
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    /// 每个用例独立目录，避免并行执行时互相覆盖设置文件。
    fn app_state() -> AppState {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "image-view-window-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        AppState::for_test(dir)
    }

    fn sample() -> WindowState {
        WindowState {
            width: 800,
            height: 600,
            x: 100,
            y: 100,
            prev_x: 80,
            prev_y: 80,
            maximized: false,
            visible: true,
            decorated: true,
            fullscreen: false,
        }
    }

    #[test]
    fn logical_and_physical_pixels_round_trip_at_fractional_dpi() {
        assert_eq!(to_logical_size(1800, 1200, 1.5), Size::new(1200.0, 800.0));
        assert_eq!(to_logical_point(300, 150, 1.5), Point::new(200.0, 100.0));
        assert_eq!(
            to_physical_size(Size::new(1200.0, 800.0), 1.5),
            (1800, 1200)
        );
        assert_eq!(to_physical_point(Point::new(200.0, 100.0), 1.5), (300, 150));
        // 非法 DPI 回落到 1.0，避免把窗口缩成 0。
        assert_eq!(to_physical_size(Size::new(640.0, 480.0), 0.0), (640, 480));
    }

    #[test]
    fn restore_position_uses_prev_position_when_maximized() {
        let mut saved = sample();
        assert_eq!(restore_position(&saved), (100, 100));

        // 最大化时系统报出的是显示器左上角，还原位置要用最大化前的坐标。
        saved.maximized = true;
        assert_eq!(restore_position(&saved), (80, 80));
    }

    #[test]
    fn moved_keeps_previous_position_and_converts_to_physical_pixels() {
        let mut state = app_state();
        state.settings.window = sample();
        state.scale_known = true;
        state.scale_factor = 2.0;

        moved(&mut state, Point::new(300.0, 200.0));

        let window = &state.settings.window;
        assert_eq!((window.x, window.y), (600, 400));
        // 旧位置留给「最大化后还原」。
        assert_eq!((window.prev_x, window.prev_y), (100, 100));
        assert_eq!((window.width, window.height), (800, 600));
    }

    #[test]
    fn moved_ignores_restore_window_and_fullscreen() {
        let mut state = app_state();
        state.settings.window = sample();
        state.scale_known = true;
        state.scale_factor = 1.0;

        // 恢复宽限期内的回抛事件不能覆盖保存值。
        state.restore_grace = 2;
        moved(&mut state, Point::new(500.0, 500.0));
        assert_eq!(
            (state.settings.window.x, state.settings.window.y),
            (100, 100)
        );

        // 等待 DPI 期间也不采集。
        state.restore_grace = 0;
        state.pending_window = Some(sample());
        moved(&mut state, Point::new(500.0, 500.0));
        assert_eq!(
            (state.settings.window.x, state.settings.window.y),
            (100, 100)
        );

        // 全屏时的移动同样不是正常态几何。
        state.pending_window = None;
        state.fullscreen = true;
        moved(&mut state, Point::new(500.0, 500.0));
        assert_eq!(
            (state.settings.window.x, state.settings.window.y),
            (100, 100)
        );
    }

    #[test]
    fn resized_converts_logical_size_to_physical() {
        let mut state = app_state();
        state.settings.window = sample();
        state.scale_known = true;
        state.scale_factor = 1.5;

        resized(&mut state, Size::new(1200.0, 800.0));

        let window = &state.settings.window;
        assert_eq!((window.width, window.height), (1800, 1200));
    }

    #[test]
    fn maximized_window_keeps_normal_geometry_when_closing() {
        let mut window = sample();

        let query = CloseQuery {
            maximized: Some(true),
            minimized: Some(false),
            fullscreen: Some(false),
            size: Some(Size::new(1920.0, 1080.0)),
            position: Some(Some(Point::new(0.0, 0.0))),
        };
        let fullscreen = apply_close_query(&mut window, &query, 1.0);

        assert!(!fullscreen);
        assert_eq!((window.width, window.height), (800, 600));
        assert_eq!((window.x, window.y), (100, 100));
        assert!(window.maximized);
        assert!(window.visible);
    }

    #[test]
    fn minimized_or_fullscreen_window_keeps_normal_geometry_when_closing() {
        for (minimized, fullscreen) in [(true, false), (false, true)] {
            let mut window = sample();
            let query = CloseQuery {
                maximized: Some(false),
                minimized: Some(minimized),
                fullscreen: Some(fullscreen),
                size: Some(Size::new(1920.0, 1080.0)),
                position: Some(Some(Point::new(0.0, 0.0))),
            };
            let captured_fullscreen = apply_close_query(&mut window, &query, 1.0);

            assert_eq!(captured_fullscreen, fullscreen);
            assert_eq!((window.width, window.height), (800, 600));
            assert_eq!((window.x, window.y), (100, 100));
        }
    }

    #[test]
    fn normal_close_captures_size_and_position_with_dpi() {
        let mut window = sample();
        let query = CloseQuery {
            maximized: Some(false),
            minimized: Some(false),
            fullscreen: Some(false),
            size: Some(Size::new(900.0, 700.0)),
            position: Some(Some(Point::new(200.0, 150.0))),
        };
        let fullscreen = apply_close_query(&mut window, &query, 2.0);

        assert!(!fullscreen);
        assert_eq!((window.width, window.height), (1800, 1400));
        assert_eq!((window.x, window.y), (400, 300));
        assert_eq!((window.prev_x, window.prev_y), (100, 100));
        assert!(!window.maximized);
    }

    #[test]
    fn close_query_waits_for_all_five_parts() {
        let mut query = CloseQuery::default();
        let parts = [
            CloseQueryPart::Maximized(false),
            CloseQueryPart::Minimized(false),
            CloseQueryPart::Fullscreen(false),
            CloseQueryPart::Size(Size::new(800.0, 600.0)),
            CloseQueryPart::Position(Some(Point::new(10.0, 20.0))),
        ];
        for (index, part) in parts.into_iter().enumerate() {
            assert!(!query.complete(), "第 {index} 项之前不应算完成");
            query.absorb(part);
        }
        assert!(query.complete());
        // 位置查询返回 `None`（平台不支持）也算作答，不能卡住关闭流程。
        let mut unsupported = CloseQuery::default();
        for part in [
            CloseQueryPart::Maximized(false),
            CloseQueryPart::Minimized(false),
            CloseQueryPart::Fullscreen(false),
            CloseQueryPart::Size(Size::new(800.0, 600.0)),
            CloseQueryPart::Position(None),
        ] {
            unsupported.absorb(part);
        }
        assert!(unsupported.complete());
    }
}
