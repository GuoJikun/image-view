//! 元数据面板：路径、格式、体积、尺寸、位深与色彩转换信息。
//!
//! 数据来自 `iv-infra` 的文件元数据与 `iv-metadata` 的解码结果；面板只做展示。

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Fill};

use crate::message::Message;
use crate::state::library_state::{MetadataState, format_age, format_size};

const PANEL_WIDTH: f32 = 260.0;

pub fn view(meta: &MetadataState) -> Element<'_, Message> {
    let Some(info) = &meta.info else {
        return panel(column![text("未打开图片").size(13)].spacing(8));
    };

    let dimensions = match (info.source_size, info.decoded_size) {
        (Some(source), Some(decoded)) if source != decoded => {
            format!(
                "{}×{}（显示 {}×{}）",
                source[0], source[1], decoded[0], decoded[1]
            )
        }
        (Some(size), _) | (_, Some(size)) => format!("{}×{}", size[0], size[1]),
        _ => "未知".to_owned(),
    };

    let mut details: Vec<Element<'_, Message>> = vec![
        field("格式", info.format.clone()),
        field("文件名", file_name(&info.path)),
        field("尺寸", dimensions),
        field(
            "文件大小",
            info.file_size
                .map(format_size)
                .unwrap_or_else(|| "未知".to_owned()),
        ),
        field("修改时间", format_age(info.modified)),
        field(
            "位深",
            info.depth
                .map(|depth| format!("{depth} bit"))
                .unwrap_or_else(|| "未知".to_owned()),
        ),
    ];

    if let Some(color) = &info.color {
        details.push(field("色彩", color.clone()));
    }

    details.push(
        row![
            button(text("定位").size(12)).on_press(Message::Reveal),
            button(text("复制路径").size(12)).on_press(Message::CopyPath),
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
