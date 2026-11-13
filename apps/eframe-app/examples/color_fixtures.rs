//! 生成 ICC 色彩管理手工验收夹具。
//!
//! 用法：`cargo run --example color_fixtures -- target/color-fixtures`
//!
//! 输出 AdobeRGB / Display P3 的 8-bit 与 16-bit PNG、JPEG、WebP、TIFF，
//! 以及同像素值的 sRGB 参考图。用本查看器与系统查看器同屏对比即可。

use std::path::{Path, PathBuf};

use image::ImageEncoder as _;
use image::{DynamicImage, ExtendedColorType, Rgb, RgbImage, Rgba};
use lcms2::{CIExyY, CIExyYTRIPLE, Locale, MLU, Profile, Tag, TagSignature, ToneCurve};

const WIDTH: u32 = 640;
const HEIGHT: u32 = 360;

struct ColorSpace {
    name: &'static str,
    slug: &'static str,
    primaries: CIExyYTRIPLE,
    curve: ToneCurve,
}

fn d65() -> CIExyY {
    CIExyY {
        x: 0.3127,
        y: 0.3290,
        Y: 1.0,
    }
}

fn color_spaces() -> [ColorSpace; 3] {
    let srgb_curve =
        ToneCurve::new_parametric(4, &[2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045])
            .expect("sRGB curve");

    [
        ColorSpace {
            name: "sRGB",
            slug: "srgb",
            primaries: CIExyYTRIPLE {
                Red: CIExyY {
                    x: 0.64,
                    y: 0.33,
                    Y: 1.0,
                },
                Green: CIExyY {
                    x: 0.30,
                    y: 0.60,
                    Y: 1.0,
                },
                Blue: CIExyY {
                    x: 0.15,
                    y: 0.06,
                    Y: 1.0,
                },
            },
            curve: srgb_curve.clone(),
        },
        ColorSpace {
            name: "Adobe RGB (1998)",
            slug: "adobe-rgb",
            primaries: CIExyYTRIPLE {
                Red: CIExyY {
                    x: 0.64,
                    y: 0.33,
                    Y: 1.0,
                },
                Green: CIExyY {
                    x: 0.21,
                    y: 0.71,
                    Y: 1.0,
                },
                Blue: CIExyY {
                    x: 0.15,
                    y: 0.06,
                    Y: 1.0,
                },
            },
            curve: ToneCurve::new(2.2),
        },
        ColorSpace {
            name: "Display P3",
            slug: "display-p3",
            primaries: CIExyYTRIPLE {
                Red: CIExyY {
                    x: 0.680,
                    y: 0.320,
                    Y: 1.0,
                },
                Green: CIExyY {
                    x: 0.265,
                    y: 0.690,
                    Y: 1.0,
                },
                Blue: CIExyY {
                    x: 0.150,
                    y: 0.060,
                    Y: 1.0,
                },
            },
            curve: srgb_curve,
        },
    ]
}

fn icc_bytes(space: &ColorSpace) -> Vec<u8> {
    let curves = [&space.curve, &space.curve, &space.curve];
    let mut profile =
        Profile::new_rgb(&d65(), &space.primaries, &curves).expect("create RGB profile");
    let mut description = MLU::new(1);
    description.set_text_ascii(space.name, Locale::new("en"));
    profile.write_tag(TagSignature::ProfileDescriptionTag, Tag::MLU(&description));
    profile.icc().expect("serialize ICC profile")
}

/// 8-bit 色块 + 渐变测试图。
fn chart_8bit() -> RgbImage {
    let mut image = RgbImage::new(WIDTH, HEIGHT);
    let patches = [
        [255, 0, 0],
        [0, 255, 0],
        [0, 0, 255],
        [0, 255, 255],
        [255, 0, 255],
        [255, 255, 0],
        [222, 150, 110], // 肤色
        [128, 128, 128],
    ];
    let patch_width = WIDTH / patches.len() as u32;

    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let patch = (x / patch_width) as usize;
        if y < HEIGHT / 2 {
            if patch < patches.len() {
                *pixel = Rgb(patches[patch]);
            }
        } else {
            // 下半部分为 RGB 三通道错位渐变，便于观察色带与色偏。
            let t = x as f32 / (WIDTH - 1) as f32;
            let r = (t * 255.0) as u8;
            let g = ((1.0 - t) * 255.0) as u8;
            let b = ((y - HEIGHT / 2) as f32 / (HEIGHT / 2 - 1) as f32 * 255.0) as u8;
            *pixel = Rgb([r, g, b]);
        }
    }
    image
}

/// 16-bit 双通道渐变，用于检查色带。
fn gradient_16bit() -> RgbaImage16 {
    let mut image = RgbaImage16::new(WIDTH, HEIGHT);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let r = (x as f64 / (WIDTH - 1) as f64 * 65535.0).round() as u16;
        let g = ((1.0 - x as f64 / (WIDTH - 1) as f64) * 65535.0).round() as u16;
        let b = (y as f64 / (HEIGHT - 1) as f64 * 65535.0).round() as u16;
        *pixel = Rgba([r, g, b, 65535]);
    }
    image
}

type RgbaImage16 = image::ImageBuffer<Rgba<u16>, Vec<u16>>;

fn write_png(path: &Path, image: &DynamicImage, icc: Vec<u8>, sixteen_bit: bool) {
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
    encoder.set_icc_profile(icc).expect("PNG ICC profile");
    if sixteen_bit {
        let rgba = image.as_rgba16().expect("RGBA16");
        encoder
            .write_image(
                bytemuck::cast_slice(rgba.as_raw()),
                rgba.width(),
                rgba.height(),
                ExtendedColorType::Rgba16,
            )
            .expect("encode 16-bit PNG");
    } else {
        let rgba = image.to_rgba8();
        encoder
            .write_image(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                ExtendedColorType::Rgba8,
            )
            .expect("encode PNG");
    }
    std::fs::write(path, bytes).expect("write PNG");
}

fn write_jpeg(path: &Path, image: &DynamicImage, icc: Vec<u8>) {
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 96);
    encoder.set_icc_profile(icc).expect("JPEG ICC profile");
    let rgb = image.to_rgb8();
    encoder
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            ExtendedColorType::Rgb8,
        )
        .expect("encode JPEG");
    std::fs::write(path, bytes).expect("write JPEG");
}

fn write_webp(path: &Path, image: &DynamicImage, icc: Vec<u8>) {
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut bytes);
    encoder.set_icc_profile(icc).expect("WebP ICC profile");
    let rgba = image.to_rgba8();
    encoder
        .write_image(
            rgba.as_raw(),
            rgba.width(),
            rgba.height(),
            ExtendedColorType::Rgba8,
        )
        .expect("encode WebP");
    std::fs::write(path, bytes).expect("write WebP");
}

fn write_tiff(path: &Path, image: &DynamicImage, icc: Vec<u8>, sixteen_bit: bool) {
    let file = std::fs::File::create(path).expect("create TIFF");
    let mut encoder = image::codecs::tiff::TiffEncoder::new(std::io::BufWriter::new(file));
    encoder.set_icc_profile(icc).expect("TIFF ICC profile");
    if sixteen_bit {
        let rgba = image.as_rgba16().expect("RGBA16");
        encoder
            .write_image(
                bytemuck::cast_slice(rgba.as_raw()),
                rgba.width(),
                rgba.height(),
                ExtendedColorType::Rgba16,
            )
            .expect("encode 16-bit TIFF");
    } else {
        let rgba = image.to_rgba8();
        encoder
            .write_image(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                ExtendedColorType::Rgba8,
            )
            .expect("encode TIFF");
    }
}

fn main() {
    let out = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/color-fixtures"));
    std::fs::create_dir_all(&out).expect("create output directory");

    let chart = DynamicImage::ImageRgb8(chart_8bit());
    let gradient_16 = DynamicImage::ImageRgba16(gradient_16bit());
    let gradient_8 = DynamicImage::ImageRgba8(gradient_16.to_rgba8());

    for space in &color_spaces() {
        let icc = icc_bytes(space);
        write_png(
            &out.join(format!("{}-8bit.png", space.slug)),
            &chart,
            icc.clone(),
            false,
        );
        write_png(
            &out.join(format!("{}-gradient-8bit.png", space.slug)),
            &gradient_8,
            icc.clone(),
            false,
        );
        write_png(
            &out.join(format!("{}-16bit.png", space.slug)),
            &gradient_16,
            icc.clone(),
            true,
        );
        write_jpeg(
            &out.join(format!("{}-8bit.jpg", space.slug)),
            &chart,
            icc.clone(),
        );
        write_webp(
            &out.join(format!("{}-8bit.webp", space.slug)),
            &chart,
            icc.clone(),
        );
        write_tiff(
            &out.join(format!("{}-8bit.tiff", space.slug)),
            &chart,
            icc.clone(),
            false,
        );
        write_tiff(
            &out.join(format!("{}-16bit.tiff", space.slug)),
            &gradient_16,
            icc,
            true,
        );
    }

    println!("已生成色彩验收夹具：{}", out.display());
}
