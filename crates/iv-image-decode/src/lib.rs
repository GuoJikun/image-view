//! 后台解码线程：尺寸检查、解码、降采样。

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

use image::ImageDecoder as _;
use image::imageops::FilterType;
use image::metadata::Orientation;
use iv_metadata::{self as color, ColorInfo};

/// 解码输出的像素缓冲类型；供 GUI 层按位深编码 PNG。
pub use iv_metadata::Pixels;
use iv_task::Generation;

/// 解码与纹理上传的安全上限。
#[derive(Clone, Debug)]
pub struct Caps {
    /// 纹理最长边（同时受 GPU 上限约束）。
    pub max_side: usize,
    /// 纹理总像素上限，控制显存/内存峰值。
    pub max_pixels: usize,
    /// 原图总像素上限，防止解压炸弹。
    pub max_source_pixels: u64,
    /// 预加载允许的原图像素上限；超过则跳过，避免大图拖慢翻页。
    pub preload_max_source_pixels: u64,
    /// 动画所有帧的总像素上限（帧数 × 单帧像素）；超过时退回静态首帧。
    pub max_animation_pixels: u64,
    /// 动画帧数上限；超过时退回静态首帧。
    pub max_animation_frames: usize,
}

impl Default for Caps {
    fn default() -> Self {
        Self {
            max_side: 8192,
            max_pixels: 40_000_000,
            max_source_pixels: 250_000_000,
            preload_max_source_pixels: 30_000_000,
            // 64 MP × 4 字节 ≈ 256 MB：长动画的帧缓冲仍有明确上限。
            max_animation_pixels: 64_000_000,
            max_animation_frames: 1_200,
        }
    }
}

impl Caps {
    /// `max_texture_side` 在首帧前可能为 0，此时使用 8192 兜底。
    pub fn from_texture_side(max_texture_side: usize) -> Self {
        let max_side = if max_texture_side == 0 {
            8192
        } else {
            max_texture_side.min(8192)
        };
        Self {
            max_side,
            ..Self::default()
        }
    }

    fn image_limits(&self) -> image::Limits {
        let mut limits = image::Limits::default();
        // 覆盖超大解码中间缓冲；真正的尺寸限制由 decode 前的检查负责。
        limits.max_alloc = Some(512 * 1024 * 1024);
        limits
    }
}

pub struct Decoded {
    pub size: [usize; 2],
    pub source_size: [usize; 2],
    pub rgba: Pixels,
    /// 源文件位深（8 或 16），与 `rgba` 的变体一致。
    pub source_depth: u8,
    pub color: ColorInfo,
}

/// 动画的一帧：已转换为 sRGB 的 RGBA，尺寸与 `DecodedAnimation::size` 一致。
#[derive(Clone, Debug)]
pub struct AnimationFrame {
    pub rgba: Pixels,
    /// 帧间隔（毫秒），已钳制到 50~1000。
    pub delay_ms: u32,
}

/// 一次动画解码的结果；帧像素总量受 `Caps::max_animation_pixels` 约束。
pub struct DecodedAnimation {
    pub size: [usize; 2],
    pub source_size: [usize; 2],
    pub source_depth: u8,
    pub color: ColorInfo,
    /// 总播放轮数；`None` 表示无限循环。
    pub loops: Option<u32>,
    pub frames: Vec<AnimationFrame>,
}

/// 帧间隔下限：GIF 的 0 delay（部分导出工具写成 0）按 20 fps 兜底。
const MIN_FRAME_DELAY_MS: u32 = 50;
/// 帧间隔上限：超大 delay（占用型文件）钳制到 1 s，避免看起来像卡死。
const MAX_FRAME_DELAY_MS: u32 = 1_000;

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Decode(image::ImageError),
    TooLarge {
        w: u32,
        h: u32,
    },
    OutOfMemory,
    /// 预加载遇到大图时主动放弃；不会展示给用户。
    PreloadSkipped {
        w: u32,
        h: u32,
    },
    #[cfg(feature = "avif")]
    Avif(String),
    #[cfg(feature = "heic")]
    Heic(String),
    #[cfg(feature = "svg")]
    Svg(String),
    /// 仅在关闭对应 feature 的构建中构造。
    #[cfg_attr(
        all(feature = "avif", feature = "heic", feature = "svg", feature = "jxl"),
        allow(dead_code)
    )]
    MissingDecoder {
        format: &'static str,
        extension: String,
    },
    #[cfg(feature = "svg")]
    SvgzUnsupported,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 稳定的调试/日志描述；界面文案由 GUI 按错误类型本地化。
        match self {
            Self::Io(error) => write!(formatter, "failed to read file: {error}"),
            Self::Decode(error) => write!(formatter, "image is corrupt or unsupported: {error}"),
            Self::TooLarge { w, h } => {
                write!(
                    formatter,
                    "image too large ({w}×{h}), exceeds the safety limit"
                )
            }
            Self::OutOfMemory => formatter.write_str("decoding exceeded the memory limit"),
            Self::PreloadSkipped { w, h } => {
                write!(formatter, "preload skipped (source {w}×{h} is too large)")
            }
            #[cfg(feature = "avif")]
            Self::Avif(detail) => write!(formatter, "AVIF decoding failed: {detail}"),
            #[cfg(feature = "heic")]
            Self::Heic(detail) => write!(formatter, "HEIC decoding failed: {detail}"),
            #[cfg(feature = "svg")]
            Self::Svg(detail) => write!(formatter, "SVG parsing failed: {detail}"),
            Self::MissingDecoder { format, extension } => write!(
                formatter,
                "{format} decoder is not included in this build; use a build with that feature to open .{extension} files"
            ),
            #[cfg(feature = "svg")]
            Self::SvgzUnsupported => formatter
                .write_str("Gzip-compressed SVGZ is not supported; extract it to .svg first"),
        }
    }
}

impl std::error::Error for LoadError {}

impl From<std::io::Error> for LoadError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<image::ImageError> for LoadError {
    fn from(error: image::ImageError) -> Self {
        Self::Decode(error)
    }
}

/// 按扩展名选择解码器；HEIC/AVIF/JXL 不走 `image` 的通用读取路径，SVG 走光栅化。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Standard,
    Avif,
    Heic,
    Jxl,
    Svg,
}

fn format_for(path: &Path) -> Format {
    match path.extension().and_then(OsStr::to_str) {
        Some(extension) if extension.eq_ignore_ascii_case("avif") => Format::Avif,
        Some(extension)
            if extension.eq_ignore_ascii_case("heic") || extension.eq_ignore_ascii_case("heif") =>
        {
            Format::Heic
        }
        Some(extension) if extension.eq_ignore_ascii_case("jxl") => Format::Jxl,
        Some(extension) if extension.eq_ignore_ascii_case("svg") => Format::Svg,
        _ => Format::Standard,
    }
}

/// 仅在关闭对应 feature 的构建中调用。
#[cfg_attr(
    all(feature = "avif", feature = "heic", feature = "svg", feature = "jxl"),
    allow(dead_code)
)]
fn missing_decoder(path: &Path, format: Format) -> LoadError {
    let (label, fallback) = match format {
        Format::Avif => ("AVIF", "avif"),
        Format::Heic => ("HEIC", "heic"),
        Format::Jxl => ("JPEG XL", "jxl"),
        Format::Svg => ("SVG", "svg"),
        Format::Standard => ("image", "img"),
    };
    let extension = path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or(fallback)
        .to_owned();
    LoadError::MissingDecoder {
        format: label,
        extension,
    }
}

/// 请求类型：打开当前图片，或后台预加载相邻图片。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    Open,
    Preload,
}

#[derive(Clone)]
struct Request {
    generation: u64,
    path: PathBuf,
    caps: Caps,
    purpose: Purpose,
}

pub struct LoadResponse {
    pub generation: u64,
    pub path: PathBuf,
    pub purpose: Purpose,
    pub result: Result<Decoded, LoadError>,
}

/// 解码线程完成一批请求后通知 GUI 重绘；由宿主注入，核心不依赖窗口系统。
pub type Repaint = Arc<dyn Fn() + Send + Sync>;

pub struct Loader {
    tx: Sender<Request>,
    rx: Receiver<LoadResponse>,
    next_generation: Generation,
}

impl Loader {
    pub fn new(repaint: impl Fn() + Send + Sync + 'static) -> Self {
        let (tx_request, rx_request) = mpsc::channel::<Request>();
        let (tx_response, rx_response) = mpsc::channel::<LoadResponse>();
        let repaint: Repaint = Arc::new(repaint);

        std::thread::Builder::new()
            .name("image-decoder".to_owned())
            .spawn(move || {
                let mut pending: Vec<Request> = Vec::new();
                loop {
                    if pending.is_empty() {
                        match rx_request.recv() {
                            Ok(request) => pending.push(request),
                            Err(_) => break,
                        }
                    }

                    // 打开请求优先于预加载；预加载只是填充缓存，不能挡住翻页。
                    let index = pending
                        .iter()
                        .position(|request| request.purpose == Purpose::Open)
                        .unwrap_or(0);
                    let request = pending.remove(index);

                    let result = decode(&request.path, &request.caps, request.purpose);
                    let response = LoadResponse {
                        generation: request.generation,
                        path: request.path,
                        purpose: request.purpose,
                        result,
                    };
                    if tx_response.send(response).is_err() {
                        break;
                    }
                    repaint();

                    let mut incoming = Vec::new();
                    while let Ok(request) = rx_request.try_recv() {
                        incoming.push(request);
                    }
                    merge_pending(&mut pending, incoming);
                }
            })
            .expect("无法启动解码线程");

        Self {
            tx: tx_request,
            rx: rx_response,
            next_generation: Generation::default(),
        }
    }

    /// 请求打开图片；返回本次请求的代号，主线程据此丢弃过期结果。
    pub fn request(&mut self, path: PathBuf, caps: Caps) -> u64 {
        let generation = self.next_generation.advance();
        self.send(Request {
            generation,
            path,
            caps,
            purpose: Purpose::Open,
        });
        generation
    }

    /// 后台预加载相邻图片；结果只用于填充缓存，不影响当前显示。
    pub fn preload(&self, path: PathBuf, caps: Caps, generation: u64) {
        self.send(Request {
            generation,
            path,
            caps,
            purpose: Purpose::Preload,
        });
    }

    /// 不使用解码线程地推进代号，使所有在途结果过期（缓存命中时使用）。
    pub fn cancel_pending(&mut self) -> u64 {
        self.next_generation.advance()
    }

    fn send(&self, request: Request) {
        let _ = self.tx.send(request);
    }

    pub fn try_recv(&self) -> Option<LoadResponse> {
        self.rx.try_recv().ok()
    }
}

/// 合并解码线程的积压请求。
///
/// - 新的打开请求覆盖所有待办（快速翻页时只解码最后一张）；
/// - 只有预加载时按路径去重后追加，保持先后顺序。
fn merge_pending(pending: &mut Vec<Request>, incoming: Vec<Request>) {
    if let Some(index) = incoming
        .iter()
        .rposition(|request| request.purpose == Purpose::Open)
    {
        let open = incoming[index].clone();
        pending.clear();
        pending.push(open);
        return;
    }

    for request in incoming {
        if !pending.iter().any(|queued| queued.path == request.path) {
            pending.push(request);
        }
    }
}

/// 计算降采样后的尺寸：同时满足最长边与总像素上限，保持宽高比。
pub fn target_size(source: [usize; 2], max_side: usize, max_pixels: usize) -> [usize; 2] {
    let width = source[0] as f64;
    let height = source[1] as f64;
    if width <= 0.0 || height <= 0.0 {
        return [1, 1];
    }

    let side_ratio = max_side as f64 / width.max(height);
    let pixel_ratio = ((max_pixels as f64) / (width * height)).sqrt();
    let ratio = side_ratio.min(pixel_ratio).min(1.0);

    if ratio >= 1.0 {
        return source;
    }

    [
        (width * ratio).round().max(1.0) as usize,
        (height * ratio).round().max(1.0) as usize,
    ]
}

fn map_decode_error(error: image::ImageError) -> LoadError {
    match error {
        image::ImageError::Limits(_) => LoadError::OutOfMemory,
        other => LoadError::Decode(other),
    }
}

fn decode(path: &Path, caps: &Caps, purpose: Purpose) -> Result<Decoded, LoadError> {
    match format_for(path) {
        Format::Standard => decode_standard(path, caps, purpose),
        Format::Avif => decode_avif(path, caps, purpose),
        Format::Heic => decode_heic(path, caps, purpose),
        Format::Jxl => decode_jxl(path, caps, purpose),
        Format::Svg => decode_svg(path, caps, purpose),
    }
}

/// 单次同步解码入口。
///
/// `Loader` 面向单窗口 GUI 的 latest-wins 队列；其他 GUI adapter（如 Tauri）
/// 需要自己的请求调度时，可直接调用本函数复用同一套格式分派、ICC 与限额逻辑。
pub fn decode_image(path: &Path, caps: &Caps, purpose: Purpose) -> Result<Decoded, LoadError> {
    decode(path, caps, purpose)
}

/// 按缩略图尺寸解码；复用主图路径的格式分派、ICC 转换与错误类型。
pub fn decode_thumbnail(path: &Path, max_side: u32) -> Result<Decoded, LoadError> {
    let side = max_side as usize;
    let caps = Caps {
        max_side: side,
        max_pixels: side * side,
        ..Caps::default()
    };
    decode(path, &caps, Purpose::Open)
}

/// 可能包含多帧的格式。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AnimationKind {
    Apng,
    Gif,
    Webp,
}

fn animation_kind(path: &Path) -> Option<AnimationKind> {
    match path.extension().and_then(OsStr::to_str) {
        Some(extension) if extension.eq_ignore_ascii_case("png") => Some(AnimationKind::Apng),
        Some(extension) if extension.eq_ignore_ascii_case("gif") => Some(AnimationKind::Gif),
        Some(extension) if extension.eq_ignore_ascii_case("webp") => Some(AnimationKind::Webp),
        _ => None,
    }
}

/// 解码 APNG / GIF / WebP 的全部动画帧。
///
/// 返回 `Ok(None)` 的三种情况：不是动画格式（含单帧 PNG）、只有一帧、帧总像素或
/// 帧数超过 `Caps` 的动画预算。三者都按普通静态图（首帧）显示，不产生错误提示。
/// 帧间隔按 APNG delay / GIF delay / WebP duration 计时，异常值钳制到 50~1000 ms。
pub fn decode_animation(path: &Path, caps: &Caps) -> Result<Option<DecodedAnimation>, LoadError> {
    match animation_kind(path) {
        Some(AnimationKind::Apng) => decode_png_animation(path, caps),
        Some(AnimationKind::Gif) => decode_gif_animation(path, caps),
        Some(AnimationKind::Webp) => decode_webp_animation(path, caps),
        None => Ok(None),
    }
}

/// APNG 复用 `image` 的 `PngDecoder`：只有带 `acTL` 块的 PNG 才算动画。
///
/// `ApngDecoder` 会把每帧合成为整幅画布（含 `dispose` / `blend` 处理），输出
/// 8-bit RGBA；16-bit APNG 会被它直接拒绝，此时动画解码报错，静态图仍由通用
/// 解码路径按 16-bit 首帧正常显示。
fn decode_png_animation(path: &Path, caps: &Caps) -> Result<Option<DecodedAnimation>, LoadError> {
    use image::AnimationDecoder as _;

    let reader = std::io::BufReader::new(std::fs::File::open(path)?);
    let mut decoder = image::codecs::png::PngDecoder::new(reader).map_err(map_decode_error)?;
    if !decoder.is_apng().map_err(map_decode_error)? {
        return Ok(None);
    }
    let (width, height) = decoder.dimensions();
    check_source_limits(width, height, caps, Purpose::Open)?;

    let icc_profile = match decoder.icc_profile() {
        Ok(profile) => profile,
        Err(error) => {
            log::debug!(
                target: "image_view::loader",
                "无法读取 {} 的 ICC profile：{error}",
                path.display()
            );
            None
        }
    };
    let decoder = decoder.apng().map_err(map_decode_error)?;
    // APNG 的 `num_plays` 即总播放轮数；0 表示无限。
    let loops = match decoder.loop_count() {
        image::metadata::LoopCount::Infinite => None,
        image::metadata::LoopCount::Finite(times) => Some(times.get()),
    };

    build_animation(path, caps, decoder.into_frames(), loops, icc_profile)
}

fn decode_gif_animation(path: &Path, caps: &Caps) -> Result<Option<DecodedAnimation>, LoadError> {
    use image::AnimationDecoder as _;

    let reader = std::io::BufReader::new(std::fs::File::open(path)?);
    let decoder = image::codecs::gif::GifDecoder::new(reader).map_err(map_decode_error)?;
    let (width, height) = decoder.dimensions();
    check_source_limits(width, height, caps, Purpose::Open)?;

    // Netscape 扩展里的循环次数 n 表示「再重复 n 次」，总播放 n+1 轮；0 表示无限。
    let loops = match decoder.loop_count() {
        image::metadata::LoopCount::Infinite => None,
        image::metadata::LoopCount::Finite(times) => Some(times.get().saturating_add(1)),
    };

    // GIF 没有 ICC profile（image 的 GifDecoder 也不提供读取入口）。
    build_animation(path, caps, decoder.into_frames(), loops, None)
}

fn decode_webp_animation(path: &Path, caps: &Caps) -> Result<Option<DecodedAnimation>, LoadError> {
    use image::AnimationDecoder as _;

    let reader = std::io::BufReader::new(std::fs::File::open(path)?);
    let mut decoder = image::codecs::webp::WebPDecoder::new(reader).map_err(map_decode_error)?;
    if !decoder.has_animation() {
        return Ok(None);
    }
    let (width, height) = decoder.dimensions();
    check_source_limits(width, height, caps, Purpose::Open)?;

    let icc_profile = match decoder.icc_profile() {
        Ok(profile) => profile,
        Err(error) => {
            log::debug!(
                target: "image_view::loader",
                "无法读取 {} 的 ICC profile：{error}",
                path.display()
            );
            None
        }
    };
    // WebP 的循环次数即总播放轮数；0 表示无限。
    let loops = match decoder.loop_count() {
        image::metadata::LoopCount::Infinite => None,
        image::metadata::LoopCount::Finite(times) => Some(times.get()),
    };

    build_animation(path, caps, decoder.into_frames(), loops, icc_profile)
}

/// 逐帧收集动画：每帧按 `Caps` 降采样并转 sRGB，累计像素超预算时整体退回静态图。
fn build_animation(
    path: &Path,
    caps: &Caps,
    frames: impl Iterator<Item = image::ImageResult<image::Frame>>,
    loops: Option<u32>,
    icc_profile: Option<Vec<u8>>,
) -> Result<Option<DecodedAnimation>, LoadError> {
    let mut decoded: Vec<AnimationFrame> = Vec::new();
    let mut size = [1usize; 2];
    let mut source_size = [1usize; 2];
    let mut color = ColorInfo::default();
    let mut total_pixels: u64 = 0;

    for frame in frames {
        let frame = frame.map_err(map_decode_error)?;
        let delay_ms = frame_delay_ms(frame.delay());
        let image = image::DynamicImage::ImageRgba8(frame.into_buffer());
        source_size = [image.width() as usize, image.height() as usize];

        let (target, rgba, info) = animation_pixels(caps, image, icc_profile.as_deref());
        total_pixels += target[0] as u64 * target[1] as u64;
        if total_pixels > caps.max_animation_pixels || decoded.len() >= caps.max_animation_frames {
            log::debug!(
                target: "image_view::loader",
                "动画超出预算，按首帧静态显示：{}（{} 帧，共 {} 像素）",
                path.display(),
                decoded.len() + 1,
                total_pixels
            );
            return Ok(None);
        }

        size = target;
        color = info;
        decoded.push(AnimationFrame { rgba, delay_ms });
    }

    if decoded.len() < 2 {
        return Ok(None);
    }

    Ok(Some(DecodedAnimation {
        size,
        source_size,
        // APNG / GIF / WebP 的解码器已统一输出 8-bit RGBA，动画不涉及 16-bit 源。
        source_depth: 8,
        color,
        loops,
        frames: decoded,
    }))
}

/// 帧间隔（毫秒）：`Delay` 以毫秒分数表示，按整数除法取整后钳制。
fn frame_delay_ms(delay: image::Delay) -> u32 {
    let (numer, denom) = delay.numer_denom_ms();
    let millis = numer.checked_div(denom).unwrap_or(numer);
    millis.clamp(MIN_FRAME_DELAY_MS, MAX_FRAME_DELAY_MS)
}

/// 单帧像素处理：与静态图共用尺寸上限与 ICC 路径，动画另用 Triangle 换取速度。
fn animation_pixels(
    caps: &Caps,
    image: image::DynamicImage,
    icc_profile: Option<&[u8]>,
) -> ([usize; 2], Pixels, ColorInfo) {
    let source_size = [image.width() as usize, image.height() as usize];
    let target = target_size(source_size, caps.max_side, caps.max_pixels);
    let image = if target != source_size {
        image.resize_exact(target[0] as u32, target[1] as u32, FilterType::Triangle)
    } else {
        image
    };
    let (pixels, info) = color::to_srgb_rgba(image, icc_profile);
    (target, pixels, info)
}

/// 解码前的统一尺寸检查：拒绝解压炸弹，并让预加载只挑小图。
fn check_source_limits(
    width: u32,
    height: u32,
    caps: &Caps,
    purpose: Purpose,
) -> Result<(), LoadError> {
    let source_pixels = u64::from(width) * u64::from(height);
    if source_pixels > caps.max_source_pixels {
        return Err(LoadError::TooLarge {
            w: width,
            h: height,
        });
    }
    if purpose == Purpose::Preload && source_pixels > caps.preload_max_source_pixels {
        return Err(LoadError::PreloadSkipped {
            w: width,
            h: height,
        });
    }
    Ok(())
}

fn decode_standard(path: &Path, caps: &Caps, purpose: Purpose) -> Result<Decoded, LoadError> {
    // 先只读头部尺寸，拒绝解压炸弹，避免为超大图分配完整缓冲。
    let reader = image::ImageReader::open(path)?.with_guessed_format()?;
    let (width, height) = reader.into_dimensions()?;
    check_source_limits(width, height, caps, purpose)?;

    let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
    reader.limits(caps.image_limits());
    let mut decoder = reader.into_decoder().map_err(map_decode_error)?;
    // JPEG/TIFF/WebP 的解码器会提供 EXIF 方向；其他格式默认无变换。
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    // ICC profile 必须在解码器被消费前读取。profile 缺失或读取失败不阻断解码。
    let icc_profile = match decoder.icc_profile() {
        Ok(profile) => profile,
        Err(error) => {
            log::debug!(
                target: "image_view::loader",
                "无法读取 {} 的 ICC profile：{error}",
                path.display()
            );
            None
        }
    };
    let mut image = image::DynamicImage::from_decoder(decoder).map_err(map_decode_error)?;
    image.apply_orientation(orientation);

    finish(path, caps, image, icc_profile)
}

#[cfg(feature = "avif")]
fn decode_avif(path: &Path, caps: &Caps, purpose: Purpose) -> Result<Decoded, LoadError> {
    use imgalifier_avif::{Av1Backend, Decoder, Limits};

    let data = std::fs::read(path)?;
    let limits = Limits {
        max_pixels: caps.max_source_pixels,
        ..Limits::default()
    };
    let decoder = Decoder::new(
        &data,
        Av1Backend {
            max_pixels: u32::try_from(caps.max_source_pixels).unwrap_or(u32::MAX),
            ..Av1Backend::default()
        },
        limits,
    )
    .map_err(|error| LoadError::Avif(error.to_string()))?;

    let info = decoder
        .info()
        .map_err(|error| LoadError::Avif(error.to_string()))?;
    check_source_limits(info.width, info.height, caps, purpose)?;

    // `colr` 中的 ICC 优先于 nclx；nclx 原色（如 Display P3）暂不做色域转换。
    let icc_profile = extract_icc(&decoder);
    let rgba = decoder
        .decode_primary()
        .map_err(|error| LoadError::Avif(error.to_string()))?;
    let image = image::RgbaImage::from_raw(rgba.width, rgba.height, rgba.data)
        .map(image::DynamicImage::ImageRgba8)
        .ok_or_else(|| LoadError::Avif("像素数据长度与尺寸不匹配".to_owned()))?;

    finish(path, caps, image, icc_profile)
}

/// 读取 HEIF 主图的 `colr` ICC profile（若存在）。
#[cfg(any(feature = "avif", feature = "heic"))]
fn extract_icc<B: imgalifier_heif::CodecBackend>(
    decoder: &imgalifier_heif::Decoder<'_, B>,
) -> Option<Vec<u8>> {
    use imgalifier_heif::{Colr, Property};

    let container = decoder.container();
    let item = container.primary().ok()?;
    container.find_property(item, |property| match property {
        Property::Colr(Colr::Icc(profile)) => Some(profile.clone()),
        _ => None,
    })
}

#[cfg(not(feature = "avif"))]
fn decode_avif(path: &Path, _caps: &Caps, _purpose: Purpose) -> Result<Decoded, LoadError> {
    Err(missing_decoder(path, Format::Avif))
}

#[cfg(feature = "heic")]
fn decode_heic(path: &Path, caps: &Caps, purpose: Purpose) -> Result<Decoded, LoadError> {
    use imgalifier_heif::hevc::HevcBackend;
    use imgalifier_heif::{Decoder, Limits};

    let data = std::fs::read(path)?;
    let limits = Limits {
        max_pixels: caps.max_source_pixels,
        ..Limits::default()
    };
    let decoder = Decoder::new(&data, HevcBackend::default(), limits)
        .map_err(|error| LoadError::Heic(error.to_string()))?;

    let info = decoder
        .info()
        .map_err(|error| LoadError::Heic(error.to_string()))?;
    check_source_limits(info.width, info.height, caps, purpose)?;

    // 解码器已应用裁剪、旋转与镜像；ICC 优先于 nclx。
    let icc_profile = extract_icc(&decoder);
    let rgba = decoder
        .decode_primary()
        .map_err(|error| LoadError::Heic(error.to_string()))?;
    let image = image::RgbaImage::from_raw(rgba.width, rgba.height, rgba.data)
        .map(image::DynamicImage::ImageRgba8)
        .ok_or_else(|| LoadError::Heic("像素数据长度与尺寸不匹配".to_owned()))?;

    finish(path, caps, image, icc_profile)
}

#[cfg(not(feature = "heic"))]
fn decode_heic(path: &Path, _caps: &Caps, _purpose: Purpose) -> Result<Decoded, LoadError> {
    Err(missing_decoder(path, Format::Heic))
}

/// JPEG XL：`jxl-oxide` 的 `image` 集成直接给出 `ImageDecoder`，走与通用路径相同的
/// 尺寸检查与 `finish` 收尾。
#[cfg(feature = "jxl")]
fn decode_jxl(path: &Path, caps: &Caps, purpose: Purpose) -> Result<Decoded, LoadError> {
    use jxl_oxide::integration::JxlDecoder;

    let reader = std::io::BufReader::new(std::fs::File::open(path)?);
    let mut decoder = JxlDecoder::new(reader).map_err(map_decode_error)?;
    // 头部已解析：先按声明尺寸做限额检查，再解像素。
    let (width, height) = decoder.dimensions();
    check_source_limits(width, height, caps, purpose)?;

    // `rendered_icc` 是解码器最终输出的色彩空间；CMYK 已在解码器内部请求为 sRGB。
    let icc_profile = match decoder.icc_profile() {
        Ok(profile) => profile,
        Err(error) => {
            log::debug!(
                target: "image_view::loader",
                "无法读取 {} 的 ICC profile：{error}",
                path.display()
            );
            None
        }
    };
    let image = image::DynamicImage::from_decoder(decoder).map_err(map_decode_error)?;

    finish(path, caps, image, icc_profile)
}

#[cfg(not(feature = "jxl"))]
fn decode_jxl(path: &Path, _caps: &Caps, _purpose: Purpose) -> Result<Decoded, LoadError> {
    Err(missing_decoder(path, Format::Jxl))
}

/// 共享的系统字体库；只在第一次解码 SVG 时扫描系统字体。
#[cfg(feature = "svg")]
fn shared_fontdb() -> std::sync::Arc<resvg::usvg::fontdb::Database> {
    use std::sync::OnceLock;

    static FONTDB: OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    FONTDB
        .get_or_init(|| {
            let mut database = resvg::usvg::fontdb::Database::new();
            database.load_system_fonts();
            log::debug!(
                target: "image_view::loader",
                "已载入 {} 个系统字体",
                database.len()
            );
            std::sync::Arc::new(database)
        })
        .clone()
}

/// SVG 的固有尺寸（逻辑像素）→ 参与限额检查的整数像素。
#[cfg(feature = "svg")]
fn svg_pixels(value: f32) -> u32 {
    if !value.is_finite() || value < 1.0 {
        1
    } else {
        value.round().min(u32::MAX as f32) as u32
    }
}

/// SVG 不能像位图那样「解码即得像素」：先解析出树与固有尺寸，
/// 按 `Caps` 做尺寸检查，再直接光栅化到目标尺寸（矢量缩放即渲染，无需二次重采样）。
#[cfg(feature = "svg")]
fn decode_svg(path: &Path, caps: &Caps, purpose: Purpose) -> Result<Decoded, LoadError> {
    use resvg::tiny_skia;

    let data = std::fs::read(path)?;
    let tree = parse_svg(path, &data)?;

    // usvg 的尺寸解析：优先 width/height，缺失时回退 viewBox；再缺失会按内容边界推导，
    // 与规范的 300×150 默认视口不一致，因此自行检测并覆盖（内容坐标不受影响）。
    let declared = tree.size();
    let intrinsic: [f32; 2] = if has_intrinsic_size(&data) {
        [declared.width(), declared.height()]
    } else {
        [300.0, 150.0]
    };
    let width = svg_pixels(intrinsic[0]);
    let height = svg_pixels(intrinsic[1]);
    check_source_limits(width, height, caps, purpose)?;

    let source_size = [width as usize, height as usize];
    let target = target_size(source_size, caps.max_side, caps.max_pixels);
    let target_w = target[0] as u32;
    let target_h = target[1] as u32;

    let mut pixmap = tiny_skia::Pixmap::new(target_w, target_h).ok_or(LoadError::OutOfMemory)?;
    let scale_x = target_w as f32 / intrinsic[0];
    let scale_y = target_h as f32 / intrinsic[1];
    {
        let mut render_target = pixmap.as_mut();
        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(scale_x, scale_y),
            &mut render_target,
        );
    }

    // tiny-skia 输出预乘 alpha，纹理管线需要非预乘的 sRGB RGBA。
    let rgba = pixmap.take_demultiplied();

    Ok(Decoded {
        size: target,
        source_size,
        rgba: Pixels::Rgba8(rgba),
        source_depth: 8,
        color: ColorInfo::default(),
    })
}

/// 解析 SVG 树：外链资源按文件所在目录解析，相对尺寸按规范的 300×150 默认视口推导。
#[cfg(feature = "svg")]
fn parse_svg(path: &Path, data: &[u8]) -> Result<resvg::usvg::Tree, LoadError> {
    use resvg::usvg;

    let options = usvg::Options {
        // 相对外链按 SVG 所在目录解析；usvg 只读本地文件，不发网络请求。
        resources_dir: path.parent().map(Path::to_path_buf),
        fontdb: shared_fontdb(),
        // 规范的默认替换尺寸：尺寸属性为相对值时按 300×150 推导。
        default_size: usvg::Size::from_wh(300.0, 150.0).unwrap(),
        ..usvg::Options::default()
    };

    match usvg::Tree::from_data(data, &options) {
        Ok(tree) => Ok(tree),
        Err(usvg::Error::SvgzFeatureNotEnabled) => Err(LoadError::SvgzUnsupported),
        Err(error) => Err(LoadError::Svg(error.to_string())),
    }
}

/// 根元素是否声明了 `width`/`height`/`viewBox`；三者都没有时按规范套用 300×150 默认视口。
#[cfg(feature = "svg")]
fn has_intrinsic_size(data: &[u8]) -> bool {
    use resvg::usvg::roxmltree;

    let Ok(text) = std::str::from_utf8(data) else {
        return true; // 非 UTF-8（如 gzip）交给解析报错，此处不做推断
    };
    let Ok(document) = roxmltree::Document::parse(text) else {
        return true;
    };
    let Some(root) = document.root().children().find(|node| node.is_element()) else {
        return true;
    };
    root.attributes()
        .any(|attribute| matches!(attribute.name(), "width" | "height" | "viewBox"))
}

#[cfg(not(feature = "svg"))]
fn decode_svg(path: &Path, _caps: &Caps, _purpose: Purpose) -> Result<Decoded, LoadError> {
    Err(missing_decoder(path, Format::Svg))
}

/// 解码后的公共出口：降采样、位深记录与 ICC → sRGB 转换。
fn finish(
    path: &Path,
    caps: &Caps,
    image: image::DynamicImage,
    icc_profile: Option<Vec<u8>>,
) -> Result<Decoded, LoadError> {
    let source_size = [image.width() as usize, image.height() as usize];
    let target = target_size(source_size, caps.max_side, caps.max_pixels);

    let image = if target != source_size {
        let filter = if source_size[0] * source_size[1] >= 50_000_000 {
            FilterType::Triangle
        } else {
            FilterType::Lanczos3
        };
        image.resize_exact(target[0] as u32, target[1] as u32, filter)
    } else {
        image
    };

    // 位深跟随源图：16-bit 在转换与降采样后仍保持 16-bit，不再提前量化。
    let source_depth = color::source_depth(&image);
    let (pixels, color_info) = color::to_srgb_rgba(image, icc_profile.as_deref());
    if color_info.converted {
        log::debug!(
            target: "image_view::loader",
            "已转换到 sRGB：{} ({})",
            path.display(),
            color_info.source_label().unwrap_or("未命名 profile")
        );
    }

    Ok(Decoded {
        size: target,
        source_size,
        rgba: pixels,
        source_depth,
        color: color_info,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 把解码结果量化为 8-bit 字节，供 8-bit 源测试的像素断言使用。
    fn rgba8(decoded: &Decoded) -> Vec<u8> {
        decoded.rgba.clone().into_rgba8()
    }

    /// 把动画帧量化为 8-bit 字节。
    fn frame_rgba8(frame: &AnimationFrame) -> Vec<u8> {
        frame.rgba.clone().into_rgba8()
    }

    #[test]
    fn target_size_respects_max_side() {
        assert_eq!(target_size([10_000, 5_000], 8192, 40_000_000), [8192, 4096]);
    }

    #[test]
    fn target_size_respects_max_pixels() {
        let target = target_size([20_000, 10_000], 8192, 40_000_000);
        assert!(target[0] * target[1] <= 40_000_000);
        let source_ratio = 20_000.0 / 10_000.0;
        let target_ratio = target[0] as f64 / target[1] as f64;
        assert!((source_ratio - target_ratio).abs() < 0.01);
    }

    #[test]
    fn small_images_are_not_upscaled() {
        assert_eq!(target_size([640, 480], 8192, 40_000_000), [640, 480]);
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "image-view-loader-test-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    /// 写一张 40×20（左红右蓝）的 JPEG，并在 SOI 后插入 EXIF orientation。
    fn write_oriented_jpeg(path: &Path, orientation: u8) {
        let image = image::RgbImage::from_fn(40, 20, |x, _| {
            if x < 20 {
                image::Rgb([255, 0, 0])
            } else {
                image::Rgb([0, 0, 255])
            }
        });
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
            .encode(&image.as_raw()[..], 40, 20, image::ExtendedColorType::Rgb8)
            .expect("encode jpeg");

        // 最小 TIFF/EXIF：一个 Orientation (0x0112) SHORT 条目。
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II");
        tiff.extend_from_slice(&42u16.to_le_bytes());
        tiff.extend_from_slice(&8u32.to_le_bytes());
        tiff.extend_from_slice(&1u16.to_le_bytes());
        tiff.extend_from_slice(&0x0112u16.to_le_bytes());
        tiff.extend_from_slice(&3u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&u16::from(orientation).to_le_bytes());
        tiff.extend_from_slice(&[0, 0]);
        tiff.extend_from_slice(&0u32.to_le_bytes());

        let mut app1 = Vec::new();
        app1.extend_from_slice(b"Exif\0\0");
        app1.extend_from_slice(&tiff);
        let length = (app1.len() + 2) as u16;

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&jpeg[..2]);
        bytes.extend_from_slice(&[0xFF, 0xE1]);
        bytes.extend_from_slice(&length.to_be_bytes());
        bytes.extend_from_slice(&app1);
        bytes.extend_from_slice(&jpeg[2..]);
        std::fs::write(path, bytes).expect("write jpeg");
    }

    #[test]
    fn thumbnail_decode_limits_the_longest_side() {
        let dir = temp_dir("thumbnail");
        let path = dir.join("wide.png");
        image::RgbImage::from_pixel(400, 200, image::Rgb([10, 20, 30]))
            .save(&path)
            .expect("write png");

        let decoded = decode_thumbnail(&path, 128).expect("decode thumbnail");
        assert_eq!(decoded.size, [128, 64]);
        assert_eq!(rgba8(&decoded).len(), decoded.size[0] * decoded.size[1] * 4);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn exif_orientation_is_applied() {
        let dir = temp_dir("exif");
        let path = dir.join("oriented.jpg");
        write_oriented_jpeg(&path, 6); // 6 = 顺时针 90°

        let decoded = decode(&path, &Caps::default(), Purpose::Open).expect("decode oriented jpeg");
        assert_eq!(decoded.source_size, [20, 40]);
        assert_eq!(decoded.size, [20, 40]);

        // 原图左红右蓝，旋转后应为上红下蓝。
        let rgba = image::RgbaImage::from_raw(20, 40, rgba8(&decoded)).expect("rgba");
        assert!(rgba.get_pixel(0, 0).0[0] > 200, "top-left should be red");
        assert!(
            rgba.get_pixel(0, 39).0[2] > 200,
            "bottom-left should be blue"
        );

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    fn request(path: &str, purpose: Purpose) -> Request {
        Request {
            generation: 1,
            path: PathBuf::from(path),
            caps: Caps::default(),
            purpose,
        }
    }

    fn paths(requests: &[Request]) -> Vec<String> {
        requests
            .iter()
            .map(|request| request.path.display().to_string())
            .collect()
    }

    #[test]
    fn backlog_keeps_only_the_latest_open() {
        let mut pending = vec![];
        merge_pending(
            &mut pending,
            vec![
                request("a.jpg", Purpose::Open),
                request("b.jpg", Purpose::Preload),
                request("c.jpg", Purpose::Open),
            ],
        );

        assert_eq!(paths(&pending), ["c.jpg"]);
    }

    #[test]
    fn new_open_request_cancels_queued_preloads() {
        let mut pending = vec![request("b.jpg", Purpose::Preload)];
        merge_pending(&mut pending, vec![request("a.jpg", Purpose::Open)]);

        assert_eq!(paths(&pending), ["a.jpg"]);
    }

    #[test]
    fn preloads_are_deduplicated_and_keep_order() {
        let mut pending = vec![request("next.jpg", Purpose::Preload)];
        merge_pending(
            &mut pending,
            vec![
                request("prev.jpg", Purpose::Preload),
                request("next.jpg", Purpose::Preload),
            ],
        );

        assert_eq!(paths(&pending), ["next.jpg", "prev.jpg"]);
    }

    #[test]
    fn queued_open_is_not_replaced_by_preloads() {
        let mut pending = vec![request("a.jpg", Purpose::Open)];
        merge_pending(&mut pending, vec![request("b.jpg", Purpose::Preload)]);

        assert_eq!(paths(&pending), ["a.jpg", "b.jpg"]);
    }

    fn write_png_with_icc(path: &Path, image: &image::DynamicImage, icc: Vec<u8>) {
        use image::ImageEncoder as _;

        let mut bytes = Vec::new();
        let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
        encoder.set_icc_profile(icc).expect("set ICC profile");
        let rgba = image.to_rgba8();
        encoder
            .write_image(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                image::ExtendedColorType::Rgba8,
            )
            .expect("encode png");
        std::fs::write(path, bytes).expect("write png");
    }

    #[test]
    fn embedded_icc_profile_is_read_and_converted() {
        let dir = temp_dir("icc");
        let path = dir.join("profile.png");
        // 线性 RGB 的中灰 128 转换到 sRGB 后应接近 188。
        let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            4,
            4,
            image::Rgb([128, 128, 128]),
        ));
        write_png_with_icc(&path, &image, iv_metadata::test_linear_rgb_icc());

        let decoded = decode(&path, &Caps::default(), Purpose::Open).expect("decode png");
        assert!(decoded.color.converted);
        assert_eq!(decoded.source_depth, 8);
        assert!(
            rgba8(&decoded)[0].abs_diff(128) >= 40,
            "应转换到约 188，实际 {}",
            rgba8(&decoded)[0]
        );

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn sixteen_bit_png_keeps_depth_and_converts() {
        let dir = temp_dir("icc-16");
        let path = dir.join("profile-16.png");
        let image = image::DynamicImage::ImageRgba16(image::ImageBuffer::from_pixel(
            4,
            4,
            image::Rgba([0x8000u16, 0x8000, 0x8000, 0xFFFF]),
        ));
        // PngEncoder 从 DynamicImage 取 8-bit 数据；直接写 16-bit 缓冲。
        use image::ImageEncoder as _;
        let mut bytes = Vec::new();
        let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
        encoder
            .set_icc_profile(iv_metadata::test_linear_rgb_icc())
            .expect("set ICC profile");
        let rgba = image.as_rgba16().expect("rgba16");
        encoder
            .write_image(
                bytemuck::cast_slice(rgba.as_raw()),
                rgba.width(),
                rgba.height(),
                image::ExtendedColorType::Rgba16,
            )
            .expect("encode 16-bit png");
        std::fs::write(&path, bytes).expect("write png");

        let decoded = decode(&path, &Caps::default(), Purpose::Open).expect("decode png");
        assert!(decoded.color.converted);
        assert_eq!(decoded.source_depth, 16);
        assert!(
            matches!(decoded.rgba, Pixels::Rgba16(_)),
            "16-bit 源不应在解码阶段量化到 8-bit"
        );
        assert!(rgba8(&decoded)[0] > 120, "实际 {}", rgba8(&decoded)[0]);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn farbfeld_is_decoded_as_16bit() {
        use image::ImageEncoder as _;

        let dir = temp_dir("farbfeld");
        let path = dir.join("sample.ff");
        // 2×2 RGBA16：红、绿、蓝、半透明白。farbfeld 没有 ICC，位深必须保留。
        let pixels: [[u16; 4]; 4] = [
            [0xffff, 0, 0, 0xffff],
            [0, 0xffff, 0, 0xffff],
            [0, 0, 0xffff, 0xffff],
            [0xffff, 0xffff, 0xffff, 0x8080],
        ];
        let mut data = Vec::with_capacity(pixels.len() * 8);
        for pixel in pixels {
            for channel in pixel {
                data.extend(channel.to_ne_bytes());
            }
        }
        let file = std::fs::File::create(&path).expect("create ff");
        image::codecs::farbfeld::FarbfeldEncoder::new(std::io::BufWriter::new(file))
            .write_image(&data, 2, 2, image::ExtendedColorType::Rgba16)
            .expect("encode farbfeld");

        let decoded = decode(&path, &Caps::default(), Purpose::Open).expect("decode ff");
        assert_eq!(decoded.source_size, [2, 2]);
        assert_eq!(decoded.size, [2, 2]);
        assert_eq!(decoded.source_depth, 16);
        let Pixels::Rgba16(values) = &decoded.rgba else {
            panic!("farbfeld 是 16-bit 源，解码结果不应量化");
        };
        assert_eq!(&values[..4], &[0xffff, 0, 0, 0xffff]);
        assert_eq!(&values[4..8], &[0, 0xffff, 0, 0xffff]);
        assert_eq!(&rgba8(&decoded)[..4], &[255, 0, 0, 255]);
        assert_eq!(rgba8(&decoded)[15], 128);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn oversized_preload_is_skipped_but_open_succeeds() {
        let dir = temp_dir("preload-skip");
        let path = dir.join("small.png");
        image::RgbaImage::from_pixel(40, 20, image::Rgba([10, 20, 30, 255]))
            .save(&path)
            .expect("write png");

        let caps = Caps {
            preload_max_source_pixels: 100,
            ..Caps::default()
        };

        assert!(matches!(
            decode(&path, &caps, Purpose::Preload),
            Err(LoadError::PreloadSkipped { .. })
        ));
        assert!(decode(&path, &caps, Purpose::Open).is_ok());

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn format_is_selected_by_extension() {
        assert_eq!(format_for(Path::new("a.AVIF")), Format::Avif);
        assert_eq!(format_for(Path::new("a.heic")), Format::Heic);
        assert_eq!(format_for(Path::new("a.HEIF")), Format::Heic);
        assert_eq!(format_for(Path::new("a.jxl")), Format::Jxl);
        assert_eq!(format_for(Path::new("a.JXL")), Format::Jxl);
        assert_eq!(format_for(Path::new("a.SVG")), Format::Svg);
        assert_eq!(format_for(Path::new("a.svg")), Format::Svg);
        assert_eq!(format_for(Path::new("a.png")), Format::Standard);
        assert_eq!(format_for(Path::new("no-extension")), Format::Standard);
    }

    #[test]
    fn missing_decoder_error_mentions_format_and_extension() {
        let error = missing_decoder(Path::new("photo.HEIF"), Format::Heic);
        let text = error.to_string();
        assert!(text.contains("HEIC"), "实际：{text}");
        assert!(text.contains("HEIF"), "实际：{text}");
        assert!(text.contains("feature"), "实际：{text}");
    }

    #[cfg(feature = "avif")]
    #[test]
    fn corrupt_avif_reports_decoder_error() {
        let dir = temp_dir("avif-corrupt");
        let path = dir.join("broken.avif");
        std::fs::write(&path, b"not an avif file").expect("write fixture");

        assert!(matches!(
            decode(&path, &Caps::default(), Purpose::Open),
            Err(LoadError::Avif(_))
        ));

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[cfg(feature = "heic")]
    #[test]
    fn corrupt_heic_reports_decoder_error() {
        let dir = temp_dir("heic-corrupt");
        let path = dir.join("broken.heic");
        std::fs::write(&path, b"not a heic file").expect("write fixture");

        assert!(matches!(
            decode(&path, &Caps::default(), Purpose::Open),
            Err(LoadError::Heic(_))
        ));

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[cfg(not(feature = "avif"))]
    #[test]
    fn avif_without_decoder_reports_missing_decoder() {
        let dir = temp_dir("avif-missing");
        let path = dir.join("photo.avif");
        std::fs::write(&path, b"").expect("write fixture");

        assert!(matches!(
            decode(&path, &Caps::default(), Purpose::Open),
            Err(LoadError::MissingDecoder { .. })
        ));

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[cfg(not(feature = "heic"))]
    #[test]
    fn heic_without_decoder_reports_missing_decoder() {
        let dir = temp_dir("heic-missing");
        let path = dir.join("photo.heif");
        std::fs::write(&path, b"").expect("write fixture");

        assert!(matches!(
            decode(&path, &Caps::default(), Purpose::Open),
            Err(LoadError::MissingDecoder { .. })
        ));

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    /// 3×3 sRGB 无损 JPEG XL 测试向量（66 字节，取自 libjxl/jxl-rs，Apache-2.0）。
    #[cfg(feature = "jxl")]
    const JXL_LOSSLESS_3X3: &[u8] = &[
        0xff, 0x0a, 0x10, 0x10, 0x10, 0x50, 0xb9, 0xb7, 0x8a, 0x22, 0x08, 0x04, 0x01, 0x00, 0xc8,
        0x00, 0x4b, 0x18, 0x8b, 0x15, 0x52, 0x6c, 0x00, 0xfc, 0xe7, 0xed, 0x42, 0xd3, 0x04, 0x84,
        0x17, 0x08, 0x82, 0x9d, 0xb3, 0x7d, 0x13, 0x1d, 0x60, 0xe8, 0x7e, 0x7d, 0xec, 0x06, 0xe8,
        0xd1, 0x73, 0xcd, 0x6b, 0x7d, 0xeb, 0xf4, 0xba, 0x8e, 0x02, 0x03, 0x7f, 0xd9, 0xfd, 0x1e,
        0xf1, 0x94, 0xca, 0x45, 0x20, 0x00,
    ];

    #[cfg(feature = "jxl")]
    #[test]
    fn jxl_is_decoded() {
        let dir = temp_dir("jxl");
        let path = dir.join("sample.jxl");
        std::fs::write(&path, JXL_LOSSLESS_3X3).expect("write fixture");

        let decoded = decode(&path, &Caps::default(), Purpose::Open).expect("decode jxl");
        assert_eq!(decoded.source_size, [3, 3]);
        assert_eq!(decoded.size, [3, 3]);
        assert_eq!(decoded.source_depth, 8);
        assert_eq!(rgba8(&decoded).len(), 3 * 3 * 4);
        assert!(
            rgba8(&decoded)
                .iter()
                .skip(3)
                .step_by(4)
                .all(|alpha| *alpha == 255),
            "3×3 测试向量不透明"
        );

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[cfg(feature = "jxl")]
    #[test]
    fn corrupt_jxl_reports_decode_error() {
        let dir = temp_dir("jxl-corrupt");
        let path = dir.join("broken.jxl");
        std::fs::write(&path, b"not a jpeg xl file").expect("write fixture");

        assert!(matches!(
            decode(&path, &Caps::default(), Purpose::Open),
            Err(LoadError::Decode(_))
        ));

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[cfg(not(feature = "jxl"))]
    #[test]
    fn jxl_without_decoder_reports_missing_decoder() {
        let dir = temp_dir("jxl-missing");
        let path = dir.join("photo.jxl");
        std::fs::write(&path, b"").expect("write fixture");

        assert!(matches!(
            decode(&path, &Caps::default(), Purpose::Open),
            Err(LoadError::MissingDecoder { .. })
        ));

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    /// 写入一个完整的 SVG 文档。
    #[cfg(feature = "svg")]
    fn write_svg(path: &Path, document: &str) {
        std::fs::write(path, document).expect("write svg");
    }

    #[cfg(feature = "svg")]
    #[test]
    fn svg_is_rasterized_at_declared_size() {
        let dir = temp_dir("svg");
        let path = dir.join("rect.svg");
        write_svg(
            &path,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="#ff0000"/></svg>"##,
        );

        let decoded = decode(&path, &Caps::default(), Purpose::Open).expect("decode svg");
        assert_eq!(decoded.source_size, [40, 20]);
        assert_eq!(decoded.size, [40, 20]);
        assert_eq!(rgba8(&decoded).len(), 40 * 20 * 4);
        assert_eq!(decoded.source_depth, 8);

        // 中心像素应为纯红且不透明（预乘 alpha 已还原）。
        let offset = (10 * 40 + 20) * 4;
        assert_eq!(&rgba8(&decoded)[offset..offset + 4], &[255, 0, 0, 255]);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[cfg(feature = "svg")]
    #[test]
    fn svg_is_rasterized_down_to_caps_without_resampling() {
        let dir = temp_dir("svg-scale");
        let path = dir.join("big.svg");
        write_svg(
            &path,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="200"><rect width="400" height="200" fill="#00ff00"/></svg>"##,
        );

        let caps = Caps {
            max_side: 64,
            max_pixels: 1024,
            ..Caps::default()
        };
        let decoded = decode(&path, &caps, Purpose::Open).expect("decode svg");

        // 原尺寸只用于记录与限额检查，光栅化直接落在目标尺寸上。
        assert_eq!(decoded.source_size, [400, 200]);
        assert_eq!(decoded.size, target_size([400, 200], 64, 1024));
        assert!(decoded.size[0] < 400);
        assert_eq!(rgba8(&decoded).len(), decoded.size[0] * decoded.size[1] * 4);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[cfg(feature = "svg")]
    #[test]
    fn svg_declared_size_is_checked_against_source_limit() {
        let dir = temp_dir("svg-limit");
        let path = dir.join("huge.svg");
        write_svg(
            &path,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="200"><rect width="400" height="200"/></svg>"##,
        );

        let caps = Caps {
            max_source_pixels: 1_000,
            ..Caps::default()
        };
        assert!(matches!(
            decode(&path, &caps, Purpose::Open),
            Err(LoadError::TooLarge { w: 400, h: 200 })
        ));

        let preload_caps = Caps {
            preload_max_source_pixels: 1_000,
            ..Caps::default()
        };
        assert!(matches!(
            decode(&path, &preload_caps, Purpose::Preload),
            Err(LoadError::PreloadSkipped { .. })
        ));

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[cfg(feature = "svg")]
    #[test]
    fn svg_without_width_and_height_falls_back_to_view_box() {
        let dir = temp_dir("svg-viewbox");
        let path = dir.join("viewbox.svg");
        write_svg(
            &path,
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 60 30"><rect width="60" height="30" fill="#0000ff"/></svg>"##,
        );

        let decoded = decode(&path, &Caps::default(), Purpose::Open).expect("decode svg");
        assert_eq!(decoded.source_size, [60, 30]);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[cfg(feature = "svg")]
    #[test]
    fn svg_without_any_size_uses_default_viewport() {
        let dir = temp_dir("svg-default");
        let path = dir.join("default.svg");
        // 内容只有 10×10，规范要求视口仍是 300×150（usvg 会按内容边界推导成 10×10）。
        write_svg(
            &path,
            r##"<svg xmlns="http://www.w3.org/2000/svg"><rect width="10" height="10" fill="#ff0000"/></svg>"##,
        );

        let decoded = decode(&path, &Caps::default(), Purpose::Open).expect("decode svg");
        assert_eq!(decoded.source_size, [300, 150]);
        assert_eq!(decoded.size, [300, 150]);
        assert_eq!(rgba8(&decoded).len(), 300 * 150 * 4);

        // 内容按原坐标落在左上角，视口其余部分透明。
        assert_eq!(&rgba8(&decoded)[0..4], &[255, 0, 0, 255]);
        let empty = (100 * 300 + 200) * 4;
        assert_eq!(&rgba8(&decoded)[empty..empty + 4], &[0, 0, 0, 0]);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[cfg(feature = "svg")]
    #[test]
    fn intrinsic_size_detection_looks_at_root_attributes() {
        let sizeless =
            r##"<svg xmlns="http://www.w3.org/2000/svg"><rect width="10" height="10"/></svg>"##;
        assert!(!has_intrinsic_size(sizeless.as_bytes()));

        let with_viewbox =
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 60 30"><rect/></svg>"##;
        assert!(has_intrinsic_size(with_viewbox.as_bytes()));

        let with_height = r##"<svg xmlns="http://www.w3.org/2000/svg" height="20"><rect/></svg>"##;
        assert!(has_intrinsic_size(with_height.as_bytes()));

        // 属性出现在内层元素上不算根元素声明了尺寸。
        let inner_only = r##"<svg xmlns="http://www.w3.org/2000/svg"><g viewBox="0 0 1 1"><rect width="1" height="1"/></g></svg>"##;
        assert!(!has_intrinsic_size(inner_only.as_bytes()));

        // 解析不了的输入不做推断，交给后续解析报错。
        assert!(has_intrinsic_size(b""));
        assert!(has_intrinsic_size(b"\xff\xfe<svg/>"));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn complex_svg_rasterizes_with_content() {
        // 按真实应用图标的构造成分（viewBox + 渐变 defs + 多段路径 + 高饱和色块）
        // 现场生成夹具，避免核心 crate 的单测依赖仓库级设计资源。
        const ICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024"
     viewBox="0 0 1024 1024" role="img" aria-label="test icon">
  <defs>
    <linearGradient id="bg" x1="0.12" y1="0" x2="0.86" y2="1">
      <stop offset="0" stop-color="#75C4FF"/>
      <stop offset="1" stop-color="#3D62E4"/>
    </linearGradient>
    <radialGradient id="gloss" cx="0.32" cy="0.08" r="0.86">
      <stop offset="0" stop-color="#FFFFFF" stop-opacity="0.20"/>
      <stop offset="1" stop-color="#FFFFFF" stop-opacity="0"/>
    </radialGradient>
  </defs>
  <rect width="1024" height="1024" rx="224" fill="url(#bg)"/>
  <rect width="1024" height="1024" rx="224" fill="url(#gloss)"/>
  <path d="M256 640 L448 448 L576 576 L704 384 L832 640 Z" fill="#FFFFFF" fill-opacity="0.92"/>
  <rect x="192" y="192" width="128" height="128" fill="#FF0000"/>
</svg>"##;
        let dir = temp_dir("svg-icon");
        let path = dir.join("icon.svg");
        std::fs::write(&path, ICON).expect("write fixture");

        let decoded = decode(&path, &Caps::default(), Purpose::Open).expect("decode icon svg");

        assert_eq!(decoded.source_size, [1024, 1024]);
        assert_eq!(decoded.size, [1024, 1024]);
        assert_eq!(rgba8(&decoded).len(), 1024 * 1024 * 4);
        assert!(
            rgba8(&decoded)
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] > 0),
            "渲染结果不应全透明"
        );

        // 左上 128×128 纯红色块的中心点，用于确认填充与路径确实参与了栅格化。
        let index = (256 * 1024 + 256) * 4;
        let pixel = &rgba8(&decoded)[index..index + 4];
        assert!(
            pixel[0] > 200 && pixel[1] < 60 && pixel[2] < 60 && pixel[3] > 200,
            "色块中心应为不透明红色，实际为 {pixel:?}"
        );

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[cfg(feature = "svg")]
    #[test]
    fn gzip_compressed_svg_content_is_accepted() {
        // usvg 默认启用 svgz：`.svg` 文件内部若为 gzip 数据也能解出（`.svgz` 扩展名仍不进白名单）。
        let dir = temp_dir("svg-gzip");
        let path = dir.join("packed.svg");
        const GZIP_SVG: [u8; 105] = [
            31, 139, 8, 0, 0, 0, 0, 0, 0, 10, 117, 204, 75, 10, 128, 48, 12, 5, 192, 171, 148, 184,
            55, 207, 186, 147, 182, 151, 81, 251, 129, 250, 193, 6, 227, 241, 197, 3, 184, 30, 24,
            215, 238, 100, 158, 173, 238, 205, 83, 22, 57, 39, 102, 85, 237, 117, 236, 143, 43,
            177, 5, 192, 237, 78, 100, 180, 44, 146, 61, 89, 144, 201, 107, 73, 89, 60, 13, 160,
            224, 174, 117, 150, 31, 52, 177, 212, 234, 169, 3, 98, 4, 136, 131, 251, 170, 240, 2,
            106, 86, 75, 44, 114, 0, 0, 0,
        ];
        std::fs::write(&path, GZIP_SVG).expect("write fixture");

        let decoded = decode(&path, &Caps::default(), Purpose::Open).expect("decode gzip svg");
        assert_eq!(decoded.source_size, [20, 10]);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[cfg(feature = "svg")]
    #[test]
    fn corrupt_svg_reports_parse_error() {
        let dir = temp_dir("svg-corrupt");
        let path = dir.join("broken.svg");
        std::fs::write(&path, b"<svg xmlns='http://www.w3.org/2000/svg'>").expect("write fixture");

        assert!(matches!(
            decode(&path, &Caps::default(), Purpose::Open),
            Err(LoadError::Svg(_))
        ));

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[cfg(not(feature = "svg"))]
    #[test]
    fn svg_without_decoder_reports_missing_decoder() {
        let dir = temp_dir("svg-missing");
        let path = dir.join("drawing.svg");
        std::fs::write(&path, b"").expect("write fixture");

        assert!(matches!(
            decode(&path, &Caps::default(), Purpose::Open),
            Err(LoadError::MissingDecoder { .. })
        ));

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    /// 写一个 4×2 的多帧 GIF；`repeat` 为空时写成无限循环。
    fn write_animated_gif(path: &Path, delays_ms: &[u32], colors: &[[u8; 4]], repeat: Option<u16>) {
        use image::codecs::gif::{GifEncoder, Repeat};

        let mut bytes = Vec::new();
        let mut encoder = GifEncoder::new(&mut bytes);
        encoder
            .set_repeat(match repeat {
                Some(times) => Repeat::Finite(times),
                None => Repeat::Infinite,
            })
            .expect("set repeat");

        let frames = delays_ms.iter().zip(colors.iter()).map(|(delay, color)| {
            let buffer = image::RgbaImage::from_pixel(4, 2, image::Rgba(*color));
            image::Frame::from_parts(buffer, 0, 0, image::Delay::from_numer_denom_ms(*delay, 1))
        });
        encoder.encode_frames(frames).expect("encode gif");
        drop(encoder);
        std::fs::write(path, bytes).expect("write gif");
    }

    fn u24(value: u32) -> [u8; 3] {
        let bytes = value.to_le_bytes();
        [bytes[0], bytes[1], bytes[2]]
    }

    /// 手写最小动画 WebP 容器：VP8X + ANIM + 每帧 ANMF（内含无损 VP8L 子块）。
    ///
    /// image crate 只能编码单帧 WebP，因此容器由测试自行拼装；子块的 bitstream
    /// 仍然来自 `WebPEncoder`，保证是合法的 VP8L 数据。
    fn write_animated_webp(path: &Path, canvas: (u32, u32), durations: &[u32], loops: u16) {
        fn vp8l_chunk(canvas: (u32, u32), color: [u8; 4]) -> Vec<u8> {
            let buffer = image::RgbaImage::from_pixel(canvas.0, canvas.1, image::Rgba(color));
            let mut encoded = Vec::new();
            image::codecs::webp::WebPEncoder::new_lossless(&mut encoded)
                .encode(
                    buffer.as_raw(),
                    canvas.0,
                    canvas.1,
                    image::ExtendedColorType::Rgba8,
                )
                .expect("encode lossless webp");
            let offset = encoded
                .windows(4)
                .position(|window| window == b"VP8L")
                .expect("VP8L chunk");
            encoded[offset..].to_vec()
        }

        let mut chunks: Vec<Vec<u8>> = Vec::new();

        let mut vp8x = Vec::new();
        vp8x.extend_from_slice(b"VP8X");
        vp8x.extend_from_slice(&10u32.to_le_bytes());
        vp8x.push(0x02); // Animation
        vp8x.extend_from_slice(&[0, 0, 0]);
        vp8x.extend_from_slice(&u24(canvas.0 - 1));
        vp8x.extend_from_slice(&u24(canvas.1 - 1));
        chunks.push(vp8x);

        let mut anim = Vec::new();
        anim.extend_from_slice(b"ANIM");
        anim.extend_from_slice(&6u32.to_le_bytes());
        anim.extend_from_slice(&[0, 0, 0, 0]); // 背景色
        anim.extend_from_slice(&loops.to_le_bytes());
        chunks.push(anim);

        for (index, duration) in durations.iter().enumerate() {
            let color = [u8::try_from(index * 40).unwrap_or(0), 0, 0, 255];
            let bitstream = vp8l_chunk(canvas, color);
            let mut payload = Vec::new();
            payload.extend_from_slice(&u24(0)); // Frame X / 2
            payload.extend_from_slice(&u24(0)); // Frame Y / 2
            payload.extend_from_slice(&u24(canvas.0 - 1));
            payload.extend_from_slice(&u24(canvas.1 - 1));
            payload.extend_from_slice(&u24(*duration));
            payload.push(0); // 不处置、参与 alpha 混合
            payload.extend_from_slice(&bitstream);

            let mut anmf = Vec::new();
            anmf.extend_from_slice(b"ANMF");
            anmf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            anmf.extend_from_slice(&payload);
            if payload.len() % 2 == 1 {
                anmf.push(0);
            }
            chunks.push(anmf);
        }

        let body_len = chunks.iter().map(Vec::len).sum::<usize>() + 4;
        let mut file = Vec::new();
        file.extend_from_slice(b"RIFF");
        file.extend_from_slice(&(body_len as u32).to_le_bytes());
        file.extend_from_slice(b"WEBP");
        for chunk in &chunks {
            file.extend_from_slice(chunk);
        }
        std::fs::write(path, file).expect("write webp");
    }

    /// 用 `png` crate 写一个多帧 APNG：每帧铺一种颜色，`plays` 为总播放轮数
    /// （0 = 无限）。`png` 只是测试依赖，生产解码仍走 `image`。
    fn write_animated_png(path: &Path, delays_ms: &[u16], colors: &[[u8; 4]], plays: u16) {
        use png::{BitDepth, ColorType, Encoder};

        let (width, height) = (4u32, 2u32);
        let file = std::fs::File::create(path).expect("create apng");
        let mut encoder = Encoder::new(std::io::BufWriter::new(file), width, height);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);
        encoder
            .set_animated(
                u32::try_from(delays_ms.len()).expect("frame count"),
                u32::from(plays),
            )
            .expect("configure apng");
        let mut writer = encoder.write_header().expect("write apng header");

        for (delay, color) in delays_ms.iter().zip(colors) {
            writer
                .set_frame_delay(*delay, 1_000)
                .expect("set frame delay");
            let mut pixels = Vec::with_capacity((width * height * 4) as usize);
            for _ in 0..width * height {
                pixels.extend_from_slice(color);
            }
            writer.write_image_data(&pixels).expect("write apng frame");
        }
        writer.finish().expect("finish apng");
    }

    #[test]
    fn frame_delays_are_clamped_to_playable_range() {
        assert_eq!(frame_delay_ms(image::Delay::from_numer_denom_ms(0, 1)), 50);
        assert_eq!(frame_delay_ms(image::Delay::from_numer_denom_ms(30, 1)), 50);
        assert_eq!(
            frame_delay_ms(image::Delay::from_numer_denom_ms(120, 1)),
            120
        );
        assert_eq!(
            frame_delay_ms(image::Delay::from_numer_denom_ms(10_000, 1)),
            1_000
        );
        // 分数形式的 delay（WebP 的 1/1000 s 基数）按整数除法取整。
        assert_eq!(
            frame_delay_ms(image::Delay::from_numer_denom_ms(200, 2)),
            100
        );
    }

    #[test]
    fn animated_gif_frames_and_loops_are_decoded() {
        let dir = temp_dir("gif-animation");
        let path = dir.join("anim.gif");
        write_animated_gif(
            &path,
            &[0, 30, 2_000],
            &[[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]],
            Some(2),
        );

        let animation = decode_animation(&path, &Caps::default())
            .expect("decode gif")
            .expect("gif is animated");

        assert_eq!(animation.frames.len(), 3);
        assert_eq!(animation.size, [4, 2]);
        assert_eq!(animation.source_size, [4, 2]);
        assert_eq!(animation.source_depth, 8);
        assert_eq!(
            animation
                .frames
                .iter()
                .map(|frame| frame.delay_ms)
                .collect::<Vec<_>>(),
            [50, 50, 1_000]
        );
        // Netscape 的 n = 2 表示「再重复 2 次」，总播放 3 轮。
        assert_eq!(animation.loops, Some(3));
        assert_eq!(frame_rgba8(&animation.frames[0]).len(), 4 * 2 * 4);
        assert_eq!(&frame_rgba8(&animation.frames[0])[0..4], &[255, 0, 0, 255]);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn gif_without_loop_extension_is_infinite() {
        let dir = temp_dir("gif-infinite");
        let path = dir.join("loop.gif");
        write_animated_gif(
            &path,
            &[100, 100],
            &[[10, 20, 30, 255], [40, 50, 60, 255]],
            None,
        );

        let animation = decode_animation(&path, &Caps::default())
            .expect("decode gif")
            .expect("gif is animated");
        assert_eq!(animation.loops, None);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn animated_webp_frames_are_decoded() {
        let dir = temp_dir("webp-animation");
        let path = dir.join("anim.webp");
        write_animated_webp(&path, (4, 2), &[80, 500], 0);

        let animation = decode_animation(&path, &Caps::default())
            .expect("decode webp")
            .expect("webp is animated");

        assert_eq!(animation.frames.len(), 2);
        assert_eq!(animation.size, [4, 2]);
        assert_eq!(
            animation
                .frames
                .iter()
                .map(|frame| frame.delay_ms)
                .collect::<Vec<_>>(),
            [80, 500]
        );
        assert_eq!(animation.loops, None, "0 表示无限循环");

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn animated_png_frames_and_loops_are_decoded() {
        let dir = temp_dir("apng-animation");
        let path = dir.join("anim.png");
        write_animated_png(&path, &[20, 120], &[[255, 0, 0, 255], [0, 255, 0, 255]], 3);

        let animation = decode_animation(&path, &Caps::default())
            .expect("decode apng")
            .expect("png is animated");

        assert_eq!(animation.frames.len(), 2);
        assert_eq!(animation.size, [4, 2]);
        assert_eq!(animation.source_size, [4, 2]);
        assert_eq!(animation.source_depth, 8);
        // APNG delay 同样是秒分数：20 ms 钳到下限 50 ms，120 ms 原样保留。
        assert_eq!(
            animation
                .frames
                .iter()
                .map(|frame| frame.delay_ms)
                .collect::<Vec<_>>(),
            [50, 120]
        );
        assert_eq!(animation.loops, Some(3), "num_plays 即总播放轮数");
        // 首帧属于动画，像素来自第一种颜色。
        assert_eq!(&frame_rgba8(&animation.frames[0])[..4], &[255, 0, 0, 255]);

        // 静态解码路径仍取首帧（通用 PNG 解码不受影响）。
        let still = decode(&path, &Caps::default(), Purpose::Open).expect("decode still");
        assert_eq!(still.size, [4, 2]);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn single_frame_files_are_not_animations() {
        let dir = temp_dir("still");

        let gif = dir.join("still.gif");
        write_animated_gif(&gif, &[100], &[[1, 2, 3, 255]], None);
        assert!(
            decode_animation(&gif, &Caps::default())
                .expect("gif")
                .is_none()
        );

        let webp = dir.join("still.webp");
        write_animated_webp(&webp, (4, 2), &[100], 0);
        assert!(
            decode_animation(&webp, &Caps::default())
                .expect("webp")
                .is_none()
        );

        let png = dir.join("still.png");
        image::RgbaImage::from_pixel(4, 2, image::Rgba([9, 9, 9, 255]))
            .save(&png)
            .expect("write png");
        assert!(
            decode_animation(&png, &Caps::default())
                .expect("png")
                .is_none()
        );

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn animation_over_pixel_budget_falls_back_to_static() {
        let dir = temp_dir("animation-budget");
        let path = dir.join("anim.gif");
        write_animated_gif(
            &path,
            &[100, 100, 100],
            &[[1, 2, 3, 255], [4, 5, 6, 255], [7, 8, 9, 255]],
            None,
        );

        // 每帧 4×2 = 8 像素；三帧共 24 像素，预算 16 时应退回静态首帧。
        let caps = Caps {
            max_animation_pixels: 16,
            ..Caps::default()
        };
        assert!(decode_animation(&path, &caps).expect("decode").is_none());

        // 帧数上限 2 时同样退回静态。
        let caps = Caps {
            max_animation_frames: 2,
            ..Caps::default()
        };
        assert!(decode_animation(&path, &caps).expect("decode").is_none());

        // 放开预算后仍是动画，静态解码路径只取首帧。
        let animated = decode_animation(&path, &Caps::default())
            .expect("decode")
            .expect("animated");
        assert_eq!(animated.frames.len(), 3);
        let still = decode(&path, &Caps::default(), Purpose::Open).expect("decode still");
        assert_eq!(still.size, [4, 2]);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn animation_kind_is_selected_by_extension() {
        assert_eq!(
            animation_kind(Path::new("a.PNG")),
            Some(AnimationKind::Apng)
        );
        assert_eq!(animation_kind(Path::new("a.GIF")), Some(AnimationKind::Gif));
        assert_eq!(
            animation_kind(Path::new("a.WebP")),
            Some(AnimationKind::Webp)
        );
        assert_eq!(animation_kind(Path::new("a.webpz")), None);
    }
}
