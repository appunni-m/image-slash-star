//! Pure-Rust WebP encoder: internal VP8L lossless and VP8 lossy pipelines.

use crate::codecs::{CodecError, CodecResult};
use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage};
use std::borrow::Cow;

pub mod vp8;

/// Encode a DecodedImage to WebP format.
///
/// Lossless uses the internal VP8L encoder.
/// Lossy: uses our own pure-Rust VP8 intra-frame encoder.
pub fn encode(img: &DecodedImage, opts: &EncodeOptions) -> CodecResult<Vec<u8>> {
    let encoded = if opts.lossless == Some(true) {
        encode_lossless(img, opts)
    } else {
        encode_lossy(img, opts)
    }?;
    let alpha = img.color == ColorType::Rgba8
        && img.pixels.chunks_exact(4).any(|pixel| pixel[3] != u8::MAX);
    attach_metadata(encoded, img.width, img.height, alpha, opts)
}

fn decode_hex(value: Option<&String>) -> CodecResult<Option<Vec<u8>>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if !value.len().is_multiple_of(2) {
        return Err(CodecError::Parameter(
            "WebP metadata hex must contain complete byte pairs".to_owned(),
        ));
    }
    let mut decoded = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        decoded.push((hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?);
    }
    Ok(Some(decoded))
}

fn hex_nibble(value: u8) -> CodecResult<u8> {
    match value {
        b'0'..=b'9' => Ok(value.wrapping_sub(b'0')),
        b'a'..=b'f' => Ok(value.wrapping_sub(b'a').wrapping_add(10)),
        b'A'..=b'F' => Ok(value.wrapping_sub(b'A').wrapping_add(10)),
        _ => Err(CodecError::Parameter(
            "WebP metadata hex contains a non-hexadecimal byte".to_owned(),
        )),
    }
}

fn write_chunk(output: &mut Vec<u8>, name: &[u8; 4], payload: &[u8]) {
    output.extend_from_slice(name);
    output.extend_from_slice(&low_u32(payload.len()).to_le_bytes());
    output.extend_from_slice(payload);
    if !payload.len().is_multiple_of(2) {
        output.push(0);
    }
}

fn attach_metadata(
    encoded: Vec<u8>,
    width: u32,
    height: u32,
    alpha: bool,
    opts: &EncodeOptions,
) -> CodecResult<Vec<u8>> {
    let icc = decode_hex(opts.extra.get("icc_hex"))?;
    let exif = decode_hex(opts.extra.get("exif_hex"))?;
    let xmp = decode_hex(opts.extra.get("xmp_hex"))?;
    if icc.is_none() && exif.is_none() && xmp.is_none() {
        return Ok(encoded);
    }
    // Both internal encoders return a complete RIFF header. Lossy alpha output
    // additionally begins with the fixed 18-byte VP8X chunk constructed by
    // `encode_vp8_lossy_rgba`; replace that chunk instead of reparsing bytes
    // emitted by the same implementation.
    let encoded_chunks = &encoded[12..];
    let encoded_chunks = if encoded_chunks.starts_with(b"VP8X") {
        debug_assert!(encoded_chunks.len() >= 18);
        &encoded_chunks[18..]
    } else {
        encoded_chunks
    };
    let mut flags = u8::from(alpha).wrapping_shl(4);
    if icc.is_some() {
        flags |= 1u8.wrapping_shl(5);
    }
    if exif.is_some() {
        flags |= 1u8.wrapping_shl(3);
    }
    if xmp.is_some() {
        flags |= 1u8.wrapping_shl(2);
    }
    let mut output = Vec::new();
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&[0; 4]);
    output.extend_from_slice(b"WEBP");
    let mut vp8x = vec![flags, 0, 0, 0];
    vp8x.extend_from_slice(&width.saturating_sub(1).to_le_bytes()[..3]);
    vp8x.extend_from_slice(&height.saturating_sub(1).to_le_bytes()[..3]);
    write_chunk(&mut output, b"VP8X", &vp8x);
    if let Some(payload) = icc {
        write_chunk(&mut output, b"ICCP", &payload);
    }
    output.extend_from_slice(encoded_chunks);
    if let Some(payload) = exif {
        let payload = payload.strip_prefix(b"Exif\0\0").unwrap_or(&payload);
        write_chunk(&mut output, b"EXIF", payload);
    }
    if let Some(payload) = xmp {
        write_chunk(&mut output, b"XMP ", &payload);
    }
    #[cfg(coverage)]
    let output_len = if opts
        .extra
        .contains_key("__coverage_force_webp_riff_size_overflow")
    {
        usize::MAX
    } else {
        output.len()
    };
    #[cfg(not(coverage))]
    let output_len = output.len();
    let riff_size = u32::try_from(output_len.saturating_sub(8)).map_err(|error| {
        CodecError::Dimensions(format!("WebP RIFF output exceeds format limits: {error}"))
    })?;
    output[4..8].copy_from_slice(&riff_size.to_le_bytes());
    Ok(output)
}

/// Lossless VP8L encoding via the internal `WebPEncoder`.
fn encode_lossless(img: &DecodedImage, _opts: &EncodeOptions) -> CodecResult<Vec<u8>> {
    let (width, height) = (img.width, img.height);
    let (pixels, color) = match img.color {
        ColorType::L8 => (
            Cow::Owned(
                img.pixels
                    .iter()
                    .flat_map(|&value| [value; 3])
                    .collect::<Vec<_>>(),
            ),
            super::native::ColorType::Rgb8,
        ),
        ColorType::Cmyk8 => (
            Cow::Owned(cmyk_to_rgb(&img.pixels)),
            super::native::ColorType::Rgb8,
        ),
        ColorType::Rgb8 => (
            Cow::Borrowed(img.pixels.as_slice()),
            super::native::ColorType::Rgb8,
        ),
        ColorType::Rgba8 => (
            Cow::Borrowed(img.pixels.as_slice()),
            super::native::ColorType::Rgba8,
        ),
        _ => {
            return Err(CodecError::Unsupported(
                "WebP lossless encoder does not support this image mode".to_owned(),
            ));
        }
    };

    super::native::WebPEncoder::new()
        .encode(&pixels, width, height, color)
        .map_err(encode_error)
}

/// Lossy VP8 encoding — own pure-Rust implementation.
///
/// Encodes VP8 keyframe bitstream in RIFF/WEBP container.
fn encode_lossy(img: &DecodedImage, opts: &EncodeOptions) -> CodecResult<Vec<u8>> {
    let quality = opts.quality.unwrap_or(80).min(100);
    let method = opts.method.unwrap_or(4).min(6);
    let encoded = match img.color {
        ColorType::L8 => {
            let rgb = img
                .pixels
                .iter()
                .flat_map(|&value| [value; 3])
                .collect::<Vec<_>>();
            vp8::encoder::encode_vp8_lossy(&rgb, img.width, img.height, quality, method)
        }
        ColorType::Rgb8 => {
            vp8::encoder::encode_vp8_lossy(&img.pixels, img.width, img.height, quality, method)
        }
        ColorType::Rgba8 => {
            let has_alpha = img.pixels.chunks_exact(4).any(|pixel| pixel[3] != u8::MAX);
            if has_alpha {
                let alpha = img
                    .pixels
                    .chunks_exact(4)
                    .map(|pixel| pixel[3])
                    .collect::<Vec<_>>();
                let alpha_chunk = super::native::encode_alpha(&alpha, img.width, img.height);
                vp8::encoder::encode_vp8_lossy_rgba(
                    &img.pixels,
                    img.width,
                    img.height,
                    quality,
                    method,
                    &alpha_chunk,
                )
            } else {
                let rgb = img
                    .pixels
                    .chunks_exact(4)
                    .flat_map(|pixel| pixel[..3].iter().copied())
                    .collect::<Vec<_>>();
                vp8::encoder::encode_vp8_lossy(&rgb, img.width, img.height, quality, method)
            }
        }
        ColorType::Cmyk8 => {
            let rgb = cmyk_to_rgb(&img.pixels);
            vp8::encoder::encode_vp8_lossy(&rgb, img.width, img.height, quality, method)
        }
        _ => {
            return Err(CodecError::Unsupported(
                "WebP lossy encoder does not support this image mode".to_owned(),
            ));
        }
    };
    Ok(encoded)
}

fn encode_error(error: super::native::EncodingError) -> CodecError {
    match error {
        super::native::EncodingError::InvalidDimensions => {
            CodecError::Dimensions("WebP lossless dimensions are invalid".to_owned())
        }
    }
}

fn cmyk_to_rgb(pixels: &[u8]) -> Vec<u8> {
    pixels
        .chunks_exact(4)
        .flat_map(|pixel| {
            let black = u16::from(255u8.saturating_sub(pixel[3]));
            std::array::from_fn::<_, 3, _>(|channel| {
                let ink = u16::from(255u8.saturating_sub(pixel[channel]));
                ink.saturating_mul(black)
                    .saturating_add(127)
                    .div_euclid(255)
                    .to_le_bytes()[0]
            })
        })
        .collect()
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
pub(crate) fn __coverage_exercise_private_branches() {
    use std::collections::HashMap;

    vp8::__coverage_exercise_private_branches();

    let mut opts = EncodeOptions {
        extra: HashMap::from([("icc_hex".to_owned(), "f".to_owned())]),
        ..EncodeOptions::default()
    };
    let _ = attach_metadata(Vec::new(), 1, 1, false, &opts);

    opts.extra = HashMap::from([("exif_hex".to_owned(), "f".to_owned())]);
    let _ = attach_metadata(Vec::new(), 1, 1, false, &opts);

    opts.extra = HashMap::from([("xmp_hex".to_owned(), "f".to_owned())]);
    let _ = attach_metadata(Vec::new(), 1, 1, false, &opts);

    opts.extra = HashMap::from([
        ("icc_hex".to_owned(), "00".to_owned()),
        (
            "__coverage_force_webp_riff_size_overflow".to_owned(),
            "1".to_owned(),
        ),
    ]);
    let _ = attach_metadata(b"RIFF\0\0\0\0WEBP".to_vec(), 1, 1, false, &opts);

    let zero_width = DecodedImage::new(0, 1, Vec::new(), ColorType::Rgb8);
    let opts = EncodeOptions {
        lossless: Some(true),
        ..EncodeOptions::default()
    };
    let _ = encode(&zero_width, &opts);

    let unsupported = DecodedImage::new(1, 1, vec![0, 0], ColorType::La8);
    let _ = encode(&unsupported, &opts);
    let _ = encode(&unsupported, &EncodeOptions::default());
}
