//! 窗口几何状态：与主窗口位置/尺寸/最大化有关的纯数据与纯逻辑。
//!
//! 不依赖任何 GUI 框架，也不做文件 IO。几何作为 [`crate::settings::Settings`] 的
//! `window` 字段写进 `settings.ron`；GUI 适配层负责采集当前几何、调用 [`plan_restore`]
//! 得到恢复方案并应用到具体窗口。Tauri 与 Iced 共用同一份状态结构与恢复策略。
//!
//! [`from_legacy_json`] 仅供一次性迁移：Tauri 插件时代的 `.window-state.json` 由适配层
//! 读入文本后交给这里解析，迁移完成后不再写该文件。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 单个窗口的几何状态，物理像素。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowState {
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    /// 最大化之前的位置。最大化时系统会报出显示器左上角，若直接覆盖 x/y，
    /// 还原窗口后位置会跳到左上角；这里保留原位置供还原时使用。
    pub prev_x: i32,
    pub prev_y: i32,
    pub maximized: bool,
    pub visible: bool,
    pub decorated: bool,
    pub fullscreen: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            x: 0,
            y: 0,
            prev_x: 0,
            prev_y: 0,
            maximized: false,
            visible: true,
            decorated: true,
            fullscreen: false,
        }
    }
}

impl WindowState {
    /// 是否尚未记录过几何。用于判断首次启动，以及是否需要从旧插件文件迁移。
    pub fn is_unset(&self) -> bool {
        self.width == 0 && self.height == 0
    }
}

/// 显示器矩形，物理像素。由 GUI 适配层从窗口系统的显示器信息转换而来。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl MonitorRect {
    /// 窗口矩形是否落在显示器内（四角任一在内即视为有效）。
    /// 显示器拔掉或分辨率变化后，旧坐标可能已不在任何显示器上，此时应交给系统摆放。
    pub fn intersects(self, x: i32, y: i32, width: u32, height: u32) -> bool {
        let left = self.x;
        let top = self.y;
        let right = left.saturating_add(self.width as i32);
        let bottom = top.saturating_add(self.height as i32);
        let corners = [
            (x, y),
            (x.saturating_add(width as i32), y),
            (x, y.saturating_add(height as i32)),
            (
                x.saturating_add(width as i32),
                y.saturating_add(height as i32),
            ),
        ];
        corners
            .into_iter()
            .any(|(cx, cy)| cx >= left && cx < right && cy >= top && cy < bottom)
    }
}

/// 启动时的恢复方案。`None` 的字段表示不要改动、交给 GUI 默认值或系统摆放。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestorePlan {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub maximized: bool,
    pub fullscreen: bool,
    pub show: bool,
}

impl Default for RestorePlan {
    fn default() -> Self {
        Self {
            x: None,
            y: None,
            width: None,
            height: None,
            maximized: false,
            fullscreen: false,
            show: true,
        }
    }
}

impl RestorePlan {
    /// 保存的状态为空（首次启动）时的方案：沿用配置里的默认尺寸，显示窗口。
    pub fn fresh() -> Self {
        Self::default()
    }
}

/// 由保存的状态与当前可用显示器推导恢复方案。
///
/// - 尺寸只在大于 0 时恢复，避免旧文件里的占位值把窗口缩成 0。
/// - 位置只在窗口矩形仍与某台显示器相交时恢复；否则交给系统，防止窗口出现在屏幕外。
/// - 最大化状态用 `prev_x/prev_y` 还原位置，再最大化。
/// - 可见性：保存时是隐藏的，启动就保持隐藏；无记录时显示。
pub fn plan_restore(state: Option<&WindowState>, monitors: &[MonitorRect]) -> RestorePlan {
    let Some(state) = state else {
        return RestorePlan::fresh();
    };
    if state == &WindowState::default() {
        return RestorePlan::fresh();
    }

    let width = (state.width > 0).then_some(state.width);
    let height = (state.height > 0).then_some(state.height);
    let restored_x = if state.maximized {
        state.prev_x
    } else {
        state.x
    };
    let restored_y = if state.maximized {
        state.prev_y
    } else {
        state.y
    };

    // 相交判定用保存时的尺寸；没有尺寸记录时退化为点判定，让系统决定。
    let on_screen = monitors.iter().any(|monitor| {
        monitor.intersects(state.x, state.y, state.width.max(1), state.height.max(1))
    });

    RestorePlan {
        x: on_screen.then_some(restored_x),
        y: on_screen.then_some(restored_y),
        width,
        height,
        maximized: state.maximized,
        fullscreen: state.fullscreen,
        show: state.visible,
    }
}

/// 采集当前窗口几何；最大化或最小化时跳过尺寸/位置，避免把系统值写进正常态。
pub fn capture_size(
    state: &mut WindowState,
    width: u32,
    height: u32,
    maximized: bool,
    minimized: bool,
) {
    if maximized || minimized || width == 0 || height == 0 {
        return;
    }
    state.width = width;
    state.height = height;
}

/// 采集当前窗口位置。移动事件里先把旧位置存入 `prev_*`，再更新 `x/y`；
/// 这样最大化前的位置总能保留下来。
pub fn capture_position(state: &mut WindowState, x: i32, y: i32, maximized: bool, minimized: bool) {
    if maximized || minimized {
        return;
    }
    state.prev_x = state.x;
    state.prev_y = state.y;
    state.x = x;
    state.y = y;
}

/// 记录最大化/全屏/可见性。关闭窗口时用当前值更新，供下次启动恢复。
pub fn capture_flags(state: &mut WindowState, maximized: bool, fullscreen: bool, visible: bool) {
    state.maximized = maximized;
    state.fullscreen = fullscreen;
    state.visible = visible;
}

/// 解析插件时代的 `.window-state.json`（按 label 索引的 JSON 对象）。
///
/// 只用于一次性迁移：适配层读到文本后调用，取指定 label 的几何写进 `Settings`。
/// 缺失或损坏时返回 `None`，让新实现使用默认几何，不阻塞启动。
pub fn from_legacy_json(text: &str, label: &str) -> Option<WindowState> {
    let states: HashMap<String, WindowState> = match serde_json::from_str(text) {
        Ok(states) => states,
        Err(error) => {
            log::debug!(
                target: "image_view::window_state",
                "旧窗口状态文件无法解析，忽略：{error}"
            );
            return None;
        }
    };
    states.get(label).cloned().filter(|state| !state.is_unset())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> WindowState {
        WindowState {
            width: 1180,
            height: 800,
            x: 120,
            y: 80,
            prev_x: 100,
            prev_y: 60,
            maximized: false,
            visible: true,
            decorated: true,
            fullscreen: false,
        }
    }

    #[test]
    fn default_keeps_visible_and_decorated() {
        let state = WindowState::default();
        assert!(state.visible);
        assert!(state.decorated);
        assert!(!state.maximized);
    }

    #[test]
    fn monitor_intersection_checks_four_corners() {
        let monitor = MonitorRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        assert!(monitor.intersects(120, 80, 1180, 800));
        // 只有右边一小块落在显示器内也算有效。
        assert!(monitor.intersects(1900, 100, 200, 200));
        // 完全在显示器右侧之外。
        assert!(!monitor.intersects(1920, 100, 200, 200));
        // 完全在左侧之外。
        assert!(!monitor.intersects(-400, 100, 200, 200));
    }

    #[test]
    fn plan_restore_uses_fresh_defaults_without_state() {
        assert_eq!(plan_restore(None, &[]), RestorePlan::fresh());
        assert_eq!(
            plan_restore(Some(&WindowState::default()), &[]),
            RestorePlan::fresh()
        );
    }

    #[test]
    fn plan_restore_keeps_position_when_visible_on_monitor() {
        let monitors = [MonitorRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        }];
        let plan = plan_restore(Some(&sample()), &monitors);
        assert_eq!(plan.x, Some(120));
        assert_eq!(plan.y, Some(80));
        assert_eq!(plan.width, Some(1180));
        assert_eq!(plan.height, Some(800));
        assert!(plan.show);
    }

    #[test]
    fn plan_restore_drops_position_when_monitor_is_gone() {
        // 保存的窗口在 2000 附近，当前只有 1920 宽的显示器：位置交给系统。
        let mut state = sample();
        state.x = 2000;
        let monitors = [MonitorRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        }];
        let plan = plan_restore(Some(&state), &monitors);
        assert_eq!(plan.x, None);
        assert_eq!(plan.y, None);
        // 尺寸与可见性仍应恢复。
        assert_eq!(plan.width, Some(1180));
        assert!(plan.show);
    }

    #[test]
    fn plan_restore_uses_prev_position_for_maximized_state() {
        let monitors = [MonitorRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        }];
        let mut state = sample();
        state.maximized = true;
        let plan = plan_restore(Some(&state), &monitors);
        assert_eq!(plan.x, Some(100));
        assert_eq!(plan.y, Some(60));
        assert!(plan.maximized);
    }

    #[test]
    fn plan_restore_hides_window_when_saved_hidden() {
        let mut state = sample();
        state.visible = false;
        let plan = plan_restore(Some(&state), &[]);
        assert!(!plan.show);
    }

    #[test]
    fn capture_keeps_prev_position_and_skips_maximized_size() {
        let mut state = sample();
        capture_position(&mut state, 300, 200, false, false);
        capture_size(&mut state, 900, 600, false, false);
        assert_eq!((state.prev_x, state.prev_y), (120, 80));
        assert_eq!((state.x, state.y), (300, 200));
        assert_eq!((state.width, state.height), (900, 600));

        // 最大化期间系统报告的尺寸/位置不应污染正常态。
        capture_position(&mut state, 0, 0, true, false);
        capture_size(&mut state, 1920, 1080, true, false);
        assert_eq!((state.x, state.y), (300, 200));
        assert_eq!((state.width, state.height), (900, 600));
    }

    #[test]
    fn is_unset_only_for_missing_geometry() {
        assert!(WindowState::default().is_unset());
        assert!(!sample().is_unset());
    }

    #[test]
    fn legacy_plugin_json_is_compatible() {
        // 插件时代的字段顺序与集合：额外字段由 serde 忽略，缺失字段走 default。
        let json = r#"{"main":{"width":1024,"height":768,"x":10,"y":20,"prev_x":8,"prev_y":18,"maximized":true,"visible":true,"decorated":true,"fullscreen":false}}"#;
        let state = from_legacy_json(json, "main").expect("parse legacy");
        assert_eq!(state.width, 1024);
        assert!(state.maximized);
        assert!(state.visible);
    }

    #[test]
    fn legacy_json_ignores_other_labels_and_corrupt_text() {
        let json = r#"{"other":{"width":1024,"height":768}}"#;
        assert!(from_legacy_json(json, "main").is_none());
        assert!(from_legacy_json("{ not json", "main").is_none());
        // 有 label 但没有几何，视为未记录。
        assert!(from_legacy_json(r#"{"main":{}}"#, "main").is_none());
    }
}
