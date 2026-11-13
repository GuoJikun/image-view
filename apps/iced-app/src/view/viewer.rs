//! 查看器视图：工具栏、画布区、状态栏与错误横幅。

use iced::widget::{button, canvas, center, container, row, sensor, stack, text};
use iced::{Element, Fill};

use crate::message::Message;
use crate::state::AppState;
use crate::state::library_state::format_label;
use crate::theme;
use crate::widgets::image_canvas::ImageCanvas;

pub fn toolbar(state: &AppState) -> Element<'_, Message> {
    let controls = row![
        button(text("打开").size(13)).on_press(Message::OpenDialog),
        button(text("上一张").size(13))
            .on_press_maybe(state.nav.can_prev().then_some(Message::Navigate(-1))),
        button(text("下一张").size(13))
            .on_press_maybe(state.nav.can_next().then_some(Message::Navigate(1))),
        button(text("适配").size(13)).on_press(Message::Fit),
        button(text("100%").size(13)).on_press(Message::ActualSize),
        button(text("−").size(15)).on_press(Message::ZoomBy(0.8)),
        button(text("+").size(15)).on_press(Message::ZoomBy(1.25)),
        button(text("旋转").size(13)).on_press(Message::Rotate(1)),
        button(
            text(if state.slideshow {
                "停止放映"
            } else {
                "幻灯片"
            })
            .size(13)
        )
        .on_press(Message::ToggleSlideshow),
        button(text("信息").size(13)).on_press(Message::ToggleMetadata),
        button(text("缩略图").size(13)).on_press(Message::ToggleThumbnails),
        button(text("⋯").size(15)).on_press(Message::ToggleMenu),
    ]
    .spacing(6);

    container(controls)
        .style(theme::chrome)
        .padding(8)
        .width(Fill)
        .into()
}

/// 画布 + 空状态/加载/错误叠层；`sensor` 上报逻辑尺寸供 Fit 计算。
pub fn canvas_area(state: &AppState) -> Element<'_, Message> {
    let zoom = state
        .viewer
        .effective_zoom(state.scale_factor)
        .unwrap_or(1.0);
    let model = ImageCanvas {
        image: state.viewer.image.as_ref(),
        zoom: zoom / state.scale_factor.max(0.01),
        pan: state.viewer.view.pan,
        rotation: state.viewer.view.rotation,
        background: theme::CANVAS_BACKGROUND,
    };

    let mut layers: Vec<Element<'_, Message>> = vec![canvas(model).width(Fill).height(Fill).into()];

    if state.viewer.image.is_none() {
        let hint = if state.viewer.loading() {
            "正在加载…".to_owned()
        } else if let Some(error) = &state.viewer.error {
            error.clone()
        } else {
            "打开一张图片开始浏览（Ctrl+O，或把文件拖进来）".to_owned()
        };
        layers.push(center(text(hint).size(16)).into());
    }

    let layers = stack(layers).width(Fill).height(Fill);
    let measured = sensor(layers).on_resize(Message::CanvasResized);

    container(measured)
        .style(theme::canvas)
        .width(Fill)
        .height(Fill)
        .into()
}

pub fn status_bar(state: &AppState) -> Element<'_, Message> {
    let mut label = match &state.viewer.image {
        Some(image) => {
            let source = image.source_size;
            let decoded = image.pixel_size;
            let size = if source == decoded {
                format!("{}×{}", source[0], source[1])
            } else {
                format!(
                    "{}×{} → {}×{}（已降采样）",
                    source[0], source[1], decoded[0], decoded[1]
                )
            };
            let zoom = state.zoom().unwrap_or(1.0) * 100.0;
            let position = state
                .nav
                .position()
                .map(|(current, total)| format!("{current} / {total}"))
                .unwrap_or_else(|| "—".to_owned());
            format!(
                "{} · {} · {} bit · {:.0}% · {}",
                format_label(&image.path),
                size,
                image.depth,
                zoom,
                position
            )
        }
        None => "未打开图片".to_owned(),
    };

    // 缩略图是后台按需解码的，把「仍在生成」告诉用户，避免整条空白被当成加载失败。
    if state.library.thumbs.pending() {
        label.push_str(" · 缩略图生成中…");
    }

    container(text(label).size(12))
        .style(theme::chrome)
        .padding([5, 12])
        .width(Fill)
        .into()
}

pub fn banner(state: &AppState) -> Option<Element<'_, Message>> {
    let message = state.notice.as_deref().or(state.viewer.error.as_deref())?;
    Some(
        container(
            row![
                text(message).size(12).width(Fill),
                button(text("关闭").size(12)).on_press(Message::Dismiss),
            ]
            .spacing(8)
            .align_y(iced::Center),
        )
        .style(container::danger)
        .padding([6, 12])
        .width(Fill)
        .into(),
    )
}
