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
const COEFFICIENT_CHECKPOINT_BLOCKS: usize = 64;
const COEFFICIENT_CHECKPOINT_MACROBLOCKS: usize = 256;

struct CoefficientCheckpoint<'a> {
    token: Option<&'a crate::CancellationToken>,
    block_items: usize,
}

fn write_category_bits(
    writer: &mut BoolEncoder,
    residue: u16,
    bit_count: usize,
    probabilities: &[u8],
) {
    for bit in (0..bit_count).rev() {
        writer.encode_bool(
            probabilities[bit_count.saturating_sub(1).saturating_sub(bit)],
            residue & 1u16.wrapping_shl(bit.to_le_bytes()[0].into()) != 0,
        );
    }
}

fn write_block(
    writer: &mut BoolEncoder,
    probabilities: &AdaptedProbabilities,
    levels: &[i16; 16],
    first: usize,
    coefficient_type: usize,
    initial_context: usize,
) -> u8 {
    let Some(last) = (first..16).rev().find(|&position| levels[position] != 0) else {
        writer.encode_bool(
            probabilities.coefficients[coefficient_type][usize::from(COEFF_BANDS[first])]
                [initial_context][0],
            false,
        );
        return 0;
    };

    let mut position = first;
    let mut band = usize::from(COEFF_BANDS[position]);
    let mut context = initial_context;
    writer.encode_bool(
        probabilities.coefficients[coefficient_type][band][context][0],
        true,
    );

    loop {
        let coefficient = levels[position];
        position = position.saturating_add(1);
        let magnitude = coefficient.unsigned_abs();
        let node_probabilities = probabilities.coefficients[coefficient_type][band][context];

        if magnitude == 0 {
            writer.encode_bool(node_probabilities[1], false);
            band = usize::from(COEFF_BANDS[position]);
            context = 0;
            continue;
        }

        writer.encode_bool(node_probabilities[1], true);
        if magnitude == 1 {
            writer.encode_bool(node_probabilities[2], false);
            context = 1;
        } else {
            writer.encode_bool(node_probabilities[2], true);
            if magnitude <= 4 {
                writer.encode_bool(node_probabilities[3], false);
                let not_two = magnitude != 2;
                writer.encode_bool(node_probabilities[4], not_two);
                if not_two {
                    writer.encode_bool(node_probabilities[5], magnitude == 4);
                }
            } else if magnitude <= 10 {
                writer.encode_bool(node_probabilities[3], true);
                writer.encode_bool(node_probabilities[6], false);
                let greater_than_six = magnitude > 6;
                writer.encode_bool(node_probabilities[7], greater_than_six);
                if greater_than_six {
                    writer.encode_bool(165, magnitude >= 9);
                    writer.encode_bool(145, magnitude & 1 == 0);
                } else {
                    writer.encode_bool(159, magnitude == 6);
                }
            } else {
                writer.encode_bool(node_probabilities[3], true);
                writer.encode_bool(node_probabilities[6], true);
                if magnitude < 19 {
                    writer.encode_bool(node_probabilities[8], false);
                    writer.encode_bool(node_probabilities[9], false);
                    write_category_bits(
                        writer,
                        magnitude.saturating_sub(11),
                        3,
                        &CAT3_PROBABILITIES,
                    );
                } else if magnitude < 35 {
                    writer.encode_bool(node_probabilities[8], false);
                    writer.encode_bool(node_probabilities[9], true);
                    write_category_bits(
                        writer,
                        magnitude.saturating_sub(19),
                        4,
                        &CAT4_PROBABILITIES,
                    );
                } else if magnitude < 67 {
                    writer.encode_bool(node_probabilities[8], true);
                    writer.encode_bool(node_probabilities[10], false);
                    write_category_bits(
                        writer,
                        magnitude.saturating_sub(35),
                        5,
                        &CAT5_PROBABILITIES,
                    );
                } else {
                    writer.encode_bool(node_probabilities[8], true);
                    writer.encode_bool(node_probabilities[10], true);
                    write_category_bits(
                        writer,
                        magnitude.saturating_sub(67),
                        11,
                        &CAT6_PROBABILITIES,
                    );
                }
            }
            context = 2;
        }

        writer.encode_bool(128, coefficient < 0);
        if position == 16 {
            return 1;
        }
        band = usize::from(COEFF_BANDS[position]);
        let has_more = position <= last;
        writer.encode_bool(
            probabilities.coefficients[coefficient_type][band][context][0],
            has_more,
        );
        if !has_more {
            return 1;
        }
    }
}

fn write_block_with_checkpoint(
    writer: &mut BoolEncoder,
    probabilities: &AdaptedProbabilities,
    levels: &[i16; 16],
    first: usize,
    coefficient_type: usize,
    initial_context: usize,
    checkpoint: &mut CoefficientCheckpoint<'_>,
) -> CodecResult<u8> {
    let nonzero = write_block(
        writer,
        probabilities,
        levels,
        first,
        coefficient_type,
        initial_context,
    );
    let Some(token) = checkpoint.token else {
        return Ok(nonzero);
    };
    checkpoint.block_items = checkpoint.block_items.saturating_add(1);
    if checkpoint
        .block_items
        .is_multiple_of(COEFFICIENT_CHECKPOINT_BLOCKS)
    {
        crate::codecs::error::check_cancelled(Some(token))?;
    }
    Ok(nonzero)
}

pub(super) fn encode_coefficients(
    decisions: &[MacroblockDecision],
    macroblock_width: usize,
    probabilities: &AdaptedProbabilities,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    let mut writer = BoolEncoder::default();
    let mut top_y = vec![[0u8; 4]; macroblock_width];
    let mut top_uv = vec![[0u8; 4]; macroblock_width];
    let mut top_y2 = vec![0u8; macroblock_width];
    let mut coefficient_checkpoint = CoefficientCheckpoint {
        token,
        block_items: 0,
    };
    let mut macroblock_items = 0usize;

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
                        &mut coefficient_checkpoint,
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
                                &mut coefficient_checkpoint,
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
                                &mut coefficient_checkpoint,
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
                            &mut coefficient_checkpoint,
                        )?;
                        top_uv[x][top_index] = nonzero;
                        left_uv[left_index] = nonzero;
                    }
                }
            }
            macroblock_items = macroblock_items.saturating_add(1);
            if macroblock_items.is_multiple_of(COEFFICIENT_CHECKPOINT_MACROBLOCKS) {
                crate::codecs::error::check_cancelled(token)?;
            }
        }
    }
    Ok(writer.finish())
}
