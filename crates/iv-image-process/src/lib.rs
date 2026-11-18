//! 像素处理。
//!
//! 90° 旋转在 GUI 绘制阶段通过变换完成，缩放在解码阶段由 `iv-image-decode`
//! 调用 `image` crate 完成；翻转需要改写像素（Iced 0.14 的 canvas 不支持镜像
//! 变换），因此在这里提供纯缓冲区函数，保持输入/输出不依赖 GUI。

/// 按行主序翻转 RGBA8 像素；`flip_x` 为水平镜像，`flip_y` 为垂直镜像。
///
/// 缓冲区长度必须是 `width * height * 4`，否则原样返回一份拷贝（调用方传入的
/// `iced::image::Handle::Rgba` 由 Iced 保证长度，这里只做防御）。不修改输入。
pub fn flip_rgba8(pixels: &[u8], width: u32, height: u32, flip_x: bool, flip_y: bool) -> Vec<u8> {
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .unwrap_or(usize::MAX);
    if pixels.len() != expected || (!flip_x && !flip_y) {
        return pixels.to_vec();
    }

    let mut out = vec![0u8; pixels.len()];
    let stride = width as usize * 4;
    for y in 0..height as usize {
        let src_y = if flip_y { height as usize - 1 - y } else { y };
        for x in 0..width as usize {
            let src_x = if flip_x { width as usize - 1 - x } else { x };
            let src = src_y * stride + src_x * 4;
            let dst = y * stride + x * 4;
            out[dst..dst + 4].copy_from_slice(&pixels[src..src + 4]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2×2 像素：每像素 4 字节用第一个字节标识，便于断言方向。
    fn fixture() -> Vec<u8> {
        vec![
            1, 0, 0, 0, 2, 0, 0, 0, //
            3, 0, 0, 0, 4, 0, 0, 0, //
        ]
    }

    fn marks(pixels: &[u8]) -> Vec<u8> {
        pixels.as_chunks::<4>().0.iter().map(|p| p[0]).collect()
    }

    #[test]
    fn no_flip_copies_input() {
        let source = fixture();
        assert_eq!(flip_rgba8(&source, 2, 2, false, false), source);
    }

    #[test]
    fn flip_x_mirrors_each_row() {
        assert_eq!(
            marks(&flip_rgba8(&fixture(), 2, 2, true, false)),
            vec![2, 1, 4, 3]
        );
    }

    #[test]
    fn flip_y_reverses_row_order() {
        assert_eq!(
            marks(&flip_rgba8(&fixture(), 2, 2, false, true)),
            vec![3, 4, 1, 2]
        );
    }

    #[test]
    fn flipping_both_axes_rotates_180() {
        assert_eq!(
            marks(&flip_rgba8(&fixture(), 2, 2, true, true)),
            vec![4, 3, 2, 1]
        );
    }

    #[test]
    fn invalid_length_is_left_untouched() {
        let source = vec![1, 2, 3];
        assert_eq!(flip_rgba8(&source, 2, 2, true, true), source);
    }
}
