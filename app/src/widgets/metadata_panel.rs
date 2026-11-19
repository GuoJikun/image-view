//! 元数据面板：字段、文案与布局与 `main` 分支的 `MetadataPanel.vue` 逐项对齐。
//!
//! 数据来自 `iv-infra` 的文件元数据与 `iv-metadata` 的解码结果；面板只做展示，
//! 文案全部来自 [`crate::i18n`]。面板只在解码结果到齐后显示详情，未就绪时显示
//! 空状态，与 main 的 `store.preview` 语义一致。

use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, rule, scrollable, space, text};
use iced::{Color, Element, Fill, Font, Padding, Theme};

use crate::adapters::errors;
use crate::i18n::Text;
use crate::menu::Action;
use crate::message::Message;
use crate::state::library_state::{MetadataState, format_size};
use crate::theme::Palette;
use crate::widgets::icons::{self, Icon};

/// main 的 `.metadata-panel` 宽 264px。
const PANEL_WIDTH: f32 = 264.0;
/// main 的 `.panel-header` 高 34px。
const HEADER_HEIGHT: f32 = 34.0;
/// main 的 `.meta-list` 首列 68px。
const LABEL_WIDTH: f32 = 68.0;

pub fn view<'a>(meta: &'a MetadataState, t: &'static Text, theme: &Theme) -> Element<'a, Message> {
    let palette = Palette::from(theme);
    let hairline = crate::theme::hairline_rule(theme);
    let note_style = crate::theme::accent_note(theme);

    let close = button(icons::view(Icon::Close, 16.0, palette.text2))
        .on_press(Message::MenuAction(Action::ToggleMetadata))
        .width(26.0)
        .height(26.0)
        // 按钮会把内容区收紧成自身尺寸，padding 取边长差的一半，图标才不被拉伸。
        .padding((26.0 - 16.0) / 2.0)
        .style(|theme, status| crate::theme::icon_button(theme, status, false));

    let header = column![
        container(
            row![
                text(t.metadata_title).size(12).font(semibold()),
                space().width(Fill),
                close,
            ]
            .spacing(8)
            .align_y(Vertical::Center),
        )
        .padding(Padding {
            top: 0.0,
            bottom: 0.0,
            left: 14.0,
            right: 8.0,
        })
        // container 默认左上对齐：外层不声明的话 row 会贴在 34px 标题栏顶部，
        // 只有 row 自身的 align_y 是不够的。
        .align_y(Vertical::Center)
        .height(HEADER_HEIGHT),
        rule::horizontal(1.0).style(move |_| hairline),
    ]
    // column 默认 width: Shrink，里面的 rule 宽度是 Fill，会被解析成 0 而看不见。
    .width(Fill);

    let body: Element<'a, Message> = match &meta.info {
        Some(info)
            if info.source_size.is_some()
                && info.decoded_size.is_some()
                && info.color.is_some() =>
        {
            let rows = vec![
                field(t.metadata_format, info.format.clone(), palette.text3),
                field(
                    t.metadata_source_size,
                    size_label(info.source_size, t),
                    palette.text3,
                ),
                field(
                    t.metadata_display_size,
                    size_label(info.decoded_size, t),
                    palette.text3,
                ),
                field(
                    t.metadata_depth,
                    info.depth
                        .map(|depth| format!("{depth} bit"))
                        .unwrap_or_else(|| t.metadata_unknown.to_owned()),
                    palette.text3,
                ),
                field(
                    t.metadata_file_size,
                    info.file_size
                        .map(format_size)
                        .unwrap_or_else(|| t.metadata_unknown.to_owned()),
                    palette.text3,
                ),
                field(
                    t.metadata_modified,
                    t.absolute_time(info.modified),
                    palette.text3,
                ),
                field(
                    t.metadata_color,
                    info.color
                        .as_ref()
                        .map(|color| errors::metadata_color_label(color, t))
                        .unwrap_or_else(|| t.metadata_unknown.to_owned()),
                    palette.text3,
                ),
                field(
                    t.metadata_downscaled,
                    if info.downscaled {
                        t.metadata_yes
                    } else {
                        t.metadata_no
                    }
                    .to_owned(),
                    palette.text3,
                ),
            ];

            // 文件区块：文件名（14px 半粗）+ 完整路径（12px 弱色），下沿 1px 分隔线。
            let mut content = column![
                column![
                    text(file_name(&info.path)).size(14).font(semibold()),
                    text(info.path.display().to_string())
                        .size(12)
                        .color(palette.text3),
                ]
                .spacing(4),
                space().height(12),
                rule::horizontal(1.0).style(move |_| hairline),
                space().height(12),
                column(rows).spacing(8),
            ]
            // 同理：不然中间那条 rule 宽度会被解析成 0。
            .width(Fill);

            if info.downscaled {
                content = content.push(
                    container(text(t.metadata_note).size(12))
                        .style(move |_| note_style)
                        .padding(Padding {
                            top: 8.0,
                            bottom: 8.0,
                            left: 10.0,
                            right: 10.0,
                        }),
                );
            }
            content.into()
        }
        _ => text(t.metadata_empty).size(12).color(palette.text3).into(),
    };

    container(column![
        header,
        container(scrollable(container(body).padding(14).width(Fill)))
            .width(Fill)
            .height(Fill),
    ])
    .style(crate::theme::panel)
    .width(PANEL_WIDTH)
    .height(Fill)
    .into()
}

fn field<'a>(label: &'a str, value: String, label_color: Color) -> Element<'a, Message> {
    row![
        text(label).size(12).color(label_color).width(LABEL_WIDTH),
        text(value).size(12).width(Fill),
    ]
    .spacing(10)
    .into()
}

fn size_label(size: Option<[u32; 2]>, t: &Text) -> String {
    size.map(|size| format!("{} × {}", size[0], size[1]))
        .unwrap_or_else(|| t.metadata_unknown.to_owned())
}

fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn semibold() -> Font {
    Font {
        weight: iced::font::Weight::Semibold,
        ..Font::default()
    }
}
