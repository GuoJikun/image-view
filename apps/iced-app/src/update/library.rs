//! 导航、目录扫描、排序与文件操作 → `iv-application::FolderNav` 用例。

use std::path::{Path, PathBuf};
use std::time::Instant;

use iced::Task;
use iced::widget::operation::{self, AbsoluteOffset};
use iv_domain::SortOrder;

use crate::adapters::errors;
use crate::message::Message;
use crate::state::AppState;
use crate::state::library_state::THUMB_ITEM_WIDTH;
use crate::widgets::thumbnail_strip::THUMB_SCROLL_ID;

/// 打开文件或目录（目录取第一张），并重建目录列表。
pub fn open_path(state: &mut AppState, path: PathBuf) -> Task<Message> {
    let resolved = if path.is_dir() {
        match state.nav.open_dir(&path) {
            Some(first) => first,
            None => {
                state.notice = Some(format!("目录中没有可显示的图片：{}", path.display()));
                return Task::none();
            }
        }
    } else if path.is_file() {
        state.nav.set_file(&path);
        path
    } else {
        state.notice = Some(format!("路径不存在或无法读取：{}", path.display()));
        return Task::none();
    };

    log::info!(target: "image_view::app", "打开图片：{}", resolved.display());
    state.notice = None;
    go_to_current(state)
}

/// 前后翻页；边界不循环。
pub fn navigate(state: &mut AppState, delta: isize) -> Task<Message> {
    if state.nav.step(delta).is_none() {
        return Task::none();
    }
    go_to_current(state)
}

pub fn select(state: &mut AppState, index: usize) -> Task<Message> {
    if state.nav.select(index).is_none() {
        return Task::none();
    }
    go_to_current(state)
}

pub fn first(state: &mut AppState) -> Task<Message> {
    select(state, 0)
}

pub fn last(state: &mut AppState) -> Task<Message> {
    let last = state.nav.files.len().saturating_sub(1);
    select(state, last)
}

pub fn set_sort(state: &mut AppState, order: SortOrder) -> Task<Message> {
    state.nav.set_sort(order);
    state.menu_open = false;
    persist(state);
    request_visible_thumbnails(state);
    center_thumbnails(state)
}

/// 当前下标变化后统一的收尾：显示、元数据、持久化、缩略图居中。
fn go_to_current(state: &mut AppState) -> Task<Message> {
    show_current(state);
    persist(state);
    request_visible_thumbnails(state);
    center_thumbnails(state)
}

fn show_current(state: &mut AppState) {
    let Some(path) = state.current_path() else {
        state.viewer.clear();
        state.library.meta.clear();
        return;
    };

    state.viewer.show(&path);
    state.library.meta.load(&path);
    if let Some(image) = state.viewer.image.clone() {
        state.library.meta.absorb(&image);
    }
    // 手动翻页重置幻灯片计时。
    state.last_advance = Instant::now();
}

/// 只为可见范围（外扩 3 张）排队缩略图；`ThumbnailState::needs` 会去重。
pub fn request_visible_thumbnails(state: &mut AppState) {
    let total = state.nav.files.len();
    let range = state.library.visible_range(total);
    let start = range.start.saturating_sub(3);
    let end = (range.end + 3).min(total);
    let paths: Vec<PathBuf> = state.nav.files[start..end].to_vec();
    for path in paths {
        state.library.thumbs.request(path);
    }
}

pub fn thumb_scrolled(state: &mut AppState, offset_x: f32, viewport_w: f32) {
    state.library.thumb_offset_x = offset_x.max(0.0);
    if viewport_w > 0.0 {
        state.library.thumb_viewport_w = viewport_w;
    }
    request_visible_thumbnails(state);
}

pub fn toggle_thumbnails(state: &mut AppState) {
    state.library.thumbs.show = !state.library.thumbs.show;
    state.settings.show_thumbnails = state.library.thumbs.show;
    persist(state);
}

/// 让缩略图条把当前项滚到中间；widget 尚未创建时该操作是空操作。
pub fn center_thumbnails(state: &AppState) -> Task<Message> {
    let Some(index) = state.nav.index else {
        return Task::none();
    };
    let offset = (index as f32 * THUMB_ITEM_WIDTH + THUMB_ITEM_WIDTH / 2.0
        - state.library.thumb_viewport_w / 2.0)
        .max(0.0);
    operation::scroll_to(
        THUMB_SCROLL_ID,
        AbsoluteOffset {
            x: Some(offset),
            y: None,
        },
    )
}

pub fn delete_current(state: &mut AppState) -> Task<Message> {
    let Some(path) = state.current_path() else {
        return Task::none();
    };

    if let Err(error) = iv_platform::delete_to_trash(&path) {
        state.notice = Some(errors::file_op_error_text(&error));
        return Task::none();
    }

    state.viewer.forget(&path);
    state.library.thumbs.remove(&path);
    state.notice = Some(format!("已移到回收站：{}", file_name(&path)));

    if state.nav.remove_current().is_some() {
        return go_to_current(state);
    }

    state.viewer.clear();
    state.library.meta.clear();
    persist(state);
    Task::none()
}

pub fn reveal(state: &mut AppState) {
    let Some(path) = state.current_path() else {
        return;
    };
    if let Err(error) = iv_platform::reveal_in_file_manager(&path) {
        state.notice = Some(errors::file_op_error_text(&error));
    }
}

pub fn copy_path(state: &AppState) -> Task<Message> {
    let Some(path) = state.current_path() else {
        return Task::none();
    };
    iced::clipboard::write::<Message>(path.display().to_string())
}

/// 只保存 Core 定义的设置字段；窗口几何由宿主平台负责。
pub fn persist(state: &mut AppState) {
    state.settings.last_dir =
        (!state.nav.dir.as_os_str().is_empty()).then(|| state.nav.dir.clone());
    state.settings.last_file = state.current_path();
    state.settings.sort = state.nav.sort;
    state.settings.show_thumbnails = state.library.thumbs.show;
    state.settings.save(&mut state.storage);
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};

    use iv_infra::storage::Storage;

    use super::*;

    /// 每个用例独立目录，避免并行执行时互相覆盖。
    fn temp_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "image-view-iced-{}-{tag}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建临时目录");
        dir
    }

    fn fixture(dir: &Path, names: &[&str]) {
        for name in names {
            fs::write(dir.join(name), b"").expect("写入夹具文件");
        }
    }

    /// 打开目录：默认按文件名排序取第一张，并同步缩略图与持久化。
    #[test]
    fn open_directory_selects_first_and_persists() {
        let images = temp_dir("open-images");
        let config = temp_dir("open-config");
        fixture(&images, &["b.png", "a.png", "c.jpg"]);

        let mut state = AppState::for_test(config.clone());
        let _ = open_path(&mut state, images.clone());

        assert_eq!(state.nav.index, Some(0));
        assert_eq!(state.current_path(), Some(images.join("a.png")));
        assert_eq!(state.nav.files.len(), 3);
        // 元数据在打开时立刻可用，尺寸等待解码结果补齐。
        let info = state.library.meta.info.as_ref().expect("应载入元数据");
        assert_eq!(info.format, "PNG");
        // 首张缩略图已排队，不再重复请求。
        assert!(!state.library.thumbs.needs(&images.join("a.png")));
        // 打开即持久化，下一次启动可恢复会话。
        assert!(config.join("settings.ron").is_file());

        let _ = fs::remove_dir_all(&images);
        let _ = fs::remove_dir_all(&config);
    }

    /// 翻页在两端停住，不循环。
    #[test]
    fn navigation_stops_at_both_ends() {
        let images = temp_dir("nav-images");
        let config = temp_dir("nav-config");
        fixture(&images, &["a.png", "b.png"]);

        let mut state = AppState::for_test(config.clone());
        let _ = open_path(&mut state, images.clone());

        let _ = navigate(&mut state, -1);
        assert_eq!(state.nav.index, Some(0), "第一张时上一张应无效果");

        let _ = navigate(&mut state, 1);
        assert_eq!(state.nav.index, Some(1));

        let _ = navigate(&mut state, 1);
        assert_eq!(state.nav.index, Some(1), "最后一张时下一张应无效果");

        let _ = first(&mut state);
        assert_eq!(state.nav.index, Some(0));
        let _ = last(&mut state);
        assert_eq!(state.nav.index, Some(1));

        let _ = fs::remove_dir_all(&images);
        let _ = fs::remove_dir_all(&config);
    }

    /// 滚动偏移被钳制到非负，视口宽度只在有效时更新。
    #[test]
    fn scroll_offset_is_clamped_and_viewport_kept() {
        let config = temp_dir("scroll-config");
        let mut state = AppState::for_test(config.clone());

        thumb_scrolled(&mut state, -40.0, 320.0);
        assert_eq!(state.library.thumb_offset_x, 0.0);
        assert_eq!(state.library.thumb_viewport_w, 320.0);

        thumb_scrolled(&mut state, 120.0, 0.0);
        assert_eq!(state.library.thumb_offset_x, 120.0);
        assert_eq!(
            state.library.thumb_viewport_w, 320.0,
            "非法视口宽度应保留旧值"
        );

        let _ = fs::remove_dir_all(&config);
    }

    /// 缩略图条开关写入设置，重启后保持一致。
    #[test]
    fn toggling_thumbnails_is_persisted() {
        let config = temp_dir("toggle-config");
        let mut state = AppState::for_test(config.clone());
        assert!(state.library.thumbs.show);

        toggle_thumbnails(&mut state);
        assert!(!state.library.thumbs.show);
        assert!(!state.settings.show_thumbnails);
        // 设置以 `iv-infra` 定义的键写入存储，重启后可读回。
        assert!(
            state
                .storage
                .get_string(iv_infra::settings::STORAGE_KEY)
                .is_some(),
            "设置应写入存储"
        );

        let _ = fs::remove_dir_all(&config);
    }
}
