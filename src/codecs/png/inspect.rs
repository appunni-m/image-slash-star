//! PNG header and palette inspection without IDAT decompression.

use crate::types::{ImageFormat, ImageInfo, ImageMode, ImagePalette};

const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

/// Inspect PNG metadata up to the first image-data chunk.
pub fn inspect(data: &[u8]) -> Option<ImageInfo> {
    if data.get(..8)? != SIGNATURE {
        return None;
    }
    let (kind, header, mut position) = read_chunk(&data[8..], 8)?;
    if kind != *b"IHDR" || header.len() != 13 {
        return None;
    }

    let width = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
    let height = u32::from_be_bytes([header[4], header[5], header[6], header[7]]);
    let bit_depth = header[8];
    let color_type = header[9];
    if width == 0 || height == 0 || header[11] != 0 || header[12] > 1 {
        return None;
    }
    let mode = png_mode(color_type, bit_depth)?;

    let mut palette_rgb = None;
    let mut palette_alpha = Vec::new();
    let mut frame_count = 1;
    while position < data.len() {
        let (kind, payload, next) = read_chunk(&data[position..], position)?;
        position = next;
        match &kind {
            b"PLTE" if palette_rgb.is_none() => {
                let entries = (payload.len() / 3).min(256);
                if entries != 0 {
                    palette_rgb = Some(payload[..entries.wrapping_mul(3)].to_vec());
                }
            }
            b"tRNS" if color_type == 3 && palette_alpha.is_empty() => {
                palette_alpha = payload.to_vec();
            }
            b"acTL" => {
                if payload.len() != 8 {
                    return None;
                }
                let frames = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
                if frames != 0 {
                    frame_count = frames;
                }
            }
            b"IDAT" | b"IEND" => break,
            _ => {}
        }
    }

    let palette = if mode == ImageMode::P8 {
        palette_rgb.map(|rgb| {
            palette_alpha.truncate(rgb.len() / 3);
            ImagePalette {
                rgb,
                alpha: palette_alpha,
            }
        })
    } else {
        None
    };
    let is_animated = frame_count > 1;
    Some(ImageInfo {
        format: ImageFormat::Png,
        width,
        height,
        mode,
        bit_depth,
        palette,
        is_animated,
        frame_count: Some(frame_count),
    })
}

fn png_mode(color_type: u8, bit_depth: u8) -> Option<ImageMode> {
    match (color_type, bit_depth) {
        (0, 1) => Some(ImageMode::L1),
        (0, 2 | 4 | 8) => Some(ImageMode::L8),
        (0, 16) => Some(ImageMode::L16),
        (2, 8) => Some(ImageMode::Rgb8),
        (2, 16) => Some(ImageMode::Rgb8),
        (3, 1 | 2 | 4 | 8) => Some(ImageMode::P8),
        (4, 8) => Some(ImageMode::La8),
        (4, 16) => Some(ImageMode::Rgba8),
        (6, 8) => Some(ImageMode::Rgba8),
        (6, 16) => Some(ImageMode::Rgba8),
        _ => None,
    }
}

fn read_chunk(chunk: &[u8], position: usize) -> Option<([u8; 4], &[u8], usize)> {
    let prefix = chunk.get(..8)?;
    let length = u32::from_be_bytes([prefix[0], prefix[1], prefix[2], prefix[3]]) as usize;
    let mut kind = [0; 4];
    kind.copy_from_slice(&prefix[4..8]);
    let rest = &chunk[8..];
    #[cfg(target_pointer_width = "64")]
    let payload_and_crc_len = length.wrapping_add(4);
    #[cfg(not(target_pointer_width = "64"))]
    let payload_and_crc_len = length.saturating_add(4);
    let payload_and_crc = rest.get(..payload_and_crc_len)?;
    let payload = &payload_and_crc[..length];
    let next = position
        .wrapping_add(8)
        .wrapping_add(length)
        .wrapping_add(4);
    Some((kind, payload, next))
}
