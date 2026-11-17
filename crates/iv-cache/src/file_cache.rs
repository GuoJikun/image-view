//! 磁盘文件缓存：把小的派生文件按“路径片段”写入目录，并按总字节数淘汰。
//!
//! 这里只负责通用文件读写、原子写入与预算裁剪；缓存键的含义（例如
//! “源路径 + 修改时间 + 目标尺寸”）由适配器决定。键使用 `/` 分段，
//! 每段只允许 ASCII 字母、数字、`-`、`_` 和 `.`，因此不会拼出 `..` 之类的路径。

use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::SystemTime;

const TARGET_NUMERATOR: u64 = 9;
const TARGET_DENOMINATOR: u64 = 10;

/// 目录级磁盘缓存。
#[derive(Debug)]
pub struct FileCache {
    root: PathBuf,
    max_bytes: u64,
    bytes: Mutex<u64>,
}

impl FileCache {
    /// 打开（必要时创建）缓存目录；已有内容超过预算时立即裁剪。
    pub fn new(root: PathBuf, max_bytes: u64) -> io::Result<Self> {
        fs::create_dir_all(&root)?;
        let total = directory_size(&root)?;
        let cache = Self {
            root,
            max_bytes,
            bytes: Mutex::new(total),
        };
        if total > max_bytes {
            let _ = cache.prune();
        }
        Ok(cache)
    }

    /// 读取缓存条目；命中时刷新文件的修改时间，供后续按 LRU 裁剪。
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        let path = self.path_for(key)?;
        let bytes = fs::read(&path).ok()?;
        // 只读打开在 Windows 上无法改时间，失败也不影响命中。
        if let Ok(file) = OpenOptions::new().write(true).open(&path) {
            let _ = file.set_modified(SystemTime::now());
        }
        Some(bytes)
    }

    /// 原子写入缓存条目：先写临时文件，再替换目标文件。
    pub fn put(&self, key: &str, data: &[u8]) -> io::Result<()> {
        let path = self
            .path_for(key)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid cache key"))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let previous = fs::metadata(&path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        let temp = path.with_extension(format!("tmp-{}", std::process::id()));
        fs::write(&temp, data)?;
        // Windows 的 rename 不覆盖已有文件，先删除再改名。
        let _ = fs::remove_file(&path);
        if let Err(error) = fs::rename(&temp, &path) {
            let _ = fs::remove_file(&temp);
            return Err(error);
        }

        self.adjust(previous, data.len() as u64);
        if self.total_bytes() > self.max_bytes {
            let _ = self.prune();
        }
        Ok(())
    }

    pub fn remove(&self, key: &str) {
        let Some(path) = self.path_for(key) else {
            return;
        };
        if let Ok(metadata) = fs::metadata(&path)
            && fs::remove_file(&path).is_ok()
        {
            self.subtract(metadata.len());
        }
    }

    /// 删除某个前缀下的全部条目；用于“源文件删除后清理它所有尺寸的缩略图”。
    pub fn remove_prefix(&self, prefix: &str) {
        if !is_valid_segment(prefix) {
            return;
        }
        let dir = self.root.join(prefix);
        let Ok(metadata) = fs::metadata(&dir) else {
            return;
        };
        if !metadata.is_dir() {
            return;
        }
        let size = directory_size(&dir).unwrap_or(0);
        if fs::remove_dir_all(&dir).is_ok() {
            self.subtract(size);
        }
    }

    pub fn total_bytes(&self) -> u64 {
        *lock(&self.bytes)
    }

    /// 按修改时间从旧到新删除，直到回落到预算的 90%，避免每次写入都触发裁剪。
    fn prune(&self) -> io::Result<()> {
        let mut entries = Vec::new();
        collect_files(&self.root, &mut entries)?;
        entries.sort_by_key(|entry| entry.modified);

        let target = self.max_bytes / TARGET_DENOMINATOR * TARGET_NUMERATOR;
        let mut total: u64 = entries.iter().map(|entry| entry.len).sum();
        for entry in entries {
            if total <= target {
                break;
            }
            if fs::remove_file(&entry.path).is_ok() {
                total = total.saturating_sub(entry.len);
            }
        }
        *lock(&self.bytes) = total;
        Ok(())
    }

    /// 覆盖写入时把旧文件大小换成新大小。
    fn adjust(&self, previous: u64, added: u64) {
        let mut total = lock(&self.bytes);
        *total = total.saturating_sub(previous).saturating_add(added);
    }

    fn subtract(&self, len: u64) {
        let mut total = lock(&self.bytes);
        *total = total.saturating_sub(len);
    }

    /// 把 `/` 分段的键转换为缓存目录内的路径；任一段非法时返回 `None`。
    fn path_for(&self, key: &str) -> Option<PathBuf> {
        let mut path = self.root.clone();
        let mut any = false;
        for segment in key.split('/') {
            if !is_valid_segment(segment) {
                return None;
            }
            any = true;
            path.push(segment);
        }
        any.then_some(path)
    }
}

fn is_valid_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment.len() <= 96
        && segment != "."
        && segment != ".."
        && segment.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' || byte == b'.'
        })
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct Entry {
    path: PathBuf,
    len: u64,
    modified: SystemTime,
}

fn collect_files(dir: &Path, out: &mut Vec<Entry>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_files(&entry.path(), out)?;
        } else if file_type.is_file() {
            let metadata = entry.metadata()?;
            out.push(Entry {
                path: entry.path(),
                len: metadata.len(),
                modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            });
        }
    }
    Ok(())
}

fn directory_size(dir: &Path) -> io::Result<u64> {
    let mut entries = Vec::new();
    collect_files(dir, &mut entries)?;
    Ok(entries.iter().map(|entry| entry.len).sum())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{Duration, UNIX_EPOCH};

    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "image-view-file-cache-{}-{tag}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn set_modified(path: &Path, time: SystemTime) {
        let file = OpenOptions::new()
            .write(true)
            .open(path)
            .expect("open file");
        file.set_modified(time).expect("set mtime");
    }

    #[test]
    fn put_and_get_round_trip() {
        let dir = temp_dir("round-trip");
        let cache = FileCache::new(dir.clone(), 1024).expect("cache");

        cache.put("photos/a.png", b"png").expect("put");
        assert_eq!(cache.get("photos/a.png"), Some(b"png".to_vec()));
        assert_eq!(cache.total_bytes(), 3);

        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn rejects_path_traversal_and_empty_keys() {
        let dir = temp_dir("traversal");
        let cache = FileCache::new(dir.clone(), 1024).expect("cache");

        assert!(cache.put("../evil", b"x").is_err());
        assert!(cache.put("a/../../evil", b"x").is_err());
        assert!(cache.put("", b"x").is_err());
        assert_eq!(cache.get("../evil"), None);

        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn remove_prefix_drops_every_size_of_one_source() {
        let dir = temp_dir("prefix");
        let cache = FileCache::new(dir.clone(), 1024).expect("cache");

        cache.put("source-1/128.png", b"a").expect("put");
        cache.put("source-1/512.png", b"b").expect("put");
        cache.put("source-2/128.png", b"c").expect("put");

        cache.remove_prefix("source-1");

        assert_eq!(cache.get("source-1/128.png"), None);
        assert_eq!(cache.get("source-1/512.png"), None);
        assert_eq!(cache.get("source-2/128.png"), Some(b"c".to_vec()));

        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn budget_prunes_oldest_files() {
        let dir = temp_dir("budget");
        let cache = FileCache::new(dir.clone(), 20).expect("cache");

        cache.put("old/1.png", b"0123456789").expect("put");
        set_modified(&dir.join("old/1.png"), UNIX_EPOCH + Duration::from_secs(1));
        cache.put("new/1.png", b"0123456789").expect("put");
        set_modified(&dir.join("new/1.png"), UNIX_EPOCH + Duration::from_secs(10));

        // 第三次写入后总量 30 字节，超过 20 字节预算。
        cache.put("new/2.png", b"0123456789").expect("put");

        assert_eq!(cache.get("old/1.png"), None, "最旧的条目应被淘汰");
        assert_eq!(cache.get("new/1.png"), None, "第二旧的条目也被淘汰");
        assert!(cache.get("new/2.png").is_some(), "最新条目必须保留");
        assert!(cache.total_bytes() <= 20);

        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn hit_refreshes_recency() {
        let dir = temp_dir("refresh");
        let cache = FileCache::new(dir.clone(), 20).expect("cache");

        cache.put("old/1.png", b"0123456789").expect("put");
        set_modified(&dir.join("old/1.png"), UNIX_EPOCH + Duration::from_secs(1));
        cache.put("new/1.png", b"0123456789").expect("put");
        set_modified(&dir.join("new/1.png"), UNIX_EPOCH + Duration::from_secs(10));

        // 命中 old 后把它最近使用时间刷新到 now；再写入时 new 才是最旧的。
        assert!(cache.get("old/1.png").is_some());
        cache.put("newest/1.png", b"x").expect("put");

        assert!(cache.get("old/1.png").is_some());
        assert_eq!(cache.get("new/1.png"), None);
        assert!(cache.get("newest/1.png").is_some());

        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn open_drops_entries_over_budget() {
        let dir = temp_dir("open");
        fs::write(dir.join("orphan.png"), vec![0u8; 32]).expect("write");
        set_modified(&dir.join("orphan.png"), UNIX_EPOCH);

        let cache = FileCache::new(dir.clone(), 16).expect("cache");

        assert_eq!(cache.total_bytes(), 0);
        assert!(!dir.join("orphan.png").exists());

        fs::remove_dir_all(&dir).expect("cleanup");
    }
}
