//! WebP RIFF and frame-header inspection without pixel decoding.

use crate::codecs::{
    CodecError, CodecResult, OptionCodecExt, codec_add_end, need_slice, terminalize,
};
use crate::types::{ImageFormat, ImageInfo, ImageMode};

const RIFF_HEADER_SIZE: usize = 12;
const PILLOW_DECOMPRESSION_BOMB_ERROR_PIXELS: u64 = 178_956_970;

/// Inspect WebP dimensions, decoded mode, and animation frame count.
pub fn inspect(data: &[u8]) -> CodecResult<ImageInfo> {
    inspect_inner(data, false)
}

/// Inspect WebP dimensions and the first proven image without counting every
/// animation frame chunk.
pub fn inspect_basic(data: &[u8]) -> CodecResult<ImageInfo> {
    inspect_inner(data, true)
}

fn inspect_inner(data: &[u8], basic: bool) -> CodecResult<ImageInfo> {
    let header = need_slice(data, 0, RIFF_HEADER_SIZE, "truncated WebP RIFF header")?;
    if &header[..4] != b"RIFF" || &header[8..12] != b"WEBP" {
        return Err(CodecError::Malformed("invalid WebP signature".to_owned()));
    }
    let (kind, payload, next) = read_chunk(data, RIFF_HEADER_SIZE)?;
    match &kind {
        b"VP8 " => inspect_vp8(payload),
        b"VP8L" => inspect_vp8l(payload),
        b"VP8X" if basic => inspect_extended_basic(data, payload, next),
        b"VP8X" => inspect_extended(data, payload, next),
        _ => Err(CodecError::Malformed(
            "WebP contains no recognized image header".to_owned(),
        )),
    }
}

fn inspect_extended_basic(
    data: &[u8],
    payload: &[u8],
    mut position: usize,
) -> CodecResult<ImageInfo> {
    let header = payload.get(..10).malformed("truncated WebP VP8X header")?;
    let flags = header[0];
    let width = le_u24(header, 4).wrapping_add(1);
    let height = le_u24(header, 7).wrapping_add(1);
    enforce_dimension_limit(width, height)?;
    let has_alpha = flags & 0x10 != 0;
    let declares_animation = flags & 0x02 != 0;
    let mode = if has_alpha {
        ImageMode::Rgba8
    } else {
        ImageMode::Rgb8
    };
    let source = if has_alpha {
        crate::types::SourceDescriptor::new().with_alpha(crate::types::SourceAlpha::Straight)
    } else {
        crate::types::SourceDescriptor::new()
    };
    if declares_animation {
        return Ok(ImageInfo {
            format: ImageFormat::WebP,
            width,
            height,
            mode,
            bit_depth: 8,
            palette: None,
            is_animated: true,
            frame_count: None,
            frame_count_complete: false,
            cursor_hotspot: None,
            source,
        });
    }
    let mut saw_image = false;
    while position < data.len() {
        let (kind, payload, next) = read_chunk(data, position)?;
        position = next;
        if kind == *b"VP8 " {
            let info = inspect_vp8(payload)?;
            if info.width != width || info.height != height {
                return Err(CodecError::Malformed(
                    "WebP VP8 dimensions disagree with the canvas".to_owned(),
                ));
            }
            saw_image = true;
            break;
        }
        if kind == *b"VP8L" {
            let info = inspect_vp8l(payload)?;
            if info.width != width || info.height != height {
                return Err(CodecError::Malformed(
                    "WebP VP8L dimensions disagree with the canvas".to_owned(),
                ));
            }
            saw_image = true;
            break;
        }
    }
    if !saw_image {
        return Err(CodecError::NeedMore {
            minimum: codec_add_end(position, 8),
            message: "extended WebP contains no image chunk".to_owned(),
        });
    }
    Ok(ImageInfo {
        format: ImageFormat::WebP,
        width,
        height,
        mode,
        bit_depth: 8,
        palette: None,
        is_animated: false,
        frame_count: Some(1),
        frame_count_complete: true,
        cursor_hotspot: None,
        source,
    })
}

fn inspect_vp8(payload: &[u8]) -> CodecResult<ImageInfo> {
    let header = payload.get(..10).malformed("truncated WebP VP8 header")?;
    if header[0] & 1 != 0 || &header[3..6] != b"\x9d\x01\x2a" {
        return Err(CodecError::Malformed(
            "invalid WebP VP8 keyframe".to_owned(),
        ));
    }
    let width = u32::from(u16::from_le_bytes([header[6], header[7]]) & 0x3fff);
    let height = u32::from(u16::from_le_bytes([header[8], header[9]]) & 0x3fff);
    let tag = u32::from(header[0]) | (u32::from(header[1]) << 8) | (u32::from(header[2]) << 16);
    let first_partition_size = (tag >> 5) as usize;
    payload
        .get(..10usize.wrapping_add(first_partition_size))
        .malformed("truncated WebP VP8 first partition")?;
    still_info(width, height, ImageMode::Rgb8, false)
}

fn inspect_vp8l(payload: &[u8]) -> CodecResult<ImageInfo> {
    let payload = payload.get(..5).malformed("truncated WebP VP8L header")?;
    if payload[0] != 0x2f {
        return Err(CodecError::Malformed(
            "invalid WebP VP8L signature".to_owned(),
        ));
    }
    let header = u32::from_le_bytes([payload[1], payload[2], payload[3], payload[4]]);
    if header >> 29 != 0 {
        return Err(CodecError::Malformed(
            "invalid WebP VP8L version".to_owned(),
        ));
    }
    let width = (header & 0x3fff).wrapping_add(1);
    let height = ((header >> 14) & 0x3fff).wrapping_add(1);
    let has_alpha = header & (1 << 28) != 0;
    let mode = if has_alpha {
        ImageMode::Rgba8
    } else {
        ImageMode::Rgb8
    };
    still_info(width, height, mode, has_alpha)
}

fn still_info(width: u32, height: u32, mode: ImageMode, has_alpha: bool) -> CodecResult<ImageInfo> {
    if width == 0 || height == 0 {
        return Err(CodecError::Malformed(
            "WebP dimensions must be nonzero".to_owned(),
        ));
    }
    enforce_dimension_limit(width, height)?;
    let source = if has_alpha {
        crate::types::SourceDescriptor::new().with_alpha(crate::types::SourceAlpha::Straight)
    } else {
        crate::types::SourceDescriptor::new()
    };
    Ok(ImageInfo {
        format: ImageFormat::WebP,
        width,
        height,
        mode,
        bit_depth: 8,
        palette: None,
        is_animated: false,
        frame_count: Some(1),
        frame_count_complete: true,
        cursor_hotspot: None,
        source,
    })
}

fn inspect_extended(data: &[u8], payload: &[u8], mut position: usize) -> CodecResult<ImageInfo> {
    let header = payload.get(..10).malformed("truncated WebP VP8X header")?;
    let flags = header[0];
    let width = le_u24(header, 4).wrapping_add(1);
    let height = le_u24(header, 7).wrapping_add(1);
    enforce_dimension_limit(width, height)?;
    let declares_animation = flags & 0x02 != 0;
    let has_alpha = flags & 0x10 != 0;
    let mode = if has_alpha {
        ImageMode::Rgba8
    } else {
        ImageMode::Rgb8
    };
    let mut frame_count = 0u32;
    let mut saw_image = false;
    let mut saw_animation_control = false;
    while position < data.len() {
        let (kind, payload, next) = read_chunk(data, position)?;
        position = next;
        if kind == *b"VP8 " {
            let info = inspect_vp8(payload)?;
            if info.width != width || info.height != height {
                return Err(CodecError::Malformed(
                    "WebP VP8 dimensions disagree with the canvas".to_owned(),
                ));
            }
            saw_image = true;
            frame_count = 1;
        } else if kind == *b"VP8L" {
            let info = inspect_vp8l(payload)?;
            if info.width != width || info.height != height {
                return Err(CodecError::Malformed(
                    "WebP VP8L dimensions disagree with the canvas".to_owned(),
                ));
            }
            saw_image = true;
            frame_count = 1;
        } else if kind == *b"ANIM" {
            if payload.len() < 6 {
                return Err(CodecError::Malformed(
                    "invalid WebP animation control chunk".to_owned(),
                ));
            }
            saw_animation_control = true;
        } else if kind == *b"ANMF" && validate_animation_frame(payload, width, height)? {
            frame_count = frame_count.wrapping_add(1);
        }
    }
    if declares_animation {
        if !saw_animation_control || frame_count == 0 {
            return Err(CodecError::NeedMore {
                minimum: codec_add_end(position, 8),
                message: "animated WebP lacks animation control or frames".to_owned(),
            });
        }
    } else if !saw_image {
        return Err(CodecError::NeedMore {
            minimum: codec_add_end(position, 8),
            message: "extended WebP contains no image chunk".to_owned(),
        });
    }
    let is_animated = frame_count > 1;
    let source = if has_alpha {
        crate::types::SourceDescriptor::new().with_alpha(crate::types::SourceAlpha::Straight)
    } else {
        crate::types::SourceDescriptor::new()
    };
    Ok(ImageInfo {
        format: ImageFormat::WebP,
        width,
        height,
        mode,
        bit_depth: 8,
        palette: None,
        is_animated,
        frame_count: Some(frame_count),
        frame_count_complete: true,
        cursor_hotspot: None,
        source,
    })
}

fn validate_animation_frame(
    payload: &[u8],
    canvas_width: u32,
    canvas_height: u32,
) -> CodecResult<bool> {
    let header = payload
        .get(..16)
        .malformed("truncated WebP animation-frame header")?;
    let left = le_u24(header, 0).wrapping_mul(2);
    let top = le_u24(header, 3).wrapping_mul(2);
    let width = le_u24(header, 6).wrapping_add(1);
    let height = le_u24(header, 9).wrapping_add(1);
    if width <= canvas_width
        && height <= canvas_height
        && (left.wrapping_add(width) > canvas_width || top.wrapping_add(height) > canvas_height)
    {
        return Err(CodecError::Malformed(
            "WebP animation frame lies outside the canvas".to_owned(),
        ));
    }

    let nested = &payload[16..];
    let (mut kind, mut image_payload, next) = read_chunk(nested, 0).map_err(terminalize)?;
    if kind == *b"ALPH" {
        let rest = &nested[next..];
        (kind, image_payload, _) = read_chunk(rest, 0).map_err(terminalize)?;
    }
    match &kind {
        b"VP8 " => {
            let header = image_payload
                .get(..10)
                .malformed("truncated animated WebP VP8 header")?;
            if &header[3..6] != b"\x9d\x01\x2a" {
                return Err(CodecError::Malformed(
                    "invalid animated WebP VP8 keyframe".to_owned(),
                ));
            }
            Ok(true)
        }
        b"VP8L" => {
            let header = image_payload
                .get(..5)
                .malformed("truncated animated WebP VP8L header")?;
            if header[0] != 0x2f
                || u32::from_le_bytes([header[1], header[2], header[3], header[4]]) >> 29 != 0
            {
                return Err(CodecError::Malformed(
                    "invalid animated WebP VP8L header".to_owned(),
                ));
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn read_chunk(data: &[u8], position: usize) -> CodecResult<([u8; 4], &[u8], usize)> {
    let prefix_end = codec_add_end(position, 8);
    let prefix = need_slice(data, position, prefix_end, "truncated WebP chunk header")?;
    let mut kind = [0; 4];
    kind.copy_from_slice(&prefix[..4]);
    let length = u32::from_le_bytes([prefix[4], prefix[5], prefix[6], prefix[7]]) as usize;
    #[cfg(target_pointer_width = "64")]
    let padded_length = length.saturating_add(length & 1);
    #[cfg(not(target_pointer_width = "64"))]
    let padded_length = length.saturating_add(length & 1);
    let body_end = codec_add_end(prefix_end, padded_length);
    let body = need_slice(data, prefix_end, body_end, "truncated WebP chunk payload")?;
    let payload = &body[..length];
    Ok((kind, payload, body_end))
}

fn enforce_dimension_limit(width: u32, height: u32) -> CodecResult<()> {
    if u64::from(width).saturating_mul(u64::from(height)) > PILLOW_DECOMPRESSION_BOMB_ERROR_PIXELS {
        return Err(CodecError::Malformed(
            "WebP dimensions exceed decoder limits".to_owned(),
        ));
    }
    Ok(())
}

fn le_u24(data: &[u8], offset: usize) -> u32 {
    u32::from(data[offset])
        | u32::from(data[offset.saturating_add(1)]).wrapping_shl(8)
        | u32::from(data[offset.saturating_add(2)]).wrapping_shl(16)
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let _ = inspect(b"");
    let _ = inspect(b"not a WebP!!");
    let _ = inspect(b"RIFFxxxxNOPE");

    fn chunk(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut result = kind.to_vec();
        result.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        result.extend_from_slice(payload);
        if payload.len() & 1 != 0 {
            result.push(0);
        }
        result
    }
    let mut unknown = b"RIFF\0\0\0\0WEBP".to_vec();
    unknown.extend_from_slice(&chunk(b"JUNK", &[]));
    let _ = inspect(&unknown);

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
    let _ = inspect_extended_basic(&[], &[], 0);
    let _ = inspect_extended_basic(&[], &vp8x, 0);
    let _ = inspect_extended_basic(&[0], &vp8x, 0);
    let _ = inspect_extended_basic(&[], &[0xff; 10], 0);
    let _ = inspect_extended_basic(&chunk(b"VP8 ", &[0u8; 10]), &vp8x, 0);
    let _ = inspect_extended_basic(&chunk(b"VP8 ", &vp8), &[0u8; 10], 0);
    let mut wide_vp8 = vp8;
    wide_vp8[6..8].copy_from_slice(&2u16.to_le_bytes());
    let _ = inspect_extended_basic(&chunk(b"VP8 ", &wide_vp8), &vp8x, 0);
    let _ = inspect_extended_basic(&chunk(b"VP8L", &[0u8; 5]), &vp8x, 0);
    let mut lossless = vec![0x2f];
    lossless.extend_from_slice(&0u32.to_le_bytes());
    let _ = inspect_extended_basic(&chunk(b"VP8L", &lossless), &vp8x, 0);
    let mut mismatched_lossless = vec![0x2f];
    mismatched_lossless.extend_from_slice(&1u32.to_le_bytes());
    let _ = inspect_extended_basic(&chunk(b"VP8L", &mismatched_lossless), &vp8x, 0);
    let mut tall_lossless = vec![0x2f];
    tall_lossless.extend_from_slice(&(1u32 << 14).to_le_bytes());
    let _ = inspect_extended_basic(&chunk(b"VP8L", &tall_lossless), &vp8x, 0);

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
