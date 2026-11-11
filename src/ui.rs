//! 顶部工具栏、底部状态栏、错误横幅与帮助窗口。

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind, Vec2,
};

use crate::folder::SortOrder;
use crate::icons::{self, Icon};
use crate::theme::{self, Palette, ThemeSetting};

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Open,
    Prev,
    Next,
    Fit,
    ZoomIn,
    ZoomOut,
    ToggleSlideshow,
    ToggleThumbnails,
    Help,
    RevealInExplorer,
    CopyPath,
    Delete,
    OpenLogs,
    SetSort(SortOrder),
    SetShowColorStatus(bool),
    SetTheme(ThemeSetting),
}

pub struct ToolbarState {
    pub can_prev: bool,
    pub can_next: bool,
    pub has_image: bool,
    pub slideshow: bool,
    pub show_thumbnails: bool,
    pub sort: SortOrder,
    pub zoom_percent: Option<f32>,
    pub show_color_status: bool,
    pub theme: ThemeSetting,
    /// 全屏且系统标题栏隐藏时，在工具栏中央显示的文件名与淡出透明度。
    pub center_title: Option<(String, f32)>,
}

fn sort_label(order: SortOrder) -> &'static str {
    match order {
        SortOrder::Name => "名称",
        SortOrder::ModifiedNewest => "修改时间",
    }
}

pub fn toolbar(ui: &mut egui::Ui, state: &ToolbarState) -> Option<Action> {
    let palette = theme::palette(ui.ctx());
    let mut action = None;

    let panel = egui::Panel::top("toolbar")
        .exact_size(52.0)
        .frame(
            egui::Frame::NONE
                .fill(palette.chrome)
                .inner_margin(egui::Margin::symmetric(12, 11)),
        )
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;

            ui.horizontal(|ui| {
                let open = icon_button(ui, Icon::Folder, &palette, true, false, false);
                if open.clicked() {
                    action = Some(Action::Open);
                }
                open.on_hover_text("打开图片 (Ctrl+O)");

                ui.add_space(10.0);
                divider(ui, &palette);
                ui.add_space(12.0);

                let prev = icon_button(
                    ui,
                    Icon::ChevronLeft,
                    &palette,
                    state.can_prev,
                    false,
                    false,
                );
                if prev.clicked() {
                    action = Some(Action::Prev);
                }
                prev.on_hover_text("上一张 (←)");

                let next = icon_button(
                    ui,
                    Icon::ChevronRight,
                    &palette,
                    state.can_next,
                    false,
                    false,
                );
                if next.clicked() {
                    action = Some(Action::Next);
                }
                next.on_hover_text("下一张 (→)");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let more = icon_button(ui, Icon::Ellipsis, &palette, true, false, false);
                    more.clone().on_hover_text("更多操作");
                    overflow_menu(&more, state, &palette, &mut action);

                    ui.add_space(12.0);
                    zoom_segment(
                        ui,
                        &palette,
                        state.zoom_percent,
                        state.has_image,
                        &mut action,
                    );

                    ui.add_space(12.0);
                    divider(ui, &palette);
                    ui.add_space(9.0);

                    let sort = sort_button(ui, &palette, state.sort);
                    sort_popup(&sort, &palette, state.sort, &mut action);

                    ui.add_space(4.0);
                    let slideshow = icon_button(
                        ui,
                        if state.slideshow {
                            Icon::Pause
                        } else {
                            Icon::Play
                        },
                        &palette,
                        state.has_image && state.can_next,
                        state.slideshow,
                        state.slideshow,
                    );
                    if slideshow.clicked() {
                        action = Some(Action::ToggleSlideshow);
                    }
                    slideshow.on_hover_text("幻灯片 (Space)");

                    ui.add_space(4.0);
                    let thumbnails = icon_button(
                        ui,
                        Icon::Sidebar,
                        &palette,
                        state.has_image,
                        state.show_thumbnails,
                        state.show_thumbnails,
                    );
                    if thumbnails.clicked() {
                        action = Some(Action::ToggleThumbnails);
                    }
                    thumbnails.on_hover_text("显示/隐藏缩略图条");
                });
            });

            if let Some((title, alpha)) = &state.center_title {
                paint_center_title(ui, &palette, title, *alpha);
            }
        });

    let rect = panel.response.rect;
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, palette.border),
    );

    action
}

fn paint_center_title(ui: &egui::Ui, palette: &Palette, title: &str, alpha: f32) {
    let alpha = alpha.clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return;
    }
    let rect = ui.max_rect();
    let color = Color32::from_rgba_unmultiplied(
        palette.text.r(),
        palette.text.g(),
        palette.text.b(),
        (alpha * 255.0) as u8,
    );
    let galley = ui
        .painter()
        .layout_no_wrap(title.to_owned(), FontId::proportional(13.0), color);
    let max_width = (rect.width() - 420.0).max(120.0);
    if galley.size().x <= max_width {
        ui.painter()
            .galley(rect.center() - galley.size() / 2.0, galley, color);
    }
}

fn icon_button(
    ui: &mut egui::Ui,
    icon: Icon,
    palette: &Palette,
    enabled: bool,
    active: bool,
    accent: bool,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(30.0), sense);

    if ui.is_rect_visible(rect) {
        let hovered = enabled && response.hovered();
        let pressed = enabled && response.is_pointer_button_down_on();
        let background = if pressed {
            palette.press
        } else if hovered || (active && enabled) {
            palette.hover
        } else {
            Color32::TRANSPARENT
        };
        if background.a() > 0 {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(6), background);
        }

        let color = if !enabled {
            palette.text3
        } else if accent || active {
            palette.accent
        } else {
            palette.icon
        };
        icons::paint(
            ui.painter(),
            Rect::from_center_size(rect.center(), Vec2::splat(17.0)),
            icon,
            color,
        );
    }

    response
}

fn divider(ui: &mut egui::Ui, palette: &Palette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 18.0), Sense::hover());
    ui.painter().vline(
        rect.center().x,
        rect.y_range(),
        Stroke::new(1.0, palette.border),
    );
}

fn sort_button(ui: &mut egui::Ui, palette: &Palette, sort: SortOrder) -> Response {
    let width = match sort {
        SortOrder::Name => 62.0,
        SortOrder::ModifiedNewest => 84.0,
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::click());

    if ui.is_rect_visible(rect) {
        let background = if response.is_pointer_button_down_on() {
            palette.press
        } else if response.hovered() {
            palette.hover
        } else {
            Color32::TRANSPARENT
        };
        if background.a() > 0 {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(6), background);
        }
        ui.painter().text(
            rect.left_center() + Vec2::new(9.0, 0.0),
            Align2::LEFT_CENTER,
            sort_label(sort),
            FontId::proportional(12.0),
            palette.text,
        );
        icons::paint(
            ui.painter(),
            Rect::from_center_size(
                egui::pos2(rect.right() - 15.0, rect.center().y),
                Vec2::splat(12.0),
            ),
            Icon::ChevronDown,
            palette.text2,
        );
    }

    response
}

fn sort_popup(
    response: &Response,
    _palette: &Palette,
    current: SortOrder,
    action: &mut Option<Action>,
) {
    egui::Popup::menu(response).show(|ui| {
        ui.set_min_width(120.0);
        for order in [SortOrder::Name, SortOrder::ModifiedNewest] {
            if ui
                .selectable_label(current == order, sort_label(order))
                .clicked()
            {
                *action = Some(Action::SetSort(order));
                ui.close();
            }
        }
    });
}

fn zoom_segment(
    ui: &mut egui::Ui,
    palette: &Palette,
    zoom_percent: Option<f32>,
    enabled: bool,
    action: &mut Option<Action>,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(118.0, 30.0), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }

    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(6),
        palette.track,
        Stroke::new(1.0, palette.control_border),
        StrokeKind::Inside,
    );

    let segment = rect.width() / 3.0;
    let minus_rect = Rect::from_min_size(rect.min, Vec2::new(segment, rect.height()));
    let plus_rect = Rect::from_min_size(
        egui::pos2(rect.right() - segment, rect.top()),
        Vec2::new(segment, rect.height()),
    );
    let value_rect = Rect::from_min_max(
        egui::pos2(rect.left() + segment + 1.5, rect.top() + 2.0),
        egui::pos2(rect.right() - segment - 1.5, rect.bottom() - 2.0),
    );

    let minus = ui.interact(
        minus_rect,
        ui.scope_id().with("zoom_segment_minus"),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let plus = ui.interact(
        plus_rect,
        ui.scope_id().with("zoom_segment_plus"),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let value = ui.interact(
        value_rect,
        ui.scope_id().with("zoom_segment_value"),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );

    if enabled {
        for (response, area) in [(&minus, minus_rect), (&plus, plus_rect)] {
            if response.hovered() || response.is_pointer_button_down_on() {
                let background = if response.is_pointer_button_down_on() {
                    palette.press
                } else {
                    palette.hover
                };
                ui.painter().rect_filled(
                    area.shrink2(Vec2::new(2.0, 3.0)),
                    CornerRadius::same(4),
                    background,
                );
            }
        }
    }

    ui.painter().rect(
        value_rect,
        CornerRadius::same(5),
        palette.control,
        Stroke::new(1.0, palette.control_border),
        StrokeKind::Inside,
    );

    let tint = if enabled { palette.icon } else { palette.text3 };
    let icon_center = |area: Rect| Rect::from_center_size(area.center(), Vec2::splat(16.0));
    icons::paint(ui.painter(), icon_center(minus_rect), Icon::Minus, tint);
    icons::paint(ui.painter(), icon_center(plus_rect), Icon::Plus, tint);

    let text = zoom_percent
        .filter(|_| enabled)
        .map(|zoom| format!("{zoom:.0}%"))
        .unwrap_or_else(|| "—".to_owned());
    ui.painter().text(
        value_rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(12.0),
        if enabled { palette.text } else { palette.text3 },
    );

    if minus.clicked() {
        *action = Some(Action::ZoomOut);
    }
    minus.on_hover_text("缩小 (-)");
    if plus.clicked() {
        *action = Some(Action::ZoomIn);
    }
    plus.on_hover_text("放大 (+)");
    if value.clicked() {
        *action = Some(Action::Fit);
    }
    value.on_hover_text("适配窗口 (0)");
}

fn overflow_menu(
    response: &Response,
    state: &ToolbarState,
    palette: &Palette,
    action: &mut Option<Action>,
) {
    egui::Popup::menu(response).show(|ui| {
        ui.set_min_width(216.0);

        if ui
            .add_enabled(state.has_image, egui::Button::new("在资源管理器中显示"))
            .clicked()
        {
            *action = Some(Action::RevealInExplorer);
            ui.close();
        }
        if ui
            .add_enabled(
                state.has_image,
                egui::Button::new("复制路径    Ctrl+Shift+C"),
            )
            .clicked()
        {
            *action = Some(Action::CopyPath);
            ui.close();
        }

        ui.separator();

        if ui
            .add_enabled(
                state.has_image,
                egui::Button::new(
                    egui::RichText::new("移到回收站    Delete").color(palette.danger),
                ),
            )
            .clicked()
        {
            *action = Some(Action::Delete);
            ui.close();
        }

        ui.separator();

        let mut show_color_status = state.show_color_status;
        if ui
            .checkbox(&mut show_color_status, "显示色彩转换标记")
            .changed()
        {
            *action = Some(Action::SetShowColorStatus(show_color_status));
        }

        ui.menu_button("主题", |ui| {
            for setting in [
                ThemeSetting::System,
                ThemeSetting::Light,
                ThemeSetting::Dark,
            ] {
                if ui
                    .selectable_label(state.theme == setting, setting.label())
                    .clicked()
                {
                    *action = Some(Action::SetTheme(setting));
                    ui.close();
                }
            }
        });

        ui.separator();

        if ui.button("打开日志文件夹").clicked() {
            *action = Some(Action::OpenLogs);
            ui.close();
        }
        if ui.button("快捷键…").clicked() {
            *action = Some(Action::Help);
            ui.close();
        }
    });
}

pub struct StatusState {
    pub dimensions: Option<String>,
    pub format: Option<String>,
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
    let palette = theme::palette(ui.ctx());
    let panel = egui::Panel::bottom("status_bar")
        .exact_size(26.0)
        .frame(
            egui::Frame::NONE
                .fill(palette.status)
                .inner_margin(egui::Margin::symmetric(14, 0)),
        )
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
            ui.horizontal_centered(|ui| {
                let small = |text: String, color: Color32| {
                    egui::RichText::new(text).size(11.0).color(color)
                };
                let dot = |ui: &mut egui::Ui| {
                    ui.label(small("·".to_owned(), palette.text3));
                };

                if state.dimensions.is_none() && state.position.is_none() {
                    ui.label(
                        egui::RichText::new("未打开图片")
                            .size(11.0)
                            .color(palette.text3),
                    );
                }
                if let Some(dimensions) = &state.dimensions {
                    ui.label(small(dimensions.clone(), palette.text2));
                }
                if let Some(format) = &state.format {
                    dot(ui);
                    ui.label(small(format.clone(), palette.text3));
                }
                if let Some(color) = &state.color {
                    dot(ui);
                    ui.label(small(color.clone(), palette.text3));
                }
                if state.downscaled {
                    dot(ui);
                    ui.label(small(
                        "已降采样".to_owned(),
                        Color32::from_rgb(230, 160, 50),
                    ));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((index, total)) = state.position {
                        ui.label(small(format!("{index} / {total}"), palette.text2));
                    }
                    if let Some(notice) = &state.notice {
                        ui.label(small(notice.clone(), palette.success));
                    }
                    if state.slideshow {
                        ui.label(small("幻灯片中".to_owned(), palette.text2));
                    }
                    if state.loading {
                        ui.spinner();
                    }
                });
            });
        });

    let rect = panel.response.rect;
    ui.painter().hline(
        rect.x_range(),
        rect.top() + 0.5,
        Stroke::new(1.0, palette.border),
    );
}

/// 绘制错误横幅；返回 `true` 表示用户点击了“重试”。
pub fn error_banner(ui: &mut egui::Ui, error: &mut Option<String>, can_retry: bool) -> bool {
    let Some(message) = error.as_ref() else {
        return false;
    };
    let palette = theme::palette(ui.ctx());
    let mut dismiss = false;
    let mut retry = false;

    let panel = egui::Panel::top("error_banner")
        .frame(
            egui::Frame::NONE
                .fill(palette.danger_soft)
                .inner_margin(egui::Margin::symmetric(16, 8)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                icons::paint(ui.painter(), icon_rect, Icon::Warning, palette.danger);
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(message)
                        .size(11.5)
                        .color(palette.danger_text),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("关闭").size(11.5).color(palette.text2),
                            )
                            .frame(false),
                        )
                        .clicked()
                    {
                        dismiss = true;
                    }
                    if can_retry
                        && ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("重试").size(11.5).color(palette.danger),
                                )
                                .frame(false),
                            )
                            .clicked()
                    {
                        retry = true;
                    }
                });
            });
        });

    let rect = panel.response.rect;
    ui.painter().rect_filled(
        Rect::from_min_size(rect.left_top(), Vec2::new(3.0, rect.height())),
        CornerRadius::ZERO,
        palette.danger,
    );

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
