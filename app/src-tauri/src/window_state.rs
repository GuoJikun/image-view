//! 主窗口几何的采集与恢复（Tauri 适配层）。
//!
//! 几何存在 [`SettingsStore`] 的 `settings.window` 字段里，与其他偏好同一份 `settings.ron`、
//! 同一条防抖写盘链路。数据结构、恢复策略与旧格式解析都在 `iv_infra::window_state`；
//! 本文件只把 Tauri 的窗口/显示器 API 与事件接进去。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use iv_infra::window_state::{self, MonitorRect};
use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow, Window, WindowEvent};

use crate::settings_store::SettingsStore;

pub struct WindowStateStore {
    settings: Arc<SettingsStore>,
    /// 恢复期间窗口会发出 Moved/Resized，跳过采集避免覆盖保存值。
    restoring: AtomicBool,
}

impl WindowStateStore {
    pub fn new(settings: Arc<SettingsStore>) -> Self {
        Self {
            settings,
            restoring: AtomicBool::new(false),
        }
    }

    /// 把保存的几何应用到主窗口，并在需要时显示。
    /// 调用前应先按平台设置好 decorations，否则无边框窗口的 inner size 会与保存值错位。
    pub fn restore(&self, window: &WebviewWindow) {
        let monitors: Vec<MonitorRect> = window
            .available_monitors()
            .unwrap_or_default()
            .into_iter()
            .map(|monitor| MonitorRect {
                x: monitor.position().x,
                y: monitor.position().y,
                width: monitor.size().width,
                height: monitor.size().height,
            })
            .collect();

        let plan = window_state::plan_restore(Some(&self.settings.snapshot().window), &monitors);

        self.restoring.store(true, Ordering::SeqCst);
        // 位置先于尺寸，与插件时代顺序一致；最大化用 prev_* 还原后再 maximize。
        if let (Some(x), Some(y)) = (plan.x, plan.y) {
            let _ = window.set_position(PhysicalPosition { x, y });
        }
        if let (Some(width), Some(height)) = (plan.width, plan.height) {
            let _ = window.set_size(PhysicalSize { width, height });
        }
        if plan.maximized {
            let _ = window.maximize();
        }
        if plan.fullscreen {
            let _ = window.set_fullscreen(true);
        }
        if plan.show {
            let _ = window.show();
            let _ = window.set_focus();
        }
        self.restoring.store(false, Ordering::SeqCst);
    }

    /// 采集窗口事件中的几何变化；变更由 `SettingsStore` 防抖动盘。
    pub fn on_window_event(&self, window: &Window, event: &WindowEvent) {
        if self.restoring.load(Ordering::SeqCst) {
            return;
        }
        match event {
            WindowEvent::Moved(position) => {
                let maximized = window.is_maximized().unwrap_or(false);
                let minimized = window.is_minimized().unwrap_or(false);
                self.settings.update(|settings| {
                    window_state::capture_position(
                        &mut settings.window,
                        position.x,
                        position.y,
                        maximized,
                        minimized,
                    );
                });
            }
            WindowEvent::Resized(size) => {
                let maximized = window.is_maximized().unwrap_or(false);
                let minimized = window.is_minimized().unwrap_or(false);
                self.settings.update(|settings| {
                    window_state::capture_size(
                        &mut settings.window,
                        size.width,
                        size.height,
                        maximized,
                        minimized,
                    );
                });
            }
            WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed => {
                let maximized = window.is_maximized().unwrap_or(false);
                let fullscreen = window.is_fullscreen().unwrap_or(false);
                self.settings.update(|settings| {
                    window_state::capture_flags(&mut settings.window, maximized, fullscreen, true);
                });
                // 退出路径由 lib.rs 的 flush_settings 同步落盘，这里不重复写。
            }
            _ => {}
        }
    }
}
