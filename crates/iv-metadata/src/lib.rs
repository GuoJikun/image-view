//! ICC 色彩管理：把图片内嵌的 profile 转换到显示目标空间。
//!
//! 目标空间只有两档（[`ColorTarget`]）：sRGB 与 Display P3。WebView 的画布
//! （`CanvasRenderingContext2DSettings.colorSpace`）只接受这两个值，任意显示器
//! profile 无法表达，所以不做「按显示器 ICC 转换」。
//!
//! 转换保留源位深：8-bit 源输出 8-bit RGBA，16-bit 源输出 16-bit RGBA。
//! 只有调用方明确需要 8-bit 时才调用 `into_rgba8()` 量化，避免二次量化造成色带。
//! profile 缺失、损坏或色彩空间不受支持时回退原像素，不阻塞显示。

use std::sync::OnceLock;

use image::DynamicImage;
use lcms2::{CIExyY, CIExyYTRIPLE, Profile, ToneCurve};

/// 色彩转换的目标空间。
///
/// 只提供两档的原因见模块文档：画布能声明的空间只有这两个值。前端的
/// `detectColorTarget()` 会先确认屏幕与引擎都支持 `display-p3`，再切到
/// [`ColorTarget::DisplayP3`]。
#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
pub enum ColorTarget {
    /// 标准 sRGB；所有平台都保证可显示。
    #[default]
    Srgb,
    /// Display P3；只在广色域屏 + 支持 `display-p3` 画布时启用。
    DisplayP3,
}

impl ColorTarget {
    /// 稳定标识，供 DTO 与前端 `colorTarget` 字段使用。
    pub fn id(self) -> &'static str {
        match self {
            Self::Srgb => "srgb",
            Self::DisplayP3 => "display-p3",
        }
    }

    /// 从标识还原；无法识别时返回 `None`，由调用方回退 sRGB。
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "srgb" => Some(Self::Srgb),
            "display-p3" => Some(Self::DisplayP3),
            _ => None,
        }
    }

    /// 目标空间的 ICC 字节；sRGB 返回 `None`（PNG 不打标签，浏览器默认即 sRGB）。
    ///
    /// 编码预览 PNG 时要把这份字节写进 `iCCP` 块：像素已经按该空间编码，
    /// 画布再按标签解释才能得到恒等结果；画布不支持时浏览器也会据此正确降级。
    pub fn icc(self) -> Option<&'static [u8]> {
        match self {
            Self::Srgb => None,
            Self::DisplayP3 => Some(display_p3_icc()),
        }
    }
}

/// 隐式 sRGB 源 profile 的 ICC 字节。
///
/// 用于「文件没有 profile，但目标不是 sRGB」的情况：此时像素按约定是 sRGB，
/// 必须真的做一次变换，否则会被当成目标空间的数值而整体过饱和。
pub fn srgb_icc() -> &'static [u8] {
    static ICC: OnceLock<Vec<u8>> = OnceLock::new();
    ICC.get_or_init(|| Profile::new_srgb().icc().expect("序列化内置 sRGB profile"))
}

/// Display P3 的 ICC 字节：D65 白点 + P3 原色 + sRGB 传递函数。
///
/// 自己构造而不是读系统文件，保证三端得到同一份字节（Windows 并不保证存在
/// 名为 Display P3 的系统 profile）。
fn display_p3_icc() -> &'static [u8] {
    static ICC: OnceLock<Vec<u8>> = OnceLock::new();
    ICC.get_or_init(|| {
        let white = CIExyY {
            x: 0.3127,
            y: 0.3290,
            Y: 1.0,
        };
        let primaries = CIExyYTRIPLE {
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
        };
        // 参数化曲线 type 4 = sRGB 的分段 EOTF，参数与 lcms 内置 sRGB profile 一致。
        let curve =
            ToneCurve::new_parametric(4, &[2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045])
                .expect("构造 sRGB 传递函数");
        Profile::new_rgb(&white, &primaries, &[&curve, &curve, &curve])
            .expect("构造 Display P3 profile")
            .icc()
            .expect("序列化 Display P3 profile")
    })
}

/// 色彩转换后的 RGBA 像素，位深跟随源图。
///
/// 每个像素 4 个通道（RGBA），通道值按 `u8` 或 `u16` 存储；字节序为本机，
/// 编码 PNG 时由 image crate 按 `ExtendedColorType` 解释。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pixels {
    /// 8-bit RGBA，`len()` 为通道数（像素数 × 4）。
    Rgba8(Vec<u8>),
    /// 16-bit RGBA，`len()` 为通道数（像素数 × 4）。
    Rgba16(Vec<u16>),
}

impl Pixels {
    /// 每个像素的字节数。
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            Self::Rgba8(_) => 4,
            Self::Rgba16(_) => 8,
        }
    }

    /// 位深（8 或 16）。
    pub fn depth(&self) -> u8 {
        match self {
            Self::Rgba8(_) => 8,
            Self::Rgba16(_) => 16,
        }
    }

    /// 像素数（通道数 ÷ 4）。
    pub fn pixel_count(&self) -> usize {
        match self {
            Self::Rgba8(values) => values.len() / 4,
            Self::Rgba16(values) => values.len() / 4,
        }
    }

    /// 原始字节视图：16-bit 为本机字节序，供 PNG 编码按 `depth()` 解释。
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Rgba8(values) => values,
            Self::Rgba16(values) => bytemuck::cast_slice(values),
        }
    }

    /// 量化为 8-bit：16-bit 源经四舍五入降位，8-bit 源原样返回。
    pub fn into_rgba8(self) -> Vec<u8> {
        match self {
            Self::Rgba8(values) => values,
            Self::Rgba16(values) => quantize_rgba16(&bytemuck::cast_vec::<u16, [u16; 4]>(values)),
        }
    }
}

/// 一次色彩转换的结果，供状态栏与调试日志使用。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ColorInfo {
    /// 是否实际执行了到目标空间的转换。
    pub converted: bool,
    /// 源 profile 的描述（可能为空或不存在）。
    pub source: Option<String>,
    /// 转换的目标空间；未转换时记录的是「像素被当作哪个空间」。
    pub target: ColorTarget,
}

impl ColorInfo {
    /// 源 profile 的描述；未转换或描述为空时返回 `None`。
    ///
    /// 状态栏文案由 GUI 负责本地化，本 crate 只提供数据。
    pub fn source_label(&self) -> Option<&str> {
        if !self.converted {
            return None;
        }
        self.source
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
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

/// 解码后的统一出口：按需做 ICC → 目标空间转换，位深跟随源图。
///
/// 8-bit 源返回 `Pixels::Rgba8`，16-bit 源返回 `Pixels::Rgba16`，全程不提前量化。
/// 失败时保留原始像素（`converted = false`），只写 debug 日志。
pub fn to_target_rgba(
    image: DynamicImage,
    icc: Option<&[u8]>,
    target: ColorTarget,
) -> (Pixels, ColorInfo) {
    let depth = source_depth(&image);
    let Some(icc) = icc.filter(|profile| !profile.is_empty()) else {
        // 没有 profile：按约定像素已经是 sRGB。
        return match target {
            // 目标就是 sRGB，数值不需要动。
            ColorTarget::Srgb => (raw_pixels(image, depth), ColorInfo::default()),
            // 目标是 P3，灰度图仍可原样输出：两个空间共用 D65 白点与同一条传递
            // 函数，中性轴的数值完全一致。
            ColorTarget::DisplayP3 if is_gray(&image) => {
                (raw_pixels(image, depth), fallback_info(target))
            }
            // 彩色像素必须真的做一次 sRGB → P3，否则会被当成 P3 数值而整体过饱和。
            ColorTarget::DisplayP3 => match convert(image, srgb_icc(), depth, target) {
                Ok((pixels, info)) => (pixels, info),
                Err((image, reason)) => fallback(image, depth, target, &reason),
            },
        };
    };

    match convert(image, icc, depth, target) {
        Ok((pixels, info)) => (pixels, info),
        Err((image, reason)) => fallback(image, depth, target, &reason),
    }
}

/// 固定 sRGB 目标的便捷入口。
pub fn to_srgb_rgba(image: DynamicImage, icc: Option<&[u8]>) -> (Pixels, ColorInfo) {
    to_target_rgba(image, icc, ColorTarget::Srgb)
}

/// 始终量化为 8-bit 的便捷包装；供确实只需要 8-bit 的调用方使用。
pub fn to_srgb_rgba8(image: DynamicImage, icc: Option<&[u8]>) -> (Vec<u8>, ColorInfo) {
    let (pixels, info) = to_srgb_rgba(image, icc);
    (pixels.into_rgba8(), info)
}

/// 未转换时记录的目标空间：像素被当作 `target` 解释。
fn fallback_info(target: ColorTarget) -> ColorInfo {
    ColorInfo {
        converted: false,
        source: None,
        target,
    }
}

/// 转换失败：保留原始像素并写 debug 日志，界面照常显示。
fn fallback(
    image: DynamicImage,
    depth: u8,
    target: ColorTarget,
    reason: &str,
) -> (Pixels, ColorInfo) {
    log::debug!(
        target: "image_view::color",
        "ICC 色彩转换失败，按原像素显示：{reason}"
    );
    (raw_pixels(image, depth), fallback_info(target))
}

/// 灰度图（单通道或带 alpha 的灰度）。
fn is_gray(image: &DynamicImage) -> bool {
    matches!(
        image,
        DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageLuma16(_)
            | DynamicImage::ImageLumaA16(_)
    )
}

/// 不做色彩转换，按源位深输出原始 RGBA。
fn raw_pixels(image: DynamicImage, depth: u8) -> Pixels {
    if depth == 16 {
        Pixels::Rgba16(image.into_rgba16().into_raw())
    } else {
        Pixels::Rgba8(image.into_rgba8().into_raw())
    }
}

fn convert(
    image: DynamicImage,
    icc: &[u8],
    depth: u8,
    target: ColorTarget,
) -> Result<(Pixels, ColorInfo), (DynamicImage, String)> {
    use lcms2::{InfoType, Locale, ThreadContext};

    // 解码线程是单线程的，但测试可能并行运行；使用私有 context 避免共享 LCMS 全局状态。
    let context = ThreadContext::new();
    let source = match Profile::new_icc_context(&context, icc) {
        Ok(profile) => profile,
        Err(error) => return Err((image, format!("无法解析 ICC profile：{error}"))),
    };
    let target_profile = match target_profile(&context, target) {
        Ok(profile) => profile,
        Err(reason) => return Err((image, reason)),
    };
    let description = source
        .info(InfoType::Description, Locale::none())
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty());
    let info = ColorInfo {
        converted: true,
        source: description,
        target,
    };
    let is_gray = is_gray(&image);

    match (source.color_space(), is_gray) {
        (lcms2::ColorSpaceSignature::RgbData, false) => {
            convert_rgb(&context, &source, &target_profile, image, depth)
                .map(|pixels| (pixels, info))
        }
        (lcms2::ColorSpaceSignature::GrayData, true) => {
            convert_gray(&context, &source, &target_profile, image, depth)
                .map(|pixels| (pixels, info))
        }
        (signature, gray) => Err((
            image,
            format!("ICC 色彩空间 {signature:?} 与图像通道不匹配（灰度={gray}）"),
        )),
    }
}

/// 目标空间的 profile；Display P3 复用同一份缓存字节，避免每次解析。
fn target_profile(
    context: &lcms2::ThreadContext,
    target: ColorTarget,
) -> Result<lcms2::Profile<lcms2::ThreadContext>, String> {
    match target {
        ColorTarget::Srgb => Ok(Profile::new_srgb_context(context)),
        ColorTarget::DisplayP3 => Profile::new_icc_context(context, display_p3_icc())
            .map_err(|error| format!("无法加载 Display P3 profile：{error}")),
    }
}

fn convert_rgb(
    context: &lcms2::ThreadContext,
    source: &lcms2::Profile<lcms2::ThreadContext>,
    target: &lcms2::Profile<lcms2::ThreadContext>,
    image: DynamicImage,
    depth: u8,
) -> Result<Pixels, (DynamicImage, String)> {
    use lcms2::{Flags, Intent, PixelFormat, Transform};

    if depth == 16 {
        // 16-bit：在 16-bit 空间完成转换与 alpha 复制，保留 16-bit 输出。
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
        Ok(Pixels::Rgba16(bytemuck::cast_vec(pixels)))
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
        Ok(Pixels::Rgba8(rgba))
    }
}

fn convert_gray(
    context: &lcms2::ThreadContext,
    source: &lcms2::Profile<lcms2::ThreadContext>,
    target: &lcms2::Profile<lcms2::ThreadContext>,
    image: DynamicImage,
    depth: u8,
) -> Result<Pixels, (DynamicImage, String)> {
    use lcms2::{Intent, PixelFormat, Transform};

    if depth == 16 {
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
            out.push(pixel[0]);
            out.push(pixel[1]);
            out.push(pixel[2]);
            out.push(alpha);
        }
        Ok(Pixels::Rgba16(out))
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
        Ok(Pixels::Rgba8(out))
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
///
/// 公开供 `iv-image-decode` 的集成测试复用；不属于稳定 API。
#[doc(hidden)]
pub fn test_linear_rgb_icc() -> Vec<u8> {
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
        assert_eq!(info.source_label(), None);
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
        assert!(info.source.is_some());
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

    #[test]
    fn to_srgb_rgba_preserves_sixteen_bit_without_icc() {
        let image = DynamicImage::ImageRgba16(ImageBuffer::from_pixel(
            2,
            2,
            image::Rgba([0x1234, 0x5678, 0x9abc, 0xdef0]),
        ));
        let (pixels, info) = to_srgb_rgba(image, None);

        assert!(!info.converted);
        assert_eq!(pixels.depth(), 16);
        assert_eq!(pixels.bytes_per_pixel(), 8);
        assert_eq!(pixels.pixel_count(), 4);
        let values = match pixels {
            Pixels::Rgba16(values) => values,
            Pixels::Rgba8(_) => panic!("16-bit 源应输出 Rgba16"),
        };
        assert_eq!(&values[..4], &[0x1234, 0x5678, 0x9abc, 0xdef0]);
    }

    #[test]
    fn to_srgb_rgba_preserves_sixteen_bit_after_icc_conversion() {
        // 16-bit 灰阶经线性 profile 转换后仍应是 16-bit，不提前量化到 8-bit。
        let image = DynamicImage::ImageRgba16(ImageBuffer::from_pixel(
            1,
            1,
            image::Rgba([0x8000, 0x8000, 0x8000, 0xffff]),
        ));
        let (pixels, info) = to_srgb_rgba(image, Some(&adobe_rgb_icc()));

        assert!(info.converted);
        assert_eq!(pixels.depth(), 16);
        let values = match pixels {
            Pixels::Rgba16(values) => values,
            Pixels::Rgba8(_) => panic!("ICC 转换应保留 16-bit"),
        };
        assert_eq!(values[3], 0xffff, "alpha 应原样保留");
        assert!(values[0] > 0, "转换后的灰度值应为非零 16-bit 值");
    }

    #[test]
    fn into_rgba8_quantizes_sixteen_bit_once() {
        let image = DynamicImage::ImageRgba16(ImageBuffer::from_pixel(
            1,
            1,
            image::Rgba([0x8080, 0, 0, 0xffff]),
        ));
        let (pixels, _) = to_srgb_rgba(image, None);
        let rgba8 = pixels.into_rgba8();
        assert_eq!(rgba8[0], 128, "0x8080 应量化为 128");
        assert_eq!(rgba8[3], 255);
    }

    #[test]
    fn display_p3_profile_is_parseable() {
        let icc = ColorTarget::DisplayP3.icc().expect("P3 应提供 ICC 字节");
        let context = lcms2::ThreadContext::new();
        let profile = Profile::new_icc_context(&context, icc).expect("P3 profile 应可解析");
        assert_eq!(profile.color_space(), lcms2::ColorSpaceSignature::RgbData);
        assert!(
            ColorTarget::Srgb.icc().is_none(),
            "sRGB 不需要给 PNG 打标签"
        );
    }

    #[test]
    fn srgb_red_converts_to_display_p3() {
        // sRGB 正红在 Display P3 中约为 (234, 51, 35)；这条断言同时校验
        // 原色坐标与传递函数是否都取对了。
        let image = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(1, 1, Rgb([255, 0, 0])));
        let (pixels, info) = to_target_rgba(image, None, ColorTarget::DisplayP3);
        let rgba = pixels.into_rgba8();

        assert!(info.converted, "无 profile 时目标为 P3 也必须真的转换");
        assert_eq!(info.target, ColorTarget::DisplayP3);
        assert!(rgba[0].abs_diff(234) <= 3, "R 实际 {}", rgba[0]);
        assert!(rgba[1].abs_diff(51) <= 3, "G 实际 {}", rgba[1]);
        assert!(rgba[2].abs_diff(35) <= 3, "B 实际 {}", rgba[2]);
        assert_eq!(rgba[3], 255);
    }

    #[test]
    fn display_p3_source_to_p3_target_is_identity() {
        let icc = ColorTarget::DisplayP3.icc().expect("P3 ICC");
        let image = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(1, 1, Rgb([200, 120, 60])));
        let (pixels, info) = to_target_rgba(image, Some(icc), ColorTarget::DisplayP3);
        let rgba = pixels.into_rgba8();

        assert!(info.converted);
        for (actual, expected) in rgba.iter().zip([200u8, 120, 60, 255]) {
            assert!(
                actual.abs_diff(expected) <= 1,
                "P3 → P3 应近似恒等，实际 {rgba:?}"
            );
        }
    }

    #[test]
    fn gray_without_profile_stays_unchanged_for_p3() {
        // 灰度与 P3 共用 D65 白点与同一条传递函数，中性轴数值不变。
        let image = DynamicImage::ImageLuma8(ImageBuffer::from_pixel(1, 1, image::Luma([128u8])));
        let (pixels, info) = to_target_rgba(image, None, ColorTarget::DisplayP3);
        let rgba = pixels.into_rgba8();

        assert!(!info.converted, "灰度的中性轴不需要变换");
        assert_eq!(rgba, [128, 128, 128, 255]);
        assert_eq!(info.target, ColorTarget::DisplayP3);
    }

    #[test]
    fn color_target_id_round_trips() {
        for target in [ColorTarget::Srgb, ColorTarget::DisplayP3] {
            assert_eq!(ColorTarget::from_id(target.id()), Some(target));
        }
        assert_eq!(ColorTarget::from_id("adobe-rgb"), None);
    }
}
