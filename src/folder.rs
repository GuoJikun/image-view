//! 目录扫描与上一个/下一个导航。

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 与解码器特性保持一致的扩展名列表。
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff", "ico", "qoi", "tga", "pnm", "hdr",
    "exr", "dds",
];

pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            SUPPORTED_EXTENSIONS
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(ext))
        })
        .unwrap_or(false)
}

/// 目录内图片的排序方式。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortOrder {
    /// 文件名自然排序（大小写不敏感）。
    #[default]
    Name,
    /// 修改时间从新到旧，时间相同时按文件名。
    ModifiedNewest,
}

/// 文件名的小写形式，用于自然排序与稳定比较。
fn natural_key(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// 按指定顺序排序图片列表。
fn sort_files(files: &mut [PathBuf], order: SortOrder) {
    match order {
        SortOrder::Name => files.sort_by(|a, b| {
            natord::compare(&natural_key(a), &natural_key(b)).then_with(|| a.cmp(b))
        }),
        SortOrder::ModifiedNewest => files.sort_by_cached_key(|path| {
            let modified = std::fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .ok();
            (std::cmp::Reverse(modified), natural_key(path))
        }),
    }
}

/// 扫描目录下所有支持的图片（不递归）。
pub fn scan(dir: &Path, order: SortOrder) -> Vec<PathBuf> {
    let started = std::time::Instant::now();
    let mut files = Vec::new();

    let Ok(entries) = std::fs::read_dir(dir) else {
        log::warn!("无法读取目录：{}", dir.display());
        return files;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        // `DirEntry::file_type` 直接复用目录枚举返回的属性，避免每条记录再查一次元数据。
        let is_file = entry
            .file_type()
            .map(|file_type| file_type.is_file() || (file_type.is_symlink() && path.is_file()))
            .unwrap_or(false);
        if is_file && is_supported_image(&path) {
            files.push(path);
        }
    }

    sort_files(&mut files, order);
    log::debug!(
        "扫描目录 {}：{} 张图片，耗时 {:?}",
        dir.display(),
        files.len(),
        started.elapsed()
    );
    files
}

/// 当前目录及其中的图片列表。
#[derive(Debug, Default)]
pub struct FolderNav {
    pub dir: PathBuf,
    pub files: Vec<PathBuf>,
    pub index: Option<usize>,
    pub sort: SortOrder,
}

impl FolderNav {
    /// 以某个文件为基准重建目录列表并定位它。
    pub fn set_file(&mut self, path: &Path) {
        let dir = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        self.dir = dir;
        self.files = scan(&self.dir, self.sort);

        let name = path.file_name().map(|name| name.to_os_string());
        self.index = name.as_ref().and_then(|name| {
            self.files
                .iter()
                .position(|candidate| candidate.file_name() == Some(name.as_os_str()))
        });

        // 通过对话框打开了一个扩展名不在扫描列表里的文件时，也允许浏览。
        if self.index.is_none() {
            self.files.push(path.to_path_buf());
            sort_files(&mut self.files, self.sort);
            self.index = self
                .files
                .iter()
                .position(|candidate| candidate.file_name() == path.file_name());
        }
    }

    /// 打开目录并返回第一张图片。
    pub fn open_dir(&mut self, dir: &Path) -> Option<PathBuf> {
        self.dir = dir.to_path_buf();
        self.files = scan(dir, self.sort);
        self.index = (!self.files.is_empty()).then_some(0);
        self.current().map(Path::to_path_buf)
    }

    /// 恢复上次会话：扫描目录并定位到保存的图片；图片不存在时退回第一张。
    pub fn restore(&mut self, dir: &Path, file: Option<&Path>, sort: SortOrder) -> Option<PathBuf> {
        self.dir = dir.to_path_buf();
        self.sort = sort;
        self.files = scan(dir, sort);

        let saved = file.and_then(|file| file.file_name()).and_then(|name| {
            self.files
                .iter()
                .position(|candidate| candidate.file_name() == Some(name))
        });
        self.index = saved.or_else(|| (!self.files.is_empty()).then_some(0));
        self.current().map(Path::to_path_buf)
    }

    /// 切换排序方式，并保持当前图片仍被选中。
    pub fn set_sort(&mut self, order: SortOrder) {
        if self.sort == order {
            return;
        }
        self.sort = order;

        let current = self
            .current()
            .and_then(|path| path.file_name())
            .map(OsString::from);
        sort_files(&mut self.files, order);
        self.index = current.and_then(|name| {
            self.files
                .iter()
                .position(|candidate| candidate.file_name() == Some(name.as_os_str()))
        });
        if self.index.is_none() && !self.files.is_empty() {
            self.index = Some(0);
        }
    }

    /// 移除当前项（例如已删除到回收站），返回新的当前图片。
    pub fn remove_current(&mut self) -> Option<PathBuf> {
        let index = self.index?;
        if index < self.files.len() {
            self.files.remove(index);
        }
        if self.files.is_empty() {
            self.index = None;
            return None;
        }
        self.index = Some(index.min(self.files.len() - 1));
        self.current().map(Path::to_path_buf)
    }

    pub fn current(&self) -> Option<&Path> {
        self.index
            .and_then(|index| self.files.get(index))
            .map(PathBuf::as_path)
    }

    /// 前进/后退一张；到达边界时返回 `None`（不循环）。
    /// 已删除/移动的文件会从列表中移除，并继续沿同一方向查找。
    pub fn step(&mut self, delta: isize) -> Option<PathBuf> {
        let mut index = self.index?;

        loop {
            let next = if delta < 0 {
                index.checked_sub(1)?
            } else {
                index + 1
            };
            let path = self.files.get(next)?.clone();

            if path.is_file() {
                self.index = Some(next);
                return Some(path);
            }

            // 失效项：移除后继续查找；向后翻时当前项的下标也会前移。
            self.files.remove(next);
            if delta < 0 {
                index -= 1;
                self.index = Some(index);
            }
            if self.files.is_empty() {
                self.index = None;
                return None;
            }
        }
    }

    /// 当前序号（从 1 开始）与总数。
    pub fn position(&self) -> Option<(usize, usize)> {
        self.index.map(|index| (index + 1, self.files.len()))
    }

    pub fn can_prev(&self) -> bool {
        self.index.is_some_and(|index| index > 0)
    }

    pub fn can_next(&self) -> bool {
        self.index.is_some_and(|index| index + 1 < self.files.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_sort_is_stable_and_case_insensitive() {
        let mut files: Vec<PathBuf> = ["img10.JPG", "img2.jpg", "img1.png", "IMG1.png"]
            .into_iter()
            .map(PathBuf::from)
            .collect();
        sort_files(&mut files, SortOrder::Name);
        let names: Vec<String> = files
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["IMG1.png", "img1.png", "img2.jpg", "img10.JPG"]);
    }

    #[test]
    fn extension_matching_is_case_insensitive() {
        assert!(is_supported_image(Path::new("a.PNG")));
        assert!(is_supported_image(Path::new("b.JpEg")));
        assert!(!is_supported_image(Path::new("c.txt")));
        assert!(!is_supported_image(Path::new("no-extension")));
    }

    fn temp_dir(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};

        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "image-view-test-{}-{tag}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn scan_filters_directories_and_non_images() {
        let dir = temp_dir("scan");
        std::fs::write(dir.join("a.png"), b"").expect("write fixture");
        std::fs::write(dir.join("b.txt"), b"").expect("write fixture");
        std::fs::create_dir(dir.join("sub")).expect("create dir");
        std::fs::write(dir.join("sub").join("c.jpg"), b"").expect("write fixture");

        let files = scan(&dir, SortOrder::Name);
        let names: Vec<String> = files
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["a.png"]);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn step_skips_deleted_files() {
        let dir = temp_dir("step-forward");
        let valid = dir.join("b.png");
        std::fs::write(&valid, b"").expect("write fixture");

        let mut nav = FolderNav {
            dir: dir.clone(),
            files: vec![dir.join("missing.png"), dir.join("gone.png"), valid.clone()],
            index: Some(0),
            ..Default::default()
        };

        assert_eq!(nav.step(1).as_deref(), Some(valid.as_path()));
        assert_eq!(nav.index, Some(1));
        assert_eq!(nav.files.len(), 2);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn step_back_removes_deleted_and_keeps_index_valid() {
        let dir = temp_dir("step-back");
        let current = dir.join("b.png");
        std::fs::write(&current, b"").expect("write fixture");

        let mut nav = FolderNav {
            dir: dir.clone(),
            files: vec![dir.join("missing.png"), current.clone()],
            index: Some(1),
            ..Default::default()
        };

        assert!(nav.step(-1).is_none());
        assert_eq!(nav.index, Some(0));
        assert_eq!(nav.files, vec![current.clone()]);
        assert_eq!(nav.current(), Some(current.as_path()));
        assert!(!nav.can_prev());

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn restore_prefers_saved_file_and_falls_back_to_first() {
        let dir = temp_dir("restore");
        for name in ["a.png", "b.png", "c.png"] {
            std::fs::write(dir.join(name), b"").expect("write fixture");
        }

        let mut nav = FolderNav::default();
        assert_eq!(
            nav.restore(&dir, Some(&dir.join("b.png")), SortOrder::Name),
            Some(dir.join("b.png"))
        );
        assert_eq!(nav.index, Some(1));

        // 保存的图片已经不存在：退回第一张，而不是报错。
        assert_eq!(
            nav.restore(&dir, Some(&dir.join("gone.png")), SortOrder::Name),
            Some(dir.join("a.png"))
        );
        assert_eq!(nav.index, Some(0));

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn set_sort_keeps_current_file_selected() {
        let dir = temp_dir("sort");
        for name in ["img10.jpg", "img2.jpg", "img1.jpg"] {
            std::fs::write(dir.join(name), b"").expect("write fixture");
        }

        let mut nav = FolderNav::default();
        nav.open_dir(&dir);
        nav.index = Some(2);
        let current = nav.current().map(Path::to_path_buf);
        assert_eq!(current, Some(dir.join("img10.jpg")));

        // 按修改时间排序后仍指向同一文件。
        nav.set_sort(SortOrder::ModifiedNewest);
        assert_eq!(nav.sort, SortOrder::ModifiedNewest);
        assert_eq!(nav.current().map(Path::to_path_buf), current);
        assert_eq!(nav.files.len(), 3);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn remove_current_advances_to_next_then_ends_empty() {
        let dir = temp_dir("remove");
        let first = dir.join("a.png");
        let second = dir.join("b.png");
        for path in [&first, &second] {
            std::fs::write(path, b"").expect("write fixture");
        }

        let mut nav = FolderNav::default();
        nav.open_dir(&dir);
        assert_eq!(nav.index, Some(0));

        // 删除第一张后前进到下一张。
        assert_eq!(nav.remove_current(), Some(second.clone()));
        assert_eq!(nav.index, Some(0));
        assert_eq!(nav.position(), Some((1, 1)));

        // 删除最后一张后回到空状态。
        assert_eq!(nav.remove_current(), None);
        assert_eq!(nav.index, None);
        assert_eq!(nav.position(), None);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
