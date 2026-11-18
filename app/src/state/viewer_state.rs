//! 查看器状态：当前图片、视图变换与解码请求。
//!
//! 像素资源由解码层持有，GUI 只保存 iced `image::Handle` 和轻量元数据；
//! LRU 只缓存少量相邻图片，切换目录或淘汰时资源随句柄释放。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use emath::Vec2;
use iced::widget::image::Handle;
use iv_cache::BudgetCache;
use iv_image_decode::{Decoded, DegradeReason, LoadError, LoadResponse, Purpose};
use iv_image_render::{ViewMode, ViewState};
use iv_metadata::ColorInfo;

use crate::adapters::errors;
use crate::adapters::preview::{AnimationLoad, AnimationService, LoadedAnimation, PreviewService};

/// 缓存最近三张预览（当前 + 前后各一张），与 `main` 一致。
const PREVIEW_CACHE_ENTRIES: usize = 3;
/// 像素预算：约三张 24 MP，当前图在淘汰时受保护，与 `main` 一致。
const PREVIEW_CACHE_PIXELS: usize = 72_000_000;

/// 缓存成本取解码后的预览像素数；`BudgetCache` 以此做双重淘汰。
fn preview_pixels(image: &LoadedImage) -> usize {
    image.pixel_size[0] as usize * image.pixel_size[1] as usize
}

/// 镜像一份 RGBA 句柄：Iced 0.14 的 canvas 不支持镜像变换，只能换句柄。
/// 未翻转或非 RGBA 句柄原样返回（句柄本身是 Arc 浅克隆）。
fn mirrored_handle(handle: &Handle, flip_x: bool, flip_y: bool) -> Handle {
    if !flip_x && !flip_y {
        return handle.clone();
    }
    let Handle::Rgba {
        width,
        height,
        pixels,
        ..
    } = handle
    else {
        return handle.clone();
    };
    Handle::from_rgba(
        *width,
        *height,
        iv_image_process::flip_rgba8(pixels, *width, *height, flip_x, flip_y),
    )
}

#[derive(Clone)]
pub struct LoadedImage {
    pub path: PathBuf,
    /// 当前显示的句柄：按 `ViewState` 的镜像状态从 `source_handle` 生成。
    pub handle: Handle,
    /// 未镜像的原始句柄；翻转时据此重新生成显示句柄，不写回文件。
    pub source_handle: Handle,
    pub pixel_size: [u32; 2],
    pub source_size: [u32; 2],
    pub depth: u8,
    pub color: ColorInfo,
}

impl LoadedImage {
    pub fn from_decoded(path: PathBuf, decoded: Decoded) -> Self {
        let handle = Handle::from_rgba(
            decoded.size[0] as u32,
            decoded.size[1] as u32,
            decoded.rgba.into_rgba8(),
        );
        Self {
            path,
            handle: handle.clone(),
            source_handle: handle,
            pixel_size: [decoded.size[0] as u32, decoded.size[1] as u32],
            source_size: [decoded.source_size[0] as u32, decoded.source_size[1] as u32],
            depth: decoded.source_depth,
            color: decoded.color,
        }
    }

    pub fn size(&self) -> Vec2 {
        Vec2::new(self.pixel_size[0] as f32, self.pixel_size[1] as f32)
    }
}

/// 正在播放的动画。帧句柄存在这里，画布按 `frame_index` 取用；不保存像素缓冲。
pub struct PlayingAnimation {
    /// 当前显示的帧（已按镜像状态处理）。
    pub frames: Vec<Handle>,
    /// 未镜像的原始帧；翻转时据此重新生成显示帧。
    pub source_frames: Vec<Handle>,
    pub delays: Vec<u32>,
    /// `None` 表示无限循环。
    pub loops: Option<u32>,
    pub frame_index: usize,
    /// 已完成多少轮，用于有限循环播完停在末帧。
    pub loops_played: u32,
    pub playing: bool,
    /// 下一帧的最早切换时刻；由 `Poll` 比对，避免额外定时消息。
    pub next_tick: Instant,
}

impl PlayingAnimation {
    fn new(loaded: LoadedAnimation) -> Self {
        let delays: Vec<u32> = loaded.frames.iter().map(|frame| frame.delay_ms).collect();
        let source_frames: Vec<Handle> = loaded
            .frames
            .into_iter()
            .map(|frame| frame.handle)
            .collect();
        Self {
            frames: source_frames.clone(),
            source_frames,
            delays,
            loops: loaded.loops,
            frame_index: 0,
            loops_played: 0,
            playing: true,
            next_tick: Instant::now(),
        }
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// 当前帧的句柄；索引由播放逻辑保证有效。
    pub fn current_handle(&self) -> &Handle {
        &self.frames[self.frame_index]
    }

    /// 当前帧间隔。
    pub fn delay(&self) -> Duration {
        Duration::from_millis(self.delays[self.frame_index] as u64)
    }

    /// 推进一帧；有限循环播完停在末帧并转为暂停。返回是否真正换了帧。
    ///
    /// 与 `main` 的 `advanceFrame` 同语义：到末帧后轮数 +1，未达总轮数则回到首帧。
    pub fn advance(&mut self) -> bool {
        let next = self.frame_index + 1;
        if next < self.frames.len() {
            self.frame_index = next;
            self.next_tick = Instant::now() + self.delay();
            return true;
        }

        self.loops_played = self.loops_played.saturating_add(1);
        if self.loops.is_some_and(|total| self.loops_played >= total) {
            // 有限循环播完，停在最后一帧。
            self.playing = false;
            return false;
        }
        self.frame_index = 0;
        self.next_tick = Instant::now() + self.delay();
        true
    }

    /// 暂停 / 继续；有限循环播完后再按播放，从首帧重新开始。
    pub fn toggle(&mut self) {
        if self.playing {
            self.playing = false;
            return;
        }
        if self.loops.is_some_and(|total| self.loops_played >= total) {
            self.loops_played = 0;
            self.frame_index = 0;
        }
        self.playing = true;
        self.next_tick = Instant::now() + self.delay();
    }
}

struct PendingPreview {
    request: u64,
    path: PathBuf,
}

struct PendingAnimation {
    request: u64,
    path: PathBuf,
}

pub struct ViewerState {
    pub image: Option<LoadedImage>,
    pub view: ViewState,
    /// 画布逻辑尺寸；`sensor` 上报，键盘缩放和边界钳制使用。
    pub canvas: Vec2,
    pub error: Option<String>,
    /// 动画因超预算 / 16-bit / 解码失败降级为静态首帧时的原因。
    pub degrade: Option<DegradeReason>,
    pub animation: Option<PlayingAnimation>,
    cache: BudgetCache<PathBuf, LoadedImage>,
    pending: Option<PendingPreview>,
    preview: PreviewService,
    animations: AnimationService,
    pending_animation: Option<PendingAnimation>,
}

impl Default for ViewerState {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewerState {
    pub fn new() -> Self {
        Self {
            image: None,
            view: ViewState::default(),
            canvas: Vec2::ZERO,
            error: None,
            degrade: None,
            animation: None,
            cache: BudgetCache::new(PREVIEW_CACHE_ENTRIES, PREVIEW_CACHE_PIXELS, preview_pixels),
            pending: None,
            preview: PreviewService::new(),
            animations: AnimationService::new(),
            pending_animation: None,
        }
    }

    pub fn loading(&self) -> bool {
        self.pending.is_some()
    }

    /// 显示指定图片：命中缓存立即显示，否则发起解码并清空旧画面。
    ///
    /// 返回 `true` 表示立即得到了结果（缓存命中），`false` 表示正在解码。
    pub fn show(&mut self, path: &PathBuf) -> bool {
        // 先重置上一张的播放状态；动画结果按路径 + generation 判定新旧。
        self.reset_animation();
        let animation_request = self.request_animation(path);

        if self.image.as_ref().is_some_and(|image| &image.path == path) {
            return true;
        }

        if let Some(cached) = self.cache.get(path) {
            self.preview.cancel();
            self.pending = None;
            self.image = Some(cached);
            self.error = None;
            self.reset_view();
            return true;
        }

        let request = self.preview.request(path.clone());
        self.pending = Some(PendingPreview {
            request,
            path: path.clone(),
        });
        self.image = None;
        self.error = None;
        self.reset_view();
        let _ = animation_request;
        false
    }

    /// 排入动画解码；命中服务内缓存则立即建立播放状态。
    /// 返回本轮请求代号，`0` 表示无需解码（已有缓存）。
    fn request_animation(&mut self, path: &Path) -> u64 {
        if let Some(loaded) = self.animations.ready_for(path) {
            self.animation = Some(PlayingAnimation::new(loaded));
            self.refresh_flip_handles();
            return 0;
        }
        let request = self.animations.request(path.to_path_buf());
        self.pending_animation = Some(PendingAnimation {
            request,
            path: path.to_path_buf(),
        });
        request
    }

    /// 当前图片被删除时，连同缓存一起清理。
    pub fn forget(&mut self, path: &PathBuf) {
        self.cache.remove(path);
        self.animations.forget(path);
        if self.image.as_ref().is_some_and(|image| &image.path == path) {
            self.image = None;
            self.pending = None;
            self.reset_animation();
        }
    }

    pub fn clear(&mut self) {
        self.image = None;
        self.pending = None;
        self.error = None;
        self.preview.cancel();
        self.reset_animation();
        self.animations.clear();
        self.reset_view();
    }

    /// 清空播放状态与在途动画请求（保留服务内已完成的结果供回翻复用）。
    fn reset_animation(&mut self) {
        self.animation = None;
        self.degrade = None;
        self.pending_animation = None;
        self.animations.cancel();
    }

    /// Space / 工具栏：有动画时切播放，返回是否消费了这次操作。
    pub fn toggle_animation(&mut self) -> bool {
        if let Some(animation) = &mut self.animation {
            animation.toggle();
            true
        } else {
            false
        }
    }

    /// 排空解码线程与动画线程的结果，并按帧间隔推进动画。
    /// 返回是否有画面变化需要重绘。
    pub fn poll(&mut self, t: &crate::i18n::Text) -> bool {
        let mut changed = self.poll_animation();

        while let Some(response) = self.preview.try_recv() {
            if response.purpose == Purpose::Preload {
                // 预加载只填缓存：不改变当前画面，也不触发重绘。
                self.absorb_preload(response);
                continue;
            }

            if !self.accepts(&response) {
                // 过期的打开请求：直接丢弃。
                continue;
            }

            self.pending = None;
            match response.result {
                Ok(decoded) => {
                    let image = LoadedImage::from_decoded(response.path.clone(), decoded);
                    // 与 `main` 一致：刚打开的图片已是当前图，淘汰时跳过它。
                    self.cache
                        .insert(response.path, image.clone(), Some(&image.path));
                    self.image = Some(image);
                    self.error = None;
                }
                Err(error) => {
                    self.image = None;
                    self.error = Some(errors::load_error_text(&error, t));
                }
            }
            self.reset_view();
            changed = true;
        }

        changed
    }

    /// 排空动画线程结果并按帧间隔推进；返回画面是否变化。
    fn poll_animation(&mut self) -> bool {
        let mut changed = false;
        while let Some(result) = self.animations.try_recv() {
            // 切图前的旧结果：路径或代号不匹配即丢弃。
            let accepted = self.pending_animation.as_ref().is_some_and(|pending| {
                pending.request == result.generation && pending.path == result.path
            });
            if !accepted {
                continue;
            }
            self.pending_animation = None;
            match result.outcome {
                AnimationLoad::Ready(loaded) => {
                    // 服务保留一份供回翻复用，播放状态另持一份浅克隆。
                    self.animations.accept(loaded.clone(), result.path.clone());
                    self.animation = Some(PlayingAnimation::new(loaded));
                    self.refresh_flip_handles();
                    changed = true;
                }
                AnimationLoad::NotAnimated => {
                    // 静态图正常显示首帧，不提示。
                }
                AnimationLoad::Failed(message) => {
                    // 静态主图已单独处理；动画失败只记录，避免重复错误横幅。
                    log::debug!(
                        target: "image_view::viewer",
                        "动画解码失败，按静态首帧显示 {}：{message}",
                        result.path.display()
                    );
                }
                AnimationLoad::Degraded(reason) => {
                    self.degrade = Some(reason);
                }
            }
        }

        let due = self
            .animation
            .as_ref()
            .is_some_and(|animation| animation.playing && Instant::now() >= animation.next_tick);
        if due
            && let Some(animation) = &mut self.animation
            && animation.advance()
        {
            changed = true;
        }
        changed
    }

    /// 预加载相邻图片；已有缓存或正在显示的图片不重复请求。
    pub fn preload(&self, path: PathBuf) {
        if self.is_cached(&path) {
            return;
        }
        self.preview.preload(path);
    }

    /// 该路径是否已有可直接显示的主图缓存。
    pub fn is_cached(&self, path: &PathBuf) -> bool {
        self.cache.contains(path) || self.image.as_ref().is_some_and(|image| &image.path == path)
    }

    /// 预加载结果进 LRU，供随后的翻页命中；失败只记日志。
    fn absorb_preload(&mut self, response: LoadResponse) {
        match response.result {
            Ok(decoded) => {
                if self
                    .image
                    .as_ref()
                    .is_some_and(|image| image.path == response.path)
                {
                    return;
                }
                log::debug!(
                    target: "image_view::viewer",
                    "预加载完成：{}",
                    response.path.display()
                );
                let image = LoadedImage::from_decoded(response.path.clone(), decoded);
                // 预加载的是邻居，淘汰时保护当前显示的图片。
                let protected = self.image.as_ref().map(|current| &current.path);
                self.cache.insert(response.path, image, protected);
            }
            Err(LoadError::PreloadSkipped { w, h }) => {
                log::debug!(
                    target: "image_view::viewer",
                    "跳过预加载：源图 {w}×{h} 超过预加载上限"
                );
            }
            Err(error) => {
                log::debug!(target: "image_view::viewer", "预加载失败：{error}");
            }
        }
    }

    fn accepts(&self, response: &LoadResponse) -> bool {
        if response.purpose != Purpose::Open {
            return false;
        }
        self.pending.as_ref().is_some_and(|pending| {
            pending.request == response.generation && pending.path == response.path
        })
    }

    pub fn reset_view(&mut self) {
        self.view = ViewState::default();
        self.refresh_flip_handles();
    }

    /// 按当前镜像状态重建显示句柄；原始句柄始终保留，可反复切换。
    fn refresh_flip_handles(&mut self) {
        let (flip_x, flip_y) = (self.view.flip_x, self.view.flip_y);
        if let Some(image) = self.image.as_mut() {
            image.handle = mirrored_handle(&image.source_handle, flip_x, flip_y);
        }
        if let Some(animation) = self.animation.as_mut() {
            animation.frames = animation
                .source_frames
                .iter()
                .map(|frame| mirrored_handle(frame, flip_x, flip_y))
                .collect();
        }
    }

    /// H / `显示 › 水平翻转`：只作用于显示，与 `main` 一致不写回文件。
    pub fn flip_horizontal(&mut self) {
        self.view.flip_horizontal();
        self.refresh_flip_handles();
    }

    pub fn flip_vertical(&mut self) {
        self.view.flip_vertical();
        self.refresh_flip_handles();
    }

    pub fn image_size(&self) -> Option<Vec2> {
        self.image.as_ref().map(LoadedImage::size)
    }

    /// 当前显示比例；Fit 模式按画布尺寸实时计算。
    pub fn effective_zoom(&self, scale_factor: f32) -> Option<f32> {
        let size = self.image_size()?;
        Some(match self.view.mode {
            ViewMode::Fit => ViewState::fit_zoom(
                self.canvas,
                ViewState::displayed_size(size, self.view.rotation),
                scale_factor,
            ),
            ViewMode::Manual => self.view.zoom,
        })
    }

    pub fn zoom_at(&mut self, cursor: Vec2, factor: f32, scale_factor: f32) {
        let Some(size) = self.image_size() else {
            return;
        };
        self.view
            .zoom_at(cursor, size, self.canvas, factor, scale_factor);
    }

    pub fn zoom_by(&mut self, factor: f32, scale_factor: f32) {
        let Some(size) = self.image_size() else {
            return;
        };
        self.view.zoom_by(size, self.canvas, factor, scale_factor);
    }

    pub fn pan_by(&mut self, delta: Vec2, scale_factor: f32) {
        let Some(size) = self.image_size() else {
            return;
        };
        self.view.mode = ViewMode::Manual;
        self.view.pan += delta;
        self.view.clamp_pan(size, self.canvas, scale_factor);
    }

    pub fn fit(&mut self) {
        self.view.reset_fit();
    }

    pub fn actual_size(&mut self, scale_factor: f32) {
        let Some(size) = self.image_size() else {
            return;
        };
        self.view.actual_size(size, self.canvas, scale_factor);
    }

    pub fn toggle_actual_size(&mut self, scale_factor: f32) {
        let Some(size) = self.image_size() else {
            return;
        };
        self.view
            .toggle_actual_size(size, self.canvas, scale_factor);
    }

    pub fn rotate(&mut self, steps: i8) {
        self.view.rotate(steps);
    }
}

#[cfg(test)]
mod tests {
    use iv_image_decode::Purpose;
    use iv_metadata::{ColorInfo, Pixels};

    use super::*;

    fn decoded() -> Decoded {
        Decoded {
            size: [2, 2],
            source_size: [2, 2],
            rgba: Pixels::Rgba8(vec![0; 16]),
            source_depth: 8,
            color: ColorInfo::default(),
        }
    }

    fn anim_frame() -> crate::adapters::preview::AnimFrame {
        crate::adapters::preview::AnimFrame {
            handle: Handle::from_rgba(1, 1, vec![0; 4]),
            delay_ms: 50,
        }
    }

    fn animation(frame_count: usize, loops: Option<u32>) -> PlayingAnimation {
        PlayingAnimation::new(LoadedAnimation {
            frames: (0..frame_count).map(|_| anim_frame()).collect(),
            loops,
        })
    }

    fn rgba_bytes(handle: &Handle) -> Vec<u8> {
        match handle {
            Handle::Rgba { pixels, .. } => pixels.to_vec(),
            _ => panic!("测试句柄应为 RGBA"),
        }
    }

    /// 2×2 图像，每像素首字节依次为 1 / 2 / 3 / 4，便于核对镜像方向。
    fn marked_decoded() -> Decoded {
        let mut decoded = decoded();
        decoded.rgba = Pixels::Rgba8(vec![
            1, 0, 0, 0, 2, 0, 0, 0, //
            3, 0, 0, 0, 4, 0, 0, 0, //
        ]);
        decoded
    }

    #[test]
    fn flip_swaps_display_handle_without_touching_source() {
        let mut viewer = ViewerState::new();
        viewer.image = Some(LoadedImage::from_decoded(
            PathBuf::from("flip.png"),
            marked_decoded(),
        ));
        let source = rgba_bytes(&viewer.image.as_ref().unwrap().source_handle);

        viewer.flip_horizontal();
        let image = viewer.image.as_ref().unwrap();
        assert!(viewer.view.flip_x);
        assert_eq!(rgba_bytes(&image.source_handle), source, "原图句柄不变");
        let flipped = rgba_bytes(&image.handle);
        assert_eq!(flipped[0], 2, "左上应取原图右上像素");
        assert_eq!(flipped[8], 4);

        viewer.flip_horizontal();
        viewer.flip_vertical();
        let flipped = rgba_bytes(&viewer.image.as_ref().unwrap().handle);
        assert_eq!(flipped[0], 3, "仅垂直翻转时左上应取原图左下像素");

        viewer.flip_vertical();
        assert_eq!(
            rgba_bytes(&viewer.image.as_ref().unwrap().handle),
            source,
            "两次翻转同一轴回到原图"
        );

        viewer.flip_horizontal();
        viewer.reset_view();
        assert!(!viewer.view.flip_x && !viewer.view.flip_y);
        assert_eq!(rgba_bytes(&viewer.image.as_ref().unwrap().handle), source);
    }

    #[test]
    fn flip_applies_to_animation_frames() {
        let mut viewer = ViewerState::new();
        viewer.animation = Some(PlayingAnimation::new(LoadedAnimation {
            frames: vec![crate::adapters::preview::AnimFrame {
                handle: Handle::from_rgba(
                    2,
                    2,
                    vec![
                        1, 0, 0, 0, 2, 0, 0, 0, //
                        3, 0, 0, 0, 4, 0, 0, 0, //
                    ],
                ),
                delay_ms: 50,
            }],
            loops: None,
        }));

        viewer.flip_horizontal();
        let animation = viewer.animation.as_ref().unwrap();
        assert_eq!(rgba_bytes(&animation.frames[0])[0], 2);
        assert_eq!(rgba_bytes(&animation.source_frames[0])[0], 1);
    }

    /// 播放推进：到末帧回到首帧；有限循环播完停在末帧并暂停。
    #[test]
    fn animation_advances_and_stops_after_finite_loops() {
        let mut animation = animation(2, Some(2));
        assert!(animation.playing);
        assert_eq!(animation.frame_index, 0);

        assert!(animation.advance());
        assert_eq!(animation.frame_index, 1);
        assert!(animation.advance(), "末帧 → 首帧（第 2 轮）");
        assert_eq!(animation.frame_index, 0);
        assert_eq!(animation.loops_played, 1);
        assert!(animation.advance());
        assert_eq!(animation.frame_index, 1);
        assert!(!animation.advance(), "第 2 轮播完应停止");
        assert!(!animation.playing);
        assert_eq!(animation.frame_index, 1, "停在最后一帧");

        // 播完后再按播放：从首帧重新开始。
        animation.toggle();
        assert!(animation.playing);
        assert_eq!(animation.frame_index, 0);
        assert_eq!(animation.loops_played, 0);
    }

    /// 无限循环不会自行停止；暂停后帧号不变。
    #[test]
    fn animation_infinite_loops_and_toggle_pause() {
        let mut animation = animation(3, None);
        for _ in 0..7 {
            assert!(animation.advance());
        }
        assert!(animation.playing);
        animation.toggle();
        assert!(!animation.playing);
        animation.toggle();
        assert!(animation.playing);
    }

    /// 只带尺寸元数据的假图：验证像素预算时无需真的分配几十 MP 内存。
    fn sized_image(name: &str, w: u32, h: u32) -> LoadedImage {
        let handle = Handle::from_rgba(1, 1, vec![0; 4]);
        LoadedImage {
            path: PathBuf::from(name),
            handle: handle.clone(),
            source_handle: handle,
            pixel_size: [w, h],
            source_size: [w, h],
            depth: 8,
            color: ColorInfo::default(),
        }
    }

    /// 预加载结果只填缓存，不接管当前画面。
    #[test]
    fn preload_results_fill_cache_without_changing_view() {
        let mut viewer = ViewerState::new();
        let path = PathBuf::from("neighbor.png");

        viewer.absorb_preload(LoadResponse {
            generation: 0,
            path: path.clone(),
            purpose: Purpose::Preload,
            result: Ok(decoded()),
        });

        assert!(viewer.is_cached(&path), "预加载结果应进入主图缓存");
        assert!(viewer.image.is_none(), "预加载不应改变当前显示");
        assert!(!viewer.loading(), "预加载不应让界面进入加载态");
    }

    /// 与 `main` 一致：最多三张预览，最旧的先淘汰，当前图始终保留。
    #[test]
    fn preview_cache_limits_entries_and_keeps_current() {
        let mut viewer = ViewerState::new();
        let current = PathBuf::from("current.png");

        for name in ["a.png", "b.png", "c.png"] {
            viewer
                .cache
                .insert(PathBuf::from(name), sized_image(name, 2, 2), None);
        }
        viewer.cache.insert(
            current.clone(),
            sized_image("current.png", 2, 2),
            Some(&current),
        );

        assert_eq!(viewer.cache.len(), 3);
        assert!(!viewer.cache.contains(&PathBuf::from("a.png")));
        assert!(viewer.cache.contains(&PathBuf::from("b.png")));
        assert!(viewer.cache.contains(&current));
    }

    /// 像素预算 72 MP：即使只有两张也要淘汰，避免单张大图吃满内存。
    #[test]
    fn preview_cache_evicts_by_pixel_budget() {
        let mut viewer = ViewerState::new();
        let protected = PathBuf::from("keep.png");

        // 每张约 42 MP，两张 84 MP 超过 72 MP 预算。
        viewer.cache.insert(
            PathBuf::from("first.png"),
            sized_image("first.png", 6500, 6500),
            None,
        );
        viewer.cache.insert(
            protected.clone(),
            sized_image("keep.png", 6500, 6500),
            Some(&protected),
        );

        assert!(!viewer.cache.contains(&PathBuf::from("first.png")));
        assert!(viewer.cache.contains(&protected), "当前图不能被预算淘汰");
    }

    /// 超过预加载上限的大图被 Core 放弃，缓存保持为空且不报错。
    #[test]
    fn skipped_preload_leaves_cache_empty() {
        let mut viewer = ViewerState::new();
        let path = PathBuf::from("huge.png");

        viewer.absorb_preload(LoadResponse {
            generation: 0,
            path: path.clone(),
            purpose: Purpose::Preload,
            result: Err(LoadError::PreloadSkipped {
                w: 60_000,
                h: 40_000,
            }),
        });

        assert!(!viewer.is_cached(&path));
        assert!(viewer.image.is_none());
        assert!(viewer.error.is_none(), "跳过预加载不是用户可见错误");
    }
}
