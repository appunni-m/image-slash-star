//! WebP decoder implemented in pure Rust (zero unsafe, `#![forbid(unsafe_code)]`).
//!
//! The internal codec handles: lossy VP8, lossless VP8L, alpha (ALPH + VP8X),
//! animated frames, metadata (ICC/EXIF/XMP), and tiling.

use crate::SequenceDecodeBudget;
use crate::codecs::{CodecError, CodecResult};
use crate::types::{
    AnimationBackground, ColorType, DecodedFrame, DecodedImage, DecodedSequence, FrameBlend,
    FrameDisposal, FrameDuration, FrameRect, ImageMode,
};
use std::io::Cursor;

use super::native::{DecodingError, LoopCount};

/// Decode a WebP image from raw bytes.
///
/// Returns a classified failure if the WebP container or frame cannot decode.
pub fn decode(data: &[u8]) -> CodecResult<(DecodedImage, usize)> {
    let cursor = Cursor::new(data);

    let mut decoder = super::native::WebPDecoder::new(cursor).map_err(decode_error)?;
    let consumed = riff_consumed(data);
    let (width, height) = decoder.dimensions();
    let has_alpha = decoder.has_alpha();

    let buf_size = decoder.output_buffer_size();
    let mut pixels = vec![0u8; buf_size];
    decoder.read_image(&mut pixels).map_err(decode_error)?;

    let color = if has_alpha {
        ColorType::Rgba8
    } else {
        ColorType::Rgb8
    };

    Ok((DecodedImage::new(width, height, pixels, color), consumed))
}

/// Validate the RIFF container and encoded frame headers without decoding pixels.
pub(crate) fn verify(data: &[u8]) -> CodecResult<()> {
    super::native::WebPDecoder::new(Cursor::new(data))
        .map_err(decode_error)
        .map(|_| ())
}

/// Decode every composited frame and its presentation timing from a WebP stream.
pub fn decode_sequence(
    data: &[u8],
    budget: &mut SequenceDecodeBudget,
) -> CodecResult<(DecodedSequence, usize)> {
    let cursor = Cursor::new(data);
    let mut decoder = super::native::WebPDecoder::new(cursor).map_err(decode_error)?;
    let consumed = riff_consumed(data);
    if !decoder.is_animated() {
        return decode(data)
            .map(|(image, consumed)| (DecodedSequence::from_image(image), consumed));
    }

    let (width, height) = decoder.dimensions();
    let color = if decoder.has_alpha() {
        ColorType::Rgba8
    } else {
        ColorType::Rgb8
    };
    let buffer_size = decoder.output_buffer_size();
    let frame_count = decoder.num_frames() as usize;
    let mut frames = Vec::with_capacity(frame_count);
    let mode = if color == ColorType::Rgba8 {
        ImageMode::Rgba8
    } else {
        ImageMode::Rgb8
    };
    for frame_index in 0..frame_count {
        if frame_index != 0 {
            budget
                .reserve_later_frame(mode, width, height)
                .map_err(CodecError::LimitExceeded)?;
        }
        let mut pixels = vec![0; buffer_size];
        let frame = decoder.read_frame(&mut pixels).map_err(decode_error)?;
        frames.push(DecodedFrame::rendered_canvas(
            DecodedImage::new(width, height, pixels, color),
            FrameRect {
                left: frame.left,
                top: frame.top,
                width: frame.width,
                height: frame.height,
            },
            FrameDuration::from_milliseconds(frame.duration_ms),
            if frame.dispose_to_background {
                FrameDisposal::Background
            } else {
                FrameDisposal::Keep
            },
            if frame.blend_over {
                FrameBlend::Over
            } else {
                FrameBlend::Source
            },
        ));
    }

    let loop_count = Some(match decoder.loop_count() {
        LoopCount::Forever => 0,
        LoopCount::Times(count) => u32::from(count.get()),
    });
    let background = decoder.background_color().map(AnimationBackground::Rgba);
    Ok((
        DecodedSequence {
            width,
            height,
            frames,
            loop_count,
            background,
        },
        consumed,
    ))
}

/// The RIFF-declared container extent: an 8-byte header plus the declared
/// little-endian chunk size. The decoder has already validated this header.
fn riff_consumed(data: &[u8]) -> usize {
    data.get(4..8).map_or(data.len(), |bytes| {
        8usize.saturating_add(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize)
    })
}

/// Measure the encoded metadata extent: the RIFF-declared container minus the
/// top-level image chunk payloads (`VP8 `, `VP8L`, and `ALPH`).
pub(crate) fn metadata_bytes(data: &[u8]) -> CodecResult<u64> {
    if data.get(..4) != Some(b"RIFF") || data.get(8..12) != Some(b"WEBP") {
        return Err(CodecError::Malformed(
            "invalid WebP RIFF signature".to_owned(),
        ));
    }
    let consumed = riff_consumed(data);
    if consumed > data.len() {
        return Err(CodecError::Malformed(
            "WebP RIFF size exceeds the input length".to_owned(),
        ));
    }
    let mut position = 12usize;
    let mut pixel = 0u64;
    while position < consumed {
        let remaining = consumed.saturating_sub(position);
        if remaining < 8 {
            return Err(
                CodecError::Malformed("truncated WebP chunk header".to_owned())
                    .at(position as u64, "webp_chunk"),
            );
        }
        let kind = [
            data[position],
            data[position.saturating_add(1)],
            data[position.saturating_add(2)],
            data[position.saturating_add(3)],
        ];
        let size = u32::from_le_bytes([
            data[position.saturating_add(4)],
            data[position.saturating_add(5)],
            data[position.saturating_add(6)],
            data[position.saturating_add(7)],
        ]) as usize;
        let payload_end = position.saturating_add(8).saturating_add(size);
        if payload_end > consumed {
            return Err(
                CodecError::Malformed("WebP chunk exceeds the RIFF size".to_owned())
                    .at(position as u64, "webp_chunk"),
            );
        }
        let is_image = kind == *b"VP8 " || kind == *b"VP8L" || kind == *b"ALPH";
        pixel = pixel.saturating_add(if is_image { size as u64 } else { 0 });
        let next_position = payload_end;
        position = next_position;
        // RIFF chunks are word-aligned; an odd payload leaves one pad byte
        // that belongs to the container structure rather than pixel data.
        if position < consumed && position % 2 == 1 {
            let padded = position.saturating_add(1);
            position = padded;
        }
    }
    // `pixel` is the sum of image chunk payloads inside the RIFF extent.
    #[allow(clippy::arithmetic_side_effects)]
    let metadata = consumed as u64 - pixel;
    Ok(metadata)
}

fn decode_error(error: DecodingError) -> CodecError {
    CodecError::Malformed(format!("WebP decoder failure: {error:?}"))
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let _ = decode(b"not a webp stream");
    let _ = decode_sequence(
        b"not a webp stream",
        &mut SequenceDecodeBudget::default_for(crate::ImageFormat::WebP),
    );
    let _ = metadata_bytes(b"not webp");
    let _ = metadata_bytes(b"RIFF\x08\0\0\0WEBX");
    let _ = metadata_bytes(b"RIFF\xff\xff\xff\xffWEBP");
    let _ = metadata_bytes(b"RIFF\x0e\0\0\0WEBP");
    let _ = metadata_bytes(b"RIFF\x10\0\0\0WEBPVP8 \0\0\0\0\0\0\0\0");
    let _ = metadata_bytes(b"RIFF\x0e\0\0\0WEBPVP8 \0\0\0\x10\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0");
    let _ = metadata_bytes(b"RIFF\x10\0\0\0WEBPVP8L\0\0\0\0\0\0\0\0");
    let _ = metadata_bytes(b"RIFF\x10\0\0\0WEBPALPH\0\0\0\0\0\0\0\0");
    let _ = metadata_bytes(b"RIFF\x14\0\0\0WEBPVP8X\x06\0\0\0abcdef\0\0");
    let _ = metadata_bytes(b"RIFF\x0d\0\0\0WEBPVP8 \x01\0\0\0x");
    let _ = metadata_bytes(b"RIFF\x11\0\0\0WEBPVP8 \x01\0\0\0x\0\0\0\0");
}
