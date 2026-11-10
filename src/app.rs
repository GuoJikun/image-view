//! 应用状态与每帧编排。

use std::path::{Path, PathBuf};

use eframe::egui::{self, Key, Modifiers};

use crate::document::LoadedImage;
use crate::folder::{self, FolderNav};
use crate::loader::{Caps, Loader};
use crate::ui;
use crate::viewer::{self, ViewMode, ViewState};

pub struct ImageViewerApp {
    folder: FolderNav,
    image: Option<LoadedImage>,
    view: ViewState,
    loader: Loader,
    caps: Caps,
    generation: u64,
    pending: Option<PathBuf>,
    error: Option<String>,
    /// 最近一次失败、可重试的路径。
    error_path: Option<PathBuf>,
    title_cache: String,
    show_help: bool,
    fullscreen: bool,
    startup_path: Option<PathBuf>,
}

impl ImageViewerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, open_path: Option<PathBuf>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        Self {
            folder: FolderNav::default(),
            image: None,
            view: ViewState::default(),
            loader: Loader::new(cc.egui_ctx.clone()),
            caps: Caps::default(),
            generation: 0,
            pending: None,
            error: None,
            error_path: None,
            title_cache: String::new(),
            show_help: false,
            fullscreen: false,
            startup_path: open_path,
        }
    }

    fn handle_action(&mut self, ctx: &egui::Context, action: ui::Action) {
        match action {
            ui::Action::Open => self.open_dialog(),
            ui::Action::Prev => self.step(-1),
            ui::Action::Next => self.step(1),
            ui::Action::Fit => self.view.reset_fit(),
            ui::Action::ActualSize => self.set_actual_size(ctx),
            ui::Action::ZoomIn => self.zoom_by(ctx, 1.25),
            ui::Action::ZoomOut => self.zoom_by(ctx, 0.8),
            ui::Action::Help => self.show_help = true,
        }
    }

    fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("图片", folder::SUPPORTED_EXTENSIONS)
            .set_title("打开图片")
            .pick_file()
        {
            self.open_path(&path);
        }
    }

    fn open_path(&mut self, path: &Path) {
        if path.is_dir() {
            if let Some(first) = self.folder.open_dir(path) {
                self.request_load(first);
            } else {
                self.error = Some(format!("目录中没有支持的图片：{}", path.display()));
                self.error_path = None;
            }
            return;
        }

        if !path.exists() {
            self.error = Some(format!("文件不存在：{}", path.display()));
            self.error_path = None;
            return;
        }

        self.folder.set_file(path);
        self.request_load(path.to_path_buf());
    }

    fn request_load(&mut self, path: PathBuf) {
        if !path.is_file() {
            self.error = Some(format!("文件不存在：{}", path.display()));
            self.error_path = None;
            return;
        }

        self.pending = Some(path.clone());
        self.error = None;
        self.error_path = None;
        self.generation = self.loader.request(path, self.caps.clone());
    }

    fn step(&mut self, delta: isize) {
        if let Some(path) = self.folder.step(delta) {
            self.request_load(path);
        }
    }

    fn poll_loader(&mut self, ctx: &egui::Context) {
        while let Some(response) = self.loader.try_recv() {
            if response.generation != self.generation {
                // 快速翻页时丢弃旧结果。
                continue;
            }

            self.pending = None;
            let path = response.path;
            match response.result {
                Ok(decoded) => {
                    log::info!(
                        "loaded {} ({}×{})",
                        path.display(),
                        decoded.source_size[0],
                        decoded.source_size[1]
                    );
                    self.image = Some(LoadedImage::from_decoded(ctx, path, decoded));
                    self.view = ViewState::default();
                    self.error = None;
                    self.error_path = None;
                    // 标题与状态栏在画布之前计算，补一帧让 Fit 百分比立即同步。
                    ctx.request_repaint();
                }
                Err(error) => {
                    log::error!("failed to load {}: {error}", path.display());
                    self.error = Some(error.to_string());
                    self.error_path = Some(path);
                }
            }
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .find(|path| !path.as_os_str().is_empty())
        });

        if let Some(path) = dropped {
            self.open_path(&path);
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|input| input.consume_key(Modifiers::CTRL, Key::O)) {
            self.open_dialog();
        }

        if self.image.is_some() || self.pending.is_some() {
            if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowLeft)) {
                self.step(-1);
            }
            if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowRight)) {
                self.step(1);
            }

            let zoom_in = ctx.input_mut(|input| {
                input.consume_key(Modifiers::NONE, Key::Plus)
                    || input.consume_key(Modifiers::SHIFT, Key::Plus)
                    || input.consume_key(Modifiers::NONE, Key::Equals)
            });
            if zoom_in {
                self.zoom_by(ctx, 1.25);
            }

            let zoom_out = ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Minus));
            if zoom_out {
                self.zoom_by(ctx, 0.8);
            }

            if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Num0)) {
                self.view.reset_fit();
            }
            if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Num1)) {
                self.set_actual_size(ctx);
            }
            if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::F)) {
                self.toggle_actual_size(ctx);
            }

            // 先匹配更具体的 Shift+R，否则共享同一逻辑键的 R 会先把它消费掉。
            let rotate = consume_rotation(ctx);
            if rotate != 0 {
                self.view.rotate(rotate);
            }
        }

        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::F11)) {
            self.fullscreen = !self.fullscreen;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
        }

        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
            if self.fullscreen {
                self.fullscreen = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            } else {
                self.show_help = false;
            }
        }
    }

    fn zoom_by(&mut self, ctx: &egui::Context, factor: f32) {
        let Some(image) = self.image.as_ref() else {
            return;
        };
        let view_size = self.view.last_view;
        if view_size.x <= 0.0 || view_size.y <= 0.0 {
            return;
        }
        self.view.zoom_by(
            image.texture_size(),
            view_size,
            factor,
            ctx.pixels_per_point(),
        );
    }

    fn set_actual_size(&mut self, ctx: &egui::Context) {
        let Some(image) = self.image.as_ref() else {
            return;
        };
        self.view.actual_size(
            image.texture_size(),
            self.view.last_view,
            ctx.pixels_per_point(),
        );
    }

    fn toggle_actual_size(&mut self, ctx: &egui::Context) {
        let Some(image) = self.image.as_ref() else {
            return;
        };
        self.view.toggle_actual_size(
            image.texture_size(),
            self.view.last_view,
            ctx.pixels_per_point(),
        );
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let texture = self.image.as_ref().map(|image| &image.texture);
        let image_size = self
            .image
            .as_ref()
            .map(LoadedImage::texture_size)
            .unwrap_or(egui::Vec2::ZERO);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_gray(18)))
            .show(ui, |ui| {
                viewer::canvas(ui, &mut self.view, texture, image_size);
            });
    }

    fn sync_title(&mut self, ctx: &egui::Context) {
        let title = match &self.image {
            Some(image) => image.window_title(self.view.zoom * 100.0),
            None => "图片查看器".to_owned(),
        };

        if title != self.title_cache {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title_cache = title;
        }
    }
}

impl eframe::App for ImageViewerApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let max_texture_side = ctx.input(|input| input.max_texture_side);
        if max_texture_side > 0 {
            self.caps = Caps::from_texture_side(max_texture_side);
        }

        if let Some(path) = self.startup_path.take() {
            self.open_path(&path);
        }

        self.handle_dropped_files(ctx);
        self.handle_shortcuts(ctx);
        self.poll_loader(ctx);

        // 标题/工具栏在画布之前绘制；先用上一帧画布尺寸重算 Fit，
        // 避免加载或旋转后标题短暂显示上一张/上一个角度的百分比。
        if let Some(image) = &self.image
            && self.view.mode == ViewMode::Fit
            && self.view.last_view.x > 0.0
            && self.view.last_view.y > 0.0
        {
            let size = image.texture_size();
            let view = self.view.last_view;
            self.view.update_fit(view, size, ctx.pixels_per_point());
        }

        self.sync_title(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let toolbar_state = ui::ToolbarState {
            can_prev: self.folder.can_prev(),
            can_next: self.folder.can_next(),
            has_image: self.image.is_some(),
            zoom_percent: self.image.as_ref().map(|_| self.view.zoom * 100.0),
        };
        if let Some(action) = ui::toolbar(ui, &toolbar_state) {
            self.handle_action(ui.ctx(), action);
        }

        let can_retry = self.error_path.is_some();
        if ui::error_banner(ui, &mut self.error, can_retry)
            && let Some(path) = self.error_path.take()
        {
            self.request_load(path);
        }

        let status = ui::StatusState {
            file_name: self.image.as_ref().map(LoadedImage::file_name),
            dimensions: self.image.as_ref().map(LoadedImage::dimensions_text),
            zoom_percent: self.image.as_ref().map(|_| self.view.zoom * 100.0),
            position: self.image.as_ref().and_then(|_| self.folder.position()),
            downscaled: self.image.as_ref().is_some_and(|image| image.downscaled),
            loading: self.pending.is_some(),
        };
        ui::status_bar(ui, &status);

        self.canvas(ui);
        ui::help_window(ui.ctx(), &mut self.show_help);
    }
}

/// 解析旋转快捷键：`R` 顺时针（+1），`Shift+R` 逆时针（-1）。
///
/// `consume_key` 使用逻辑修饰键匹配，未按下的 Shift 不会阻止 `Modifiers::NONE`
/// 命中；因此必须先匹配更具体的 `Shift+R`，否则会被 `R` 先消费。
fn consume_rotation(ctx: &egui::Context) -> i8 {
    let counter_clockwise = ctx.input_mut(|input| input.consume_key(Modifiers::SHIFT, Key::R));
    let clockwise = ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::R));
    if clockwise {
        1
    } else if counter_clockwise {
        -1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rotation_for(key: Key, modifiers: Modifiers) -> i8 {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        };

        let mut step = 0;
        let mut output = ctx.run_pass(input, |ctx| step = consume_rotation(ctx));
        output.textures_delta.clear();
        step
    }

    #[test]
    fn rotate_shortcuts_map_to_clockwise_and_counter_clockwise() {
        assert_eq!(rotation_for(Key::R, Modifiers::NONE), 1, "R 应顺时针");
        assert_eq!(
            rotation_for(Key::R, Modifiers::SHIFT),
            -1,
            "Shift+R 应逆时针"
        );
    }

    #[test]
    fn rotate_shortcut_ignores_other_keys() {
        assert_eq!(rotation_for(Key::F, Modifiers::NONE), 0);
        assert_eq!(rotation_for(Key::F, Modifiers::SHIFT), 0);
    }
}
