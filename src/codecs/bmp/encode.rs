//! Pure-Rust BMP encoder for indexed grayscale and true-color images.

use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage, ImageMode, ImagePalette};

const FILE_HEADER_SIZE: usize = 14;
const INFO_HEADER_SIZE: u32 = 40;
const V4_HEADER_SIZE: u32 = 108;
const V5_HEADER_SIZE: u32 = 124;
const BI_RGB: u32 = 0;
const BI_RLE8: u32 = 1;
const BI_RLE4: u32 = 2;
const BI_BITFIELDS: u32 = 3;
// Pillow 12.2.0 BmpImagePlugin.py:437-440 defaults to 96 DPI and converts
// using round(96 * 39.3701), yielding 3,780 pixels per meter on both axes.
const DEFAULT_PIXELS_PER_METER: i32 = 3_780;

#[derive(Clone, Copy)]
enum Compression {
    Rgb,
    Rle8,
    Rle4,
    Bitfields,
}

fn row_size(bits_per_pixel: u16, width: u32) -> Option<usize> {
    usize::try_from(
        u64::from(bits_per_pixel)
            .checked_mul(u64::from(width))?
            .div_ceil(32)
            .checked_mul(4)?,
    )
    .ok()
}

/// Encode a `DecodedImage` as BMP bytes.
///
/// The `bit_depth`, `top_down`, `header`, and `compression` entries in
/// [`EncodeOptions::extra`] select 1/4/8/16/24/32-bit output, scanline
/// direction, BITMAPINFO/V4/V5 headers, and RGB/RLE/bitfield compression.
pub fn encode(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    if img.width == 0 || img.height == 0 {
        return None;
    }
    let channels = match img.color {
        ColorType::L8 => 1usize,
        ColorType::Rgb8 => 3,
        ColorType::Rgba8 => 4,
        _ => return None,
    };
    let mut depth = opts
        .extra
        .get("bit_depth")
        .and_then(|value| value.parse::<u16>().ok());
    let top_down = opts
        .extra
        .get("top_down")
        .is_some_and(|value| value == "true");
    let header_size = match opts.extra.get("header").map(String::as_str) {
        None | Some("V3") => INFO_HEADER_SIZE,
        Some("V4") => V4_HEADER_SIZE,
        Some("V5") => V5_HEADER_SIZE,
        Some(_) => return None,
    };
    let compression = match opts.extra.get("compression").map(String::as_str) {
        None | Some("BI_RGB") => Compression::Rgb,
        Some("BI_RLE8") => Compression::Rle8,
        Some("BI_RLE4") => Compression::Rle4,
        Some("BI_BITFIELDS") => Compression::Bitfields,
        Some(_) => return None,
    };
    if depth.is_none() {
        depth = match compression {
            Compression::Rle8 => Some(8),
            Compression::Rle4 => Some(4),
            Compression::Bitfields => Some(32),
            Compression::Rgb => None,
        };
    }

    let unpacked_len = usize::try_from(img.width)
        .ok()?
        .checked_mul(usize::try_from(img.height).ok()?)?
        .checked_mul(channels)?;
    let valid_unpacked = img.pixels.len() == unpacked_len;

    match (compression, img.color, depth) {
        (Compression::Rgb, ColorType::L8, Some(1)) => {
            encode_1bit(img.width, img.height, &img.pixels, top_down, header_size)
        }
        (Compression::Rgb, ColorType::L8, Some(4)) if valid_unpacked => {
            encode_4bit(img.width, img.height, &img.pixels, top_down, header_size)
        }
        (Compression::Rgb, ColorType::L8, None | Some(8)) if valid_unpacked => encode_l8(
            img.width,
            img.height,
            &img.pixels,
            (img.mode == ImageMode::P8)
                .then_some(img.palette.as_ref())
                .flatten(),
            top_down,
            header_size,
        ),
        (Compression::Rle8, ColorType::L8, Some(8)) if valid_unpacked && !top_down => {
            encode_rle(img.width, img.height, &img.pixels, 8, header_size)
        }
        (Compression::Rle4, ColorType::L8, Some(4)) if valid_unpacked && !top_down => {
            encode_rle(img.width, img.height, &img.pixels, 4, header_size)
        }
        (Compression::Rgb, ColorType::Rgb8, Some(16)) if valid_unpacked => {
            encode_rgb16(img.width, img.height, &img.pixels, top_down, header_size)
        }
        (Compression::Rgb, ColorType::Rgb8, None | Some(24)) if valid_unpacked => {
            encode_rgb24(img.width, img.height, &img.pixels, top_down, header_size)
        }
        (Compression::Rgb | Compression::Bitfields, ColorType::Rgb8, Some(32))
            if valid_unpacked =>
        {
            encode_rgb32(
                img.width,
                img.height,
                &rgb_to_rgba(&img.pixels),
                top_down,
                header_size,
                matches!(compression, Compression::Bitfields),
            )
        }
        (Compression::Rgb, ColorType::Rgba8, Some(16)) if valid_unpacked => encode_rgb16(
            img.width,
            img.height,
            &rgba_to_rgb(&img.pixels),
            top_down,
            header_size,
        ),
        (Compression::Rgb | Compression::Bitfields, ColorType::Rgba8, None | Some(32))
            if valid_unpacked =>
        {
            encode_rgb32(
                img.width,
                img.height,
                &img.pixels,
                top_down,
                header_size,
                matches!(compression, Compression::Bitfields),
            )
        }
        (Compression::Rgb, ColorType::Rgba8, Some(24)) if valid_unpacked => encode_rgb24(
            img.width,
            img.height,
            &rgba_to_rgb(&img.pixels),
            top_down,
            header_size,
        ),
        _ => None,
    }
}

fn encode_1bit(
    width: u32,
    height: u32,
    pixels: &[u8],
    top_down: bool,
    header_size: u32,
) -> Option<Vec<u8>> {
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    let packed_width = width.div_ceil(8);
    let packed_len = packed_width.checked_mul(height)?;
    let sample_len = width.checked_mul(height)?;
    if pixels.len() != packed_len && pixels.len() != sample_len {
        return None;
    }
    let stride = row_size(1, u32::try_from(width).ok()?)?;
    let pixel_bytes = stride.checked_mul(height)?;
    let palette_bytes = 8usize;
    let pixel_offset = FILE_HEADER_SIZE
        .checked_add(usize::try_from(header_size).ok()?)?
        .checked_add(palette_bytes)?;
    let mut output = bmp_headers(
        u32::try_from(width).ok()?,
        u32::try_from(height).ok()?,
        1,
        BI_RGB,
        header_size,
        top_down,
        2,
        pixel_offset,
        pixel_bytes,
    )?;
    output.extend_from_slice(&[0, 0, 0, 0, 255, 255, 255, 0]);
    for output_row in 0..height {
        let source_row = source_row(output_row, height, top_down)?;
        let row_start = source_row.checked_mul(if pixels.len() == packed_len {
            packed_width
        } else {
            width
        })?;
        if pixels.len() == packed_len {
            output.extend_from_slice(pixels.get(row_start..row_start + packed_width)?);
        } else {
            let row = pixels.get(row_start..row_start + width)?;
            for byte_start in (0..width).step_by(8) {
                let mut packed = 0u8;
                for bit in 0..8 {
                    if row.get(byte_start + bit).is_some_and(|&value| value >= 128) {
                        packed |= 0x80 >> bit;
                    }
                }
                output.push(packed);
            }
        }
        output.resize(
            output
                .len()
                .checked_add(stride.checked_sub(packed_width)?)?,
            0,
        );
    }
    Some(output)
}

fn encode_4bit(
    width: u32,
    height: u32,
    pixels: &[u8],
    top_down: bool,
    header_size: u32,
) -> Option<Vec<u8>> {
    let width_usize = usize::try_from(width).ok()?;
    let height_usize = usize::try_from(height).ok()?;
    let packed_width = width_usize.div_ceil(2);
    let stride = row_size(4, width)?;
    let pixel_bytes = stride.checked_mul(height_usize)?;
    let palette_bytes = 16usize.checked_mul(4)?;
    let pixel_offset = FILE_HEADER_SIZE
        .checked_add(usize::try_from(header_size).ok()?)?
        .checked_add(palette_bytes)?;
    let mut output = bmp_headers(
        width,
        height,
        4,
        BI_RGB,
        header_size,
        top_down,
        16,
        pixel_offset,
        pixel_bytes,
    )?;
    write_gray_palette(&mut output, 16);
    for output_row in 0..height_usize {
        let source_row = source_row(output_row, height_usize, top_down)?;
        let start = source_row.checked_mul(width_usize)?;
        let row = pixels.get(start..start.checked_add(width_usize)?)?;
        for pair in row.chunks(2) {
            output.push(((pair[0] & 0x0f) << 4) | (pair.get(1).copied().unwrap_or(0) & 0x0f));
        }
        output.resize(
            output
                .len()
                .checked_add(stride.checked_sub(packed_width)?)?,
            0,
        );
    }
    Some(output)
}

fn encode_rle(
    width: u32,
    height: u32,
    pixels: &[u8],
    depth: u16,
    header_size: u32,
) -> Option<Vec<u8>> {
    let width_usize = usize::try_from(width).ok()?;
    let height_usize = usize::try_from(height).ok()?;
    let mut encoded = Vec::new();
    for output_row in 0..height_usize {
        let source_row = source_row(output_row, height_usize, false)?;
        let start = source_row.checked_mul(width_usize)?;
        let row = pixels.get(start..start.checked_add(width_usize)?)?;
        for &index in row {
            encoded.push(1);
            encoded.push(if depth == 4 {
                (index & 0x0f) << 4
            } else {
                index
            });
        }
        encoded.extend_from_slice(&[0, 0]);
    }
    encoded.extend_from_slice(&[0, 1]);

    let color_count = 1usize.checked_shl(u32::from(depth))?;
    let palette_bytes = color_count.checked_mul(4)?;
    let pixel_offset = FILE_HEADER_SIZE
        .checked_add(usize::try_from(header_size).ok()?)?
        .checked_add(palette_bytes)?;
    let mut output = bmp_headers(
        width,
        height,
        depth,
        if depth == 4 { BI_RLE4 } else { BI_RLE8 },
        header_size,
        false,
        u32::try_from(color_count).ok()?,
        pixel_offset,
        encoded.len(),
    )?;
    write_gray_palette(&mut output, color_count);
    output.extend_from_slice(&encoded);
    Some(output)
}

fn encode_rgb16(
    width: u32,
    height: u32,
    pixels: &[u8],
    top_down: bool,
    header_size: u32,
) -> Option<Vec<u8>> {
    let stride = row_size(16, width)?;
    let pixel_bytes = stride.checked_mul(usize::try_from(height).ok()?)?;
    let pixel_offset = FILE_HEADER_SIZE.checked_add(usize::try_from(header_size).ok()?)?;
    let mut output = bmp_headers(
        width,
        height,
        16,
        BI_RGB,
        header_size,
        top_down,
        0,
        pixel_offset,
        pixel_bytes,
    )?;
    let width_usize = usize::try_from(width).ok()?;
    let height_usize = usize::try_from(height).ok()?;
    let source_stride = width_usize.checked_mul(3)?;
    let encoded_row = width_usize.checked_mul(2)?;
    for output_row in 0..height_usize {
        let source_row = source_row(output_row, height_usize, top_down)?;
        let start = source_row.checked_mul(source_stride)?;
        let row = pixels.get(start..start.checked_add(source_stride)?)?;
        for pixel in row.chunks_exact(3) {
            let red = (u16::from(pixel[0]) * 31 + 127) / 255;
            let green = (u16::from(pixel[1]) * 31 + 127) / 255;
            let blue = (u16::from(pixel[2]) * 31 + 127) / 255;
            let packed = (red << 10) | (green << 5) | blue;
            output.extend_from_slice(&packed.to_le_bytes());
        }
        output.resize(
            output.len().checked_add(stride.checked_sub(encoded_row)?)?,
            0,
        );
    }
    Some(output)
}

fn write_gray_palette(output: &mut Vec<u8>, color_count: usize) {
    let divisor = color_count.saturating_sub(1).max(1);
    for index in 0..color_count {
        let value = u8::try_from(index * 255 / divisor).unwrap_or(255);
        output.extend_from_slice(&[value, value, value, 0]);
    }
}

fn source_row(output_row: usize, height: usize, top_down: bool) -> Option<usize> {
    if top_down {
        Some(output_row)
    } else {
        height.checked_sub(output_row)?.checked_sub(1)
    }
}

fn encode_l8(
    width: u32,
    height: u32,
    pixels: &[u8],
    palette: Option<&ImagePalette>,
    top_down: bool,
    header_size: u32,
) -> Option<Vec<u8>> {
    let stride = row_size(8, width)?;
    // Pillow 12.2.0 BmpImagePlugin.py:446-452 retains the exact P-mode
    // palette length and writes it as BGRX. Ordinary L mode gets 256 gray
    // entries instead.
    let color_count = palette.map_or(256, ImagePalette::len);
    let palette_bytes = color_count.checked_mul(4)?;
    let pixel_bytes = stride.checked_mul(usize::try_from(height).ok()?)?;
    let pixel_offset = FILE_HEADER_SIZE
        .checked_add(usize::try_from(header_size).ok()?)?
        .checked_add(palette_bytes)?;
    let mut output = bmp_headers(
        width,
        height,
        8,
        BI_RGB,
        header_size,
        top_down,
        u32::try_from(color_count).ok()?,
        pixel_offset,
        pixel_bytes,
    )?;
    if let Some(palette) = palette {
        for rgb in palette.rgb.chunks_exact(3) {
            output.extend_from_slice(&[rgb[2], rgb[1], rgb[0], 0]);
        }
    } else {
        for value in 0..=255u8 {
            output.extend_from_slice(&[value, value, value, 0]);
        }
    }
    write_rows(
        &mut output,
        pixels,
        width,
        height,
        1,
        stride,
        top_down,
        |pixel, out| {
            out.push(pixel[0]);
        },
    )?;
    Some(output)
}

fn encode_rgb24(
    width: u32,
    height: u32,
    pixels: &[u8],
    top_down: bool,
    header_size: u32,
) -> Option<Vec<u8>> {
    let stride = row_size(24, width)?;
    let pixel_bytes = stride.checked_mul(usize::try_from(height).ok()?)?;
    let pixel_offset = FILE_HEADER_SIZE.checked_add(usize::try_from(header_size).ok()?)?;
    let mut output = bmp_headers(
        width,
        height,
        24,
        BI_RGB,
        header_size,
        top_down,
        0,
        pixel_offset,
        pixel_bytes,
    )?;
    write_rows(
        &mut output,
        pixels,
        width,
        height,
        3,
        stride,
        top_down,
        |pixel, out| {
            out.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
        },
    )?;
    Some(output)
}

fn encode_rgb32(
    width: u32,
    height: u32,
    pixels: &[u8],
    top_down: bool,
    requested_header: u32,
    bitfields: bool,
) -> Option<Vec<u8>> {
    let header_size = if bitfields {
        requested_header.max(56)
    } else {
        requested_header
    };
    let stride = row_size(32, width)?;
    let pixel_bytes = stride.checked_mul(usize::try_from(height).ok()?)?;
    let pixel_offset = FILE_HEADER_SIZE.checked_add(usize::try_from(header_size).ok()?)?;
    let mut output = bmp_headers(
        width,
        height,
        32,
        if bitfields { BI_BITFIELDS } else { BI_RGB },
        header_size,
        top_down,
        0,
        pixel_offset,
        pixel_bytes,
    )?;
    if bitfields {
        let mask_start = FILE_HEADER_SIZE.checked_add(40)?;
        output
            .get_mut(mask_start..mask_start + 4)?
            .copy_from_slice(&0x00ff_0000u32.to_le_bytes());
        output
            .get_mut(mask_start + 4..mask_start + 8)?
            .copy_from_slice(&0x0000_ff00u32.to_le_bytes());
        output
            .get_mut(mask_start + 8..mask_start + 12)?
            .copy_from_slice(&0x0000_00ffu32.to_le_bytes());
        output
            .get_mut(mask_start + 12..mask_start + 16)?
            .copy_from_slice(&0xff00_0000u32.to_le_bytes());
    }
    write_rows(
        &mut output,
        pixels,
        width,
        height,
        4,
        stride,
        top_down,
        |pixel, out| {
            out.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        },
    )?;
    Some(output)
}

#[allow(clippy::too_many_arguments)]
fn bmp_headers(
    width: u32,
    height: u32,
    depth: u16,
    compression: u32,
    header_size: u32,
    top_down: bool,
    colors: u32,
    pixel_offset: usize,
    pixel_bytes: usize,
) -> Option<Vec<u8>> {
    let file_size = pixel_offset.checked_add(pixel_bytes)?;
    let signed_width = i32::try_from(width).ok()?;
    let mut signed_height = i32::try_from(height).ok()?;
    if top_down {
        signed_height = signed_height.checked_neg()?;
    }
    let mut output = Vec::with_capacity(file_size);
    output.extend_from_slice(b"BM");
    output.extend_from_slice(&u32::try_from(file_size).ok()?.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&u32::try_from(pixel_offset).ok()?.to_le_bytes());
    output.extend_from_slice(&header_size.to_le_bytes());
    output.extend_from_slice(&signed_width.to_le_bytes());
    output.extend_from_slice(&signed_height.to_le_bytes());
    output.extend_from_slice(&1u16.to_le_bytes());
    output.extend_from_slice(&depth.to_le_bytes());
    output.extend_from_slice(&compression.to_le_bytes());
    output.extend_from_slice(&u32::try_from(pixel_bytes).ok()?.to_le_bytes());
    output.extend_from_slice(&DEFAULT_PIXELS_PER_METER.to_le_bytes());
    output.extend_from_slice(&DEFAULT_PIXELS_PER_METER.to_le_bytes());
    output.extend_from_slice(&colors.to_le_bytes());
    output.extend_from_slice(&colors.to_le_bytes());
    output.resize(
        FILE_HEADER_SIZE.checked_add(usize::try_from(header_size).ok()?)?,
        0,
    );
    Some(output)
}

#[allow(clippy::too_many_arguments)]
fn write_rows(
    output: &mut Vec<u8>,
    pixels: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    stride: usize,
    top_down: bool,
    mut write_pixel: impl FnMut(&[u8], &mut Vec<u8>),
) -> Option<()> {
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    let source_stride = width.checked_mul(channels)?;
    let encoded_row = width.checked_mul(channels)?;
    for output_row in 0..height {
        let source_row = if top_down {
            output_row
        } else {
            height.checked_sub(output_row)?.checked_sub(1)?
        };
        let start = source_row.checked_mul(source_stride)?;
        let row = pixels.get(start..start.checked_add(source_stride)?)?;
        for pixel in row.chunks_exact(channels) {
            write_pixel(pixel, output);
        }
        output.resize(
            output.len().checked_add(stride.checked_sub(encoded_row)?)?,
            0,
        );
    }
    Some(())
}

fn rgb_to_rgba(pixels: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(pixels.len() / 3 * 4);
    for pixel in pixels.chunks_exact(3) {
        output.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
    }
    output
}

fn rgba_to_rgb(pixels: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(pixels.len() / 4 * 3);
    for pixel in pixels.chunks_exact(4) {
        output.extend_from_slice(&pixel[..3]);
    }
    output
}
