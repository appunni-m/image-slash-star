// Modified Rust port copyright (c) 2026 Appunni M.
// Derived from libjpeg-turbo/IJG sources; see third_party/libjpeg-turbo/.

use crate::codecs::{CodecError, CodecResult, OptionCodecExt};
use crate::types::{ColorType, DecodedImage};

use super::bit_reader::BitReader;
use super::huffman::HuffTable;
use super::idct::{self, YccColorConverter, extend, jpeg_idct_islow};
use super::parser::{JpegInfo, parse_jpeg};
use super::progressive::progressive_reconstruct;
use super::upsample::{crop_component, fancy_upsample};

// ── Entropy Decoding ──────────────────────────────────────────────────────

pub(super) fn decode_block(
    br: &mut BitReader,
    dc_table: &HuffTable,
    ac_table: &HuffTable,
    last_dc: &mut i32,
    block_zigzag: &mut [i32; 64],
) -> CodecResult<()> {
    for coeff in block_zigzag.iter_mut() {
        *coeff = 0;
    }

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
    block_zigzag[0] = *last_dc;

    let mut k = 1usize;
    while k < 64 {
        let sym = ac_table.decode(br)?;
        if sym == 0x00 {
            break;
        }
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
            block_zigzag[k] = extend(bits, size);
            k = k.saturating_add(1);
        } else {
            return Err(CodecError::Malformed(
                "invalid JPEG AC run-length symbol".to_owned(),
            ));
        }
    }
    Ok(())
}

// ── Image Reconstruction (baseline) ───────────────────────────────────────

pub(super) fn reconstruct_image(info: &JpegInfo, data: &[u8]) -> CodecResult<DecodedImage> {
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

    let mut comp_buffers: Vec<Vec<u8>> = info
        .components
        .iter()
        .enumerate()
        .map(|(i, _)| vec![128u8; comp_buf_width[i].saturating_mul(comp_buf_height[i])])
        .collect();

    let mut dc_predictors: Vec<i32> = vec![0; usize::from(info.num_components)];
    let mut block_zigzag = [0i32; 64];
    let mut block_natural = [0i32; 64];
    let mut workspace = [0i32; 64];
    let converter = YccColorConverter::new();

    // Extract entropy segments (between RST markers)
    let entropy_segments = extract_entropy_segments(data, info.entropy_start, info.eoi_pos);
    if entropy_segments.segments.is_empty() {
        return Err(CodecError::Malformed(
            "JPEG contains no entropy segment".to_owned(),
        ));
    }

    let total_mcus = bounded_usize(num_mcus_x.saturating_mul(num_mcus_y));
    let mut segment_iter = entropy_segments.segments.iter().peekable();
    let mut seg_idx = 0usize;
    let mcus_per_seg = if info.restart_interval > 0 {
        usize::from(info.restart_interval)
    } else {
        total_mcus
    };

    while let Some(&(seg_start, seg_end)) = segment_iter.next() {
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
                let quant_table = info
                    .quant_tables
                    .get(usize::from(comp.quant_tbl))
                    .and_then(Option::as_ref)
                    .malformed("missing JPEG quantization table")?;

                for by in 0..usize::from(comp.v_samp) {
                    for bx in 0..usize::from(comp.h_samp) {
                        if let Err(error) = decode_block(
                            &mut br,
                            dc_table,
                            ac_table,
                            &mut dc_predictors[scan_comp.comp_index],
                            &mut block_zigzag,
                        ) {
                            return Err(error.context("baseline block"));
                        }
                        // Dequantize and IDCT
                        for (coefficient, &quantizer) in block_zigzag.iter_mut().zip(quant_table) {
                            *coefficient = coefficient.saturating_mul(i32::from(quantizer));
                        }
                        for i in 0..64 {
                            block_natural[idct::JPEG_NATURAL_ORDER[i]] = block_zigzag[i];
                        }
                        jpeg_idct_islow(&mut block_natural, &mut workspace);

                        let buf_w = comp_buf_width[scan_comp.comp_index];
                        let block_x = mcu_x
                            .saturating_mul(usize::from(comp.h_samp))
                            .saturating_add(bx)
                            .saturating_mul(8);
                        let block_y = mcu_y
                            .saturating_mul(usize::from(comp.v_samp))
                            .saturating_add(by)
                            .saturating_mul(8);
                        for row in 0usize..8 {
                            for col in 0usize..8 {
                                let natural_index = row.saturating_mul(8).saturating_add(col);
                                let px =
                                    block_natural[natural_index].clamp(0, 255).to_le_bytes()[0];
                                let bi = block_y
                                    .saturating_add(row)
                                    .saturating_mul(buf_w)
                                    .saturating_add(block_x.saturating_add(col));
                                comp_buffers[scan_comp.comp_index][bi] = px;
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
        let mut pixels = Vec::with_capacity(w.saturating_mul(h).saturating_mul(3));
        for y in 0..h {
            for x in 0..w {
                let (r, g, b) = converter.ycc_to_rgb(
                    y_buf[y.saturating_mul(y_w).saturating_add(x)],
                    cb_upsampled[y.saturating_mul(chroma_stride).saturating_add(x)],
                    cr_upsampled[y.saturating_mul(chroma_stride).saturating_add(x)],
                );
                pixels.push(r);
                pixels.push(g);
                pixels.push(b);
            }
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

// ── Public API ────────────────────────────────────────────────────────────

/// Decode JPEG bytes into a DecodedImage (pixel-perfect with libjpeg).
///
/// Supports baseline JPEG (SOF0) and progressive JPEG (SOF2) with:
/// - 8-bit precision
/// - 4:2:0, 4:2:2, 4:4:4 and 4:1:1 chroma subsampling
/// - Grayscale (1 component) and YCbCr (3 components)
/// - Restart markers (DRI)
/// - Progressive: DC first, DC refine, AC first, AC refine scans
pub fn decode(data: &[u8]) -> CodecResult<(DecodedImage, usize)> {
    let info = parse_jpeg(data)?;

    debug_assert!(!info.scan_components.is_empty());

    let consumed = info.eoi_pos.saturating_add(2);
    let image = if info.progressive {
        progressive_reconstruct(&info, data)
    } else {
        reconstruct_image(&info, data)
    }?;
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
        dc_huff_tables: vec![Some(dc_zero.clone())],
        ac_huff_tables: vec![Some(ac_eob.clone())],
        scan_components: vec![super::parser::ScanComponent {
            comp_index: 0,
            dc_tbl: 0,
            ac_tbl: 0,
        }],
        restart_interval: 1,
        entropy_start: 0,
        eoi_pos: 5,
        max_h_samp: 1,
        max_v_samp: 1,
        progressive: false,
        scans: Vec::new(),
        adobe_transform: None,
    };
    let _ = reconstruct_image(&info, &[0, 0, 0xFF, 0xD0, 0]);
}
