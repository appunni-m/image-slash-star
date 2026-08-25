//! Pillow-compatible AVIF decoding with a portable closed-class fast path.

use crate::SequenceDecodeBudget;
use crate::codecs::CodecError;
use crate::codecs::CodecResult;
use crate::types::{ColorType, DecodedImage, ImageMode, SourceColor};

/// Decode the first AVIF frame to Pillow-observable 8-bit RGB or RGBA bytes.
pub fn decode(
    data: &[u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(DecodedImage, usize)> {
    crate::codecs::error::check_cancelled(token)?;
    let file_type = read_avif_file_type(data)?;
    let extracted = extract_av1(data)?;
    let validated = super::av1::validate_first(&extracted)
        .map_err(|error| error.context("AVIF AV1 validation failed"))?;
    let image = decode_portable(&validated).ok_or_else(|| {
        CodecError::NotImplemented(
            "AVIF input is outside the supported pure-Rust decode subset".to_owned(),
        )
    })?;
    let mut extracted = extracted;
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
    let item_properties = std::mem::take(&mut extracted.item_properties);
    let item_plane_properties = std::mem::take(&mut extracted.item_plane_properties);
    let item_codec_properties = std::mem::take(&mut extracted.item_codec_properties);
    let item_locations = std::mem::take(&mut extracted.item_locations);
    let grid_item_ids = std::mem::take(&mut extracted.grid_item_ids);
    let grid_properties = extracted.grid_properties;
    let transform = extracted.transform;
    let image = {
        let source = image.source.clone().with_avif_file_type(file_type);
        image.with_source_descriptor(source)
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
    let image = if item_properties.is_empty() {
        image
    } else {
        let source = image
            .source
            .clone()
            .with_avif_item_properties(item_properties);
        image.with_source_descriptor(source)
    };
    let image = if item_plane_properties.is_empty() {
        image
    } else {
        let source = image
            .source
            .clone()
            .with_avif_item_plane_properties(item_plane_properties);
        image.with_source_descriptor(source)
    };
    let image = if item_codec_properties.is_empty() {
        image
    } else {
        let source = image
            .source
            .clone()
            .with_avif_item_codec_properties(item_codec_properties);
        image.with_source_descriptor(source)
    };
    let image = {
        let source = image
            .source
            .clone()
            .with_avif_item_locations(item_locations);
        image.with_source_descriptor(source)
    };
    let image = if grid_item_ids.is_empty() {
        image
    } else {
        let source = image.source.clone().with_avif_grid_item_ids(grid_item_ids);
        image.with_source_descriptor(source)
    };
    let image = if let Some(grid_properties) = grid_properties {
        let source = image
            .source
            .clone()
            .with_avif_grid_properties(grid_properties);
        image.with_source_descriptor(source)
    } else {
        image
    };
    Ok((
        image
            .with_opaque_blocks(retained_boxes)
            .with_metadata(metadata)
            .with_source_color(source_color),
        consumed,
    ))
}

/// Decode an AVIF sequence with the pure-Rust backend.
///
/// Still-image decoding is available for the closed portable subset below,
/// while sequence timing, track references, and multi-frame presentation are
/// not implemented yet. A supported still is therefore exposed as a single
/// frame, and an input outside the pure-Rust subset returns the same explicit
/// gap rather than reaching for a foreign codec stack.
pub fn decode_sequence(
    data: &[u8],
    budget: &mut SequenceDecodeBudget,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(crate::types::DecodedSequence, usize)> {
    crate::codecs::error::check_cancelled(token)?;
    let extracted = extract_av1(data)?;
    reserve_sequence_frames(data, &extracted, budget)?;
    super::av1::validate_sequence(&extracted)
        .map_err(|error| error.context("AVIF sequence validation failed"))?;
    if extracted.sequence.is_some() {
        return Err(CodecError::NotImplemented(
            "AVIF sequence rendering is not implemented in the pure-Rust backend".to_owned(),
        ));
    }
    let (mut image, consumed) = decode(data, token)?;
    let opaque_blocks = std::mem::take(&mut image.opaque_blocks);
    let metadata = std::mem::take(&mut image.metadata);
    let source_color = std::mem::take(&mut image.source_color);
    let mut sequence = crate::types::DecodedSequence::from_image(image);
    sequence.opaque_blocks = opaque_blocks;
    sequence.metadata = metadata;
    sequence.source_color = source_color;
    Ok((sequence, consumed))
}

fn extract_av1(data: &[u8]) -> CodecResult<super::samples::ExtractedAvif<'_>> {
    super::samples::validated(data)
        .map_err(|error| error.context("AVIF container validation failed"))
}

/// Reserve every later AVIF frame before validating or eventually presenting
/// the sequence. The portable renderer is still a planned gap, but policy
/// limits must not disappear merely because presentation is not implemented.
fn reserve_sequence_frames(
    data: &[u8],
    extracted: &super::samples::ExtractedAvif<'_>,
    budget: &mut SequenceDecodeBudget,
) -> CodecResult<()> {
    if extracted.sequence.is_none() {
        return Ok(());
    }
    let info = super::inspect::inspect(data)?;
    let frame_count = info.frame_count.unwrap_or(1);
    for _ in 1..frame_count {
        budget
            .reserve_later_frame(info.mode, info.width, info.height)
            .map_err(CodecError::LimitExceeded)?;
    }
    Ok(())
}

fn read_avif_file_type(data: &[u8]) -> CodecResult<crate::types::AvifFileTypeProperties> {
    super::samples::file_type(data)
        .map_err(|error| error.context("AVIF container validation failed"))
}

/// Measure the encoded AVIF metadata extent through the same container rules.
pub(crate) fn metadata_bytes(data: &[u8]) -> CodecResult<u64> {
    super::samples::metadata_bytes(data)
        .map_err(|error| error.context("AVIF container validation failed"))
}

fn decode_portable(validated: &super::av1::ValidatedAv1) -> Option<DecodedImage> {
    let still = validated.portable_still.as_ref()?;
    let width = usize::try_from(still.width).ok()?;
    let height = usize::try_from(still.height).ok()?;
    let plane_length = width.checked_mul(height)?;
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
    let mut canvas = super::av1::FrameCanvas::new(
        still.width,
        still.height,
        still.subsampling_x,
        still.subsampling_y,
    )
    .ok()?;
    if still.width.is_multiple_of(4) && still.height.is_multiple_of(4) {
        canvas
            .place_partition_leaf(0, 0, still.width / 4, still.height / 4, &still.planes)
            .ok()?;
    } else {
        canvas
            .place_planes(still.width, still.height, &still.planes, 0, 0)
            .ok()?;
    }
    let planes = canvas.finish().ok()?;
    let [y_plane, u_plane, v_plane] = &planes;
    let chroma_width = if subsampled { width.div_ceil(2) } else { width };
    if still
        .alpha_plane
        .as_ref()
        .is_some_and(|plane| plane.samples.len() != plane_length)
    {
        return None;
    }
    let has_alpha = still.alpha_plane.is_some();
    let channel_count = if has_alpha { 4 } else { 3 };
    let pixel_capacity = plane_length.checked_mul(channel_count)?;
    let mut pixels = Vec::with_capacity(pixel_capacity);
    for (index, &y_sample) in y_plane.samples.iter().enumerate() {
        let (u_sample, v_sample) = if subsampled {
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
        let y = super::av1::truncate_to_u8(y_sample, still.bit_depth)?;
        let u = super::av1::truncate_to_u8(u_sample, still.bit_depth)?;
        let v = super::av1::truncate_to_u8(v_sample, still.bit_depth)?;
        pixels.extend_from_slice(&libyuv_bt601_full_range_rgb(y, u, v));
        if let Some(alpha_plane) = &still.alpha_plane {
            pixels.push(super::av1::truncate_to_u8(
                alpha_plane.samples[index],
                still.bit_depth,
            )?);
        }
    }
    Some(DecodedImage {
        width: still.width,
        height: still.height,
        pixels,
        color: if has_alpha {
            ColorType::Rgba8
        } else {
            ColorType::Rgb8
        },
        mode: if has_alpha {
            ImageMode::Rgba8
        } else {
            ImageMode::Rgb8
        },
        palette: None,
        cursor_hotspot: None,
        source: if has_alpha {
            crate::types::SourceDescriptor::new().with_alpha(crate::types::SourceAlpha::Auxiliary)
        } else {
            crate::types::SourceDescriptor::new()
        },
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    })
}

// ✅ VERIFIED: libyuv 1922 commit 6067afde, source/convert_argb.cc:6799-6927
// (`I420ToRGB24MatrixBilinear`) and source/scale_common.cc:572-621. The
// first and last output rows use horizontal 3:1 interpolation. Interior rows
// are produced in pairs using libyuv's four-tap 3:1/1:3 vertical and
// horizontal weights with one combined rounding operation.
fn libyuv_420_bilinear_sample(
    plane: &[u16],
    chroma_width: usize,
    width: usize,
    height: usize,
    column: usize,
    row: usize,
) -> u16 {
    let chroma_height = height.div_ceil(2);
    if plane.is_empty() || chroma_width == 0 || chroma_height == 0 || width == 0 {
        return 0;
    }

    let sample = |source_row: usize, source_column: usize| {
        let source_row = source_row.min(chroma_height.saturating_sub(1));
        let source_column = source_column.min(chroma_width.saturating_sub(1));
        plane
            .get(
                source_row
                    .saturating_mul(chroma_width)
                    .saturating_add(source_column),
            )
            .copied()
            .unwrap_or_default()
    };
    let source_column = column.saturating_sub(1).div_euclid(2);
    let next_source_column = source_column.saturating_add(1);
    // libyuv's two-row scaler starts the interpolated stream at output
    // column one. Column zero is the vertically filtered source sample at
    // column zero; thereafter odd output columns use 3:1 and even output
    // columns use 1:3 horizontal weights. Keeping that phase is observable
    // at chroma edges (and is different from a generic centered 2x filter).
    let (left_weight, right_weight) = if column == 0 {
        (4_u32, 0_u32)
    } else if column.is_multiple_of(2) {
        (1_u32, 3_u32)
    } else {
        (3_u32, 1_u32)
    };

    // `I420ToRGB24MatrixBilinear` starts with ScaleRowUp2_Linear and uses the
    // same horizontal-only operation for the final row of an even-height
    // image. This is intentionally not the generic separable boundary rule.
    if row == 0 || (height.is_multiple_of(2) && row == height.saturating_sub(1)) {
        let source_row = if row == 0 {
            0
        } else {
            chroma_height.saturating_sub(1)
        };
        let weighted = u32::from(sample(source_row, source_column))
            .saturating_mul(left_weight)
            .saturating_add(
                u32::from(sample(source_row, next_source_column)).saturating_mul(right_weight),
            );
        return u16::try_from(weighted.saturating_add(2).wrapping_shr(2)).unwrap_or(u16::MAX);
    }

    let (source_row, next_source_row, top_weight, bottom_weight) = if row.is_multiple_of(2) {
        (
            row.div_euclid(2).saturating_sub(1),
            row.div_euclid(2),
            1_u32,
            3_u32,
        )
    } else {
        (
            row.div_euclid(2),
            row.div_euclid(2).saturating_add(1),
            3_u32,
            1_u32,
        )
    };
    let weighted = u32::from(sample(source_row, source_column))
        .saturating_mul(top_weight)
        .saturating_mul(left_weight)
        .saturating_add(
            u32::from(sample(source_row, next_source_column))
                .saturating_mul(top_weight)
                .saturating_mul(right_weight),
        )
        .saturating_add(
            u32::from(sample(next_source_row, source_column))
                .saturating_mul(bottom_weight)
                .saturating_mul(left_weight),
        )
        .saturating_add(
            u32::from(sample(next_source_row, next_source_column))
                .saturating_mul(bottom_weight)
                .saturating_mul(right_weight),
        );
    u16::try_from(weighted.saturating_add(8).wrapping_shr(4)).unwrap_or(u16::MAX)
}

// ✅ VERIFIED: Pillow's libavif 1.4.1 uses libyuv 1922's JPEG-range BT.601
// I444/I420-to-RGB24 integer path for this exact output declaration.
fn libyuv_bt601_full_range_rgb(y: u8, u: u8, v: u8) -> [u8; 3] {
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

#[cfg(coverage)]
fn decode_sequence_rust_unavailable(
    data: &[u8],
    validated: &super::av1::ValidatedAv1,
    budget: &mut SequenceDecodeBudget,
    consumed: usize,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(crate::types::DecodedSequence, usize)> {
    crate::codecs::error::check_cancelled(token)?;
    let _ = data;
    let _ = validated.portable_still.as_ref();
    let _ = budget;
    let _ = consumed;
    Err(CodecError::NotImplemented(
        "AVIF sequence decoding is not implemented in the pure-Rust backend".to_owned(),
    ))
}

#[cfg(coverage)]
#[coverage(off)]
pub(crate) fn __coverage_exercise_private_branches() {
    use super::av1::{PortableStill, ValidatedAv1};

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
    let _ = metadata_bytes(b"");
    let _ = decode_sequence_rust_unavailable(
        b"not an AVIF container",
        &validated,
        &mut SequenceDecodeBudget::default_for(crate::ImageFormat::Avif),
        0,
        None,
    );
    let baseline = include_bytes!("../../../tests/fixtures/input/images/avif/baseline.avif");
    let animated = include_bytes!("../../../tests/fixtures/input/images/avif/animated.avif");
    let mut malformed_file_type = baseline.to_vec();
    malformed_file_type[8..12].copy_from_slice(b"free");
    for offset in (16..32).step_by(4) {
        malformed_file_type[offset..offset + 4].copy_from_slice(b"free");
    }
    let _ = decode(&malformed_file_type, None);
    let _ = decode_sequence(
        &malformed_file_type,
        &mut SequenceDecodeBudget::default_for(crate::ImageFormat::Avif),
        None,
    );
    let _ = read_avif_file_type(&malformed_file_type);
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

#[cfg(test)]
mod tests {
    mod sha256 {
        include!("../../../tests/support/sha256.rs");
    }

    use super::decode_portable;
    use crate::codecs::avif::av1::{PortableStill, ReconstructedPlane, ValidatedAv1};
    use crate::codecs::{CodecError, CodecResult};
    use crate::types::{ColorType, ImageMode, SourceAlpha};

    fn portable_still(alpha_plane: Option<Vec<u16>>) -> PortableStill {
        PortableStill {
            width: 4,
            height: 4,
            bit_depth: 8,
            monochrome: false,
            color_primaries: 1,
            transfer_characteristics: 13,
            matrix_coefficients: 6,
            color_range: true,
            subsampling_x: false,
            subsampling_y: false,
            planes: std::array::from_fn(|_| ReconstructedPlane {
                samples: vec![128; 16],
            }),
            alpha_plane: alpha_plane.map(|samples| ReconstructedPlane { samples }),
            #[cfg(coverage)]
            entropy_operations: Vec::new(),
        }
    }

    #[test]
    fn portable_alpha_is_composed_as_unassociated_rgba() {
        let image = decode_portable(&ValidatedAv1 {
            portable_still: Some(portable_still(Some(vec![64; 16]))),
        });
        assert!(
            image.is_some(),
            "the closed portable alpha class must materialize"
        );
        let Some(image) = image else { return };
        assert_eq!(image.color, ColorType::Rgba8);
        assert_eq!(image.mode, ImageMode::Rgba8);
        assert_eq!(image.source.alpha(), Some(SourceAlpha::Auxiliary));
        assert_eq!(image.pixels.get(..4), Some([128, 128, 128, 64].as_slice()));
        assert_eq!(image.pixels.len(), 64);
    }

    #[test]
    fn portable_alpha_requires_a_plane_for_every_color_sample() {
        let still = portable_still(Some(vec![64; 15]));
        assert!(
            decode_portable(&ValidatedAv1 {
                portable_still: Some(still),
            })
            .is_none()
        );
    }

    #[test]
    fn alpha_fixture_decodes_to_pure_rust_rgba() -> CodecResult<()> {
        let bytes = include_bytes!("../../../tests/fixtures/input/images/avif/alpha.avif");
        let extracted = super::super::samples::validated(bytes)?;
        let validated = super::super::av1::validate_first(&extracted)?;
        let still = validated
            .portable_still
            .as_ref()
            .ok_or_else(|| CodecError::NotImplemented("alpha fixture did not close".to_owned()))?;
        assert_eq!(still.width, 64);
        assert_eq!(still.height, 64);
        assert!(still.alpha_plane.as_ref().is_some_and(|plane| {
            plane.samples.len() == 64 * 64 && plane.samples.iter().any(|&sample| sample != 0)
        }));
        let image = decode_portable(&validated).ok_or_else(|| {
            CodecError::NotImplemented("color primary did not materialize".to_owned())
        })?;
        assert_eq!(image.color, ColorType::Rgba8);
        assert_eq!(image.mode, ImageMode::Rgba8);
        assert_eq!(image.source.alpha(), Some(SourceAlpha::Auxiliary));
        assert_eq!(image.pixels.len(), 64 * 64 * 4);
        assert_eq!(
            sha256::digest_hex(&image.pixels),
            "7b62ff36a098d8bdea49e0b72c834b6fd3e4e04f70d884eee274fc69ed869ddd"
        );
        assert_eq!(
            &image.pixels[..16],
            &[
                255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255
            ]
        );
        Ok(())
    }

    #[test]
    fn grid_fixture_decodes_to_pure_rust_rgba() -> CodecResult<()> {
        let bytes = include_bytes!("../../../tests/fixtures/input/images/avif/grid.avif");
        let extracted = super::super::samples::validated(bytes)?;
        let validated = super::super::av1::validate_first(&extracted)?;
        let still = validated
            .portable_still
            .as_ref()
            .ok_or_else(|| CodecError::NotImplemented("grid fixture did not close".to_owned()))?;
        assert_eq!((still.width, still.height), (80, 80));
        assert_eq!(still.bit_depth, 8);
        assert!(!still.monochrome);
        assert_eq!(
            (still.color_primaries, still.transfer_characteristics),
            (1, 13)
        );
        assert_eq!(still.matrix_coefficients, 6);
        assert!(still.color_range);
        assert!(!still.subsampling_x && !still.subsampling_y);
        assert!(still.alpha_plane.as_ref().is_some_and(|plane| {
            plane.samples.len() == 80 * 80 && plane.samples.iter().any(|&sample| sample != 0)
        }));
        let expected_raw =
            include_bytes!("../../../tests/fixtures/outputs/raws/Decode.avif_grid_avif.bin");
        let alpha = still
            .alpha_plane
            .as_ref()
            .ok_or_else(|| CodecError::NotImplemented("grid alpha did not close".to_owned()))?;
        assert!(
            alpha
                .samples
                .iter()
                .zip(expected_raw.as_chunks::<4>().0)
                .all(|(&actual, rgba)| actual == u16::from(rgba[3]))
        );
        let image = decode_portable(&validated).ok_or_else(|| {
            CodecError::NotImplemented("grid pixels did not materialize".to_owned())
        })?;
        assert_eq!(image.color, ColorType::Rgba8);
        assert_eq!(image.mode, ImageMode::Rgba8);
        assert_eq!(image.pixels.len(), 80 * 80 * 4);
        assert_eq!(image.pixels.as_slice(), &expected_raw[..]);
        assert_eq!(
            sha256::digest_hex(&image.pixels),
            "4ede0d909351b9b09c7e3e9475bfd3baf458141dbdd08f08aee19c11d2ec2983"
        );
        Ok(())
    }

    #[test]
    fn public_grid_decode_preserves_validation_inputs() -> CodecResult<()> {
        let bytes = include_bytes!("../../../tests/fixtures/input/images/avif/grid.avif");
        let (image, _) = super::decode(bytes, None)?;
        let expected =
            include_bytes!("../../../tests/fixtures/outputs/raws/Decode.avif_grid_avif.bin");
        assert_eq!(image.color, ColorType::Rgba8);
        assert_eq!(image.mode, ImageMode::Rgba8);
        assert_eq!(image.pixels.as_slice(), &expected[..]);
        assert_eq!(
            sha256::digest_hex(&image.pixels),
            "4ede0d909351b9b09c7e3e9475bfd3baf458141dbdd08f08aee19c11d2ec2983"
        );
        Ok(())
    }

    #[test]
    fn public_multitile_decode_materializes_exact_rgb() -> CodecResult<()> {
        let bytes = include_bytes!("../../../tests/fixtures/input/images/avif/multitile.avif");
        let (image, _) = super::decode(bytes, None)?;
        let expected =
            include_bytes!("../../../tests/fixtures/outputs/raws/Decode.avif_multitile_avif.bin");
        assert_eq!(image.color, ColorType::Rgb8);
        assert_eq!(image.mode, ImageMode::Rgb8);
        assert_eq!(image.pixels.as_slice(), &expected[..]);
        assert_eq!(
            sha256::digest_hex(&image.pixels),
            "8fddfd016bbc17e2f00a5154ee6e50c8f1d9d6a8254279ddf57be490ecdf9e44"
        );
        Ok(())
    }
}
