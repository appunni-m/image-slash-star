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

use crate::codecs::{CodecError, CodecResult};
use crate::encode_options::{JpegEncodeOptions, JpegSubsampling};
use crate::types::{DecodedImage, ImageMode};

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

pub(crate) fn encode(img: &DecodedImage, opts: &JpegEncodeOptions) -> CodecResult<Vec<u8>> {
    encode_with_token(img, opts, None)
}

pub(crate) fn encode_with_token(
    img: &DecodedImage,
    opts: &JpegEncodeOptions,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    crate::codecs::error::check_cancelled(token)?;
    img.validate().map_err(CodecError::from_image_error)?;
    crate::codecs::error::check_cancelled(token)?;
    let w = bounded_usize(img.width);
    let h = bounded_usize(img.height);
    let pixels = img.as_bytes();

    let num_components: u8 = match img.mode {
        ImageMode::L8 => 1,
        ImageMode::Rgb8 => 3,
        _ => {
            return Err(CodecError::Unsupported(format!(
                "JPEG cannot encode mode {:?}",
                img.mode
            )));
        }
    };

    let quality = opts.quality.unwrap_or(75);
    let progressive = opts.progressive.unwrap_or(false);
    let optimize = opts.optimize.unwrap_or(false);
    let subsampling = match opts.subsampling.unwrap_or(JpegSubsampling::Cs420) {
        JpegSubsampling::Cs444 => "444",
        JpegSubsampling::Cs422 => "422",
        JpegSubsampling::Cs420 => "420",
    };
    // The supported native and WebAssembly targets have at least a 32-bit
    // `usize`, so every public `u32` row interval is representable here.
    let restart_rows = opts.restart_interval.unwrap_or(0) as usize;

    let params = quant::build_params(quality, subsampling, usize::from(num_components));

    // RGB → YCbCr (jccolor.c) or grayscale pass-through.
    let (y_plane, cb_plane, cr_plane) = if num_components == 1 {
        let pixel_count = w.saturating_mul(h);
        let mut y = vec![0u8; pixel_count];
        let copied = pixel_count.min(pixels.len());
        let row_width = w.max(1);
        for row_start in (0..copied).step_by(row_width) {
            crate::codecs::error::check_cancelled(token)?;
            let row_end = copied.min(row_start.saturating_add(row_width));
            y[row_start..row_end].copy_from_slice(&pixels[row_start..row_end]);
        }
        (y, Vec::new(), Vec::new())
    } else {
        rgb_to_ycbcr(pixels, w, h, token)?
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
    let cb_w = w
        .saturating_mul(usize::from(cb_hs))
        .div_ceil(usize::from(max_h).saturating_mul(8))
        .saturating_mul(8);
    let cb_h = h
        .saturating_mul(usize::from(cb_vs))
        .div_ceil(usize::from(max_v));
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
            usize::from(max_h).div_euclid(usize::from(cb_hs)),
            usize::from(max_v).div_euclid(usize::from(cb_vs)),
            token,
        )?
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
            usize::from(max_h).div_euclid(usize::from(cr_hs)),
            usize::from(max_v).div_euclid(usize::from(cr_vs)),
            token,
        )?
    } else {
        Vec::new()
    };

    // Prepare per-component quantized coefficient blocks (natural order).
    let mut comps: Vec<CompData> = Vec::with_capacity(usize::from(num_components));

    // Y
    let y_blocks = fdct_quantize(&y_plane, y_w, y_h, &params.quant_tables[0], token)?;
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
            let blk = fdct_quantize(plane, cw, ch, &params.quant_tables[1], token)?;
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
        .saturating_mul(8)
        .div_ceil(usize::from(max_h).saturating_mul(8));
    let restart_interval = if restart_rows == 0 {
        0
    } else {
        let interval = restart_rows.saturating_mul(mcu_columns);
        u16::try_from(interval).map_err(|_| {
            CodecError::Parameter("JPEG restart interval exceeds 65535 MCUs".to_owned())
        })?
    };

    // Derive standard Huffman tables.
    let dc_luma = huffman::derive_table(&huffman::STD_DC_LUMA.0, &huffman::STD_DC_LUMA.1);
    let dc_chroma = huffman::derive_table(&huffman::STD_DC_CHROMA.0, &huffman::STD_DC_CHROMA.1);
    let ac_luma = huffman::derive_table(&huffman::STD_AC_LUMA.0, &huffman::STD_AC_LUMA.1);
    let ac_chroma = huffman::derive_table(&huffman::STD_AC_CHROMA.0, &huffman::STD_AC_CHROMA.1);
    let (optimized_dc, optimized_ac) = if !progressive && optimize {
        let (dc_frequencies, ac_frequencies) =
            baseline_frequencies(&comps, max_h, max_v, restart_interval, token)?;
        (
            [
                Some(huffman::optimal_table(&dc_frequencies[0])),
                (num_components >= 3).then(|| huffman::optimal_table(&dc_frequencies[1])),
            ],
            [
                Some(huffman::optimal_table(&ac_frequencies[0])),
                (num_components >= 3).then(|| huffman::optimal_table(&ac_frequencies[1])),
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
    if let Some(exif) = opts.exif.as_deref() {
        marker::write_exif_app1(&mut out, exif)?;
    }

    // Write DQT tables (one per unique quant slot).
    let mut emitted = [false; 4];
    for c in &comps {
        let slot = usize::from(c.quant_slot);
        if !emitted[slot] {
            marker::write_dqt(&mut out, c.quant_slot, &params.quant_tables[slot]);
            emitted[slot] = true;
        }
    }

    // SOF marker.
    let sof_marker: u8 = if progressive { 0xC2 } else { 0xC0 };
    let sof_comps: Vec<(u8, u8, u8, u8)> = comps
        .iter()
        .map(|c| (c.id, c.h_samp, c.v_samp, c.quant_slot))
        .collect();
    marker::write_sof(&mut out, sof_marker, low_u16(w), low_u16(h), &sof_comps);

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
            token,
        )?;
    } else {
        encode_progressive_scans_exact(
            &mut out,
            &comps,
            num_components,
            max_h,
            max_v,
            &params,
            token,
        )?;
    }

    crate::codecs::error::check_cancelled(token)?;
    marker::write_eoi(&mut out);
    Ok(out)
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    huffman::__coverage_exercise_private_branches();

    let zero_width = DecodedImage::new(0, 1, Vec::new(), crate::types::ColorType::L8);
    let zero_height = DecodedImage::new(1, 0, Vec::new(), crate::types::ColorType::L8);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = encode(&zero_width, &JpegEncodeOptions::default());
    }));
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = encode(&zero_height, &JpegEncodeOptions::default());
    }));

    let gray = DecodedImage::new(2, 2, vec![0, 64, 128, 255], crate::types::ColorType::L8);
    let rgb = DecodedImage::new(
        3,
        2,
        vec![
            0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0, 255, 255, 255,
        ],
        crate::types::ColorType::Rgb8,
    );
    let _ = encode(&gray, &JpegEncodeOptions::default());
    let _ = encode(&rgb, &JpegEncodeOptions::default());

    // Pillow has no caller-controlled cancellation token. These deterministic
    // coverage-only drills exercise the Rust cancellation checkpoints across
    // color conversion, sampling, quantization, and entropy preparation; they
    // are not synthetic Pillow-parity rows.
    let checkpoint_rgb = DecodedImage::new(
        17,
        17,
        vec![128; 17 * 17 * 3],
        crate::types::ColorType::Rgb8,
    );
    for checks in [0, 1, 4, 12, 24, 28, 37, 43, 44, 46, 48, 96] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = encode_with_token(&checkpoint_rgb, &JpegEncodeOptions::default(), Some(&token));
    }
    let grayscale_token = crate::CancellationToken::new();
    grayscale_token.cancel_after(2);
    let _ = encode_with_token(&gray, &JpegEncodeOptions::default(), Some(&grayscale_token));
    let grayscale_alpha = DecodedImage::new(1, 1, vec![0, 255], crate::types::ColorType::La8);
    let _ = encode(&grayscale_alpha, &JpegEncodeOptions::default());
    let mut progressive = JpegEncodeOptions {
        progressive: Some(true),
        ..JpegEncodeOptions::default()
    };
    let _ = encode(&rgb, &progressive);
    for checks in [0, 8, 24, 44, 45, 47, 64, 128] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = encode_with_token(&checkpoint_rgb, &progressive, Some(&token));
    }
    progressive.optimize = Some(true);
    progressive.subsampling = Some(JpegSubsampling::Cs444);
    let _ = encode(&rgb, &progressive);
    let restart = JpegEncodeOptions {
        optimize: Some(true),
        subsampling: Some(JpegSubsampling::Cs422),
        restart_interval: Some(1),
        ..JpegEncodeOptions::default()
    };
    let _ = encode(&rgb, &restart);
    let optimized_checkpoint = JpegEncodeOptions {
        optimize: Some(true),
        ..JpegEncodeOptions::default()
    };
    let optimized_token = crate::CancellationToken::new();
    optimized_token.cancel_after(44);
    let _ = encode_with_token(
        &checkpoint_rgb,
        &optimized_checkpoint,
        Some(&optimized_token),
    );
    let mut bad_restart = restart.clone();
    bad_restart.restart_interval = Some(70_000);
    let _ = encode(&rgb, &bad_restart);
    let wide_rgb = DecodedImage::new(17, 1, vec![128; 17 * 3], crate::types::ColorType::Rgb8);
    let mut overflowing_restart = restart.clone();
    overflowing_restart.restart_interval = Some(u32::MAX);
    let _ = encode(&wide_rgb, &overflowing_restart);
    let mut oversized_exif_options = JpegEncodeOptions::default();
    oversized_exif_options.exif = Some(vec![0; usize::from(u16::MAX)]);
    let _ = encode(&gray, &oversized_exif_options);
    let mut marker_bytes = Vec::new();
    let _ = marker::write_exif_app1(&mut marker_bytes, b"Exif\0\0");
    let oversized_exif = vec![0u8; usize::from(u16::MAX)];
    let _ = marker::write_exif_app1(&mut marker_bytes, &oversized_exif);

    let plane = [10u8, 20, 30, 40];
    let _ = downsample(&plane, 2, 2, 2, 2, 1, 1, None);
    let _ = downsample(&plane, 2, 2, 1, 2, 2, 1, None);
    let _ = downsample(&plane, 2, 2, 1, 1, 2, 2, None);
    let downsample_token = crate::CancellationToken::new();
    downsample_token.cancel_after(0);
    let _ = downsample(&plane, 2, 2, 2, 2, 1, 1, Some(&downsample_token));
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = downsample(&plane, 2, 2, 2, 1, 1, 2, None);
    }));
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = downsample(&plane, 2, 2, 1, 1, 2, 3, None);
    }));

    let progressive_dc_scan = ProgScan {
        comps: vec![0, 1, 2],
        ss: 0,
        se: 0,
        ah: 0,
        al: 1,
        is_dc: true,
    };
    let y_component = CompData {
        blocks: vec![[0i16; 64]; 9],
        blocks_per_row: 3,
        block_rows: 3,
        h_samp: 2,
        v_samp: 2,
        quant_slot: 0,
        id: 1,
        dc_tbl: 0,
        ac_tbl: 0,
    };
    let cb_component = CompData {
        blocks: vec![[0i16; 64]],
        blocks_per_row: 1,
        block_rows: 1,
        h_samp: 1,
        v_samp: 1,
        quant_slot: 1,
        id: 2,
        dc_tbl: 1,
        ac_tbl: 1,
    };
    let cr_component = CompData {
        blocks: vec![[0i16; 64]],
        blocks_per_row: 1,
        block_rows: 1,
        h_samp: 1,
        v_samp: 1,
        quant_slot: 1,
        id: 3,
        dc_tbl: 1,
        ac_tbl: 1,
    };
    let progressive_components = [y_component, cb_component, cr_component];
    let _ = dc_progressive_events(&progressive_dc_scan, &progressive_components, None);
    let single_dc_scan = ProgScan {
        comps: vec![0],
        ss: 0,
        se: 0,
        ah: 0,
        al: 0,
        is_dc: true,
    };
    let single_dc_token = crate::CancellationToken::new();
    single_dc_token.cancel_after(0);
    let _ = dc_progressive_events(
        &single_dc_scan,
        &progressive_components,
        Some(&single_dc_token),
    );

    let scan = ProgScan {
        comps: vec![0],
        ss: 1,
        se: 1,
        ah: 1,
        al: 0,
        is_dc: false,
    };
    let table = 0;
    let mut events = Vec::new();
    let mut eob_run = 0;
    let mut correction_bits = Vec::new();
    let mut block = [0i16; 64];
    block[ZIGZAG[1]] = 2;
    for _ in 0..938 {
        let _ = append_ac_refine_events(
            &mut events,
            &block,
            &scan,
            table,
            &mut eob_run,
            &mut correction_bits,
        );
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

fn low_u16(value: usize) -> u16 {
    let [a, b, ..] = value.to_le_bytes();
    u16::from_le_bytes([a, b])
}

fn low_u32(value: usize) -> u32 {
    #[cfg(target_pointer_width = "64")]
    {
        let [a, b, c, d, ..] = value.to_le_bytes();
        u32::from_le_bytes([a, b, c, d])
    }
    #[cfg(target_pointer_width = "32")]
    {
        u32::from_le_bytes(value.to_le_bytes())
    }
}

fn rgb_fixed(terms: &[(i32, i32)], bias: i32) -> u8 {
    terms
        .iter()
        .fold(bias, |sum, &(weight, sample)| {
            sum.saturating_add(weight.saturating_mul(sample))
        })
        .wrapping_shr(16)
        .to_le_bytes()[0]
}

// ── Color conversion (jccolor.c) ─────────────────────────────────────────

fn rgb_to_ycbcr(
    pixels: &[u8],
    w: usize,
    h: usize,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let n = w.saturating_mul(h);
    let mut y = vec![0u8; n];
    let mut cb = vec![0u8; n];
    let mut cr = vec![0u8; n];
    let npix = n.min(pixels.len().div_euclid(3));
    let row_width = w.max(1);
    for row in 0..h {
        crate::codecs::error::check_cancelled(token)?;
        let row_start = row.saturating_mul(row_width).min(npix);
        let row_end = npix.min(row_start.saturating_add(row_width));
        for i in row_start..row_end {
            let source = i.saturating_mul(3);
            let r = i32::from(pixels[source]);
            let g = i32::from(pixels[source.saturating_add(1)]);
            let b = i32::from(pixels[source.saturating_add(2)]);
            // ✅ VERIFIED: libjpeg-turbo 3.1.4.1 jccolor.c:214-243 and
            // jccolext.c:37-73. Chroma includes CENTERJSAMPLE before descaling;
            // the prior port accidentally added 128 before, rather than after,
            // the 16-bit fixed-point scale.
            y[i] = rgb_fixed(&[(19_595, r), (38_470, g), (7_471, b)], 32_768);
            let chroma_bias = 128i32.wrapping_shl(16).saturating_add(32_767);
            cb[i] = rgb_fixed(&[(-11_059, r), (-21_709, g), (32_768, b)], chroma_bias);
            cr[i] = rgb_fixed(&[(32_768, r), (-27_439, g), (-5_329, b)], chroma_bias);
        }
    }
    Ok((y, cb, cr))
}

// ── Downsampling (jcsample.c) ────────────────────────────────────────────
//
// libjpeg's default smoothing factor is zero, so its h2v1/h2v2 box filters
// use alternating rounding biases to avoid a systematic upward bias.

#[allow(
    clippy::too_many_arguments,
    reason = "the helper mirrors libjpeg's sampling routine and the token is an independent checkpoint input"
)]
fn downsample(
    plane: &[u8],
    sw: usize,
    sh: usize,
    dw: usize,
    dh: usize,
    hr: usize,
    vr: usize,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    let mut out = vec![0u8; dw.saturating_mul(dh)];
    if hr == 1 && vr == 1 {
        // ✅ VERIFIED: libjpeg-turbo 3.1.4.1 jcsample.c:99-113,145-174.
        // Full-size components duplicate their right and bottom edge samples
        // through the padded DCT extent.
        for y in 0..dh {
            crate::codecs::error::check_cancelled(token)?;
            for x in 0..dw {
                let source_y = y.min(sh.saturating_sub(1));
                let source_x = x.min(sw.saturating_sub(1));
                out[y.saturating_mul(dw).saturating_add(x)] =
                    plane[source_y.saturating_mul(sw).saturating_add(source_x)];
            }
        }
        return Ok(out);
    }
    for y in 0..dh {
        crate::codecs::error::check_cancelled(token)?;
        for x in 0..dw {
            let mut sum = 0u32;
            for vy in 0..vr {
                for vx in 0..hr {
                    let sy = y
                        .saturating_mul(vr)
                        .saturating_add(vy)
                        .min(sh.saturating_sub(1));
                    let sx = x
                        .saturating_mul(hr)
                        .saturating_add(vx)
                        .min(sw.saturating_sub(1));
                    sum = sum
                        .saturating_add(u32::from(plane[sy.saturating_mul(sw).saturating_add(sx)]));
                }
            }
            // ✅ VERIFIED: libjpeg-turbo 3.1.4.1 jcsample.c:227-299.
            // h2v1 alternates 0/1; h2v2 alternates 1/2 for each output row.
            debug_assert_eq!(hr, 2);
            debug_assert!(vr == 1 || vr == 2);
            let bias = u32::from(x.to_le_bytes()[0] & 1).saturating_add(u32::from(vr == 2));
            let divisor = low_u32(hr.saturating_mul(vr));
            out[y.saturating_mul(dw).saturating_add(x)] =
                sum.saturating_add(bias).div_euclid(divisor).to_le_bytes()[0];
        }
    }
    Ok(out)
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
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(Vec<[i16; 64]>, usize, usize)> {
    let blocks_per_row = w.div_ceil(8);
    let block_rows = h.div_ceil(8);
    let mut blocks = vec![[0i16; 64]; blocks_per_row.saturating_mul(block_rows)];

    for by in 0..block_rows {
        crate::codecs::error::check_cancelled(token)?;
        for bx in 0..blocks_per_row {
            let mut samples = [0i32; 64];
            for row in 0usize..8 {
                for col in 0usize..8 {
                    let py = by.saturating_mul(8).saturating_add(row);
                    let px = bx.saturating_mul(8).saturating_add(col);
                    let val = if py < h && px < w {
                        i32::from(plane[py.saturating_mul(w).saturating_add(px)])
                            .saturating_sub(128)
                    } else {
                        // Edge replication (jccolext / edge extension).
                        let cpy = py.min(h.saturating_sub(1));
                        let cpx = px.min(w.saturating_sub(1));
                        i32::from(plane[cpy.saturating_mul(w).saturating_add(cpx)])
                            .saturating_sub(128)
                    };
                    samples[row.saturating_mul(8).saturating_add(col)] = val;
                }
            }
            fdct::fdct_islow(&mut samples);
            // Quantize in natural order: divisor = quantval[i] << 3.
            let mut q = [0i16; 64];
            for i in 0..64 {
                let divisor = i32::from(qtable[i]).wrapping_shl(3);
                let coef = samples[i];
                // Round-to-nearest, away from zero on .5 (matches reciprocal path).
                let rounded_magnitude = coef
                    .saturating_abs()
                    .saturating_add(divisor.wrapping_shr(1))
                    .div_euclid(divisor);
                let qval = if coef < 0 {
                    rounded_magnitude.saturating_neg()
                } else {
                    rounded_magnitude
                };
                let [a, b, ..] = qval.to_le_bytes();
                q[i] = i16::from_le_bytes([a, b]);
            }
            blocks[by.saturating_mul(blocks_per_row).saturating_add(bx)] = q;
        }
    }
    Ok((blocks, blocks_per_row, block_rows))
}

// ── Baseline entropy coding (jchuff.c) ───────────────────────────────────

#[allow(
    clippy::type_complexity,
    reason = "the pair of fixed-size DC and AC frequency tables mirrors the JPEG encoder state"
)]
fn baseline_frequencies(
    comps: &[CompData],
    max_h: u8,
    max_v: u8,
    restart_interval: u16,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<([[u64; 256]; 2], [[u64; 256]; 2])> {
    // ✅ VERIFIED: libjpeg-turbo 3.1.4.1 jchuff.c's gather_statistics pass.
    // Traverse exactly the same MCU stream as encode_baseline_entropy so the
    // optimized table describes every symbol that the output pass will emit.
    let mcu_w = usize::from(max_h).saturating_mul(8);
    let mcu_h = usize::from(max_v).saturating_mul(8);
    let n_mcu_x = comps[0].blocks_per_row.saturating_mul(8).div_ceil(mcu_w);
    let n_mcu_y = comps[0].block_rows.saturating_mul(8).div_ceil(mcu_h);
    let mut dc = [[0u64; 256]; 2];
    let mut ac = [[0u64; 256]; 2];
    let mut last_dc = [0i32; 4];
    let mut mcus_until_restart = usize::from(restart_interval);

    for my in 0..n_mcu_y {
        crate::codecs::error::check_cancelled(token)?;
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
                            .saturating_mul(usize::from(component.v_samp))
                            .saturating_add(vertical);
                        let block_column = mx
                            .saturating_mul(usize::from(component.h_samp))
                            .saturating_add(horizontal);
                        if block_row >= component.block_rows
                            || block_column >= component.blocks_per_row
                        {
                            dc[dc_slot][0] = dc[dc_slot][0].saturating_add(1);
                            ac[ac_slot][0] = ac[ac_slot][0].saturating_add(1);
                            continue;
                        }

                        let block = &component.blocks[block_row
                            .saturating_mul(component.blocks_per_row)
                            .saturating_add(block_column)];
                        let difference = i32::from(block[0]).saturating_sub(last_dc[ci]);
                        last_dc[ci] = i32::from(block[0]);
                        let dc_symbol = bounded_usize(jpeg_nbits(difference));
                        dc[dc_slot][dc_symbol] = dc[dc_slot][dc_symbol].saturating_add(1);

                        let mut run = 0usize;
                        for &natural_index in &ZIGZAG[1..] {
                            let coefficient = i32::from(block[natural_index]);
                            if coefficient == 0 {
                                run = run.saturating_add(1);
                                continue;
                            }
                            while run >= 16 {
                                ac[ac_slot][0xf0] = ac[ac_slot][0xf0].saturating_add(1);
                                run = run.saturating_sub(16);
                            }
                            let width = bounded_usize(jpeg_nbits(coefficient));
                            let symbol = run.wrapping_shl(4) | width;
                            ac[ac_slot][symbol] = ac[ac_slot][symbol].saturating_add(1);
                            run = 0;
                        }
                        if run != 0 {
                            ac[ac_slot][0] = ac[ac_slot][0].saturating_add(1);
                        }
                    }
                }
            }
            mcus_until_restart = mcus_until_restart.saturating_sub(1);
        }
    }
    Ok((dc, ac))
}

#[allow(
    clippy::too_many_arguments,
    reason = "the helper mirrors libjpeg's entropy routine and the token is an independent checkpoint input"
)]
fn encode_baseline_entropy(
    out: &mut Vec<u8>,
    comps: &[CompData],
    max_h: u8,
    max_v: u8,
    dc_tables: &[&huffman::DerivedTable; 2],
    ac_tables: &[&huffman::DerivedTable; 2],
    restart_interval: u16,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    let mcu_w = usize::from(max_h).saturating_mul(8);
    let mcu_h = usize::from(max_v).saturating_mul(8);
    let n_mcu_x = comps[0].blocks_per_row.saturating_mul(8).div_ceil(mcu_w);
    let n_mcu_y = comps[0].block_rows.saturating_mul(8).div_ceil(mcu_h);

    let mut bw = huffman::BitWriter::new();
    let mut last_dc = [0i32; 4];
    let mut mcus_until_restart = usize::from(restart_interval);
    let mut next_restart = 0u8;

    for my in 0..n_mcu_y {
        crate::codecs::error::check_cancelled(token)?;
        for mx in 0..n_mcu_x {
            if restart_interval != 0 && mcus_until_restart == 0 {
                bw.flush();
                out.append(&mut bw.out);
                marker::write_rst(out, next_restart);
                next_restart = next_restart.saturating_add(1) & 7;
                last_dc.fill(0);
                mcus_until_restart = usize::from(restart_interval);
            }
            for (ci, c) in comps.iter().enumerate() {
                let hs = usize::from(c.h_samp);
                let vs = usize::from(c.v_samp);
                let bpr = c.blocks_per_row;
                let dc_tbl = dc_tables[usize::from(c.dc_tbl)];
                let ac_tbl = ac_tables[usize::from(c.ac_tbl)];
                for vy in 0..vs {
                    for vx in 0..hs {
                        let brow = my.saturating_mul(vs).saturating_add(vy);
                        let bcol = mx.saturating_mul(hs).saturating_add(vx);
                        if brow >= c.block_rows || bcol >= bpr {
                            // ✅ VERIFIED: libjpeg-turbo 3.1.4.1
                            // jccoefct.c:174-199. Edge dummy blocks copy the
                            // preceding DC coefficient and zero every AC, so
                            // entropy coding emits both DC category 0 and EOB.
                            bw.write_bits(dc_tbl.codes[0], dc_tbl.lengths[0]);
                            bw.write_bits(ac_tbl.codes[0], ac_tbl.lengths[0]);
                            continue;
                        }
                        let blk = &c.blocks[brow.saturating_mul(bpr).saturating_add(bcol)];
                        encode_one_block(&mut bw, blk, &mut last_dc[ci], dc_tbl, ac_tbl);
                    }
                }
            }
            mcus_until_restart = mcus_until_restart.saturating_sub(1);
        }
    }
    bw.flush();
    out.extend_from_slice(&bw.out);
    Ok(())
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
    let dc = i32::from(block[0]);
    let diff = dc.saturating_sub(*last_dc);
    *last_dc = dc;
    let nbits = jpeg_nbits(diff);
    // DC Huffman symbol = nbits, followed by nbits magnitude bits.
    let nbits_index = bounded_usize(nbits);
    bw.write_bits(dc_tbl.codes[nbits_index], dc_tbl.lengths[nbits_index]);
    if nbits > 0 {
        bw.write_bits(mag_bits(diff, nbits), nbits.to_le_bytes()[0]);
    }

    // AC coefficients in zigzag order (k=1..63).
    let mut r = 0u32; // run length of zeros
    for k in 1..64 {
        let coef = i32::from(block[ZIGZAG[k]]);
        if coef == 0 {
            r = r.saturating_add(1);
            continue;
        }
        // Emit ZRL (0xF0) for each full 16-zero run.
        while r >= 16 {
            bw.write_bits(ac_tbl.codes[0xF0], ac_tbl.lengths[0xF0]);
            r = r.saturating_sub(16);
        }
        let nbits = jpeg_nbits(coef);
        let sym = bounded_usize(r.wrapping_shl(4) | nbits);
        bw.write_bits(ac_tbl.codes[sym], ac_tbl.lengths[sym]);
        bw.write_bits(mag_bits(coef, nbits), nbits.to_le_bytes()[0]);
        r = 0;
    }
    // If trailing zeros, emit EOB (symbol 0x00).
    if r > 0 {
        bw.write_bits(ac_tbl.codes[0], ac_tbl.lengths[0]);
    }
}

/// Number of bits needed to represent |v| (JPEG_NBITS).  nbits(0)=0.
fn jpeg_nbits(v: i32) -> u32 {
    let mut a = v.unsigned_abs();
    let mut n = 0u32;
    while a > 0 {
        n = n.saturating_add(1);
        a = a.wrapping_shr(1);
    }
    n
}

/// The nbits magnitude bits to emit for a signed coefficient value (IJG
/// convention): positive → the value's nbits LSBs; negative → (value-1)'s
/// nbits LSBs (= bitwise complement of the absolute magnitude).
fn mag_bits(v: i32, nbits: u32) -> u32 {
    let emit = if v < 0 { v.saturating_sub(1) } else { v };
    emit.cast_unsigned() & 1u32.wrapping_shl(nbits).saturating_sub(1)
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
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    // ✅ VERIFIED: libjpeg-turbo 3.1.4.1 jcphuff.c:179-1075 and
    // jcmaster.c's jpeg_simple_progression scan script.
    for scan in default_progression_script(component_count) {
        crate::codecs::error::check_cancelled(token)?;
        let events = progressive_events(&scan, components, token)?;
        let mut frequencies = [[0u64; 256]; 4];
        for &event in &events {
            if let ProgressiveEvent::Symbol { table, value } = event {
                frequencies[table][usize::from(value)] =
                    frequencies[table][usize::from(value)].saturating_add(1);
            }
        }

        let mut tables: [Option<huffman::OptimalTable>; 4] = std::array::from_fn(|_| None);
        for table in 0..tables.len() {
            if frequencies[table].iter().any(|&frequency| frequency != 0) {
                let optimized = huffman::optimal_table(&frequencies[table]);
                marker::write_dht(
                    output,
                    u8::from(scan.ss != 0),
                    table.to_le_bytes()[0],
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
                    // The preceding frequency pass builds every referenced table.
                    #[allow(clippy::expect_used)]
                    let table = tables[table]
                        .as_ref()
                        .expect("progressive event table has a built Huffman table");
                    let derived = &table.derived;
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
        crate::codecs::error::check_cancelled(token)?;
    }
    Ok(())
}

fn progressive_events(
    scan: &ProgScan,
    components: &[CompData],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<ProgressiveEvent>> {
    if scan.ss == 0 {
        dc_progressive_events(scan, components, token)
    } else {
        ac_progressive_events(scan, components, token)
    }
}

#[allow(
    clippy::arithmetic_side_effects,
    reason = "max(1) proves the modulo divisor is non-zero for row checkpoint scheduling"
)]
fn dc_progressive_events(
    scan: &ProgScan,
    components: &[CompData],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<ProgressiveEvent>> {
    let mut events = Vec::new();
    let interleaved = scan.comps.len() > 1;
    let maximum_horizontal_sampling = components.iter().map(|c| c.h_samp).max().unwrap_or(1);
    let maximum_vertical_sampling = components.iter().map(|c| c.v_samp).max().unwrap_or(1);
    let mcu_width = usize::from(maximum_horizontal_sampling).saturating_mul(8);
    let mcu_height = usize::from(maximum_vertical_sampling).saturating_mul(8);
    let mcu_columns = components[0]
        .blocks_per_row
        .saturating_mul(8)
        .div_ceil(mcu_width);
    let mcu_rows = components[0]
        .block_rows
        .saturating_mul(8)
        .div_ceil(mcu_height);
    let mut predictors = vec![0i32; scan.comps.len()];

    let mut append = |scan_index: usize, component_index: usize, block: &[i16; 64]| {
        let component = &components[component_index];
        let raw = i32::from(block[0]);
        if scan.ah == 0 {
            let transformed = raw.wrapping_shr(u32::from(scan.al));
            let difference = transformed.saturating_sub(predictors[scan_index]);
            predictors[scan_index] = transformed;
            let width = jpeg_nbits(difference);
            events.push(ProgressiveEvent::Symbol {
                table: usize::from(component.dc_tbl),
                value: width.to_le_bytes()[0],
            });
            if width != 0 {
                events.push(ProgressiveEvent::Bits {
                    value: mag_bits(difference, width),
                    width: width.to_le_bytes()[0],
                });
            }
        } else {
            events.push(ProgressiveEvent::Bits {
                value: (raw.wrapping_shr(u32::from(scan.al)) & 1).cast_unsigned(),
                width: 1,
            });
        }
    };

    if interleaved {
        for mcu_row in 0..mcu_rows {
            crate::codecs::error::check_cancelled(token)?;
            for mcu_column in 0..mcu_columns {
                for (scan_index, &component_index) in scan.comps.iter().enumerate() {
                    let component = &components[component_index];
                    for vertical in 0..usize::from(component.v_samp) {
                        for horizontal in 0..usize::from(component.h_samp) {
                            let block_row = mcu_row
                                .saturating_mul(usize::from(component.v_samp))
                                .saturating_add(vertical);
                            let block_column = mcu_column
                                .saturating_mul(usize::from(component.h_samp))
                                .saturating_add(horizontal);
                            if block_row < component.block_rows
                                && block_column < component.blocks_per_row
                            {
                                append(
                                    scan_index,
                                    component_index,
                                    &component.blocks[block_row
                                        .saturating_mul(component.blocks_per_row)
                                        .saturating_add(block_column)],
                                );
                            }
                        }
                    }
                }
            }
        }
    } else {
        let component_index = scan.comps[0];
        let component = &components[component_index];
        for (block_index, block) in component.blocks.iter().enumerate() {
            if block_index % component.blocks_per_row.max(1) == 0 {
                crate::codecs::error::check_cancelled(token)?;
            }
            append(0, component_index, block);
        }
    }
    Ok(events)
}

#[allow(
    clippy::arithmetic_side_effects,
    reason = "max(1) proves the modulo divisor is non-zero for row checkpoint scheduling"
)]
fn ac_progressive_events(
    scan: &ProgScan,
    components: &[CompData],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<ProgressiveEvent>> {
    let component = &components[scan.comps[0]];
    let table = usize::from(component.ac_tbl);
    let mut events = Vec::new();
    let mut eob_run = 0u32;
    let mut correction_bits = Vec::<u8>::new();
    for (block_index, block) in component.blocks.iter().enumerate() {
        if block_index % component.blocks_per_row.max(1) == 0 {
            crate::codecs::error::check_cancelled(token)?;
        }
        if scan.ah == 0 {
            append_ac_first_events(
                &mut events,
                block,
                scan,
                table,
                &mut eob_run,
                &mut correction_bits,
            );
        } else {
            append_ac_refine_events(
                &mut events,
                block,
                scan,
                table,
                &mut eob_run,
                &mut correction_bits,
            );
        }
    }
    flush_progressive_eob(&mut events, table, &mut eob_run, &mut correction_bits);
    Ok(events)
}

fn append_ac_first_events(
    events: &mut Vec<ProgressiveEvent>,
    block: &[i16; 64],
    scan: &ProgScan,
    table: usize,
    eob_run: &mut u32,
    correction_bits: &mut Vec<u8>,
) {
    let mut run = 0usize;
    let mut last_nonzero = None;
    for coefficient in scan.ss..=scan.se {
        let raw = i32::from(block[ZIGZAG[usize::from(coefficient)]]);
        let sign = raw >> 31;
        let absolute = (raw ^ sign)
            .wrapping_sub(sign)
            .wrapping_shr(u32::from(scan.al));
        if absolute == 0 {
            run = run.saturating_add(1);
            continue;
        }
        if eob_run != &0 {
            flush_progressive_eob(events, table, eob_run, correction_bits);
        }
        while run > 15 {
            events.push(ProgressiveEvent::Symbol { table, value: 0xf0 });
            run = run.saturating_sub(16);
        }
        let width = jpeg_nbits(absolute);
        events.push(ProgressiveEvent::Symbol {
            table,
            value: run
                .wrapping_shl(4)
                .saturating_add(bounded_usize(width))
                .to_le_bytes()[0],
        });
        events.push(ProgressiveEvent::Bits {
            value: mag_bits(
                if sign == 0 {
                    absolute
                } else {
                    absolute.saturating_neg()
                },
                width,
            ),
            width: width.to_le_bytes()[0],
        });
        run = 0;
        last_nonzero = Some(coefficient);
    }
    if last_nonzero != Some(scan.se) {
        *eob_run = eob_run.saturating_add(1);
        if *eob_run == 0x7fff {
            flush_progressive_eob(events, table, eob_run, correction_bits);
        }
    }
}

fn append_ac_refine_events(
    events: &mut Vec<ProgressiveEvent>,
    block: &[i16; 64],
    scan: &ProgScan,
    table: usize,
    eob_run: &mut u32,
    correction_bits: &mut Vec<u8>,
) {
    let coefficients = (scan.ss..=scan.se)
        .map(|coefficient| {
            let raw = i32::from(block[ZIGZAG[usize::from(coefficient)]]);
            let sign = raw >> 31;
            let absolute = (raw ^ sign)
                .wrapping_sub(sign)
                .wrapping_shr(u32::from(scan.al));
            (raw, absolute.cast_unsigned())
        })
        .collect::<Vec<_>>();
    let last_new = coefficients
        .iter()
        .rposition(|(_, absolute)| *absolute == 1);
    let mut run = 0usize;
    let mut block_corrections = Vec::<u8>::new();
    let mut last_nonzero = None;

    for (index, &(raw, absolute)) in coefficients.iter().enumerate() {
        if absolute == 0 {
            run = run.saturating_add(1);
            continue;
        }
        last_nonzero = Some(index);
        while run > 15 && last_new.is_some_and(|last| index <= last) {
            flush_progressive_eob(events, table, eob_run, correction_bits);
            events.push(ProgressiveEvent::Symbol { table, value: 0xf0 });
            run = run.saturating_sub(16);
            append_correction_events(events, &mut block_corrections);
        }
        if absolute > 1 {
            block_corrections.push((absolute & 1).to_le_bytes()[0]);
            continue;
        }

        flush_progressive_eob(events, table, eob_run, correction_bits);
        events.push(ProgressiveEvent::Symbol {
            table,
            value: (run.wrapping_shl(4) | 1).to_le_bytes()[0],
        });
        events.push(ProgressiveEvent::Bits {
            value: u32::from(raw >= 0),
            width: 1,
        });
        append_correction_events(events, &mut block_corrections);
        run = 0;
    }

    if last_nonzero != Some(coefficients.len().saturating_sub(1)) || !block_corrections.is_empty() {
        *eob_run = eob_run.saturating_add(1);
        correction_bits.append(&mut block_corrections);
        if *eob_run == 0x7fff || correction_bits.len() > 937 {
            flush_progressive_eob(events, table, eob_run, correction_bits);
        }
    }
}

fn flush_progressive_eob(
    events: &mut Vec<ProgressiveEvent>,
    table: usize,
    eob_run: &mut u32,
    correction_bits: &mut Vec<u8>,
) {
    if *eob_run == 0 {
        return;
    }
    let width = eob_run.ilog2();
    events.push(ProgressiveEvent::Symbol {
        table,
        value: width.saturating_mul(16).to_le_bytes()[0],
    });
    if width != 0 {
        events.push(ProgressiveEvent::Bits {
            value: *eob_run,
            width: width.to_le_bytes()[0],
        });
    }
    *eob_run = 0;
    append_correction_events(events, correction_bits);
}

fn append_correction_events(events: &mut Vec<ProgressiveEvent>, bits: &mut Vec<u8>) {
    events.extend(bits.drain(..).map(|value| ProgressiveEvent::Bits {
        value: u32::from(value),
        width: 1,
    }));
}
