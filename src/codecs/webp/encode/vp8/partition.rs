//! VP8 first-partition syntax matching libwebp 1.6.0.

#![allow(dead_code)]

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

fn write_signed(writer: &mut BoolEncoder, value: i32, magnitude_bits: u8) {
    writer.encode_bool(128, value != 0);
    if value != 0 {
        let magnitude_and_sign = (value.unsigned_abs() << 1) | u32::from(value < 0);
        writer.encode_literal(magnitude_and_sign, magnitude_bits + 1);
    }
}

fn segment_probability(zero: usize, one: usize) -> u8 {
    let total = zero + one;
    if total == 0 {
        255
    } else {
        ((255 * zero + total / 2) / total) as u8
    }
}

fn segment_probabilities(decisions: &[MacroblockDecision]) -> [u8; 3] {
    let mut counts = [0usize; 4];
    for decision in decisions {
        counts[usize::from(decision.segment)] += 1;
    }
    [
        segment_probability(counts[0] + counts[1], counts[2] + counts[3]),
        segment_probability(counts[0], counts[1]),
        segment_probability(counts[2], counts[3]),
    ]
}

fn adjusted_frame_params(decisions: &[MacroblockDecision], params: &FrameParams) -> FrameParams {
    let mut adjusted = FrameParams {
        segments: params.segments,
        chroma_dc_delta: params.chroma_dc_delta,
        chroma_ac_delta: params.chroma_ac_delta,
    };
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
        let minimum_distortion = 20 * u32::from(matrices.y1.q[0]);
        if only_y2_nonzero && luma.distortion > minimum_distortion {
            let edge = [luma.y2_levels[1], luma.y2_levels[2], luma.y2_levels[4]]
                .into_iter()
                .map(i16::unsigned_abs)
                .max()
                .unwrap();
            maximum_edges[segment] = maximum_edges[segment].max(edge);
        }
    }
    for segment in 0..4 {
        let matrices = libwebp_segment_matrices(
            params.segments[segment].quantizer,
            params.chroma_dc_delta,
            params.chroma_ac_delta,
        );
        let delta = (u32::from(maximum_edges[segment]) * u32::from(matrices.y2.q[1])) >> 3;
        adjusted.segments[segment].filter_strength = adjusted.segments[segment]
            .filter_strength
            .max(delta.min(63) as u8);
    }
    adjusted
}

fn write_segment_header(writer: &mut BoolEncoder, params: &FrameParams, probabilities: [u8; 3]) {
    writer.encode_bool(128, true); // four segments
    writer.encode_bool(128, true); // update map
    writer.encode_bool(128, true); // update data
    writer.encode_bool(128, true); // absolute feature values
    for segment in params.segments {
        write_signed(writer, i32::from(segment.quantizer), 7);
    }
    for segment in params.segments {
        write_signed(writer, i32::from(segment.filter_strength), 6);
    }
    for probability in probabilities {
        let update = probability != 255;
        writer.encode_bool(128, update);
        if update {
            writer.encode_literal(u32::from(probability), 8);
        }
    }
}

fn write_coefficient_probabilities(writer: &mut BoolEncoder, probabilities: &AdaptedProbabilities) {
    for coefficient_type in 0..4 {
        for band in 0..8 {
            for context in 0..3 {
                for node in 0..11 {
                    let update = probabilities.updates[coefficient_type][band][context][node];
                    writer.encode_bool(
                        coefficient_update_probability(coefficient_type, band, context, node),
                        update,
                    );
                    if update {
                        writer.encode_literal(
                            u32::from(
                                probabilities.coefficients[coefficient_type][band][context][node],
                            ),
                            8,
                        );
                    }
                }
            }
        }
    }
    writer.encode_bool(128, false); // no skip probability
}

fn write_segment(writer: &mut BoolEncoder, segment: u8, probabilities: [u8; 3]) {
    let upper_half = segment >= 2;
    writer.encode_bool(probabilities[0], upper_half);
    writer.encode_bool(
        probabilities[if upper_half { 2 } else { 1 }],
        segment & 1 != 0,
    );
}

fn write_intra4_mode(writer: &mut BoolEncoder, mode: Intra4Mode, probabilities: &[u8; 9]) {
    let mode = mode as u8;
    if writer_bit(writer, probabilities[0], mode != 0)
        && writer_bit(writer, probabilities[1], mode != 1)
        && writer_bit(writer, probabilities[2], mode != 2)
    {
        if !writer_bit(writer, probabilities[3], mode >= 6) {
            if writer_bit(writer, probabilities[4], mode != 3) {
                writer_bit(writer, probabilities[5], mode != 4);
            }
        } else if writer_bit(writer, probabilities[6], mode != 6)
            && writer_bit(writer, probabilities[7], mode != 7)
        {
            writer_bit(writer, probabilities[8], mode != 8);
        }
    }
}

fn writer_bit(writer: &mut BoolEncoder, probability: u8, bit: bool) -> bool {
    writer.encode_bool(probability, bit);
    bit
}

fn write_intra16_mode(writer: &mut BoolEncoder, mode: Intra16Mode) {
    let horizontal_or_true_motion =
        matches!(mode, Intra16Mode::TrueMotion | Intra16Mode::Horizontal);
    writer.encode_bool(156, horizontal_or_true_motion);
    if horizontal_or_true_motion {
        writer.encode_bool(128, mode == Intra16Mode::TrueMotion);
    } else {
        writer.encode_bool(163, mode == Intra16Mode::Vertical);
    }
}

fn write_chroma_mode(writer: &mut BoolEncoder, mode: ChromaMode) {
    if writer_bit(writer, 142, mode != ChromaMode::Dc)
        && writer_bit(writer, 114, mode != ChromaMode::Vertical)
    {
        writer_bit(writer, 183, mode != ChromaMode::Horizontal);
    }
}

fn intra16_as_intra4(mode: Intra16Mode) -> Intra4Mode {
    match mode {
        Intra16Mode::Dc => Intra4Mode::Dc,
        Intra16Mode::TrueMotion => Intra4Mode::TrueMotion,
        Intra16Mode::Vertical => Intra4Mode::Vertical,
        Intra16Mode::Horizontal => Intra4Mode::Horizontal,
    }
}

fn write_modes(
    writer: &mut BoolEncoder,
    decisions: &[MacroblockDecision],
    macroblock_width: usize,
    segment_probabilities: [u8; 3],
) {
    let mode_stride = macroblock_width * 4;
    let macroblock_height = decisions.len() / macroblock_width;
    let mut modes = vec![Intra4Mode::Dc; mode_stride * macroblock_height * 4];
    for decision in decisions {
        write_segment(writer, decision.segment, segment_probabilities);
        let is_intra16 = matches!(decision.luma, LumaDecision::Intra16(_));
        writer.encode_bool(145, is_intra16);
        match &decision.luma {
            LumaDecision::Intra16(luma) => write_intra16_mode(writer, luma.mode),
            LumaDecision::Intra4(luma) => {
                for block_y in 0..4 {
                    for block_x in 0..4 {
                        let grid_x = decision.x * 4 + block_x;
                        let grid_y = decision.y * 4 + block_y;
                        let top = if grid_y == 0 {
                            Intra4Mode::Dc
                        } else {
                            modes[(grid_y - 1) * mode_stride + grid_x]
                        };
                        let left = if grid_x == 0 {
                            Intra4Mode::Dc
                        } else {
                            modes[grid_y * mode_stride + grid_x - 1]
                        };
                        let mode = luma.modes[block_y * 4 + block_x];
                        write_intra4_mode(
                            writer,
                            mode,
                            &INTRA4_MODE_PROBABILITIES[top as usize][left as usize],
                        );
                        modes[grid_y * mode_stride + grid_x] = mode;
                    }
                }
            }
        }
        if let LumaDecision::Intra16(luma) = &decision.luma {
            let mode = intra16_as_intra4(luma.mode);
            for block_y in 0..4 {
                for block_x in 0..4 {
                    modes[(decision.y * 4 + block_y) * mode_stride + decision.x * 4 + block_x] =
                        mode;
                }
            }
        }
        write_chroma_mode(writer, decision.chroma.mode);
    }
}

pub(super) fn encode_first_partition(
    decisions: &[MacroblockDecision],
    macroblock_width: usize,
    params: &FrameParams,
    probabilities: &AdaptedProbabilities,
) -> Vec<u8> {
    let params = adjusted_frame_params(decisions, params);
    let segment_probabilities = segment_probabilities(decisions);
    let mut writer = BoolEncoder::new();
    writer.encode_bool(128, false); // colorspace
    writer.encode_bool(128, false); // clamp type
    write_segment_header(&mut writer, &params, segment_probabilities);
    writer.encode_bool(128, false); // strong loop filter
    let filter_level = params
        .segments
        .iter()
        .map(|segment| segment.filter_strength)
        .max()
        .unwrap();
    writer.encode_literal(u32::from(filter_level), 6);
    writer.encode_literal(0, 3); // sharpness
    writer.encode_bool(128, false); // no loop-filter deltas
    writer.encode_literal(0, 2); // one coefficient partition
    writer.encode_literal(u32::from(params.segments[0].quantizer), 7);
    write_signed(&mut writer, 0, 4); // Y1 DC
    write_signed(&mut writer, 0, 4); // Y2 DC
    write_signed(&mut writer, 0, 4); // Y2 AC
    write_signed(&mut writer, i32::from(params.chroma_dc_delta), 4);
    write_signed(&mut writer, i32::from(params.chroma_ac_delta), 4);
    writer.encode_bool(128, false); // no entropy refresh
    write_coefficient_probabilities(&mut writer, probabilities);
    write_modes(
        &mut writer,
        decisions,
        macroblock_width,
        segment_probabilities,
    );
    writer.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codecs::webp::encode::vp8::{
        analysis::{analyze, segment_params},
        encoder::rgb_to_yuv_planes_internal,
        frame::select_frame,
        probability::adapt_coefficients,
    };

    #[test]
    fn q80_first_partition_matches_libwebp_1_6_0() {
        let rgb = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/outputs/raws/Decode.webp_lossless_webp.bin"
        ));
        let expected_webp = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/outputs/encoded/Encode.webp_enc_lossy_q80.bin"
        ));
        let (y, u, v) = rgb_to_yuv_planes_internal(rgb, 128, 128);
        let analysis = analyze(&y, &u, &v, 128, 128);
        let params = segment_params(&analysis, 80.0);
        let decisions = select_frame(&y, &u, &v, 128, 128, 80.0);
        let probabilities = adapt_coefficients(&decisions, 8);
        let actual = encode_first_partition(&decisions, 8, &params, &probabilities);

        let vp8_payload = &expected_webp[20..];
        let frame_tag = u32::from(vp8_payload[0])
            | (u32::from(vp8_payload[1]) << 8)
            | (u32::from(vp8_payload[2]) << 16);
        let first_partition_size = ((frame_tag >> 5) & 0x7ffff) as usize;
        let expected = &vp8_payload[10..10 + first_partition_size];
        assert_eq!(actual.len(), 489);
        if actual != expected {
            let first = actual
                .iter()
                .zip(expected)
                .position(|(actual, expected)| actual != expected)
                .expect("equal-length partitions must have a differing byte");
            panic!(
                "first partition mismatch at byte {first}: actual={:02x}, expected={:02x}",
                actual[first], expected[first]
            );
        }
    }
}
