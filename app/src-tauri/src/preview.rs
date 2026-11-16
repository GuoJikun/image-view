//! 预览资源：后台解码、按文件身份缓存像素、通过事件通知前端。
//!
//! - `Open` 优先：队列里只执行最新的打开请求，预加载会让路。
//! - `Preload` 只在后台填充缓存，不发送事件；超过 30 MP 的图片直接跳过。
//! - 缓存键是 `image_id + 路径 + 修改时间 + 文件大小`，预加载结果可被后续打开复用。
//!
//! 像素只保存在 Rust 侧；事件 payload 只有尺寸/元数据与资源句柄。

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, mpsc};
use std::thread;
use std::time::UNIX_EPOCH;

use image::ImageEncoder as _;
use iv_image_decode::{Caps, LoadError, Purpose, decode_image};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::dto::{PreviewFailedDto, PreviewReadyDto, PreviewRequestDto};

pub const PREVIEW_READY_EVENT: &str = "image:preview-ready";
pub const PREVIEW_FAILED_EVENT: &str = "image:preview-failed";

/// 缓存最近三张预览：当前 + 前后各一张。
const MAX_CACHED_PREVIEWS: usize = 3;
/// 像素预算：约 3 张 24 MP，或当前大图 + 两侧预加载。
const MAX_CACHED_PIXELS: usize = 72_000_000;

/// 文件身份；路径、修改时间和大小都一致才认为缓存仍然有效。
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct FileStamp {
    path: PathBuf,
    modified: u64,
    len: u64,
}

fn file_stamp(path: &Path) -> FileStamp {
    let metadata = std::fs::metadata(path).ok();
    let modified = metadata
        .as_ref()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    let len = metadata.as_ref().map(|meta| meta.len()).unwrap_or(0);
    FileStamp {
        path: path.to_path_buf(),
        modified,
        len,
    }
}

/// 一个已解码的预览资源。PNG 编码延迟到协议首次访问时进行并缓存。
pub struct PreviewEntry {
    pub image_id: String,
    pub revision: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
    png: OnceLock<Arc<[u8]>>,
    stamp: FileStamp,
    source_width: u32,
    source_height: u32,
    source_depth: u8,
    color_converted: bool,
    color_source: Option<String>,
    file_size: u64,
    modified_at: Option<i64>,
}

impl PreviewEntry {
    /// 编码为 WebView 可直接解码的 PNG。失败时返回空字节，由协议层转成 500。
    pub fn png(&self) -> Arc<[u8]> {
        self.png
            .get_or_init(|| {
                let mut bytes = Vec::new();
                let encoder = image::codecs::png::PngEncoder::new_with_quality(
                    &mut bytes,
                    image::codecs::png::CompressionType::Fast,
                    image::codecs::png::FilterType::Adaptive,
                );
                match encoder.write_image(
                    &self.rgba,
                    self.width,
                    self.height,
                    image::ExtendedColorType::Rgba8,
                ) {
                    Ok(()) => Arc::from(bytes),
                    Err(error) => {
                        log::error!(
                            target: "image_view::preview",
                            "预览 PNG 编码失败（{}×{}）：{error}",
                            self.width,
                            self.height
                        );
                        Arc::from(&[][..])
                    }
                }
            })
            .clone()
    }

    fn pixels(&self) -> usize {
        (self.width as usize).saturating_mul(self.height as usize)
    }

    fn ready_payload(&self, request_id: u64) -> PreviewReadyDto {
        PreviewReadyDto {
            request_id,
            image_id: self.image_id.clone(),
            revision: self.revision,
            path: self.stamp.path.to_string_lossy().into_owned(),
            file_name: self
                .stamp
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            format: crate::dto::format_label(&self.stamp.path),
            width: self.width,
            height: self.height,
            source_width: self.source_width,
            source_height: self.source_height,
            downscaled: (self.width, self.height) != (self.source_width, self.source_height),
            source_depth: self.source_depth,
            color_converted: self.color_converted,
            color_source: self.color_source.clone(),
            file_size: self.file_size,
            modified_at: self.modified_at,
        }
    }
}

struct PreviewStore {
    entries: VecDeque<Arc<PreviewEntry>>,
    /// 单调递增的 URL revision；命中缓存时复用条目自带的 revision。
    next_revision: u64,
}

impl PreviewStore {
    fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            next_revision: 0,
        }
    }

    fn reserve_revision(&mut self) -> u64 {
        self.next_revision = self.next_revision.wrapping_add(1);
        self.next_revision
    }

    /// 按文件身份查找；命中后刷新最近使用顺序。
    fn find(&mut self, image_id: &str, stamp: &FileStamp) -> Option<Arc<PreviewEntry>> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.image_id == image_id && &entry.stamp == stamp)?;
        let entry = self.entries.remove(index)?;
        self.entries.push_back(entry.clone());
        Some(entry)
    }

    fn contains(&self, image_id: &str, stamp: &FileStamp) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.image_id == image_id && &entry.stamp == stamp)
    }

    fn get(&mut self, image_id: &str, revision: u64) -> Option<Arc<PreviewEntry>> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.image_id == image_id && entry.revision == revision)?;
        let entry = self.entries.remove(index)?;
        self.entries.push_back(entry.clone());
        Some(entry)
    }

    /// 同一图片只保留最新 revision；`protected` 为当前显示图片，淘汰时跳过。
    fn insert(&mut self, entry: Arc<PreviewEntry>, protected: Option<&str>) {
        self.entries
            .retain(|candidate| candidate.image_id != entry.image_id);
        self.entries.push_back(entry);

        let mut pixels: usize = self.entries.iter().map(|entry| entry.pixels()).sum();
        let mut index = 0;
        while (self.entries.len() > MAX_CACHED_PREVIEWS || pixels > MAX_CACHED_PIXELS)
            && index < self.entries.len()
        {
            if protected.is_some_and(|image_id| self.entries[index].image_id == image_id) {
                index += 1;
                continue;
            }
            pixels -= self.entries[index].pixels();
            self.entries.remove(index);
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JobKind {
    Open,
    Preload,
}

struct Job {
    kind: JobKind,
    /// 打开请求的代号；预加载为 0。
    request_id: u64,
    /// 导航世代；用户切换图片后旧的预加载直接丢弃。
    epoch: u64,
    image_id: String,
    revision: u64,
    stamp: FileStamp,
    caps: Caps,
}

struct Shared {
    app: AppHandle,
    store: Mutex<PreviewStore>,
    /// 最新打开请求的代号；解码线程据此丢弃过期结果。
    current: AtomicU64,
    /// 每次打开/取消都推进，旧预加载不再写入缓存。
    epoch: AtomicU64,
    /// 当前显示图片，避免插入预加载时把它挤出去。
    current_image: Mutex<String>,
    /// 在途预加载，避免同一张图重复入队。
    pending: Mutex<HashSet<(String, FileStamp)>>,
}

/// 预览服务的句柄：命令侧只提交请求与查询缓存，解码在独立线程完成。
#[derive(Clone)]
pub struct PreviewService {
    tx: mpsc::Sender<Job>,
    shared: Arc<Shared>,
}

impl PreviewService {
    pub fn new(app: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel::<Job>();
        let shared = Arc::new(Shared {
            app,
            store: Mutex::new(PreviewStore::new()),
            current: AtomicU64::new(0),
            epoch: AtomicU64::new(0),
            current_image: Mutex::new(String::new()),
            pending: Mutex::new(HashSet::new()),
        });
        let worker_shared = shared.clone();

        thread::Builder::new()
            .name("iv-preview".to_owned())
            .spawn(move || worker(rx, worker_shared))
            .expect("无法启动预览解码线程");

        Self { tx, shared }
    }

    /// 提交打开请求；返回的 requestId 是前端过滤过期事件的依据。
    ///
    /// 命中预加载缓存时立即发送 `preview-ready`，不重复解码。
    pub fn request(&self, image_id: String, path: PathBuf, caps: Caps) -> PreviewRequestDto {
        let request_id = self.shared.current.fetch_add(1, Ordering::Relaxed) + 1;
        let epoch = self.shared.epoch.fetch_add(1, Ordering::Relaxed) + 1;
        *lock(&self.shared.current_image) = image_id.clone();

        let stamp = file_stamp(&path);
        let revision = {
            let mut store = lock(&self.shared.store);
            if let Some(entry) = store.find(&image_id, &stamp) {
                let revision = entry.revision;
                let payload = entry.ready_payload(request_id);
                drop(store);
                self.emit_ready(payload);
                return PreviewRequestDto {
                    request_id,
                    image_id,
                    revision,
                };
            }
            store.reserve_revision()
        };

        let job = Job {
            kind: JobKind::Open,
            request_id,
            epoch,
            image_id: image_id.clone(),
            revision,
            stamp,
            caps,
        };
        if self.tx.send(job).is_err() {
            log::warn!(
                target: "image_view::preview",
                "预览解码线程已退出，无法打开 {image_id}"
            );
        }

        PreviewRequestDto {
            request_id,
            image_id,
            revision,
        }
    }

    /// 后台预加载一张相邻图片；命中缓存或在途时直接跳过。
    pub fn preload(&self, image_id: String, path: PathBuf, caps: Caps) {
        let stamp = file_stamp(&path);
        if lock(&self.shared.store).contains(&image_id, &stamp) {
            return;
        }

        let pending_key = (image_id.clone(), stamp.clone());
        if !lock(&self.shared.pending).insert(pending_key.clone()) {
            return;
        }

        let revision = lock(&self.shared.store).reserve_revision();
        let job = Job {
            kind: JobKind::Preload,
            request_id: 0,
            epoch: self.shared.epoch.load(Ordering::Relaxed),
            image_id,
            revision,
            stamp,
            caps,
        };
        if self.tx.send(job).is_err() {
            lock(&self.shared.pending).remove(&pending_key);
        }
    }

    /// 推进代号，让所有在途结果过期；预加载也会被丢弃。
    pub fn cancel(&self) -> u64 {
        self.shared.epoch.fetch_add(1, Ordering::Relaxed);
        self.shared.current.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn get(&self, image_id: &str, revision: u64) -> Option<Arc<PreviewEntry>> {
        lock(&self.shared.store).get(image_id, revision)
    }

    pub fn clear(&self) {
        lock(&self.shared.store).clear();
        lock(&self.shared.pending).clear();
    }

    fn emit_ready(&self, payload: PreviewReadyDto) {
        emit_ready(&self.shared, payload);
    }
}

fn worker(rx: mpsc::Receiver<Job>, shared: Arc<Shared>) {
    while let Ok(first) = rx.recv() {
        let mut batch = vec![first];
        while let Ok(next) = rx.try_recv() {
            batch.push(next);
        }

        // Open 优先：先执行批次里最新的打开请求，再处理其余预加载。
        if let Some(index) = batch.iter().rposition(|job| job.kind == JobKind::Open) {
            let job = batch.remove(index);
            run_open(&shared, job);
        }

        let mut seen = HashSet::new();
        for job in batch {
            if job.kind != JobKind::Preload || !seen.insert((job.image_id.clone(), job.epoch)) {
                continue;
            }
            run_preload(&shared, job);
        }
    }
}

fn run_open(shared: &Shared, job: Job) {
    if shared.current.load(Ordering::Relaxed) != job.request_id {
        return;
    }

    // 预加载可能刚好在入队后完成：重新查一次缓存，避免重复解码。
    if let Some(entry) = lock(&shared.store).find(&job.image_id, &file_stamp(&job.stamp.path)) {
        let payload = entry.ready_payload(job.request_id);
        emit_ready(shared, payload);
        return;
    }

    match decode_entry(&job, Purpose::Open) {
        Ok(entry) => {
            if shared.current.load(Ordering::Relaxed) != job.request_id {
                return;
            }
            let payload = entry.ready_payload(job.request_id);
            let current = lock(&shared.current_image).clone();
            lock(&shared.store).insert(entry, Some(current.as_str()));
            emit_ready(shared, payload);
        }
        Err(error) => {
            if shared.current.load(Ordering::Relaxed) != job.request_id {
                return;
            }
            let payload = failed_payload(job.request_id, &job.image_id, &error);
            emit_failed(shared, payload);
        }
    }
}

fn run_preload(shared: &Shared, job: Job) {
    let pending_key = (job.image_id.clone(), job.stamp.clone());

    if shared.epoch.load(Ordering::Relaxed) != job.epoch {
        lock(&shared.pending).remove(&pending_key);
        return;
    }

    let stamp = file_stamp(&job.stamp.path);
    if lock(&shared.store).contains(&job.image_id, &stamp) {
        lock(&shared.pending).remove(&pending_key);
        return;
    }

    let decoded = decode_entry(&job, Purpose::Preload);
    lock(&shared.pending).remove(&pending_key);

    // 用户已经切换图片，或打开请求先完成：预加载结果直接丢弃。
    if shared.epoch.load(Ordering::Relaxed) != job.epoch {
        return;
    }
    let Ok(entry) = decoded else {
        return;
    };
    if lock(&shared.store).contains(&job.image_id, &stamp) {
        return;
    }

    let current = lock(&shared.current_image).clone();
    lock(&shared.store).insert(entry, Some(current.as_str()));
}

fn decode_entry(job: &Job, purpose: Purpose) -> Result<Arc<PreviewEntry>, LoadError> {
    let path = job.stamp.path.as_path();
    let decoded = decode_image(path, &job.caps, purpose)?;
    let stamp = file_stamp(path);
    let modified_at = std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64);
    Ok(Arc::new(PreviewEntry {
        image_id: job.image_id.clone(),
        revision: job.revision,
        width: decoded.size[0] as u32,
        height: decoded.size[1] as u32,
        rgba: Arc::from(decoded.rgba),
        png: OnceLock::new(),
        file_size: stamp.len,
        stamp,
        source_width: decoded.source_size[0] as u32,
        source_height: decoded.source_size[1] as u32,
        source_depth: decoded.source_depth,
        color_converted: decoded.color.converted,
        color_source: decoded.color.source_label().map(str::to_owned),
        modified_at,
    }))
}

fn emit_ready(shared: &Shared, payload: PreviewReadyDto) {
    if let Err(error) = shared.app.emit(PREVIEW_READY_EVENT, payload) {
        log::warn!(
            target: "image_view::preview",
            "无法发送预览事件：{error}"
        );
    }
}

fn emit_failed(shared: &Shared, payload: PreviewFailedDto) {
    if let Err(error) = shared.app.emit(PREVIEW_FAILED_EVENT, payload) {
        log::warn!(
            target: "image_view::preview",
            "无法发送预览失败事件：{error}"
        );
    }
}

fn failed_payload(request_id: u64, image_id: &str, error: &LoadError) -> PreviewFailedDto {
    let code = match error {
        LoadError::Io(_) => "io",
        LoadError::Decode(_) => "decode",
        LoadError::TooLarge { .. } => "too_large",
        LoadError::OutOfMemory => "out_of_memory",
        LoadError::PreloadSkipped { .. } => "preload_skipped",
        #[cfg(feature = "avif")]
        LoadError::Avif(_) => "decode",
        #[cfg(feature = "heic")]
        LoadError::Heic(_) => "decode",
        #[cfg(feature = "svg")]
        LoadError::Svg(_) => "decode",
        LoadError::MissingDecoder { .. } => "missing_decoder",
        #[cfg(feature = "svg")]
        LoadError::SvgzUnsupported => "unsupported",
    };
    PreviewFailedDto {
        request_id,
        image_id: image_id.to_owned(),
        code,
        message: error.to_string(),
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // 解码线程不 panic；即使 poison 也继续使用缓存，避免整个查看器不可用。
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 授权句柄必须是后端生成的 UUID；协议层拒绝任意字符串。
pub fn is_valid_image_id(value: &str) -> bool {
    Uuid::parse_str(value).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp(name: &str, modified: u64) -> FileStamp {
        FileStamp {
            path: PathBuf::from(name),
            modified,
            len: 1,
        }
    }

    fn entry(id: &str, revision: u64, stamp: FileStamp) -> Arc<PreviewEntry> {
        Arc::new(PreviewEntry {
            image_id: id.to_owned(),
            revision,
            width: 1,
            height: 1,
            rgba: Arc::from(&[0u8, 0, 0, 255][..]),
            png: OnceLock::new(),
            stamp,
            source_width: 1,
            source_height: 1,
            source_depth: 8,
            color_converted: false,
            color_source: None,
            file_size: 4,
            modified_at: None,
        })
    }

    #[test]
    fn revisions_increase_globally() {
        let mut store = PreviewStore::new();
        assert_eq!(store.reserve_revision(), 1);
        assert_eq!(store.reserve_revision(), 2);
    }

    #[test]
    fn cache_hit_requires_the_same_file_stamp() {
        let mut store = PreviewStore::new();
        store.insert(entry("id-a", 7, stamp("a.jpg", 100)), None);

        assert_eq!(
            store.find("id-a", &stamp("a.jpg", 100)).map(|e| e.revision),
            Some(7)
        );
        assert!(store.find("id-a", &stamp("a.jpg", 101)).is_none());
        assert!(store.find("id-b", &stamp("a.jpg", 100)).is_none());
    }

    #[test]
    fn cache_evicts_oldest_and_keeps_current_image() {
        let mut store = PreviewStore::new();
        for index in 0..MAX_CACHED_PREVIEWS {
            store.insert(
                entry(&format!("id-{index}"), 1, stamp("a", 1)),
                Some("id-0"),
            );
        }

        // 超出条目上限时，最旧的 id-1 被淘汰，但当前显示的 id-0 保留。
        store.insert(entry("id-next", 1, stamp("a", 1)), Some("id-0"));

        assert!(store.get("id-0", 1).is_some());
        assert!(store.get("id-1", 1).is_none());
        assert!(store.get("id-next", 1).is_some());
    }

    #[test]
    fn inserting_new_revision_replaces_older_one() {
        let mut store = PreviewStore::new();
        store.insert(entry("id-a", 1, stamp("a", 1)), None);
        store.insert(entry("id-a", 2, stamp("a", 1)), None);

        assert!(store.get("id-a", 1).is_none());
        assert!(store.get("id-a", 2).is_some());
    }

    #[test]
    fn pixel_budget_evicts_non_current_entries() {
        let mut store = PreviewStore::new();

        let mut first = entry("first", 1, stamp("a", 1));
        Arc::get_mut(&mut first).expect("unique").width = 6_000;
        Arc::get_mut(&mut first).expect("unique").height = 5_000;
        store.insert(first, None);

        let mut second = entry("second", 1, stamp("b", 1));
        Arc::get_mut(&mut second).expect("unique").width = 6_000;
        Arc::get_mut(&mut second).expect("unique").height = 5_000;
        store.insert(second, None);

        // 60 MP 再加 20 MP 超过 72 MP 预算；淘汰第二张而不是当前显示的 first。
        let mut preload = entry("preload", 1, stamp("c", 1));
        Arc::get_mut(&mut preload).expect("unique").width = 5_000;
        Arc::get_mut(&mut preload).expect("unique").height = 4_000;
        store.insert(preload, Some("first"));

        assert!(store.get("first", 1).is_some());
        assert!(store.get("second", 1).is_none());
        assert!(store.get("preload", 1).is_some());
    }

    #[test]
    fn image_ids_must_be_uuids() {
        assert!(is_valid_image_id(&Uuid::new_v4().to_string()));
        assert!(!is_valid_image_id("../secret"));
        assert!(!is_valid_image_id("C:\\photo.jpg"));
    }
}
