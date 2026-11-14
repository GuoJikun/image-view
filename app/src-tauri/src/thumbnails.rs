//! 缩略图资源：按“路径 + 修改时间 + 目标边长”缓存 PNG。
//!
//! 缩略图请求来自 `iviewer://thumbnail/{id}/{size}/{revision}`，协议层只接受
//! 目录扫描时登记过的 ImageId，不能直接读取任意路径。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use iv_cache::LruCache;

const MAX_CACHED_THUMBNAILS: usize = 512;
const MIN_SIZE: u32 = 32;
const MAX_SIZE: u32 = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Key {
    path: PathBuf,
    modified: u64,
    size: u32,
}

/// 缩略图内存缓存；磁盘缓存留给后续阶段，POC 先验证协议与虚拟滚动。
pub struct ThumbnailService {
    cache: Mutex<LruCache<Key, Arc<[u8]>>>,
}

impl Default for ThumbnailService {
    fn default() -> Self {
        Self {
            cache: Mutex::new(LruCache::new(MAX_CACHED_THUMBNAILS)),
        }
    }
}

impl ThumbnailService {
    /// 返回 PNG 字节；尺寸被限制在 32..=512，避免前端请求超大缩略图。
    pub fn get_or_decode(&self, path: &Path, size: u32) -> Option<Arc<[u8]>> {
        let size = size.clamp(MIN_SIZE, MAX_SIZE);
        let modified = std::fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let key = Key {
            path: path.to_path_buf(),
            modified,
            size,
        };

        if let Some(bytes) = lock(&self.cache).get(&key) {
            return Some(bytes);
        }

        let decoded = iv_image_decode::decode_thumbnail(path, size).ok()?;
        let mut bytes = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new_with_quality(
            &mut bytes,
            image::codecs::png::CompressionType::Fast,
            image::codecs::png::FilterType::Adaptive,
        );
        use image::ImageEncoder as _;
        encoder
            .write_image(
                &decoded.rgba,
                decoded.size[0] as u32,
                decoded.size[1] as u32,
                image::ExtendedColorType::Rgba8,
            )
            .ok()?;

        let bytes: Arc<[u8]> = Arc::from(bytes);
        lock(&self.cache).insert(key, bytes.clone());
        Some(bytes)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnail_size_is_clamped() {
        // 仅验证范围常量，不依赖真实图片解码。
        assert_eq!(MIN_SIZE.clamp(MIN_SIZE, MAX_SIZE), 32);
        assert_eq!(1024.clamp(MIN_SIZE, MAX_SIZE), 512);
        assert_eq!(128.clamp(MIN_SIZE, MAX_SIZE), 128);
    }
}
