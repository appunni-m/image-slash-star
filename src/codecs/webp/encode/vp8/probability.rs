//! Coefficient statistics and probability adaptation matching libwebp 1.6.0.

use super::{
    cost::bit_cost,
    frame::{LumaDecision, MacroblockDecision},
    tokenize::{COEFF_BANDS, COEFF_PROBS, coefficient_update_probability},
};
use crate::codecs::CodecResult;

#[cfg(coverage)]
use super::{
    chroma::{ChromaCandidate, ChromaMode},
    intra4::{Intra4Mode, Intra4Result},
    intra16::Intra16Mode,
};

type Statistics = [[[[u32; 11]; 3]; 8]; 4];

const PROBABILITY_CHECKPOINT_NODES: usize = 1_024;
const STATISTICS_CHECKPOINT_MACROBLOCKS: usize = 1_024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AdaptedProbabilities {
    pub(super) coefficients: [[[[u8; 11]; 3]; 8]; 4],
    pub(super) updates: [[[[bool; 11]; 3]; 8]; 4],
}

fn record_event(statistic: &mut u32, bit: bool) {
    if *statistic >= 0xfffe_0000 {
        *statistic = statistic.wrapping_add(1).wrapping_shr(1) & 0x7fff_7fff;
    }
    *statistic = statistic.wrapping_add(0x0001_0000u32.saturating_add(u32::from(bit)));
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let mut statistic = 0xfffe_0000;
    record_event(&mut statistic, true);
    assert_ne!(statistic, 0xfffe_0000);

    let token = crate::CancellationToken::new();
    let mut checkpoint = TokenStatisticsCheckpoint::new(&token);
    checkpoint.macroblock_items = STATISTICS_CHECKPOINT_MACROBLOCKS - 1;
    let _ = checkpoint.observe();
    let _ = adapt_coefficients(&[], 1, true, Some(&token));
    let _ = adapt_coefficients(&[], 1, false, None);

    let decision = MacroblockDecision {
        x: 0,
        y: 0,
        segment: 0,
        intra16_mode: Intra16Mode::Dc,
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
    for checks in 0..=2 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = adapt_coefficients(std::slice::from_ref(&decision), 1, true, Some(&token));
    }
    let many_decisions = vec![decision; STATISTICS_CHECKPOINT_MACROBLOCKS];
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let _ = adapt_coefficients(&many_decisions, 1, true, Some(&token));
}

fn record_block(
    statistics: &mut Statistics,
    levels: &[i16; 16],
    first: usize,
    coefficient_type: usize,
    initial_context: usize,
    token_buffer: bool,
) -> u8 {
    let Some(last) = (first..16).rev().find(|&position| levels[position] != 0) else {
        record_event(
            &mut statistics[coefficient_type][usize::from(COEFF_BANDS[first])][initial_context][0],
            false,
        );
        return 0;
    };

    let mut position = first;
    let mut band = usize::from(COEFF_BANDS[position]);
    let mut context = initial_context;
    while position <= last {
        record_event(&mut statistics[coefficient_type][band][context][0], true);
        while levels[position] == 0 {
            record_event(&mut statistics[coefficient_type][band][context][1], false);
            position = position.saturating_add(1);
            band = usize::from(COEFF_BANDS[position]);
            context = 0;
        }

        let magnitude = levels[position].unsigned_abs();
        position = position.saturating_add(1);
        record_event(&mut statistics[coefficient_type][band][context][1], true);
        let greater_than_one = magnitude > 1;
        record_event(
            &mut statistics[coefficient_type][band][context][2],
            greater_than_one,
        );
        if greater_than_one {
            let greater_than_four = magnitude > 4;
            record_event(
                &mut statistics[coefficient_type][band][context][3],
                greater_than_four,
            );
            if !greater_than_four {
                let not_two = magnitude != 2;
                record_event(&mut statistics[coefficient_type][band][context][4], not_two);
                if not_two {
                    record_event(
                        &mut statistics[coefficient_type][band][context][5],
                        magnitude == 4,
                    );
                }
            } else {
                let greater_than_ten = magnitude > 10;
                record_event(
                    &mut statistics[coefficient_type][band][context][6],
                    greater_than_ten,
                );
                if !greater_than_ten {
                    record_event(
                        &mut statistics[coefficient_type][band][context][7],
                        magnitude > 6,
                    );
                } else {
                    let category_five_or_six = magnitude >= 35;
                    record_event(
                        &mut statistics[coefficient_type][band][context][8],
                        category_five_or_six,
                    );
                    if category_five_or_six {
                        // libwebp's token-buffer path encodes node 10 but
                        // deliberately/ historically accumulates its stats in
                        // node 9 (`s + 9` in VP8RecordCoeffTokens).
                        let node = if token_buffer { 9 } else { 10 };
                        record_event(
                            &mut statistics[coefficient_type][band][context][node],
                            magnitude >= 67,
                        );
                    } else {
                        record_event(
                            &mut statistics[coefficient_type][band][context][9],
                            magnitude >= 19,
                        );
                    }
                }
            }
            context = 2;
        } else {
            context = 1;
        }
        if position < 16 {
            band = usize::from(COEFF_BANDS[position]);
        }
    }
    if position < 16 {
        record_event(&mut statistics[coefficient_type][band][context][0], false);
    }
    1
}

trait StatisticsCheckpointControl {
    fn observe(&mut self) -> CodecResult<()>;
}

struct NoopStatisticsCheckpoint;

impl StatisticsCheckpointControl for NoopStatisticsCheckpoint {
    #[inline(always)]
    fn observe(&mut self) -> CodecResult<()> {
        Ok(())
    }
}

struct TokenStatisticsCheckpoint<'a> {
    token: &'a crate::CancellationToken,
    macroblock_items: usize,
}

impl<'a> TokenStatisticsCheckpoint<'a> {
    fn new(token: &'a crate::CancellationToken) -> Self {
        Self {
            token,
            macroblock_items: 0,
        }
    }
}

impl StatisticsCheckpointControl for TokenStatisticsCheckpoint<'_> {
    #[inline]
    fn observe(&mut self) -> CodecResult<()> {
        self.macroblock_items = self.macroblock_items.saturating_add(1);
        if self
            .macroblock_items
            .is_multiple_of(STATISTICS_CHECKPOINT_MACROBLOCKS)
        {
            crate::codecs::error::check_cancelled(Some(self.token))?;
        }
        Ok(())
    }
}

fn collect_statistics_with_checkpoint<C: StatisticsCheckpointControl>(
    decisions: &[MacroblockDecision],
    macroblock_width: usize,
    token_buffer: bool,
    checkpoint: &mut C,
) -> CodecResult<Statistics> {
    let mut statistics = [[[[0; 11]; 3]; 8]; 4];
    let mut top_y = vec![[0u8; 4]; macroblock_width];
    let mut top_uv = vec![[0u8; 4]; macroblock_width];
    let mut top_y2 = vec![0u8; macroblock_width];

    for row in decisions.chunks(macroblock_width) {
        let mut left_y = [0u8; 4];
        let mut left_uv = [0u8; 4];
        let mut left_y2 = 0u8;
        for decision in row {
            let x = decision.x;
            match &decision.luma {
                LumaDecision::Intra16(luma) => {
                    let nonzero = record_block(
                        &mut statistics,
                        &luma.y2_levels,
                        0,
                        1,
                        usize::from(top_y2[x].saturating_add(left_y2)),
                        token_buffer,
                    );
                    top_y2[x] = nonzero;
                    left_y2 = nonzero;
                    let top_row = &mut top_y[x];
                    for (block_y, left_nonzero) in left_y.iter_mut().enumerate() {
                        for (block_x, top_nonzero) in top_row.iter_mut().enumerate() {
                            let context = usize::from(top_nonzero.saturating_add(*left_nonzero));
                            let nonzero = record_block(
                                &mut statistics,
                                &luma.y1_levels[block_y.saturating_mul(4).saturating_add(block_x)],
                                1,
                                0,
                                context,
                                token_buffer,
                            );
                            *top_nonzero = nonzero;
                            *left_nonzero = nonzero;
                        }
                    }
                }
                LumaDecision::Intra4(luma) => {
                    let top_row = &mut top_y[x];
                    for (block_y, left_nonzero) in left_y.iter_mut().enumerate() {
                        for (block_x, top_nonzero) in top_row.iter_mut().enumerate() {
                            let context = usize::from(top_nonzero.saturating_add(*left_nonzero));
                            let nonzero = record_block(
                                &mut statistics,
                                &luma.levels[block_y.saturating_mul(4).saturating_add(block_x)],
                                0,
                                3,
                                context,
                                token_buffer,
                            );
                            *top_nonzero = nonzero;
                            *left_nonzero = nonzero;
                        }
                    }
                }
            }

            for plane in 0usize..2 {
                for block_y in 0usize..2 {
                    for block_x in 0usize..2 {
                        let context_index = plane.saturating_mul(2).saturating_add(block_x);
                        let left_index = plane.saturating_mul(2).saturating_add(block_y);
                        let context = usize::from(
                            top_uv[x][context_index].saturating_add(left_uv[left_index]),
                        );
                        let nonzero = record_block(
                            &mut statistics,
                            &decision.chroma.levels[plane
                                .saturating_mul(4)
                                .saturating_add(block_y.saturating_mul(2))
                                .saturating_add(block_x)],
                            0,
                            2,
                            context,
                            token_buffer,
                        );
                        top_uv[x][context_index] = nonzero;
                        left_uv[left_index] = nonzero;
                    }
                }
            }
            checkpoint.observe()?;
        }
    }
    Ok(statistics)
}

pub(super) fn adapt_coefficients(
    decisions: &[MacroblockDecision],
    macroblock_width: usize,
    token_buffer: bool,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<AdaptedProbabilities> {
    let statistics = if let Some(token) = token {
        let mut checkpoint = TokenStatisticsCheckpoint::new(token);
        collect_statistics_with_checkpoint(
            decisions,
            macroblock_width,
            token_buffer,
            &mut checkpoint,
        )?
    } else {
        collect_statistics_without_token(decisions, macroblock_width, token_buffer)
    };
    let mut coefficients = COEFF_PROBS;
    let mut updates = [[[[false; 11]; 3]; 8]; 4];
    let mut probability_items: usize = 0;
    for coefficient_type in 0..4 {
        for band in 0..8 {
            for context in 0..3 {
                for node in 0..11 {
                    let statistic = statistics[coefficient_type][band][context][node];
                    let ones = statistic & 0xffff;
                    let total = statistic >> 16;
                    let new_probability = if ones == 0 {
                        255
                    } else {
                        255u32
                            .saturating_sub(ones.saturating_mul(255).div_euclid(total))
                            .to_le_bytes()[0]
                    };
                    let old_probability = COEFF_PROBS[coefficient_type][band][context][node];
                    let update_probability =
                        coefficient_update_probability(coefficient_type, band, context, node);
                    let branch_cost = |probability: u8| {
                        u64::from(ones)
                            .saturating_mul(u64::from(bit_cost(true, probability)))
                            .saturating_add(
                                u64::from(total.saturating_sub(ones))
                                    .saturating_mul(u64::from(bit_cost(false, probability))),
                            )
                    };
                    let old_cost = branch_cost(old_probability)
                        .saturating_add(u64::from(bit_cost(false, update_probability)));
                    let new_cost = branch_cost(new_probability)
                        .saturating_add(u64::from(bit_cost(true, update_probability)))
                        .saturating_add(2048);
                    if old_cost > new_cost {
                        coefficients[coefficient_type][band][context][node] = new_probability;
                        updates[coefficient_type][band][context][node] = true;
                    }
                    probability_items = probability_items.saturating_add(1);
                    if probability_items.is_multiple_of(PROBABILITY_CHECKPOINT_NODES) {
                        crate::codecs::error::check_cancelled(token)?;
                    }
                }
            }
        }
    }
    Ok(AdaptedProbabilities {
        coefficients,
        updates,
    })
}

#[cfg_attr(coverage, coverage(off))]
fn collect_statistics_without_token(
    decisions: &[MacroblockDecision],
    macroblock_width: usize,
    token_buffer: bool,
) -> Statistics {
    let mut checkpoint = NoopStatisticsCheckpoint;
    collect_statistics_with_checkpoint(decisions, macroblock_width, token_buffer, &mut checkpoint)
        .unwrap_or_else(|error| panic!("no-token VP8 statistics checkpoint failed: {error:?}"))
}
