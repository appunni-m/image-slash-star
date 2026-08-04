//! VP8 first-partition syntax matching libwebp 1.6.0.

use super::{
    analysis::FrameParams,
    bool_enc::BoolEncoder,
    chroma::ChromaMode,
    frame::{LumaDecision, MacroblockDecision},
    intra4::Intra4Mode,
    intra16::Intra16Mode,
    mode_probability::INTRA4_MODE_PROBABILITIES,
    probability::AdaptedProbabilities,
    quant::libwebp_segment_matrices,
    tokenize::coefficient_update_probability,
};
use crate::codecs::CodecResult;

const PARTITION_PROBABILITY_CHECKPOINT_NODES: usize = 1_024;
const PARTITION_MODE_CHECKPOINT_MACROBLOCKS: usize = 256;
const PARTITION_8_BIT_CHECKPOINT_BITS: usize = 8;
const PARTITION_16_BIT_CHECKPOINT_BITS: usize = 16;
const PARTITION_32_BIT_CHECKPOINT_BITS: usize = 32;
const PARTITION_64_BIT_CHECKPOINT_BITS: usize = 64;
const PARTITION_FINER_BIT_CHECKPOINT_BITS: usize = 128;
const PARTITION_FINEST_BIT_CHECKPOINT_BITS: usize = 256;
const PARTITION_FINE_BIT_CHECKPOINT_BITS: usize = 512;
const PARTITION_1024_BIT_CHECKPOINT_BITS: usize = 1_024;
const PARTITION_2048_BIT_CHECKPOINT_BITS: usize = 2_048;
const PARTITION_4096_BIT_CHECKPOINT_BITS: usize = 4_096;
const PARTITION_8192_BIT_CHECKPOINT_BITS: usize = 8_192;
const PARTITION_BIT_CHECKPOINT_BITS: usize = 16_384;
const PARTITION_32768_BIT_CHECKPOINT_BITS: usize = 32_768;
const PARTITION_OUTPUT_CHECKPOINT_BYTES: usize = 1_024;

trait PartitionCheckpointControl {
    fn checkpoint_probability(&mut self) -> CodecResult<()>;
    fn checkpoint_macroblock(&mut self) -> CodecResult<()>;
    fn checkpoint_bit(&mut self) -> CodecResult<()>;
    fn checkpoint_output_bytes(&mut self, emitted: usize) -> CodecResult<()>;
    fn encode_bool(
        &mut self,
        writer: &mut BoolEncoder,
        probability: u8,
        value: bool,
    ) -> CodecResult<()>;
    fn finish(&mut self, writer: BoolEncoder) -> CodecResult<Vec<u8>>;
}

struct NoopPartitionCheckpoint;

impl PartitionCheckpointControl for NoopPartitionCheckpoint {
    #[inline(always)]
    fn checkpoint_probability(&mut self) -> CodecResult<()> {
        Ok(())
    }

    #[inline(always)]
    fn checkpoint_macroblock(&mut self) -> CodecResult<()> {
        Ok(())
    }

    #[inline(always)]
    fn checkpoint_bit(&mut self) -> CodecResult<()> {
        Ok(())
    }

    #[inline(always)]
    fn checkpoint_output_bytes(&mut self, _emitted: usize) -> CodecResult<()> {
        Ok(())
    }

    #[inline(always)]
    fn encode_bool(
        &mut self,
        writer: &mut BoolEncoder,
        probability: u8,
        value: bool,
    ) -> CodecResult<()> {
        writer.encode_bool(probability, value);
        Ok(())
    }

    #[inline(always)]
    fn finish(&mut self, writer: BoolEncoder) -> CodecResult<Vec<u8>> {
        Ok(writer.finish())
    }
}

struct TokenPartitionCheckpoint<'a> {
    token: &'a crate::CancellationToken,
    probability_items: usize,
    macroblock_items: usize,
    bit_items: usize,
    output_bytes: usize,
}

impl PartitionCheckpointControl for TokenPartitionCheckpoint<'_> {
    #[inline]
    fn checkpoint_probability(&mut self) -> CodecResult<()> {
        self.probability_items = self.probability_items.saturating_add(1);
        if self
            .probability_items
            .is_multiple_of(PARTITION_PROBABILITY_CHECKPOINT_NODES)
        {
            crate::codecs::error::check_cancelled(Some(self.token))?;
        }
        Ok(())
    }

    #[inline]
    fn checkpoint_macroblock(&mut self) -> CodecResult<()> {
        self.macroblock_items = self.macroblock_items.saturating_add(1);
        if self
            .macroblock_items
            .is_multiple_of(PARTITION_MODE_CHECKPOINT_MACROBLOCKS)
        {
            crate::codecs::error::check_cancelled(Some(self.token))?;
        }
        Ok(())
    }

    #[inline]
    fn checkpoint_bit(&mut self) -> CodecResult<()> {
        // Every logical interval counts the same boolean operations. Keep one
        // counter and nest the larger intervals under the 8-bit poll so the
        // token-aware path does not perform redundant modulo tests per bit.
        self.bit_items = self.bit_items.saturating_add(1);
        if self
            .bit_items
            .is_multiple_of(PARTITION_8_BIT_CHECKPOINT_BITS)
        {
            crate::codecs::error::check_cancelled(Some(self.token))?;
            if self
                .bit_items
                .is_multiple_of(PARTITION_16_BIT_CHECKPOINT_BITS)
            {
                crate::codecs::error::check_cancelled(Some(self.token))?;
                if self
                    .bit_items
                    .is_multiple_of(PARTITION_32_BIT_CHECKPOINT_BITS)
                {
                    crate::codecs::error::check_cancelled(Some(self.token))?;
                    if self
                        .bit_items
                        .is_multiple_of(PARTITION_64_BIT_CHECKPOINT_BITS)
                    {
                        crate::codecs::error::check_cancelled(Some(self.token))?;
                        if self
                            .bit_items
                            .is_multiple_of(PARTITION_FINER_BIT_CHECKPOINT_BITS)
                        {
                            crate::codecs::error::check_cancelled(Some(self.token))?;
                            if self
                                .bit_items
                                .is_multiple_of(PARTITION_FINEST_BIT_CHECKPOINT_BITS)
                            {
                                crate::codecs::error::check_cancelled(Some(self.token))?;
                                if self
                                    .bit_items
                                    .is_multiple_of(PARTITION_FINE_BIT_CHECKPOINT_BITS)
                                {
                                    crate::codecs::error::check_cancelled(Some(self.token))?;
                                    if self
                                        .bit_items
                                        .is_multiple_of(PARTITION_1024_BIT_CHECKPOINT_BITS)
                                    {
                                        crate::codecs::error::check_cancelled(Some(self.token))?;
                                        if self
                                            .bit_items
                                            .is_multiple_of(PARTITION_2048_BIT_CHECKPOINT_BITS)
                                        {
                                            crate::codecs::error::check_cancelled(Some(
                                                self.token,
                                            ))?;
                                            if self
                                                .bit_items
                                                .is_multiple_of(PARTITION_4096_BIT_CHECKPOINT_BITS)
                                            {
                                                crate::codecs::error::check_cancelled(Some(
                                                    self.token,
                                                ))?;
                                                if self.bit_items.is_multiple_of(
                                                    PARTITION_8192_BIT_CHECKPOINT_BITS,
                                                ) {
                                                    crate::codecs::error::check_cancelled(Some(
                                                        self.token,
                                                    ))?;
                                                    if self.bit_items.is_multiple_of(
                                                        PARTITION_BIT_CHECKPOINT_BITS,
                                                    ) {
                                                        crate::codecs::error::check_cancelled(
                                                            Some(self.token),
                                                        )?;
                                                        if self.bit_items.is_multiple_of(
                                                            PARTITION_32768_BIT_CHECKPOINT_BITS,
                                                        ) {
                                                            crate::codecs::error::check_cancelled(
                                                                Some(self.token),
                                                            )?;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn checkpoint_output_bytes(&mut self, emitted: usize) -> CodecResult<()> {
        let previous = self.output_bytes;
        self.output_bytes = self.output_bytes.saturating_add(emitted);
        let mut previous_interval = previous / PARTITION_OUTPUT_CHECKPOINT_BYTES;
        let current_interval = self.output_bytes / PARTITION_OUTPUT_CHECKPOINT_BYTES;
        while previous_interval < current_interval {
            previous_interval = previous_interval.saturating_add(1);
            crate::codecs::error::check_cancelled(Some(self.token))?;
        }
        Ok(())
    }

    #[inline]
    fn encode_bool(
        &mut self,
        writer: &mut BoolEncoder,
        probability: u8,
        value: bool,
    ) -> CodecResult<()> {
        writer.encode_bool_with_checkpoint(probability, value, &mut |emitted| {
            self.checkpoint_output_bytes(emitted)
        })?;
        self.checkpoint_bit()
    }

    #[inline]
    fn finish(&mut self, writer: BoolEncoder) -> CodecResult<Vec<u8>> {
        writer.finish_with_checkpoint(|emitted| self.checkpoint_output_bytes(emitted))
    }
}

#[inline]
fn encode_bool<P: PartitionCheckpointControl>(
    writer: &mut BoolEncoder,
    probability: u8,
    value: bool,
    checkpoint: &mut P,
) -> CodecResult<()> {
    checkpoint.encode_bool(writer, probability, value)
}

#[inline]
fn encode_literal<P: PartitionCheckpointControl>(
    writer: &mut BoolEncoder,
    value: u32,
    bits: u8,
    checkpoint: &mut P,
) -> CodecResult<()> {
    let n = bits.min(32);
    for i in (0..n).rev() {
        let bit = (value.wrapping_shr(u32::from(i)) & 1) != 0;
        encode_bool(writer, 128, bit, checkpoint)?;
    }
    Ok(())
}

fn write_signed<P: PartitionCheckpointControl>(
    writer: &mut BoolEncoder,
    value: i32,
    magnitude_bits: u8,
    checkpoint: &mut P,
) -> CodecResult<()> {
    encode_bool(writer, 128, value != 0, checkpoint)?;
    if value != 0 {
        let magnitude_and_sign = value.unsigned_abs().wrapping_shl(1) | u32::from(value < 0);
        encode_literal(
            writer,
            magnitude_and_sign,
            magnitude_bits.saturating_add(1),
            checkpoint,
        )?;
    }
    Ok(())
}

fn segment_probability(zero: usize, one: usize) -> u8 {
    let total = zero.saturating_add(one);
    let Some(total) = std::num::NonZeroUsize::new(total) else {
        return 255;
    };
    255usize
        .saturating_mul(zero)
        .saturating_add(total.get().div_euclid(2))
        .div_euclid(total.get())
        .to_le_bytes()[0]
}

fn segment_probabilities(decisions: &[MacroblockDecision]) -> [u8; 3] {
    let mut counts = [0usize; 4];
    for decision in decisions {
        counts[usize::from(decision.segment)] =
            counts[usize::from(decision.segment)].saturating_add(1);
    }
    [
        segment_probability(
            counts[0].saturating_add(counts[1]),
            counts[2].saturating_add(counts[3]),
        ),
        segment_probability(counts[0], counts[1]),
        segment_probability(counts[2], counts[3]),
    ]
}

fn adjusted_frame_params(
    decisions: &[MacroblockDecision],
    params: &FrameParams,
    adjust_filter_edges: bool,
) -> FrameParams {
    let mut adjusted = FrameParams {
        segments: params.segments,
        num_segments: params.num_segments,
        chroma_dc_delta: params.chroma_dc_delta,
        chroma_ac_delta: params.chroma_ac_delta,
    };
    if !adjust_filter_edges {
        return adjusted;
    }
    let mut maximum_edges = [0u16; 4];
    for decision in decisions {
        let LumaDecision::Intra16(luma) = &decision.luma else {
            continue;
        };
        let segment = usize::from(decision.segment);
        let matrices = libwebp_segment_matrices(
            params.segments[segment].quantizer,
            params.chroma_dc_delta,
            params.chroma_ac_delta,
        );
        let only_y2_nonzero = luma.nonzero & 0x0100_ffff == 0x0100_0000;
        let minimum_distortion = 20u32.saturating_mul(u32::from(matrices.y1.q[0]));
        if only_y2_nonzero && luma.distortion > minimum_distortion {
            let edge = luma.y2_levels[1]
                .unsigned_abs()
                .max(luma.y2_levels[2].unsigned_abs())
                .max(luma.y2_levels[4].unsigned_abs());
            maximum_edges[segment] = maximum_edges[segment].max(edge);
        }
    }
    for (segment, &maximum_edge) in maximum_edges.iter().enumerate() {
        let matrices = libwebp_segment_matrices(
            params.segments[segment].quantizer,
            params.chroma_dc_delta,
            params.chroma_ac_delta,
        );
        let delta = u32::from(maximum_edge)
            .saturating_mul(u32::from(matrices.y2.q[1]))
            .wrapping_shr(3);
        adjusted.segments[segment].filter_strength = adjusted.segments[segment]
            .filter_strength
            .max(delta.min(63).to_le_bytes()[0]);
    }
    adjusted
}

fn write_segment_header<P: PartitionCheckpointControl>(
    writer: &mut BoolEncoder,
    params: &FrameParams,
    probabilities: [u8; 3],
    checkpoint: &mut P,
) -> CodecResult<()> {
    let segmentation_enabled = params.num_segments > 1;
    encode_bool(writer, 128, segmentation_enabled, checkpoint)?;
    if !segmentation_enabled {
        return Ok(());
    }
    encode_bool(writer, 128, true, checkpoint)?; // update map
    encode_bool(writer, 128, true, checkpoint)?; // update data
    encode_bool(writer, 128, true, checkpoint)?; // absolute feature values
    for segment in params.segments {
        write_signed(writer, i32::from(segment.quantizer), 7, checkpoint)?;
    }
    for segment in params.segments {
        write_signed(writer, i32::from(segment.filter_strength), 6, checkpoint)?;
    }
    for probability in probabilities {
        let update = probability != 255;
        encode_bool(writer, 128, update, checkpoint)?;
        if update {
            encode_literal(writer, u32::from(probability), 8, checkpoint)?;
        }
    }
    Ok(())
}

fn write_coefficient_probabilities<P: PartitionCheckpointControl>(
    writer: &mut BoolEncoder,
    probabilities: &AdaptedProbabilities,
    checkpoint: &mut P,
) -> CodecResult<()> {
    let mut probability_items = 0usize;
    for coefficient_type in 0..4 {
        for band in 0..8 {
            for context in 0..3 {
                for node in 0..11 {
                    let update = probabilities.updates[coefficient_type][band][context][node];
                    encode_bool(
                        writer,
                        coefficient_update_probability(coefficient_type, band, context, node),
                        update,
                        checkpoint,
                    )?;
                    if update {
                        encode_literal(
                            writer,
                            u32::from(
                                probabilities.coefficients[coefficient_type][band][context][node],
                            ),
                            8,
                            checkpoint,
                        )?;
                    }
                    probability_items = probability_items.saturating_add(1);
                    checkpoint.checkpoint_probability()?;
                }
            }
        }
    }
    encode_bool(writer, 128, false, checkpoint)?; // no skip probability
    Ok(())
}

fn write_segment<P: PartitionCheckpointControl>(
    writer: &mut BoolEncoder,
    segment: u8,
    probabilities: [u8; 3],
    checkpoint: &mut P,
) -> CodecResult<()> {
    let upper_half = segment >= 2;
    encode_bool(writer, probabilities[0], upper_half, checkpoint)?;
    encode_bool(
        writer,
        probabilities[if upper_half { 2 } else { 1 }],
        segment & 1 != 0,
        checkpoint,
    )
}

fn write_intra4_mode<P: PartitionCheckpointControl>(
    writer: &mut BoolEncoder,
    mode: Intra4Mode,
    probabilities: &[u8; 9],
    checkpoint: &mut P,
) -> CodecResult<()> {
    let mode = mode as u8;
    if writer_bit(writer, probabilities[0], mode != 0, checkpoint)?
        && writer_bit(writer, probabilities[1], mode != 1, checkpoint)?
        && writer_bit(writer, probabilities[2], mode != 2, checkpoint)?
    {
        if !writer_bit(writer, probabilities[3], mode >= 6, checkpoint)? {
            if writer_bit(writer, probabilities[4], mode != 3, checkpoint)? {
                writer_bit(writer, probabilities[5], mode != 4, checkpoint)?;
            }
        } else if writer_bit(writer, probabilities[6], mode != 6, checkpoint)?
            && writer_bit(writer, probabilities[7], mode != 7, checkpoint)?
        {
            writer_bit(writer, probabilities[8], mode != 8, checkpoint)?;
        }
    }
    Ok(())
}

fn writer_bit<P: PartitionCheckpointControl>(
    writer: &mut BoolEncoder,
    probability: u8,
    bit: bool,
    checkpoint: &mut P,
) -> CodecResult<bool> {
    encode_bool(writer, probability, bit, checkpoint)?;
    Ok(bit)
}

fn write_intra16_mode<P: PartitionCheckpointControl>(
    writer: &mut BoolEncoder,
    mode: Intra16Mode,
    checkpoint: &mut P,
) -> CodecResult<()> {
    let horizontal_or_true_motion =
        matches!(mode, Intra16Mode::TrueMotion | Intra16Mode::Horizontal);
    encode_bool(writer, 156, horizontal_or_true_motion, checkpoint)?;
    if horizontal_or_true_motion {
        encode_bool(writer, 128, mode == Intra16Mode::TrueMotion, checkpoint)?;
    } else {
        encode_bool(writer, 163, mode == Intra16Mode::Vertical, checkpoint)?;
    }
    Ok(())
}

fn write_chroma_mode<P: PartitionCheckpointControl>(
    writer: &mut BoolEncoder,
    mode: ChromaMode,
    checkpoint: &mut P,
) -> CodecResult<()> {
    if writer_bit(writer, 142, mode != ChromaMode::Dc, checkpoint)?
        && writer_bit(writer, 114, mode != ChromaMode::Vertical, checkpoint)?
    {
        writer_bit(writer, 183, mode != ChromaMode::Horizontal, checkpoint)?;
    }
    Ok(())
}

fn intra16_as_intra4(mode: Intra16Mode) -> Intra4Mode {
    match mode {
        Intra16Mode::Dc => Intra4Mode::Dc,
        Intra16Mode::TrueMotion => Intra4Mode::TrueMotion,
        Intra16Mode::Vertical => Intra4Mode::Vertical,
        Intra16Mode::Horizontal => Intra4Mode::Horizontal,
    }
}

fn write_modes<P: PartitionCheckpointControl>(
    writer: &mut BoolEncoder,
    decisions: &[MacroblockDecision],
    macroblock_width: usize,
    segment_probabilities: [u8; 3],
    segmentation_enabled: bool,
    checkpoint: &mut P,
) -> CodecResult<()> {
    let mode_stride = macroblock_width.saturating_mul(4);
    let macroblock_height = decisions.len().div_euclid(macroblock_width);
    let mut modes = vec![
        Intra4Mode::Dc;
        mode_stride
            .saturating_mul(macroblock_height)
            .saturating_mul(4)
    ];
    let mut macroblock_items = 0usize;
    for decision in decisions {
        if segmentation_enabled {
            write_segment(writer, decision.segment, segment_probabilities, checkpoint)?;
        }
        let is_intra16 = matches!(decision.luma, LumaDecision::Intra16(_));
        encode_bool(writer, 145, is_intra16, checkpoint)?;
        match &decision.luma {
            LumaDecision::Intra16(luma) => write_intra16_mode(writer, luma.mode, checkpoint)?,
            LumaDecision::Intra4(luma) => {
                for block_y in 0..4 {
                    for block_x in 0..4 {
                        let grid_x = decision.x.saturating_mul(4).saturating_add(block_x);
                        let grid_y = decision.y.saturating_mul(4).saturating_add(block_y);
                        let top = if grid_y == 0 {
                            Intra4Mode::Dc
                        } else {
                            modes[grid_y
                                .saturating_sub(1)
                                .saturating_mul(mode_stride)
                                .saturating_add(grid_x)]
                        };
                        let left = if grid_x == 0 {
                            Intra4Mode::Dc
                        } else {
                            modes[grid_y
                                .saturating_mul(mode_stride)
                                .saturating_add(grid_x)
                                .saturating_sub(1)]
                        };
                        let mode = luma.modes[block_y.saturating_mul(4).saturating_add(block_x)];
                        write_intra4_mode(
                            writer,
                            mode,
                            &INTRA4_MODE_PROBABILITIES[top as usize][left as usize],
                            checkpoint,
                        )?;
                        modes[grid_y.saturating_mul(mode_stride).saturating_add(grid_x)] = mode;
                    }
                }
            }
        }
        if let LumaDecision::Intra16(luma) = &decision.luma {
            let mode = intra16_as_intra4(luma.mode);
            for block_y in 0..4 {
                for block_x in 0..4 {
                    let grid_y = decision.y.saturating_mul(4).saturating_add(block_y);
                    let grid_x = decision.x.saturating_mul(4).saturating_add(block_x);
                    modes[grid_y.saturating_mul(mode_stride).saturating_add(grid_x)] = mode;
                }
            }
        }
        write_chroma_mode(writer, decision.chroma.mode, checkpoint)?;
        macroblock_items = macroblock_items.saturating_add(1);
        checkpoint.checkpoint_macroblock()?;
    }
    Ok(())
}

fn encode_first_partition_with_checkpoint<P: PartitionCheckpointControl>(
    decisions: &[MacroblockDecision],
    macroblock_width: usize,
    params: &FrameParams,
    probabilities: &AdaptedProbabilities,
    adjust_filter_edges: bool,
    checkpoint: &mut P,
) -> CodecResult<Vec<u8>> {
    let params = adjusted_frame_params(decisions, params, adjust_filter_edges);
    let segment_probabilities = segment_probabilities(decisions);
    let mut writer = BoolEncoder::default();
    encode_bool(&mut writer, 128, false, checkpoint)?; // colorspace
    encode_bool(&mut writer, 128, false, checkpoint)?; // clamp type
    write_segment_header(&mut writer, &params, segment_probabilities, checkpoint)?;
    encode_bool(&mut writer, 128, false, checkpoint)?; // strong loop filter
    let filter_level = params
        .segments
        .iter()
        .fold(0, |maximum, segment| maximum.max(segment.filter_strength));
    encode_literal(&mut writer, u32::from(filter_level), 6, checkpoint)?;
    encode_literal(&mut writer, 0, 3, checkpoint)?; // sharpness
    encode_bool(&mut writer, 128, false, checkpoint)?; // no loop-filter deltas
    encode_literal(&mut writer, 0, 2, checkpoint)?; // one coefficient partition
    encode_literal(
        &mut writer,
        u32::from(params.segments[0].quantizer),
        7,
        checkpoint,
    )?;
    write_signed(&mut writer, 0, 4, checkpoint)?; // Y1 DC
    write_signed(&mut writer, 0, 4, checkpoint)?; // Y2 DC
    write_signed(&mut writer, 0, 4, checkpoint)?; // Y2 AC
    write_signed(
        &mut writer,
        i32::from(params.chroma_dc_delta),
        4,
        checkpoint,
    )?;
    write_signed(
        &mut writer,
        i32::from(params.chroma_ac_delta),
        4,
        checkpoint,
    )?;
    encode_bool(&mut writer, 128, false, checkpoint)?; // no entropy refresh
    write_coefficient_probabilities(&mut writer, probabilities, checkpoint)?;
    write_modes(
        &mut writer,
        decisions,
        macroblock_width,
        segment_probabilities,
        params.num_segments > 1,
        checkpoint,
    )?;
    checkpoint.finish(writer)
}

pub(super) fn encode_first_partition(
    decisions: &[MacroblockDecision],
    macroblock_width: usize,
    params: &FrameParams,
    probabilities: &AdaptedProbabilities,
    adjust_filter_edges: bool,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    if let Some(token) = token {
        let mut checkpoint = TokenPartitionCheckpoint {
            token,
            probability_items: 0,
            macroblock_items: 0,
            bit_items: 0,
            output_bytes: 0,
        };
        encode_first_partition_with_checkpoint(
            decisions,
            macroblock_width,
            params,
            probabilities,
            adjust_filter_edges,
            &mut checkpoint,
        )
    } else {
        let mut checkpoint = NoopPartitionCheckpoint;
        encode_first_partition_with_checkpoint(
            decisions,
            macroblock_width,
            params,
            probabilities,
            adjust_filter_edges,
            &mut checkpoint,
        )
    }
}
