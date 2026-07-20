//! Pure-Rust BMP encoder for indexed grayscale and true-color images.

use crate::encode_options::EncodeOptions;
use crate::types::{DecodedImage, ImageMode, ImagePalette};

const FILE_HEADER_SIZE: usize = 14;
const INFO_HEADER_SIZE: u32 = 40;
const BI_RGB: u32 = 0;
// Pillow 12.2.0 BmpImagePlugin.py:437-440 defaults to 96 DPI and converts
// using round(96 * 39.3701), yielding 3,780 pixels per meter on both axes.
const DEFAULT_PIXELS_PER_METER: i32 = 3_780;

fn row_size(bits_per_pixel: usize, width: usize) -> usize {
    (bits_per_pixel * width).div_ceil(32) * 4
}

/// Encode a `DecodedImage` as BMP bytes.
///
/// Pillow derives 1/8/24/32-bit output from the source mode and ignores save
/// options requesting compression, row direction, or alternate DIB headers.
pub fn encode(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    let _ = opts;
    if !bmp_file_fits(img) {
        return None;
    }
    img.validate().ok()?;
    match img.mode {
        ImageMode::L1 => Some(encode_1bit(img.width, img.height, &img.pixels)),
        ImageMode::P8 | ImageMode::L8 => Some(encode_l8(
            img.width,
            img.height,
            &img.pixels,
            (img.mode == ImageMode::P8)
                .then_some(img.palette.as_ref())
                .flatten(),
        )),
        ImageMode::Rgb8 => Some(encode_rgb24(img.width, img.height, &img.pixels)),
        ImageMode::Rgba8 => Some(encode_rgb32(img.width, img.height, &img.pixels)),
        _ => None,
    }
}

fn bmp_file_fits(img: &DecodedImage) -> bool {
    // A classic BMP stores signed dimensions and unsigned 32-bit file offsets.
    // Once this bound and DecodedImage::validate both hold, the private writers
    // below can use direct arithmetic and slicing without duplicating fallible
    // checks at every row and header field.
    let (depth, colors) = match img.mode {
        ImageMode::L1 => (1u16, 2usize),
        ImageMode::P8 => (8, img.palette.as_ref().map_or(256, ImagePalette::len)),
        ImageMode::L8 => (8, 256),
        ImageMode::Rgb8 => (24, 0),
        ImageMode::Rgba8 => (32, 0),
        _ => return true,
    };
    let row_bytes = (u128::from(depth) * u128::from(img.width)).div_ceil(32) * 4;
    let pixel_bytes = row_bytes * u128::from(img.height);
    let pixel_offset =
        FILE_HEADER_SIZE as u128 + u128::from(INFO_HEADER_SIZE) + (colors as u128) * 4;
    img.width <= 2_147_483_647
        && img.height <= 2_147_483_647
        && pixel_offset + pixel_bytes <= u128::from(u32::MAX)
}

fn encode_1bit(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    let encoded_width = width;
    let encoded_height = height;
    let width = width as usize;
    let height = height as usize;
    let packed_width = width.div_ceil(8);
    let stride = row_size(1, width);
    let pixel_bytes = stride * height;
    let palette_bytes = 8usize;
    let pixel_offset = FILE_HEADER_SIZE + INFO_HEADER_SIZE as usize + palette_bytes;
    let mut output = bmp_headers(
        encoded_width,
        encoded_height,
        1,
        2,
        pixel_offset,
        pixel_bytes,
    );
    output.extend_from_slice(&[0, 0, 0, 0, 255, 255, 255, 0]);
    for output_row in 0..height {
        let source_row = source_row(output_row, height);
        let row_start = source_row * packed_width;
        output.extend_from_slice(&pixels[row_start..row_start + packed_width]);
        output.resize(output.len() + stride - packed_width, 0);
    }
    output
}

fn source_row(output_row: usize, height: usize) -> usize {
    height - output_row - 1
}

fn encode_l8(width: u32, height: u32, pixels: &[u8], palette: Option<&ImagePalette>) -> Vec<u8> {
    let width_usize = width as usize;
    let height_usize = height as usize;
    let stride = row_size(8, width_usize);
    // Pillow 12.2.0 BmpImagePlugin.py:446-452 retains the exact P-mode
    // palette length and writes it as BGRX. Ordinary L mode gets 256 gray
    // entries instead.
    let color_count = palette.map_or(256, ImagePalette::len);
    let palette_bytes = color_count * 4;
    let pixel_bytes = stride * height_usize;
    let pixel_offset = FILE_HEADER_SIZE + INFO_HEADER_SIZE as usize + palette_bytes;
    let mut output = bmp_headers(
        width,
        height,
        8,
        color_count as u32,
        pixel_offset,
        pixel_bytes,
    );
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
        width_usize,
        height_usize,
        1,
        stride,
        |pixel, out| {
            out.push(pixel[0]);
        },
    );
    output
}

fn encode_rgb24(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    let width_usize = width as usize;
    let height_usize = height as usize;
    let stride = row_size(24, width_usize);
    let pixel_bytes = stride * height_usize;
    let pixel_offset = FILE_HEADER_SIZE + INFO_HEADER_SIZE as usize;
    let mut output = bmp_headers(width, height, 24, 0, pixel_offset, pixel_bytes);
    write_rows(
        &mut output,
        pixels,
        width_usize,
        height_usize,
        3,
        stride,
        |pixel, out| {
            out.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
        },
    );
    output
}

fn encode_rgb32(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    let width_usize = width as usize;
    let height_usize = height as usize;
    let stride = row_size(32, width_usize);
    let pixel_bytes = stride * height_usize;
    let pixel_offset = FILE_HEADER_SIZE + INFO_HEADER_SIZE as usize;
    let mut output = bmp_headers(width, height, 32, 0, pixel_offset, pixel_bytes);
    write_rows(
        &mut output,
        pixels,
        width_usize,
        height_usize,
        4,
        stride,
        |pixel, out| {
            out.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        },
    );
    output
}

#[allow(clippy::too_many_arguments)]
fn bmp_headers(
    width: u32,
    height: u32,
    depth: u16,
    colors: u32,
    pixel_offset: usize,
    pixel_bytes: usize,
) -> Vec<u8> {
    let file_size = pixel_offset + pixel_bytes;
    let mut output = Vec::with_capacity(file_size);
    output.extend_from_slice(b"BM");
    output.extend_from_slice(&(file_size as u64).to_le_bytes()[..4]);
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&(pixel_offset as u64).to_le_bytes()[..4]);
    output.extend_from_slice(&INFO_HEADER_SIZE.to_le_bytes());
    output.extend_from_slice(&width.to_le_bytes());
    output.extend_from_slice(&height.to_le_bytes());
    output.extend_from_slice(&1u16.to_le_bytes());
    output.extend_from_slice(&depth.to_le_bytes());
    output.extend_from_slice(&BI_RGB.to_le_bytes());
    output.extend_from_slice(&(pixel_bytes as u64).to_le_bytes()[..4]);
    output.extend_from_slice(&DEFAULT_PIXELS_PER_METER.to_le_bytes());
    output.extend_from_slice(&DEFAULT_PIXELS_PER_METER.to_le_bytes());
    output.extend_from_slice(&colors.to_le_bytes());
    output.extend_from_slice(&colors.to_le_bytes());
    output.resize(FILE_HEADER_SIZE + INFO_HEADER_SIZE as usize, 0);
    output
}

#[allow(clippy::too_many_arguments)]
fn write_rows(
    output: &mut Vec<u8>,
    pixels: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    stride: usize,
    mut write_pixel: impl FnMut(&[u8], &mut Vec<u8>),
) {
    let source_stride = width * channels;
    let encoded_row = width * channels;
    for output_row in 0..height {
        let source_row = source_row(output_row, height);
        let start = source_row * source_stride;
        let row = &pixels[start..start + source_stride];
        for pixel in row.chunks_exact(channels) {
            write_pixel(pixel, output);
        }
        output.resize(output.len() + stride - encoded_row, 0);
    }
}
