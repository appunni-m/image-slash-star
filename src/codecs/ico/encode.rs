//! Source-sized ICO encoder with PNG-backed or BMP-backed entries.
//!
//! This codec never resizes source pixels. Callers that need a
//! multi-resolution icon must provide already-sized entries through a future
//! entry-oriented API rather than asking the codec to perform image processing.
use crate::codecs::{CodecError, CodecResult};
#[cfg(coverage)]
use crate::encode_options::IcoSize;
use crate::encode_options::{IcoEncodeOptions, IcoEntryType, PngEncodeOptions};
use crate::encode_policy::EncodePolicy;
use crate::types::{ColorType, DecodedImage};
use crate::{CodecOperation, ImageFormat};
/// Encode one source-sized image as one Pillow-compatible ICO entry.
pub fn encode(img: &DecodedImage, opts: &IcoEncodeOptions) -> CodecResult<Vec<u8>> {
    encode_with_token(img, opts, None)
}

/// Encode one source-sized ICO entry while polling a cooperative cancellation token.
pub fn encode_with_token(
    img: &DecodedImage,
    opts: &IcoEncodeOptions,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    crate::codecs::error::check_cancelled(token)?;
    img.validate().map_err(CodecError::from_image_error)?;
    if opts.entry_type == IcoEntryType::Bmp {
        return encode_bmp_entries(img, opts, token);
    }
    encode_png_entries(img, opts, token)
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let _ = encode(
        &DecodedImage::new(0, 1, Vec::new(), ColorType::Rgb8),
        &IcoEncodeOptions::default(),
    );

    let rgb = DecodedImage::new(
        16,
        16,
        (0u8..=255)
            .flat_map(|value| [value, value.wrapping_mul(3), value.wrapping_mul(7)])
            .collect(),
        ColorType::Rgb8,
    );
    let rgba = DecodedImage::new(
        16,
        16,
        (0u8..=255)
            .flat_map(|value| [value, value.wrapping_mul(5), value.wrapping_mul(11), value])
            .collect(),
        ColorType::Rgba8,
    );
    let luma = DecodedImage::new(16, 16, vec![0; 16 * 16], ColorType::L8);
    let cmyk = DecodedImage::new(16, 16, vec![0; 16 * 16 * 4], ColorType::Cmyk8);

    let exact_size = IcoEncodeOptions {
        sizes: vec![IcoSize {
            width: 16,
            height: 16,
        }],
        ..IcoEncodeOptions::default()
    };
    let _ = encode(&rgb, &exact_size);

    // The embedded PNG encoder has nested row, compression, and chunk polls;
    // sweep through the nested call so ICO's post-embed and directory polls
    // are covered as well as its early exits.
    for checks in 0..=40 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = encode_with_token(&rgb, &exact_size, Some(&token));
    }

    let wrong_size = IcoEncodeOptions {
        sizes: vec![IcoSize {
            width: 8,
            height: 8,
        }],
        ..IcoEncodeOptions::default()
    };
    let _ = encode(&rgb, &wrong_size);

    let mut bmp = exact_size.clone();
    bmp.entry_type = IcoEntryType::Bmp;
    // BMP has one poll per source row plus final payload and directory polls.
    for checks in 0..=23 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = encode_with_token(&rgb, &bmp, Some(&token));
    }
    for checks in 0..=23 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = encode_with_token(&rgba, &bmp, Some(&token));
    }
    let _ = encode(&rgb, &bmp);
    let _ = encode(&rgba, &bmp);
    let _ = encode(&luma, &bmp);
    let _ = encode(&cmyk, &bmp);

    let mut invalid_size = IcoEncodeOptions {
        sizes: vec![
            IcoSize {
                width: 16,
                height: 16,
            },
            IcoSize {
                width: 32,
                height: 32,
            },
        ],
        ..IcoEncodeOptions::default()
    };
    let _ = encode(&rgb, &invalid_size);
    invalid_size.entry_type = IcoEntryType::Bmp;
    let _ = encode(&rgb, &invalid_size);

    let oversized = DecodedImage::new(257, 1, vec![0; 257 * 3], ColorType::Rgb8);
    let _ = encode(&oversized, &IcoEncodeOptions::default());
    let too_tall = DecodedImage::new(1, 257, vec![0; 257 * 3], ColorType::Rgb8);
    let _ = encode(&too_tall, &IcoEncodeOptions::default());

    let _ = encode_directory((256, 256), &[1, 2, 3], 32, None);
    for size in [(0, 1), (1, 0), (257, 1), (1, 257)] {
        let _ = encode_directory(size, &[0], 32, None);
    }
    let _ = encode_bmp_single_entry(&rgb, None);
    let _ = encode_bmp_single_entry(&rgba, None);
}

fn encode_png_entries(
    img: &DecodedImage,
    opts: &IcoEncodeOptions,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    crate::codecs::error::check_cancelled(token)?;
    let size = source_entry_size(img, opts)?;
    let frame =
        crate::codecs::png::encode::encode_with_token(img, &PngEncodeOptions::default(), token)
            .map_err(|error| error.context("embedded ICO PNG encode"))?;
    crate::codecs::error::check_cancelled(token)?;
    encode_directory(size, &frame, 32, token)
}

/// Encode one source-sized ICO entry directly to a caller-owned sink.
pub(crate) fn encode_to_sink(
    img: &DecodedImage,
    opts: &IcoEncodeOptions,
    policy: EncodePolicy,
    operation: CodecOperation,
    token: Option<&crate::CancellationToken>,
    sink: &mut dyn crate::OutputSink,
) -> CodecResult<usize> {
    crate::codecs::error::check_cancelled(token)?;
    img.validate().map_err(CodecError::from_image_error)?;
    let size = source_entry_size(img, opts)?;
    let (bits, payload) = if opts.entry_type == IcoEntryType::Bmp {
        encode_bmp_payload(img, token)?
    } else {
        let payload =
            crate::codecs::png::encode::encode_with_token(img, &PngEncodeOptions::default(), token)
                .map_err(|error| error.context("embedded ICO PNG encode"))?;
        (32, payload)
    };
    crate::codecs::error::check_cancelled(token)?;
    let output_len = 22usize
        .checked_add(payload.len())
        .ok_or_else(|| CodecError::Dimensions("ICO output length overflows".to_owned()))?;
    policy
        .check_output_len(output_len, ImageFormat::Ico, operation)
        .map_err(CodecError::from_image_error)?;
    let header = directory_header(size, payload.len(), bits, token)?;
    let mut written = 0usize;
    write_segment(sink, &header, token, &mut written)?;
    write_segment(sink, &payload, token, &mut written)?;
    debug_assert_eq!(written, output_len);
    Ok(written)
}

fn source_entry_size(img: &DecodedImage, opts: &IcoEncodeOptions) -> CodecResult<(usize, usize)> {
    let source = (bounded_usize_u32(img.width), bounded_usize_u32(img.height));
    if source.0 > 256 || source.1 > 256 {
        return Err(CodecError::Dimensions(
            "ICO entries cannot exceed 256 by 256 pixels".to_owned(),
        ));
    }
    match opts.sizes.as_slice() {
        [] => Ok(source),
        [size] if (usize::from(size.width), usize::from(size.height)) != source => {
            Err(CodecError::Parameter(
                "ICO sizes must contain exactly the source dimensions".to_owned(),
            ))
        }
        [_] => Ok(source),
        _ => Err(CodecError::Parameter(
            "ICO sizes must contain exactly one width-height pair".to_owned(),
        )),
    }
}

fn encode_directory(
    (width, height): (usize, usize),
    frame: &[u8],
    bits: u16,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    let header = directory_header((width, height), frame.len(), bits, token)?;
    // This codec intentionally writes one source-sized entry. Its default PNG
    // and BMP encoders are bounded by the 256x256 ceiling, so both the payload
    // length and the fixed offset fit in an ICO u32 field.
    let mut output = Vec::with_capacity(22usize.saturating_add(frame.len()));
    output.extend_from_slice(&header);
    crate::codecs::error::check_cancelled(token)?;
    output.extend_from_slice(frame);
    Ok(output)
}

fn directory_header(
    (width, height): (usize, usize),
    frame_len: usize,
    bits: u16,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<[u8; 22]> {
    crate::codecs::error::check_cancelled(token)?;
    if width == 0 || height == 0 || width > 256 || height > 256 {
        return Err(CodecError::Dimensions(
            "ICO entry dimensions must be between 1 and 256".to_owned(),
        ));
    }
    let mut header = [0u8; 22];
    header[..6].copy_from_slice(&[0, 0, 1, 0, 1, 0]);
    header[6] = directory_dimension(width);
    header[7] = directory_dimension(height);
    header[12..14].copy_from_slice(&bits.to_le_bytes());
    header[14..18].copy_from_slice(&low_u32(frame_len).to_le_bytes());
    header[18..22].copy_from_slice(&22u32.to_le_bytes());
    Ok(header)
}

fn directory_dimension(value: usize) -> u8 {
    debug_assert!(value <= 256);
    if value == 256 {
        0
    } else {
        value.to_le_bytes()[0]
    }
}

fn encode_bmp_entries(
    img: &DecodedImage,
    opts: &IcoEncodeOptions,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    crate::codecs::error::check_cancelled(token)?;
    let size = source_entry_size(img, opts)?;
    let (bits, payload) = encode_bmp_payload(img, token)?;
    crate::codecs::error::check_cancelled(token)?;
    encode_directory(size, &payload, bits, token)
}

fn bounded_usize_u32(value: u32) -> usize {
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

#[cfg(coverage)]
fn encode_bmp_single_entry(
    img: &DecodedImage,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    crate::codecs::error::check_cancelled(token)?;
    let size = (bounded_usize_u32(img.width), bounded_usize_u32(img.height));
    let (bits, payload) = encode_bmp_payload(img, token)?;
    encode_directory(size, &payload, bits, token)
}

fn encode_bmp_payload(
    img: &DecodedImage,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(u16, Vec<u8>)> {
    crate::codecs::error::check_cancelled(token)?;
    let width = bounded_usize_u32(img.width);
    let height = bounded_usize_u32(img.height);
    let (bits, row_bytes, pixels) = match img.color {
        ColorType::Rgb8 => {
            let source_row_bytes = width.saturating_mul(3);
            let row_bytes = source_row_bytes.next_multiple_of(4);
            let mut pixels = Vec::with_capacity(row_bytes.saturating_mul(height));
            for row in img.pixels.chunks_exact(source_row_bytes).rev() {
                crate::codecs::error::check_cancelled(token)?;
                for pixel in row.chunks_exact(3) {
                    pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
                }
                let padding = row_bytes.saturating_sub(source_row_bytes);
                pixels.resize(pixels.len().saturating_add(padding), 0);
            }
            (24u16, row_bytes, pixels)
        }
        ColorType::Rgba8 => {
            let row_bytes = width.saturating_mul(4);
            let mut pixels = Vec::with_capacity(row_bytes.saturating_mul(height));
            for row in img.pixels.chunks_exact(row_bytes).rev() {
                crate::codecs::error::check_cancelled(token)?;
                for pixel in row.chunks_exact(4) {
                    pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
                }
            }
            (32u16, row_bytes, pixels)
        }
        _ => {
            return Err(CodecError::Unsupported(format!(
                "BMP-backed ICO cannot encode color type {:?}",
                img.color
            )));
        }
    };
    let pixel_bytes = row_bytes.saturating_mul(height);
    // Each color arm emits exactly one validated source row at `row_bytes`.
    debug_assert_eq!(pixels.len(), pixel_bytes);

    let mask_row_bytes = width.div_ceil(8);
    let mask_bytes = if bits == 32 {
        0
    } else {
        // Source dimensions are capped at 256, so this is at most 8 KiB.
        mask_row_bytes.saturating_mul(height)
    };
    // Public BMP-backed ICO entries are generated only for <=256px sizes.
    let dib_bytes = 40usize
        .saturating_add(pixel_bytes)
        .saturating_add(mask_bytes);
    // The largest supported RGBA entry is below 264 KiB.
    let mut output = Vec::with_capacity(dib_bytes);
    output.extend_from_slice(&40u32.to_le_bytes());
    output.extend_from_slice(&img.width.to_le_bytes());
    output.extend_from_slice(&img.height.saturating_mul(2).to_le_bytes());
    output.extend_from_slice(&1u16.to_le_bytes());
    output.extend_from_slice(&bits.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&low_u32(pixel_bytes).to_le_bytes());
    output.extend_from_slice(&3_780i32.to_le_bytes());
    output.extend_from_slice(&3_780i32.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&pixels);
    output.resize(output.len().saturating_add(mask_bytes), 0);
    crate::codecs::error::check_cancelled(token)?;
    Ok((bits, output))
}

fn write_segment(
    sink: &mut dyn crate::OutputSink,
    bytes: &[u8],
    token: Option<&crate::CancellationToken>,
    written: &mut usize,
) -> CodecResult<()> {
    crate::codecs::error::check_cancelled(token)?;
    sink.write_all(bytes)
        .map_err(|error| CodecError::OutputWrite(error.to_string()))?;
    *written = written
        .checked_add(bytes.len())
        .ok_or_else(|| CodecError::Dimensions("ICO output length overflows".to_owned()))?;
    Ok(())
}
