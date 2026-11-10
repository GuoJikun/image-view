//! 后台解码线程：尺寸检查、解码、降采样。

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
    // 先只读头部尺寸，拒绝解压炸弹，避免为超大图分配完整缓冲。
    let reader = image::ImageReader::open(path)?.with_guessed_format()?;
    let (width, height) = reader.into_dimensions()?;
    let source_pixels = u64::from(width) * u64::from(height);
    if source_pixels > caps.max_source_pixels {
        return Err(LoadError::TooLarge {
            w: width,
            h: height,
        });
    }
    // 预加载只挑小图，避免后台解码大图拖慢用户翻页。
    if purpose == Purpose::Preload && source_pixels > caps.preload_max_source_pixels {
        return Err(LoadError::PreloadSkipped {
            w: width,
            h: height,
        });
    }

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
}
