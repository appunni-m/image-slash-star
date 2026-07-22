//! WebP RIFF and frame-header inspection without pixel decoding.

use crate::types::{ImageFormat, ImageInfo, ImageMode};

const RIFF_HEADER_SIZE: usize = 12;

/// Inspect WebP dimensions, decoded mode, and animation frame count.
pub fn inspect(data: &[u8]) -> Option<ImageInfo> {
    let header = data.get(..RIFF_HEADER_SIZE)?;
    if &header[..4] != b"RIFF" || &header[8..12] != b"WEBP" {
        return None;
    }
    let (kind, payload, next) = read_chunk(&data[RIFF_HEADER_SIZE..], RIFF_HEADER_SIZE)?;
    match &kind {
        b"VP8 " => inspect_vp8(payload),
        b"VP8L" => inspect_vp8l(payload),
        b"VP8X" => inspect_extended(data, payload, next),
        _ => None,
    }
}

fn inspect_vp8(payload: &[u8]) -> Option<ImageInfo> {
    let header = payload.get(..10)?;
    if header[0] & 1 != 0 || &header[3..6] != b"\x9d\x01\x2a" {
        return None;
    }
    let width = u32::from(u16::from_le_bytes([header[6], header[7]]) & 0x3fff);
    let height = u32::from(u16::from_le_bytes([header[8], header[9]]) & 0x3fff);
    still_info(width, height, ImageMode::Rgb8)
}

fn inspect_vp8l(payload: &[u8]) -> Option<ImageInfo> {
    let payload = payload.get(..5)?;
    if payload[0] != 0x2f {
        return None;
    }
    let header = u32::from_le_bytes([payload[1], payload[2], payload[3], payload[4]]);
    if header >> 29 != 0 {
        return None;
    }
    let width = (header & 0x3fff).wrapping_add(1);
    let height = ((header >> 14) & 0x3fff).wrapping_add(1);
    let mode = if header & (1 << 28) != 0 {
        ImageMode::Rgba8
    } else {
        ImageMode::Rgb8
    };
    still_info(width, height, mode)
}

fn still_info(width: u32, height: u32, mode: ImageMode) -> Option<ImageInfo> {
    if width == 0 || height == 0 {
        return None;
    }
    Some(ImageInfo {
        format: ImageFormat::WebP,
        width,
        height,
        mode,
        bit_depth: 8,
        palette: None,
        is_animated: false,
        frame_count: Some(1),
    })
}

fn inspect_extended(data: &[u8], payload: &[u8], mut position: usize) -> Option<ImageInfo> {
    let header = payload.get(..10)?;
    let flags = header[0];
    let width = le_u24(header, 4).wrapping_add(1);
    let height = le_u24(header, 7).wrapping_add(1);
    let declares_animation = flags & 0x02 != 0;
    let mode = if flags & 0x10 != 0 {
        ImageMode::Rgba8
    } else {
        ImageMode::Rgb8
    };
    let mut frame_count = u32::from(!declares_animation);
    while position < data.len() {
        let (kind, payload, next) = read_chunk(&data[position..], position)?;
        position = next;
        if kind == *b"ANMF" {
            let frame_kind = payload.get(16..20)?;
            if matches!(frame_kind, b"VP8 " | b"VP8L" | b"ALPH") {
                frame_count = frame_count.wrapping_add(1);
            }
        }
    }
    if declares_animation && frame_count == 0 {
        return None;
    }
    let is_animated = frame_count > 1;
    Some(ImageInfo {
        format: ImageFormat::WebP,
        width,
        height,
        mode,
        bit_depth: 8,
        palette: None,
        is_animated,
        frame_count: Some(frame_count),
    })
}

fn read_chunk(chunk: &[u8], position: usize) -> Option<([u8; 4], &[u8], usize)> {
    let prefix = chunk.get(..8)?;
    let mut kind = [0; 4];
    kind.copy_from_slice(&prefix[..4]);
    let length = u32::from_le_bytes([prefix[4], prefix[5], prefix[6], prefix[7]]) as usize;
    #[cfg(target_pointer_width = "64")]
    let padded_length = length + (length & 1);
    #[cfg(not(target_pointer_width = "64"))]
    let padded_length = length.saturating_add(length & 1);
    let body = chunk[8..].get(..padded_length)?;
    let payload = &body[..length];
    let next = position.wrapping_add(8).wrapping_add(padded_length);
    Some((kind, payload, next))
}

fn le_u24(data: &[u8], offset: usize) -> u32 {
    u32::from(data[offset])
        | (u32::from(data[offset + 1]) << 8)
        | (u32::from(data[offset + 2]) << 16)
}
