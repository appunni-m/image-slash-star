//! PNG encoder using the internal zlib/DEFLATE implementation.

use crate::codecs::compression::deflate::compress_zlib_png_repeated;
use crate::codecs::{CodecError, CodecResult};
use crate::encode_options::{PngCompression, PngEncodeOptions};
use crate::encode_policy::EncodePolicy;
use crate::types::{ColorType, DecodedImage, ImageMode};
use crate::{CodecOperation, ImageFormat};
use std::borrow::Cow;

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
// Keep long adaptive-filter rows interruptible without charging every byte.
// The interval is part of the Rust-only work-budget contract; it does not
// affect the no-token Pillow-parity path.
const FILTER_CHECKPOINT_BYTES: usize = 1_024;
/// Encode an 8-bit grayscale, grayscale-alpha, RGB, or RGBA image as PNG.
///
/// Pillow ignores PNG interlace save options, so this encoder also always
/// emits non-interlaced rows. Compression levels select the corresponding
/// strategy in the internal zlib/DEFLATE implementation.
pub fn encode(img: &DecodedImage, opts: &PngEncodeOptions) -> CodecResult<Vec<u8>> {
    encode_with_token(img, opts, None)
}

/// Encode a PNG while polling an optional cooperative cancellation token.
pub fn encode_with_token(
    img: &DecodedImage,
    opts: &PngEncodeOptions,
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

/// Encode a PNG directly to a caller-owned sink.
pub(crate) fn encode_to_sink(
    img: &DecodedImage,
    opts: &PngEncodeOptions,
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
    opts: &PngEncodeOptions,
    token: Option<&crate::CancellationToken>,
    policy: Option<(EncodePolicy, CodecOperation)>,
    writer: &mut dyn FnMut(&[u8]) -> CodecResult<()>,
) -> CodecResult<usize> {
    crate::codecs::error::check_cancelled(token)?;
    img.validate().map_err(CodecError::from_image_error)?;

    let width = bounded_usize(img.width);
    let height = bounded_usize(img.height);
    let (png_color, depth, row_bytes, filter_bytes, pixels): (u8, u8, usize, usize, Cow<'_, [u8]>) =
        match img.mode {
            ImageMode::L1 => {
                let row_bytes = width.div_ceil(8);
                (0, 1, row_bytes, 1, Cow::Borrowed(&img.pixels))
            }
            ImageMode::P8 => (3, 8, width, 1, Cow::Borrowed(&img.pixels)),
            ImageMode::L16 => {
                let mut big_endian = Vec::with_capacity(img.pixels.len());
                for sample in img.pixels.as_chunks::<2>().0 {
                    crate::codecs::error::check_cancelled(token)?;
                    big_endian.extend_from_slice(
                        &u16::from_le_bytes([sample[0], sample[1]]).to_be_bytes(),
                    );
                }
                (0, 16, width.saturating_mul(2), 2, Cow::Owned(big_endian))
            }
            _ => {
                let (png_color, channels) = match img.color {
                    ColorType::L8 => (0, 1usize),
                    ColorType::La8 => (4, 2),
                    ColorType::Rgb8 => (2, 3),
                    ColorType::Rgba8 => (6, 4),
                    _ => {
                        return Err(CodecError::Unsupported(
                            "PNG cannot encode this image mode".to_owned(),
                        ));
                    }
                };
                (
                    png_color,
                    8,
                    width.saturating_mul(channels),
                    channels,
                    Cow::Borrowed(&img.pixels),
                )
            }
        };

    let filter = if img.mode == ImageMode::P8 {
        Filter::None
    } else {
        Filter::Adaptive
    };
    let optimize = opts.optimize.unwrap_or(false);
    let filtered = plain_rows(
        &pixels,
        row_bytes,
        height,
        filter_bytes,
        filter,
        optimize,
        token,
    )?;
    let compression_level = if optimize {
        9
    } else {
        match opts.compression.unwrap_or(PngCompression::Default) {
            PngCompression::None => 0,
            PngCompression::Default => 6,
            PngCompression::Maximum => 9,
            PngCompression::Level(level) => level,
        }
    };
    crate::codecs::error::check_cancelled(token)?;
    let compressed = compress_zlib_png_repeated(
        &filtered,
        row_bytes.saturating_add(1),
        height,
        compression_level,
        token,
    )?;
    crate::codecs::error::check_cancelled(token)?;

    let mut header = [0u8; 13];
    header[..4].copy_from_slice(&img.width.to_be_bytes());
    header[4..8].copy_from_slice(&img.height.to_be_bytes());
    header[8..].copy_from_slice(&[depth, png_color, 0, 0, 0]);

    let output_len = png_output_len(img, opts, header.len(), compressed.len())?;
    if let Some((policy, operation)) = policy {
        policy
            .check_output_len(output_len, ImageFormat::Png, operation)
            .map_err(CodecError::from_image_error)?;
    }

    let mut written = 0usize;
    emit(PNG_SIGNATURE, token, writer, &mut written)?;
    write_chunk(*b"IHDR", &header, token, writer, &mut written)?;
    if img.mode == ImageMode::P8 {
        if let Some(palette) = img.palette.as_ref() {
            write_chunk(*b"PLTE", &palette.rgb, token, writer, &mut written)?;
            if !palette.alpha.is_empty() {
                write_chunk(*b"tRNS", &palette.alpha, token, writer, &mut written)?;
            }
        } else {
            // Pillow saves a palette-less P image with its implicit all-black
            // 256-entry palette.
            write_chunk(*b"PLTE", &[0; 256 * 3], token, writer, &mut written)?;
        }
    }
    write_requested_ancillary_chunks(opts, token, writer, &mut written)?;
    write_idat_chunks(&compressed, token, writer, &mut written)?;
    write_chunk(*b"IEND", &[], token, writer, &mut written)?;
    debug_assert_eq!(written, output_len);
    Ok(written)
}

fn plain_rows(
    pixels: &[u8],
    stride: usize,
    height: usize,
    filter_bytes: usize,
    filter: Filter,
    optimize: bool,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    let row_len = stride.saturating_add(1);
    let mut output = Vec::with_capacity(row_len.saturating_mul(height));
    let mut previous = None;
    for row in pixels.chunks_exact(stride) {
        crate::codecs::error::check_cancelled(token)?;
        append_filtered_row(
            &mut output,
            row,
            previous,
            filter_bytes,
            filter,
            optimize,
            token,
        )?;
        previous = Some(row);
    }
    Ok(output)
}

#[derive(Clone, Copy)]
enum Filter {
    None,
    Sub,
    Up,
    Average,
    Paeth,
    Adaptive,
}

fn append_filtered_row(
    output: &mut Vec<u8>,
    row: &[u8],
    previous: Option<&[u8]>,
    bytes_per_pixel: usize,
    requested: Filter,
    optimize: bool,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    let selected = if matches!(requested, Filter::Adaptive) {
        select_adaptive_filter(row, previous, bytes_per_pixel, optimize, token)?
    } else {
        requested
    };
    output.push(filter_byte(selected));
    for (index, &value) in row.iter().enumerate() {
        if index != 0 && index.is_multiple_of(FILTER_CHECKPOINT_BYTES) {
            crate::codecs::error::check_cancelled(token)?;
        }
        let left = index
            .checked_sub(bytes_per_pixel)
            .map_or(0, |position| row[position]);
        let above = previous.map_or(0, |prior| prior[index]);
        let upper_left = previous.map_or(0, |prior| {
            index
                .checked_sub(bytes_per_pixel)
                .map_or(0, |position| prior[position])
        });
        let prediction = match selected {
            Filter::None | Filter::Adaptive => 0,
            Filter::Sub => left,
            Filter::Up => above,
            Filter::Average => u16::from(left)
                .saturating_add(u16::from(above))
                .div_euclid(2)
                .to_le_bytes()[0],
            Filter::Paeth => paeth(left, above, upper_left),
        };
        output.push(value.wrapping_sub(prediction));
    }
    Ok(())
}

fn select_adaptive_filter(
    row: &[u8],
    previous: Option<&[u8]>,
    bytes_per_pixel: usize,
    optimize: bool,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Filter> {
    // Pillow's ZipEncode.c starts with None, then replaces it only on a
    // strictly lower score in this order. Average is deliberately excluded
    // unless optimize=True.
    let mut selected = Filter::None;
    let mut score = filter_score(row, previous, bytes_per_pixel, selected, token)?;
    for candidate in [Filter::Up, Filter::Sub]
        .into_iter()
        .chain(optimize.then_some(Filter::Average))
        .chain([Filter::Paeth])
    {
        let candidate_score = filter_score(row, previous, bytes_per_pixel, candidate, token)?;
        if candidate_score < score {
            selected = candidate;
            score = candidate_score;
        }
    }
    Ok(selected)
}

fn filter_score(
    row: &[u8],
    previous: Option<&[u8]>,
    bytes_per_pixel: usize,
    filter: Filter,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<u64> {
    let mut score = 0u64;
    for (index, &value) in row.iter().enumerate() {
        if index != 0 && index.is_multiple_of(FILTER_CHECKPOINT_BYTES) {
            crate::codecs::error::check_cancelled(token)?;
        }
        let left = index
            .checked_sub(bytes_per_pixel)
            .map_or(0, |position| row[position]);
        let above = previous.map_or(0, |prior| prior[index]);
        let upper_left = previous.map_or(0, |prior| {
            index
                .checked_sub(bytes_per_pixel)
                .map_or(0, |position| prior[position])
        });
        let prediction = match filter {
            Filter::None | Filter::Adaptive => 0,
            Filter::Sub => left,
            Filter::Up => above,
            Filter::Average => u16::from(left)
                .saturating_add(u16::from(above))
                .div_euclid(2)
                .to_le_bytes()[0],
            Filter::Paeth => paeth(left, above, upper_left),
        };
        score = score.saturating_add(u64::from(
            value.wrapping_sub(prediction).cast_signed().unsigned_abs(),
        ));
    }
    Ok(score)
}

fn filter_byte(filter: Filter) -> u8 {
    match filter {
        Filter::None | Filter::Adaptive => 0,
        Filter::Sub => 1,
        Filter::Up => 2,
        Filter::Average => 3,
        Filter::Paeth => 4,
    }
}

fn paeth(left: u8, above: u8, upper_left: u8) -> u8 {
    let left = i32::from(left);
    let above = i32::from(above);
    let upper_left = i32::from(upper_left);
    let estimate = left.saturating_add(above).saturating_sub(upper_left);
    let left_distance = estimate.saturating_sub(left).saturating_abs();
    let above_distance = estimate.saturating_sub(above).saturating_abs();
    let diagonal_distance = estimate.saturating_sub(upper_left).saturating_abs();
    if left_distance <= above_distance && left_distance <= diagonal_distance {
        left.to_le_bytes()[0]
    } else if above_distance <= diagonal_distance {
        above.to_le_bytes()[0]
    } else {
        upper_left.to_le_bytes()[0]
    }
}

fn write_requested_ancillary_chunks(
    opts: &PngEncodeOptions,
    token: Option<&crate::CancellationToken>,
    writer: &mut dyn FnMut(&[u8]) -> CodecResult<()>,
    written: &mut usize,
) -> CodecResult<()> {
    if opts.legacy_gamma() {
        write_chunk(*b"gAMA", &45_455u32.to_be_bytes(), token, writer, written)?;
    }
    if opts.legacy_srgb() {
        write_chunk(*b"sRGB", &[0], token, writer, written)?;
    }
    if opts.legacy_physical() {
        let mut payload = [0u8; 9];
        payload[..4].copy_from_slice(&2_835u32.to_be_bytes());
        payload[4..8].copy_from_slice(&2_835u32.to_be_bytes());
        payload[8] = 1;
        write_chunk(*b"pHYs", &payload, token, writer, written)?;
    }
    if opts.legacy_text_chunks() {
        write_chunk(*b"tEXt", b"Comment\0pillow-rs", token, writer, written)?;
    }
    if opts.legacy_time() {
        let payload = [0x07, 0xea, 7, 4, 0, 0, 0]; // 2026-07-04 00:00:00 UTC.
        write_chunk(*b"tIME", &payload, token, writer, written)?;
    }
    Ok(())
}

fn write_idat_chunks(
    payload: &[u8],
    token: Option<&crate::CancellationToken>,
    writer: &mut dyn FnMut(&[u8]) -> CodecResult<()>,
    written: &mut usize,
) -> CodecResult<()> {
    let max_chunk_len = bounded_usize(u32::MAX);

    for chunk in payload.chunks(max_chunk_len) {
        write_chunk(*b"IDAT", chunk, token, writer, written)?;
    }
    Ok(())
}

fn write_chunk(
    kind: [u8; 4],
    payload: &[u8],
    token: Option<&crate::CancellationToken>,
    writer: &mut dyn FnMut(&[u8]) -> CodecResult<()>,
    written: &mut usize,
) -> CodecResult<()> {
    let mut header = [0u8; 8];
    header[..4].copy_from_slice(&low_u32(payload.len()).to_be_bytes());
    header[4..].copy_from_slice(&kind);
    emit(&header, token, writer, written)?;
    emit(payload, token, writer, written)?;
    let crc = crc32(&kind, payload).to_be_bytes();
    emit(&crc, token, writer, written)
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
        .ok_or_else(|| CodecError::Dimensions("PNG output length overflows".to_owned()))?;
    Ok(())
}

fn png_output_len(
    img: &DecodedImage,
    opts: &PngEncodeOptions,
    header_len: usize,
    compressed_len: usize,
) -> CodecResult<usize> {
    let mut total = PNG_SIGNATURE.len();
    add_chunk_len(&mut total, header_len)?;
    if img.mode == ImageMode::P8 {
        let palette_len = img
            .palette
            .as_ref()
            .map_or(256usize.saturating_mul(3), |palette| palette.rgb.len());
        add_chunk_len(&mut total, palette_len)?;
        if let Some(palette) = img.palette.as_ref()
            && !palette.alpha.is_empty()
        {
            add_chunk_len(&mut total, palette.alpha.len())?;
        }
    }
    if opts.legacy_gamma() {
        add_chunk_len(&mut total, 4)?;
    }
    if opts.legacy_srgb() {
        add_chunk_len(&mut total, 1)?;
    }
    if opts.legacy_physical() {
        add_chunk_len(&mut total, 9)?;
    }
    if opts.legacy_text_chunks() {
        add_chunk_len(&mut total, b"Comment\0pillow-rs".len())?;
    }
    if opts.legacy_time() {
        add_chunk_len(&mut total, 7)?;
    }
    let max_chunk_len = bounded_usize(u32::MAX);
    let idat_chunks = compressed_len.div_ceil(max_chunk_len);
    let idat_bytes = compressed_len
        .checked_add(idat_chunks.saturating_mul(12))
        .ok_or_else(|| CodecError::Dimensions("PNG output length overflows".to_owned()))?;
    total = total
        .checked_add(idat_bytes)
        .ok_or_else(|| CodecError::Dimensions("PNG output length overflows".to_owned()))?;
    add_chunk_len(&mut total, 0)?;
    Ok(total)
}

fn add_chunk_len(total: &mut usize, payload_len: usize) -> CodecResult<()> {
    *total = total
        .checked_add(payload_len)
        .and_then(|value| value.checked_add(12))
        .ok_or_else(|| CodecError::Dimensions("PNG output length overflows".to_owned()))?;
    Ok(())
}

fn crc32(kind: &[u8; 4], data: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in kind.iter().chain(data) {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

fn bounded_usize(value: u32) -> usize {
    #[cfg(target_pointer_width = "64")]
    {
        let [a, b, c, d] = value.to_le_bytes();
        usize::from_le_bytes([a, b, c, d, 0, 0, 0, 0])
    }
    #[cfg(target_pointer_width = "32")]
    {
        usize::from_le_bytes(value.to_le_bytes())
    }
}

fn low_u32(value: usize) -> u32 {
    #[cfg(target_pointer_width = "64")]
    {
        let [a, b, c, d, ..] = value.to_le_bytes();
        u32::from_le_bytes([a, b, c, d])
    }
    #[cfg(target_pointer_width = "32")]
    {
        u32::from_le_bytes(value.to_le_bytes())
    }
}
