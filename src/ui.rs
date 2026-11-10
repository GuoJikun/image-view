//! 顶部工具栏、底部状态栏、错误横幅与帮助窗口。

use eframe::egui;

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Open,
    Prev,
    Next,
    Fit,
    ActualSize,
    ZoomIn,
    ZoomOut,
    ToggleSlideshow,
    Help,
}

pub struct ToolbarState {
    pub can_prev: bool,
    pub can_next: bool,
    pub has_image: bool,
    pub slideshow: bool,
    pub zoom_percent: Option<f32>,
}

pub fn toolbar(ui: &mut egui::Ui, state: &ToolbarState) -> Option<Action> {
    let mut action = None;

    egui::Panel::top("toolbar").show(ui, |ui| {
        ui.horizontal(|ui| {
            if ui
                .button("打开")
                .on_hover_text("打开图片 (Ctrl+O)")
                .clicked()
            {
                action = Some(Action::Open);
            }

            ui.separator();

            if ui
                .add_enabled(state.can_prev, egui::Button::new("<"))
                .on_hover_text("上一张 (←)")
                .clicked()
            {
                action = Some(Action::Prev);
            }
            if ui
                .add_enabled(state.can_next, egui::Button::new(">"))
                .on_hover_text("下一张 (→)")
                .clicked()
            {
                action = Some(Action::Next);
            }

            ui.separator();

            if ui
                .add_enabled(state.has_image, egui::Button::new("适配"))
                .on_hover_text("适配窗口 (0)")
                .clicked()
            {
                action = Some(Action::Fit);
            }
            if ui
                .add_enabled(state.has_image, egui::Button::new("100%"))
                .on_hover_text("原始大小 (1)")
                .clicked()
            {
                action = Some(Action::ActualSize);
            }

            ui.separator();

            if ui
                .add_enabled(state.has_image, egui::Button::new("-"))
                .on_hover_text("缩小 (-)")
                .clicked()
            {
                action = Some(Action::ZoomOut);
            }
            let percent = state
                .zoom_percent
                .map(|zoom| format!("{zoom:.0}%"))
                .unwrap_or_else(|| "—".to_owned());
            ui.label(percent);
            if ui
                .add_enabled(state.has_image, egui::Button::new("+"))
                .on_hover_text("放大 (+)")
                .clicked()
            {
                action = Some(Action::ZoomIn);
            }

            ui.separator();

            if ui
                .add_enabled(
                    state.has_image && state.can_next,
                    egui::Button::new("幻灯片").selected(state.slideshow),
                )
                .on_hover_text("每 3 秒自动前进 (Space)")
                .clicked()
            {
                action = Some(Action::ToggleSlideshow);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("帮助").clicked() {
                    action = Some(Action::Help);
                }
            });
        });
    });

    action
}

pub struct StatusState {
    pub file_name: Option<String>,
    pub dimensions: Option<String>,
    pub zoom_percent: Option<f32>,
    pub position: Option<(usize, usize)>,
    pub downscaled: bool,
    pub loading: bool,
    pub slideshow: bool,
}

pub fn status_bar(ui: &mut egui::Ui, state: &StatusState) {
    egui::Panel::bottom("status_bar").show(ui, |ui| {
        ui.horizontal(|ui| {
            if let Some(file_name) = &state.file_name {
                ui.label(file_name).on_hover_text(file_name);
            } else {
                ui.weak("未打开图片");
            }

            if let Some(dimensions) = &state.dimensions {
                ui.separator();
                ui.label(dimensions);
            }

            if let Some(zoom) = state.zoom_percent {
                ui.separator();
                ui.label(format!("{zoom:.0}%"));
            }

            if let Some((index, total)) = state.position {
                ui.separator();
                ui.label(format!("{index} / {total}"));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if state.loading {
                    ui.spinner();
                    ui.label("加载中…");
                }
                if state.slideshow {
                    ui.label("幻灯片中");
                }
                if state.downscaled {
                    ui.label(
                        egui::RichText::new("已降采样")
                            .color(egui::Color32::from_rgb(240, 180, 80)),
                    );
                }
            });
        });
    });
}

/// 绘制错误横幅；返回 `true` 表示用户点击了“重试”。
pub fn error_banner(ui: &mut egui::Ui, error: &mut Option<String>, can_retry: bool) -> bool {
    let Some(message) = error.as_ref() else {
        return false;
    };
    let mut dismiss = false;
    let mut retry = false;

    egui::Panel::top("error_banner")
        .frame(
            egui::Frame::NONE
                .fill(egui::Color32::from_rgb(110, 32, 32))
                .inner_margin(egui::Margin::symmetric(10, 6)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(message).color(egui::Color32::WHITE));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("关闭").clicked() {
                        dismiss = true;
                    }
                    if can_retry && ui.button("重试").clicked() {
                        retry = true;
                    }
                });
            });
        });

    if dismiss {
        *error = None;
    }
    retry
}

pub fn help_window(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }

    egui::Window::new("快捷键")
        .open(open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            egui::Grid::new("shortcut_grid")
                .num_columns(2)
                .spacing([28.0, 6.0])
                .show(ui, |ui| {
                    for (key, action) in [
                        ("Ctrl+O", "打开文件"),
                        ("← / →", "上一张 / 下一张"),
                        ("Space", "开始 / 停止幻灯片"),
                        ("+ / -", "放大 / 缩小"),
                        ("0", "适配窗口"),
                        ("1", "100%"),
                        ("F", "切换适配 / 100%"),
                        ("R", "顺时针旋转 90°"),
                        ("Shift+R", "逆时针旋转 90°"),
                        ("F11", "全屏"),
                        ("Esc", "退出全屏 / 关闭帮助"),
                    ] {
                        ui.monospace(key);
                        ui.label(action);
                        ui.end_row();
                    }
                });
        });
}
