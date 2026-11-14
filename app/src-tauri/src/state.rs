//! 应用状态：目录导航、ImageId 授权表与预览/缩略图服务。
//!
//! Command 只能通过 ImageId 访问文件；`registry` 在目录扫描时建立，
//! 协议层也复用它，避免把前端传入的路径直接拼进 URL 读取。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use iv_application::FolderNav;
use iv_domain::SortOrder;
use iv_image_decode::Caps;
use tauri::AppHandle;
use uuid::Uuid;

use crate::dto::{CommandError, DirectoryEntryDto, DirectorySnapshot, PreviewRequestDto};
use crate::preview::PreviewService;
use crate::thumbnails::ThumbnailService;

struct DirectoryState {
    nav: FolderNav,
    entries: Vec<DirectoryEntryDto>,
}

impl DirectoryState {
    fn new() -> Self {
        Self {
            nav: FolderNav::default(),
            entries: Vec::new(),
        }
    }

    /// 文件列表变化时重建 ImageId；仅切换下标时保持 ID 不变，缩略图不重载。
    fn sync_entries(&mut self, registry: &mut HashMap<String, PathBuf>) {
        let unchanged = self.entries.len() == self.nav.files.len()
            && self
                .entries
                .iter()
                .zip(self.nav.files.iter())
                .all(|(entry, path)| entry.path == path.to_string_lossy());
        if unchanged {
            return;
        }

        self.entries = self
            .nav
            .files
            .iter()
            .map(|path| DirectoryEntryDto {
                id: Uuid::new_v4().to_string(),
                path: path.to_string_lossy().into_owned(),
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            })
            .collect();

        registry.clear();
        for entry in &self.entries {
            registry.insert(entry.id.clone(), PathBuf::from(&entry.path));
        }
    }

    fn snapshot(&self) -> DirectorySnapshot {
        let current = self
            .nav
            .index
            .and_then(|index| self.entries.get(index))
            .cloned();
        DirectorySnapshot {
            dir: self.nav.dir.to_string_lossy().into_owned(),
            entries: self.entries.clone(),
            index: self.nav.index,
            position: self.nav.position(),
            can_prev: self.nav.can_prev(),
            can_next: self.nav.can_next(),
            current,
            sort: sort_label(self.nav.sort).to_owned(),
        }
    }
}

pub struct AppState {
    directory: Mutex<DirectoryState>,
    registry: Mutex<HashMap<String, PathBuf>>,
    pub previews: PreviewService,
    pub thumbnails: ThumbnailService,
    caps: Caps,
    startup_path: Option<String>,
}

impl AppState {
    pub fn new(app: &AppHandle, startup_path: Option<String>) -> Self {
        Self {
            directory: Mutex::new(DirectoryState::new()),
            registry: Mutex::new(HashMap::new()),
            previews: PreviewService::new(app.clone()),
            thumbnails: ThumbnailService::default(),
            caps: Caps::default(),
            startup_path,
        }
    }

    /// 打开文件或目录；目录打开第一张，文件则扫描同目录并定位。
    pub fn open_path(&self, path: &Path) -> Result<DirectorySnapshot, CommandError> {
        let canonical = std::fs::canonicalize(path).map_err(|error| {
            CommandError::invalid_path(format!("无法访问 {}：{error}", path.display()))
        })?;
        if !canonical.is_file() && !canonical.is_dir() {
            return Err(CommandError::invalid_path(format!(
                "{} 不是文件或目录",
                canonical.display()
            )));
        }

        let mut directory = lock(&self.directory);
        let mut registry = lock(&self.registry);
        if canonical.is_dir() {
            directory.nav.open_dir(&canonical);
        } else {
            directory.nav.set_file(&canonical);
        }
        directory.sync_entries(&mut registry);
        Ok(directory.snapshot())
    }

    /// 前进/后退一张；到边界返回 `None`（不循环）。
    pub fn navigate(&self, delta: isize) -> Option<DirectorySnapshot> {
        let mut directory = lock(&self.directory);
        let mut registry = lock(&self.registry);
        directory.nav.step(delta)?;
        directory.sync_entries(&mut registry);
        Some(directory.snapshot())
    }

    /// 点击缩略图或键盘直接选中某一项。
    pub fn select(&self, index: usize) -> Option<DirectorySnapshot> {
        let mut directory = lock(&self.directory);
        let mut registry = lock(&self.registry);
        directory.nav.select(index)?;
        directory.sync_entries(&mut registry);
        Some(directory.snapshot())
    }

    pub fn set_sort(&self, order: SortOrder) -> DirectorySnapshot {
        let mut directory = lock(&self.directory);
        let mut registry = lock(&self.registry);
        directory.nav.set_sort(order);
        directory.sync_entries(&mut registry);
        directory.snapshot()
    }

    pub fn snapshot(&self) -> DirectorySnapshot {
        lock(&self.directory).snapshot()
    }

    pub fn startup_path(&self) -> Option<String> {
        self.startup_path.clone()
    }

    /// 只接受当前目录登记过的 ImageId。
    pub fn path_for(&self, image_id: &str) -> Option<PathBuf> {
        lock(&self.registry).get(image_id).cloned()
    }

    pub fn request_preview(&self, image_id: &str) -> Result<PreviewRequestDto, CommandError> {
        let path = self
            .path_for(image_id)
            .ok_or_else(|| CommandError::invalid_path("图片句柄已失效，请重新打开目录"))?;
        if !path.is_file() {
            return Err(CommandError::io(format!(
                "文件已移动或删除：{}",
                path.display()
            )));
        }
        Ok(self
            .previews
            .request(image_id.to_owned(), path, self.caps.clone()))
    }

    pub fn cancel_preview(&self) -> u64 {
        self.previews.cancel()
    }

    pub fn close(&self) {
        self.previews.cancel();
        self.previews.clear();
    }

    pub fn reveal(&self, image_id: &str) -> Result<(), CommandError> {
        let path = self
            .path_for(image_id)
            .ok_or_else(|| CommandError::invalid_path("图片句柄已失效"))?;
        iv_platform::reveal_in_file_manager(&path)
            .map_err(|error| CommandError::io(error.to_string()))
    }

    /// 把当前图片移到系统回收站；随后选中下一张（若已是最后一张则退回新的末尾）。
    pub fn delete_current(&self, image_id: &str) -> Result<DirectorySnapshot, CommandError> {
        let path = self
            .path_for(image_id)
            .ok_or_else(|| CommandError::invalid_path("图片句柄已失效"))?;

        let mut directory = lock(&self.directory);
        let mut registry = lock(&self.registry);
        let is_current = directory
            .snapshot()
            .current
            .is_some_and(|entry| entry.id == image_id);
        if !is_current {
            return Err(CommandError::invalid_path("只能删除当前正在查看的图片"));
        }

        iv_platform::delete_to_trash(&path).map_err(|error| {
            CommandError::io(format!("无法移到回收站（{}）：{error}", path.display()))
        })?;

        directory.nav.remove_current();
        directory.sync_entries(&mut registry);
        self.previews.cancel();
        Ok(directory.snapshot())
    }
}

pub fn sort_label(order: SortOrder) -> &'static str {
    match order {
        SortOrder::Name => "name",
        SortOrder::ModifiedNewest => "modified",
    }
}

pub fn parse_sort(value: &str) -> Result<SortOrder, CommandError> {
    match value {
        "name" => Ok(SortOrder::Name),
        "modified" => Ok(SortOrder::ModifiedNewest),
        other => Err(CommandError::new(
            "invalid_argument",
            format!("未知排序方式：{other}"),
        )),
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
    fn sort_labels_round_trip() {
        assert_eq!(sort_label(parse_sort("name").expect("name")), "name");
        assert_eq!(
            sort_label(parse_sort("modified").expect("modified")),
            "modified"
        );
        assert_eq!(
            parse_sort("unknown").expect_err("unknown").code,
            "invalid_argument"
        );
    }
}
