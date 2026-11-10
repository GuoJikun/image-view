//! 目录扫描与上一个/下一个导航。

use std::path::{Path, PathBuf};

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

/// 按文件名自然排序（`img2` 在 `img10` 之前），大小写不敏感。
fn sort_files(files: &mut [PathBuf]) {
    files.sort_by(|a, b| {
        let an = a
            .file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let bn = b
            .file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        natord::compare(&an, &bn).then_with(|| a.cmp(b))
    });
}

/// 扫描目录下所有支持的图片（不递归）。
pub fn scan(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    let Ok(entries) = std::fs::read_dir(dir) else {
        return files;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && is_supported_image(&path) {
            files.push(path);
        }
    }

    sort_files(&mut files);
    files
}

/// 当前目录及其中的图片列表。
#[derive(Debug, Default)]
pub struct FolderNav {
    pub dir: PathBuf,
    pub files: Vec<PathBuf>,
    pub index: Option<usize>,
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
        self.files = scan(&self.dir);

        let name = path.file_name().map(|name| name.to_os_string());
        self.index = name.as_ref().and_then(|name| {
            self.files
                .iter()
                .position(|candidate| candidate.file_name() == Some(name.as_os_str()))
        });

        // 通过对话框打开了一个扩展名不在扫描列表里的文件时，也允许浏览。
        if self.index.is_none() {
            self.files.push(path.to_path_buf());
            sort_files(&mut self.files);
            self.index = self
                .files
                .iter()
                .position(|candidate| candidate.file_name() == path.file_name());
        }
    }

    /// 打开目录并返回第一张图片。
    pub fn open_dir(&mut self, dir: &Path) -> Option<PathBuf> {
        self.dir = dir.to_path_buf();
        self.files = scan(dir);
        self.index = (!self.files.is_empty()).then_some(0);
        self.current().map(Path::to_path_buf)
    }

    pub fn current(&self) -> Option<&Path> {
        self.index
            .and_then(|index| self.files.get(index))
            .map(PathBuf::as_path)
    }

    /// 前进/后退一张；到达边界时返回 `None`（不循环）。
    pub fn step(&mut self, delta: isize) -> Option<PathBuf> {
        let index = self.index?;
        let next = if delta < 0 {
            index.checked_sub(1)?
        } else {
            index + 1
        };
        let path = self.files.get(next)?.clone();
        self.index = Some(next);
        Some(path)
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
        sort_files(&mut files);
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
}
