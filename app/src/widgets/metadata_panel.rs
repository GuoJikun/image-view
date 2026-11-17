//! 元数据面板：路径、格式、体积、尺寸、位深与色彩转换信息。
//!
//! 数据来自 `iv-infra` 的文件元数据与 `iv-metadata` 的解码结果；面板只做展示，
//! 文案全部来自 [`crate::i18n`]。

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Fill};

use crate::i18n::Text;
use crate::message::Message;
use crate::state::library_state::{MetadataState, format_size};

const PANEL_WIDTH: f32 = 260.0;

pub fn view<'a>(meta: &'a MetadataState, t: &'static Text) -> Element<'a, Message> {
    let Some(info) = &meta.info else {
        return panel(column![text(t.metadata_empty).size(13)].spacing(8));
    };

    let dimensions = match (info.source_size, info.decoded_size) {
        (Some(source), Some(decoded)) if source != decoded => {
            t.dimensions_with_display(source, decoded)
        }
        (Some(size), _) | (_, Some(size)) => format!("{}×{}", size[0], size[1]),
        _ => t.metadata_unknown.to_owned(),
    };

    let mut details: Vec<Element<'a, Message>> = vec![
        field(t.metadata_format, info.format.clone()),
        field(t.metadata_file_name, file_name(&info.path)),
        field(t.metadata_dimensions, dimensions),
        field(
            t.metadata_file_size,
            info.file_size
                .map(format_size)
                .unwrap_or_else(|| t.metadata_unknown.to_owned()),
        ),
        field(t.metadata_modified, t.relative_age(info.modified)),
        field(
            t.metadata_depth,
            info.depth
                .map(|depth| format!("{depth} bit"))
                .unwrap_or_else(|| t.metadata_unknown.to_owned()),
        ),
    ];

    if let Some(color) = &info.color
        && let Some(label) = crate::adapters::errors::color_label(color, t)
    {
        details.push(field(t.metadata_color, label));
    }

    details.push(
        row![
            button(text(t.metadata_locate).size(12)).on_press(Message::Reveal),
            button(text(t.metadata_copy_path).size(12)).on_press(Message::CopyPath),
        ]
        .spacing(8)
        .into(),
    );

    panel(column(details).spacing(8))
}

fn field<'a>(label: &'a str, value: String) -> Element<'a, Message> {
    row![
        text(label).size(12).width(64.0),
        text(value).size(12).width(Fill),
    ]
    .spacing(8)
    .into()
}

fn panel<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(scrollable(container(content).padding(12).width(Fill)))
        .style(crate::theme::panel)
        .width(PANEL_WIDTH)
        .height(Fill)
        .into()
}

fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
