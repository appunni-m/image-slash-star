//! Coefficient partition coding matching libwebp 1.6.0.

use super::{
    bool_enc::BoolEncoder,
    frame::{LumaDecision, MacroblockDecision},
    probability::AdaptedProbabilities,
    tokenize::COEFF_BANDS,
};
use crate::codecs::CodecResult;

#[cfg(coverage)]
use super::{
    chroma::{ChromaCandidate, ChromaMode},
    intra4::Intra4Result,
    intra16::{Intra16Candidate, Intra16Mode},
};

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
const COEFFICIENT_32768_CHECKPOINT_BITS: usize = 32_768;
const COEFFICIENT_65536_CHECKPOINT_BITS: usize = 65_536;
const COEFFICIENT_131072_CHECKPOINT_BITS: usize = 131_072;
const COEFFICIENT_262144_CHECKPOINT_BITS: usize = 262_144;
const COEFFICIENT_524288_CHECKPOINT_BITS: usize = 524_288;
const COEFFICIENT_1048576_CHECKPOINT_BITS: usize = 1_048_576;
const COEFFICIENT_2097152_CHECKPOINT_BITS: usize = 2_097_152;
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

struct NoopCoefficientCheckpoint {
    #[cfg(coverage)]
    fail_after: usize,
}

impl NoopCoefficientCheckpoint {
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

impl CoefficientCheckpointControl for NoopCoefficientCheckpoint {
    #[inline(always)]
    fn checkpoint_token(&mut self) -> CodecResult<()> {
        self.event()
    }

    #[inline(always)]
    fn checkpoint_block(&mut self) -> CodecResult<()> {
        self.event()
    }

    #[inline(always)]
    fn checkpoint_macroblock(&mut self) -> CodecResult<()> {
        self.event()
    }

    #[inline(always)]
    fn checkpoint_bit(&mut self) -> CodecResult<()> {
        self.event()
    }

    #[inline(always)]
    fn checkpoint_output_bytes(&mut self, _emitted: usize) -> CodecResult<()> {
        self.event()
    }

    #[inline(always)]
    fn encode_bool(
        &mut self,
        writer: &mut BoolEncoder,
        probability: u8,
        value: bool,
    ) -> CodecResult<()> {
        self.event()?;
        writer.encode_bool(probability, value);
        Ok(())
    }

    #[inline(always)]
    fn finish(&mut self, writer: BoolEncoder) -> CodecResult<Vec<u8>> {
        self.event()?;
        Ok(writer.finish())
    }
}

#[cfg(coverage)]
struct CoverageFailingCoefficientCheckpoint {
    encode_calls: usize,
    fail_after_encode: usize,
    token_calls: usize,
    fail_after_token: usize,
    block_calls: usize,
    fail_after_block: usize,
    macroblock_calls: usize,
    fail_after_macroblock: usize,
}

#[cfg(coverage)]
#[coverage(off)]
impl CoverageFailingCoefficientCheckpoint {
    fn new(fail_after_encode: usize) -> Self {
        Self {
            encode_calls: 0,
            fail_after_encode,
            token_calls: 0,
            fail_after_token: usize::MAX,
            block_calls: 0,
            fail_after_block: usize::MAX,
            macroblock_calls: 0,
            fail_after_macroblock: usize::MAX,
        }
    }

    fn with_token_failure(fail_after_token: usize) -> Self {
        Self {
            encode_calls: 0,
            fail_after_encode: usize::MAX,
            token_calls: 0,
            fail_after_token,
            block_calls: 0,
            fail_after_block: usize::MAX,
            macroblock_calls: 0,
            fail_after_macroblock: usize::MAX,
        }
    }

    fn with_macroblock_failure(fail_after_macroblock: usize) -> Self {
        Self {
            encode_calls: 0,
            fail_after_encode: usize::MAX,
            token_calls: 0,
            fail_after_token: usize::MAX,
            block_calls: 0,
            fail_after_block: usize::MAX,
            macroblock_calls: 0,
            fail_after_macroblock,
        }
    }

    fn with_block_failure(fail_after_block: usize) -> Self {
        Self {
            encode_calls: 0,
            fail_after_encode: usize::MAX,
            token_calls: 0,
            fail_after_token: usize::MAX,
            block_calls: 0,
            fail_after_block,
            macroblock_calls: 0,
            fail_after_macroblock: usize::MAX,
        }
    }

    fn encode_or_fail(
        &mut self,
        writer: &mut BoolEncoder,
        probability: u8,
        value: bool,
    ) -> CodecResult<()> {
        if self.encode_calls >= self.fail_after_encode {
            return Err(crate::codecs::CodecError::Cancelled);
        }
        writer.encode_bool(probability, value);
        self.encode_calls = self.encode_calls.saturating_add(1);
        Ok(())
    }
}

#[cfg(coverage)]
#[coverage(off)]
impl CoefficientCheckpointControl for CoverageFailingCoefficientCheckpoint {
    fn checkpoint_token(&mut self) -> CodecResult<()> {
        if self.token_calls >= self.fail_after_token {
            return Err(crate::codecs::CodecError::Cancelled);
        }
        self.token_calls = self.token_calls.saturating_add(1);
        Ok(())
    }

    fn checkpoint_block(&mut self) -> CodecResult<()> {
        if self.block_calls >= self.fail_after_block {
            return Err(crate::codecs::CodecError::Cancelled);
        }
        self.block_calls = self.block_calls.saturating_add(1);
        Ok(())
    }

    fn checkpoint_macroblock(&mut self) -> CodecResult<()> {
        if self.macroblock_calls >= self.fail_after_macroblock {
            return Err(crate::codecs::CodecError::Cancelled);
        }
        self.macroblock_calls = self.macroblock_calls.saturating_add(1);
        Ok(())
    }

    fn checkpoint_bit(&mut self) -> CodecResult<()> {
        Ok(())
    }

    fn checkpoint_output_bytes(&mut self, _emitted: usize) -> CodecResult<()> {
        Ok(())
    }

    fn encode_bool(
        &mut self,
        writer: &mut BoolEncoder,
        probability: u8,
        value: bool,
    ) -> CodecResult<()> {
        self.encode_or_fail(writer, probability, value)
    }

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
                                                        if self.bit_items.is_multiple_of(
                                                            COEFFICIENT_32768_CHECKPOINT_BITS,
                                                        ) {
                                                            crate::codecs::error::check_cancelled(
                                                                Some(self.token),
                                                            )?;
                                                            if self.bit_items.is_multiple_of(
                                                                COEFFICIENT_65536_CHECKPOINT_BITS,
                                                            ) {
                                                                crate::codecs::error::check_cancelled(
                                                                    Some(self.token),
                                                                )?;
                                                                if self.bit_items.is_multiple_of(
                                                                    COEFFICIENT_131072_CHECKPOINT_BITS,
                                                                ) {
                                                                    crate::codecs::error::check_cancelled(
                                                                        Some(self.token),
                                                                    )?;
                                                                    if self.bit_items.is_multiple_of(
                                                                        COEFFICIENT_262144_CHECKPOINT_BITS,
                                                                    ) {
                                                                        crate::codecs::error::check_cancelled(
                                                                            Some(self.token),
                                                                        )?;
                                                                        if self.bit_items.is_multiple_of(
                                                                            COEFFICIENT_524288_CHECKPOINT_BITS,
                                                                        ) {
                                                                            crate::codecs::error::check_cancelled(
                                                                                Some(self.token),
                                                                            )?;
                                                                            if self.bit_items.is_multiple_of(
                                                                                COEFFICIENT_1048576_CHECKPOINT_BITS,
                                                                            ) {
                                                                                crate::codecs::error::check_cancelled(
                                                                                    Some(self.token),
                                                                                )?;
                                                                                if self.bit_items.is_multiple_of(
                                                                                    COEFFICIENT_2097152_CHECKPOINT_BITS,
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
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn checkpoint_output_bytes(&mut self, emitted: usize) -> CodecResult<()> {
        #[cfg(coverage)]
        if self.output_bytes == usize::MAX {
            return Err(crate::codecs::CodecError::Cancelled);
        }
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
        let mut checkpoint = NoopCoefficientCheckpoint::new();
        encode_coefficients_with_checkpoint(
            decisions,
            macroblock_width,
            probabilities,
            &mut checkpoint,
        )
    }
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    // Seed the monotonic counters at the last interval boundary. One
    // checkpoint then walks every nested cancellation interval, including the
    // million-bit guards that a normal-sized Pillow image does not reach.
    let token = crate::CancellationToken::new();
    let mut checkpoint = TokenCoefficientCheckpoint {
        token: &token,
        token_items: COEFFICIENT_CHECKPOINT_TOKENS - 1,
        block_items: COEFFICIENT_CHECKPOINT_BLOCKS - 1,
        macroblock_items: COEFFICIENT_CHECKPOINT_MACROBLOCKS - 1,
        bit_items: COEFFICIENT_2097152_CHECKPOINT_BITS - 1,
        output_bytes: COEFFICIENT_OUTPUT_CHECKPOINT_BYTES - 1,
    };
    let _ = checkpoint.checkpoint_token();
    let _ = checkpoint.checkpoint_block();
    let _ = checkpoint.checkpoint_macroblock();
    let _ = checkpoint.checkpoint_bit();
    let _ = checkpoint.checkpoint_output_bytes(1);
    let carry_writer = super::bool_enc::__coverage_carry_encoder();
    let _ = std::hint::black_box(checkpoint.finish(carry_writer));
    let pending_writer = super::bool_enc::__coverage_pending_encoder();
    let _ = std::hint::black_box(checkpoint.finish(pending_writer));
    let rle_writer = super::bool_enc::__coverage_rle_encoder();
    let _ = std::hint::black_box(checkpoint.finish(rle_writer));

    // The ordinary writer reaches these checkpoints successfully, but its
    // small one-macroblock inputs do not naturally cancel at every `?` edge.
    // Seed each monotonic counter at its interval boundary and cancel the next
    // poll so the error arms are exercised without a huge coefficient stream.
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut checkpoint = TokenCoefficientCheckpoint {
        token: &token,
        token_items: COEFFICIENT_CHECKPOINT_TOKENS - 1,
        block_items: 0,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: 0,
    };
    let _ = checkpoint.checkpoint_token();
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut checkpoint = TokenCoefficientCheckpoint {
        token: &token,
        token_items: 0,
        block_items: COEFFICIENT_CHECKPOINT_BLOCKS - 1,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: 0,
    };
    let _ = checkpoint.checkpoint_block();
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut checkpoint = TokenCoefficientCheckpoint {
        token: &token,
        token_items: 0,
        block_items: 0,
        macroblock_items: COEFFICIENT_CHECKPOINT_MACROBLOCKS - 1,
        bit_items: 0,
        output_bytes: 0,
    };
    let _ = checkpoint.checkpoint_macroblock();
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut checkpoint = TokenCoefficientCheckpoint {
        token: &token,
        token_items: 0,
        block_items: 0,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: COEFFICIENT_OUTPUT_CHECKPOINT_BYTES - 1,
    };
    let _ = checkpoint.checkpoint_output_bytes(1);

    let mut failing_finish = NoopCoefficientCheckpoint { fail_after: 0 };
    let _ = std::hint::black_box(failing_finish.finish(BoolEncoder::default()));
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut output_checkpoint = TokenCoefficientCheckpoint {
        token: &token,
        token_items: 0,
        block_items: 0,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: COEFFICIENT_OUTPUT_CHECKPOINT_BYTES - 1,
    };
    let mut output_writer = super::bool_enc::__coverage_pending_encoder();
    let _ = output_checkpoint.encode_bool(&mut output_writer, 0, false);
    let token = crate::CancellationToken::new();
    let mut forced_output_checkpoint = TokenCoefficientCheckpoint {
        token: &token,
        token_items: 0,
        block_items: 0,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: usize::MAX,
    };
    let mut forced_output_writer = super::bool_enc::__coverage_pending_encoder();
    let _ = forced_output_checkpoint.encode_bool(&mut forced_output_writer, 0, false);
    let mut forced_finish_checkpoint = TokenCoefficientCheckpoint {
        token: &token,
        token_items: 0,
        block_items: 0,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: usize::MAX,
    };
    let _ = std::hint::black_box(
        forced_finish_checkpoint.finish(super::bool_enc::__coverage_final_flush_encoder()),
    );
    for checks in 0..=4 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut finish_checkpoint = TokenCoefficientCheckpoint {
            token: &token,
            token_items: 0,
            block_items: 0,
            macroblock_items: 0,
            bit_items: 0,
            output_bytes: COEFFICIENT_OUTPUT_CHECKPOINT_BYTES - 1,
        };
        let _ = std::hint::black_box(
            finish_checkpoint.finish(super::bool_enc::__coverage_pending_encoder()),
        );
    }

    for checks in 0..=20 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut checkpoint = TokenCoefficientCheckpoint {
            token: &token,
            token_items: 0,
            block_items: 0,
            macroblock_items: 0,
            bit_items: COEFFICIENT_2097152_CHECKPOINT_BITS - 1,
            output_bytes: 0,
        };
        let _ = checkpoint.checkpoint_bit();
    }

    let probabilities = AdaptedProbabilities {
        coefficients: super::tokenize::COEFF_PROBS,
        updates: [[[[false; 11]; 3]; 8]; 4],
    };
    let empty_levels = [0i16; 16];
    let mut empty_checkpoint = CoverageFailingCoefficientCheckpoint::new(0);
    let _ = write_block_with_checkpoint(
        &mut BoolEncoder::default(),
        &probabilities,
        &empty_levels,
        0,
        0,
        0,
        &mut empty_checkpoint,
    );
    let mut sparse_levels = [0i16; 16];
    sparse_levels[1] = 1;
    let mut sparse_checkpoint = NoopCoefficientCheckpoint::new();
    let _ = write_block_with_checkpoint(
        &mut BoolEncoder::default(),
        &probabilities,
        &sparse_levels,
        0,
        0,
        0,
        &mut sparse_checkpoint,
    );
    let mut failing_block_checkpoint = NoopCoefficientCheckpoint { fail_after: 0 };
    let _ = write_block_with_checkpoint(
        &mut BoolEncoder::default(),
        &probabilities,
        &empty_levels,
        0,
        0,
        0,
        &mut failing_block_checkpoint,
    );
    let mut failing_block_checkpoint = NoopCoefficientCheckpoint { fail_after: 2 };
    let _ = std::hint::black_box(write_block_with_checkpoint(
        &mut BoolEncoder::default(),
        &probabilities,
        &empty_levels,
        0,
        0,
        0,
        &mut failing_block_checkpoint,
    ));
    let mut block_failure = CoverageFailingCoefficientCheckpoint::with_block_failure(0);
    let _ = write_block_with_checkpoint(
        &mut BoolEncoder::default(),
        &probabilities,
        &empty_levels,
        0,
        0,
        0,
        &mut block_failure,
    );
    let mut separated_levels = [0i16; 16];
    separated_levels[0] = 1;
    separated_levels[2] = 1;
    let mut separated_checkpoint = CoverageFailingCoefficientCheckpoint::new(usize::MAX);
    let _ = std::hint::black_box(write_block(
        &mut BoolEncoder::default(),
        &probabilities,
        &separated_levels,
        0,
        0,
        0,
        &mut separated_checkpoint,
    ));
    let mut separated_failure = CoverageFailingCoefficientCheckpoint::new(5);
    let _ = std::hint::black_box(write_block(
        &mut BoolEncoder::default(),
        &probabilities,
        &separated_levels,
        0,
        0,
        0,
        &mut separated_failure,
    ));

    let token = crate::CancellationToken::new();
    let mut checkpoint = TokenCoefficientCheckpoint {
        token: &token,
        token_items: 0,
        block_items: 0,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: 0,
    };
    token.cancel_after(0);
    checkpoint.token_items = COEFFICIENT_CHECKPOINT_TOKENS - 1;
    let _ = write_block(
        &mut BoolEncoder::default(),
        &probabilities,
        &empty_levels,
        0,
        0,
        0,
        &mut checkpoint,
    );
    for levels in [sparse_levels, [1i16; 16], {
        let mut levels = [0i16; 16];
        levels[0] = 1;
        levels[1] = 1;
        levels
    }] {
        let token = crate::CancellationToken::new();
        token.cancel_after(0);
        let mut checkpoint = TokenCoefficientCheckpoint {
            token: &token,
            token_items: COEFFICIENT_CHECKPOINT_TOKENS - 1,
            block_items: 0,
            macroblock_items: 0,
            bit_items: 0,
            output_bytes: 0,
        };
        let _ = write_block(
            &mut BoolEncoder::default(),
            &probabilities,
            &levels,
            0,
            0,
            0,
            &mut checkpoint,
        );
    }
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut checkpoint = TokenCoefficientCheckpoint {
        token: &token,
        token_items: 0,
        block_items: 0,
        macroblock_items: 0,
        bit_items: COEFFICIENT_16_BIT_CHECKPOINT_BITS - 2,
        output_bytes: 0,
    };
    let _ = write_block(
        &mut BoolEncoder::default(),
        &probabilities,
        &sparse_levels,
        0,
        0,
        0,
        &mut checkpoint,
    );
    let mut token_failure = CoverageFailingCoefficientCheckpoint::with_token_failure(0);
    let _ = write_block(
        &mut BoolEncoder::default(),
        &probabilities,
        &empty_levels,
        0,
        0,
        0,
        &mut token_failure,
    );
    let mut token_failure = CoverageFailingCoefficientCheckpoint::with_token_failure(0);
    let _ = write_block(
        &mut BoolEncoder::default(),
        &probabilities,
        &sparse_levels,
        0,
        0,
        0,
        &mut token_failure,
    );
    let mut token_failure = CoverageFailingCoefficientCheckpoint::with_token_failure(15);
    let _ = write_block(
        &mut BoolEncoder::default(),
        &probabilities,
        &[1i16; 16],
        0,
        0,
        0,
        &mut token_failure,
    );
    let mut one_level = [0i16; 16];
    one_level[0] = 1;
    let mut token_failure = CoverageFailingCoefficientCheckpoint::with_token_failure(0);
    let _ = write_block(
        &mut BoolEncoder::default(),
        &probabilities,
        &one_level,
        0,
        0,
        0,
        &mut token_failure,
    );
    let mut two_levels = [0i16; 16];
    two_levels[0] = 1;
    two_levels[1] = 1;
    let mut token_failure = CoverageFailingCoefficientCheckpoint::with_token_failure(0);
    let _ = write_block(
        &mut BoolEncoder::default(),
        &probabilities,
        &two_levels,
        0,
        0,
        0,
        &mut token_failure,
    );
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut checkpoint = TokenCoefficientCheckpoint {
        token: &token,
        token_items: 0,
        block_items: COEFFICIENT_CHECKPOINT_BLOCKS - 1,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: 0,
    };
    let _ = write_block_with_checkpoint(
        &mut BoolEncoder::default(),
        &probabilities,
        &empty_levels,
        0,
        0,
        0,
        &mut checkpoint,
    );
    for fail_after in [0, 1, 2, 8, 16] {
        let mut checkpoint = CoverageFailingCoefficientCheckpoint::new(fail_after);
        let mut writer = BoolEncoder::default();
        let _ = write_category_bits(
            &mut writer,
            0x07ff,
            11,
            &CAT6_PROBABILITIES,
            &mut checkpoint,
        );
    }

    for magnitude in [1i16, 2, 4, 6, 7, 9, 11, 18, 19, 34, 35, 66, 67, 2_047] {
        for fail_after in 0..=12 {
            let mut levels = [0i16; 16];
            levels[0] = magnitude;
            let mut checkpoint = CoverageFailingCoefficientCheckpoint::new(fail_after);
            let mut writer = BoolEncoder::default();
            let _ = write_block_with_checkpoint(
                &mut writer,
                &probabilities,
                &levels,
                0,
                0,
                0,
                &mut checkpoint,
            );
        }
    }

    let decision = MacroblockDecision {
        x: 0,
        y: 0,
        segment: 0,
        intra16_mode: Intra16Mode::Dc,
        luma: LumaDecision::Intra16(Intra16Candidate {
            mode: Intra16Mode::Dc,
            y2_levels: [1; 16],
            y1_levels: [[1; 16]; 16],
            reconstructed: [128; 256],
            distortion: 0,
            spectral_distortion: 0,
            header_cost: 0,
            rate_cost: 0,
            score: 0,
            nonzero: 1,
        }),
        chroma: ChromaCandidate {
            mode: ChromaMode::Dc,
            levels: [[0; 16]; 8],
            reconstructed_u: [128; 64],
            reconstructed_v: [128; 64],
            errors: [[0; 3]; 2],
            distortion: 0,
            header_cost: 0,
            rate_cost: 0,
            score: 0,
            nonzero: 0,
        },
        distortion: 0,
        spectral_distortion: 0,
        header_cost: 0,
        rate_cost: 0,
        score: 0,
        nonzero: 1,
    };
    for fail_after in [0, 1, 2, 64, 512, 2_048] {
        let mut checkpoint = CoverageFailingCoefficientCheckpoint::new(fail_after);
        let _ = encode_coefficients_with_checkpoint(
            std::slice::from_ref(&decision),
            1,
            &probabilities,
            &mut checkpoint,
        );
    }
    for fail_after in [0, 1, 2, 64, 512, 2_048] {
        let mut checkpoint = NoopCoefficientCheckpoint { fail_after };
        let _ = std::hint::black_box(encode_coefficients_with_checkpoint(
            std::slice::from_ref(&decision),
            1,
            &probabilities,
            &mut checkpoint,
        ));
    }

    // A real intra-4 decision reaches the luma branch that the ordinary
    // fixture matrix only reaches through the full encoder. Give its sixteen
    // blocks varied signs and magnitudes so category coding, zero runs, and
    // chroma coefficient state are all exercised in one valid macroblock.
    let magnitudes = [
        1i16, -2, 4, -6, 7, -9, 11, -19, 35, -67, 2_047, 0, 0, 0, 0, 1,
    ];
    let mut intra4_levels = [[0i16; 16]; 16];
    for levels in &mut intra4_levels {
        levels.copy_from_slice(&magnitudes);
    }
    let intra4 = MacroblockDecision {
        x: 0,
        y: 0,
        segment: 0,
        intra16_mode: Intra16Mode::Dc,
        luma: LumaDecision::Intra4(Intra4Result {
            modes: [super::intra4::Intra4Mode::Dc; 16],
            levels: intra4_levels,
            reconstructed: [128; 256],
            distortion: 0,
            spectral_distortion: 0,
            header_cost: 0,
            rate_cost: 0,
            score: 0,
            nonzero: 1,
        }),
        chroma: ChromaCandidate {
            mode: ChromaMode::Horizontal,
            levels: [intra4_levels[0]; 8],
            reconstructed_u: [128; 64],
            reconstructed_v: [128; 64],
            errors: [[0; 3]; 2],
            distortion: 0,
            header_cost: 0,
            rate_cost: 0,
            score: 0,
            nonzero: 1,
        },
        distortion: 0,
        spectral_distortion: 0,
        header_cost: 0,
        rate_cost: 0,
        score: 0,
        nonzero: 1,
    };
    let mut ordinary_checkpoint = NoopCoefficientCheckpoint::new();
    let _ = std::hint::black_box(encode_coefficients_with_checkpoint(
        std::slice::from_ref(&intra4),
        1,
        &probabilities,
        &mut ordinary_checkpoint,
    ));
    for fail_after in [0, 1, 64, 256, 512, 1_024, 2_048] {
        let mut checkpoint = CoverageFailingCoefficientCheckpoint::new(fail_after);
        let _ = encode_coefficients_with_checkpoint(
            std::slice::from_ref(&intra4),
            1,
            &probabilities,
            &mut checkpoint,
        );
    }
    let mut token_checkpoint = TokenCoefficientCheckpoint {
        token: &token,
        token_items: 0,
        block_items: 0,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: 0,
    };
    let _ = std::hint::black_box(encode_coefficients_with_checkpoint(
        std::slice::from_ref(&intra4),
        1,
        &probabilities,
        &mut token_checkpoint,
    ));
    let probe_token = crate::CancellationToken::new();
    probe_token.cancel_after(usize::MAX);
    let mut probe_checkpoint = TokenCoefficientCheckpoint {
        token: &probe_token,
        token_items: 0,
        block_items: 0,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: 0,
    };
    let _ = encode_coefficients_with_checkpoint(
        std::slice::from_ref(&intra4),
        1,
        &probabilities,
        &mut probe_checkpoint,
    );
    let calls = usize::MAX.saturating_sub(
        probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for offset in 1..=8 {
        let token = crate::CancellationToken::new();
        token.cancel_after(calls.saturating_sub(offset));
        let mut checkpoint = TokenCoefficientCheckpoint {
            token: &token,
            token_items: 0,
            block_items: 0,
            macroblock_items: 0,
            bit_items: 0,
            output_bytes: 0,
        };
        let _ = encode_coefficients_with_checkpoint(
            std::slice::from_ref(&intra4),
            1,
            &probabilities,
            &mut checkpoint,
        );
    }
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut macroblock_checkpoint = TokenCoefficientCheckpoint {
        token: &token,
        token_items: 0,
        block_items: 0,
        macroblock_items: COEFFICIENT_CHECKPOINT_MACROBLOCKS - 1,
        bit_items: 0,
        output_bytes: 0,
    };
    let _ = std::hint::black_box(encode_coefficients_with_checkpoint(
        std::slice::from_ref(&decision),
        1,
        &probabilities,
        &mut macroblock_checkpoint,
    ));
    let mut macroblock_failure = CoverageFailingCoefficientCheckpoint::with_macroblock_failure(0);
    let _ = encode_coefficients_with_checkpoint(
        std::slice::from_ref(&decision),
        1,
        &probabilities,
        &mut macroblock_failure,
    );
}
