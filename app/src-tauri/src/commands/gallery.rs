//! 图库相关 Command：把前端的筛选/分页/标签操作转成领域调用。
//!
//! 业务逻辑在 `iv-library`，这里只做参数解析与 DTO 转换。

use std::path::Path;

use tauri::State;

use iv_domain::{GalleryFilter, GallerySortKey};

use crate::dto::{CommandError, CompareViewDto, GalleryFilterDto, GallerySnapshotDto};
use crate::state::AppState;

/// 解析前端传来的排序标识；未知值回退名称序（与 `parse_sort` 同样的容错策略）。
fn parse_gallery_sort(value: &str) -> GallerySortKey {
    match value {
        "modified" => GallerySortKey::Modified,
        "size" => GallerySortKey::Size,
        "format" => GallerySortKey::Format,
        _ => GallerySortKey::Name,
    }
}

fn to_domain(filter: GalleryFilterDto) -> GalleryFilter {
    GalleryFilter {
        query: filter.query,
        tags: filter.tags,
        format: filter.format,
        min_width: filter.min_width,
        max_width: filter.max_width,
        from_date: filter.from_date,
        to_date: filter.to_date,
        sort: parse_gallery_sort(&filter.sort),
    }
}

#[tauri::command]
pub fn open_gallery(
    state: State<'_, AppState>,
    path: String,
    filter: GalleryFilterDto,
) -> Result<GallerySnapshotDto, CommandError> {
    state.open_gallery(Path::new(&path), to_domain(filter))
}

#[tauri::command]
pub fn refresh_gallery(state: State<'_, AppState>) -> Result<GallerySnapshotDto, CommandError> {
    state.refresh_gallery()
}

#[tauri::command]
pub fn gallery_page(
    state: State<'_, AppState>,
    offset: usize,
    limit: usize,
) -> Result<GallerySnapshotDto, CommandError> {
    state.gallery_page(offset, limit)
}

#[tauri::command]
pub fn set_gallery_filter(
    state: State<'_, AppState>,
    filter: GalleryFilterDto,
) -> Result<GallerySnapshotDto, CommandError> {
    state.set_gallery_filter(to_domain(filter))
}

#[tauri::command]
pub fn add_gallery_tag(
    state: State<'_, AppState>,
    image_id: String,
    tag: String,
) -> Result<(), CommandError> {
    state.add_gallery_tag(&image_id, &tag)
}

#[tauri::command]
pub fn remove_gallery_tag(
    state: State<'_, AppState>,
    image_id: String,
    tag: String,
) -> Result<(), CommandError> {
    state.remove_gallery_tag(&image_id, &tag)
}

/// 进入对比：接收 2~4 个图库图片句柄，校验授权后返回对比视图。
/// `full_resolution=true` 时批量解码全尺寸预览（默认缩略图模式为 false）。
#[tauri::command]
pub fn enter_compare(
    state: State<'_, AppState>,
    image_ids: Vec<String>,
    full_resolution: bool,
) -> Result<CompareViewDto, CommandError> {
    state.enter_compare(image_ids, full_resolution)
}

/// 退出对比：解除全像素预览的缓存保护。
#[tauri::command]
pub fn exit_compare(state: State<'_, AppState>) {
    state.exit_compare();
}

/// 切换对比布局（`side-by-side` / `overlay`）。
#[tauri::command]
pub fn set_compare_layout(
    state: State<'_, AppState>,
    layout: String,
) -> Result<CompareViewDto, CommandError> {
    state.set_compare_layout(&layout)
}
