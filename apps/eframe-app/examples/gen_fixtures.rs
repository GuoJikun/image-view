//! 生成手工测试矩阵所需的图片夹具（开发用，不随程序发布）。

use std::io::BufWriter;
use std::path::{Path, PathBuf};

use image::codecs::png::PngEncoder;
use image::{DynamicImage, ExtendedColorType, ImageEncoder, ImageFormat, Rgba, RgbaImage};

fn gradient(width: u32, height: u32, alpha_right: bool) -> RgbaImage {
    let mut img = RgbaImage::new(width, height);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let alpha = if alpha_right && x > width * 3 / 4 {
            140
        } else {
            255
        };
        *px = Rgba([
            (x % 256) as u8,
            (y % 256) as u8,
            ((x + y) % 256) as u8,
            alpha,
        ]);
    }
    img
}

fn save(dynimg: &DynamicImage, dir: &Path, name: &str, format: ImageFormat) {
    let path = dir.join(name);
    match dynimg.save_with_format(&path, format) {
        Ok(()) => println!("ok   {name}"),
        Err(error) => println!("skip {name}: {error}"),
    }
}

/// 保存 JPEG 并注入 EXIF Orientation（用于验证自动方向校正）。
fn save_oriented_jpeg(dynimg: &DynamicImage, dir: &Path, name: &str, orientation: u8) {
    let mut jpeg = Vec::new();
    if let Err(error) = dynimg.write_to(&mut std::io::Cursor::new(&mut jpeg), ImageFormat::Jpeg) {
        println!("skip {name}: {error}");
        return;
    }

    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"II");
    tiff.extend_from_slice(&42u16.to_le_bytes());
    tiff.extend_from_slice(&8u32.to_le_bytes());
    tiff.extend_from_slice(&1u16.to_le_bytes());
    tiff.extend_from_slice(&0x0112u16.to_le_bytes()); // Orientation
    tiff.extend_from_slice(&3u16.to_le_bytes()); // SHORT
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
    std::fs::write(dir.join(name), bytes).expect("写入 EXIF JPEG");
    println!("ok   {name} (orientation {orientation})");
}

fn main() {
    let dir = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .expect("用法: gen_fixtures <输出目录>"),
    );
    std::fs::create_dir_all(&dir).expect("创建输出目录");

    let rgba = DynamicImage::ImageRgba8(gradient(320, 240, true));
    let rgb = DynamicImage::ImageRgb8(rgba.to_rgb8());
    let gray = DynamicImage::ImageLuma8(rgba.to_luma8());

    save(&rgba, &dir, "fmt.png", ImageFormat::Png);
    save(&rgb, &dir, "fmt.jpg", ImageFormat::Jpeg);
    save(&gray, &dir, "gray.jpg", ImageFormat::Jpeg);
    save_oriented_jpeg(&rgb, &dir, "exif-orient6.jpg", 6);
    save_oriented_jpeg(&rgb, &dir, "exif-orient3.jpg", 3);
    save(&rgba, &dir, "fmt.gif", ImageFormat::Gif);
    save(&rgba, &dir, "fmt.webp", ImageFormat::WebP);
    save(&rgb, &dir, "fmt.bmp", ImageFormat::Bmp);
    save(&rgba, &dir, "fmt.tiff", ImageFormat::Tiff);
    save(&rgba, &dir, "fmt.qoi", ImageFormat::Qoi);
    save(&rgba, &dir, "fmt.tga", ImageFormat::Tga);
    save(&rgb, &dir, "fmt.pnm", ImageFormat::Pnm);

    // ICO 通常限制在 256×256 以内
    let ico = DynamicImage::ImageRgba8(gradient(256, 256, true));
    save(&ico, &dir, "fmt.ico", ImageFormat::Ico);

    // 16-bit RGBA PNG
    let w = 160u32;
    let h = 120u32;
    let mut data = Vec::with_capacity((w * h * 8) as usize);
    for y in 0..h {
        for x in 0..w {
            let v = |scale: u32| ((x + y) * scale % 65536) as u16;
            for channel in [v(257), v(131), v(67), 65535u16] {
                data.extend_from_slice(&channel.to_be_bytes());
            }
        }
    }
    let path = dir.join("png16.png");
    let file = std::fs::File::create(&path).expect("创建 png16");
    match PngEncoder::new(BufWriter::new(file)).write_image(&data, w, h, ExtendedColorType::Rgba16)
    {
        Ok(()) => println!("ok   png16.png"),
        Err(error) => println!("skip png16.png: {error}"),
    }

    // 1×1
    let one = DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([200, 100, 50, 255])));
    save(&one, &dir, "tiny1x1.png", ImageFormat::Png);

    // 24MP JPEG
    let big =
        DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(gradient(6000, 4000, false)).to_rgb8());
    save(&big, &dir, "big24mp.jpg", ImageFormat::Jpeg);

    // 100MP JPEG（若内存/时间允许）
    if std::env::var_os("SKIP_100MP").is_none() {
        let huge = DynamicImage::ImageRgb8(
            DynamicImage::ImageRgba8(gradient(10000, 10000, false)).to_rgb8(),
        );
        save(&huge, &dir, "huge100mp.jpg", ImageFormat::Jpeg);
    }

    // HDR / EXR（浮点格式，失败则跳过）
    let hdr = DynamicImage::ImageRgb32F(image::Rgb32FImage::from_fn(160, 120, |x, y| {
        image::Rgb([x as f32 / 160.0, y as f32 / 120.0, 0.5])
    }));
    save(&hdr, &dir, "fmt.hdr", ImageFormat::Hdr);
    save(&hdr, &dir, "fmt.exr", ImageFormat::OpenExr);

    // 超大尺寸 PNG 头（20000×15000 = 300MP），仅用于验证“拒绝解码”分支。
    write_huge_header_png(&dir.join("huge300mp.png"), 20_000, 15_000);
}

/// 只写签名 + IHDR + IEND，不带像素数据：足够让读取尺寸的代码拒绝它。
fn write_huge_header_png(path: &Path, width: u32, height: u32) {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    let mut png = Vec::new();
    png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(b"IHDR");
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    png.extend_from_slice(&13u32.to_be_bytes());
    png.extend_from_slice(&ihdr);
    png.extend_from_slice(&crc32(&ihdr).to_be_bytes());

    let mut iend = Vec::new();
    iend.extend_from_slice(b"IEND");
    png.extend_from_slice(&0u32.to_be_bytes());
    png.extend_from_slice(&iend);
    png.extend_from_slice(&crc32(&iend).to_be_bytes());

    std::fs::write(path, png).expect("写 300MP 头");
    println!("ok   huge300mp.png (header only)");
}
