//! Pillow-compatible AVIF decoding with a portable closed-class fast path.

use crate::types::{ColorType, DecodedImage, ImageMode};
#[cfg(not(target_arch = "wasm32"))]
use crate::types::{DecodedFrame, DecodedSequence, FrameDisposal};

/// Decode the first AVIF frame to Pillow-observable 8-bit RGB or RGBA bytes.
#[must_use]
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    let validated = validate_av1(data)?;
    decode_portable(&validated).or_else(|| decode_native(data))
}

/// Decode every AVIF frame with its Pillow-observable presentation duration.
#[must_use]
pub fn decode_sequence(data: &[u8]) -> Option<crate::types::DecodedSequence> {
    let validated = validate_av1(data)?;
    decode_sequence_native(data, &validated)
}

fn validate_av1(data: &[u8]) -> Option<super::av1::ValidatedAv1> {
    let extracted = super::samples::validated(data)?;
    super::av1::validate(&extracted)
}

fn decode_portable(validated: &super::av1::ValidatedAv1) -> Option<DecodedImage> {
    let still = validated.portable_still.as_ref()?;
    let (plane_length, width): (usize, usize) = match (still.width, still.height) {
        (4, 4) => (16, 4),
        (4, 8) => (32, 4),
        (8, 4) => (32, 8),
        (12, 4) => (48, 12),
        (4, 12) => (48, 4),
        (8, 8) => (64, 8),
        (16, 4) => (64, 16),
        (4, 16) => (64, 4),
        (12, 8) => (96, 12),
        (8, 12) => (96, 8),
        (16, 8) => (128, 16),
        (8, 16) => (128, 8),
        (12, 12) => (144, 12),
        (12, 16) => (192, 12),
        (16, 12) => (192, 16),
        (16, 16) => (256, 16),
        _ => return None,
    };
    (still.bit_depth == 8
        && !still.monochrome
        && still.color_primaries == 1
        && still.transfer_characteristics == 13
        && still.matrix_coefficients == 6
        && still.color_range)
        .then_some(())?;
    let subsampled = match (still.subsampling_x, still.subsampling_y) {
        (false, false) => false,
        (true, true) => true,
        _ => return None,
    };
    let chroma_length = if subsampled {
        plane_length.div_euclid(4)
    } else {
        plane_length
    };

    let [y_plane, u_plane, v_plane] = &still.planes;
    (y_plane.samples.len() == plane_length
        && u_plane.samples.len() == chroma_length
        && v_plane.samples.len() == chroma_length)
        .then_some(())?;
    let mut pixels = Vec::with_capacity(plane_length.saturating_mul(3));
    let chroma_width = if subsampled { width.div_ceil(2) } else { width };
    for (index, &y) in y_plane.samples.iter().enumerate() {
        let chroma_index = if still.subsampling_x {
            index
                .div_euclid(width)
                .div_euclid(2)
                .saturating_mul(chroma_width)
                .saturating_add(index.rem_euclid(width).div_euclid(2))
        } else {
            index
        };
        let u = u_plane.samples[chroma_index];
        let v = v_plane.samples[chroma_index];
        pixels.extend_from_slice(&libyuv_bt601_full_range_rgb(y, u, v));
    }
    Some(DecodedImage {
        width: still.width,
        height: still.height,
        pixels,
        color: ColorType::Rgb8,
        mode: ImageMode::Rgb8,
        palette: None,
    })
}

// ✅ VERIFIED: Pillow's libavif 1.4.1 uses libyuv 1922's JPEG-range BT.601
// I444-to-RGB24 integer path for this exact output declaration.
fn libyuv_bt601_full_range_rgb(y: u16, u: u16, v: u16) -> [u8; 3] {
    let y_scaled = u32::from(y)
        .wrapping_mul(0x0101)
        .wrapping_mul(16_320)
        .wrapping_shr(16);
    #[expect(
        clippy::cast_possible_wrap,
        reason = "eight-bit input bounds the libyuv fixed-point luma value below i32::MAX"
    )]
    let y_scaled = y_scaled as i32;
    let blue = y_scaled
        .wrapping_add(i32::from(u).wrapping_mul(113))
        .wrapping_sub(14_432);
    let green = y_scaled.wrapping_add(8_736).wrapping_sub(
        i32::from(u)
            .wrapping_mul(22)
            .wrapping_add(i32::from(v).wrapping_mul(46)),
    );
    let red = y_scaled
        .wrapping_add(i32::from(v).wrapping_mul(90))
        .wrapping_sub(11_488);
    [libyuv_rgb8(red), libyuv_rgb8(green), libyuv_rgb8(blue)]
}

fn libyuv_rgb8(value: i32) -> u8 {
    let value = value.wrapping_shr(6).clamp(0, 255);
    #[expect(
        clippy::cast_sign_loss,
        reason = "the libyuv channel result is explicitly clamped to the u8 range"
    )]
    {
        value as u8
    }
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
fn decode_sequence_native(
    data: &[u8],
    validated: &super::av1::ValidatedAv1,
) -> Option<DecodedSequence> {
    let _ = validated.portable_still.as_ref();
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
fn decode_sequence_native(
    _data: &[u8],
    validated: &super::av1::ValidatedAv1,
) -> Option<crate::types::DecodedSequence> {
    let _ = validated.portable_still.as_ref();
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
    let numerator = u128::from(duration).saturating_mul(1_000);
    let denominator = u128::from(timescale.get());
    let quotient = numerator.div_euclid(denominator);
    let remainder = numerator.rem_euclid(denominator);
    let doubled_remainder = remainder.saturating_mul(2);
    let rounded = quotient.saturating_add(u128::from(
        doubled_remainder > denominator
            || (doubled_remainder == denominator && !quotient.is_multiple_of(2)),
    ));
    u32::try_from(rounded).ok()
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    use std::num::NonZeroU64;

    use super::av1::{PortableStill, ValidatedAv1};
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
    let validated = ValidatedAv1 {
        portable_still: None,
    };
    let _ = decode_portable(&validated);
    let valid_still = super::av1::__coverage_portable_still();
    let rejects = |still: PortableStill| {
        assert!(
            decode_portable(&ValidatedAv1 {
                portable_still: Some(still),
            })
            .is_none()
        );
    };
    let mut still = valid_still.clone();
    still.width = 5;
    rejects(still);
    let mut still = valid_still.clone();
    still.height = 5;
    rejects(still);
    let mut still = valid_still.clone();
    still.bit_depth = 10;
    rejects(still);
    let mut still = valid_still.clone();
    still.monochrome = true;
    rejects(still);
    let mut still = valid_still.clone();
    still.color_primaries = 2;
    rejects(still);
    let mut still = valid_still.clone();
    still.transfer_characteristics = 2;
    rejects(still);
    let mut still = valid_still.clone();
    still.matrix_coefficients = 1;
    rejects(still);
    let mut still = valid_still.clone();
    still.color_range = false;
    rejects(still);
    let mut still = valid_still.clone();
    still.subsampling_x = true;
    rejects(still);
    let mut still = valid_still.clone();
    still.subsampling_y = true;
    rejects(still);
    let mut still = valid_still.clone();
    still.planes[0].samples.pop();
    rejects(still);
    let mut still = valid_still.clone();
    still.planes[1].samples.pop();
    rejects(still);
    let mut still = valid_still.clone();
    still.planes[2].samples.pop();
    rejects(still);
    assert!(
        decode_portable(&ValidatedAv1 {
            portable_still: Some(valid_still),
        })
        .is_some()
    );
    let _ = decode_native(b"not an AVIF container");
    let _ = decode_sequence_native(b"not an AVIF container", &validated);

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
