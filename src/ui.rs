//! 顶部工具栏、底部状态栏、错误横幅与帮助窗口。

use eframe::egui;

use crate::folder::SortOrder;

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
    RevealInExplorer,
    CopyPath,
    Delete,
    OpenLogs,
    SetSort(SortOrder),
    SetShowColorStatus(bool),
}

pub struct ToolbarState {
    pub can_prev: bool,
    pub can_next: bool,
    pub has_image: bool,
    pub slideshow: bool,
    pub sort: SortOrder,
    pub zoom_percent: Option<f32>,
    pub show_color_status: bool,
}

fn sort_label(order: SortOrder) -> &'static str {
    match order {
        SortOrder::Name => "排序：名称",
        SortOrder::ModifiedNewest => "排序：修改时间",
    }
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

            ui.separator();

            egui::ComboBox::from_id_salt("sort_order")
                .width(104.0)
                .selected_text(sort_label(state.sort))
                .show_ui(ui, |ui| {
                    for order in [SortOrder::Name, SortOrder::ModifiedNewest] {
                        if ui
                            .selectable_label(state.sort == order, sort_label(order))
                            .clicked()
                        {
                            action = Some(Action::SetSort(order));
                        }
                    }
                });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("帮助").clicked() {
                    action = Some(Action::Help);
                }
                ui.menu_button("文件", |ui| {
                    let mut show_color_status = state.show_color_status;
                    if ui
                        .checkbox(&mut show_color_status, "显示色彩转换标记")
                        .changed()
                    {
                        action = Some(Action::SetShowColorStatus(show_color_status));
                    }
                    ui.separator();
                    if ui
                        .add_enabled(state.has_image, egui::Button::new("在资源管理器中显示"))
                        .clicked()
                    {
                        action = Some(Action::RevealInExplorer);
                        ui.close();
                    }
                    if ui
                        .add_enabled(state.has_image, egui::Button::new("复制路径  Ctrl+Shift+C"))
                        .clicked()
                    {
                        action = Some(Action::CopyPath);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("打开日志文件夹").clicked() {
                        action = Some(Action::OpenLogs);
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            state.has_image,
                            egui::Button::new(
                                egui::RichText::new("移到回收站  Delete")
                                    .color(egui::Color32::from_rgb(240, 120, 120)),
                            ),
                        )
                        .clicked()
                    {
                        action = Some(Action::Delete);
                        ui.close();
                    }
                });
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
    /// 色彩转换标记（仅在设置开启时由 app 填入）。
    pub color: Option<String>,
    /// 短暂提示，例如“已复制路径”。
    pub notice: Option<String>,
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
                if let Some(color) = &state.color {
                    ui.label(
                        egui::RichText::new(color).color(egui::Color32::from_rgb(120, 180, 240)),
                    )
                    .on_hover_text("已按内嵌 ICC profile 转换到 sRGB");
                }
                if let Some(notice) = &state.notice {
                    ui.label(
                        egui::RichText::new(notice).color(egui::Color32::from_rgb(140, 210, 150)),
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
                        ("Ctrl+Shift+C", "复制当前图片路径"),
                        ("Delete", "移到回收站"),
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
