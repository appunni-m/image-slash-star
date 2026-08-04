//! Coefficient partition coding matching libwebp 1.6.0.

use super::{
    bool_enc::BoolEncoder,
    frame::{LumaDecision, MacroblockDecision},
    probability::AdaptedProbabilities,
    tokenize::COEFF_BANDS,
};
use crate::codecs::CodecResult;

const CAT3_PROBABILITIES: [u8; 3] = [173, 148, 140];
const CAT4_PROBABILITIES: [u8; 4] = [176, 155, 140, 135];
const CAT5_PROBABILITIES: [u8; 5] = [180, 157, 141, 134, 130];
const CAT6_PROBABILITIES: [u8; 11] = [254, 254, 243, 230, 196, 177, 153, 140, 133, 130, 129];
const COEFFICIENT_CHECKPOINT_TOKENS: usize = 4_000;
const COEFFICIENT_CHECKPOINT_BLOCKS: usize = 64;
const COEFFICIENT_CHECKPOINT_MACROBLOCKS: usize = 256;
const COEFFICIENT_8_BIT_CHECKPOINT_BITS: usize = 8;
const COEFFICIENT_16_BIT_CHECKPOINT_BITS: usize = 16;
const COEFFICIENT_32_BIT_CHECKPOINT_BITS: usize = 32;
const COEFFICIENT_64_BIT_CHECKPOINT_BITS: usize = 64;
const COEFFICIENT_FINER_CHECKPOINT_BITS: usize = 128;
const COEFFICIENT_FINEST_CHECKPOINT_BITS: usize = 256;
const COEFFICIENT_FINE_CHECKPOINT_BITS: usize = 512;
const COEFFICIENT_1024_CHECKPOINT_BITS: usize = 1_024;
const COEFFICIENT_2048_CHECKPOINT_BITS: usize = 2_048;
const COEFFICIENT_4096_CHECKPOINT_BITS: usize = 4_096;
const COEFFICIENT_8192_CHECKPOINT_BITS: usize = 8_192;
const COEFFICIENT_CHECKPOINT_BITS: usize = 16_384;
const COEFFICIENT_OUTPUT_CHECKPOINT_BYTES: usize = 1_024;

trait CoefficientCheckpointControl {
    fn checkpoint_token(&mut self) -> CodecResult<()>;
    fn checkpoint_block(&mut self) -> CodecResult<()>;
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

struct NoopCoefficientCheckpoint;

impl CoefficientCheckpointControl for NoopCoefficientCheckpoint {
    #[inline(always)]
    fn checkpoint_token(&mut self) -> CodecResult<()> {
        Ok(())
    }

    #[inline(always)]
    fn checkpoint_block(&mut self) -> CodecResult<()> {
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

struct TokenCoefficientCheckpoint<'a> {
    token: &'a crate::CancellationToken,
    token_items: usize,
    block_items: usize,
    macroblock_items: usize,
    bit_items: usize,
    output_bytes: usize,
}

impl CoefficientCheckpointControl for TokenCoefficientCheckpoint<'_> {
    #[inline]
    fn checkpoint_token(&mut self) -> CodecResult<()> {
        self.token_items = self.token_items.saturating_add(1);
        if self
            .token_items
            .is_multiple_of(COEFFICIENT_CHECKPOINT_TOKENS)
        {
            crate::codecs::error::check_cancelled(Some(self.token))?;
        }
        Ok(())
    }

    #[inline]
    fn checkpoint_block(&mut self) -> CodecResult<()> {
        self.block_items = self.block_items.saturating_add(1);
        if self
            .block_items
            .is_multiple_of(COEFFICIENT_CHECKPOINT_BLOCKS)
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
            .is_multiple_of(COEFFICIENT_CHECKPOINT_MACROBLOCKS)
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
            .is_multiple_of(COEFFICIENT_8_BIT_CHECKPOINT_BITS)
        {
            crate::codecs::error::check_cancelled(Some(self.token))?;
            if self
                .bit_items
                .is_multiple_of(COEFFICIENT_16_BIT_CHECKPOINT_BITS)
            {
                crate::codecs::error::check_cancelled(Some(self.token))?;
                if self
                    .bit_items
                    .is_multiple_of(COEFFICIENT_32_BIT_CHECKPOINT_BITS)
                {
                    crate::codecs::error::check_cancelled(Some(self.token))?;
                    if self
                        .bit_items
                        .is_multiple_of(COEFFICIENT_64_BIT_CHECKPOINT_BITS)
                    {
                        crate::codecs::error::check_cancelled(Some(self.token))?;
                        if self
                            .bit_items
                            .is_multiple_of(COEFFICIENT_FINER_CHECKPOINT_BITS)
                        {
                            crate::codecs::error::check_cancelled(Some(self.token))?;
                            if self
                                .bit_items
                                .is_multiple_of(COEFFICIENT_FINEST_CHECKPOINT_BITS)
                            {
                                crate::codecs::error::check_cancelled(Some(self.token))?;
                                if self
                                    .bit_items
                                    .is_multiple_of(COEFFICIENT_FINE_CHECKPOINT_BITS)
                                {
                                    crate::codecs::error::check_cancelled(Some(self.token))?;
                                    if self
                                        .bit_items
                                        .is_multiple_of(COEFFICIENT_1024_CHECKPOINT_BITS)
                                    {
                                        crate::codecs::error::check_cancelled(Some(self.token))?;
                                        if self
                                            .bit_items
                                            .is_multiple_of(COEFFICIENT_2048_CHECKPOINT_BITS)
                                        {
                                            crate::codecs::error::check_cancelled(Some(
                                                self.token,
                                            ))?;
                                            if self
                                                .bit_items
                                                .is_multiple_of(COEFFICIENT_4096_CHECKPOINT_BITS)
                                            {
                                                crate::codecs::error::check_cancelled(Some(
                                                    self.token,
                                                ))?;
                                                if self.bit_items.is_multiple_of(
                                                    COEFFICIENT_8192_CHECKPOINT_BITS,
                                                ) {
                                                    crate::codecs::error::check_cancelled(Some(
                                                        self.token,
                                                    ))?;
                                                    if self
                                                        .bit_items
                                                        .is_multiple_of(COEFFICIENT_CHECKPOINT_BITS)
                                                    {
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
        Ok(())
    }

    fn checkpoint_output_bytes(&mut self, emitted: usize) -> CodecResult<()> {
        let previous = self.output_bytes;
        self.output_bytes = self.output_bytes.saturating_add(emitted);
        let mut previous_interval = previous / COEFFICIENT_OUTPUT_CHECKPOINT_BYTES;
        let current_interval = self.output_bytes / COEFFICIENT_OUTPUT_CHECKPOINT_BYTES;
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
fn encode_bool<P: CoefficientCheckpointControl>(
    writer: &mut BoolEncoder,
    probability: u8,
    value: bool,
    checkpoint: &mut P,
) -> CodecResult<()> {
    checkpoint.encode_bool(writer, probability, value)
}

fn write_category_bits<P: CoefficientCheckpointControl>(
    writer: &mut BoolEncoder,
    residue: u16,
    bit_count: usize,
    probabilities: &[u8],
    checkpoint: &mut P,
) -> CodecResult<()> {
    for bit in (0..bit_count).rev() {
        encode_bool(
            writer,
            probabilities[bit_count.saturating_sub(1).saturating_sub(bit)],
            residue & 1u16.wrapping_shl(bit.to_le_bytes()[0].into()) != 0,
            checkpoint,
        )?;
    }
    Ok(())
}

fn write_block<P: CoefficientCheckpointControl>(
    writer: &mut BoolEncoder,
    probabilities: &AdaptedProbabilities,
    levels: &[i16; 16],
    first: usize,
    coefficient_type: usize,
    initial_context: usize,
    checkpoint: &mut P,
) -> CodecResult<u8> {
    let Some(last) = (first..16).rev().find(|&position| levels[position] != 0) else {
        encode_bool(
            writer,
            probabilities.coefficients[coefficient_type][usize::from(COEFF_BANDS[first])]
                [initial_context][0],
            false,
            checkpoint,
        )?;
        checkpoint.checkpoint_token()?;
        return Ok(0);
    };

    let mut position = first;
    let mut band = usize::from(COEFF_BANDS[position]);
    let mut context = initial_context;
    encode_bool(
        writer,
        probabilities.coefficients[coefficient_type][band][context][0],
        true,
        checkpoint,
    )?;

    loop {
        let coefficient = levels[position];
        position = position.saturating_add(1);
        let magnitude = coefficient.unsigned_abs();
        let node_probabilities = probabilities.coefficients[coefficient_type][band][context];

        if magnitude == 0 {
            encode_bool(writer, node_probabilities[1], false, checkpoint)?;
            band = usize::from(COEFF_BANDS[position]);
            context = 0;
            checkpoint.checkpoint_token()?;
            continue;
        }

        encode_bool(writer, node_probabilities[1], true, checkpoint)?;
        if magnitude == 1 {
            encode_bool(writer, node_probabilities[2], false, checkpoint)?;
            context = 1;
        } else {
            encode_bool(writer, node_probabilities[2], true, checkpoint)?;
            if magnitude <= 4 {
                encode_bool(writer, node_probabilities[3], false, checkpoint)?;
                let not_two = magnitude != 2;
                encode_bool(writer, node_probabilities[4], not_two, checkpoint)?;
                if not_two {
                    encode_bool(writer, node_probabilities[5], magnitude == 4, checkpoint)?;
                }
            } else if magnitude <= 10 {
                encode_bool(writer, node_probabilities[3], true, checkpoint)?;
                encode_bool(writer, node_probabilities[6], false, checkpoint)?;
                let greater_than_six = magnitude > 6;
                encode_bool(writer, node_probabilities[7], greater_than_six, checkpoint)?;
                if greater_than_six {
                    encode_bool(writer, 165, magnitude >= 9, checkpoint)?;
                    encode_bool(writer, 145, magnitude & 1 == 0, checkpoint)?;
                } else {
                    encode_bool(writer, 159, magnitude == 6, checkpoint)?;
                }
            } else {
                encode_bool(writer, node_probabilities[3], true, checkpoint)?;
                encode_bool(writer, node_probabilities[6], true, checkpoint)?;
                if magnitude < 19 {
                    encode_bool(writer, node_probabilities[8], false, checkpoint)?;
                    encode_bool(writer, node_probabilities[9], false, checkpoint)?;
                    write_category_bits(
                        writer,
                        magnitude.saturating_sub(11),
                        3,
                        &CAT3_PROBABILITIES,
                        checkpoint,
                    )?;
                } else if magnitude < 35 {
                    encode_bool(writer, node_probabilities[8], false, checkpoint)?;
                    encode_bool(writer, node_probabilities[9], true, checkpoint)?;
                    write_category_bits(
                        writer,
                        magnitude.saturating_sub(19),
                        4,
                        &CAT4_PROBABILITIES,
                        checkpoint,
                    )?;
                } else if magnitude < 67 {
                    encode_bool(writer, node_probabilities[8], true, checkpoint)?;
                    encode_bool(writer, node_probabilities[10], false, checkpoint)?;
                    write_category_bits(
                        writer,
                        magnitude.saturating_sub(35),
                        5,
                        &CAT5_PROBABILITIES,
                        checkpoint,
                    )?;
                } else {
                    encode_bool(writer, node_probabilities[8], true, checkpoint)?;
                    encode_bool(writer, node_probabilities[10], true, checkpoint)?;
                    write_category_bits(
                        writer,
                        magnitude.saturating_sub(67),
                        11,
                        &CAT6_PROBABILITIES,
                        checkpoint,
                    )?;
                }
            }
            context = 2;
        }

        encode_bool(writer, 128, coefficient < 0, checkpoint)?;
        if position == 16 {
            checkpoint.checkpoint_token()?;
            return Ok(1);
        }
        band = usize::from(COEFF_BANDS[position]);
        let has_more = position <= last;
        encode_bool(
            writer,
            probabilities.coefficients[coefficient_type][band][context][0],
            has_more,
            checkpoint,
        )?;
        if !has_more {
            checkpoint.checkpoint_token()?;
            return Ok(1);
        }
        checkpoint.checkpoint_token()?;
    }
}

fn write_block_with_checkpoint<P: CoefficientCheckpointControl>(
    writer: &mut BoolEncoder,
    probabilities: &AdaptedProbabilities,
    levels: &[i16; 16],
    first: usize,
    coefficient_type: usize,
    initial_context: usize,
    checkpoint: &mut P,
) -> CodecResult<u8> {
    let nonzero = write_block(
        writer,
        probabilities,
        levels,
        first,
        coefficient_type,
        initial_context,
        checkpoint,
    )?;
    checkpoint.checkpoint_block()?;
    Ok(nonzero)
}

fn encode_coefficients_with_checkpoint<P: CoefficientCheckpointControl>(
    decisions: &[MacroblockDecision],
    macroblock_width: usize,
    probabilities: &AdaptedProbabilities,
    checkpoint: &mut P,
) -> CodecResult<Vec<u8>> {
    let mut writer = BoolEncoder::default();
    let mut top_y = vec![[0u8; 4]; macroblock_width];
    let mut top_uv = vec![[0u8; 4]; macroblock_width];
    let mut top_y2 = vec![0u8; macroblock_width];

    for row in decisions.chunks_exact(macroblock_width) {
        let mut left_y = [0u8; 4];
        let mut left_uv = [0u8; 4];
        let mut left_y2 = 0u8;
        for decision in row {
            let x = decision.x;
            match &decision.luma {
                LumaDecision::Intra16(luma) => {
                    let nonzero = write_block_with_checkpoint(
                        &mut writer,
                        probabilities,
                        &luma.y2_levels,
                        0,
                        1,
                        usize::from(top_y2[x].saturating_add(left_y2)),
                        checkpoint,
                    )?;
                    top_y2[x] = nonzero;
                    left_y2 = nonzero;
                    let top_row = &mut top_y[x];
                    for (block_y, left_nonzero) in left_y.iter_mut().enumerate() {
                        for (block_x, top_nonzero) in top_row.iter_mut().enumerate() {
                            let nonzero = write_block_with_checkpoint(
                                &mut writer,
                                probabilities,
                                &luma.y1_levels[block_y.saturating_mul(4).saturating_add(block_x)],
                                1,
                                0,
                                usize::from(top_nonzero.saturating_add(*left_nonzero)),
                                checkpoint,
                            )?;
                            *top_nonzero = nonzero;
                            *left_nonzero = nonzero;
                        }
                    }
                }
                LumaDecision::Intra4(luma) => {
                    let top_row = &mut top_y[x];
                    for (block_y, left_nonzero) in left_y.iter_mut().enumerate() {
                        for (block_x, top_nonzero) in top_row.iter_mut().enumerate() {
                            let nonzero = write_block_with_checkpoint(
                                &mut writer,
                                probabilities,
                                &luma.levels[block_y.saturating_mul(4).saturating_add(block_x)],
                                0,
                                3,
                                usize::from(top_nonzero.saturating_add(*left_nonzero)),
                                checkpoint,
                            )?;
                            *top_nonzero = nonzero;
                            *left_nonzero = nonzero;
                        }
                    }
                }
            }

            for plane in 0usize..2 {
                for block_y in 0usize..2 {
                    for block_x in 0usize..2 {
                        let top_index = plane.saturating_mul(2).saturating_add(block_x);
                        let left_index = plane.saturating_mul(2).saturating_add(block_y);
                        let nonzero = write_block_with_checkpoint(
                            &mut writer,
                            probabilities,
                            &decision.chroma.levels[plane
                                .saturating_mul(4)
                                .saturating_add(block_y.saturating_mul(2))
                                .saturating_add(block_x)],
                            0,
                            2,
                            usize::from(top_uv[x][top_index].saturating_add(left_uv[left_index])),
                            checkpoint,
                        )?;
                        top_uv[x][top_index] = nonzero;
                        left_uv[left_index] = nonzero;
                    }
                }
            }
            checkpoint.checkpoint_macroblock()?;
        }
    }
    checkpoint.finish(writer)
}

pub(super) fn encode_coefficients(
    decisions: &[MacroblockDecision],
    macroblock_width: usize,
    probabilities: &AdaptedProbabilities,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    if let Some(token) = token {
        let mut checkpoint = TokenCoefficientCheckpoint {
            token,
            token_items: 0,
            block_items: 0,
            macroblock_items: 0,
            bit_items: 0,
            output_bytes: 0,
        };
        encode_coefficients_with_checkpoint(
            decisions,
            macroblock_width,
            probabilities,
            &mut checkpoint,
        )
    } else {
        let mut checkpoint = NoopCoefficientCheckpoint;
        encode_coefficients_with_checkpoint(
            decisions,
            macroblock_width,
            probabilities,
            &mut checkpoint,
        )
    }
}
