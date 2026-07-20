//! Coefficient partition coding matching libwebp 1.6.0.

use super::{
    bool_enc::BoolEncoder,
    frame::{LumaDecision, MacroblockDecision},
    probability::AdaptedProbabilities,
    tokenize::COEFF_BANDS,
};

const CAT3_PROBABILITIES: [u8; 3] = [173, 148, 140];
const CAT4_PROBABILITIES: [u8; 4] = [176, 155, 140, 135];
const CAT5_PROBABILITIES: [u8; 5] = [180, 157, 141, 134, 130];
const CAT6_PROBABILITIES: [u8; 11] = [254, 254, 243, 230, 196, 177, 153, 140, 133, 130, 129];

fn write_category_bits(
    writer: &mut BoolEncoder,
    residue: u16,
    bit_count: usize,
    probabilities: &[u8],
) {
    for bit in (0..bit_count).rev() {
        writer.encode_bool(
            probabilities[bit_count - 1 - bit],
            residue & (1 << bit) != 0,
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
        position += 1;
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
                    write_category_bits(writer, magnitude - 11, 3, &CAT3_PROBABILITIES);
                } else if magnitude < 35 {
                    writer.encode_bool(node_probabilities[8], false);
                    writer.encode_bool(node_probabilities[9], true);
                    write_category_bits(writer, magnitude - 19, 4, &CAT4_PROBABILITIES);
                } else if magnitude < 67 {
                    writer.encode_bool(node_probabilities[8], true);
                    writer.encode_bool(node_probabilities[10], false);
                    write_category_bits(writer, magnitude - 35, 5, &CAT5_PROBABILITIES);
                } else {
                    writer.encode_bool(node_probabilities[8], true);
                    writer.encode_bool(node_probabilities[10], true);
                    write_category_bits(writer, magnitude - 67, 11, &CAT6_PROBABILITIES);
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

pub(super) fn encode_coefficients(
    decisions: &[MacroblockDecision],
    macroblock_width: usize,
    probabilities: &AdaptedProbabilities,
) -> Vec<u8> {
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
                    let nonzero = write_block(
                        &mut writer,
                        probabilities,
                        &luma.y2_levels,
                        0,
                        1,
                        usize::from(top_y2[x] + left_y2),
                    );
                    top_y2[x] = nonzero;
                    left_y2 = nonzero;
                    for block_y in 0..4 {
                        for block_x in 0..4 {
                            let nonzero = write_block(
                                &mut writer,
                                probabilities,
                                &luma.y1_levels[block_y * 4 + block_x],
                                1,
                                0,
                                usize::from(top_y[x][block_x] + left_y[block_y]),
                            );
                            top_y[x][block_x] = nonzero;
                            left_y[block_y] = nonzero;
                        }
                    }
                }
                LumaDecision::Intra4(luma) => {
                    for block_y in 0..4 {
                        for block_x in 0..4 {
                            let nonzero = write_block(
                                &mut writer,
                                probabilities,
                                &luma.levels[block_y * 4 + block_x],
                                0,
                                3,
                                usize::from(top_y[x][block_x] + left_y[block_y]),
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
                        let top_index = plane * 2 + block_x;
                        let left_index = plane * 2 + block_y;
                        let nonzero = write_block(
                            &mut writer,
                            probabilities,
                            &decision.chroma.levels[plane * 4 + block_y * 2 + block_x],
                            0,
                            2,
                            usize::from(top_uv[x][top_index] + left_uv[left_index]),
                        );
                        top_uv[x][top_index] = nonzero;
                        left_uv[left_index] = nonzero;
                    }
                }
            }
        }
    }
    writer.finish()
}
