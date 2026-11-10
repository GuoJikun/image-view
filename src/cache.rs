//! 已解码纹理的小型 LRU 缓存，供翻页与预加载复用。

use std::collections::VecDeque;
use std::path::Path;

use crate::document::LoadedImage;

/// 缓存张数上限（含当前图片）。
pub const MAX_ENTRIES: usize = 3;
/// 缓存纹理总像素上限：约 288 MB RGBA8，可容纳 3 张 24MP。
pub const MAX_PIXELS: usize = 72_000_000;

/// 按“最近使用”顺序保存纹理；命中时移到队尾，淘汰从队首开始。
#[derive(Default)]
pub struct ImageCache {
    entries: VecDeque<LoadedImage>,
}

impl ImageCache {
    /// 取出一张已缓存的图片并刷新其最近使用顺序。
    pub fn get(&mut self, path: &Path) -> Option<LoadedImage> {
        let index = self.entries.iter().position(|image| image.path == path)?;
        let image = self.entries.remove(index)?;
        self.entries.push_back(image.clone());
        Some(image)
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.entries.iter().any(|image| image.path == path)
    }

    /// 移除指定路径（例如图片已删除到回收站）。
    pub fn remove(&mut self, path: &Path) {
        self.entries.retain(|image| image.path != path);
    }

    /// 插入或刷新条目；`current` 是正在显示的图片，淘汰时不会移除它。
    pub fn insert(&mut self, image: LoadedImage, current: &Path) {
        self.insert_with_limits(image, current, MAX_ENTRIES, MAX_PIXELS);
    }

    fn insert_with_limits(
        &mut self,
        image: LoadedImage,
        current: &Path,
        max_entries: usize,
        max_pixels: usize,
    ) {
        self.entries.retain(|entry| entry.path != image.path);
        self.entries.push_back(image);

        let mut pixels: usize = self.entries.iter().map(LoadedImage::display_pixels).sum();
        let mut index = 0;
        while (self.entries.len() > max_entries || pixels > max_pixels)
            && index < self.entries.len()
        {
            // 当前图片必须留在缓存里，跳过它继续淘汰更旧的条目。
            if self.entries[index].path == current {
                index += 1;
                continue;
            }
            pixels -= self.entries[index].display_pixels();
            self.entries.remove(index);
        }
    }

    #[cfg(test)]
    fn paths(&self) -> Vec<&Path> {
        self.entries
            .iter()
            .map(|image| image.path.as_path())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use eframe::egui;

    fn image(ctx: &egui::Context, name: &str, display: [usize; 2]) -> LoadedImage {
        let texture = ctx.load_texture(
            name,
            egui::ColorImage::filled([1, 1], egui::Color32::WHITE),
            egui::TextureOptions::NEAREST,
        );
        LoadedImage {
            path: PathBuf::from(name),
            texture,
            display_size: display,
            source_size: display,
            downscaled: false,
            source_depth: 8,
            color: crate::color::ColorInfo::default(),
        }
    }

    fn names(cache: &ImageCache) -> Vec<String> {
        cache
            .paths()
            .iter()
            .map(|path| path.display().to_string())
            .collect()
    }

    #[test]
    fn oldest_entry_is_evicted_after_capacity() {
        let ctx = egui::Context::default();
        let mut cache = ImageCache::default();

        for name in ["a", "b", "c"] {
            cache.insert(image(&ctx, name, [10, 10]), Path::new("c"));
        }
        cache.insert(image(&ctx, "d", [10, 10]), Path::new("d"));

        assert_eq!(names(&cache), ["b", "c", "d"]);
    }

    #[test]
    fn cache_hit_refreshes_recency() {
        let ctx = egui::Context::default();
        let mut cache = ImageCache::default();

        for name in ["a", "b", "c"] {
            cache.insert(image(&ctx, name, [10, 10]), Path::new("c"));
        }

        assert!(cache.get(Path::new("a")).is_some());
        cache.insert(image(&ctx, "d", [10, 10]), Path::new("d"));

        assert_eq!(names(&cache), ["c", "a", "d"]);
        assert!(cache.contains(Path::new("a")));
    }

    #[test]
    fn current_image_is_never_evicted() {
        let ctx = egui::Context::default();
        let mut cache = ImageCache::default();

        // a 是最旧的一条，但它是当前图片，必须保留。
        for name in ["a", "b", "c"] {
            cache.insert(image(&ctx, name, [10, 10]), Path::new("a"));
        }
        cache.insert(image(&ctx, "d", [10, 10]), Path::new("a"));

        assert_eq!(names(&cache), ["a", "c", "d"]);
    }

    #[test]
    fn pixel_budget_evicts_oldest() {
        let ctx = egui::Context::default();
        let mut cache = ImageCache::default();

        cache.insert_with_limits(image(&ctx, "a", [5, 1]), Path::new("c"), 9, 10);
        cache.insert_with_limits(image(&ctx, "b", [5, 1]), Path::new("c"), 9, 10);
        cache.insert_with_limits(image(&ctx, "c", [5, 1]), Path::new("c"), 9, 10);

        assert_eq!(names(&cache), ["b", "c"]);
    }

    #[test]
    fn missing_path_is_not_cached() {
        let mut cache = ImageCache::default();
        assert!(cache.get(Path::new("missing")).is_none());
        assert!(!cache.contains(Path::new("missing")));
    }

    #[test]
    fn remove_drops_only_the_requested_entry() {
        let ctx = egui::Context::default();
        let mut cache = ImageCache::default();
        for name in ["a", "b"] {
            cache.insert(image(&ctx, name, [10, 10]), Path::new("a"));
        }

        cache.remove(Path::new("b"));

        assert!(cache.contains(Path::new("a")));
        assert!(!cache.contains(Path::new("b")));
    }
}
