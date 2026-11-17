//! 查看器视图：工具栏、画布区、状态栏与错误横幅。
//!
//! 视觉按 `design/macos-first/README.md` 4.1 / 4.2 / 4.4 / 4.6 / 4.7 实现：
//! 图标按钮、分隔线、胶囊缩放、26px 状态栏、居中空状态与 danger 横幅。

use iced::widget::{button, canvas, center, column, container, row, sensor, stack, text};
use iced::{Color, Element, Fill};

use crate::adapters::errors;
use crate::message::Message;
use crate::state::AppState;
use crate::state::library_state::format_label;
use crate::theme::{self, Palette};
use crate::widgets::icons::{self, Icon};

/// 画布/空状态上的固定文字色（不随主题，画布恒深）。
fn fixed_color(hex: u32) -> Color {
    Color::from_rgb(
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
    )
}

/// 工具条按钮统一尺寸（规范 4.1：30×30 / 圆角 6 / 图标 17）。
const TOOL_BUTTON: f32 = 30.0;
/// 规范 4.1 的图标边长。
const TOOL_ICON: f32 = 17.0;
/// 翻页箭头与菜单子项共用同一份 `CHEVRON` 几何（24 viewBox / 2.0 线宽），但箭头在
/// viewBox 里只占 12/24，按 17px 渲染后实际只有 8px 高、1.4px 描边，比相邻的文件夹
/// 图标弱一截。给到 20px 后约 10px 高、1.7px 描边，与菜单里的箭头观感一致。
const TOOL_ARROW: f32 = 20.0;
/// 播放 / 停止：三角与方块只占 viewBox 的一半，17px 下又小又虚，放大 2px。
const TOOL_MEDIA: f32 = 19.0;

fn tool_button(
    icon: Icon,
    message: Message,
    enabled: bool,
    active: bool,
    theme_ref: &iced::Theme,
) -> Element<'static, Message> {
    tool(icon, TOOL_ICON, message, enabled, active, theme_ref)
}

fn tool(
    icon: Icon,
    size: f32,
    message: Message,
    enabled: bool,
    active: bool,
    theme_ref: &iced::Theme,
) -> Element<'static, Message> {
    let palette = Palette::from(theme_ref);
    let color = if active {
        palette.accent
    } else if enabled {
        palette.icon
    } else {
        palette.text3
    };
    let content = icons::view(icon, size, color);
    // `button` 会把内容区的 min/max 都收紧成按钮尺寸，内部固定尺寸的 SVG 会被
    // `Limits::resolve` 拉到按钮大小；用 padding 把内容区收回到图标边长。
    let mut btn = button(content)
        .width(TOOL_BUTTON)
        .height(TOOL_BUTTON)
        .padding((TOOL_BUTTON - size) / 2.0)
        .style(move |t, status| theme::icon_button(t, status, active));
    if enabled {
        btn = btn.on_press(message);
    }
    btn.into()
}

/// 分隔线（规范 4.1：1×18，两侧各 12px）。
fn separator(theme_ref: &iced::Theme) -> Element<'static, Message> {
    let color = Palette::from(theme_ref).hairline;
    container(text(""))
        .width(1)
        .height(18)
        .style(move |_t: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(color)),
            ..Default::default()
        })
        .into()
}

pub fn toolbar(state: &AppState) -> Element<'_, Message> {
    let t = theme_of(state);

    // 左：打开 | 上一张 / 下一张
    let left = row![
        tool_button(Icon::Open, Message::OpenDialog, true, false, &t),
        separator(&t),
        tool(
            Icon::Prev,
            TOOL_ARROW,
            Message::Navigate(-1),
            state.nav.can_prev(),
            false,
            &t,
        ),
        tool(
            Icon::Next,
            TOOL_ARROW,
            Message::Navigate(1),
            state.nav.can_next(),
            false,
            &t,
        ),
    ]
    .spacing(6);

    // 右：缩略图 / 幻灯片 / 排序 | 胶囊缩放（元数据面板等次级操作只在菜单里，与 main 一致）
    let zoom_label = match state.zoom() {
        Some(value) => format!("{:.0}%", value * 100.0),
        None => "—".to_owned(),
    };
    let has_image = state.viewer.image.is_some();

    // 排序按钮显示当前排序值 + 下拉箭头（与 `main` 的 AppToolbar 一致），
    // 点击展开菜单栏里的排序子菜单。
    let sort_label = match state.nav.sort {
        iv_domain::SortOrder::Name => state.text().menu_sort_name,
        iv_domain::SortOrder::ModifiedNewest => state.text().menu_sort_modified,
    };
    let sort_button = button(
        row![
            text(sort_label).size(12),
            icons::view(Icon::ChevronDown, 12.0, Palette::from(&t).text3),
        ]
        .spacing(4)
        .align_y(iced::Center),
    )
    .height(TOOL_BUTTON)
    .padding([0, 8])
    .style(theme::text_button)
    .on_press(Message::SortMenuToggled);

    let right = row![
        tool_button(
            Icon::Thumbs,
            Message::ToggleThumbnails,
            true,
            state.library.thumbs.show,
            &t,
        ),
        tool(
            if state.slideshow {
                Icon::Stop
            } else {
                Icon::Play
            },
            TOOL_MEDIA,
            Message::ToggleSlideshow,
            true,
            state.slideshow,
            &t,
        ),
        sort_button,
        separator(&t),
        capsule_zoom(&zoom_label, has_image),
    ]
    .spacing(6)
    .align_y(iced::Center);

    // 全屏时自绘标题栏隐藏，文件名回到工具栏中央（规范二/4.1）。
    let center_title: Element<'_, Message> = if state.fullscreen {
        let name = state
            .current_path()
            .and_then(|path| path.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        text(name).size(13).color(Palette::from(&t).text).into()
    } else {
        iced::widget::space().width(Fill).into()
    };

    let bar = row![left, center_title, right]
        .spacing(12)
        .align_y(iced::Center);

    container(bar)
        .style(theme::chrome)
        .padding([11, 12])
        .width(Fill)
        .into()
}

/// 118×30 胶囊缩放：− / 值（点击=适配）/ +（规范 4.2）。
///
/// 三段的 `button` 都会把内容区收紧成按钮尺寸，文字若不显式居中会被画在
/// 内容区左侧；默认 padding 还会挤掉中段「值」的宽度，所以统一 `padding(0)`
/// + `center()`。
fn capsule_zoom(label: &str, has_image: bool) -> Element<'static, Message> {
    let minus = button(text("−").size(15).center())
        .width(38)
        .height(30)
        .padding(0)
        .style(|t, s| theme::capsule_segment(t, s, false))
        .on_press_maybe(has_image.then_some(Message::ZoomBy(0.8)));
    let middle = button(text(label.to_owned()).size(12).center())
        .width(42)
        .height(30)
        .padding(0)
        .style(|t, s| theme::capsule_segment(t, s, true))
        .on_press_maybe(has_image.then_some(Message::Fit));
    let plus = button(text("+").size(15).center())
        .width(38)
        .height(30)
        .padding(0)
        .style(|t, s| theme::capsule_segment(t, s, false))
        .on_press_maybe(has_image.then_some(Message::ZoomBy(1.25)));

    let track = container(row![minus, middle, plus])
        .style(|t: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(Palette::from(t).track)),
            border: iced::Border {
                color: Palette::from(t).hairline,
                width: 1.0,
                radius: 7.0.into(),
            },
            ..Default::default()
        })
        .padding(0);

    track.into()
}

/// 从 AppState 推断当前 Iced Theme（与 app::theme 规则一致）。
pub(crate) fn theme_of(state: &AppState) -> iced::Theme {
    match state.settings.theme {
        iv_domain::ThemeSetting::Light => iced::Theme::Light,
        _ => iced::Theme::Dark,
    }
}

/// 画布 + 空状态/加载/错误叠层；`sensor` 上报逻辑尺寸供 Fit 计算。
pub fn canvas_area(state: &AppState) -> Element<'_, Message> {
    let zoom = state
        .viewer
        .effective_zoom(state.scale_factor)
        .unwrap_or(1.0);
    let model = crate::widgets::image_canvas::ImageCanvas {
        image: state.viewer.image.as_ref(),
        zoom: zoom / state.scale_factor.max(0.01),
        pan: state.viewer.view.pan,
        rotation: state.viewer.view.rotation,
        background: theme::CANVAS_BACKGROUND,
    };

    let mut layers: Vec<Element<'_, Message>> = vec![canvas(model).width(Fill).height(Fill).into()];

    if state.viewer.image.is_none() && !state.viewer.loading() && state.viewer.error.is_none() {
        layers.push(empty_state(state));
    } else if state.viewer.loading() {
        layers.push(
            center(
                text(state.text().status_loading)
                    .size(13)
                    .color(fixed_color(0xF5F5F7)),
            )
            .into(),
        );
    }

    // 拖放反馈（体验清单 1）：半透明蒙层 + 强调色内描边 + 居中提示。
    if state.drag_hover {
        let accent = Palette::from(&theme_of(state)).accent;
        let inner = container(text(state.text().empty_drag).size(15).color(Color::WHITE))
            .padding(20)
            .style(move |_t: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(Palette::from(_t).control)),
                border: iced::Border {
                    color: accent,
                    width: 2.0,
                    radius: 12.0.into(),
                },
                ..Default::default()
            });
        layers.push(
            container(inner)
                .align_x(iced::Center)
                .align_y(iced::Center)
                .style(move |_t: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(Color { a: 0.35, ..accent })),
                    ..Default::default()
                })
                .into(),
        );
    }

    let layers = stack(layers).width(Fill).height(Fill);
    let measured = sensor(layers).on_resize(Message::CanvasResized);

    container(measured)
        .style(theme::canvas)
        .width(Fill)
        .height(Fill)
        .into()
}

/// 空状态（规范 4.6）：图标 → 标题 → 说明 → 主按钮，整体居中偏上。
fn empty_state(state: &AppState) -> Element<'_, Message> {
    let t = state.text();
    let hint = t.empty_hint_with(&crate::menu::accel(&["mod", "O"]));
    let content = column![
        icons::view(Icon::Open, 60.0, iced::Color::from_rgb(0.45, 0.45, 0.48)),
        text(t.empty_title).size(15).color(fixed_color(0xF5F5F7)),
        text(hint).size(12).color(fixed_color(0xA1A1A6)),
        button(text(t.empty_open).size(13))
            .style(theme::primary_button)
            .padding([6, 16])
            .height(28)
            .on_press(Message::OpenDialog),
    ]
    .spacing(12)
    .align_x(iced::Center);

    // 容器必须撑满画布区才能居中；居中后再用底部内边距把整体上移 24 px（规范 4.6）。
    container(content)
        .width(Fill)
        .height(Fill)
        .align_x(iced::Center)
        .align_y(iced::Center)
        .padding(iced::Padding::from(0).bottom(48))
        .into()
}

pub fn status_bar(state: &AppState) -> Element<'_, Message> {
    let t = theme_of(state);
    let p = Palette::from(&t);

    let left: Element<'_, Message> = match &state.viewer.image {
        Some(image) => {
            let source = image.source_size;
            let decoded = image.pixel_size;
            let size = if source == decoded {
                format!("{} × {}", source[0], source[1])
            } else {
                format!(
                    "{} × {} → {} × {}",
                    source[0], source[1], decoded[0], decoded[1]
                )
            };
            // 顺序与设计规范 4.4 一致：尺寸 · 位深 · 格式 · 色彩标记。
            let mut parts = vec![
                text(size).size(11).color(p.text2).into(),
                text("·").size(11).color(p.text3).into(),
                text(format!("{} bit", image.depth))
                    .size(11)
                    .color(p.text2)
                    .into(),
                text("·").size(11).color(p.text3).into(),
                text(format_label(&image.path, state.text().metadata_unknown))
                    .size(11)
                    .color(p.text2)
                    .into(),
            ];
            if source != decoded {
                parts.push(text("·").size(11).color(p.text3).into());
                parts.push(
                    text(state.text().status_downscaled)
                        .size(11)
                        .color(fixed_color(0xE6A032))
                        .into(),
                );
            }
            // 色彩转换标记：菜单里的「显示色彩转换标记」控制是否呈现。
            if state.settings.show_color_status
                && let Some(label) = errors::color_label(&image.color, state.text())
            {
                parts.push(text("·").size(11).color(p.text3).into());
                parts.push(text(label).size(11).color(p.text2).into());
            }
            row(parts).spacing(6).align_y(iced::Center).into()
        }
        None => text(state.text().status_no_image)
            .size(11)
            .color(p.text2)
            .into(),
    };

    let right: Element<'_, Message> = {
        let mut parts: Vec<Element<'_, Message>> = Vec::new();
        if state.slideshow {
            parts.push(
                text(state.text().status_slideshow)
                    .size(11)
                    .color(p.success)
                    .into(),
            );
        }
        if state.library.thumbs.pending() {
            parts.push(
                text(state.text().status_thumbnails)
                    .size(11)
                    .color(p.text3)
                    .into(),
            );
        }
        let position = state
            .nav
            .position()
            .map(|(current, total)| format!("{current} / {total}"))
            .unwrap_or_else(|| "—".to_owned());
        parts.push(text(position).size(11).color(p.text2).into());
        row(parts).spacing(8).align_y(iced::Center).into()
    };

    let bar = row![left, iced::widget::space().width(Fill), right]
        .spacing(12)
        .align_y(iced::Center);

    container(bar)
        .style(theme::chrome)
        .padding([5, 12])
        .width(Fill)
        .height(26)
        .into()
}

pub fn banner(state: &AppState) -> Option<Element<'_, Message>> {
    let message = state.notice.as_deref().or(state.viewer.error.as_deref())?;
    // 左侧 3px 竖条 + 文本 + 按钮；竖条由带 3px 边框的容器实现（规范 4.7）。
    Some(
        container(
            row![
                text(message).size(11.5).width(Fill),
                button(text(state.text().banner_dismiss).size(11))
                    .style(theme::text_button)
                    .on_press(Message::Dismiss),
            ]
            .spacing(8)
            .align_y(iced::Center),
        )
        .style(theme::danger_banner)
        .padding([6, 12])
        .width(Fill)
        .into(),
    )
}
