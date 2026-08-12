// Modified Rust port copyright (c) 2026 Appunni M.
// Derived from libjpeg-turbo/IJG sources; see third_party/libjpeg-turbo/.

use crate::codecs::{CodecError, CodecResult, OptionCodecExt};
use crate::types::{ColorType, DecodedImage};

use super::bit_reader::BitReader;
#[cfg(target_arch = "aarch64")]
use super::bit_reader::FastBitReader;
use super::huffman::HuffTable;
use super::idct::{self, YccColorConverter, extend, jpeg_idct_islow};
use super::parser::{JpegInfo, parse_jpeg};
use super::progressive::progressive_reconstruct;
use super::upsample::{crop_component, fancy_upsample};
#[cfg(target_arch = "aarch64")]
use wide::bytemuck::{cast, pod_read_unaligned};
#[cfg(target_arch = "aarch64")]
use wide::{i16x8, u8x16, u16x8};

#[cfg(target_arch = "aarch64")]
const BASELINE_MCU_CHECKPOINT: usize = 1_024;

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn check_baseline_mcu_checkpoint(
    token: Option<&crate::CancellationToken>,
    completed_mcus: usize,
) -> CodecResult<()> {
    if let Some(token) = token
        && completed_mcus != 0
        && completed_mcus.is_multiple_of(BASELINE_MCU_CHECKPOINT)
    {
        crate::codecs::error::check_cancelled(Some(token))?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BlockKind {
    Full,
    DcOnly,
}

// ── Entropy Decoding ──────────────────────────────────────────────────────

pub(super) fn decode_block(
    br: &mut BitReader,
    dc_table: &HuffTable,
    ac_table: &HuffTable,
    last_dc: &mut i32,
    block_natural: &mut [i32; 64],
) -> CodecResult<BlockKind> {
    block_natural.fill(0);

    let dc_cat = dc_table.decode(br)?;
    if dc_cat > 15 {
        return Err(CodecError::Malformed(
            "invalid JPEG DC coefficient category".to_owned(),
        ));
    }
    if dc_cat > 0 {
        let bits = br.read_padded_bits(u32::from(dc_cat));
        *last_dc = last_dc.saturating_add(extend(bits, dc_cat));
    }
    block_natural[0] = *last_dc;

    let mut k = 1usize;
    let mut dc_only = true;
    while k < 64 {
        let sym = ac_table.decode(br)?;
        if sym == 0x00 {
            break;
        }
        dc_only = false;
        let run = usize::from(sym >> 4);
        let size = sym & 0x0F;
        if size == 0 && run == 15 {
            k = k.saturating_add(16);
            continue;
        }
        if size > 0 {
            k = k.saturating_add(run);
            if k >= 64 {
                break;
            }
            // JPEG AC symbols encode at most 15 coefficient bits. The bit
            // reader matches libjpeg by zero-padding exhausted entropy data to
            // MIN_GET_BITS, so this read cannot fail on the AC path.
            let bits = br.read_padded_bits(u32::from(size));
            block_natural[idct::JPEG_NATURAL_ORDER[k]] = extend(bits, size);
            k = k.saturating_add(1);
        } else {
            return Err(CodecError::Malformed(
                "invalid JPEG AC run-length symbol".to_owned(),
            ));
        }
    }
    Ok(if dc_only {
        BlockKind::DcOnly
    } else {
        BlockKind::Full
    })
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn decode_block_fast(
    br: &mut FastBitReader,
    dc_table: &HuffTable,
    ac_table: &HuffTable,
    last_dc: &mut i32,
    block_natural: &mut [i32; 64],
) -> CodecResult<(BlockKind, bool)> {
    block_natural.fill(0);

    let dc_category = dc_table.decode_fast(br)?;
    if dc_category > 15 {
        return Err(CodecError::Malformed(
            "invalid JPEG DC coefficient category".to_owned(),
        ));
    }
    if dc_category > 0 {
        let bits = br.read_padded_bits(u32::from(dc_category));
        *last_dc = last_dc.saturating_add(extend(bits, dc_category));
    }
    block_natural[0] = *last_dc;

    let mut coefficient = 1usize;
    let mut dc_only = true;
    let mut high_horizontal_nonzero = false;
    while coefficient < 64 {
        if let Some((first, first_bits, second, second_bits, consumed)) =
            ac_table.peek_general_pair_fast(br)
        {
            let first_position = coefficient.saturating_add(usize::from(first >> 4));
            let second_position = first_position
                .saturating_add(1)
                .saturating_add(usize::from(second >> 4));
            if first_position < 64 && second_position < 64 {
                br.drop_bits(consumed);
                let first_size = first & 0x0F;
                let second_size = second & 0x0F;
                let first_natural = idct::JPEG_NATURAL_ORDER[first_position];
                let second_natural = idct::JPEG_NATURAL_ORDER[second_position];
                block_natural[first_natural] = extend(first_bits, first_size);
                block_natural[second_natural] = extend(second_bits, second_size);
                high_horizontal_nonzero |= first_natural & 4 != 0;
                high_horizontal_nonzero |= second_natural & 4 != 0;
                coefficient = second_position.saturating_add(1);
                dc_only = false;
                continue;
            }
        }

        let symbol = ac_table.decode_fast(br)?;
        if symbol == 0 {
            break;
        }
        dc_only = false;

        if symbol < 0x10 {
            let size = symbol;
            let bits = br.read_padded_bits(u32::from(size));
            let natural = idct::JPEG_NATURAL_ORDER[coefficient];
            block_natural[natural] = extend(bits, size);
            high_horizontal_nonzero |= natural & 4 != 0;
            coefficient = coefficient.saturating_add(1);
            continue;
        }

        let run = usize::from(symbol >> 4);
        let size = symbol & 0x0F;
        if size == 0 && run == 15 {
            coefficient = coefficient.saturating_add(16);
            continue;
        }
        if size == 0 {
            return Err(CodecError::Malformed(
                "invalid JPEG AC run-length symbol".to_owned(),
            ));
        }
        coefficient = coefficient.saturating_add(run);
        if coefficient >= 64 {
            break;
        }
        let bits = br.read_padded_bits(u32::from(size));
        let natural = idct::JPEG_NATURAL_ORDER[coefficient];
        block_natural[natural] = extend(bits, size);
        high_horizontal_nonzero |= natural & 4 != 0;
        coefficient = coefficient.saturating_add(1);
    }

    Ok((
        if dc_only {
            BlockKind::DcOnly
        } else {
            BlockKind::Full
        },
        high_horizontal_nonzero,
    ))
}

#[cfg(target_arch = "aarch64")]
#[allow(
    clippy::too_many_arguments,
    reason = "the block operation receives explicit entropy, transform, and destination state"
)]
#[inline(always)]
fn decode_and_store_block_fast(
    br: &mut FastBitReader,
    dc_table: &HuffTable,
    ac_table: &HuffTable,
    last_dc: &mut i32,
    quant_natural: &[i32; 64],
    destination: &mut [u8],
    stride: usize,
    block_x: usize,
    block_y: usize,
    block_natural: &mut [i32; 64],
    workspace: &mut [i32; 64],
) -> CodecResult<()> {
    let (kind, high_horizontal_nonzero) =
        decode_block_fast(br, dc_table, ac_table, last_dc, block_natural)
            .map_err(|error| error.context("baseline block"))?;
    if kind == BlockKind::DcOnly {
        let dc = block_natural[0].saturating_mul(quant_natural[0]);
        let value = idct::dc_only_output(dc);
        for row in 0usize..8 {
            let start = block_y
                .saturating_add(row)
                .saturating_mul(stride)
                .saturating_add(block_x);
            destination[start..start.saturating_add(8)].fill(value);
        }
        return Ok(());
    }

    if block_natural[0].checked_mul(quant_natural[0]).is_some() {
        idct::jpeg_idct_islow_dequantized_to_u8_safe(
            block_natural,
            quant_natural,
            workspace,
            destination,
            stride,
            block_x,
            block_y,
            high_horizontal_nonzero,
        );
        return Ok(());
    }
    for (coefficient, &quantizer) in block_natural.iter_mut().zip(quant_natural) {
        *coefficient = coefficient.saturating_mul(quantizer);
    }
    idct::jpeg_idct_islow_to_u8_safe(
        block_natural,
        workspace,
        destination,
        stride,
        block_x,
        block_y,
    );
    Ok(())
}

// ── Image Reconstruction (baseline) ───────────────────────────────────────

#[cfg(target_arch = "aarch64")]
#[allow(
    clippy::too_many_arguments,
    reason = "the specialized MCU loop receives the validated frame state and reusable buffers"
)]
fn reconstruct_baseline_420_fast(
    info: &JpegInfo,
    entropy_segments: &EntropySegments,
    data: &[u8],
    num_mcus_x: u32,
    num_mcus_y: u32,
    component_widths: &[usize],
    quant_tables: &[[i32; 64]],
    component_buffers: &mut [Vec<u8>],
    dc_predictors: &mut [i32],
    block_natural: &mut [i32; 64],
    workspace: &mut [i32; 64],
) -> CodecResult<bool> {
    if info.progressive
        || info.num_components != 3
        || info.restart_interval != 0
        || info.components.len() != 3
        || info.scan_components.len() != 3
        || info.components[0].h_samp != 2
        || info.components[0].v_samp != 2
        || info.components[1].h_samp != 1
        || info.components[1].v_samp != 1
        || info.components[2].h_samp != 1
        || info.components[2].v_samp != 1
        || info.max_h_samp != 2
        || info.max_v_samp != 2
        || info.scan_components[0].comp_index != 0
        || info.scan_components[1].comp_index != 1
        || info.scan_components[2].comp_index != 2
        || entropy_segments.segments.len() != 1
    {
        return Ok(false);
    }

    let y_scan = info.scan_components[0];
    let cb_scan = info.scan_components[1];
    let cr_scan = info.scan_components[2];
    let y_dc = info
        .dc_huff_tables
        .get(usize::from(y_scan.dc_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG DC Huffman table")?;
    let y_ac = info
        .ac_huff_tables
        .get(usize::from(y_scan.ac_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG AC Huffman table")?;
    let cb_dc = info
        .dc_huff_tables
        .get(usize::from(cb_scan.dc_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG DC Huffman table")?;
    let cb_ac = info
        .ac_huff_tables
        .get(usize::from(cb_scan.ac_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG AC Huffman table")?;
    let cr_dc = info
        .dc_huff_tables
        .get(usize::from(cr_scan.dc_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG DC Huffman table")?;
    let cr_ac = info
        .ac_huff_tables
        .get(usize::from(cr_scan.ac_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG AC Huffman table")?;

    let (entropy_start, entropy_end) = entropy_segments.segments[0];
    let mut reader = FastBitReader::new(data, entropy_start, entropy_end);
    dc_predictors.fill(0);
    let columns = bounded_usize(num_mcus_x);
    let rows = bounded_usize(num_mcus_y);
    'rows: for mcu_y in 0..rows {
        for mcu_x in 0..columns {
            let y_x = mcu_x.saturating_mul(16);
            let y_y = mcu_y.saturating_mul(16);
            for block_y in 0..2usize {
                for block_x in 0..2usize {
                    decode_and_store_block_fast(
                        &mut reader,
                        y_dc,
                        y_ac,
                        &mut dc_predictors[0],
                        &quant_tables[0],
                        &mut component_buffers[0],
                        component_widths[0],
                        y_x.saturating_add(block_x.saturating_mul(8)),
                        y_y.saturating_add(block_y.saturating_mul(8)),
                        block_natural,
                        workspace,
                    )?;
                }
            }

            let chroma_x = mcu_x.saturating_mul(8);
            let chroma_y = mcu_y.saturating_mul(8);
            decode_and_store_block_fast(
                &mut reader,
                cb_dc,
                cb_ac,
                &mut dc_predictors[1],
                &quant_tables[1],
                &mut component_buffers[1],
                component_widths[1],
                chroma_x,
                chroma_y,
                block_natural,
                workspace,
            )?;
            decode_and_store_block_fast(
                &mut reader,
                cr_dc,
                cr_ac,
                &mut dc_predictors[2],
                &quant_tables[2],
                &mut component_buffers[2],
                component_widths[2],
                chroma_x,
                chroma_y,
                block_natural,
                workspace,
            )?;

            if reader.insufficient_data() {
                break 'rows;
            }
        }
    }
    Ok(true)
}

#[cfg(target_arch = "aarch64")]
#[allow(
    clippy::too_many_arguments,
    reason = "the row decoder receives the validated component tables and reusable row buffers"
)]
#[inline(never)]
fn decode_baseline_420_row_fast(
    reader: &mut FastBitReader,
    y_dc: &HuffTable,
    y_ac: &HuffTable,
    cb_dc: &HuffTable,
    cb_ac: &HuffTable,
    cr_dc: &HuffTable,
    cr_ac: &HuffTable,
    dc_predictors: &mut [i32; 3],
    quant_tables: &[[i32; 64]],
    y_buffer: &mut [u8],
    cb_buffer: &mut [u8],
    cr_buffer: &mut [u8],
    y_stride: usize,
    chroma_stride: usize,
    valid_chroma_width: usize,
    mcu_columns: usize,
    block_natural: &mut [i32; 64],
    workspace: &mut [i32; 64],
) -> CodecResult<bool> {
    y_buffer.fill(128);
    cb_buffer.fill(128);
    cr_buffer.fill(128);

    for mcu_x in 0..mcu_columns {
        let y_x = mcu_x.saturating_mul(16);
        for block_y in 0..2usize {
            for block_x in 0..2usize {
                decode_and_store_block_fast(
                    reader,
                    y_dc,
                    y_ac,
                    &mut dc_predictors[0],
                    &quant_tables[0],
                    y_buffer,
                    y_stride,
                    y_x.saturating_add(block_x.saturating_mul(8)),
                    block_y.saturating_mul(8),
                    block_natural,
                    workspace,
                )?;
            }
        }

        let chroma_x = 1usize.saturating_add(mcu_x.saturating_mul(8));
        decode_and_store_block_fast(
            reader,
            cb_dc,
            cb_ac,
            &mut dc_predictors[1],
            &quant_tables[1],
            cb_buffer,
            chroma_stride,
            chroma_x,
            0,
            block_natural,
            workspace,
        )?;
        decode_and_store_block_fast(
            reader,
            cr_dc,
            cr_ac,
            &mut dc_predictors[2],
            &quant_tables[2],
            cr_buffer,
            chroma_stride,
            chroma_x,
            0,
            block_natural,
            workspace,
        )?;

        if reader.insufficient_data() {
            return Ok(false);
        }
    }

    let padded_chroma_width = mcu_columns.saturating_mul(8);
    for row in 0..8usize {
        let row_start = row.saturating_mul(chroma_stride);
        cb_buffer[row_start] = cb_buffer[row_start.saturating_add(1)];
        cr_buffer[row_start] = cr_buffer[row_start.saturating_add(1)];
        let last = row_start
            .saturating_add(valid_chroma_width)
            .min(row_start.saturating_add(padded_chroma_width));
        let cb_edge = cb_buffer[last];
        let cr_edge = cr_buffer[last];
        cb_buffer[last.saturating_add(1)..row_start.saturating_add(chroma_stride)].fill(cb_edge);
        cr_buffer[last.saturating_add(1)..row_start.saturating_add(chroma_stride)].fill(cr_edge);
    }
    Ok(true)
}

#[cfg(target_arch = "aarch64")]
#[allow(
    clippy::too_many_arguments,
    reason = "the row decoder receives the validated component tables and reusable row buffers"
)]
#[inline(never)]
fn decode_baseline_422_row_fast(
    reader: &mut FastBitReader,
    y_dc: &HuffTable,
    y_ac: &HuffTable,
    cb_dc: &HuffTable,
    cb_ac: &HuffTable,
    cr_dc: &HuffTable,
    cr_ac: &HuffTable,
    dc_predictors: &mut [i32; 3],
    quant_tables: &[[i32; 64]],
    y_buffer: &mut [u8],
    cb_buffer: &mut [u8],
    cr_buffer: &mut [u8],
    y_stride: usize,
    chroma_stride: usize,
    valid_chroma_width: usize,
    mcu_columns: usize,
    block_natural: &mut [i32; 64],
    workspace: &mut [i32; 64],
) -> CodecResult<bool> {
    y_buffer.fill(128);
    cb_buffer.fill(128);
    cr_buffer.fill(128);

    for mcu_x in 0..mcu_columns {
        let y_x = mcu_x.saturating_mul(16);
        decode_and_store_block_fast(
            reader,
            y_dc,
            y_ac,
            &mut dc_predictors[0],
            &quant_tables[0],
            y_buffer,
            y_stride,
            y_x,
            0,
            block_natural,
            workspace,
        )?;
        decode_and_store_block_fast(
            reader,
            y_dc,
            y_ac,
            &mut dc_predictors[0],
            &quant_tables[0],
            y_buffer,
            y_stride,
            y_x.saturating_add(8),
            0,
            block_natural,
            workspace,
        )?;

        let chroma_x = 1usize.saturating_add(mcu_x.saturating_mul(8));
        decode_and_store_block_fast(
            reader,
            cb_dc,
            cb_ac,
            &mut dc_predictors[1],
            &quant_tables[1],
            cb_buffer,
            chroma_stride,
            chroma_x,
            0,
            block_natural,
            workspace,
        )?;
        decode_and_store_block_fast(
            reader,
            cr_dc,
            cr_ac,
            &mut dc_predictors[2],
            &quant_tables[2],
            cr_buffer,
            chroma_stride,
            chroma_x,
            0,
            block_natural,
            workspace,
        )?;

        if reader.insufficient_data() {
            return Ok(false);
        }
    }

    let padded_chroma_width = mcu_columns.saturating_mul(8);
    for row in 0..8usize {
        let row_start = row.saturating_mul(chroma_stride);
        cb_buffer[row_start] = cb_buffer[row_start.saturating_add(1)];
        cr_buffer[row_start] = cr_buffer[row_start.saturating_add(1)];
        let last = row_start
            .saturating_add(valid_chroma_width)
            .min(row_start.saturating_add(padded_chroma_width));
        let cb_edge = cb_buffer[last];
        let cr_edge = cr_buffer[last];
        cb_buffer[last.saturating_add(1)..row_start.saturating_add(chroma_stride)].fill(cb_edge);
        cr_buffer[last.saturating_add(1)..row_start.saturating_add(chroma_stride)].fill(cr_edge);
    }
    Ok(true)
}

#[cfg(target_arch = "aarch64")]
const INTERLEAVE_EIGHT_BYTES: u8x16 =
    u8x16::new([0, 8, 1, 9, 2, 10, 3, 11, 4, 12, 5, 13, 6, 14, 7, 15]);
#[cfg(target_arch = "aarch64")]
const CMYK_PAIR_ORDER: u8x16 = u8x16::new([0, 2, 1, 3, 4, 6, 5, 7, 8, 10, 9, 11, 12, 14, 13, 15]);
#[cfg(target_arch = "aarch64")]
const UPSAMPLE_THREE: u16x8 = u16x8::new([3; 8]);
#[cfg(target_arch = "aarch64")]
const UPSAMPLE_EIGHT: u16x8 = u16x8::new([8; 8]);
#[cfg(target_arch = "aarch64")]
const UPSAMPLE_SEVEN: u16x8 = u16x8::new([7; 8]);
#[cfg(target_arch = "aarch64")]
const UPSAMPLE_ONE: u16x8 = u16x8::new([1; 8]);
#[cfg(target_arch = "aarch64")]
const UPSAMPLE_TWO: u16x8 = u16x8::new([2; 8]);

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn load_eight_chroma_samples(samples: &[u8; 16]) -> u16x8 {
    let packed = pod_read_unaligned::<u8x16>(samples);
    u16x8::from_u8x16_low(packed)
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn interleaved_chroma_pair(even: u16x8, odd: u16x8) -> [u8; 16] {
    let packed = u8x16::narrow_i16x8(cast::<u16x8, i16x8>(even), cast::<u16x8, i16x8>(odd));
    packed.swizzle_relaxed(INTERLEAVE_EIGHT_BYTES).to_array()
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn invert_interleave_cmyk_eight(
    cyan: &[u8; 8],
    magenta: &[u8; 8],
    yellow: &[u8; 8],
    black: &[u8; 8],
) -> ([u8; 16], [u8; 16]) {
    let cyan = !cast::<[u64; 2], u8x16>([u64::from_ne_bytes(*cyan), 0]);
    let magenta = !cast::<[u64; 2], u8x16>([u64::from_ne_bytes(*magenta), 0]);
    let yellow = !cast::<[u64; 2], u8x16>([u64::from_ne_bytes(*yellow), 0]);
    let black = !cast::<[u64; 2], u8x16>([u64::from_ne_bytes(*black), 0]);
    let cyan_magenta = u8x16::unpack_low(cyan, magenta);
    let yellow_black = u8x16::unpack_low(yellow, black);
    (
        u8x16::unpack_low(cyan_magenta, yellow_black)
            .swizzle_relaxed(CMYK_PAIR_ORDER)
            .to_array(),
        u8x16::unpack_high(cyan_magenta, yellow_black)
            .swizzle_relaxed(CMYK_PAIR_ORDER)
            .to_array(),
    )
}

#[cfg(target_arch = "aarch64")]
#[expect(
    clippy::arithmetic_side_effects,
    reason = "chroma samples are 8-bit and the fixed upsample filter stays within i32 range"
)]
#[inline(always)]
fn fancy_upsample_h2v1_eight_safe(
    center: &[u8; 16],
    left: &[u8; 16],
    right: &[u8; 16],
) -> [u8; 16] {
    let center = load_eight_chroma_samples(center);
    let left = load_eight_chroma_samples(left);
    let right = load_eight_chroma_samples(right);
    let center_three = center * UPSAMPLE_THREE;
    let even = (center_three + left + UPSAMPLE_ONE).unbounded_shr_scalar(2);
    let odd = (center_three + right + UPSAMPLE_TWO).unbounded_shr_scalar(2);
    interleaved_chroma_pair(even, odd)
}

#[cfg(target_arch = "aarch64")]
#[inline(never)]
fn fancy_upsample_h2v1_row_safe(
    source: &[u8],
    source_width: usize,
    source_stride: usize,
    source_row: usize,
    output: &mut [u8],
) {
    let row_start = source_row.saturating_mul(source_stride);
    let row = &source[row_start..row_start.saturating_add(source_stride)];
    for source_start in (0usize..source_width).step_by(8) {
        let center_start = source_start.saturating_add(1);
        let right_start = source_start.saturating_add(2);
        let center: &[u8; 16] = row[center_start..center_start.saturating_add(16)]
            .try_into()
            .unwrap_or_else(|_| unreachable!("center chroma window has invalid length"));
        let left: &[u8; 16] = row[source_start..source_start.saturating_add(16)]
            .try_into()
            .unwrap_or_else(|_| unreachable!("left chroma window has invalid length"));
        let right: &[u8; 16] = row[right_start..right_start.saturating_add(16)]
            .try_into()
            .unwrap_or_else(|_| unreachable!("right chroma window has invalid length"));
        let upsampled = fancy_upsample_h2v1_eight_safe(center, left, right);
        let output_start = source_start.saturating_mul(2);
        output[output_start..output_start.saturating_add(16)].copy_from_slice(&upsampled);
    }
}

#[cfg(target_arch = "aarch64")]
#[allow(
    clippy::too_many_arguments,
    reason = "the fixed kernel receives the exact guarded windows for both output rows"
)]
#[expect(
    clippy::arithmetic_side_effects,
    reason = "chroma samples are 8-bit and the fixed upsample filter stays within i32 range"
)]
#[inline(never)]
fn fancy_upsample_h2v2_eight_pair_safe(
    center: &[u8; 16],
    left: &[u8; 16],
    right: &[u8; 16],
    above_center: &[u8; 16],
    above_left: &[u8; 16],
    above_right: &[u8; 16],
    below_center: &[u8; 16],
    below_left: &[u8; 16],
    below_right: &[u8; 16],
) -> ([u8; 16], [u8; 16]) {
    let center = load_eight_chroma_samples(center);
    let left = load_eight_chroma_samples(left);
    let right = load_eight_chroma_samples(right);
    let above_center = load_eight_chroma_samples(above_center);
    let above_left = load_eight_chroma_samples(above_left);
    let above_right = load_eight_chroma_samples(above_right);
    let below_center = load_eight_chroma_samples(below_center);
    let below_left = load_eight_chroma_samples(below_left);
    let below_right = load_eight_chroma_samples(below_right);

    let center_three = center * UPSAMPLE_THREE;
    let left_three = left * UPSAMPLE_THREE;
    let right_three = right * UPSAMPLE_THREE;
    let top_center = center_three + above_center;
    let top_even = (top_center * UPSAMPLE_THREE + left_three + above_left + UPSAMPLE_EIGHT)
        .unbounded_shr_scalar(4);
    let top_odd = (top_center * UPSAMPLE_THREE + right_three + above_right + UPSAMPLE_SEVEN)
        .unbounded_shr_scalar(4);
    let bottom_center = center_three + below_center;
    let bottom_even = (bottom_center * UPSAMPLE_THREE + left_three + below_left + UPSAMPLE_EIGHT)
        .unbounded_shr_scalar(4);
    let bottom_odd = (bottom_center * UPSAMPLE_THREE + right_three + below_right + UPSAMPLE_SEVEN)
        .unbounded_shr_scalar(4);
    (
        interleaved_chroma_pair(top_even, top_odd),
        interleaved_chroma_pair(bottom_even, bottom_odd),
    )
}

#[cfg(target_arch = "aarch64")]
#[allow(
    clippy::too_many_arguments,
    reason = "the exact h2v2 row filter receives its three-row guarded window explicitly"
)]
#[inline(never)]
fn fancy_upsample_h2v2_row_pair_safe(
    previous: &[u8],
    current: &[u8],
    next: &[u8],
    source_width: usize,
    source_stride: usize,
    source_row: usize,
    source_height: usize,
    output_stride: usize,
    output: &mut [u8],
) {
    debug_assert!(output_stride >= source_width.saturating_mul(2));
    let (above, above_row) = if source_row == 0 {
        (previous, 7usize)
    } else {
        (current, source_row.saturating_sub(1))
    };
    let (below, below_row) = if source_row.saturating_add(1) >= source_height {
        (next, 0usize)
    } else {
        (current, source_row.saturating_add(1))
    };
    let current_row_start = source_row.saturating_mul(source_stride);
    let above_row_start = above_row.saturating_mul(source_stride);
    let below_row_start = below_row.saturating_mul(source_stride);
    let current_row = &current[current_row_start..current_row_start.saturating_add(source_stride)];
    let above_row = &above[above_row_start..above_row_start.saturating_add(source_stride)];
    let below_row = &below[below_row_start..below_row_start.saturating_add(source_stride)];
    let (top_output, bottom_output) = output.split_at_mut(output_stride);

    for source_start in (0..source_width).step_by(8) {
        let center = source_start.saturating_add(1);
        let right = source_start.saturating_add(2);
        let end = source_start.saturating_add(16);
        let right_end = right.saturating_add(16);
        let current_center: &[u8; 16] = current_row[center..center.saturating_add(16)]
            .try_into()
            .unwrap_or_else(|_| unreachable!("current center window has invalid length"));
        let current_left: &[u8; 16] = current_row[source_start..end]
            .try_into()
            .unwrap_or_else(|_| unreachable!("current left window has invalid length"));
        let current_right: &[u8; 16] = current_row[right..right_end]
            .try_into()
            .unwrap_or_else(|_| unreachable!("current right window has invalid length"));
        let above_center: &[u8; 16] = above_row[center..center.saturating_add(16)]
            .try_into()
            .unwrap_or_else(|_| unreachable!("above center window has invalid length"));
        let above_left: &[u8; 16] = above_row[source_start..end]
            .try_into()
            .unwrap_or_else(|_| unreachable!("above left window has invalid length"));
        let above_right: &[u8; 16] = above_row[right..right_end]
            .try_into()
            .unwrap_or_else(|_| unreachable!("above right window has invalid length"));
        let below_center: &[u8; 16] = below_row[center..center.saturating_add(16)]
            .try_into()
            .unwrap_or_else(|_| unreachable!("below center window has invalid length"));
        let below_left: &[u8; 16] = below_row[source_start..end]
            .try_into()
            .unwrap_or_else(|_| unreachable!("below left window has invalid length"));
        let below_right: &[u8; 16] = below_row[right..right_end]
            .try_into()
            .unwrap_or_else(|_| unreachable!("below right window has invalid length"));
        let (top, bottom) = fancy_upsample_h2v2_eight_pair_safe(
            current_center,
            current_left,
            current_right,
            above_center,
            above_left,
            above_right,
            below_center,
            below_left,
            below_right,
        );
        let output_start = source_start.saturating_mul(2);
        top_output[output_start..output_start.saturating_add(16)].copy_from_slice(&top);
        bottom_output[output_start..output_start.saturating_add(16)].copy_from_slice(&bottom);
    }
}

/// Decode a baseline grayscale scan directly into the final luminance
/// allocation, using one initialized edge block only for partial MCUs.
#[cfg(target_arch = "aarch64")]
#[allow(
    clippy::too_many_arguments,
    reason = "the direct path keeps validated JPEG state and its final output explicit"
)]
fn reconstruct_baseline_grayscale_direct_safe(
    info: &JpegInfo,
    entropy_segments: &EntropySegments,
    data: &[u8],
    token: Option<&crate::CancellationToken>,
    num_mcus_x: u32,
    num_mcus_y: u32,
    quant_tables: &[[i32; 64]],
) -> CodecResult<Option<DecodedImage>> {
    if info.progressive
        || info.num_components != 1
        || info.restart_interval != 0
        || info.components.len() != 1
        || info.scan_components.len() != 1
        || info.components[0].h_samp != 1
        || info.components[0].v_samp != 1
        || info.max_h_samp != 1
        || info.max_v_samp != 1
        || info.scan_components[0].comp_index != 0
        || entropy_segments.segments.len() != 1
        || quant_tables.len() != 1
    {
        return Ok(None);
    }

    let scan = info.scan_components[0];
    let dc_table = info
        .dc_huff_tables
        .get(usize::from(scan.dc_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG DC Huffman table")?;
    let ac_table = info
        .ac_huff_tables
        .get(usize::from(scan.ac_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG AC Huffman table")?;
    let (entropy_start, entropy_end) = entropy_segments.segments[0];
    let mut reader = FastBitReader::new(data, entropy_start, entropy_end);
    let mut dc_predictor = 0i32;
    let mut block_natural = [0i32; 64];
    let mut workspace = [0i32; 64];
    let mut edge_block = [128u8; 64];
    let width = usize::from(info.width);
    let height = usize::from(info.height);
    let mut pixels = vec![128u8; width.saturating_mul(height)];

    for mcu_y in 0..bounded_usize(num_mcus_y) {
        for mcu_x in 0..bounded_usize(num_mcus_x) {
            let block_x = mcu_x.saturating_mul(8);
            let block_y = mcu_y.saturating_mul(8);
            if block_x.saturating_add(8) <= width && block_y.saturating_add(8) <= height {
                decode_and_store_block_fast(
                    &mut reader,
                    dc_table,
                    ac_table,
                    &mut dc_predictor,
                    &quant_tables[0],
                    &mut pixels,
                    width,
                    block_x,
                    block_y,
                    &mut block_natural,
                    &mut workspace,
                )?;
            } else {
                decode_and_store_block_fast(
                    &mut reader,
                    dc_table,
                    ac_table,
                    &mut dc_predictor,
                    &quant_tables[0],
                    &mut edge_block,
                    8,
                    0,
                    0,
                    &mut block_natural,
                    &mut workspace,
                )?;
                let valid_width = width.saturating_sub(block_x).min(8);
                let valid_height = height.saturating_sub(block_y).min(8);
                for row in 0usize..valid_height {
                    let source = row.saturating_mul(8);
                    let destination = block_y
                        .saturating_add(row)
                        .saturating_mul(width)
                        .saturating_add(block_x);
                    pixels[destination..destination.saturating_add(valid_width)]
                        .copy_from_slice(&edge_block[source..source.saturating_add(valid_width)]);
                }
            }
            if reader.insufficient_data() {
                return Ok(None);
            }
            check_baseline_mcu_checkpoint(
                token,
                mcu_y
                    .saturating_mul(bounded_usize(num_mcus_x))
                    .saturating_add(mcu_x)
                    .saturating_add(1),
            )?;
        }
    }

    Ok(Some(DecodedImage::new(
        u32::from(info.width),
        u32::from(info.height),
        pixels,
        ColorType::L8,
    )))
}

/// Decode a baseline 4:4:4 CMYK scan through one MCU-local component packet.
///
/// The general path materializes four padded image-sized component planes and
/// then walks them again to invert and interleave the final pixels. Common
/// CMYK JPEGs carry four 1x1 components in one scan, so their blocks can be
/// transformed into four reusable 8x8 buffers and published directly into the
/// caller-visible allocation.
#[cfg(target_arch = "aarch64")]
#[allow(
    clippy::too_many_arguments,
    reason = "the direct path keeps validated JPEG state and its four reusable component blocks explicit"
)]
fn reconstruct_baseline_cmyk_direct_safe(
    info: &JpegInfo,
    entropy_segments: &EntropySegments,
    data: &[u8],
    token: Option<&crate::CancellationToken>,
    num_mcus_x: u32,
    num_mcus_y: u32,
    quant_tables: &[[i32; 64]],
) -> CodecResult<Option<DecodedImage>> {
    if info.progressive
        || info.num_components != 4
        || info.restart_interval != 0
        || info.components.len() != 4
        || info.scan_components.len() != 4
        || info
            .components
            .iter()
            .any(|component| component.h_samp != 1 || component.v_samp != 1)
        || info.max_h_samp != 1
        || info.max_v_samp != 1
        || entropy_segments.segments.len() != 1
        || quant_tables.len() != 4
    {
        return Ok(None);
    }

    let mut seen_components = [false; 4];
    let mut component_to_scan = [0usize; 4];
    for (scan_index, scan) in info.scan_components.iter().enumerate() {
        if scan.comp_index >= 4 || seen_components[scan.comp_index] {
            return Ok(None);
        }
        seen_components[scan.comp_index] = true;
        component_to_scan[scan.comp_index] = scan_index;
    }

    let dc_table = |scan_index: usize| {
        let scan = info.scan_components[scan_index];
        info.dc_huff_tables
            .get(usize::from(scan.dc_tbl))
            .and_then(Option::as_ref)
            .malformed("missing JPEG DC Huffman table")
    };
    let ac_table = |scan_index: usize| {
        let scan = info.scan_components[scan_index];
        info.ac_huff_tables
            .get(usize::from(scan.ac_tbl))
            .and_then(Option::as_ref)
            .malformed("missing JPEG AC Huffman table")
    };
    let dc_tables = [dc_table(0)?, dc_table(1)?, dc_table(2)?, dc_table(3)?];
    let ac_tables = [ac_table(0)?, ac_table(1)?, ac_table(2)?, ac_table(3)?];
    let scan_components = [
        info.scan_components[0].comp_index,
        info.scan_components[1].comp_index,
        info.scan_components[2].comp_index,
        info.scan_components[3].comp_index,
    ];
    let quant_tables_by_scan = [
        &quant_tables[scan_components[0]],
        &quant_tables[scan_components[1]],
        &quant_tables[scan_components[2]],
        &quant_tables[scan_components[3]],
    ];
    let scan_tables = [
        (dc_tables[0], ac_tables[0], quant_tables_by_scan[0]),
        (dc_tables[1], ac_tables[1], quant_tables_by_scan[1]),
        (dc_tables[2], ac_tables[2], quant_tables_by_scan[2]),
        (dc_tables[3], ac_tables[3], quant_tables_by_scan[3]),
    ];

    let (entropy_start, entropy_end) = entropy_segments.segments[0];
    let mut reader = FastBitReader::new(data, entropy_start, entropy_end);
    let mut dc_predictors_by_scan = [0i32; 4];
    let mut blocks_by_scan = [[128u8; 64]; 4];
    let mut block_natural = [0i32; 64];
    let mut workspace = [0i32; 64];
    let width = usize::from(info.width);
    let height = usize::from(info.height);
    let mut pixels = vec![0u8; width.saturating_mul(height).saturating_mul(4)];

    for mcu_y in 0..bounded_usize(num_mcus_y) {
        for mcu_x in 0..bounded_usize(num_mcus_x) {
            for (((dc_table, ac_table, quant_table), dc_predictor), block) in scan_tables
                .iter()
                .copied()
                .zip(&mut dc_predictors_by_scan)
                .zip(&mut blocks_by_scan)
            {
                decode_and_store_block_fast(
                    &mut reader,
                    dc_table,
                    ac_table,
                    dc_predictor,
                    quant_table,
                    block,
                    8,
                    0,
                    0,
                    &mut block_natural,
                    &mut workspace,
                )?;
            }
            if reader.insufficient_data() {
                return Ok(None);
            }

            let block_x = mcu_x.saturating_mul(8);
            let block_y = mcu_y.saturating_mul(8);
            let valid_width = width.saturating_sub(block_x).min(8);
            let valid_height = height.saturating_sub(block_y).min(8);
            let cyan_block = &blocks_by_scan[component_to_scan[0]];
            let magenta_block = &blocks_by_scan[component_to_scan[1]];
            let yellow_block = &blocks_by_scan[component_to_scan[2]];
            let black_block = &blocks_by_scan[component_to_scan[3]];
            for row in 0usize..valid_height {
                let source_start = row.saturating_mul(8);
                let destination_start = block_y
                    .saturating_add(row)
                    .saturating_mul(width)
                    .saturating_add(block_x)
                    .saturating_mul(4);
                if valid_width == 8 {
                    let cyan: &[u8; 8] = cyan_block[source_start..source_start.saturating_add(8)]
                        .try_into()
                        .unwrap_or_else(|_| unreachable!("CMYK cyan row is not eight bytes"));
                    let magenta: &[u8; 8] = magenta_block
                        [source_start..source_start.saturating_add(8)]
                        .try_into()
                        .unwrap_or_else(|_| unreachable!("CMYK magenta row is not eight bytes"));
                    let yellow: &[u8; 8] = yellow_block
                        [source_start..source_start.saturating_add(8)]
                        .try_into()
                        .unwrap_or_else(|_| unreachable!("CMYK yellow row is not eight bytes"));
                    let black: &[u8; 8] = black_block[source_start..source_start.saturating_add(8)]
                        .try_into()
                        .unwrap_or_else(|_| unreachable!("CMYK black row is not eight bytes"));
                    let (first, second) =
                        invert_interleave_cmyk_eight(cyan, magenta, yellow, black);
                    pixels[destination_start..destination_start.saturating_add(16)]
                        .copy_from_slice(&first);
                    pixels[destination_start.saturating_add(16)
                        ..destination_start.saturating_add(32)]
                        .copy_from_slice(&second);
                    continue;
                }
                for column in 0usize..valid_width {
                    let source = source_start.saturating_add(column);
                    let destination = destination_start.saturating_add(column.saturating_mul(4));
                    pixels[destination] = 255u8.saturating_sub(cyan_block[source]);
                    pixels[destination.saturating_add(1)] =
                        255u8.saturating_sub(magenta_block[source]);
                    pixels[destination.saturating_add(2)] =
                        255u8.saturating_sub(yellow_block[source]);
                    pixels[destination.saturating_add(3)] =
                        255u8.saturating_sub(black_block[source]);
                }
            }
            check_baseline_mcu_checkpoint(
                token,
                mcu_y
                    .saturating_mul(bounded_usize(num_mcus_x))
                    .saturating_add(mcu_x)
                    .saturating_add(1),
            )?;
        }
    }

    Ok(Some(DecodedImage::new(
        u32::from(info.width),
        u32::from(info.height),
        pixels,
        ColorType::Cmyk8,
    )))
}

#[cfg(target_arch = "aarch64")]
#[allow(
    clippy::too_many_arguments,
    reason = "the direct path keeps validated JPEG state and its bounded row buffers explicit"
)]
fn reconstruct_baseline_420_direct_safe(
    info: &JpegInfo,
    entropy_segments: &EntropySegments,
    data: &[u8],
    token: Option<&crate::CancellationToken>,
    num_mcus_x: u32,
    num_mcus_y: u32,
    quant_tables: &[[i32; 64]],
    converter: &YccColorConverter,
) -> CodecResult<Option<DecodedImage>> {
    let mcu_columns = bounded_usize(num_mcus_x);
    let mcu_rows = bounded_usize(num_mcus_y);
    let restart_interval = usize::from(info.restart_interval);
    let restart_is_row_aligned =
        restart_interval == 0 || (mcu_columns != 0 && restart_interval.is_multiple_of(mcu_columns));
    let expected_segments = if restart_interval == 0 {
        1
    } else {
        mcu_columns
            .saturating_mul(mcu_rows)
            .div_ceil(restart_interval)
    };
    if info.progressive
        || info.num_components != 3
        || info.components.len() != 3
        || info.scan_components.len() != 3
        || info.components[0].h_samp != 2
        || info.components[0].v_samp != 2
        || info.components[1].h_samp != 1
        || info.components[1].v_samp != 1
        || info.components[2].h_samp != 1
        || info.components[2].v_samp != 1
        || info.max_h_samp != 2
        || info.max_v_samp != 2
        || info.scan_components[0].comp_index != 0
        || info.scan_components[1].comp_index != 1
        || info.scan_components[2].comp_index != 2
        || !restart_is_row_aligned
        || entropy_segments.segments.len() != expected_segments
        || quant_tables.len() != 3
    {
        return Ok(None);
    }

    let y_scan = info.scan_components[0];
    let cb_scan = info.scan_components[1];
    let cr_scan = info.scan_components[2];
    let y_dc = info
        .dc_huff_tables
        .get(usize::from(y_scan.dc_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG DC Huffman table")?;
    let y_ac = info
        .ac_huff_tables
        .get(usize::from(y_scan.ac_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG AC Huffman table")?;
    let cb_dc = info
        .dc_huff_tables
        .get(usize::from(cb_scan.dc_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG DC Huffman table")?;
    let cb_ac = info
        .ac_huff_tables
        .get(usize::from(cb_scan.ac_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG AC Huffman table")?;
    let cr_dc = info
        .dc_huff_tables
        .get(usize::from(cr_scan.dc_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG DC Huffman table")?;
    let cr_ac = info
        .ac_huff_tables
        .get(usize::from(cr_scan.ac_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG AC Huffman table")?;

    let (entropy_start, entropy_end) = entropy_segments.segments[0];
    let mut reader = FastBitReader::new(data, entropy_start, entropy_end);
    let mut dc_predictors = [0i32; 3];
    let mut block_natural = [0i32; 64];
    let mut workspace = [0i32; 64];
    let rows_per_segment = if restart_interval == 0 {
        mcu_rows
    } else {
        restart_interval.div_euclid(mcu_columns)
    };
    let mut segment_index = 0usize;
    let padded_y_width = mcu_columns.saturating_mul(16);
    let padded_chroma_width = mcu_columns.saturating_mul(8);
    let valid_chroma_width = usize::from(info.width).div_ceil(2);
    // The safe color kernel reads a complete 16-byte vector while consuming
    // eight lanes. Eight initialized guard bytes let the final batch use that
    // same efficient load without exposing uninitialized memory.
    let y_stride = padded_y_width.saturating_add(8);
    let y_row_length = y_stride.saturating_mul(16);
    // One left guard plus fifteen right guards make every eight-sample
    // source window a valid safe 16-byte load, including the final MCU. This
    // lets the safe SIMD abstraction lower to one vector load and widening
    // operation instead of reconstructing a vector from a scalar u64.
    let chroma_stride = padded_chroma_width.saturating_add(16);
    let chroma_row_length = chroma_stride.saturating_mul(8);

    let mut y_current = vec![128u8; y_row_length];
    let mut y_next = vec![128u8; y_row_length];
    let mut previous_cb = vec![128u8; chroma_row_length];
    let mut current_cb = vec![128u8; chroma_row_length];
    let mut next_cb = vec![128u8; chroma_row_length];
    let mut previous_cr = vec![128u8; chroma_row_length];
    let mut current_cr = vec![128u8; chroma_row_length];
    let mut next_cr = vec![128u8; chroma_row_length];

    if !decode_baseline_420_row_fast(
        &mut reader,
        y_dc,
        y_ac,
        cb_dc,
        cb_ac,
        cr_dc,
        cr_ac,
        &mut dc_predictors,
        quant_tables,
        &mut y_current,
        &mut current_cb,
        &mut current_cr,
        y_stride,
        chroma_stride,
        valid_chroma_width,
        mcu_columns,
        &mut block_natural,
        &mut workspace,
    )? {
        return Ok(None);
    }

    let previous_edge = 7usize.saturating_mul(chroma_stride);
    previous_cb[previous_edge..previous_edge.saturating_add(chroma_stride)]
        .copy_from_slice(&current_cb[..chroma_stride]);
    previous_cr[previous_edge..previous_edge.saturating_add(chroma_stride)]
        .copy_from_slice(&current_cr[..chroma_stride]);

    let width = usize::from(info.width);
    let height = usize::from(info.height);
    let output_stride = padded_chroma_width.saturating_mul(2).saturating_add(8);
    let mut cb_pair = vec![0u8; output_stride.saturating_mul(2)];
    let mut cr_pair = vec![0u8; output_stride.saturating_mul(2)];
    let mut pixels = vec![0u8; width.saturating_mul(height).saturating_mul(3)];

    for mcu_y in 0..mcu_rows {
        let has_next = mcu_y.saturating_add(1) < mcu_rows;
        let next_row = mcu_y.saturating_add(1);
        if has_next && restart_interval != 0 && next_row.is_multiple_of(rows_per_segment) {
            segment_index = segment_index.saturating_add(1);
            let Some(&(next_start, next_end)) = entropy_segments.segments.get(segment_index) else {
                return Ok(None);
            };
            reader = FastBitReader::new(data, next_start, next_end);
            dc_predictors.fill(0);
        }
        if has_next
            && !decode_baseline_420_row_fast(
                &mut reader,
                y_dc,
                y_ac,
                cb_dc,
                cb_ac,
                cr_dc,
                cr_ac,
                &mut dc_predictors,
                quant_tables,
                &mut y_next,
                &mut next_cb,
                &mut next_cr,
                y_stride,
                chroma_stride,
                valid_chroma_width,
                mcu_columns,
                &mut block_natural,
                &mut workspace,
            )?
        {
            return Ok(None);
        }

        let image_y_start = mcu_y.saturating_mul(16);
        let valid_rows = height.saturating_sub(image_y_start).min(16);
        let source_height = valid_rows.div_ceil(2);
        if !has_next {
            let current_edge = source_height
                .saturating_sub(1)
                .saturating_mul(chroma_stride);
            next_cb[..chroma_stride].copy_from_slice(
                &current_cb[current_edge..current_edge.saturating_add(chroma_stride)],
            );
            next_cr[..chroma_stride].copy_from_slice(
                &current_cr[current_edge..current_edge.saturating_add(chroma_stride)],
            );
        }

        for source_row in 0..source_height {
            fancy_upsample_h2v2_row_pair_safe(
                &previous_cb,
                &current_cb,
                &next_cb,
                padded_chroma_width,
                chroma_stride,
                source_row,
                source_height,
                output_stride,
                &mut cb_pair,
            );
            fancy_upsample_h2v2_row_pair_safe(
                &previous_cr,
                &current_cr,
                &next_cr,
                padded_chroma_width,
                chroma_stride,
                source_row,
                source_height,
                output_stride,
                &mut cr_pair,
            );

            let local_y = source_row.saturating_mul(2);
            let y_start = local_y.saturating_mul(y_stride);
            let output_y = image_y_start.saturating_add(local_y);
            let output_start = output_y.saturating_mul(width.saturating_mul(3));
            converter.ycc_to_rgb_batch(
                &y_current[y_start..y_start.saturating_add(y_stride)],
                &cb_pair[..output_stride],
                &cr_pair[..output_stride],
                &mut pixels[output_start..output_start.saturating_add(width.saturating_mul(3))],
            );
            if local_y.saturating_add(1) < valid_rows {
                let y_start = y_start.saturating_add(y_stride);
                let output_start = output_start.saturating_add(width.saturating_mul(3));
                converter.ycc_to_rgb_batch(
                    &y_current[y_start..y_start.saturating_add(y_stride)],
                    &cb_pair[output_stride..output_stride.saturating_mul(2)],
                    &cr_pair[output_stride..output_stride.saturating_mul(2)],
                    &mut pixels[output_start..output_start.saturating_add(width.saturating_mul(3))],
                );
            }
        }

        if has_next {
            core::mem::swap(&mut previous_cb, &mut current_cb);
            core::mem::swap(&mut current_cb, &mut next_cb);
            core::mem::swap(&mut previous_cr, &mut current_cr);
            core::mem::swap(&mut current_cr, &mut next_cr);
            core::mem::swap(&mut y_current, &mut y_next);
        }
        check_baseline_mcu_checkpoint(token, mcu_y.saturating_add(1).saturating_mul(mcu_columns))?;
    }

    Ok(Some(DecodedImage::new(
        u32::from(info.width),
        u32::from(info.height),
        pixels,
        ColorType::Rgb8,
    )))
}

/// Decode a baseline 4:2:2 scan through one reusable MCU-row window and write
/// converted RGB rows directly into the final allocation.
#[cfg(target_arch = "aarch64")]
#[allow(
    clippy::too_many_arguments,
    reason = "the direct path keeps validated JPEG state and bounded row buffers explicit"
)]
fn reconstruct_baseline_422_direct_safe(
    info: &JpegInfo,
    entropy_segments: &EntropySegments,
    data: &[u8],
    token: Option<&crate::CancellationToken>,
    num_mcus_x: u32,
    num_mcus_y: u32,
    quant_tables: &[[i32; 64]],
    converter: &YccColorConverter,
) -> CodecResult<Option<DecodedImage>> {
    if info.progressive
        || info.num_components != 3
        || info.restart_interval != 0
        || info.components.len() != 3
        || info.scan_components.len() != 3
        || info.components[0].h_samp != 2
        || info.components[0].v_samp != 1
        || info.components[1].h_samp != 1
        || info.components[1].v_samp != 1
        || info.components[2].h_samp != 1
        || info.components[2].v_samp != 1
        || info.max_h_samp != 2
        || info.max_v_samp != 1
        || info.scan_components[0].comp_index != 0
        || info.scan_components[1].comp_index != 1
        || info.scan_components[2].comp_index != 2
        || entropy_segments.segments.len() != 1
        || quant_tables.len() != 3
    {
        return Ok(None);
    }

    let y_scan = info.scan_components[0];
    let cb_scan = info.scan_components[1];
    let cr_scan = info.scan_components[2];
    let y_dc = info
        .dc_huff_tables
        .get(usize::from(y_scan.dc_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG DC Huffman table")?;
    let y_ac = info
        .ac_huff_tables
        .get(usize::from(y_scan.ac_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG AC Huffman table")?;
    let cb_dc = info
        .dc_huff_tables
        .get(usize::from(cb_scan.dc_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG DC Huffman table")?;
    let cb_ac = info
        .ac_huff_tables
        .get(usize::from(cb_scan.ac_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG AC Huffman table")?;
    let cr_dc = info
        .dc_huff_tables
        .get(usize::from(cr_scan.dc_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG DC Huffman table")?;
    let cr_ac = info
        .ac_huff_tables
        .get(usize::from(cr_scan.ac_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG AC Huffman table")?;

    let (entropy_start, entropy_end) = entropy_segments.segments[0];
    let mut reader = FastBitReader::new(data, entropy_start, entropy_end);
    let mut dc_predictors = [0i32; 3];
    let mut block_natural = [0i32; 64];
    let mut workspace = [0i32; 64];
    let mcu_columns = bounded_usize(num_mcus_x);
    let mcu_rows = bounded_usize(num_mcus_y);
    let padded_y_width = mcu_columns.saturating_mul(16);
    let padded_chroma_width = mcu_columns.saturating_mul(8);
    let valid_chroma_width = usize::from(info.width).div_ceil(2);
    let y_stride = padded_y_width.saturating_add(8);
    let chroma_stride = padded_chroma_width.saturating_add(16);
    let mut y_row = vec![128u8; y_stride.saturating_mul(8)];
    let mut cb_row = vec![128u8; chroma_stride.saturating_mul(8)];
    let mut cr_row = vec![128u8; chroma_stride.saturating_mul(8)];
    let mut cb_upsampled = vec![128u8; y_stride];
    let mut cr_upsampled = vec![128u8; y_stride];
    let width = usize::from(info.width);
    let height = usize::from(info.height);
    let mut pixels = vec![0u8; width.saturating_mul(height).saturating_mul(3)];

    for mcu_y in 0..mcu_rows {
        if !decode_baseline_422_row_fast(
            &mut reader,
            y_dc,
            y_ac,
            cb_dc,
            cb_ac,
            cr_dc,
            cr_ac,
            &mut dc_predictors,
            quant_tables,
            &mut y_row,
            &mut cb_row,
            &mut cr_row,
            y_stride,
            chroma_stride,
            valid_chroma_width,
            mcu_columns,
            &mut block_natural,
            &mut workspace,
        )? {
            return Ok(None);
        }

        let image_y_start = mcu_y.saturating_mul(8);
        let valid_rows = height.saturating_sub(image_y_start).min(8);
        for row in 0usize..valid_rows {
            fancy_upsample_h2v1_row_safe(
                &cb_row,
                padded_chroma_width,
                chroma_stride,
                row,
                &mut cb_upsampled,
            );
            fancy_upsample_h2v1_row_safe(
                &cr_row,
                padded_chroma_width,
                chroma_stride,
                row,
                &mut cr_upsampled,
            );
            let y_start = row.saturating_mul(y_stride);
            let output_start = image_y_start
                .saturating_add(row)
                .saturating_mul(width.saturating_mul(3));
            converter.ycc_to_rgb_batch(
                &y_row[y_start..y_start.saturating_add(y_stride)],
                &cb_upsampled,
                &cr_upsampled,
                &mut pixels[output_start..output_start.saturating_add(width.saturating_mul(3))],
            );
        }
        check_baseline_mcu_checkpoint(token, mcu_y.saturating_add(1).saturating_mul(mcu_columns))?;
    }

    Ok(Some(DecodedImage::new(
        u32::from(info.width),
        u32::from(info.height),
        pixels,
        ColorType::Rgb8,
    )))
}

/// Decode a baseline 4:4:4 scan directly into the final RGB allocation.
///
/// The general reconstruction path stores three complete padded component
/// planes, crops both chroma planes, and then reads all three planes again for
/// color conversion. A 4:4:4 MCU contains one block from each component, so
/// keeping those three blocks local removes that representation boundary.
/// All buffers are initialized Rust values; an incomplete entropy stream can
/// therefore fall back to the general decoder without exposing partial output.
#[cfg(target_arch = "aarch64")]
#[allow(
    clippy::too_many_arguments,
    reason = "the direct path keeps validated JPEG state and its bounded block buffers explicit"
)]
fn reconstruct_baseline_444_direct_safe(
    info: &JpegInfo,
    entropy_segments: &EntropySegments,
    data: &[u8],
    token: Option<&crate::CancellationToken>,
    num_mcus_x: u32,
    num_mcus_y: u32,
    quant_tables: &[[i32; 64]],
    converter: &YccColorConverter,
) -> CodecResult<Option<DecodedImage>> {
    if info.progressive
        || info.num_components != 3
        || info.restart_interval != 0
        || info.components.len() != 3
        || info.scan_components.len() != 3
        || info
            .components
            .iter()
            .any(|component| component.h_samp != 1 || component.v_samp != 1)
        || info.max_h_samp != 1
        || info.max_v_samp != 1
        || info.scan_components[0].comp_index != 0
        || info.scan_components[1].comp_index != 1
        || info.scan_components[2].comp_index != 2
        || entropy_segments.segments.len() != 1
        || quant_tables.len() != 3
    {
        return Ok(None);
    }

    let y_scan = info.scan_components[0];
    let cb_scan = info.scan_components[1];
    let cr_scan = info.scan_components[2];
    let y_dc = info
        .dc_huff_tables
        .get(usize::from(y_scan.dc_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG DC Huffman table")?;
    let y_ac = info
        .ac_huff_tables
        .get(usize::from(y_scan.ac_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG AC Huffman table")?;
    let cb_dc = info
        .dc_huff_tables
        .get(usize::from(cb_scan.dc_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG DC Huffman table")?;
    let cb_ac = info
        .ac_huff_tables
        .get(usize::from(cb_scan.ac_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG AC Huffman table")?;
    let cr_dc = info
        .dc_huff_tables
        .get(usize::from(cr_scan.dc_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG DC Huffman table")?;
    let cr_ac = info
        .ac_huff_tables
        .get(usize::from(cr_scan.ac_tbl))
        .and_then(Option::as_ref)
        .malformed("missing JPEG AC Huffman table")?;

    let (entropy_start, entropy_end) = entropy_segments.segments[0];
    let mut reader = FastBitReader::new(data, entropy_start, entropy_end);
    let mut dc_predictors = [0i32; 3];
    let mut block_natural = [0i32; 64];
    let mut workspace = [0i32; 64];
    let mut y_block = [128u8; 64];
    let mut cb_block = [128u8; 64];
    let mut cr_block = [128u8; 64];
    let width = usize::from(info.width);
    let height = usize::from(info.height);
    let mut pixels = vec![0u8; width.saturating_mul(height).saturating_mul(3)];

    for mcu_y in 0..bounded_usize(num_mcus_y) {
        for mcu_x in 0..bounded_usize(num_mcus_x) {
            decode_and_store_block_fast(
                &mut reader,
                y_dc,
                y_ac,
                &mut dc_predictors[0],
                &quant_tables[0],
                &mut y_block,
                8,
                0,
                0,
                &mut block_natural,
                &mut workspace,
            )?;
            decode_and_store_block_fast(
                &mut reader,
                cb_dc,
                cb_ac,
                &mut dc_predictors[1],
                &quant_tables[1],
                &mut cb_block,
                8,
                0,
                0,
                &mut block_natural,
                &mut workspace,
            )?;
            decode_and_store_block_fast(
                &mut reader,
                cr_dc,
                cr_ac,
                &mut dc_predictors[2],
                &quant_tables[2],
                &mut cr_block,
                8,
                0,
                0,
                &mut block_natural,
                &mut workspace,
            )?;
            if reader.insufficient_data() {
                return Ok(None);
            }

            let block_x = mcu_x.saturating_mul(8);
            let block_y = mcu_y.saturating_mul(8);
            let valid_width = width.saturating_sub(block_x).min(8);
            let valid_height = height.saturating_sub(block_y).min(8);
            for row in 0..valid_height {
                let source_start = row.saturating_mul(8);
                let output_start = block_y
                    .saturating_add(row)
                    .saturating_mul(width.saturating_mul(3))
                    .saturating_add(block_x.saturating_mul(3));
                converter.ycc_to_rgb_batch(
                    &y_block[source_start..source_start.saturating_add(valid_width)],
                    &cb_block[source_start..source_start.saturating_add(valid_width)],
                    &cr_block[source_start..source_start.saturating_add(valid_width)],
                    &mut pixels
                        [output_start..output_start.saturating_add(valid_width.saturating_mul(3))],
                );
            }
            check_baseline_mcu_checkpoint(
                token,
                mcu_y
                    .saturating_mul(bounded_usize(num_mcus_x))
                    .saturating_add(mcu_x)
                    .saturating_add(1),
            )?;
        }
    }

    Ok(Some(DecodedImage::new(
        u32::from(info.width),
        u32::from(info.height),
        pixels,
        ColorType::Rgb8,
    )))
}

pub(super) fn reconstruct_image(
    info: &JpegInfo,
    data: &[u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<DecodedImage> {
    crate::codecs::error::check_cancelled(token)?;
    let mcu_width = u32::from(info.max_h_samp).saturating_mul(8);
    let mcu_height = u32::from(info.max_v_samp).saturating_mul(8);
    let num_mcus_x = u32::from(info.width).div_ceil(mcu_width);
    let num_mcus_y = u32::from(info.height).div_ceil(mcu_height);

    let comp_buf_width: Vec<usize> = info
        .components
        .iter()
        .map(|c| {
            bounded_usize(num_mcus_x)
                .saturating_mul(usize::from(c.h_samp))
                .saturating_mul(8)
        })
        .collect();
    let comp_buf_height: Vec<usize> = info
        .components
        .iter()
        .map(|c| {
            bounded_usize(num_mcus_y)
                .saturating_mul(usize::from(c.v_samp))
                .saturating_mul(8)
        })
        .collect();

    let mut quant_natural_by_component = Vec::with_capacity(info.components.len());
    for component in &info.components {
        let quant_table = info
            .quant_tables
            .get(usize::from(component.quant_tbl))
            .and_then(Option::as_ref)
            .malformed("missing JPEG quantization table")?;
        let mut quant_natural = [0i32; 64];
        for zigzag in 0usize..64 {
            quant_natural[idct::JPEG_NATURAL_ORDER[zigzag]] = i32::from(quant_table[zigzag]);
        }
        quant_natural_by_component.push(quant_natural);
    }

    let converter = YccColorConverter::shared();

    // Extract entropy segments (between RST markers)
    let entropy_segments = known_single_entropy_segment(info, data)
        .unwrap_or_else(|| extract_entropy_segments(data, info.entropy_start, info.eoi_pos));
    if entropy_segments.segments.is_empty() {
        return Err(CodecError::Malformed(
            "JPEG contains no entropy segment".to_owned(),
        ));
    }

    #[cfg(target_arch = "aarch64")]
    if let Some(image) = reconstruct_baseline_grayscale_direct_safe(
        info,
        &entropy_segments,
        data,
        token,
        num_mcus_x,
        num_mcus_y,
        &quant_natural_by_component,
    )? {
        return Ok(image);
    }

    #[cfg(target_arch = "aarch64")]
    if let Some(image) = reconstruct_baseline_cmyk_direct_safe(
        info,
        &entropy_segments,
        data,
        token,
        num_mcus_x,
        num_mcus_y,
        &quant_natural_by_component,
    )? {
        return Ok(image);
    }

    #[cfg(target_arch = "aarch64")]
    if let Some(image) = reconstruct_baseline_420_direct_safe(
        info,
        &entropy_segments,
        data,
        token,
        num_mcus_x,
        num_mcus_y,
        &quant_natural_by_component,
        converter,
    )? {
        return Ok(image);
    }

    #[cfg(target_arch = "aarch64")]
    if let Some(image) = reconstruct_baseline_422_direct_safe(
        info,
        &entropy_segments,
        data,
        token,
        num_mcus_x,
        num_mcus_y,
        &quant_natural_by_component,
        converter,
    )? {
        return Ok(image);
    }

    #[cfg(target_arch = "aarch64")]
    if let Some(image) = reconstruct_baseline_444_direct_safe(
        info,
        &entropy_segments,
        data,
        token,
        num_mcus_x,
        num_mcus_y,
        &quant_natural_by_component,
        converter,
    )? {
        return Ok(image);
    }

    let mut comp_buffers: Vec<Vec<u8>> = info
        .components
        .iter()
        .enumerate()
        .map(|(i, _)| vec![128u8; comp_buf_width[i].saturating_mul(comp_buf_height[i])])
        .collect();
    let mut dc_predictors: Vec<i32> = vec![0; usize::from(info.num_components)];
    let mut block_natural = [0i32; 64];
    let mut workspace = [0i32; 64];

    let total_mcus = bounded_usize(num_mcus_x.saturating_mul(num_mcus_y));
    let mut segment_iter = entropy_segments.segments.iter().peekable();
    let mut seg_idx = 0usize;
    let mcus_per_seg = if info.restart_interval > 0 {
        usize::from(info.restart_interval)
    } else {
        total_mcus
    };

    #[cfg(target_arch = "aarch64")]
    let used_fast_path = reconstruct_baseline_420_fast(
        info,
        &entropy_segments,
        data,
        num_mcus_x,
        num_mcus_y,
        &comp_buf_width,
        &quant_natural_by_component,
        &mut comp_buffers,
        &mut dc_predictors,
        &mut block_natural,
        &mut workspace,
    )?;
    #[cfg(not(target_arch = "aarch64"))]
    let used_fast_path = false;

    if !used_fast_path {
        while let Some(&(seg_start, seg_end)) = segment_iter.next() {
            crate::codecs::error::check_cancelled(token)?;
            let mut br = BitReader::new(data, seg_start, seg_end);
            let mcu_offset = seg_idx.saturating_mul(mcus_per_seg);

            for mcu_idx in 0..mcus_per_seg {
                let absolute_mcu = mcu_offset.saturating_add(mcu_idx);
                if absolute_mcu >= total_mcus {
                    break;
                }
                let mcu_width = bounded_usize(num_mcus_x);
                let mcu_y = absolute_mcu.div_euclid(mcu_width);
                let mcu_x = absolute_mcu.rem_euclid(mcu_width);

                for scan_comp in &info.scan_components {
                    let comp = &info.components[scan_comp.comp_index];
                    let dc_table = info
                        .dc_huff_tables
                        .get(usize::from(scan_comp.dc_tbl))
                        .and_then(Option::as_ref)
                        .malformed("missing JPEG DC Huffman table")?;
                    let ac_table = info
                        .ac_huff_tables
                        .get(usize::from(scan_comp.ac_tbl))
                        .and_then(Option::as_ref)
                        .malformed("missing JPEG AC Huffman table")?;
                    let quant_natural = &quant_natural_by_component[scan_comp.comp_index];

                    for by in 0..usize::from(comp.v_samp) {
                        for bx in 0..usize::from(comp.h_samp) {
                            let kind = match decode_block(
                                &mut br,
                                dc_table,
                                ac_table,
                                &mut dc_predictors[scan_comp.comp_index],
                                &mut block_natural,
                            ) {
                                Ok(kind) => kind,
                                Err(error) => return Err(error.context("baseline block")),
                            };

                            let buf_w = comp_buf_width[scan_comp.comp_index];
                            let block_x = mcu_x
                                .saturating_mul(usize::from(comp.h_samp))
                                .saturating_add(bx)
                                .saturating_mul(8);
                            let block_y = mcu_y
                                .saturating_mul(usize::from(comp.v_samp))
                                .saturating_add(by)
                                .saturating_mul(8);

                            if kind == BlockKind::DcOnly {
                                let dequantized = block_natural[0].saturating_mul(quant_natural[0]);
                                let value = idct::dc_only_output(dequantized);
                                for row in 0usize..8 {
                                    let start = block_y
                                        .saturating_add(row)
                                        .saturating_mul(buf_w)
                                        .saturating_add(block_x);
                                    comp_buffers[scan_comp.comp_index]
                                        [start..start.saturating_add(8)]
                                        .fill(value);
                                }
                                continue;
                            }

                            for (coefficient, &quantizer) in
                                block_natural.iter_mut().zip(quant_natural)
                            {
                                *coefficient = coefficient.saturating_mul(quantizer);
                            }
                            jpeg_idct_islow(&mut block_natural, &mut workspace);

                            for row in 0usize..8 {
                                let source_start = row.saturating_mul(8);
                                let destination_start = block_y
                                    .saturating_add(row)
                                    .saturating_mul(buf_w)
                                    .saturating_add(block_x);
                                let destination = &mut comp_buffers[scan_comp.comp_index]
                                    [destination_start..destination_start.saturating_add(8)];
                                for (output, &value) in destination.iter_mut().zip(
                                    &block_natural[source_start..source_start.saturating_add(8)],
                                ) {
                                    *output = value.clamp(0, 255).to_le_bytes()[0];
                                }
                            }
                        }
                    }
                }

                // Handle RST at segment boundaries (except the last segment)
                if mcu_idx.saturating_add(1) >= mcus_per_seg && segment_iter.peek().is_some() {
                    for pred in dc_predictors.iter_mut() {
                        *pred = 0;
                    }
                    seg_idx = seg_idx.saturating_add(1);
                }

                // ✅ FIX: Match libjpeg-turbo's `insufficient_data` handling.
                //    C reference: jdhuff.c `decode_mcu()` completes the current
                //    MCU from synthetic zero bits, then leaves later MCUs
                //    initialized to gray once the current bit request cannot be
                //    satisfied from the remaining entropy buffer. This check must
                //    run after restart-boundary state updates.
                if br.insufficient_data() {
                    break;
                }

                // A no-restart baseline scan can contain many MCUs inside one
                // entropy segment. Keep the ordinary path free of a per-MCU token
                // branch while giving callers a bounded checkpoint after each
                // completed 1,024-MCU batch.
                if let Some(token) = token
                    && absolute_mcu.saturating_add(1).is_multiple_of(1_024)
                {
                    crate::codecs::error::check_cancelled(Some(token))?;
                }
            }
        }
    }

    // ── Assemble output image ──
    let w = usize::from(info.width);
    let h = usize::from(info.height);

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
        let h_ratio = info.max_h_samp.div_euclid(info.components[1].h_samp);
        let v_ratio = info.max_v_samp.div_euclid(info.components[1].v_samp);
        let h_ratio_us = usize::from(h_ratio);
        let v_ratio_us = usize::from(v_ratio);

        // Image-derived chroma dimensions (not MCU-padded)
        let chroma_src_w = w.div_ceil(h_ratio_us);
        let chroma_src_h = h.div_ceil(v_ratio_us);

        // Crop then upsample
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
        let cb_upsampled = fancy_upsample(
            &cb_cropped,
            chroma_src_w,
            chroma_src_h,
            h_ratio_us,
            v_ratio_us,
            w,
            h,
        );
        let cr_upsampled = fancy_upsample(
            &cr_cropped,
            chroma_src_w,
            chroma_src_h,
            h_ratio_us,
            v_ratio_us,
            w,
            h,
        );

        let chroma_stride = chroma_src_w.saturating_mul(h_ratio_us);
        let mut pixels = vec![0u8; w.saturating_mul(h).saturating_mul(3)];
        for y in 0..h {
            let y_start = y.saturating_mul(y_w);
            let chroma_start = y.saturating_mul(chroma_stride);
            let output_start = y.saturating_mul(w).saturating_mul(3);
            converter.ycc_to_rgb_batch(
                &y_buf[y_start..y_start.saturating_add(w)],
                &cb_upsampled[chroma_start..chroma_start.saturating_add(w)],
                &cr_upsampled[chroma_start..chroma_start.saturating_add(w)],
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
        // Pillow exposes four-component JPEGs through its inverted CMYK byte
        // convention even when the Adobe APP14 marker is absent. The
        // no-APP14 CMYK fixture keeps this tied to the oracle instead of the
        // marker alone.
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
                    pixels.push(255u8.saturating_sub(sample));
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

// ── Progressive JPEG Reconstruction ───────────────────────────────────────

pub(super) fn extract_entropy_segments(
    data: &[u8],
    start: usize,
    end_hint: usize,
) -> EntropySegments {
    let mut segments = Vec::new();
    let mut seg_start = start;
    let mut pos = start;
    let mut eoi_pos = 0;

    // ✅ FIX: Preserve an empty scan before EOI as a real entropy segment.
    //    C reference: libjpeg-turbo jdhuff.c `jpeg_fill_bit_buffer` consumes
    //    synthetic zero bits after entropy data ends, so an SOS followed
    //    immediately by EOI still decodes deterministic coefficients.
    //    Old Rust dropped this segment and rejected the image before bit fill.
    if start == end_hint && data.get(start..start.saturating_add(2)) == Some(&[0xFF, 0xD9]) {
        return EntropySegments {
            segments: vec![(start, start)],
            eoi_pos: start,
        };
    }

    while pos < end_hint {
        if data[pos] == 0xFF {
            let marker_start = pos;
            pos = pos.saturating_add(1);
            while pos < end_hint && data[pos] == 0xFF {
                pos = pos.saturating_add(1);
            }
            if pos >= end_hint {
                break;
            }
            match data[pos] {
                0x00 => {
                    pos = pos.saturating_add(1);
                }
                0xD0..=0xD7 => {
                    segments.push((seg_start, marker_start));
                    pos = pos.saturating_add(1);
                    seg_start = pos;
                }
                0xD9 => {
                    segments.push((seg_start, marker_start));
                    eoi_pos = marker_start;
                    break;
                }
                _ => {
                    return EntropySegments {
                        segments: Vec::new(),
                        eoi_pos: 0,
                    };
                }
            }
        } else {
            pos = pos.saturating_add(1);
        }
    }

    if seg_start < end_hint && eoi_pos == 0 {
        segments.push((seg_start, end_hint));
    }

    EntropySegments { segments, eoi_pos }
}

/// Entropy segment information (between RST/EOI markers).
pub(super) struct EntropySegments {
    pub(super) segments: Vec<(usize, usize)>,
    #[allow(dead_code)]
    eoi_pos: usize,
}

#[inline(always)]
fn known_single_entropy_segment(info: &JpegInfo, data: &[u8]) -> Option<EntropySegments> {
    let scan = info.scans.first()?;
    if info.progressive
        || info.scans.len() != 1
        || info.entropy_has_restart_markers
        || scan.entropy_start != info.entropy_start
        || scan.entropy_end != info.eoi_pos
        || data.get(info.eoi_pos..info.eoi_pos.saturating_add(2)) != Some(&[0xFF, 0xD9])
    {
        return None;
    }
    Some(EntropySegments {
        segments: vec![(info.entropy_start, info.eoi_pos)],
        eoi_pos: info.eoi_pos,
    })
}

// ── Public API ────────────────────────────────────────────────────────────

/// Decode JPEG bytes into a DecodedImage (pixel-perfect with libjpeg).
///
/// Supports baseline JPEG (SOF0) and progressive JPEG (SOF2) with:
/// - 8-bit precision
/// - 4:2:0, 4:2:2, 4:4:4 and 4:1:1 chroma subsampling
/// - Grayscale (1 component) and YCbCr (3 components)
/// - Restart markers (DRI)
/// - Progressive: DC first, DC refine, AC first, AC refine scans
pub fn decode(
    data: &[u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(DecodedImage, usize)> {
    crate::codecs::error::check_cancelled(token)?;
    let info = parse_jpeg(data)?;

    debug_assert!(!info.scan_components.is_empty());

    let consumed = info.eoi_pos.saturating_add(2);
    crate::codecs::error::check_cancelled(token)?;
    let mut image = if info.progressive {
        progressive_reconstruct(&info, data, token)
    } else {
        reconstruct_image(&info, data, token)
    }?;
    image = image.with_metadata(info.metadata);
    Ok((image, consumed))
}

/// Measure the encoded metadata extent: the consumed stream (through EOI)
/// minus the entropy-coded scan payload bytes.
pub(crate) fn metadata_bytes(data: &[u8]) -> CodecResult<u64> {
    let info = parse_jpeg(data)?;
    let mut pixel = 0u64;
    for scan in &info.scans {
        // The parser guarantees entropy_end >= entropy_start.
        #[allow(clippy::arithmetic_side_effects)]
        let span = scan.entropy_end - scan.entropy_start;
        pixel = pixel.saturating_add(span as u64);
    }
    let consumed = info.eoi_pos.saturating_add(2) as u64;
    // `pixel` is the sum of entropy spans inside the consumed stream.
    #[allow(clippy::arithmetic_side_effects)]
    let metadata = consumed - pixel;
    Ok(metadata)
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    use super::huffman::HuffTable;

    let _ = metadata_bytes(b"");
    let _ = metadata_bytes(b"\xff");

    let entropy = [0x00; 16];
    let dc_cat_64 = HuffTable::build(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], &[64]);
    let ac_eob = HuffTable::build(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], &[0]);
    let mut br = BitReader::new(&entropy, 0, entropy.len());
    let mut block = [0i32; 64];
    let mut last_dc = 0;
    assert!(decode_block(&mut br, &dc_cat_64, &ac_eob, &mut last_dc, &mut block,).is_err());

    let dc_zero = HuffTable::build(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], &[0]);
    let ac_run_overflow =
        HuffTable::build(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], &[0xF1]);
    let mut br = BitReader::new(&entropy, 0, entropy.len());
    assert!(
        decode_block(
            &mut br,
            &dc_zero,
            &ac_run_overflow,
            &mut last_dc,
            &mut block,
        )
        .is_ok()
    );

    let ac_literal = HuffTable::build(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], &[0x01]);
    let mut br = BitReader::new(&[0; 2], 0, 2);
    assert!(decode_block(&mut br, &dc_zero, &ac_literal, &mut last_dc, &mut block,).is_ok());

    let ac_invalid_zero =
        HuffTable::build(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], &[0x10]);
    let mut br = BitReader::new(&[0; 2], 0, 2);
    assert!(
        decode_block(
            &mut br,
            &dc_zero,
            &ac_invalid_zero,
            &mut last_dc,
            &mut block,
        )
        .is_err()
    );

    let ac_missing = HuffTable::build(&[0; 16], &[]);
    let mut br = BitReader::new(&[0; 2], 0, 2);
    assert!(decode_block(&mut br, &dc_zero, &ac_missing, &mut last_dc, &mut block,).is_err());

    let segments = extract_entropy_segments(&[0, 0xFF, 0xFF, 0xD9], 0, 4);
    assert_eq!(segments.eoi_pos, 1);

    let info = JpegInfo {
        width: 8,
        height: 8,
        num_components: 1,
        components: vec![super::parser::FrameComponent {
            id: 1,
            h_samp: 1,
            v_samp: 1,
            quant_tbl: 0,
        }],
        quant_tables: vec![Some([1; 64])],
        dc_huff_tables: vec![Some(dc_zero.clone().into())],
        ac_huff_tables: vec![Some(ac_eob.clone().into())],
        scan_components: vec![super::parser::ScanComponent {
            comp_index: 0,
            dc_tbl: 0,
            ac_tbl: 0,
        }],
        restart_interval: 1,
        entropy_has_restart_markers: false,
        entropy_start: 0,
        eoi_pos: 5,
        max_h_samp: 1,
        max_v_samp: 1,
        progressive: false,
        scans: Vec::new(),
        adobe_transform: None,
        metadata: Vec::new(),
    };
    let _ = reconstruct_image(&info, &[0, 0, 0xFF, 0xD0, 0], None);
    let baseline = include_bytes!("../../../../tests/fixtures/input/images/jpeg/1x1.jpg");
    let progressive =
        include_bytes!("../../../../tests/fixtures/input/images/jpeg/progressive.jpg");
    for checks in 0..=7 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = decode(baseline, Some(&token));
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = decode(progressive, Some(&token));
    }
}
