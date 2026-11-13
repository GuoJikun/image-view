//! 生成缩略图条验收用图片目录。
//!
//! 用法：cargo run --example thumb_fixtures -- <目录> [数量] [宽] [高]

use std::path::PathBuf;

fn main() -> image::ImageResult<()> {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(
        args.next()
            .unwrap_or_else(|| "target/thumb-fixtures".into()),
    );
    let count: usize = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1000);
    let width: u32 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1600);
    let height: u32 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1200);

    std::fs::create_dir_all(&dir).expect("创建输出目录");

    for index in 0..count {
        let hue = (index % 24) as u8;
        let image = image::RgbImage::from_fn(width, height, |x, y| {
            let bands = ((x * 5 / width.max(1)) + (y * 3 / height.max(1))) as u8;
            image::Rgb([
                hue.wrapping_mul(10).wrapping_add(bands * 18),
                hue.wrapping_mul(4).wrapping_add(bands * 30),
                hue.wrapping_mul(7).wrapping_add(bands * 12),
            ])
        });
        let path = dir.join(format!("img{index:04}.jpg"));
        image.save(&path)?;
    }

    println!("已生成 {count} 张 {width}×{height} 图片：{}", dir.display());
    Ok(())
}
