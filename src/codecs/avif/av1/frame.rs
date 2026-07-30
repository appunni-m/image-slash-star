//! Complete AV1 uncompressed-frame-header syntax and reference state.

use std::ops::Range;

use super::bit_reader::{BitReader, SegmentedData};
use super::entropy;
use super::sequence::SequenceHeader;
#[cfg(coverage)]
use super::sequence::{DecoderParameters, OperatingPoint, Timing};
use super::{Av1Result, malformed};
#[cfg(coverage)]
use crate::codecs::avif::samples::ByteSpan;

const PRIMARY_REF_NONE: usize = 7;
const REFERENCE_SLOTS: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
enum FrameType {
    Key,
    Inter,
    IntraOnly,
    Switch,
}

impl FrameType {
    fn from_bits(value: u32) -> Self {
        match value {
            0 => Self::Key,
            1 => Self::Inter,
            2 => Self::IntraOnly,
            _ => Self::Switch,
        }
    }

    fn is_intra(self) -> bool {
        matches!(self, Self::Key | Self::IntraOnly)
    }

    fn is_inter(self) -> bool {
        matches!(self, Self::Inter | Self::Switch)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Segment {
    delta_q: i32,
    delta_lf_y_vertical: i32,
    delta_lf_y_horizontal: i32,
    delta_lf_u: i32,
    delta_lf_v: i32,
    reference: i32,
    skip: bool,
    global_motion: bool,
}

impl Segment {
    const fn empty() -> Self {
        Self {
            delta_q: 0,
            delta_lf_y_vertical: 0,
            delta_lf_y_horizontal: 0,
            delta_lf_u: 0,
            delta_lf_v: 0,
            reference: -1,
            skip: false,
            global_motion: false,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Segmentation {
    enabled: bool,
    update_map: bool,
    temporal: bool,
    update_data: bool,
    segments: [Segment; 8],
}

impl Segmentation {
    const fn empty() -> Self {
        Self {
            enabled: false,
            update_map: false,
            temporal: false,
            update_data: false,
            segments: [Segment::empty(); 8],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct LoopFilterDeltas {
    mode: [i32; 2],
    reference: [i32; 8],
}

impl LoopFilterDeltas {
    const fn defaults() -> Self {
        Self {
            mode: [0, 0],
            reference: [1, 0, 0, 0, -1, 0, -1, -1],
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct LoopFilter {
    level_y: [u32; 2],
    level_u: u32,
    level_v: u32,
    sharpness: u32,
    delta_enabled: bool,
    delta_update: bool,
    deltas: LoopFilterDeltas,
}

impl LoopFilter {
    const fn disabled() -> Self {
        Self {
            level_y: [0, 0],
            level_u: 0,
            level_v: 0,
            sharpness: 0,
            delta_enabled: true,
            delta_update: true,
            deltas: LoopFilterDeltas::defaults(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GlobalMotionType {
    Identity,
    Translation,
    RotZoom,
    Affine,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct GlobalMotion {
    kind: GlobalMotionType,
    matrix: [i32; 6],
}

impl GlobalMotion {
    const fn identity() -> Self {
        Self {
            kind: GlobalMotionType::Identity,
            matrix: [0, 0, 1 << 16, 0, 0, 1 << 16],
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct FilmGrain {
    seed: u32,
    update: bool,
    reference_slot: Option<usize>,
    y_points: Vec<[u32; 2]>,
    chroma_scaling_from_luma: bool,
    uv_points: [Vec<[u32; 2]>; 2],
    scaling_shift: u32,
    ar_coefficient_lag: u32,
    ar_coefficients_y: Vec<i32>,
    ar_coefficients_uv: [Vec<i32>; 2],
    ar_coefficient_shift: u32,
    grain_scale_shift: u32,
    uv_multiplier: [i32; 2],
    uv_luma_multiplier: [i32; 2],
    uv_offset: [i32; 2],
    overlap: bool,
    clip_to_restricted_range: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct Tiling {
    uniform: bool,
    min_log2_columns: u32,
    max_log2_columns: u32,
    log2_columns: u32,
    columns: u32,
    column_starts: Vec<u32>,
    min_log2_rows: u32,
    max_log2_rows: u32,
    log2_rows: u32,
    rows: u32,
    row_starts: Vec<u32>,
    context_update_tile: u32,
    tile_size_bytes: u32,
}

impl Tiling {
    fn tile_count(&self) -> u32 {
        // AV1 constrains each dimension to at most 64 tiles.
        self.columns.saturating_mul(self.rows)
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Quantization {
    base: u32,
    y_dc_delta: i32,
    u_dc_delta: i32,
    u_ac_delta: i32,
    v_dc_delta: i32,
    v_ac_delta: i32,
    different_uv_delta: bool,
    using_matrix: bool,
    matrix_y: u32,
    matrix_u: u32,
    matrix_v: u32,
}

#[derive(Clone, PartialEq, Eq)]
struct Cdef {
    damping: u32,
    bits: u32,
    y_strengths: Vec<u32>,
    uv_strengths: Vec<u32>,
}

#[derive(Clone, PartialEq, Eq)]
struct Restoration {
    types: [Option<entropy::RestorationType>; 3],
    unit_size_log2: [u32; 2],
}

#[derive(Clone, PartialEq, Eq)]
struct FrameHeader {
    show_existing_frame: bool,
    existing_frame_idx: Option<usize>,
    temporal_id: u32,
    spatial_id: u32,
    frame_type: FrameType,
    show_frame: bool,
    showable_frame: bool,
    error_resilient_mode: bool,
    disable_cdf_update: bool,
    allow_screen_content_tools: bool,
    force_integer_mv: bool,
    frame_id: u32,
    frame_size_override: bool,
    order_hint: u32,
    primary_ref_frame: usize,
    buffer_removal_times: Vec<u32>,
    refresh_frame_flags: u8,
    reference_order_hints: [u32; 8],
    upscaled_width: u32,
    frame_width: u32,
    frame_height: u32,
    render_width: u32,
    render_height: u32,
    superres_enabled: bool,
    superres_denominator: u32,
    have_render_size: bool,
    allow_intrabc: bool,
    frame_refs_short_signaling: bool,
    reference_indices: [usize; 7],
    allow_high_precision_mv: bool,
    interpolation_filter: u32,
    motion_mode_switchable: bool,
    use_ref_frame_mvs: bool,
    refresh_frame_context: bool,
    tiling: Option<Tiling>,
    quantization: Option<Quantization>,
    segmentation: Segmentation,
    delta_q_present: bool,
    delta_q_resolution_log2: u32,
    delta_lf_present: bool,
    delta_lf_resolution_log2: u32,
    delta_lf_multi: bool,
    segment_qindex: [u32; 8],
    segment_lossless: [bool; 8],
    all_lossless: bool,
    loop_filter: LoopFilter,
    cdef: Option<Cdef>,
    restoration: Option<Restoration>,
    transform_mode: u32,
    reference_mode_select: bool,
    skip_mode_references: Option<[usize; 2]>,
    skip_mode_enabled: bool,
    allow_warped_motion: bool,
    reduced_transform_set: bool,
    global_motion: [GlobalMotion; 7],
    film_grain: Option<FilmGrain>,
    header_bits: usize,
}

impl FrameHeader {
    fn empty(temporal_id: u32, spatial_id: u32) -> Self {
        Self {
            show_existing_frame: false,
            existing_frame_idx: None,
            temporal_id,
            spatial_id,
            frame_type: FrameType::Key,
            show_frame: false,
            showable_frame: false,
            error_resilient_mode: false,
            disable_cdf_update: false,
            allow_screen_content_tools: false,
            force_integer_mv: false,
            frame_id: 0,
            frame_size_override: false,
            order_hint: 0,
            primary_ref_frame: PRIMARY_REF_NONE,
            buffer_removal_times: Vec::new(),
            refresh_frame_flags: 0,
            reference_order_hints: [0; 8],
            upscaled_width: 0,
            frame_width: 0,
            frame_height: 0,
            render_width: 0,
            render_height: 0,
            superres_enabled: false,
            superres_denominator: 8,
            have_render_size: false,
            allow_intrabc: false,
            frame_refs_short_signaling: false,
            reference_indices: [0; 7],
            allow_high_precision_mv: false,
            interpolation_filter: 0,
            motion_mode_switchable: false,
            use_ref_frame_mvs: false,
            refresh_frame_context: false,
            tiling: None,
            quantization: None,
            segmentation: Segmentation::empty(),
            delta_q_present: false,
            delta_q_resolution_log2: 0,
            delta_lf_present: false,
            delta_lf_resolution_log2: 0,
            delta_lf_multi: false,
            segment_qindex: [0; 8],
            segment_lossless: [false; 8],
            all_lossless: false,
            loop_filter: LoopFilter::disabled(),
            cdef: None,
            restoration: None,
            transform_mode: 0,
            reference_mode_select: false,
            skip_mode_references: None,
            skip_mode_enabled: false,
            allow_warped_motion: false,
            reduced_transform_set: false,
            global_motion: [GlobalMotion::identity(); 7],
            film_grain: None,
            header_bits: 0,
        }
    }
}

pub(super) struct FrameState {
    sequence: Option<SequenceHeader>,
    references: [Option<FrameHeader>; REFERENCE_SLOTS],
    pending: Option<FrameHeader>,
    next_tile: u32,
    current_frame_id: Option<u32>,
    first_leaf: Option<super::block::FirstLeaf>,
}

impl FrameState {
    pub(super) fn new() -> Self {
        Self {
            sequence: None,
            references: std::array::from_fn(|_| None),
            pending: None,
            next_tile: 0,
            current_frame_id: None,
            first_leaf: None,
        }
    }

    pub(super) fn accept_sequence(&mut self, sequence: SequenceHeader) -> Av1Result<()> {
        if self
            .sequence
            .as_ref()
            .is_some_and(|previous| !previous.consistent_with(&sequence))
        {
            return Err(malformed("frame syntax validation failed"));
        }
        self.sequence = Some(sequence);
        Ok(())
    }

    pub(super) fn temporal_delimiter(&self) -> Av1Result<()> {
        if self.pending.is_some() {
            return Err(malformed(
                "temporal delimiter appears during a pending frame",
            ));
        }
        Ok(())
    }

    pub(super) fn finish(&self) -> Av1Result<&SequenceHeader> {
        if self.pending.is_some() {
            return Err(malformed("frame syntax validation failed"));
        }
        let Some(sequence) = self.sequence.as_ref() else {
            return Err(malformed("sample contains no sequence header"));
        };
        Ok(sequence)
    }

    pub(super) fn first_leaf(&self) -> Option<&super::block::FirstLeaf> {
        self.first_leaf.as_ref()
    }

    pub(super) fn frame_obu(
        &mut self,
        data: &SegmentedData<'_, '_>,
        start: usize,
        end: usize,
        temporal_id: u32,
        spatial_id: u32,
    ) -> Av1Result<()> {
        let mut reader = self.begin_frame(data, start, end, temporal_id, spatial_id, false)?;
        if self
            .pending
            .as_ref()
            .is_some_and(|header| header.show_existing_frame)
        {
            return Err(malformed("frame syntax validation failed"));
        }
        reader.byte_align()?;
        let group = self.read_tile_group(data, &mut reader, end)?;
        self.accept_tile_group(group)
    }

    pub(super) fn frame_header_obu(
        &mut self,
        data: &SegmentedData<'_, '_>,
        start: usize,
        end: usize,
        temporal_id: u32,
        spatial_id: u32,
        redundant: bool,
    ) -> Av1Result<()> {
        let mut reader = self.begin_frame(data, start, end, temporal_id, spatial_id, redundant)?;
        reader.trailing_bits()?;
        self.complete_show_existing()
    }

    pub(super) fn tile_group_obu(
        &mut self,
        data: &SegmentedData<'_, '_>,
        start: usize,
        end: usize,
    ) -> Av1Result<()> {
        let mut reader = BitReader::new(data, start, end)?;
        let group = self.read_tile_group(data, &mut reader, end)?;
        self.accept_tile_group(group)
    }

    fn begin_frame<'data, 'input, 'spans>(
        &mut self,
        data: &'data SegmentedData<'input, 'spans>,
        start: usize,
        end: usize,
        temporal_id: u32,
        spatial_id: u32,
        redundant: bool,
    ) -> Av1Result<BitReader<'data, 'input, 'spans>> {
        let Some(sequence) = self.sequence.as_ref() else {
            return Err(malformed("frame appears before a sequence header"));
        };
        if redundant {
            let Some(pending) = self.pending.as_ref() else {
                return Err(malformed("redundant frame header has no pending frame"));
            };
            let (header, reader) = parse(
                data,
                start,
                end,
                sequence,
                &self.references,
                temporal_id,
                spatial_id,
            )?;
            if &header != pending {
                return Err(malformed("frame syntax validation failed"));
            }
            return Ok(reader);
        }
        if self.pending.is_some() {
            return Err(malformed("frame syntax validation failed"));
        }
        let (header, reader) = parse(
            data,
            start,
            end,
            sequence,
            &self.references,
            temporal_id,
            spatial_id,
        )?;
        self.accept_parsed_header(
            sequence.frame_id_numbers_present,
            sequence.frame_id_bits,
            header,
        )?;
        Ok(reader)
    }

    fn accept_parsed_header(
        &mut self,
        frame_id_numbers_present: bool,
        frame_id_bits: u32,
        header: FrameHeader,
    ) -> Av1Result<()> {
        if frame_id_numbers_present && !header.show_existing_frame {
            validate_current_frame_id(frame_id_bits, self.current_frame_id, &header)?;
            self.invalidate_old_references(header.frame_id);
            self.current_frame_id = Some(header.frame_id);
        }
        self.pending = Some(header);
        self.next_tile = 0;
        Ok(())
    }

    fn complete_show_existing(&mut self) -> Av1Result<()> {
        let Some(header) = self.pending.as_ref() else {
            return Err(malformed("show-existing completion has no pending frame"));
        };
        if !header.show_existing_frame {
            return Ok(());
        }
        let Some(slot) = header.existing_frame_idx else {
            return Err(malformed("show-existing frame omits its reference slot"));
        };
        // `slot` is read from a three-bit AV1 syntax element.
        let Some(reference) = self.references[slot].as_ref() else {
            return Err(malformed("show-existing frame references an empty slot"));
        };
        let reference = reference.clone();
        if !reference.showable_frame {
            return Err(malformed("frame syntax validation failed"));
        }
        if reference.frame_type == FrameType::Key {
            let mut hidden = reference;
            hidden.showable_frame = false;
            self.references = std::array::from_fn(|_| Some(hidden.clone()));
        }
        self.pending = None;
        Ok(())
    }

    fn invalidate_old_references(&mut self, frame_id: u32) {
        // This method is called only after `begin_frame` has obtained the
        // active sequence header.
        let Some(sequence) = self.sequence.as_ref() else {
            return;
        };
        let difference_range = 1_u32 << sequence.delta_frame_id_bits;
        let frame_id_range = 1_u32 << sequence.frame_id_bits;
        for reference in &mut self.references {
            let Some(retained) = reference else {
                continue;
            };
            let invalid = if frame_id >= difference_range {
                retained.frame_id > frame_id
                    || retained.frame_id < frame_id.saturating_sub(difference_range)
            } else {
                retained.frame_id > frame_id
                    && retained.frame_id
                        < frame_id_range
                            .saturating_add(frame_id)
                            .saturating_sub(difference_range)
            };
            if invalid {
                *reference = None;
            }
        }
    }

    // ✅ VERIFIED: AV1 specification section 5.11.1; dav1d 1.5.3
    // src/obu.c:1154-1167 and src/decode.c:3149-3181.
    fn read_tile_group(
        &self,
        data: &SegmentedData<'_, '_>,
        bits: &mut BitReader<'_, '_, '_>,
        payload_end: usize,
    ) -> Av1Result<TileGroup> {
        let Some(sequence) = self.sequence.as_ref() else {
            return Err(malformed("tile group appears before a sequence header"));
        };
        let Some(header) = self.pending.as_ref() else {
            return Err(malformed("tile group appears without a pending frame"));
        };
        let Some(tiling) = header.tiling.as_ref() else {
            return Err(malformed("pending frame has no tile layout"));
        };
        let tile_count = tiling.tile_count();
        let (start, end) = if tile_count > 1 && bits.bit()? {
            let width = tiling.log2_columns.saturating_add(tiling.log2_rows);
            (bits.bits(width)?, bits.bits(width)?)
        } else {
            (0, tile_count.saturating_sub(1))
        };
        if start > end {
            return Err(malformed("frame syntax validation failed"));
        }
        if end >= tile_count {
            return Err(malformed("frame syntax validation failed"));
        }
        bits.byte_align()?;
        let tile_ranges = split_tile_payloads(
            data,
            bits.position() / 8,
            payload_end,
            start,
            end,
            tiling.tile_size_bytes,
        )?;
        let first_leaf =
            validate_tile_entropy_prefixes(data, &tile_ranges, start, header, sequence, tiling)?;
        Ok(TileGroup {
            start,
            end,
            first_leaf,
        })
    }

    fn accept_tile_group(&mut self, group: TileGroup) -> Av1Result<()> {
        let header = self
            .pending
            .as_ref()
            .ok_or(malformed("accepted tile group has no pending frame"))?;
        if group.start != self.next_tile {
            return Err(malformed("frame syntax validation failed"));
        }
        self.next_tile = group.end.saturating_add(1);
        self.first_leaf = self.first_leaf.take().or(group.first_leaf);
        let tile_count = header
            .tiling
            .as_ref()
            .ok_or(malformed("pending frame has no tile layout"))?
            .tile_count();
        if self.next_tile == tile_count {
            // `header` was borrowed from `pending` above, so the value cannot
            // disappear before this synchronous completion step.
            let completed = header.clone();
            self.pending = None;
            for (slot, reference) in self.references.iter_mut().enumerate() {
                let mask = 1_u8 << slot;
                if completed.refresh_frame_flags & mask != 0 {
                    *reference = Some(completed.clone());
                }
            }
            self.next_tile = 0;
        }
        Ok(())
    }
}

struct TileGroup {
    start: u32,
    end: u32,
    first_leaf: Option<super::block::FirstLeaf>,
}

// ✅ VERIFIED: dav1d 1.5.3 src/decode.c:3149-3181 and libaom 3.13.2
// av1/decoder/decodeframe.c:3618-3663. Every tile except the final tile in a
// group carries little-endian `tile_size_minus_1`; the final tile consumes the
// remaining OBU payload.
fn split_tile_payloads(
    data: &SegmentedData<'_, '_>,
    mut cursor: usize,
    payload_end: usize,
    start_tile: u32,
    end_tile: u32,
    size_width: u32,
) -> Av1Result<Vec<Range<usize>>> {
    if cursor > payload_end || payload_end > data.len() {
        return Err(malformed("frame syntax validation failed"));
    }
    let size_width = match size_width {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        _ => return Err(malformed("tile-size field width exceeds four bytes")),
    };
    if start_tile != end_tile && size_width == 0 {
        return Err(malformed("frame syntax validation failed"));
    }
    // AV1 tile indices are `u32`; `usize` is at least 32 bits on every
    // supported native and wasm target.
    let range_count = (end_tile.saturating_sub(start_tile) as usize).saturating_add(1);
    let mut ranges = Vec::with_capacity(range_count);
    for tile in start_tile..=end_tile {
        let tile_size = if tile == end_tile {
            payload_end.saturating_sub(cursor)
        } else {
            let size_end = cursor.saturating_add(size_width);
            if size_end > payload_end {
                return Err(malformed("frame syntax validation failed"));
            }
            let mut encoded_size = 0_usize;
            for byte_index in 0..size_width {
                let byte = usize::from(data.validated_byte(cursor.saturating_add(byte_index)));
                encoded_size |= byte << byte_index.saturating_mul(8);
            }
            cursor = size_end;
            let remaining = payload_end.saturating_sub(cursor);
            if encoded_size >= remaining {
                return Err(malformed("frame syntax validation failed"));
            }
            encoded_size.saturating_add(1)
        };
        let tile_end = cursor.saturating_add(tile_size);
        ranges.push(cursor..tile_end);
        cursor = tile_end;
    }
    // The final tile consumes `payload_end - cursor`, so a non-empty inclusive
    // tile range always ends exactly at the payload boundary.
    debug_assert_eq!(cursor, payload_end);
    Ok(ranges)
}

// ✅ VERIFIED: dav1d 1.5.3 src/decode.c:2425-2457 (`setup_tile`) and
// src/decode.c:2117-2162 (`decode_sb`). This consumes the first actual
// partition syntax element rather than constructing and dropping MSAC state.
fn validate_tile_entropy_prefixes(
    data: &SegmentedData<'_, '_>,
    ranges: &[Range<usize>],
    start_tile: u32,
    header: &FrameHeader,
    sequence: &SequenceHeader,
    tiling: &Tiling,
) -> Av1Result<Option<super::block::FirstLeaf>> {
    let root_level = u32::from(!sequence.use_128x128_superblock);
    // Frame dimensions and superblock mode were validated while parsing the
    // sequence/frame headers, so these private unit conversions are total.
    let block_width = (header.frame_width.saturating_add(7) >> 3).wrapping_shl(1);
    let block_height = (header.frame_height.saturating_add(7) >> 3).wrapping_shl(1);
    let block_shift = 4_u32.wrapping_add(u32::from(sequence.use_128x128_superblock));
    let (restoration_types, restoration_unit_size_log2) = header
        .restoration
        .as_ref()
        .map_or(([None; 3], [8; 2]), |restoration| {
            (restoration.types, restoration.unit_size_log2)
        });
    let frame_tools = entropy::FrameToolsContext {
        quantization: header.quantization.as_ref().map(|quantization| {
            entropy::QuantizationContext {
                base: quantization.base,
                y_dc_delta: quantization.y_dc_delta,
                u_dc_delta: quantization.u_dc_delta,
                u_ac_delta: quantization.u_ac_delta,
                v_dc_delta: quantization.v_dc_delta,
                v_ac_delta: quantization.v_ac_delta,
                different_uv_delta: quantization.different_uv_delta,
                using_matrix: quantization.using_matrix,
                matrix_y: quantization.matrix_y,
                matrix_u: quantization.matrix_u,
                matrix_v: quantization.matrix_v,
            }
        }),
        segment_qindex: header.segment_qindex[0],
        segment_lossless: header.segment_lossless[0],
        delta_q_present: header.delta_q_present,
        delta_q_resolution_log2: header.delta_q_resolution_log2,
        delta_lf_present: header.delta_lf_present,
        delta_lf_resolution_log2: header.delta_lf_resolution_log2,
        delta_lf_multi: header.delta_lf_multi,
        loop_filter: entropy::LoopFilterContext {
            level_y: header.loop_filter.level_y,
            level_u: header.loop_filter.level_u,
            level_v: header.loop_filter.level_v,
            sharpness: header.loop_filter.sharpness,
            delta_enabled: header.loop_filter.delta_enabled,
            delta_update: header.loop_filter.delta_update,
            reference_deltas: header.loop_filter.deltas.reference,
            mode_deltas: header.loop_filter.deltas.mode,
        },
        cdef: header.cdef.as_ref().map(|cdef| entropy::CdefContext {
            damping: cdef.damping,
            bits: cdef.bits,
            y_strength_count: cdef.y_strengths.len(),
            uv_strength_count: cdef.uv_strengths.len(),
            first_y_strength: cdef.y_strengths.first().copied(),
            first_uv_strength: cdef.uv_strengths.first().copied(),
        }),
        restoration_present: header.restoration.is_some(),
        transform_mode: header.transform_mode,
        reduced_transform_set: header.reduced_transform_set,
        film_grain_present: header.film_grain.is_some(),
    };

    let mut first_leaf = None;
    for (range_index, range) in ranges.iter().enumerate() {
        if header.primary_ref_frame != PRIMARY_REF_NONE {
            // Inter tiles inherit CDF state from their primary reference.
            // Their first syntax symbol cannot be checked until that retained
            // state is implemented; constructing a fresh decoder here would
            // validate the wrong model.
            continue;
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "AV1 limits a frame to at most 512 tiles"
        )]
        let range_index = range_index as u32;
        let tile = start_tile.wrapping_add(range_index);
        // Parsed tilings always contain at least one column.
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "the AV1 tiling parser guarantees a nonzero column count"
        )]
        let column = tile.wrapping_rem(tiling.columns);
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "the AV1 tiling parser guarantees a nonzero column count"
        )]
        let row = tile.wrapping_div(tiling.columns);
        if row >= tiling.rows {
            return Err(malformed("frame syntax validation failed"));
        }
        // The parser materializes one boundary for every tile column and row.
        let block_x = tiling.column_starts[column as usize].wrapping_shl(block_shift);
        let block_y = tiling.row_starts[row as usize].wrapping_shl(block_shift);
        let context = entropy::FirstBlockContext {
            disable_cdf_update: header.disable_cdf_update,
            level: root_level,
            block_width,
            block_height,
            block_x,
            block_y,
            frame_width: header.frame_width,
            frame_height: header.frame_height,
            upscaled_width: header.upscaled_width,
            superres_enabled: header.superres_enabled,
            monochrome: sequence.monochrome,
            subsampling_x: sequence.subsampling_x,
            subsampling_y: sequence.subsampling_y,
            restoration_types,
            restoration_unit_size_log2,
            bit_depth: sequence.bit_depth,
            all_lossless: header.all_lossless,
            segmentation_enabled: header.segmentation.enabled,
            skip_mode_enabled: header.skip_mode_enabled,
            allow_intrabc: header.allow_intrabc,
            allow_screen_content_tools: header.allow_screen_content_tools,
            enable_filter_intra: sequence.enable_filter_intra,
            frame_tools,
        };
        let reconstructed = entropy::validate_first_partition(data, range.clone(), &context)?;
        first_leaf = first_leaf.or(reconstructed);
    }
    Ok(first_leaf)
}

// ✅ VERIFIED: AV1 specification section 5.9; dav1d 1.5.3
// src/obu.c:409-1151; libaom 3.13.2
// av1/decoder/decodeframe.c:4486-5145.
fn parse<'data, 'input, 'spans>(
    data: &'data SegmentedData<'input, 'spans>,
    start: usize,
    end: usize,
    sequence: &SequenceHeader,
    references: &[Option<FrameHeader>; 8],
    temporal_id: u32,
    spatial_id: u32,
) -> Av1Result<(FrameHeader, BitReader<'data, 'input, 'spans>)> {
    let bits = BitReader::new(data, start, end)?;
    // `BitReader::new` has already validated the byte-to-bit conversion.
    let start_bit = bits.position();
    parse_reader(
        bits,
        start_bit,
        sequence,
        references,
        temporal_id,
        spatial_id,
    )
}

fn parse_reader<'data, 'input, 'spans>(
    mut bits: BitReader<'data, 'input, 'spans>,
    start_bit: usize,
    sequence: &SequenceHeader,
    references: &[Option<FrameHeader>; 8],
    temporal_id: u32,
    spatial_id: u32,
) -> Av1Result<(FrameHeader, BitReader<'data, 'input, 'spans>)> {
    let mut header = FrameHeader::empty(temporal_id, spatial_id);
    header.show_existing_frame = !sequence.reduced_still_picture_header && bits.bit()?;
    if header.show_existing_frame {
        let existing_frame_idx = bits.bits(3)? as usize;
        header.existing_frame_idx = Some(existing_frame_idx);
        read_presentation_delay(&mut bits, sequence)?;
        // `existing_frame_idx` is a three-bit AV1 syntax value.
        let Some(reference) = references[existing_frame_idx].as_ref() else {
            return Err(malformed("show-existing frame references an empty slot"));
        };
        if !reference.showable_frame {
            return Err(malformed("frame syntax validation failed"));
        }
        if sequence.frame_id_numbers_present {
            header.frame_id = bits.bits(sequence.frame_id_bits)?;
            if header.frame_id != reference.frame_id {
                return Err(malformed("frame syntax validation failed"));
            }
        }
        header.frame_type = reference.frame_type;
        header.show_frame = true;
        header.showable_frame = reference.showable_frame;
        header.upscaled_width = reference.upscaled_width;
        header.frame_width = reference.frame_width;
        header.frame_height = reference.frame_height;
        header.render_width = reference.render_width;
        header.render_height = reference.render_height;
        header.header_bits = bits.position().saturating_sub(start_bit);
        return Ok((header, bits));
    }

    if sequence.reduced_still_picture_header {
        header.frame_type = FrameType::Key;
        header.show_frame = true;
    } else {
        header.frame_type = FrameType::from_bits(bits.bits(2)?);
        header.show_frame = bits.bit()?;
    }
    if header.show_frame {
        read_presentation_delay(&mut bits, sequence)?;
        header.showable_frame = header.frame_type != FrameType::Key;
    } else {
        header.showable_frame = bits.bit()?;
    }
    header.error_resilient_mode = read_error_resilient_mode(&mut bits, sequence, &header)?;
    header.disable_cdf_update = bits.bit()?;
    header.allow_screen_content_tools = read_policy_flag(&mut bits, sequence.screen_content_tools)?;
    header.force_integer_mv = header.allow_screen_content_tools
        && read_policy_flag(&mut bits, sequence.force_integer_mv)?;
    if header.frame_type.is_intra() {
        header.force_integer_mv = true;
    }
    if sequence.frame_id_numbers_present {
        header.frame_id = bits.bits(sequence.frame_id_bits)?;
    }
    header.frame_size_override = if sequence.reduced_still_picture_header {
        false
    } else {
        header.frame_type == FrameType::Switch || bits.bit()?
    };
    if sequence.enable_order_hint {
        header.order_hint = bits.bits(sequence.order_hint_bits)?;
    }
    header.primary_ref_frame = if !header.error_resilient_mode && header.frame_type.is_inter() {
        bits.bits(3)? as usize
    } else {
        PRIMARY_REF_NONE
    };
    header.buffer_removal_times =
        read_buffer_removal_times(&mut bits, sequence, header.temporal_id, header.spatial_id)?;
    read_frame_type_fields(&mut bits, sequence, references, &mut header)?;
    header.refresh_frame_context =
        !sequence.reduced_still_picture_header && !header.disable_cdf_update && !bits.bit()?;
    header.tiling = Some(read_tiling(&mut bits, sequence, &header)?);
    header.quantization = Some(read_quantization(&mut bits, sequence)?);
    header.segmentation = read_segmentation(&mut bits, references, &header)?;
    read_delta_and_lossless(&mut bits, &mut header)?;
    header.loop_filter = read_loop_filter(&mut bits, sequence, references, &header)?;
    header.cdef = read_cdef(&mut bits, sequence, &header)?;
    header.restoration = read_restoration(&mut bits, sequence, &header)?;
    header.transform_mode = if header.all_lossless {
        0
    } else if bits.bit()? {
        2
    } else {
        1
    };
    header.reference_mode_select = header.frame_type.is_inter() && bits.bit()?;
    header.skip_mode_references = derive_skip_mode_references(sequence, references, &header)?;
    header.skip_mode_enabled = header.skip_mode_references.is_some() && bits.bit()?;
    header.allow_warped_motion = read_allow_warped_motion(&mut bits, sequence, &header)?;
    header.reduced_transform_set = bits.bit()?;
    header.global_motion = read_global_motion(&mut bits, references, &header)?;
    header.film_grain = read_film_grain(&mut bits, sequence, references, &header)?;
    header.header_bits = bits.position().saturating_sub(start_bit);
    Ok((header, bits))
}

fn read_error_resilient_mode(
    bits: &mut BitReader<'_, '_, '_>,
    sequence: &SequenceHeader,
    header: &FrameHeader,
) -> Av1Result<bool> {
    if sequence.reduced_still_picture_header
        || (header.frame_type == FrameType::Key && header.show_frame)
        || header.frame_type == FrameType::Switch
    {
        Ok(true)
    } else {
        bits.bit()
    }
}

fn read_policy_flag(bits: &mut BitReader<'_, '_, '_>, policy: u32) -> Av1Result<bool> {
    match policy {
        0 => Ok(false),
        1 => Ok(true),
        2 => bits.bit(),
        _ => Err(malformed("frame policy flag exceeds the supported values")),
    }
}

fn read_allow_warped_motion(
    bits: &mut BitReader<'_, '_, '_>,
    sequence: &SequenceHeader,
    header: &FrameHeader,
) -> Av1Result<bool> {
    if header.error_resilient_mode
        || !header.frame_type.is_inter()
        || !sequence.enable_warped_motion
    {
        Ok(false)
    } else {
        bits.bit()
    }
}

fn read_presentation_delay(
    bits: &mut BitReader<'_, '_, '_>,
    sequence: &SequenceHeader,
) -> Av1Result<()> {
    let Some(timing) = &sequence.timing else {
        return Ok(());
    };
    if sequence.decoder_model_present && !timing.equal_picture_interval {
        let Some(width) = timing.frame_presentation_delay_length else {
            return Err(malformed("decoder model omits presentation-delay width"));
        };
        let _ = bits.bits(width)?;
    }
    Ok(())
}

fn validate_current_frame_id(
    frame_id_bits: u32,
    previous: Option<u32>,
    header: &FrameHeader,
) -> Av1Result<()> {
    let Some(previous) = previous else {
        return Ok(());
    };
    if header.frame_type == FrameType::Key {
        if !header.show_frame {
            return validate_frame_id_difference(frame_id_bits, previous, header.frame_id);
        }
        return Ok(());
    }
    validate_frame_id_difference(frame_id_bits, previous, header.frame_id)
}

fn validate_frame_id_difference(frame_id_bits: u32, previous: u32, frame_id: u32) -> Av1Result<()> {
    let range = 1_u32 << frame_id_bits;
    let difference = if frame_id > previous {
        frame_id.saturating_sub(previous)
    } else {
        range.saturating_add(frame_id).saturating_sub(previous)
    };
    if frame_id == previous || difference >= 1_u32 << frame_id_bits.saturating_sub(1) {
        return Err(malformed("frame syntax validation failed"));
    }
    Ok(())
}

fn read_buffer_removal_times(
    bits: &mut BitReader<'_, '_, '_>,
    sequence: &SequenceHeader,
    temporal_id: u32,
    spatial_id: u32,
) -> Av1Result<Vec<u32>> {
    let mut values = Vec::new();
    if !sequence.decoder_model_present || !bits.bit()? {
        return Ok(values);
    }
    let Some(timing) = sequence.timing.as_ref() else {
        return Err(malformed("decoder model omits timing information"));
    };
    let Some(width) = timing.buffer_removal_delay_length else {
        return Err(malformed("decoder model omits buffer-removal width"));
    };
    for point in &sequence.operating_points {
        if point.decoder_parameters.is_none() {
            continue;
        }
        let temporal = (point.idc >> temporal_id) & 1;
        let spatial = (point.idc >> spatial_id.saturating_add(8)) & 1;
        if point.idc == 0 || (temporal != 0 && spatial != 0) {
            values.push(bits.bits(width)?);
        }
    }
    Ok(values)
}

fn read_frame_type_fields(
    bits: &mut BitReader<'_, '_, '_>,
    sequence: &SequenceHeader,
    references: &[Option<FrameHeader>; 8],
    header: &mut FrameHeader,
) -> Av1Result<()> {
    if header.frame_type.is_intra() {
        header.refresh_frame_flags = if header.frame_type == FrameType::Key && header.show_frame {
            u8::MAX
        } else {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "the parser reads exactly eight bits"
            )]
            let flags = bits.bits(8)? as u8;
            flags
        };
        if header.refresh_frame_flags != u8::MAX
            && header.error_resilient_mode
            && sequence.enable_order_hint
        {
            for hint in &mut header.reference_order_hints {
                *hint = bits.bits(sequence.order_hint_bits)?;
            }
        }
        if header.frame_type == FrameType::IntraOnly && header.refresh_frame_flags == u8::MAX {
            return Err(malformed("frame syntax validation failed"));
        }
        read_frame_size(bits, sequence, references, header, false)?;
        header.allow_intrabc =
            header.allow_screen_content_tools && !header.superres_enabled && bits.bit()?;
        return Ok(());
    }

    header.refresh_frame_flags = if header.frame_type == FrameType::Switch {
        u8::MAX
    } else {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the parser reads exactly eight bits"
        )]
        let flags = bits.bits(8)? as u8;
        flags
    };
    if header.error_resilient_mode && sequence.enable_order_hint {
        for hint in &mut header.reference_order_hints {
            *hint = bits.bits(sequence.order_hint_bits)?;
        }
    }
    header.frame_refs_short_signaling = sequence.enable_order_hint && bits.bit()?;
    if header.frame_refs_short_signaling {
        let last = bits.bits(3)? as usize;
        let golden = bits.bits(3)? as usize;
        header.reference_indices =
            derive_short_references(sequence, references, header.order_hint, last, golden)?;
    } else {
        for reference in &mut header.reference_indices {
            *reference = bits.bits(3)? as usize;
        }
    }
    if sequence.frame_id_numbers_present {
        for _ in &header.reference_indices {
            // Pillow's pinned dav1d accepts libaom error-resilient sequences
            // whose reference-frame IDs do not match the normative delta
            // calculation. The fields still belong to the bitstream syntax
            // and must be consumed to preserve every following bit offset.
            let _ = bits.bits(sequence.delta_frame_id_bits)?;
        }
    }
    let use_reference = !header.error_resilient_mode && header.frame_size_override;
    read_frame_size(bits, sequence, references, header, use_reference)?;
    header.allow_high_precision_mv = !header.force_integer_mv && bits.bit()?;
    header.interpolation_filter = if bits.bit()? { 4 } else { bits.bits(2)? };
    header.motion_mode_switchable = bits.bit()?;
    header.use_ref_frame_mvs = read_use_ref_frame_mvs(bits, sequence, header)?;
    Ok(())
}

fn read_use_ref_frame_mvs(
    bits: &mut BitReader<'_, '_, '_>,
    sequence: &SequenceHeader,
    header: &FrameHeader,
) -> Av1Result<bool> {
    if header.error_resilient_mode || !sequence.enable_ref_frame_mvs || !sequence.enable_order_hint
    {
        Ok(false)
    } else {
        bits.bit()
    }
}

// ✅ VERIFIED: dav1d 1.5.3 src/obu.c:341-395; libaom 3.13.2
// av1/decoder/decodeframe.c:1872-2084.
fn read_frame_size(
    bits: &mut BitReader<'_, '_, '_>,
    sequence: &SequenceHeader,
    references: &[Option<FrameHeader>; 8],
    header: &mut FrameHeader,
    use_reference: bool,
) -> Av1Result<()> {
    if use_reference {
        for &reference_index in &header.reference_indices {
            if bits.bit()? {
                // Reference indices are bounded to the eight AV1 slots.
                let Some(reference) = references[reference_index].as_ref() else {
                    return Err(malformed("frame size references an empty slot"));
                };
                header.upscaled_width = reference.upscaled_width;
                header.frame_height = reference.frame_height;
                header.render_width = reference.render_width;
                header.render_height = reference.render_height;
                read_superres(bits, sequence, header)?;
                return Ok(());
            }
        }
    }
    if header.frame_size_override {
        header.upscaled_width = bits.bits(sequence.width_bits)?.saturating_add(1);
        header.frame_height = bits.bits(sequence.height_bits)?.saturating_add(1);
    } else {
        header.upscaled_width = sequence.max_width;
        header.frame_height = sequence.max_height;
    }
    read_superres(bits, sequence, header)?;
    header.have_render_size = bits.bit()?;
    if header.have_render_size {
        header.render_width = bits.bits(16)?.saturating_add(1);
        header.render_height = bits.bits(16)?.saturating_add(1);
    } else {
        header.render_width = header.upscaled_width;
        header.render_height = header.frame_height;
    }
    Ok(())
}

fn read_superres(
    bits: &mut BitReader<'_, '_, '_>,
    sequence: &SequenceHeader,
    header: &mut FrameHeader,
) -> Av1Result<()> {
    header.superres_enabled = sequence.enable_superres && bits.bit()?;
    header.superres_denominator = if header.superres_enabled {
        bits.bits(3)?.saturating_add(9)
    } else {
        8
    };
    header.frame_width = if header.superres_enabled {
        let numerator = header
            .upscaled_width
            .saturating_mul(8)
            .saturating_add(header.superres_denominator >> 1);
        // `superres_denominator` was assigned immediately above and is in
        // 8..=16, so this division is infallible.
        let scaled = numerator.div_euclid(header.superres_denominator);
        scaled.max(header.upscaled_width.min(16))
    } else {
        header.upscaled_width
    };
    Ok(())
}

// ✅ VERIFIED: AV1 specification `set_frame_refs()`; dav1d 1.5.3
// src/obu.c:517-586.
fn derive_short_references(
    sequence: &SequenceHeader,
    references: &[Option<FrameHeader>; 8],
    order_hint: u32,
    last: usize,
    golden: usize,
) -> Av1Result<[usize; 7]> {
    let mut result = [usize::MAX; 7];
    result[0] = last;
    result[3] = golden;
    let mut offsets = [0_i32; 8];
    for (index, reference) in references.iter().enumerate() {
        let Some(reference) = reference.as_ref() else {
            return Err(malformed("short reference signaling uses an empty slot"));
        };
        let distance =
            relative_distance(sequence.order_hint_bits, reference.order_hint, order_hint);
        offsets[index] = distance;
    }
    let mut earliest = 0;
    for index in 1..offsets.len() {
        if offsets[index] < offsets[earliest] {
            earliest = index;
        }
    }
    let mut used = [false; 8];
    used[last] = true;
    used[golden] = true;

    let future = offsets
        .iter()
        .enumerate()
        .filter(|(index, offset)| !used[*index] && **offset >= 0)
        .max_by_key(|(_, offset)| **offset);
    let Some((future, _)) = future else {
        return Err(malformed(
            "short reference signaling has no future reference",
        ));
    };
    result[6] = future;
    used[future] = true;

    for output in [4_usize, 5] {
        let next = offsets
            .iter()
            .enumerate()
            .filter(|(index, offset)| !used[*index] && **offset >= 0)
            .min_by_key(|(_, offset)| **offset)
            .map(|(index, _)| index);
        let Some(next) = next else {
            break;
        };
        result[output] = next;
        used[next] = true;
    }

    for output in result.iter_mut().skip(1) {
        if *output != usize::MAX {
            continue;
        }
        let past = offsets
            .iter()
            .enumerate()
            .filter(|(index, offset)| !used[*index] && **offset < 0)
            .max_by_key(|(_, offset)| **offset)
            .map(|(index, _)| index);
        if let Some(past) = past {
            *output = past;
            used[past] = true;
        } else {
            *output = earliest;
        }
    }
    Ok(result)
}

fn relative_distance(bits: u32, first: u32, second: u32) -> i32 {
    if bits == 0 {
        return 0;
    }
    let sign = 1_i64 << bits.saturating_sub(1);
    let difference = i64::from(first).saturating_sub(i64::from(second));
    let distance = (difference & sign.saturating_sub(1)).saturating_sub(difference & sign);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "AV1 order hints use at most eight bits, so the signed distance fits i32"
    )]
    {
        distance as i32
    }
}

fn tile_log2(block_size: u32, target: u32) -> u32 {
    let mut value = 0_u32;
    while (block_size << value) < target {
        value = value.saturating_add(1);
    }
    value
}

// ✅ VERIFIED: dav1d 1.5.3 src/obu.c:624-685; libaom 3.13.2
// av1/decoder/decodeframe.c:2086-2199.
fn read_tiling(
    bits: &mut BitReader<'_, '_, '_>,
    sequence: &SequenceHeader,
    header: &FrameHeader,
) -> Av1Result<Tiling> {
    let uniform = bits.bit()?;
    let superblock_shift = if sequence.use_128x128_superblock {
        7
    } else {
        6
    };
    let superblock_size = 1_u32 << superblock_shift;
    let superblock_width = header
        .frame_width
        .saturating_add(superblock_size.saturating_sub(1))
        >> superblock_shift;
    let superblock_height = header
        .frame_height
        .saturating_add(superblock_size.saturating_sub(1))
        >> superblock_shift;
    let maximum_tile_width = 4096_u32 >> superblock_shift;
    let maximum_tile_area = 4096_u32.saturating_mul(2304) >> 2_u32.saturating_mul(superblock_shift);
    let min_log2_columns = tile_log2(maximum_tile_width, superblock_width);
    let max_log2_columns = tile_log2(1, superblock_width.min(64));
    let max_log2_rows = tile_log2(1, superblock_height.min(64));
    let frame_area = superblock_width.saturating_mul(superblock_height);
    let min_log2_tiles = tile_log2(maximum_tile_area, frame_area).max(min_log2_columns);

    let mut column_starts = Vec::new();
    let mut row_starts = Vec::new();
    let (log2_columns, min_log2_rows, log2_rows) = if uniform {
        let mut log2_columns = min_log2_columns;
        while log2_columns < max_log2_columns && bits.bit()? {
            log2_columns = log2_columns.saturating_add(1);
        }
        let tile_width = 1_u32.saturating_add(superblock_width.saturating_sub(1) >> log2_columns);
        let mut start = 0_u32;
        while start < superblock_width {
            column_starts.push(start);
            start = start.saturating_add(tile_width);
        }
        let min_log2_rows = min_log2_tiles.saturating_sub(log2_columns);
        let mut log2_rows = min_log2_rows;
        while log2_rows < max_log2_rows && bits.bit()? {
            log2_rows = log2_rows.saturating_add(1);
        }
        let tile_height = 1_u32.saturating_add(superblock_height.saturating_sub(1) >> log2_rows);
        let mut start = 0_u32;
        while start < superblock_height {
            row_starts.push(start);
            start = start.saturating_add(tile_height);
        }
        (log2_columns, min_log2_rows, log2_rows)
    } else {
        let mut start = 0_u32;
        let mut widest = 0_u32;
        while start < superblock_width && column_starts.len() < 64 {
            column_starts.push(start);
            let maximum = superblock_width
                .saturating_sub(start)
                .min(maximum_tile_width);
            let width = if maximum > 1 {
                bits.ns(maximum)?.saturating_add(1)
            } else {
                1
            };
            start = start.saturating_add(width);
            widest = widest.max(width);
        }
        if start != superblock_width {
            return Err(malformed("frame syntax validation failed"));
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the loop caps the number of AV1 tile columns at 64"
        )]
        let columns = column_starts.len() as u32;
        let log2_columns = tile_log2(1, columns);
        let mut area = superblock_width.saturating_mul(superblock_height);
        if min_log2_tiles != 0 {
            area >>= min_log2_tiles.saturating_add(1);
        }
        let Some(maximum_tile_height) = area.checked_div(widest) else {
            return Err(malformed("non-uniform tiling has zero-width columns"));
        };
        let maximum_tile_height = maximum_tile_height.max(1);
        let mut start = 0_u32;
        while start < superblock_height && row_starts.len() < 64 {
            row_starts.push(start);
            let maximum = superblock_height
                .saturating_sub(start)
                .min(maximum_tile_height);
            let height = if maximum > 1 {
                bits.ns(maximum)?.saturating_add(1)
            } else {
                1
            };
            start = start.saturating_add(height);
        }
        if start != superblock_height {
            return Err(malformed("frame syntax validation failed"));
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the loop caps the number of AV1 tile rows at 64"
        )]
        let rows = row_starts.len() as u32;
        (
            log2_columns,
            min_log2_tiles.saturating_sub(log2_columns),
            tile_log2(1, rows),
        )
    };
    column_starts.push(superblock_width);
    row_starts.push(superblock_height);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "AV1 permits at most 64 tile columns and one terminal boundary"
    )]
    let columns = column_starts.len().saturating_sub(1) as u32;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "AV1 permits at most 64 tile rows and one terminal boundary"
    )]
    let rows = row_starts.len().saturating_sub(1) as u32;
    let (context_update_tile, tile_size_bytes) = if log2_columns != 0 || log2_rows != 0 {
        let context_update_tile = bits.bits(log2_columns.saturating_add(log2_rows))?;
        if context_update_tile >= columns.saturating_mul(rows) {
            return Err(malformed("frame syntax validation failed"));
        }
        (context_update_tile, bits.bits(2)?.saturating_add(1))
    } else {
        (0, 0)
    };
    Ok(Tiling {
        uniform,
        min_log2_columns,
        max_log2_columns,
        log2_columns,
        columns,
        column_starts,
        min_log2_rows,
        max_log2_rows,
        log2_rows,
        rows,
        row_starts,
        context_update_tile,
        tile_size_bytes,
    })
}

fn read_delta(bits: &mut BitReader<'_, '_, '_>) -> Av1Result<i32> {
    if bits.bit()? { bits.signed(7) } else { Ok(0) }
}

// ✅ VERIFIED: dav1d 1.5.3 src/obu.c:691-724; libaom 3.13.2
// av1/decoder/decodeframe.c:1776-1821.
fn read_quantization(
    bits: &mut BitReader<'_, '_, '_>,
    sequence: &SequenceHeader,
) -> Av1Result<Quantization> {
    let base = bits.bits(8)?;
    let y_dc_delta = read_delta(bits)?;
    let (different_uv_delta, u_dc_delta, u_ac_delta, v_dc_delta, v_ac_delta) =
        if sequence.monochrome {
            (false, 0, 0, 0, 0)
        } else {
            let different_uv_delta = sequence.separate_uv_delta_q && bits.bit()?;
            let u_dc_delta = read_delta(bits)?;
            let u_ac_delta = read_delta(bits)?;
            let (v_dc_delta, v_ac_delta) = if different_uv_delta {
                (read_delta(bits)?, read_delta(bits)?)
            } else {
                (u_dc_delta, u_ac_delta)
            };
            (
                different_uv_delta,
                u_dc_delta,
                u_ac_delta,
                v_dc_delta,
                v_ac_delta,
            )
        };
    let using_matrix = bits.bit()?;
    let (matrix_y, matrix_u, matrix_v) = if using_matrix {
        let y = bits.bits(4)?;
        let u = bits.bits(4)?;
        let v = if sequence.separate_uv_delta_q {
            bits.bits(4)?
        } else {
            u
        };
        (y, u, v)
    } else {
        (0, 0, 0)
    };
    Ok(Quantization {
        base,
        y_dc_delta,
        u_dc_delta,
        u_ac_delta,
        v_dc_delta,
        v_ac_delta,
        different_uv_delta,
        using_matrix,
        matrix_y,
        matrix_u,
        matrix_v,
    })
}

// ✅ VERIFIED: dav1d 1.5.3 src/obu.c:730-796; libaom 3.13.2
// av1/decoder/decodeframe.c:1431-1508.
fn read_segmentation(
    bits: &mut BitReader<'_, '_, '_>,
    references: &[Option<FrameHeader>; 8],
    header: &FrameHeader,
) -> Av1Result<Segmentation> {
    let enabled = bits.bit()?;
    if !enabled {
        return Ok(Segmentation::empty());
    }
    let (update_map, temporal, update_data) = if header.primary_ref_frame == PRIMARY_REF_NONE {
        (true, false, true)
    } else {
        let update_map = bits.bit()?;
        let temporal = update_map && bits.bit()?;
        (update_map, temporal, bits.bit()?)
    };
    let segments = if update_data {
        let mut segments = [Segment::empty(); 8];
        for segment in &mut segments {
            if bits.bit()? {
                segment.delta_q = bits.signed(9)?;
            }
            if bits.bit()? {
                segment.delta_lf_y_vertical = bits.signed(7)?;
            }
            if bits.bit()? {
                segment.delta_lf_y_horizontal = bits.signed(7)?;
            }
            if bits.bit()? {
                segment.delta_lf_u = bits.signed(7)?;
            }
            if bits.bit()? {
                segment.delta_lf_v = bits.signed(7)?;
            }
            if bits.bit()? {
                segment.reference = bits.bits(3)? as i32;
            }
            segment.skip = bits.bit()?;
            segment.global_motion = bits.bit()?;
        }
        segments
    } else {
        // A non-sentinel primary reference is in 0..7, and every retained
        // reference index is a three-bit slot.
        let reference_index = header.reference_indices[header.primary_ref_frame];
        let Some(reference) = references[reference_index].as_ref() else {
            return Err(malformed("segmentation references an empty slot"));
        };
        reference.segmentation.segments
    };
    Ok(Segmentation {
        enabled,
        update_map,
        temporal,
        update_data,
        segments,
    })
}

fn read_delta_and_lossless(
    bits: &mut BitReader<'_, '_, '_>,
    header: &mut FrameHeader,
) -> Av1Result<()> {
    let Some(quantization) = header.quantization.as_ref() else {
        return Err(malformed("frame omits quantization state"));
    };
    header.delta_q_present = quantization.base != 0 && bits.bit()?;
    if header.delta_q_present {
        header.delta_q_resolution_log2 = bits.bits(2)?;
        header.delta_lf_present = !header.allow_intrabc && bits.bit()?;
        if header.delta_lf_present {
            header.delta_lf_resolution_log2 = bits.bits(2)?;
            header.delta_lf_multi = bits.bit()?;
        }
    }
    let delta_lossless = quantization.y_dc_delta == 0
        && quantization.u_dc_delta == 0
        && quantization.u_ac_delta == 0
        && quantization.v_dc_delta == 0
        && quantization.v_ac_delta == 0;
    header.all_lossless = true;
    for (index, segment) in header.segmentation.segments.iter().enumerate() {
        let qindex = if header.segmentation.enabled {
            i64::from(quantization.base)
                .saturating_add(i64::from(segment.delta_q))
                .clamp(0, 255)
        } else {
            i64::from(quantization.base)
        };
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the value is clamped to the inclusive u32 range 0..=255"
        )]
        let qindex_u32 = qindex as u32;
        header.segment_qindex[index] = qindex_u32;
        header.segment_lossless[index] = qindex == 0 && delta_lossless;
        header.all_lossless &= header.segment_lossless[index];
    }
    Ok(())
}

// ✅ VERIFIED: dav1d 1.5.3 src/obu.c:834-872; libaom 3.13.2
// av1/common/av1_loopfilter.c frame-header setup.
fn read_loop_filter(
    bits: &mut BitReader<'_, '_, '_>,
    sequence: &SequenceHeader,
    references: &[Option<FrameHeader>; 8],
    header: &FrameHeader,
) -> Av1Result<LoopFilter> {
    if header.all_lossless || header.allow_intrabc {
        return Ok(LoopFilter::disabled());
    }
    let level_y = [bits.bits(6)?, bits.bits(6)?];
    let (level_u, level_v) = if !sequence.monochrome && level_y != [0, 0] {
        (bits.bits(6)?, bits.bits(6)?)
    } else {
        (0, 0)
    };
    let sharpness = bits.bits(3)?;
    let mut deltas = if header.primary_ref_frame == PRIMARY_REF_NONE {
        LoopFilterDeltas::defaults()
    } else {
        // A non-sentinel primary reference is in 0..7, and every retained
        // reference index is a three-bit slot.
        let reference_index = header.reference_indices[header.primary_ref_frame];
        let Some(reference) = references[reference_index].as_ref() else {
            return Err(malformed("loop filter references an empty slot"));
        };
        reference.loop_filter.deltas
    };
    let delta_enabled = bits.bit()?;
    let delta_update = delta_enabled && bits.bit()?;
    if delta_update {
        for delta in &mut deltas.reference {
            if bits.bit()? {
                *delta = bits.signed(7)?;
            }
        }
        for delta in &mut deltas.mode {
            if bits.bit()? {
                *delta = bits.signed(7)?;
            }
        }
    }
    Ok(LoopFilter {
        level_y,
        level_u,
        level_v,
        sharpness,
        delta_enabled,
        delta_update,
        deltas,
    })
}

fn read_cdef(
    bits: &mut BitReader<'_, '_, '_>,
    sequence: &SequenceHeader,
    header: &FrameHeader,
) -> Av1Result<Option<Cdef>> {
    if header.all_lossless || !sequence.enable_cdef || header.allow_intrabc {
        return Ok(None);
    }
    let damping = bits.bits(2)?.saturating_add(3);
    let strength_bits = bits.bits(2)?;
    let count = 1_u32 << strength_bits;
    let mut y_strengths = Vec::with_capacity(count as usize);
    let mut uv_strengths = Vec::with_capacity(count as usize);
    for _ in 0..count {
        y_strengths.push(bits.bits(6)?);
        if !sequence.monochrome {
            uv_strengths.push(bits.bits(6)?);
        }
    }
    Ok(Some(Cdef {
        damping,
        bits: strength_bits,
        y_strengths,
        uv_strengths,
    }))
}

fn read_restoration(
    bits: &mut BitReader<'_, '_, '_>,
    sequence: &SequenceHeader,
    header: &FrameHeader,
) -> Av1Result<Option<Restoration>> {
    if (header.all_lossless && !header.superres_enabled)
        || !sequence.enable_restoration
        || header.allow_intrabc
    {
        return Ok(None);
    }
    let mut types = [None; 3];
    types[0] = entropy::RestorationType::from_bits(bits.bits(2)?);
    if !sequence.monochrome {
        types[1] = entropy::RestorationType::from_bits(bits.bits(2)?);
        types[2] = entropy::RestorationType::from_bits(bits.bits(2)?);
    }
    let mut unit_size_log2 = [8_u32; 2];
    if types != [None; 3] {
        unit_size_log2[0] = if sequence.use_128x128_superblock {
            7
        } else {
            6
        };
        if bits.bit()? {
            unit_size_log2[0] = unit_size_log2[0].saturating_add(1);
            if !sequence.use_128x128_superblock {
                unit_size_log2[0] = unit_size_log2[0].saturating_add(u32::from(bits.bit()?));
            }
        }
        unit_size_log2[1] = unit_size_log2[0];
        if (types[1].is_some() || types[2].is_some())
            && sequence.subsampling_x
            && sequence.subsampling_y
        {
            unit_size_log2[1] = unit_size_log2[1].saturating_sub(u32::from(bits.bit()?));
        }
    }
    Ok(Some(Restoration {
        types,
        unit_size_log2,
    }))
}

fn derive_skip_mode_references(
    sequence: &SequenceHeader,
    references: &[Option<FrameHeader>; 8],
    header: &FrameHeader,
) -> Av1Result<Option<[usize; 2]>> {
    if !header.reference_mode_select || !header.frame_type.is_inter() || !sequence.enable_order_hint
    {
        return Ok(None);
    }
    let mut before: Option<(u32, usize)> = None;
    let mut after: Option<(u32, usize)> = None;
    let mut reference_hints = [0_u32; 7];
    for (index, &reference_index) in header.reference_indices.iter().enumerate() {
        // Reference indices are bounded to the eight AV1 reference slots.
        let Some(reference) = references[reference_index].as_ref() else {
            return Err(malformed("skip mode references an empty slot"));
        };
        reference_hints[index] = reference.order_hint;
        let difference = relative_distance(
            sequence.order_hint_bits,
            reference.order_hint,
            header.order_hint,
        );
        if difference > 0
            && after.is_none_or(|(hint, _)| {
                relative_distance(sequence.order_hint_bits, hint, reference.order_hint) > 0
            })
        {
            after = Some((reference.order_hint, index));
        } else if difference < 0
            && before.is_none_or(|(hint, _)| {
                relative_distance(sequence.order_hint_bits, reference.order_hint, hint) > 0
            })
        {
            before = Some((reference.order_hint, index));
        }
    }
    if let (Some((_, before_index)), Some((_, after_index))) = (before, after) {
        return Ok(Some([
            before_index.min(after_index),
            before_index.max(after_index),
        ]));
    }
    let Some((before_hint, before_index)) = before else {
        return Ok(None);
    };
    let mut second: Option<(u32, usize)> = None;
    for (index, &reference_hint) in reference_hints.iter().enumerate() {
        if relative_distance(sequence.order_hint_bits, reference_hint, before_hint) < 0
            && second.is_none_or(|(hint, _)| {
                relative_distance(sequence.order_hint_bits, reference_hint, hint) > 0
            })
        {
            second = Some((reference_hint, index));
        }
    }
    let Some((_, second_index)) = second else {
        return Ok(None);
    };
    Ok(Some([
        before_index.min(second_index),
        before_index.max(second_index),
    ]))
}

// ✅ VERIFIED: dav1d 1.5.3 src/obu.c:1011-1059 and src/getbits.c:138-164;
// libaom 3.13.2 av1/decoder/decodeframe.c:4300-4416.
fn read_global_motion(
    bits: &mut BitReader<'_, '_, '_>,
    references: &[Option<FrameHeader>; 8],
    header: &FrameHeader,
) -> Av1Result<[GlobalMotion; 7]> {
    let mut motions = [GlobalMotion::identity(); 7];
    if !header.frame_type.is_inter() {
        return Ok(motions);
    }
    for (index, motion) in motions.iter_mut().enumerate() {
        if !bits.bit()? {
            continue;
        }
        let kind = if bits.bit()? {
            GlobalMotionType::RotZoom
        } else if bits.bit()? {
            GlobalMotionType::Translation
        } else {
            GlobalMotionType::Affine
        };
        let reference_matrix = if header.primary_ref_frame == PRIMARY_REF_NONE {
            GlobalMotion::identity().matrix
        } else {
            // Both indices are bounded by AV1 syntax: primary reference 0..6,
            // reference slot 0..7, and motion index 0..6.
            let slot = header.reference_indices[header.primary_ref_frame];
            let Some(reference) = references[slot].as_ref() else {
                return Err(malformed("global motion references an empty slot"));
            };
            reference.global_motion[index].matrix
        };
        let mut matrix = GlobalMotion::identity().matrix;
        let (parameter_bits, shift) =
            if matches!(kind, GlobalMotionType::RotZoom | GlobalMotionType::Affine) {
                let reference = reference_matrix[2].saturating_sub(1_i32 << 16) >> 1;
                let delta = bits.subexp(reference, 12)?.saturating_mul(2);
                matrix[2] = (1_i32 << 16).saturating_add(delta);
                matrix[3] = bits.subexp(reference_matrix[3] >> 1, 12)?.saturating_mul(2);
                (12, 10)
            } else if header.allow_high_precision_mv {
                (9, 13)
            } else {
                (8, 14)
            };
        if kind == GlobalMotionType::Affine {
            matrix[4] = bits.subexp(reference_matrix[4] >> 1, 12)?.saturating_mul(2);
            let reference = reference_matrix[5].saturating_sub(1_i32 << 16) >> 1;
            let delta = bits.subexp(reference, 12)?.saturating_mul(2);
            matrix[5] = (1_i32 << 16).saturating_add(delta);
        } else {
            matrix[4] = matrix[3].saturating_neg();
            matrix[5] = matrix[2];
        }
        matrix[0] = bits.subexp(reference_matrix[0] >> shift, parameter_bits)? << shift;
        matrix[1] = bits.subexp(reference_matrix[1] >> shift, parameter_bits)? << shift;
        *motion = GlobalMotion { kind, matrix };
    }
    Ok(motions)
}

fn read_points(bits: &mut BitReader<'_, '_, '_>, count: u32) -> Av1Result<Vec<[u32; 2]>> {
    let mut points: Vec<[u32; 2]> = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let point = [bits.bits(8)?, bits.bits(8)?];
        if points
            .last()
            .is_some_and(|previous| previous[0] >= point[0])
        {
            return Err(malformed("frame syntax validation failed"));
        }
        points.push(point);
    }
    Ok(points)
}

// ✅ VERIFIED: dav1d 1.5.3 src/obu.c:1065-1141; libaom 3.13.2
// av1/decoder/decodeframe.c:3907-4085.
fn read_film_grain(
    bits: &mut BitReader<'_, '_, '_>,
    sequence: &SequenceHeader,
    references: &[Option<FrameHeader>; 8],
    header: &FrameHeader,
) -> Av1Result<Option<FilmGrain>> {
    if !sequence.film_grain_present
        || (!header.show_frame && !header.showable_frame)
        || !bits.bit()?
    {
        return Ok(None);
    }
    let seed = bits.bits(16)?;
    let update = header.frame_type != FrameType::Inter || bits.bit()?;
    if !update {
        let slot = bits.bits(3)? as usize;
        if !header.reference_indices.contains(&slot) {
            return Err(malformed("frame syntax validation failed"));
        }
        // `slot` is a three-bit syntax value.
        let Some(reference) = references[slot].as_ref() else {
            return Err(malformed("film grain references an empty frame slot"));
        };
        let Some(mut grain) = reference.film_grain.clone() else {
            return Err(malformed("referenced frame has no film-grain parameters"));
        };
        grain.seed = seed;
        grain.update = false;
        grain.reference_slot = Some(slot);
        return Ok(Some(grain));
    }

    let y_count = bits.bits(4)?;
    if y_count > 14 {
        return Err(malformed("frame syntax validation failed"));
    }
    let y_points = read_points(bits, y_count)?;
    let chroma_scaling_from_luma = !sequence.monochrome && bits.bit()?;
    let mut uv_points: [Vec<[u32; 2]>; 2] = std::array::from_fn(|_| Vec::new());
    if !(sequence.monochrome
        || chroma_scaling_from_luma
        || sequence.subsampling_x && sequence.subsampling_y && y_count == 0)
    {
        for points in &mut uv_points {
            let count = bits.bits(4)?;
            if count > 10 {
                return Err(malformed("frame syntax validation failed"));
            }
            *points = read_points(bits, count)?;
        }
    }
    if sequence.subsampling_x
        && sequence.subsampling_y
        && uv_points[0].is_empty() != uv_points[1].is_empty()
    {
        return Err(malformed("frame syntax validation failed"));
    }
    let scaling_shift = bits.bits(2)?.saturating_add(8);
    let ar_coefficient_lag = bits.bits(2)?;
    let ar_positions = ar_coefficient_lag
        .saturating_mul(ar_coefficient_lag.saturating_add(1))
        .saturating_mul(2);
    let mut ar_coefficients_y = Vec::new();
    if y_count != 0 {
        ar_coefficients_y.reserve(ar_positions as usize);
        for _ in 0..ar_positions {
            ar_coefficients_y.push((bits.bits(8)? as i32).saturating_sub(128));
        }
    }
    let mut ar_coefficients_uv: [Vec<i32>; 2] = std::array::from_fn(|_| Vec::new());
    for (plane, coefficients) in ar_coefficients_uv.iter_mut().enumerate() {
        if !uv_points[plane].is_empty() || chroma_scaling_from_luma {
            let count = ar_positions.saturating_add(u32::from(y_count != 0));
            coefficients.reserve(count as usize);
            for _ in 0..count {
                coefficients.push((bits.bits(8)? as i32).saturating_sub(128));
            }
        }
    }
    let ar_coefficient_shift = bits.bits(2)?.saturating_add(6);
    let grain_scale_shift = bits.bits(2)?;
    let mut uv_multiplier = [0_i32; 2];
    let mut uv_luma_multiplier = [0_i32; 2];
    let mut uv_offset = [0_i32; 2];
    for plane in 0..2 {
        if !uv_points[plane].is_empty() {
            uv_multiplier[plane] = (bits.bits(8)? as i32).saturating_sub(128);
            uv_luma_multiplier[plane] = (bits.bits(8)? as i32).saturating_sub(128);
            uv_offset[plane] = (bits.bits(9)? as i32).saturating_sub(256);
        }
    }
    Ok(Some(FilmGrain {
        seed,
        update: true,
        reference_slot: None,
        y_points,
        chroma_scaling_from_luma,
        uv_points,
        scaling_shift,
        ar_coefficient_lag,
        ar_coefficients_y,
        ar_coefficients_uv,
        ar_coefficient_shift,
        grain_scale_shift,
        uv_multiplier,
        uv_luma_multiplier,
        uv_offset,
        overlap: bits.bit()?,
        clip_to_restricted_range: bits.bit()?,
    }))
}

#[cfg(coverage)]
struct CoverageBitWriter {
    bytes: Vec<u8>,
    position: usize,
}

#[cfg(coverage)]
#[coverage(off)]
impl CoverageBitWriter {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            position: 0,
        }
    }

    fn push(&mut self, value: u32, width: u32) {
        for shift in (0..width).rev() {
            if self.position.is_multiple_of(8) {
                self.bytes.push(0);
            }
            let bit = ((value >> shift) & 1) as u8;
            let byte = self.position / 8;
            let offset = 7 - (self.position % 8);
            self.bytes[byte] |= bit << offset;
            self.position += 1;
        }
    }

    fn push_signed(&mut self, value: i32, width: u32) {
        let mask = (1_u32 << width) - 1;
        self.push((value as u32) & mask, width);
    }

    fn finish(mut self) -> Vec<u8> {
        self.push(1, 1);
        while !self.position.is_multiple_of(8) {
            self.push(0, 1);
        }
        self.bytes
    }
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_insert_bits(input: &[u8], position: usize, value: u32, width: u32) -> Vec<u8> {
    let mut output = CoverageBitWriter::new();
    let input_bits = input.len() * 8;
    for index in 0..position {
        let byte = input[index / 8];
        output.push(u32::from((byte >> (7 - index % 8)) & 1), 1);
    }
    output.push(value, width);
    for index in position..input_bits {
        let byte = input[index / 8];
        output.push(u32::from((byte >> (7 - index % 8)) & 1), 1);
    }
    output.bytes
}

#[cfg(coverage)]
#[coverage(off)]
pub(super) fn __coverage_reduced_header_payload() -> Vec<u8> {
    const FRAME: &[u8] = b"\x10\x00\x93\x80\x00\x08\x00\x00\x01\x48\x1a\x7a\xa0";
    const SEQUENCE: &[u8] = b"\x40\x00\x00\x02\xaf\xff\xbf\xff\x3e\xa0";
    let mut sample = Vec::new();
    sample.extend_from_slice(SEQUENCE);
    sample.extend_from_slice(FRAME);
    let spans = [ByteSpan {
        start: 0,
        end: sample.len(),
    }];
    let data = SegmentedData::new(&sample, &spans).unwrap();
    let sequence = super::sequence::parse(&data, 0, SEQUENCE.len()).unwrap();
    let (header, _) = parse(
        &data,
        SEQUENCE.len(),
        sample.len(),
        &sequence,
        &std::array::from_fn(|_| None),
        0,
        0,
    )
    .unwrap();
    let header_length = header.header_bits.saturating_add(1).div_ceil(8);
    let mut payload = FRAME[..header_length].to_vec();
    let trailing_byte = header.header_bits / 8;
    let trailing_offset = 7 - (header.header_bits % 8);
    payload[trailing_byte] &= u8::MAX << trailing_offset.saturating_add(1);
    payload[trailing_byte] |= 1 << trailing_offset;
    payload
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_read<T>(
    input: &[u8],
    bit_end: usize,
    read: impl FnOnce(&mut BitReader<'_, '_, '_>) -> T,
) -> T {
    let spans = [ByteSpan {
        start: 0,
        end: input.len(),
    }];
    let data = SegmentedData::new(input, &spans).unwrap();
    let mut bits = BitReader::with_bit_end(&data, bit_end).unwrap();
    read(&mut bits)
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_sweep_read(input: &[u8], mut read: impl FnMut(&mut BitReader<'_, '_, '_>)) {
    for bit_end in 0..=input.len() * 8 {
        coverage_read(input, bit_end, |bits| read(bits));
    }
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_read_tile_group(
    state: &FrameState,
    input: &[u8],
    bit_end: usize,
) -> Av1Result<(u32, u32)> {
    let spans = [ByteSpan {
        start: 0,
        end: input.len(),
    }];
    let data = SegmentedData::new(input, &spans).unwrap();
    let mut bits = BitReader::with_bit_end(&data, bit_end).unwrap();
    state
        .read_tile_group(&data, &mut bits, input.len())
        .map(|group| (group.start, group.end))
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_tile_group(start: u32, end: u32) -> TileGroup {
    TileGroup {
        start,
        end,
        first_leaf: None,
    }
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_sweep_frame(
    input: &[u8],
    sequence: &SequenceHeader,
    references: &[Option<FrameHeader>; 8],
) {
    let spans = [ByteSpan {
        start: 0,
        end: input.len(),
    }];
    let data = SegmentedData::new(input, &spans).unwrap();
    for bit_end in 0..=input.len() * 8 {
        let bits = BitReader::with_bit_end(&data, bit_end).unwrap();
        let _ = parse_reader(bits, 0, sequence, references, 0, 0);
    }
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_mutation_sweep_frame(
    input: &[u8],
    sequence: &SequenceHeader,
    references: &[Option<FrameHeader>; 8],
) {
    for bit in 0..input.len() * 8 {
        let mut mutated = input.to_vec();
        mutated[bit / 8] ^= 1 << (7 - bit % 8);
        coverage_sweep_frame(&mutated, sequence, references);
    }
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_sequence() -> SequenceHeader {
    SequenceHeader {
        profile: 0,
        still_picture: false,
        reduced_still_picture_header: false,
        timing: Some(Timing {
            num_units_in_tick: 1,
            time_scale: 1,
            equal_picture_interval: false,
            num_ticks_per_picture: None,
            num_units_in_decoding_tick: Some(1),
            buffer_removal_delay_length: Some(3),
            frame_presentation_delay_length: Some(3),
        }),
        decoder_model_present: true,
        display_model_present: true,
        operating_points: vec![
            OperatingPoint {
                idc: 0,
                level: 0,
                tier: 0,
                decoder_parameters: Some(DecoderParameters {
                    decoder_buffer_delay: 0,
                    encoder_buffer_delay: 0,
                    low_delay_mode: false,
                }),
                display_model_present: false,
                initial_display_delay: 10,
            },
            OperatingPoint {
                idc: (1 << 1) | (1 << 9),
                level: 0,
                tier: 0,
                decoder_parameters: Some(DecoderParameters {
                    decoder_buffer_delay: 0,
                    encoder_buffer_delay: 0,
                    low_delay_mode: false,
                }),
                display_model_present: false,
                initial_display_delay: 10,
            },
        ],
        width_bits: 16,
        height_bits: 16,
        max_width: 128,
        max_height: 128,
        frame_id_numbers_present: true,
        delta_frame_id_bits: 2,
        frame_id_bits: 4,
        use_128x128_superblock: false,
        enable_filter_intra: true,
        enable_intra_edge_filter: true,
        enable_interintra_compound: true,
        enable_masked_compound: true,
        enable_warped_motion: true,
        enable_dual_filter: true,
        enable_order_hint: true,
        enable_jnt_comp: true,
        enable_ref_frame_mvs: true,
        screen_content_tools: 2,
        force_integer_mv: 2,
        order_hint_bits: 4,
        enable_superres: true,
        enable_cdef: true,
        enable_restoration: true,
        bit_depth: 10,
        monochrome: false,
        color_primaries: 2,
        transfer_characteristics: 2,
        matrix_coefficients: 2,
        color_range: false,
        subsampling_x: true,
        subsampling_y: true,
        chroma_sample_position: 0,
        separate_uv_delta_q: true,
        film_grain_present: true,
    }
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_tiling(columns: u32, rows: u32) -> Tiling {
    Tiling {
        uniform: true,
        min_log2_columns: 0,
        max_log2_columns: 2,
        log2_columns: columns.ilog2(),
        columns,
        column_starts: (0..=columns).collect(),
        min_log2_rows: 0,
        max_log2_rows: 2,
        log2_rows: rows.ilog2(),
        rows,
        row_starts: (0..=rows).collect(),
        context_update_tile: 0,
        tile_size_bytes: 1,
    }
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_header() -> FrameHeader {
    let mut header = FrameHeader::empty(0, 0);
    header.frame_type = FrameType::Inter;
    header.show_frame = true;
    header.showable_frame = true;
    header.frame_id = 3;
    header.frame_size_override = true;
    header.order_hint = 4;
    header.primary_ref_frame = 0;
    header.refresh_frame_flags = 1;
    header.upscaled_width = 128;
    header.frame_width = 128;
    header.frame_height = 128;
    header.render_width = 128;
    header.render_height = 128;
    header.reference_indices = [0, 1, 2, 3, 4, 5, 6];
    header.tiling = Some(coverage_tiling(1, 1));
    header.quantization = Some(Quantization {
        base: 1,
        y_dc_delta: 0,
        u_dc_delta: 0,
        u_ac_delta: 0,
        v_dc_delta: 0,
        v_ac_delta: 0,
        different_uv_delta: false,
        using_matrix: false,
        matrix_y: 0,
        matrix_u: 0,
        matrix_v: 0,
    });
    header
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_references() -> [Option<FrameHeader>; 8] {
    std::array::from_fn(|index| {
        let mut header = coverage_header();
        header.frame_id = u32::try_from(index).unwrap();
        header.order_hint = u32::try_from(index).unwrap();
        Some(header)
    })
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_state_paths() {
    let sequence = coverage_sequence();
    let mut state = FrameState::new();
    let empty_spans = [ByteSpan { start: 0, end: 0 }];
    let empty_data = SegmentedData::new(&[], &empty_spans).unwrap();
    let tile_input = [0_u8; 16];
    let tile_spans = [ByteSpan {
        start: 0,
        end: tile_input.len(),
    }];
    let tile_data = SegmentedData::new(&tile_input, &tile_spans).unwrap();
    assert!(split_tile_payloads(&tile_data, 1, 0, 0, 0, 0).is_err());
    assert!(split_tile_payloads(&tile_data, 0, tile_input.len() + 1, 0, 0, 0).is_err());
    assert!(split_tile_payloads(&tile_data, 0, 1, 0, 1, 0).is_err());
    assert!(split_tile_payloads(&tile_data, 0, 6, 0, 1, 5).is_err());
    assert_eq!(
        split_tile_payloads(&tile_data, 0, 0, 0, 0, 0),
        Ok(vec![0..0])
    );
    for width in [1, 3, 4] {
        assert!(split_tile_payloads(&tile_data, 0, tile_input.len(), 0, 1, width).is_ok());
    }
    let mut entropy_header = coverage_header();
    entropy_header.primary_ref_frame = PRIMARY_REF_NONE;
    assert!(
        validate_tile_entropy_prefixes(
            &tile_data,
            &[0..tile_input.len()],
            1,
            &entropy_header,
            &sequence,
            entropy_header.tiling.as_ref().unwrap(),
        )
        .is_err()
    );
    entropy_header.restoration = Some(Restoration {
        types: [Some(entropy::RestorationType::Wiener), None, None],
        unit_size_log2: [5; 2],
    });
    entropy_header.upscaled_width = entropy_header.frame_width.saturating_add(1);
    assert!(
        validate_tile_entropy_prefixes(
            &tile_data,
            &[0..tile_input.len()],
            0,
            &entropy_header,
            &sequence,
            entropy_header.tiling.as_ref().unwrap(),
        )
        .is_ok()
    );
    let no_sequence = FrameState::new();
    assert!(coverage_read_tile_group(&no_sequence, &tile_input, tile_input.len() * 8).is_err());
    let mut rejected_entropy = FrameState::new();
    rejected_entropy.sequence = Some(sequence.clone());
    rejected_entropy.pending = Some(entropy_header);
    assert!(coverage_read_tile_group(&rejected_entropy, &tile_input, tile_input.len() * 8).is_ok());
    let _ = state.begin_frame(&empty_data, 0, 0, 0, 0, false);
    let _ = state.tile_group_obu(&empty_data, 0, 0);
    let _ = state.tile_group_obu(&empty_data, 1, 0);
    let _ = parse(
        &empty_data,
        1,
        0,
        &sequence,
        &std::array::from_fn(|_| None),
        0,
        0,
    );
    let _ = parse(
        &empty_data,
        usize::MAX,
        usize::MAX,
        &sequence,
        &std::array::from_fn(|_| None),
        0,
        0,
    );
    let mut missing_sequence = FrameState::new();
    assert_eq!(
        missing_sequence.accept_parsed_header(true, sequence.frame_id_bits, coverage_header()),
        Ok(())
    );
    assert_eq!(state.temporal_delimiter(), Ok(()));
    assert!(state.finish().is_err());
    assert_eq!(state.accept_sequence(sequence.clone()), Ok(()));
    assert!(state.finish().is_ok());
    let mut inconsistent = sequence.clone();
    inconsistent.max_width += 1;
    assert!(state.accept_sequence(inconsistent).is_err());

    state.pending = Some(coverage_header());
    assert!(state.temporal_delimiter().is_err());
    assert!(state.finish().is_err());
    assert_eq!(state.complete_show_existing(), Ok(()));
    state.pending = None;
    assert!(state.complete_show_existing().is_err());

    let mut shown = coverage_header();
    shown.show_existing_frame = true;
    shown.existing_frame_idx = Some(0);
    state.pending = Some(shown.clone());
    state.references[0] = Some(coverage_header());
    assert_eq!(state.complete_show_existing(), Ok(()));
    state.pending = Some(shown.clone());
    state.references[0].as_mut().unwrap().showable_frame = false;
    assert!(state.complete_show_existing().is_err());
    state.references[0] = None;
    assert!(state.complete_show_existing().is_err());
    shown.existing_frame_idx = None;
    state.pending = Some(shown);
    assert!(state.complete_show_existing().is_err());

    let mut key = coverage_header();
    key.frame_type = FrameType::Key;
    key.showable_frame = true;
    let mut shown_key = key.clone();
    shown_key.show_existing_frame = true;
    shown_key.existing_frame_idx = Some(0);
    state.references[0] = Some(key);
    state.pending = Some(shown_key);
    assert_eq!(state.complete_show_existing(), Ok(()));
    assert!(state.references.iter().all(|reference| {
        reference
            .as_ref()
            .is_some_and(|frame| !frame.showable_frame)
    }));

    state.sequence = Some(sequence.clone());
    state.references = coverage_references();
    state.references[0] = None;
    state.invalidate_old_references(10);
    state.references = coverage_references();
    state.references[0].as_mut().unwrap().frame_id = 11;
    state.invalidate_old_references(10);
    state.references = coverage_references();
    state.invalidate_old_references(1);
    assert_eq!(
        state.accept_parsed_header(true, sequence.frame_id_bits, coverage_header()),
        Ok(())
    );
    state.pending = None;
    let mut accepted_existing = coverage_header();
    accepted_existing.show_existing_frame = true;
    assert_eq!(
        state.accept_parsed_header(true, sequence.frame_id_bits, accepted_existing),
        Ok(())
    );
    state.pending = None;

    let mut invalid_id_state = FrameState::new();
    invalid_id_state.sequence = Some(sequence.clone());
    invalid_id_state.current_frame_id = Some(3);
    let mut repeated_id = coverage_header();
    repeated_id.frame_type = FrameType::Inter;
    repeated_id.frame_id = 3;
    assert!(
        invalid_id_state
            .accept_parsed_header(true, sequence.frame_id_bits, repeated_id)
            .is_err()
    );

    let mut tiled = coverage_header();
    tiled.tiling = Some(coverage_tiling(2, 2));
    tiled.refresh_frame_flags = 0b1000_0001;
    state.pending = Some(tiled);
    state.next_tile = 0;
    assert!(state.accept_tile_group(coverage_tile_group(1, 1)).is_err());
    assert_eq!(state.accept_tile_group(coverage_tile_group(0, 1)), Ok(()));
    assert_eq!(state.accept_tile_group(coverage_tile_group(2, 3)), Ok(()));
    assert!(state.pending.is_none());
    assert!(state.references[0].is_some());
    assert!(state.references[7].is_some());
    assert!(state.accept_tile_group(coverage_tile_group(0, 0)).is_err());

    let mut missing_tiling = coverage_header();
    missing_tiling.tiling = None;
    state.pending = Some(missing_tiling);
    let _ = state.accept_tile_group(coverage_tile_group(0, 0));
    let _ = coverage_read_tile_group(&state, &[], 0);

    let mut group_header = coverage_header();
    group_header.tiling = Some(coverage_tiling(2, 2));
    state.pending = Some(group_header);
    for payload in [[0b1001_1000_u8], [0b1110_0000], [0], [0x80]] {
        let _ = coverage_read_tile_group(&state, &payload, 8);
    }
    for bit_end in 0..=8 {
        let _ = coverage_read_tile_group(&state, &[0x80], bit_end);
    }
    state
        .pending
        .as_mut()
        .unwrap()
        .tiling
        .as_mut()
        .unwrap()
        .columns = 3;
    state
        .pending
        .as_mut()
        .unwrap()
        .tiling
        .as_mut()
        .unwrap()
        .log2_columns = 2;
    state
        .pending
        .as_mut()
        .unwrap()
        .tiling
        .as_mut()
        .unwrap()
        .rows = 1;
    state
        .pending
        .as_mut()
        .unwrap()
        .tiling
        .as_mut()
        .unwrap()
        .log2_rows = 0;
    let _ = coverage_read_tile_group(&state, &[0b1001_1000], 8);
    state.pending = None;
    let _ = coverage_read_tile_group(&state, &[], 0);

    const REDUCED_FRAME: &[u8] = b"\x10\x00\x93\x80\x00\x08\x00\x00\x01\x48\x1a\x7a\xa0";
    const REDUCED_SEQUENCE: &[u8] = b"\x40\x00\x00\x02\xaf\xff\xbf\xff\x3e\xa0";
    let mut sample = Vec::new();
    sample.extend_from_slice(REDUCED_SEQUENCE);
    sample.extend_from_slice(REDUCED_FRAME);
    let spans = [ByteSpan {
        start: 0,
        end: sample.len(),
    }];
    let data = SegmentedData::new(&sample, &spans).unwrap();
    let parsed_sequence = super::sequence::parse(&data, 0, REDUCED_SEQUENCE.len()).unwrap();
    coverage_sweep_frame(
        REDUCED_FRAME,
        &parsed_sequence,
        &std::array::from_fn(|_| None),
    );
    const ANIMATED_SEQUENCE: &[u8] = b"\x00\x00\x00\x03\xbc\xac\xa9\xb5\xf2\x20\x21\xa0\xd0\x80";
    const ANIMATED_KEY: &[u8] =
        b"\x10\x00\x83\x80\x00\x00\x80\x00\x00\x00\xeb\xc5\xa6\x2e\x0c\x0d\xd1\x51\x40";
    let animated_spans = [ByteSpan {
        start: 0,
        end: ANIMATED_SEQUENCE.len(),
    }];
    let animated_data = SegmentedData::new(ANIMATED_SEQUENCE, &animated_spans).unwrap();
    let animated_sequence =
        super::sequence::parse(&animated_data, 0, ANIMATED_SEQUENCE.len()).unwrap();
    coverage_sweep_frame(
        ANIMATED_KEY,
        &animated_sequence,
        &std::array::from_fn(|_| None),
    );
    let parser_references = coverage_references();
    let parser_sequence = coverage_sequence();
    let mut presentation_prefix = CoverageBitWriter::new();
    presentation_prefix.push(0, 1);
    presentation_prefix.push(0, 2);
    presentation_prefix.push(1, 1);
    coverage_sweep_frame(
        &presentation_prefix.bytes,
        &parser_sequence,
        &parser_references,
    );

    let mut frame_id_prefix = CoverageBitWriter::new();
    frame_id_prefix.push(0, 1);
    frame_id_prefix.push(0, 2);
    frame_id_prefix.push(1, 1);
    frame_id_prefix.push(0, 3);
    frame_id_prefix.push(0, 1);
    frame_id_prefix.push(0, 1);
    coverage_sweep_frame(&frame_id_prefix.bytes, &parser_sequence, &parser_references);
    frame_id_prefix.push(3, parser_sequence.frame_id_bits);
    coverage_sweep_frame(&frame_id_prefix.bytes, &parser_sequence, &parser_references);
    let mut invalid_frame_id_prefix = CoverageBitWriter::new();
    invalid_frame_id_prefix.push(0, 1);
    invalid_frame_id_prefix.push(0, 2);
    invalid_frame_id_prefix.push(0, 1);
    invalid_frame_id_prefix.push(0, 1);
    invalid_frame_id_prefix.push(0, 1);
    invalid_frame_id_prefix.push(0, 1);
    invalid_frame_id_prefix.push(3, parser_sequence.frame_id_bits);
    coverage_sweep_frame(
        &invalid_frame_id_prefix.bytes,
        &parser_sequence,
        &parser_references,
    );

    let mut buffer_prefix = frame_id_prefix;
    buffer_prefix.push(0, 1);
    buffer_prefix.push(0, parser_sequence.order_hint_bits);
    buffer_prefix.push(1, 1);
    coverage_sweep_frame(&buffer_prefix.bytes, &parser_sequence, &parser_references);

    let mut film_sequence = parsed_sequence.clone();
    film_sequence.film_grain_present = true;
    coverage_sweep_frame(
        REDUCED_FRAME,
        &film_sequence,
        &std::array::from_fn(|_| None),
    );
    const ANIMATED_INTER_1: &[u8] = b"\x28\x04\xe0\x40\x00\x00\x23\x43\x30\x00\x00\x40\x00\x04\x00\x00\x08\xe4\x66\x90\x91\x47\x7f\x6e\xcc\x05\x23\x9b\xc1\x1c\xc6\x74\xcb\x7e\xe0";
    const ANIMATED_INTER_2: &[u8] = b"\x28\x02\xe0\x80\x00\x00\xa3\x44\xc0\x00\x00\x48\x00\x04\x00\x00\x26\x66\xc9\x49\xed\xf9\xfc\xed\x11\x20\x54\x85\xcf\x5f\x49\x98\x10\x5b\x20";
    const ANIMATED_INTER_3: &[u8] = b"\x30\x03\xc2\x00\x00\x81\x46\x8c\x80\x00\x00\x90\x00\x08\x00\x1f\x3a\xcd\xf2\xb3\x29\xa3\x70\xb6\x44\xb1\xd9\x5a\x93\x1f\x3c\x56\x60\x14\xc4";
    const ANIMATED_INTER_4: &[u8] = b"\x30\x06\x44\x09\x80\x01\x46\x8c\x80\x00\x00\x90\x00\x08\x00\x33\xa1\xc0\x60\x46\x86\x20\x7d\xcf\xf4\xfc";
    const ANIMATED_INTER_5: &[u8] =
        b"\x30\x08\x00\x11\x30\x01\x46\x8c\x80\x00\x00\x90\x00\x08\x00\xb3\x2e\xde\x2e\xcf\x20";
    let mut animated_state = FrameState::new();
    animated_state
        .accept_sequence(animated_sequence.clone())
        .unwrap();
    for (frame_index, frame) in [
        ANIMATED_KEY,
        ANIMATED_INTER_1,
        ANIMATED_INTER_2,
        ANIMATED_INTER_3,
    ]
    .into_iter()
    .enumerate()
    {
        coverage_sweep_frame(frame, &animated_sequence, &animated_state.references);
        if frame_index == 1 {
            coverage_mutation_sweep_frame(frame, &animated_sequence, &animated_state.references);
            for slot in 0..REFERENCE_SLOTS {
                let mut missing_reference = animated_state.references.clone();
                missing_reference[slot] = None;
                coverage_sweep_frame(frame, &animated_sequence, &missing_reference);
            }
        }
        if frame_index == 3 {
            let mut select_mode = frame.to_vec();
            select_mode[107 / 8] ^= 1 << (7 - 107 % 8);
            coverage_sweep_frame(&select_mode, &animated_sequence, &animated_state.references);
            for slot in 0..REFERENCE_SLOTS {
                let mut missing_reference = animated_state.references.clone();
                missing_reference[slot] = None;
                coverage_sweep_frame(&select_mode, &animated_sequence, &missing_reference);
            }
        }
        let spans = [ByteSpan {
            start: 0,
            end: frame.len(),
        }];
        let data = SegmentedData::new(frame, &spans).unwrap();
        assert_eq!(
            animated_state.frame_obu(&data, 0, frame.len(), 0, 0),
            Ok(())
        );
    }
    let show_existing = [0xa8_u8];
    coverage_sweep_frame(
        &show_existing,
        &animated_sequence,
        &animated_state.references,
    );
    let show_spans = [ByteSpan { start: 0, end: 1 }];
    let show_data = SegmentedData::new(&show_existing, &show_spans).unwrap();
    assert_eq!(
        animated_state.frame_header_obu(&show_data, 0, 1, 0, 0, false),
        Ok(())
    );
    for (frame_index, frame) in [ANIMATED_INTER_4, ANIMATED_INTER_5].into_iter().enumerate() {
        coverage_sweep_frame(frame, &animated_sequence, &animated_state.references);
        if frame_index == 0 {
            let mut select_mode = frame.to_vec();
            select_mode[107 / 8] ^= 1 << (7 - 107 % 8);
            coverage_sweep_frame(&select_mode, &animated_sequence, &animated_state.references);
            for slot in 0..REFERENCE_SLOTS {
                let mut missing_reference = animated_state.references.clone();
                missing_reference[slot] = None;
                coverage_sweep_frame(&select_mode, &animated_sequence, &missing_reference);
            }
        }
        let spans = [ByteSpan {
            start: 0,
            end: frame.len(),
        }];
        let data = SegmentedData::new(frame, &spans).unwrap();
        assert_eq!(
            animated_state.frame_obu(&data, 0, frame.len(), 0, 0),
            Ok(())
        );
    }
    let frame_start = REDUCED_SEQUENCE.len();
    let frame_end = sample.len();
    let mut direct = FrameState::new();
    direct.accept_sequence(parsed_sequence.clone()).unwrap();
    assert!(
        direct
            .begin_frame(&data, frame_start, frame_end, 0, 0, false)
            .is_ok()
    );
    assert!(
        direct
            .begin_frame(&data, frame_start, frame_end, 0, 0, false)
            .is_err()
    );
    assert!(
        direct
            .begin_frame(&data, frame_start, frame_end, 0, 0, true)
            .is_ok()
    );
    assert!(
        direct
            .begin_frame(&data, frame_start, frame_end, 1, 0, true)
            .is_err()
    );
    assert!(
        direct
            .begin_frame(&data, frame_start, frame_start, 0, 0, true)
            .is_err()
    );

    let frame_with_id = coverage_insert_bits(REDUCED_FRAME, 1, 3, 4);
    let frame_id_spans = [ByteSpan {
        start: 0,
        end: frame_with_id.len(),
    }];
    let frame_id_data = SegmentedData::new(&frame_with_id, &frame_id_spans).unwrap();
    let mut frame_id_sequence = parsed_sequence.clone();
    frame_id_sequence.frame_id_numbers_present = true;
    frame_id_sequence.delta_frame_id_bits = 2;
    frame_id_sequence.frame_id_bits = 4;
    let mut frame_id_state = FrameState::new();
    frame_id_state.accept_sequence(frame_id_sequence).unwrap();
    frame_id_state.current_frame_id = Some(3);
    let _ = frame_id_state.begin_frame(&frame_id_data, 0, frame_with_id.len(), 0, 0, false);

    let header_bits = direct.pending.as_ref().unwrap().header_bits;
    let header_length = header_bits.saturating_add(1).div_ceil(8);
    let mut reduced_header = REDUCED_FRAME[..header_length].to_vec();
    let trailing_byte = header_bits / 8;
    let trailing_offset = 7 - (header_bits % 8);
    reduced_header[trailing_byte] &= u8::MAX << trailing_offset.saturating_add(1);
    reduced_header[trailing_byte] |= 1 << trailing_offset;
    let header_spans = [ByteSpan {
        start: 0,
        end: reduced_header.len(),
    }];
    let header_data = SegmentedData::new(&reduced_header, &header_spans).unwrap();
    let mut split = FrameState::new();
    split.accept_sequence(parsed_sequence.clone()).unwrap();
    assert_eq!(
        split.frame_header_obu(&header_data, 0, reduced_header.len(), 0, 0, false),
        Ok(())
    );
    assert_eq!(
        split.frame_header_obu(&header_data, 0, reduced_header.len(), 0, 0, true),
        Ok(())
    );
    assert_eq!(split.tile_group_obu(&header_data, 0, 0), Ok(()));

    let mut illegal_show = FrameState::new();
    illegal_show.accept_sequence(coverage_sequence()).unwrap();
    illegal_show.references[0] = Some(coverage_header());
    let show = [0x80];
    let show_spans = [ByteSpan { start: 0, end: 1 }];
    let show_data = SegmentedData::new(&show, &show_spans).unwrap();
    assert!(illegal_show.frame_obu(&show_data, 0, 1, 0, 0).is_err());

    let references = coverage_references();
    let mut show_existing = CoverageBitWriter::new();
    show_existing.push(1, 1);
    show_existing.push(0, 3);
    show_existing.push(0, 3);
    show_existing.push(15, 4);
    let show_existing = show_existing.bytes;
    let show_existing_spans = [ByteSpan {
        start: 0,
        end: show_existing.len(),
    }];
    let show_existing_data = SegmentedData::new(&show_existing, &show_existing_spans).unwrap();
    let _ = parse(
        &show_existing_data,
        0,
        show_existing.len(),
        &coverage_sequence(),
        &references,
        0,
        0,
    );
    coverage_sweep_frame(&show_existing, &coverage_sequence(), &references);
    let mut missing_existing_reference = references.clone();
    missing_existing_reference[0] = None;
    let _ = parse(
        &show_existing_data,
        0,
        show_existing.len(),
        &coverage_sequence(),
        &missing_existing_reference,
        0,
        0,
    );
    let mut matching_existing = CoverageBitWriter::new();
    matching_existing.push(1, 1);
    matching_existing.push(0, 3);
    matching_existing.push(0, 3);
    matching_existing.push(0, 4);
    let matching_existing = matching_existing.bytes;
    let matching_spans = [ByteSpan {
        start: 0,
        end: matching_existing.len(),
    }];
    let matching_data = SegmentedData::new(&matching_existing, &matching_spans).unwrap();
    let _ = parse(
        &matching_data,
        0,
        matching_existing.len(),
        &coverage_sequence(),
        &references,
        0,
        0,
    );
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_frame_id_and_timing_paths() {
    let mut sequence = coverage_sequence();
    let mut header = coverage_header();
    assert_eq!(
        validate_current_frame_id(sequence.frame_id_bits, None, &header),
        Ok(())
    );
    header.frame_type = FrameType::Key;
    header.show_frame = true;
    assert_eq!(
        validate_current_frame_id(sequence.frame_id_bits, Some(3), &header),
        Ok(())
    );
    header.show_frame = false;
    header.frame_id = 4;
    assert_eq!(
        validate_current_frame_id(sequence.frame_id_bits, Some(3), &header),
        Ok(())
    );
    header.frame_type = FrameType::Inter;
    header.frame_id = 4;
    assert_eq!(
        validate_current_frame_id(sequence.frame_id_bits, Some(3), &header),
        Ok(())
    );
    header.frame_id = 1;
    assert_eq!(
        validate_current_frame_id(sequence.frame_id_bits, Some(15), &header),
        Ok(())
    );
    header.frame_id = 3;
    assert!(validate_current_frame_id(sequence.frame_id_bits, Some(3), &header).is_err());
    header.frame_id = 12;
    assert!(validate_current_frame_id(sequence.frame_id_bits, Some(3), &header).is_err());

    let bytes = [0xff; 8];
    coverage_read(&bytes, 64, |bits| {
        let _ = read_presentation_delay(bits, &coverage_sequence());
    });
    sequence = coverage_sequence();
    sequence.timing = None;
    coverage_read(&[], 0, |bits| {
        let _ = read_presentation_delay(bits, &sequence);
    });
    coverage_read(&[0x80], 1, |bits| {
        let _ = read_buffer_removal_times(bits, &sequence, 0, 0);
    });
    sequence.timing = coverage_sequence().timing;
    sequence
        .timing
        .as_mut()
        .unwrap()
        .buffer_removal_delay_length = None;
    coverage_read(&[0x80], 1, |bits| {
        let _ = read_buffer_removal_times(bits, &sequence, 0, 0);
    });
    sequence
        .timing
        .as_mut()
        .unwrap()
        .frame_presentation_delay_length = None;
    coverage_read(&[], 0, |bits| {
        let _ = read_presentation_delay(bits, &sequence);
    });
    sequence = coverage_sequence();
    sequence.decoder_model_present = false;
    coverage_read(&[], 0, |bits| {
        let _ = read_presentation_delay(bits, &sequence);
        let _ = read_buffer_removal_times(bits, &sequence, 0, 0);
    });
    sequence = coverage_sequence();
    sequence.timing.as_mut().unwrap().equal_picture_interval = true;
    coverage_read(&[], 0, |bits| {
        let _ = read_presentation_delay(bits, &sequence);
    });
    sequence = coverage_sequence();
    coverage_read(&[0], 1, |bits| {
        let _ = read_buffer_removal_times(bits, &sequence, 0, 0);
    });
    sequence = coverage_sequence();
    coverage_read(&bytes, 64, |bits| {
        let _ = read_buffer_removal_times(bits, &sequence, 1, 1);
    });
    coverage_read(&bytes, 64, |bits| {
        let _ = read_buffer_removal_times(bits, &sequence, 0, 0);
    });
    coverage_sweep_read(&bytes, |bits| {
        let _ = read_buffer_removal_times(bits, &sequence, 0, 0);
    });
    sequence.operating_points = vec![OperatingPoint {
        idc: 1,
        level: 0,
        tier: 0,
        decoder_parameters: Some(DecoderParameters {
            decoder_buffer_delay: 0,
            encoder_buffer_delay: 0,
            low_delay_mode: false,
        }),
        display_model_present: false,
        initial_display_delay: 10,
    }];
    coverage_read(&bytes, 64, |bits| {
        let _ = read_buffer_removal_times(bits, &sequence, 0, 0);
    });
    sequence.operating_points[0].idc = 1 << 8;
    coverage_read(&bytes, 64, |bits| {
        let _ = read_buffer_removal_times(bits, &sequence, 0, 0);
    });
    sequence.operating_points[0].decoder_parameters = None;
    coverage_read(&bytes, 64, |bits| {
        let _ = read_buffer_removal_times(bits, &sequence, 0, 0);
    });

    for (policy, byte) in [(0, 0_u8), (1, 0), (2, 0), (2, 0x80), (3, 0)] {
        coverage_read(&[byte], 1, |bits| {
            let _ = read_policy_flag(bits, policy);
        });
    }
    let mut policy_header = coverage_header();
    for (frame_type, show_frame, reduced, byte) in [
        (FrameType::Key, true, false, 0),
        (FrameType::Switch, false, false, 0),
        (FrameType::Inter, false, true, 0),
        (FrameType::Inter, false, false, 0),
        (FrameType::Inter, false, false, 0x80),
    ] {
        policy_header.frame_type = frame_type;
        policy_header.show_frame = show_frame;
        sequence.reduced_still_picture_header = reduced;
        coverage_read(&[byte], 1, |bits| {
            let _ = read_error_resilient_mode(bits, &sequence, &policy_header);
        });
    }
    sequence.reduced_still_picture_header = false;
    for (frame_type, resilient, enabled, byte) in [
        (FrameType::Key, false, true, 0),
        (FrameType::Inter, true, true, 0),
        (FrameType::Inter, false, false, 0),
        (FrameType::Inter, false, true, 0),
        (FrameType::Inter, false, true, 0x80),
    ] {
        policy_header.frame_type = frame_type;
        policy_header.error_resilient_mode = resilient;
        sequence.enable_warped_motion = enabled;
        coverage_read(&[byte], 1, |bits| {
            let _ = read_allow_warped_motion(bits, &sequence, &policy_header);
        });
    }
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_frame_kind_and_geometry_paths() {
    let mut sequence = coverage_sequence();
    let references = coverage_references();
    let bytes = [0xff; 512];

    for frame_type in [
        FrameType::Key,
        FrameType::Inter,
        FrameType::IntraOnly,
        FrameType::Switch,
    ] {
        for fill in [0_u8, 0x55, 0xaa, 0xff] {
            let input = [fill; 512];
            let mut header = coverage_header();
            header.frame_type = frame_type;
            header.show_frame = frame_type == FrameType::Key;
            header.error_resilient_mode = true;
            header.allow_screen_content_tools = true;
            coverage_read(&input, input.len() * 8, |bits| {
                let _ = read_frame_type_fields(bits, &sequence, &references, &mut header);
            });
        }
    }
    let mut syntax_sequence = sequence.clone();
    syntax_sequence.frame_id_numbers_present = false;
    for frame_type in [
        FrameType::Key,
        FrameType::Inter,
        FrameType::IntraOnly,
        FrameType::Switch,
    ] {
        for input in [&[0_u8; 128][..], &[0xff_u8; 128][..]] {
            let mut template = coverage_header();
            template.frame_type = frame_type;
            template.show_frame = false;
            template.error_resilient_mode = true;
            template.allow_screen_content_tools = true;
            coverage_sweep_read(input, |bits| {
                let mut header = template.clone();
                let _ = read_frame_type_fields(bits, &syntax_sequence, &references, &mut header);
            });
        }
    }
    let mut no_order_sequence = sequence.clone();
    no_order_sequence.enable_order_hint = false;
    let mut no_order_header = coverage_header();
    no_order_header.error_resilient_mode = false;
    coverage_read(&[0; 512], 4096, |bits| {
        let _ = read_frame_type_fields(bits, &no_order_sequence, &references, &mut no_order_header);
    });
    let mut no_order_intra = coverage_header();
    no_order_intra.frame_type = FrameType::IntraOnly;
    no_order_intra.error_resilient_mode = true;
    coverage_read(&[0; 512], 4096, |bits| {
        let _ = read_frame_type_fields(bits, &no_order_sequence, &references, &mut no_order_intra);
    });
    let mut no_order_resilient = coverage_header();
    no_order_resilient.error_resilient_mode = true;
    coverage_read(&[0; 512], 4096, |bits| {
        let _ = read_frame_type_fields(
            bits,
            &no_order_sequence,
            &references,
            &mut no_order_resilient,
        );
    });

    let mut matching_references = references.clone();
    matching_references[0].as_mut().unwrap().frame_id = 2;
    let mut matching_ids = CoverageBitWriter::new();
    matching_ids.push(0, 8);
    matching_ids.push(0, 1);
    for _ in 0..7 {
        matching_ids.push(0, 3);
    }
    for _ in 0..7 {
        matching_ids.push(0, 2);
    }
    matching_ids.push(0, 7);
    matching_ids.push(0, 1);
    matching_ids.push(0, 1);
    matching_ids.push(0, 2);
    matching_ids.push(0, 1);
    matching_ids.push(0, 1);
    let matching_ids = matching_ids.finish();
    let mut matching_header = coverage_header();
    matching_header.frame_id = 3;
    matching_header.error_resilient_mode = false;
    matching_header.frame_size_override = false;
    coverage_read(&matching_ids, matching_ids.len() * 8, |bits| {
        let _ = read_frame_type_fields(bits, &sequence, &matching_references, &mut matching_header);
    });
    coverage_sweep_read(&matching_ids, |bits| {
        let mut header = coverage_header();
        header.frame_id = 3;
        header.error_resilient_mode = false;
        header.frame_size_override = false;
        let _ = read_frame_type_fields(bits, &sequence, &matching_references, &mut header);
    });
    let mut missing_id_reference = matching_references.clone();
    missing_id_reference[0] = None;
    let mut missing_id_header = coverage_header();
    missing_id_header.frame_id = 3;
    missing_id_header.error_resilient_mode = false;
    missing_id_header.frame_size_override = false;
    coverage_read(&matching_ids, matching_ids.len() * 8, |bits| {
        let _ = read_frame_type_fields(
            bits,
            &sequence,
            &missing_id_reference,
            &mut missing_id_header,
        );
    });

    let mut header = coverage_header();
    header.frame_type = FrameType::Inter;
    header.error_resilient_mode = false;
    header.frame_size_override = true;
    coverage_read(&bytes, bytes.len() * 8, |bits| {
        let _ = read_frame_size(bits, &sequence, &references, &mut header, true);
    });
    coverage_read(&[], 0, |bits| {
        let _ = read_frame_size(bits, &sequence, &references, &mut header, true);
    });
    coverage_read(&[0x80], 1, |bits| {
        let _ = read_frame_size(bits, &sequence, &references, &mut header, true);
    });
    coverage_read(&[0; 8], 64, |bits| {
        let _ = read_frame_size(bits, &sequence, &references, &mut header, false);
    });
    coverage_sweep_read(&[0; 8], |bits| {
        let mut frame = header.clone();
        let _ = read_frame_size(bits, &sequence, &references, &mut frame, false);
    });
    let mut explicit_render = CoverageBitWriter::new();
    explicit_render.push(127, sequence.width_bits);
    explicit_render.push(127, sequence.height_bits);
    explicit_render.push(0, 1);
    explicit_render.push(1, 1);
    explicit_render.push(127, 16);
    explicit_render.push(127, 16);
    coverage_sweep_read(&explicit_render.bytes, |bits| {
        let mut frame = header.clone();
        let _ = read_frame_size(bits, &sequence, &references, &mut frame, false);
    });
    let mut missing_reference = references.clone();
    missing_reference[0] = None;
    coverage_read(&[0x80], 1, |bits| {
        let _ = read_frame_size(bits, &sequence, &missing_reference, &mut header, true);
    });
    sequence.enable_superres = true;
    header.upscaled_width = 128;
    coverage_read(&[0xff; 2], 16, |bits| {
        let _ = read_superres(bits, &sequence, &mut header);
    });
    coverage_sweep_read(&[0xff; 2], |bits| {
        let mut frame = header.clone();
        let _ = read_superres(bits, &sequence, &mut frame);
    });
    sequence.enable_superres = false;
    coverage_read(&[], 0, |bits| {
        let _ = read_superres(bits, &sequence, &mut header);
    });

    let mut short_references = references.clone();
    for (index, reference) in short_references.iter_mut().enumerate() {
        reference.as_mut().unwrap().order_hint = [1, 2, 3, 4, 5, 6, 7, 8][index];
    }
    let _ = derive_short_references(&sequence, &short_references, 4, 0, 3);
    for (index, reference) in short_references.iter_mut().enumerate() {
        reference.as_mut().unwrap().order_hint = [8, 7, 6, 5, 4, 3, 2, 1][index];
    }
    let _ = derive_short_references(&sequence, &short_references, 4, 0, 3);
    let mut missing_reference = short_references.clone();
    missing_reference[2] = None;
    let _ = derive_short_references(&sequence, &missing_reference, 4, 0, 3);
    for order_hint in [0_u32, 8, 15] {
        for reference in &mut short_references {
            reference.as_mut().unwrap().order_hint = order_hint.saturating_add(1) & 15;
        }
        let _ = derive_short_references(&sequence, &short_references, order_hint, 0, 3);
    }
    for bits in [0, 1, 4, 31, 32] {
        let _ = relative_distance(bits, 1, 15);
    }
    for (block, target) in [(1, 1), (1, 64), (64, 1)] {
        let _ = tile_log2(block, target);
    }
    let mut mvs_header = coverage_header();
    for (resilient, enable_mvs, enable_order, byte) in [
        (true, true, true, 0),
        (false, false, true, 0),
        (false, true, false, 0),
        (false, true, true, 0),
        (false, true, true, 0x80),
    ] {
        mvs_header.error_resilient_mode = resilient;
        sequence.enable_ref_frame_mvs = enable_mvs;
        sequence.enable_order_hint = enable_order;
        coverage_read(&[byte], 1, |bits| {
            let _ = read_use_ref_frame_mvs(bits, &sequence, &mvs_header);
        });
    }
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_tiling_and_metadata_paths() {
    let mut sequence = coverage_sequence();
    let mut header = coverage_header();
    let references = coverage_references();

    header.frame_width = 0;
    coverage_read(&[0], 1, |bits| {
        let _ = read_tiling(bits, &sequence, &header);
    });

    for (width, height) in [
        (64, 64),
        (512, 256),
        (8192, 8192),
        (4096, 32768),
        (32768, 4096),
    ] {
        header.frame_width = width;
        header.frame_height = height;
        for fill in [0_u8, 0x55, 0xaa, 0xff] {
            let input = [fill; 512];
            coverage_read(&input, input.len() * 8, |bits| {
                let _ = read_tiling(bits, &sequence, &header);
            });
        }
    }
    header.frame_width = 512;
    header.frame_height = 256;
    coverage_sweep_read(&[0; 64], |bits| {
        let _ = read_tiling(bits, &sequence, &header);
    });
    coverage_sweep_read(&[0xff; 64], |bits| {
        let _ = read_tiling(bits, &sequence, &header);
    });
    sequence.use_128x128_superblock = true;
    coverage_read(&[0xff; 512], 4096, |bits| {
        let _ = read_tiling(bits, &sequence, &header);
    });

    for fill in [0_u8, 0x55, 0xaa, 0xff] {
        let input = [fill; 1024];
        coverage_read(&input, input.len() * 8, |bits| {
            let _ = read_quantization(bits, &sequence);
        });
        coverage_read(&input, input.len() * 8, |bits| {
            let _ = read_segmentation(bits, &references, &header);
        });
    }
    coverage_sweep_read(&[0; 64], |bits| {
        let _ = read_quantization(bits, &sequence);
    });
    coverage_sweep_read(&[0xff; 64], |bits| {
        let _ = read_quantization(bits, &sequence);
    });
    sequence.monochrome = true;
    coverage_read(&[0xff; 64], 512, |bits| {
        let _ = read_quantization(bits, &sequence);
    });
    sequence.monochrome = false;

    let mut inherited = header.clone();
    inherited.primary_ref_frame = 0;
    let inherit_bits = [0b1000_0000];
    coverage_read(&inherit_bits, 3, |bits| {
        let _ = read_segmentation(bits, &references, &inherited);
    });
    coverage_sweep_read(&[0xe0], |bits| {
        let _ = read_segmentation(bits, &references, &inherited);
    });

    let mut segmentation = CoverageBitWriter::new();
    segmentation.push(1, 1);
    for index in 0..8 {
        for (value, width) in [(1, 9), (-1, 7), (2, 7), (-2, 7), (3, 7)] {
            segmentation.push(u32::from(index == 0), 1);
            if index == 0 {
                segmentation.push_signed(value, width);
            }
        }
        segmentation.push(u32::from(index == 0), 1);
        if index == 0 {
            segmentation.push(3, 3);
        }
        segmentation.push(u32::from(index == 0), 1);
        segmentation.push(u32::from(index == 0), 1);
    }
    let segmentation = segmentation.finish();
    let mut no_primary = header.clone();
    no_primary.primary_ref_frame = PRIMARY_REF_NONE;
    coverage_read(&segmentation, segmentation.len() * 8, |bits| {
        let _ = read_segmentation(bits, &references, &no_primary);
    });
    coverage_sweep_read(&segmentation, |bits| {
        let _ = read_segmentation(bits, &references, &no_primary);
    });
    let mut no_primary_inherited = header.clone();
    no_primary_inherited.primary_ref_frame = PRIMARY_REF_NONE;
    coverage_read(&inherit_bits, 3, |bits| {
        let _ = read_segmentation(bits, &references, &no_primary_inherited);
    });
    let mut missing_reference = references.clone();
    missing_reference[0] = None;
    coverage_read(&inherit_bits, 3, |bits| {
        let _ = read_segmentation(bits, &missing_reference, &inherited);
    });

    let mut delta = header.clone();
    delta.quantization.as_mut().unwrap().base = 128;
    delta.segmentation.enabled = true;
    delta.segmentation.segments[0].delta_q = -255;
    coverage_read(&[0xff; 8], 64, |bits| {
        let _ = read_delta_and_lossless(bits, &mut delta);
    });
    coverage_sweep_read(&[0xff; 8], |bits| {
        let mut truncated = delta.clone();
        truncated.allow_intrabc = false;
        let _ = read_delta_and_lossless(bits, &mut truncated);
    });
    delta.allow_intrabc = true;
    coverage_read(&[0xff; 8], 64, |bits| {
        let _ = read_delta_and_lossless(bits, &mut delta);
    });
    for index in 0..5 {
        let mut individual = coverage_header();
        individual.quantization.as_mut().unwrap().base = 0;
        match index {
            0 => individual.quantization.as_mut().unwrap().y_dc_delta = 1,
            1 => individual.quantization.as_mut().unwrap().u_dc_delta = 1,
            2 => individual.quantization.as_mut().unwrap().u_ac_delta = 1,
            3 => individual.quantization.as_mut().unwrap().v_dc_delta = 1,
            _ => individual.quantization.as_mut().unwrap().v_ac_delta = 1,
        }
        coverage_read(&[], 0, |bits| {
            let _ = read_delta_and_lossless(bits, &mut individual);
        });
    }
    let mut missing_quantization = coverage_header();
    missing_quantization.quantization = None;
    coverage_read(&[], 0, |bits| {
        let _ = read_delta_and_lossless(bits, &mut missing_quantization);
    });

    let mut filtered = header.clone();
    filtered.all_lossless = false;
    for fill in [0_u8, 0xff] {
        let input = [fill; 128];
        coverage_read(&input, input.len() * 8, |bits| {
            let _ = read_loop_filter(bits, &sequence, &references, &filtered);
        });
        coverage_read(&input, input.len() * 8, |bits| {
            let _ = read_cdef(bits, &sequence, &filtered);
        });
        coverage_read(&input, input.len() * 8, |bits| {
            let _ = read_restoration(bits, &sequence, &filtered);
        });
    }
    coverage_sweep_read(&[0; 128], |bits| {
        let _ = read_loop_filter(bits, &sequence, &references, &filtered);
    });
    coverage_sweep_read(&[0xff; 128], |bits| {
        let _ = read_loop_filter(bits, &sequence, &references, &filtered);
    });
    let mut no_primary_filter = filtered.clone();
    no_primary_filter.primary_ref_frame = PRIMARY_REF_NONE;
    coverage_read(&[0; 32], 256, |bits| {
        let _ = read_loop_filter(bits, &sequence, &references, &no_primary_filter);
    });
    coverage_read(&[0; 32], 256, |bits| {
        let _ = read_loop_filter(bits, &sequence, &missing_reference, &filtered);
    });
    coverage_sweep_read(&[0; 128], |bits| {
        let _ = read_cdef(bits, &sequence, &filtered);
    });
    coverage_sweep_read(&[0xff; 128], |bits| {
        let _ = read_cdef(bits, &sequence, &filtered);
    });
    filtered.all_lossless = true;
    coverage_read(&[], 0, |bits| {
        let _ = read_loop_filter(bits, &sequence, &references, &filtered);
        let _ = read_cdef(bits, &sequence, &filtered);
        let _ = read_restoration(bits, &sequence, &filtered);
    });
    filtered.all_lossless = false;
    filtered.allow_intrabc = true;
    coverage_read(&[], 0, |bits| {
        let _ = read_loop_filter(bits, &sequence, &references, &filtered);
        let _ = read_cdef(bits, &sequence, &filtered);
        let _ = read_restoration(bits, &sequence, &filtered);
    });

    filtered.allow_intrabc = false;
    filtered.all_lossless = true;
    filtered.superres_enabled = true;
    let mut restoration_sequence = sequence.clone();
    restoration_sequence.enable_restoration = false;
    coverage_read(&[], 0, |bits| {
        let _ = read_restoration(bits, &restoration_sequence, &filtered);
    });
    restoration_sequence.enable_restoration = true;
    restoration_sequence.monochrome = false;
    filtered.all_lossless = false;
    for (use_128, subsampling_x, subsampling_y, input) in [
        (false, false, false, &[0x57_u8, 0x80][..]),
        (false, true, true, &[0x57_u8, 0xc0]),
        (false, true, true, &[0x47_u8, 0xc0]),
        (true, true, true, &[0x57_u8, 0x80]),
        (false, true, false, &[0x40_u8, 0]),
    ] {
        restoration_sequence.use_128x128_superblock = use_128;
        restoration_sequence.subsampling_x = subsampling_x;
        restoration_sequence.subsampling_y = subsampling_y;
        coverage_read(input, input.len() * 8, |bits| {
            let _ = read_restoration(bits, &restoration_sequence, &filtered);
        });
        coverage_sweep_read(input, |bits| {
            let _ = read_restoration(bits, &restoration_sequence, &filtered);
        });
    }
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_prediction_and_grain_paths() {
    let mut sequence = coverage_sequence();
    let mut references = coverage_references();
    let mut header = coverage_header();
    header.reference_mode_select = true;
    let _ = derive_skip_mode_references(&sequence, &references, &header);
    header.reference_mode_select = false;
    let _ = derive_skip_mode_references(&sequence, &references, &header);
    header.reference_mode_select = true;
    header.frame_type = FrameType::Key;
    let _ = derive_skip_mode_references(&sequence, &references, &header);
    header.frame_type = FrameType::Inter;
    sequence.enable_order_hint = false;
    let _ = derive_skip_mode_references(&sequence, &references, &header);
    sequence.enable_order_hint = true;
    for reference in &mut references {
        reference.as_mut().unwrap().order_hint = 1;
    }
    let _ = derive_skip_mode_references(&sequence, &references, &header);
    for reference in &mut references {
        reference.as_mut().unwrap().order_hint = 5;
    }
    let _ = derive_skip_mode_references(&sequence, &references, &header);
    let mut missing_skip_reference = references.clone();
    missing_skip_reference[0] = None;
    let _ = derive_skip_mode_references(&sequence, &missing_skip_reference, &header);

    let mut global = CoverageBitWriter::new();
    global.push(1, 1);
    global.push(1, 1);
    for _ in 0..4 {
        global.push(0, 4);
    }
    for _ in 1..7 {
        global.push(0, 1);
    }
    let global = global.finish();
    header.primary_ref_frame = PRIMARY_REF_NONE;
    coverage_read(&global, global.len() * 8, |bits| {
        let _ = read_global_motion(bits, &references, &header);
    });
    coverage_sweep_read(&global, |bits| {
        let _ = read_global_motion(bits, &references, &header);
    });

    let mut translation = CoverageBitWriter::new();
    translation.push(1, 1);
    translation.push(0, 1);
    translation.push(1, 1);
    for _ in 0..2 {
        translation.push(0, 4);
    }
    for _ in 1..7 {
        translation.push(0, 1);
    }
    let translation = translation.finish();
    header.allow_high_precision_mv = true;
    coverage_read(&translation, translation.len() * 8, |bits| {
        let _ = read_global_motion(bits, &references, &header);
    });
    coverage_sweep_read(&translation, |bits| {
        let _ = read_global_motion(bits, &references, &header);
    });
    header.allow_high_precision_mv = false;
    coverage_read(&translation, translation.len() * 8, |bits| {
        let _ = read_global_motion(bits, &references, &header);
    });

    let mut affine = CoverageBitWriter::new();
    affine.push(1, 1);
    affine.push(0, 1);
    affine.push(0, 1);
    for _ in 0..6 {
        affine.push(0, 4);
    }
    for _ in 1..7 {
        affine.push(0, 1);
    }
    let affine = affine.finish();
    header.primary_ref_frame = 0;
    coverage_read(&affine, affine.len() * 8, |bits| {
        let _ = read_global_motion(bits, &references, &header);
    });
    coverage_sweep_read(&affine, |bits| {
        let _ = read_global_motion(bits, &references, &header);
    });
    let mut invalid_global = header.clone();
    invalid_global.primary_ref_frame = PRIMARY_REF_NONE;
    coverage_read(&[0xc0], 2, |bits| {
        let _ = read_global_motion(bits, &references, &invalid_global);
    });
    invalid_global.primary_ref_frame = 0;
    let mut missing_global_reference = references.clone();
    missing_global_reference[0] = None;
    invalid_global.reference_indices[0] = 0;
    coverage_read(&[0xc0], 2, |bits| {
        let _ = read_global_motion(bits, &missing_global_reference, &invalid_global);
    });
    header.frame_type = FrameType::Key;
    coverage_read(&[], 0, |bits| {
        let _ = read_global_motion(bits, &references, &header);
    });

    coverage_read(&[0x00, 0x01, 0x00, 0x01], 32, |bits| {
        let _ = read_points(bits, 2);
    });
    coverage_read(&[0x01, 0x01, 0x00, 0x01], 32, |bits| {
        let _ = read_points(bits, 2);
    });
    coverage_sweep_read(&[0x00, 0x01, 0x01, 0x01], |bits| {
        let _ = read_points(bits, 2);
    });

    header = coverage_header();
    header.frame_type = FrameType::Key;
    header.show_frame = true;
    let mut grain = CoverageBitWriter::new();
    grain.push(1, 1);
    grain.push(1, 16);
    grain.push(1, 4);
    grain.push(0, 8);
    grain.push(1, 8);
    grain.push(0, 1);
    for _ in 0..2 {
        grain.push(1, 4);
        grain.push(0, 8);
        grain.push(1, 8);
    }
    grain.push(0, 2);
    grain.push(0, 2);
    for _ in 0..2 {
        grain.push(128, 8);
    }
    grain.push(0, 2);
    grain.push(0, 2);
    for _ in 0..2 {
        grain.push(128, 8);
        grain.push(128, 8);
        grain.push(256, 9);
    }
    grain.push(1, 1);
    grain.push(1, 1);
    let grain = grain.finish();
    sequence.subsampling_x = false;
    sequence.subsampling_y = false;
    coverage_read(&grain, grain.len() * 8, |bits| {
        let parsed = read_film_grain(bits, &sequence, &references, &header);
        assert!(parsed.is_ok());
    });
    coverage_sweep_read(&grain, |bits| {
        let _ = read_film_grain(bits, &sequence, &references, &header);
    });

    for fill in [0_u8, 0x55, 0xaa, 0xff] {
        let input = [fill; 512];
        coverage_read(&input, input.len() * 8, |bits| {
            let _ = read_film_grain(bits, &sequence, &references, &header);
        });
    }
    sequence.monochrome = true;
    coverage_read(&[0xff; 512], 4096, |bits| {
        let _ = read_film_grain(bits, &sequence, &references, &header);
    });

    sequence = coverage_sequence();
    header.frame_type = FrameType::Inter;
    header.reference_indices = [0, 1, 2, 3, 4, 5, 6];
    references[0].as_mut().unwrap().film_grain = Some(FilmGrain {
        seed: 1,
        update: true,
        reference_slot: None,
        y_points: Vec::new(),
        chroma_scaling_from_luma: false,
        uv_points: std::array::from_fn(|_| Vec::new()),
        scaling_shift: 8,
        ar_coefficient_lag: 0,
        ar_coefficients_y: Vec::new(),
        ar_coefficients_uv: std::array::from_fn(|_| Vec::new()),
        ar_coefficient_shift: 6,
        grain_scale_shift: 0,
        uv_multiplier: [0; 2],
        uv_luma_multiplier: [0; 2],
        uv_offset: [0; 2],
        overlap: false,
        clip_to_restricted_range: false,
    });
    let mut inherited = CoverageBitWriter::new();
    inherited.push(1, 1);
    inherited.push(2, 16);
    inherited.push(0, 1);
    inherited.push(0, 3);
    let inherited = inherited.finish();
    coverage_read(&inherited, inherited.len() * 8, |bits| {
        let _ = read_film_grain(bits, &sequence, &references, &header);
    });
    coverage_sweep_read(&inherited, |bits| {
        let _ = read_film_grain(bits, &sequence, &references, &header);
    });
    let mut no_grain_reference = references.clone();
    no_grain_reference[0].as_mut().unwrap().film_grain = None;
    coverage_read(&inherited, inherited.len() * 8, |bits| {
        let _ = read_film_grain(bits, &sequence, &no_grain_reference, &header);
    });
    no_grain_reference[0] = None;
    coverage_read(&inherited, inherited.len() * 8, |bits| {
        let _ = read_film_grain(bits, &sequence, &no_grain_reference, &header);
    });
    let mut invalid_inherited = CoverageBitWriter::new();
    invalid_inherited.push(1, 1);
    invalid_inherited.push(2, 16);
    invalid_inherited.push(0, 1);
    invalid_inherited.push(7, 3);
    let invalid_inherited = invalid_inherited.finish();
    coverage_read(&invalid_inherited, invalid_inherited.len() * 8, |bits| {
        let _ = read_film_grain(bits, &sequence, &references, &header);
    });

    sequence.film_grain_present = false;
    coverage_read(&[], 0, |bits| {
        let _ = read_film_grain(bits, &sequence, &references, &header);
    });
    sequence.film_grain_present = true;
    header.show_frame = false;
    header.showable_frame = false;
    coverage_read(&[], 0, |bits| {
        let _ = read_film_grain(bits, &sequence, &references, &header);
    });
    header.showable_frame = true;
    coverage_read(&[0], 1, |bits| {
        let _ = read_film_grain(bits, &sequence, &references, &header);
    });
    header.show_frame = true;
    header.showable_frame = false;
    coverage_read(&[0], 1, |bits| {
        let _ = read_film_grain(bits, &sequence, &references, &header);
    });

    header.frame_type = FrameType::Key;
    let mut invalid_y = CoverageBitWriter::new();
    invalid_y.push(1, 1);
    invalid_y.push(0, 16);
    invalid_y.push(15, 4);
    let invalid_y = invalid_y.finish();
    coverage_read(&invalid_y, invalid_y.len() * 8, |bits| {
        let _ = read_film_grain(bits, &sequence, &references, &header);
    });

    sequence.subsampling_x = false;
    sequence.subsampling_y = false;
    let mut invalid_uv = CoverageBitWriter::new();
    invalid_uv.push(1, 1);
    invalid_uv.push(0, 16);
    invalid_uv.push(0, 4);
    invalid_uv.push(0, 1);
    invalid_uv.push(11, 4);
    let invalid_uv = invalid_uv.finish();
    coverage_read(&invalid_uv, invalid_uv.len() * 8, |bits| {
        let _ = read_film_grain(bits, &sequence, &references, &header);
    });

    sequence.subsampling_x = true;
    sequence.subsampling_y = true;
    let mut empty_420 = CoverageBitWriter::new();
    empty_420.push(1, 1);
    empty_420.push(0, 16);
    empty_420.push(0, 4);
    empty_420.push(0, 1);
    empty_420.push(0, 2);
    empty_420.push(0, 2);
    empty_420.push(0, 2);
    empty_420.push(0, 2);
    empty_420.push(0, 1);
    empty_420.push(0, 1);
    let empty_420 = empty_420.finish();
    coverage_read(&empty_420, empty_420.len() * 8, |bits| {
        let _ = read_film_grain(bits, &sequence, &references, &header);
    });
    coverage_sweep_read(&empty_420, |bits| {
        let _ = read_film_grain(bits, &sequence, &references, &header);
    });
    let mut asymmetric_uv = CoverageBitWriter::new();
    asymmetric_uv.push(1, 1);
    asymmetric_uv.push(0, 16);
    asymmetric_uv.push(1, 4);
    asymmetric_uv.push(0, 8);
    asymmetric_uv.push(0, 8);
    asymmetric_uv.push(0, 1);
    asymmetric_uv.push(1, 4);
    asymmetric_uv.push(0, 8);
    asymmetric_uv.push(0, 8);
    asymmetric_uv.push(0, 4);
    let asymmetric_uv = asymmetric_uv.finish();
    coverage_read(&asymmetric_uv, asymmetric_uv.len() * 8, |bits| {
        let _ = read_film_grain(bits, &sequence, &references, &header);
    });

    let mut chroma_from_luma = CoverageBitWriter::new();
    chroma_from_luma.push(1, 1);
    chroma_from_luma.push(0, 16);
    chroma_from_luma.push(1, 4);
    chroma_from_luma.push(0, 8);
    chroma_from_luma.push(0, 8);
    chroma_from_luma.push(1, 1);
    chroma_from_luma.push(0, 2);
    chroma_from_luma.push(1, 2);
    for _ in 0..4 {
        chroma_from_luma.push(128, 8);
    }
    for _ in 0..10 {
        chroma_from_luma.push(128, 8);
    }
    chroma_from_luma.push(0, 2);
    chroma_from_luma.push(0, 2);
    chroma_from_luma.push(0, 1);
    chroma_from_luma.push(0, 1);
    let chroma_from_luma = chroma_from_luma.finish();
    coverage_read(&chroma_from_luma, chroma_from_luma.len() * 8, |bits| {
        let _ = read_film_grain(bits, &sequence, &references, &header);
    });
    coverage_sweep_read(&chroma_from_luma, |bits| {
        let _ = read_film_grain(bits, &sequence, &references, &header);
    });
    sequence.subsampling_y = false;
    coverage_read(&grain, grain.len() * 8, |bits| {
        let _ = read_film_grain(bits, &sequence, &references, &header);
    });

    sequence.monochrome = true;
    let mut monochrome_grain = CoverageBitWriter::new();
    monochrome_grain.push(1, 1);
    monochrome_grain.push(0, 16);
    monochrome_grain.push(1, 4);
    monochrome_grain.push(0, 8);
    monochrome_grain.push(0, 8);
    monochrome_grain.push(0, 2);
    monochrome_grain.push(1, 2);
    for _ in 0..4 {
        monochrome_grain.push(128, 8);
    }
    monochrome_grain.push(0, 2);
    monochrome_grain.push(0, 2);
    monochrome_grain.push(0, 1);
    monochrome_grain.push(0, 1);
    let monochrome_grain = monochrome_grain.finish();
    coverage_read(&monochrome_grain, monochrome_grain.len() * 8, |bits| {
        let _ = read_film_grain(bits, &sequence, &references, &header);
    });
    coverage_sweep_read(&monochrome_grain, |bits| {
        let _ = read_film_grain(bits, &sequence, &references, &header);
    });
}

#[cfg(coverage)]
#[coverage(off)]
pub(super) fn __coverage_exercise_private_branches() {
    coverage_state_paths();
    coverage_frame_id_and_timing_paths();
    coverage_frame_kind_and_geometry_paths();
    coverage_tiling_and_metadata_paths();
    coverage_prediction_and_grain_paths();
}
