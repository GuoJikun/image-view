//! 已解码纹理的小型 LRU 缓存适配器。
//!
//! 通用淘汰逻辑在 `iv-cache`；这里只把 `LoadedImage` 的像素数作为成本，
//! 并保持原有“当前图片不淘汰”的行为。

use std::path::{Path, PathBuf};

use iv_cache::BudgetCache;

use crate::document::LoadedImage;

/// 缓存张数上限（含当前图片）。
pub const MAX_ENTRIES: usize = 3;
/// 缓存纹理总像素上限：约 288 MB RGBA8，可容纳 3 张 24MP。
pub const MAX_PIXELS: usize = 72_000_000;

pub struct ImageCache {
    inner: BudgetCache<PathBuf, LoadedImage>,
}

impl Default for ImageCache {
    fn default() -> Self {
        Self {
            inner: BudgetCache::new(MAX_ENTRIES, MAX_PIXELS, LoadedImage::display_pixels),
        }
    }
}

impl ImageCache {
    /// 取出一张已缓存的图片并刷新其最近使用顺序。
    pub fn get(&mut self, path: &Path) -> Option<LoadedImage> {
        self.inner.get(&path.to_path_buf())
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.inner.contains(&path.to_path_buf())
    }

    /// 移除指定路径（例如图片已删除到回收站）。
    pub fn remove(&mut self, path: &Path) {
        self.inner.remove(&path.to_path_buf());
    }

    /// 插入或刷新条目；`current` 是正在显示的图片，淘汰时不会移除它。
    pub fn insert(&mut self, image: LoadedImage, current: &Path) {
        let key = image.path.clone();
        let current = current.to_path_buf();
        self.inner.insert(key, image, Some(&current));
    }
}
