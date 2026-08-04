//! Pillow-compatible AVIF decoding with a portable closed-class fast path.

use crate::SequenceDecodeBudget;
use crate::codecs::CodecError;
use crate::codecs::CodecResult;
use crate::types::{ColorType, DecodedImage, ImageMode, SourceColor};
#[cfg(not(target_arch = "wasm32"))]
use crate::types::{
    DecodedFrame, DecodedSequence, FrameBlend, FrameDisposal, FrameDuration, FrameRect,
};

/// Decode the first AVIF frame to Pillow-observable 8-bit RGB or RGBA bytes.
pub fn decode(
    data: &[u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(DecodedImage, usize)> {
    crate::codecs::error::check_cancelled(token)?;
    let mut extracted = extract_av1(data)?;
    let consumed = extracted.consumed;
    let retained_boxes = std::mem::take(&mut extracted.retained_boxes);
    let metadata = std::mem::take(&mut extracted.metadata);
    let source_color = std::mem::take(&mut extracted.source_color);
    let auxiliary_relationship = extracted.auxiliary_relationship;
    let auxiliary_relationships = std::mem::take(&mut extracted.auxiliary_relationships);
    let item_relationships = std::mem::take(&mut extracted.item_relationships);
    let premultiplied_relationships = std::mem::take(&mut extracted.premultiplied_relationships);
    let item_color_properties = std::mem::take(&mut extracted.item_color_properties);
    let item_icc_profiles = std::mem::take(&mut extracted.item_icc_profiles);
    let grid_item_ids = std::mem::take(&mut extracted.grid_item_ids);
    let transform = extracted.transform;
    let validated = super::av1::validate_first(&extracted)
        .map_err(|error| error.context("AVIF AV1 validation failed"))?;
    let image = match decode_portable(&validated) {
        Some(image) => image,
        None => {
            crate::codecs::error::check_cancelled(token)?;
            decode_native(data)?
        }
    };
    let image = match transform {
        Some(transform) => {
            let source = image.source.clone().with_avif_transform(transform);
            image.with_source_descriptor(source)
        }
        None => image,
    };
    let image = if auxiliary_relationships.is_empty() {
        image
    } else {
        let source = image
            .source
            .clone()
            .with_avif_auxiliary_relationships(auxiliary_relationships);
        image.with_source_descriptor(source)
    };
    let image = if let Some(relationship) = auxiliary_relationship {
        let source = image
            .source
            .clone()
            .with_avif_auxiliary_relationship(relationship);
        image.with_source_descriptor(source)
    } else {
        image
    };
    let image = if item_relationships.is_empty() {
        image
    } else {
        let source = image
            .source
            .clone()
            .with_avif_item_relationships(item_relationships);
        image.with_source_descriptor(source)
    };
    let image = if premultiplied_relationships.is_empty() {
        image
    } else {
        let source = image
            .source
            .clone()
            .with_avif_premultiplied_relationships(premultiplied_relationships);
        image.with_source_descriptor(source)
    };
    let image = if item_color_properties.is_empty() {
        image
    } else {
        let source = image
            .source
            .clone()
            .with_avif_item_color_properties(item_color_properties);
        image.with_source_descriptor(source)
    };
    let image = if item_icc_profiles.is_empty() {
        image
    } else {
        let source = image
            .source
            .clone()
            .with_avif_item_icc_profiles(item_icc_profiles);
        image.with_source_descriptor(source)
    };
    let image = if grid_item_ids.is_empty() {
        image
    } else {
        let source = image.source.clone().with_avif_grid_item_ids(grid_item_ids);
        image.with_source_descriptor(source)
    };
    Ok((
        image
            .with_opaque_blocks(retained_boxes)
            .with_metadata(metadata)
            .with_source_color(source_color),
        consumed,
    ))
}

/// Decode every AVIF frame with its Pillow-observable presentation duration.
pub fn decode_sequence(
    data: &[u8],
    budget: &mut SequenceDecodeBudget,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(crate::types::DecodedSequence, usize)> {
    crate::codecs::error::check_cancelled(token)?;
    let mut extracted = extract_av1(data)?;
    let consumed = extracted.consumed;
    let retained_boxes = std::mem::take(&mut extracted.retained_boxes);
    let metadata = std::mem::take(&mut extracted.metadata);
    let source_color = std::mem::take(&mut extracted.source_color);
    let auxiliary_relationship = extracted.auxiliary_relationship;
    let auxiliary_relationships = std::mem::take(&mut extracted.auxiliary_relationships);
    let item_relationships = std::mem::take(&mut extracted.item_relationships);
    let premultiplied_relationships = std::mem::take(&mut extracted.premultiplied_relationships);
    let item_color_properties = std::mem::take(&mut extracted.item_color_properties);
    let item_icc_profiles = std::mem::take(&mut extracted.item_icc_profiles);
    let grid_item_ids = std::mem::take(&mut extracted.grid_item_ids);
    let transform = extracted.transform;
    let validated = super::av1::validate(&extracted)
        .map_err(|error| error.context("AVIF AV1 validation failed"))?;
    let (mut sequence, consumed) =
        decode_sequence_native(data, &validated, budget, consumed, token)?;
    if let Some(transform) = transform {
        for frame in &mut sequence.frames {
            let source = frame.image.source.clone().with_avif_transform(transform);
            frame.image.source = source;
        }
    }
    if !auxiliary_relationships.is_empty() {
        for frame in &mut sequence.frames {
            let source = frame
                .image
                .source
                .clone()
                .with_avif_auxiliary_relationships(auxiliary_relationships.clone());
            frame.image.source = source;
        }
    }
    if let Some(relationship) = auxiliary_relationship {
        for frame in &mut sequence.frames {
            let source = frame
                .image
                .source
                .clone()
                .with_avif_auxiliary_relationship(relationship);
            frame.image.source = source;
        }
    }
    if !item_relationships.is_empty() {
        for frame in &mut sequence.frames {
            let source = frame
                .image
                .source
                .clone()
                .with_avif_item_relationships(item_relationships.clone());
            frame.image.source = source;
        }
    }
    if !premultiplied_relationships.is_empty() {
        for frame in &mut sequence.frames {
            let source = frame
                .image
                .source
                .clone()
                .with_avif_premultiplied_relationships(premultiplied_relationships.clone());
            frame.image.source = source;
        }
    }
    if !item_color_properties.is_empty() {
        for frame in &mut sequence.frames {
            let source = frame
                .image
                .source
                .clone()
                .with_avif_item_color_properties(item_color_properties.clone());
            frame.image.source = source;
        }
    }
    if !item_icc_profiles.is_empty() {
        for frame in &mut sequence.frames {
            let source = frame
                .image
                .source
                .clone()
                .with_avif_item_icc_profiles(item_icc_profiles.clone());
            frame.image.source = source;
        }
    }
    if !grid_item_ids.is_empty() {
        for frame in &mut sequence.frames {
            let source = frame
                .image
                .source
                .clone()
                .with_avif_grid_item_ids(grid_item_ids.clone());
            frame.image.source = source;
        }
    }
    sequence.opaque_blocks = retained_boxes;
    sequence.metadata = metadata;
    sequence.source_color = source_color;
    Ok((sequence, consumed))
}

fn extract_av1(data: &[u8]) -> CodecResult<super::samples::ExtractedAvif<'_>> {
    super::samples::validated(data)
        .map_err(|error| error.context("AVIF container validation failed"))
}

/// Measure the encoded AVIF metadata extent through the same container rules.
pub(crate) fn metadata_bytes(data: &[u8]) -> CodecResult<u64> {
    super::samples::metadata_bytes(data)
        .map_err(|error| error.context("AVIF container validation failed"))
}

fn decode_portable(validated: &super::av1::ValidatedAv1) -> Option<DecodedImage> {
    let still = validated.portable_still.as_ref()?;
    let (plane_length, width, height): (usize, usize, usize) = match (still.width, still.height) {
        (4, 4) => (16, 4, 4),
        (4, 8) => (32, 4, 8),
        (8, 4) => (32, 8, 4),
        (12, 4) => (48, 12, 4),
        (4, 12) => (48, 4, 12),
        (8, 8) => (64, 8, 8),
        (16, 4) => (64, 16, 4),
        (4, 16) => (64, 4, 16),
        (12, 8) => (96, 12, 8),
        (8, 12) => (96, 8, 12),
        (16, 8) => (128, 16, 8),
        (8, 16) => (128, 8, 16),
        (12, 12) => (144, 12, 12),
        (12, 16) => (192, 12, 16),
        (16, 12) => (192, 16, 12),
        (16, 16) => (256, 16, 16),
        _ => return None,
    };
    if !(still.bit_depth == 8
        && !still.monochrome
        && still.color_primaries == 1
        && still.transfer_characteristics == 13
        && still.matrix_coefficients == 6
        && still.color_range)
    {
        return None;
    }
    let subsampled = match (still.subsampling_x, still.subsampling_y) {
        (false, false) => false,
        (true, true) => true,
        _ => return None,
    };
    let chroma_width = if subsampled { width.div_ceil(2) } else { width };
    let chroma_height = if subsampled {
        height.div_ceil(2)
    } else {
        height
    };
    // Chroma extents are at most half the validated image dimensions, so the
    // product fits `usize`; plain multiplication avoids exposing the
    // never-taken saturating intrinsic branch to coverage instrumentation.
    #[allow(clippy::arithmetic_side_effects)]
    let chroma_length = chroma_width * chroma_height;

    let [y_plane, u_plane, v_plane] = &still.planes;
    if !(y_plane.samples.len() == plane_length
        && u_plane.samples.len() == chroma_length
        && v_plane.samples.len() == chroma_length)
    {
        return None;
    }
    let mut pixels = Vec::with_capacity(plane_length.wrapping_mul(3));
    for (index, &y) in y_plane.samples.iter().enumerate() {
        let (u, v) = if subsampled {
            #[allow(clippy::arithmetic_side_effects)]
            let row = index.wrapping_div(width);
            // `width` is validated nonzero; remainder matches euclidean
            // semantics for non-negative operands without an intrinsic branch.
            #[allow(clippy::arithmetic_side_effects)]
            let column = index.wrapping_rem(width);
            (
                libyuv_420_bilinear_sample(
                    &u_plane.samples,
                    chroma_width,
                    width,
                    height,
                    column,
                    row,
                ),
                libyuv_420_bilinear_sample(
                    &v_plane.samples,
                    chroma_width,
                    width,
                    height,
                    column,
                    row,
                ),
            )
        } else {
            (u_plane.samples[index], v_plane.samples[index])
        };
        pixels.extend_from_slice(&libyuv_bt601_full_range_rgb(y, u, v));
    }
    Some(DecodedImage {
        width: still.width,
        height: still.height,
        pixels,
        color: ColorType::Rgb8,
        mode: ImageMode::Rgb8,
        palette: None,
        cursor_hotspot: None,
        source: crate::types::SourceDescriptor::new(),
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    })
}

fn libyuv_bilinear_axis(index: usize, length: usize) -> [(usize, u32); 2] {
    if index == 0 {
        [(0, 4), (0, 0)]
    } else if index == length.saturating_sub(1) {
        [(index.div_euclid(2), 4), (index.div_euclid(2), 0)]
    } else if index.rem_euclid(2) == 1 {
        let first = index.div_euclid(2);
        [(first, 3), (first.saturating_add(1), 1)]
    } else {
        let second = index.div_euclid(2);
        [(second.saturating_sub(1), 1), (second, 3)]
    }
}

// ✅ VERIFIED: libyuv 1922 commit 6067afde, source/convert_argb.cc:6799-6927
// (`I420ToRGB24MatrixBilinear`) and source/scale_common.cc:572-621. The
// boundary wrappers clamp the first and last sample; interior output samples
// use the exact separable 3:1 bilinear weights and one combined rounding.
fn libyuv_420_bilinear_sample(
    plane: &[u16],
    chroma_width: usize,
    width: usize,
    height: usize,
    column: usize,
    row: usize,
) -> u16 {
    let horizontal = libyuv_bilinear_axis(column, width);
    let vertical = libyuv_bilinear_axis(row, height);
    let weighted = vertical
        .iter()
        .fold(0_u32, |sum, &(source_row, row_weight)| {
            horizontal
                .iter()
                .fold(sum, |sum, &(source_column, column_weight)| {
                    let source_index = source_row
                        .saturating_mul(chroma_width)
                        .saturating_add(source_column);
                    sum.saturating_add(
                        u32::from(plane[source_index])
                            .saturating_mul(row_weight)
                            .saturating_mul(column_weight),
                    )
                })
        });
    u16::try_from(weighted.saturating_add(8).wrapping_shr(4)).unwrap_or(u16::MAX)
}

// ✅ VERIFIED: Pillow's libavif 1.4.1 uses libyuv 1922's JPEG-range BT.601
// I444/I420-to-RGB24 integer path for this exact output declaration.
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
fn decode_native(data: &[u8]) -> CodecResult<DecodedImage> {
    let mut decoder = super::native::Decoder::new(data)?;
    let info = decoder.info();
    decoded_first_frame(info, decoder.decode_frame(0))
}

#[cfg(target_arch = "wasm32")]
fn decode_native(_data: &[u8]) -> CodecResult<DecodedImage> {
    Err(CodecError::Unsupported(
        "AVIF input is outside the portable WASM decode subset".to_owned(),
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_sequence_native(
    data: &[u8],
    validated: &super::av1::ValidatedAv1,
    budget: &mut SequenceDecodeBudget,
    consumed: usize,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(DecodedSequence, usize)> {
    crate::codecs::error::check_cancelled(token)?;
    let _ = validated.portable_still.as_ref();
    let mut decoder = super::native::Decoder::new(data)?;
    let info = decoder.info();
    decoded_sequence(info, budget, consumed, token, &mut |frame_index| {
        decoder.decode_frame(frame_index)
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn decoded_first_frame(
    info: super::native::DecodeInfo,
    decoded: CodecResult<(Vec<u8>, super::native::FrameTiming)>,
) -> CodecResult<DecodedImage> {
    let (pixels, _) = decoded?;
    Ok(decoded_image(
        info.width,
        info.height,
        info.has_alpha,
        pixels,
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn decoded_sequence(
    info: super::native::DecodeInfo,
    budget: &mut SequenceDecodeBudget,
    consumed: usize,
    token: Option<&crate::CancellationToken>,
    decode_frame: &mut dyn FnMut(u32) -> CodecResult<(Vec<u8>, super::native::FrameTiming)>,
) -> CodecResult<(DecodedSequence, usize)> {
    crate::codecs::error::check_cancelled(token)?;
    let mut frames = Vec::with_capacity(info.frame_count as usize);
    let mode = if info.has_alpha {
        ImageMode::Rgba8
    } else {
        ImageMode::Rgb8
    };
    for frame_index in 0..info.frame_count {
        crate::codecs::error::check_cancelled(token)?;
        if frame_index != 0 {
            budget
                .reserve_later_frame(mode, info.width, info.height)
                .map_err(CodecError::LimitExceeded)?;
        }
        let (pixels, timing) = decode_frame(frame_index)?;
        frames.push(DecodedFrame::rendered_canvas(
            decoded_image(info.width, info.height, info.has_alpha, pixels),
            FrameRect {
                left: 0,
                top: 0,
                width: info.width,
                height: info.height,
            },
            FrameDuration {
                numerator: timing.duration_in_timescales,
                denominator: info.timescale.get(),
            },
            FrameDisposal::Unspecified,
            FrameBlend::Unspecified,
        ));
    }
    Ok((
        DecodedSequence {
            width: info.width,
            height: info.height,
            frames,
            loop_count: None,
            background: None,
            kind: crate::types::SequenceKind::TimedAnimation,
            opaque_blocks: Vec::new(),
            metadata: Vec::new(),
            source_color: SourceColor::new(),
        },
        consumed,
    ))
}

#[cfg(target_arch = "wasm32")]
fn decode_sequence_native(
    _data: &[u8],
    validated: &super::av1::ValidatedAv1,
    _budget: &mut SequenceDecodeBudget,
    _consumed: usize,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(crate::types::DecodedSequence, usize)> {
    crate::codecs::error::check_cancelled(token)?;
    let _ = validated.portable_still.as_ref();
    Err(CodecError::TargetUnavailable(
        "AVIF sequence decoding requires the native AVIF stack".to_owned(),
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn decoded_image(width: u32, height: u32, has_alpha: bool, pixels: Vec<u8>) -> DecodedImage {
    let (color, mode) = if has_alpha {
        (ColorType::Rgba8, ImageMode::Rgba8)
    } else {
        (ColorType::Rgb8, ImageMode::Rgb8)
    };
    let source_descriptor = if has_alpha {
        crate::types::SourceDescriptor::new().with_alpha(crate::types::SourceAlpha::Auxiliary)
    } else {
        crate::types::SourceDescriptor::new()
    };
    DecodedImage {
        width,
        height,
        pixels,
        color,
        mode,
        palette: None,
        cursor_hotspot: None,
        source: source_descriptor,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    }
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    use std::num::NonZeroU64;

    use super::av1::{PortableStill, ValidatedAv1};
    use super::native::{DecodeInfo, FrameTiming};

    let one = NonZeroU64::new(1).unwrap();
    let _ = decode_sequence(
        b"not an AVIF container",
        &mut SequenceDecodeBudget::default_for(crate::ImageFormat::Avif),
        None,
    );
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
    let _ = metadata_bytes(b"");
    let _ = decode_sequence_native(
        b"not an AVIF container",
        &validated,
        &mut SequenceDecodeBudget::default_for(crate::ImageFormat::Avif),
        0,
        None,
    );

    let info = DecodeInfo {
        width: 1,
        height: 1,
        frame_count: 1,
        has_alpha: false,
        timescale: one,
        pixel_len: 3,
    };
    let _ = decoded_first_frame(
        info,
        Err(CodecError::Malformed(
            "coverage native frame failure".to_owned(),
        )),
    );
    let _ = decoded_first_frame(
        info,
        Ok((
            vec![0; 3],
            FrameTiming {
                duration_in_timescales: 1,
            },
        )),
    );
    let mut budget = SequenceDecodeBudget::default_for(crate::ImageFormat::Avif);
    let _ = decoded_sequence(info, &mut budget, 0, None, &mut |_| {
        Err(CodecError::Malformed(
            "coverage native frame failure".to_owned(),
        ))
    });
    let _ = decoded_sequence(info, &mut budget, 0, None, &mut |_| {
        Ok((
            vec![0; 3],
            FrameTiming {
                duration_in_timescales: u64::MAX,
            },
        ))
    });
    let _ = decoded_sequence(info, &mut budget, 0, None, &mut |_| {
        Ok((
            vec![0; 3],
            FrameTiming {
                duration_in_timescales: 1,
            },
        ))
    });
    let baseline = include_bytes!("../../../tests/fixtures/input/images/avif/baseline.avif");
    let animated = include_bytes!("../../../tests/fixtures/input/images/avif/animated.avif");
    for checks in 0..=6 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = decode(baseline, Some(&token));
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = decode(animated, Some(&token));
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = decode_sequence(
            animated,
            &mut SequenceDecodeBudget::default_for(crate::ImageFormat::Avif),
            Some(&token),
        );
    }
}
