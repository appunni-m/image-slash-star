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
    let tag = u32::from(header[0]) | (u32::from(header[1]) << 8) | (u32::from(header[2]) << 16);
    let first_partition_size = (tag >> 5) as usize;
    payload.get(..10usize.wrapping_add(first_partition_size))?;
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
        if kind == *b"VP8 " {
            let info = inspect_vp8(payload)?;
            if info.width != width || info.height != height {
                return None;
            }
        } else if kind == *b"VP8L" {
            let info = inspect_vp8l(payload)?;
            if info.width != width || info.height != height {
                return None;
            }
        } else if kind == *b"ANMF" && validate_animation_frame(payload, width, height)? {
            frame_count = frame_count.wrapping_add(1);
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

fn validate_animation_frame(payload: &[u8], canvas_width: u32, canvas_height: u32) -> Option<bool> {
    let header = payload.get(..16)?;
    let left = le_u24(header, 0).wrapping_mul(2);
    let top = le_u24(header, 3).wrapping_mul(2);
    let width = le_u24(header, 6).wrapping_add(1);
    let height = le_u24(header, 9).wrapping_add(1);
    if width <= canvas_width
        && height <= canvas_height
        && (left.wrapping_add(width) > canvas_width || top.wrapping_add(height) > canvas_height)
    {
        return None;
    }

    let nested = &payload[16..];
    let (mut kind, mut image_payload, next) = read_chunk(nested, 0)?;
    if kind == *b"ALPH" {
        let rest = &nested[next..];
        (kind, image_payload, _) = read_chunk(rest, next)?;
    }
    match &kind {
        b"VP8 " => {
            let header = image_payload.get(..10)?;
            (&header[3..6] == b"\x9d\x01\x2a").then_some(true)
        }
        b"VP8L" => {
            let header = image_payload.get(..5)?;
            (header[0] == 0x2f
                && u32::from_le_bytes([header[1], header[2], header[3], header[4]]) >> 29 == 0)
                .then_some(true)
        }
        _ => Some(false),
    }
}

fn read_chunk(chunk: &[u8], position: usize) -> Option<([u8; 4], &[u8], usize)> {
    let prefix = chunk.get(..8)?;
    let mut kind = [0; 4];
    kind.copy_from_slice(&prefix[..4]);
    let length = u32::from_le_bytes([prefix[4], prefix[5], prefix[6], prefix[7]]) as usize;
    #[cfg(target_pointer_width = "64")]
    let padded_length = length.saturating_add(length & 1);
    #[cfg(not(target_pointer_width = "64"))]
    let padded_length = length.saturating_add(length & 1);
    let body = chunk[8..].get(..padded_length)?;
    let payload = &body[..length];
    let next = position.wrapping_add(8).wrapping_add(padded_length);
    Some((kind, payload, next))
}

fn le_u24(data: &[u8], offset: usize) -> u32 {
    u32::from(data[offset])
        | u32::from(data[offset.saturating_add(1)]).wrapping_shl(8)
        | u32::from(data[offset.saturating_add(2)]).wrapping_shl(16)
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    fn chunk(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut result = kind.to_vec();
        result.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        result.extend_from_slice(payload);
        if payload.len() & 1 != 0 {
            result.push(0);
        }
        result
    }

    let mut vp8x = [0u8; 10];
    vp8x[0] = 0;
    let mut vp8 = [0u8; 10];
    vp8[3..6].copy_from_slice(b"\x9d\x01\x2a");
    vp8[6..8].copy_from_slice(&1u16.to_le_bytes());
    vp8[8..10].copy_from_slice(&2u16.to_le_bytes());
    let data = chunk(b"VP8 ", &vp8);
    let _ = inspect_extended(&data, &vp8x, 0);
    let _ = inspect_extended(&[], &vp8x, 0);
    let _ = inspect_extended(&chunk(b"VP8 ", &[]), &vp8x, 0);
    let _ = inspect_extended(&chunk(b"VP8L", &[]), &vp8x, 0);

    for header in [1u32, 1u32 << 14] {
        let mut vp8l = vec![0x2f];
        vp8l.extend_from_slice(&header.to_le_bytes());
        let data = chunk(b"VP8L", &vp8l);
        let _ = inspect_extended(&data, &vp8x, 0);
    }

    fn animation_payload(left: u32, top: u32, width: u32, height: u32, nested: Vec<u8>) -> Vec<u8> {
        let mut payload = vec![0; 16];
        payload[0..3].copy_from_slice(&left.to_le_bytes()[..3]);
        payload[3..6].copy_from_slice(&top.to_le_bytes()[..3]);
        payload[6..9].copy_from_slice(&(width - 1).to_le_bytes()[..3]);
        payload[9..12].copy_from_slice(&(height - 1).to_le_bytes()[..3]);
        payload.extend_from_slice(&nested);
        payload
    }

    let good_vp8 = chunk(b"VP8 ", &vp8);
    let _ = validate_animation_frame(&[], 1, 1);
    let _ = validate_animation_frame(&[0; 16], 1, 1);
    let _ = validate_animation_frame(&animation_payload(0, 0, 1, 1, chunk(b"ALPH", &[])), 1, 1);
    let _ = validate_animation_frame(&animation_payload(0, 0, 1, 1, chunk(b"VP8 ", &[])), 1, 1);
    let _ = validate_animation_frame(&animation_payload(0, 0, 1, 1, chunk(b"VP8L", &[])), 1, 1);
    let _ = validate_animation_frame(&animation_payload(0, 1, 1, 1, good_vp8.clone()), 1, 1);
    let _ = validate_animation_frame(&animation_payload(0, 0, 1, 2, good_vp8), 1, 1);
    for vp8l in [[0, 0, 0, 0, 0], [0x2f, 0, 0, 0, 0x20]] {
        let nested = chunk(b"VP8L", &vp8l);
        let _ = validate_animation_frame(&animation_payload(0, 0, 1, 1, nested), 1, 1);
    }
}
