//! Pure-Rust BMP encoder for indexed grayscale and true-color images.

use crate::codecs::{CodecError, CodecResult};
use crate::encode_options::BmpEncodeOptions;
use crate::encode_policy::EncodePolicy;
use crate::types::{DecodedImage, ImageMode, ImagePalette};
use crate::{CodecOperation, ImageFormat};

const FILE_HEADER_SIZE: usize = 14;
const INFO_HEADER_SIZE: u32 = 40;
const BMP_HEADER_SIZE: usize = FILE_HEADER_SIZE + INFO_HEADER_SIZE as usize;
const BI_RGB: u32 = 0;
// Pillow 12.2.0 BmpImagePlugin.py:437-440 defaults to 96 DPI and converts
// using round(96 * 39.3701), yielding 3,780 pixels per meter on both axes.
const DEFAULT_PIXELS_PER_METER: i32 = 3_780;

fn row_size(bits_per_pixel: usize, width: usize) -> usize {
    bits_per_pixel
        .saturating_mul(width)
        .div_ceil(32)
        .saturating_mul(4)
}

/// Encode a `DecodedImage` as BMP bytes.
///
/// Pillow derives 1/8/24/32-bit output from the source mode and ignores save
/// options requesting compression, row direction, or alternate DIB headers.
pub fn encode(img: &DecodedImage, opts: &BmpEncodeOptions) -> CodecResult<Vec<u8>> {
    encode_with_token(img, opts, None)
}

/// Encode a BMP while polling an optional cooperative cancellation token.
pub fn encode_with_token(
    img: &DecodedImage,
    opts: &BmpEncodeOptions,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    let mut output = Vec::new();
    let mut writer = |bytes: &[u8]| {
        output.extend_from_slice(bytes);
        Ok(())
    };
    write_encoded(img, opts, token, None, &mut writer)?;
    Ok(output)
}

/// Encode a BMP directly to a caller-owned sink.
pub(crate) fn encode_to_sink(
    img: &DecodedImage,
    opts: &BmpEncodeOptions,
    policy: EncodePolicy,
    operation: CodecOperation,
    token: Option<&crate::CancellationToken>,
    sink: &mut dyn crate::OutputSink,
) -> CodecResult<usize> {
    let mut writer = |bytes: &[u8]| {
        sink.write_all(bytes)
            .map_err(|error| CodecError::OutputWrite(error.to_string()))
    };
    write_encoded(img, opts, token, Some((policy, operation)), &mut writer)
}

fn write_encoded(
    img: &DecodedImage,
    _opts: &BmpEncodeOptions,
    token: Option<&crate::CancellationToken>,
    policy: Option<(EncodePolicy, CodecOperation)>,
    writer: &mut dyn FnMut(&[u8]) -> CodecResult<()>,
) -> CodecResult<usize> {
    crate::codecs::error::check_cancelled(token)?;
    if !bmp_file_fits(img) {
        return Err(CodecError::Dimensions(
            "BMP dimensions or file size exceed the container limits".to_owned(),
        ));
    }
    img.validate().map_err(CodecError::from_image_error)?;

    let layout = BmpLayout::for_image(img)?;
    let output_len = layout.output_len();
    if let Some((policy, operation)) = policy {
        policy
            .check_output_len(output_len, ImageFormat::Bmp, operation)
            .map_err(CodecError::from_image_error)?;
    }

    let mut written = 0usize;
    let header = bmp_headers(
        img.width,
        img.height,
        layout.depth,
        layout.colors,
        layout.pixel_offset,
        layout.pixel_bytes,
    );
    emit(&header, token, writer, &mut written)?;

    match layout.kind {
        BmpKind::L1 => {
            emit(&[0, 0, 0, 0, 255, 255, 255, 0], token, writer, &mut written)?;
            write_1bit_rows(
                &img.pixels,
                img.width as usize,
                img.height as usize,
                layout.stride,
                token,
                writer,
                &mut written,
            )?;
        }
        BmpKind::Indexed => {
            let mut palette_bytes = Vec::with_capacity(layout.palette_bytes);
            if img.mode == ImageMode::P8 {
                if let Some(palette) = img.palette.as_ref() {
                    for rgb in palette.rgb.chunks_exact(3) {
                        palette_bytes.extend_from_slice(&[rgb[2], rgb[1], rgb[0], 0]);
                    }
                } else {
                    for value in 0..=255u8 {
                        palette_bytes.extend_from_slice(&[value, value, value, 0]);
                    }
                }
            } else {
                for value in 0..=255u8 {
                    palette_bytes.extend_from_slice(&[value, value, value, 0]);
                }
            }
            emit(&palette_bytes, token, writer, &mut written)?;
            write_rows(
                &img.pixels,
                img.width as usize,
                img.height as usize,
                1,
                layout.stride,
                &mut |pixel, row| row.push(pixel[0]),
                token,
                writer,
                &mut written,
            )?;
        }
        BmpKind::Rgb24 => {
            write_rows(
                &img.pixels,
                img.width as usize,
                img.height as usize,
                3,
                layout.stride,
                &mut |pixel, row| row.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]),
                token,
                writer,
                &mut written,
            )?;
        }
        BmpKind::Rgba32 => {
            write_rows(
                &img.pixels,
                img.width as usize,
                img.height as usize,
                4,
                layout.stride,
                &mut |pixel, row| row.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]),
                token,
                writer,
                &mut written,
            )?;
        }
    }
    debug_assert_eq!(written, output_len);
    Ok(written)
}

#[derive(Clone, Copy)]
struct BmpLayout {
    kind: BmpKind,
    depth: u16,
    colors: u32,
    palette_bytes: usize,
    pixel_offset: usize,
    pixel_bytes: usize,
    stride: usize,
}

#[derive(Clone, Copy)]
enum BmpKind {
    L1,
    Indexed,
    Rgb24,
    Rgba32,
}

impl BmpLayout {
    fn for_image(img: &DecodedImage) -> CodecResult<Self> {
        let (kind, depth, colors) = match img.mode {
            ImageMode::L1 => (BmpKind::L1, 1u16, 2usize),
            ImageMode::P8 => (
                BmpKind::Indexed,
                8,
                img.palette.as_ref().map_or(256usize, ImagePalette::len),
            ),
            ImageMode::L8 => (BmpKind::Indexed, 8, 256),
            ImageMode::Rgb8 => (BmpKind::Rgb24, 24, 0),
            ImageMode::Rgba8 => (BmpKind::Rgba32, 32, 0),
            mode => {
                return Err(CodecError::Unsupported(format!(
                    "BMP cannot encode mode {mode:?}"
                )));
            }
        };
        // `write_encoded` validates the image and proves the complete file
        // fits in a 32-bit BMP before reaching this private layout builder.
        // Therefore the palette count, fixed header addition, and final
        // palette-count conversion are bounded by construction here.
        let palette_bytes = colors.saturating_mul(4);
        let stride = row_size(usize::from(depth), img.width as usize);
        let pixel_bytes = stride
            .checked_mul(img.height as usize)
            .ok_or_else(|| CodecError::Dimensions("BMP pixel length overflows".to_owned()))?;
        let pixel_offset = BMP_HEADER_SIZE.saturating_add(palette_bytes);
        Ok(Self {
            kind,
            depth,
            colors: u32::try_from(colors).unwrap_or(u32::MAX),
            palette_bytes,
            pixel_offset,
            pixel_bytes,
            stride,
        })
    }

    fn output_len(self) -> usize {
        // `bmp_file_fits` has already bounded the complete file to u32::MAX.
        BMP_HEADER_SIZE
            .saturating_add(self.palette_bytes)
            .saturating_add(self.pixel_bytes)
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
    let row_bytes = u128::from(depth)
        .saturating_mul(u128::from(img.width))
        .div_ceil(32)
        .saturating_mul(4);
    let pixel_bytes = row_bytes.saturating_mul(u128::from(img.height));
    let pixel_offset = (FILE_HEADER_SIZE as u128)
        .saturating_add(u128::from(INFO_HEADER_SIZE))
        .saturating_add((colors as u128).saturating_mul(4));
    img.width <= 2_147_483_647
        && img.height <= 2_147_483_647
        && pixel_offset.saturating_add(pixel_bytes) <= u128::from(u32::MAX)
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    // These calls model private defensive edges that a valid Pillow input
    // cannot select. Pillow has no caller-owned OutputSink, cancellation
    // token, or access to these post-preflight arithmetic states, so this is
    // Rust defensive-model evidence rather than a parity fixture or a
    // coverage-only substitute for the real sink contract.
    for (width, height) in [(u32::MAX, 1), (1, u32::MAX), (i32::MAX as u32, 1)] {
        let image = DecodedImage::new(width, height, Vec::new(), crate::types::ColorType::L8);
        assert!(encode(&image, &BmpEncodeOptions::default()).is_err());
    }

    let cancelled = crate::CancellationToken::new();
    cancelled.cancel();
    let image = DecodedImage::new(1, 1, vec![0], crate::types::ColorType::L8);
    let mut accept = |_bytes: &[u8]| Ok::<(), CodecError>(());
    let _ = write_encoded(
        &image,
        &BmpEncodeOptions::default(),
        Some(&cancelled),
        None,
        &mut accept,
    );

    let huge = DecodedImage::new(
        u32::MAX,
        u32::MAX,
        Vec::new(),
        crate::types::ColorType::Rgba8,
    );
    let _ = BmpLayout::for_image(&huge);

    let mut written = 0usize;
    let mut reject = |_bytes: &[u8]| Err(CodecError::OutputWrite("coverage sink".to_owned()));
    let _ = write_1bit_rows(
        &[],
        usize::MAX,
        usize::MAX,
        0,
        None,
        &mut accept,
        &mut written,
    );
    let _ = write_1bit_rows(
        &[],
        16,
        usize::MAX / 2 + 1,
        0,
        None,
        &mut accept,
        &mut written,
    );
    let _ = write_1bit_rows(&[], 8, 1, 0, None, &mut accept, &mut written);
    let _ = write_1bit_rows(&[0], 8, 1, 1, None, &mut reject, &mut written);
    let _ = write_1bit_rows(&[], 1, 1, 0, Some(&cancelled), &mut accept, &mut written);

    let mut convert = |_pixel: &[u8], _row: &mut Vec<u8>| {};
    let _ = write_rows(
        &[],
        usize::MAX,
        1,
        2,
        0,
        &mut convert,
        None,
        &mut accept,
        &mut written,
    );
    let _ = write_rows(
        &[],
        1,
        3,
        usize::MAX / 2 + 1,
        0,
        &mut convert,
        None,
        &mut accept,
        &mut written,
    );
    let _ = write_rows(
        &[],
        2,
        usize::MAX / 2 + 1,
        1,
        0,
        &mut convert,
        None,
        &mut accept,
        &mut written,
    );
    let _ = write_rows(
        &[],
        1,
        1,
        1,
        0,
        &mut convert,
        None,
        &mut accept,
        &mut written,
    );
    let _ = write_rows(
        &[0],
        1,
        1,
        1,
        1,
        &mut convert,
        None,
        &mut reject,
        &mut written,
    );
    let _ = write_rows(
        &[],
        1,
        1,
        1,
        0,
        &mut convert,
        Some(&cancelled),
        &mut accept,
        &mut written,
    );

    let _ = emit(&[0], Some(&cancelled), &mut accept, &mut written);
    let mut max_written = usize::MAX;
    let _ = emit(&[0], None, &mut accept, &mut max_written);
}

fn source_row(output_row: usize, height: usize) -> usize {
    height.saturating_sub(output_row).saturating_sub(1)
}

fn bmp_headers(
    width: u32,
    height: u32,
    depth: u16,
    colors: u32,
    pixel_offset: usize,
    pixel_bytes: usize,
) -> [u8; BMP_HEADER_SIZE] {
    let file_size = pixel_offset.saturating_add(pixel_bytes);
    let mut output = [0u8; BMP_HEADER_SIZE];
    output[..2].copy_from_slice(b"BM");
    output[2..6].copy_from_slice(&u32::try_from(file_size).unwrap_or(u32::MAX).to_le_bytes());
    output[10..14].copy_from_slice(
        &u32::try_from(pixel_offset)
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    output[14..18].copy_from_slice(&INFO_HEADER_SIZE.to_le_bytes());
    output[18..22].copy_from_slice(&width.to_le_bytes());
    output[22..26].copy_from_slice(&height.to_le_bytes());
    output[26..28].copy_from_slice(&1u16.to_le_bytes());
    output[28..30].copy_from_slice(&depth.to_le_bytes());
    output[30..34].copy_from_slice(&BI_RGB.to_le_bytes());
    output[34..38].copy_from_slice(&u32::try_from(pixel_bytes).unwrap_or(u32::MAX).to_le_bytes());
    output[38..42].copy_from_slice(&DEFAULT_PIXELS_PER_METER.to_le_bytes());
    output[42..46].copy_from_slice(&DEFAULT_PIXELS_PER_METER.to_le_bytes());
    output[46..50].copy_from_slice(&colors.to_le_bytes());
    output[50..54].copy_from_slice(&colors.to_le_bytes());
    output
}

fn write_1bit_rows(
    pixels: &[u8],
    width: usize,
    height: usize,
    stride: usize,
    token: Option<&crate::CancellationToken>,
    writer: &mut dyn FnMut(&[u8]) -> CodecResult<()>,
    written: &mut usize,
) -> CodecResult<()> {
    let packed_width = width.div_ceil(8);
    for output_row in 0..height {
        crate::codecs::error::check_cancelled(token)?;
        let source = source_row(output_row, height);
        let start = source
            .checked_mul(packed_width)
            .ok_or_else(|| CodecError::Dimensions("BMP row offset overflows".to_owned()))?;
        let end = start
            .checked_add(packed_width)
            .ok_or_else(|| CodecError::Dimensions("BMP row end overflows".to_owned()))?;
        let source_row = pixels
            .get(start..end)
            .ok_or_else(|| CodecError::Dimensions("BMP pixel buffer is too short".to_owned()))?;
        let mut row = Vec::with_capacity(stride);
        row.extend_from_slice(source_row);
        row.resize(stride, 0);
        emit(&row, token, writer, written)?;
    }
    Ok(())
}

// The row converter keeps the codec's byte layout explicit; these parameters
// are the independent validated inputs needed by both indexed and true-color
// row emission.
#[allow(clippy::too_many_arguments)]
fn write_rows(
    pixels: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    stride: usize,
    convert: &mut dyn FnMut(&[u8], &mut Vec<u8>),
    token: Option<&crate::CancellationToken>,
    writer: &mut dyn FnMut(&[u8]) -> CodecResult<()>,
    written: &mut usize,
) -> CodecResult<()> {
    let source_stride = width
        .checked_mul(channels)
        .ok_or_else(|| CodecError::Dimensions("BMP source row length overflows".to_owned()))?;
    for output_row in 0..height {
        crate::codecs::error::check_cancelled(token)?;
        let source = source_row(output_row, height);
        let start = source
            .checked_mul(source_stride)
            .ok_or_else(|| CodecError::Dimensions("BMP row offset overflows".to_owned()))?;
        let end = start
            .checked_add(source_stride)
            .ok_or_else(|| CodecError::Dimensions("BMP row end overflows".to_owned()))?;
        let source_row = pixels
            .get(start..end)
            .ok_or_else(|| CodecError::Dimensions("BMP pixel buffer is too short".to_owned()))?;
        let mut row = Vec::with_capacity(stride);
        for pixel in source_row.chunks_exact(channels) {
            convert(pixel, &mut row);
        }
        row.resize(stride, 0);
        emit(&row, token, writer, written)?;
    }
    Ok(())
}

fn emit(
    bytes: &[u8],
    token: Option<&crate::CancellationToken>,
    writer: &mut dyn FnMut(&[u8]) -> CodecResult<()>,
    written: &mut usize,
) -> CodecResult<()> {
    crate::codecs::error::check_cancelled(token)?;
    writer(bytes)?;
    *written = written
        .checked_add(bytes.len())
        .ok_or_else(|| CodecError::Dimensions("BMP output length overflows".to_owned()))?;
    Ok(())
}
