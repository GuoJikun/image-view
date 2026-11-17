//! 生成 APNG / GIF / WebP 动画验收夹具。
//!
//! ```text
//! cargo run -p iv-image-decode --example animation_fixtures -- target/animation-fixtures
//! ```
//!
//! 覆盖三处边界：正常循环、超大 delay（应钳制到 1000 ms）、0 delay（应钳制到 50 ms），
//! 以及一个多帧 WebP（image crate 只能编码单帧，容器由本示例拼装）与一个多帧 APNG。
//!
//! 另生成两个「降级」样本，用默认 `Caps` 就能触发顶部提示横幅，供目视确认：
//! - `degrade-16bit-apng.png`：16-bit APNG，触发「暂不支持动画」（按 16-bit 静态首帧显示）。
//! - `degrade-over-budget.gif`：超过 1200 帧预算，触发「超出内存预算」（按静态首帧显示）。
//!
//! 生成后会调用 `decode_animation` 自检，确认两个样本确实被归类为对应降级原因。

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
    // 320×180 / 4 帧 APNG：不同 delay，循环 3 轮。
    write_animated_png(&dir.join("anim-4f.png"), 320, 180, &[60, 90, 120, 200], 3);

    // 降级样本：使用默认 Caps 即可触发顶部提示横幅。
    let p16 = dir.join("degrade-16bit-apng.png");
    write_16bit_apng(&p16);
    let pbudget = dir.join("degrade-over-budget.gif");
    write_over_budget_gif(&pbudget);
    verify_degraded(&p16, "unsupported16Bit");
    verify_degraded(&pbudget, "overBudget");

    println!("已生成动画夹具：{}", dir.display());
}

/// 用默认 `Caps` 解码样本，断言它被归类为预期的降级原因。
fn verify_degraded(path: &Path, expected: &str) {
    use iv_image_decode::{AnimationOutcome, Caps, DegradeReason, decode_animation};

    let outcome = decode_animation(path, &Caps::default()).expect("解码降级样本");
    let actual = match outcome {
        AnimationOutcome::Degraded(reason) => match reason {
            DegradeReason::OverBudget { .. } => "overBudget",
            DegradeReason::Unsupported16Bit => "unsupported16Bit",
            DegradeReason::DecodeFailed { .. } => "decodeFailed",
        },
        AnimationOutcome::Ready(_) => "ready",
        AnimationOutcome::NotAnimated => "notAnimated",
    };
    assert_eq!(
        actual,
        expected,
        "{} 应归类为 {expected}，实际 {actual}",
        path.display()
    );
    println!("  降级样本 {} → {actual} ✓", path.display());
}

/// 16-bit APNG：IHDR 位深为 16，`ApngDecoder` 会拒绝逐帧合成，触发位深降级。
fn write_16bit_apng(path: &Path) {
    use png::{BitDepth, ColorType, Encoder};

    let (width, height) = (64u32, 64u32);
    let file = std::fs::File::create(path).expect("创建 16-bit APNG");
    let mut encoder = Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Sixteen);
    encoder.set_animated(3, 0).expect("配置 16-bit APNG");
    let mut writer = encoder.write_header().expect("写 16-bit APNG 头");

    for frame in 0..3u16 {
        writer.set_frame_delay(300, 1_000).expect("设置帧间隔");
        // 每通道 16-bit 大端，三个色带交替，静态首帧也能看出内容。
        let mut pixels = Vec::with_capacity(width as usize * height as usize * 8);
        for y in 0..height {
            for x in 0..width {
                let band = (x / 16 + y / 16 + u32::from(frame)) % 4;
                let (r, g, b): (u16, u16, u16) = match band {
                    0 => (0xffff, 0x4000, 0x4000),
                    1 => (0x4000, 0xffff, 0x4000),
                    2 => (0x4000, 0x4000, 0xffff),
                    _ => (0xffff, 0xffff, 0x8000),
                };
                pixels.extend_from_slice(&r.to_be_bytes());
                pixels.extend_from_slice(&g.to_be_bytes());
                pixels.extend_from_slice(&b.to_be_bytes());
                pixels.extend_from_slice(&0xffffu16.to_be_bytes());
            }
        }
        writer.write_image_data(&pixels).expect("写 16-bit 帧");
    }
    writer.finish().expect("完成 16-bit APNG");
}

/// 超过默认帧预算（1200 帧）的 GIF：逐帧累加到第 1201 帧时触发超预算降级。
/// 用 8×8 小尺寸 + 交替纯色，控制文件体积与解码耗时。
fn write_over_budget_gif(path: &Path) {
    const FRAMES: u32 = 1_201;

    let mut bytes = Vec::new();
    let mut encoder = GifEncoder::new(&mut bytes);
    encoder.set_repeat(Repeat::Infinite).expect("设置循环次数");

    let images: Vec<Frame> = (0..FRAMES)
        .map(|index| {
            let color = if index % 2 == 0 {
                Rgba([220, 64, 64, 255])
            } else {
                Rgba([48, 96, 220, 255])
            };
            Frame::from_parts(
                RgbaImage::from_pixel(8, 8, color),
                0,
                0,
                Delay::from_numer_denom_ms(100, 1),
            )
        })
        .collect();

    encoder.encode_frames(images).expect("编码超预算 GIF");
    drop(encoder);
    std::fs::write(path, bytes).expect("写入超预算 GIF");
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

/// 用 `png` crate 写一个多帧 APNG：每帧一条彩色竖条，`plays` 为总播放轮数（0 = 无限）。
fn write_animated_png(path: &Path, width: u32, height: u32, delays_ms: &[u16], plays: u16) {
    use png::{BitDepth, ColorType, Encoder};

    let file = std::fs::File::create(path).expect("创建 APNG");
    let mut encoder = Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    encoder
        .set_animated(
            u32::try_from(delays_ms.len()).expect("帧数"),
            u32::from(plays),
        )
        .expect("配置 APNG");
    let mut writer = encoder.write_header().expect("写 APNG 头");

    let frames = u32::try_from(delays_ms.len()).unwrap_or(1);
    for (index, delay) in delays_ms.iter().enumerate() {
        writer.set_frame_delay(*delay, 1_000).expect("设置帧间隔");

        let mut buffer = RgbaImage::from_pixel(width, height, Rgba([18, 18, 20, 255]));
        let bar = (width / 6).max(1);
        let offset = (width - bar) * index as u32 / frames;
        for y in 0..height {
            for x in offset..(offset + bar).min(width) {
                let level = (255 * index as u32 / frames) as u8;
                buffer.put_pixel(x, y, Rgba([level, 90, 255 - level, 255]));
            }
        }
        writer
            .write_image_data(buffer.as_raw())
            .expect("写 APNG 帧");
    }
    writer.finish().expect("完成 APNG");
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
