//! 生成 `test_files/` 下的格式样本：每种格式图案、尺寸、颜色都不同。
//!
//! 用法：`cargo run -p iv-image-decode --example format_fixtures`
//! AVIF / HEIC / JPEG XL 需要外部编码器，不在此生成。

use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use image::codecs::bmp::BmpEncoder;
use image::codecs::farbfeld::FarbfeldEncoder;
use image::codecs::gif::GifEncoder;
use image::codecs::hdr::HdrEncoder;
use image::codecs::ico::IcoEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::openexr::OpenExrEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::pnm::PnmEncoder;
use image::codecs::qoi::QoiEncoder;
use image::codecs::tga::TgaEncoder;
use image::codecs::tiff::TiffEncoder;
use image::codecs::webp::WebPEncoder;
use image::{ColorType, ExtendedColorType, ImageEncoder};

fn main() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test_files");
    fs::create_dir_all(&dir).expect("create test_files");

    let mut index = 0;
    save("sample.png", &dir, index, |w, h, rgba| {
        PngEncoder::new(file(&dir, "sample.png")).write_image(rgba, w, h, ExtendedColorType::Rgba8)
    });
    index += 1;
    save("sample.jpg", &dir, index, |w, h, rgba| {
        JpegEncoder::new(file(&dir, "sample.jpg")).write_image(
            &to_rgb(rgba),
            w,
            h,
            ExtendedColorType::Rgb8,
        )
    });
    index += 1;
    save("sample.gif", &dir, index, |w, h, rgba| {
        GifEncoder::new(file(&dir, "sample.gif")).write_image(rgba, w, h, ColorType::Rgba8.into())
    });
    index += 1;
    save("sample.webp", &dir, index, |w, h, rgba| {
        WebPEncoder::new_lossless(file(&dir, "sample.webp")).write_image(
            rgba,
            w,
            h,
            ColorType::Rgba8.into(),
        )
    });
    index += 1;
    save("sample.bmp", &dir, index, |w, h, rgba| {
        BmpEncoder::new(&mut file(&dir, "sample.bmp")).write_image(
            rgba,
            w,
            h,
            ColorType::Rgba8.into(),
        )
    });
    index += 1;
    save("sample.tiff", &dir, index, |w, h, rgba| {
        TiffEncoder::new(&mut file(&dir, "sample.tiff")).write_image(
            rgba,
            w,
            h,
            ColorType::Rgba8.into(),
        )
    });
    index += 1;
    // ICO 要求边长不超过 256，单独用 64×64 的图案。
    let ico = image_rgba(index, 64, 64);
    IcoEncoder::new(file(&dir, "sample.ico"))
        .write_image(&ico, 64, 64, ColorType::Rgba8.into())
        .expect("encode ico");
    index += 1;
    save("sample.qoi", &dir, index, |w, h, rgba| {
        QoiEncoder::new(&mut file(&dir, "sample.qoi")).write_image(
            rgba,
            w,
            h,
            ColorType::Rgba8.into(),
        )
    });
    index += 1;
    save("sample.tga", &dir, index, |w, h, rgba| {
        TgaEncoder::new(file(&dir, "sample.tga")).write_image(rgba, w, h, ColorType::Rgba8.into())
    });
    index += 1;
    save("sample.pnm", &dir, index, |w, h, rgba| {
        PnmEncoder::new(file(&dir, "sample.pnm")).write_image(
            &to_rgb(rgba),
            w,
            h,
            ExtendedColorType::Rgb8,
        )
    });
    index += 1;
    let (w, h) = size(index);
    let rgb32f = to_rgb32f(&image_rgba(index, w, h));
    HdrEncoder::new(BufWriter::new(file(&dir, "sample.hdr")))
        .write_image(&rgb32f, w, h, ExtendedColorType::Rgb32F)
        .expect("encode hdr");
    index += 1;
    let (w, h) = size(index);
    let rgb32f = to_rgb32f(&image_rgba(index, w, h));
    OpenExrEncoder::new(file(&dir, "sample.exr"))
        .write_image(&rgb32f, w, h, ExtendedColorType::Rgb32F)
        .expect("encode exr");
    index += 1;
    let (w, h) = size(index);
    let rgba16 = to_rgba16be(&image_rgba(index, w, h));
    FarbfeldEncoder::new(BufWriter::new(file(&dir, "sample.ff")))
        .write_image(&rgba16, w, h, ExtendedColorType::Rgba16)
        .expect("encode ff");
    index += 1;
    write_dds(&dir.join("sample.dds"), index);
    index += 1;
    write_svg(&dir.join("sample.svg"), index);

    println!("已生成样本到 {}", dir.display());
}

fn file(dir: &Path, name: &str) -> fs::File {
    fs::File::create(dir.join(name)).unwrap_or_else(|e| panic!("create {name}: {e}"))
}

fn save<F: FnOnce(u32, u32, &[u8]) -> image::ImageResult<()>>(
    name: &str,
    _dir: &Path,
    i: usize,
    f: F,
) {
    let (w, h) = size(i);
    let rgba = image_rgba(i, w, h);
    f(w, h, &rgba).unwrap_or_else(|e| panic!("encode {name}: {e}"));
}

fn size(i: usize) -> (u32, u32) {
    (320 + i as u32 * 31, 240 + i as u32 * 17)
}

fn hsv(h: f32, s: f32, v: f32) -> [u8; 3] {
    let c = v * s;
    let hp = (h / 60.0) % 6.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [(r + m) * 255.0, (g + m) * 255.0, (b + m) * 255.0].map(|v| v as u8)
}

/// 8 种图案 × 递增色相与尺寸，保证每张图像内容都不同。
fn image_rgba(i: usize, w: u32, h: u32) -> Vec<u8> {
    let hue = (i as f32 * 47.0) % 360.0;
    let base = hsv(hue, 0.55, 0.96);
    let accent = hsv((hue + 150.0) % 360.0, 0.82, 0.88);
    let mut out = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..h {
        for x in 0..w {
            let fx = x as f32 / w as f32;
            let fy = y as f32 / h as f32;
            let filled = match i % 8 {
                0 => (x / 16 + y / 16) % 2 == 0,
                1 => (x + y) / 12 % 2 == 0,
                2 => {
                    let (dx, dy) = (fx - 0.5, fy - 0.5);
                    dx * dx + dy * dy < 0.18
                }
                3 => fx < fy,
                4 => x % 48 < 12 || y % 48 < 12,
                5 => fy < 0.35 + 0.25 * (fx * std::f32::consts::TAU).sin(),
                6 => ((fx + fy) * 8.0) as i32 % 2 == 0,
                _ => (fx - 0.5).abs() + (fy - 0.5).abs() < 0.35,
            };
            let shade = if filled { accent } else { base };
            let vignette = 1.0 - 0.18 * ((fx - 0.5).powi(2) + (fy - 0.5).powi(2));
            out.extend([
                (shade[0] as f32 * vignette) as u8,
                (shade[1] as f32 * vignette) as u8,
                (shade[2] as f32 * vignette) as u8,
                255,
            ]);
        }
    }
    out
}

fn to_rgb(rgba: &[u8]) -> Vec<u8> {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect()
}

fn to_rgb32f(rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgba.len());
    for p in rgba.as_chunks::<4>().0 {
        for c in &p[..3] {
            out.extend((*c as f32 / 255.0).to_ne_bytes());
        }
    }
    out
}

/// farbfeld 规定 16-bit 通道为大端序。
fn to_rgba16be(rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgba.len() * 2);
    for p in rgba.as_chunks::<4>().0 {
        for c in p {
            let wide = u16::from(*c) * 257;
            out.extend(wide.to_be_bytes());
        }
    }
    out
}

/// 手写 DXT1 压缩 DDS（`image` 只有 DDS 解码器，没有编码器，且只支持 DXT1/3/5）。
fn write_dds(path: &Path, i: usize) {
    let (raw_w, raw_h) = size(i);
    let (w, h) = (raw_w / 4 * 4, raw_h / 4 * 4);
    let rgba = image_rgba(i, w, h);
    let blocks = (w / 4) * (h / 4);
    let mut dds = Vec::with_capacity(128 + blocks as usize * 8);
    dds.extend(b"DDS "); // magic
    dds.extend(124u32.to_le_bytes()); // dwSize
    dds.extend(0x100Fu32.to_le_bytes()); // CAPS | HEIGHT | WIDTH | PIXELFORMAT
    dds.extend(h.to_le_bytes());
    dds.extend(w.to_le_bytes());
    dds.extend((blocks * 8).to_le_bytes()); // dwPitchOrLinearSize
    dds.extend(0u32.to_le_bytes()); // dwDepth
    dds.extend(0u32.to_le_bytes()); // dwMipMapCount
    dds.extend([0u8; 44]); // dwReserved1[11]
    dds.extend(32u32.to_le_bytes()); // ddspf.dwSize
    dds.extend(0x4u32.to_le_bytes()); // DDPF_FOURCC
    dds.extend(b"DXT1");
    dds.extend([0u8; 20]); // dwRGBBitCount 与四个 mask：FourCC 格式下均为 0
    dds.extend(0x1000u32.to_le_bytes()); // dwCaps: DDSCAPS_TEXTURE
    dds.extend([0u8; 12]); // dwCaps2 / dwCaps3 / dwCaps4
    dds.extend(0u32.to_le_bytes()); // dwReserved2

    for by in (0..h).step_by(4) {
        for bx in (0..w).step_by(4) {
            let mut colors = [[0u8; 3]; 16];
            for j in 0..4 {
                for k in 0..4 {
                    let p = ((by + j) * w + bx + k) as usize * 4;
                    colors[(j * 4 + k) as usize] = [rgba[p], rgba[p + 1], rgba[p + 2]];
                }
            }
            let luma = |c: &[u8; 3]| c[0] as u32 * 299 + c[1] as u32 * 587 + c[2] as u32 * 114;
            let (mut lo, mut hi) = (0, 0);
            for n in 1..16 {
                if luma(&colors[n]) < luma(&colors[lo]) {
                    lo = n;
                }
                if luma(&colors[n]) > luma(&colors[hi]) {
                    hi = n;
                }
            }
            let (mut c0, mut c1) = (rgb565(&colors[hi]), rgb565(&colors[lo]));
            let swapped = c0 < c1;
            if swapped {
                std::mem::swap(&mut c0, &mut c1);
            }
            let (a, b) = (unpack565(c0), unpack565(c1));
            let palette = [
                a,
                b,
                [
                    ((2 * a[0] as u16 + b[0] as u16) / 3) as u8,
                    ((2 * a[1] as u16 + b[1] as u16) / 3) as u8,
                    ((2 * a[2] as u16 + b[2] as u16) / 3) as u8,
                ],
                [
                    ((a[0] as u16 + 2 * b[0] as u16) / 3) as u8,
                    ((a[1] as u16 + 2 * b[1] as u16) / 3) as u8,
                    ((a[2] as u16 + 2 * b[2] as u16) / 3) as u8,
                ],
            ];
            let mut bits = 0u32;
            for (n, color) in colors.iter().enumerate() {
                let mut best = 0;
                let mut best_dist = u64::MAX;
                for (idx, candidate) in palette.iter().enumerate() {
                    let dist = (0..3)
                        .map(|c| (color[c] as i32 - candidate[c] as i32).pow(2) as u64)
                        .sum();
                    if dist < best_dist {
                        best = idx;
                        best_dist = dist;
                    }
                }
                let idx = if swapped {
                    match best {
                        0 => 1,
                        1 => 0,
                        2 => 3,
                        _ => 2,
                    }
                } else {
                    best
                };
                bits |= (idx as u32) << (2 * n);
            }
            dds.extend(c0.to_le_bytes());
            dds.extend(c1.to_le_bytes());
            dds.extend(bits.to_le_bytes());
        }
    }
    fs::write(path, dds).expect("write dds");
}

fn rgb565(c: &[u8; 3]) -> u16 {
    ((c[0] as u16 >> 3) << 11) | ((c[1] as u16 >> 2) << 5) | (c[2] as u16 >> 3)
}

fn unpack565(v: u16) -> [u8; 3] {
    [
        (((v >> 11) & 0x1F) * 255 / 31) as u8,
        (((v >> 5) & 0x3F) * 255 / 63) as u8,
        ((v & 0x1F) * 255 / 31) as u8,
    ]
}

fn write_svg(path: &Path, i: usize) {
    let (w, h) = size(i);
    let hue = (i as f32 * 47.0) % 360.0;
    let base = hsv(hue, 0.55, 0.96);
    let accent = hsv((hue + 150.0) % 360.0, 0.82, 0.88);
    let hex = |c: [u8; 3]| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n\
         <defs><linearGradient id=\"g\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\">\
         <stop offset=\"0\" stop-color=\"{}\"/><stop offset=\"1\" stop-color=\"{}\"/>\
         </linearGradient></defs>\n\
         <rect width=\"{w}\" height=\"{h}\" fill=\"url(#g)\"/>\n\
         <circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" opacity=\"0.85\"/>\n\
         <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"12\"/>\n\
         <path d=\"M {},{} L {},{} L {},{} Z\" fill=\"{}\" opacity=\"0.6\"/>\n\
         </svg>\n",
        hex(base),
        hex(accent),
        w / 3,
        h / 2,
        h / 4,
        hex(accent),
        w / 6,
        h / 6,
        w * 2 / 3,
        h * 2 / 3,
        hex(base),
        w / 2,
        h / 8,
        w * 3 / 4,
        h * 7 / 8,
        w / 4,
        h * 7 / 8,
        hex(base),
    );
    fs::write(path, svg).expect("write svg");
}
