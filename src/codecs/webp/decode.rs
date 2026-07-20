//! WebP decoder implemented in pure Rust (zero unsafe, `#![forbid(unsafe_code)]`).
//!
//! The internal codec handles: lossy VP8, lossless VP8L, alpha (ALPH + VP8X),
//! animated frames, metadata (ICC/EXIF/XMP), and tiling.

use crate::types::{
    AnimationBackground, ColorType, DecodedFrame, DecodedImage, DecodedSequence, FrameDisposal,
};
use std::io::Cursor;

use super::native::LoopCount;

/// Decode a WebP image from raw bytes.
///
/// Returns `None` if the data is not valid WebP or if decoding fails.
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    let cursor = Cursor::new(data);

    let mut decoder = super::native::WebPDecoder::new(cursor).ok()?;
    let (width, height) = decoder.dimensions();
    let has_alpha = decoder.has_alpha();

    let buf_size = decoder.output_buffer_size()?;
    let mut pixels = vec![0u8; buf_size];
    decoder.read_image(&mut pixels).ok()?;

    let color = if has_alpha {
        ColorType::Rgba8
    } else {
        ColorType::Rgb8
    };

    Some(DecodedImage::new(width, height, pixels, color))
}

/// Decode every composited frame and its presentation timing from a WebP stream.
pub fn decode_sequence(data: &[u8]) -> Option<DecodedSequence> {
    let cursor = Cursor::new(data);
    let mut decoder = super::native::WebPDecoder::new(cursor).ok()?;
    if !decoder.is_animated() {
        return decode(data).map(DecodedSequence::from_image);
    }

    let (width, height) = decoder.dimensions();
    let color = if decoder.has_alpha() {
        ColorType::Rgba8
    } else {
        ColorType::Rgb8
    };
    let buffer_size = decoder.output_buffer_size()?;
    let frame_count = usize::try_from(decoder.num_frames()).ok()?;
    let mut frames = Vec::with_capacity(frame_count);
    for _ in 0..frame_count {
        let mut pixels = vec![0; buffer_size];
        let duration_ms = decoder.read_frame(&mut pixels).ok()?;
        frames.push(DecodedFrame {
            image: DecodedImage::new(width, height, pixels, color),
            left: 0,
            top: 0,
            duration_ms,
            disposal: FrameDisposal::Unspecified,
            interlaced: false,
        });
    }

    let loop_count = Some(match decoder.loop_count() {
        LoopCount::Forever => 0,
        LoopCount::Times(count) => u32::from(count.get()),
    });
    let background = decoder.background_color().map(AnimationBackground::Rgba);
    let sequence = DecodedSequence {
        width,
        height,
        frames,
        loop_count,
        background,
    };
    sequence.validate().ok()?;
    Some(sequence)
}
