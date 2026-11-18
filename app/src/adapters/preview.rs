//! 后台解码服务：主图预览与缩略图。
//!
//! 主图复用 `iv-image-decode::Loader`（单解码线程 + latest-wins 合并）；
//! 缩略图使用独立的小线程池，避免大量小图拖住当前图片的打开。
//!
//! 缩略图按「源路径 + 修改时间 + 目标边长」写入 `iv_cache::FileCache`
//! 管理的磁盘目录，下一次启动可直接复用，不必重新解码。

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::UNIX_EPOCH;

use iced::widget::image::Handle;
use image::ImageEncoder;
use iv_cache::FileCache;
use iv_image_decode::{
    AnimationOutcome, Caps, DecodedAnimation, DegradeReason, LoadResponse, Loader,
    decode_animation, decode_thumbnail,
};

/// 缩略图最长边（逻辑 64 px，按 2x DPI 预留）。
pub const MAX_THUMB_SIDE: u32 = 128;
const THUMB_WORKERS: usize = 2;

/// 覆盖缓存目录的环境变量，便于在临时目录里验证或排查缓存问题。
pub const CACHE_DIR_ENV: &str = "IMAGE_VIEW_ICED_CACHE_DIR";
/// 磁盘缓存预算：与 `main` 对齐，128 px 缩略图约 5~20 KiB，256 MiB 可覆盖数万张图的目录。
const THUMB_DISK_BYTES: u64 = 256 * 1024 * 1024;

pub struct PreviewService {
    loader: Loader,
}

impl Default for PreviewService {
    fn default() -> Self {
        Self::new()
    }
}

impl PreviewService {
    pub fn new() -> Self {
        Self {
            loader: Loader::new(|| {}),
        }
    }

    /// 请求打开图片；返回的代号用于丢弃过期结果。
    pub fn request(&mut self, path: PathBuf) -> u64 {
        self.loader.request(path, Caps::default())
    }

    /// 后台预加载相邻图片。结果由 `ViewerState::poll` 收进主图缓存，
    /// 不改变当前显示；超过预加载像素上限的大图由 Core 自行放弃。
    pub fn preload(&self, path: PathBuf) {
        // 预加载不带 generation 语义：`ViewerState` 按 `Purpose` 分流，
        // 只判断路径是否已经是当前图片。
        self.loader.preload(path, Caps::default(), 0);
    }

    pub fn cancel(&mut self) {
        self.loader.cancel_pending();
    }

    pub fn try_recv(&self) -> Option<LoadResponse> {
        self.loader.try_recv()
    }
}

/// 解码完成的一帧：像素已转成 iced `Handle`，GUI 侧不持有原始缓冲。
#[derive(Clone, Debug)]
pub struct AnimFrame {
    pub handle: Handle,
    /// 帧间隔（毫秒），已由 Core 钳制到 50~1000。
    pub delay_ms: u32,
}

/// 一次可用的动画：帧句柄、循环次数与尺寸元数据。
/// 派生 `Clone`：帧句柄是 Arc 浅克隆，服务缓存与播放状态可各持一份。
#[derive(Clone, Debug)]
pub struct LoadedAnimation {
    pub frames: Vec<AnimFrame>,
    /// `None` 表示无限循环；`Some(0)` 不会出现（Core 把 0 视为无限）。
    pub loops: Option<u32>,
}

impl LoadedAnimation {
    fn from_decoded(decoded: DecodedAnimation) -> Self {
        let size = [decoded.size[0] as u32, decoded.size[1] as u32];
        let frames = decoded
            .frames
            .into_iter()
            .map(|frame| AnimFrame {
                handle: Handle::from_rgba(size[0], size[1], frame.rgba.into_rgba8()),
                delay_ms: frame.delay_ms,
            })
            .collect();
        Self {
            frames,
            loops: decoded.loops,
        }
    }
}

/// 动画解码的结构化结果，对应 Core 的 `AnimationOutcome` 与 IO/解码错误。
#[derive(Debug)]
pub enum AnimationLoad {
    /// 可用动画；帧句柄随结果交给 `ViewerState`。
    Ready(LoadedAnimation),
    /// 静态图或不足两帧：正常情况，不提示。
    NotAnimated,
    /// 本应能播却退回静态首帧：GUI 据此提示原因。
    Degraded(DegradeReason),
    /// 读取/解码彻底失败（静态主图已单独处理，这里只记录）。
    Failed(String),
}

pub struct AnimationResult {
    /// 与请求代号对应，用于丢弃切图前的过期结果。
    pub generation: u64,
    pub path: PathBuf,
    pub outcome: AnimationLoad,
}

#[derive(Debug)]
struct AnimQueue {
    /// 只保留最新请求：切图后立即覆盖，旧结果按 generation 丢弃。
    pending: Option<(u64, PathBuf)>,
    closed: bool,
}

/// 动画解码服务：独立后台线程调用 Core 的 `decode_animation`，不阻塞 UI。
///
/// 与 [`PreviewService`] 分开，因为动画是「整段一次性解码」，而主图是
/// latest-wins 的单张解码；两者混用会让长动画占住主图通道、拖慢翻页。
pub struct AnimationService {
    queue: Arc<(Mutex<AnimQueue>, Condvar)>,
    results: Receiver<AnimationResult>,
    next_generation: u64,
    /// 已接受、正供画布逐帧显示的结果；`Result` 便于失败时保留静态首帧。
    ready: Option<LoadedAnimation>,
    ready_path: Option<PathBuf>,
}

impl Default for AnimationService {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimationService {
    pub fn new() -> Self {
        let queue = Arc::new((
            Mutex::new(AnimQueue {
                pending: None,
                closed: false,
            }),
            Condvar::new(),
        ));
        let (sender, results) = mpsc::channel();
        let worker_queue = Arc::clone(&queue);
        std::thread::Builder::new()
            .name("animation-decoder".to_owned())
            .spawn(move || run_animation_worker(&worker_queue, &sender))
            .expect("无法启动动画解码线程");
        Self {
            queue,
            results,
            next_generation: 0,
            ready: None,
            ready_path: None,
        }
    }

    /// 排入一次动画解码；返回代号供丢弃过期结果。
    pub fn request(&mut self, path: PathBuf) -> u64 {
        self.next_generation += 1;
        let generation = self.next_generation;
        let (mutex, ready) = &*self.queue;
        let mut queue = lock_anim_queue(mutex);
        queue.pending = Some((generation, path));
        ready.notify_one();
        generation
    }

    /// 清空尚未开始的请求；已开始的解码仍会回报，但调用方按 generation 丢弃。
    pub fn cancel(&mut self) {
        let (mutex, _) = &*self.queue;
        lock_anim_queue(mutex).pending = None;
    }

    pub fn try_recv(&self) -> Option<AnimationResult> {
        self.results.try_recv().ok()
    }

    /// 接受一份动画结果并缓存到服务内，返回帧数供状态层建立播放状态。
    pub fn accept(&mut self, animation: LoadedAnimation, path: PathBuf) {
        self.ready = Some(animation);
        self.ready_path = Some(path);
    }

    /// 当前路径的动画帧（浅克隆）；命中则切图回翻时不重复解码。
    pub fn ready_for(&self, path: &Path) -> Option<LoadedAnimation> {
        if self.ready_path.as_deref() == Some(path) {
            self.ready.clone()
        } else {
            None
        }
    }

    pub fn forget(&mut self, path: &Path) {
        if self.ready_path.as_deref() == Some(path) {
            self.ready = None;
            self.ready_path = None;
        }
    }

    pub fn clear(&mut self) {
        self.ready = None;
        self.ready_path = None;
    }
}

impl Drop for AnimationService {
    fn drop(&mut self) {
        let (mutex, ready) = &*self.queue;
        let mut queue = lock_anim_queue(mutex);
        queue.closed = true;
        queue.pending = None;
        ready.notify_all();
    }
}

fn run_animation_worker(
    queue: &Arc<(Mutex<AnimQueue>, Condvar)>,
    sender: &Sender<AnimationResult>,
) {
    loop {
        let Some((generation, path)) = next_animation_request(queue) else {
            return;
        };
        let outcome = match decode_animation(&path, &Caps::default()) {
            Ok(AnimationOutcome::Ready(decoded)) => {
                AnimationLoad::Ready(LoadedAnimation::from_decoded(decoded))
            }
            Ok(AnimationOutcome::NotAnimated) => AnimationLoad::NotAnimated,
            Ok(AnimationOutcome::Degraded(reason)) => AnimationLoad::Degraded(reason),
            Err(error) => AnimationLoad::Failed(error.to_string()),
        };
        if sender
            .send(AnimationResult {
                generation,
                path,
                outcome,
            })
            .is_err()
        {
            return;
        }
    }
}

fn next_animation_request(queue: &Arc<(Mutex<AnimQueue>, Condvar)>) -> Option<(u64, PathBuf)> {
    let (mutex, ready) = &**queue;
    let mut queue = lock_anim_queue(mutex);
    loop {
        if let Some(request) = queue.pending.take() {
            return Some(request);
        }
        if queue.closed {
            return None;
        }
        queue = ready
            .wait(queue)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
}

fn lock_anim_queue(mutex: &Mutex<AnimQueue>) -> MutexGuard<'_, AnimQueue> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub struct ThumbResult {
    pub path: PathBuf,
    pub result: Result<Handle, String>,
    /// 结果来自磁盘缓存（未重新解码），用于测试与调试日志。
    pub from_disk: bool,
}

#[derive(Debug)]
struct Queue {
    items: VecDeque<PathBuf>,
    closed: bool,
}

#[derive(Debug)]
pub struct ThumbnailService {
    queue: Arc<(Mutex<Queue>, Condvar)>,
    results: Receiver<ThumbResult>,
    inflight: HashSet<PathBuf>,
    disk: Option<Arc<FileCache>>,
}

impl Default for ThumbnailService {
    fn default() -> Self {
        Self::new()
    }
}

impl ThumbnailService {
    pub fn new() -> Self {
        Self::with_cache_dir(cache_dir())
    }

    /// `cache_dir` 为 `None` 或目录不可用时只使用内存缓存。
    fn with_cache_dir(dir: Option<PathBuf>) -> Self {
        let disk = dir.and_then(|dir| match FileCache::new(dir, THUMB_DISK_BYTES) {
            Ok(cache) => Some(Arc::new(cache)),
            Err(error) => {
                log::warn!(target: "image_view::thumbnail", "无法打开缩略图磁盘缓存：{error}");
                None
            }
        });

        let queue = Arc::new((
            Mutex::new(Queue {
                items: VecDeque::new(),
                closed: false,
            }),
            Condvar::new(),
        ));
        let (sender, results) = mpsc::channel();

        for index in 0..THUMB_WORKERS {
            let queue = Arc::clone(&queue);
            let disk = disk.clone();
            let sender = sender.clone();
            std::thread::Builder::new()
                .name(format!("thumbnail-decoder-{index}"))
                .spawn(move || run_worker(&queue, &sender, disk.as_deref()))
                .expect("无法启动缩略图解码线程");
        }

        Self {
            queue,
            results,
            inflight: HashSet::new(),
            disk,
        }
    }

    /// 排入一个缩略图请求；已在队列中时返回 `false`。
    pub fn request(&mut self, path: PathBuf) -> bool {
        if !self.inflight.insert(path.clone()) {
            return false;
        }

        let (mutex, ready) = &*self.queue;
        let mut queue = lock_queue(mutex);
        queue.items.push_back(path);
        ready.notify_one();
        true
    }

    pub fn pending(&self) -> bool {
        !self.inflight.is_empty()
    }

    /// 某个路径是否已有请求在队列中或正在解码；视图层据此避免重复排队。
    pub fn is_inflight(&self, path: &Path) -> bool {
        self.inflight.contains(path)
    }

    /// 源文件被删除或覆盖后丢弃它的全部磁盘缓存。
    pub fn invalidate(&self, path: &Path) {
        if let Some(disk) = &self.disk {
            disk.remove_prefix(&path_key(path));
        }
    }

    pub fn try_recv(&mut self) -> Option<ThumbResult> {
        let result = self.results.try_recv().ok()?;
        self.inflight.remove(&result.path);
        Some(result)
    }
}

impl Drop for ThumbnailService {
    fn drop(&mut self) {
        let (mutex, ready) = &*self.queue;
        let mut queue = lock_queue(mutex);
        queue.closed = true;
        queue.items.clear();
        ready.notify_all();
    }
}

fn run_worker(
    queue: &Arc<(Mutex<Queue>, Condvar)>,
    sender: &Sender<ThumbResult>,
    disk: Option<&FileCache>,
) {
    loop {
        let Some(path) = next_path(queue) else {
            return;
        };

        let (result, from_disk) = load_thumbnail(&path, disk);
        let response = ThumbResult {
            path,
            result,
            from_disk,
        };
        if sender.send(response).is_err() {
            return;
        }
    }
}

/// 先查磁盘缓存，未命中才解码；解码成功后把结果写成 PNG 供下次复用。
fn load_thumbnail(path: &Path, disk: Option<&FileCache>) -> (Result<Handle, String>, bool) {
    let key = disk.map(|_| disk_key(path));

    if let (Some(disk), Some(key)) = (disk, key.as_deref())
        && let Some(bytes) = disk.get(key)
        && let Some((width, height, rgba)) = decode_png(&bytes)
    {
        return (Ok(Handle::from_rgba(width, height, rgba)), true);
    }

    match decode_thumbnail(path, MAX_THUMB_SIDE) {
        Ok(decoded) => {
            let width = decoded.size[0] as u32;
            let height = decoded.size[1] as u32;
            let rgba = decoded.rgba.into_rgba8();
            // 写失败只影响下次启动是否复用，不打断当前显示。
            if let (Some(disk), Some(key)) = (disk, key.as_deref())
                && let Some(png) = encode_png(width, height, &rgba)
                && let Err(error) = disk.put(key, &png)
            {
                log::debug!(
                    target: "image_view::thumbnail",
                    "写入缩略图缓存失败 {}：{error}",
                    path.display()
                );
            }
            (Ok(Handle::from_rgba(width, height, rgba)), false)
        }
        Err(error) => (Err(error.to_string()), false),
    }
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .ok()?;
    Some(bytes)
}

/// 校验尺寸后再用；缓存里的半个文件或旧尺寸内容直接当作未命中。
fn decode_png(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 || width > MAX_THUMB_SIDE || height > MAX_THUMB_SIDE {
        return None;
    }
    Some((width, height, rgba.into_raw()))
}

/// 缓存键：源路径摘要作为目录，`mtime + 尺寸` 的摘要作为文件名。
///
/// `FileCache` 只接受 ASCII 字母、数字与 `-_.`，因此不能直接拼接路径。
fn disk_key(path: &Path) -> String {
    let mut seed = path.to_string_lossy().into_owned().into_bytes();
    seed.extend_from_slice(&modified_millis(path).to_le_bytes());
    seed.extend_from_slice(&MAX_THUMB_SIDE.to_le_bytes());
    format!("{}/{:016x}.png", path_key(path), digest(&seed))
}

fn path_key(path: &Path) -> String {
    format!("{:016x}", digest(path.to_string_lossy().as_bytes()))
}

fn modified_millis(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// FNV-1a：只用来生成稳定的短键，不承担密码学强度。
fn digest(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// 与平台约定一致的缓存目录；目录不可用时返回 `None`，退化为纯内存缓存。
fn cache_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(CACHE_DIR_ENV).filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(dir).join("thumbnails"));
    }

    let base = if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Caches"))
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
    };

    base.map(|dir| dir.join("image-view").join("thumbnails"))
}

fn next_path(queue: &Arc<(Mutex<Queue>, Condvar)>) -> Option<PathBuf> {
    let (mutex, ready) = &**queue;
    let mut queue = lock_queue(mutex);
    loop {
        if let Some(path) = queue.items.pop_front() {
            return Some(path);
        }
        if queue.closed {
            return None;
        }
        queue = ready
            .wait(queue)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
}

fn lock_queue(mutex: &Mutex<Queue>) -> MutexGuard<'_, Queue> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{Duration, Instant};

    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("image-view-thumb-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建临时目录");
        dir
    }

    fn write_png(path: &Path, width: u32, height: u32) {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([180, 40, 40, 255]));
        image.save(path).expect("写入测试图片");
    }

    /// 写一个两帧 gif，帧间隔 120 ms。
    fn write_animated_gif(path: &Path) {
        use image::codecs::gif::{GifEncoder, Repeat};
        use image::{Delay, Frame};

        let file = fs::File::create(path).expect("创建 gif");
        let mut encoder = GifEncoder::new(file);
        encoder.set_repeat(Repeat::Infinite).expect("设置循环");
        for color in [[255, 0, 0, 255], [0, 0, 255, 255]] {
            let buffer = image::RgbaImage::from_pixel(4, 2, image::Rgba(color));
            let frame = Frame::from_parts(buffer, 0, 0, Delay::from_numer_denom_ms(120, 1));
            encoder.encode_frame(frame).expect("写入 gif 帧");
        }
    }

    /// 动画服务解码真实 gif：帧数、间隔与代号正确，缓存可供回翻复用。
    #[test]
    fn animation_service_decodes_and_caches_gif() {
        let dir = temp_dir("anim");
        let path = dir.join("loop.gif");
        write_animated_gif(&path);

        let mut service = AnimationService::new();
        let generation = service.request(path.clone());
        assert_eq!(generation, 1, "代号从 1 递增");

        let deadline = Instant::now() + Duration::from_secs(5);
        let loaded = loop {
            if let Some(result) = service.try_recv() {
                assert_eq!(result.generation, generation);
                assert_eq!(result.path, path);
                match result.outcome {
                    AnimationLoad::Ready(loaded) => break loaded,
                    other => panic!("期望 Ready，得到 {other:?}"),
                }
            }
            assert!(Instant::now() < deadline, "动画解码超时");
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(loaded.frames.len(), 2);
        assert_eq!(loaded.frames[0].delay_ms, 120);
        assert_eq!(loaded.loops, None, "Repeat::Infinite 对应无限循环");

        service.accept(loaded, path.clone());
        assert!(service.ready_for(&path).is_some(), "回翻应命中缓存");
        assert!(service.ready_for(&dir.join("other.gif")).is_none());

        drop(service);
        let _ = fs::remove_dir_all(&dir);
    }

    /// 静态图应返回 NotAnimated，不触发播放与横幅。
    #[test]
    fn animation_service_reports_static_image_as_not_animated() {
        let dir = temp_dir("anim-static");
        let path = dir.join("still.png");
        write_png(&path, 8, 8);

        let mut service = AnimationService::new();
        let generation = service.request(path.clone());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(result) = service.try_recv() {
                assert_eq!(result.generation, generation);
                assert!(
                    matches!(result.outcome, AnimationLoad::NotAnimated),
                    "静态 PNG 应返回 NotAnimated"
                );
                break;
            }
            assert!(Instant::now() < deadline, "动画解码超时");
            std::thread::sleep(Duration::from_millis(10));
        }

        drop(service);
        let _ = fs::remove_dir_all(&dir);
    }

    /// 等待解码线程回报，超时视为失败，避免用例挂住。
    fn recv(service: &mut ThumbnailService, path: &Path) -> ThumbResult {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            while let Some(result) = service.try_recv() {
                if result.path == path {
                    return result;
                }
            }
            assert!(
                Instant::now() < deadline,
                "缩略图解码超时：{}",
                path.display()
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// 首次请求写入磁盘，第二次请求直接复用，失效后重新解码。
    #[test]
    fn thumbnails_round_trip_through_disk_cache() {
        let images = temp_dir("images");
        let cache = temp_dir("cache");
        let path = images.join("sample.png");
        write_png(&path, 40, 20);

        let mut first = ThumbnailService::with_cache_dir(Some(cache.clone()));
        first.request(path.clone());
        let result = recv(&mut first, &path);
        assert!(result.result.is_ok(), "首次解码应成功");
        assert!(!result.from_disk, "首次请求不应命中磁盘");

        let entries: Vec<PathBuf> = walk(&cache);
        assert_eq!(entries.len(), 1, "应写入一份缩略图缓存：{entries:?}");

        // 新服务指向同一目录，模拟重启后复用缓存。
        let mut second = ThumbnailService::with_cache_dir(Some(cache.clone()));
        second.request(path.clone());
        let result = recv(&mut second, &path);
        assert!(result.from_disk, "同路径同 mtime 应命中磁盘缓存");
        assert!(result.result.is_ok());

        // 源文件删除后失效，下一次请求重新解码（此时必然失败）。
        second.invalidate(&path);
        assert!(walk(&cache).is_empty(), "失效应清掉该路径的缓存目录");
    }

    /// 无法建立缓存目录时退化为纯内存模式，不 panic。
    #[test]
    fn missing_cache_dir_degrades_to_memory_only() {
        let images = temp_dir("images-no-cache");
        let path = images.join("sample.png");
        write_png(&path, 8, 8);

        let mut service = ThumbnailService::with_cache_dir(None);
        service.request(path.clone());
        let result = recv(&mut service, &path);
        assert!(result.result.is_ok());
        assert!(!result.from_disk);
    }

    fn walk(root: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let Ok(entries) = fs::read_dir(root) else {
            return files;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(walk(&path));
            } else {
                files.push(path);
            }
        }
        files
    }
}
