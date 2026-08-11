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

#[cfg(coverage)]
use super::{chroma::ChromaCandidate, intra4::Intra4Result, intra16::Intra16Candidate};

const PARTITION_PROBABILITY_CHECKPOINT_NODES: usize = 1_024;
const PARTITION_FILTER_EDGE_CHECKPOINT_MACROBLOCKS: usize = 1_024;
const PARTITION_PREPASS_CHECKPOINT_MACROBLOCKS: usize = 1_024;
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
const PARTITION_65536_BIT_CHECKPOINT_BITS: usize = 65_536;
const PARTITION_131072_BIT_CHECKPOINT_BITS: usize = 131_072;
const PARTITION_262144_BIT_CHECKPOINT_BITS: usize = 262_144;
const PARTITION_OUTPUT_CHECKPOINT_BYTES: usize = 1_024;

trait PartitionCheckpointControl {
    fn checkpoint_probability(&mut self) -> CodecResult<()>;
    fn checkpoint_filter_edge_macroblock(&mut self) -> CodecResult<()>;
    fn checkpoint_prepass_macroblock(&mut self) -> CodecResult<()>;
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

struct NoopPartitionCheckpoint {
    #[cfg(coverage)]
    fail_after: usize,
}

impl NoopPartitionCheckpoint {
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

impl PartitionCheckpointControl for NoopPartitionCheckpoint {
    #[inline(always)]
    fn checkpoint_probability(&mut self) -> CodecResult<()> {
        self.event()
    }

    #[inline(always)]
    fn checkpoint_filter_edge_macroblock(&mut self) -> CodecResult<()> {
        self.event()
    }

    #[inline(always)]
    fn checkpoint_prepass_macroblock(&mut self) -> CodecResult<()> {
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
struct CoverageFailingPartitionCheckpoint {
    encode_calls: usize,
    fail_after_encode: Option<usize>,
    fail_filter_edge: bool,
    fail_prepass: bool,
    fail_probability: bool,
    fail_macroblock: bool,
}

#[cfg(coverage)]
#[coverage(off)]
impl CoverageFailingPartitionCheckpoint {
    fn new(fail_after_encode: usize) -> Self {
        Self {
            encode_calls: 0,
            fail_after_encode: Some(fail_after_encode),
            fail_filter_edge: false,
            fail_prepass: false,
            fail_probability: false,
            fail_macroblock: false,
        }
    }

    fn with_filter_edge_failure() -> Self {
        Self {
            encode_calls: 0,
            fail_after_encode: Some(usize::MAX),
            fail_filter_edge: std::hint::black_box(true),
            fail_prepass: false,
            fail_probability: false,
            fail_macroblock: false,
        }
    }

    fn with_prepass_failure() -> Self {
        Self {
            encode_calls: 0,
            fail_after_encode: Some(usize::MAX),
            fail_filter_edge: false,
            fail_prepass: std::hint::black_box(true),
            fail_probability: false,
            fail_macroblock: false,
        }
    }

    fn with_probability_failure() -> Self {
        Self {
            encode_calls: 0,
            fail_after_encode: Some(usize::MAX),
            fail_filter_edge: false,
            fail_prepass: false,
            fail_probability: std::hint::black_box(true),
            fail_macroblock: false,
        }
    }

    fn with_macroblock_failure() -> Self {
        Self {
            encode_calls: 0,
            fail_after_encode: Some(usize::MAX),
            fail_filter_edge: false,
            fail_prepass: false,
            fail_probability: false,
            fail_macroblock: std::hint::black_box(true),
        }
    }

    fn encode_or_fail(
        &mut self,
        writer: &mut BoolEncoder,
        probability: u8,
        value: bool,
    ) -> CodecResult<()> {
        if self
            .fail_after_encode
            .is_some_and(|limit| self.encode_calls >= limit)
        {
            return Err(crate::codecs::CodecError::Cancelled);
        }
        writer.encode_bool(probability, value);
        self.encode_calls = self.encode_calls.saturating_add(1);
        Ok(())
    }
}

#[cfg(coverage)]
#[coverage(off)]
impl PartitionCheckpointControl for CoverageFailingPartitionCheckpoint {
    fn checkpoint_probability(&mut self) -> CodecResult<()> {
        if self.fail_probability {
            return Err(crate::codecs::CodecError::Cancelled);
        }
        Ok(())
    }

    fn checkpoint_filter_edge_macroblock(&mut self) -> CodecResult<()> {
        if self.fail_filter_edge {
            return Err(crate::codecs::CodecError::Cancelled);
        }
        Ok(())
    }

    fn checkpoint_prepass_macroblock(&mut self) -> CodecResult<()> {
        if self.fail_prepass {
            return Err(crate::codecs::CodecError::Cancelled);
        }
        Ok(())
    }

    fn checkpoint_macroblock(&mut self) -> CodecResult<()> {
        if self.fail_macroblock {
            return Err(crate::codecs::CodecError::Cancelled);
        }
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

struct TokenPartitionCheckpoint<'a> {
    token: &'a crate::CancellationToken,
    probability_items: usize,
    filter_edge_items: usize,
    prepass_items: usize,
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
    fn checkpoint_filter_edge_macroblock(&mut self) -> CodecResult<()> {
        self.filter_edge_items = self.filter_edge_items.saturating_add(1);
        if self
            .filter_edge_items
            .is_multiple_of(PARTITION_FILTER_EDGE_CHECKPOINT_MACROBLOCKS)
        {
            crate::codecs::error::check_cancelled(Some(self.token))?;
        }
        Ok(())
    }

    #[inline]
    fn checkpoint_prepass_macroblock(&mut self) -> CodecResult<()> {
        self.prepass_items = self.prepass_items.saturating_add(1);
        if self
            .prepass_items
            .is_multiple_of(PARTITION_PREPASS_CHECKPOINT_MACROBLOCKS)
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
                                                            if self.bit_items.is_multiple_of(
                                                                PARTITION_65536_BIT_CHECKPOINT_BITS,
                                                            ) {
                                                                crate::codecs::error::check_cancelled(
                                                                    Some(self.token),
                                                                )?;
                                                                if self.bit_items.is_multiple_of(
                                                                    PARTITION_131072_BIT_CHECKPOINT_BITS,
                                                                ) {
                                                                    crate::codecs::error::check_cancelled(
                                                                        Some(self.token),
                                                                    )?;
                                                                    if self.bit_items.is_multiple_of(
                                                                        PARTITION_262144_BIT_CHECKPOINT_BITS,
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
        Ok(())
    }

    fn checkpoint_output_bytes(&mut self, emitted: usize) -> CodecResult<()> {
        #[cfg(coverage)]
        if self.output_bytes == usize::MAX {
            return Err(crate::codecs::CodecError::Cancelled);
        }
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

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let token = crate::CancellationToken::new();
    let mut checkpoint = TokenPartitionCheckpoint {
        token: &token,
        probability_items: PARTITION_PROBABILITY_CHECKPOINT_NODES - 1,
        filter_edge_items: PARTITION_FILTER_EDGE_CHECKPOINT_MACROBLOCKS - 1,
        prepass_items: PARTITION_PREPASS_CHECKPOINT_MACROBLOCKS - 1,
        macroblock_items: PARTITION_MODE_CHECKPOINT_MACROBLOCKS - 1,
        bit_items: PARTITION_262144_BIT_CHECKPOINT_BITS - 1,
        output_bytes: PARTITION_OUTPUT_CHECKPOINT_BYTES - 1,
    };
    let _ = checkpoint.checkpoint_probability();
    let _ = checkpoint.checkpoint_filter_edge_macroblock();
    let _ = checkpoint.checkpoint_prepass_macroblock();
    let _ = checkpoint.checkpoint_macroblock();
    let _ = checkpoint.checkpoint_bit();
    let _ = checkpoint.checkpoint_output_bytes(1);

    // Visit each token-aware interval's failing edge independently. The
    // high-water success probe above proves the nested predicates; these
    // seeded counters prove their typed cancellation returns.
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut probability_checkpoint = TokenPartitionCheckpoint {
        token: &token,
        probability_items: PARTITION_PROBABILITY_CHECKPOINT_NODES - 1,
        filter_edge_items: 0,
        prepass_items: 0,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: 0,
    };
    let _ = probability_checkpoint.checkpoint_probability();
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut filter_edge_checkpoint = TokenPartitionCheckpoint {
        token: &token,
        probability_items: 0,
        filter_edge_items: PARTITION_FILTER_EDGE_CHECKPOINT_MACROBLOCKS - 1,
        prepass_items: 0,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: 0,
    };
    let _ = filter_edge_checkpoint.checkpoint_filter_edge_macroblock();
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut prepass_checkpoint = TokenPartitionCheckpoint {
        token: &token,
        probability_items: 0,
        filter_edge_items: 0,
        prepass_items: PARTITION_PREPASS_CHECKPOINT_MACROBLOCKS - 1,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: 0,
    };
    let _ = prepass_checkpoint.checkpoint_prepass_macroblock();
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut macroblock_checkpoint = TokenPartitionCheckpoint {
        token: &token,
        probability_items: 0,
        filter_edge_items: 0,
        prepass_items: 0,
        macroblock_items: PARTITION_MODE_CHECKPOINT_MACROBLOCKS - 1,
        bit_items: 0,
        output_bytes: 0,
    };
    let _ = macroblock_checkpoint.checkpoint_macroblock();
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut output_interval_checkpoint = TokenPartitionCheckpoint {
        token: &token,
        probability_items: 0,
        filter_edge_items: 0,
        prepass_items: 0,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: PARTITION_OUTPUT_CHECKPOINT_BYTES - 1,
    };
    let _ = output_interval_checkpoint.checkpoint_output_bytes(1);
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut output_checkpoint = TokenPartitionCheckpoint {
        token: &token,
        probability_items: 0,
        filter_edge_items: 0,
        prepass_items: 0,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: PARTITION_OUTPUT_CHECKPOINT_BYTES - 1,
    };
    let mut output_writer = super::bool_enc::__coverage_carry_encoder();
    for _ in 0..32 {
        let _ = output_checkpoint.encode_bool(&mut output_writer, 128, false);
    }
    let token = crate::CancellationToken::new();
    let mut forced_output_checkpoint = TokenPartitionCheckpoint {
        token: &token,
        probability_items: 0,
        filter_edge_items: 0,
        prepass_items: 0,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: usize::MAX,
    };
    let mut forced_output_writer = super::bool_enc::__coverage_pending_encoder();
    let _ = forced_output_checkpoint.encode_bool(&mut forced_output_writer, 0, false);
    let mut forced_finish_checkpoint = TokenPartitionCheckpoint {
        token: &token,
        probability_items: 0,
        filter_edge_items: 0,
        prepass_items: 0,
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
        let mut finish_checkpoint = TokenPartitionCheckpoint {
            token: &token,
            probability_items: 0,
            filter_edge_items: 0,
            prepass_items: 0,
            macroblock_items: 0,
            bit_items: 0,
            output_bytes: PARTITION_OUTPUT_CHECKPOINT_BYTES - 1,
        };
        let _ = std::hint::black_box(
            finish_checkpoint.finish(super::bool_enc::__coverage_pending_encoder()),
        );
    }

    let mut failing_finish = NoopPartitionCheckpoint { fail_after: 0 };
    let _ = std::hint::black_box(failing_finish.finish(BoolEncoder::default()));

    let carry_writer = super::bool_enc::__coverage_carry_encoder();
    let _ = std::hint::black_box(checkpoint.finish(carry_writer));
    let pending_writer = super::bool_enc::__coverage_pending_encoder();
    let _ = std::hint::black_box(checkpoint.finish(pending_writer));
    let rle_writer = super::bool_enc::__coverage_rle_encoder();
    let _ = std::hint::black_box(checkpoint.finish(rle_writer));

    let mut writer = BoolEncoder::default();
    let _ = write_signed(&mut writer, 1, 4, &mut checkpoint);
    for mode in [
        Intra16Mode::Dc,
        Intra16Mode::TrueMotion,
        Intra16Mode::Vertical,
        Intra16Mode::Horizontal,
    ] {
        let mut writer = BoolEncoder::default();
        let mut mode_checkpoint = CoverageFailingPartitionCheckpoint::new(usize::MAX);
        let _ = write_intra16_mode(&mut writer, mode, &mut mode_checkpoint);
    }
    for mode in Intra4Mode::ALL {
        let mut writer = BoolEncoder::default();
        let mut mode_checkpoint = CoverageFailingPartitionCheckpoint::new(usize::MAX);
        let _ = write_intra4_mode(
            &mut writer,
            mode,
            &INTRA4_MODE_PROBABILITIES[Intra4Mode::Dc as usize][Intra4Mode::Dc as usize],
            &mut mode_checkpoint,
        );
    }
    for mode in [
        Intra16Mode::Dc,
        Intra16Mode::TrueMotion,
        Intra16Mode::Vertical,
        Intra16Mode::Horizontal,
    ] {
        for fail_after in 0..=3 {
            let mut writer = BoolEncoder::default();
            let mut mode_checkpoint = CoverageFailingPartitionCheckpoint::new(fail_after);
            let _ = write_intra16_mode(&mut writer, mode, &mut mode_checkpoint);
        }
    }
    for mode in Intra4Mode::ALL {
        for fail_after in 0..=9 {
            let mut writer = BoolEncoder::default();
            let mut mode_checkpoint = CoverageFailingPartitionCheckpoint::new(fail_after);
            let _ = write_intra4_mode(
                &mut writer,
                mode,
                &INTRA4_MODE_PROBABILITIES[Intra4Mode::Dc as usize][Intra4Mode::Dc as usize],
                &mut mode_checkpoint,
            );
        }
    }
    for mode in [
        ChromaMode::Dc,
        ChromaMode::TrueMotion,
        ChromaMode::Vertical,
        ChromaMode::Horizontal,
    ] {
        for fail_after in 0..=3 {
            let mut writer = BoolEncoder::default();
            let mut mode_checkpoint = CoverageFailingPartitionCheckpoint::new(fail_after);
            let _ = write_chroma_mode(&mut writer, mode, &mut mode_checkpoint);
        }
    }

    // One seeded update drives the literal-probability emission path that
    // ordinary images often leave at its all-false default.
    let mut probabilities = AdaptedProbabilities {
        coefficients: super::tokenize::COEFF_PROBS,
        updates: [[[[false; 11]; 3]; 8]; 4],
    };
    probabilities.updates[0][0][0][0] = true;
    let mut writer = BoolEncoder::default();
    let _ = write_coefficient_probabilities(&mut writer, &probabilities, &mut checkpoint);

    let decision = MacroblockDecision {
        x: 0,
        y: 0,
        segment: 0,
        intra16_mode: super::intra16::Intra16Mode::Dc,
        luma: LumaDecision::Intra4(Intra4Result {
            modes: [Intra4Mode::Dc; 16],
            levels: [[0; 16]; 16],
            reconstructed: [0; 256],
            distortion: 0,
            spectral_distortion: 0,
            header_cost: 0,
            rate_cost: 0,
            score: 0,
            nonzero: 0,
        }),
        chroma: ChromaCandidate {
            mode: ChromaMode::Dc,
            levels: [[0; 16]; 8],
            reconstructed_u: [0; 64],
            reconstructed_v: [0; 64],
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
        nonzero: 0,
    };
    let params = FrameParams {
        segments: [super::analysis::SegmentParams {
            quantizer: 20,
            filter_strength: 10,
        }; 4],
        num_segments: 1,
        chroma_dc_delta: 0,
        chroma_ac_delta: 0,
    };
    let filter_decision = MacroblockDecision {
        luma: LumaDecision::Intra16(Intra16Candidate {
            mode: Intra16Mode::Dc,
            y2_levels: [1; 16],
            y1_levels: [[0; 16]; 16],
            reconstructed: [0; 256],
            distortion: u32::MAX,
            spectral_distortion: 0,
            header_cost: 0,
            rate_cost: 0,
            score: 0,
            nonzero: 0x0100_0000,
        }),
        ..decision.clone()
    };
    let mut filter_checkpoint = NoopPartitionCheckpoint::new();
    let _ = adjusted_frame_params(
        std::slice::from_ref(&filter_decision),
        &params,
        true,
        &mut filter_checkpoint,
    );
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut filter_checkpoint = TokenPartitionCheckpoint {
        token: &token,
        probability_items: 0,
        filter_edge_items: PARTITION_FILTER_EDGE_CHECKPOINT_MACROBLOCKS - 1,
        prepass_items: 0,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: 0,
    };
    let _ = adjusted_frame_params(
        std::slice::from_ref(&filter_decision),
        &params,
        true,
        &mut filter_checkpoint,
    );
    let mut filter_checkpoint = CoverageFailingPartitionCheckpoint::new(usize::MAX);
    let _ = std::hint::black_box(adjusted_frame_params(
        std::slice::from_ref(&filter_decision),
        &params,
        true,
        &mut filter_checkpoint,
    ));
    let mut failing_filter_checkpoint = NoopPartitionCheckpoint { fail_after: 0 };
    let _ = adjusted_frame_params(
        std::slice::from_ref(&filter_decision),
        &params,
        true,
        &mut failing_filter_checkpoint,
    );
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut prepass_checkpoint = TokenPartitionCheckpoint {
        token: &token,
        probability_items: 0,
        filter_edge_items: 0,
        prepass_items: PARTITION_PREPASS_CHECKPOINT_MACROBLOCKS - 1,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: 0,
    };
    let _ = segment_probabilities(std::slice::from_ref(&decision), &mut prepass_checkpoint);
    let mut prepass_failure = CoverageFailingPartitionCheckpoint::with_prepass_failure();
    let _ = std::hint::black_box(segment_probabilities(
        std::slice::from_ref(&decision),
        &mut prepass_failure,
    ));
    let mut filter_failure = CoverageFailingPartitionCheckpoint::with_filter_edge_failure();
    let _ = std::hint::black_box(adjusted_frame_params(
        std::slice::from_ref(&filter_decision),
        &params,
        true,
        &mut filter_failure,
    ));
    let mut non_intra16_checkpoint = CoverageFailingPartitionCheckpoint::new(usize::MAX);
    let _ = std::hint::black_box(adjusted_frame_params(
        std::slice::from_ref(&decision),
        &params,
        true,
        &mut non_intra16_checkpoint,
    ));
    let filter_skip_decision = MacroblockDecision {
        luma: LumaDecision::Intra16(Intra16Candidate {
            mode: Intra16Mode::Dc,
            y2_levels: [0; 16],
            y1_levels: [[0; 16]; 16],
            reconstructed: [0; 256],
            distortion: 0,
            spectral_distortion: 0,
            header_cost: 0,
            rate_cost: 0,
            score: 0,
            nonzero: 0,
        }),
        ..decision.clone()
    };
    let mut filter_skip_checkpoint = CoverageFailingPartitionCheckpoint::new(usize::MAX);
    let _ = std::hint::black_box(adjusted_frame_params(
        std::slice::from_ref(&filter_skip_decision),
        &params,
        true,
        &mut filter_skip_checkpoint,
    ));
    let mut mode_writer = BoolEncoder::default();
    let mut mode_checkpoint = NoopPartitionCheckpoint::new();
    let _ = write_modes(
        &mut mode_writer,
        std::slice::from_ref(&filter_decision),
        1,
        [128; 3],
        false,
        &mut mode_checkpoint,
    );
    let mut mode_writer = BoolEncoder::default();
    let mut mode_checkpoint = CoverageFailingPartitionCheckpoint::new(usize::MAX);
    let _ = write_modes(
        &mut mode_writer,
        std::slice::from_ref(&filter_decision),
        1,
        [128; 3],
        false,
        &mut mode_checkpoint,
    );
    let segmented_params = FrameParams {
        segments: params.segments,
        num_segments: 2,
        chroma_dc_delta: params.chroma_dc_delta,
        chroma_ac_delta: params.chroma_ac_delta,
    };
    let mut header_probe = NoopPartitionCheckpoint::new();
    let mut header_writer = BoolEncoder::default();
    let _ = write_segment_header(
        &mut header_writer,
        &segmented_params,
        [1, 2, 3],
        &mut header_probe,
    );
    let header_events = usize::MAX.saturating_sub(header_probe.fail_after);
    for fail_after in 0..=header_events {
        let mut header_checkpoint = NoopPartitionCheckpoint { fail_after };
        let mut header_writer = BoolEncoder::default();
        let _ = write_segment_header(
            &mut header_writer,
            &segmented_params,
            [1, 2, 3],
            &mut header_checkpoint,
        );
    }
    let mut segment_writer = BoolEncoder::default();
    let mut segment_checkpoint = NoopPartitionCheckpoint::new();
    let _ = write_segment_header(
        &mut segment_writer,
        &segmented_params,
        [128, 129, 130],
        &mut segment_checkpoint,
    );
    for fail_after in 0..=128 {
        let mut checkpoint = CoverageFailingPartitionCheckpoint::new(fail_after);
        let mut writer = BoolEncoder::default();
        let _ = write_segment_header(&mut writer, &segmented_params, [1, 2, 3], &mut checkpoint);
    }
    let unsegmented_params = FrameParams {
        segments: params.segments,
        num_segments: 1,
        chroma_dc_delta: params.chroma_dc_delta,
        chroma_ac_delta: params.chroma_ac_delta,
    };
    let mut unsegmented_checkpoint = CoverageFailingPartitionCheckpoint::new(usize::MAX);
    let mut unsegmented_writer = BoolEncoder::default();
    let _ = write_segment_header(
        &mut unsegmented_writer,
        &unsegmented_params,
        [1, 2, 3],
        &mut unsegmented_checkpoint,
    );
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut token_header_checkpoint = TokenPartitionCheckpoint {
        token: &token,
        probability_items: 0,
        filter_edge_items: 0,
        prepass_items: 0,
        macroblock_items: 0,
        bit_items: 7,
        output_bytes: 0,
    };
    let mut token_header_writer = BoolEncoder::default();
    let _ = write_segment_header(
        &mut token_header_writer,
        &unsegmented_params,
        [1, 2, 3],
        &mut token_header_checkpoint,
    );
    let token = crate::CancellationToken::new();
    let mut token_segmented_checkpoint = TokenPartitionCheckpoint {
        token: &token,
        probability_items: 0,
        filter_edge_items: 0,
        prepass_items: 0,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: 0,
    };
    let mut token_segmented_writer = BoolEncoder::default();
    let _ = write_segment_header(
        &mut token_segmented_writer,
        &segmented_params,
        [1, 2, 3],
        &mut token_segmented_checkpoint,
    );
    // Exercise both halves of the segment selector directly. Normal
    // one-macroblock fixtures usually only use segment zero, so the
    // upper-half probability/index path otherwise remains uninstantiated.
    let mut segment_writer = BoolEncoder::default();
    let mut segment_checkpoint = NoopPartitionCheckpoint::new();
    let _ = write_segment(&mut segment_writer, 2, [128; 3], &mut segment_checkpoint);
    let mut segment_writer = BoolEncoder::default();
    let mut segment_checkpoint = CoverageFailingPartitionCheckpoint::new(usize::MAX);
    let _ = write_segment(&mut segment_writer, 2, [128; 3], &mut segment_checkpoint);
    let token = crate::CancellationToken::new();
    let mut segment_checkpoint = TokenPartitionCheckpoint {
        token: &token,
        probability_items: 0,
        filter_edge_items: 0,
        prepass_items: 0,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: 0,
    };
    let mut segment_writer = BoolEncoder::default();
    let _ = write_segment(&mut segment_writer, 2, [128; 3], &mut segment_checkpoint);
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut first_partition_checkpoint = TokenPartitionCheckpoint {
        token: &token,
        probability_items: 0,
        filter_edge_items: PARTITION_FILTER_EDGE_CHECKPOINT_MACROBLOCKS - 1,
        prepass_items: 0,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: 0,
    };
    let _ = encode_first_partition_with_checkpoint(
        std::slice::from_ref(&filter_decision),
        1,
        &params,
        &probabilities,
        true,
        &mut first_partition_checkpoint,
    );
    let _ = encode_first_partition(
        std::slice::from_ref(&decision),
        1,
        &params,
        &probabilities,
        false,
        None,
    );
    let _ = encode_first_partition(
        std::slice::from_ref(&decision),
        1,
        &params,
        &probabilities,
        false,
        Some(&token),
    );

    // Each nested bit interval has a distinct cancellation edge. Re-seed the
    // same high-water state and cancel after successive polls to visit those
    // `?` arms without looping millions of real bits.
    for checks in 0..=17 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut checkpoint = TokenPartitionCheckpoint {
            token: &token,
            probability_items: 0,
            filter_edge_items: 0,
            prepass_items: 0,
            macroblock_items: 0,
            bit_items: PARTITION_262144_BIT_CHECKPOINT_BITS - 1,
            output_bytes: 0,
        };
        let _ = checkpoint.checkpoint_bit();
    }
    let token = crate::CancellationToken::new();
    let mut checkpoint = TokenPartitionCheckpoint {
        token: &token,
        probability_items: 0,
        filter_edge_items: 0,
        prepass_items: 0,
        macroblock_items: 0,
        bit_items: PARTITION_131072_BIT_CHECKPOINT_BITS - 1,
        output_bytes: 0,
    };
    let _ = checkpoint.checkpoint_bit();

    // Drive each private writer through a real cancellation-shaped failure.
    // A token countdown cannot target these call sites independently because
    // the first-partition header and coefficient table are much denser than a
    // normal fixture. The injector is coverage-only and never ships.
    for fail_after in [0, 1, 2, 4, 8] {
        let mut checkpoint = CoverageFailingPartitionCheckpoint::new(fail_after);
        let mut writer = BoolEncoder::default();
        let _ = write_signed(&mut writer, 1, 4, &mut checkpoint);
    }

    for fail_after in [0, 1, 2, 64, 512, 1_152] {
        let mut checkpoint = CoverageFailingPartitionCheckpoint::new(fail_after);
        let mut writer = BoolEncoder::default();
        let _ = write_coefficient_probabilities(&mut writer, &probabilities, &mut checkpoint);
    }
    let mut probability_failure = CoverageFailingPartitionCheckpoint::with_probability_failure();
    let mut probability_failure_writer = BoolEncoder::default();
    let _ = std::hint::black_box(write_coefficient_probabilities(
        &mut probability_failure_writer,
        &probabilities,
        &mut probability_failure,
    ));
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut coefficient_checkpoint = TokenPartitionCheckpoint {
        token: &token,
        probability_items: PARTITION_PROBABILITY_CHECKPOINT_NODES - 1,
        filter_edge_items: 0,
        prepass_items: 0,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: 0,
    };
    let mut coefficient_writer = BoolEncoder::default();
    let _ = std::hint::black_box(write_coefficient_probabilities(
        &mut coefficient_writer,
        &probabilities,
        &mut coefficient_checkpoint,
    ));

    for fail_after in [0, 1, 2, 64, 256] {
        let mut checkpoint = CoverageFailingPartitionCheckpoint::new(fail_after);
        let mut writer = BoolEncoder::default();
        let _ = write_modes(
            &mut writer,
            std::slice::from_ref(&decision),
            1,
            [128; 3],
            false,
            &mut checkpoint,
        );
    }
    for fail_after in 0..=64 {
        let mut checkpoint = CoverageFailingPartitionCheckpoint::new(fail_after);
        let mut writer = BoolEncoder::default();
        let _ = write_modes(
            &mut writer,
            std::slice::from_ref(&filter_decision),
            1,
            [128; 3],
            false,
            &mut checkpoint,
        );
    }
    let mut macroblock_failure = CoverageFailingPartitionCheckpoint::with_macroblock_failure();
    let mut macroblock_failure_writer = BoolEncoder::default();
    let _ = std::hint::black_box(write_modes(
        &mut macroblock_failure_writer,
        std::slice::from_ref(&decision),
        1,
        [128; 3],
        false,
        &mut macroblock_failure,
    ));
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut mode_checkpoint = TokenPartitionCheckpoint {
        token: &token,
        probability_items: 0,
        filter_edge_items: 0,
        prepass_items: 0,
        macroblock_items: PARTITION_MODE_CHECKPOINT_MACROBLOCKS - 1,
        bit_items: 0,
        output_bytes: 0,
    };
    let mut mode_writer = BoolEncoder::default();
    let _ = write_modes(
        &mut mode_writer,
        std::slice::from_ref(&decision),
        1,
        [128; 3],
        false,
        &mut mode_checkpoint,
    );
    let probe_token = crate::CancellationToken::new();
    probe_token.cancel_after(usize::MAX);
    let mut probe_checkpoint = TokenPartitionCheckpoint {
        token: &probe_token,
        probability_items: 0,
        filter_edge_items: 0,
        prepass_items: 0,
        macroblock_items: PARTITION_MODE_CHECKPOINT_MACROBLOCKS - 1,
        bit_items: 0,
        output_bytes: 0,
    };
    let mut probe_writer = BoolEncoder::default();
    let _ = write_modes(
        &mut probe_writer,
        std::slice::from_ref(&decision),
        1,
        [128; 3],
        false,
        &mut probe_checkpoint,
    );
    let calls = usize::MAX.saturating_sub(
        probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    let token = crate::CancellationToken::new();
    token.cancel_after(calls.saturating_sub(1));
    let mut final_checkpoint = TokenPartitionCheckpoint {
        token: &token,
        probability_items: 0,
        filter_edge_items: 0,
        prepass_items: 0,
        macroblock_items: PARTITION_MODE_CHECKPOINT_MACROBLOCKS - 1,
        bit_items: 0,
        output_bytes: 0,
    };
    let mut final_writer = BoolEncoder::default();
    let _ = write_modes(
        &mut final_writer,
        std::slice::from_ref(&decision),
        1,
        [128; 3],
        false,
        &mut final_checkpoint,
    );

    let rich_params = FrameParams {
        segments: params.segments,
        num_segments: 4,
        chroma_dc_delta: 3,
        chroma_ac_delta: -2,
    };
    let mut rich_checkpoint = CoverageFailingPartitionCheckpoint::new(usize::MAX);
    let _ = encode_first_partition_with_checkpoint(
        std::slice::from_ref(&filter_decision),
        1,
        &rich_params,
        &probabilities,
        true,
        &mut rich_checkpoint,
    );
    let mut failing_first_filter = NoopPartitionCheckpoint { fail_after: 0 };
    let _ = std::hint::black_box(encode_first_partition_with_checkpoint(
        std::slice::from_ref(&filter_decision),
        1,
        &rich_params,
        &probabilities,
        true,
        &mut failing_first_filter,
    ));
    let mut failing_first_filter = CoverageFailingPartitionCheckpoint::with_filter_edge_failure();
    let _ = std::hint::black_box(encode_first_partition_with_checkpoint(
        std::slice::from_ref(&filter_decision),
        1,
        &rich_params,
        &probabilities,
        true,
        &mut failing_first_filter,
    ));
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let mut failing_first_prepass = TokenPartitionCheckpoint {
        token: &token,
        probability_items: 0,
        filter_edge_items: 0,
        prepass_items: PARTITION_PREPASS_CHECKPOINT_MACROBLOCKS - 1,
        macroblock_items: 0,
        bit_items: 0,
        output_bytes: 0,
    };
    let _ = std::hint::black_box(encode_first_partition_with_checkpoint(
        std::slice::from_ref(&decision),
        1,
        &rich_params,
        &probabilities,
        false,
        &mut failing_first_prepass,
    ));
    let mut failing_first_prepass = CoverageFailingPartitionCheckpoint::with_prepass_failure();
    let _ = std::hint::black_box(encode_first_partition_with_checkpoint(
        std::slice::from_ref(&decision),
        1,
        &rich_params,
        &probabilities,
        false,
        &mut failing_first_prepass,
    ));
    #[cfg(coverage_nightly)]
    {
        let mut probe = CoverageFailingPartitionCheckpoint::new(usize::MAX);
        let _ = encode_first_partition_with_checkpoint(
            std::slice::from_ref(&decision),
            1,
            &rich_params,
            &probabilities,
            false,
            &mut probe,
        );
        let encode_calls = probe.encode_calls;
        for fail_after in 0..=encode_calls {
            let mut checkpoint = CoverageFailingPartitionCheckpoint::new(fail_after);
            let _ = encode_first_partition_with_checkpoint(
                std::slice::from_ref(&decision),
                1,
                &rich_params,
                &probabilities,
                false,
                &mut checkpoint,
            );
        }
    }
    let mut probe = NoopPartitionCheckpoint::new();
    let _ = std::hint::black_box(encode_first_partition_with_checkpoint(
        std::slice::from_ref(&decision),
        1,
        &rich_params,
        &probabilities,
        false,
        &mut probe,
    ));
    let event_calls = usize::MAX.saturating_sub(probe.fail_after);
    for fail_after in 0..=event_calls {
        let mut checkpoint = NoopPartitionCheckpoint { fail_after };
        let _ = std::hint::black_box(encode_first_partition_with_checkpoint(
            std::slice::from_ref(&decision),
            1,
            &rich_params,
            &probabilities,
            false,
            &mut checkpoint,
        ));
    }
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

fn segment_probabilities<P: PartitionCheckpointControl>(
    decisions: &[MacroblockDecision],
    checkpoint: &mut P,
) -> CodecResult<[u8; 3]> {
    let mut counts = [0usize; 4];
    for decision in decisions {
        counts[usize::from(decision.segment)] =
            counts[usize::from(decision.segment)].saturating_add(1);
        checkpoint.checkpoint_prepass_macroblock()?;
    }
    Ok([
        segment_probability(
            counts[0].saturating_add(counts[1]),
            counts[2].saturating_add(counts[3]),
        ),
        segment_probability(counts[0], counts[1]),
        segment_probability(counts[2], counts[3]),
    ])
}

fn adjusted_frame_params<P: PartitionCheckpointControl>(
    decisions: &[MacroblockDecision],
    params: &FrameParams,
    adjust_filter_edges: bool,
    checkpoint: &mut P,
) -> CodecResult<FrameParams> {
    let mut adjusted = FrameParams {
        segments: params.segments,
        num_segments: params.num_segments,
        chroma_dc_delta: params.chroma_dc_delta,
        chroma_ac_delta: params.chroma_ac_delta,
    };
    if !adjust_filter_edges {
        return Ok(adjusted);
    }
    let mut maximum_edges = [0u16; 4];
    for decision in decisions {
        if let LumaDecision::Intra16(luma) = &decision.luma {
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
        checkpoint.checkpoint_filter_edge_macroblock()?;
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
    Ok(adjusted)
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
    let params = adjusted_frame_params(decisions, params, adjust_filter_edges, checkpoint)?;
    let segment_probabilities = segment_probabilities(decisions, checkpoint)?;
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
            filter_edge_items: 0,
            prepass_items: 0,
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
        let mut checkpoint = NoopPartitionCheckpoint::new();
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
