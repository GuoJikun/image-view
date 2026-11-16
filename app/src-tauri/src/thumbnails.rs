//! 缩略图资源：按“路径 + 修改时间 + 目标边长”缓存 PNG。
//!
//! 内存 LRU 保存最近用到的 512 张，磁盘缓存把解码/编码结果留到下次启动；
//! 缩略图请求来自 `iviewer://thumbnail/{id}/{size}/{revision}`，协议层只接受
//! 目录扫描时登记过的 ImageId，不能直接读取任意路径。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::UNIX_EPOCH;

use iv_cache::{FileCache, LruCache};
use uuid::Uuid;

const MAX_CACHED_THUMBNAILS: usize = 512;
/// 磁盘缓存预算；超出后按最近使用时间淘汰。
const MAX_DISK_BYTES: u64 = 256 * 1024 * 1024;
const MIN_SIZE: u32 = 32;
const MAX_SIZE: u32 = 512;
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

#[derive(Clone, Debug, PartialEq, Eq)]
struct Key {
    path: PathBuf,
    modified: u64,
    size: u32,
}

/// 缩略图内存 + 磁盘缓存。
pub struct ThumbnailService {
    cache: Mutex<LruCache<Key, Arc<[u8]>>>,
    disk: Option<FileCache>,
}

impl Default for ThumbnailService {
    fn default() -> Self {
        Self::new(None)
    }
}

impl ThumbnailService {
    /// `cache_dir` 为 `None` 时只使用内存缓存（测试或无法确定缓存目录时）。
    pub fn new(cache_dir: Option<PathBuf>) -> Self {
        let disk = cache_dir.and_then(|dir| match FileCache::new(dir, MAX_DISK_BYTES) {
            Ok(cache) => Some(cache),
            Err(error) => {
                log::warn!(
                    target: "image_view::thumbnail",
                    "无法打开缩略图磁盘缓存：{error}"
                );
                None
            }
        });
        Self {
            cache: Mutex::new(LruCache::new(MAX_CACHED_THUMBNAILS)),
            disk,
        }
    }

    /// 返回 PNG 字节；尺寸被限制在 32..=512，避免前端请求超大缩略图。
    pub fn get_or_decode(&self, path: &Path, size: u32) -> Option<Arc<[u8]>> {
        let size = size.clamp(MIN_SIZE, MAX_SIZE);
        let modified = modified_millis(path);
        let key = Key {
            path: path.to_path_buf(),
            modified,
            size,
        };

        if let Some(bytes) = lock(&self.cache).get(&key) {
            return Some(bytes);
        }

        let disk_key = disk_key(path, modified, size);
        if let Some(disk) = &self.disk
            && let Some(bytes) = disk.get(&disk_key)
        {
            if is_valid_png(&bytes, size) {
                let bytes: Arc<[u8]> = Arc::from(bytes);
                lock(&self.cache).insert(key, bytes.clone());
                return Some(bytes);
            }
            // 旧版本或写入中断留下的坏文件：删掉后重新解码。
            disk.remove(&disk_key);
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

        if let Some(disk) = &self.disk
            && let Err(error) = disk.put(&disk_key, &bytes)
        {
            log::debug!(
                target: "image_view::thumbnail",
                "无法写入缩略图缓存：{error}"
            );
        }

        let bytes: Arc<[u8]> = Arc::from(bytes);
        lock(&self.cache).insert(key, bytes.clone());
        Some(bytes)
    }

    /// 源文件删除后清理它的全部尺寸；内存与磁盘都按路径匹配。
    pub fn remove_path(&self, path: &Path) {
        lock(&self.cache).retain(|key| key.path != path);
        if let Some(disk) = &self.disk {
            disk.remove_prefix(&path_key(path));
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 修改时间用毫秒：既作为缓存键，也作为 URL revision（前端 number 可安全表示）。
fn modified_millis(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// 同一源文件的所有尺寸共享一个目录，删除时一次 `remove_dir_all` 即可。
fn path_key(path: &Path) -> String {
    let digest = Uuid::new_v5(&Uuid::NAMESPACE_OID, path.to_string_lossy().as_bytes());
    digest.simple().to_string()
}

fn disk_key(path: &Path, modified: u64, size: u32) -> String {
    let mut seed = path.to_string_lossy().into_owned().into_bytes();
    seed.extend_from_slice(&modified.to_le_bytes());
    seed.extend_from_slice(&size.to_le_bytes());
    let digest = Uuid::new_v5(&Uuid::NAMESPACE_OID, &seed);
    format!("{}/{}.png", path_key(path), digest.simple())
}

/// 磁盘内容可能是旧版本或损坏的半个文件；读取时校验 PNG 头与尺寸。
fn is_valid_png(bytes: &[u8], max_side: u32) -> bool {
    if bytes.len() < 24 || bytes[..8] != PNG_SIGNATURE[..] {
        return false;
    }
    // IHDR 必须是第一个块：长度 13、类型 "IHDR"、然后是宽高。
    if bytes[8..12] != 13u32.to_be_bytes()[..] || bytes[12..16] != b"IHDR"[..] {
        return false;
    }
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    width > 0 && height > 0 && width <= max_side && height <= max_side
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "image-view-thumbnail-{}-{tag}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::from(PNG_SIGNATURE);
        bytes.extend_from_slice(&13u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes
    }

    #[test]
    fn thumbnail_size_is_clamped() {
        // 仅验证范围常量，不依赖真实图片解码。
        assert_eq!(MIN_SIZE.clamp(MIN_SIZE, MAX_SIZE), 32);
        assert_eq!(1024.clamp(MIN_SIZE, MAX_SIZE), 512);
        assert_eq!(128.clamp(MIN_SIZE, MAX_SIZE), 128);
    }

    #[test]
    fn png_header_validation_rejects_corrupt_cache_files() {
        assert!(is_valid_png(&png_header(128, 96), 128));
        assert!(!is_valid_png(&png_header(129, 96), 128));
        assert!(!is_valid_png(&png_header(0, 96), 128));
        assert!(!is_valid_png(b"not a png at all, definitely", 128));
        assert!(!is_valid_png(&[], 128));
    }

    #[test]
    fn cache_key_separates_sizes_and_paths() {
        let photo = Path::new(r"D:\photos\a.jpg");
        let other = Path::new(r"D:\photos\b.jpg");

        assert_eq!(disk_key(photo, 1, 128), disk_key(photo, 1, 128));
        assert_ne!(disk_key(photo, 1, 128), disk_key(photo, 1, 512));
        assert_ne!(disk_key(photo, 1, 128), disk_key(photo, 2, 128));
        assert_ne!(disk_key(photo, 1, 128), disk_key(other, 1, 128));
        assert_eq!(path_key(photo).len(), 32);
    }

    #[test]
    fn disk_hit_avoids_decoding() {
        let dir = temp_dir("disk-hit");
        let service = ThumbnailService::new(Some(dir.clone()));
        let path = Path::new(r"D:\missing\photo.jpg");
        let key = disk_key(path, 0, 128);

        service
            .disk
            .as_ref()
            .expect("disk cache")
            .put(&key, &png_header(128, 96))
            .expect("put");

        let bytes = service.get_or_decode(path, 128).expect("disk hit");
        assert!(is_valid_png(&bytes, 128));

        service.remove_path(path);
        assert!(
            service
                .disk
                .as_ref()
                .expect("disk cache")
                .get(&key)
                .is_none()
        );

        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn corrupt_disk_entry_is_removed_before_decoding() {
        let dir = temp_dir("corrupt");
        let service = ThumbnailService::new(Some(dir.clone()));
        let path = Path::new(r"D:\missing\photo.jpg");
        let key = disk_key(path, 0, 128);
        let disk = service.disk.as_ref().expect("disk cache");
        disk.put(&key, b"broken png").expect("put");

        assert!(service.get_or_decode(path, 128).is_none());
        assert!(disk.get(&key).is_none(), "坏缓存应在重试前删除");

        fs::remove_dir_all(&dir).expect("cleanup");
    }
}
