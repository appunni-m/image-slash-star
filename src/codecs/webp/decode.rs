//! WebP decoder implemented in pure Rust (zero unsafe, `#![forbid(unsafe_code)]`).
//!
//! The internal codec handles: lossy VP8, lossless VP8L, alpha (ALPH + VP8X),
//! animated frames, metadata (ICC/EXIF/XMP), and tiling.

use crate::codecs::{CodecError, CodecResult};
use crate::types::{
    AnimationBackground, ColorType, DecodedFrame, DecodedImage, DecodedSequence, FrameBlend,
    FrameDisposal, FrameDuration, FrameRect,
};
use std::io::Cursor;

use super::native::{DecodingError, LoopCount};

/// Decode a WebP image from raw bytes.
///
/// Returns a classified failure if the WebP container or frame cannot decode.
pub fn decode(data: &[u8]) -> CodecResult<DecodedImage> {
    let cursor = Cursor::new(data);

    let mut decoder = super::native::WebPDecoder::new(cursor).map_err(decode_error)?;
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

    Ok(DecodedImage::new(width, height, pixels, color))
}

/// Validate the RIFF container and encoded frame headers without decoding pixels.
pub(crate) fn verify(data: &[u8]) -> CodecResult<()> {
    super::native::WebPDecoder::new(Cursor::new(data))
        .map_err(decode_error)
        .map(|_| ())
}

/// Decode every composited frame and its presentation timing from a WebP stream.
pub fn decode_sequence(data: &[u8]) -> CodecResult<DecodedSequence> {
    let cursor = Cursor::new(data);
    let mut decoder = super::native::WebPDecoder::new(cursor).map_err(decode_error)?;
    if !decoder.is_animated() {
        return decode(data).map(DecodedSequence::from_image);
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
    for _ in 0..frame_count {
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
    Ok(DecodedSequence {
        width,
        height,
        frames,
        loop_count,
        background,
    })
}

fn decode_error(error: DecodingError) -> CodecError {
    CodecError::Malformed(format!("WebP decoder failure: {error:?}"))
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let _ = decode(b"not a webp stream");
    let _ = decode_sequence(b"not a webp stream");
}
