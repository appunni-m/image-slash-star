// Modified Rust port copyright (c) 2026 Appunni M.
// Derived from libjpeg-turbo/IJG sources; see third_party/libjpeg-turbo/.

// ── Progressive JPEG Reconstruction ──────────────────────────────────────
// Port of libjpeg-turbo 3.1.4.1 jdphuff.c progressive Huffman decoding.
//
// The four scan routines (DC_first, DC_refine, AC_first, AC_refine)
// operate ONE BLOCK AT A TIME, matching IJG's decode_mcu_xxx function pattern.
// State (EOBRUN, DC predictors) is managed in ProgressiveState and
// persists across blocks within the entire scan segment.
//
// Key difference from the old implementation: blocks are processed in
// IJG-specified MCU_membership order. For 4:2:0 Y component scans (Ah!=0),
// this means 4 blocks per MCU (2×2), processed in raster order within the MCU:
// (0,0), (0,1), (1,0), (1,1).

use super::bit_reader::BitReader;
#[cfg(target_arch = "aarch64")]
use super::bit_reader::FastBitReader;
#[cfg(target_arch = "aarch64")]
use super::idct::jpeg_idct_islow_dequantized_to_u8_safe;
use super::idct::{JPEG_NATURAL_ORDER, YccColorConverter, extend, jpeg_idct_islow};
use super::implementation::extract_entropy_segments;
use super::parser::JpegInfo;
use super::upsample::{crop_component, fancy_upsample};
use crate::codecs::{CodecError, CodecResult, OptionCodecExt};
use crate::types::{ColorType, DecodedImage};

trait ProgressiveEntropyReader {
    fn decode_huffman(&mut self, table: &super::huffman::HuffTable) -> CodecResult<u8>;
    fn read_padded(&mut self, bits: u32) -> u32;
}

impl ProgressiveEntropyReader for BitReader<'_> {
    #[inline(always)]
    fn decode_huffman(&mut self, table: &super::huffman::HuffTable) -> CodecResult<u8> {
        table.decode(self)
    }

    #[inline(always)]
    fn read_padded(&mut self, bits: u32) -> u32 {
        self.read_padded_bits_optional(bits).unwrap_or_default()
    }
}

#[cfg(target_arch = "aarch64")]
impl ProgressiveEntropyReader for FastBitReader<'_> {
    #[inline(always)]
    fn decode_huffman(&mut self, table: &super::huffman::HuffTable) -> CodecResult<u8> {
        table.decode_fast(self)
    }

    #[inline(always)]
    fn read_padded(&mut self, bits: u32) -> u32 {
        self.read_padded_bits(bits)
    }
}

struct ProgressiveState {
    eobrun: u32,
    dc_predictors: [i32; 4],
}

impl ProgressiveState {
    fn new(num_components: usize) -> Self {
        debug_assert!(num_components <= 4);
        ProgressiveState {
            eobrun: 0,
            dc_predictors: [0; 4],
        }
    }
    fn reset(&mut self) {
        self.eobrun = 0;
        self.dc_predictors.fill(0);
    }
}

/// Process one DC-first block (decode_mcu_DC_first).
fn dc_first_block<R: ProgressiveEntropyReader>(
    br: &mut R,
    dc_table: &super::huffman::HuffTable,
    dc_pred: &mut i32,
    al: u8,
) -> CodecResult<i32> {
    let dc_cat = br.decode_huffman(dc_table)?;
    if dc_cat > 0 {
        if dc_cat > 15 {
            return Err(CodecError::Malformed(
                "invalid progressive JPEG DC coefficient category".to_owned(),
            ));
        }
        let bits = br.read_padded(u32::from(dc_cat));
        *dc_pred = dc_pred.saturating_add(extend(bits, dc_cat));
    }
    Ok(dc_pred.wrapping_shl(u32::from(al)))
}

/// Process one DC-refinement block (decode_mcu_DC_refine).
fn dc_refine_block(coeff: &mut i32, p1: i32) {
    // One more bit of precision.  The caller reads the bit.
    *coeff |= p1;
}

/// Process one AC-first block (decode_mcu_AC_first).
/// Updates eobrun.  Returns the number of coefficients decoded (for debugging).
fn ac_first_block<R: ProgressiveEntropyReader>(
    br: &mut R,
    ac_table: &super::huffman::HuffTable,
    ss: u8,
    se: u8,
    al: u8,
    coeffs: &mut [i32; 64],
    eobrun: &mut u32,
) -> CodecResult<usize> {
    if *eobrun > 0 {
        *eobrun = eobrun.saturating_sub(1);
        return Ok(0); // entire block zero in this band
    }
    let ss = usize::from(ss);
    let se = usize::from(se);
    let mut k = ss;
    let mut ncoeffs = 0usize;
    while k <= se && k < 64 {
        let sym = br.decode_huffman(ac_table)?;
        let run = usize::from(sym >> 4);
        let run_bits = u32::from(sym >> 4);
        let size = sym & 0x0F;
        if size == 0 {
            if run == 15 {
                k = k.saturating_add(16); // ZRL
                continue;
            }
            // EOB: EOBRUN = (1<<run) + extra_bits
            *eobrun = 1u32.wrapping_shl(run_bits);
            if run > 0 {
                *eobrun = eobrun.saturating_add(br.read_padded(run_bits));
            }
            *eobrun = eobrun.saturating_sub(1); // this block consumes one from the run
            break;
        }
        // Coefficient at position k + run
        k = k.saturating_add(run);
        if k > se || k >= 64 {
            break;
        }
        let bits = br.read_padded(u32::from(size));
        coeffs[k] = extend(bits, size).wrapping_shl(u32::from(al));
        ncoeffs = ncoeffs.saturating_add(1);
        k = k.saturating_add(1);
    }
    Ok(ncoeffs)
}

/// Process one AC-refinement block (decode_mcu_AC_refine).
/// Updates eobrun.
fn ac_refine_block<R: ProgressiveEntropyReader>(
    br: &mut R,
    ac_table: &super::huffman::HuffTable,
    ss: u8,
    se: u8,
    al: u8,
    coeffs: &mut [i32; 64],
    eobrun: &mut u32,
) -> CodecResult<()> {
    let p1 = 1i32.wrapping_shl(u32::from(al));
    let m1 = (-1i32).wrapping_shl(u32::from(al));
    let ss = usize::from(ss);
    let se = usize::from(se);
    let mut k = ss;

    // Phase 1: Huffman decode when EOBRUN == 0
    if *eobrun == 0 {
        while k <= se && k < 64 {
            let sym = br.decode_huffman(ac_table)?;
            let mut r = i32::from(sym >> 4);
            let size = sym & 0x0F;

            // New coefficient value
            let new_val = if size != 0 {
                let bit = br.read_padded(1);
                Some(if bit != 0 { p1 } else { m1 })
            } else {
                if r != 15 {
                    *eobrun = 1u32.wrapping_shl(r.cast_unsigned());
                    if r > 0 {
                        *eobrun = eobrun.saturating_add(br.read_padded(r.cast_unsigned()));
                    }
                    break; // → Phase 2
                }
                None // ZRL
            };

            // do-while: traverse, refine non-zeros, count zeros
            loop {
                if k > se || k >= 64 {
                    break;
                }
                if coeffs[k] != 0 {
                    let bit = br.read_padded(1);
                    if bit != 0 && (coeffs[k] & p1) == 0 {
                        coeffs[k] = coeffs[k].saturating_add(if coeffs[k] >= 0 { p1 } else { m1 });
                    }
                } else {
                    r = r.saturating_sub(1);
                    if r < 0 {
                        break;
                    }
                }
                k = k.saturating_add(1);
            }

            if let Some(val) = new_val
                && k <= se
                && k < 64
            {
                coeffs[k] = val;
            }
            k = k.saturating_add(1);
        }
    }

    // Phase 2: EOBRUN handler — refine remaining non-zero coeffs
    if *eobrun > 0 {
        while k <= se && k < 64 {
            if coeffs[k] != 0 {
                let bit = br.read_padded(1);
                if bit != 0 && (coeffs[k] & p1) == 0 {
                    coeffs[k] = coeffs[k].saturating_add(if coeffs[k] >= 0 { p1 } else { m1 });
                }
            }
            k = k.saturating_add(1);
        }
        *eobrun = eobrun.saturating_sub(1);
    }

    Ok(())
}

fn smooth_pred(num: i64, quant: i64, al: i32) -> i32 {
    if quant == 0 {
        return 0;
    }
    let denom = quant.saturating_mul(256);
    let round = quant.saturating_mul(128);
    let mut pred = if num >= 0 {
        low_i32(round.saturating_add(num).div_euclid(denom))
    } else {
        low_i32(round.saturating_sub(num).div_euclid(denom)).saturating_neg()
    };
    if al > 0 {
        let limit = 1i32.wrapping_shl(al.cast_unsigned());
        if pred >= limit {
            pred = limit.saturating_sub(1);
        }
    }
    pred
}

fn low_i32(value: i64) -> i32 {
    let [a, b, c, d, ..] = value.to_le_bytes();
    i32::from_le_bytes([a, b, c, d])
}

fn bounded_usize(value: u32) -> usize {
    #[cfg(target_pointer_width = "64")]
    {
        let [a, b, c, d] = value.to_le_bytes();
        usize::from_le_bytes([a, b, c, d, 0, 0, 0, 0])
    }
    #[cfg(target_pointer_width = "32")]
    {
        usize::from_le_bytes(value.to_le_bytes())
    }
}

fn bounded_isize(value: usize) -> isize {
    isize::from_le_bytes(value.to_le_bytes())
}

fn weighted_dc(terms: &[(i32, i32)]) -> i64 {
    terms.iter().fold(0i64, |sum, &(weight, dc)| {
        sum.saturating_add(i64::from(weight).saturating_mul(i64::from(dc)))
    })
}

fn dc_at(blocks: &[[i32; 64]], blocks_x: usize, blocks_y: usize, x: isize, y: isize) -> i32 {
    let clamped_x = x
        .clamp(0, bounded_isize(blocks_x.saturating_sub(1)))
        .cast_unsigned();
    let clamped_y = y
        .clamp(0, bounded_isize(blocks_y.saturating_sub(1)))
        .cast_unsigned();
    blocks[clamped_y.saturating_mul(blocks_x).saturating_add(clamped_x)][0]
}

fn smooth_dc_only_block(
    blocks: &[[i32; 64]],
    blocks_x: usize,
    blocks_y: usize,
    block_idx: usize,
    quant_natural: &[i32; 64],
    workspace: &mut [i32; 64],
) {
    workspace.fill(0);
    let x = bounded_isize(block_idx.rem_euclid(blocks_x));
    let y = bounded_isize(block_idx.div_euclid(blocks_x));

    let dc01 = dc_at(
        blocks,
        blocks_x,
        blocks_y,
        x.saturating_sub(2),
        y.saturating_sub(2),
    );
    let dc02 = dc_at(
        blocks,
        blocks_x,
        blocks_y,
        x.saturating_sub(1),
        y.saturating_sub(2),
    );
    let dc03 = dc_at(blocks, blocks_x, blocks_y, x, y.saturating_sub(2));
    let dc04 = dc_at(
        blocks,
        blocks_x,
        blocks_y,
        x.saturating_add(1),
        y.saturating_sub(2),
    );
    let dc05 = dc_at(
        blocks,
        blocks_x,
        blocks_y,
        x.saturating_add(2),
        y.saturating_sub(2),
    );
    let dc06 = dc_at(
        blocks,
        blocks_x,
        blocks_y,
        x.saturating_sub(2),
        y.saturating_sub(1),
    );
    let dc07 = dc_at(
        blocks,
        blocks_x,
        blocks_y,
        x.saturating_sub(1),
        y.saturating_sub(1),
    );
    let dc08 = dc_at(blocks, blocks_x, blocks_y, x, y.saturating_sub(1));
    let dc09 = dc_at(
        blocks,
        blocks_x,
        blocks_y,
        x.saturating_add(1),
        y.saturating_sub(1),
    );
    let dc10 = dc_at(
        blocks,
        blocks_x,
        blocks_y,
        x.saturating_add(2),
        y.saturating_sub(1),
    );
    let dc11 = dc_at(blocks, blocks_x, blocks_y, x.saturating_sub(2), y);
    let dc12 = dc_at(blocks, blocks_x, blocks_y, x.saturating_sub(1), y);
    let dc13 = dc_at(blocks, blocks_x, blocks_y, x, y);
    let dc14 = dc_at(blocks, blocks_x, blocks_y, x.saturating_add(1), y);
    let dc15 = dc_at(blocks, blocks_x, blocks_y, x.saturating_add(2), y);
    let dc16 = dc_at(
        blocks,
        blocks_x,
        blocks_y,
        x.saturating_sub(2),
        y.saturating_add(1),
    );
    let dc17 = dc_at(
        blocks,
        blocks_x,
        blocks_y,
        x.saturating_sub(1),
        y.saturating_add(1),
    );
    let dc18 = dc_at(blocks, blocks_x, blocks_y, x, y.saturating_add(1));
    let dc19 = dc_at(
        blocks,
        blocks_x,
        blocks_y,
        x.saturating_add(1),
        y.saturating_add(1),
    );
    let dc20 = dc_at(
        blocks,
        blocks_x,
        blocks_y,
        x.saturating_add(2),
        y.saturating_add(1),
    );
    let dc21 = dc_at(
        blocks,
        blocks_x,
        blocks_y,
        x.saturating_sub(2),
        y.saturating_add(2),
    );
    let dc22 = dc_at(
        blocks,
        blocks_x,
        blocks_y,
        x.saturating_sub(1),
        y.saturating_add(2),
    );
    let dc23 = dc_at(blocks, blocks_x, blocks_y, x, y.saturating_add(2));
    let dc24 = dc_at(
        blocks,
        blocks_x,
        blocks_y,
        x.saturating_add(1),
        y.saturating_add(2),
    );
    let dc25 = dc_at(
        blocks,
        blocks_x,
        blocks_y,
        x.saturating_add(2),
        y.saturating_add(2),
    );

    let q00 = i64::from(quant_natural[0]);
    let q01 = i64::from(quant_natural[1]);
    let q10 = i64::from(quant_natural[8]);
    let q20 = i64::from(quant_natural[16]);
    let q11 = i64::from(quant_natural[9]);
    let q02 = i64::from(quant_natural[2]);
    let q03 = i64::from(quant_natural[3]);
    let q12 = i64::from(quant_natural[10]);
    let q21 = i64::from(quant_natural[17]);
    let q30 = i64::from(quant_natural[24]);
    workspace[1] = smooth_pred(
        q00.saturating_mul(weighted_dc(&[
            (-1, dc01),
            (-1, dc02),
            (1, dc04),
            (1, dc05),
            (-3, dc06),
            (13, dc07),
            (-13, dc09),
            (3, dc10),
            (-3, dc11),
            (38, dc12),
            (-38, dc14),
            (3, dc15),
            (-3, dc16),
            (13, dc17),
            (-13, dc19),
            (3, dc20),
            (-1, dc21),
            (-1, dc22),
            (1, dc24),
            (1, dc25),
        ])),
        q01,
        -1,
    );
    workspace[8] = smooth_pred(
        q00.saturating_mul(weighted_dc(&[
            (-1, dc01),
            (-3, dc02),
            (-3, dc03),
            (-3, dc04),
            (-1, dc05),
            (-1, dc06),
            (13, dc07),
            (38, dc08),
            (13, dc09),
            (-1, dc10),
            (1, dc16),
            (-13, dc17),
            (-38, dc18),
            (-13, dc19),
            (1, dc20),
            (1, dc21),
            (3, dc22),
            (3, dc23),
            (3, dc24),
            (1, dc25),
        ])),
        q10,
        -1,
    );
    workspace[16] = smooth_pred(
        q00.saturating_mul(weighted_dc(&[
            (1, dc03),
            (2, dc07),
            (7, dc08),
            (2, dc09),
            (-5, dc12),
            (-14, dc13),
            (-5, dc14),
            (2, dc17),
            (7, dc18),
            (2, dc19),
            (1, dc23),
        ])),
        q20,
        -1,
    );
    workspace[9] = smooth_pred(
        q00.saturating_mul(weighted_dc(&[
            (-1, dc01),
            (1, dc05),
            (9, dc07),
            (-9, dc09),
            (-9, dc17),
            (9, dc19),
            (1, dc21),
            (-1, dc25),
        ])),
        q11,
        -1,
    );
    workspace[2] = smooth_pred(
        q00.saturating_mul(weighted_dc(&[
            (2, dc07),
            (-5, dc08),
            (2, dc09),
            (1, dc11),
            (7, dc12),
            (-14, dc13),
            (7, dc14),
            (1, dc15),
            (2, dc17),
            (-5, dc18),
            (2, dc19),
        ])),
        q02,
        -1,
    );
    workspace[3] = smooth_pred(
        q00.saturating_mul(weighted_dc(&[
            (1, dc07),
            (-1, dc09),
            (2, dc12),
            (-2, dc14),
            (1, dc17),
            (-1, dc19),
        ])),
        q03,
        -1,
    );
    workspace[10] = smooth_pred(
        q00.saturating_mul(weighted_dc(&[
            (1, dc07),
            (-3, dc08),
            (1, dc09),
            (-1, dc17),
            (3, dc18),
            (-1, dc19),
        ])),
        q12,
        -1,
    );
    workspace[17] = smooth_pred(
        q00.saturating_mul(weighted_dc(&[
            (1, dc07),
            (-1, dc09),
            (-3, dc12),
            (3, dc14),
            (1, dc17),
            (-1, dc19),
        ])),
        q21,
        -1,
    );
    workspace[24] = smooth_pred(
        q00.saturating_mul(weighted_dc(&[
            (1, dc07),
            (2, dc08),
            (1, dc09),
            (-1, dc17),
            (-2, dc18),
            (-1, dc19),
        ])),
        q30,
        -1,
    );
    workspace[0] = smooth_pred(
        q00.saturating_mul(weighted_dc(&[
            (-2, dc01),
            (-6, dc02),
            (-8, dc03),
            (-6, dc04),
            (-2, dc05),
            (-6, dc06),
            (6, dc07),
            (42, dc08),
            (6, dc09),
            (-6, dc10),
            (-8, dc11),
            (42, dc12),
            (152, dc13),
            (42, dc14),
            (-8, dc15),
            (-6, dc16),
            (6, dc17),
            (42, dc18),
            (6, dc19),
            (-6, dc20),
            (-2, dc21),
            (-6, dc22),
            (-8, dc23),
            (-6, dc24),
            (-2, dc25),
        ])),
        q00,
        0,
    );
}

#[cfg(target_arch = "aarch64")]
#[inline(never)]
// Keep the hot IDCT operands explicit so this wrapper remains allocation-free
// and matches the vectorized kernel's calling convention.
#[allow(clippy::too_many_arguments)]
fn progressive_dequantize_block(
    block_natural: &mut [i32; 64],
    quant_natural: &[i32; 64],
    workspace: &mut [i32; 64],
    destination: &mut [u8],
    stride: usize,
    block_x: usize,
    block_y: usize,
    high_horizontal_nonzero: bool,
) -> bool {
    if block_natural[0].checked_mul(quant_natural[0]).is_some() {
        jpeg_idct_islow_dequantized_to_u8_safe(
            block_natural,
            quant_natural,
            workspace,
            destination,
            stride,
            block_x,
            block_y,
            high_horizontal_nonzero,
        );
        return true;
    }
    for (coefficient, &quantizer) in block_natural.iter_mut().zip(quant_natural) {
        *coefficient = coefficient.saturating_mul(quantizer);
    }
    false
}

// Progressive scan parsing validates tables and uses a zero-padding bit reader.
pub(super) fn progressive_reconstruct(
    info: &JpegInfo,
    data: &[u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<DecodedImage> {
    crate::codecs::error::check_cancelled(token)?;
    let mcu_width = u32::from(info.max_h_samp).saturating_mul(8);
    let mcu_height = u32::from(info.max_v_samp).saturating_mul(8);
    let num_mcus_x = u32::from(info.width).div_ceil(mcu_width);
    let num_mcus_y = u32::from(info.height).div_ceil(mcu_height);

    // JPEG frames accepted by the parser contain at most four components.
    // Keep their small geometry and outer storage owners inline; only the
    // image-sized coefficient and sample buffers need heap allocations.
    let component_count = info.components.len();
    debug_assert!(component_count <= 4);
    let mut comp_buf_width = [0usize; 4];
    let mut comp_buf_height = [0usize; 4];
    let mut comp_num_blocks = [0usize; 4];
    for (component_index, component) in info.components.iter().enumerate() {
        comp_buf_width[component_index] = bounded_usize(num_mcus_x)
            .saturating_mul(usize::from(component.h_samp))
            .saturating_mul(8);
        comp_buf_height[component_index] = bounded_usize(num_mcus_y)
            .saturating_mul(usize::from(component.v_samp))
            .saturating_mul(8);
        comp_num_blocks[component_index] = comp_buf_width[component_index]
            .div_euclid(8)
            .saturating_mul(comp_buf_height[component_index].div_euclid(8));
    }

    // Coefficient storage: [component][block_idx][64] in zigzag order
    let mut coeff_storage: Vec<Vec<[i32; 64]>> = (0..component_count)
        .map(|component_index| vec![[0i32; 64]; comp_num_blocks[component_index]])
        .collect();
    let mut comp_buffers: Vec<Vec<u8>> = (0..component_count)
        .map(|component_index| {
            vec![
                128u8;
                comp_buf_width[component_index].saturating_mul(comp_buf_height[component_index])
            ]
        })
        .collect();

    // ── Process scans ────────────────────────────────────────────────────
    // Per IJG jcmaster.c per_scan_setup: a scan with a single component is
    // NON-interleaved — it iterates that component's own block raster (1 block
    // per MCU, MCUs_per_row = width_in_blocks). A scan with >1 component is
    // interleaved over the image MCU grid (max_h_samp*8 × max_v_samp*8), with
    // h_samp*v_samp blocks per component per MCU.  Failing to distinguish
    // these scrambles block order for subsampled components (e.g. 4:2:0 chroma).
    for scan in info.scans.iter() {
        crate::codecs::error::check_cancelled(token)?;
        let extracted_segments = (scan.restart_interval != 0)
            .then(|| extract_entropy_segments(data, scan.entropy_start, scan.entropy_end));
        let single_segment = [(scan.entropy_start, scan.entropy_end)];
        let segments = extracted_segments
            .as_ref()
            .map_or(&single_segment[..], |value| &value.segments[..]);
        if segments.is_empty() {
            continue;
        }

        let is_dc_scan = scan.ss == 0 && scan.se == 0;
        let is_dc_first = is_dc_scan && scan.ah == 0;
        let is_dc_refine = is_dc_scan && scan.ah > 0;
        let is_ac_first = !is_dc_scan && scan.ah == 0;

        let interleaved = scan.components.len() > 1;

        // For non-interleaved scans, the "MCU" is a single block of the one
        // component, iterated over that component's block grid.
        let (scan_mcus_x, scan_mcus_y): (usize, usize) = if interleaved {
            (bounded_usize(num_mcus_x), bounded_usize(num_mcus_y))
        } else {
            let ci = scan.components[0].comp_index;
            (
                comp_buf_width[ci].div_euclid(8),
                comp_buf_height[ci].div_euclid(8),
            )
        };
        let scan_total_mcus = scan_mcus_x.saturating_mul(scan_mcus_y);

        let mcus_per_seg = if scan.restart_interval > 0 {
            usize::from(scan.restart_interval)
        } else {
            scan_total_mcus
        };

        // State persists across segments but resets at each restart
        let mut state = ProgressiveState::new(usize::from(info.num_components));

        for (seg_idx, &(seg_start, seg_end)) in segments.iter().enumerate() {
            #[cfg(target_arch = "aarch64")]
            let mut br = FastBitReader::new(data, seg_start, seg_end);
            #[cfg(not(target_arch = "aarch64"))]
            let mut br = BitReader::new(data, seg_start, seg_end);
            let mcu_offset = seg_idx.saturating_mul(mcus_per_seg);

            // Reset state at restart boundary (IJG process_restart)
            state.reset();

            for mcu_idx in 0..mcus_per_seg {
                let absolute_mcu = mcu_offset.saturating_add(mcu_idx);
                if absolute_mcu >= scan_total_mcus {
                    break;
                }
                let mcu_y = absolute_mcu.div_euclid(scan_mcus_x);
                let mcu_x = absolute_mcu.rem_euclid(scan_mcus_x);

                for scan_comp in &scan.components {
                    let comp_idx = scan_comp.comp_index;
                    let comp = &info.components[comp_idx];
                    let blocks_per_row = comp_buf_width[comp_idx].div_euclid(8);

                    // Compute the list of block indices this MCU covers.
                    // Interleaved: h_samp × v_samp blocks offset by the MCU's
                    //   top-left block (mcu_x*h_samp, mcu_y*v_samp).
                    // Non-interleaved: a single block at (mcu_x, mcu_y).
                    let mut block_indices = [0usize; 16];
                    let block_count = if interleaved {
                        let mut count = 0usize;
                        for by in 0..usize::from(comp.v_samp) {
                            for bx in 0..usize::from(comp.h_samp) {
                                block_indices[count] = mcu_y
                                    .saturating_mul(usize::from(comp.v_samp))
                                    .saturating_add(by)
                                    .saturating_mul(blocks_per_row)
                                    .saturating_add(
                                        mcu_x
                                            .saturating_mul(usize::from(comp.h_samp))
                                            .saturating_add(bx),
                                    );
                                count = count.saturating_add(1);
                            }
                        }
                        count
                    } else {
                        block_indices[0] =
                            mcu_y.saturating_mul(blocks_per_row).saturating_add(mcu_x);
                        1
                    };
                    let block_list = &block_indices[..block_count];

                    if is_dc_first {
                        let dc_table = scan
                            .dc_huff_tables
                            .get(usize::from(scan_comp.dc_tbl))
                            .and_then(Option::as_ref)
                            .malformed("missing progressive JPEG DC Huffman table")?;
                        for &block_idx in block_list {
                            coeff_storage[comp_idx][block_idx][0] = dc_first_block(
                                &mut br,
                                dc_table,
                                &mut state.dc_predictors[comp_idx],
                                scan.al,
                            )?;
                        }
                    } else if is_dc_refine {
                        let p1 = 1i32.wrapping_shl(u32::from(scan.al));
                        for &block_idx in block_list {
                            // DC refine: read 1 bit, OR into coefficient
                            let bit = br.read_padded(1);
                            if bit != 0 {
                                dc_refine_block(&mut coeff_storage[comp_idx][block_idx][0], p1);
                            }
                        }
                    } else if is_ac_first {
                        let ac_table = scan
                            .ac_huff_tables
                            .get(usize::from(scan_comp.ac_tbl))
                            .and_then(Option::as_ref)
                            .malformed("missing progressive JPEG AC Huffman table")?;
                        for &block_idx in block_list {
                            ac_first_block(
                                &mut br,
                                ac_table,
                                scan.ss,
                                scan.se,
                                scan.al,
                                &mut coeff_storage[comp_idx][block_idx],
                                &mut state.eobrun,
                            )?;
                        }
                    } else {
                        let ac_table = scan
                            .ac_huff_tables
                            .get(usize::from(scan_comp.ac_tbl))
                            .and_then(Option::as_ref)
                            .malformed("missing progressive JPEG AC Huffman table")?;
                        for &block_idx in block_list {
                            ac_refine_block(
                                &mut br,
                                ac_table,
                                scan.ss,
                                scan.se,
                                scan.al,
                                &mut coeff_storage[comp_idx][block_idx],
                                &mut state.eobrun,
                            )?;
                        }
                    }
                }

                if br.insufficient_data() {
                    break;
                }

                // A progressive scan can contain many MCUs inside one
                // entropy segment. Keep the ordinary path free of a per-MCU
                // token branch while giving callers a bounded checkpoint
                // after each completed 1,024-MCU batch.
                if let Some(token) = token
                    && absolute_mcu.saturating_add(1).is_multiple_of(1_024)
                {
                    crate::codecs::error::check_cancelled(Some(token))?;
                }
            }
        }
    }

    // ── Final IDCT + assembly ────────────────────────────────────────────
    let smooth_dc_only = info.scans.iter().all(|scan| scan.ss == 0 && scan.se == 0);
    let mut block_natural = [0i32; 64];
    let mut workspace = [0i32; 64];
    for comp_idx in 0..usize::from(info.num_components) {
        let comp = &info.components[comp_idx];
        let buf_w = comp_buf_width[comp_idx];
        let blocks_x = buf_w.div_euclid(8);
        let blocks_y = comp_buf_height[comp_idx].div_euclid(8);
        let quant_table = info
            .quant_tables
            .get(usize::from(comp.quant_tbl))
            .and_then(Option::as_ref)
            .malformed("missing JPEG quantization table")?;
        let mut quant_natural = [0i32; 64];
        for i in 0..64 {
            quant_natural[JPEG_NATURAL_ORDER[i]] = i32::from(quant_table[i]);
        }
        for (block_idx, coeffs) in coeff_storage[comp_idx].iter().enumerate() {
            let block_y = block_idx.div_euclid(blocks_x).saturating_mul(8);
            let block_x = block_idx.rem_euclid(blocks_x).saturating_mul(8);
            if smooth_dc_only {
                smooth_dc_only_block(
                    &coeff_storage[comp_idx],
                    blocks_x,
                    blocks_y,
                    block_idx,
                    &quant_natural,
                    &mut block_natural,
                );
                for i in 0..64 {
                    block_natural[i] = block_natural[i].saturating_mul(quant_natural[i]);
                }
            } else {
                #[cfg(target_arch = "aarch64")]
                let mut high_horizontal_nonzero = false;
                for i in 0..64 {
                    let natural_index = JPEG_NATURAL_ORDER[i];
                    block_natural[natural_index] = coeffs[i];
                    #[cfg(target_arch = "aarch64")]
                    {
                        high_horizontal_nonzero |= coeffs[i] != 0 && natural_index & 4 != 0;
                    }
                }
                #[cfg(target_arch = "aarch64")]
                if progressive_dequantize_block(
                    &mut block_natural,
                    &quant_natural,
                    &mut workspace,
                    &mut comp_buffers[comp_idx],
                    buf_w,
                    block_x,
                    block_y,
                    high_horizontal_nonzero,
                ) {
                    continue;
                }
            }
            jpeg_idct_islow(&mut block_natural, &mut workspace);
            for row in 0usize..8 {
                for col in 0usize..8 {
                    let natural_index = row.saturating_mul(8).saturating_add(col);
                    let px = block_natural[natural_index].clamp(0, 255).to_le_bytes()[0];
                    let bi = block_y
                        .saturating_add(row)
                        .saturating_mul(buf_w)
                        .saturating_add(block_x.saturating_add(col));
                    comp_buffers[comp_idx][bi] = px;
                }
            }
        }
    }

    // ── Assemble output ──────────────────────────────────────────────────
    let w = usize::from(info.width);
    let h = usize::from(info.height);
    let converter = YccColorConverter::shared();
    if info.num_components == 1 {
        let y_buf = &comp_buffers[0];
        let y_w = comp_buf_width[0];
        let mut pixels = Vec::with_capacity(w.saturating_mul(h));
        for y in 0..h {
            for x in 0..w {
                pixels.push(y_buf[y.saturating_mul(y_w).saturating_add(x)]);
            }
        }
        Ok(DecodedImage::new(
            u32::from(info.width),
            u32::from(info.height),
            pixels,
            ColorType::L8,
        ))
    } else if info.num_components == 3 {
        let y_buf = &comp_buffers[0];
        let y_w = comp_buf_width[0];
        let h_ratio = usize::from(info.max_h_samp.div_euclid(info.components[1].h_samp));
        let v_ratio = usize::from(info.max_v_samp.div_euclid(info.components[1].v_samp));
        let chroma_src_w = w.div_ceil(h_ratio);
        let chroma_src_h = h.div_ceil(v_ratio);
        let cb_cropped = crop_component(
            &comp_buffers[1],
            comp_buf_width[1],
            comp_buf_height[1],
            chroma_src_w,
            chroma_src_h,
        );
        let cr_cropped = crop_component(
            &comp_buffers[2],
            comp_buf_width[2],
            comp_buf_height[2],
            chroma_src_w,
            chroma_src_h,
        );
        let cb_up = fancy_upsample(
            &cb_cropped,
            chroma_src_w,
            chroma_src_h,
            h_ratio,
            v_ratio,
            w,
            h,
        );
        let cr_up = fancy_upsample(
            &cr_cropped,
            chroma_src_w,
            chroma_src_h,
            h_ratio,
            v_ratio,
            w,
            h,
        );
        let chroma_stride = chroma_src_w.saturating_mul(h_ratio);
        let mut pixels = vec![0u8; w.saturating_mul(h).saturating_mul(3)];
        for y in 0..h {
            let y_start = y.saturating_mul(y_w);
            let chroma_start = y.saturating_mul(chroma_stride);
            let output_start = y.saturating_mul(w).saturating_mul(3);
            converter.ycc_to_rgb_batch(
                &y_buf[y_start..y_start.saturating_add(w)],
                &cb_up[chroma_start..chroma_start.saturating_add(w)],
                &cr_up[chroma_start..chroma_start.saturating_add(w)],
                &mut pixels[output_start..output_start.saturating_add(w.saturating_mul(3))],
            );
        }
        Ok(DecodedImage::new(
            u32::from(info.width),
            u32::from(info.height),
            pixels,
            ColorType::Rgb8,
        ))
    } else {
        debug_assert_eq!(info.num_components, 4);
        let inverted = info.adobe_transform.is_some();
        let mut pixels = Vec::with_capacity(w.saturating_mul(h).saturating_mul(4));
        for y in 0..h {
            for x in 0..w {
                for component in 0..4 {
                    let horizontal_ratio = usize::from(
                        info.max_h_samp
                            .div_euclid(info.components[component].h_samp),
                    );
                    let vertical_ratio = usize::from(
                        info.max_v_samp
                            .div_euclid(info.components[component].v_samp),
                    );
                    let source_x = x.div_euclid(horizontal_ratio);
                    let source_y = y.div_euclid(vertical_ratio);
                    let sample = comp_buffers[component][source_y
                        .saturating_mul(comp_buf_width[component])
                        .saturating_add(source_x)];
                    pixels.push(if inverted {
                        255u8.saturating_sub(sample)
                    } else {
                        sample
                    });
                }
            }
        }
        Ok(DecodedImage::new(
            u32::from(info.width),
            u32::from(info.height),
            pixels,
            ColorType::Cmyk8,
        ))
    }
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    use super::parser::{FrameComponent, ScanComponent, ScanInfo};

    #[cfg(target_arch = "aarch64")]
    {
        let mut block = [0i32; 64];
        let mut quant = [1i32; 64];
        let mut workspace = [0i32; 64];
        let mut output = [0u8; 64];
        assert!(progressive_dequantize_block(
            &mut block,
            &quant,
            &mut workspace,
            &mut output,
            8,
            0,
            0,
            false,
        ));
        block[0] = i32::MAX;
        quant[0] = i32::MAX;
        assert!(!progressive_dequantize_block(
            &mut block,
            &quant,
            &mut workspace,
            &mut output,
            8,
            0,
            0,
            false,
        ));
    }

    assert_eq!(smooth_pred(1, 0, 0), 0);
    assert_eq!(smooth_pred(1, 1, 0), 0);
    assert_eq!(smooth_pred(1, 1, 2), 0);
    assert_eq!(smooth_pred(-512, 2, -1), -1);
    assert_eq!(smooth_pred(1_000_000, 1, 2), 3);

    let entropy = [0x00; 16];
    let zero =
        super::huffman::HuffTable::build(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], &[0]);
    let invalid_dc_category =
        super::huffman::HuffTable::build(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], &[64]);
    let overflow = super::huffman::HuffTable::build(
        &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        &[0xF1],
    );
    let eob = super::huffman::HuffTable::build(
        &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        &[0x10],
    );
    let new_coeff = super::huffman::HuffTable::build(
        &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        &[0x01],
    );
    let empty_table = super::huffman::HuffTable::build(&[0; 16], &[]);
    let one_new_coeff = super::huffman::HuffTable::build(
        &[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        &[0x00, 0x01],
    );
    let mut br = BitReader::new(&entropy, 0, entropy.len());
    let mut dc_pred = 0;
    assert!(dc_first_block(&mut br, &invalid_dc_category, &mut dc_pred, 0).is_err());
    let positive_dc = super::huffman::HuffTable::build(
        &[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        &[0, 1],
    );
    let mut br = BitReader::new(&[0x80], 0, 1);
    assert_eq!(
        dc_first_block(&mut br, &positive_dc, &mut dc_pred, 1),
        Ok(-2)
    );
    let mut br = BitReader::new(&entropy, 0, entropy.len());
    let mut coeffs = [0i32; 64];
    let mut eobrun = 1;
    assert_eq!(
        ac_first_block(&mut br, &zero, 1, 1, 0, &mut coeffs, &mut eobrun),
        Ok(0)
    );
    assert_eq!(
        ac_first_block(&mut br, &zero, 64, 63, 0, &mut coeffs, &mut eobrun),
        Ok(0)
    );
    let mut br = BitReader::new(&entropy, 0, entropy.len());
    assert_eq!(
        ac_first_block(&mut br, &zero, 64, 64, 0, &mut coeffs, &mut eobrun),
        Ok(0)
    );

    let mut br = BitReader::new(&entropy, 0, entropy.len());
    assert_eq!(
        ac_first_block(&mut br, &zero, 1, 1, 0, &mut coeffs, &mut eobrun),
        Ok(0)
    );

    let mut br = BitReader::new(&entropy, 0, entropy.len());
    assert_eq!(
        ac_first_block(&mut br, &eob, 1, 1, 0, &mut coeffs, &mut eobrun),
        Ok(0)
    );
    eobrun = 0;

    let zrl = super::huffman::HuffTable::build(
        &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        &[0xF0],
    );
    let mut br = BitReader::new(&entropy, 0, entropy.len());
    assert_eq!(
        ac_first_block(&mut br, &zrl, 1, 17, 0, &mut coeffs, &mut eobrun),
        Ok(0)
    );
    let mut br = BitReader::new(&entropy, 0, entropy.len());
    assert_eq!(
        ac_first_block(&mut br, &eob, 1, 2, 0, &mut coeffs, &mut eobrun),
        Ok(0)
    );
    eobrun = 0;
    let mut br = BitReader::new(&entropy, 0, entropy.len());
    assert_eq!(
        ac_first_block(&mut br, &new_coeff, 1, 1, 0, &mut coeffs, &mut eobrun),
        Ok(1)
    );

    let mut br = BitReader::new(&entropy, 0, entropy.len());
    assert_eq!(
        ac_first_block(&mut br, &overflow, 63, 63, 0, &mut coeffs, &mut eobrun),
        Ok(0)
    );
    let mut br = BitReader::new(&entropy, 0, entropy.len());
    assert_eq!(
        ac_first_block(&mut br, &overflow, 63, 80, 0, &mut coeffs, &mut eobrun),
        Ok(0)
    );

    let mut br = BitReader::new(&entropy, 0, entropy.len());
    assert!(ac_refine_block(&mut br, &zero, 63, 63, 0, &mut coeffs, &mut eobrun).is_ok());
    let mut br = BitReader::new(&entropy, 0, entropy.len());
    assert!(ac_refine_block(&mut br, &zero, 64, 64, 0, &mut coeffs, &mut eobrun).is_ok());
    let mut br = BitReader::new(&entropy, 0, entropy.len());
    assert!(ac_refine_block(&mut br, &new_coeff, 1, 1, 0, &mut coeffs, &mut eobrun).is_ok());
    let mut br = BitReader::new(&entropy, 0, entropy.len());
    eobrun = 0;
    assert!(ac_refine_block(&mut br, &eob, 1, 2, 0, &mut coeffs, &mut eobrun).is_ok());
    let entropy_ones = [0xff; 16];
    let mut br = BitReader::new(&entropy_ones, 0, entropy_ones.len());
    coeffs[1] = 2;
    assert!(ac_refine_block(&mut br, &new_coeff, 1, 1, 0, &mut coeffs, &mut eobrun).is_ok());
    let mut coeffs_bit_false = [0i32; 64];
    coeffs_bit_false[1] = 2;
    let mut br = BitReader::new(&entropy, 0, entropy.len());
    assert!(
        ac_refine_block(
            &mut br,
            &new_coeff,
            1,
            1,
            0,
            &mut coeffs_bit_false,
            &mut eobrun
        )
        .is_ok()
    );
    let mut coeffs_masked = [0i32; 64];
    coeffs_masked[1] = 1;
    let mut br = BitReader::new(&entropy_ones, 0, entropy_ones.len());
    assert!(
        ac_refine_block(
            &mut br,
            &new_coeff,
            1,
            1,
            0,
            &mut coeffs_masked,
            &mut eobrun
        )
        .is_ok()
    );
    let entropy_valid_one_bit_false = [0b1000_0000; 16];
    let mut coeffs_valid_bit_false = [0i32; 64];
    coeffs_valid_bit_false[1] = 2;
    let mut br = BitReader::new(
        &entropy_valid_one_bit_false,
        0,
        entropy_valid_one_bit_false.len(),
    );
    eobrun = 0;
    assert!(
        ac_refine_block(
            &mut br,
            &one_new_coeff,
            1,
            1,
            0,
            &mut coeffs_valid_bit_false,
            &mut eobrun
        )
        .is_ok()
    );
    let mut coeffs_valid_mask_false = [0i32; 64];
    coeffs_valid_mask_false[1] = 1;
    let mut br = BitReader::new(&entropy_ones, 0, entropy_ones.len());
    eobrun = 0;
    assert!(
        ac_refine_block(
            &mut br,
            &one_new_coeff,
            1,
            1,
            0,
            &mut coeffs_valid_mask_false,
            &mut eobrun
        )
        .is_ok()
    );
    let entropy_symbol_one_then_bit_one = [0b1110_0000u8; 16];
    let mut coeffs_non_marker_mask_false = [0i32; 64];
    coeffs_non_marker_mask_false[1] = 1;
    let mut br = BitReader::new(
        &entropy_symbol_one_then_bit_one,
        0,
        entropy_symbol_one_then_bit_one.len(),
    );
    eobrun = 0;
    assert!(
        ac_refine_block(
            &mut br,
            &one_new_coeff,
            1,
            1,
            0,
            &mut coeffs_non_marker_mask_false,
            &mut eobrun
        )
        .is_ok()
    );
    let mut coeffs_boundary = [0i32; 64];
    coeffs_boundary[63] = 1;
    let mut br = BitReader::new(&entropy_ones, 0, entropy_ones.len());
    assert!(
        ac_refine_block(
            &mut br,
            &new_coeff,
            63,
            80,
            0,
            &mut coeffs_boundary,
            &mut eobrun
        )
        .is_ok()
    );
    let mut br = BitReader::new(&entropy_ones, 0, entropy_ones.len());
    eobrun = 1;
    assert!(ac_refine_block(&mut br, &zero, 1, 1, 0, &mut coeffs, &mut eobrun).is_ok());
    let mut br = BitReader::new(&entropy_ones, 0, entropy_ones.len());
    eobrun = 1;
    assert!(ac_refine_block(&mut br, &zero, 64, 64, 0, &mut coeffs, &mut eobrun).is_ok());
    let mut coeffs_phase2 = [0i32; 64];
    coeffs_phase2[1] = 1;
    let mut br = BitReader::new(&entropy_ones, 0, entropy_ones.len());
    eobrun = 1;
    assert!(ac_refine_block(&mut br, &zero, 1, 1, 0, &mut coeffs_phase2, &mut eobrun).is_ok());
    let mut coeffs_phase2_false = [0i32; 64];
    coeffs_phase2_false[1] = 2;
    let mut br = BitReader::new(&entropy, 0, entropy.len());
    eobrun = 1;
    assert!(
        ac_refine_block(
            &mut br,
            &zero,
            1,
            1,
            0,
            &mut coeffs_phase2_false,
            &mut eobrun
        )
        .is_ok()
    );
    let entropy_refine_bit_one = [0b1000_0000u8; 16];
    let mut coeffs_phase2_mask_false = [0i32; 64];
    coeffs_phase2_mask_false[1] = 1;
    let mut br = BitReader::new(&entropy_refine_bit_one, 0, entropy_refine_bit_one.len());
    eobrun = 1;
    assert!(
        ac_refine_block(
            &mut br,
            &zero,
            1,
            1,
            0,
            &mut coeffs_phase2_mask_false,
            &mut eobrun
        )
        .is_ok()
    );

    let component = FrameComponent {
        id: 1,
        h_samp: 1,
        v_samp: 1,
        quant_tbl: 0,
    };
    let scan_component = ScanComponent {
        comp_index: 0,
        dc_tbl: 0,
        ac_tbl: 0,
    };
    let base_scan = |ss, se, ah, al, entropy_start, entropy_end| ScanInfo {
        components: vec![scan_component],
        entropy_start,
        entropy_end,
        ss,
        se,
        ah,
        al,
        restart_interval: 1,
        dc_huff_tables: vec![Some(zero.clone().into())],
        ac_huff_tables: vec![Some(zero.clone().into())],
    };
    let info = JpegInfo {
        width: 8,
        height: 8,
        num_components: 1,
        components: vec![component],
        quant_tables: vec![Some([1; 64])],
        dc_huff_tables: vec![Some(zero.clone().into())],
        ac_huff_tables: vec![Some(zero.clone().into())],
        scan_components: vec![scan_component],
        restart_interval: 0,
        entropy_has_restart_markers: false,
        entropy_start: 0,
        eoi_pos: 0,
        max_h_samp: 1,
        max_v_samp: 1,
        progressive: true,
        scans: vec![
            base_scan(0, 0, 0, 0, 0, 0),
            base_scan(0, 0, 0, 0, 0, 5),
            base_scan(1, 1, 0, 0, 0, 5),
            base_scan(1, 1, 1, 0, 0, 5),
        ],
        adobe_transform: None,
        metadata: Vec::new(),
    };
    let _ = progressive_reconstruct(&info, &[0, 0, 0xFF, 0xD0, 0], None);
    let mut fast_info = info.clone();
    fast_info.scans = vec![base_scan(1, 1, 0, 0, 0, 1)];
    let _ = progressive_reconstruct(&fast_info, &[0], None);

    let failing_scan = |ss, se, ah, al| ScanInfo {
        components: vec![scan_component],
        entropy_start: 0,
        entropy_end: 1,
        ss,
        se,
        ah,
        al,
        restart_interval: 1,
        dc_huff_tables: vec![Some(empty_table.clone().into())],
        ac_huff_tables: vec![Some(empty_table.clone().into())],
    };
    let failing_info = |scans| JpegInfo {
        width: 8,
        height: 8,
        num_components: 1,
        components: vec![component],
        quant_tables: vec![Some([1; 64])],
        dc_huff_tables: vec![Some(zero.clone().into())],
        ac_huff_tables: vec![Some(zero.clone().into())],
        scan_components: vec![scan_component],
        restart_interval: 0,
        entropy_has_restart_markers: false,
        entropy_start: 0,
        eoi_pos: 0,
        max_h_samp: 1,
        max_v_samp: 1,
        progressive: true,
        scans,
        adobe_transform: None,
        metadata: Vec::new(),
    };
    let _ = progressive_reconstruct(&failing_info(vec![failing_scan(0, 0, 0, 0)]), &[0], None);
    let _ = progressive_reconstruct(&failing_info(vec![failing_scan(1, 1, 0, 0)]), &[0], None);
    let _ = progressive_reconstruct(&failing_info(vec![failing_scan(1, 1, 1, 0)]), &[0], None);
    let missing_dc_scan = ScanInfo {
        components: vec![scan_component],
        entropy_start: 0,
        entropy_end: 1,
        ss: 0,
        se: 0,
        ah: 0,
        al: 0,
        restart_interval: 1,
        dc_huff_tables: vec![None],
        ac_huff_tables: vec![Some(zero.clone().into())],
    };
    let missing_ac_first_scan = ScanInfo {
        components: vec![scan_component],
        entropy_start: 0,
        entropy_end: 1,
        ss: 1,
        se: 1,
        ah: 0,
        al: 0,
        restart_interval: 1,
        dc_huff_tables: vec![Some(zero.clone().into())],
        ac_huff_tables: vec![None],
    };
    let missing_ac_refine_scan = ScanInfo {
        ah: 1,
        ..missing_ac_first_scan.clone()
    };
    let _ = progressive_reconstruct(&failing_info(vec![missing_dc_scan]), &[0], None);
    let _ = progressive_reconstruct(&failing_info(vec![missing_ac_first_scan]), &[0], None);
    let _ = progressive_reconstruct(&failing_info(vec![missing_ac_refine_scan]), &[0], None);

    let cmyk_components = (0..4)
        .map(|id| FrameComponent {
            id,
            h_samp: 1,
            v_samp: 1,
            quant_tbl: 0,
        })
        .collect::<Vec<_>>();
    let cmyk_info = JpegInfo {
        width: 1,
        height: 1,
        num_components: 4,
        components: cmyk_components,
        quant_tables: vec![Some([1; 64])],
        dc_huff_tables: vec![Some(zero.clone().into())],
        ac_huff_tables: vec![Some(zero.into())],
        scan_components: Vec::new(),
        restart_interval: 0,
        entropy_has_restart_markers: false,
        entropy_start: 0,
        eoi_pos: 0,
        max_h_samp: 1,
        max_v_samp: 1,
        progressive: true,
        scans: Vec::new(),
        adobe_transform: None,
        metadata: Vec::new(),
    };
    let _ = progressive_reconstruct(&cmyk_info, &[], None);
    let cmyk_info = JpegInfo {
        adobe_transform: Some(0),
        ..cmyk_info
    };
    let _ = progressive_reconstruct(&cmyk_info, &[], None);
}
