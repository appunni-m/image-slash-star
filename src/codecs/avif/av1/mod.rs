//! Private, zero-copy AV1 syntax needed by the portable AVIF decoder.

mod bit_reader;
mod block;
mod cdef;
mod coefficient_cdfs;
mod entropy;
mod film_grain;
mod filter;
mod frame;
mod frame_cdfs;
mod geometry;
mod large_cdfs;
mod mc;
mod motion;
mod quantization;
mod quantization_matrices;
mod raster;
mod resize;
mod restoration;
pub(super) mod sample_depth;
mod sequence;
mod surface;
mod tile_state;
mod transform;

pub(super) use sample_depth::truncate_to_u8;

use self::bit_reader::SegmentedData;
#[cfg(test)]
pub(super) use self::block::ReconstructedPlane;
use self::frame::{DisplayGeometryProof, FrameState};
pub(super) use self::raster::FrameCanvas;
#[cfg(coverage)]
use super::samples::ByteSpan;
use super::samples::{EncodedPlane, EncodedSample, ExtractedAvif};
#[cfg(coverage)]
use super::samples::{SequencePayload, StillPayload};
use crate::codecs::{CodecError, CodecResult};
#[cfg(coverage)]
use std::num::NonZeroU32;

const MAX_OBUS_PER_SAMPLE: usize = 4_096;

type Av1Result<T> = CodecResult<T>;

fn malformed(stage: &'static str) -> CodecError {
    CodecError::Malformed(format!("invalid AV1 bitstream: {stage}"))
}

/// One complete still-image class that the portable decoder can materialize.
#[derive(Clone)]
pub(super) struct PortableStill {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) bit_depth: u32,
    pub(super) monochrome: bool,
    pub(super) color_primaries: u32,
    pub(super) transfer_characteristics: u32,
    pub(super) matrix_coefficients: u32,
    pub(super) color_range: bool,
    pub(super) subsampling_x: bool,
    pub(super) subsampling_y: bool,
    /// AV1 frame-ID syntax is retained so first-frame APIs can distinguish
    /// an error-resilient movie from the ordinary multi-frame gap.
    pub(super) frame_id_numbers_present: bool,
    pub(super) planes: [block::ReconstructedPlane; 3],
    /// A validated monochrome auxiliary plane for the narrow composition
    /// class. Unsupported alpha syntax never becomes a silent RGB decode.
    pub(super) alpha_plane: Option<block::ReconstructedPlane>,
    #[cfg(coverage)]
    pub(super) entropy_operations: Vec<crate::Av1EntropyOperationState>,
}

/// AV1 syntax accepted by the production parser, retaining a complete
/// portable still only when container and codec state prove that class.
pub(super) struct ValidatedAv1 {
    pub(super) portable_still: Option<PortableStill>,
}

struct ValidatedPlane {
    first_leaf: Option<block::FirstLeaf>,
    complete_monochrome_plane: Option<block::ReconstructedPlane>,
    sequence: sequence::SequenceHeader,
    frame_dimensions: Option<(u32, u32)>,
    display_geometry: Option<DisplayGeometryProof>,
}

struct MonochromeGridCell {
    width: u32,
    height: u32,
    luma: block::ReconstructedPlane,
    alpha: Option<block::ReconstructedPlane>,
}

// ✅ VERIFIED: AV1 specification sections 5.3.2-5.3.3; dav1d 1.5.3
// src/getbits.c:95-112 and src/obu.c:1169-1195.
fn read_uleb128(data: &SegmentedData<'_, '_>, offset: &mut usize) -> Av1Result<u32> {
    let mut value = 0_u64;
    for index in 0..8_u32 {
        let byte = data.byte(*offset)?;
        *offset = offset.saturating_add(1);
        value |= u64::from(byte & 0x7f) << index.saturating_mul(7);
        if byte & 0x80 == 0 {
            return u32::try_from(value).map_err(|_| malformed("ULEB128 value exceeds u32"));
        }
    }
    Err(malformed("ULEB128 value exceeds eight bytes"))
}

// ✅ VERIFIED: AV1 specification sections 5.3.1-5.3.3 and 6.2.2; dav1d
// 1.5.3 src/obu.c:1169-1209.
fn validate_sample(input: &[u8], sample: &EncodedSample, state: &mut FrameState) -> Av1Result<()> {
    validate_sample_and_return_temporal_unit(input, sample, state).map(|_| ())
}

fn validate_sample_and_return_temporal_unit(
    input: &[u8],
    sample: &EncodedSample,
    state: &mut FrameState,
) -> Av1Result<u64> {
    let data = SegmentedData::new(input, &sample.spans)?;
    // The AVIF sample extractor constructs codec-configuration spans only
    // after validating them against the immutable input buffer.
    let config = &input[sample.config.start..sample.config.end];
    let mut offset = 0_usize;
    let mut obu_count = 0_usize;
    let mut frame_bearing = false;
    while offset < data.len() {
        obu_count = obu_count.saturating_add(1);
        if obu_count > MAX_OBUS_PER_SAMPLE {
            return Err(malformed("sample contains too many OBUs"));
        }
        // The loop condition proves the logical OBU header byte is present.
        let header = data.validated_byte(offset);
        offset = offset.saturating_add(1);
        if header & 0x80 != 0 || header & 1 != 0 {
            return Err(malformed("OBU header reserved bits are set"));
        }
        let obu_type = (header >> 3) & 0x0f;
        let has_extension = header & 4 != 0;
        let has_size_field = header & 2 != 0;
        let mut temporal_id = 0_u32;
        let mut spatial_id = 0_u32;
        if has_extension {
            let extension = data.byte(offset)?;
            offset = offset.saturating_add(1);
            if extension & 7 != 0 {
                return Err(malformed("OBU extension reserved bits are set"));
            }
            temporal_id = u32::from(extension >> 5);
            spatial_id = u32::from((extension >> 3) & 3);
        }
        if !has_size_field {
            return Err(malformed("OBU omits its payload-size field"));
        }
        let payload_size = read_uleb128(&data, &mut offset)? as usize;
        let payload_start = offset;
        let remaining = data.len().saturating_sub(payload_start);
        if payload_size > remaining {
            return Err(malformed("OBU payload exceeds its sample"));
        }
        let payload_end = payload_start.saturating_add(payload_size);
        if matches!(obu_type, 1 | 2 | 8) && has_extension {
            return Err(malformed("non-layer-specific OBU has an extension header"));
        }
        if matches!(obu_type, 3 | 4 | 6 | 7) {
            frame_bearing = true;
            if !state.admits_layer_specific_obu(has_extension, temporal_id, spatial_id)? {
                // Membership is decided only after validating the complete
                // OBU envelope. An ignored layer must leave decoder state
                // exactly untouched.
                offset = payload_end;
                continue;
            }
        }
        match obu_type {
            1 => {
                let sequence = sequence::parse(&data, payload_start, payload_end)?;
                if !sequence.matches_config(config) {
                    return Err(malformed(
                        "sequence header disagrees with the AV1 codec configuration",
                    ));
                }
                state.accept_sequence(sequence)?;
            }
            2 => state.temporal_delimiter()?,
            3 => {
                state.frame_header_obu(
                    &data,
                    payload_start..payload_end,
                    has_extension,
                    temporal_id,
                    spatial_id,
                    false,
                )?;
            }
            4 => {
                state.tile_group_obu(
                    &data,
                    payload_start,
                    payload_end,
                    has_extension,
                    temporal_id,
                    spatial_id,
                )?;
            }
            6 => {
                state.frame_obu(
                    &data,
                    payload_start,
                    payload_end,
                    has_extension,
                    temporal_id,
                    spatial_id,
                )?;
            }
            7 => {
                state.frame_header_obu(
                    &data,
                    payload_start..payload_end,
                    has_extension,
                    temporal_id,
                    spatial_id,
                    true,
                )?;
            }
            _ => {}
        }
        offset = payload_end;
    }
    if !frame_bearing {
        return Err(malformed("sample contains no frame-bearing OBU"));
    }
    state.sample_flush()
}

fn validate_plane_state(input: &[u8], plane: &EncodedPlane) -> Av1Result<FrameState> {
    let mut state = FrameState::new();
    for sample in &plane.samples {
        validate_sample(input, sample, &mut state)?;
    }
    state.finish()?;
    Ok(state)
}

/// Validate and materialize every displayable sample in one AVIF track.
///
/// A movie sample is a temporal unit, not necessarily a complete intra frame.
/// Keeping one [`FrameState`] alive across the loop preserves reference
/// surfaces, frame IDs, CDFs, and segmentation state while the returned
/// displays remain owned snapshots suitable for the public sequence API.
pub(super) fn validate_sequence_frames(
    extracted: &ExtractedAvif<'_>,
    token: Option<&crate::CancellationToken>,
) -> Av1Result<Option<Vec<PortableStill>>> {
    let Some(sequence) = &extracted.sequence else {
        return Ok(None);
    };
    if sequence.color.samples.is_empty() {
        return Err(malformed("AVIF sequence has no color samples"));
    }

    let mut color_state = FrameState::new();
    let mut color_displays = Vec::new();
    color_displays
        .try_reserve(sequence.color.samples.len())
        .map_err(|_| {
            CodecError::Dimensions("unable to reserve AVIF color sequence state".to_owned())
        })?;
    for (sample_index, sample) in sequence.color.samples.iter().enumerate() {
        crate::codecs::error::check_cancelled(token)?;
        let temporal_unit =
            validate_sample_and_return_temporal_unit(extracted.input, sample, &mut color_state)?;
        let display =
            color_state.selected_display_for_temporal_unit_with_token(temporal_unit, token)?;
        color_displays.push(display);
        if sample_index == 0 {
            let first_sequence = color_state.finish()?;
            // Reject an unsupported color profile before validating later
            // reference samples. This keeps a known pure-Rust capability gap
            // typed as Unsupported instead of exposing an incidental missing
            // reference surface from the next temporal unit.
            if !first_sequence.monochrome && !portable_color_sequence_supported(first_sequence) {
                return Ok(None);
            }
            // Ordinary multi-frame AVIF presentation still lacks the
            // reference and timing contract. Keep that gap typed even when a
            // later sample would otherwise fail during partial validation.
            if sequence.color.samples.len() > 1 && !first_sequence.frame_id_numbers_present {
                return Ok(None);
            }
        }
    }
    let color_sequence = color_state.finish()?.clone();

    let mut alpha_displays = None;
    let mut alpha_sequence = None;
    if let Some(alpha) = &sequence.alpha {
        let mut alpha_state = FrameState::new();
        let mut displays = Vec::new();
        displays.try_reserve(alpha.samples.len()).map_err(|_| {
            CodecError::Dimensions("unable to reserve AVIF alpha sequence state".to_owned())
        })?;
        for sample in &alpha.samples {
            crate::codecs::error::check_cancelled(token)?;
            let temporal_unit = validate_sample_and_return_temporal_unit(
                extracted.input,
                sample,
                &mut alpha_state,
            )?;
            let display =
                alpha_state.selected_display_for_temporal_unit_with_token(temporal_unit, token)?;
            displays.push(display);
        }
        alpha_sequence = Some(alpha_state.finish()?.clone());
        alpha_displays = Some(displays);
    }

    if alpha_displays
        .as_ref()
        .is_some_and(|displays| displays.len() != color_displays.len())
    {
        return Ok(None);
    }
    if let Some(alpha_sequence) = &alpha_sequence
        && !monochrome_alpha_sequence_supported(alpha_sequence, color_sequence.bit_depth)
    {
        return Ok(None);
    }

    let mut frames = Vec::new();
    frames.try_reserve(color_displays.len()).map_err(|_| {
        CodecError::Dimensions("unable to reserve AVIF decoded sequence frames".to_owned())
    })?;
    if color_sequence.monochrome {
        if !monochrome_primary_sequence_supported(&color_sequence) {
            return Ok(None);
        }
        for display in &mut color_displays {
            let Some(mut display) = display.take() else {
                return Ok(None);
            };
            let Some(dimensions) = monochrome_display_dimensions(&display) else {
                return Ok(None);
            };
            let Some(color_plane) = display.monochrome_plane.take() else {
                return Ok(None);
            };
            let alpha_plane = if let Some(alpha_displays) = alpha_displays.as_mut() {
                let Some(mut alpha_display) =
                    alpha_displays.get_mut(frames.len()).and_then(Option::take)
                else {
                    return Ok(None);
                };
                let Some(alpha_dimensions) = monochrome_display_dimensions(&alpha_display) else {
                    return Ok(None);
                };
                if alpha_dimensions != dimensions {
                    return Ok(None);
                }
                let Some(alpha_plane) = alpha_display.monochrome_plane.take() else {
                    return Ok(None);
                };
                Some(alpha_plane)
            } else {
                None
            };
            frames.push(portable_monochrome_still(
                color_plane,
                dimensions,
                color_sequence.clone(),
                alpha_plane,
            ));
        }
        return Ok(Some(frames));
    }
    if !portable_color_sequence_supported(&color_sequence) {
        return Ok(None);
    }
    for (index, display) in color_displays.iter_mut().enumerate() {
        let Some(display) = display.take() else {
            return Ok(None);
        };
        let Some(color_leaf) = display.color_leaf else {
            return Ok(None);
        };
        if display.dimensions != Some((color_leaf.width, color_leaf.height)) {
            return Ok(None);
        }
        let alpha_plane = if let Some(alpha_displays) = alpha_displays.as_mut() {
            let Some(alpha_display) = alpha_displays.get_mut(index).and_then(Option::take) else {
                return Ok(None);
            };
            if monochrome_display_dimensions(&alpha_display)
                != Some((color_leaf.width, color_leaf.height))
            {
                return Ok(None);
            };
            let Some(alpha_plane) = alpha_display.monochrome_plane else {
                return Ok(None);
            };
            Some(alpha_plane)
        } else {
            None
        };
        frames.push(portable_still(
            color_leaf,
            color_sequence.clone(),
            alpha_plane,
        ));
    }
    Ok(Some(frames))
}

#[allow(
    dead_code,
    reason = "compatibility wrapper for token-aware production validation"
)]
fn validate_plane(input: &[u8], plane: &EncodedPlane) -> Av1Result<ValidatedPlane> {
    validate_plane_with_token(input, plane, None)
}

fn validate_plane_with_token(
    input: &[u8],
    plane: &EncodedPlane,
    token: Option<&crate::CancellationToken>,
) -> Av1Result<ValidatedPlane> {
    let state = validate_plane_state(input, plane)?;
    let sequence = state.finish()?.clone();
    let selected = state.selected_display_with_token(token)?;
    Ok(ValidatedPlane {
        first_leaf: selected.color_leaf,
        complete_monochrome_plane: selected.monochrome_plane,
        sequence,
        frame_dimensions: selected.dimensions,
        display_geometry: selected.geometry,
    })
}

fn monochrome_primary_sequence_supported(sequence: &sequence::SequenceHeader) -> bool {
    sequence.monochrome
        && matches!(sequence.bit_depth, 8 | 10 | 12)
        && sequence.color_range
        && (
            sequence.color_primaries,
            sequence.transfer_characteristics,
            sequence.matrix_coefficients,
        ) == (1, 13, 6)
        && (sequence.subsampling_x, sequence.subsampling_y) == (true, true)
        && !sequence.film_grain_present
}

/// Color formats that have a checked RGB8 boundary in `decode_portable`.
///
/// Keep this sequence-level admission in the AV1 module independent from the
/// private decoder matrix enum. Every returned frame still goes through the
/// final conversion gate, while this predicate prevents a stateful track from
/// being rejected before that per-frame path can run.
fn portable_color_sequence_supported(sequence: &sequence::SequenceHeader) -> bool {
    if sequence.monochrome
        || !sequence.color_range
        || (!sequence.subsampling_x && sequence.subsampling_y)
    {
        return false;
    }
    matches!(
        (
            (
                sequence.color_primaries,
                sequence.transfer_characteristics,
                sequence.matrix_coefficients,
            ),
            sequence.bit_depth,
        ),
        ((1, 13, 6), 8 | 10 | 12)
    )
}

fn monochrome_alpha_sequence_supported(
    sequence: &sequence::SequenceHeader,
    bit_depth: u32,
) -> bool {
    sequence.monochrome
        && matches!(sequence.bit_depth, 8 | 10 | 12)
        && sequence.color_range
        && sequence.bit_depth == bit_depth
        && (sequence.subsampling_x, sequence.subsampling_y) == (true, true)
        && !sequence.film_grain_present
}

fn monochrome_plane_dimensions(
    display_geometry: Option<DisplayGeometryProof>,
    frame_dimensions: Option<(u32, u32)>,
    plane: Option<&block::ReconstructedPlane>,
) -> Option<(u32, u32)> {
    let geometry = display_geometry?;
    if geometry.superres_enabled {
        return None;
    }
    let dimensions = frame_dimensions?;
    if dimensions.0 == 0
        || dimensions.1 == 0
        || geometry.render_width != dimensions.0
        || geometry.render_height != dimensions.1
    {
        return None;
    }
    let width = usize::try_from(dimensions.0).ok()?;
    let height = usize::try_from(dimensions.1).ok()?;
    let sample_count = width.checked_mul(height)?;
    plane.filter(|plane| plane.samples.len() == sample_count)?;
    Some(dimensions)
}

fn monochrome_display_dimensions(display: &frame::SelectedDisplay) -> Option<(u32, u32)> {
    if display.color_leaf.is_some() {
        return None;
    }
    monochrome_plane_dimensions(
        display.geometry,
        display.dimensions,
        display.monochrome_plane.as_ref(),
    )
}

fn monochrome_primary_dimensions(plane: &ValidatedPlane) -> Option<(u32, u32)> {
    if !monochrome_primary_sequence_supported(&plane.sequence) {
        return None;
    }
    monochrome_plane_dimensions(
        plane.display_geometry,
        plane.frame_dimensions,
        plane.complete_monochrome_plane.as_ref(),
    )
}

fn monochrome_alpha_matches(
    plane: &ValidatedPlane,
    dimensions: (u32, u32),
    bit_depth: u32,
) -> bool {
    if !monochrome_alpha_sequence_supported(&plane.sequence, bit_depth)
        || plane.frame_dimensions != Some(dimensions)
    {
        return false;
    }
    monochrome_plane_dimensions(
        plane.display_geometry,
        plane.frame_dimensions,
        plane.complete_monochrome_plane.as_ref(),
    ) == Some(dimensions)
}

fn portable_still(
    leaf: block::FirstLeaf,
    sequence: sequence::SequenceHeader,
    alpha_plane: Option<block::ReconstructedPlane>,
) -> PortableStill {
    PortableStill {
        width: leaf.width,
        height: leaf.height,
        bit_depth: sequence.bit_depth,
        monochrome: sequence.monochrome,
        color_primaries: sequence.color_primaries,
        transfer_characteristics: sequence.transfer_characteristics,
        matrix_coefficients: sequence.matrix_coefficients,
        color_range: sequence.color_range,
        subsampling_x: sequence.subsampling_x,
        subsampling_y: sequence.subsampling_y,
        frame_id_numbers_present: sequence.frame_id_numbers_present,
        planes: leaf.planes,
        alpha_plane,
        #[cfg(coverage)]
        entropy_operations: leaf.entropy_operations,
    }
}

fn portable_monochrome_still(
    plane: block::ReconstructedPlane,
    dimensions: (u32, u32),
    sequence: sequence::SequenceHeader,
    alpha_plane: Option<block::ReconstructedPlane>,
) -> PortableStill {
    let empty = block::ReconstructedPlane {
        samples: Vec::new(),
    };
    PortableStill {
        width: dimensions.0,
        height: dimensions.1,
        bit_depth: sequence.bit_depth,
        monochrome: true,
        color_primaries: sequence.color_primaries,
        transfer_characteristics: sequence.transfer_characteristics,
        matrix_coefficients: sequence.matrix_coefficients,
        color_range: sequence.color_range,
        subsampling_x: sequence.subsampling_x,
        subsampling_y: sequence.subsampling_y,
        frame_id_numbers_present: sequence.frame_id_numbers_present,
        planes: [plane, empty.clone(), empty],
        alpha_plane,
        #[cfg(coverage)]
        entropy_operations: Vec::new(),
    }
}

fn assembled_leaf(
    width: u32,
    height: u32,
    planes: [block::ReconstructedPlane; 3],
) -> block::FirstLeaf {
    block::FirstLeaf {
        width,
        height,
        block_skipped: false,
        planes,
        luma_predictor: block::LumaPredictor::Dc,
        chroma_predictor: None,
        luma_context: 0x40,
        chroma_contexts: [0x40; 2],
        chroma_right_contexts: [[0x40; 16]; 2],
        chroma_bottom_contexts: [[0x40; 16]; 2],
        tx_context_width: 0,
        tx_context_height: 0,
        luma_transform_split: false,
        luma_right_contexts: [0x40; 16],
        luma_bottom_contexts: [0x40; 16],
        wide_coefficient_contexts: None,
        palette_cache: Default::default(),
        #[cfg(coverage)]
        entropy_operations: Vec::new(),
    }
}

/// Validate and assemble a bounded AVIF `grid` item.
///
/// Each derived item is decoded as an independent still image. The complete
/// coded cell is retained, then only the declared top-left visible rectangle
/// is copied into a checked output canvas. This keeps grid composition free of
/// native state and makes malformed overlap, gaps, and auxiliary geometry
/// explicit errors rather than partially published pixels.
#[allow(
    dead_code,
    reason = "compatibility wrapper for token-aware production validation"
)]
fn validate_grid(
    extracted: &ExtractedAvif<'_>,
    still: &super::samples::StillPayload,
) -> Av1Result<Option<PortableStill>> {
    validate_grid_with_token(extracted, still, None)
}

/// Validate and assemble a grid whose cells are complete monochrome AV1
/// displays. Grid-level cropping is performed by the checked monochrome
/// canvases; per-cell display transforms remain unsupported by the narrow
/// portable admission.
fn validate_monochrome_grid_with_token(
    extracted: &ExtractedAvif<'_>,
    still: &super::samples::StillPayload,
    token: Option<&crate::CancellationToken>,
    properties: crate::types::AvifGridProperties,
    columns: usize,
    cell_count: usize,
    first_color: ValidatedPlane,
) -> Av1Result<Option<PortableStill>> {
    if first_color.first_leaf.is_some() {
        return Ok(None);
    }
    let Some(first_dimensions) = monochrome_primary_dimensions(&first_color) else {
        return Ok(None);
    };
    let first_sequence = first_color.sequence.clone();
    let first_geometry = first_color.display_geometry;
    let mut first_color = Some(first_color);

    let mut cells = Vec::new();
    cells.try_reserve(cell_count).map_err(|_| {
        CodecError::Dimensions("unable to reserve AVIF monochrome grid cells".to_owned())
    })?;
    for index in 0..cell_count {
        crate::codecs::error::check_cancelled(token)?;
        let mut color = if index == 0 {
            first_color
                .take()
                .ok_or_else(|| malformed("AVIF grid first cell was consumed"))?
        } else {
            let sample = still
                .color
                .samples
                .get(index)
                .ok_or_else(|| malformed("AVIF grid color sample is missing"))?;
            validate_plane_with_token(
                extracted.input,
                &super::samples::EncodedPlane {
                    samples: vec![sample.clone()],
                },
                token,
            )?
        };
        if color.first_leaf.is_some() {
            return Ok(None);
        }
        let Some(dimensions) = monochrome_primary_dimensions(&color) else {
            return Ok(None);
        };
        if dimensions != first_dimensions
            || color.display_geometry != first_geometry
            || color.sequence.bit_depth != first_sequence.bit_depth
            || color.sequence.monochrome != first_sequence.monochrome
            || color.sequence.color_primaries != first_sequence.color_primaries
            || color.sequence.transfer_characteristics != first_sequence.transfer_characteristics
            || color.sequence.matrix_coefficients != first_sequence.matrix_coefficients
            || color.sequence.color_range != first_sequence.color_range
            || color.sequence.subsampling_x != first_sequence.subsampling_x
            || color.sequence.subsampling_y != first_sequence.subsampling_y
        {
            return Err(malformed(
                "AVIF grid cells disagree on decoded monochrome format",
            ));
        }
        let Some(luma) = color.complete_monochrome_plane.take() else {
            return Ok(None);
        };
        let alpha = if let Some(alpha_track) = &still.alpha {
            let alpha_sample = alpha_track
                .samples
                .get(index)
                .ok_or_else(|| malformed("AVIF grid alpha sample is missing"))?;
            let mut alpha = validate_plane_with_token(
                extracted.input,
                &super::samples::EncodedPlane {
                    samples: vec![alpha_sample.clone()],
                },
                token,
            )?;
            if alpha.first_leaf.is_some()
                || alpha.display_geometry != color.display_geometry
                || !monochrome_alpha_matches(&alpha, dimensions, first_sequence.bit_depth)
            {
                return Ok(None);
            }
            let Some(alpha) = alpha.complete_monochrome_plane.take() else {
                return Ok(None);
            };
            Some(alpha)
        } else {
            None
        };
        cells.push(MonochromeGridCell {
            width: dimensions.0,
            height: dimensions.1,
            luma,
            alpha,
        });
    }

    let first = cells
        .first()
        .ok_or_else(|| malformed("AVIF grid has no monochrome cells"))?;
    let cell_width = first.width;
    let cell_height = first.height;
    let output_width = properties.output_width();
    let output_height = properties.output_height();
    if cell_width == 0 || cell_height == 0 || output_width == 0 || output_height == 0 {
        return Err(malformed(
            "AVIF monochrome grid has an empty cell or output canvas",
        ));
    }
    let total_width = cell_width
        .checked_mul(properties.columns())
        .ok_or_else(|| malformed("AVIF monochrome grid width overflows"))?;
    let total_height = cell_height
        .checked_mul(properties.rows())
        .ok_or_else(|| malformed("AVIF monochrome grid height overflows"))?;
    if total_width < output_width || total_height < output_height {
        return Err(malformed(
            "AVIF monochrome grid cells do not cover the output canvas",
        ));
    }
    let last_column = properties
        .columns()
        .checked_sub(1)
        .ok_or_else(|| malformed("AVIF monochrome grid has no columns"))?;
    let last_row = properties
        .rows()
        .checked_sub(1)
        .ok_or_else(|| malformed("AVIF monochrome grid has no rows"))?;
    if cell_width
        .checked_mul(last_column)
        .ok_or_else(|| malformed("AVIF monochrome grid last column overflows"))?
        >= output_width
        || cell_height
            .checked_mul(last_row)
            .ok_or_else(|| malformed("AVIF monochrome grid last row overflows"))?
            >= output_height
    {
        return Err(malformed(
            "AVIF monochrome grid has an invisible final row or column",
        ));
    }

    let mut luma_canvas = raster::MonochromeFrameCanvas::new(output_width, output_height)?;
    let mut alpha_canvas = if first.alpha.is_some() {
        Some(raster::MonochromeFrameCanvas::new(
            output_width,
            output_height,
        )?)
    } else {
        None
    };
    for (index, cell) in cells.iter().enumerate() {
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "cell_count is nonzero above, so the validated grid column count cannot be zero"
        )]
        let (row, column) = (index / columns, index % columns);
        let x = u32::try_from(
            column
                .checked_mul(
                    usize::try_from(cell_width)
                        .map_err(|_| malformed("AVIF monochrome cell width exceeds usize"))?,
                )
                .ok_or_else(|| malformed("AVIF monochrome grid x origin overflows usize"))?,
        )
        .map_err(|_| malformed("AVIF monochrome grid x origin exceeds u32"))?;
        let y = u32::try_from(
            row.checked_mul(
                usize::try_from(cell_height)
                    .map_err(|_| malformed("AVIF monochrome cell height exceeds usize"))?,
            )
            .ok_or_else(|| malformed("AVIF monochrome grid y origin overflows usize"))?,
        )
        .map_err(|_| malformed("AVIF monochrome grid y origin exceeds u32"))?;
        let visible_width = output_width
            .checked_sub(x)
            .ok_or_else(|| malformed("AVIF monochrome cell starts outside output width"))?
            .min(cell.width);
        let visible_height = output_height
            .checked_sub(y)
            .ok_or_else(|| malformed("AVIF monochrome cell starts outside output height"))?
            .min(cell.height);
        if visible_width == 0 || visible_height == 0 {
            return Err(malformed(
                "AVIF monochrome grid cell has no visible samples",
            ));
        }
        luma_canvas.place_cropped_plane(
            cell.width,
            cell.height,
            visible_width,
            visible_height,
            x,
            y,
            &cell.luma,
        )?;
        if let (Some(alpha_canvas), Some(alpha)) = (alpha_canvas.as_mut(), cell.alpha.as_ref()) {
            alpha_canvas.place_cropped_plane(
                cell.width,
                cell.height,
                visible_width,
                visible_height,
                x,
                y,
                alpha,
            )?;
        }
    }

    let sample_depth = sample_depth::SampleDepth::new(first_sequence.bit_depth)
        .ok_or_else(|| malformed("AVIF monochrome grid sample depth is unsupported"))?;
    let luma = luma_canvas.finish(sample_depth)?;
    let alpha = alpha_canvas
        .map(|canvas| canvas.finish(sample_depth))
        .transpose()?;
    Ok(Some(portable_monochrome_still(
        luma,
        (output_width, output_height),
        first_sequence,
        alpha,
    )))
}

fn validate_grid_with_token(
    extracted: &ExtractedAvif<'_>,
    still: &super::samples::StillPayload,
    token: Option<&crate::CancellationToken>,
) -> Av1Result<Option<PortableStill>> {
    let Some(properties) = extracted.grid_properties else {
        return Ok(None);
    };
    let rows = usize::try_from(properties.rows())
        .map_err(|_| malformed("AVIF grid row count exceeds usize"))?;
    let columns = usize::try_from(properties.columns())
        .map_err(|_| malformed("AVIF grid column count exceeds usize"))?;
    let cell_count = rows
        .checked_mul(columns)
        .ok_or_else(|| malformed("AVIF grid cell count overflows usize"))?;
    if cell_count == 0
        || extracted.grid_item_ids.len() != cell_count
        || still.color.samples.len() != cell_count
    {
        return Ok(None);
    }
    if let Some(alpha) = &still.alpha
        && alpha.samples.len() != cell_count
    {
        return Ok(None);
    }

    let first_sample = still
        .color
        .samples
        .first()
        .ok_or_else(|| malformed("AVIF grid has no color samples"))?;
    let mut first_color = Some(validate_plane_with_token(
        extracted.input,
        &super::samples::EncodedPlane {
            samples: vec![first_sample.clone()],
        },
        token,
    )?);
    if first_color
        .as_ref()
        .is_some_and(|color| color.sequence.monochrome)
    {
        return validate_monochrome_grid_with_token(
            extracted,
            still,
            token,
            properties,
            columns,
            cell_count,
            first_color
                .take()
                .ok_or_else(|| malformed("AVIF grid first cell was consumed"))?,
        );
    }

    let mut cells = Vec::with_capacity(cell_count);
    for (index, sample) in still.color.samples.iter().enumerate() {
        let color = if index == 0 {
            first_color
                .take()
                .ok_or_else(|| malformed("AVIF grid first cell was consumed"))?
        } else {
            validate_plane_with_token(
                extracted.input,
                &super::samples::EncodedPlane {
                    samples: vec![sample.clone()],
                },
                token,
            )?
        };
        let Some(color_leaf) = color.first_leaf else {
            return Ok(None);
        };
        if color.frame_dimensions != Some((color_leaf.width, color_leaf.height)) {
            return Ok(None);
        }
        let alpha_plane = if let Some(alpha) = &still.alpha {
            let mut alpha = validate_plane_with_token(
                extracted.input,
                &super::samples::EncodedPlane {
                    samples: vec![alpha.samples[index].clone()],
                },
                token,
            )?;
            if alpha.first_leaf.is_some()
                || !monochrome_alpha_matches(
                    &alpha,
                    (color_leaf.width, color_leaf.height),
                    color.sequence.bit_depth,
                )
            {
                return Ok(None);
            }
            let Some(alpha_plane) = alpha.complete_monochrome_plane.take() else {
                return Ok(None);
            };
            Some(alpha_plane)
        } else {
            None
        };
        cells.push(portable_still(color_leaf, color.sequence, alpha_plane));
    }

    let first = cells
        .first()
        .ok_or_else(|| malformed("AVIF grid has no cells"))?;
    let cell_width = first.width;
    let cell_height = first.height;
    if cell_width == 0
        || cell_height == 0
        || properties.output_width() == 0
        || properties.output_height() == 0
    {
        return Err(malformed("AVIF grid has an empty cell or output canvas"));
    }
    if cells.iter().any(|cell| {
        cell.width != cell_width
            || cell.height != cell_height
            || cell.bit_depth != first.bit_depth
            || cell.monochrome != first.monochrome
            || cell.color_primaries != first.color_primaries
            || cell.transfer_characteristics != first.transfer_characteristics
            || cell.matrix_coefficients != first.matrix_coefficients
            || cell.color_range != first.color_range
            || cell.subsampling_x != first.subsampling_x
            || cell.subsampling_y != first.subsampling_y
            || cell.alpha_plane.is_some() != first.alpha_plane.is_some()
    }) {
        return Err(malformed(
            "AVIF grid cells disagree on decoded geometry or format",
        ));
    }
    let output_width = properties.output_width();
    let output_height = properties.output_height();
    let total_width = cell_width
        .checked_mul(properties.columns())
        .ok_or_else(|| malformed("AVIF grid width overflows"))?;
    let total_height = cell_height
        .checked_mul(properties.rows())
        .ok_or_else(|| malformed("AVIF grid height overflows"))?;
    if total_width < output_width || total_height < output_height {
        return Err(malformed("AVIF grid cells do not cover the output canvas"));
    }
    let last_column = properties
        .columns()
        .checked_sub(1)
        .ok_or_else(|| malformed("AVIF grid has no columns"))?;
    let last_row = properties
        .rows()
        .checked_sub(1)
        .ok_or_else(|| malformed("AVIF grid has no rows"))?;
    if cell_width
        .checked_mul(last_column)
        .ok_or_else(|| malformed("AVIF grid last column overflows"))?
        >= output_width
        || cell_height
            .checked_mul(last_row)
            .ok_or_else(|| malformed("AVIF grid last row overflows"))?
            >= output_height
    {
        return Err(malformed("AVIF grid has an invisible final row or column"));
    }
    let mut canvas = FrameCanvas::new(
        output_width,
        output_height,
        first.subsampling_x,
        first.subsampling_y,
    )?;
    let mut alpha_canvas = if first.alpha_plane.is_some() {
        Some(raster::MonochromeFrameCanvas::new(
            output_width,
            output_height,
        )?)
    } else {
        None
    };

    for (index, cell) in cells.iter().enumerate() {
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "cell_count is nonzero above, so the validated grid column count cannot be zero"
        )]
        let (row, column) = (index / columns, index % columns);
        let x = u32::try_from(
            column
                .checked_mul(
                    usize::try_from(cell_width)
                        .map_err(|_| malformed("AVIF grid cell width exceeds usize"))?,
                )
                .ok_or_else(|| malformed("AVIF grid x origin overflows usize"))?,
        )
        .map_err(|_| malformed("AVIF grid x origin exceeds u32"))?;
        let y = u32::try_from(
            row.checked_mul(
                usize::try_from(cell_height)
                    .map_err(|_| malformed("AVIF grid cell height exceeds usize"))?,
            )
            .ok_or_else(|| malformed("AVIF grid y origin overflows usize"))?,
        )
        .map_err(|_| malformed("AVIF grid y origin exceeds u32"))?;
        let visible_width = output_width
            .checked_sub(x)
            .ok_or_else(|| malformed("AVIF grid cell starts outside the output width"))?
            .min(cell.width);
        let visible_height = output_height
            .checked_sub(y)
            .ok_or_else(|| malformed("AVIF grid cell starts outside the output height"))?
            .min(cell.height);
        if visible_width == 0 || visible_height == 0 {
            return Err(malformed("AVIF grid cell has no visible samples"));
        }
        canvas.place_cropped_cell(
            cell.width,
            cell.height,
            visible_width,
            visible_height,
            x,
            y,
            &cell.planes,
        )?;
        if let (Some(alpha_canvas), Some(alpha_plane)) =
            (alpha_canvas.as_mut(), cell.alpha_plane.as_ref())
        {
            alpha_canvas.place_cropped_plane(
                cell.width,
                cell.height,
                visible_width,
                visible_height,
                x,
                y,
                alpha_plane,
            )?;
        }
    }

    let planes = canvas.finish()?;
    let alpha_sample_depth = sample_depth::SampleDepth::new(first.bit_depth)
        .ok_or_else(|| malformed("AVIF alpha sample depth is unsupported"))?;
    let alpha_plane = alpha_canvas
        .map(|canvas| canvas.finish(alpha_sample_depth))
        .transpose()?;
    Ok(Some(portable_still(
        assembled_leaf(output_width, output_height, planes),
        sequence::SequenceHeader {
            profile: 0,
            still_picture: true,
            reduced_still_picture_header: true,
            timing: None,
            decoder_model_present: false,
            display_model_present: false,
            operating_points: Vec::new(),
            width_bits: 0,
            height_bits: 0,
            max_width: 0,
            max_height: 0,
            frame_id_numbers_present: false,
            delta_frame_id_bits: 0,
            frame_id_bits: 0,
            use_128x128_superblock: false,
            enable_filter_intra: false,
            enable_intra_edge_filter: false,
            enable_interintra_compound: false,
            enable_masked_compound: false,
            enable_warped_motion: false,
            enable_dual_filter: false,
            enable_order_hint: false,
            enable_jnt_comp: false,
            enable_ref_frame_mvs: false,
            screen_content_tools: 0,
            force_integer_mv: 0,
            order_hint_bits: 0,
            enable_superres: false,
            enable_cdef: false,
            enable_restoration: false,
            bit_depth: first.bit_depth,
            monochrome: first.monochrome,
            color_primaries: first.color_primaries,
            transfer_characteristics: first.transfer_characteristics,
            matrix_coefficients: first.matrix_coefficients,
            color_range: first.color_range,
            subsampling_x: first.subsampling_x,
            subsampling_y: first.subsampling_y,
            chroma_sample_position: 0,
            separate_uv_delta_q: false,
            film_grain_present: false,
        },
        alpha_plane,
    )))
}

#[allow(
    dead_code,
    reason = "compatibility wrapper for token-aware production validation"
)]
fn validate_still(extracted: &ExtractedAvif<'_>) -> Av1Result<Option<PortableStill>> {
    validate_still_with_token(extracted, None)
}

fn validate_still_with_token(
    extracted: &ExtractedAvif<'_>,
    token: Option<&crate::CancellationToken>,
) -> Av1Result<Option<PortableStill>> {
    let mut portable = None;
    if let Some(still) = &extracted.still {
        if extracted.grid_properties.is_some() || !extracted.grid_item_ids.is_empty() {
            return validate_grid_with_token(extracted, still, token);
        }
        let mut color = validate_plane_with_token(extracted.input, &still.color, token)?;
        if color.first_leaf.is_none() {
            if still.color.samples.len() != 1 {
                return Ok(None);
            }
            let Some(dimensions) = monochrome_primary_dimensions(&color) else {
                return Ok(None);
            };
            let Some(color_plane) = color.complete_monochrome_plane.take() else {
                return Ok(None);
            };
            let alpha_plane = if let Some(alpha) = &still.alpha {
                if alpha.samples.len() != 1 {
                    return Ok(None);
                }
                let mut alpha = validate_plane_with_token(extracted.input, alpha, token)?;
                if !monochrome_alpha_matches(&alpha, dimensions, color.sequence.bit_depth) {
                    return Ok(None);
                }
                let Some(alpha_plane) = alpha.complete_monochrome_plane.take() else {
                    return Ok(None);
                };
                Some(alpha_plane)
            } else {
                None
            };
            return Ok(Some(portable_monochrome_still(
                color_plane,
                dimensions,
                color.sequence,
                alpha_plane,
            )));
        }
        let Some(color_leaf) = color.first_leaf.as_ref() else {
            return Ok(None);
        };
        if color.frame_dimensions != Some((color_leaf.width, color_leaf.height)) {
            return Ok(None);
        }
        let alpha_plane = if let Some(alpha) = &still.alpha {
            let mut alpha = validate_plane_with_token(extracted.input, alpha, token)?;
            if alpha.first_leaf.is_some()
                || !monochrome_alpha_matches(
                    &alpha,
                    (color_leaf.width, color_leaf.height),
                    color.sequence.bit_depth,
                )
            {
                return Ok(None);
            }
            let Some(alpha_plane) = alpha.complete_monochrome_plane.take() else {
                return Ok(None);
            };
            Some(alpha_plane)
        } else {
            None
        };
        // An `avis` file may retain the first/default image as a primary
        // `av01` item as well as carrying a later movie track.  The default
        // image is an independent decode contract: later track samples must
        // not make `decode()` fail or hide a supported primary item.  Full
        // sequence presentation still validates every track sample through
        // `validate_sequence` and remains a separate capability.
        portable = match (still.color.samples.as_slice(), color.first_leaf) {
            ([_], Some(leaf)) => Some(portable_still(leaf, color.sequence, alpha_plane)),
            _ => None,
        };
    }
    Ok(portable)
}

/// Validate and materialize the first sample of a sequence as a standalone
/// image.
///
/// The sequence validator remains responsible for every sample and for
/// cross-sample frame-ID/reference continuity. This helper is deliberately
/// limited to the first sample so ordinary image decoding can preserve the
/// format's default-image behavior when a later movie sample is malformed.
/// `validate_plane` still creates the complete AV1 sequence-aware frame state
/// for that sample; an inter frame that needs an earlier reference therefore
/// remains rejected instead of being mistaken for an independent still.
fn validate_first_sequence_sample(
    extracted: &ExtractedAvif<'_>,
    sequence: &super::samples::SequencePayload,
    token: Option<&crate::CancellationToken>,
) -> Av1Result<Option<PortableStill>> {
    let color_sample = sequence
        .color
        .samples
        .first()
        .ok_or_else(|| malformed("AVIF sequence has no color samples"))?;
    let color = validate_plane_with_token(
        extracted.input,
        &super::samples::EncodedPlane {
            samples: vec![color_sample.clone()],
        },
        token,
    )?;
    if color.first_leaf.is_none() {
        let Some(dimensions) = monochrome_primary_dimensions(&color) else {
            return Ok(None);
        };
        let Some(color_plane) = color.complete_monochrome_plane else {
            return Ok(None);
        };
        let alpha_plane = if let Some(alpha) = &sequence.alpha {
            let alpha_sample = alpha
                .samples
                .first()
                .ok_or_else(|| malformed("AVIF sequence has no alpha sample"))?;
            let mut alpha = validate_plane_with_token(
                extracted.input,
                &super::samples::EncodedPlane {
                    samples: vec![alpha_sample.clone()],
                },
                token,
            )?;
            if !monochrome_alpha_matches(&alpha, dimensions, color.sequence.bit_depth) {
                return Ok(None);
            }
            let Some(alpha_plane) = alpha.complete_monochrome_plane.take() else {
                return Ok(None);
            };
            Some(alpha_plane)
        } else {
            None
        };
        return Ok(Some(portable_monochrome_still(
            color_plane,
            dimensions,
            color.sequence,
            alpha_plane,
        )));
    }
    let Some((color_width, color_height)) = color
        .first_leaf
        .as_ref()
        .map(|leaf| (leaf.width, leaf.height))
    else {
        return Ok(None);
    };
    if color.frame_dimensions != Some((color_width, color_height)) {
        return Ok(None);
    }
    let alpha_plane = if let Some(alpha) = &sequence.alpha {
        let alpha_sample = alpha
            .samples
            .first()
            .ok_or_else(|| malformed("AVIF sequence has no alpha sample"))?;
        let mut alpha = validate_plane_with_token(
            extracted.input,
            &super::samples::EncodedPlane {
                samples: vec![alpha_sample.clone()],
            },
            token,
        )?;
        if alpha.first_leaf.is_some()
            || !monochrome_alpha_matches(
                &alpha,
                (color_width, color_height),
                color.sequence.bit_depth,
            )
        {
            return Ok(None);
        }
        let Some(alpha_plane) = alpha.complete_monochrome_plane.take() else {
            return Ok(None);
        };
        Some(alpha_plane)
    } else {
        None
    };
    let color_leaf = color
        .first_leaf
        .ok_or_else(|| malformed("validated first sequence sample lost its color surface"))?;
    Ok(Some(portable_still(
        color_leaf,
        color.sequence,
        alpha_plane,
    )))
}

#[allow(
    dead_code,
    reason = "compatibility wrapper for token-aware display materialization"
)]
pub(super) fn validate_first(extracted: &ExtractedAvif<'_>) -> Av1Result<ValidatedAv1> {
    validate_first_with_token(extracted, None)
}

pub(super) fn validate_first_with_token(
    extracted: &ExtractedAvif<'_>,
    token: Option<&crate::CancellationToken>,
) -> Av1Result<ValidatedAv1> {
    let portable_still = if extracted.still.is_some() {
        validate_still_with_token(extracted, token)?
    } else if let Some(sequence) = &extracted.sequence {
        validate_first_sequence_sample(extracted, sequence, token)?
    } else {
        None
    };
    Ok(ValidatedAv1 { portable_still })
}

#[cfg(test)]
fn validate_sequence(extracted: &ExtractedAvif<'_>) -> Av1Result<()> {
    let Some(sequence) = &extracted.sequence else {
        return Ok(());
    };
    validate_plane_state(extracted.input, &sequence.color)?;
    if let Some(alpha) = &sequence.alpha {
        validate_plane_state(extracted.input, alpha)?;
    }
    Ok(())
}

#[cfg(coverage)]
pub(super) fn validate(extracted: &ExtractedAvif<'_>) -> Av1Result<ValidatedAv1> {
    let portable_still = validate_still(extracted)?;
    if let Some(sequence) = &extracted.sequence {
        validate_plane_state(extracted.input, &sequence.color)?;
        if let Some(alpha) = &sequence.alpha {
            validate_plane_state(extracted.input, alpha)?;
        }
    }
    Ok(ValidatedAv1 { portable_still })
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_sample(sample: &[u8], config: [u8; 4]) -> (Vec<u8>, EncodedSample) {
    let mut input = sample.to_vec();
    let config_start = input.len();
    input.extend_from_slice(&config);
    let input_length = input.len();
    (
        input,
        EncodedSample {
            spans: vec![ByteSpan {
                start: 0,
                end: config_start,
            }],
            config: ByteSpan {
                start: config_start,
                end: input_length,
            },
            sync: true,
            duration: 1,
        },
    )
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_track_prefix(
    samples: &[&[u8]],
    target: usize,
    replacement: &[u8],
    config: [u8; 4],
) -> Av1Result<()> {
    let mut state = FrameState::new();
    for (index, sample) in samples.iter().enumerate().take(target.saturating_add(1)) {
        let bytes = if index == target { replacement } else { sample };
        let (input, encoded) = coverage_sample(bytes, config);
        validate_sample(&input, &encoded, &mut state)?;
        state.__coverage_seed_missing_reference_surfaces();
    }
    Ok(())
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_sweep_track(samples: &[&[u8]], config: [u8; 4]) {
    for (target, sample) in samples.iter().enumerate() {
        assert!(coverage_track_prefix(samples, target, sample, config).is_ok());
        for end in 0..sample.len() {
            let _ = coverage_track_prefix(samples, target, &sample[..end], config);
        }
        for index in 0..sample.len() {
            for replacement in [0, 1, 0x55, 0x7f, 0x80, 0xaa, 0xff] {
                if sample[index] == replacement {
                    continue;
                }
                let mut mutated = sample.to_vec();
                mutated[index] = replacement;
                let _ = coverage_track_prefix(samples, target, &mutated, config);
            }
        }
    }
}

#[cfg(coverage)]
#[coverage(off)]
pub(crate) fn __coverage_exercise_private_branches() {
    transform::__coverage_exercise_private_branches();
    bit_reader::__coverage_exercise_private_branches();
    block::__coverage_exercise_private_branches();
    entropy::__coverage_exercise_private_branches();
    frame::__coverage_exercise_private_branches();
    raster::__coverage_exercise_private_branches();
    sequence::__coverage_exercise_private_branches();

    let valid = b"\x12\x00\x0a\x0a\x40\x00\x00\x02\xaf\xff\xbf\xff\x3e\xa0\x32\x0d\x10\x00\x93\x80\x00\x08\x00\x00\x01\x48\x1a\x7a\xa0";
    let valid_config = [0x81, 0x40, 0x7c, 0];
    let (input, sample) = coverage_sample(valid, valid_config);
    assert_eq!(
        validate_sample(&input, &sample, &mut FrameState::new()),
        Ok(())
    );
    let header = frame::__coverage_reduced_header_payload();
    let mut split = b"\x12\x00\x0a\x0a\x40\x00\x00\x02\xaf\xff\xbf\xff\x3e\xa0".to_vec();
    split.extend_from_slice(&[0x1a, u8::try_from(header.len()).unwrap()]);
    split.extend_from_slice(&header);
    split.extend_from_slice(&[0x22, 1, 0]);
    let (input, sample) = coverage_sample(&split, valid_config);
    assert!(matches!(
        validate_sample(&input, &sample, &mut FrameState::new()),
        Err(CodecError::Malformed(message))
            if message == "invalid AV1 bitstream: entropy symbol coder overread the tile padding"
    ));
    let mut pending_then_delimiter = split[..split.len() - 2].to_vec();
    pending_then_delimiter.extend_from_slice(&[0x12, 0]);
    let (input, sample) = coverage_sample(&pending_then_delimiter, valid_config);
    assert!(validate_sample(&input, &sample, &mut FrameState::new()).is_err());
    let mut redundant = b"\x12\x00\x0a\x0a\x40\x00\x00\x02\xaf\xff\xbf\xff\x3e\xa0".to_vec();
    for obu_type in [0x1a, 0x3a] {
        redundant.extend_from_slice(&[obu_type, u8::try_from(header.len()).unwrap()]);
        redundant.extend_from_slice(&header);
    }
    redundant.extend_from_slice(&[0x22, 1, 0]);
    let (input, sample) = coverage_sample(&redundant, valid_config);
    assert!(matches!(
        validate_sample(&input, &sample, &mut FrameState::new()),
        Err(CodecError::Malformed(message))
            if message == "invalid AV1 bitstream: entropy symbol coder overread the tile padding"
    ));
    let invalid_span = EncodedSample {
        spans: vec![ByteSpan { start: 0, end: 1 }],
        config: ByteSpan { start: 0, end: 0 },
        sync: true,
        duration: 1,
    };
    assert!(validate_sample(&[], &invalid_span, &mut FrameState::new()).is_err());
    for end in 0..valid.len() {
        let (input, sample) = coverage_sample(&valid[..end], valid_config);
        let _ = validate_sample(&input, &sample, &mut FrameState::new());
    }
    for index in 0..valid.len() {
        for replacement in 0..=u8::MAX {
            if valid[index] == replacement {
                continue;
            }
            let mut mutated = valid.to_vec();
            mutated[index] = replacement;
            let (input, sample) = coverage_sample(&mutated, valid_config);
            let _ = validate_sample(&input, &sample, &mut FrameState::new());
        }
    }

    let animated: &[&[u8]] = &[
        b"\x12\x00\x0a\x0e\x00\x00\x00\x03\xbc\xac\xa9\xb5\xf2\x20\x21\xa0\xd0\x80\x32\x13\x10\x00\x83\x80\x00\x00\x80\x00\x00\x00\xeb\xc5\xa6\x2e\x0c\x0d\xd1\x51\x40",
        b"\x12\x00\x32\x23\x28\x04\xe0\x40\x00\x00\x23\x43\x30\x00\x00\x40\x00\x04\x00\x00\x08\xe4\x66\x90\x91\x47\x7f\x6e\xcc\x05\x23\x9b\xc1\x1c\xc6\x74\xcb\x7e\xe0",
        b"\x32\x23\x28\x02\xe0\x80\x00\x00\xa3\x44\xc0\x00\x00\x48\x00\x04\x00\x00\x26\x66\xc9\x49\xed\xf9\xfc\xed\x11\x20\x54\x85\xcf\x5f\x49\x98\x10\x5b\x20",
        b"\x32\x23\x30\x03\xc2\x00\x00\x81\x46\x8c\x80\x00\x00\x90\x00\x08\x00\x1f\x3a\xcd\xf2\xb3\x29\xa3\x70\xb6\x44\xb1\xd9\x5a\x93\x1f\x3c\x56\x60\x14\xc4",
        b"\x12\x00\x1a\x01\xa8",
        b"\x12\x00\x32\x1a\x30\x06\x44\x09\x80\x01\x46\x8c\x80\x00\x00\x90\x00\x08\x00\x33\xa1\xc0\x60\x46\x86\x20\x7d\xcf\xf4\xfc",
        b"\x12\x00\x32\x15\x30\x08\x00\x11\x30\x01\x46\x8c\x80\x00\x00\x90\x00\x08\x00\xb3\x2e\xde\x2e\xcf\x20",
    ];
    coverage_sweep_track(animated, [0x81, 0x00, 0x0c, 0]);

    let twelve_bit_alpha: &[&[u8]] = &[
        valid,
        b"\x12\x00\x32\x10\x30\x03\x80\x80\x00\x00\x46\xa7\x80\x00\x09\x00\x08\x00\x9c\x50",
        b"\x12\x00\x0a\x0a\x40\x00\x00\x02\xaf\xff\xbf\xff\x3e\xa0\x32\x0a\x10\x00\xbe\x00\x00\x09\x00\x00\x0e\x36",
        b"\x12\x00\x0a\x0a\x40\x00\x00\x02\xaf\xff\xbf\xff\x3e\xa0\x32\x1c\x10\x00\xbe\x00\x00\x09\x18\x00\x3b\x95\xa6\xa8\x47\x2b\xdf\x67\x4b\xd6\x0e\x45\xbd\xbf\xf5\x1b\x6f\x23\x48\x62",
        b"\x12\x00\x0a\x0a\x40\x00\x00\x02\xaf\xff\xbf\xff\x3e\xa0\x32\x23\x10\x00\xa7\x80\x00\x09\x24\xe0\xff\xfc\xe9\x1e\xd8\x9f\x6e\x05\x5e\x6f\xc7\x36\x3a\x9d\x64\xd6\x35\x31\xf9\xc1\x4d\xfb\x26\x00\xd6\xbc\x5c",
    ];
    coverage_sweep_track(twelve_bit_alpha, [0x81, 0x40, 0x7c, 0]);

    let invalid_streams: &[&[u8]] = &[
        b"\x92\x00",
        b"\x13\x00",
        b"\x10",
        b"\x16",
        b"\x16\x01\x00",
        b"\x12\x80",
        b"\x12\x01",
        b"\x12\x80\x80\x80\x80\x80\x80\x80\x80",
        b"\x32\x00",
        b"\x0a\x09\x18\x19\xbf\xff\x68\x80\x86\x83\x42",
    ];
    for stream in invalid_streams {
        let (input, sample) = coverage_sample(stream, valid_config);
        let _ = validate_sample(&input, &sample, &mut FrameState::new());
    }
    let extension = b"\x16\x00\x00\x0a\x09\x18\x19\xbf\xff\x68\x80\x86\x83\x42\x32\x00";
    let (input, sample) = coverage_sample(extension, valid_config);
    let _ = validate_sample(&input, &sample, &mut FrameState::new());
    let reserved = b"\x4a\x00\x0a\x09\x18\x19\xbf\xff\x68\x80\x86\x83\x42\x32\x00";
    let (input, sample) = coverage_sample(reserved, valid_config);
    let _ = validate_sample(&input, &sample, &mut FrameState::new());
    let (input, sample) = coverage_sample(valid, [0x81, 0x20, 0, 0]);
    let _ = validate_sample(&input, &sample, &mut FrameState::new());

    let many_delimiters = b"\x12\x00".repeat(MAX_OBUS_PER_SAMPLE.saturating_add(1));
    let (input, sample) = coverage_sample(&many_delimiters, [0x81, 0x00, 0x0c, 0]);
    let _ = validate_sample(&input, &sample, &mut FrameState::new());

    let baseline_payload = b"\x18\x19\xbf\xff\x68\x80\x86\x83\x42";
    let animated_payload = b"\x00\x00\x00\x03\xbc\xac\xa9\xb5\xf2\x20\x21\xa0\xd0\x80";
    let baseline_spans = [ByteSpan {
        start: 0,
        end: baseline_payload.len(),
    }];
    let baseline_data = SegmentedData::new(baseline_payload, &baseline_spans).unwrap();
    let baseline_header = sequence::parse(&baseline_data, 0, baseline_payload.len()).unwrap();
    let animated_spans = [ByteSpan {
        start: 0,
        end: animated_payload.len(),
    }];
    let animated_data = SegmentedData::new(animated_payload, &animated_spans).unwrap();
    let animated_header = sequence::parse(&animated_data, 0, animated_payload.len()).unwrap();
    let mut state = FrameState::new();
    assert_eq!(state.accept_sequence(baseline_header.clone()), Ok(()));
    assert_eq!(state.accept_sequence(baseline_header), Ok(()));
    assert!(state.accept_sequence(animated_header.clone()).is_err());

    let (input, sample) = coverage_sample(valid, valid_config);
    let mut inconsistent_state = FrameState::new();
    assert_eq!(inconsistent_state.accept_sequence(animated_header), Ok(()));
    assert!(validate_sample(&input, &sample, &mut inconsistent_state).is_err());

    let _ = validate_plane(
        &[],
        &EncodedPlane {
            samples: Vec::new(),
        },
    );
    let invalid_plane = EncodedPlane {
        samples: vec![EncodedSample {
            spans: Vec::new(),
            config: ByteSpan { start: 0, end: 0 },
            sync: true,
            duration: 1,
        }],
    };
    assert!(validate_plane(&[], &invalid_plane).is_err());
    assert!(
        validate(&ExtractedAvif {
            input: &[],
            still: None,
            sequence: None,
            consumed: 0,
            retained_boxes: Vec::new(),
            metadata: Vec::new(),
            source_color: crate::types::SourceColor::new(),
            auxiliary_relationship: None,
            auxiliary_relationships: Vec::new(),
            item_relationships: Vec::new(),
            premultiplied_relationships: Vec::new(),
            item_color_properties: Vec::new(),
            item_icc_profiles: Vec::new(),
            item_properties: Vec::new(),
            item_plane_properties: Vec::new(),
            item_codec_properties: Vec::new(),
            item_locations: Vec::new(),
            grid_item_ids: Vec::new(),
            grid_properties: None,
            transform: None,
        })
        .is_ok()
    );

    let invalid_plane = || EncodedPlane {
        samples: vec![EncodedSample {
            spans: Vec::new(),
            config: ByteSpan { start: 0, end: 0 },
            sync: true,
            duration: 1,
        }],
    };
    assert!(
        validate(&ExtractedAvif {
            input: &[],
            still: Some(StillPayload {
                color: invalid_plane(),
                alpha: None,
            }),
            sequence: None,
            consumed: 0,
            retained_boxes: Vec::new(),
            metadata: Vec::new(),
            source_color: crate::types::SourceColor::new(),
            auxiliary_relationship: None,
            auxiliary_relationships: Vec::new(),
            item_relationships: Vec::new(),
            premultiplied_relationships: Vec::new(),
            item_color_properties: Vec::new(),
            item_icc_profiles: Vec::new(),
            item_properties: Vec::new(),
            item_plane_properties: Vec::new(),
            item_codec_properties: Vec::new(),
            item_locations: Vec::new(),
            grid_item_ids: Vec::new(),
            grid_properties: None,
            transform: None,
        })
        .is_err()
    );

    let valid_plane = || EncodedPlane {
        samples: vec![EncodedSample {
            spans: vec![ByteSpan {
                start: 0,
                end: valid.len(),
            }],
            config: ByteSpan {
                start: valid.len(),
                end: input.len(),
            },
            sync: true,
            duration: 1,
        }],
    };
    let valid_sample = || EncodedSample {
        spans: vec![ByteSpan {
            start: 0,
            end: valid.len(),
        }],
        config: ByteSpan {
            start: valid.len(),
            end: input.len(),
        },
        sync: true,
        duration: 1,
    };
    let validated_multi_sample = validate(&ExtractedAvif {
        input: &input,
        consumed: 0,
        retained_boxes: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
        auxiliary_relationship: None,
        auxiliary_relationships: Vec::new(),
        item_relationships: Vec::new(),
        premultiplied_relationships: Vec::new(),
        item_color_properties: Vec::new(),
        item_icc_profiles: Vec::new(),
        item_properties: Vec::new(),
        item_plane_properties: Vec::new(),
        item_codec_properties: Vec::new(),
        item_locations: Vec::new(),
        grid_item_ids: Vec::new(),
        grid_properties: None,
        transform: None,
        still: Some(StillPayload {
            color: EncodedPlane {
                samples: vec![valid_sample(), valid_sample()],
            },
            alpha: None,
        }),
        sequence: None,
    });
    assert!(validated_multi_sample.is_ok_and(|validated| validated.portable_still.is_none()));
    assert!(
        validate(&ExtractedAvif {
            input: &input,
            still: Some(StillPayload {
                color: valid_plane(),
                alpha: Some(invalid_plane()),
            }),
            sequence: None,
            consumed: 0,
            retained_boxes: Vec::new(),
            metadata: Vec::new(),
            source_color: crate::types::SourceColor::new(),
            auxiliary_relationship: None,
            auxiliary_relationships: Vec::new(),
            item_relationships: Vec::new(),
            premultiplied_relationships: Vec::new(),
            item_color_properties: Vec::new(),
            item_icc_profiles: Vec::new(),
            item_properties: Vec::new(),
            item_plane_properties: Vec::new(),
            item_codec_properties: Vec::new(),
            item_locations: Vec::new(),
            grid_item_ids: Vec::new(),
            grid_properties: None,
            transform: None,
        })
        .is_ok_and(|validated| validated.portable_still.is_none())
    );
    assert!(
        validate(&ExtractedAvif {
            input: &[],
            still: None,
            sequence: Some(SequencePayload {
                color: invalid_plane(),
                alpha: None,
                timescale: NonZeroU32::new(1).unwrap(),
                loop_count: crate::types::AnimationLoop::Unspecified,
            }),
            consumed: 0,
            retained_boxes: Vec::new(),
            metadata: Vec::new(),
            source_color: crate::types::SourceColor::new(),
            auxiliary_relationship: None,
            auxiliary_relationships: Vec::new(),
            item_relationships: Vec::new(),
            premultiplied_relationships: Vec::new(),
            item_color_properties: Vec::new(),
            item_icc_profiles: Vec::new(),
            item_properties: Vec::new(),
            item_plane_properties: Vec::new(),
            item_codec_properties: Vec::new(),
            item_locations: Vec::new(),
            grid_item_ids: Vec::new(),
            grid_properties: None,
            transform: None,
        })
        .is_err()
    );
    assert!(
        validate(&ExtractedAvif {
            input: &input,
            still: None,
            sequence: Some(SequencePayload {
                color: valid_plane(),
                alpha: Some(invalid_plane()),
                timescale: NonZeroU32::new(1).unwrap(),
                loop_count: crate::types::AnimationLoop::Unspecified,
            }),
            consumed: 0,
            retained_boxes: Vec::new(),
            metadata: Vec::new(),
            source_color: crate::types::SourceColor::new(),
            auxiliary_relationship: None,
            auxiliary_relationships: Vec::new(),
            item_relationships: Vec::new(),
            premultiplied_relationships: Vec::new(),
            item_color_properties: Vec::new(),
            item_icc_profiles: Vec::new(),
            item_properties: Vec::new(),
            item_plane_properties: Vec::new(),
            item_codec_properties: Vec::new(),
            item_locations: Vec::new(),
            grid_item_ids: Vec::new(),
            grid_properties: None,
            transform: None,
        })
        .is_err()
    );
}

#[cfg(coverage)]
pub(crate) fn __coverage_entropy_reference_trace() -> CodecResult<Vec<crate::Av1EntropyTraceState>>
{
    entropy::reference_trace()
}

#[cfg(coverage)]
#[coverage(off)]
pub(crate) fn __coverage_reconstruction(
    input: &[u8],
) -> CodecResult<Option<crate::Av1ReconstructionTrace>> {
    let extracted = super::samples::validated(input)?;
    let validated = validate(&extracted)?;
    let Some(still) = validated.portable_still else {
        return Ok(None);
    };
    Ok(Some(crate::Av1ReconstructionTrace {
        width: still.width,
        height: still.height,
        bit_depth: still.bit_depth,
        monochrome: still.monochrome,
        color_primaries: still.color_primaries,
        transfer_characteristics: still.transfer_characteristics,
        matrix_coefficients: still.matrix_coefficients,
        color_range: still.color_range,
        subsampling_x: still.subsampling_x,
        subsampling_y: still.subsampling_y,
        planes: still.planes.map(|plane| plane.samples),
        entropy_operations: still.entropy_operations,
    }))
}

#[cfg(coverage)]
#[coverage(off)]
pub(super) fn __coverage_portable_still() -> PortableStill {
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
        frame_id_numbers_present: false,
        planes: std::array::from_fn(|_| block::ReconstructedPlane {
            samples: vec![128; 16],
        }),
        alpha_plane: None,
        entropy_operations: Vec::new(),
    }
}

#[cfg(coverage)]
#[coverage(off)]
pub(crate) fn __coverage_sweep_first_leaf(input: &[u8]) {
    let extracted = super::samples::validated(input).expect("portable AVIF fixture must extract");
    let spans = extracted
        .still
        .as_ref()
        .and_then(|still| still.color.samples.first())
        .map(|sample| sample.spans.clone())
        .expect("portable AVIF fixture must contain one still sample");
    assert!(validate(&extracted).is_ok());
    drop(extracted);

    let validate_mutation = |mutated: &[u8]| {
        if let Ok(extracted) = super::samples::validated(mutated) {
            let _ = validate(&extracted);
        }
    };
    for span in spans {
        for offset in span.start..span.end {
            for replacement in 0..=u8::MAX {
                if input[offset] == replacement {
                    continue;
                }
                let mut mutated = input.to_vec();
                mutated[offset] = replacement;
                validate_mutation(&mutated);
            }
            for fill in [0, u8::MAX] {
                let mut mutated = input.to_vec();
                mutated[offset..span.end].fill(fill);
                validate_mutation(&mutated);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_fixture_production_validation_retains_complete_plane() -> Av1Result<()> {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/test_support/fixtures/input/images/avif/alpha.avif"
        ));
        let extracted = super::super::samples::validated(bytes)?;
        let still = extracted
            .still
            .as_ref()
            .ok_or_else(|| malformed("alpha fixture has no still payload"))?;
        let alpha = still
            .alpha
            .as_ref()
            .ok_or_else(|| malformed("alpha fixture has no auxiliary payload"))?;
        let validated = validate_plane(bytes, alpha)?;
        let plane = validated
            .complete_monochrome_plane
            .ok_or_else(|| malformed("safe production path omitted complete alpha plane"))?;
        assert_eq!(plane.samples.len(), 64 * 64);
        assert!(plane.samples.iter().any(|&sample| sample != 0));
        Ok(())
    }

    #[test]
    fn grid_fixture_production_validation_retains_complete_cells() -> Av1Result<()> {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/test_support/fixtures/input/images/avif/grid.avif"
        ));
        let extracted = super::super::samples::validated(bytes)?;
        let still = extracted
            .still
            .as_ref()
            .ok_or_else(|| malformed("grid fixture has no still payload"))?;
        for sample in &still.color.samples {
            validate_plane(
                bytes,
                &super::super::samples::EncodedPlane {
                    samples: vec![sample.clone()],
                },
            )?;
        }
        if let Some(alpha) = &still.alpha {
            for sample in &alpha.samples {
                validate_plane(
                    bytes,
                    &super::super::samples::EncodedPlane {
                        samples: vec![sample.clone()],
                    },
                )?;
            }
        }
        let validated = validate_first(&extracted)?;
        assert!(validated.portable_still.is_some());
        Ok(())
    }

    #[test]
    fn primary_item_validation_is_independent_of_sequence_track() -> Av1Result<()> {
        use std::num::NonZeroU32;

        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/test_support/fixtures/input/images/avif/alpha.avif"
        ));
        let mut extracted = super::super::samples::validated(bytes)?;
        let still = extracted
            .still
            .take()
            .ok_or_else(|| malformed("alpha fixture has no still payload"))?;
        let sequence = super::super::samples::SequencePayload {
            color: still.color.clone(),
            alpha: still
                .alpha
                .as_ref()
                .map(|plane| super::super::samples::EncodedPlane {
                    samples: plane.samples.clone(),
                }),
            timescale: NonZeroU32::new(1)
                .ok_or_else(|| malformed("test timescale unexpectedly became zero"))?,
            loop_count: crate::types::AnimationLoop::Unspecified,
        };
        extracted.still = Some(still);
        extracted.sequence = Some(sequence);

        let validated = validate_first(&extracted)?;
        assert!(
            validated.portable_still.is_some(),
            "the primary item must remain independently eligible when a movie track exists"
        );

        // Sequence validation deliberately remains a separate contract; no
        // sequence renderer is implied by the primary-item result above.
        validate_sequence(&extracted)?;
        Ok(())
    }
}
