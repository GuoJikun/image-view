//! 后台解码线程：尺寸检查、解码、降采样。

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use eframe::egui;
use image::imageops::FilterType;

/// 解码与纹理上传的安全上限。
#[derive(Clone, Debug)]
pub struct Caps {
    /// 纹理最长边（同时受 GPU 上限约束）。
    pub max_side: usize,
    /// 纹理总像素上限，控制显存/内存峰值。
    pub max_pixels: usize,
    /// 原图总像素上限，防止解压炸弹。
    pub max_source_pixels: u64,
}

impl Default for Caps {
    fn default() -> Self {
        Self {
            max_side: 8192,
            max_pixels: 40_000_000,
            max_source_pixels: 250_000_000,
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
}

struct Request {
    generation: u64,
    path: PathBuf,
    caps: Caps,
}

pub struct LoadResponse {
    pub generation: u64,
    pub path: PathBuf,
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
                while let Ok(mut request) = rx_request.recv() {
                    // latest-wins：翻页很快时只解码队列中最新的一条。
                    while let Ok(newer) = rx_request.try_recv() {
                        request = newer;
                    }

                    let result = decode(&request.path, &request.caps);
                    let response = LoadResponse {
                        generation: request.generation,
                        path: request.path,
                        result,
                    };
                    if tx_response.send(response).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("无法启动解码线程");

        Self {
            tx: tx_request,
            rx: rx_response,
            next_generation: 0,
        }
    }

    pub fn request(&mut self, path: PathBuf, caps: Caps) -> u64 {
        self.next_generation += 1;
        let generation = self.next_generation;
        let _ = self.tx.send(Request {
            generation,
            path,
            caps,
        });
        generation
    }

    pub fn try_recv(&self) -> Option<LoadResponse> {
        self.rx.try_recv().ok()
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

fn decode(path: &Path, caps: &Caps) -> Result<Decoded, LoadError> {
    // 先只读头部尺寸，拒绝解压炸弹，避免为超大图分配完整缓冲。
    let reader = image::ImageReader::open(path)?.with_guessed_format()?;
    let (width, height) = reader.into_dimensions()?;
    if u64::from(width) * u64::from(height) > caps.max_source_pixels {
        return Err(LoadError::TooLarge {
            w: width,
            h: height,
        });
    }

    let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
    reader.limits(caps.image_limits());
    let image = reader.decode().map_err(|error| match error {
        image::ImageError::Limits(_) => LoadError::OutOfMemory,
        other => LoadError::Decode(other),
    })?;

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

    let rgba = image.into_rgba8().into_raw();
    Ok(Decoded {
        size: target,
        source_size,
        rgba,
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
}
