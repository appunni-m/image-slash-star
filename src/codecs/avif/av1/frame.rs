//! Complete AV1 uncompressed-frame-header syntax and reference state.

use std::{ops::Range, sync::Arc};

use super::bit_reader::{BitReader, SegmentedData};
use super::entropy;
use super::geometry::PixelLayout;
use super::motion::{
    GlobalMotion, GlobalMotionType, ProjectedTemporalField, ReferenceFrame, ReferencePair,
    RetainedTemporalSample, ScaleFactors, TemporalMotionField, load_projected_temporal_field,
    relative_distance,
};
use super::resize;
use super::restoration;
use super::sample_depth::SampleDepth;
use super::sequence::SequenceHeader;
use super::surface::{FramePlane, FrameSurface};
use super::{Av1Result, malformed};
use crate::codecs::CodecError;

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
    features: u8,
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
            features: 0,
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
    preskip: bool,
    last_active_id: i32,
}

impl Segmentation {
    const fn empty() -> Self {
        Self {
            enabled: false,
            update_map: false,
            temporal: false,
            update_data: false,
            segments: [Segment::empty(); 8],
            preskip: false,
            last_active_id: 0,
        }
    }
}

const SEG_ALT_Q: u8 = 1 << 0;
const SEG_ALT_LF_Y_VERTICAL: u8 = 1 << 1;
const SEG_ALT_LF_Y_HORIZONTAL: u8 = 1 << 2;
const SEG_ALT_LF_U: u8 = 1 << 3;
const SEG_ALT_LF_V: u8 = 1 << 4;
const SEG_REFERENCE: u8 = 1 << 5;
const SEG_SKIP: u8 = 1 << 6;
const SEG_GLOBAL_MOTION: u8 = 1 << 7;

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

#[derive(Clone, PartialEq, Eq)]
pub(super) struct FilmGrain {
    pub(super) seed: u32,
    pub(super) update: bool,
    pub(super) reference_slot: Option<usize>,
    pub(super) y_points: Vec<[u32; 2]>,
    pub(super) chroma_scaling_from_luma: bool,
    pub(super) uv_points: [Vec<[u32; 2]>; 2],
    pub(super) scaling_shift: u32,
    pub(super) ar_coefficient_lag: u32,
    pub(super) ar_coefficients_y: Vec<i32>,
    pub(super) ar_coefficients_uv: [Vec<i32>; 2],
    pub(super) ar_coefficient_shift: u32,
    pub(super) grain_scale_shift: u32,
    pub(super) uv_multiplier: [i32; 2],
    pub(super) uv_luma_multiplier: [i32; 2],
    pub(super) uv_offset: [i32; 2],
    pub(super) overlap: bool,
    pub(super) clip_to_restricted_range: bool,
    pub(super) matrix_coefficients: u32,
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

impl FrameSurface {
    fn validate_color_leaf(
        leaf: &super::block::FirstLeaf,
        header: &FrameHeader,
        sequence: &SequenceHeader,
    ) -> Av1Result<(SampleDepth, PixelLayout, [usize; 3])> {
        let depth = SampleDepth::new(sequence.bit_depth)
            .ok_or_else(|| malformed("reference surface sample depth is unsupported"))?;
        let layout = PixelLayout::from_sequence(
            sequence.monochrome,
            sequence.subsampling_x,
            sequence.subsampling_y,
        )
        .ok_or_else(|| malformed("reference surface layout is unsupported"))?;
        if matches!(layout, PixelLayout::Monochrome)
            || leaf.width != header.upscaled_width
            || leaf.height != header.frame_height
            || header.render_width == 0
            || header.render_height == 0
        {
            return Err(malformed(
                "decoded color surface disagrees with its frame header",
            ));
        }
        let mut strides = [0_usize; 3];
        for (plane, source) in leaf.planes.iter().enumerate() {
            let (width, height) =
                Self::plane_dimensions(layout, header.upscaled_width, header.frame_height, plane)
                    .ok_or_else(|| malformed("color surface unexpectedly omits chroma"))?;
            strides[plane] = FramePlane::validate_reconstructed(source, width, height, depth)?;
        }
        Ok((depth, layout, strides))
    }

    fn from_validated_color_leaf(
        leaf: super::block::FirstLeaf,
        header: &FrameHeader,
        depth: SampleDepth,
        layout: PixelLayout,
        strides: [usize; 3],
        motion: TemporalMotionField,
    ) -> Self {
        let chroma_width = if matches!(layout, PixelLayout::I420 | PixelLayout::I422) {
            header.upscaled_width.div_ceil(2)
        } else {
            header.upscaled_width
        };
        let chroma_height = if matches!(layout, PixelLayout::I420) {
            header.frame_height.div_ceil(2)
        } else {
            header.frame_height
        };
        #[cfg(coverage)]
        let entropy_operations = leaf.entropy_operations;
        let [y, u, v] = leaf.planes;
        let [y_stride, u_stride, v_stride] = strides;
        let planes = [
            Some(FramePlane::from_validated(
                y,
                header.upscaled_width,
                header.frame_height,
                y_stride,
            )),
            Some(FramePlane::from_validated(
                u,
                chroma_width,
                chroma_height,
                u_stride,
            )),
            Some(FramePlane::from_validated(
                v,
                chroma_width,
                chroma_height,
                v_stride,
            )),
        ];
        Self {
            depth,
            layout,
            coded_width: header.frame_width,
            upscaled_width: header.upscaled_width,
            superres_enabled: header.superres_enabled,
            frame_height: header.frame_height,
            render_width: header.render_width,
            render_height: header.render_height,
            planes,
            motion,
            #[cfg(coverage)]
            entropy_operations,
        }
    }

    fn validate_monochrome_plane(
        plane: &super::block::ReconstructedPlane,
        header: &FrameHeader,
        sequence: &SequenceHeader,
    ) -> Av1Result<(SampleDepth, usize)> {
        let depth = SampleDepth::new(sequence.bit_depth)
            .ok_or_else(|| malformed("monochrome reference depth is unsupported"))?;
        if !sequence.monochrome || header.render_width == 0 || header.render_height == 0 {
            return Err(malformed(
                "decoded monochrome surface disagrees with its frame header",
            ));
        }
        let stride = FramePlane::validate_reconstructed(
            plane,
            header.upscaled_width,
            header.frame_height,
            depth,
        )?;
        Ok((depth, stride))
    }

    fn from_validated_monochrome_plane(
        plane: super::block::ReconstructedPlane,
        header: &FrameHeader,
        depth: SampleDepth,
        stride: usize,
        motion: TemporalMotionField,
    ) -> Self {
        Self {
            depth,
            layout: PixelLayout::Monochrome,
            coded_width: header.frame_width,
            upscaled_width: header.upscaled_width,
            superres_enabled: header.superres_enabled,
            frame_height: header.frame_height,
            render_width: header.render_width,
            render_height: header.render_height,
            planes: [
                Some(FramePlane::from_validated(
                    plane,
                    header.upscaled_width,
                    header.frame_height,
                    stride,
                )),
                None,
                None,
            ],
            motion,
            #[cfg(coverage)]
            entropy_operations: Vec::new(),
        }
    }

    fn materialize(&self) -> Av1Result<SelectedDisplay> {
        self.validate()?;
        if matches!(self.layout, PixelLayout::Monochrome) {
            return Ok(SelectedDisplay {
                color_leaf: None,
                monochrome_plane: self.planes[0]
                    .as_ref()
                    .map(FramePlane::reconstructed_copy)
                    .transpose()?,
                dimensions: Some((self.upscaled_width, self.frame_height)),
                geometry: Some(DisplayGeometryProof {
                    superres_enabled: self.superres_enabled,
                    render_width: self.render_width,
                    render_height: self.render_height,
                }),
            });
        }
        let planes = [
            self.planes[0]
                .as_ref()
                .ok_or_else(|| malformed("display surface omits luma"))?
                .reconstructed_copy()?,
            self.planes[1]
                .as_ref()
                .ok_or_else(|| malformed("display surface omits first chroma"))?
                .reconstructed_copy()?,
            self.planes[2]
                .as_ref()
                .ok_or_else(|| malformed("display surface omits second chroma"))?
                .reconstructed_copy()?,
        ];
        Ok(SelectedDisplay {
            color_leaf: Some(super::block::FirstLeaf {
                width: self.upscaled_width,
                height: self.frame_height,
                block_skipped: false,
                planes,
                luma_predictor: super::block::LumaPredictor::Dc,
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
                entropy_operations: self.entropy_operations.clone(),
            }),
            monochrome_plane: None,
            dimensions: Some((self.upscaled_width, self.frame_height)),
            geometry: Some(DisplayGeometryProof {
                superres_enabled: self.superres_enabled,
                render_width: self.render_width,
                render_height: self.render_height,
            }),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DisplayGeometryProof {
    pub(super) superres_enabled: bool,
    pub(super) render_width: u32,
    pub(super) render_height: u32,
}

pub(super) struct SelectedDisplay {
    pub(super) color_leaf: Option<super::block::FirstLeaf>,
    pub(super) monochrome_plane: Option<super::block::ReconstructedPlane>,
    pub(super) dimensions: Option<(u32, u32)>,
    pub(super) geometry: Option<DisplayGeometryProof>,
}

impl SelectedDisplay {
    const fn unavailable() -> Self {
        Self {
            color_leaf: None,
            monochrome_plane: None,
            dimensions: None,
            geometry: None,
        }
    }
}

#[derive(Clone)]
struct FrameCompletion {
    surface: Option<Arc<FrameSurface>>,
    temporal_unit: u64,
    temporal_id: u32,
    spatial_id: u32,
    film_grain: Option<FilmGrain>,
    show_existing: bool,
    diagnostic_leaf: Option<super::block::FirstLeaf>,
    diagnostic_frame_dimensions: Option<(u32, u32)>,
}

impl FrameCompletion {
    fn priority(&self) -> (u64, u32, u32) {
        (self.temporal_unit, self.spatial_id, self.temporal_id)
    }
}

#[derive(Clone)]
struct ReferenceDecodeState {
    cdfs: entropy::FrameCdfs,
    segmentation_map: Option<entropy::SegmentMap>,
}

#[derive(Clone)]
struct ReferenceState {
    header: FrameHeader,
    decode: Option<Arc<ReferenceDecodeState>>,
    surface: Option<Arc<FrameSurface>>,
}

fn new_temporal_motion_field(
    header: &FrameHeader,
    sequence: &SequenceHeader,
    references: &[Option<ReferenceState>; REFERENCE_SLOTS],
) -> Av1Result<TemporalMotionField> {
    let mut reference_order_hints = [0_u32; 7];
    if header.frame_type.is_inter() {
        for (logical, output) in reference_order_hints.iter_mut().enumerate() {
            let slot = header.reference_indices[logical];
            let reference = references
                .get(slot)
                .and_then(Option::as_ref)
                .ok_or_else(|| malformed("decoded inter frame references an empty slot"))?;
            *output = reference.header.order_hint;
        }
    }
    TemporalMotionField::new(
        header.frame_width,
        header.frame_height,
        header.order_hint,
        sequence.order_hint_bits,
        reference_order_hints,
    )
}

fn populate_temporal_motion_field(
    field: &mut TemporalMotionField,
    pending_samples: &[RetainedTemporalSample],
    current_samples: &[RetainedTemporalSample],
    inter_frame: bool,
) -> Av1Result<()> {
    if !inter_frame && (!pending_samples.is_empty() || !current_samples.is_empty()) {
        return Err(malformed("intra frame carries retained temporal MVs"));
    }
    for sample in pending_samples.iter().chain(current_samples) {
        if field.get(sample.x8, sample.y8).is_some() {
            return Err(malformed("duplicate retained temporal-MV sample"));
        }
        field.set(sample.x8, sample.y8, Some(sample.entry))?;
    }
    Ok(())
}

fn validate_inter_reference_headers(
    header: &FrameHeader,
    references: &[Option<ReferenceState>; REFERENCE_SLOTS],
) -> Av1Result<()> {
    // Check the complete syntax-level reference set before reporting any
    // implementation gap in a present reference's reconstructed surface.
    for &slot in &header.reference_indices {
        let _ = references
            .get(slot)
            .and_then(Option::as_ref)
            .ok_or_else(|| malformed("inter reference slot is empty"))?;
    }
    Ok(())
}

fn inter_frame_context<'a>(
    header: &FrameHeader,
    sequence: &SequenceHeader,
    references: &'a [Option<ReferenceState>; REFERENCE_SLOTS],
    projected_temporal: Option<&'a ProjectedTemporalField>,
) -> Av1Result<entropy::InterFrameContext<'a>> {
    if !header.frame_type.is_inter() {
        return Err(malformed("intra frame requested inter reference context"));
    }
    validate_inter_reference_headers(header, references)?;
    let mut reference_order_hints = [0_u32; 7];
    let mut sign_bias = [false; 7];
    let mut inter_references = [None; 7];
    for logical in super::motion::ReferenceFrame::ALL {
        let index = logical.index();
        let slot = header
            .reference_indices
            .get(index)
            .copied()
            .ok_or_else(|| malformed("inter reference index exceeds seven"))?;
        let reference = references
            .get(slot)
            .and_then(Option::as_ref)
            .ok_or_else(|| malformed("decoded inter frame references an empty slot"))?;
        let surface = reference.surface.as_deref().ok_or_else(|| {
            CodecError::NotImplemented(
                "AV1 inter reference reconstruction is incomplete".to_owned(),
            )
        })?;
        surface.validate()?;
        let order_hint = reference.header.order_hint;
        reference_order_hints[index] = order_hint;
        sign_bias[index] =
            relative_distance(sequence.order_hint_bits, order_hint, header.order_hint) > 0;
        let scale = ScaleFactors::new(
            header.frame_width,
            header.frame_height,
            surface.upscaled_width,
            surface.frame_height,
        )?;
        inter_references[index] = Some(entropy::InterReference {
            logical,
            surface,
            scale,
            order_hint,
            global_motion: header.global_motion[index],
            sign_bias: sign_bias[index],
            temporal: Some(&surface.motion),
        });
    }
    let references: [Av1Result<entropy::InterReference<'a>>; 7] = std::array::from_fn(|index| {
        // Every entry is populated by the checked loop above.  The explicit
        // match keeps this conversion non-panicking if the logical reference
        // table is ever expanded independently of `ReferenceFrame::ALL`.
        inter_references[index].ok_or_else(|| malformed("inter reference table is incomplete"))
    });
    let references = references.into_iter().collect::<Av1Result<Vec<_>>>()?;
    let references: [entropy::InterReference<'a>; 7] = references
        .try_into()
        .map_err(|_| malformed("inter reference table has invalid length"))?;
    let skip_mode_references = header
        .skip_mode_references
        .map(|indices| {
            let first = ReferenceFrame::from_index(indices[0])
                .ok_or_else(|| malformed("skip mode first reference exceeds seven"))?;
            let second = ReferenceFrame::from_index(indices[1])
                .ok_or_else(|| malformed("skip mode second reference exceeds seven"))?;
            if first == second {
                return Err(malformed("skip mode references must be distinct"));
            }
            Ok(ReferencePair::compound(first, second))
        })
        .transpose()?;
    if header.skip_mode_enabled && skip_mode_references.is_none() {
        return Err(malformed("skip mode is enabled without derived references"));
    }
    Ok(entropy::InterFrameContext {
        references,
        skip_mode_references,
        projected_temporal,
        current_order_hint: header.order_hint,
        order_hint_bits: sequence.order_hint_bits,
        reference_order_hints,
        global_motion: header.global_motion,
        sign_bias,
        force_integer_mv: header.force_integer_mv,
        high_precision_mv: header.allow_high_precision_mv,
        use_ref_frame_mvs: header.use_ref_frame_mvs,
        interpolation_filter: header.interpolation_filter,
        dual_filter: sequence.enable_dual_filter,
        reference_mode_select: header.reference_mode_select,
        motion_mode_switchable: header.motion_mode_switchable,
        allow_warped_motion: header.allow_warped_motion,
        enable_interintra_compound: sequence.enable_interintra_compound,
        enable_masked_compound: sequence.enable_masked_compound,
        enable_jnt_comp: sequence.enable_jnt_comp,
    })
}

fn projected_temporal_field(
    header: &FrameHeader,
    sequence: &SequenceHeader,
    references: &[Option<ReferenceState>; REFERENCE_SLOTS],
) -> Av1Result<Option<ProjectedTemporalField>> {
    if !header.use_ref_frame_mvs {
        return Ok(None);
    }
    validate_inter_reference_headers(header, references)?;
    let mut retained = [None; 7];
    let mut reference_order_hints = [0_u32; 7];
    for logical in ReferenceFrame::ALL {
        let index = logical.index();
        let slot = header
            .reference_indices
            .get(index)
            .copied()
            .ok_or_else(|| malformed("temporal reference index exceeds seven"))?;
        let reference = references
            .get(slot)
            .and_then(Option::as_ref)
            .ok_or_else(|| malformed("temporal reference slot is empty"))?;
        let surface = reference.surface.as_deref().ok_or_else(|| {
            CodecError::NotImplemented(
                "AV1 temporal reference reconstruction is incomplete".to_owned(),
            )
        })?;
        surface.validate()?;
        retained[index] = Some(&surface.motion);
        reference_order_hints[index] = reference.header.order_hint;
    }
    let width8 = header.frame_width.div_ceil(8);
    let height8 = header.frame_height.div_ceil(8);
    Ok(Some(load_projected_temporal_field(
        super::motion::TemporalProjectionRequest {
            current_width: header.frame_width,
            current_height: header.frame_height,
            current_order_hint: header.order_hint,
            order_hint_bits: sequence.order_hint_bits,
            reference_order_hints,
            col_start8: 0,
            col_end8: width8,
            row_start8: 0,
            row_end8: height8,
        },
        retained,
    )?))
}

fn invalidate_reference_slots(
    references: &mut [Option<ReferenceState>; REFERENCE_SLOTS],
    frame_id: u32,
    delta_frame_id_bits: u32,
    frame_id_bits: u32,
) {
    let difference_range = 1_u32 << delta_frame_id_bits;
    let frame_id_range = 1_u32 << frame_id_bits;
    for reference in references {
        let Some(retained) = reference else {
            continue;
        };
        let invalid = if frame_id >= difference_range {
            retained.header.frame_id > frame_id
                || retained.header.frame_id < frame_id.saturating_sub(difference_range)
        } else {
            retained.header.frame_id > frame_id
                && retained.header.frame_id
                    < frame_id_range
                        .saturating_add(frame_id)
                        .saturating_sub(difference_range)
        };
        if invalid {
            *reference = None;
        }
    }
}

struct PendingFrame {
    header: FrameHeader,
    obu_extension: bool,
    staged_references: [Option<ReferenceState>; REFERENCE_SLOTS],
    staged_current_frame_id: Option<u32>,
    next_tile: u32,
    input_cdfs: Option<entropy::FrameCdfs>,
    selected_cdfs: Option<entropy::FrameCdfs>,
    previous_segment_map: Option<entropy::SegmentMap>,
    segment_map: Option<entropy::SegmentMap>,
    decode_complete: bool,
    first_leaf: Option<super::block::FirstLeaf>,
    complete_color_leaf: Option<super::block::FirstLeaf>,
    complete_color_tiles: Vec<ReconstructedColorTile>,
    temporal_samples: Vec<RetainedTemporalSample>,
    complete_monochrome_tiles: Vec<ReconstructedMonochromeTile>,
    complete_monochrome_plane: Option<super::block::ReconstructedPlane>,
}

pub(super) struct FrameState {
    sequence: Option<SequenceHeader>,
    /// Present only for sequence presentation. The output budget was reserved
    /// for this canvas before any AV1 reference or display was reconstructed.
    expected_sequence_dimensions: Option<(u32, u32)>,
    operating_point_idc: u32,
    has_nonzero_operating_point_idc: bool,
    references: [Option<ReferenceState>; REFERENCE_SLOTS],
    pending: Option<PendingFrame>,
    current_frame_id: Option<u32>,
    temporal_unit: u64,
    // Only the selected display owns an additional surface reference. Lower
    // candidates can never win a later maximum, so release them at completion
    // rather than retaining all shown surfaces until the sample is flushed.
    completion: Option<FrameCompletion>,
}

impl FrameState {
    pub(super) fn new() -> Self {
        Self {
            sequence: None,
            expected_sequence_dimensions: None,
            operating_point_idc: 0,
            has_nonzero_operating_point_idc: false,
            references: std::array::from_fn(|_| None),
            pending: None,
            current_frame_id: None,
            temporal_unit: 0,
            completion: None,
        }
    }

    pub(super) fn for_sequence(width: u32, height: u32) -> Self {
        let mut state = Self::new();
        state.expected_sequence_dimensions = Some((width, height));
        state
    }

    pub(super) fn accept_sequence(&mut self, sequence: SequenceHeader) -> Av1Result<()> {
        if self
            .sequence
            .as_ref()
            .is_some_and(|previous| !previous.consistent_with(&sequence))
        {
            return Err(malformed("frame syntax validation failed"));
        }
        self.operating_point_idc = sequence
            .operating_points
            .first()
            .map_or(0, |operating_point| operating_point.idc);
        self.has_nonzero_operating_point_idc = sequence
            .operating_points
            .iter()
            .any(|operating_point| operating_point.idc != 0);
        self.sequence = Some(sequence);
        Ok(())
    }

    /// Apply the selected operating point to one layer-specific OBU after its
    /// header and payload bounds have already been validated.
    pub(super) fn admits_layer_specific_obu(
        &self,
        has_extension: bool,
        temporal_id: u32,
        spatial_id: u32,
    ) -> Av1Result<bool> {
        if self.sequence.is_none() {
            return Err(malformed(
                "layer-specific OBU appears before a sequence header",
            ));
        }
        if self.has_nonzero_operating_point_idc != has_extension {
            return Err(malformed(
                "layer OBU extension disagrees with sequence operating points",
            ));
        }
        if self.operating_point_idc == 0 {
            return Ok(true);
        }
        let temporal_member = self
            .operating_point_idc
            .checked_shr(temporal_id)
            .is_some_and(|mask| mask & 1 != 0);
        let spatial_bit = spatial_id.saturating_add(8);
        let spatial_member = self
            .operating_point_idc
            .checked_shr(spatial_bit)
            .is_some_and(|mask| mask & 1 != 0);
        Ok(temporal_member && spatial_member)
    }

    fn advance_temporal_unit(&mut self, stage: &'static str) -> Av1Result<()> {
        if self.pending.is_some() {
            return Err(malformed(stage));
        }
        self.temporal_unit = self
            .temporal_unit
            .checked_add(1)
            .ok_or_else(|| malformed("temporal-unit counter overflows"))?;
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

    /// Finish the one temporal unit carried by an AVIF sample. A temporal
    /// delimiter inside that sample validates placement but does not create a
    /// second boundary.
    pub(super) fn sample_flush(&mut self) -> Av1Result<u64> {
        let completed_temporal_unit = self.temporal_unit;
        self.advance_temporal_unit("AVIF sample ends during a pending frame")?;
        Ok(completed_temporal_unit)
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

    /// Commit a completed display after fallible reconstruction succeeds.
    /// References/CDFs commit independently, including for a losing candidate.
    fn retain_completion(&mut self, candidate: FrameCompletion) {
        if self
            .completion
            .as_ref()
            .is_none_or(|retained| candidate.priority() >= retained.priority())
        {
            // Equality deliberately replaces: the former vector maximum used
            // insertion order as its last key, selecting the latest exact tie.
            self.completion = Some(candidate);
        }
    }

    fn selected_completion(&self) -> Option<&FrameCompletion> {
        self.completion.as_ref()
    }

    #[allow(
        dead_code,
        reason = "compatibility wrapper for token-aware display materialization"
    )]
    pub(super) fn selected_display(&self) -> Av1Result<SelectedDisplay> {
        self.selected_display_with_token(None)
    }

    pub(super) fn selected_display_with_token(
        &self,
        token: Option<&crate::CancellationToken>,
    ) -> Av1Result<SelectedDisplay> {
        let Some(completion) = self.selected_completion() else {
            return Ok(SelectedDisplay::unavailable());
        };
        self.materialize_completion_with_token(completion, token)
    }

    /// Materialize only the completion produced by one AVIF sample.
    ///
    /// A frame state can retain an earlier shown completion while a later
    /// sample carries only hidden reference state. Sequence presentation must
    /// not mistake that retained completion for a newly displayed frame, so
    /// callers query only after a successful `sample_flush`, using its returned
    /// temporal unit. This method does not expose intermediate layer displays.
    pub(super) fn selected_display_for_temporal_unit_with_token(
        &self,
        temporal_unit: u64,
        token: Option<&crate::CancellationToken>,
    ) -> Av1Result<Option<SelectedDisplay>> {
        let Some(completion) = self
            .completion
            .as_ref()
            .filter(|completion| completion.temporal_unit == temporal_unit)
        else {
            return Ok(None);
        };
        self.materialize_completion_with_token(completion, token)
            .map(Some)
    }

    fn materialize_completion_with_token(
        &self,
        completion: &FrameCompletion,
        token: Option<&crate::CancellationToken>,
    ) -> Av1Result<SelectedDisplay> {
        crate::codecs::error::check_cancelled(token)?;
        if completion.show_existing && completion.diagnostic_leaf.is_some() {
            return Err(malformed(
                "show-existing completion contains diagnostic reconstruction",
            ));
        }
        let mut display = if let Some(surface) = completion.surface.as_deref() {
            surface.materialize()
        } else {
            Ok(SelectedDisplay {
                color_leaf: completion.diagnostic_leaf.clone(),
                monochrome_plane: None,
                dimensions: completion.diagnostic_frame_dimensions,
                geometry: None,
            })
        }?;
        let Some(grain) = completion.film_grain.as_ref() else {
            return Ok(display);
        };
        let Some(surface) = completion.surface.as_deref() else {
            return Ok(SelectedDisplay::unavailable());
        };
        if surface.layout == PixelLayout::Monochrome {
            if !matches!(surface.depth.bits(), 8 | 10 | 12)
                || surface.render_width != surface.upscaled_width
                || surface.render_height != surface.frame_height
            {
                return Ok(SelectedDisplay::unavailable());
            }
            let Some(plane) = display.monochrome_plane.take() else {
                return Ok(SelectedDisplay::unavailable());
            };
            display.monochrome_plane = Some(super::film_grain::apply_monochrome(
                plane,
                surface.upscaled_width,
                surface.frame_height,
                surface.depth.bits(),
                grain,
                token,
            )?);
            return Ok(display);
        }
        let i444_film_grain_dimensions_supported = if surface.superres_enabled {
            // Super-resolution I444 keeps the legacy display whitelist. The
            // syntax marker is carried by the retained surface because width
            // equality is allowed after AV1's minimum coded-width clamp.
            entropy::bounded_i444_film_grain_dimensions(
                surface.upscaled_width,
                surface.frame_height,
            )
        } else {
            super::film_grain::bounded_fullres_i444_film_grain_dimensions(
                surface.upscaled_width,
                surface.frame_height,
            )
        };
        if !matches!(surface.depth.bits(), 8 | 10 | 12)
            || !matches!(
                surface.layout,
                PixelLayout::I420 | PixelLayout::I422 | PixelLayout::I444
            )
            || surface.render_width != surface.upscaled_width
            || surface.render_height != surface.frame_height
            || (surface.layout == PixelLayout::I444 && !i444_film_grain_dimensions_supported)
            || surface.planes.iter().any(Option::is_none)
            || matches!(grain.matrix_coefficients, 0 | 3)
        {
            return Ok(SelectedDisplay::unavailable());
        }
        let Some(leaf) = display.color_leaf.take() else {
            return Ok(SelectedDisplay::unavailable());
        };
        display.color_leaf = Some(match surface.layout {
            PixelLayout::I420 => {
                super::film_grain::apply_i420(leaf, surface.depth.bits(), grain, token)?
            }
            PixelLayout::I422 => {
                super::film_grain::apply_i422(leaf, surface.depth.bits(), grain, token)?
            }
            PixelLayout::I444 => {
                super::film_grain::apply_i444(leaf, surface.depth.bits(), grain, token)?
            }
            _ => return Ok(SelectedDisplay::unavailable()),
        });
        Ok(display)
    }

    pub(super) fn frame_obu(
        &mut self,
        data: &SegmentedData<'_, '_>,
        start: usize,
        end: usize,
        has_extension: bool,
        temporal_id: u32,
        spatial_id: u32,
    ) -> Av1Result<()> {
        let mut reader = self.begin_frame(
            data,
            start..end,
            has_extension,
            temporal_id,
            spatial_id,
            false,
        )?;
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.header.show_existing_frame)
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
        payload: Range<usize>,
        has_extension: bool,
        temporal_id: u32,
        spatial_id: u32,
        redundant: bool,
    ) -> Av1Result<()> {
        let mut reader = self.begin_frame(
            data,
            payload,
            has_extension,
            temporal_id,
            spatial_id,
            redundant,
        )?;
        reader.trailing_bits()?;
        self.complete_show_existing()
    }

    pub(super) fn tile_group_obu(
        &mut self,
        data: &SegmentedData<'_, '_>,
        start: usize,
        end: usize,
        has_extension: bool,
        temporal_id: u32,
        spatial_id: u32,
    ) -> Av1Result<()> {
        self.validate_pending_layer(has_extension, temporal_id, spatial_id)?;
        let mut reader = BitReader::new(data, start, end)?;
        let group = self.read_tile_group(data, &mut reader, end)?;
        self.accept_tile_group(group)
    }

    fn begin_frame<'data, 'input, 'spans>(
        &mut self,
        data: &'data SegmentedData<'input, 'spans>,
        payload: Range<usize>,
        has_extension: bool,
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
            self.validate_pending_layer(has_extension, temporal_id, spatial_id)?;
            let references = std::array::from_fn(|index| {
                pending.staged_references[index]
                    .as_ref()
                    .map(|reference| reference.header.clone())
            });
            let (header, reader) = parse(
                data,
                payload.start,
                payload.end,
                sequence,
                &references,
                (temporal_id, spatial_id),
                self.current_frame_id,
            )?;
            if header != pending.header {
                return Err(malformed("frame syntax validation failed"));
            }
            return Ok(reader);
        }
        if self.pending.is_some() {
            return Err(malformed("frame syntax validation failed"));
        }
        let references = self.reference_headers();
        let (header, reader) = parse(
            data,
            payload.start,
            payload.end,
            sequence,
            &references,
            (temporal_id, spatial_id),
            self.current_frame_id,
        )?;
        self.accept_parsed_header(
            sequence.frame_id_numbers_present,
            sequence.frame_id_bits,
            has_extension,
            header,
        )?;
        Ok(reader)
    }

    fn accept_parsed_header(
        &mut self,
        frame_id_numbers_present: bool,
        frame_id_bits: u32,
        obu_extension: bool,
        header: FrameHeader,
    ) -> Av1Result<()> {
        let mut references = self.references.clone();
        let mut current_frame_id = self.current_frame_id;
        if frame_id_numbers_present && !header.show_existing_frame {
            validate_current_frame_id(frame_id_bits, self.current_frame_id, &header)?;
            let sequence = self
                .sequence
                .as_ref()
                .ok_or(malformed("frame header has no sequence state"))?;
            invalidate_reference_slots(
                &mut references,
                header.frame_id,
                sequence.delta_frame_id_bits,
                sequence.frame_id_bits,
            );
            current_frame_id = Some(header.frame_id);
        }
        if let Some((width, height)) = self.expected_sequence_dimensions
            && ((header.upscaled_width, header.frame_height) != (width, height)
                || (header.render_width, header.render_height) != (width, height)
                || header.frame_width == 0
                || header.frame_width > width)
        {
            // Check every coded frame, including hidden references and
            // inherited show-existing geometry, before allocating its maps
            // or pixels. Inspection may have selected a different item.
            return Err(CodecError::NotImplemented(
                "AVIF sequence frame geometry differs from the reserved canvas".to_owned(),
            ));
        }
        let block_width = header
            .frame_width
            .div_ceil(8)
            .checked_mul(2)
            .ok_or_else(|| malformed("frame block width overflows"))?;
        let block_height = header
            .frame_height
            .div_ceil(8)
            .checked_mul(2)
            .ok_or_else(|| malformed("frame block height overflows"))?;
        let primary_decode = if header.primary_ref_frame == PRIMARY_REF_NONE {
            None
        } else {
            let slot = header.reference_indices[header.primary_ref_frame];
            references
                .get(slot)
                .and_then(Option::as_ref)
                .and_then(|reference| reference.decode.as_ref())
        };
        let previous_segment_map = primary_decode
            .and_then(|decode| decode.segmentation_map.as_ref())
            .filter(|map| map.compatible_with(block_width, block_height))
            .cloned();
        let segment_map = if header.segmentation.enabled {
            if header.segmentation.update_map {
                Some(entropy::SegmentMap::new(block_width, block_height)?)
            } else {
                Some(match previous_segment_map.as_ref() {
                    Some(previous) => previous.clone(),
                    None => entropy::SegmentMap::new(block_width, block_height)?,
                })
            }
        } else {
            None
        };
        let input_cdfs = if header.primary_ref_frame == PRIMARY_REF_NONE {
            let qindex = header
                .quantization
                .as_ref()
                .map_or(0, |quantization| quantization.base);
            Some(entropy::FrameCdfs::defaults(qindex)?)
        } else {
            primary_decode.map(|decode| decode.cdfs.clone())
        };
        self.pending = Some(PendingFrame {
            header,
            obu_extension,
            staged_references: references,
            staged_current_frame_id: current_frame_id,
            next_tile: 0,
            input_cdfs,
            selected_cdfs: None,
            previous_segment_map,
            segment_map,
            decode_complete: true,
            first_leaf: None,
            complete_color_leaf: None,
            complete_color_tiles: Vec::new(),
            temporal_samples: Vec::new(),
            complete_monochrome_tiles: Vec::new(),
            complete_monochrome_plane: None,
        });
        Ok(())
    }

    fn validate_pending_layer(
        &self,
        has_extension: bool,
        temporal_id: u32,
        spatial_id: u32,
    ) -> Av1Result<()> {
        let pending = self
            .pending
            .as_ref()
            .ok_or(malformed("layer OBU has no pending frame"))?;
        if pending.obu_extension != has_extension
            || pending.header.temporal_id != temporal_id
            || pending.header.spatial_id != spatial_id
        {
            return Err(malformed(
                "frame header and tile group layer identities disagree",
            ));
        }
        Ok(())
    }

    fn reference_headers(&self) -> [Option<FrameHeader>; REFERENCE_SLOTS] {
        std::array::from_fn(|index| {
            self.references[index]
                .as_ref()
                .map(|reference| reference.header.clone())
        })
    }

    fn complete_show_existing(&mut self) -> Av1Result<()> {
        let Some(pending) = self.pending.as_ref() else {
            return Err(malformed("show-existing completion has no pending frame"));
        };
        let header = &pending.header;
        if !header.show_existing_frame {
            return Ok(());
        }
        let Some(slot) = header.existing_frame_idx else {
            return Err(malformed("show-existing frame omits its reference slot"));
        };
        // `slot` is read from a three-bit AV1 syntax element.
        let Some(reference) = pending.staged_references[slot].as_ref() else {
            return Err(malformed("show-existing frame references an empty slot"));
        };
        let reference = reference.clone();
        if !reference.header.showable_frame {
            return Err(malformed("frame syntax validation failed"));
        }
        let completion = FrameCompletion {
            surface: reference.surface.clone(),
            temporal_unit: self.temporal_unit,
            temporal_id: header.temporal_id,
            spatial_id: header.spatial_id,
            film_grain: reference.header.film_grain.clone(),
            show_existing: true,
            diagnostic_leaf: None,
            diagnostic_frame_dimensions: None,
        };
        let pending = self
            .pending
            .take()
            .ok_or(malformed("show-existing completion disappeared"))?;
        let mut references = pending.staged_references;
        if reference.header.frame_type == FrameType::Key {
            let mut hidden = reference;
            hidden.header.showable_frame = false;
            references = std::array::from_fn(|_| Some(hidden.clone()));
        }
        self.references = references;
        self.current_frame_id = pending.staged_current_frame_id;
        self.retain_completion(completion);
        Ok(())
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
        let Some(pending) = self.pending.as_ref() else {
            return Err(malformed("tile group appears without a pending frame"));
        };
        let header = &pending.header;
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
        if start != pending.next_tile {
            return Err(malformed("frame syntax validation failed"));
        }
        bits.byte_align()?;
        // A missing implementation surface must not hide malformed tile
        // lengths. Validate the complete group envelope before reference work.
        let tile_ranges = split_tile_payloads(
            data,
            bits.position() / 8,
            payload_end,
            start,
            end,
            tiling.tile_size_bytes,
        )?;
        let projected_temporal = if header.frame_type.is_inter() {
            projected_temporal_field(header, sequence, &pending.staged_references)?
        } else {
            None
        };
        let inter_context = if header.frame_type.is_inter() {
            Some(inter_frame_context(
                header,
                sequence,
                &pending.staged_references,
                projected_temporal.as_ref(),
            )?)
        } else {
            None
        };
        let validation = validate_tile_entropy_prefixes(
            data,
            &tile_ranges,
            start,
            header,
            sequence,
            tiling,
            TileEntropyInputs {
                input_cdfs: pending.input_cdfs.as_ref(),
                current_segment_map: pending.segment_map.as_ref(),
                previous_segment_map: pending.previous_segment_map.as_ref(),
                inter_context: inter_context.as_ref(),
            },
        )?;
        Ok(TileGroup {
            start,
            end,
            first_leaf: validation.first_leaf,
            complete_color_leaf: validation.complete_color_leaf,
            complete_color_tiles: validation.complete_color_tiles,
            temporal_samples: validation.temporal_samples,
            complete_monochrome_tiles: validation.complete_monochrome_tiles,
            complete_monochrome_plane: validation.complete_monochrome_plane,
            selected_cdfs: validation.selected_cdfs,
            segment_map: validation.segment_map,
            decode_complete: validation.decode_complete,
        })
    }

    fn accept_tile_group(&mut self, group: TileGroup) -> Av1Result<()> {
        let pending = self
            .pending
            .as_ref()
            .ok_or(malformed("accepted tile group has no pending frame"))?;
        if group.start != pending.next_tile {
            return Err(malformed("frame syntax validation failed"));
        }
        let tile_count = pending
            .header
            .tiling
            .as_ref()
            .ok_or(malformed("pending frame has no tile layout"))?
            .tile_count();
        let next_tile = group.end.saturating_add(1);
        if next_tile != tile_count {
            let pending = self
                .pending
                .as_mut()
                .ok_or(malformed("accepted tile group disappeared"))?;
            #[cfg(coverage)]
            let color_tile_reservation = if crate::coverage_support::take_fault_point(
                crate::coverage_support::CoverageFaultPoint::Av1ColorTileReservation,
            ) {
                usize::MAX
            } else {
                group.complete_color_tiles.len()
            };
            #[cfg(not(coverage))]
            let color_tile_reservation = group.complete_color_tiles.len();
            pending
                .complete_color_tiles
                .try_reserve(color_tile_reservation)
                .map_err(|_| {
                    CodecError::Dimensions(
                        "unable to reserve reconstructed AV1 tile state".to_owned(),
                    )
                })?;
            pending
                .temporal_samples
                .try_reserve(group.temporal_samples.len())
                .map_err(|_| {
                    CodecError::Dimensions(
                        "unable to reserve retained AV1 temporal-MV samples".to_owned(),
                    )
                })?;
            #[cfg(coverage)]
            let monochrome_tile_reservation = if crate::coverage_support::take_fault_point(
                crate::coverage_support::CoverageFaultPoint::Av1MonochromeTileReservation,
            ) {
                usize::MAX
            } else {
                group.complete_monochrome_tiles.len()
            };
            #[cfg(not(coverage))]
            let monochrome_tile_reservation = group.complete_monochrome_tiles.len();
            pending
                .complete_monochrome_tiles
                .try_reserve(monochrome_tile_reservation)
                .map_err(|_| {
                    CodecError::Dimensions(
                        "unable to reserve reconstructed AV1 monochrome tiles".to_owned(),
                    )
                })?;
            pending.next_tile = next_tile;
            pending.first_leaf = pending.first_leaf.take().or(group.first_leaf);
            pending.complete_color_leaf = pending
                .complete_color_leaf
                .take()
                .or(group.complete_color_leaf);
            pending
                .complete_color_tiles
                .extend(group.complete_color_tiles);
            pending.temporal_samples.extend(group.temporal_samples);
            pending
                .complete_monochrome_tiles
                .extend(group.complete_monochrome_tiles);
            pending.complete_monochrome_plane = pending
                .complete_monochrome_plane
                .take()
                .or(group.complete_monochrome_plane);
            pending.selected_cdfs = pending.selected_cdfs.take().or(group.selected_cdfs);
            let previous_segment_map = pending.segment_map.take();
            pending.segment_map = group.segment_map.or(previous_segment_map);
            pending.decode_complete &= group.decode_complete;
            return Ok(());
        }

        let monochrome_tile_count = pending
            .complete_monochrome_tiles
            .len()
            .saturating_add(group.complete_monochrome_tiles.len());
        let color_tile_count = pending
            .complete_color_tiles
            .len()
            .saturating_add(group.complete_color_tiles.len());
        if monochrome_tile_count != 0
            && (color_tile_count != 0
                || pending.complete_color_leaf.is_some()
                || group.complete_color_leaf.is_some())
        {
            return Err(malformed(
                "assembled frame mixes monochrome and color tiles",
            ));
        }
        let mut assembled_color_leaf = None;
        let mut assembled_monochrome_plane = None;
        if tile_count > 1
            && pending
                .complete_color_tiles
                .len()
                .saturating_add(group.complete_color_tiles.len())
                == usize::try_from(tile_count).unwrap_or(0)
        {
            let sequence = self
                .sequence
                .as_ref()
                .ok_or(malformed("assembled tile frame has no sequence"))?;
            assembled_color_leaf = assemble_color_tiles(
                &pending.complete_color_tiles,
                &group.complete_color_tiles,
                &pending.header,
                sequence,
            )?;
            if pending.header.superres_enabled {
                let depth = SampleDepth::new(sequence.bit_depth)
                    .ok_or_else(|| malformed("super-resolution sample depth is unsupported"))?;
                assembled_color_leaf = assembled_color_leaf
                    .map(|leaf| {
                        upscale_color_leaf_for_superres(
                            leaf,
                            &pending.header,
                            sequence,
                            pending.header.frame_width,
                            pending.header.frame_height,
                            depth,
                        )
                    })
                    .transpose()?;
            }
        }
        if tile_count > 1
            && pending
                .complete_monochrome_tiles
                .len()
                .saturating_add(group.complete_monochrome_tiles.len())
                == usize::try_from(tile_count).unwrap_or(0)
        {
            let sequence = self
                .sequence
                .as_ref()
                .ok_or(malformed("assembled monochrome frame has no sequence"))?;
            assembled_monochrome_plane = Some(assemble_monochrome_tiles(
                &pending.complete_monochrome_tiles,
                &group.complete_monochrome_tiles,
                &pending.header,
                sequence,
            )?);
            if pending.header.superres_enabled {
                let depth = SampleDepth::new(sequence.bit_depth)
                    .ok_or_else(|| malformed("super-resolution sample depth is unsupported"))?;
                assembled_monochrome_plane = assembled_monochrome_plane
                    .map(|plane| {
                        resize::upscale_monochrome_plane(
                            plane,
                            pending.header.frame_width,
                            pending.header.upscaled_width,
                            pending.header.frame_height,
                            pending.header.superres_denominator,
                            depth,
                        )
                    })
                    .transpose()?;
            }
        }
        enum SurfacePlan {
            Color {
                depth: SampleDepth,
                layout: PixelLayout,
                strides: [usize; 3],
                motion: TemporalMotionField,
            },
            Monochrome {
                depth: SampleDepth,
                stride: usize,
                motion: TemporalMotionField,
            },
            None,
        }
        let sequence = self
            .sequence
            .as_ref()
            .ok_or(malformed("completed frame has no sequence"))?;
        let color_leaf = assembled_color_leaf
            .as_ref()
            .or(pending.complete_color_leaf.as_ref())
            .or(group.complete_color_leaf.as_ref());
        let monochrome_plane = assembled_monochrome_plane.as_ref().or(pending
            .complete_monochrome_plane
            .as_ref()
            .or(group.complete_monochrome_plane.as_ref()));
        let surface_plan = if let Some(leaf) = color_leaf {
            let (depth, layout, strides) =
                FrameSurface::validate_color_leaf(leaf, &pending.header, sequence)?;
            let mut motion =
                new_temporal_motion_field(&pending.header, sequence, &pending.staged_references)?;
            populate_temporal_motion_field(
                &mut motion,
                &pending.temporal_samples,
                &group.temporal_samples,
                pending.header.frame_type.is_inter(),
            )?;
            SurfacePlan::Color {
                depth,
                layout,
                strides,
                motion,
            }
        } else if let Some(plane) = monochrome_plane {
            let (depth, stride) =
                FrameSurface::validate_monochrome_plane(plane, &pending.header, sequence)?;
            let mut motion =
                new_temporal_motion_field(&pending.header, sequence, &pending.staged_references)?;
            populate_temporal_motion_field(
                &mut motion,
                &pending.temporal_samples,
                &group.temporal_samples,
                pending.header.frame_type.is_inter(),
            )?;
            SurfacePlan::Monochrome {
                depth,
                stride,
                motion,
            }
        } else {
            SurfacePlan::None
        };
        let has_first_leaf = pending.first_leaf.is_some() || group.first_leaf.is_some();
        let diagnostic_fallback = matches!(surface_plan, SurfacePlan::None)
            && has_first_leaf
            && tile_count == 1
            && pending.header.loop_filter.level_y == [0; 2]
            && pending.header.loop_filter.level_u == 0
            && pending.header.loop_filter.level_v == 0
            && pending.header.cdef.is_none()
            && !pending.header.superres_enabled
            && pending.header.restoration.is_none()
            && pending.header.film_grain.is_none();
        // Assembly and surface validation are fallible. Ownership moves only
        // after they succeed. Retaining the selected completion requires no
        // collection allocation; reference refresh still commits for every
        // decoded frame, even when its display loses selection.
        let pending = self
            .pending
            .take()
            .ok_or(malformed("completed frame disappeared"))?;
        let first_leaf = pending.first_leaf.or(group.first_leaf);
        let complete_color_leaf = assembled_color_leaf
            .or(pending.complete_color_leaf)
            .or(group.complete_color_leaf);
        let complete_monochrome_plane = assembled_monochrome_plane.or(pending
            .complete_monochrome_plane
            .or(group.complete_monochrome_plane));
        let selected_cdfs = pending.selected_cdfs.or(group.selected_cdfs);
        let segment_map = group.segment_map.or(pending.segment_map);
        let decode_complete = pending.decode_complete && group.decode_complete;
        let completed = pending.header;
        let surface = match surface_plan {
            SurfacePlan::Color {
                depth,
                layout,
                strides,
                motion,
            } => complete_color_leaf.map(|leaf| {
                Arc::new(FrameSurface::from_validated_color_leaf(
                    leaf, &completed, depth, layout, strides, motion,
                ))
            }),
            SurfacePlan::Monochrome {
                depth,
                stride,
                motion,
            } => complete_monochrome_plane.map(|plane| {
                Arc::new(FrameSurface::from_validated_monochrome_plane(
                    plane, &completed, depth, stride, motion,
                ))
            }),
            SurfacePlan::None => None,
        };
        let mut references = pending.staged_references;
        if completed.refresh_frame_flags != 0 {
            let retained_cdfs = if completed.refresh_frame_context {
                selected_cdfs
            } else {
                pending.input_cdfs
            };
            let decode = if decode_complete {
                retained_cdfs.map(|cdfs| {
                    Arc::new(ReferenceDecodeState {
                        cdfs,
                        segmentation_map: segment_map,
                    })
                })
            } else {
                None
            };
            let reference = ReferenceState {
                header: completed.clone(),
                decode,
                surface: surface.clone(),
            };
            for (slot, retained) in references.iter_mut().enumerate() {
                let mask = 1_u8 << slot;
                if completed.refresh_frame_flags & mask != 0 {
                    *retained = Some(reference.clone());
                }
            }
        }
        if completed.show_frame {
            let diagnostic_leaf = diagnostic_fallback.then_some(first_leaf).flatten();
            self.retain_completion(FrameCompletion {
                surface,
                temporal_unit: self.temporal_unit,
                temporal_id: completed.temporal_id,
                spatial_id: completed.spatial_id,
                film_grain: completed.film_grain.clone(),
                show_existing: false,
                diagnostic_leaf,
                diagnostic_frame_dimensions: diagnostic_fallback
                    .then_some((completed.upscaled_width, completed.frame_height)),
            });
        }
        self.references = references;
        self.current_frame_id = pending.staged_current_frame_id;
        Ok(())
    }
}

struct TileGroup {
    start: u32,
    end: u32,
    first_leaf: Option<super::block::FirstLeaf>,
    complete_color_leaf: Option<super::block::FirstLeaf>,
    complete_color_tiles: Vec<ReconstructedColorTile>,
    temporal_samples: Vec<RetainedTemporalSample>,
    complete_monochrome_tiles: Vec<ReconstructedMonochromeTile>,
    complete_monochrome_plane: Option<super::block::ReconstructedPlane>,
    selected_cdfs: Option<entropy::FrameCdfs>,
    segment_map: Option<entropy::SegmentMap>,
    decode_complete: bool,
}

struct TileValidation {
    first_leaf: Option<super::block::FirstLeaf>,
    complete_color_leaf: Option<super::block::FirstLeaf>,
    complete_color_tiles: Vec<ReconstructedColorTile>,
    temporal_samples: Vec<RetainedTemporalSample>,
    complete_monochrome_tiles: Vec<ReconstructedMonochromeTile>,
    complete_monochrome_plane: Option<super::block::ReconstructedPlane>,
    selected_cdfs: Option<entropy::FrameCdfs>,
    segment_map: Option<entropy::SegmentMap>,
    decode_complete: bool,
}

struct ReconstructedColorTile {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    reconstruction: entropy::Lossy420Reconstruction,
}

struct ReconstructedMonochromeTile {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    coded_width: u32,
    coded_height: u32,
    plane: super::block::ReconstructedPlane,
    cdef_parameters: Option<super::cdef::FrameParameters>,
    cdef_indices: Vec<Option<usize>>,
    cdef_active: Vec<bool>,
    loop_parameters: Option<super::filter::Parameters>,
    filter_blocks: Vec<super::filter::Block>,
}

fn assemble_color_tiles(
    tiles: &[ReconstructedColorTile],
    trailing_tiles: &[ReconstructedColorTile],
    header: &FrameHeader,
    sequence: &SequenceHeader,
) -> Av1Result<Option<super::block::FirstLeaf>> {
    if tiles.is_empty() && trailing_tiles.is_empty() {
        return Ok(None);
    }
    let frame_width = usize::try_from(header.frame_width)
        .map_err(|_| malformed("assembled frame width exceeds usize"))?;
    let frame_height = usize::try_from(header.frame_height)
        .map_err(|_| malformed("assembled frame height exceeds usize"))?;
    let active_width = frame_width.div_ceil(8);
    let active_height = frame_height.div_ceil(8);
    let region_width = frame_width.div_ceil(64);
    let region_height = frame_height.div_ceil(64);
    let mut canvas = super::raster::FrameCanvas::new(
        header.frame_width,
        header.frame_height,
        sequence.subsampling_x,
        sequence.subsampling_y,
    )?;
    let filter_block_count = tiles
        .iter()
        .chain(trailing_tiles)
        .try_fold(0_usize, |count, tile| {
            count.checked_add(tile.reconstruction.filter_blocks.len())
        })
        .ok_or_else(|| malformed("assembled loop-filter block count overflows"))?;
    let mut filter_blocks = Vec::new();
    #[cfg(coverage)]
    let filter_block_reservation_count = if crate::coverage_support::take_fault_point(
        crate::coverage_support::CoverageFaultPoint::Av1AssembledLoopFilterMetadataReservation,
    ) {
        usize::MAX
    } else {
        filter_block_count
    };
    #[cfg(not(coverage))]
    let filter_block_reservation_count = filter_block_count;
    filter_blocks
        .try_reserve_exact(filter_block_reservation_count)
        .map_err(|_| {
            CodecError::Dimensions("unable to allocate AV1 loop-filter metadata".to_owned())
        })?;
    let cdef_region_count = region_width
        .checked_mul(region_height)
        .ok_or_else(|| malformed("assembled CDEF region map allocation overflows"))?;
    let cdef_active_count = active_width
        .checked_mul(active_height)
        .ok_or_else(|| malformed("assembled CDEF active map allocation overflows"))?;
    let mut cdef_indices = Vec::new();
    cdef_indices
        .try_reserve_exact(cdef_region_count)
        .map_err(|_| {
            CodecError::Dimensions("unable to allocate assembled AV1 CDEF region map".to_owned())
        })?;
    cdef_indices.resize(cdef_region_count, None);
    let mut cdef_active = Vec::new();
    cdef_active
        .try_reserve_exact(cdef_active_count)
        .map_err(|_| {
            CodecError::Dimensions("unable to allocate assembled AV1 CDEF active map".to_owned())
        })?;
    cdef_active.resize(cdef_active_count, false);
    let mut loop_parameters: Option<super::filter::Parameters> = None;
    let mut cdef_parameters: Option<super::cdef::FrameParameters> = None;
    let mut filter_parameters_initialized = false;
    for tile in tiles.iter().chain(trailing_tiles) {
        let tile_x = usize::try_from(tile.x)
            .map_err(|_| malformed("assembled tile x origin exceeds usize"))?;
        let tile_y = usize::try_from(tile.y)
            .map_err(|_| malformed("assembled tile y origin exceeds usize"))?;
        let tile_width = usize::try_from(tile.width)
            .map_err(|_| malformed("assembled tile width exceeds usize"))?;
        let tile_height = usize::try_from(tile.height)
            .map_err(|_| malformed("assembled tile height exceeds usize"))?;
        let tile_loop_parameters = tile.reconstruction.loop_parameters;
        let tile_cdef_parameters = tile.reconstruction.cdef_parameters;
        if !filter_parameters_initialized {
            loop_parameters = tile_loop_parameters;
            cdef_parameters = tile_cdef_parameters;
            filter_parameters_initialized = true;
        } else if tile_loop_parameters != loop_parameters || tile_cdef_parameters != cdef_parameters
        {
            return Err(malformed("assembled tiles disagree on filter parameters"));
        }
        if tile_cdef_parameters.is_some()
            && (!tile_x.is_multiple_of(64) || !tile_y.is_multiple_of(64))
        {
            return Err(malformed("CDEF tile origin is not 64-pixel aligned"));
        }
        canvas.place_planes(
            tile.width,
            tile.height,
            &tile.reconstruction.leaf.planes,
            tile.x,
            tile.y,
        )?;

        for block in &tile.reconstruction.filter_blocks {
            filter_blocks.push(super::filter::Block {
                x: tile_x
                    .checked_add(block.x)
                    .ok_or_else(|| malformed("assembled loop-filter x overflows"))?,
                y: tile_y
                    .checked_add(block.y)
                    .ok_or_else(|| malformed("assembled loop-filter y overflows"))?,
                ..block.clone()
            });
        }

        if tile_cdef_parameters.is_none() {
            if !tile.reconstruction.cdef_active.is_empty()
                || !tile.reconstruction.cdef_indices.is_empty()
            {
                return Err(malformed("CDEF-disabled tile carries non-empty CDEF maps"));
            }
            continue;
        }

        let local_active_width = tile_width.div_ceil(8);
        let local_active_height = tile_height.div_ceil(8);
        if tile.reconstruction.cdef_active.len()
            != local_active_width.saturating_mul(local_active_height)
        {
            return Err(malformed("tile CDEF active map has an invalid extent"));
        }
        for local_y in 0..local_active_height {
            for local_x in 0..local_active_width {
                let local_index = local_y
                    .checked_mul(local_active_width)
                    .and_then(|row| row.checked_add(local_x))
                    .ok_or_else(|| malformed("tile CDEF active index overflows"))?;
                if !tile.reconstruction.cdef_active[local_index] {
                    continue;
                }
                let x = tile_x
                    .checked_add(local_x.saturating_mul(8))
                    .ok_or_else(|| malformed("assembled CDEF x overflows"))?;
                let y = tile_y
                    .checked_add(local_y.saturating_mul(8))
                    .ok_or_else(|| malformed("assembled CDEF y overflows"))?;
                if x >= frame_width || y >= frame_height {
                    continue;
                }
                let global_index = (y / 8)
                    .checked_mul(active_width)
                    .and_then(|row| row.checked_add(x / 8))
                    .ok_or_else(|| malformed("assembled CDEF active index overflows"))?;
                let Some(slot) = cdef_active.get_mut(global_index) else {
                    return Err(malformed("assembled CDEF active index exceeds frame"));
                };
                *slot = true;
            }
        }

        let local_region_width = tile_width.div_ceil(64);
        let local_region_height = tile_height.div_ceil(64);
        if tile.reconstruction.cdef_indices.len()
            != local_region_width.saturating_mul(local_region_height)
        {
            return Err(malformed("tile CDEF index map has an invalid extent"));
        }
        for local_y in 0..local_region_height {
            for local_x in 0..local_region_width {
                let local_index = local_y
                    .checked_mul(local_region_width)
                    .and_then(|row| row.checked_add(local_x))
                    .ok_or_else(|| malformed("tile CDEF index overflows"))?;
                let Some(cdef_index) = tile.reconstruction.cdef_indices[local_index] else {
                    continue;
                };
                let x = tile_x
                    .checked_add(local_x.saturating_mul(64))
                    .ok_or_else(|| malformed("assembled CDEF region x overflows"))?;
                let y = tile_y
                    .checked_add(local_y.saturating_mul(64))
                    .ok_or_else(|| malformed("assembled CDEF region y overflows"))?;
                if x >= frame_width || y >= frame_height {
                    continue;
                }
                let global_index = (y / 64)
                    .checked_mul(region_width)
                    .and_then(|row| row.checked_add(x / 64))
                    .ok_or_else(|| malformed("assembled CDEF region index overflows"))?;
                let Some(slot) = cdef_indices.get_mut(global_index) else {
                    return Err(malformed("assembled CDEF region exceeds frame"));
                };
                if let Some(existing) = *slot {
                    if existing != cdef_index {
                        return Err(malformed("assembled CDEF regions disagree"));
                    }
                } else {
                    *slot = Some(cdef_index);
                }
            }
        }
    }
    let planes = canvas.finish_with_filters(
        loop_parameters,
        &filter_blocks,
        None,
        None,
        cdef_parameters,
        &cdef_indices,
        &cdef_active,
    )?;
    Ok(Some(super::block::FirstLeaf {
        width: header.frame_width,
        height: header.frame_height,
        block_skipped: false,
        planes,
        luma_predictor: super::block::LumaPredictor::Dc,
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
    }))
}

fn upscale_color_leaf_for_superres(
    leaf: super::block::FirstLeaf,
    header: &FrameHeader,
    sequence: &SequenceHeader,
    source_width: u32,
    source_height: u32,
    depth: SampleDepth,
) -> Av1Result<super::block::FirstLeaf> {
    let geometry = resize::ResizeGeometry {
        coded_width: header.frame_width,
        source_width,
        source_height,
        output_width: header.upscaled_width,
        output_height: header.frame_height,
        superres_denominator: header.superres_denominator,
    };
    match PixelLayout::from_sequence(
        sequence.monochrome,
        sequence.subsampling_x,
        sequence.subsampling_y,
    ) {
        Some(PixelLayout::I420) => resize::upscale_i420_leaf(leaf, geometry, depth),
        Some(PixelLayout::I422) => resize::upscale_i422_leaf(leaf, geometry, depth),
        Some(PixelLayout::I444) => resize::upscale_i444_leaf(leaf, geometry, depth),
        _ => Err(malformed(
            "super-resolution carries an unsupported chroma sampling",
        )),
    }
}

fn assemble_monochrome_tiles(
    tiles: &[ReconstructedMonochromeTile],
    trailing_tiles: &[ReconstructedMonochromeTile],
    header: &FrameHeader,
    sequence: &SequenceHeader,
) -> Av1Result<super::block::ReconstructedPlane> {
    let depth = SampleDepth::new(sequence.bit_depth)
        .ok_or_else(|| malformed("monochrome tile sample depth is unsupported"))?;
    let frame_width = usize::try_from(header.frame_width)
        .map_err(|_| malformed("assembled monochrome frame width exceeds usize"))?;
    let frame_height = usize::try_from(header.frame_height)
        .map_err(|_| malformed("assembled monochrome frame height exceeds usize"))?;
    let cdef_enabled = tiles
        .iter()
        .chain(trailing_tiles)
        .any(|tile| tile.cdef_parameters.is_some());
    let loop_filter_enabled = tiles
        .iter()
        .chain(trailing_tiles)
        .any(|tile| tile.loop_parameters.is_some());
    let coded_frame_width = if cdef_enabled {
        frame_width
            .div_ceil(8)
            .checked_mul(8)
            .ok_or(malformed("coded monochrome frame width overflows"))?
    } else {
        frame_width
    };
    let coded_frame_height = if cdef_enabled {
        frame_height
            .div_ceil(8)
            .checked_mul(8)
            .ok_or(malformed("coded monochrome frame height overflows"))?
    } else {
        frame_height
    };
    let coded_frame_width_u32 = u32::try_from(coded_frame_width)
        .map_err(|_| malformed("coded monochrome frame width exceeds u32"))?;
    let coded_frame_height_u32 = u32::try_from(coded_frame_height)
        .map_err(|_| malformed("coded monochrome frame height exceeds u32"))?;
    let mut loop_parameters = None;
    let mut filter_blocks = Vec::new();
    if loop_filter_enabled {
        let block_count = tiles
            .iter()
            .chain(trailing_tiles)
            .try_fold(0_usize, |count, tile| {
                count.checked_add(tile.filter_blocks.len())
            })
            .ok_or_else(|| malformed("assembled monochrome loop-filter count overflows"))?;
        #[cfg(coverage)]
        let reservation_count = if crate::coverage_support::take_fault_point(
            crate::coverage_support::CoverageFaultPoint::
                Av1AssembledMonochromeLoopFilterMetadataReservation,
        ) {
            usize::MAX
        } else {
            block_count
        };
        #[cfg(not(coverage))]
        let reservation_count = block_count;
        filter_blocks
            .try_reserve_exact(reservation_count)
            .map_err(|_| {
                CodecError::Dimensions(
                    "unable to allocate assembled monochrome loop-filter metadata".to_owned(),
                )
            })?;
    }
    let active_width = frame_width.div_ceil(8);
    let active_height = frame_height.div_ceil(8);
    let region_width = frame_width.div_ceil(64);
    let region_height = frame_height.div_ceil(64);
    let mut cdef_indices = Vec::new();
    let mut cdef_active = Vec::new();
    if cdef_enabled {
        let region_count = region_width
            .checked_mul(region_height)
            .ok_or_else(|| malformed("assembled monochrome CDEF region map overflows"))?;
        #[cfg(coverage)]
        let region_reservation_count = if crate::coverage_support::take_fault_point(
            crate::coverage_support::CoverageFaultPoint::
                Av1AssembledMonochromeCdefRegionMapReservation,
        ) {
            usize::MAX
        } else {
            region_count
        };
        #[cfg(not(coverage))]
        let region_reservation_count = region_count;
        cdef_indices
            .try_reserve_exact(region_reservation_count)
            .map_err(|_| {
                CodecError::Dimensions(
                    "unable to allocate assembled monochrome CDEF map".to_owned(),
                )
            })?;
        cdef_indices.resize(region_count, None);
        let active_count = active_width
            .checked_mul(active_height)
            .ok_or_else(|| malformed("assembled monochrome CDEF active map overflows"))?;
        #[cfg(coverage)]
        let active_reservation_count = if crate::coverage_support::take_fault_point(
            crate::coverage_support::CoverageFaultPoint::
                Av1AssembledMonochromeCdefActiveMapReservation,
        ) {
            usize::MAX
        } else {
            active_count
        };
        #[cfg(not(coverage))]
        let active_reservation_count = active_count;
        cdef_active
            .try_reserve_exact(active_reservation_count)
            .map_err(|_| {
                CodecError::Dimensions(
                    "unable to allocate assembled monochrome CDEF active map".to_owned(),
                )
            })?;
        cdef_active.resize(active_count, false);
    }
    let mut cdef_parameters = None;
    let mut canvas = super::raster::MonochromeFrameCanvas::new_padded(
        header.frame_width,
        header.frame_height,
        coded_frame_width_u32,
        coded_frame_height_u32,
    )?;
    for tile in tiles.iter().chain(trailing_tiles) {
        if loop_filter_enabled {
            let Some(tile_parameters) = tile.loop_parameters else {
                return Err(malformed(
                    "assembled monochrome loop-filter tile omits parameters",
                ));
            };
            if let Some(existing) = loop_parameters {
                if existing != tile_parameters {
                    return Err(malformed(
                        "assembled monochrome tiles disagree on loop-filter parameters",
                    ));
                }
            } else {
                loop_parameters = Some(tile_parameters);
            }
        } else if tile.loop_parameters.is_some() || !tile.filter_blocks.is_empty() {
            return Err(malformed(
                "loop-filter-disabled monochrome tile carries filter metadata",
            ));
        }
        if cdef_enabled {
            let Some(tile_parameters) = tile.cdef_parameters else {
                return Err(malformed(
                    "assembled monochrome CDEF tile omits frame parameters",
                ));
            };
            if let Some(existing) = cdef_parameters {
                if existing != tile_parameters {
                    return Err(malformed(
                        "assembled monochrome tiles disagree on CDEF parameters",
                    ));
                }
            } else {
                cdef_parameters = Some(tile_parameters);
            }
        } else if tile.cdef_parameters.is_some()
            || !tile.cdef_indices.is_empty()
            || !tile.cdef_active.is_empty()
        {
            return Err(malformed(
                "CDEF-disabled monochrome tile carries CDEF metadata",
            ));
        }
        let tile_x = usize::try_from(tile.x)
            .map_err(|_| malformed("monochrome filter tile x origin exceeds usize"))?;
        let tile_y = usize::try_from(tile.y)
            .map_err(|_| malformed("monochrome filter tile y origin exceeds usize"))?;
        let coded_tile_width = usize::try_from(tile.coded_width)
            .map_err(|_| malformed("coded monochrome tile width exceeds usize"))?;
        let coded_tile_height = usize::try_from(tile.coded_height)
            .map_err(|_| malformed("coded monochrome tile height exceeds usize"))?;
        let tile_width = usize::try_from(tile.width)
            .map_err(|_| malformed("monochrome filter tile width exceeds usize"))?;
        let tile_height = usize::try_from(tile.height)
            .map_err(|_| malformed("monochrome filter tile height exceeds usize"))?;
        let visible_coded_tile_width = coded_frame_width
            .saturating_sub(tile_x)
            .min(coded_tile_width);
        let visible_coded_tile_height = coded_frame_height
            .saturating_sub(tile_y)
            .min(coded_tile_height);
        canvas.place_cropped_plane(
            tile.coded_width,
            tile.coded_height,
            u32::try_from(visible_coded_tile_width)
                .map_err(|_| malformed("visible coded monochrome tile width exceeds u32"))?,
            u32::try_from(visible_coded_tile_height)
                .map_err(|_| malformed("visible coded monochrome tile height exceeds u32"))?,
            tile.x,
            tile.y,
            &tile.plane,
        )?;

        if !cdef_enabled && !loop_filter_enabled {
            continue;
        }
        if cdef_enabled {
            if tile_x % 64 != 0 || tile_y % 64 != 0 {
                return Err(malformed(
                    "monochrome CDEF tile origin is not 64-pixel aligned",
                ));
            }
            if coded_tile_width < 8
                || coded_tile_height < 8
                || !coded_tile_width.is_multiple_of(8)
                || !coded_tile_height.is_multiple_of(8)
            {
                return Err(malformed(
                    "coded monochrome CDEF tile extent is not 8-pixel aligned",
                ));
            }
            let local_active_width = tile_width.div_ceil(8);
            let local_active_height = tile_height.div_ceil(8);
            let local_active_length = local_active_width
                .checked_mul(local_active_height)
                .ok_or_else(|| malformed("monochrome tile CDEF active map overflows"))?;
            if tile.cdef_active.len() != local_active_length {
                return Err(malformed(
                    "monochrome tile CDEF active map has an invalid extent",
                ));
            }
            for local_y in 0..local_active_height {
                for local_x in 0..local_active_width {
                    let local_index = local_y
                        .checked_mul(local_active_width)
                        .and_then(|row| row.checked_add(local_x))
                        .ok_or_else(|| malformed("monochrome tile CDEF active index overflows"))?;
                    if !tile.cdef_active[local_index] {
                        continue;
                    }
                    let x = tile_x
                        .checked_add(
                            local_x
                                .checked_mul(8)
                                .ok_or_else(|| malformed("monochrome CDEF x overflows"))?,
                        )
                        .ok_or_else(|| malformed("monochrome CDEF x overflows"))?;
                    let y = tile_y
                        .checked_add(
                            local_y
                                .checked_mul(8)
                                .ok_or_else(|| malformed("monochrome CDEF y overflows"))?,
                        )
                        .ok_or_else(|| malformed("monochrome CDEF y overflows"))?;
                    if x >= frame_width || y >= frame_height {
                        return Err(malformed("monochrome CDEF active block exceeds frame"));
                    }
                    let global_index = (y / 8)
                        .checked_mul(active_width)
                        .and_then(|row| row.checked_add(x / 8))
                        .ok_or_else(|| {
                            malformed("assembled monochrome CDEF active index overflows")
                        })?;
                    let Some(slot) = cdef_active.get_mut(global_index) else {
                        return Err(malformed(
                            "assembled monochrome CDEF active index exceeds frame",
                        ));
                    };
                    *slot = true;
                }
            }

            let local_region_width = tile_width.div_ceil(64);
            let local_region_height = tile_height.div_ceil(64);
            let local_region_length = local_region_width
                .checked_mul(local_region_height)
                .ok_or_else(|| malformed("monochrome tile CDEF index map overflows"))?;
            if tile.cdef_indices.len() != local_region_length {
                return Err(malformed(
                    "monochrome tile CDEF index map has an invalid extent",
                ));
            }
            for local_y in 0..local_region_height {
                for local_x in 0..local_region_width {
                    let local_index = local_y
                        .checked_mul(local_region_width)
                        .and_then(|row| row.checked_add(local_x))
                        .ok_or_else(|| malformed("monochrome tile CDEF index overflows"))?;
                    let Some(cdef_index) = tile.cdef_indices[local_index] else {
                        continue;
                    };
                    let x = tile_x
                        .checked_add(
                            local_x
                                .checked_mul(64)
                                .ok_or_else(|| malformed("monochrome CDEF region x overflows"))?,
                        )
                        .ok_or_else(|| malformed("monochrome CDEF region x overflows"))?;
                    let y = tile_y
                        .checked_add(
                            local_y
                                .checked_mul(64)
                                .ok_or_else(|| malformed("monochrome CDEF region y overflows"))?,
                        )
                        .ok_or_else(|| malformed("monochrome CDEF region y overflows"))?;
                    if x >= frame_width || y >= frame_height {
                        return Err(malformed("monochrome CDEF region exceeds frame"));
                    }
                    let global_index = (y / 64)
                        .checked_mul(region_width)
                        .and_then(|row| row.checked_add(x / 64))
                        .ok_or_else(|| malformed("assembled monochrome CDEF region overflows"))?;
                    let Some(slot) = cdef_indices.get_mut(global_index) else {
                        return Err(malformed("assembled monochrome CDEF region exceeds frame"));
                    };
                    if let Some(existing) = *slot {
                        if existing != cdef_index {
                            return Err(malformed("assembled monochrome CDEF regions disagree"));
                        }
                    } else {
                        *slot = Some(cdef_index);
                    }
                }
            }
        }
        if loop_filter_enabled {
            for block in &tile.filter_blocks {
                if block.has_chroma || block.levels[2] != 0 || block.levels[3] != 0 {
                    return Err(malformed(
                        "monochrome loop-filter block carries chroma metadata",
                    ));
                }
                let local_end_x = block
                    .x
                    .checked_add(block.width)
                    .ok_or_else(|| malformed("monochrome loop-filter block x overflows"))?;
                let local_end_y = block
                    .y
                    .checked_add(block.height)
                    .ok_or_else(|| malformed("monochrome loop-filter block y overflows"))?;
                if block.width == 0
                    || block.height == 0
                    || local_end_x > coded_tile_width
                    || local_end_y > coded_tile_height
                {
                    return Err(malformed("monochrome loop-filter block exceeds its tile"));
                }
                let x = tile_x
                    .checked_add(block.x)
                    .ok_or_else(|| malformed("assembled monochrome loop-filter x overflows"))?;
                let y = tile_y
                    .checked_add(block.y)
                    .ok_or_else(|| malformed("assembled monochrome loop-filter y overflows"))?;
                let end_x = x
                    .checked_add(block.width)
                    .ok_or_else(|| malformed("assembled monochrome loop-filter x overflows"))?;
                let end_y = y
                    .checked_add(block.height)
                    .ok_or_else(|| malformed("assembled monochrome loop-filter y overflows"))?;
                if end_x > coded_frame_width || end_y > coded_frame_height {
                    return Err(malformed(
                        "assembled monochrome loop-filter block exceeds coded frame",
                    ));
                }
                filter_blocks.push(super::filter::Block {
                    x,
                    y,
                    ..block.clone()
                });
            }
        }
    }
    let plane = if cdef_enabled && loop_filter_enabled {
        canvas.finish_monochrome_with_loop_filter_and_cdef(
            loop_parameters,
            &filter_blocks,
            cdef_parameters,
            &cdef_indices,
            &cdef_active,
            depth,
        )?
    } else if cdef_enabled {
        canvas.finish_monochrome_with_cdef(cdef_parameters, &cdef_indices, &cdef_active, depth)?
    } else if loop_filter_enabled {
        canvas.finish_monochrome_with_loop_filter(loop_parameters, &filter_blocks, depth)?
    } else {
        canvas.finish(depth)?
    };
    Ok(plane)
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
        if tile_size == 0 {
            return Err(malformed("tile payload is empty"));
        }
        let tile_end = cursor.saturating_add(tile_size);
        ranges.push(cursor..tile_end);
        cursor = tile_end;
    }
    // The final tile consumes `payload_end - cursor`, so a non-empty inclusive
    // tile range always ends exactly at the payload boundary.
    debug_assert_eq!(cursor, payload_end);
    Ok(ranges)
}

struct TileEntropyInputs<'state, 'reference> {
    input_cdfs: Option<&'state entropy::FrameCdfs>,
    current_segment_map: Option<&'state entropy::SegmentMap>,
    previous_segment_map: Option<&'state entropy::SegmentMap>,
    inter_context: Option<&'state entropy::InterFrameContext<'reference>>,
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
    inputs: TileEntropyInputs<'_, '_>,
) -> Av1Result<TileValidation> {
    let TileEntropyInputs {
        input_cdfs,
        current_segment_map,
        previous_segment_map,
        inter_context,
    } = inputs;
    let root_level = u32::from(!sequence.use_128x128_superblock);
    // Frame dimensions and superblock mode were validated while parsing the
    // sequence/frame headers, so overflow is unreachable for valid AV1. Keep
    // the private unit conversion checked independently of that invariant.
    let block_width = header
        .frame_width
        .div_ceil(8)
        .checked_mul(2)
        .ok_or_else(|| malformed("frame block width overflows"))?;
    let block_height = header
        .frame_height
        .div_ceil(8)
        .checked_mul(2)
        .ok_or_else(|| malformed("frame block height overflows"))?;
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
            y_strengths: std::array::from_fn(|index| {
                cdef.y_strengths.get(index).copied().unwrap_or(0)
            }),
            uv_strengths: std::array::from_fn(|index| {
                cdef.uv_strengths.get(index).copied().unwrap_or(0)
            }),
            first_y_strength: cdef.y_strengths.first().copied(),
            first_uv_strength: cdef.uv_strengths.first().copied(),
        }),
        restoration_present: header.restoration.is_some(),
        transform_mode: header.transform_mode,
        reduced_transform_set: header.reduced_transform_set,
        film_grain_present: header.film_grain.is_some(),
        segmentation: entropy::SegmentationContext {
            enabled: header.segmentation.enabled,
            update_map: header.segmentation.update_map,
            temporal: header.segmentation.temporal,
            preskip: header.segmentation.preskip,
            last_active_id: header.segmentation.last_active_id,
            segments: std::array::from_fn(|index| {
                let segment = header.segmentation.segments[index];
                entropy::SegmentContext {
                    delta_q: segment.delta_q,
                    delta_lf: [
                        segment.delta_lf_y_vertical,
                        segment.delta_lf_y_horizontal,
                        segment.delta_lf_u,
                        segment.delta_lf_v,
                    ],
                    reference: segment.reference,
                    skip: segment.skip,
                    global_motion: segment.global_motion,
                    qindex: header.segment_qindex[index],
                    lossless: header.segment_lossless[index],
                }
            }),
        },
    };
    let mut first_leaf = None;
    let mut complete_color_leaf = None;
    let mut complete_color_tiles = Vec::new();
    let mut temporal_samples = Vec::new();
    let mut complete_monochrome_tiles = Vec::new();
    let mut complete_monochrome_plane = None;
    let mut selected_cdfs = None;
    let mut segment_map = current_segment_map.cloned();
    let mut decode_complete = true;
    for (range_index, range) in ranges.iter().enumerate() {
        if range.is_empty() {
            return Err(malformed("tile payload is empty"));
        }
        let Some(input_cdfs) = input_cdfs else {
            // The reference header remains useful for structural validation,
            // but an incomplete retained entropy bundle cannot be substituted
            // with defaults without changing the bitstream grammar.
            decode_complete = false;
            continue;
        };
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
            intra_frame: header.frame_type.is_intra(),
            level: root_level,
            block_width,
            block_height,
            block_x,
            block_y,
            tile_origin_b4_x: 0,
            tile_origin_b4_y: 0,
            single_tile: tiling.tile_count() == 1 && ranges.len() == 1,
            frame_block_width: block_width,
            frame_block_height: block_height,
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
            enable_intra_edge_filter: sequence.enable_intra_edge_filter,
            frame_tools,
        };
        let next_column = column.saturating_add(1) as usize;
        let next_row = row.saturating_add(1) as usize;
        let tile_block_end_x = tiling
            .column_starts
            .get(next_column)
            .copied()
            .ok_or(malformed("frame syntax validation failed"))?
            .wrapping_shl(block_shift)
            .min(block_width);
        let tile_block_end_y = tiling
            .row_starts
            .get(next_row)
            .copied()
            .ok_or(malformed("frame syntax validation failed"))?
            .wrapping_shl(block_shift)
            .min(block_height);
        let tile_block_width = tile_block_end_x.saturating_sub(block_x);
        let tile_block_height = tile_block_end_y.saturating_sub(block_y);
        let coded_tile_width = tile_block_width
            .checked_mul(4)
            .ok_or(malformed("coded tile width overflows pixels"))?;
        let coded_tile_height = tile_block_height
            .checked_mul(4)
            .ok_or(malformed("coded tile height overflows pixels"))?;
        let tile_origin_x = block_x.wrapping_mul(4);
        let tile_origin_y = block_y.wrapping_mul(4);
        // Tile boundaries are coded-superblock boundaries, so the final tile
        // can extend beyond a cropped visible frame. Keep reconstruction's
        // local canvas visible rather than handing the padded extent to the
        // block walker (a 4x4 image otherwise masquerades as a 64x64 tile).
        let tile_width = header
            .frame_width
            .saturating_sub(tile_origin_x)
            .min(tile_block_width.wrapping_mul(4));
        let tile_height = header
            .frame_height
            .saturating_sub(tile_origin_y)
            .min(tile_block_height.wrapping_mul(4));
        let mut tile_context = context;
        tile_context.block_width = tile_block_width;
        tile_context.block_height = tile_block_height;
        tile_context.block_x = 0;
        tile_context.block_y = 0;
        tile_context.tile_origin_b4_x = block_x;
        tile_context.tile_origin_b4_y = block_y;
        tile_context.frame_width = tile_width;
        tile_context.frame_height = tile_height;
        // Super-resolution is a frame-wide post-filter. Keep the target
        // display width in the entropy context so its admission gate can
        // distinguish coded and upscaled coordinates; non-superres tiles
        // retain the historical tile-local width used by the block walker.
        tile_context.upscaled_width = if header.superres_enabled {
            header.upscaled_width
        } else {
            tile_width
        };
        let complete = entropy::validate_complete_lossy_420_partition(
            data,
            range.clone(),
            &tile_context,
            input_cdfs,
            segment_map.as_mut(),
            previous_segment_map,
            inter_context,
        )?;
        if let Some(mut reconstruction) = complete {
            let tile_temporal_samples = std::mem::take(&mut reconstruction.temporal_samples);
            temporal_samples
                .try_reserve(tile_temporal_samples.len())
                .map_err(|_| {
                    CodecError::Dimensions(
                        "unable to allocate AV1 tile-group temporal-MV samples".to_owned(),
                    )
                })?;
            temporal_samples.extend(tile_temporal_samples);
            let tile_cdfs = reconstruction
                .cdfs
                .take()
                .ok_or_else(|| malformed("complete tile omits its entropy snapshot"))?;
            if header.refresh_frame_context && tile == tiling.context_update_tile {
                selected_cdfs = Some(if header.frame_type.is_inter() {
                    entropy::FrameCdfs::publish_inter(input_cdfs, &tile_cdfs)
                } else {
                    entropy::FrameCdfs::publish_intra(input_cdfs, &tile_cdfs)
                });
            }
            if tiling.tile_count() == 1 && ranges.len() == 1 {
                let mut restoration_plan = reconstruction.restoration;
                if sequence.monochrome {
                    let plane = reconstruction.into_monochrome_plane()?;
                    let plane = if header.superres_enabled {
                        let depth = SampleDepth::new(sequence.bit_depth).ok_or_else(|| {
                            malformed("super-resolution sample depth is unsupported")
                        })?;
                        resize::upscale_monochrome_plane(
                            plane,
                            header.frame_width,
                            header.upscaled_width,
                            header.frame_height,
                            header.superres_denominator,
                            depth,
                        )?
                    } else {
                        plane
                    };
                    let plane = if let Some(plan) = restoration_plan {
                        let depth = SampleDepth::new(sequence.bit_depth)
                            .ok_or_else(|| malformed("restoration sample depth is unsupported"))?;
                        restoration::restore_monochrome_plane(
                            plane,
                            header.upscaled_width,
                            header.frame_height,
                            plan,
                            depth,
                        )?
                    } else {
                        plane
                    };
                    complete_monochrome_plane = Some(plane);
                    continue;
                }
                let full_resolution =
                    !reconstruction.subsampling_x && !reconstruction.subsampling_y;
                let subsampled_420 = reconstruction.subsampling_x && reconstruction.subsampling_y;
                let subsampled_422 = reconstruction.subsampling_x && !reconstruction.subsampling_y;
                let striped_restoration = if !header.superres_enabled && header.frame_height > 56 {
                    restoration_plan
                        .take()
                        .map(|plan| {
                            SampleDepth::new(sequence.bit_depth)
                                .map(|depth| (plan, depth))
                                .ok_or_else(|| malformed("restoration sample depth is unsupported"))
                        })
                        .transpose()?
                } else {
                    None
                };
                let filtered = reconstruction.into_filtered_leaf(striped_restoration)?;
                let leaf = if header.superres_enabled {
                    let depth = SampleDepth::new(sequence.bit_depth)
                        .ok_or_else(|| malformed("super-resolution sample depth is unsupported"))?;
                    upscale_color_leaf_for_superres(
                        filtered.leaf,
                        header,
                        sequence,
                        filtered.source_width,
                        filtered.source_height,
                        depth,
                    )?
                } else {
                    filtered.leaf
                };
                let leaf = if let Some(plan) = restoration_plan {
                    let depth = SampleDepth::new(sequence.bit_depth)
                        .ok_or_else(|| malformed("restoration sample depth is unsupported"))?;
                    if full_resolution {
                        restoration::restore_i444_leaf(leaf, plan, depth)?
                    } else if subsampled_420 {
                        restoration::restore_i420_leaf(leaf, plan, depth)?
                    } else if subsampled_422 {
                        restoration::restore_i422_leaf(leaf, plan, depth)?
                    } else {
                        return Err(malformed(
                            "restoration carries an unsupported chroma sampling",
                        ));
                    }
                } else {
                    leaf
                };
                complete_color_leaf = Some(leaf);
            } else if sequence.monochrome {
                if reconstruction.restoration.is_some() {
                    return Err(malformed(
                        "multi-tile monochrome reconstruction carries a post-filter",
                    ));
                }
                let (
                    plane,
                    cdef_parameters,
                    cdef_indices,
                    cdef_active,
                    loop_parameters,
                    filter_blocks,
                ) = reconstruction.into_unfiltered_monochrome_tile()?;
                complete_monochrome_tiles.try_reserve(1).map_err(|_| {
                    CodecError::Dimensions(
                        "unable to allocate reconstructed AV1 monochrome tiles".to_owned(),
                    )
                })?;
                complete_monochrome_tiles.push(ReconstructedMonochromeTile {
                    x: tile_origin_x,
                    y: tile_origin_y,
                    width: tile_width,
                    height: tile_height,
                    coded_width: if cdef_parameters.is_some() {
                        coded_tile_width
                    } else {
                        tile_width
                    },
                    coded_height: if cdef_parameters.is_some() {
                        coded_tile_height
                    } else {
                        tile_height
                    },
                    plane,
                    cdef_parameters,
                    cdef_indices,
                    cdef_active,
                    loop_parameters,
                    filter_blocks,
                });
            } else {
                complete_color_tiles.push(ReconstructedColorTile {
                    x: tile_origin_x,
                    y: tile_origin_y,
                    width: tile_width,
                    height: tile_height,
                    reconstruction,
                });
            }
        } else {
            // Unsupported complete classes still retain the earlier bounded
            // prefix/first-leaf diagnostic. It operates on private state and
            // must never become the reference CDF publication source.
            if !header.segmentation.enabled {
                first_leaf = first_leaf.or(entropy::validate_first_partition(
                    data,
                    range.clone(),
                    &tile_context,
                )?);
            }
            if sequence.monochrome {
                let plane = entropy::validate_complete_monochrome_partition(
                    data,
                    range.clone(),
                    &tile_context,
                )?;
                if tiling.tile_count() == 1 && ranges.len() == 1 {
                    complete_monochrome_plane = plane
                        .map(|plane| {
                            if header.superres_enabled {
                                let depth =
                                    SampleDepth::new(sequence.bit_depth).ok_or_else(|| {
                                        malformed("super-resolution sample depth is unsupported")
                                    })?;
                                resize::upscale_monochrome_plane(
                                    plane,
                                    header.frame_width,
                                    header.upscaled_width,
                                    header.frame_height,
                                    header.superres_denominator,
                                    depth,
                                )
                            } else {
                                Ok(plane)
                            }
                        })
                        .transpose()?;
                } else if header.superres_enabled
                    && let Some(plane) = plane
                {
                    complete_monochrome_tiles.try_reserve(1).map_err(|_| {
                        CodecError::Dimensions(
                            "unable to allocate reconstructed AV1 monochrome tiles".to_owned(),
                        )
                    })?;
                    complete_monochrome_tiles.push(ReconstructedMonochromeTile {
                        x: tile_origin_x,
                        y: tile_origin_y,
                        width: tile_width,
                        height: tile_height,
                        coded_width: tile_width,
                        coded_height: tile_height,
                        plane,
                        cdef_parameters: None,
                        cdef_indices: Vec::new(),
                        cdef_active: Vec::new(),
                        loop_parameters: None,
                        filter_blocks: Vec::new(),
                    });
                }
            }
            decode_complete = false;
        }
    }
    Ok(TileValidation {
        first_leaf,
        complete_color_leaf,
        complete_color_tiles,
        temporal_samples,
        complete_monochrome_tiles,
        complete_monochrome_plane,
        selected_cdfs,
        segment_map,
        decode_complete,
    })
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
    layer: (u32, u32),
    previous_frame_id: Option<u32>,
) -> Av1Result<(FrameHeader, BitReader<'data, 'input, 'spans>)> {
    let bits = BitReader::new(data, start, end)?;
    // `BitReader::new` has already validated the byte-to-bit conversion.
    let start_bit = bits.position();
    parse_reader(
        bits,
        start_bit,
        sequence,
        references,
        layer,
        previous_frame_id,
    )
}

fn parse_reader<'data, 'input, 'spans>(
    mut bits: BitReader<'data, 'input, 'spans>,
    start_bit: usize,
    sequence: &SequenceHeader,
    references: &[Option<FrameHeader>; 8],
    layer: (u32, u32),
    previous_frame_id: Option<u32>,
) -> Av1Result<(FrameHeader, BitReader<'data, 'input, 'spans>)> {
    let (temporal_id, spatial_id) = layer;
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
        // Validate continuity before reference deltas. Otherwise a repeated
        // current ID also changes the expected reference ID and hides the
        // earlier, more specific structural error.
        validate_current_frame_id(sequence.frame_id_bits, previous_frame_id, &header)?;
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
    if frame_id == previous {
        return Err(malformed("current frame ID repeats the previous frame ID"));
    }
    if difference >= 1_u32 << frame_id_bits.saturating_sub(1) {
        return Err(malformed("current frame ID is outside sequence continuity"));
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
    }
    // Explicit indices and their deltas are interleaved. With short
    // signaling, the indices above are derived and only deltas are coded.
    // Pinned dav1d src/obu.c reads and validates each pair in the same order.
    for slot in &mut header.reference_indices {
        if !header.frame_refs_short_signaling {
            *slot = bits.bits(3)? as usize;
        }
        if sequence.frame_id_numbers_present {
            let delta = bits
                .bits(sequence.delta_frame_id_bits)?
                .checked_add(1)
                .ok_or_else(|| malformed("reference frame ID delta overflows"))?;
            let range = 1_u32
                .checked_shl(sequence.frame_id_bits)
                .ok_or_else(|| malformed("reference frame ID width is invalid"))?;
            let mask = range
                .checked_sub(1)
                .ok_or_else(|| malformed("reference frame ID range is empty"))?;
            let expected = header
                .frame_id
                .checked_add(range)
                .and_then(|value| value.checked_sub(delta))
                .ok_or_else(|| malformed("reference frame ID arithmetic overflows"))?
                & mask;
            let reference = references
                .get(*slot)
                .and_then(Option::as_ref)
                .ok_or_else(|| malformed("reference frame ID refers to an empty slot"))?;
            if reference.frame_id != expected {
                return Err(malformed("reference frame ID does not match its delta"));
            }
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
    let mut hints = [0_u32; 8];
    for (index, reference) in references.iter().enumerate() {
        let Some(reference) = reference.as_ref() else {
            return Err(malformed("short reference signaling uses an empty slot"));
        };
        hints[index] = reference.order_hint;
    }
    select_short_references(sequence.order_hint_bits, order_hint, &hints, last, golden)
}

pub(crate) fn select_short_references(
    order_hint_bits: u32,
    order_hint: u32,
    hints: &[u32; 8],
    last: usize,
    golden: usize,
) -> Av1Result<[usize; 7]> {
    if !(1..=8).contains(&order_hint_bits) || last >= hints.len() || golden >= hints.len() {
        return Err(malformed(
            "short reference signaling has invalid dimensions",
        ));
    }
    // AV1 order hints are at most eight bits. This also makes the internal
    // trace boundary reject impossible model values before array indexing.
    let range = 1_u32 << order_hint_bits;
    if order_hint >= range || hints.iter().any(|hint| *hint >= range) {
        return Err(malformed("short reference order hint exceeds its width"));
    }
    let mut result = [usize::MAX; 7];
    result[0] = last;
    result[3] = golden;
    let offsets = hints.map(|hint| relative_distance(order_hint_bits, hint, order_hint));
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
    if let Some((future, _)) = future {
        result[6] = future;
        used[future] = true;
    }
    // Without a future reference, ALTREF remains unset until the same
    // nearest-past/earliest fallback used by every other missing output.

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
                segment.features |= SEG_ALT_Q;
                segment.delta_q = bits.signed(9)?;
            }
            if bits.bit()? {
                segment.features |= SEG_ALT_LF_Y_VERTICAL;
                segment.delta_lf_y_vertical = bits.signed(7)?;
            }
            if bits.bit()? {
                segment.features |= SEG_ALT_LF_Y_HORIZONTAL;
                segment.delta_lf_y_horizontal = bits.signed(7)?;
            }
            if bits.bit()? {
                segment.features |= SEG_ALT_LF_U;
                segment.delta_lf_u = bits.signed(7)?;
            }
            if bits.bit()? {
                segment.features |= SEG_ALT_LF_V;
                segment.delta_lf_v = bits.signed(7)?;
            }
            if bits.bit()? {
                segment.features |= SEG_REFERENCE;
                segment.reference = bits.bits(3)?.cast_signed();
            }
            segment.skip = bits.bit()?;
            if segment.skip {
                segment.features |= SEG_SKIP;
            }
            segment.global_motion = bits.bit()?;
            if segment.global_motion {
                segment.features |= SEG_GLOBAL_MOTION;
            }
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
    let mut preskip = false;
    let mut last_active_id = 0_i32;
    for (index, segment) in segments.iter().enumerate() {
        if segment.features != 0 {
            last_active_id = i32::try_from(index).unwrap_or(7);
        }
        preskip |= segment.features & (SEG_REFERENCE | SEG_SKIP | SEG_GLOBAL_MOTION) != 0;
    }
    Ok(Segmentation {
        enabled,
        update_map,
        temporal,
        update_data,
        segments,
        preskip,
        last_active_id,
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
    let count =
        usize::try_from(count).map_err(|_| malformed("film-grain point count exceeds usize"))?;
    let mut points: Vec<[u32; 2]> = Vec::new();
    points
        .try_reserve_exact(count)
        .map_err(|_| malformed("unable to allocate film-grain points"))?;
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
            // Consistent sequence headers preserve film_grain_present, so
            // this reference had grain syntax available. None represents
            // apply_grain=false; reuse inherits those inactive parameters.
            // libaom 3.13.2 decodeframe.c:3912-3915,3958,5135-5137.
            return Ok(None);
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
        let ar_count = usize::try_from(ar_positions)
            .map_err(|_| malformed("film-grain AR coefficient count exceeds usize"))?;
        ar_coefficients_y
            .try_reserve_exact(ar_count)
            .map_err(|_| malformed("unable to allocate film-grain luma AR coefficients"))?;
        for _ in 0..ar_positions {
            ar_coefficients_y.push(bits.bits(8)?.cast_signed().saturating_sub(128));
        }
    }
    let mut ar_coefficients_uv: [Vec<i32>; 2] = std::array::from_fn(|_| Vec::new());
    for (plane, coefficients) in ar_coefficients_uv.iter_mut().enumerate() {
        if !uv_points[plane].is_empty() || chroma_scaling_from_luma {
            let count = ar_positions.saturating_add(u32::from(y_count != 0));
            let count = usize::try_from(count)
                .map_err(|_| malformed("film-grain AR coefficient count exceeds usize"))?;
            coefficients
                .try_reserve_exact(count)
                .map_err(|_| malformed("unable to allocate film-grain chroma AR coefficients"))?;
            for _ in 0..count {
                coefficients.push(bits.bits(8)?.cast_signed().saturating_sub(128));
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
            uv_multiplier[plane] = bits.bits(8)?.cast_signed().saturating_sub(128);
            uv_luma_multiplier[plane] = bits.bits(8)?.cast_signed().saturating_sub(128);
            uv_offset[plane] = bits.bits(9)?.cast_signed().saturating_sub(256);
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
        matrix_coefficients: sequence.matrix_coefficients,
    }))
}

#[cfg(test)]
mod presentation_tests {
    use super::*;

    // Tiny owned surfaces model retention; they are not encoded-file or
    // Pillow observations. Public pixel parity uses the full native fixtures.
    fn model_completion(priority: (u64, u32, u32), sample: u16) -> Av1Result<FrameCompletion> {
        let mut header = FrameHeader::empty(priority.2, priority.1);
        header.frame_width = 1;
        header.upscaled_width = 1;
        header.frame_height = 1;
        header.render_width = 1;
        header.render_height = 1;
        let depth = SampleDepth::new(8).ok_or_else(|| malformed("model depth"))?;
        let surface = FrameSurface::from_validated_monochrome_plane(
            super::super::block::ReconstructedPlane {
                samples: vec![sample],
            },
            &header,
            depth,
            1,
            TemporalMotionField::new(1, 1, 0, 0, [0; 7])?,
        );
        surface.validate()?;
        Ok(FrameCompletion {
            surface: Some(Arc::new(surface)),
            temporal_unit: priority.0,
            temporal_id: priority.2,
            spatial_id: priority.1,
            film_grain: None,
            show_existing: false,
            diagnostic_leaf: None,
            diagnostic_frame_dimensions: None,
        })
    }

    #[test]
    fn selected_completion_releases_superseded_and_losing_surfaces() -> Av1Result<()> {
        let mut state = FrameState::new();
        let mut lifetimes = Vec::new();
        // Each expected index is explicit: newer unit, latest exact tie,
        // higher temporal layer, higher spatial layer, then two losing frames.
        for (index, (priority, selected)) in [
            ((0, 3, 7), 0),
            ((1, 0, 0), 1),
            ((1, 0, 0), 2),
            ((1, 0, 1), 3),
            ((1, 1, 0), 4),
            ((0, 3, 7), 4),
            ((1, 0, 7), 4),
        ]
        .into_iter()
        .enumerate()
        {
            let candidate = model_completion(
                priority,
                u16::try_from(index).map_err(|_| malformed("model index"))?,
            )?;
            lifetimes.push(Arc::downgrade(
                candidate
                    .surface
                    .as_ref()
                    .ok_or_else(|| malformed("model surface"))?,
            ));
            state.retain_completion(candidate);
            for (retained_index, weak) in lifetimes.iter().enumerate() {
                assert_eq!(weak.strong_count(), usize::from(retained_index == selected));
            }
        }
        // A higher-priority capability gap must not silently fall back to an
        // older, lower-priority reconstructed display.
        let mut gap = model_completion((1, 1, 0), 7)?;
        gap.surface = None;
        state.retain_completion(gap);
        assert!(lifetimes.iter().all(|weak| weak.strong_count() == 0));
        assert!(state.selected_display()?.monochrome_plane.is_none());
        Ok(())
    }

    #[test]
    fn display_replacement_preserves_reference_ownership_and_unit_filtering() -> Av1Result<()> {
        let mut state = FrameState::new();
        let first = model_completion((0, 0, 0), 12)?;
        let surface = first
            .surface
            .as_ref()
            .ok_or_else(|| malformed("model surface"))?;
        let old_lifetime = Arc::downgrade(surface);
        state.references[0] = Some(ReferenceState {
            header: FrameHeader::empty(0, 0),
            decode: None,
            surface: Some(Arc::clone(surface)),
        });
        state.retain_completion(first);
        assert_eq!(old_lifetime.strong_count(), 2);
        assert_eq!(state.sample_flush()?, 0);
        state.retain_completion(model_completion((1, 0, 0), 34)?);
        assert_eq!(old_lifetime.strong_count(), 1);
        assert_eq!(state.sample_flush()?, 1);
        state.references[0] = None;
        assert!(old_lifetime.upgrade().is_none());

        let hidden_unit = state.sample_flush()?;
        assert_eq!(hidden_unit, 2);
        assert!(
            state
                .selected_display_for_temporal_unit_with_token(hidden_unit, None)?
                .is_none()
        );
        let display = state.selected_display()?;
        assert_eq!(
            display
                .monochrome_plane
                .ok_or_else(|| malformed("model display"))?
                .samples,
            [34]
        );

        // A failed flush preserves the last completed-frame commit. This is
        // not a rollback promise for frames completed earlier in a sample.
        state.accept_parsed_header(false, 0, false, FrameHeader::empty(0, 0))?;
        let unit_before = state.temporal_unit;
        let selected_before = state
            .selected_completion()
            .ok_or_else(|| malformed("model completion"))?
            .priority();
        assert!(matches!(
            state.sample_flush(),
            Err(CodecError::Malformed(_))
        ));
        assert_eq!(state.temporal_unit, unit_before);
        assert_eq!(
            state
                .selected_completion()
                .ok_or_else(|| malformed("model completion"))?
                .priority(),
            selected_before
        );
        state.pending = None;
        state.temporal_unit = u64::MAX;
        assert!(matches!(
            state.sample_flush(),
            Err(CodecError::Malformed(_))
        ));
        assert_eq!(state.temporal_unit, u64::MAX);
        assert_eq!(
            state
                .selected_completion()
                .ok_or_else(|| malformed("model completion"))?
                .priority(),
            selected_before
        );
        Ok(())
    }

    #[test]
    fn sequence_geometry_rejects_before_pending_frame_allocation() -> Av1Result<()> {
        let bytes =
            include_bytes!("../../../test_support/fixtures/input/images/avif/animated.avif");
        let extracted = super::super::super::samples::validated(bytes)?;
        let track = extracted
            .sequence
            .as_ref()
            .ok_or_else(|| malformed("fixture has no movie track"))?;
        let sample = track
            .color
            .samples
            .first()
            .ok_or_else(|| malformed("fixture has no first sample"))?;
        // The encoded source remains unchanged. This is an internal model of
        // a reservation made for a different inspected canvas.
        let mut state = FrameState::for_sequence(149, 150);
        assert!(matches!(
            super::super::validate_sample(bytes, sample, &mut state),
            Err(CodecError::NotImplemented(_))
        ));
        assert!(state.pending.is_none());
        assert!(state.references.iter().all(Option::is_none));
        assert!(state.completion.is_none());
        Ok(())
    }

    #[test]
    fn missing_reference_surface_is_a_gap_but_an_empty_slot_is_malformed() -> Av1Result<()> {
        let bytes =
            include_bytes!("../../../test_support/fixtures/input/images/avif/animated.avif");
        let extracted = super::super::super::samples::validated(bytes)?;
        let track = extracted
            .sequence
            .as_ref()
            .ok_or_else(|| malformed("fixture has no movie track"))?;
        let first = track
            .color
            .samples
            .first()
            .ok_or_else(|| malformed("fixture has no first sample"))?;
        let second = track
            .color
            .samples
            .get(1)
            .ok_or_else(|| malformed("fixture has no second sample"))?;
        for omit_header in [false, true] {
            let mut state = FrameState::new();
            super::super::validate_sample(bytes, first, &mut state)?;
            assert!(state.references.iter().all(|reference| {
                reference
                    .as_ref()
                    .is_some_and(|reference| reference.surface.is_some())
            }));
            // Internal capability-state mutation, not a malformed-file oracle.
            for reference in &mut state.references {
                if omit_header {
                    *reference = None;
                } else if let Some(reference) = reference {
                    reference.surface = None;
                }
            }
            let result = super::super::validate_sample(bytes, second, &mut state);
            if omit_header {
                assert!(matches!(result, Err(CodecError::Malformed(_))));
            } else {
                assert!(matches!(result, Err(CodecError::NotImplemented(_))));
            }
        }
        Ok(())
    }

    #[test]
    fn later_empty_reference_precedes_an_earlier_surface_gap() -> Av1Result<()> {
        let bytes =
            include_bytes!("../../../test_support/fixtures/input/images/avif/animated.avif");
        let extracted = super::super::super::samples::validated(bytes)?;
        let first = extracted
            .sequence
            .as_ref()
            .and_then(|track| track.color.samples.first())
            .ok_or_else(|| malformed("fixture has no first sample"))?;
        let mut state = FrameState::new();
        super::super::validate_sample(bytes, first, &mut state)?;
        let mut header = state.references[0]
            .as_ref()
            .ok_or_else(|| malformed("fixture omitted key reference"))?
            .header
            .clone();
        header.frame_type = FrameType::Inter;
        header.reference_indices = [0, 1, 2, 3, 4, 5, 6];
        header.use_ref_frame_mvs = true;
        state.references[0]
            .as_mut()
            .ok_or_else(|| malformed("key reference disappeared"))?
            .surface = None;
        state.references[6] = None;
        let sequence = state.finish()?;
        assert!(matches!(
            inter_frame_context(&header, sequence, &state.references, None),
            Err(CodecError::Malformed(_))
        ));
        assert!(matches!(
            projected_temporal_field(&header, sequence, &state.references),
            Err(CodecError::Malformed(_))
        ));
        Ok(())
    }

    #[test]
    fn empty_final_tile_is_rejected_by_the_envelope_check() -> Av1Result<()> {
        let bytes =
            include_bytes!("../../../test_support/fixtures/input/images/avif/animated.avif");
        let extracted = super::super::super::samples::validated(bytes)?;
        let first = extracted
            .sequence
            .as_ref()
            .and_then(|track| track.color.samples.first())
            .ok_or_else(|| malformed("fixture has no first sample"))?;
        let data = SegmentedData::new(bytes, &first.spans)?;
        // A defensive internal range mutation; the full source is unchanged.
        assert!(matches!(
            split_tile_payloads(&data, data.len(), data.len(), 0, 0, 0),
            Err(CodecError::Malformed(_))
        ));
        Ok(())
    }
}
