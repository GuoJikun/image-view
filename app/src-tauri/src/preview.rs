//! 预览资源：后台解码、按文件身份缓存像素、通过事件通知前端。
//!
//! - `Open` 优先：队列里只执行最新的打开请求，预加载会让路。
//! - `Preload` 只在后台填充缓存，不发送事件；超过 30 MP 的图片直接跳过。
//! - 缓存键是 `image_id + 路径 + 修改时间 + 文件大小`，预加载结果可被后续打开复用。
//! - 动画（APNG / GIF / WebP）分两段：首帧随预览先显示，完整帧由独立线程解码后
//!   追加到同一条目并发 `image:animation-ready`；帧随条目一起淘汰。
//!
//! 像素只保存在 Rust 侧；事件 payload 只有尺寸/元数据与资源句柄。

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, mpsc};
use std::thread;
use std::time::{Instant, UNIX_EPOCH};

use iv_image_decode::{
    AnimationOutcome, Caps, ColorTarget, DecodedAnimation, DegradeReason, LoadError, Pixels,
    Purpose, decode_animation_to, decode_image_to,
};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::dto::{
    AnimationReadyDto, DegradedDto, PreviewFailedDto, PreviewReadyDto, PreviewRequestDto,
};

pub const PREVIEW_READY_EVENT: &str = "image:preview-ready";
pub const PREVIEW_FAILED_EVENT: &str = "image:preview-failed";
pub const ANIMATION_READY_EVENT: &str = "image:animation-ready";
pub const DEGRADED_EVENT: &str = "image:degraded";

/// 当前显示目标空间；前端探测完屏幕与画布能力后通过 `set_color_target` 写入。
///
/// 0 = sRGB，1 = Display P3。解码线程与命令线程都读这个值，条目上另存一份用于
/// 判断缓存是否仍然有效（目标变了必须重新解码，不能只重编码 PNG）。
static COLOR_TARGET: AtomicU8 = AtomicU8::new(0);

/// 读取当前目标空间。
pub fn current_color_target() -> ColorTarget {
    match COLOR_TARGET.load(Ordering::Relaxed) {
        1 => ColorTarget::DisplayP3,
        _ => ColorTarget::Srgb,
    }
}

/// 写入目标空间；返回是否真的发生了变化。
fn store_color_target(target: ColorTarget) -> bool {
    let raw = match target {
        ColorTarget::Srgb => 0u8,
        ColorTarget::DisplayP3 => 1u8,
    };
    COLOR_TARGET.swap(raw, Ordering::Relaxed) != raw
}

/// 缓存最近预览：当前 + 前后各一张 + 最多 4 张对比图。
const MAX_CACHED_PREVIEWS: usize = 6;
/// 像素预算：约 3 张 24 MP（普通浏览），或 4 张全像素对比图 + 当前图。
const MAX_CACHED_PIXELS: usize = 200_000_000;
/// 批量对比预览的解码等待上限；超时后该槽位按失败回退缩略图。
const COMPARE_BATCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

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
    rgba: Pixels,
    png: OnceLock<Arc<[u8]>>,
    /// 动画帧；静态图与「尚未解码完/超预算」的动画都是 `None`。
    animation: Mutex<Option<Arc<Animation>>>,
    stamp: FileStamp,
    source_width: u32,
    source_height: u32,
    source_depth: u8,
    color_converted: bool,
    color_source: Option<String>,
    /// 解码时的目标空间；与当前目标不一致时缓存视为失效。
    color_target: ColorTarget,
    /// 像素实际所在的空间：转换成功时等于 `color_target`，回退时像素仍是原样，
    /// 按 sRGB 处理。PNG 的 `iCCP` 必须标它，标错会让浏览器按错误的前提换算。
    png_space: ColorTarget,
    file_size: u64,
    modified_at: Option<i64>,
}

/// 动画的一帧：RGBA 在协议首次请求时编码为 PNG 并缓存。
struct AnimationFrameEntry {
    rgba: Pixels,
    png: OnceLock<Arc<[u8]>>,
}

/// 一个动画：帧随所属预览条目一起缓存，切换图片时由 `drop_animations_except` 释放。
pub struct Animation {
    loops: Option<u32>,
    delays: Vec<u32>,
    frames: Vec<AnimationFrameEntry>,
    width: u32,
    height: u32,
    /// 帧像素实际所在的空间，规则与 `PreviewEntry::png_space` 一致。
    png_space: ColorTarget,
}

impl Animation {
    fn new(decoded: DecodedAnimation, png_space: ColorTarget) -> Self {
        let width = decoded.size[0] as u32;
        let height = decoded.size[1] as u32;
        Self {
            png_space,
            loops: decoded.loops,
            delays: decoded.frames.iter().map(|frame| frame.delay_ms).collect(),
            frames: decoded
                .frames
                .into_iter()
                .map(|frame| AnimationFrameEntry {
                    rgba: frame.rgba,
                    png: OnceLock::new(),
                })
                .collect(),
            width,
            height,
        }
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// 第 `index` 帧的 PNG；越界或编码失败返回 `None`。
    pub fn frame_png(&self, index: usize) -> Option<Arc<[u8]>> {
        let frame = self.frames.get(index)?;
        let bytes = frame
            .png
            .get_or_init(|| {
                encode_png(
                    &frame.rgba,
                    self.width,
                    self.height,
                    self.png_space,
                    "动画帧",
                )
            })
            .clone();
        (!bytes.is_empty()).then_some(bytes)
    }
}

/// RGBA → PNG，位深跟随 `Pixels` 变体；失败时返回空字节，由调用方转成 500 / 404。
///
/// 非 sRGB 目标必须写入对应的 `iCCP` 块：像素已经按该空间编码，画布再按标签解释
/// 才是恒等；画布不支持该空间时，浏览器也会据此正确降级到 sRGB。
fn encode_png(
    pixels: &Pixels,
    width: u32,
    height: u32,
    color_target: ColorTarget,
    context: &str,
) -> Arc<[u8]> {
    use image::ImageEncoder as _;

    let color_type = match pixels {
        Pixels::Rgba8(_) => image::ExtendedColorType::Rgba8,
        Pixels::Rgba16(_) => image::ExtendedColorType::Rgba16,
    };
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::png::PngEncoder::new_with_quality(
        &mut bytes,
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::Adaptive,
    );
    if let Some(icc) = color_target.icc()
        && let Err(error) = encoder.set_icc_profile(icc.to_vec())
    {
        log::debug!(
            target: "image_view::preview",
            "{context} 写入 ICC profile 失败，按无标签 PNG 输出：{error}"
        );
    }
    match encoder.write_image(pixels.as_bytes(), width, height, color_type) {
        Ok(()) => Arc::from(bytes),
        Err(error) => {
            log::error!(
                target: "image_view::preview",
                "{context} PNG 编码失败（{width}×{height}）：{error}"
            );
            Arc::from(&[][..])
        }
    }
}

impl PreviewEntry {
    /// 编码为 WebView 可直接解码的 PNG。失败时返回空字节，由协议层转成 500。
    pub fn png(&self) -> Arc<[u8]> {
        self.png
            .get_or_init(|| encode_png(&self.rgba, self.width, self.height, self.png_space, "预览"))
            .clone()
    }

    /// 已解码完成的动画；未解码、静态图或超出预算时为 `None`。
    pub fn animation(&self) -> Option<Arc<Animation>> {
        lock(&self.animation).clone()
    }

    fn set_animation(&self, animation: Arc<Animation>) {
        *lock(&self.animation) = Some(animation);
    }

    fn clear_animation(&self) {
        *lock(&self.animation) = None;
    }

    /// 第 `index` 帧的 PNG；非动画或越界返回 `None`。
    pub fn frame_png(&self, index: usize) -> Option<Arc<[u8]>> {
        self.animation()
            .and_then(|animation| animation.frame_png(index))
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
            color_target: self.color_target.id(),
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

    /// 按文件身份与目标空间查找；命中后刷新最近使用顺序。
    ///
    /// 目标空间必须一起匹配：条目里的像素和 PNG 标签都按当时的目标生成，
    /// 换了目标只能重新解码，不能复用。
    fn find(
        &mut self,
        image_id: &str,
        stamp: &FileStamp,
        target: ColorTarget,
    ) -> Option<Arc<PreviewEntry>> {
        let index = self.entries.iter().position(|entry| {
            entry.image_id == image_id && &entry.stamp == stamp && entry.color_target == target
        })?;
        let entry = self.entries.remove(index)?;
        self.entries.push_back(entry.clone());
        Some(entry)
    }

    fn contains(&self, image_id: &str, stamp: &FileStamp, target: ColorTarget) -> bool {
        self.entries.iter().any(|entry| {
            entry.image_id == image_id && &entry.stamp == stamp && entry.color_target == target
        })
    }

    /// 只保留指定目标的条目；目标空间切换后其余条目必须丢弃。
    fn retain_target(&mut self, target: ColorTarget) {
        self.entries.retain(|entry| entry.color_target == target);
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

    /// 同一图片只保留最新 revision；`protected` 为不可淘汰的图片集合
    /// （当前显示图片 + 对比会话的 2~4 张）。
    fn insert(&mut self, entry: Arc<PreviewEntry>, protected: &[String]) {
        self.entries
            .retain(|candidate| candidate.image_id != entry.image_id);
        self.entries.push_back(entry);

        let mut pixels: usize = self.entries.iter().map(|entry| entry.pixels()).sum();
        let mut index = 0;
        while (self.entries.len() > MAX_CACHED_PREVIEWS || pixels > MAX_CACHED_PIXELS)
            && index < self.entries.len()
        {
            if protected.contains(&self.entries[index].image_id) {
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

    /// 只保留当前图片的动画帧：切图后旧动画立即释放，不参与主图缓存预算。
    fn drop_animations_except(&mut self, image_id: &str) {
        for entry in &self.entries {
            if entry.image_id != image_id {
                entry.clear_animation();
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JobKind {
    Open,
    Preload,
    /// 全像素对比的批量请求：不参与 latest-wins，带完成回执。
    Compare,
}

struct Job {
    kind: JobKind,
    /// 打开请求的代号；预加载与对比为 0。
    request_id: u64,
    /// 导航世代；用户切换图片后旧的预加载直接丢弃。对比为 0，不受导航影响。
    epoch: u64,
    image_id: String,
    revision: u64,
    stamp: FileStamp,
    caps: Caps,
    /// 对比批量请求的完成回执（槽位下标, revision；失败时 revision 为 0）。
    compare_done: Option<mpsc::Sender<(usize, u64)>>,
    /// 对比批量请求在结果数组中的槽位下标。
    compare_index: usize,
}

/// 动画解码任务：只对当前打开的图片执行，过期任务直接丢弃。
struct AnimationJob {
    request_id: u64,
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
    /// 对比会话锁定的图片；这些条目的预览不能被淘汰。
    compare_ids: Mutex<Vec<String>>,
}

impl Shared {
    /// 当前显示图片 + 对比图片，作为淘汰时的保护集合。
    fn protected_ids(&self) -> Vec<String> {
        let mut ids = self
            .compare_ids
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let current = self
            .current_image
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if !current.is_empty() && !ids.contains(&current) {
            ids.push(current);
        }
        ids
    }
}

/// 预览服务的句柄：命令侧只提交请求与查询缓存，解码在独立线程完成。
#[derive(Clone)]
pub struct PreviewService {
    tx: mpsc::Sender<Job>,
    animation_tx: mpsc::Sender<AnimationJob>,
    shared: Arc<Shared>,
}

impl PreviewService {
    pub fn new(app: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel::<Job>();
        let (animation_tx, animation_rx) = mpsc::channel::<AnimationJob>();
        let shared = Arc::new(Shared {
            app,
            store: Mutex::new(PreviewStore::new()),
            current: AtomicU64::new(0),
            epoch: AtomicU64::new(0),
            current_image: Mutex::new(String::new()),
            pending: Mutex::new(HashSet::new()),
            compare_ids: Mutex::new(Vec::new()),
        });

        let worker_shared = shared.clone();
        thread::Builder::new()
            .name("iv-preview".to_owned())
            .spawn(move || worker(rx, worker_shared))
            .expect("无法启动预览解码线程");

        let animation_shared = shared.clone();
        thread::Builder::new()
            .name("iv-animation".to_owned())
            .spawn(move || animation_worker(animation_rx, animation_shared))
            .expect("无法启动动画解码线程");

        Self {
            tx,
            animation_tx,
            shared,
        }
    }

    /// 提交打开请求；返回的 requestId 是前端过滤过期事件的依据。
    ///
    /// 命中预加载缓存时立即发送 `preview-ready`，不重复解码。
    pub fn request(&self, image_id: String, path: PathBuf, caps: Caps) -> PreviewRequestDto {
        let request_id = self.shared.current.fetch_add(1, Ordering::Relaxed) + 1;
        let epoch = self.shared.epoch.fetch_add(1, Ordering::Relaxed) + 1;
        *lock(&self.shared.current_image) = image_id.clone();
        // 动画帧只在当前图片上占用内存；切图后旧帧（含静态图留下的）立即释放。
        lock(&self.shared.store).drop_animations_except(&image_id);

        let stamp = file_stamp(&path);
        let target = current_color_target();
        let cached = {
            let mut store = lock(&self.shared.store);
            store.find(&image_id, &stamp, target).map(|entry| {
                (
                    entry.revision,
                    entry.ready_payload(request_id),
                    entry.animation(),
                )
            })
        };

        if let Some((revision, payload, animation)) = cached {
            self.emit_ready(payload);
            match animation {
                Some(animation) => {
                    let info = AnimationReadyDto {
                        request_id,
                        image_id: image_id.clone(),
                        revision,
                        frame_count: animation.frame_count() as u32,
                        loops: animation.loops,
                        delays: animation.delays.clone(),
                    };
                    self.emit_animation_ready(info);
                }
                // 动画帧可能在切图后被释放；缓存命中的图片仍需要重新解码动画。
                None => self.request_animation(&image_id, revision, &stamp, caps, request_id),
            }
            return PreviewRequestDto {
                request_id,
                image_id,
                revision,
            };
        }

        let revision = lock(&self.shared.store).reserve_revision();
        let job = Job {
            kind: JobKind::Open,
            request_id,
            epoch,
            image_id: image_id.clone(),
            revision,
            stamp: stamp.clone(),
            caps: caps.clone(),
            compare_done: None,
            compare_index: 0,
        };
        if self.tx.send(job).is_err() {
            log::warn!(
                target: "image_view::preview",
                "预览解码线程已退出，无法打开 {image_id}"
            );
        }
        self.request_animation(&image_id, revision, &stamp, caps, request_id);

        PreviewRequestDto {
            request_id,
            image_id,
            revision,
        }
    }

    /// 排队解码动画帧；非动画扩展名直接跳过，避免无谓的文件读取。
    fn request_animation(
        &self,
        image_id: &str,
        revision: u64,
        stamp: &FileStamp,
        caps: Caps,
        request_id: u64,
    ) {
        if !is_animatable(&stamp.path) {
            return;
        }
        let job = AnimationJob {
            request_id,
            image_id: image_id.to_owned(),
            revision,
            stamp: stamp.clone(),
            caps,
        };
        if self.animation_tx.send(job).is_err() {
            log::warn!(
                target: "image_view::preview",
                "动画解码线程已退出，跳过 {image_id} 的动画"
            );
        }
    }

    /// 后台预加载一张相邻图片；命中缓存或在途时直接跳过。
    pub fn preload(&self, image_id: String, path: PathBuf, caps: Caps) {
        let stamp = file_stamp(&path);
        let target = current_color_target();
        if lock(&self.shared.store).contains(&image_id, &stamp, target) {
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
            compare_done: None,
            compare_index: 0,
        };
        if self.tx.send(job).is_err() {
            lock(&self.shared.pending).remove(&pending_key);
        }
    }

    /// 为全像素对比批量准备预览：不推进 latest-wins（不影响当前浏览），
    /// 保护全部对比条目不被淘汰，等待解码完成后返回每个 image_id 的 revision。
    ///
    /// 调用方须先确保这些 image_id 的预览不依赖当前导航世代；返回项按入参顺序排列，
    /// 解码失败/超时项为 `None`，前端据此回退缩略图。
    /// 结果汇总逻辑在 [`collect_compare_results`]（纯函数，可单测）。
    pub fn request_compare_batch(
        &self,
        items: Vec<(String, PathBuf)>,
    ) -> Vec<Option<(String, u64)>> {
        let ids: Vec<String> = items.iter().map(|(id, _)| id.clone()).collect();
        *lock(&self.shared.compare_ids) = ids.clone();

        let (done_tx, done_rx) = mpsc::channel::<(usize, u64)>();
        let target = current_color_target();
        let mut cached: Vec<Option<u64>> = Vec::with_capacity(items.len());
        let mut pending = 0usize;

        for (index, (image_id, path)) in items.into_iter().enumerate() {
            let stamp = file_stamp(&path);
            // 命中缓存直接复用 revision（find 会刷新 LRU 顺序）。
            let hit = lock(&self.shared.store)
                .find(&image_id, &stamp, target)
                .map(|entry| entry.revision);
            cached.push(hit);
            if hit.is_some() {
                continue;
            }

            let revision = lock(&self.shared.store).reserve_revision();
            let job = Job {
                kind: JobKind::Compare,
                request_id: 0,
                epoch: 0,
                image_id,
                revision,
                stamp,
                caps: Caps::default(),
                compare_done: Some(done_tx.clone()),
                compare_index: index,
            };
            if self.tx.send(job).is_ok() {
                pending += 1;
            } else {
                log::warn!(target: "image_view::preview", "对比预览解码线程已退出：{index}");
            }
        }
        drop(done_tx);

        // 等待解码完成回执（含失败项 revision=0），交给纯函数汇总结果。
        let mut receipts = Vec::with_capacity(pending);
        let deadline = std::time::Instant::now() + COMPARE_BATCH_TIMEOUT;
        while receipts.len() < pending {
            let wait = deadline.saturating_duration_since(std::time::Instant::now());
            if wait.is_zero() {
                log::warn!(target: "image_view::preview", "对比预览批量等待超时，仍有 {} 张未完成", pending - receipts.len());
                break;
            }
            match done_rx.recv_timeout(wait) {
                Ok(receipt) => receipts.push(receipt),
                Err(_) => break,
            }
        }
        collect_compare_results(&ids, &cached, &receipts)
    }

    /// 退出对比时解除保护，让这些条目回到正常淘汰策略。
    pub fn clear_compare(&self) {
        lock(&self.shared.compare_ids).clear();
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

    /// 切换目标空间；返回是否真的发生变化。
    ///
    /// 变化时丢弃其他目标的缓存条目并推进代号：在途的旧目标解码结果不再写入缓存。
    /// 前端收到 `true` 后应重新请求当前图片，新条目会分配新的 revision。
    pub fn set_color_target(&self, target: ColorTarget) -> bool {
        if !store_color_target(target) {
            return false;
        }
        lock(&self.shared.store).retain_target(target);
        lock(&self.shared.pending).clear();
        lock(&self.shared.compare_ids).clear();
        self.shared.epoch.fetch_add(1, Ordering::Relaxed);
        log::debug!(
            target: "image_view::preview",
            "色彩目标切换为 {}，已丢弃旧缓存",
            target.id()
        );
        true
    }

    fn emit_ready(&self, payload: PreviewReadyDto) {
        emit_ready(&self.shared, payload);
    }

    fn emit_animation_ready(&self, payload: AnimationReadyDto) {
        emit_animation_ready(&self.shared, payload);
    }
}

/// 只有 APNG / GIF / WebP 可能带多帧；扩展名过滤避免为普通图片排队动画任务。
///
/// `.png` 大多不是动画，真正的判断由解码器读 `acTL` 块完成，重复开销只是一次
/// 头部解析。
fn is_animatable(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("png")
                || extension.eq_ignore_ascii_case("gif")
                || extension.eq_ignore_ascii_case("webp")
        })
}

fn animation_worker(rx: mpsc::Receiver<AnimationJob>, shared: Arc<Shared>) {
    while let Ok(first) = rx.recv() {
        // 只保留最新的动画任务：快速翻页时旧动画没有必要再解码。
        let mut job = first;
        while let Ok(next) = rx.try_recv() {
            job = next;
        }
        run_animation(&shared, job);
    }
}

fn run_animation(shared: &Shared, job: AnimationJob) {
    // 用户已经切换到别的图片：连首帧都不再需要这次结果。
    if shared.current.load(Ordering::Relaxed) != job.request_id {
        return;
    }

    let started = Instant::now();
    let target = current_color_target();
    let decoded = decode_animation_to(&job.stamp.path, &job.caps, target);
    if shared.current.load(Ordering::Relaxed) != job.request_id {
        return;
    }

    match decoded {
        Ok(AnimationOutcome::Ready(decoded)) => {
            let png_space = pixel_space(decoded.color.converted, decoded.color.target);
            let animation = Arc::new(Animation::new(decoded, png_space));
            let payload = AnimationReadyDto {
                request_id: job.request_id,
                image_id: job.image_id.clone(),
                revision: job.revision,
                frame_count: animation.frame_count() as u32,
                loops: animation.loops,
                delays: animation.delays.clone(),
            };

            let entry = lock(&shared.store).get(&job.image_id, job.revision);
            let Some(entry) = entry else {
                // 条目已被淘汰（切目录/删除）时直接丢弃。
                return;
            };
            entry.set_animation(animation);
            // 动画帧只保留给当前图片，旧动画随切图释放。
            lock(&shared.store).drop_animations_except(&job.image_id);

            log::debug!(
                target: "image_view::preview",
                "动画解码完成：{}（{} 帧 / {} ms）",
                job.stamp.path.display(),
                payload.frame_count,
                started.elapsed().as_millis()
            );
            emit_animation_ready(shared, payload);
        }
        // 本来就是静态图（单帧 / 非动画格式）：正常情况，保持静默。
        Ok(AnimationOutcome::NotAnimated) => log::debug!(
            target: "image_view::preview",
            "{} 不是多帧动画，按静态图显示",
            job.stamp.path.display()
        ),
        // 本应能播却被降级：发事件让界面提示用户。
        Ok(AnimationOutcome::Degraded(reason)) => {
            let (code, detail) = degraded_code_and_detail(&reason);
            log::debug!(
                target: "image_view::preview",
                "{} 动画降级（{code}）：{detail}",
                job.stamp.path.display()
            );
            emit_degraded(
                shared,
                DegradedDto {
                    request_id: job.request_id,
                    image_id: job.image_id.clone(),
                    code,
                    detail,
                },
            );
        }
        // 真正的文件/解码错误：也走降级提示，但 code 标为 decodeFailed。
        Err(error) => {
            log::warn!(
                target: "image_view::preview",
                "动画解码失败（{}），按静态首帧显示：{error}",
                job.stamp.path.display()
            );
            emit_degraded(
                shared,
                DegradedDto {
                    request_id: job.request_id,
                    image_id: job.image_id.clone(),
                    code: "decodeFailed",
                    detail: error.to_string(),
                },
            );
        }
    }
}

/// 把降级原因映射为稳定错误码 + 日志细节；文案由前端按 code 本地化。
fn degraded_code_and_detail(reason: &DegradeReason) -> (&'static str, String) {
    match reason {
        DegradeReason::OverBudget {
            frames,
            pixels,
            max_frames,
            max_pixels,
        } => (
            "overBudget",
            format!(
                "{} 帧 / {} 像素，超出上限 {} 帧 / {} 像素",
                frames, pixels, max_frames, max_pixels
            ),
        ),
        DegradeReason::Unsupported16Bit => {
            ("unsupported16Bit", "16-bit APNG 不支持逐帧动画".into())
        }
        DegradeReason::DecodeFailed { message } => ("decodeFailed", message.clone()),
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
            match job.kind {
                JobKind::Compare => run_compare(&shared, job),
                JobKind::Preload if seen.insert((job.image_id.clone(), job.epoch)) => {
                    run_preload(&shared, job)
                }
                _ => {}
            }
        }
    }
}

fn run_open(shared: &Shared, job: Job) {
    if shared.current.load(Ordering::Relaxed) != job.request_id {
        return;
    }

    // 预加载可能刚好在入队后完成：重新查一次缓存，避免重复解码。
    let target = current_color_target();
    if let Some(entry) =
        lock(&shared.store).find(&job.image_id, &file_stamp(&job.stamp.path), target)
    {
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
            let protected = shared.protected_ids();
            lock(&shared.store).insert(entry, &protected);
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
    let target = current_color_target();
    if lock(&shared.store).contains(&job.image_id, &stamp, target) {
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
    if lock(&shared.store).contains(&job.image_id, &stamp, target) {
        return;
    }

    let protected = shared.protected_ids();
    lock(&shared.store).insert(entry, &protected);
}

/// 全像素对比的批量解码：不检查导航代号，完成后写入缓存并发回执。
fn run_compare(shared: &Shared, job: Job) {
    let done = job.compare_done.clone();
    let (index, revision, image_id) = (job.compare_index, job.revision, job.image_id.clone());
    let stamp = file_stamp(&job.stamp.path);
    let target = current_color_target();

    // 双重检查：入队后可能已被同一次批处理的其它 job 填入。
    if lock(&shared.store).contains(&image_id, &stamp, target) {
        if let Some(tx) = done {
            let _ = tx.send((index, revision));
        }
        return;
    }

    match decode_entry(&job, Purpose::Preload) {
        Ok(entry) => {
            let protected = shared.protected_ids();
            lock(&shared.store).insert(entry, &protected);
            if let Some(tx) = done {
                let _ = tx.send((index, revision));
            }
        }
        Err(error) => {
            log::warn!(target: "image_view::preview", "对比预览解码失败 {image_id}：{error}");
            if let Some(tx) = done {
                let _ = tx.send((index, 0));
            }
        }
    }
}

/// 汇总全像素批量预览的结果（纯函数：无 IO、无 `AppHandle`，可单测）。
///
/// - `ids`：入参顺序的 image_id。
/// - `cached`：每项命中缓存的 revision；`None` 表示需要解码。
/// - `receipts`：解码回执 `(槽位下标, revision)`，`revision=0` 表示解码失败。
///
/// 命中项用缓存 revision，未命中项按回执下标回填；失败与超时（无回执）保持 `None`，
/// 前端据此回退缩略图。回执顺序无关，越界下标被忽略。
fn collect_compare_results(
    ids: &[String],
    cached: &[Option<u64>],
    receipts: &[(usize, u64)],
) -> Vec<Option<(String, u64)>> {
    ids.iter()
        .enumerate()
        .map(|(index, id)| {
            if let Some(revision) = cached.get(index).copied().flatten() {
                return Some((id.clone(), revision));
            }
            receipts
                .iter()
                .find(|(slot, _)| *slot == index)
                .and_then(|(_, revision)| (*revision > 0).then(|| (id.clone(), *revision)))
        })
        .collect()
}

fn decode_entry(job: &Job, purpose: Purpose) -> Result<Arc<PreviewEntry>, LoadError> {
    let path = job.stamp.path.as_path();
    let target = current_color_target();
    let decoded = decode_image_to(path, &job.caps, purpose, target)?;
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
        rgba: decoded.rgba,
        png: OnceLock::new(),
        animation: Mutex::new(None),
        file_size: stamp.len,
        stamp,
        source_width: decoded.source_size[0] as u32,
        source_height: decoded.source_size[1] as u32,
        source_depth: decoded.source_depth,
        color_converted: decoded.color.converted,
        color_source: decoded.color.source_label().map(str::to_owned),
        color_target: decoded.color.target,
        png_space: pixel_space(decoded.color.converted, decoded.color.target),
        modified_at,
    }))
}

/// 像素实际所在的空间：成功转换时等于目标空间，回退时像素保持原样（按 sRGB 约定）。
fn pixel_space(converted: bool, target: ColorTarget) -> ColorTarget {
    if converted { target } else { ColorTarget::Srgb }
}

fn emit_ready(shared: &Shared, payload: PreviewReadyDto) {
    if let Err(error) = shared.app.emit(PREVIEW_READY_EVENT, payload) {
        log::warn!(
            target: "image_view::preview",
            "无法发送预览事件：{error}"
        );
    }
}

fn emit_animation_ready(shared: &Shared, payload: AnimationReadyDto) {
    if let Err(error) = shared.app.emit(ANIMATION_READY_EVENT, payload) {
        log::warn!(
            target: "image_view::preview",
            "无法发送动画事件：{error}"
        );
    }
}

fn emit_degraded(shared: &Shared, payload: DegradedDto) {
    if let Err(error) = shared.app.emit(DEGRADED_EVENT, payload) {
        log::warn!(
            target: "image_view::preview",
            "无法发送动画降级事件：{error}"
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
            rgba: Pixels::Rgba8(vec![0, 0, 0, 255]),
            png: OnceLock::new(),
            animation: Mutex::new(None),
            stamp,
            source_width: 1,
            source_height: 1,
            source_depth: 8,
            color_converted: false,
            color_source: None,
            color_target: ColorTarget::Srgb,
            png_space: ColorTarget::Srgb,
            file_size: 4,
            modified_at: None,
        })
    }

    /// 两帧动画：`delays` 与帧数对应，帧内容为纯色。
    fn animation(delays: &[u32]) -> Arc<Animation> {
        Arc::new(Animation {
            loops: Some(2),
            delays: delays.to_vec(),
            png_space: ColorTarget::Srgb,
            frames: delays
                .iter()
                .enumerate()
                .map(|(index, _)| AnimationFrameEntry {
                    rgba: Pixels::Rgba8(vec![index as u8, 0, 0, 255]),
                    png: OnceLock::new(),
                })
                .collect(),
            width: 1,
            height: 1,
        })
    }

    #[test]
    fn revisions_increase_globally() {
        let mut store = PreviewStore::new();
        assert_eq!(store.reserve_revision(), 1);
        assert_eq!(store.reserve_revision(), 2);
    }

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn compare_results_all_cached() {
        let result =
            collect_compare_results(&ids(&["a", "b", "c"]), &[Some(10), Some(20), Some(30)], &[]);
        assert_eq!(
            result,
            vec![
                Some(("a".into(), 10)),
                Some(("b".into(), 20)),
                Some(("c".into(), 30)),
            ]
        );
    }

    #[test]
    fn compare_results_fills_pending_from_receipts_regardless_of_order() {
        // b 命中缓存；a、c 解码成功，回执乱序到达；下标必须与入参对齐。
        let result = collect_compare_results(
            &ids(&["a", "b", "c"]),
            &[None, Some(20), None],
            &[(2, 33), (0, 11)],
        );
        assert_eq!(
            result,
            vec![
                Some(("a".into(), 11)),
                Some(("b".into(), 20)),
                Some(("c".into(), 33)),
            ]
        );
    }

    #[test]
    fn compare_results_failed_and_timed_out_slots_stay_none() {
        // a 解码失败（revision=0）；b 超时（无回执）；c 命中缓存。
        let result =
            collect_compare_results(&ids(&["a", "b", "c"]), &[None, None, Some(30)], &[(0, 0)]);
        assert_eq!(result, vec![None, None, Some(("c".into(), 30))]);
    }

    #[test]
    fn compare_results_ignores_out_of_range_receipts() {
        let result = collect_compare_results(&ids(&["a"]), &[None], &[(5, 99), (0, 7)]);
        assert_eq!(result, vec![Some(("a".into(), 7))]);
    }

    #[test]
    fn compare_results_handles_empty_input() {
        assert!(collect_compare_results(&[], &[], &[]).is_empty());
    }

    #[test]
    fn cache_hit_requires_the_same_file_stamp() {
        let mut store = PreviewStore::new();
        store.insert(entry("id-a", 7, stamp("a.jpg", 100)), &[]);

        assert_eq!(
            store
                .find("id-a", &stamp("a.jpg", 100), ColorTarget::Srgb)
                .map(|e| e.revision),
            Some(7)
        );
        assert!(
            store
                .find("id-a", &stamp("a.jpg", 101), ColorTarget::Srgb)
                .is_none()
        );
        assert!(
            store
                .find("id-b", &stamp("a.jpg", 100), ColorTarget::Srgb)
                .is_none()
        );
    }

    #[test]
    fn cache_hit_requires_the_same_color_target() {
        let mut store = PreviewStore::new();
        store.insert(entry("id-a", 7, stamp("a.jpg", 100)), &[]);

        assert!(
            store
                .find("id-a", &stamp("a.jpg", 100), ColorTarget::DisplayP3)
                .is_none(),
            "目标空间不同必须视为未命中，否则会拿到旧空间的像素"
        );
        assert!(
            store
                .find("id-a", &stamp("a.jpg", 100), ColorTarget::Srgb)
                .is_some()
        );
    }

    #[test]
    fn switching_target_drops_entries_from_other_spaces() {
        let mut store = PreviewStore::new();
        store.insert(entry("id-a", 1, stamp("a", 1)), &[]);

        store.retain_target(ColorTarget::DisplayP3);

        assert!(store.get("id-a", 1).is_none(), "sRGB 条目应被丢弃");
    }

    #[test]
    fn cache_evicts_oldest_and_keeps_current_image() {
        let mut store = PreviewStore::new();
        for index in 0..MAX_CACHED_PREVIEWS {
            store.insert(
                entry(&format!("id-{index}"), 1, stamp("a", 1)),
                &["id-0".to_owned()],
            );
        }

        // 超出条目上限时，最旧的 id-1 被淘汰，但当前显示的 id-0 保留。
        store.insert(entry("id-next", 1, stamp("a", 1)), &["id-0".to_owned()]);

        assert!(store.get("id-0", 1).is_some());
        assert!(store.get("id-1", 1).is_none());
        assert!(store.get("id-next", 1).is_some());
    }

    #[test]
    fn inserting_new_revision_replaces_older_one() {
        let mut store = PreviewStore::new();
        store.insert(entry("id-a", 1, stamp("a", 1)), &[]);
        store.insert(entry("id-a", 2, stamp("a", 1)), &[]);

        assert!(store.get("id-a", 1).is_none());
        assert!(store.get("id-a", 2).is_some());
    }

    #[test]
    fn pixel_budget_evicts_non_current_entries() {
        let mut store = PreviewStore::new();

        let mut first = entry("first", 1, stamp("a", 1));
        Arc::get_mut(&mut first).expect("unique").width = 10_000;
        Arc::get_mut(&mut first).expect("unique").height = 8_000;
        store.insert(first, &[]);

        let mut second = entry("second", 1, stamp("b", 1));
        Arc::get_mut(&mut second).expect("unique").width = 10_000;
        Arc::get_mut(&mut second).expect("unique").height = 8_000;
        store.insert(second, &[]);

        // 80 MP + 80 MP + 60 MP 超过 200 MP 预算；淘汰第二张而不是当前显示的 first。
        let mut preload = entry("preload", 1, stamp("c", 1));
        Arc::get_mut(&mut preload).expect("unique").width = 10_000;
        Arc::get_mut(&mut preload).expect("unique").height = 6_000;
        store.insert(preload, &["first".to_owned()]);

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

    #[test]
    fn animation_frames_are_encoded_lazily_and_bounded() {
        let entry = entry("id-a", 1, stamp("a.gif", 1));
        assert!(entry.frame_png(0).is_none(), "未附加动画时没有帧");

        entry.set_animation(animation(&[80, 120]));
        assert_eq!(entry.animation().map(|value| value.frame_count()), Some(2));

        let first = entry.frame_png(0).expect("first frame");
        assert_eq!(&first[..4], &[0x89, b'P', b'N', b'G']);
        // 第二次访问命中缓存：不会重复编码。
        assert!(Arc::ptr_eq(
            &first,
            &entry.frame_png(0).expect("cached frame")
        ));
        assert!(entry.frame_png(2).is_none(), "越界帧返回 None");
    }

    #[test]
    fn switching_images_releases_old_animation_frames() {
        let mut store = PreviewStore::new();
        let previous = entry("id-old", 1, stamp("old.gif", 1));
        previous.set_animation(animation(&[100, 100]));
        store.insert(previous.clone(), &[]);
        store.insert(entry("id-new", 1, stamp("new.jpg", 1)), &[]);

        store.drop_animations_except("id-new");
        assert!(previous.animation().is_none(), "旧动画应随切图释放");
    }

    #[test]
    fn only_apng_gif_and_webp_queue_animation_jobs() {
        assert!(is_animatable(Path::new("a.png")));
        assert!(is_animatable(Path::new("a.gif")));
        assert!(is_animatable(Path::new("a.WEBP")));
        assert!(!is_animatable(Path::new("a.jpg")));
        assert!(!is_animatable(Path::new("no-extension")));
    }
}
