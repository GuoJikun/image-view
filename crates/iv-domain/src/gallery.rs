//! 图库领域类型：条目、筛选、分页、索引进度与对比布局。
//!
//! 与 `iv-domain` 的其它类型一样，这里只保存纯值对象，不访问文件系统或数据库。
//! 路径用 `PathBuf` 仅作为值保存，实际读写由 `iv-library` 与壳层负责。

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// 图库条目的排序方式。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GallerySortKey {
    /// 文件名自然序（默认）。
    #[default]
    Name,
    /// 修改时间从新到旧。
    Modified,
    /// 文件体积从大到小。
    Size,
    /// 按格式分组，组内按文件名。
    Format,
}

/// 图库筛选条件。空字符串 / 0 / `None` 表示不限制该维度。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GalleryFilter {
    pub query: String,
    pub tags: Vec<String>,
    pub format: String,
    pub min_width: usize,
    pub max_width: usize,
    pub from_date: Option<i64>,
    pub to_date: Option<i64>,
    pub sort: GallerySortKey,
}

impl Default for GalleryFilter {
    fn default() -> Self {
        Self {
            query: String::new(),
            tags: Vec::new(),
            format: String::new(),
            min_width: 0,
            max_width: usize::MAX,
            from_date: None,
            to_date: None,
            sort: GallerySortKey::default(),
        }
    }
}

/// 图库中的一张图片（索引条目）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GalleryEntry {
    /// UUID v4，作为资源协议授权句柄。
    pub id: String,
    pub path: PathBuf,
    pub name: String,
    /// 文件修改时间（毫秒），用于缩略图 URL 的 revision。
    pub revision: u64,
    /// 像素宽；0 表示尺寸探测失败。
    pub width: usize,
    /// 像素高；0 表示尺寸探测失败。
    pub height: usize,
    /// 扩展名小写形式。
    pub format: String,
    pub modified_at: Option<i64>,
    pub size_bytes: u64,
    pub tags: Vec<String>,
}

/// 分页查询结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GalleryPage {
    pub entries: Vec<GalleryEntry>,
    pub total: usize,
}

/// 索引进度。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GalleryProgress {
    pub indexed: usize,
    pub total: usize,
    pub ready: bool,
    /// 索引失败或目录不可读：降级为普通列表。
    pub degraded: bool,
}

/// 对比视图布局。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompareMode {
    #[default]
    SideBySide,
    Overlay,
}

/// 图库索引错误。壳层映射成 `CommandError`，实现层包装具体 IO / 数据库错误。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GalleryError {
    Io(String),
    InvalidArgument(String),
    Unavailable(String),
}

impl fmt::Display for GalleryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "图库 IO 错误：{msg}"),
            Self::InvalidArgument(msg) => write!(f, "图库参数错误：{msg}"),
            Self::Unavailable(msg) => write!(f, "图库不可用：{msg}"),
        }
    }
}

impl std::error::Error for GalleryError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gallery_filter_default_means_no_restriction() {
        let filter = GalleryFilter::default();
        assert!(filter.query.is_empty());
        assert!(filter.tags.is_empty());
        assert!(filter.format.is_empty());
        assert_eq!(filter.min_width, 0);
        assert_eq!(filter.max_width, usize::MAX);
        assert_eq!(filter.sort, GallerySortKey::Name);
    }

    #[test]
    fn gallery_sort_key_serde_round_trips() {
        for key in [
            GallerySortKey::Name,
            GallerySortKey::Modified,
            GallerySortKey::Size,
            GallerySortKey::Format,
        ] {
            let json = serde_json::to_string(&key).expect("serialize");
            let back: GallerySortKey = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, key);
        }
    }

    #[test]
    fn compare_mode_defaults_to_side_by_side() {
        assert_eq!(CompareMode::default(), CompareMode::SideBySide);
    }

    #[test]
    fn gallery_error_display_is_chinese() {
        let err = GalleryError::Io("磁盘不可读".into());
        assert!(err.to_string().contains("磁盘不可读"));
    }
}
