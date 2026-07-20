//! Pure-Rust BMP encoder for indexed grayscale and true-color images.

use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage, ImageMode, ImagePalette};

const FILE_HEADER_SIZE: usize = 14;
const INFO_HEADER_SIZE: u32 = 40;
const BI_RGB: u32 = 0;
// Pillow 12.2.0 BmpImagePlugin.py:437-440 defaults to 96 DPI and converts
// using round(96 * 39.3701), yielding 3,780 pixels per meter on both axes.
const DEFAULT_PIXELS_PER_METER: i32 = 3_780;

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
/// The `bit_depth` entry in [`EncodeOptions::extra`] selects Pillow's
/// mode-derived 1/8/24/32-bit output. Pillow emits bottom-up BI_RGB images
/// with a BITMAPINFOHEADER; unsupported BMP save options are rejected.
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
    let depth = opts
        .extra
        .get("bit_depth")
        .and_then(|value| value.parse::<u16>().ok());
    if opts
        .extra
        .get("top_down")
        .is_some_and(|value| value != "false")
        || opts.extra.get("header").is_some_and(|value| value != "V3")
        || opts
            .extra
            .get("compression")
            .is_some_and(|value| value != "BI_RGB")
    {
        return None;
    }

    let unpacked_len = usize::try_from(img.width)
        .ok()?
        .checked_mul(usize::try_from(img.height).ok()?)?
        .checked_mul(channels)?;
    let valid_unpacked = img.pixels.len() == unpacked_len;

    match (img.color, depth) {
        (ColorType::L8, Some(1)) => {
            encode_1bit(img.width, img.height, &img.pixels, INFO_HEADER_SIZE)
        }
        (ColorType::L8, None | Some(8)) if valid_unpacked => encode_l8(
            img.width,
            img.height,
            &img.pixels,
            (img.mode == ImageMode::P8)
                .then_some(img.palette.as_ref())
                .flatten(),
            INFO_HEADER_SIZE,
        ),
        (ColorType::Rgb8, None | Some(24)) if valid_unpacked => {
            encode_rgb24(img.width, img.height, &img.pixels, INFO_HEADER_SIZE)
        }
        (ColorType::Rgba8, None | Some(32)) if valid_unpacked => {
            encode_rgb32(img.width, img.height, &img.pixels, INFO_HEADER_SIZE)
        }
        (ColorType::Rgba8, Some(24)) if valid_unpacked => encode_rgb24(
            img.width,
            img.height,
            &rgba_to_rgb(&img.pixels),
            INFO_HEADER_SIZE,
        ),
        _ => None,
    }
}

fn encode_1bit(width: u32, height: u32, pixels: &[u8], header_size: u32) -> Option<Vec<u8>> {
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
        header_size,
        2,
        pixel_offset,
        pixel_bytes,
    )?;
    output.extend_from_slice(&[0, 0, 0, 0, 255, 255, 255, 0]);
    for output_row in 0..height {
        let source_row = source_row(output_row, height)?;
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

fn source_row(output_row: usize, height: usize) -> Option<usize> {
    height.checked_sub(output_row)?.checked_sub(1)
}

fn encode_l8(
    width: u32,
    height: u32,
    pixels: &[u8],
    palette: Option<&ImagePalette>,
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
        header_size,
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
        |pixel, out| {
            out.push(pixel[0]);
        },
    )?;
    Some(output)
}

fn encode_rgb24(width: u32, height: u32, pixels: &[u8], header_size: u32) -> Option<Vec<u8>> {
    let stride = row_size(24, width)?;
    let pixel_bytes = stride.checked_mul(usize::try_from(height).ok()?)?;
    let pixel_offset = FILE_HEADER_SIZE.checked_add(usize::try_from(header_size).ok()?)?;
    let mut output = bmp_headers(width, height, 24, header_size, 0, pixel_offset, pixel_bytes)?;
    write_rows(
        &mut output,
        pixels,
        width,
        height,
        3,
        stride,
        |pixel, out| {
            out.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
        },
    )?;
    Some(output)
}

fn encode_rgb32(width: u32, height: u32, pixels: &[u8], header_size: u32) -> Option<Vec<u8>> {
    let stride = row_size(32, width)?;
    let pixel_bytes = stride.checked_mul(usize::try_from(height).ok()?)?;
    let pixel_offset = FILE_HEADER_SIZE.checked_add(usize::try_from(header_size).ok()?)?;
    let mut output = bmp_headers(width, height, 32, header_size, 0, pixel_offset, pixel_bytes)?;
    write_rows(
        &mut output,
        pixels,
        width,
        height,
        4,
        stride,
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
    header_size: u32,
    colors: u32,
    pixel_offset: usize,
    pixel_bytes: usize,
) -> Option<Vec<u8>> {
    let file_size = pixel_offset.checked_add(pixel_bytes)?;
    let signed_width = i32::try_from(width).ok()?;
    let signed_height = i32::try_from(height).ok()?;
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
    output.extend_from_slice(&BI_RGB.to_le_bytes());
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
    mut write_pixel: impl FnMut(&[u8], &mut Vec<u8>),
) -> Option<()> {
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    let source_stride = width.checked_mul(channels)?;
    let encoded_row = width.checked_mul(channels)?;
    for output_row in 0..height {
        let source_row = height.checked_sub(output_row)?.checked_sub(1)?;
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

fn rgba_to_rgb(pixels: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(pixels.len() / 4 * 3);
    for pixel in pixels.chunks_exact(4) {
        output.extend_from_slice(&pixel[..3]);
    }
    output
}
