//! 菜单栏与下拉面板（自绘）。
//!
//! Iced 0.14 没有系统菜单 API，因此三端都在窗口内绘制：Windows / Linux 的菜单栏
//! 位于自绘标题栏左侧，macOS 单独占窗口顶部一行。结构、勾选状态与快捷键提示
//! 来自 [`crate::menu`]，与 `main` 分支的原生菜单 / `MenuBar.vue` 保持一致。
//!
//! 下拉面板用 `stack` 叠在内容之上：菜单栏高度固定 28，条目宽度固定，因此
//! 面板的落点可以直接算出来，不需要测量文字宽度。

use crate::menu::{self, Entry, Hover, Item, Mark};
use crate::message::Message;
use crate::state::AppState;
use crate::theme::{self, Palette};
use crate::widgets::icons::{self, Icon};
use iced::widget::{column, container, mouse_area, row, rule, space, text};
use iced::{Color, Element, Fill, Padding, mouse};

/// 菜单栏高度（与自绘标题栏同高）。
pub const BAR_HEIGHT: f32 = 28.0;
/// 菜单栏左起偏移；标题栏与 macOS 顶部栏都给同样的内边距。
const BAR_LEFT: f32 = 12.0;
/// 单个菜单项的固定宽度：面板横向位置据此累加，无需测量文字。
const ITEM_WIDTH: f32 = 54.0;
const PANEL_WIDTH: f32 = 240.0;
const PANEL_PADDING: f32 = 5.0;
const ROW_HEIGHT: f32 = 26.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
/// 勾选 / 单选图标槽宽度（规范 4.5）。
const GUTTER: f32 = 22.0;
/// 子菜单指示箭头边长；路径在 24 的 viewBox 里纵向只占 12 单位，所以渲染时
/// 图形墨迹约为这里的一半（16 → 约 8px 高），定为 16 才比原来的 `›` 明显。
const CHEVRON: f32 = 16.0;
/// 工具栏高度（与 `viewer::toolbar` 一致：30px 控件 + 上下 11px 内边距）。
const TOOLBAR_HEIGHT: f32 = 52.0;
/// 工具栏排序下拉的宽度：只放两个选项，比菜单栏面板窄。
const SORT_PANEL_WIDTH: f32 = 168.0;
/// 排序按钮右缘到窗口右缘的距离：12（工具栏右内边距）+ 118（胶囊）+ 6 + 1（分隔线）+ 6。
/// 面板右对齐到这条线，就落在按钮正下方，无需测量文字宽度。
const SORT_PANEL_RIGHT: f32 = 143.0;

/// 菜单栏：`文件` `显示` `帮助`。外层决定左侧内边距（标题栏或顶部栏）。
pub fn bar(state: &AppState) -> Element<'static, Message> {
    let groups = menu::groups(state);

    let items: Vec<Element<'static, Message>> = groups
        .iter()
        .enumerate()
        .map(|(index, group)| {
            let open = state.menu.open == Some(index);
            let hovered = state.menu.bar_hover == Some(index);

            // `container` 默认是左上对齐（`Horizontal::Left` / `Vertical::Top`），
            // 必须显式声明两个方向居中，否则文字会贴在 54×28 格子的左上角。
            // cosmic-text 按字体 ascent 放基线，1.3 倍行盒里中文墨迹重心比行盒中心
            // 高约 2px（实测字形重心 y≈12.3，同栏标题文字 y≈14）。给内容区上边
            // 留 4px、下边不留，align_y(Center) 的偏移从 5.875 变成 7.875，正好把
            // 字压回 28px 标题栏的中心；背景与点击区仍是整个 54×28 格子。
            let item = container(text(group.label).size(12.5).center())
                .width(ITEM_WIDTH)
                .height(BAR_HEIGHT)
                .padding(Padding {
                    top: 4.0,
                    bottom: 0.0,
                    left: 0.0,
                    right: 0.0,
                })
                .align_x(iced::Center)
                .align_y(iced::Center)
                .style(move |t| theme::menu_bar_item(t, hovered, open));

            // 标题用 container 自绘而非 button：按钮会把悬停光标改成手型，
            // 菜单栏按原生习惯保持箭头，因此这里显式声明 `Idle`。
            mouse_area(item)
                .interaction(mouse::Interaction::Idle)
                .on_enter(Message::MenuGroupHovered(index))
                .on_exit(Message::MenuGroupLeft(index))
                .on_press(Message::MenuGroupClicked(index))
                .into()
        })
        .collect();

    container(row(items).height(BAR_HEIGHT).align_y(iced::Center))
        .style(theme::chrome)
        .height(BAR_HEIGHT)
        .into()
}

/// 下拉面板 + 点击空白关闭的遮罩；菜单未展开时返回 `None`。
///
/// 返回的是要叠在内容之上的若干层，顺序即叠放顺序：遮罩在下，面板在上。
pub fn overlay(state: &AppState) -> Option<Vec<Element<'static, Message>>> {
    let index = state.menu.open?;
    let groups = menu::groups(state);
    let group = groups.get(index)?;

    let theme = crate::view::viewer::theme_of(state);
    let palette = Palette::from(&theme);

    let left = BAR_LEFT + index as f32 * ITEM_WIDTH;
    let top = BAR_HEIGHT;

    let mut layers = vec![
        dismiss_layer(),
        positioned(
            claim(panel(&group.entries, None, state, palette)),
            left,
            top,
        ),
    ];

    if let Some(sub_index) = state.menu.sub
        && let Some(Entry::Sub { items, .. }) = group.entries.get(sub_index)
    {
        // 子菜单面板自身有内边距，落点要减去它，内容首行才与父级行对齐。
        let y = top + entries_height(&group.entries[..sub_index]);
        layers.push(positioned(
            claim(panel(items, Some(sub_index), state, palette)),
            left + PANEL_WIDTH,
            y,
        ));
    }

    Some(layers)
}

/// 工具栏排序按钮的下拉面板：锚定在按钮右缘正下方，条目与「显示 › 排序」同源。
pub fn sort_overlay(state: &AppState) -> Option<Vec<Element<'static, Message>>> {
    if !state.menu.sort_open {
        return None;
    }

    let palette = Palette::from(&crate::view::viewer::theme_of(state));
    let rows: Vec<Element<'static, Message>> = menu::sort_items(state)
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let hovered = state.menu.hover
                == Some(Hover {
                    entry: index,
                    child: None,
                });
            item_row(item, hovered, index, None, palette)
        })
        .collect();

    let panel = container(column(rows).spacing(0.0))
        .style(theme::menu_panel)
        .padding(PANEL_PADDING)
        .width(SORT_PANEL_WIDTH);

    let top = sort_panel_top(state);
    Some(vec![
        dismiss_layer_from(top),
        positioned_right(claim(panel.into()), SORT_PANEL_RIGHT, top),
    ])
}

/// 下拉面板的顶边：标题栏（或 macOS 顶部菜单行）+ 工具栏；全屏时标题栏隐藏。
fn sort_panel_top(state: &AppState) -> f32 {
    #[cfg(target_os = "macos")]
    {
        let _ = state;
        BAR_HEIGHT + TOOLBAR_HEIGHT
    }
    #[cfg(not(target_os = "macos"))]
    {
        let chrome = if state.fullscreen { 0.0 } else { BAR_HEIGHT };
        chrome + TOOLBAR_HEIGHT
    }
}

/// 面板高度之和，用于把子菜单对齐到父级行。
fn entries_height(entries: &[Entry]) -> f32 {
    entries
        .iter()
        .map(|entry| match entry {
            Entry::Separator => SEPARATOR_HEIGHT,
            Entry::Item(_) | Entry::Sub { .. } => ROW_HEIGHT,
        })
        .sum()
}

/// 把元素放到窗口的 (left, top) 处。
fn positioned(
    content: Element<'static, Message>,
    left: f32,
    top: f32,
) -> Element<'static, Message> {
    container(content)
        .width(Fill)
        .height(Fill)
        .align_x(iced::Left)
        .align_y(iced::Top)
        .padding(Padding {
            top,
            left,
            right: 0.0,
            bottom: 0.0,
        })
        .into()
}

/// 把元素右对齐放到窗口右缘内 `right` 处（从窗口底部量 `top`）。
fn positioned_right(
    content: Element<'static, Message>,
    right: f32,
    top: f32,
) -> Element<'static, Message> {
    container(content)
        .width(Fill)
        .height(Fill)
        .align_x(iced::Right)
        .align_y(iced::Top)
        .padding(Padding {
            top,
            right,
            bottom: 0.0,
            left: 0.0,
        })
        .into()
}

/// 菜单下方整块可点击区域：点一下关闭菜单（首次点击不穿透到画布）。
fn dismiss_layer() -> Element<'static, Message> {
    dismiss_layer_from(BAR_HEIGHT)
}

/// 遮罩从 `top` 开始：工具栏下拉不能让遮罩盖住按钮本身，否则再点按钮无法收起。
fn dismiss_layer_from(top: f32) -> Element<'static, Message> {
    container(
        mouse_area(space().width(Fill).height(Fill))
            .interaction(mouse::Interaction::Idle)
            .on_press(Message::MenuClose),
    )
    .width(Fill)
    .height(Fill)
    .padding(Padding {
        top,
        left: 0.0,
        right: 0.0,
        bottom: 0.0,
    })
    .into()
}

/// 让叠层「占有」自己覆盖的区域：光标固定为系统箭头，点击也不再穿透到底层控件。
///
/// `stack` 合成光标时自上而下取第一个非 `None` 的结果，而 `mouse_area` 默认不改光标
/// （`interaction: None`），不显式声明就会把底部按钮的手型、画布的抓取手透上来。
fn claim(content: Element<'static, Message>) -> Element<'static, Message> {
    mouse_area(content)
        .interaction(mouse::Interaction::Idle)
        .on_press(Message::Noop)
        .into()
}

fn panel(
    entries: &[Entry],
    parent: Option<usize>,
    state: &AppState,
    palette: Palette,
) -> Element<'static, Message> {
    let mut rows: Vec<Element<'static, Message>> = Vec::with_capacity(entries.len());

    for (index, entry) in entries.iter().enumerate() {
        let hovered = state.menu.hover
            == Some(Hover {
                entry: index,
                child: parent,
            });

        match entry {
            Entry::Separator => rows.push(separator(palette)),
            Entry::Item(item) => rows.push(item_row(item, hovered, index, parent, palette)),
            Entry::Sub { label, .. } => {
                let open = parent.is_none() && state.menu.sub == Some(index);
                rows.push(sub_row(label, open, hovered, index, palette));
            }
        }
    }
    container(column(rows).spacing(0.0))
        .style(theme::menu_panel)
        .padding(PANEL_PADDING)
        .width(PANEL_WIDTH)
        .into()
}

fn separator(palette: Palette) -> Element<'static, Message> {
    container(rule::horizontal(1.0).style(move |_t| rule::Style {
        color: palette.hairline,
        radius: 0.0.into(),
        fill_mode: rule::FillMode::Full,
        snap: true,
    }))
    .padding(Padding {
        top: 4.0,
        bottom: 4.0,
        left: 8.0 + GUTTER,
        right: 8.0,
    })
    .into()
}

fn item_row(
    item: &Item,
    hovered: bool,
    index: usize,
    parent: Option<usize>,
    palette: Palette,
) -> Element<'static, Message> {
    let mark = match item.mark {
        Mark::Check if item.checked => "✓",
        Mark::Dot if item.checked => "•",
        _ => "",
    };

    let cells = vec![
        gutter(mark, item, hovered, palette),
        text(item.label)
            .size(13)
            .color(row_text_color(item, hovered, palette))
            .width(Fill)
            .into(),
        shortcut_label(item.shortcut.as_deref(), item, hovered, palette),
    ];

    row_shell(
        cells,
        hovered,
        item.enabled,
        Message::MenuAction(item.action),
        Hover {
            entry: index,
            child: parent,
        },
    )
}

fn sub_row(
    label: &'static str,
    open: bool,
    hovered: bool,
    index: usize,
    palette: Palette,
) -> Element<'static, Message> {
    let color = if hovered { Color::WHITE } else { palette.text };
    let cells = vec![
        container(text("").size(12)).width(GUTTER).into(),
        text(label).size(13).color(color).width(Fill).into(),
        chevron(hovered, palette),
    ];

    let mut element = row_shell(
        cells,
        hovered,
        true,
        Message::MenuSubmenuToggled(index),
        Hover {
            entry: index,
            child: None,
        },
    );

    // 已展开的子菜单行保持强调色，和 macOS 的层级反馈一致。
    if open && !hovered {
        element = container(element)
            .width(Fill)
            .height(ROW_HEIGHT)
            .style(move |t| {
                let p = Palette::from(t);
                container::Style {
                    background: Some(iced::Background::Color(p.accent_soft)),
                    text_color: Some(p.accent),
                    ..Default::default()
                }
            })
            .into();
    }

    element
}

/// 子菜单指示箭头。
///
/// 早期用字形 `›`（13px）：这个字符只在 x-height 区域落下一个小三角，实测高度不足
/// 5px，看着像个点。改成 SVG 后尺寸由像素决定，也能和行内文字精确垂直居中。
fn chevron(hovered: bool, palette: Palette) -> Element<'static, Message> {
    container(icons::view(
        Icon::Chevron,
        CHEVRON,
        submenu_arrow_color(hovered, palette),
    ))
    .width(CHEVRON)
    .into()
}

/// 子菜单箭头颜色：悬停时行底变强调色，箭头必须转白才看得清。
fn submenu_arrow_color(hovered: bool, palette: Palette) -> Color {
    if hovered { Color::WHITE } else { palette.text3 }
}

fn gutter(
    mark: &'static str,
    item: &Item,
    hovered: bool,
    palette: Palette,
) -> Element<'static, Message> {
    container(
        text(mark)
            .size(12)
            .color(row_text_color(item, hovered, palette)),
    )
    .width(GUTTER)
    .into()
}

fn shortcut_label(
    shortcut: Option<&str>,
    item: &Item,
    hovered: bool,
    palette: Palette,
) -> Element<'static, Message> {
    let Some(shortcut) = shortcut else {
        return space().width(0.0).into();
    };
    let color = if !item.enabled {
        palette.text3
    } else if hovered {
        Color {
            a: 0.85,
            ..Color::WHITE
        }
    } else {
        palette.text3
    };
    text(shortcut.to_owned()).size(12).color(color).into()
}

fn row_text_color(item: &Item, hovered: bool, palette: Palette) -> Color {
    if !item.enabled {
        palette.text3
    } else if hovered {
        Color::WHITE
    } else if item.danger {
        palette.danger
    } else {
        palette.text
    }
}

/// 整行外壳：悬停高亮、点击派发；禁用的行既不高亮也不响应点击。
fn row_shell(
    cells: Vec<Element<'static, Message>>,
    hovered: bool,
    enabled: bool,
    message: Message,
    hover: Hover,
) -> Element<'static, Message> {
    let body = container(
        row(cells)
            .height(ROW_HEIGHT)
            .align_y(iced::Center)
            .width(Fill),
    )
    .width(Fill)
    .height(ROW_HEIGHT)
    .padding([0.0, 8.0])
    .style(move |t| theme::menu_row(t, hovered));

    let mut area = mouse_area(body);
    if enabled {
        area = area
            // 可点击的菜单行给人手型；禁用的行保持箭头（由面板统一声明 Idle）。
            .interaction(mouse::Interaction::Pointer)
            .on_enter(Message::MenuRowHovered(hover))
            .on_exit(Message::MenuRowLeft(hover))
            .on_press(message);
    }
    area.into()
}
