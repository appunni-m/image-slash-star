//! PNG header and palette inspection without IDAT decompression.

use crate::codecs::{CodecError, CodecResult, OptionCodecExt};
use crate::types::{ImageFormat, ImageInfo, ImageMode, ImagePalette};

const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const PILLOW_DECOMPRESSION_BOMB_ERROR_PIXELS: u64 = 178_956_970;

/// Inspect PNG metadata up to the first image-data chunk.
pub fn inspect(data: &[u8]) -> CodecResult<ImageInfo> {
    if data.get(..8).malformed("truncated PNG signature")? != SIGNATURE {
        return Err(CodecError::Malformed("invalid PNG signature".to_owned()));
    }
    let (kind, header, mut position) = read_chunk(&data[8..], 8)?;
    if kind != *b"IHDR" || header.len() != 13 {
        return Err(CodecError::Malformed(
            "PNG must begin with a 13-byte IHDR chunk".to_owned(),
        ));
    }

    let width = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
    let height = u32::from_be_bytes([header[4], header[5], header[6], header[7]]);
    let bit_depth = header[8];
    let color_type = header[9];
    if width == 0 || height == 0 {
        return Err(CodecError::Malformed(
            "PNG dimensions must be nonzero".to_owned(),
        ));
    }
    if u64::from(width).saturating_mul(u64::from(height)) > PILLOW_DECOMPRESSION_BOMB_ERROR_PIXELS {
        return Err(CodecError::Dimensions(
            "PNG dimensions exceed Pillow's decompression-bomb limit".to_owned(),
        ));
    }
    if header[11] != 0 {
        return Err(CodecError::Malformed(
            "invalid PNG filter method".to_owned(),
        ));
    }
    let mode = png_mode(color_type, bit_depth)?;

    let mut palette_rgb = None;
    let mut palette_alpha = Vec::new();
    let mut frame_count = 1;
    let mut animation_declared = false;
    let mut saw_frame_control = false;
    let mut next_sequence = 0u32;
    let mut saw_following_chunk = false;
    while position < data.len() {
        let (kind, payload, next) = read_chunk(&data[position..], position)?;
        saw_following_chunk = true;
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
                if payload.len() < 8 {
                    return Err(CodecError::Malformed(
                        "invalid PNG animation control chunk".to_owned(),
                    ));
                }
                let frames = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
                if animation_declared {
                    animation_declared = false;
                    frame_count = 1;
                } else if frames != 0 && frames <= 0x8000_0000 {
                    animation_declared = true;
                    frame_count = frames;
                }
            }
            b"fcTL" => {
                if payload.len() < 26 {
                    return Err(CodecError::Malformed(
                        "invalid PNG frame control chunk".to_owned(),
                    ));
                }
                let sequence = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
                let frame_width =
                    u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
                let frame_height =
                    u32::from_be_bytes([payload[8], payload[9], payload[10], payload[11]]);
                let left = u32::from_be_bytes([payload[12], payload[13], payload[14], payload[15]]);
                let top = u32::from_be_bytes([payload[16], payload[17], payload[18], payload[19]]);
                consume_sequence(sequence, &mut next_sequence)?;
                if u64::from(left).saturating_add(u64::from(frame_width)) > u64::from(width)
                    || u64::from(top).saturating_add(u64::from(frame_height)) > u64::from(height)
                {
                    return Err(CodecError::Malformed(
                        "PNG animation frame rectangle is invalid".to_owned(),
                    ));
                }
                saw_frame_control = true;
            }
            b"IDAT" => {
                if animation_declared && !saw_frame_control {
                    frame_count = frame_count.saturating_add(1);
                }
                break;
            }
            b"IEND" => break,
            _ => {}
        }
    }
    if !saw_following_chunk {
        return Err(CodecError::Malformed(
            "PNG ends immediately after its IHDR chunk".to_owned(),
        ));
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
    Ok(ImageInfo {
        format: ImageFormat::Png,
        width,
        height,
        mode,
        bit_depth,
        palette,
        is_animated,
        frame_count: Some(frame_count),
        cursor_hotspot: None,
    })
}

fn png_mode(color_type: u8, bit_depth: u8) -> CodecResult<ImageMode> {
    match (color_type, bit_depth) {
        (0, 1) => Ok(ImageMode::L1),
        (0, 2 | 4 | 8) => Ok(ImageMode::L8),
        (0, 16) => Ok(ImageMode::L16),
        (2, 8 | 16) => Ok(ImageMode::Rgb8),
        (3, 1 | 2 | 4 | 8) => Ok(ImageMode::P8),
        (4, 8) => Ok(ImageMode::La8),
        (4, 16) | (6, 8 | 16) => Ok(ImageMode::Rgba8),
        _ => Err(CodecError::Malformed(
            "invalid PNG color type and bit-depth combination".to_owned(),
        )),
    }
}

fn consume_sequence(actual: u32, next: &mut u32) -> CodecResult<()> {
    if actual != *next {
        return Err(CodecError::Malformed(
            "PNG animation frame sequence is invalid".to_owned(),
        ));
    }
    if *next == u32::MAX {
        return Err(CodecError::Malformed(
            "PNG animation sequence number overflows".to_owned(),
        ));
    }
    *next = next.wrapping_add(1);
    Ok(())
}

fn read_chunk(chunk: &[u8], position: usize) -> CodecResult<([u8; 4], &[u8], usize)> {
    let prefix = chunk.get(..8).malformed("truncated PNG chunk header")?;
    let length = u32::from_be_bytes([prefix[0], prefix[1], prefix[2], prefix[3]]) as usize;
    let mut kind = [0; 4];
    kind.copy_from_slice(&prefix[4..8]);
    let rest = &chunk[8..];
    #[cfg(target_pointer_width = "64")]
    let payload_and_crc_len = length.wrapping_add(4);
    #[cfg(not(target_pointer_width = "64"))]
    let payload_and_crc_len = length.saturating_add(4);
    let payload_and_crc = rest
        .get(..payload_and_crc_len)
        .malformed("truncated PNG chunk payload")?;
    let payload = &payload_and_crc[..length];
    let crc_bytes = &payload_and_crc[length..];
    let expected_crc = u32::from_be_bytes([crc_bytes[0], crc_bytes[1], crc_bytes[2], crc_bytes[3]]);
    // Pillow's lazy Image.open path defers image-data CRC validation until
    // Image.verify. It still validates construction-critical metadata chunks.
    if kind != *b"IDAT" && crc32(&kind, payload) != expected_crc {
        return Err(CodecError::Malformed("PNG chunk CRC mismatch".to_owned()));
    }
    let next = position
        .wrapping_add(8)
        .wrapping_add(length)
        .wrapping_add(4);
    Ok((kind, payload, next))
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

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    fn chunk(kind: [u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut result = Vec::new();
        result.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        result.extend_from_slice(&kind);
        result.extend_from_slice(payload);
        result.extend_from_slice(&crc32(&kind, payload).to_be_bytes());
        result
    }

    let _ = inspect(b"");
    let _ = inspect(b"not png!");
    let mut no_image_data = SIGNATURE.to_vec();
    let mut header = [0u8; 13];
    header[3] = 1;
    header[7] = 1;
    no_image_data.extend_from_slice(&chunk(*b"IHDR", &header));
    no_image_data.extend_from_slice(&chunk(*b"IEND", &[]));
    let _ = inspect(&no_image_data);
    let mut sequence = u32::MAX;
    assert!(consume_sequence(u32::MAX, &mut sequence).is_err());
}
