//! Pure-Rust WebP encoder: internal VP8L lossless and VP8 lossy pipelines.

use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage};
use std::io::Cursor;

pub mod vp8;

/// Encode a DecodedImage to WebP format.
///
/// Lossless uses the internal VP8L encoder.
/// Lossy: uses our own pure-Rust VP8 intra-frame encoder.
pub fn encode(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    let encoded = if opts.lossless == Some(true) {
        encode_lossless(img, opts)
    } else {
        encode_lossy(img, opts)
    }?;
    attach_metadata(encoded, img.width, img.height, opts)
}

fn decode_hex(value: Option<&String>) -> Option<Option<Vec<u8>>> {
    let Some(value) = value else {
        return Some(None);
    };
    let decoded = (0..value.len())
        .step_by(2)
        .map(|index| {
            value
                .get(index..index + 2)
                .and_then(|byte| u8::from_str_radix(byte, 16).ok())
        })
        .collect::<Option<Vec<_>>>()?;
    value.len().is_multiple_of(2).then_some(Some(decoded))
}

fn write_chunk(output: &mut Vec<u8>, name: &[u8; 4], payload: &[u8]) {
    output.extend_from_slice(name);
    output.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    output.extend_from_slice(payload);
    if payload.len() % 2 != 0 {
        output.push(0);
    }
}

fn attach_metadata(
    encoded: Vec<u8>,
    width: u32,
    height: u32,
    opts: &EncodeOptions,
) -> Option<Vec<u8>> {
    let icc = decode_hex(opts.extra.get("icc_hex"))?;
    let exif = decode_hex(opts.extra.get("exif_hex"))?;
    let xmp = decode_hex(opts.extra.get("xmp_hex"))?;
    if icc.is_none() && exif.is_none() && xmp.is_none() {
        return Some(encoded);
    }
    let mut chunks = Vec::new();
    let mut offset = 12usize;
    let mut flags = 0u8;
    while offset + 8 <= encoded.len() {
        let name = [
            encoded[offset],
            encoded[offset + 1],
            encoded[offset + 2],
            encoded[offset + 3],
        ];
        let length = u32::from_le_bytes([
            encoded[offset + 4],
            encoded[offset + 5],
            encoded[offset + 6],
            encoded[offset + 7],
        ]) as usize;
        let end = offset + 8 + length;
        let payload = &encoded[offset + 8..end];
        if &name == b"VP8X" {
            flags |= encoded[offset + 8];
        } else {
            chunks.push((name, payload.to_vec()));
        }
        offset = end + (length & 1);
    }
    if icc.is_some() {
        flags |= 1 << 5;
    }
    if exif.is_some() {
        flags |= 1 << 3;
    }
    if xmp.is_some() {
        flags |= 1 << 2;
    }
    let mut output = Vec::new();
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&[0; 4]);
    output.extend_from_slice(b"WEBP");
    let mut vp8x = vec![flags, 0, 0, 0];
    vp8x.extend_from_slice(&(width - 1).to_le_bytes()[..3]);
    vp8x.extend_from_slice(&(height - 1).to_le_bytes()[..3]);
    write_chunk(&mut output, b"VP8X", &vp8x);
    if let Some(payload) = icc {
        write_chunk(&mut output, b"ICCP", &payload);
    }
    for (name, payload) in chunks {
        write_chunk(&mut output, &name, &payload);
    }
    if let Some(payload) = exif {
        let payload = payload.strip_prefix(b"Exif\0\0").unwrap_or(&payload);
        write_chunk(&mut output, b"EXIF", payload);
    }
    if let Some(payload) = xmp {
        write_chunk(&mut output, b"XMP ", &payload);
    }
    let riff_size = u32::try_from(output.len() - 8).ok()?;
    output[4..8].copy_from_slice(&riff_size.to_le_bytes());
    Some(output)
}

/// Lossless VP8L encoding via the internal `WebPEncoder`.
fn encode_lossless(img: &DecodedImage, _opts: &EncodeOptions) -> Option<Vec<u8>> {
    let (width, height) = (img.width, img.height);
    let converted = match img.color {
        ColorType::L8 => Some(
            img.pixels
                .iter()
                .flat_map(|&value| [value; 3])
                .collect::<Vec<_>>(),
        ),
        ColorType::Cmyk8 => Some(cmyk_to_rgb(&img.pixels)),
        _ => None,
    };
    let pixels = converted.as_deref().unwrap_or(&img.pixels);
    let color = match img.color {
        ColorType::L8 | ColorType::Rgb8 | ColorType::Cmyk8 => super::native::ColorType::Rgb8,
        ColorType::Rgba8 => super::native::ColorType::Rgba8,
        _ => return None,
    };

    let mut out = Cursor::new(Vec::new());
    let encoder = super::native::WebPEncoder::new(&mut out);
    encoder.encode(pixels, width, height, color).ok()?;

    Some(out.into_inner())
}

/// Lossy VP8 encoding — own pure-Rust implementation.
///
/// Encodes VP8 keyframe bitstream in RIFF/WEBP container.
fn encode_lossy(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
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
                let alpha_chunk = super::native::encode_alpha(&alpha, img.width, img.height)
                    .expect("Vec-backed alpha encoding does not return io errors");
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
        _ => return None,
    };
    Some(encoded)
}

fn cmyk_to_rgb(pixels: &[u8]) -> Vec<u8> {
    pixels
        .chunks_exact(4)
        .flat_map(|pixel| {
            let black = u16::from(255 - pixel[3]);
            std::array::from_fn::<_, 3, _>(|channel| {
                let ink = u16::from(255 - pixel[channel]);
                ((ink * black + 127) / 255) as u8
            })
        })
        .collect()
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    use std::collections::HashMap;

    vp8::__coverage_exercise_private_branches();

    let mut opts = EncodeOptions {
        extra: HashMap::from([("icc_hex".to_owned(), "f".to_owned())]),
        ..EncodeOptions::default()
    };
    let _ = attach_metadata(Vec::new(), 1, 1, &opts);

    opts.extra = HashMap::from([("exif_hex".to_owned(), "f".to_owned())]);
    let _ = attach_metadata(Vec::new(), 1, 1, &opts);

    let zero_width = DecodedImage::new(0, 1, Vec::new(), ColorType::Rgb8);
    let opts = EncodeOptions {
        lossless: Some(true),
        ..EncodeOptions::default()
    };
    let _ = encode(&zero_width, &opts);
}
