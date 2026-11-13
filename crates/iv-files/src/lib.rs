//! 目录遍历与排序。
//!
//! 只处理文件系统与排序规则，不保存“当前目录/当前下标”这类用例状态；
//! 导航状态由 `iv-application` 持有。

use std::path::{Path, PathBuf};

pub use iv_domain::{SUPPORTED_EXTENSIONS, SortOrder, is_supported_image};

/// 文件名的小写形式，用于自然排序与稳定比较。
fn natural_key(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// 按指定顺序排序图片列表。
pub fn sort_files(files: &mut [PathBuf], order: SortOrder) {
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
        log::warn!(target: "image_view::files", "无法读取目录：{}", dir.display());
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
        target: "image_view::files",
        "扫描目录 {}：{} 张图片，耗时 {:?}",
        dir.display(),
        files.len(),
        started.elapsed()
    );
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};

        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "image-view-files-test-{}-{tag}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

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
}
