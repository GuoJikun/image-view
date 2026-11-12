//! ICC 色彩管理：把图片内嵌的 profile 转换到 sRGB。
//!
//! 显示端始终输出 8-bit RGBA 纹理；16-bit 图片在转换与降采样阶段保持
//! 16-bit 精度，最后才量化到 8-bit，避免二次量化造成色带。
//! profile 缺失、损坏或色彩空间不受支持时回退原像素，不阻塞显示。

use image::DynamicImage;

use crate::i18n::{self, Key};

/// 一次色彩转换的结果，供状态栏与调试日志使用。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ColorInfo {
    /// 是否实际执行了到 sRGB 的转换。
    pub converted: bool,
    /// 源 profile 的描述（可能为空或不存在）。
    pub source: Option<String>,
}

impl ColorInfo {
    /// 状态栏文案；未转换时返回 `None`。
    pub fn label(&self) -> Option<String> {
        if !self.converted {
            return None;
        }
        match self.source.as_deref().map(str::trim) {
            Some(name) if !name.is_empty() => Some(format!("{name} → sRGB")),
            _ => Some(i18n::t(Key::ColorConverted).to_owned()),
        }
    }
}

/// 源图片的位深（仅区分 8/16-bit；其他浮点格式按 8-bit 处理）。
pub fn source_depth(image: &DynamicImage) -> u8 {
    match image {
        DynamicImage::ImageLuma16(_)
        | DynamicImage::ImageLumaA16(_)
        | DynamicImage::ImageRgb16(_)
        | DynamicImage::ImageRgba16(_) => 16,
        _ => 8,
    }
}

/// 解码后的统一出口：按需做 ICC → sRGB 转换并输出 8-bit RGBA。
///
/// 失败时保留原始像素（`converted = false`），只写 debug 日志。
pub fn to_srgb_rgba8(image: DynamicImage, icc: Option<&[u8]>) -> (Vec<u8>, ColorInfo) {
    let Some(icc) = icc.filter(|profile| !profile.is_empty()) else {
        return (image.into_rgba8().into_raw(), ColorInfo::default());
    };

    match convert(image, icc) {
        Ok((rgba, info)) => (rgba, info),
        Err((image, reason)) => {
            log::debug!("ICC 色彩转换失败，按原像素显示：{reason}");
            (image.into_rgba8().into_raw(), ColorInfo::default())
        }
    }
}

fn convert(
    image: DynamicImage,
    icc: &[u8],
) -> Result<(Vec<u8>, ColorInfo), (DynamicImage, String)> {
    use lcms2::{InfoType, Locale, Profile, ThreadContext};

    // 解码线程是单线程的，但测试可能并行运行；使用私有 context 避免共享 LCMS 全局状态。
    let context = ThreadContext::new();
    let source = match Profile::new_icc_context(&context, icc) {
        Ok(profile) => profile,
        Err(error) => return Err((image, format!("无法解析 ICC profile：{error}"))),
    };
    let target = Profile::new_srgb_context(&context);
    let description = source
        .info(InfoType::Description, Locale::none())
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty());
    let info = ColorInfo {
        converted: true,
        source: description,
    };
    let is_gray = matches!(
        &image,
        DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageLuma16(_)
            | DynamicImage::ImageLumaA16(_)
    );

    match (source.color_space(), is_gray) {
        (lcms2::ColorSpaceSignature::RgbData, false) => {
            convert_rgb(&context, &source, &target, image).map(|rgba| (rgba, info))
        }
        (lcms2::ColorSpaceSignature::GrayData, true) => {
            convert_gray(&context, &source, &target, image).map(|rgba| (rgba, info))
        }
        (signature, gray) => Err((
            image,
            format!("ICC 色彩空间 {signature:?} 与图像通道不匹配（灰度={gray}）"),
        )),
    }
}

fn convert_rgb(
    context: &lcms2::ThreadContext,
    source: &lcms2::Profile<lcms2::ThreadContext>,
    target: &lcms2::Profile<lcms2::ThreadContext>,
    image: DynamicImage,
) -> Result<Vec<u8>, (DynamicImage, String)> {
    use lcms2::{Flags, Intent, PixelFormat, Transform};

    if source_depth(&image) == 16 {
        // 16-bit：在 16-bit 空间完成转换与 alpha 复制，最后量化一次。
        let transform =
            match Transform::<[u16; 4], [u16; 4], lcms2::ThreadContext>::new_flags_context(
                context,
                source,
                PixelFormat::RGBA_16,
                target,
                PixelFormat::RGBA_16,
                Intent::RelativeColorimetric,
                Flags::COPY_ALPHA,
            ) {
                Ok(transform) => transform,
                Err(error) => return Err((image, format!("无法创建 16-bit 转换：{error}"))),
            };
        let raw = image.into_rgba16().into_raw();
        let mut pixels: Vec<[u16; 4]> = bytemuck::cast_vec(raw);
        transform.transform_in_place(&mut pixels);
        Ok(quantize_rgba16(&pixels))
    } else {
        let transform = match Transform::<u8, u8, lcms2::ThreadContext>::new_flags_context(
            context,
            source,
            PixelFormat::RGBA_8,
            target,
            PixelFormat::RGBA_8,
            Intent::RelativeColorimetric,
            Flags::COPY_ALPHA,
        ) {
            Ok(transform) => transform,
            Err(error) => return Err((image, format!("无法创建 8-bit 转换：{error}"))),
        };
        let mut rgba = image.into_rgba8().into_raw();
        transform.transform_in_place(&mut rgba);
        Ok(rgba)
    }
}

fn convert_gray(
    context: &lcms2::ThreadContext,
    source: &lcms2::Profile<lcms2::ThreadContext>,
    target: &lcms2::Profile<lcms2::ThreadContext>,
    image: DynamicImage,
) -> Result<Vec<u8>, (DynamicImage, String)> {
    use lcms2::{Intent, PixelFormat, Transform};

    if source_depth(&image) == 16 {
        let transform = match Transform::<u16, [u16; 3], lcms2::ThreadContext>::new_context(
            context,
            source,
            PixelFormat::GRAY_16,
            target,
            PixelFormat::RGB_16,
            Intent::RelativeColorimetric,
        ) {
            Ok(transform) => transform,
            Err(error) => return Err((image, format!("无法创建灰度转换：{error}"))),
        };
        let raw = image.into_rgba16().into_raw();
        let shallow = raw
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| pixel[0])
            .collect::<Vec<_>>();
        let mut rgb = vec![[0u16; 3]; shallow.len()];
        transform.transform_pixels(&shallow, &mut rgb);

        let mut out = Vec::with_capacity(shallow.len() * 4);
        for (pixel, alpha) in rgb
            .iter()
            .zip(raw.as_chunks::<4>().0.iter().map(|source| source[3]))
        {
            out.push(quantize_u16(pixel[0]));
            out.push(quantize_u16(pixel[1]));
            out.push(quantize_u16(pixel[2]));
            out.push(quantize_u16(alpha));
        }
        Ok(out)
    } else {
        let transform = match Transform::<u8, u8, lcms2::ThreadContext>::new_context(
            context,
            source,
            PixelFormat::GRAY_8,
            target,
            PixelFormat::RGB_8,
            Intent::RelativeColorimetric,
        ) {
            Ok(transform) => transform,
            Err(error) => return Err((image, format!("无法创建灰度转换：{error}"))),
        };
        let raw = image.into_rgba8().into_raw();
        let shallow = raw.iter().copied().step_by(4).collect::<Vec<_>>();
        let mut rgb = vec![0u8; shallow.len() * 3];
        transform.transform_pixels(&shallow, &mut rgb);

        let mut out = Vec::with_capacity(shallow.len() * 4);
        for (pixel, alpha) in rgb
            .as_chunks::<3>()
            .0
            .iter()
            .zip(raw.as_chunks::<4>().0.iter().map(|source| source[3]))
        {
            out.extend_from_slice(pixel);
            out.push(alpha);
        }
        Ok(out)
    }
}

fn quantize_rgba16(pixels: &[[u16; 4]]) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixels.len() * 4);
    for pixel in pixels {
        for value in pixel {
            out.push(quantize_u16(*value));
        }
    }
    out
}

/// 16-bit → 8-bit 四舍五入，与 `image` crate 的转换保持一致。
fn quantize_u16(value: u16) -> u8 {
    ((u32::from(value) + 128) / 257) as u8
}

/// 测试用线性 RGB profile：中灰 128 转换到 sRGB 后约为 188。
#[cfg(test)]
pub(crate) fn test_linear_rgb_icc() -> Vec<u8> {
    use lcms2::{CIExyY, CIExyYTRIPLE, Profile, ToneCurve};

    let white = CIExyY {
        x: 0.3127,
        y: 0.3290,
        Y: 1.0,
    };
    let primaries = CIExyYTRIPLE {
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
    };
    let curve = ToneCurve::new(1.0);
    Profile::new_rgb(&white, &primaries, &[&curve, &curve, &curve])
        .expect("create linear RGB profile")
        .icc()
        .expect("serialize profile")
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, ImageBuffer, Rgb, Rgba};
    use lcms2::{CIExyY, CIExyYTRIPLE, Profile, ToneCurve};

    use super::*;

    fn d65() -> CIExyY {
        CIExyY {
            x: 0.3127,
            y: 0.3290,
            Y: 1.0,
        }
    }

    fn adobe_rgb_icc() -> Vec<u8> {
        let primaries = CIExyYTRIPLE {
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
        };
        let curve = ToneCurve::new(2.2);
        Profile::new_rgb(&d65(), &primaries, &[&curve, &curve, &curve])
            .expect("create AdobeRGB profile")
            .icc()
            .expect("serialize profile")
    }

    fn gray_icc() -> Vec<u8> {
        // 线性灰度：128 转换到 sRGB 后约为 188，便于验证确实发生了转换。
        let curve = ToneCurve::new(1.0);
        Profile::new_gray(&d65(), &curve)
            .expect("create gray profile")
            .icc()
            .expect("serialize profile")
    }

    #[test]
    fn missing_profile_keeps_pixels() {
        let image = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(2, 2, Rgb([10, 20, 30])));
        let (rgba, info) = to_srgb_rgba8(image, None);

        assert!(!info.converted);
        assert_eq!(info.label(), None);
        assert_eq!(rgba, [10, 20, 30, 255].repeat(4));
    }

    #[test]
    fn corrupt_profile_falls_back_to_original_pixels() {
        let image = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(2, 2, Rgb([10, 20, 30])));
        let (rgba, info) = to_srgb_rgba8(image, Some(b"not an ICC profile"));

        assert!(!info.converted);
        assert_eq!(rgba, [10, 20, 30, 255].repeat(4));
    }

    #[test]
    fn rgb_profile_is_converted_to_srgb() {
        // 线性 RGB 的中灰 128 转换到 sRGB 后约为 188。
        let image = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(2, 2, Rgb([128, 128, 128])));
        let (rgba, info) = to_srgb_rgba8(image, Some(&test_linear_rgb_icc()));

        assert!(info.converted);
        assert!(info.label().is_some());
        assert!(
            rgba[0].abs_diff(128) >= 40,
            "线性 RGB 中灰应转换到约 188，实际 {rgba:?}"
        );
        assert_eq!(rgba[0], rgba[1]);
        assert_eq!(rgba[1], rgba[2]);
        assert_eq!(rgba[3], 255);
    }

    #[test]
    fn alpha_is_preserved_for_rgb_profile() {
        let image =
            DynamicImage::ImageRgba8(ImageBuffer::from_pixel(2, 2, Rgba([200, 40, 60, 77])));
        let (rgba, info) = to_srgb_rgba8(image, Some(&adobe_rgb_icc()));

        assert!(info.converted);
        assert_eq!(rgba[3], 77, "alpha 必须保持不变");
        assert_eq!(rgba[7], 77);
    }

    #[test]
    fn sixteen_bit_values_stay_high_precision_until_quantization() {
        // 16-bit 灰阶：0x8000 应量化到 128，而不是截断成 127。
        let image = DynamicImage::ImageRgba16(ImageBuffer::from_pixel(
            2,
            2,
            image::Rgba([0x8000, 0x8000, 0x8000, 0x8080]),
        ));
        let (rgba, info) = to_srgb_rgba8(image, Some(&adobe_rgb_icc()));

        assert!(info.converted);
        assert_eq!(rgba[3], 128, "16-bit alpha 应保留并四舍五入");
        // 转换后的值允许因 profile 变化，但不能出现截断导致的 127。
        assert!(rgba[0] > 120, "实际 {}", rgba[0]);
    }

    #[test]
    fn gray_profile_is_converted_and_alpha_preserved() {
        let image =
            DynamicImage::ImageLumaA8(ImageBuffer::from_pixel(2, 2, image::LumaA([128, 42])));
        let (rgba, info) = to_srgb_rgba8(image, Some(&gray_icc()));

        assert!(info.converted);
        assert_eq!(rgba[3], 42);
        assert_eq!(rgba[0], rgba[1]);
        assert_eq!(rgba[1], rgba[2]);
        assert!(
            rgba[0].abs_diff(128) >= 40,
            "线性灰度 128 应转换到约 188，实际 {}",
            rgba[0]
        );
    }

    #[test]
    fn mismatched_profile_and_channels_fall_back() {
        let image = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(2, 2, Rgb([200, 40, 60])));
        let (rgba, info) = to_srgb_rgba8(image, Some(&gray_icc()));

        assert!(!info.converted, "灰度 profile 不应套用到 RGB 像素");
        assert_eq!(rgba, [200, 40, 60, 255].repeat(4));
    }

    #[test]
    fn sixteen_bit_source_depth_is_detected() {
        let image =
            DynamicImage::ImageRgb16(ImageBuffer::from_pixel(1, 1, image::Rgb([0u16, 0, 0])));
        assert_eq!(source_depth(&image), 16);
        assert_eq!(source_depth(&DynamicImage::new_rgb8(1, 1)), 8);
    }
}
