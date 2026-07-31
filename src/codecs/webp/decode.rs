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
}
