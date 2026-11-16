//! 生成 GIF / WebP 动画验收夹具。
//!
//! ```text
//! cargo run -p iv-image-decode --example animation_fixtures -- target/animation-fixtures
//! ```
//!
//! 覆盖三处边界：正常循环、超大 delay（应钳制到 1000 ms）、0 delay（应钳制到 50 ms），
//! 以及一个多帧 WebP（image crate 只能编码单帧，容器由本示例拼装）。

use std::path::{Path, PathBuf};

use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame, Rgba, RgbaImage};

fn main() {
    let dir = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/animation-fixtures"));
    std::fs::create_dir_all(&dir).expect("创建输出目录");

    // 640×360 / 12 帧 / 80 ms：常规短循环。
    write_gif(&dir.join("anim-12f.gif"), 640, 360, 12, 80, None);
    // 320×180 / 4 帧 / 1500 ms：超大 delay，播放时钳制到 1000 ms。
    write_gif(&dir.join("anim-slow.gif"), 320, 180, 4, 1_500, Some(2));
    // 240×135 / 6 帧 / 0 ms：零 delay，播放时钳制到 50 ms。
    write_gif(&dir.join("anim-zero-delay.gif"), 240, 135, 6, 0, None);
    // 320×180 / 4 帧 WebP：循环次数 0 表示无限。
    write_animated_webp(&dir.join("anim-4f.webp"), 320, 180, &[60, 90, 120, 200], 0);

    println!("已生成动画夹具：{}", dir.display());
}

/// 写一个 `frames` 帧的 GIF：每帧一条彩色竖条向右移动，便于目视确认逐帧切换。
fn write_gif(
    path: &Path,
    width: u32,
    height: u32,
    frames: u32,
    delay_ms: u32,
    repeat: Option<u16>,
) {
    let mut bytes = Vec::new();
    let mut encoder = GifEncoder::new(&mut bytes);
    encoder
        .set_repeat(match repeat {
            Some(times) => Repeat::Finite(times),
            None => Repeat::Infinite,
        })
        .expect("设置循环次数");

    let images: Vec<Frame> = (0..frames)
        .map(|index| {
            let mut buffer = RgbaImage::from_pixel(width, height, Rgba([18, 18, 20, 255]));
            let bar = (width / 6).max(1);
            let offset = (width - bar) * index / frames.max(1);
            for y in 0..height {
                for x in offset..(offset + bar).min(width) {
                    let level = (255 * index / frames.max(1)) as u8;
                    buffer.put_pixel(x, y, Rgba([level, 90, 255 - level, 255]));
                }
            }
            Frame::from_parts(buffer, 0, 0, Delay::from_numer_denom_ms(delay_ms, 1))
        })
        .collect();

    encoder.encode_frames(images).expect("编码 GIF");
    drop(encoder);
    std::fs::write(path, bytes).expect("写入 GIF");
}

fn u24(value: u32) -> [u8; 3] {
    let bytes = value.to_le_bytes();
    [bytes[0], bytes[1], bytes[2]]
}

/// 手写最小动画 WebP：VP8X + ANIM + 每帧 ANMF（内含无损 VP8L 子块）。
fn write_animated_webp(path: &Path, width: u32, height: u32, durations: &[u32], loops: u16) {
    let subchunk = |color: [u8; 4]| {
        let buffer = RgbaImage::from_pixel(width, height, Rgba(color));
        let mut encoded = Vec::new();
        image::codecs::webp::WebPEncoder::new_lossless(&mut encoded)
            .encode(
                buffer.as_raw(),
                width,
                height,
                image::ExtendedColorType::Rgba8,
            )
            .expect("编码无损 WebP 帧");
        let offset = encoded
            .windows(4)
            .position(|window| window == b"VP8L")
            .expect("VP8L 子块");
        encoded[offset..].to_vec()
    };

    let mut chunks: Vec<Vec<u8>> = Vec::new();

    let mut vp8x = Vec::new();
    vp8x.extend_from_slice(b"VP8X");
    vp8x.extend_from_slice(&10u32.to_le_bytes());
    vp8x.push(0x02); // Animation 标志
    vp8x.extend_from_slice(&[0, 0, 0]);
    vp8x.extend_from_slice(&u24(width - 1));
    vp8x.extend_from_slice(&u24(height - 1));
    chunks.push(vp8x);

    let mut anim = Vec::new();
    anim.extend_from_slice(b"ANIM");
    anim.extend_from_slice(&6u32.to_le_bytes());
    anim.extend_from_slice(&[0, 0, 0, 0]); // 背景色
    anim.extend_from_slice(&loops.to_le_bytes());
    chunks.push(anim);

    for (index, duration) in durations.iter().enumerate() {
        let level = u8::try_from(index * 30).unwrap_or(0);
        let bitstream = subchunk([level, 120, 255 - level, 255]);

        let mut payload = Vec::new();
        payload.extend_from_slice(&u24(0)); // Frame X / 2
        payload.extend_from_slice(&u24(0)); // Frame Y / 2
        payload.extend_from_slice(&u24(width - 1));
        payload.extend_from_slice(&u24(height - 1));
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
    std::fs::write(path, file).expect("写入 WebP");
}
