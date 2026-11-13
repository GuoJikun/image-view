//! 缩略图条：当前目录、虚拟滚动、内存 LRU、磁盘缓存与后台解码。

use std::collections::{HashSet, VecDeque};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use eframe::egui;
use image::ImageEncoder as _;
use iv_cache::LruCache;
use iv_image_decode::decode_thumbnail;

/// 缩略图最长边（逻辑尺寸 64 px，按 2x DPI 预留）。
pub const MAX_SIDE: u32 = 128;
/// 内存中保留的缩略图张数。
pub const MAX_ENTRIES: usize = 512;
/// 磁盘缓存预算；超过后删除最旧的文件。
const DISK_BUDGET_BYTES: u64 = 256 * 1024 * 1024;

const STRIP_HEIGHT: f32 = 88.0;
const ITEM_WIDTH: f32 = 76.0;
const ITEM_HEIGHT: f32 = 56.0;
const ITEM_GAP: f32 = 9.0;
const ITEM_STEP: f32 = ITEM_WIDTH + ITEM_GAP;
const ITEM_PADDING: f32 = 2.0;
/// 每批缩略图并行解码线程数；限制峰值内存（大图解码缓冲较大）。
const DECODE_THREADS: usize = 2;

/// 按最近使用顺序保存缩略图纹理；命中时移到队尾。
pub struct ThumbnailCache {
    inner: LruCache<PathBuf, egui::TextureHandle>,
}

impl Default for ThumbnailCache {
    fn default() -> Self {
        Self {
            inner: LruCache::new(MAX_ENTRIES),
        }
    }
}

impl ThumbnailCache {
    /// 取出一张缩略图纹理并刷新其最近使用顺序。
    pub fn get(&mut self, path: &Path) -> Option<egui::TextureHandle> {
        self.inner.get(&path.to_path_buf())
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.inner.contains(&path.to_path_buf())
    }

    pub fn insert(&mut self, path: PathBuf, texture: egui::TextureHandle) {
        self.inner.insert(path, texture);
    }

    #[cfg(test)]
    fn insert_with_limit(
        &mut self,
        path: PathBuf,
        texture: egui::TextureHandle,
        max_entries: usize,
    ) {
        self.inner.insert_with_limit(path, texture, max_entries);
    }

    pub fn remove(&mut self, path: &Path) {
        self.inner.remove(&path.to_path_buf());
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

/// 缩略图纹理的采样选项：缩小走 mipmap + LINEAR。
pub fn texture_options() -> egui::TextureOptions {
    egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear))
}

#[derive(Debug)]
pub struct ThumbData {
    pub size: [usize; 2],
    pub rgba: Vec<u8>,
}

pub struct ThumbResponse {
    pub generation: u64,
    pub path: PathBuf,
    pub result: Result<ThumbData, String>,
}

enum Command {
    /// 当前视口与预取范围；新命令会丢弃尚未开始的旧请求。
    Focus {
        generation: u64,
        paths: Vec<PathBuf>,
    },
    Cancel(u64),
}

/// 缩略图后台解码线程；与主图解码分离，避免缩略图阻塞翻页。
pub struct ThumbnailLoader {
    tx: Sender<Command>,
    rx: Receiver<ThumbResponse>,
}

impl ThumbnailLoader {
    pub fn new(ctx: egui::Context) -> Self {
        let (tx, rx_request) = mpsc::channel::<Command>();
        let (tx_response, rx_response) = mpsc::channel::<ThumbResponse>();

        std::thread::Builder::new()
            .name("thumbnail-decoder".to_owned())
            .spawn(move || run(rx_request, tx_response, ctx))
            .expect("无法启动缩略图解码线程");

        Self {
            tx,
            rx: rx_response,
        }
    }

    pub fn focus(&self, generation: u64, paths: Vec<PathBuf>) {
        let _ = self.tx.send(Command::Focus { generation, paths });
    }

    /// 切换目录时丢弃旧一代的待办请求。
    pub fn cancel(&self, generation: u64) {
        let _ = self.tx.send(Command::Cancel(generation));
    }

    pub fn try_recv(&self) -> Option<ThumbResponse> {
        self.rx.try_recv().ok()
    }
}

fn run(rx: Receiver<Command>, tx: Sender<ThumbResponse>, ctx: egui::Context) {
    let session_started = Instant::now();
    let dir = cache_dir();
    if std::fs::create_dir_all(&dir).is_ok() {
        prune_to(&dir, DISK_BUDGET_BYTES);
    } else {
        log::warn!("无法创建缩略图缓存目录：{}", dir.display());
    }

    let mut latest = 0u64;
    let mut queue: VecDeque<PathBuf> = VecDeque::new();
    let mut focus_started: Option<(Instant, usize)> = None;

    loop {
        while let Ok(command) = rx.try_recv() {
            apply_command(command, &mut latest, &mut queue, &mut focus_started);
        }

        if queue.is_empty() {
            if let Some((started, count)) = focus_started.take() {
                log::debug!(
                    "缩略图批次 {count} 张，耗时 {:?}；解码线程启动后 {:?}",
                    started.elapsed(),
                    session_started.elapsed()
                );
            }
            match rx.recv() {
                Ok(command) => apply_command(command, &mut latest, &mut queue, &mut focus_started),
                Err(_) => break,
            }
            continue;
        }

        // 一次只解一小批，批间检查新的视口请求，快速滚动时不会积压旧缩略图。
        let count = DECODE_THREADS.min(queue.len());
        let wave: Vec<PathBuf> = queue.drain(..count).collect();
        decode_wave(wave, latest, &dir, &tx, &ctx);
    }
}

fn apply_command(
    command: Command,
    latest: &mut u64,
    queue: &mut VecDeque<PathBuf>,
    focus_started: &mut Option<(Instant, usize)>,
) {
    match command {
        Command::Focus { generation, paths } => {
            if generation < *latest {
                return;
            }
            *latest = generation;
            let mut seen = HashSet::new();
            *queue = paths
                .into_iter()
                .filter(|path| seen.insert(path.clone()))
                .collect();
            *focus_started = (!queue.is_empty())
                .then(Instant::now)
                .map(|now| (now, queue.len()));
        }
        Command::Cancel(generation) => {
            if generation >= *latest {
                *latest = generation;
                queue.clear();
                *focus_started = None;
            }
        }
    }
}

fn decode_wave(
    wave: Vec<PathBuf>,
    generation: u64,
    dir: &Path,
    tx: &Sender<ThumbResponse>,
    ctx: &egui::Context,
) {
    let queue = std::sync::Mutex::new(VecDeque::from(wave));
    let threads = queue.lock().expect("缩略图队列锁已损坏").len();

    std::thread::scope(|scope| {
        for _ in 0..threads.min(DECODE_THREADS) {
            scope.spawn(|| {
                loop {
                    let path = queue.lock().expect("缩略图队列锁已损坏").pop_front();
                    let Some(path) = path else {
                        break;
                    };

                    let result = load_thumbnail(dir, &path);
                    let response = ThumbResponse {
                        generation,
                        path,
                        result,
                    };
                    if tx.send(response).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            });
        }
    });
}

fn load_thumbnail(dir: &Path, path: &Path) -> Result<ThumbData, String> {
    if let Some(key) = cache_key(path) {
        if let Some(data) = read_cached(dir, &key) {
            return Ok(data);
        }

        let decoded = decode_thumbnail(path, MAX_SIDE).map_err(|error| error.to_string())?;
        let data = ThumbData {
            size: decoded.size,
            rgba: decoded.rgba,
        };
        write_cached(dir, &key, &data);
        return Ok(data);
    }

    let decoded = decode_thumbnail(path, MAX_SIDE).map_err(|error| error.to_string())?;
    Ok(ThumbData {
        size: decoded.size,
        rgba: decoded.rgba,
    })
}

/// `%LOCALAPPDATA%\image-view\thumbs`；环境变量缺失时退回临时目录。
pub fn cache_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("image-view")
        .join("thumbs")
}

/// 缓存键：绝对路径 + 修改时间 + 文件长度 + 目标尺寸。
fn cache_key(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());

    // FNV-1a 64：跨进程稳定，不依赖 `DefaultHasher` 的内部实现。
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut write = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    write(b"image-view-thumb-v1");
    write(absolute.to_string_lossy().as_bytes());
    write(&modified.to_le_bytes());
    write(&metadata.len().to_le_bytes());
    write(&MAX_SIDE.to_le_bytes());

    Some(format!("{hash:016x}.qoi"))
}

fn read_cached(dir: &Path, key: &str) -> Option<ThumbData> {
    let path = dir.join(key);
    let bytes = std::fs::read(&path).ok()?;

    // QOI 头：magic + 宽高（u32 BE）+ 通道 + 色彩空间。
    if bytes.len() < 14 || &bytes[..4] != b"qoif" {
        let _ = std::fs::remove_file(&path);
        return None;
    }
    let width = u32::from_be_bytes(bytes[4..8].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[8..12].try_into().ok()?);
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        let _ = std::fs::remove_file(&path);
        return None;
    }

    let image = match image::load_from_memory_with_format(&bytes, image::ImageFormat::Qoi) {
        Ok(image) => image.to_rgba8(),
        Err(error) => {
            log::debug!("缩略图缓存损坏，已删除 {}：{error}", path.display());
            let _ = std::fs::remove_file(&path);
            return None;
        }
    };

    let size = [image.width() as usize, image.height() as usize];
    let rgba = image.into_raw();
    if rgba.len() != size[0] * size[1] * 4 {
        let _ = std::fs::remove_file(&path);
        return None;
    }

    Some(ThumbData { size, rgba })
}

fn write_cached(dir: &Path, key: &str, data: &ThumbData) {
    let Some(image) =
        image::RgbaImage::from_raw(data.size[0] as u32, data.size[1] as u32, data.rgba.clone())
    else {
        return;
    };

    let mut bytes = Vec::new();
    let encoder = image::codecs::qoi::QoiEncoder::new(&mut bytes);
    if let Err(error) = encoder.write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        image::ExtendedColorType::Rgba8,
    ) {
        log::debug!("无法编码缩略图缓存：{error}");
        return;
    }

    let target = dir.join(key);
    let temporary = dir.join(format!(".{key}.{}.tmp", std::process::id()));
    if let Err(error) = std::fs::write(&temporary, bytes) {
        log::debug!("无法写入缩略图缓存 {}：{error}", temporary.display());
        return;
    }
    // Windows 上 rename 不能覆盖已存在的文件；先删除旧缓存。
    let _ = std::fs::remove_file(&target);
    if let Err(error) = std::fs::rename(&temporary, &target) {
        log::debug!("无法提交缩略图缓存 {}：{error}", target.display());
        let _ = std::fs::remove_file(&temporary);
    }
}

/// 启动时把磁盘缓存控制在预算内，删除最旧的文件。
fn prune_to(dir: &Path, budget: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    let mut files: Vec<(PathBuf, u64, SystemTime)> = Vec::new();
    let mut total = 0u64;
    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
        total = total.saturating_add(metadata.len());
        files.push((entry.path(), metadata.len(), modified));
    }

    if total <= budget {
        return;
    }

    files.sort_by_key(|(_, _, modified)| *modified);
    let target = budget * 9 / 10;
    for (path, size, _) in files {
        if total <= target {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(size);
        }
    }
    log::info!("缩略图磁盘缓存已清理至 {total} 字节");
}

pub struct StripState<'a> {
    pub files: &'a [PathBuf],
    pub current: Option<usize>,
    pub cache: &'a mut ThumbnailCache,
    pub follow: bool,
}

#[derive(Default)]
pub struct StripOutput {
    pub clicked: Option<usize>,
    pub visible: Range<usize>,
    pub followed: bool,
}

/// 绘制底部缩略图条；只创建视口内的卡片。
pub fn strip(ui: &mut egui::Ui, state: StripState<'_>) -> StripOutput {
    let StripState {
        files,
        current,
        cache,
        follow,
    } = state;

    let mut output = StripOutput::default();
    if files.is_empty() {
        return output;
    }

    let palette = crate::theme::palette(ui.ctx());
    let panel = egui::Panel::bottom("thumbnail_strip")
        .exact_size(STRIP_HEIGHT)
        .frame(
            egui::Frame::NONE
                .fill(palette.strip)
                .inner_margin(egui::Margin::symmetric(8, 16)),
        )
        .show(ui, |ui| {
            // 缩略图条只有横向滚动，滚轮直接横向移动。
            ui.style_mut().always_scroll_the_only_direction = true;

            let total_width = files.len() as f32 * ITEM_STEP - ITEM_GAP;
            let height = ui.available_height();
            egui::ScrollArea::horizontal()
                .id_salt("thumbnail_scroll")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .auto_shrink([false, false])
                .show_viewport(ui, |ui, viewport| {
                    let (content, _) = ui
                        .allocate_exact_size(egui::vec2(total_width, height), egui::Sense::hover());

                    output.visible = visible_range(viewport.min.x, viewport.width(), files.len());

                    if follow
                        && let Some(index) = current
                        && index < files.len()
                    {
                        ui.scroll_to_rect(item_rect(content.min, index), Some(egui::Align::Center));
                        output.followed = true;
                    }

                    for index in output.visible.clone() {
                        let rect = item_rect(content.min, index);
                        let response = ui.interact(
                            rect,
                            ui.scope_id().with(("thumbnail", index)),
                            egui::Sense::click(),
                        );
                        let selected = current == Some(index);
                        let painter = ui.painter();

                        painter.rect_filled(rect, egui::CornerRadius::same(6), palette.tile);
                        if response.hovered() && !selected {
                            painter.rect_filled(rect, egui::CornerRadius::same(6), palette.hover);
                        }

                        if let Some(texture) = cache.get(&files[index]) {
                            let texture_size = texture.size_vec2();
                            if texture_size.x > 0.0 && texture_size.y > 0.0 {
                                // 缩略图按 cover 填满卡片，再裁掉圆角外的部分。
                                let available = rect.shrink(ITEM_PADDING).size();
                                let scale = (available.x / texture_size.x)
                                    .max(available.y / texture_size.y);
                                let image_rect = egui::Rect::from_center_size(
                                    rect.center(),
                                    texture_size * scale,
                                );
                                let clipped = painter.with_clip_rect(rect.shrink(1.0));
                                clipped.image(
                                    texture.id(),
                                    image_rect,
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    egui::Color32::WHITE,
                                );
                            }
                        } else {
                            painter.text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "…",
                                egui::FontId::proportional(18.0),
                                palette.text3,
                            );
                        }

                        painter.rect_stroke(
                            rect,
                            egui::CornerRadius::same(6),
                            egui::Stroke::new(1.0, palette.tile_border),
                            egui::StrokeKind::Inside,
                        );
                        if selected {
                            painter.rect_stroke(
                                rect.expand(3.0),
                                egui::CornerRadius::same(9),
                                egui::Stroke::new(2.0, palette.accent),
                                egui::StrokeKind::Middle,
                            );
                        }

                        if response.clicked() {
                            output.clicked = Some(index);
                        }
                        if response.hovered() {
                            let name = files[index]
                                .file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_else(|| files[index].display().to_string());
                            let _ = response.on_hover_text(name);
                        }
                    }
                });
        });

    let rect = panel.response.rect;
    ui.painter().hline(
        rect.x_range(),
        rect.top() + 0.5,
        egui::Stroke::new(1.0, palette.border),
    );

    output
}

fn item_rect(origin: egui::Pos2, index: usize) -> egui::Rect {
    egui::Rect::from_min_size(
        origin + egui::vec2(index as f32 * ITEM_STEP, 0.0),
        egui::vec2(ITEM_WIDTH, ITEM_HEIGHT),
    )
}

/// 视口覆盖的缩略图下标；前后各多取一张，避免快速滚动时出现空白。
fn visible_range(offset: f32, width: f32, total: usize) -> Range<usize> {
    if total == 0 {
        return 0..0;
    }
    let offset = offset.max(0.0);
    let start = (offset / ITEM_STEP).floor() as usize;
    let end = ((offset + width.max(0.0)) / ITEM_STEP).ceil() as usize + 1;
    start.saturating_sub(1)..end.min(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "image-view-thumb-test-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn texture(ctx: &egui::Context, name: &str) -> egui::TextureHandle {
        ctx.load_texture(
            name,
            egui::ColorImage::filled([2, 2], egui::Color32::WHITE),
            egui::TextureOptions::LINEAR,
        )
    }

    #[test]
    fn focus_replaces_pending_work_and_ignores_stale_generations() {
        let mut latest = 0;
        let mut queue = VecDeque::new();
        let mut started = None;

        apply_command(
            Command::Focus {
                generation: 1,
                paths: vec![PathBuf::from("a"), PathBuf::from("b"), PathBuf::from("a")],
            },
            &mut latest,
            &mut queue,
            &mut started,
        );
        assert_eq!(latest, 1);
        assert_eq!(
            queue,
            VecDeque::from([PathBuf::from("a"), PathBuf::from("b")])
        );
        assert_eq!(started.map(|(_, count)| count), Some(2));

        apply_command(
            Command::Focus {
                generation: 0,
                paths: vec![PathBuf::from("c")],
            },
            &mut latest,
            &mut queue,
            &mut started,
        );
        assert_eq!(
            queue,
            VecDeque::from([PathBuf::from("a"), PathBuf::from("b")]),
            "旧目录请求不应替换当前队列"
        );

        apply_command(
            Command::Focus {
                generation: 2,
                paths: Vec::new(),
            },
            &mut latest,
            &mut queue,
            &mut started,
        );
        assert!(queue.is_empty());
        assert!(started.is_none());

        apply_command(
            Command::Focus {
                generation: 2,
                paths: vec![PathBuf::from("d")],
            },
            &mut latest,
            &mut queue,
            &mut started,
        );
        apply_command(Command::Cancel(2), &mut latest, &mut queue, &mut started);
        assert_eq!(latest, 2);
        assert!(queue.is_empty());
        assert!(started.is_none());
    }

    #[test]
    fn memory_cache_evicts_oldest_and_refreshes_hits() {
        let ctx = egui::Context::default();
        let mut cache = ThumbnailCache::default();

        for name in ["a", "b", "c"] {
            cache.insert_with_limit(PathBuf::from(name), texture(&ctx, name), 3);
        }
        assert!(cache.get(Path::new("a")).is_some());
        cache.insert_with_limit(PathBuf::from("d"), texture(&ctx, "d"), 3);

        assert!(cache.contains(Path::new("a")));
        assert!(cache.contains(Path::new("d")));
        assert!(!cache.contains(Path::new("b")), "最旧的 b 应被淘汰");
        assert_eq!(cache.len(), 3);
    }

    #[test]
    fn thumbnail_textures_use_linear_mipmaps() {
        let options = texture_options();
        assert_eq!(options.magnification, egui::TextureFilter::Linear);
        assert_eq!(options.minification, egui::TextureFilter::Linear);
        assert_eq!(
            options.mipmap_mode,
            Some(egui::TextureFilter::Linear),
            "缩小应使用 mipmap"
        );
    }

    #[test]
    fn visible_range_virtualizes_large_lists() {
        assert_eq!(visible_range(0.0, 800.0, 3000), 0..11);
        let middle = visible_range(ITEM_STEP * 1000.0, 800.0, 3000);
        assert!(middle.start >= 999);
        assert!(middle.end > middle.start);
        assert!(middle.end < 1012);
        assert_eq!(visible_range(0.0, 800.0, 0), 0..0);
    }

    #[test]
    fn disk_cache_round_trip_and_corruption_cleanup() {
        let dir = temp_dir("round-trip");
        let source = dir.join("source.png");
        std::fs::write(&source, b"fixture").expect("write source");

        let key = cache_key(&source).expect("cache key");
        let data = ThumbData {
            size: [2, 1],
            rgba: vec![255, 0, 0, 255, 0, 255, 0, 255],
        };
        write_cached(&dir, &key, &data);
        let loaded = read_cached(&dir, &key).expect("read cache");
        assert_eq!(loaded.size, [2, 1]);
        assert_eq!(loaded.rgba, data.rgba);

        let corrupt = dir.join("bad.qoi");
        std::fs::write(&corrupt, b"not a qoi").expect("write corrupt");
        assert!(read_cached(&dir, "bad.qoi").is_none());
        assert!(!corrupt.exists(), "损坏缓存应被删除");

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn cache_key_includes_path_and_target_size() {
        let dir = temp_dir("keys");
        let first = dir.join("a.png");
        let second = dir.join("b.png");
        std::fs::write(&first, b"a").expect("write first");
        std::fs::write(&second, b"b").expect("write second");

        assert_ne!(cache_key(&first), cache_key(&second));
        assert_eq!(cache_key(&first), cache_key(&first));

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn quota_pruning_removes_files_until_under_budget() {
        let dir = temp_dir("prune");
        for index in 0..3 {
            let path = dir.join(format!("{index}.qoi"));
            std::fs::write(&path, vec![0u8; 8]).expect("write cache file");
        }

        prune_to(&dir, 16);
        let remaining: u64 = std::fs::read_dir(&dir)
            .expect("read dir")
            .flatten()
            .filter_map(|entry| entry.metadata().ok())
            .map(|metadata| metadata.len())
            .sum();
        assert!(remaining <= 16, "清理后仍超出预算：{remaining}");
        assert!(
            std::fs::read_dir(&dir).expect("read dir").count() < 3,
            "应至少删除一个缓存文件"
        );

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
