// Modified Rust port copyright (c) 2026 Appunni M.
// Derived from libjpeg-turbo/IJG sources; see third_party/libjpeg-turbo/.

// ── JPEG Encoder ─────────────────────────────────────────────────────────
// libjpeg-turbo 3.1.4.1 port (jfdctint.c, jcparam.c, jchuff.c, jcphuff.c,
// jcmaster.c, jcmarker.c, jccolor.c, jcsample.c).
//
// Supports baseline (SOF0) and progressive (SOF2) encoding for YCbCr 4:4:4,
// 4:2:2, 4:2:0 and grayscale.  Entropy coding uses the standard IJG Huffman
// tables; quantization uses ISLOW divisors (quantval<<3) with round-to-nearest
// division matching jcdctmgr.c's reciprocal quantize path.

#![allow(dead_code)] // encoder wired up incrementally; tables/markers used by encode_*

mod fdct;
mod huffman;
mod marker;
mod quant;

use crate::encode_options::EncodeOptions;
use crate::types::DecodedImage;

/// Zigzag scan order (matches idct.rs JPEG_NATURAL_ORDER).
const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// Per-component prepared data: quantized coefficient blocks in NATURAL order,
/// indexed by block_row * blocks_per_row + block_col.
struct CompData {
    blocks: Vec<[i16; 64]>,
    blocks_per_row: usize,
    block_rows: usize,
    h_samp: u8,
    v_samp: u8,
    quant_slot: u8,
    /// Component id (Y=1, Cb=2, Cr=3 in libjpeg JCS_YCbCr).
    id: u8,
    dc_tbl: u8,
    ac_tbl: u8,
}

pub(crate) fn encode(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    let w = img.width as usize;
    let h = img.height as usize;
    if w == 0 || h == 0 {
        return None;
    }
    let pixels = img.as_bytes();

    let num_components: u8 = match img.color {
        crate::types::ColorType::L8 => 1,
        _ => 3,
    };

    let quality = opts.quality.unwrap_or(75);
    let progressive = opts.progressive.unwrap_or(false);
    let optimize = opts.optimize.unwrap_or(false);
    let subsampling = opts.subsampling.as_deref().unwrap_or("420");
    let restart_rows = opts
        .extra
        .get("restart_interval")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);

    let params = quant::build_params(quality, subsampling, num_components as usize);

    // RGB → YCbCr (jccolor.c) or grayscale pass-through.
    let (y_plane, cb_plane, cr_plane) = if num_components == 1 {
        let mut y = vec![0u8; w * h];
        for i in 0..(w * h).min(pixels.len()) {
            y[i] = pixels[i];
        }
        (y, Vec::new(), Vec::new())
    } else {
        rgb_to_ycbcr(pixels, w, h)
    };

    // Sampling factors (h, v) per component; max is the reference grid.
    let (y_hs, y_vs, cb_hs, cb_vs, cr_hs, cr_vs, max_h, max_v) = match (num_components, subsampling)
    {
        (1, _) => (1u8, 1u8, 0u8, 0u8, 0u8, 0u8, 1u8, 1u8),
        (_, "444") => (1, 1, 1, 1, 1, 1, 1, 1),
        (_, "422") => (2, 1, 1, 1, 1, 1, 2, 1),
        // 4:2:0 (default)
        _ => (2, 2, 1, 1, 1, 1, 2, 2),
    };

    // Component image dimensions on the sampling grid. libjpeg expands the
    // horizontal source edge through a complete downsampled DCT block before
    // filtering; bottom rows are replicated later by fdct_quantize.
    let y_w = w;
    let y_h = h;
    let cb_w = (w * cb_hs as usize)
        .div_ceil(max_h as usize * 8)
        .checked_mul(8)?;
    let cb_h = (h * cb_vs as usize).div_ceil(max_v as usize);
    let cr_w = cb_w;
    let cr_h = cb_h;

    // Downsample chroma (jcsample.c h2v2 / h2v1 / identity).
    let cb_ds = if num_components >= 3 {
        downsample(
            &cb_plane,
            w,
            h,
            cb_w,
            cb_h,
            max_h as usize / cb_hs as usize,
            max_v as usize / cb_vs as usize,
        )
    } else {
        Vec::new()
    };
    let cr_ds = if num_components >= 3 {
        downsample(
            &cr_plane,
            w,
            h,
            cr_w,
            cr_h,
            max_h as usize / cr_hs as usize,
            max_v as usize / cr_vs as usize,
        )
    } else {
        Vec::new()
    };

    // Prepare per-component quantized coefficient blocks (natural order).
    let mut comps: Vec<CompData> = Vec::with_capacity(num_components as usize);

    // Y
    let y_blocks = fdct_quantize(&y_plane, y_w, y_h, &params.quant_tables[0]);
    comps.push(CompData {
        blocks: y_blocks.0,
        blocks_per_row: y_blocks.1,
        block_rows: y_blocks.2,
        h_samp: y_hs,
        v_samp: y_vs,
        quant_slot: 0,
        id: 1,
        dc_tbl: 0,
        ac_tbl: 0,
    });

    if num_components >= 3 {
        for (plane, cw, ch, hs, vs, id) in [
            (&cb_ds, cb_w, cb_h, cb_hs, cb_vs, 2u8),
            (&cr_ds, cr_w, cr_h, cr_hs, cr_vs, 3u8),
        ] {
            let blk = fdct_quantize(plane, cw, ch, &params.quant_tables[1]);
            comps.push(CompData {
                blocks: blk.0,
                blocks_per_row: blk.1,
                block_rows: blk.2,
                h_samp: hs,
                v_samp: vs,
                quant_slot: 1,
                id,
                dc_tbl: 1,
                ac_tbl: 1,
            });
        }
    }

    let mcu_columns = comps[0]
        .blocks_per_row
        .checked_mul(8)?
        .div_ceil(usize::from(max_h).checked_mul(8)?);
    let restart_interval = if restart_rows == 0 {
        0
    } else {
        u16::try_from(restart_rows.checked_mul(mcu_columns)?).ok()?
    };

    // Derive standard Huffman tables.
    let dc_luma = huffman::derive_table(&huffman::STD_DC_LUMA.0, &huffman::STD_DC_LUMA.1);
    let dc_chroma = huffman::derive_table(&huffman::STD_DC_CHROMA.0, &huffman::STD_DC_CHROMA.1);
    let ac_luma = huffman::derive_table(&huffman::STD_AC_LUMA.0, &huffman::STD_AC_LUMA.1);
    let ac_chroma = huffman::derive_table(&huffman::STD_AC_CHROMA.0, &huffman::STD_AC_CHROMA.1);
    let (optimized_dc, optimized_ac) = if !progressive && optimize {
        let (dc_frequencies, ac_frequencies) =
            baseline_frequencies(&comps, max_h, max_v, restart_interval)?;
        (
            [
                Some(huffman::optimal_table(&dc_frequencies[0])?),
                (num_components >= 3)
                    .then(|| huffman::optimal_table(&dc_frequencies[1]))
                    .flatten(),
            ],
            [
                Some(huffman::optimal_table(&ac_frequencies[0])?),
                (num_components >= 3)
                    .then(|| huffman::optimal_table(&ac_frequencies[1]))
                    .flatten(),
            ],
        )
    } else {
        ([None, None], [None, None])
    };
    let dc_tables = [
        optimized_dc[0]
            .as_ref()
            .map_or(&dc_luma, |table| &table.derived),
        optimized_dc[1]
            .as_ref()
            .map_or(&dc_chroma, |table| &table.derived),
    ];
    let ac_tables = [
        optimized_ac[0]
            .as_ref()
            .map_or(&ac_luma, |table| &table.derived),
        optimized_ac[1]
            .as_ref()
            .map_or(&ac_chroma, |table| &table.derived),
    ];

    let mut out = Vec::new();
    marker::write_soi(&mut out);
    marker::write_jfif_app0(&mut out);
    if let Some(exif_hex) = opts.extra.get("exif_hex") {
        let exif = decode_hex(exif_hex)?;
        marker::write_exif_app1(&mut out, &exif)?;
    }

    // Write DQT tables (one per unique quant slot).
    let mut emitted = [false; 4];
    for c in &comps {
        let slot = c.quant_slot as usize;
        if !emitted[slot] {
            marker::write_dqt(&mut out, c.quant_slot, 0, &params.quant_tables[slot]);
            emitted[slot] = true;
        }
    }

    // SOF marker.
    let sof_marker: u8 = if progressive { 0xC2 } else { 0xC0 };
    let sof_comps: Vec<(u8, u8, u8, u8)> = comps
        .iter()
        .map(|c| (c.id, c.h_samp, c.v_samp, c.quant_slot))
        .collect();
    marker::write_sof(&mut out, sof_marker, w as u16, h as u16, &sof_comps);

    // DHT tables. Baseline: all 4 standard tables up front. Progressive: DHT
    // is emitted per-scan with only the tables that scan uses.
    if !progressive {
        if let Some(table) = &optimized_dc[0] {
            marker::write_dht(&mut out, 0, 0, &table.bits, &table.values);
        } else {
            marker::write_dht(
                &mut out,
                0,
                0,
                &huffman::STD_DC_LUMA.0,
                &huffman::STD_DC_LUMA.1,
            );
        }
        if let Some(table) = &optimized_ac[0] {
            marker::write_dht(&mut out, 1, 0, &table.bits, &table.values);
        } else {
            marker::write_dht(
                &mut out,
                1,
                0,
                &huffman::STD_AC_LUMA.0,
                &huffman::STD_AC_LUMA.1,
            );
        }
        if num_components >= 3 {
            if let Some(table) = &optimized_dc[1] {
                marker::write_dht(&mut out, 0, 1, &table.bits, &table.values);
            } else {
                marker::write_dht(
                    &mut out,
                    0,
                    1,
                    &huffman::STD_DC_CHROMA.0,
                    &huffman::STD_DC_CHROMA.1,
                );
            }
            if let Some(table) = &optimized_ac[1] {
                marker::write_dht(&mut out, 1, 1, &table.bits, &table.values);
            } else {
                marker::write_dht(
                    &mut out,
                    1,
                    1,
                    &huffman::STD_AC_CHROMA.0,
                    &huffman::STD_AC_CHROMA.1,
                );
            }
        }

        if restart_interval != 0 {
            marker::write_dri(&mut out, restart_interval);
        }

        // Single SOS for baseline (interleaved).
        let sos_comps: Vec<(u8, u8, u8)> =
            comps.iter().map(|c| (c.id, c.dc_tbl, c.ac_tbl)).collect();
        marker::write_sos(&mut out, &sos_comps, 0, 63, 0, 0);

        encode_baseline_entropy(
            &mut out,
            &comps,
            max_h,
            max_v,
            &dc_tables,
            &ac_tables,
            restart_interval,
        );
    } else {
        encode_progressive_scans_exact(&mut out, &comps, num_components, max_h, max_v, &params)?;
    }

    marker::write_eoi(&mut out);
    Some(out)
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    (0..value.len())
        .step_by(2)
        .map(|index| {
            let end = index.checked_add(2)?;
            u8::from_str_radix(value.get(index..end)?, 16).ok()
        })
        .collect()
}

// ── Color conversion (jccolor.c) ─────────────────────────────────────────

fn rgb_to_ycbcr(pixels: &[u8], w: usize, h: usize) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let n = w * h;
    let mut y = vec![0u8; n];
    let mut cb = vec![0u8; n];
    let mut cr = vec![0u8; n];
    let npix = n.min(pixels.len() / 3);
    for i in 0..npix {
        let r = pixels[i * 3] as i32;
        let g = pixels[i * 3 + 1] as i32;
        let b = pixels[i * 3 + 2] as i32;
        // ✅ VERIFIED: libjpeg-turbo 3.1.4.1 jccolor.c:214-243 and
        // jccolext.c:37-73. Chroma includes CENTERJSAMPLE before descaling;
        // the prior port accidentally added 128 before, rather than after,
        // the 16-bit fixed-point scale.
        y[i] = ((19595 * r + 38470 * g + 7471 * b + 32768) >> 16) as u8;
        cb[i] = ((-11059 * r - 21709 * g + 32768 * b + (128 << 16) + 32767) >> 16) as u8;
        cr[i] = ((32768 * r - 27439 * g - 5329 * b + (128 << 16) + 32767) >> 16) as u8;
    }
    (y, cb, cr)
}

// ── Downsampling (jcsample.c) ────────────────────────────────────────────
//
// libjpeg's default smoothing factor is zero, so its h2v1/h2v2 box filters
// use alternating rounding biases to avoid a systematic upward bias.

fn downsample(
    plane: &[u8],
    sw: usize,
    sh: usize,
    dw: usize,
    dh: usize,
    hr: usize,
    vr: usize,
) -> Vec<u8> {
    let mut out = vec![0u8; dw * dh];
    if hr == 1 && vr == 1 {
        // ✅ VERIFIED: libjpeg-turbo 3.1.4.1 jcsample.c:99-113,145-174.
        // Full-size components duplicate their right and bottom edge samples
        // through the padded DCT extent.
        for y in 0..dh {
            for x in 0..dw {
                out[y * dw + x] = plane[y.min(sh - 1) * sw + x.min(sw - 1)];
            }
        }
        return out;
    }
    for y in 0..dh {
        for x in 0..dw {
            let mut sum = 0u32;
            for vy in 0..vr {
                for vx in 0..hr {
                    let sy = (y * vr + vy).min(sh - 1);
                    let sx = (x * hr + vx).min(sw - 1);
                    sum += plane[sy * sw + sx] as u32;
                }
            }
            // ✅ VERIFIED: libjpeg-turbo 3.1.4.1 jcsample.c:227-299.
            // h2v1 alternates 0/1; h2v2 alternates 1/2 for each output row.
            let bias = match (hr, vr) {
                (2, 1) => (x & 1) as u32,
                (2, 2) => 1 + (x & 1) as u32,
                _ => 0,
            };
            out[y * dw + x] = ((sum + bias) / u32::try_from(hr * vr).unwrap_or(1)) as u8;
        }
    }
    out
}

// ── FDCT + quantize (jfdctint.c + jcdctmgr.c) ────────────────────────────

/// Forward DCT all blocks of a component plane, then quantize with ISLOW
/// divisors (quantval<<3) and round-to-nearest. Returns (blocks, blocks_per_row,
/// block_rows) in natural order.
fn fdct_quantize(
    plane: &[u8],
    w: usize,
    h: usize,
    qtable: &[u16; 64],
) -> (Vec<[i16; 64]>, usize, usize) {
    let blocks_per_row = (w + 7) / 8;
    let block_rows = (h + 7) / 8;
    let mut blocks = vec![[0i16; 64]; blocks_per_row * block_rows];

    for by in 0..block_rows {
        for bx in 0..blocks_per_row {
            let mut samples = [0i32; 64];
            for row in 0..8 {
                for col in 0..8 {
                    let py = by * 8 + row;
                    let px = bx * 8 + col;
                    let val = if py < h && px < w {
                        plane[py * w + px] as i32 - 128
                    } else {
                        // Edge replication (jccolext / edge extension).
                        let cpy = py.min(h.saturating_sub(1));
                        let cpx = px.min(w.saturating_sub(1));
                        plane[cpy * w + cpx] as i32 - 128
                    };
                    samples[row * 8 + col] = val;
                }
            }
            fdct::fdct_islow(&mut samples);
            // Quantize in natural order: divisor = quantval[i] << 3.
            let mut q = [0i16; 64];
            for i in 0..64 {
                let divisor = (qtable[i] as i32) << 3;
                let coef = samples[i];
                // Round-to-nearest, away from zero on .5 (matches reciprocal path).
                let qval = if coef < 0 {
                    -((-coef + (divisor >> 1)) / divisor)
                } else {
                    (coef + (divisor >> 1)) / divisor
                };
                q[i] = qval as i16;
            }
            blocks[by * blocks_per_row + bx] = q;
        }
    }
    (blocks, blocks_per_row, block_rows)
}

// ── Baseline entropy coding (jchuff.c) ───────────────────────────────────

fn baseline_frequencies(
    comps: &[CompData],
    max_h: u8,
    max_v: u8,
    restart_interval: u16,
) -> Option<([[u64; 256]; 2], [[u64; 256]; 2])> {
    // ✅ VERIFIED: libjpeg-turbo 3.1.4.1 jchuff.c's gather_statistics pass.
    // Traverse exactly the same MCU stream as encode_baseline_entropy so the
    // optimized table describes every symbol that the output pass will emit.
    let mcu_w = usize::from(max_h).checked_mul(8)?;
    let mcu_h = usize::from(max_v).checked_mul(8)?;
    let n_mcu_x = comps[0].blocks_per_row.checked_mul(8)?.div_ceil(mcu_w);
    let n_mcu_y = comps[0].block_rows.checked_mul(8)?.div_ceil(mcu_h);
    let mut dc = [[0u64; 256]; 2];
    let mut ac = [[0u64; 256]; 2];
    let mut last_dc = [0i32; 4];
    let mut mcus_until_restart = usize::from(restart_interval);

    for my in 0..n_mcu_y {
        for mx in 0..n_mcu_x {
            if restart_interval != 0 && mcus_until_restart == 0 {
                last_dc.fill(0);
                mcus_until_restart = usize::from(restart_interval);
            }
            for (ci, component) in comps.iter().enumerate() {
                let dc_slot = usize::from(component.dc_tbl);
                let ac_slot = usize::from(component.ac_tbl);
                for vertical in 0..usize::from(component.v_samp) {
                    for horizontal in 0..usize::from(component.h_samp) {
                        let block_row = my
                            .checked_mul(usize::from(component.v_samp))?
                            .checked_add(vertical)?;
                        let block_column = mx
                            .checked_mul(usize::from(component.h_samp))?
                            .checked_add(horizontal)?;
                        if block_row >= component.block_rows
                            || block_column >= component.blocks_per_row
                        {
                            dc[dc_slot][0] = dc[dc_slot][0].checked_add(1)?;
                            ac[ac_slot][0] = ac[ac_slot][0].checked_add(1)?;
                            continue;
                        }

                        let block =
                            &component.blocks[block_row * component.blocks_per_row + block_column];
                        let difference = i32::from(block[0]) - last_dc[ci];
                        last_dc[ci] = i32::from(block[0]);
                        let dc_symbol = usize::try_from(jpeg_nbits(difference)).ok()?;
                        dc[dc_slot][dc_symbol] = dc[dc_slot][dc_symbol].checked_add(1)?;

                        let mut run = 0usize;
                        for &natural_index in &ZIGZAG[1..] {
                            let coefficient = i32::from(block[natural_index]);
                            if coefficient == 0 {
                                run = run.checked_add(1)?;
                                continue;
                            }
                            while run >= 16 {
                                ac[ac_slot][0xf0] = ac[ac_slot][0xf0].checked_add(1)?;
                                run -= 16;
                            }
                            let width = usize::try_from(jpeg_nbits(coefficient)).ok()?;
                            let symbol = (run << 4) | width;
                            ac[ac_slot][symbol] = ac[ac_slot][symbol].checked_add(1)?;
                            run = 0;
                        }
                        if run != 0 {
                            ac[ac_slot][0] = ac[ac_slot][0].checked_add(1)?;
                        }
                    }
                }
            }
            mcus_until_restart = mcus_until_restart.saturating_sub(1);
        }
    }
    Some((dc, ac))
}

fn encode_baseline_entropy(
    out: &mut Vec<u8>,
    comps: &[CompData],
    max_h: u8,
    max_v: u8,
    dc_tables: &[&huffman::DerivedTable; 2],
    ac_tables: &[&huffman::DerivedTable; 2],
    restart_interval: u16,
) {
    let mcu_w = max_h as usize * 8;
    let mcu_h = max_v as usize * 8;
    let n_mcu_x = (comps[0].blocks_per_row * 8 + mcu_w - 1) / mcu_w;
    let n_mcu_y = (comps[0].block_rows * 8 + mcu_h - 1) / mcu_h;

    let mut bw = huffman::BitWriter::new();
    let mut last_dc = [0i32; 4];
    let mut mcus_until_restart = usize::from(restart_interval);
    let mut next_restart = 0u8;

    for my in 0..n_mcu_y {
        for mx in 0..n_mcu_x {
            if restart_interval != 0 && mcus_until_restart == 0 {
                bw.flush();
                out.append(&mut bw.out);
                marker::write_rst(out, next_restart);
                next_restart = (next_restart + 1) & 7;
                last_dc.fill(0);
                mcus_until_restart = usize::from(restart_interval);
            }
            for (ci, c) in comps.iter().enumerate() {
                let hs = c.h_samp as usize;
                let vs = c.v_samp as usize;
                let bpr = c.blocks_per_row;
                let dc_tbl = dc_tables[c.dc_tbl as usize];
                let ac_tbl = ac_tables[c.ac_tbl as usize];
                for vy in 0..vs {
                    for vx in 0..hs {
                        let brow = my * vs + vy;
                        let bcol = mx * hs + vx;
                        if brow >= c.block_rows || bcol >= bpr {
                            // ✅ VERIFIED: libjpeg-turbo 3.1.4.1
                            // jccoefct.c:174-199. Edge dummy blocks copy the
                            // preceding DC coefficient and zero every AC, so
                            // entropy coding emits both DC category 0 and EOB.
                            bw.write_bits(dc_tbl.codes[0], dc_tbl.lengths[0]);
                            bw.write_bits(ac_tbl.codes[0], ac_tbl.lengths[0]);
                            continue;
                        }
                        let blk = &c.blocks[brow * bpr + bcol];
                        encode_one_block(&mut bw, blk, &mut last_dc[ci], dc_tbl, ac_tbl);
                    }
                }
            }
            mcus_until_restart = mcus_until_restart.saturating_sub(1);
        }
    }
    bw.flush();
    out.extend_from_slice(&bw.out);
}

/// Encode one 8×8 block: DC difference + AC run/length in zigzag order.
fn encode_one_block(
    bw: &mut huffman::BitWriter,
    block: &[i16; 64],
    last_dc: &mut i32,
    dc_tbl: &huffman::DerivedTable,
    ac_tbl: &huffman::DerivedTable,
) {
    // DC coefficient difference (natural-order index 0).
    let dc = block[0] as i32;
    let diff = dc - *last_dc;
    *last_dc = dc;
    let nbits = jpeg_nbits(diff);
    // DC Huffman symbol = nbits, followed by nbits magnitude bits.
    bw.write_bits(dc_tbl.codes[nbits as usize], dc_tbl.lengths[nbits as usize]);
    if nbits > 0 {
        bw.write_bits(mag_bits(diff, nbits), nbits as u8);
    }

    // AC coefficients in zigzag order (k=1..63).
    let mut r = 0u32; // run length of zeros
    for k in 1..64 {
        let coef = block[ZIGZAG[k]] as i32;
        if coef == 0 {
            r += 1;
            continue;
        }
        // Emit ZRL (0xF0) for each full 16-zero run.
        while r >= 16 {
            bw.write_bits(ac_tbl.codes[0xF0], ac_tbl.lengths[0xF0]);
            r -= 16;
        }
        let nbits = jpeg_nbits(coef);
        let sym = ((r << 4) | nbits) as usize;
        bw.write_bits(ac_tbl.codes[sym], ac_tbl.lengths[sym]);
        bw.write_bits(mag_bits(coef, nbits), nbits as u8);
        r = 0;
    }
    // If trailing zeros, emit EOB (symbol 0x00).
    if r > 0 {
        bw.write_bits(ac_tbl.codes[0], ac_tbl.lengths[0]);
    }
}

/// Number of bits needed to represent |v| (JPEG_NBITS).  nbits(0)=0.
fn jpeg_nbits(v: i32) -> u32 {
    let mut a = if v < 0 { -v } else { v };
    let mut n = 0u32;
    while a > 0 {
        n += 1;
        a >>= 1;
    }
    n
}

/// The nbits magnitude bits to emit for a signed coefficient value (IJG
/// convention): positive → the value's nbits LSBs; negative → (value-1)'s
/// nbits LSBs (= bitwise complement of the absolute magnitude).
fn mag_bits(v: i32, nbits: u32) -> u32 {
    let emit = if v < 0 { v - 1 } else { v };
    (emit as u32) & ((1u32 << nbits) - 1)
}

// ── Progressive entropy coding (jcphuff.c + jcmaster.c scan script) ──────

/// A progressive scan descriptor (component index, Ss, Se, Ah, Al).
struct ProgScan {
    comps: Vec<usize>, // component indices in this scan
    ss: u8,
    se: u8,
    ah: u8,
    al: u8,
    is_dc: bool,
}

/// Build the default progressive scan script (jpeg_simple_progression, YCbCr).
fn default_progression_script(ncomp: u8) -> Vec<ProgScan> {
    let mut s = Vec::new();
    // ci index: 0=Y, 1=Cb, 2=Cr
    let dc = |comps: Vec<usize>| ProgScan {
        comps,
        ss: 0,
        se: 0,
        ah: 0,
        al: 1,
        is_dc: true,
    };
    let dc_refine = |comps: Vec<usize>, ah, al| ProgScan {
        comps,
        ss: 0,
        se: 0,
        ah,
        al,
        is_dc: true,
    };
    let ac = |comps: Vec<usize>, ss, se, ah, al| ProgScan {
        comps,
        ss,
        se,
        ah,
        al,
        is_dc: false,
    };

    if ncomp == 3 {
        // Initial DC scan (interleaved), Al=1
        s.push(dc(vec![0, 1, 2]));
        // Initial AC luma Ss=1,Se=5 Al=2
        s.push(ac(vec![0], 1, 5, 0, 2));
        // Chroma AC full band Ss=1,Se=63 Al=1
        s.push(ac(vec![2], 1, 63, 0, 1));
        s.push(ac(vec![1], 1, 63, 0, 1));
        // Complete luma AC Ss=6,Se=63 Al=2
        s.push(ac(vec![0], 6, 63, 0, 2));
        // Refine next bit of luma AC Ss=1,Se=63 Ah=2 Al=1
        s.push(ac(vec![0], 1, 63, 2, 1));
        // Finish DC successive approximation Ah=1 Al=0 (interleaved)
        s.push(dc_refine(vec![0, 1, 2], 1, 0));
        // Finish AC successive approximation (chroma then luma)
        s.push(ac(vec![2], 1, 63, 1, 0));
        s.push(ac(vec![1], 1, 63, 1, 0));
        s.push(ac(vec![0], 1, 63, 1, 0));
    } else {
        // Grayscale: 2 DC + 4 AC scans.
        s.push(dc(vec![0]));
        s.push(ac(vec![0], 1, 5, 0, 2));
        s.push(ac(vec![0], 6, 63, 0, 2));
        s.push(ac(vec![0], 1, 63, 2, 1));
        s.push(dc_refine(vec![0], 1, 0));
        s.push(ac(vec![0], 1, 63, 1, 0));
    }
    s
}

#[derive(Clone, Copy)]
enum ProgressiveEvent {
    Symbol { table: usize, value: u8 },
    Bits { value: u32, width: u8 },
}

fn encode_progressive_scans_exact(
    output: &mut Vec<u8>,
    components: &[CompData],
    component_count: u8,
    _maximum_horizontal_sampling: u8,
    _maximum_vertical_sampling: u8,
    _params: &quant::EncodeParams,
) -> Option<()> {
    // ✅ VERIFIED: libjpeg-turbo 3.1.4.1 jcphuff.c:179-1075 and
    // jcmaster.c's jpeg_simple_progression scan script.
    for scan in default_progression_script(component_count) {
        let events = progressive_events(&scan, components)?;
        let mut frequencies = [[0u64; 256]; 4];
        for &event in &events {
            if let ProgressiveEvent::Symbol { table, value } = event {
                frequencies[table][usize::from(value)] =
                    frequencies[table][usize::from(value)].checked_add(1)?;
            }
        }

        let mut tables: [Option<huffman::OptimalTable>; 4] = std::array::from_fn(|_| None);
        for table in 0..tables.len() {
            if frequencies[table].iter().any(|&frequency| frequency != 0) {
                let optimized = huffman::optimal_table(&frequencies[table])?;
                marker::write_dht(
                    output,
                    u8::from(scan.ss != 0),
                    u8::try_from(table).ok()?,
                    &optimized.bits,
                    &optimized.values,
                );
                tables[table] = Some(optimized);
            }
        }

        let scan_components = scan
            .comps
            .iter()
            .map(|&index| {
                let component = &components[index];
                let (dc_table, ac_table) = if scan.ss == 0 {
                    (if scan.ah == 0 { component.dc_tbl } else { 0 }, 0)
                } else {
                    (0, component.ac_tbl)
                };
                (component.id, dc_table, ac_table)
            })
            .collect::<Vec<_>>();
        marker::write_sos(output, &scan_components, scan.ss, scan.se, scan.ah, scan.al);

        let mut writer = huffman::BitWriter::new();
        for event in events {
            match event {
                ProgressiveEvent::Symbol { table, value } => {
                    let derived = &tables.get(table)?.as_ref()?.derived;
                    writer.write_bits(
                        derived.codes[usize::from(value)],
                        derived.lengths[usize::from(value)],
                    );
                }
                ProgressiveEvent::Bits { value, width } => writer.write_bits(value, width),
            }
        }
        writer.flush();
        output.extend_from_slice(&writer.out);
    }
    Some(())
}

fn progressive_events(scan: &ProgScan, components: &[CompData]) -> Option<Vec<ProgressiveEvent>> {
    if scan.ss == 0 {
        dc_progressive_events(scan, components)
    } else {
        ac_progressive_events(scan, components)
    }
}

fn dc_progressive_events(
    scan: &ProgScan,
    components: &[CompData],
) -> Option<Vec<ProgressiveEvent>> {
    let mut events = Vec::new();
    let interleaved = scan.comps.len() > 1;
    let maximum_horizontal_sampling = components.iter().map(|c| c.h_samp).max().unwrap_or(1);
    let maximum_vertical_sampling = components.iter().map(|c| c.v_samp).max().unwrap_or(1);
    let mcu_width = usize::from(maximum_horizontal_sampling).checked_mul(8)?;
    let mcu_height = usize::from(maximum_vertical_sampling).checked_mul(8)?;
    let mcu_columns = components[0]
        .blocks_per_row
        .checked_mul(8)?
        .div_ceil(mcu_width);
    let mcu_rows = components[0]
        .block_rows
        .checked_mul(8)?
        .div_ceil(mcu_height);
    let mut predictors = vec![0i32; scan.comps.len()];

    let mut append = |scan_index: usize, component_index: usize, block: &[i16; 64]| {
        let component = &components[component_index];
        let raw = i32::from(block[0]);
        if scan.ah == 0 {
            let transformed = raw >> scan.al;
            let difference = transformed - predictors[scan_index];
            predictors[scan_index] = transformed;
            let width = jpeg_nbits(difference);
            events.push(ProgressiveEvent::Symbol {
                table: usize::from(component.dc_tbl),
                value: u8::try_from(width).ok()?,
            });
            if width != 0 {
                events.push(ProgressiveEvent::Bits {
                    value: mag_bits(difference, width),
                    width: u8::try_from(width).ok()?,
                });
            }
        } else {
            events.push(ProgressiveEvent::Bits {
                value: u32::try_from((raw >> scan.al) & 1).ok()?,
                width: 1,
            });
        }
        Some(())
    };

    if interleaved {
        for mcu_row in 0..mcu_rows {
            for mcu_column in 0..mcu_columns {
                for (scan_index, &component_index) in scan.comps.iter().enumerate() {
                    let component = &components[component_index];
                    for vertical in 0..usize::from(component.v_samp) {
                        for horizontal in 0..usize::from(component.h_samp) {
                            let block_row = mcu_row
                                .checked_mul(usize::from(component.v_samp))?
                                .checked_add(vertical)?;
                            let block_column = mcu_column
                                .checked_mul(usize::from(component.h_samp))?
                                .checked_add(horizontal)?;
                            if block_row < component.block_rows
                                && block_column < component.blocks_per_row
                            {
                                append(
                                    scan_index,
                                    component_index,
                                    &component.blocks
                                        [block_row * component.blocks_per_row + block_column],
                                )?;
                            }
                        }
                    }
                }
            }
        }
    } else {
        let component_index = scan.comps[0];
        let component = &components[component_index];
        for block in &component.blocks {
            append(0, component_index, block)?;
        }
    }
    Some(events)
}

fn ac_progressive_events(
    scan: &ProgScan,
    components: &[CompData],
) -> Option<Vec<ProgressiveEvent>> {
    let component = &components[*scan.comps.first()?];
    let table = usize::from(component.ac_tbl);
    let mut events = Vec::new();
    let mut eob_run = 0u32;
    let mut correction_bits = Vec::<u8>::new();
    for block in &component.blocks {
        if scan.ah == 0 {
            append_ac_first_events(
                &mut events,
                block,
                scan,
                table,
                &mut eob_run,
                &mut correction_bits,
            )?;
        } else {
            append_ac_refine_events(
                &mut events,
                block,
                scan,
                table,
                &mut eob_run,
                &mut correction_bits,
            )?;
        }
    }
    flush_progressive_eob(&mut events, table, &mut eob_run, &mut correction_bits)?;
    Some(events)
}

fn append_ac_first_events(
    events: &mut Vec<ProgressiveEvent>,
    block: &[i16; 64],
    scan: &ProgScan,
    table: usize,
    eob_run: &mut u32,
    correction_bits: &mut Vec<u8>,
) -> Option<()> {
    let mut run = 0usize;
    let mut last_nonzero = None;
    for coefficient in scan.ss..=scan.se {
        let raw = i32::from(block[ZIGZAG[usize::from(coefficient)]]);
        let sign = raw >> 31;
        let absolute = (raw ^ sign).wrapping_sub(sign) >> scan.al;
        if absolute == 0 {
            run = run.checked_add(1)?;
            continue;
        }
        if eob_run != &0 {
            flush_progressive_eob(events, table, eob_run, correction_bits)?;
        }
        while run > 15 {
            events.push(ProgressiveEvent::Symbol { table, value: 0xf0 });
            run -= 16;
        }
        let width = jpeg_nbits(absolute);
        events.push(ProgressiveEvent::Symbol {
            table,
            value: u8::try_from((run << 4).checked_add(usize::try_from(width).ok()?)?).ok()?,
        });
        events.push(ProgressiveEvent::Bits {
            value: mag_bits(if sign == 0 { absolute } else { -absolute }, width),
            width: u8::try_from(width).ok()?,
        });
        run = 0;
        last_nonzero = Some(coefficient);
    }
    if last_nonzero != Some(scan.se) {
        *eob_run = eob_run.checked_add(1)?;
        if *eob_run == 0x7fff {
            flush_progressive_eob(events, table, eob_run, correction_bits)?;
        }
    }
    Some(())
}

fn append_ac_refine_events(
    events: &mut Vec<ProgressiveEvent>,
    block: &[i16; 64],
    scan: &ProgScan,
    table: usize,
    eob_run: &mut u32,
    correction_bits: &mut Vec<u8>,
) -> Option<()> {
    let coefficients = (scan.ss..=scan.se)
        .map(|coefficient| {
            let raw = i32::from(block[ZIGZAG[usize::from(coefficient)]]);
            let sign = raw >> 31;
            let absolute = (raw ^ sign).wrapping_sub(sign) >> scan.al;
            (raw, u32::try_from(absolute).ok())
        })
        .collect::<Vec<_>>();
    let last_new = coefficients
        .iter()
        .rposition(|(_, absolute)| *absolute == Some(1));
    let mut run = 0usize;
    let mut block_corrections = Vec::<u8>::new();
    let mut last_nonzero = None;

    for (index, &(raw, absolute)) in coefficients.iter().enumerate() {
        let absolute = absolute?;
        if absolute == 0 {
            run = run.checked_add(1)?;
            continue;
        }
        last_nonzero = Some(index);
        while run > 15 && last_new.is_some_and(|last| index <= last) {
            flush_progressive_eob(events, table, eob_run, correction_bits)?;
            events.push(ProgressiveEvent::Symbol { table, value: 0xf0 });
            run -= 16;
            append_correction_events(events, &mut block_corrections);
        }
        if absolute > 1 {
            block_corrections.push((absolute & 1) as u8);
            continue;
        }

        flush_progressive_eob(events, table, eob_run, correction_bits)?;
        events.push(ProgressiveEvent::Symbol {
            table,
            value: u8::try_from((run << 4) | 1).ok()?,
        });
        events.push(ProgressiveEvent::Bits {
            value: u32::from(raw >= 0),
            width: 1,
        });
        append_correction_events(events, &mut block_corrections);
        run = 0;
    }

    if last_nonzero != Some(coefficients.len().checked_sub(1)?) || !block_corrections.is_empty() {
        *eob_run = eob_run.checked_add(1)?;
        correction_bits.append(&mut block_corrections);
        if *eob_run == 0x7fff || correction_bits.len() > 937 {
            flush_progressive_eob(events, table, eob_run, correction_bits)?;
        }
    }
    Some(())
}

fn flush_progressive_eob(
    events: &mut Vec<ProgressiveEvent>,
    table: usize,
    eob_run: &mut u32,
    correction_bits: &mut Vec<u8>,
) -> Option<()> {
    if *eob_run == 0 {
        return Some(());
    }
    let width = eob_run.ilog2();
    events.push(ProgressiveEvent::Symbol {
        table,
        value: u8::try_from(width.checked_mul(16)?).ok()?,
    });
    if width != 0 {
        events.push(ProgressiveEvent::Bits {
            value: *eob_run,
            width: u8::try_from(width).ok()?,
        });
    }
    *eob_run = 0;
    append_correction_events(events, correction_bits);
    Some(())
}

fn append_correction_events(events: &mut Vec<ProgressiveEvent>, bits: &mut Vec<u8>) {
    events.extend(bits.drain(..).map(|value| ProgressiveEvent::Bits {
        value: u32::from(value),
        width: 1,
    }));
}
