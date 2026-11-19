//! 应用状态：目录导航、ImageId 授权表与预览/缩略图服务。
//!
//! Command 只能通过 ImageId 访问文件；`registry` 在目录扫描时建立，
//! 协议层也复用它，避免把前端传入的路径直接拼进 URL 读取。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::UNIX_EPOCH;

use iv_application::{FolderNav, GalleryIndex};
use iv_domain::{GalleryEntry, GalleryFilter, Language, SortOrder, ThemeSetting};
use iv_image_decode::{Caps, ColorTarget};
use iv_library::SqliteGalleryIndex;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::dto::{
    CommandError, CompareViewDto, DirectoryEntryDto, DirectorySnapshot, GalleryEntryDto,
    GalleryFilterDto, GallerySnapshotDto, MenuStateDto, PreviewRequestDto, UiPreferencesDto,
};
use crate::preview::PreviewService;
use crate::settings_store::SettingsStore;
use crate::thumbnails::ThumbnailService;

struct DirectoryState {
    nav: FolderNav,
    entries: Vec<DirectoryEntryDto>,
}

/// 对比视图会话：槽位的图片句柄、布局与全像素预览版本。
/// 缩放/平移由前端 view 状态负责。
struct CompareSession {
    image_ids: Vec<String>,
    layout: String,
    previews: Vec<Option<crate::dto::ComparePreviewDto>>,
}

impl DirectoryState {
    fn new(sort: SortOrder) -> Self {
        Self {
            nav: FolderNav {
                sort,
                ..FolderNav::default()
            },
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
            // 只刷新当前图片的 revision，避免每次翻页都为整个目录做 metadata。
            if let Some(entry) = self.nav.index.and_then(|index| self.entries.get_mut(index)) {
                entry.revision = modified_millis(Path::new(&entry.path));
            }
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
                revision: modified_millis(path),
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
    /// 图库跨目录条目用的独立授权表；与当前目录的 `registry` 分开，避免翻目录时被清空。
    gallery_registry: Mutex<HashMap<String, PathBuf>>,
    gallery: Mutex<SqliteGalleryIndex>,
    gallery_dir: Mutex<Option<PathBuf>>,
    gallery_filter: Mutex<GalleryFilter>,
    gallery_offset: Mutex<usize>,
    compare: Mutex<Option<CompareSession>>,
    pub previews: PreviewService,
    pub thumbnails: ThumbnailService,
    caps: Caps,
    startup_path: Option<String>,
    settings: Arc<SettingsStore>,
}

impl AppState {
    /// 窗口几何与其他偏好共用同一份 `SettingsStore`，由适配层读写 `settings.window`。
    pub fn settings_store(&self) -> Arc<SettingsStore> {
        self.settings.clone()
    }

    pub fn new(app: &AppHandle, startup_path: Option<String>) -> Self {
        let settings = SettingsStore::open(settings_path(app));
        let restored = settings.snapshot();

        let gallery = match SqliteGalleryIndex::open(&library_db_path(app), &sidecar_dir(app)) {
            Ok(index) => index,
            Err(error) => {
                log::warn!(target: "image_view::library", "图库索引初始化失败，图库功能不可用：{error}");
                // 尝试用临时目录再开一次；仍失败时下面用占位路径，命令层返回降级快照。
                SqliteGalleryIndex::open(
                    &std::env::temp_dir().join("image-view-library.sqlite3"),
                    &std::env::temp_dir().join("image-view-sidecars"),
                )
                .expect("临时图库索引也必须可创建")
            }
        };

        // 命令行参数优先于上次会话；会话记录的文件不存在时退回上次目录。
        let startup_path = startup_path.or_else(|| {
            if let Some(file) = restored.last_file.as_ref().filter(|file| file.is_file()) {
                return Some(file.to_string_lossy().into_owned());
            }
            restored
                .last_dir
                .as_ref()
                .filter(|dir| dir.is_dir())
                .map(|dir| dir.to_string_lossy().into_owned())
        });

        Self {
            directory: Mutex::new(DirectoryState::new(restored.sort)),
            registry: Mutex::new(HashMap::new()),
            gallery_registry: Mutex::new(HashMap::new()),
            gallery: Mutex::new(gallery),
            gallery_dir: Mutex::new(None),
            gallery_filter: Mutex::new(GalleryFilter::default()),
            gallery_offset: Mutex::new(0),
            compare: Mutex::new(None),
            previews: PreviewService::new(app.clone()),
            thumbnails: ThumbnailService::new(thumbnail_cache_dir(app)),
            caps: Caps::default(),
            startup_path,
            settings,
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
        self.persist_session(&directory);
        Ok(directory.snapshot())
    }

    /// 前进/后退一张；到边界返回 `None`（不循环）。
    pub fn navigate(&self, delta: isize) -> Option<DirectorySnapshot> {
        let mut directory = lock(&self.directory);
        let mut registry = lock(&self.registry);
        directory.nav.step(delta)?;
        directory.sync_entries(&mut registry);
        self.persist_session(&directory);
        Some(directory.snapshot())
    }

    /// 点击缩略图或键盘直接选中某一项。
    pub fn select(&self, index: usize) -> Option<DirectorySnapshot> {
        let mut directory = lock(&self.directory);
        let mut registry = lock(&self.registry);
        directory.nav.select(index)?;
        directory.sync_entries(&mut registry);
        self.persist_session(&directory);
        Some(directory.snapshot())
    }

    pub fn set_sort(&self, order: SortOrder) -> DirectorySnapshot {
        let mut directory = lock(&self.directory);
        let mut registry = lock(&self.registry);
        directory.nav.set_sort(order);
        directory.sync_entries(&mut registry);
        self.persist_session(&directory);
        directory.snapshot()
    }

    pub fn snapshot(&self) -> DirectorySnapshot {
        lock(&self.directory).snapshot()
    }

    pub fn startup_path(&self) -> Option<String> {
        self.startup_path.clone()
    }

    /// 只接受当前目录或图库登记过的 ImageId。
    pub fn path_for(&self, image_id: &str) -> Option<PathBuf> {
        lock(&self.registry)
            .get(image_id)
            .cloned()
            .or_else(|| lock(&self.gallery_registry).get(image_id).cloned())
    }

    /// 打开图库目录：启动后台索引并返回首屏快照。
    pub fn open_gallery(
        &self,
        path: &Path,
        filter: GalleryFilter,
    ) -> Result<GallerySnapshotDto, CommandError> {
        if !path.is_dir() {
            return Err(CommandError::invalid_path(format!(
                "{} 不是目录",
                path.display()
            )));
        }
        *self
            .gallery_filter
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = filter;
        *self
            .gallery_offset
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = 0;
        *self.gallery_dir.lock().unwrap_or_else(|p| p.into_inner()) = Some(path.to_path_buf());
        self.gallery
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .open(path)
            .map_err(gallery_error)?;
        self.gallery_snapshot(0, 500)
    }

    pub fn refresh_gallery(&self) -> Result<GallerySnapshotDto, CommandError> {
        self.gallery
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .refresh()
            .map_err(gallery_error)?;
        let offset = *self
            .gallery_offset
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        self.gallery_snapshot(offset, 500)
    }

    pub fn set_gallery_filter(
        &self,
        filter: GalleryFilter,
    ) -> Result<GallerySnapshotDto, CommandError> {
        *self
            .gallery_filter
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = filter;
        *self
            .gallery_offset
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = 0;
        self.gallery_snapshot(0, 500)
    }

    pub fn gallery_page(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<GallerySnapshotDto, CommandError> {
        let limit = limit.clamp(1, 500);
        *self
            .gallery_offset
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = offset;
        self.gallery_snapshot(offset, limit)
    }

    /// 组装图库快照：查询索引、登记授权表、拼 DTO。
    fn gallery_snapshot(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<GallerySnapshotDto, CommandError> {
        let filter = self
            .gallery_filter
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let index = self.gallery.lock().unwrap_or_else(|p| p.into_inner());
        let progress = index.progress();
        let page = index.page(&filter, offset, limit).map_err(gallery_error)?;
        drop(index);

        // 图库条目的 id 也登记到授权表，供缩略图与对比预览使用。
        {
            let mut registry = self
                .gallery_registry
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            for entry in &page.entries {
                registry.insert(entry.id.clone(), PathBuf::from(&entry.path));
            }
        }

        let dir = self
            .gallery_dir
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();

        Ok(GallerySnapshotDto {
            dir,
            entries: page.entries.into_iter().map(gallery_entry_dto).collect(),
            total: page.total,
            offset,
            limit,
            indexed: progress.indexed,
            index_ready: progress.ready,
            degraded: progress.degraded,
            filter: gallery_filter_dto(&filter),
        })
    }

    pub fn add_gallery_tag(&self, image_id: &str, tag: &str) -> Result<(), CommandError> {
        let path = self
            .gallery_registry
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(image_id)
            .cloned()
            .ok_or_else(|| CommandError::invalid_path("图库图片句柄已失效"))?;
        self.gallery
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .add_tag(&path, tag)
            .map_err(gallery_error)
    }

    pub fn remove_gallery_tag(&self, image_id: &str, tag: &str) -> Result<(), CommandError> {
        let path = self
            .gallery_registry
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(image_id)
            .cloned()
            .ok_or_else(|| CommandError::invalid_path("图库图片句柄已失效"))?;
        self.gallery
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove_tag(&path, tag)
            .map_err(gallery_error)
    }

    pub fn enter_compare(
        &self,
        image_ids: Vec<String>,
        full_resolution: bool,
    ) -> Result<CompareViewDto, CommandError> {
        if !(2..=4).contains(&image_ids.len()) {
            return Err(CommandError::new(
                "invalid_argument",
                "对比模式需要 2~4 张图片",
            ));
        }
        // 每个句柄都必须已登记，否则缩略图/预览协议会 404；同时收集全像素解码所需的路径。
        let paths: Vec<PathBuf> = {
            let registry = self
                .gallery_registry
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            let directory = self.registry.lock().unwrap_or_else(|p| p.into_inner());
            image_ids
                .iter()
                .map(|id| {
                    registry
                        .get(id)
                        .or_else(|| directory.get(id))
                        .cloned()
                        .ok_or_else(|| CommandError::invalid_path("对比图片句柄已失效"))
                })
                .collect::<Result<Vec<_>, _>>()?
        };

        let previews: Vec<Option<crate::dto::ComparePreviewDto>> = if full_resolution {
            let items: Vec<(String, PathBuf)> = image_ids.iter().cloned().zip(paths).collect();
            self.previews
                .request_compare_batch(items)
                .into_iter()
                .map(|item| {
                    item.map(|(image_id, revision)| crate::dto::ComparePreviewDto {
                        image_id,
                        revision,
                    })
                })
                .collect()
        } else {
            vec![None; image_ids.len()]
        };

        let session = CompareSession {
            image_ids: image_ids.clone(),
            layout: "side-by-side".to_owned(),
            previews: previews.clone(),
        };
        let dto = CompareViewDto {
            image_ids,
            layout: session.layout.clone(),
            previews,
        };
        *self.compare.lock().unwrap_or_else(|p| p.into_inner()) = Some(session);
        Ok(dto)
    }

    pub fn set_compare_layout(&self, layout: &str) -> Result<CompareViewDto, CommandError> {
        let normalized = match layout {
            "overlay" => "overlay",
            _ => "side-by-side",
        };
        let mut guard = self.compare.lock().unwrap_or_else(|p| p.into_inner());
        let session = guard
            .as_mut()
            .ok_or_else(|| CommandError::new("gallery_unavailable", "尚未进入对比模式"))?;
        session.layout = normalized.to_owned();
        Ok(CompareViewDto {
            image_ids: session.image_ids.clone(),
            layout: session.layout.clone(),
            previews: session.previews.clone(),
        })
    }

    /// 退出对比：解除预览保护并清空会话。
    pub fn exit_compare(&self) {
        self.previews.clear_compare();
        *self.compare.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }

    /// 切换显示目标色彩空间；返回是否真的发生变化。
    ///
    /// 变化时缓存会失效，前端据此重新请求当前图片。
    pub fn set_color_target(&self, target: ColorTarget) -> bool {
        self.previews.set_color_target(target)
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
        let request = self
            .previews
            .request(image_id.to_owned(), path, self.caps.clone());
        self.preload_neighbors();
        Ok(request)
    }

    /// 当前图片的前后各一张交给后台预加载；缓存命中/在途请求会自行跳过。
    fn preload_neighbors(&self) {
        let directory = lock(&self.directory);
        let Some(index) = directory.nav.index else {
            return;
        };

        let mut neighbors = Vec::with_capacity(2);
        if index > 0
            && let Some(entry) = directory.entries.get(index - 1)
        {
            neighbors.push((entry.id.clone(), PathBuf::from(&entry.path)));
        }
        if let Some(entry) = directory.entries.get(index + 1) {
            neighbors.push((entry.id.clone(), PathBuf::from(&entry.path)));
        }
        drop(directory);

        for (image_id, path) in neighbors {
            self.previews.preload(image_id, path, self.caps.clone());
        }
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
        self.thumbnails.remove_path(&path);
        self.persist_session(&directory);
        Ok(directory.snapshot())
    }

    /// 当前界面偏好（主题、语言、色彩标记、缩略图条、启动检查更新）。
    pub fn ui_preferences(&self) -> UiPreferencesDto {
        let settings = self.settings.snapshot();
        UiPreferencesDto {
            show_thumbnails: settings.show_thumbnails,
            show_color_status: settings.show_color_status,
            theme: theme_label(settings.theme).to_owned(),
            language: language_label(settings.language).to_owned(),
            check_updates_on_startup: settings.check_updates_on_startup,
        }
    }

    /// 整体写回界面偏好；取值非法时拒绝整次写入，避免半套设置落盘。
    pub fn set_ui_preferences(&self, prefs: &UiPreferencesDto) -> Result<(), CommandError> {
        let theme = parse_theme(&prefs.theme)?;
        let language = parse_language(&prefs.language)?;
        self.settings.update(|settings| {
            settings.show_thumbnails = prefs.show_thumbnails;
            settings.show_color_status = prefs.show_color_status;
            settings.theme = theme;
            settings.language = language;
            settings.check_updates_on_startup = prefs.check_updates_on_startup;
        });
        Ok(())
    }

    /// 当前应使用的界面语言标签（`"zh"` / `"en"`）。
    ///
    /// 前端有自己的 `navigator.language`，这里只服务原生菜单首帧与启动日志——
    /// 那时前端还没跑起来，只能由后端读设置与系统语言。
    pub fn locale_tag(&self) -> &'static str {
        self.settings
            .snapshot()
            .language
            .resolve_tag(iv_platform::system_locale().as_deref())
    }

    /// 原生菜单首帧的勾选状态：同样只服务前端启动前的那一刻，
    /// 之后由前端 `useMenuState()` 推送真实状态。
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn menu_state(&self) -> MenuStateDto {
        let settings = self.settings.snapshot();
        MenuStateDto {
            thumbnails: settings.show_thumbnails,
            // 幻灯片与元数据面板只是运行时状态，启动时一定未开启。
            slideshow: false,
            color_badge: settings.show_color_status,
            metadata: false,
            auto_update: settings.check_updates_on_startup,
            sort: self.snapshot().sort,
            theme: theme_label(settings.theme).to_owned(),
            language: language_label(settings.language).to_owned(),
        }
    }

    /// 退出前把防抖队列里的最后一次修改同步写盘。
    pub fn flush_settings(&self) {
        self.settings.flush();
    }

    /// 会话变化后记录目录、当前图片与排序；写盘由 `SettingsStore` 防抖。
    fn persist_session(&self, directory: &DirectoryState) {
        self.settings.update(|settings| {
            settings.last_dir =
                (!directory.nav.dir.as_os_str().is_empty()).then(|| directory.nav.dir.clone());
            settings.last_file = directory.nav.current().map(Path::to_path_buf);
            settings.sort = directory.nav.sort;
        });
    }
}

/// 图库领域错误 → 命令错误：`GalleryError::Unavailable` 用独立错误码供前端降级。
fn gallery_error(error: iv_domain::GalleryError) -> CommandError {
    let code = match &error {
        iv_domain::GalleryError::Io(_) => "io",
        iv_domain::GalleryError::InvalidArgument(_) => "invalid_argument",
        iv_domain::GalleryError::Unavailable(_) => "gallery_unavailable",
    };
    CommandError::new(code, error.to_string())
}

fn gallery_entry_dto(entry: GalleryEntry) -> GalleryEntryDto {
    GalleryEntryDto {
        id: entry.id,
        path: entry.path.to_string_lossy().into_owned(),
        name: entry.name,
        revision: entry.revision,
        width: entry.width,
        height: entry.height,
        format: entry.format,
        modified_at: entry.modified_at,
        size_bytes: entry.size_bytes,
        tags: entry.tags,
    }
}

fn gallery_filter_dto(filter: &GalleryFilter) -> GalleryFilterDto {
    let sort = match filter.sort {
        iv_domain::GallerySortKey::Name => "name",
        iv_domain::GallerySortKey::Modified => "modified",
        iv_domain::GallerySortKey::Size => "size",
        iv_domain::GallerySortKey::Format => "format",
    };
    GalleryFilterDto {
        query: filter.query.clone(),
        tags: filter.tags.clone(),
        format: filter.format.clone(),
        min_width: filter.min_width,
        max_width: filter.max_width,
        from_date: filter.from_date,
        to_date: filter.to_date,
        sort: sort.to_owned(),
    }
}

/// 设置文件放在应用配置目录（Windows：`%APPDATA%\dev.jikun.imageview`）。
fn settings_path(app: &AppHandle) -> PathBuf {
    match app.path().app_config_dir() {
        Ok(dir) => dir.join("settings.ron"),
        Err(error) => {
            log::warn!(
                target: "image_view::settings",
                "无法确定设置目录，改用临时目录：{error}"
            );
            std::env::temp_dir().join("image-view").join("settings.ron")
        }
    }
}

/// 图库索引库放在应用数据目录，与 `settings.ron` 同根。
fn library_db_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join("library.sqlite3"))
        .unwrap_or_else(|_| std::env::temp_dir().join("image-view-library.sqlite3"))
}

/// 标签 sidecar 放在应用数据目录下，原图目录保持只读。
fn sidecar_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join("sidecars"))
        .unwrap_or_else(|_| std::env::temp_dir().join("image-view-sidecars"))
}

/// 缩略图磁盘缓存放在应用缓存目录；与配置分开，浏览器/系统清理缓存时可直接删除。
fn thumbnail_cache_dir(app: &AppHandle) -> Option<PathBuf> {
    match app.path().app_cache_dir() {
        Ok(dir) => Some(dir.join("thumbs")),
        Err(error) => {
            log::warn!(
                target: "image_view::thumbnail",
                "无法确定缓存目录，缩略图只保存在内存：{error}"
            );
            None
        }
    }
}

/// 图片修改时间（毫秒）；0 表示无法读取，前端据此刷新缩略图。
fn modified_millis(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub fn sort_label(order: SortOrder) -> &'static str {
    match order {
        SortOrder::Name => "name",
        SortOrder::ModifiedNewest => "modified",
    }
}

/// 主题的界面取值，与前端 `ThemeSetting` 一致。
pub fn theme_label(theme: ThemeSetting) -> &'static str {
    match theme {
        ThemeSetting::System => "system",
        ThemeSetting::Light => "light",
        ThemeSetting::Dark => "dark",
    }
}

pub fn parse_theme(value: &str) -> Result<ThemeSetting, CommandError> {
    match value {
        "system" => Ok(ThemeSetting::System),
        "light" => Ok(ThemeSetting::Light),
        "dark" => Ok(ThemeSetting::Dark),
        other => Err(CommandError::new(
            "invalid_argument",
            format!("未知主题：{other}"),
        )),
    }
}

/// 语言设置的界面取值，与前端 `LocaleSetting` 一致（`system` 表示跟随系统）。
pub fn language_label(language: Language) -> &'static str {
    match language {
        Language::System => "system",
        Language::Zh => "zh",
        Language::En => "en",
    }
}

pub fn parse_language(value: &str) -> Result<Language, CommandError> {
    match value {
        "system" => Ok(Language::System),
        "zh" => Ok(Language::Zh),
        "en" => Ok(Language::En),
        other => Err(CommandError::new(
            "invalid_argument",
            format!("未知语言：{other}"),
        )),
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

    /// 这些取值直接传给前端与 `settings.ron`，改动会打破两边约定。
    #[test]
    fn preference_labels_match_frontend_values() {
        assert_eq!(theme_label(ThemeSetting::System), "system");
        assert_eq!(theme_label(ThemeSetting::Light), "light");
        assert_eq!(theme_label(ThemeSetting::Dark), "dark");
        assert_eq!(language_label(Language::System), "system");
        assert_eq!(language_label(Language::Zh), "zh");
        assert_eq!(language_label(Language::En), "en");
    }

    #[test]
    fn preference_values_round_trip() {
        for theme in [
            ThemeSetting::System,
            ThemeSetting::Light,
            ThemeSetting::Dark,
        ] {
            assert_eq!(parse_theme(theme_label(theme)).expect("theme"), theme);
        }
        for language in [Language::System, Language::Zh, Language::En] {
            assert_eq!(
                parse_language(language_label(language)).expect("language"),
                language
            );
        }
    }

    /// 前端传错取值时整次写入被拒绝，而不是悄悄写进默认值。
    #[test]
    fn unknown_preference_values_are_rejected() {
        assert_eq!(
            parse_theme("auto").expect_err("theme").code,
            "invalid_argument"
        );
        assert_eq!(
            parse_language("fr").expect_err("language").code,
            "invalid_argument"
        );
    }
}
