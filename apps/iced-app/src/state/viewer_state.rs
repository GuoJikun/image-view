//! 查看器状态：当前图片、视图变换与解码请求。
//!
//! 像素资源由解码层持有，GUI 只保存 iced `image::Handle` 和轻量元数据；
//! LRU 只缓存少量相邻图片，切换目录或淘汰时资源随句柄释放。

use std::path::PathBuf;

use emath::Vec2;
use iced::widget::image::Handle;
use iv_cache::LruCache;
use iv_image_decode::{Decoded, LoadResponse, Purpose};
use iv_image_render::{ViewMode, ViewState};
use iv_metadata::ColorInfo;

use crate::adapters::errors;
use crate::adapters::preview::PreviewService;

/// 前后各缓存一张，快速来回翻页时不重复解码。
const PREVIEW_CACHE_ENTRIES: usize = 4;

#[derive(Clone)]
pub struct LoadedImage {
    pub path: PathBuf,
    pub handle: Handle,
    pub pixel_size: [u32; 2],
    pub source_size: [u32; 2],
    pub depth: u8,
    pub color: ColorInfo,
}

impl LoadedImage {
    pub fn from_decoded(path: PathBuf, decoded: Decoded) -> Self {
        Self {
            path,
            handle: Handle::from_rgba(
                decoded.size[0] as u32,
                decoded.size[1] as u32,
                decoded.rgba,
            ),
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

struct PendingPreview {
    request: u64,
    path: PathBuf,
}

pub struct ViewerState {
    pub image: Option<LoadedImage>,
    pub view: ViewState,
    /// 画布逻辑尺寸；`sensor` 上报，键盘缩放和边界钳制使用。
    pub canvas: Vec2,
    pub error: Option<String>,
    cache: LruCache<PathBuf, LoadedImage>,
    pending: Option<PendingPreview>,
    preview: PreviewService,
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
            cache: LruCache::new(PREVIEW_CACHE_ENTRIES),
            pending: None,
            preview: PreviewService::new(),
        }
    }

    pub fn loading(&self) -> bool {
        self.pending.is_some()
    }

    pub fn current_path(&self) -> Option<&PathBuf> {
        self.image.as_ref().map(|image| &image.path)
    }

    /// 显示指定图片：命中缓存立即显示，否则发起解码并清空旧画面。
    ///
    /// 返回 `true` 表示立即得到了结果（缓存命中），`false` 表示正在解码。
    pub fn show(&mut self, path: &PathBuf) -> bool {
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
        false
    }

    /// 当前图片被删除时，连同缓存一起清理。
    pub fn forget(&mut self, path: &PathBuf) {
        self.cache.remove(path);
        if self.image.as_ref().is_some_and(|image| &image.path == path) {
            self.image = None;
            self.pending = None;
        }
    }

    pub fn clear(&mut self) {
        self.image = None;
        self.pending = None;
        self.error = None;
        self.preview.cancel();
        self.reset_view();
    }

    /// 排空解码线程；返回是否有当前图片的结果被接受。
    pub fn poll(&mut self) -> bool {
        let mut changed = false;

        while let Some(response) = self.preview.try_recv() {
            if !self.accepts(&response) {
                // 过期的打开请求或预加载结果：直接丢弃。
                continue;
            }

            self.pending = None;
            match response.result {
                Ok(decoded) => {
                    let image = LoadedImage::from_decoded(response.path.clone(), decoded);
                    self.cache.insert(response.path, image.clone());
                    self.image = Some(image);
                    self.error = None;
                }
                Err(error) => {
                    self.image = None;
                    self.error = Some(errors::load_error_text(&error));
                }
            }
            self.reset_view();
            changed = true;
        }

        changed
    }

    fn accepts(&self, response: &LoadResponse) -> bool {
        if response.purpose != Purpose::Open {
            return false;
        }
        self.pending
            .as_ref()
            .is_some_and(|pending| pending.request == response.generation && pending.path == response.path)
    }

    pub fn reset_view(&mut self) {
        self.view = ViewState::default();
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
        self.view
            .zoom_by(size, self.canvas, factor, scale_factor);
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

    #[cfg(test)]
    pub(crate) fn insert_cached(&mut self, image: LoadedImage) {
        self.cache.insert(image.path.clone(), image);
    }

    #[cfg(test)]
    pub(crate) fn pending_path(&self) -> Option<&PathBuf> {
        self.pending.as_ref().map(|pending| &pending.path)
    }
}