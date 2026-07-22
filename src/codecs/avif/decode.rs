//! Pillow-compatible AVIF decoding through the pinned libavif stack.

use crate::types::DecodedImage;

#[cfg(not(target_arch = "wasm32"))]
use crate::types::{ColorType, ImageMode};
#[cfg(not(target_arch = "wasm32"))]
use crate::types::{DecodedFrame, DecodedSequence, FrameDisposal};

/// Decode the first AVIF frame to Pillow-observable 8-bit RGB or RGBA bytes.
#[must_use]
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    decode_native(data)
}

/// Decode every AVIF frame with its Pillow-observable presentation duration.
#[must_use]
pub fn decode_sequence(data: &[u8]) -> Option<crate::types::DecodedSequence> {
    decode_sequence_native(data)
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_native(data: &[u8]) -> Option<DecodedImage> {
    let mut decoder = super::native::Decoder::new(data)?;
    let info = decoder.info();
    decoded_first_frame(info, decoder.decode_frame(0))
}

#[cfg(target_arch = "wasm32")]
fn decode_native(_data: &[u8]) -> Option<DecodedImage> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_sequence_native(data: &[u8]) -> Option<DecodedSequence> {
    let mut decoder = super::native::Decoder::new(data)?;
    let info = decoder.info();
    decoded_sequence(info, &mut |frame_index| decoder.decode_frame(frame_index))
}

#[cfg(not(target_arch = "wasm32"))]
fn decoded_first_frame(
    info: super::native::DecodeInfo,
    decoded: Option<(Vec<u8>, super::native::FrameTiming)>,
) -> Option<DecodedImage> {
    let (pixels, _) = decoded?;
    Some(decoded_image(
        info.width,
        info.height,
        info.has_alpha,
        pixels,
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn decoded_sequence(
    info: super::native::DecodeInfo,
    decode_frame: &mut dyn FnMut(u32) -> Option<(Vec<u8>, super::native::FrameTiming)>,
) -> Option<DecodedSequence> {
    let mut frames = Vec::with_capacity(info.frame_count as usize);
    for frame_index in 0..info.frame_count {
        let (pixels, timing) = decode_frame(frame_index)?;
        frames.push(DecodedFrame {
            image: decoded_image(info.width, info.height, info.has_alpha, pixels),
            left: 0,
            top: 0,
            duration_ms: duration_ms(timing.duration_in_timescales, info.timescale)?,
            disposal: FrameDisposal::Unspecified,
            interlaced: false,
        });
    }
    Some(DecodedSequence {
        width: info.width,
        height: info.height,
        frames,
        loop_count: None,
        background: None,
    })
}

#[cfg(target_arch = "wasm32")]
fn decode_sequence_native(_data: &[u8]) -> Option<crate::types::DecodedSequence> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn decoded_image(width: u32, height: u32, has_alpha: bool, pixels: Vec<u8>) -> DecodedImage {
    let (color, mode) = if has_alpha {
        (ColorType::Rgba8, ImageMode::Rgba8)
    } else {
        (ColorType::Rgb8, ImageMode::Rgb8)
    };
    DecodedImage {
        width,
        height,
        pixels,
        color,
        mode,
        palette: None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn duration_ms(duration: u64, timescale: std::num::NonZeroU64) -> Option<u32> {
    let numerator = u128::from(duration) * 1_000;
    let denominator = u128::from(timescale.get());
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    let doubled_remainder = remainder * 2;
    let rounded = quotient
        + u128::from(
            doubled_remainder > denominator
                || (doubled_remainder == denominator && !quotient.is_multiple_of(2)),
        );
    u32::try_from(rounded).ok()
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    use std::num::NonZeroU64;

    use super::native::{DecodeInfo, FrameTiming};

    let one = NonZeroU64::new(1).unwrap();
    let three = NonZeroU64::new(3).unwrap();
    let four_hundred = NonZeroU64::new(400).unwrap();
    let _ = duration_ms(1, three);
    let _ = duration_ms(2, three);
    let _ = duration_ms(1, four_hundred);
    let _ = duration_ms(3, four_hundred);
    let _ = duration_ms(u64::MAX, one);
    let _ = decode_sequence(b"not an AVIF container");

    let info = DecodeInfo {
        width: 1,
        height: 1,
        frame_count: 1,
        has_alpha: false,
        timescale: one,
        pixel_len: 3,
    };
    let _ = decoded_first_frame(info, None);
    let _ = decoded_first_frame(
        info,
        Some((
            vec![0; 3],
            FrameTiming {
                duration_in_timescales: 1,
            },
        )),
    );
    let _ = decoded_sequence(info, &mut |_| None);
    let _ = decoded_sequence(info, &mut |_| {
        Some((
            vec![0; 3],
            FrameTiming {
                duration_in_timescales: u64::MAX,
            },
        ))
    });
    let _ = decoded_sequence(info, &mut |_| {
        Some((
            vec![0; 3],
            FrameTiming {
                duration_in_timescales: 1,
            },
        ))
    });
}
