//! 顶部工具栏、底部状态栏、错误横幅与帮助窗口。

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind, Vec2,
};

use crate::folder::SortOrder;
use crate::i18n::{self, Key, Language};
use crate::icons::{self, Icon};
use crate::theme::{self, Palette, ThemeSetting};
use crate::update::{Manifest, UpdateState, Updater};

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
    CheckUpdate,
    SetSort(SortOrder),
    SetShowColorStatus(bool),
    SetTheme(ThemeSetting),
    SetLanguage(Language),
    SetCheckUpdatesOnStartup(bool),
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
    pub language: Language,
    pub check_updates_on_startup: bool,
    /// 全屏且系统标题栏隐藏时，在工具栏中央显示的文件名与淡出透明度。
    pub center_title: Option<(String, f32)>,
}

fn sort_label(order: SortOrder) -> &'static str {
    match order {
        SortOrder::Name => i18n::t(Key::SortName),
        SortOrder::ModifiedNewest => i18n::t(Key::SortModified),
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
                open.on_hover_text(i18n::t(Key::OpenTooltip));

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
                prev.on_hover_text(i18n::t(Key::PrevTooltip));

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
                next.on_hover_text(i18n::t(Key::NextTooltip));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let more = icon_button(ui, Icon::Ellipsis, &palette, true, false, false);
                    more.clone().on_hover_text(i18n::t(Key::MoreActions));
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
                    slideshow.on_hover_text(i18n::t(Key::SlideshowTooltip));

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
                    thumbnails.on_hover_text(i18n::t(Key::ThumbnailsTooltip));
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
    let label = sort_label(sort);
    let text_width = ui
        .painter()
        .layout_no_wrap(label.to_owned(), FontId::proportional(12.0), palette.text)
        .size()
        .x;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new((text_width + 38.0).max(62.0), 30.0),
        Sense::click(),
    );

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
            label,
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
    minus.on_hover_text(i18n::t(Key::ZoomOutTooltip));
    if plus.clicked() {
        *action = Some(Action::ZoomIn);
    }
    plus.on_hover_text(i18n::t(Key::ZoomInTooltip));
    if value.clicked() {
        *action = Some(Action::Fit);
    }
    value.on_hover_text(i18n::t(Key::FitTooltip));
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
            .add_enabled(
                state.has_image,
                egui::Button::new(i18n::t(Key::RevealInExplorer)),
            )
            .clicked()
        {
            *action = Some(Action::RevealInExplorer);
            ui.close();
        }
        if ui
            .add_enabled(
                state.has_image,
                egui::Button::new(i18n::t(Key::CopyPathMenu)),
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
                    egui::RichText::new(i18n::t(Key::MoveToTrashMenu)).color(palette.danger),
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
            .checkbox(&mut show_color_status, i18n::t(Key::ShowColorStatus))
            .changed()
        {
            *action = Some(Action::SetShowColorStatus(show_color_status));
        }

        ui.menu_button(i18n::t(Key::ThemeMenu), |ui| {
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

        ui.menu_button(i18n::t(Key::LanguageMenu), |ui| {
            for language in [Language::System, Language::Zh, Language::En] {
                if ui
                    .selectable_label(state.language == language, language.label())
                    .clicked()
                {
                    *action = Some(Action::SetLanguage(language));
                    ui.close();
                }
            }
        });

        let mut check_updates = state.check_updates_on_startup;
        if ui
            .checkbox(&mut check_updates, i18n::t(Key::UpdateStartup))
            .changed()
        {
            *action = Some(Action::SetCheckUpdatesOnStartup(check_updates));
        }

        ui.separator();

        if ui.button(i18n::t(Key::CheckUpdates)).clicked() {
            *action = Some(Action::CheckUpdate);
            ui.close();
        }
        if ui.button(i18n::t(Key::OpenLogsFolder)).clicked() {
            *action = Some(Action::OpenLogs);
            ui.close();
        }
        if ui.button(i18n::t(Key::ShortcutsEllipsis)).clicked() {
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
                        egui::RichText::new(i18n::t(Key::NoImage))
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
                        i18n::t(Key::Downscaled).to_owned(),
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
                        ui.label(small(
                            i18n::t(Key::SlideshowRunning).to_owned(),
                            palette.text2,
                        ));
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
                                egui::RichText::new(i18n::t(Key::Close))
                                    .size(11.5)
                                    .color(palette.text2),
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
                                    egui::RichText::new(i18n::t(Key::Retry))
                                        .size(11.5)
                                        .color(palette.danger),
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

    egui::Window::new(i18n::t(Key::HelpTitle))
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
                        ("Ctrl+O", i18n::t(Key::HelpOpen)),
                        ("← / →", i18n::t(Key::HelpNavigate)),
                        ("Space", i18n::t(Key::HelpSlideshow)),
                        ("+ / -", i18n::t(Key::HelpZoom)),
                        ("0", i18n::t(Key::HelpFit)),
                        ("1", "100%"),
                        ("F", i18n::t(Key::HelpToggleFit)),
                        ("R", i18n::t(Key::HelpRotateCw)),
                        ("Shift+R", i18n::t(Key::HelpRotateCcw)),
                        ("Ctrl+Shift+C", i18n::t(Key::HelpCopyPath)),
                        ("Delete", i18n::t(Key::HelpTrash)),
                        ("F11", i18n::t(Key::HelpFullscreen)),
                        ("Esc", i18n::t(Key::HelpEscape)),
                    ] {
                        ui.monospace(key);
                        ui.label(action);
                        ui.end_row();
                    }
                });
        });
}

/// 更新窗口返回的用户操作。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateCommand {
    Check,
    Download(Manifest),
    Cancel,
    Install(std::path::PathBuf),
}

/// 弹窗底部的动作区：按钮靠右排列，闭包中先添加的按钮在最右侧。
fn dialog_actions(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add);
}

/// 主操作按钮：主色填充、白色文字。
fn primary_button(ui: &mut egui::Ui, label: &str) -> Response {
    let accent = theme::palette(ui.ctx()).accent;
    ui.add(egui::Button::new(egui::RichText::new(label).color(Color32::WHITE)).fill(accent))
}

/// 软件更新窗口；`state` 由 `app` 每帧轮询后台任务后传入。
pub fn update_window(
    ctx: &egui::Context,
    open: &mut bool,
    state: &UpdateState,
    notes_cache: &mut egui_commonmark::CommonMarkCache,
) -> Option<UpdateCommand> {
    if !*open {
        return None;
    }

    let palette = theme::palette(ctx);
    let mut command = None;
    let mut close_requested = false;

    egui::Window::new(i18n::t(Key::UpdateTitle))
        .open(open)
        .collapsible(false)
        .resizable(false)
        .default_width(420.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(i18n::tf(Key::UpdateCurrent, &[&Updater::current_version()]))
                    .size(11.5)
                    .color(palette.text2),
            );
            ui.add_space(8.0);

            match state {
                UpdateState::Idle => {
                    ui.label(i18n::t(Key::UpdateHint));
                    ui.add_space(8.0);
                    dialog_actions(ui, |ui| {
                        if ui.button(i18n::t(Key::UpdateCheck)).clicked() {
                            command = Some(UpdateCommand::Check);
                        }
                    });
                }
                UpdateState::Checking => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(i18n::t(Key::UpdateChecking));
                    });
                }
                UpdateState::UpToDate => {
                    ui.label(
                        egui::RichText::new(i18n::t(Key::UpdateUpToDate)).color(palette.success),
                    );
                    ui.add_space(8.0);
                    dialog_actions(ui, |ui| {
                        if ui.button(i18n::t(Key::UpdateCheck)).clicked() {
                            command = Some(UpdateCommand::Check);
                        }
                    });
                }
                UpdateState::Available { manifest } => {
                    ui.label(i18n::tf(
                        Key::UpdateAvailable,
                        &[&manifest.version, &Updater::current_version()],
                    ));
                    if !manifest.notes.trim().is_empty() {
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(i18n::t(Key::UpdateNotes))
                                .strong()
                                .size(11.5),
                        );
                        egui::ScrollArea::vertical()
                            .max_height(160.0)
                            .show(ui, |ui| {
                                // 发布说明是 Markdown；固定 Id 让渲染缓存跨帧复用，
                                // 悬停链接时顺便显示完整地址。
                                ui.style_mut().url_in_tooltip = true;
                                egui_commonmark::CommonMarkViewer::new().show_with_id(
                                    egui::Id::unique("update-notes"),
                                    ui,
                                    notes_cache,
                                    &manifest.notes,
                                );
                            });
                    }
                    ui.add_space(12.0);
                    dialog_actions(ui, |ui| {
                        if primary_button(ui, i18n::t(Key::UpdateDownload)).clicked() {
                            command = Some(UpdateCommand::Download(manifest.clone()));
                        }
                        if ui.button(i18n::t(Key::Later)).clicked() {
                            close_requested = true;
                        }
                    });
                }
                UpdateState::Downloading { received, total } => {
                    ui.label(i18n::t(Key::UpdateDownloading));
                    ui.add_space(6.0);
                    let known_total = total.filter(|total| *total > 0);
                    // egui 用 `extreme_bg_color` 画进度条轨道，而主题里它等于窗口底色，
                    // 深色主题下轨道会完全看不见；这里只在进度条作用域内换成对比色。
                    ui.scope(|ui| {
                        ui.visuals_mut().extreme_bg_color = theme::progress_track(ctx);
                        let bar = match known_total {
                            Some(total) => {
                                let fraction = (*received as f32 / total as f32).clamp(0.0, 1.0);
                                egui::ProgressBar::new(fraction).show_percentage()
                            }
                            // 总长度未知时用 egui 的循环动画，避免进度条完全静止。
                            None => egui::ProgressBar::new(0.0).animate(true),
                        };
                        ui.add(bar.fill(palette.accent).desired_width(380.0));
                    });
                    let amount = match known_total {
                        Some(total) => format!("{} / {}", human_size(*received), human_size(total)),
                        None => human_size(*received),
                    };
                    ui.label(egui::RichText::new(amount).size(11.0).color(palette.text2));
                    ui.add_space(8.0);
                    dialog_actions(ui, |ui| {
                        if ui.button(i18n::t(Key::UpdateCancel)).clicked() {
                            command = Some(UpdateCommand::Cancel);
                        }
                    });
                }
                UpdateState::Ready {
                    installer,
                    manifest,
                } => {
                    ui.label(
                        egui::RichText::new(i18n::tf(
                            Key::UpdateAvailable,
                            &[&manifest.version, &Updater::current_version()],
                        ))
                        .size(11.5),
                    );
                    ui.label(egui::RichText::new(i18n::t(Key::UpdateReady)).color(palette.success));
                    ui.add_space(8.0);
                    dialog_actions(ui, |ui| {
                        if primary_button(ui, i18n::t(Key::UpdateInstall)).clicked() {
                            command = Some(UpdateCommand::Install(installer.clone()));
                        }
                        if ui.button(i18n::t(Key::Later)).clicked() {
                            close_requested = true;
                        }
                    });
                }
                UpdateState::Error(error) => {
                    ui.label(
                        egui::RichText::new(i18n::tf(Key::UpdateError, &[&error.message()]))
                            .color(palette.danger),
                    );
                    ui.add_space(8.0);
                    dialog_actions(ui, |ui| {
                        if ui.button(i18n::t(Key::UpdateCheck)).clicked() {
                            command = Some(UpdateCommand::Check);
                        }
                    });
                }
            }
        });

    if close_requested {
        *open = false;
    }
    command
}

fn human_size(bytes: u64) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    const KIB: f64 = 1024.0;
    let bytes = bytes as f64;
    if bytes >= MIB {
        format!("{:.1} MiB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes / KIB)
    } else {
        format!("{bytes:.0} B")
    }
}
