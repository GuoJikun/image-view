//! 预览资源：后台解码、按 revision 缓存像素、通过事件通知前端。
//!
//! 像素只保存在 Rust 侧；事件 payload 只有尺寸/元数据与资源句柄。
//! 快速切换时以 `request_id` 做 latest-wins，过期结果不写入缓存、不发送事件。

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, mpsc};
use std::thread;

use image::ImageEncoder as _;
use iv_image_decode::{Caps, LoadError, Purpose, decode_image};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::dto::{PreviewFailedDto, PreviewReadyDto, PreviewRequestDto};

pub const PREVIEW_READY_EVENT: &str = "image:preview-ready";
pub const PREVIEW_FAILED_EVENT: &str = "image:preview-failed";

/// 缓存最近几张预览，便于前后翻页命中；像素预算由 `Caps` 间接约束。
const MAX_CACHED_PREVIEWS: usize = 4;

/// 一个已解码的预览资源。PNG 编码延迟到协议首次访问时进行并缓存。
pub struct PreviewEntry {
    pub image_id: String,
    pub revision: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
    png: OnceLock<Arc<[u8]>>,
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
}

struct Job {
    request_id: u64,
    image_id: String,
    revision: u64,
    path: PathBuf,
    caps: Caps,
}

struct PreviewStore {
    entries: VecDeque<((String, u64), Arc<PreviewEntry>)>,
    revisions: HashMap<String, u64>,
}

impl PreviewStore {
    fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            revisions: HashMap::new(),
        }
    }

    fn next_revision(&mut self, image_id: &str) -> u64 {
        let revision = self.revisions.entry(image_id.to_owned()).or_insert(0);
        *revision = revision.wrapping_add(1);
        *revision
    }

    fn insert(&mut self, entry: Arc<PreviewEntry>) {
        let key = (entry.image_id.clone(), entry.revision);
        self.entries.retain(|(candidate, _)| candidate != &key);
        self.entries.push_back((key, entry));
        while self.entries.len() > MAX_CACHED_PREVIEWS {
            self.entries.pop_front();
        }
    }

    fn get(&mut self, image_id: &str, revision: u64) -> Option<Arc<PreviewEntry>> {
        let key = (image_id.to_owned(), revision);
        let index = self
            .entries
            .iter()
            .position(|(candidate, _)| *candidate == key)?;
        let entry = self.entries.remove(index)?;
        let value = entry.1.clone();
        self.entries.push_back(entry);
        Some(value)
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

struct Shared {
    store: Mutex<PreviewStore>,
    current: AtomicU64,
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
            store: Mutex::new(PreviewStore::new()),
            current: AtomicU64::new(0),
        });
        let worker_shared = shared.clone();

        thread::Builder::new()
            .name("iv-preview".to_owned())
            .spawn(move || worker(app, rx, worker_shared))
            .expect("无法启动预览解码线程");

        Self { tx, shared }
    }

    /// 提交打开请求；返回的 requestId 是前端过滤过期事件的依据。
    pub fn request(&self, image_id: String, path: PathBuf, caps: Caps) -> PreviewRequestDto {
        let request_id = self.shared.current.fetch_add(1, Ordering::Relaxed) + 1;
        let revision = lock(&self.shared.store).next_revision(&image_id);
        let _ = self.tx.send(Job {
            request_id,
            image_id: image_id.clone(),
            revision,
            path,
            caps,
        });
        PreviewRequestDto {
            request_id,
            image_id,
            revision,
        }
    }

    /// 推进代号，让所有在途结果过期。
    pub fn cancel(&self) -> u64 {
        self.shared.current.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn get(&self, image_id: &str, revision: u64) -> Option<Arc<PreviewEntry>> {
        lock(&self.shared.store).get(image_id, revision)
    }

    pub fn clear(&self) {
        lock(&self.shared.store).clear();
    }
}

fn worker(app: AppHandle, rx: mpsc::Receiver<Job>, shared: Arc<Shared>) {
    while let Ok(first) = rx.recv() {
        // 队列里只保留最新请求；旧请求连解码都不做。
        let mut job = first;
        while let Ok(next) = rx.try_recv() {
            job = next;
        }

        if shared.current.load(Ordering::Relaxed) != job.request_id {
            continue;
        }

        let decoded = decode_image(&job.path, &job.caps, Purpose::Open);
        // 解码期间用户可能已经切换：过期结果既不缓存也不通知前端。
        if shared.current.load(Ordering::Relaxed) != job.request_id {
            continue;
        }

        match decoded {
            Ok(decoded) => {
                let entry = Arc::new(PreviewEntry {
                    image_id: job.image_id.clone(),
                    revision: job.revision,
                    width: decoded.size[0] as u32,
                    height: decoded.size[1] as u32,
                    rgba: Arc::from(decoded.rgba),
                    png: OnceLock::new(),
                });
                lock(&shared.store).insert(entry);

                let metadata = std::fs::metadata(&job.path).ok();
                let payload = PreviewReadyDto {
                    request_id: job.request_id,
                    image_id: job.image_id,
                    revision: job.revision,
                    path: job.path.to_string_lossy().into_owned(),
                    file_name: job
                        .path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    format: crate::dto::format_label(&job.path),
                    width: decoded.size[0] as u32,
                    height: decoded.size[1] as u32,
                    source_width: decoded.source_size[0] as u32,
                    source_height: decoded.source_size[1] as u32,
                    downscaled: decoded.size != decoded.source_size,
                    source_depth: decoded.source_depth,
                    color_converted: decoded.color.converted,
                    color_source: decoded.color.source_label().map(str::to_owned),
                    file_size: metadata.as_ref().map(|meta| meta.len()).unwrap_or(0),
                    modified_at: metadata
                        .and_then(|meta| meta.modified().ok())
                        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|duration| duration.as_secs() as i64),
                };
                if let Err(error) = app.emit(PREVIEW_READY_EVENT, payload) {
                    log::warn!(
                        target: "image_view::preview",
                        "无法发送预览事件：{error}"
                    );
                }
            }
            Err(error) => {
                let payload = failed_payload(job.request_id, &job.image_id, &error);
                if let Err(error) = app.emit(PREVIEW_FAILED_EVENT, payload) {
                    log::warn!(
                        target: "image_view::preview",
                        "无法发送预览失败事件：{error}"
                    );
                }
            }
        }
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

    fn entry(id: &str, revision: u64) -> Arc<PreviewEntry> {
        Arc::new(PreviewEntry {
            image_id: id.to_owned(),
            revision,
            width: 1,
            height: 1,
            rgba: Arc::from(&[0u8, 0, 0, 255][..]),
            png: OnceLock::new(),
        })
    }

    #[test]
    fn revisions_increase_per_image() {
        let mut store = PreviewStore::new();
        assert_eq!(store.next_revision("a"), 1);
        assert_eq!(store.next_revision("a"), 2);
        assert_eq!(store.next_revision("b"), 1);
    }

    #[test]
    fn cache_evicts_oldest_and_keeps_latest_revision() {
        let mut store = PreviewStore::new();
        for index in 0..=MAX_CACHED_PREVIEWS {
            store.insert(entry(&format!("id-{index}"), 1));
        }
        assert_eq!(store.entries.len(), MAX_CACHED_PREVIEWS);
        assert!(store.get("id-0", 1).is_none());
        assert!(store.get("id-1", 1).is_some());

        store.insert(entry("id-1", 2));
        assert!(
            store.get("id-1", 1).is_some(),
            "旧 revision 仍可按 URL 命中"
        );
        assert!(store.get("id-1", 2).is_some());
    }

    #[test]
    fn image_ids_must_be_uuids() {
        assert!(is_valid_image_id(&Uuid::new_v4().to_string()));
        assert!(!is_valid_image_id("../secret"));
        assert!(!is_valid_image_id("C:\\photo.jpg"));
    }
}
