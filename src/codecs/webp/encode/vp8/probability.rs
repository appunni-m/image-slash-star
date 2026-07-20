//! Coefficient statistics and probability adaptation matching libwebp 1.6.0.

use super::{
    cost::bit_cost,
    frame::{LumaDecision, MacroblockDecision},
    tokenize::{COEFF_BANDS, COEFF_PROBS, coefficient_update_probability},
};

type Statistics = [[[[u32; 11]; 3]; 8]; 4];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AdaptedProbabilities {
    pub(super) coefficients: [[[[u8; 11]; 3]; 8]; 4],
    pub(super) updates: [[[[bool; 11]; 3]; 8]; 4],
}

fn record_event(statistic: &mut u32, bit: bool) {
    if *statistic >= 0xfffe_0000 {
        *statistic = ((*statistic + 1) >> 1) & 0x7fff_7fff;
    }
    *statistic += 0x0001_0000 + u32::from(bit);
}

fn record_block(
    statistics: &mut Statistics,
    levels: &[i16; 16],
    first: usize,
    coefficient_type: usize,
    initial_context: usize,
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
            position += 1;
            band = usize::from(COEFF_BANDS[position]);
            context = 0;
        }

        let magnitude = levels[position].unsigned_abs();
        position += 1;
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
                        record_event(
                            &mut statistics[coefficient_type][band][context][9],
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

fn collect_statistics(decisions: &[MacroblockDecision], macroblock_width: usize) -> Statistics {
    let mut statistics = [[[[0; 11]; 3]; 8]; 4];
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
                    let nonzero = record_block(
                        &mut statistics,
                        &luma.y2_levels,
                        0,
                        1,
                        usize::from(top_y2[x] + left_y2),
                    );
                    top_y2[x] = nonzero;
                    left_y2 = nonzero;
                    for block_y in 0..4 {
                        for block_x in 0..4 {
                            let context = usize::from(top_y[x][block_x] + left_y[block_y]);
                            let nonzero = record_block(
                                &mut statistics,
                                &luma.y1_levels[block_y * 4 + block_x],
                                1,
                                0,
                                context,
                            );
                            top_y[x][block_x] = nonzero;
                            left_y[block_y] = nonzero;
                        }
                    }
                }
                LumaDecision::Intra4(luma) => {
                    for block_y in 0..4 {
                        for block_x in 0..4 {
                            let context = usize::from(top_y[x][block_x] + left_y[block_y]);
                            let nonzero = record_block(
                                &mut statistics,
                                &luma.levels[block_y * 4 + block_x],
                                0,
                                3,
                                context,
                            );
                            top_y[x][block_x] = nonzero;
                            left_y[block_y] = nonzero;
                        }
                    }
                }
            }

            for plane in 0..2 {
                for block_y in 0..2 {
                    for block_x in 0..2 {
                        let context_index = plane * 2 + block_x;
                        let left_index = plane * 2 + block_y;
                        let context = usize::from(top_uv[x][context_index] + left_uv[left_index]);
                        let nonzero = record_block(
                            &mut statistics,
                            &decision.chroma.levels[plane * 4 + block_y * 2 + block_x],
                            0,
                            2,
                            context,
                        );
                        top_uv[x][context_index] = nonzero;
                        left_uv[left_index] = nonzero;
                    }
                }
            }
        }
    }
    statistics
}

pub(super) fn adapt_coefficients(
    decisions: &[MacroblockDecision],
    macroblock_width: usize,
) -> AdaptedProbabilities {
    let statistics = collect_statistics(decisions, macroblock_width);
    let mut coefficients = COEFF_PROBS;
    let mut updates = [[[[false; 11]; 3]; 8]; 4];
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
                        (255 - ones * 255 / total) as u8
                    };
                    let old_probability = COEFF_PROBS[coefficient_type][band][context][node];
                    let update_probability =
                        coefficient_update_probability(coefficient_type, band, context, node);
                    let branch_cost = |probability: u8| {
                        u64::from(ones) * u64::from(bit_cost(true, probability))
                            + u64::from(total - ones) * u64::from(bit_cost(false, probability))
                    };
                    let old_cost = branch_cost(old_probability)
                        + u64::from(bit_cost(false, update_probability));
                    let new_cost = branch_cost(new_probability)
                        + u64::from(bit_cost(true, update_probability))
                        + 8 * 256;
                    if old_cost > new_cost {
                        coefficients[coefficient_type][band][context][node] = new_probability;
                        updates[coefficient_type][band][context][node] = true;
                    }
                }
            }
        }
    }
    AdaptedProbabilities {
        coefficients,
        updates,
    }
}
