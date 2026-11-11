//! 后台解码线程：尺寸检查、解码、降采样。

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use eframe::egui;
use image::ImageDecoder as _;
use image::imageops::FilterType;
use image::metadata::Orientation;

use crate::color::{self, ColorInfo};

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
}

impl Default for Caps {
    fn default() -> Self {
        Self {
            max_side: 8192,
            max_pixels: 40_000_000,
            max_source_pixels: 250_000_000,
            preload_max_source_pixels: 30_000_000,
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
    pub rgba: Vec<u8>,
    /// 源文件位深（8 或 16）；纹理始终为 8-bit。
    pub source_depth: u8,
    pub color: ColorInfo,
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("无法读取文件：{0}")]
    Io(#[from] std::io::Error),
    #[error("图片已损坏或格式不受支持：{0}")]
    Decode(#[from] image::ImageError),
    #[error("图片过大（{w}×{h}），已超过安全上限")]
    TooLarge { w: u32, h: u32 },
    #[error("解码超出内存限制")]
    OutOfMemory,
    /// 预加载遇到大图时主动放弃；不会展示给用户。
    #[error("预加载已跳过（原图 {w}×{h} 过大）")]
    PreloadSkipped { w: u32, h: u32 },
    #[cfg(feature = "avif")]
    #[error("AVIF 解码失败：{0}")]
    Avif(String),
    #[cfg(feature = "heic")]
    #[error("HEIC 解码失败：{0}")]
    Heic(String),
    #[cfg(feature = "svg")]
    #[error("SVG 解析失败：{0}")]
    Svg(String),
    /// 仅在关闭对应 feature 的构建中构造。
    #[cfg_attr(
        all(feature = "avif", feature = "heic", feature = "svg"),
        allow(dead_code)
    )]
    #[error("{format} 解码器未包含在此构建中；请使用启用该特性的版本打开 .{extension} 文件")]
    MissingDecoder {
        format: &'static str,
        extension: String,
    },
}

/// 按扩展名选择解码器；HEIC/AVIF 不走 `image` 的通用读取路径，SVG 走光栅化。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Standard,
    Avif,
    Heic,
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
        Some(extension) if extension.eq_ignore_ascii_case("svg") => Format::Svg,
        _ => Format::Standard,
    }
}

/// 仅在关闭对应 feature 的构建中调用。
#[cfg_attr(
    all(feature = "avif", feature = "heic", feature = "svg"),
    allow(dead_code)
)]
fn missing_decoder(path: &Path, format: Format) -> LoadError {
    let (label, fallback) = match format {
        Format::Avif => ("AVIF", "avif"),
        Format::Heic => ("HEIC", "heic"),
        Format::Svg => ("SVG", "svg"),
        Format::Standard => ("图片", "img"),
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

pub struct Loader {
    tx: Sender<Request>,
    rx: Receiver<LoadResponse>,
    next_generation: u64,
}

impl Loader {
    pub fn new(ctx: egui::Context) -> Self {
        let (tx_request, rx_request) = mpsc::channel::<Request>();
        let (tx_response, rx_response) = mpsc::channel::<LoadResponse>();

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
                    ctx.request_repaint();

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
            next_generation: 0,
        }
    }

    /// 请求打开图片；返回本次请求的代号，主线程据此丢弃过期结果。
    pub fn request(&mut self, path: PathBuf, caps: Caps) -> u64 {
        self.next_generation += 1;
        self.send(Request {
            generation: self.next_generation,
            path,
            caps,
            purpose: Purpose::Open,
        });
        self.next_generation
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
        self.next_generation += 1;
        self.next_generation
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
        Format::Svg => decode_svg(path, caps, purpose),
    }
}

/// 按缩略图尺寸解码；复用主图路径的格式分派、ICC 转换与错误类型。
pub(crate) fn decode_thumbnail(path: &Path) -> Result<Decoded, LoadError> {
    let side = crate::thumbs::MAX_SIDE as usize;
    let caps = Caps {
        max_side: side,
        max_pixels: side * side,
        ..Caps::default()
    };
    decode(path, &caps, Purpose::Open)
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
            log::debug!("无法读取 {} 的 ICC profile：{error}", path.display());
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

/// 共享的系统字体库；只在第一次解码 SVG 时扫描系统字体。
#[cfg(feature = "svg")]
fn shared_fontdb() -> std::sync::Arc<resvg::usvg::fontdb::Database> {
    use std::sync::OnceLock;

    static FONTDB: OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    FONTDB
        .get_or_init(|| {
            let mut database = resvg::usvg::fontdb::Database::new();
            database.load_system_fonts();
            log::debug!("已载入 {} 个系统字体", database.len());
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
        rgba,
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
        Err(usvg::Error::SvgzFeatureNotEnabled) => Err(LoadError::Svg(
            "暂不支持 gzip 压缩的 SVGZ，请解压为 .svg 后再打开".to_owned(),
        )),
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

    // 16-bit 在转换与降采样后仍保持 16-bit，最后统一量化为 8-bit 纹理。
    let source_depth = color::source_depth(&image);
    let (rgba, color_info) = color::to_srgb_rgba8(image, icc_profile.as_deref());
    if color_info.converted {
        log::debug!(
            "已转换到 sRGB：{} ({})",
            path.display(),
            color_info.label().unwrap_or_default()
        );
    }

    Ok(Decoded {
        size: target,
        source_size,
        rgba,
        source_depth,
        color: color_info,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let decoded = decode_thumbnail(&path).expect("decode thumbnail");
        assert_eq!(decoded.size, [crate::thumbs::MAX_SIDE as usize, 64]);
        assert_eq!(decoded.rgba.len(), decoded.size[0] * decoded.size[1] * 4);

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
        let rgba = image::RgbaImage::from_raw(20, 40, decoded.rgba).expect("rgba");
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
        write_png_with_icc(&path, &image, crate::color::test_linear_rgb_icc());

        let decoded = decode(&path, &Caps::default(), Purpose::Open).expect("decode png");
        assert!(decoded.color.converted);
        assert!(decoded.color.label().is_some());
        assert_eq!(decoded.source_depth, 8);
        assert!(
            decoded.rgba[0].abs_diff(128) >= 40,
            "应转换到约 188，实际 {}",
            decoded.rgba[0]
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
            .set_icc_profile(crate::color::test_linear_rgb_icc())
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
        assert!(decoded.rgba[0] > 120, "实际 {}", decoded.rgba[0]);

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
        assert!(text.contains("启用该特性"), "实际：{text}");
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
        assert_eq!(decoded.rgba.len(), 40 * 20 * 4);
        assert_eq!(decoded.source_depth, 8);

        // 中心像素应为纯红且不透明（预乘 alpha 已还原）。
        let offset = (10 * 40 + 20) * 4;
        assert_eq!(&decoded.rgba[offset..offset + 4], &[255, 0, 0, 255]);

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
        assert_eq!(decoded.rgba.len(), decoded.size[0] * decoded.size[1] * 4);

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
        assert_eq!(decoded.rgba.len(), 300 * 150 * 4);

        // 内容按原坐标落在左上角，视口其余部分透明。
        assert_eq!(&decoded.rgba[0..4], &[255, 0, 0, 255]);
        let empty = (100 * 300 + 200) * 4;
        assert_eq!(&decoded.rgba[empty..empty + 4], &[0, 0, 0, 0]);

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
    fn repo_icon_svg_rasterizes_with_content() {
        // 用仓库自带的真实图标（1024×1024，含 viewBox 与多段路径）做端到端校验。
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("design/icon-master.svg");
        let decoded = decode(&path, &Caps::default(), Purpose::Open).expect("decode icon svg");

        assert_eq!(decoded.source_size, [1024, 1024]);
        assert_eq!(decoded.size, [1024, 1024]);
        assert_eq!(decoded.rgba.len(), 1024 * 1024 * 4);
        assert!(
            decoded
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] > 0),
            "渲染结果不应全透明"
        );
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
}
