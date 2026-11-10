//! 应用状态与每帧编排。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, Key, Modifiers};

use crate::cache::ImageCache;
use crate::document::LoadedImage;
use crate::fileops;
use crate::folder::{self, FolderNav, SortOrder};
use crate::loader::{Caps, LoadError, LoadResponse, Loader, Purpose};
use crate::settings::Settings;
use crate::ui;
use crate::viewer::{self, ViewMode, ViewState};

/// 幻灯片自动前进间隔。
const SLIDESHOW_INTERVAL: Duration = Duration::from_secs(3);

/// 状态栏短提示的停留时间。
const NOTICE_DURATION: Duration = Duration::from_secs(4);

#[derive(Clone, Copy, Debug)]
struct Slideshow {
    /// 下一张的触发时刻。
    next_at: Instant,
}

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
    cache: ImageCache,
    slideshow: Option<Slideshow>,
    /// 换图后需要再画一帧，让标题/状态栏的 Fit 百分比与画布同步。
    needs_repaint: bool,
    settings: Settings,
    /// 启动参数与上次会话只恢复一次。
    session_restored: bool,
    /// 状态栏短提示：文本与过期时刻。
    notice: Option<(String, Instant)>,
}

impl ImageViewerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, open_path: Option<PathBuf>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let settings = Settings::load(cc.storage);
        Self::with_context(cc.egui_ctx.clone(), open_path, settings)
    }

    fn with_context(ctx: egui::Context, open_path: Option<PathBuf>, settings: Settings) -> Self {
        Self {
            folder: FolderNav {
                sort: settings.sort,
                ..Default::default()
            },
            image: None,
            view: ViewState::default(),
            loader: Loader::new(ctx),
            caps: Caps::default(),
            generation: 0,
            pending: None,
            error: None,
            error_path: None,
            title_cache: String::new(),
            show_help: false,
            fullscreen: false,
            startup_path: open_path,
            cache: ImageCache::default(),
            slideshow: None,
            needs_repaint: false,
            settings,
            session_restored: false,
            notice: None,
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
            ui::Action::ToggleSlideshow => self.toggle_slideshow(),
            ui::Action::Help => self.show_help = true,
            ui::Action::RevealInExplorer => self.reveal_current(),
            ui::Action::CopyPath => self.copy_current_path(ctx),
            ui::Action::Delete => self.delete_current(),
            ui::Action::SetSort(order) => self.set_sort(order),
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
        // 显式打开文件/目录时停止幻灯片，避免自动翻页打断用户。
        self.slideshow = None;

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

    /// 无命令行参数时恢复上次浏览位置；目录或文件已不存在时静默跳过。
    fn restore_last_session(&mut self) {
        let Some(dir) = self.settings.last_dir.clone() else {
            return;
        };
        if !dir.is_dir() {
            log::debug!("上次浏览目录不存在：{}", dir.display());
            return;
        }

        let file = self.settings.last_file.clone();
        if let Some(path) = self
            .folder
            .restore(&dir, file.as_deref(), self.settings.sort)
        {
            self.request_load(path);
        }
    }

    fn set_sort(&mut self, order: SortOrder) {
        self.folder.set_sort(order);
        self.settings.sort = order;
        self.update_preloads();
    }

    fn current_path(&self) -> Option<PathBuf> {
        self.image.as_ref().map(|image| absolute_path(&image.path))
    }

    fn copy_current_path(&mut self, ctx: &egui::Context) {
        let Some(path) = self.current_path() else {
            return;
        };
        ctx.copy_text(path.display().to_string());
        self.set_notice("已复制路径");
    }

    fn reveal_current(&mut self) {
        let Some(path) = self.current_path() else {
            return;
        };
        if let Err(error) = fileops::reveal_in_file_manager(&path) {
            log::warn!("无法在文件管理器中定位 {}：{error}", path.display());
            self.error = Some(error.to_string());
            self.error_path = None;
        }
    }

    /// 把当前图片移到回收站，并自动前进到下一张（目录清空时回到空状态）。
    fn delete_current(&mut self) {
        let Some(image) = self.image.as_ref() else {
            return;
        };
        let path = image.path.clone();
        let name = image.file_name();

        if let Err(error) = fileops::delete_to_trash(&path) {
            log::warn!("删除失败 {}：{error}", path.display());
            self.error = Some(error.to_string());
            self.error_path = None;
            return;
        }

        log::info!("已移到回收站：{}", path.display());
        self.slideshow = None;
        self.cache.remove(&path);
        self.image = None;
        self.view = ViewState::default();
        self.set_notice(format!("已移到回收站：{name}"));

        match self.folder.remove_current() {
            Some(next) => self.request_load(next),
            None => {
                // 目录已清空：取消在途请求，回到空状态。
                self.generation = self.loader.cancel_pending();
                self.pending = None;
                self.error = None;
                self.error_path = None;
                self.needs_repaint = true;
            }
        }
    }

    fn set_notice(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), Instant::now() + NOTICE_DURATION));
    }

    fn tick_notice(&mut self, ctx: &egui::Context) {
        let Some((_, until)) = &self.notice else {
            return;
        };
        let now = Instant::now();
        if now >= *until {
            self.notice = None;
        } else {
            ctx.request_repaint_after(*until - now);
        }
    }

    fn request_load(&mut self, path: PathBuf) {
        if !path.is_file() {
            self.error = Some(format!("文件不存在：{}", path.display()));
            self.error_path = None;
            return;
        }

        self.error = None;
        self.error_path = None;

        // 缓存命中：直接显示；同时推进代号，让在途解码结果全部过期，
        // 防止较慢的旧请求在新图片显示后覆盖它。
        if let Some(image) = self.cache.get(&path) {
            log::debug!("cache hit {}", path.display());
            self.generation = self.loader.cancel_pending();
            self.pending = None;
            self.show_image(image);
            return;
        }

        self.pending = Some(path.clone());
        self.generation = self.loader.request(path, self.caps.clone());
    }

    /// 显示一张已解码的图片，并安排相邻图片的预加载。
    fn show_image(&mut self, image: LoadedImage) {
        let path = image.path.clone();
        self.view = ViewState::default();
        // 当前图片始终入缓存，回翻时可直接命中。
        self.cache.insert(image.clone(), &path);
        self.image = Some(image);
        self.error = None;
        self.error_path = None;
        // 视图重置后 Fit 需要下一帧才能用画布尺寸重算。
        self.needs_repaint = true;
        self.update_preloads();
    }

    /// 预加载当前图片的前后邻居；已缓存或不存在时跳过。
    fn update_preloads(&mut self) {
        let Some(index) = self.folder.index else {
            return;
        };

        let mut neighbors = Vec::with_capacity(2);
        if index > 0 {
            neighbors.push(self.folder.files[index - 1].clone());
        }
        if let Some(next) = self.folder.files.get(index + 1) {
            neighbors.push(next.clone());
        }

        for path in neighbors {
            if self.cache.contains(&path) || !path.is_file() {
                continue;
            }
            self.loader
                .preload(path, self.caps.clone(), self.generation);
        }
    }

    fn step(&mut self, delta: isize) {
        if let Some(path) = self.folder.step(delta) {
            // 手动翻页后重新计时，避免刚切到下一张就被自动翻走。
            if let Some(slideshow) = &mut self.slideshow {
                slideshow.next_at = Instant::now() + SLIDESHOW_INTERVAL;
            }
            self.request_load(path);
        }
    }

    fn toggle_slideshow(&mut self) {
        if self.slideshow.is_some() {
            self.slideshow = None;
        } else if self.image.is_some() && self.folder.can_next() {
            self.slideshow = Some(Slideshow {
                next_at: Instant::now() + SLIDESHOW_INTERVAL,
            });
        }
    }

    /// 驱动幻灯片：到点后前进一步；到最后一张自动停止（不循环）。
    fn tick_slideshow(&mut self, ctx: &egui::Context) {
        let Some(slideshow) = self.slideshow else {
            return;
        };

        // 仍在加载时稍后重试，避免把翻页请求堆积起来。
        if self.pending.is_some() {
            ctx.request_repaint_after(Duration::from_millis(250));
            return;
        }

        let now = Instant::now();
        if now < slideshow.next_at {
            ctx.request_repaint_after(slideshow.next_at - now);
            return;
        }

        if self.folder.can_next() {
            self.step(1);
            ctx.request_repaint_after(SLIDESHOW_INTERVAL);
        } else {
            self.slideshow = None;
            ctx.request_repaint();
        }
    }

    fn poll_loader(&mut self, ctx: &egui::Context) {
        while let Some(response) = self.loader.try_recv() {
            match response.purpose {
                Purpose::Open => self.handle_open_result(ctx, response),
                Purpose::Preload => self.handle_preload_result(ctx, response),
            }
        }
    }

    fn handle_open_result(&mut self, ctx: &egui::Context, response: LoadResponse) {
        if response.generation != self.generation {
            // 快速翻页/缓存命中时丢弃已过期的结果。
            return;
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
                self.show_image(LoadedImage::from_decoded(ctx, path, decoded));
            }
            Err(error) => {
                log::error!("failed to load {}: {error}", path.display());
                self.error = Some(error.to_string());
                self.error_path = Some(path);
                // 加载失败时停下，不让错误横幅和自动翻页互相打断。
                self.slideshow = None;
            }
        }
    }

    fn handle_preload_result(&mut self, ctx: &egui::Context, response: LoadResponse) {
        // 预加载结果只入缓存，不影响当前显示；直接接受而不校验代号，
        // 这样快速翻页时已经解码完成的邻居仍然可复用。
        match response.result {
            Ok(decoded) => {
                let path = response.path;
                // 忽略旧目录的结果，避免挤占当前目录的缓存。
                let wanted = self.folder.files.iter().any(|file| file == &path);
                if wanted && !self.cache.contains(&path) {
                    log::debug!(
                        "preloaded {} ({}×{})",
                        path.display(),
                        decoded.source_size[0],
                        decoded.source_size[1]
                    );
                    let image = LoadedImage::from_decoded(ctx, path.clone(), decoded);
                    let current = self.image.as_ref().map(|image| image.path.clone());
                    self.cache
                        .insert(image, current.as_deref().unwrap_or(&path));
                }
            }
            Err(LoadError::PreloadSkipped { w, h }) => {
                log::debug!("preload skipped {} ({}×{})", response.path.display(), w, h);
            }
            Err(error) => {
                log::debug!("preload failed {}: {error}", response.path.display());
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

            if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Space)) {
                self.toggle_slideshow();
            }

            if ctx.input_mut(|input| input.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::C))
            {
                self.copy_current_path(ctx);
            }

            if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Delete)) {
                self.delete_current();
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

    fn canvas(&mut self, ui: &mut egui::Ui) -> Option<ui::Action> {
        let texture = self.image.as_ref().map(|image| &image.texture);
        let image_size = self
            .image
            .as_ref()
            .map(LoadedImage::texture_size)
            .unwrap_or(egui::Vec2::ZERO);
        let has_image = self.image.is_some();
        let mut action = None;

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_gray(18)))
            .show(ui, |ui| {
                let response = viewer::canvas(ui, &mut self.view, texture, image_size);
                if !has_image {
                    return;
                }
                response.context_menu(|ui| {
                    if ui.button("复制路径  Ctrl+Shift+C").clicked() {
                        action = Some(ui::Action::CopyPath);
                        ui.close();
                    }
                    if ui.button("在资源管理器中显示").clicked() {
                        action = Some(ui::Action::RevealInExplorer);
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .button(
                            egui::RichText::new("移到回收站  Delete")
                                .color(egui::Color32::from_rgb(240, 120, 120)),
                        )
                        .clicked()
                    {
                        action = Some(ui::Action::Delete);
                        ui.close();
                    }
                });
            });

        action
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
        } else if !self.session_restored {
            self.restore_last_session();
        }
        self.session_restored = true;

        self.handle_dropped_files(ctx);
        self.handle_shortcuts(ctx);
        self.poll_loader(ctx);
        self.tick_slideshow(ctx);
        self.tick_notice(ctx);

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

        if std::mem::take(&mut self.needs_repaint) {
            ctx.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let toolbar_state = ui::ToolbarState {
            can_prev: self.folder.can_prev(),
            can_next: self.folder.can_next(),
            has_image: self.image.is_some(),
            slideshow: self.slideshow.is_some(),
            sort: self.folder.sort,
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
            slideshow: self.slideshow.is_some(),
            notice: self.notice.as_ref().map(|(text, _)| text.clone()),
        };
        ui::status_bar(ui, &status);

        if let Some(action) = self.canvas(ui) {
            self.handle_action(ui.ctx(), action);
        }
        ui::help_window(ui.ctx(), &mut self.show_help);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.settings.last_dir =
            (!self.folder.dir.as_os_str().is_empty()).then(|| absolute_path(&self.folder.dir));
        self.settings.last_file = self
            .image
            .as_ref()
            .map(|image| absolute_path(&image.path))
            .or_else(|| self.pending.as_deref().map(absolute_path));
        self.settings.sort = self.folder.sort;
        self.settings.save(storage);
    }
}

/// 转成绝对路径；失败时保留原路径（例如路径已不可访问）。
fn absolute_path(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// 解析旋转快捷键：`R` 顺时针（+1），`Shift+R` 逆时针（-1）。
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

    fn temp_dir(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "image-view-app-test-{}-{tag}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn fixture(ctx: &egui::Context, path: &Path) -> LoadedImage {
        LoadedImage::from_decoded(
            ctx,
            path.to_path_buf(),
            crate::loader::Decoded {
                size: [2, 2],
                source_size: [2, 2],
                rgba: vec![255; 2 * 2 * 4],
            },
        )
    }

    #[test]
    fn cached_image_is_shown_without_pending_decode() {
        let dir = temp_dir("cache-hit");
        let path = dir.join("a.png");
        std::fs::write(&path, b"fixture").expect("write fixture");

        let ctx = egui::Context::default();
        let mut app = ImageViewerApp::with_context(ctx.clone(), None, Settings::default());
        app.folder = FolderNav {
            dir: dir.clone(),
            files: vec![path.clone()],
            index: Some(0),
            ..Default::default()
        };
        app.cache.insert(fixture(&ctx, &path), &path);
        app.pending = Some(path.clone());

        app.request_load(path.clone());

        assert!(app.pending.is_none(), "缓存命中不应再排队解码");
        assert_eq!(
            app.image.as_ref().map(|image| image.path.as_path()),
            Some(path.as_path())
        );
        assert_eq!(app.generation, 1, "命中时应推进代号使在途结果过期");

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn slideshow_stops_at_the_last_image() {
        let dir = temp_dir("slideshow-end");
        let first = dir.join("a.png");
        let last = dir.join("b.png");
        for path in [&first, &last] {
            std::fs::write(path, b"fixture").expect("write fixture");
        }

        let ctx = egui::Context::default();
        let mut app = ImageViewerApp::with_context(ctx.clone(), None, Settings::default());
        app.folder = FolderNav {
            dir: dir.clone(),
            files: vec![first.clone(), last.clone()],
            index: Some(0),
            ..Default::default()
        };
        app.image = Some(fixture(&ctx, &first));

        app.toggle_slideshow();
        assert!(app.slideshow.is_some());

        // 到达最后一张：切换会停止，并且不能重新启动。
        app.folder.index = Some(1);
        app.toggle_slideshow();
        assert!(app.slideshow.is_none());
        app.toggle_slideshow();
        assert!(app.slideshow.is_none(), "最后一张不应启动幻灯片");

        // 即使手动置为运行中，下一次触发也会自动停止。
        app.slideshow = Some(Slideshow {
            next_at: Instant::now(),
        });
        app.tick_slideshow(&ctx);
        assert!(app.slideshow.is_none(), "到最后一张应自动停止");

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn manual_navigation_resets_slideshow_timer_and_uses_cache() {
        let dir = temp_dir("slideshow-step");
        let first = dir.join("a.png");
        let second = dir.join("b.png");
        for path in [&first, &second] {
            std::fs::write(path, b"fixture").expect("write fixture");
        }

        let ctx = egui::Context::default();
        let mut app = ImageViewerApp::with_context(ctx.clone(), None, Settings::default());
        app.folder = FolderNav {
            dir: dir.clone(),
            files: vec![first.clone(), second.clone()],
            index: Some(0),
            ..Default::default()
        };
        app.image = Some(fixture(&ctx, &first));
        app.cache.insert(fixture(&ctx, &first), &first);
        app.cache.insert(fixture(&ctx, &second), &first);

        let started = Instant::now();
        app.slideshow = Some(Slideshow { next_at: started });

        app.step(1);

        let slideshow = app.slideshow.expect("幻灯片仍在运行");
        assert!(
            slideshow.next_at >= started + SLIDESHOW_INTERVAL,
            "手动翻页应重新计时"
        );
        assert!(app.pending.is_none(), "相邻图片应命中缓存");
        assert_eq!(app.folder.index, Some(1));
        assert_eq!(
            app.image.as_ref().map(|image| image.path.as_path()),
            Some(second.as_path())
        );

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn save_records_absolute_session_and_sort() {
        use std::collections::HashMap;

        use eframe::Storage;

        #[derive(Default)]
        struct MemoryStorage(HashMap<String, String>);

        impl Storage for MemoryStorage {
            fn get_string(&self, key: &str) -> Option<String> {
                self.0.get(key).cloned()
            }

            fn set_string(&mut self, key: &str, value: String) {
                self.0.insert(key.to_owned(), value);
            }

            fn remove_string(&mut self, key: &str) {
                self.0.remove(key);
            }

            fn flush(&mut self) {}
        }

        let dir = temp_dir("save-session");
        let path = dir.join("a.png");
        std::fs::write(&path, b"fixture").expect("write fixture");

        let ctx = egui::Context::default();
        let mut app = ImageViewerApp::with_context(ctx.clone(), None, Settings::default());
        app.folder = FolderNav {
            dir: dir.clone(),
            files: vec![path.clone()],
            index: Some(0),
            ..Default::default()
        };
        app.image = Some(fixture(&ctx, &path));
        app.set_sort(SortOrder::ModifiedNewest);

        let mut storage = MemoryStorage::default();
        <ImageViewerApp as eframe::App>::save(&mut app, &mut storage);

        let saved = Settings::load(Some(&storage));
        assert_eq!(saved.last_dir, Some(absolute_path(&dir)));
        assert_eq!(saved.last_file, Some(absolute_path(&path)));
        assert_eq!(saved.sort, SortOrder::ModifiedNewest);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
