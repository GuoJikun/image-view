//! 图片库侧状态：缩略图缓存与元数据面板。
//!
//! 缩略图使用固定容量的“插入顺序淘汰”缓存，避免视图层为了刷新 LRU
//! 而需要 `&mut AppState`；预览资源仍由 `iv-cache::LruCache` 管理。

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use iced::widget::image::Handle;

use crate::adapters::errors;
use crate::adapters::preview::ThumbnailService;
use crate::state::viewer_state::LoadedImage;

const THUMB_CACHE_ENTRIES: usize = 512;

/// 缩略图卡片宽度（含间距），视图层与虚拟化计算共用。
pub const THUMB_ITEM_WIDTH: f32 = 88.0;

#[derive(Debug, Default)]
pub struct ThumbnailState {
    entries: HashMap<PathBuf, Handle>,
    order: VecDeque<PathBuf>,
    failed: HashSet<PathBuf>,
    service: ThumbnailService,
    pub show: bool,
}

impl ThumbnailState {
    pub fn new() -> Self {
        Self {
            show: true,
            ..Self::default()
        }
    }

    pub fn handle(&self, path: &Path) -> Option<&Handle> {
        self.entries.get(path)
    }

    pub fn needs(&self, path: &Path) -> bool {
        !self.entries.contains_key(path)
            && !self.failed.contains(path)
            && !self.service.is_inflight(path)
    }

    pub fn request(&mut self, path: PathBuf) {
        if self.needs(&path) {
            self.service.request(path);
        }
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Some(result) = self.service.try_recv() {
            changed = true;
            match result.result {
                Ok(handle) => self.insert(result.path, handle),
                Err(error) => {
                    log::debug!("缩略图解码失败 {}：{error}", result.path.display());
                    self.failed.insert(result.path);
                }
            }
        }
        changed
    }

    pub fn pending(&self) -> bool {
        self.service.pending()
    }

    pub fn remove(&mut self, path: &Path) {
        self.entries.remove(path);
        self.failed.remove(path);
    }

    fn insert(&mut self, path: PathBuf, handle: Handle) {
        if self.entries.insert(path.clone(), handle).is_none() {
            self.order.push_back(path);
        }
        while self.order.len() > THUMB_CACHE_ENTRIES {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct MetaInfo {
    pub path: PathBuf,
    pub format: String,
    pub file_size: Option<u64>,
    pub modified: Option<SystemTime>,
    pub source_size: Option<[u32; 2]>,
    pub decoded_size: Option<[u32; 2]>,
    pub depth: Option<u8>,
    pub color: Option<String>,
}

impl MetaInfo {
    pub fn from_path(path: &Path) -> Self {
        let metadata = std::fs::metadata(path).ok();
        Self {
            path: path.to_path_buf(),
            format: format_label(path),
            file_size: metadata.as_ref().map(std::fs::Metadata::len),
            modified: metadata.and_then(|metadata| metadata.modified().ok()),
            source_size: None,
            decoded_size: None,
            depth: None,
            color: None,
        }
    }

    pub fn absorb(&mut self, image: &LoadedImage) {
        self.source_size = Some(image.source_size);
        self.decoded_size = Some(image.pixel_size);
        self.depth = Some(image.depth);
        self.color = errors::color_label(&image.color);
    }
}

#[derive(Debug, Default)]
pub struct MetadataState {
    pub show: bool,
    pub info: Option<MetaInfo>,
}

impl MetadataState {
    pub fn load(&mut self, path: &Path) {
        self.info = Some(MetaInfo::from_path(path));
    }

    pub fn absorb(&mut self, image: &LoadedImage) {
        if let Some(info) = &mut self.info
            && info.path == image.path
        {
            info.absorb(image);
        }
    }

    pub fn clear(&mut self) {
        self.info = None;
    }
}

/// 图库侧状态：缩略图、元数据面板与缩略图条的视口信息。
///
/// 视口数据由 `scrollable::on_scroll` 上报，virtual strip 只构建可见卡片，
/// 因此大目录不会在每帧生成成千上万个 widget。
#[derive(Debug)]
pub struct LibraryState {
    pub thumbs: ThumbnailState,
    pub meta: MetadataState,
    pub thumb_offset_x: f32,
    pub thumb_viewport_w: f32,
}

impl Default for LibraryState {
    fn default() -> Self {
        Self {
            thumbs: ThumbnailState::new(),
            meta: MetadataState::default(),
            thumb_offset_x: 0.0,
            thumb_viewport_w: THUMB_ITEM_WIDTH * 8.0,
        }
    }
}

impl LibraryState {
    /// 当前需要绘制的缩略图下标区间（两侧各多留几张，滚动时不易露白）。
    pub fn visible_range(&self, total: usize) -> std::ops::Range<usize> {
        if total == 0 {
            return 0..0;
        }
        let viewport = self.thumb_viewport_w.max(THUMB_ITEM_WIDTH * 2.0);
        let first = (self.thumb_offset_x / THUMB_ITEM_WIDTH).floor().max(0.0) as usize;
        let count = (viewport / THUMB_ITEM_WIDTH).ceil() as usize + 4;
        let start = first.min(total);
        let end = (start + count).min(total);
        start..end
    }
}

pub fn format_label(path: &Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_uppercase())
        .unwrap_or_else(|| "未知".to_owned())
}

pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// 相对修改时间；不引入额外时区依赖。
pub fn format_age(modified: Option<SystemTime>) -> String {
    let Some(modified) = modified else {
        return "未知".to_owned();
    };
    let elapsed = SystemTime::now()
        .duration_since(modified)
        .unwrap_or(Duration::ZERO);
    let seconds = elapsed.as_secs();
    if seconds < 60 {
        "刚刚".to_owned()
    } else if seconds < 3600 {
        format!("{} 分钟前", seconds / 60)
    } else if seconds < 86_400 {
        format!("{} 小时前", seconds / 3600)
    } else if seconds < 86_400 * 30 {
        format!("{} 天前", seconds / 86_400)
    } else if seconds < 86_400 * 365 {
        format!("{} 个月前", seconds / (86_400 * 30))
    } else {
        format!("{} 年前", seconds / (86_400 * 365))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_sizes_use_binary_units() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1024), "1.0 KiB");
        assert_eq!(format_size(5 * 1024 * 1024), "5.0 MiB");
    }

    #[test]
    fn relative_time_covers_recent_ranges() {
        let now = SystemTime::now();
        assert_eq!(format_age(Some(now - Duration::from_secs(5))), "刚刚");
        assert_eq!(format_age(Some(now - Duration::from_secs(120))), "2 分钟前");
        assert_eq!(
            format_age(Some(now - Duration::from_secs(7200))),
            "2 小时前"
        );
        assert_eq!(format_age(None), "未知");
    }

    #[test]
    fn format_label_is_uppercase() {
        assert_eq!(format_label(Path::new("a.PnG")), "PNG");
        assert_eq!(format_label(Path::new("no-extension")), "未知");
    }

    #[test]
    fn visible_range_tracks_scroll_and_clamps_to_total() {
        let mut library = LibraryState {
            thumb_viewport_w: THUMB_ITEM_WIDTH * 4.0,
            ..LibraryState::default()
        };

        assert_eq!(library.visible_range(1000), 0..8);
        library.thumb_offset_x = THUMB_ITEM_WIDTH * 50.0;
        assert_eq!(library.visible_range(1000), 50..58);

        library.thumb_offset_x = THUMB_ITEM_WIDTH * 999.0;
        let range = library.visible_range(1000);
        assert_eq!(range.start, 999);
        assert_eq!(range.end, 1000);
        assert_eq!(library.visible_range(0), 0..0);
    }
}
