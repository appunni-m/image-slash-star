//! Macroblock susceptibility analysis used by libwebp's lossy VP8 encoder.

use super::dct::vp8_fdct_4x4;
use super::quant::Y_AC_QUANT;
use crate::codecs::CodecResult;

const MAX_ALPHA: usize = 255;
const NUM_SEGMENTS: usize = 4;
const MAX_K_MEANS_ITERATIONS: usize = 6;
const MAX_COEFFICIENT_THRESHOLD: usize = 31;
const ANALYSIS_CHECKPOINT_MACROBLOCKS: usize = 1_024;
const ANALYSIS_HISTOGRAM_CHECKPOINT_BLOCKS: usize = 64;
const SEGMENT_ASSIGNMENT_CHECKPOINT_MACROBLOCKS: usize = 1_024;
const SEGMENT_CLUSTER_CHECKPOINT_ALPHA_VALUES: usize = 64;

trait AnalysisCheckpointControl {
    fn checkpoint_analysis_macroblock(&mut self) -> CodecResult<()>;
    fn checkpoint_analysis_histogram_block(&mut self) -> CodecResult<()>;
    fn checkpoint_segment_cluster(&mut self) -> CodecResult<()>;
    fn checkpoint_segment_assignment(&mut self) -> CodecResult<()>;
}

struct NoopAnalysisCheckpoint {
    #[cfg(coverage)]
    fail_after: usize,
}

impl NoopAnalysisCheckpoint {
    fn new() -> Self {
        Self {
            #[cfg(coverage)]
            fail_after: usize::MAX,
        }
    }

    #[inline(always)]
    fn event(&mut self) -> CodecResult<()> {
        #[cfg(coverage)]
        {
            if self.fail_after == 0 {
                return Err(crate::codecs::CodecError::Cancelled);
            }
            self.fail_after = self.fail_after.saturating_sub(1);
        }
        Ok(())
    }
}

impl AnalysisCheckpointControl for NoopAnalysisCheckpoint {
    #[inline(always)]
    fn checkpoint_analysis_macroblock(&mut self) -> CodecResult<()> {
        self.event()
    }

    #[inline(always)]
    fn checkpoint_analysis_histogram_block(&mut self) -> CodecResult<()> {
        self.event()
    }

    #[inline(always)]
    fn checkpoint_segment_cluster(&mut self) -> CodecResult<()> {
        self.event()
    }

    #[inline(always)]
    fn checkpoint_segment_assignment(&mut self) -> CodecResult<()> {
        self.event()
    }
}

#[cfg(coverage)]
struct CoverageFailingSegmentCheckpoint {
    calls: usize,
    fail_after: usize,
    fail_assignment: bool,
}

#[cfg(coverage)]
#[coverage(off)]
impl AnalysisCheckpointControl for CoverageFailingSegmentCheckpoint {
    fn checkpoint_analysis_macroblock(&mut self) -> CodecResult<()> {
        Ok(())
    }

    fn checkpoint_analysis_histogram_block(&mut self) -> CodecResult<()> {
        Ok(())
    }

    fn checkpoint_segment_cluster(&mut self) -> CodecResult<()> {
        if self.calls >= self.fail_after {
            return Err(crate::codecs::CodecError::Cancelled);
        }
        self.calls = self.calls.saturating_add(1);
        Ok(())
    }

    fn checkpoint_segment_assignment(&mut self) -> CodecResult<()> {
        if self.fail_assignment {
            Err(crate::codecs::CodecError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[cfg(coverage)]
struct CoverageFailingSegmentClusterCheckpoint {
    successful_calls: usize,
    fail_after: usize,
    fail_assignment: bool,
}

#[cfg(coverage)]
#[coverage(off)]
impl AnalysisCheckpointControl for CoverageFailingSegmentClusterCheckpoint {
    fn checkpoint_analysis_macroblock(&mut self) -> CodecResult<()> {
        Ok(())
    }

    fn checkpoint_analysis_histogram_block(&mut self) -> CodecResult<()> {
        Ok(())
    }

    fn checkpoint_segment_cluster(&mut self) -> CodecResult<()> {
        if self.successful_calls >= self.fail_after {
            return Err(crate::codecs::CodecError::Cancelled);
        }
        self.successful_calls = self.successful_calls.saturating_add(1);
        Ok(())
    }

    fn checkpoint_segment_assignment(&mut self) -> CodecResult<()> {
        if self.fail_assignment {
            Err(crate::codecs::CodecError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[cfg(coverage)]
struct CoverageFailingSegmentAssignmentCheckpoint {
    fail: bool,
    histogram_fail_after: usize,
    fail_cluster: bool,
    fail_macroblock: bool,
}

#[cfg(coverage)]
#[coverage(off)]
impl CoverageFailingSegmentAssignmentCheckpoint {
    fn new() -> Self {
        Self {
            fail: std::hint::black_box(true),
            histogram_fail_after: usize::MAX,
            fail_cluster: false,
            fail_macroblock: false,
        }
    }

    fn successful() -> Self {
        Self {
            fail: std::hint::black_box(false),
            histogram_fail_after: usize::MAX,
            fail_cluster: false,
            fail_macroblock: false,
        }
    }

    fn histogram_failure() -> Self {
        Self {
            fail: false,
            histogram_fail_after: std::hint::black_box(0),
            fail_cluster: false,
            fail_macroblock: false,
        }
    }

    fn chroma_histogram_failure() -> Self {
        Self {
            fail: false,
            histogram_fail_after: std::hint::black_box(32),
            fail_cluster: false,
            fail_macroblock: false,
        }
    }

    fn cluster_failure() -> Self {
        Self {
            fail: false,
            histogram_fail_after: usize::MAX,
            fail_cluster: true,
            fail_macroblock: false,
        }
    }

    fn macroblock_failure() -> Self {
        Self {
            fail: false,
            histogram_fail_after: usize::MAX,
            fail_cluster: false,
            fail_macroblock: true,
        }
    }
}

#[cfg(coverage)]
#[coverage(off)]
impl AnalysisCheckpointControl for CoverageFailingSegmentAssignmentCheckpoint {
    fn checkpoint_analysis_macroblock(&mut self) -> CodecResult<()> {
        if self.fail_macroblock {
            Err(crate::codecs::CodecError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn checkpoint_analysis_histogram_block(&mut self) -> CodecResult<()> {
        if self.histogram_fail_after == 0 {
            Err(crate::codecs::CodecError::Cancelled)
        } else {
            self.histogram_fail_after = self.histogram_fail_after.saturating_sub(1);
            Ok(())
        }
    }

    fn checkpoint_segment_cluster(&mut self) -> CodecResult<()> {
        if self.fail_cluster {
            Err(crate::codecs::CodecError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn checkpoint_segment_assignment(&mut self) -> CodecResult<()> {
        if self.fail {
            Err(crate::codecs::CodecError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[cfg(coverage)]
struct CoverageFailingMacroblockCheckpoint {
    calls: usize,
    fail_after: usize,
    fail_histogram: bool,
    histogram_fail_after: usize,
    fail_cluster: bool,
    fail_assignment: bool,
}

#[cfg(coverage)]
#[coverage(off)]
impl AnalysisCheckpointControl for CoverageFailingMacroblockCheckpoint {
    fn checkpoint_analysis_macroblock(&mut self) -> CodecResult<()> {
        if self.calls >= self.fail_after {
            return Err(crate::codecs::CodecError::Cancelled);
        }
        self.calls = self.calls.saturating_add(1);
        Ok(())
    }

    fn checkpoint_analysis_histogram_block(&mut self) -> CodecResult<()> {
        if self.fail_histogram || self.histogram_fail_after == 0 {
            return Err(crate::codecs::CodecError::Cancelled);
        }
        self.histogram_fail_after = self.histogram_fail_after.saturating_sub(1);
        Ok(())
    }

    fn checkpoint_segment_cluster(&mut self) -> CodecResult<()> {
        if self.fail_cluster {
            Err(crate::codecs::CodecError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn checkpoint_segment_assignment(&mut self) -> CodecResult<()> {
        if self.fail_assignment {
            Err(crate::codecs::CodecError::Cancelled)
        } else {
            Ok(())
        }
    }
}

struct TokenAnalysisCheckpoint<'a> {
    token: &'a crate::CancellationToken,
    analysis_items: usize,
    analysis_histogram_blocks: usize,
    segment_assignment_items: usize,
}

impl AnalysisCheckpointControl for TokenAnalysisCheckpoint<'_> {
    #[inline]
    fn checkpoint_analysis_macroblock(&mut self) -> CodecResult<()> {
        self.analysis_items = self.analysis_items.saturating_add(1);
        if self
            .analysis_items
            .is_multiple_of(ANALYSIS_CHECKPOINT_MACROBLOCKS)
        {
            crate::codecs::error::check_cancelled(Some(self.token))?;
        }
        Ok(())
    }

    #[inline]
    fn checkpoint_analysis_histogram_block(&mut self) -> CodecResult<()> {
        self.analysis_histogram_blocks = self.analysis_histogram_blocks.saturating_add(1);
        if self
            .analysis_histogram_blocks
            .is_multiple_of(ANALYSIS_HISTOGRAM_CHECKPOINT_BLOCKS)
        {
            crate::codecs::error::check_cancelled(Some(self.token))?;
        }
        Ok(())
    }

    #[inline]
    fn checkpoint_segment_cluster(&mut self) -> CodecResult<()> {
        crate::codecs::error::check_cancelled(Some(self.token))
    }

    #[inline]
    fn checkpoint_segment_assignment(&mut self) -> CodecResult<()> {
        self.segment_assignment_items = self.segment_assignment_items.saturating_add(1);
        if self
            .segment_assignment_items
            .is_multiple_of(SEGMENT_ASSIGNMENT_CHECKPOINT_MACROBLOCKS)
        {
            crate::codecs::error::check_cancelled(Some(self.token))?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct MacroblockAnalysis {
    pub(super) alpha: u8,
    pub(super) segment: u8,
    pub(super) use_intra4: bool,
    pub(super) luma_mode: u8,
    pub(super) chroma_mode: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SegmentAnalysis {
    pub(super) alpha: i32,
    pub(super) beta: i32,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct FrameAnalysis {
    pub(super) alpha: i32,
    pub(super) chroma_alpha: i32,
    pub(super) macroblocks: Vec<MacroblockAnalysis>,
    pub(super) segments: [SegmentAnalysis; NUM_SEGMENTS],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SegmentParams {
    pub(super) quantizer: u8,
    pub(super) filter_strength: u8,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct FrameParams {
    pub(super) segments: [SegmentParams; NUM_SEGMENTS],
    pub(super) num_segments: usize,
    pub(super) chroma_dc_delta: i8,
    pub(super) chroma_ac_delta: i8,
}

#[derive(Clone, Copy)]
struct Histogram {
    max_value: i32,
    last_non_zero: i32,
}

fn predict_block<const SIZE: usize>(
    top: Option<&[u8]>,
    left: Option<&[u8]>,
    top_left: u8,
    mode: u8,
    output: &mut [u8],
) {
    debug_assert_eq!(output.len(), SIZE.wrapping_mul(SIZE));
    let size_u32 = u32::from(SIZE.to_le_bytes()[0]);
    let denominator = size_u32.wrapping_mul(2);
    match mode {
        0 => {
            let value = match (top, left) {
                (Some(top), Some(left)) => {
                    let sum: u32 = top
                        .iter()
                        .take(SIZE)
                        .chain(left.iter().take(SIZE))
                        .fold(0_u32, |sum, &value| sum.wrapping_add(u32::from(value)));
                    sum.wrapping_add(size_u32)
                        .checked_div(denominator)
                        .unwrap_or_default()
                        .to_le_bytes()[0]
                }
                (Some(top), None) => {
                    let sum = top
                        .iter()
                        .take(SIZE)
                        .fold(0_u32, |sum, &value| sum.wrapping_add(u32::from(value)));
                    sum.wrapping_mul(2)
                        .wrapping_add(size_u32)
                        .checked_div(denominator)
                        .unwrap_or_default()
                        .to_le_bytes()[0]
                }
                (None, Some(left)) => {
                    let sum = left
                        .iter()
                        .take(SIZE)
                        .fold(0_u32, |sum, &value| sum.wrapping_add(u32::from(value)));
                    sum.wrapping_mul(2)
                        .wrapping_add(size_u32)
                        .checked_div(denominator)
                        .unwrap_or_default()
                        .to_le_bytes()[0]
                }
                (None, None) => 128,
            };
            output.fill(value);
        }
        _ => {
            debug_assert_eq!(mode, 1);
            match (top, left) {
                (Some(top), Some(left)) => {
                    for row in 0..SIZE {
                        for column in 0..SIZE {
                            output[row.wrapping_mul(SIZE).wrapping_add(column)] =
                                i16::from(top[column])
                                    .wrapping_add(i16::from(left[row]))
                                    .wrapping_sub(i16::from(top_left))
                                    .clamp(0, 255)
                                    .to_le_bytes()[0];
                        }
                    }
                }
                (Some(top), None) => {
                    for row in output.as_chunks_mut::<SIZE>().0 {
                        row.copy_from_slice(&top[..SIZE]);
                    }
                }
                (None, Some(left)) => {
                    for (row, &value) in
                        output.as_chunks_mut::<SIZE>().0.iter_mut().zip(left.iter())
                    {
                        row.fill(value);
                    }
                }
                (None, None) => output.fill(129),
            }
        }
    }
}

fn collect_histogram<C: AnalysisCheckpointControl>(
    blocks: &[(&[u8], &[u8], usize)],
    checkpoint: &mut C,
) -> CodecResult<Histogram> {
    let mut distribution = [0i32; MAX_COEFFICIENT_THRESHOLD + 1];
    for &(source, prediction, stride) in blocks {
        for block_y in 0..stride / 4 {
            for block_x in 0..stride / 4 {
                let mut residual = [0i16; 16];
                for row in 0_usize..4 {
                    for column in 0_usize..4 {
                        let index = block_y
                            .wrapping_mul(4)
                            .wrapping_add(row)
                            .wrapping_mul(stride)
                            .wrapping_add(block_x.wrapping_mul(4))
                            .wrapping_add(column);
                        residual[row.wrapping_mul(4).wrapping_add(column)] =
                            i16::from(source[index]).wrapping_sub(i16::from(prediction[index]));
                    }
                }
                for coefficient in vp8_fdct_4x4(&residual) {
                    let bin = (usize::from(coefficient.unsigned_abs()) >> 3)
                        .min(MAX_COEFFICIENT_THRESHOLD);
                    distribution[bin] = distribution[bin].wrapping_add(1);
                }
                checkpoint.checkpoint_analysis_histogram_block()?;
            }
        }
    }

    let mut histogram = Histogram {
        max_value: 0,
        last_non_zero: 1,
    };
    for (bin, &count) in distribution.iter().enumerate() {
        if count > 0 {
            histogram.max_value = histogram.max_value.max(count);
            histogram.last_non_zero = i32::from(bin.to_le_bytes()[0]);
        }
    }
    Ok(histogram)
}

fn histogram_alpha(histogram: Histogram) -> i32 {
    debug_assert!(histogram.max_value > 1);
    510_i32
        .wrapping_mul(histogram.last_non_zero)
        .checked_div(histogram.max_value)
        .unwrap_or_default()
}

#[derive(Clone, Copy)]
struct BlockRegion {
    stride: usize,
    width: usize,
    height: usize,
    origin_x: usize,
    origin_y: usize,
    size: usize,
}

impl BlockRegion {
    fn new(
        stride: usize,
        width: usize,
        height: usize,
        origin_x: usize,
        origin_y: usize,
        size: usize,
    ) -> Self {
        Self {
            stride,
            width,
            height,
            origin_x,
            origin_y,
            size,
        }
    }
}

fn extract_block_into(plane: &[u8], region: BlockRegion, output: &mut [u8]) {
    debug_assert_eq!(output.len(), region.size.wrapping_mul(region.size));
    for row in 0..region.size {
        let source_y = region
            .origin_y
            .wrapping_add(row)
            .min(region.height.saturating_sub(1));
        for column in 0..region.size {
            let source_x = region
                .origin_x
                .wrapping_add(column)
                .min(region.width.saturating_sub(1));
            output[row.wrapping_mul(region.size).wrapping_add(column)] =
                plane[source_y.wrapping_mul(region.stride).wrapping_add(source_x)];
        }
    }
}

fn fill_boundary(
    plane: &[u8],
    region: BlockRegion,
    top: &mut [u8],
    left: &mut [u8],
) -> (bool, bool, u8) {
    debug_assert!(top.len() >= region.size);
    debug_assert!(left.len() >= region.size);
    let has_top = region.origin_y > 0;
    if has_top {
        for (column, value) in top.iter_mut().take(region.size).enumerate() {
            *value = plane[region
                .origin_y
                .wrapping_sub(1)
                .wrapping_mul(region.stride)
                .wrapping_add(
                    region
                        .origin_x
                        .wrapping_add(column)
                        .min(region.width.saturating_sub(1)),
                )];
        }
    }
    let has_left = region.origin_x > 0;
    if has_left {
        for (row, value) in left.iter_mut().take(region.size).enumerate() {
            *value = plane[region
                .origin_y
                .wrapping_add(row)
                .min(region.height.saturating_sub(1))
                .wrapping_mul(region.stride)
                .wrapping_add(region.origin_x)
                .wrapping_sub(1)];
        }
    }
    let top_left = if region.origin_x > 0 && region.origin_y > 0 {
        plane[region
            .origin_y
            .wrapping_sub(1)
            .wrapping_mul(region.stride)
            .wrapping_add(region.origin_x)
            .wrapping_sub(1)]
    } else if region.origin_y > 0 {
        129
    } else {
        127
    };
    (has_top, has_left, top_left)
}

fn assign_segments<C: AnalysisCheckpointControl>(
    macroblocks: &mut [MacroblockAnalysis],
    alpha_counts: &[i32; MAX_ALPHA + 1],
    checkpoint: &mut C,
) -> CodecResult<[SegmentAnalysis; NUM_SEGMENTS]> {
    let minimum = alpha_counts
        .iter()
        .position(|&count| count != 0)
        .unwrap_or(0);
    let maximum = alpha_counts
        .iter()
        .rposition(|&count| count != 0)
        .unwrap_or(minimum);
    let range = maximum.saturating_sub(minimum);
    let mut centers = [0i32; NUM_SEGMENTS];
    for (index, center) in centers.iter_mut().enumerate() {
        let numerator = index.wrapping_mul(2).wrapping_add(1).wrapping_mul(range);
        let denominator = NUM_SEGMENTS.wrapping_mul(2);
        let offset = numerator.checked_div(denominator).unwrap_or_default();
        *center =
            i32::from(minimum.to_le_bytes()[0]).wrapping_add(i32::from(offset.to_le_bytes()[0]));
    }

    let mut map = [0u8; MAX_ALPHA + 1];
    let mut weighted_average = 0_i32;
    for _ in 0..MAX_K_MEANS_ITERATIONS {
        let mut accumulations = [0i32; NUM_SEGMENTS];
        let mut distance_accumulations = [0i32; NUM_SEGMENTS];
        let mut nearest = 0_usize;
        let mut scanned = 0_usize;
        for alpha in minimum..=maximum {
            let count = alpha_counts[alpha];
            if count != 0 {
                let alpha_i32 = i32::from(alpha.to_le_bytes()[0]);
                while nearest.wrapping_add(1) < NUM_SEGMENTS
                    && alpha_i32
                        .wrapping_sub(centers[nearest.wrapping_add(1)])
                        .abs()
                        < alpha_i32.wrapping_sub(centers[nearest]).abs()
                {
                    nearest = nearest.wrapping_add(1);
                }
                map[alpha] = nearest.to_le_bytes()[0];
                distance_accumulations[nearest] =
                    distance_accumulations[nearest].wrapping_add(alpha_i32.wrapping_mul(count));
                accumulations[nearest] = accumulations[nearest].wrapping_add(count);
            }
            scanned = scanned.wrapping_add(1);
            if scanned.is_multiple_of(SEGMENT_CLUSTER_CHECKPOINT_ALPHA_VALUES) {
                checkpoint.checkpoint_segment_cluster()?;
            }
        }
        if !scanned.is_multiple_of(SEGMENT_CLUSTER_CHECKPOINT_ALPHA_VALUES) {
            match checkpoint.checkpoint_segment_cluster() {
                Ok(()) => {}
                Err(error) => return Err(error),
            }
        }

        let mut displaced = 0_i32;
        let mut weighted_sum = 0_i32;
        let mut total_weight = 0_i32;
        for index in 0..NUM_SEGMENTS {
            if accumulations[index] != 0 {
                let center = distance_accumulations[index]
                    .wrapping_add(accumulations[index] / 2)
                    .checked_div(accumulations[index])
                    .unwrap_or_default();
                displaced = displaced.wrapping_add(centers[index].wrapping_sub(center).abs());
                centers[index] = center;
                weighted_sum = weighted_sum.wrapping_add(center.wrapping_mul(accumulations[index]));
                total_weight = total_weight.wrapping_add(accumulations[index]);
            }
        }
        weighted_average = weighted_sum
            .wrapping_add(total_weight / 2)
            .checked_div(total_weight)
            .unwrap_or_default();
        if displaced < 5 {
            break;
        }
    }

    for macroblock in macroblocks {
        let segment = map[macroblock.alpha as usize];
        macroblock.segment = segment;
        macroblock.alpha = centers[usize::from(segment)].to_le_bytes()[0];
        checkpoint.checkpoint_segment_assignment()?;
    }

    let minimum_center = *centers.iter().min().unwrap_or(&0);
    let mut maximum_center = *centers.iter().max().unwrap_or(&0);
    if maximum_center == minimum_center {
        maximum_center = minimum_center.wrapping_add(1);
    }
    let center_range = maximum_center.wrapping_sub(minimum_center);
    Ok(std::array::from_fn(|index| {
        let alpha = 255_i32
            .wrapping_mul(centers[index].wrapping_sub(weighted_average))
            .checked_div(center_range)
            .unwrap_or_default()
            .clamp(-127, 127);
        let beta = 255_i32
            .wrapping_mul(centers[index].wrapping_sub(minimum_center))
            .checked_div(center_range)
            .unwrap_or_default()
            .clamp(0, 255);
        SegmentAnalysis { alpha, beta }
    }))
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let mut alpha_counts = [0i32; MAX_ALPHA + 1];
    alpha_counts[0] = 1;
    alpha_counts[10] = 1;
    let mut checkpoint = CoverageFailingSegmentCheckpoint {
        calls: 0,
        fail_after: std::hint::black_box(0),
        fail_assignment: false,
    };
    let mut macroblocks = Vec::new();
    let _ = assign_segments(&mut macroblocks, &alpha_counts, &mut checkpoint);
    macroblocks.push(MacroblockAnalysis {
        alpha: 10,
        ..MacroblockAnalysis::default()
    });
    let mut assignment_checkpoint = CoverageFailingSegmentCheckpoint {
        calls: 0,
        fail_after: std::hint::black_box(0),
        fail_assignment: false,
    };
    let _ = assign_segments(&mut macroblocks, &alpha_counts, &mut assignment_checkpoint);
    let mut assignment_error_checkpoint = CoverageFailingSegmentAssignmentCheckpoint::new();
    let _ = assign_segments(
        &mut macroblocks,
        &alpha_counts,
        &mut assignment_error_checkpoint,
    );
    let mut successful_assignment_checkpoint =
        CoverageFailingSegmentAssignmentCheckpoint::successful();
    let mut successful_assignment_macroblocks = vec![
        MacroblockAnalysis {
            alpha: 0,
            ..MacroblockAnalysis::default()
        },
        MacroblockAnalysis {
            alpha: 10,
            ..MacroblockAnalysis::default()
        },
    ];
    let _ = std::hint::black_box(assign_segments(
        &mut successful_assignment_macroblocks,
        &alpha_counts,
        &mut successful_assignment_checkpoint,
    ));
    let mut successful_macroblock_checkpoint = CoverageFailingMacroblockCheckpoint {
        calls: 0,
        fail_after: std::hint::black_box(usize::MAX),
        fail_histogram: false,
        histogram_fail_after: usize::MAX,
        fail_cluster: false,
        fail_assignment: false,
    };
    let mut successful_macroblock_list = successful_assignment_macroblocks.clone();
    let _ = std::hint::black_box(assign_segments(
        &mut successful_macroblock_list,
        &alpha_counts,
        &mut successful_macroblock_checkpoint,
    ));
    let mut successful_checkpoint = CoverageFailingSegmentCheckpoint {
        calls: 0,
        fail_after: std::hint::black_box(usize::MAX),
        fail_assignment: false,
    };
    let _ = assign_segments(&mut macroblocks, &alpha_counts, &mut successful_checkpoint);

    let mut single_alpha_counts = [0i32; MAX_ALPHA + 1];
    single_alpha_counts[42] = 1;
    let mut single_macroblocks = vec![MacroblockAnalysis {
        alpha: 42,
        ..MacroblockAnalysis::default()
    }];
    let mut checkpoint = CoverageFailingSegmentCheckpoint {
        calls: 0,
        fail_after: std::hint::black_box(usize::MAX),
        fail_assignment: false,
    };
    let _ = assign_segments(
        &mut single_macroblocks,
        &single_alpha_counts,
        &mut checkpoint,
    );
    let mut checkpoint = CoverageFailingSegmentClusterCheckpoint {
        successful_calls: 0,
        fail_after: usize::MAX,
        fail_assignment: false,
    };
    let _ = assign_segments(
        &mut single_macroblocks,
        &single_alpha_counts,
        &mut checkpoint,
    );
    let mut checkpoint = NoopAnalysisCheckpoint {
        fail_after: usize::MAX,
    };
    let _ = assign_segments(
        &mut single_macroblocks,
        &single_alpha_counts,
        &mut checkpoint,
    );

    // One successful 64-value cluster checkpoint followed by a non-aligned
    // tail reaches the second checkpoint branch independently of the first
    // branch's early-return edge.
    let mut full_alpha_counts = [0i32; MAX_ALPHA + 1];
    full_alpha_counts[..=64].fill(1);
    let mut checkpoint = CoverageFailingSegmentClusterCheckpoint {
        successful_calls: 0,
        fail_after: 1,
        fail_assignment: false,
    };
    let _ = assign_segments(&mut macroblocks, &full_alpha_counts, &mut checkpoint);
    let mut checkpoint = CoverageFailingSegmentClusterCheckpoint {
        successful_calls: 0,
        fail_after: 0,
        fail_assignment: false,
    };
    let _ = assign_segments(&mut macroblocks, &full_alpha_counts, &mut checkpoint);
    let mut checkpoint = CoverageFailingSegmentClusterCheckpoint {
        successful_calls: 0,
        fail_after: usize::MAX,
        fail_assignment: false,
    };
    let _ = assign_segments(&mut macroblocks, &full_alpha_counts, &mut checkpoint);
    let mut exact_alpha_counts = [0i32; MAX_ALPHA + 1];
    exact_alpha_counts[..SEGMENT_CLUSTER_CHECKPOINT_ALPHA_VALUES].fill(1);
    // The assignment-failure checkpoint is normally used with a populated
    // macroblock list, so it returns before the final center-range guard.
    // Empty valid lists let that same checkpoint type reach both outcomes of
    // the guard and the aligned cluster checkpoint.
    for counts in [&single_alpha_counts, &exact_alpha_counts] {
        let mut empty_macroblocks = Vec::new();
        let mut checkpoint = CoverageFailingSegmentAssignmentCheckpoint::new();
        let _ = assign_segments(&mut empty_macroblocks, counts, &mut checkpoint);
    }
    let mut checkpoint = CoverageFailingSegmentClusterCheckpoint {
        successful_calls: 0,
        fail_after: usize::MAX,
        fail_assignment: false,
    };
    let _ = std::hint::black_box(assign_segments(
        &mut macroblocks,
        &exact_alpha_counts,
        &mut checkpoint,
    ));

    let mut sparse_alpha_counts = [0i32; MAX_ALPHA + 1];
    sparse_alpha_counts[0] = 1;
    sparse_alpha_counts[MAX_ALPHA] = 1;
    let mut empty_macroblocks = Vec::new();
    let mut checkpoint = CoverageFailingSegmentClusterCheckpoint {
        successful_calls: 0,
        fail_after: usize::MAX,
        fail_assignment: false,
    };
    let _ = std::hint::black_box(assign_segments(
        &mut empty_macroblocks,
        &sparse_alpha_counts,
        &mut checkpoint,
    ));
    let mut empty_macroblocks = Vec::new();
    let mut checkpoint = CoverageFailingMacroblockCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
        fail_histogram: false,
        histogram_fail_after: usize::MAX,
        fail_cluster: false,
        fail_assignment: false,
    };
    let _ = std::hint::black_box(assign_segments(
        &mut empty_macroblocks,
        &sparse_alpha_counts,
        &mut checkpoint,
    ));
    let mut empty_macroblocks = Vec::new();
    let mut checkpoint = CoverageFailingSegmentAssignmentCheckpoint::successful();
    let _ = std::hint::black_box(assign_segments(
        &mut empty_macroblocks,
        &sparse_alpha_counts,
        &mut checkpoint,
    ));

    for counts in [&single_alpha_counts, &exact_alpha_counts] {
        let mut empty_macroblocks = Vec::new();
        let mut checkpoint = CoverageFailingMacroblockCheckpoint {
            calls: 0,
            fail_after: usize::MAX,
            fail_histogram: false,
            histogram_fail_after: usize::MAX,
            fail_cluster: true,
            fail_assignment: false,
        };
        let _ = assign_segments(&mut empty_macroblocks, counts, &mut checkpoint);
        let mut empty_macroblocks = Vec::new();
        let mut checkpoint = CoverageFailingSegmentAssignmentCheckpoint::cluster_failure();
        let _ = assign_segments(&mut empty_macroblocks, counts, &mut checkpoint);
    }
    let mut assignment_error_checkpoint = CoverageFailingSegmentCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
        fail_assignment: true,
    };
    let mut assignment_macroblocks = single_macroblocks.clone();
    let _ = assign_segments(
        &mut assignment_macroblocks,
        &single_alpha_counts,
        &mut assignment_error_checkpoint,
    );
    let mut assignment_error_checkpoint = CoverageFailingSegmentClusterCheckpoint {
        successful_calls: 0,
        fail_after: usize::MAX,
        fail_assignment: true,
    };
    let mut assignment_macroblocks = single_macroblocks.clone();
    let _ = assign_segments(
        &mut assignment_macroblocks,
        &single_alpha_counts,
        &mut assignment_error_checkpoint,
    );
    let assignment_token = crate::CancellationToken::new();
    assignment_token.cancel_after(1);
    let mut assignment_token_checkpoint = TokenAnalysisCheckpoint {
        token: &assignment_token,
        analysis_items: 0,
        analysis_histogram_blocks: 0,
        segment_assignment_items: SEGMENT_ASSIGNMENT_CHECKPOINT_MACROBLOCKS - 1,
    };
    let mut assignment_macroblocks = single_macroblocks.clone();
    let _ = assign_segments(
        &mut assignment_macroblocks,
        &single_alpha_counts,
        &mut assignment_token_checkpoint,
    );

    // A 65-value range is the smallest input that takes the aligned
    // checkpoint inside the loop and then the non-aligned tail checkpoint.
    // Exercise that shape for each checkpoint specialization used by the
    // encoder; the generic branches are counted independently by LLVM.
    let mut segment_checkpoint = CoverageFailingSegmentCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
        fail_assignment: false,
    };
    let mut empty_macroblocks = Vec::new();
    let _ = std::hint::black_box(assign_segments(
        &mut empty_macroblocks,
        &exact_alpha_counts,
        &mut segment_checkpoint,
    ));
    let mut noop_checkpoint = NoopAnalysisCheckpoint {
        fail_after: usize::MAX,
    };
    let mut empty_macroblocks = Vec::new();
    let _ = std::hint::black_box(assign_segments(
        &mut empty_macroblocks,
        &exact_alpha_counts,
        &mut noop_checkpoint,
    ));
    let token = crate::CancellationToken::new();
    let mut token_checkpoint = TokenAnalysisCheckpoint {
        token: &token,
        analysis_items: 0,
        analysis_histogram_blocks: 0,
        segment_assignment_items: 0,
    };
    let mut empty_macroblocks = Vec::new();
    let _ = std::hint::black_box(assign_segments(
        &mut empty_macroblocks,
        &exact_alpha_counts,
        &mut token_checkpoint,
    ));

    let y_plane = [0_u8; 16 * 16];
    let chroma_plane = [0_u8; 8 * 8];
    let mut histogram_checkpoint = NoopAnalysisCheckpoint { fail_after: 2 };
    let _ = analyze_with_checkpoint(
        [&y_plane, &chroma_plane, &chroma_plane],
        (16, 16),
        50,
        2,
        &mut histogram_checkpoint,
    );
    let mut successful_checkpoint = NoopAnalysisCheckpoint::new();
    let _ = std::hint::black_box(analyze_with_checkpoint(
        [&y_plane, &chroma_plane, &chroma_plane],
        (16, 16),
        50,
        2,
        &mut successful_checkpoint,
    ));
    let completed_events = usize::MAX.saturating_sub(successful_checkpoint.fail_after);
    let mut assignment_failure = NoopAnalysisCheckpoint {
        fail_after: completed_events.saturating_sub(1),
    };
    let _ = std::hint::black_box(analyze_with_checkpoint(
        [&y_plane, &chroma_plane, &chroma_plane],
        (16, 16),
        50,
        2,
        &mut assignment_failure,
    ));

    for fail_after in [0, 1, 2, 64, 256] {
        let mut checkpoint = NoopAnalysisCheckpoint { fail_after };
        let mut modeled_macroblocks = vec![MacroblockAnalysis {
            alpha: 10,
            ..MacroblockAnalysis::default()
        }];
        let _ = assign_segments(
            &mut modeled_macroblocks,
            &full_alpha_counts,
            &mut checkpoint,
        );
    }

    for fail_after in [0, usize::MAX] {
        let mut checkpoint = CoverageFailingMacroblockCheckpoint {
            calls: 0,
            fail_after,
            fail_histogram: false,
            histogram_fail_after: usize::MAX,
            fail_cluster: false,
            fail_assignment: false,
        };
        let _ = std::hint::black_box(analyze_with_checkpoint(
            [&[0_u8; 16 * 16], &[0_u8; 8 * 8], &[0_u8; 8 * 8]],
            (16, 16),
            50,
            2,
            &mut checkpoint,
        ));
    }
    let mut histogram_checkpoint = CoverageFailingMacroblockCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
        fail_histogram: true,
        histogram_fail_after: usize::MAX,
        fail_cluster: false,
        fail_assignment: false,
    };
    let _ = std::hint::black_box(analyze_with_checkpoint(
        [&[0_u8; 32 * 32], &[0_u8; 16 * 16], &[0_u8; 16 * 16]],
        (32, 32),
        50,
        2,
        &mut histogram_checkpoint,
    ));
    let mut macroblock_assignment_failure = CoverageFailingMacroblockCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
        fail_histogram: false,
        histogram_fail_after: usize::MAX,
        fail_cluster: false,
        fail_assignment: std::hint::black_box(true),
    };
    let _ = std::hint::black_box(analyze_with_checkpoint(
        [&[0_u8; 16 * 16], &[0_u8; 8 * 8], &[0_u8; 8 * 8]],
        (16, 16),
        50,
        2,
        &mut macroblock_assignment_failure,
    ));
    let mut assignment_failure = CoverageFailingSegmentAssignmentCheckpoint::new();
    let _ = std::hint::black_box(analyze_with_checkpoint(
        [&[0_u8; 16 * 16], &[0_u8; 8 * 8], &[0_u8; 8 * 8]],
        (16, 16),
        50,
        2,
        &mut assignment_failure,
    ));
    let mut histogram_failure = CoverageFailingSegmentAssignmentCheckpoint::histogram_failure();
    let _ = std::hint::black_box(analyze_with_checkpoint(
        [&[0_u8; 16 * 16], &[0_u8; 8 * 8], &[0_u8; 8 * 8]],
        (16, 16),
        50,
        2,
        &mut histogram_failure,
    ));
    let mut chroma_histogram_failure =
        CoverageFailingSegmentAssignmentCheckpoint::chroma_histogram_failure();
    let _ = std::hint::black_box(analyze_with_checkpoint(
        [&[0_u8; 16 * 16], &[0_u8; 8 * 8], &[0_u8; 8 * 8]],
        (16, 16),
        50,
        2,
        &mut chroma_histogram_failure,
    ));
    let mut noop_chroma_histogram_failure = NoopAnalysisCheckpoint { fail_after: 32 };
    let _ = std::hint::black_box(analyze_with_checkpoint(
        [&[0_u8; 16 * 16], &[0_u8; 8 * 8], &[0_u8; 8 * 8]],
        (16, 16),
        50,
        2,
        &mut noop_chroma_histogram_failure,
    ));
    let mut macroblock_chroma_histogram_failure = CoverageFailingMacroblockCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
        fail_histogram: false,
        histogram_fail_after: 32,
        fail_cluster: false,
        fail_assignment: false,
    };
    let _ = std::hint::black_box(analyze_with_checkpoint(
        [&[0_u8; 16 * 16], &[0_u8; 8 * 8], &[0_u8; 8 * 8]],
        (16, 16),
        50,
        2,
        &mut macroblock_chroma_histogram_failure,
    ));
    let mut noop_macroblock_failure = NoopAnalysisCheckpoint { fail_after: 48 };
    let _ = std::hint::black_box(analyze_with_checkpoint(
        [&[0_u8; 16 * 16], &[0_u8; 8 * 8], &[0_u8; 8 * 8]],
        (16, 16),
        50,
        2,
        &mut noop_macroblock_failure,
    ));
    let macroblock_token = crate::CancellationToken::new();
    macroblock_token.cancel_after(0);
    let mut macroblock_token_checkpoint = TokenAnalysisCheckpoint {
        token: &macroblock_token,
        analysis_items: ANALYSIS_CHECKPOINT_MACROBLOCKS - 1,
        analysis_histogram_blocks: 0,
        segment_assignment_items: 0,
    };
    let _ = std::hint::black_box(analyze_with_checkpoint(
        [&[0_u8; 16 * 16], &[0_u8; 8 * 8], &[0_u8; 8 * 8]],
        (16, 16),
        50,
        2,
        &mut macroblock_token_checkpoint,
    ));
    let mut segment_assignment_macroblock_failure =
        CoverageFailingSegmentAssignmentCheckpoint::macroblock_failure();
    let _ = std::hint::black_box(analyze_with_checkpoint(
        [&[0_u8; 16 * 16], &[0_u8; 8 * 8], &[0_u8; 8 * 8]],
        (16, 16),
        50,
        2,
        &mut segment_assignment_macroblock_failure,
    ));
    let mut method_one_macroblock_checkpoint = CoverageFailingMacroblockCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
        fail_histogram: false,
        histogram_fail_after: usize::MAX,
        fail_cluster: false,
        fail_assignment: false,
    };
    let _ = std::hint::black_box(analyze_with_checkpoint(
        [&[0_u8; 16 * 16], &[0_u8; 8 * 8], &[0_u8; 8 * 8]],
        (16, 16),
        50,
        1,
        &mut method_one_macroblock_checkpoint,
    ));
    let mut method_one_assignment_checkpoint =
        CoverageFailingSegmentAssignmentCheckpoint::successful();
    let _ = std::hint::black_box(analyze_with_checkpoint(
        [&[0_u8; 16 * 16], &[0_u8; 8 * 8], &[0_u8; 8 * 8]],
        (16, 16),
        50,
        1,
        &mut method_one_assignment_checkpoint,
    ));
    let mut assignment_success = CoverageFailingSegmentAssignmentCheckpoint::successful();
    let _ = std::hint::black_box(analyze_with_checkpoint(
        [&[0_u8; 16 * 16], &[0_u8; 8 * 8], &[0_u8; 8 * 8]],
        (16, 16),
        50,
        2,
        &mut assignment_success,
    ));
    let cancelled = crate::CancellationToken::new();
    cancelled.cancel();
    let mut checkpoint = TokenAnalysisCheckpoint {
        token: &cancelled,
        analysis_items: 0,
        analysis_histogram_blocks: 0,
        segment_assignment_items: ANALYSIS_CHECKPOINT_MACROBLOCKS - 1,
    };
    let mut macroblocks = vec![MacroblockAnalysis {
        alpha: 42,
        ..MacroblockAnalysis::default()
    }];
    let mut alpha_counts = [0_i32; MAX_ALPHA + 1];
    alpha_counts[42] = 1;
    let _ = assign_segments(&mut macroblocks, &alpha_counts, &mut checkpoint);

    let mut checkpoint = TokenAnalysisCheckpoint {
        token: &cancelled,
        analysis_items: ANALYSIS_CHECKPOINT_MACROBLOCKS - 1,
        analysis_histogram_blocks: 0,
        segment_assignment_items: 0,
    };
    let _ = checkpoint.checkpoint_analysis_macroblock();
    let mut checkpoint = TokenAnalysisCheckpoint {
        token: &cancelled,
        analysis_items: 0,
        analysis_histogram_blocks: 0,
        segment_assignment_items: SEGMENT_ASSIGNMENT_CHECKPOINT_MACROBLOCKS - 1,
    };
    let _ = checkpoint.checkpoint_segment_assignment();
}

pub(super) fn analyze(
    planes: [&[u8]; 3],
    dimensions: (usize, usize),
    quality: u8,
    method: u8,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<FrameAnalysis> {
    if let Some(token) = token {
        let mut checkpoint = TokenAnalysisCheckpoint {
            token,
            analysis_items: 0,
            analysis_histogram_blocks: 0,
            segment_assignment_items: 0,
        };
        analyze_with_checkpoint(planes, dimensions, quality, method, &mut checkpoint)
    } else {
        let mut checkpoint = NoopAnalysisCheckpoint::new();
        analyze_with_checkpoint(planes, dimensions, quality, method, &mut checkpoint)
    }
}

fn analyze_with_checkpoint<C: AnalysisCheckpointControl>(
    planes: [&[u8]; 3],
    dimensions: (usize, usize),
    quality: u8,
    method: u8,
    checkpoint: &mut C,
) -> CodecResult<FrameAnalysis> {
    let [y_plane, u_plane, v_plane] = planes;
    let (width, height) = dimensions;
    let chroma_width = width.div_ceil(2);
    let chroma_height = height.div_ceil(2);
    let macroblock_width = width.div_ceil(16);
    let macroblock_height = height.div_ceil(16);
    let mut macroblocks = Vec::with_capacity(macroblock_width.wrapping_mul(macroblock_height));
    let mut alpha_counts = [0i32; MAX_ALPHA + 1];
    let mut alpha_sum = 0_i32;
    let mut chroma_alpha_sum = 0_i32;
    let mut luma_prediction = [0_u8; 16 * 16];
    let mut u_prediction = [0_u8; 8 * 8];
    let mut v_prediction = [0_u8; 8 * 8];
    let mut y_block = [0_u8; 16 * 16];
    let mut u_block = [0_u8; 8 * 8];
    let mut v_block = [0_u8; 8 * 8];
    let mut y_top_buffer = [0_u8; 16];
    let mut y_left_buffer = [0_u8; 16];
    let mut u_top_buffer = [0_u8; 8];
    let mut u_left_buffer = [0_u8; 8];
    let mut v_top_buffer = [0_u8; 8];
    let mut v_left_buffer = [0_u8; 8];
    for macroblock_y in 0..macroblock_height {
        for macroblock_x in 0..macroblock_width {
            let y_x = macroblock_x.wrapping_mul(16);
            let y_y = macroblock_y.wrapping_mul(16);
            let y_region = BlockRegion::new(width, width, height, y_x, y_y, 16);
            extract_block_into(y_plane, y_region, &mut y_block);
            let (has_y_top, has_y_left, y_top_left) =
                fill_boundary(y_plane, y_region, &mut y_top_buffer, &mut y_left_buffer);
            let y_top = has_y_top.then_some(&y_top_buffer[..16]);
            let y_left = has_y_left.then_some(&y_left_buffer[..16]);

            let (best_luma_alpha, luma_mode, use_intra4) = if method <= 1 {
                let strip_sums = std::array::from_fn::<_, 16, _>(|strip| {
                    let block_x = strip % 4;
                    let block_y = strip / 4;
                    (0..4)
                        .flat_map(|row| {
                            let offset = block_y
                                .wrapping_mul(4)
                                .wrapping_add(row)
                                .wrapping_mul(16)
                                .wrapping_add(block_x.wrapping_mul(4));
                            &y_block[offset..][..4]
                        })
                        .map(|&value| u32::from(value))
                        .sum::<u32>()
                });
                let mean = strip_sums.iter().sum::<u32>();
                let squared_mean = strip_sums.iter().fold(0_u32, |sum, &value| {
                    sum.wrapping_add(value.wrapping_mul(value))
                });
                let threshold =
                    8_u32.wrapping_add(9_u32.wrapping_mul(u32::from(quality)).wrapping_div(100));
                (
                    0,
                    0,
                    threshold.wrapping_mul(squared_mean) >= mean.wrapping_mul(mean),
                )
            } else {
                let mut best_luma_alpha = -1;
                let mut luma_mode = 0;
                for mode in 0..2 {
                    predict_block::<16>(y_top, y_left, y_top_left, mode, &mut luma_prediction);
                    let histogram =
                        collect_histogram(&[(&y_block, &luma_prediction, 16)], checkpoint)?;
                    let alpha = histogram_alpha(histogram);
                    if alpha > best_luma_alpha {
                        best_luma_alpha = alpha;
                        luma_mode = mode;
                    }
                }
                (best_luma_alpha, luma_mode, false)
            };

            let uv_x = macroblock_x.wrapping_mul(8);
            let uv_y = macroblock_y.wrapping_mul(8);
            let uv_region =
                BlockRegion::new(chroma_width, chroma_width, chroma_height, uv_x, uv_y, 8);
            extract_block_into(u_plane, uv_region, &mut u_block);
            extract_block_into(v_plane, uv_region, &mut v_block);
            let (has_u_top, has_u_left, u_top_left) =
                fill_boundary(u_plane, uv_region, &mut u_top_buffer, &mut u_left_buffer);
            let (has_v_top, has_v_left, v_top_left) =
                fill_boundary(v_plane, uv_region, &mut v_top_buffer, &mut v_left_buffer);
            let u_top = has_u_top.then_some(&u_top_buffer[..8]);
            let u_left = has_u_left.then_some(&u_left_buffer[..8]);
            let v_top = has_v_top.then_some(&v_top_buffer[..8]);
            let v_left = has_v_left.then_some(&v_left_buffer[..8]);
            let mut best_chroma_alpha = -1;
            let mut smallest_chroma_alpha = 0;
            let mut chroma_mode = 0;
            for mode in 0..2 {
                predict_block::<8>(u_top, u_left, u_top_left, mode, &mut u_prediction);
                predict_block::<8>(v_top, v_left, v_top_left, mode, &mut v_prediction);
                let histogram = collect_histogram(
                    &[(&u_block, &u_prediction, 8), (&v_block, &v_prediction, 8)],
                    checkpoint,
                )?;
                let alpha = histogram_alpha(histogram);
                best_chroma_alpha = best_chroma_alpha.max(alpha);
                if mode == 0 || alpha < smallest_chroma_alpha {
                    smallest_chroma_alpha = alpha;
                    chroma_mode = mode;
                }
            }

            let mixed_alpha = 255_i32
                .wrapping_sub(
                    3_i32
                        .wrapping_mul(best_luma_alpha)
                        .wrapping_add(best_chroma_alpha)
                        .wrapping_add(2)
                        .wrapping_shr(2),
                )
                .clamp(0, 255);
            let mixed_alpha_index = usize::from(mixed_alpha.to_le_bytes()[0]);
            alpha_counts[mixed_alpha_index] = alpha_counts[mixed_alpha_index].wrapping_add(1);
            alpha_sum = alpha_sum.wrapping_add(mixed_alpha);
            chroma_alpha_sum = chroma_alpha_sum.wrapping_add(best_chroma_alpha);
            macroblocks.push(MacroblockAnalysis {
                alpha: mixed_alpha.to_le_bytes()[0],
                segment: 0,
                use_intra4,
                luma_mode,
                chroma_mode,
            });
            checkpoint.checkpoint_analysis_macroblock()?;
        }
    }

    let count_bytes = macroblocks.len().to_le_bytes();
    let macroblock_count = i32::from_le_bytes([
        count_bytes[0],
        count_bytes[1],
        count_bytes[2],
        count_bytes[3],
    ]);
    let alpha = alpha_sum.checked_div(macroblock_count).unwrap_or_default();
    let chroma_alpha = chroma_alpha_sum
        .checked_div(macroblock_count)
        .unwrap_or_default();
    let segments = assign_segments(&mut macroblocks, &alpha_counts, checkpoint)?;
    Ok(FrameAnalysis {
        alpha,
        chroma_alpha,
        macroblocks,
        segments,
    })
}

/// Converts the bounded libwebp quantizer expression with Rust's truncation
/// semantics after the floating-point quality transform.
#[allow(clippy::cast_possible_truncation)]
fn trunc_quantizer(value: f64) -> i32 {
    value as i32
}

pub(super) fn segment_params(analysis: &FrameAnalysis, quality: f64) -> FrameParams {
    let compression = {
        let quality = quality / 100.0;
        let linear = if quality < 0.75 {
            quality * (2.0 / 3.0)
        } else {
            2.0 * quality - 1.0
        };
        linear.powf(1.0 / 3.0)
    };
    let segments = std::array::from_fn(|index| {
        let exponent = 1.0 - (0.9 * 50.0 / 100.0 / 128.0) * analysis.segments[index].alpha as f64;
        let quantizer = trunc_quantizer(127.0 * (1.0 - compression.powf(exponent)));
        let quantizer = quantizer.clamp(0, 127).to_le_bytes()[0];
        let quantizer_step = Y_AC_QUANT[quantizer as usize] >> 2;
        let strength = i32::from(quantizer_step)
            .wrapping_mul(300)
            .checked_div(256_i32.wrapping_add(analysis.segments[index].beta))
            .unwrap_or_default();
        let filter_strength = if strength < 2 {
            0
        } else {
            strength.min(63).to_le_bytes()[0]
        };
        SegmentParams {
            quantizer,
            filter_strength,
        }
    });
    let chroma_ac_value = analysis
        .chroma_alpha
        .wrapping_sub(64)
        .wrapping_mul(10)
        .wrapping_div(70)
        .wrapping_mul(50)
        .wrapping_div(100)
        .clamp(-4, 6);
    let chroma_ac_delta = i8::from_le_bytes([chroma_ac_value.to_le_bytes()[0]]);
    let chroma_dc_value = (-4_i32).wrapping_mul(50).wrapping_div(100).clamp(-15, 15);
    let chroma_dc_delta = i8::from_le_bytes([chroma_dc_value.to_le_bytes()[0]]);
    FrameParams {
        segments,
        num_segments: NUM_SEGMENTS,
        chroma_dc_delta,
        chroma_ac_delta,
    }
}
