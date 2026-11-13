//! 顶部工具栏、底部状态栏、菜单共用件与帮助 / 更新窗口。
//!
//! 顶栏按「3 组胶囊 + 独立 ⋯」重构：56 pt 高、16 pt 窗口边距、
//! 组间 12 / 组内 2 / ⋯ 与前组 16，胶囊 36 高圆角 10 + 1 pt 控件描边。
//! 六态控件状态统一由 `visuals::resolve_state` 解析；错误横幅已迁至
//! `overlay::error_card`（画布内浮层，不占布局）。

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind, Vec2,
};

use iv_domain::{Language, SortOrder, ThemeSetting};
pub use iv_gui_contract::{Action, ToolbarState};

use crate::i18n::{self, Key};
use crate::icons::{self, Icon};
use crate::overlay;
use crate::theme::{self, Palette};
use crate::update::{Manifest, UpdateState, Updater};
use crate::visuals::{self, ControlState, WidgetKind};

fn sort_label(order: SortOrder) -> &'static str {
    match order {
        SortOrder::Name => i18n::t(Key::SortName),
        SortOrder::ModifiedNewest => i18n::t(Key::SortModified),
    }
}

/// 分组胶囊：分配固定宽度、绘制 S2 底 + 1 pt 控件描边，内部按 2 pt 节奏排按钮。
fn capsule<R>(
    ui: &mut egui::Ui,
    palette: &Palette,
    width: f32,
    pad_h: f32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, visuals::CAPSULE_H), Sense::hover());
    let ppp = ui.ctx().pixels_per_point();
    let draw = visuals::snap_rect(rect, ppp);
    ui.painter().rect(
        draw,
        CornerRadius::same(visuals::snap_radius(visuals::CAPSULE_RADIUS, ppp)),
        palette.control,
        Stroke::new(visuals::STRUCTURE_LINE, palette.control_border),
        StrokeKind::Inside,
    );

    let inner = rect.shrink2(Vec2::new(pad_h, visuals::CAPSULE_PAD_V));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.spacing_mut().item_spacing = Vec2::new(visuals::IN_GROUP_GAP, 0.0);
    add(&mut child)
}

pub fn toolbar(ui: &mut egui::Ui, state: &ToolbarState) -> Option<Action> {
    let base = theme::palette(ui.ctx());
    let palette = visuals::chrome_palette(ui.ctx(), base);
    let mut action: Option<Action> = None;

    // egui 0.37 移除了 `Context::screen_rect()`，等价入口为 `viewport_rect()`；
    // 每帧读取，不缓存，窄窗口降级即时生效。
    let screen_width = ui.ctx().viewport_rect().width();
    let collapse_view = screen_width < visuals::NARROW_COLLAPSE_W;
    let compact_sort = screen_width < visuals::NARROW_SORT_W;

    let panel = egui::Panel::top("toolbar")
        .exact_size(visuals::TOPBAR_H)
        .frame(
            egui::Frame::NONE
                .fill(palette.chrome)
                .inner_margin(egui::Margin::symmetric(
                    visuals::WINDOW_MARGIN.round() as i8,
                    ((visuals::TOPBAR_H - visuals::CAPSULE_H) / 2.0).round() as i8,
                )),
        )
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;

            ui.horizontal_centered(|ui| {
                // 左组胶囊：打开 / 上一张 / 下一张。
                let left_w = 2.0 * visuals::CAPSULE_PAD_H
                    + 3.0 * visuals::BTN_SIZE
                    + 2.0 * visuals::IN_GROUP_GAP;
                capsule(ui, &palette, left_w, visuals::CAPSULE_PAD_H, |ui| {
                    let open = icon_button(ui, Icon::Folder, &palette, true, false);
                    if open.clicked() {
                        action = Some(Action::Open);
                    }
                    open.on_hover_text(i18n::t(Key::OpenTooltip));

                    let prev = icon_button(ui, Icon::ChevronLeft, &palette, state.can_prev, false);
                    if prev.clicked() {
                        action = Some(Action::Prev);
                    }
                    prev.on_hover_text(i18n::t(Key::PrevTooltip));

                    let next = icon_button(ui, Icon::ChevronRight, &palette, state.can_next, false);
                    if next.clicked() {
                        action = Some(Action::Next);
                    }
                    next.on_hover_text(i18n::t(Key::NextTooltip));
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // 右组 C：独立 ⋯ 胶囊（36×36）。
                    let more = capsule(ui, &palette, 36.0, 2.0, |ui| {
                        let response = icon_button(ui, Icon::Ellipsis, &palette, true, false);
                        response.clone().on_hover_text(i18n::t(Key::MoreActions));
                        response
                    });
                    overflow_menu(&more, state, &base, &mut action, collapse_view);

                    ui.add_space(visuals::MORE_GAP);

                    // 右组 B：缩放分段 132×36。
                    zoom_segment(
                        ui,
                        &palette,
                        state.zoom_percent,
                        state.has_image,
                        &mut action,
                    );

                    if !collapse_view {
                        ui.add_space(visuals::GROUP_GAP);

                        // 右组 A：视图组（缩略图条 / 幻灯片 / 排序）。
                        let sort_w = if compact_sort {
                            visuals::BTN_SIZE
                        } else {
                            visuals::SORT_MIN_W
                        };
                        let view_w = 2.0 * visuals::CAPSULE_PAD_H
                            + 2.0 * visuals::BTN_SIZE
                            + sort_w
                            + 2.0 * visuals::IN_GROUP_GAP;
                        capsule(ui, &palette, view_w, visuals::CAPSULE_PAD_H, |ui| {
                            let thumbnails = icon_button(
                                ui,
                                Icon::Thumbnails,
                                &palette,
                                state.has_image,
                                state.show_thumbnails,
                            );
                            if thumbnails.clicked() {
                                action = Some(Action::ToggleThumbnails);
                            }
                            thumbnails.on_hover_text(i18n::t(Key::ThumbnailsTooltip));

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
                            );
                            if slideshow.clicked() {
                                action = Some(Action::ToggleSlideshow);
                            }
                            slideshow.on_hover_text(i18n::t(Key::SlideshowTooltip));

                            let sort = if compact_sort {
                                // Q8：窄窗口降级为图标按钮，tooltip 复用现有排序文案键。
                                icon_button(ui, Icon::Sort, &palette, true, false)
                                    .on_hover_text(sort_label(state.sort))
                            } else {
                                sort_button(ui, &palette, state.sort)
                            };
                            sort_popup(&sort, &base, state.sort, &mut action);
                        });
                    }
                });
            });
        });

    let rect = panel.response.rect;
    let y = visuals::snap_line_center(rect.bottom() - 0.5, ui.ctx().pixels_per_point());
    ui.painter().hline(
        rect.x_range(),
        y,
        Stroke::new(visuals::STRUCTURE_LINE, palette.structure_stroke),
    );

    action
}

/// 六态图标按钮：32×32 命中区、圆角 8、18 pt 图标。
///
/// 开启态 = 强调浅底 + 强调色图标 + 1 pt 强调 @45% 描边；
/// hover = 中性叠色（与开启态明显不同色）；禁用不响应 hover/click 但保留 tooltip；
/// 焦点环仅在键盘导航且该按钮持有键盘焦点时出现（B2）。
fn icon_button(
    ui: &mut egui::Ui,
    icon: Icon,
    palette: &Palette,
    enabled: bool,
    active: bool,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(visuals::BTN_SIZE), sense);

    if ui.is_rect_visible(rect) {
        let ppp = ui.ctx().pixels_per_point();
        let draw = visuals::snap_rect(rect, ppp);
        let radius = CornerRadius::same(visuals::snap_radius(visuals::BTN_RADIUS, ppp));

        let control = if !enabled {
            ControlState::Disabled
        } else if active {
            ControlState::Selected
        } else if response.is_pointer_button_down_on() {
            ControlState::Pressed
        } else if response.hovered() {
            ControlState::Hovered
        } else {
            ControlState::Default
        };
        let style = visuals::resolve_state(palette, control, WidgetKind::IconButton);
        if style.bg.a() > 0 {
            ui.painter().rect_filled(draw, radius, style.bg);
        }
        if let Some(stroke) = style.stroke {
            ui.painter()
                .rect_stroke(draw, radius, stroke, StrokeKind::Inside);
        }

        if enabled
            && visuals::keyboard_nav(ui.ctx())
            && ui.memory(|memory| memory.focused() == Some(response.id))
        {
            let focused =
                visuals::resolve_state(palette, ControlState::Focused, WidgetKind::IconButton);
            if focused.focus_ring {
                ui.painter().rect_stroke(
                    draw.expand(2.0),
                    CornerRadius::same(visuals::snap_radius(10.0, ppp)),
                    Stroke::new(2.0, palette.focus_ring),
                    StrokeKind::Inside,
                );
            }
        }

        icons::paint(
            ui.painter(),
            Rect::from_center_size(rect.center(), Vec2::splat(visuals::ICON_FRAME)),
            icon,
            style.fg,
        );
    }

    response
}

fn sort_button(ui: &mut egui::Ui, palette: &Palette, sort: SortOrder) -> Response {
    let label = sort_label(sort);
    let text_width = ui
        .painter()
        .layout_no_wrap(
            label.to_owned(),
            FontId::proportional(visuals::FS_BODY),
            palette.text,
        )
        .size()
        .x;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(
            (text_width + 38.0).max(visuals::SORT_MIN_W),
            visuals::SORT_H,
        ),
        Sense::click(),
    );

    if ui.is_rect_visible(rect) {
        let control = if response.is_pointer_button_down_on() {
            ControlState::Pressed
        } else if response.hovered() {
            ControlState::Hovered
        } else {
            ControlState::Default
        };
        let style = visuals::resolve_state(palette, control, WidgetKind::SortButton);
        if style.bg.a() > 0 {
            let ppp = ui.ctx().pixels_per_point();
            ui.painter().rect_filled(
                visuals::snap_rect(rect, ppp),
                CornerRadius::same(visuals::snap_radius(visuals::BTN_RADIUS, ppp)),
                style.bg,
            );
        }
        ui.painter().text(
            rect.left_center() + Vec2::new(9.0, 0.0),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(visuals::FS_BODY),
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
    palette: &Palette,
    current: SortOrder,
    action: &mut Option<Action>,
) {
    egui::Popup::menu(response).show(|ui| {
        ui.set_min_width(visuals::MENU_MIN_W);
        for order in [SortOrder::Name, SortOrder::ModifiedNewest] {
            if overlay::menu_radio(ui, palette, current == order, sort_label(order), true) {
                *action = Some(Action::SetSort(order));
                ui.close();
            }
        }
    });
}

/// 缩放分段 132×36 圆角 10；值块 56×30 圆角 7；点击值块 = 适配窗口（行为不变）。
fn zoom_segment(
    ui: &mut egui::Ui,
    palette: &Palette,
    zoom_percent: Option<f32>,
    enabled: bool,
    action: &mut Option<Action>,
) {
    let ppp = ui.ctx().pixels_per_point();
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(visuals::ZOOM_W, visuals::ZOOM_H), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }

    ui.painter().rect(
        visuals::snap_rect(rect, ppp),
        CornerRadius::same(visuals::snap_radius(visuals::ZOOM_RADIUS, ppp)),
        palette.track,
        Stroke::new(visuals::STRUCTURE_LINE, palette.control_border),
        StrokeKind::Inside,
    );

    let minus_rect = Rect::from_min_size(
        rect.min,
        Vec2::new(visuals::ZOOM_BTN_W, visuals::ZOOM_BTN_H),
    );
    let plus_rect = Rect::from_min_size(
        egui::pos2(rect.right() - visuals::ZOOM_BTN_W, rect.top()),
        Vec2::new(visuals::ZOOM_BTN_W, visuals::ZOOM_BTN_H),
    );
    let value_rect = Rect::from_center_size(
        rect.center(),
        Vec2::new(visuals::ZOOM_VALUE_W, visuals::ZOOM_VALUE_H),
    );

    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let minus = ui.interact(minus_rect, ui.scope_id().with("zoom_segment_minus"), sense);
    let plus = ui.interact(plus_rect, ui.scope_id().with("zoom_segment_plus"), sense);
    let value = ui.interact(value_rect, ui.scope_id().with("zoom_segment_value"), sense);

    if enabled {
        for (response, area) in [(&minus, minus_rect), (&plus, plus_rect)] {
            if response.hovered() || response.is_pointer_button_down_on() {
                let control = if response.is_pointer_button_down_on() {
                    ControlState::Pressed
                } else {
                    ControlState::Hovered
                };
                let style = visuals::resolve_state(palette, control, WidgetKind::ZoomSegment);
                ui.painter().rect_filled(
                    area.shrink2(Vec2::new(2.0, 3.0)),
                    CornerRadius::same(visuals::snap_radius(4.0, ppp)),
                    style.bg,
                );
            }
        }
    }

    ui.painter().rect(
        visuals::snap_rect(value_rect, ppp),
        CornerRadius::same(visuals::snap_radius(visuals::ZOOM_VALUE_RADIUS, ppp)),
        palette.control,
        Stroke::new(visuals::STRUCTURE_LINE, palette.control_border),
        StrokeKind::Inside,
    );

    let tint = if enabled { palette.icon } else { palette.text3 };
    let icon_center =
        |area: Rect| Rect::from_center_size(area.center(), Vec2::splat(visuals::ICON_FRAME));
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
        FontId::proportional(visuals::FS_BODY),
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

/// 溢出菜单：S3 浮层、最小宽 232、项高 32、分组 8 pt + 跨组 1 pt 线、自绘复选/单选。
fn overflow_menu(
    response: &Response,
    state: &ToolbarState,
    palette: &Palette,
    action: &mut Option<Action>,
    collapse_view: bool,
) {
    egui::Popup::menu(response).show(|ui| {
        ui.set_min_width(visuals::MENU_MIN_W);

        // 组 1：当前图片操作。
        if overlay::menu_item(
            ui,
            palette,
            Some(Icon::External),
            i18n::t(Key::RevealInExplorer),
            false,
            state.has_image,
        )
        .clicked()
        {
            *action = Some(Action::RevealInExplorer);
            ui.close();
        }
        if overlay::menu_item(
            ui,
            palette,
            Some(Icon::Copy),
            i18n::t(Key::CopyPathMenu),
            false,
            state.has_image,
        )
        .clicked()
        {
            *action = Some(Action::CopyPath);
            ui.close();
        }

        overlay::menu_group_gap(ui, palette);

        // 危险项单独成组。
        if overlay::menu_item(
            ui,
            palette,
            Some(Icon::Trash),
            i18n::t(Key::MoveToTrashMenu),
            true,
            state.has_image,
        )
        .clicked()
        {
            *action = Some(Action::Delete);
            ui.close();
        }

        // <560 px：视图组折叠进 ⋯，所有功能仍可达（P1-1）。
        if collapse_view {
            overlay::menu_group_gap(ui, palette);
            if overlay::menu_item(
                ui,
                palette,
                Some(Icon::Thumbnails),
                i18n::t(Key::ThumbnailsTooltip),
                false,
                state.has_image,
            )
            .clicked()
            {
                *action = Some(Action::ToggleThumbnails);
                ui.close();
            }
            if overlay::menu_item(
                ui,
                palette,
                Some(if state.slideshow {
                    Icon::Pause
                } else {
                    Icon::Play
                }),
                i18n::t(Key::SlideshowTooltip),
                false,
                state.has_image && state.can_next,
            )
            .clicked()
            {
                *action = Some(Action::ToggleSlideshow);
                ui.close();
            }
            for order in [SortOrder::Name, SortOrder::ModifiedNewest] {
                if overlay::menu_radio(ui, palette, state.sort == order, sort_label(order), true) {
                    *action = Some(Action::SetSort(order));
                    ui.close();
                }
            }
        }

        overlay::menu_group_gap(ui, palette);

        // 组 2：设置项。
        let mut show_color_status = state.show_color_status;
        if overlay::menu_checkbox(
            ui,
            palette,
            show_color_status,
            i18n::t(Key::ShowColorStatus),
            true,
        ) {
            show_color_status = !show_color_status;
            *action = Some(Action::SetShowColorStatus(show_color_status));
        }

        ui.menu_button(i18n::t(Key::ThemeMenu), |ui| {
            ui.set_min_width(visuals::MENU_MIN_W);
            for setting in [
                ThemeSetting::System,
                ThemeSetting::Light,
                ThemeSetting::Dark,
            ] {
                if overlay::menu_radio(
                    ui,
                    palette,
                    state.theme == setting,
                    theme::theme_label(setting),
                    true,
                ) {
                    *action = Some(Action::SetTheme(setting));
                    ui.close();
                }
            }
        });

        ui.menu_button(i18n::t(Key::LanguageMenu), |ui| {
            ui.set_min_width(visuals::MENU_MIN_W);
            for language in [Language::System, Language::Zh, Language::En] {
                if overlay::menu_radio(
                    ui,
                    palette,
                    state.language == language,
                    i18n::language_label(language),
                    true,
                ) {
                    *action = Some(Action::SetLanguage(language));
                    ui.close();
                }
            }
        });

        let mut check_updates = state.check_updates_on_startup;
        if overlay::menu_checkbox(
            ui,
            palette,
            check_updates,
            i18n::t(Key::UpdateStartup),
            true,
        ) {
            check_updates = !check_updates;
            *action = Some(Action::SetCheckUpdatesOnStartup(check_updates));
        }

        overlay::menu_group_gap(ui, palette);

        // 组 3：更新 / 日志 / 许可证 / 帮助。
        if overlay::menu_item(ui, palette, None, i18n::t(Key::CheckUpdates), false, true).clicked()
        {
            *action = Some(Action::CheckUpdate);
            ui.close();
        }
        if overlay::menu_item(ui, palette, None, i18n::t(Key::OpenLogsFolder), false, true)
            .clicked()
        {
            *action = Some(Action::OpenLogs);
            ui.close();
        }
        if overlay::menu_item(
            ui,
            palette,
            None,
            i18n::t(Key::ThirdPartyLicenses),
            false,
            true,
        )
        .clicked()
        {
            *action = Some(Action::OpenLicenses);
            ui.close();
        }
        if overlay::menu_item(
            ui,
            palette,
            None,
            i18n::t(Key::ShortcutsEllipsis),
            false,
            true,
        )
        .clicked()
        {
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
    /// 窗口四角是否做圆角。仅 Linux 自绘装饰且窗口化时为 `true`。
    pub rounded_corners: bool,
}

/// 状态栏 28 pt：11.5 pt、尺寸字重 500、3 pt 圆点分隔符、元信息归栏。
/// 短提示已迁至右下 toast 浮层，状态栏不再绘制绿色提示文字。
pub fn status_bar(ui: &mut egui::Ui, state: &StatusState) {
    let base = theme::palette(ui.ctx());
    let palette = visuals::chrome_palette(ui.ctx(), base);
    let radius = if state.rounded_corners {
        crate::titlebar::CORNER_RADIUS as u8
    } else {
        0
    };

    let panel = egui::Panel::bottom("status_bar")
        .exact_size(visuals::STATUS_H)
        .frame(
            egui::Frame::NONE
                .fill(palette.status)
                .inner_margin(egui::Margin::symmetric(
                    visuals::WINDOW_MARGIN.round() as i8,
                    0,
                ))
                .corner_radius(egui::CornerRadius {
                    nw: 0,
                    ne: 0,
                    sw: radius,
                    se: radius,
                }),
        )
        .show(ui, |ui| {
            // 每项之间 6 pt，圆点分隔符左右各 6 pt。
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
            ui.horizontal_centered(|ui| {
                let normal = |text: String, color: Color32| {
                    egui::RichText::new(text)
                        .size(visuals::FS_STATUS)
                        .color(color)
                };
                let strong = |text: String, color: Color32| {
                    egui::RichText::new(text)
                        .size(visuals::FS_STATUS)
                        .strong()
                        .color(color)
                };
                let dot = |ui: &mut egui::Ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(3.0), Sense::hover());
                    ui.painter()
                        .circle_filled(rect.center(), 1.5, palette.text3);
                };

                if state.dimensions.is_none() && state.position.is_none() {
                    ui.label(normal(i18n::t(Key::NoImage).to_owned(), palette.text3));
                }
                if let Some(dimensions) = &state.dimensions {
                    ui.label(strong(dimensions.clone(), palette.text));
                }
                if let Some(format) = &state.format {
                    dot(ui);
                    ui.label(normal(format.clone(), palette.text2));
                }
                if let Some(color) = &state.color {
                    dot(ui);
                    ui.label(normal(color.clone(), palette.text2));
                }
                if state.downscaled {
                    dot(ui);
                    ui.label(normal(i18n::t(Key::Downscaled).to_owned(), palette.warning));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((index, total)) = state.position {
                        ui.label(strong(format!("{index} / {total}"), palette.text));
                    }
                    if state.slideshow {
                        ui.label(normal(
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
    let y = visuals::snap_line_center(rect.top() + 0.5, ui.ctx().pixels_per_point());
    ui.painter().hline(
        rect.x_range(),
        y,
        Stroke::new(visuals::STRUCTURE_LINE, palette.structure_stroke),
    );
}

pub fn help_window(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }

    // S3 浮层外观由 `theme::install` 的 window_fill / window_corner_radius 统一提供。
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
    let height = ui.spacing().interact_size.y;
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), height),
        egui::Layout::right_to_left(egui::Align::Center),
        add,
    );
}

/// 主操作按钮：强调色实心 + `on_accent` 白字，≥96×36、圆角 9。
fn primary_button(ui: &mut egui::Ui, label: &str) -> Response {
    let palette = theme::palette(ui.ctx());
    let text_width = ui
        .painter()
        .layout_no_wrap(
            label.to_owned(),
            FontId::proportional(visuals::FS_BUTTON),
            palette.on_accent,
        )
        .size()
        .x;
    let width = (text_width + 32.0).max(visuals::PRIMARY_BTN_MIN_W);
    ui.add(
        egui::Button::new(egui::RichText::new(label).color(palette.on_accent))
            .fill(palette.accent)
            .min_size(Vec2::new(width, visuals::PRIMARY_BTN_H))
            .corner_radius(CornerRadius::same(visuals::PRIMARY_BTN_RADIUS as u8)),
    )
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
    // 内容区留白：左右 16、上下 16；标题栏保持默认内边距。
    let base_frame = egui::Frame::window(ctx.style_of(ctx.theme()).as_ref());
    let content_frame = base_frame.inner_margin(egui::Margin::symmetric(16, 16));

    egui::Window::new(i18n::t(Key::UpdateTitle))
        .open(open)
        .collapsible(false)
        .resizable(false)
        .default_width(460.0)
        .frame(content_frame)
        .title_frame(base_frame)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(i18n::tf(Key::UpdateCurrent, &[&Updater::current_version()]))
                    .size(visuals::FS_STATUS)
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
                                .size(visuals::FS_STATUS),
                        );
                        egui::ScrollArea::vertical()
                            .max_height(180.0)
                            .show(ui, |ui| {
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
                    ui.scope(|ui| {
                        ui.visuals_mut().extreme_bg_color = theme::progress_track(ctx);
                        let bar = match known_total {
                            Some(total) => {
                                let fraction = (*received as f32 / total as f32).clamp(0.0, 1.0);
                                egui::ProgressBar::new(fraction).show_percentage()
                            }
                            None => egui::ProgressBar::new(0.0).animate(true),
                        };
                        ui.add(bar.fill(palette.accent).desired_width(ui.available_width()));
                    });
                    let amount = match known_total {
                        Some(total) => format!("{} / {}", human_size(*received), human_size(total)),
                        None => human_size(*received),
                    };
                    ui.label(
                        egui::RichText::new(amount)
                            .size(visuals::FS_STATUS)
                            .color(palette.text2),
                    );
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
                        .size(visuals::FS_STATUS),
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

// ---------------------------------------------------------------------------
// QA 回归测试：顶栏 / 缩放 / 六态映射（PRD §5.5 / §5.7）
// ---------------------------------------------------------------------------

#[cfg(test)]
mod qa_redesign_tests {
    use super::*;

    #[test]
    fn toolbar_uses_the_prd_dimensions_and_group_rhythm() {
        let v = |value: f32, want: f32, label: &str| {
            assert!(
                (value - want).abs() < 1e-4,
                "{label}: 期望 {want}，实际 {value}"
            );
        };
        v(crate::visuals::TOPBAR_H, 56.0, "顶栏高");
        v(crate::visuals::STATUS_H, 28.0, "状态栏高");
        v(crate::visuals::ZOOM_W, 132.0, "缩放分段宽");
        v(crate::visuals::ZOOM_H, 36.0, "缩放分段高");
        v(crate::visuals::ZOOM_VALUE_W, 56.0, "值块宽");
        v(crate::visuals::ZOOM_VALUE_H, 30.0, "值块高");
        v(crate::visuals::ZOOM_VALUE_RADIUS, 7.0, "值块圆角");
        v(crate::visuals::BTN_RADIUS, 8.0, "按钮圆角");
        v(crate::visuals::CAPSULE_RADIUS, 10.0, "胶囊圆角");
        v(crate::visuals::GROUP_GAP, 12.0, "组间距");
        v(crate::visuals::IN_GROUP_GAP, 2.0, "组内间距");
        v(crate::visuals::MORE_GAP, 16.0, "更多按钮间距");
        v(crate::visuals::WINDOW_MARGIN, 16.0, "窗口边距");
    }

    #[test]
    fn icon_button_six_state_mapping_matches_the_design_matrix() {
        use crate::visuals::{ControlState, WidgetKind};

        let palette = crate::theme::Palette::light();
        let resolve =
            |state| crate::visuals::resolve_state(&palette, state, WidgetKind::IconButton);
        let selected = resolve(ControlState::Selected);
        let hovered = resolve(ControlState::Hovered);
        let pressed = resolve(ControlState::Pressed);
        let disabled = resolve(ControlState::Disabled);
        let focused = resolve(ControlState::Focused);

        assert_ne!(selected.bg, hovered.bg, "B1：开启态与 hover 底色必须不同");
        assert_ne!(hovered.bg, pressed.bg, "hover 与按下底色必须不同");
        assert_eq!(selected.bg, palette.accent_soft);
        assert_eq!(selected.fg, palette.accent);
        assert_eq!(disabled.bg, eframe::egui::Color32::TRANSPARENT);
        assert!(focused.focus_ring, "B2：焦点态必须带焦点环");
        assert!(!hovered.focus_ring, "hover 不应出现焦点环");
        assert!((crate::visuals::BTN_SIZE - 32.0).abs() < 1e-4);
    }

    #[test]
    fn human_size_formats_bytes_kib_and_mib() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 KiB");
        assert_eq!(human_size(3 * 1024 * 1024), "3.0 MiB");
    }
}
