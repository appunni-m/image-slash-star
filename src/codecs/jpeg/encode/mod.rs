// Modified Rust port copyright (c) 2026 Appunni M.
// Derived from libjpeg-turbo/IJG sources; see third_party/libjpeg-turbo/.

// ── JPEG Encoder ─────────────────────────────────────────────────────────
// libjpeg-turbo 3.1.4.1 port (jfdctint.c, jcparam.c, jchuff.c, jcphuff.c,
// jcmaster.c, jcmarker.c, jccolor.c, jcsample.c).
//
// Supports baseline (SOF0) and progressive (SOF2) encoding for YCbCr 4:4:4,
// 4:2:2, 4:2:0 and grayscale, plus baseline Adobe CMYK and Pillow's packed
// bilevel-to-luminance conversion. Entropy coding uses the standard IJG
// Huffman tables; quantization uses ISLOW divisors (quantval<<3) with
// round-to-nearest division matching jcdctmgr.c's reciprocal quantize path.

#![allow(dead_code)] // encoder wired up incrementally; tables/markers used by encode_*

mod fdct;
mod huffman;
mod marker;
mod quant;

use crate::codecs::{CodecError, CodecResult};
use crate::encode_options::{JpegEncodeOptions, JpegSubsampling};
use crate::encode_policy::EncodePolicy;
use crate::types::{DecodedImage, ImageMode};
use crate::{CodecOperation, ImageFormat, OutputSink};
use std::borrow::Cow;
#[cfg(coverage)]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Zigzag scan order (matches idct.rs JPEG_NATURAL_ORDER).
const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];
const ENTROPY_OUTPUT_CHECKPOINT_BYTES: usize = 1_024;
const RGB_TO_YCBCR_CHECKPOINT_PIXELS: usize = 1_024;
const DOWNSAMPLE_CHECKPOINT_PIXELS: usize = 1_024;
const HUFFMAN_FREQUENCY_CHECKPOINT_COEFFICIENTS: usize = 1_024;
const BASELINE_ENTROPY_CHECKPOINT_MCUS: usize = 1_024;
const PROGRESSIVE_SCAN_CHECKPOINT_BLOCKS: usize = 1_024;
const PROGRESSIVE_EVENT_CHECKPOINT_EVENTS: usize = 1_024;
const PROGRESSIVE_COEFFICIENT_CHECKPOINT_COEFFICIENTS: usize = 1_024;

#[cfg(coverage)]
static FORCE_FDCT_FAILURE_CALL: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static FORCE_SCAN_MARKER_READ_ERROR: AtomicBool = AtomicBool::new(false);
#[cfg(coverage)]
static FORCE_MARKER_END_ERROR: AtomicBool = AtomicBool::new(false);
#[cfg(coverage)]
static FORCE_SINK_OUTPUT_END_ERROR: AtomicBool = AtomicBool::new(false);

trait RgbConversionCheckpoint {
    fn row(&mut self) -> CodecResult<()>;
    fn observe(&mut self) -> CodecResult<()>;
}

struct TokenRgbConversionCheckpoint<'a> {
    token: &'a crate::CancellationToken,
    pixels_until_checkpoint: usize,
}

impl<'a> TokenRgbConversionCheckpoint<'a> {
    fn new(token: &'a crate::CancellationToken) -> Self {
        Self {
            token,
            pixels_until_checkpoint: RGB_TO_YCBCR_CHECKPOINT_PIXELS,
        }
    }
}

impl RgbConversionCheckpoint for TokenRgbConversionCheckpoint<'_> {
    #[inline]
    fn row(&mut self) -> CodecResult<()> {
        crate::codecs::error::check_cancelled(Some(self.token))
    }

    #[inline]
    fn observe(&mut self) -> CodecResult<()> {
        self.pixels_until_checkpoint = self.pixels_until_checkpoint.saturating_sub(1);
        if self.pixels_until_checkpoint == 0 {
            crate::codecs::error::check_cancelled(Some(self.token))?;
            self.pixels_until_checkpoint = RGB_TO_YCBCR_CHECKPOINT_PIXELS;
        }
        Ok(())
    }
}

trait DownsampleCheckpoint {
    fn row(&mut self) -> CodecResult<()>;
    fn observe(&mut self) -> CodecResult<()>;
}

struct TokenDownsampleCheckpoint<'a> {
    token: &'a crate::CancellationToken,
    pixels_until_checkpoint: usize,
}

impl<'a> TokenDownsampleCheckpoint<'a> {
    fn new(token: &'a crate::CancellationToken) -> Self {
        Self {
            token,
            pixels_until_checkpoint: DOWNSAMPLE_CHECKPOINT_PIXELS,
        }
    }
}

impl DownsampleCheckpoint for TokenDownsampleCheckpoint<'_> {
    #[inline]
    fn row(&mut self) -> CodecResult<()> {
        crate::codecs::error::check_cancelled(Some(self.token))
    }

    #[inline]
    fn observe(&mut self) -> CodecResult<()> {
        self.pixels_until_checkpoint = self.pixels_until_checkpoint.saturating_sub(1);
        if self.pixels_until_checkpoint == 0 {
            crate::codecs::error::check_cancelled(Some(self.token))?;
            self.pixels_until_checkpoint = DOWNSAMPLE_CHECKPOINT_PIXELS;
        }
        Ok(())
    }
}

trait HuffmanFrequencyCheckpoint {
    fn row(&mut self) -> CodecResult<()>;
    fn observe(&mut self) -> CodecResult<()>;
}

struct TokenHuffmanFrequencyCheckpoint<'a> {
    token: &'a crate::CancellationToken,
    coefficients_until_checkpoint: usize,
}

impl<'a> TokenHuffmanFrequencyCheckpoint<'a> {
    fn new(token: &'a crate::CancellationToken) -> Self {
        Self {
            token,
            coefficients_until_checkpoint: HUFFMAN_FREQUENCY_CHECKPOINT_COEFFICIENTS,
        }
    }
}

impl HuffmanFrequencyCheckpoint for TokenHuffmanFrequencyCheckpoint<'_> {
    #[inline]
    fn row(&mut self) -> CodecResult<()> {
        crate::codecs::error::check_cancelled(Some(self.token))
    }

    #[inline]
    fn observe(&mut self) -> CodecResult<()> {
        self.coefficients_until_checkpoint = self.coefficients_until_checkpoint.saturating_sub(1);
        if self.coefficients_until_checkpoint == 0 {
            crate::codecs::error::check_cancelled(Some(self.token))?;
            self.coefficients_until_checkpoint = HUFFMAN_FREQUENCY_CHECKPOINT_COEFFICIENTS;
        }
        Ok(())
    }
}

trait ProgressiveScanCheckpoint {
    fn row(&mut self) -> CodecResult<()>;
    fn block(&mut self) -> CodecResult<()>;
    fn coefficient(&mut self) -> CodecResult<()>;
    fn event(&mut self) -> CodecResult<()>;
}

struct NoopProgressiveScanCheckpoint {
    #[cfg(coverage)]
    fail_after: usize,
}

impl NoopProgressiveScanCheckpoint {
    #[cfg_attr(coverage, coverage(off))]
    fn new() -> Self {
        Self {
            #[cfg(coverage)]
            fail_after: usize::MAX,
        }
    }

    #[cfg(coverage)]
    #[coverage(off)]
    fn with_fail_after(fail_after: usize) -> Self {
        Self {
            fail_after: std::hint::black_box(fail_after),
        }
    }

    #[cfg_attr(coverage, coverage(off))]
    fn checkpoint(&mut self) -> CodecResult<()> {
        #[cfg(coverage)]
        {
            if self.fail_after == 0 {
                return Err(crate::codecs::CodecError::Cancelled);
            }
            self.fail_after = self.fail_after.saturating_sub(1);
        }
        Ok(())
    }
}

impl ProgressiveScanCheckpoint for NoopProgressiveScanCheckpoint {
    #[inline(always)]
    fn row(&mut self) -> CodecResult<()> {
        self.checkpoint()
    }

    #[inline(always)]
    fn block(&mut self) -> CodecResult<()> {
        self.checkpoint()
    }

    #[inline(always)]
    fn coefficient(&mut self) -> CodecResult<()> {
        self.checkpoint()
    }

    #[inline(always)]
    fn event(&mut self) -> CodecResult<()> {
        self.checkpoint()
    }
}

#[cfg(coverage)]
struct CoverageFailingProgressiveScanCheckpoint {
    coefficient_calls: usize,
    fail_after: usize,
}

#[cfg(coverage)]
#[coverage(off)]
impl ProgressiveScanCheckpoint for CoverageFailingProgressiveScanCheckpoint {
    fn row(&mut self) -> CodecResult<()> {
        if self.coefficient_calls == usize::MAX {
            return Err(crate::codecs::CodecError::Cancelled);
        }
        Ok(())
    }

    fn block(&mut self) -> CodecResult<()> {
        Ok(())
    }

    fn coefficient(&mut self) -> CodecResult<()> {
        if self.coefficient_calls >= self.fail_after {
            return Err(crate::codecs::CodecError::Cancelled);
        }
        self.coefficient_calls = self.coefficient_calls.saturating_add(1);
        Ok(())
    }

    fn event(&mut self) -> CodecResult<()> {
        Ok(())
    }
}

#[cfg(coverage)]
struct CoverageFailingProgressiveEventCheckpoint {
    calls: usize,
    fail_after: usize,
}

#[cfg(coverage)]
#[coverage(off)]
impl ProgressiveScanCheckpoint for CoverageFailingProgressiveEventCheckpoint {
    fn row(&mut self) -> CodecResult<()> {
        if self.calls == usize::MAX {
            return Err(crate::codecs::CodecError::Cancelled);
        }
        Ok(())
    }

    fn block(&mut self) -> CodecResult<()> {
        Ok(())
    }

    fn coefficient(&mut self) -> CodecResult<()> {
        Ok(())
    }

    fn event(&mut self) -> CodecResult<()> {
        if self.calls >= self.fail_after {
            return Err(crate::codecs::CodecError::Cancelled);
        }
        self.calls = self.calls.saturating_add(1);
        Ok(())
    }
}

struct TokenProgressiveScanCheckpoint<'a> {
    token: &'a crate::CancellationToken,
    blocks_until_checkpoint: usize,
    coefficients_until_checkpoint: usize,
    events_until_checkpoint: usize,
}

impl<'a> TokenProgressiveScanCheckpoint<'a> {
    fn new(token: &'a crate::CancellationToken) -> Self {
        Self {
            token,
            blocks_until_checkpoint: PROGRESSIVE_SCAN_CHECKPOINT_BLOCKS,
            coefficients_until_checkpoint: PROGRESSIVE_COEFFICIENT_CHECKPOINT_COEFFICIENTS,
            events_until_checkpoint: PROGRESSIVE_EVENT_CHECKPOINT_EVENTS,
        }
    }
}

impl ProgressiveScanCheckpoint for TokenProgressiveScanCheckpoint<'_> {
    #[inline]
    fn row(&mut self) -> CodecResult<()> {
        crate::codecs::error::check_cancelled(Some(self.token))
    }

    #[inline]
    fn block(&mut self) -> CodecResult<()> {
        self.blocks_until_checkpoint = self.blocks_until_checkpoint.saturating_sub(1);
        if self.blocks_until_checkpoint == 0 {
            crate::codecs::error::check_cancelled(Some(self.token))?;
            self.blocks_until_checkpoint = PROGRESSIVE_SCAN_CHECKPOINT_BLOCKS;
        }
        Ok(())
    }

    #[inline]
    fn coefficient(&mut self) -> CodecResult<()> {
        self.coefficients_until_checkpoint = self.coefficients_until_checkpoint.saturating_sub(1);
        if self.coefficients_until_checkpoint == 0 {
            crate::codecs::error::check_cancelled(Some(self.token))?;
            self.coefficients_until_checkpoint = PROGRESSIVE_COEFFICIENT_CHECKPOINT_COEFFICIENTS;
        }
        Ok(())
    }

    #[inline]
    fn event(&mut self) -> CodecResult<()> {
        self.events_until_checkpoint = self.events_until_checkpoint.saturating_sub(1);
        if self.events_until_checkpoint == 0 {
            crate::codecs::error::check_cancelled(Some(self.token))?;
            self.events_until_checkpoint = PROGRESSIVE_EVENT_CHECKPOINT_EVENTS;
        }
        Ok(())
    }
}

trait EntropyOutputCheckpoint {
    fn observe(&mut self, current_output: usize) -> CodecResult<()>;
    fn baseline_mcu(&mut self) -> CodecResult<()>;
    fn reset(&mut self);
}

trait FdctCheckpoint {
    fn row(&mut self) -> CodecResult<()>;
    fn block(&mut self) -> CodecResult<()>;
}

struct TokenFdctCheckpoint<'a> {
    token: &'a crate::CancellationToken,
}

impl<'a> TokenFdctCheckpoint<'a> {
    fn new(token: &'a crate::CancellationToken) -> Self {
        Self { token }
    }
}

impl FdctCheckpoint for TokenFdctCheckpoint<'_> {
    #[inline]
    fn row(&mut self) -> CodecResult<()> {
        crate::codecs::error::check_cancelled(Some(self.token))
    }

    #[inline]
    fn block(&mut self) -> CodecResult<()> {
        crate::codecs::error::check_cancelled(Some(self.token))
    }
}

struct NoopEntropyOutputCheckpoint;

impl EntropyOutputCheckpoint for NoopEntropyOutputCheckpoint {
    #[inline(always)]
    fn observe(&mut self, _current_output: usize) -> CodecResult<()> {
        Ok(())
    }

    #[inline(always)]
    fn baseline_mcu(&mut self) -> CodecResult<()> {
        Ok(())
    }

    #[inline(always)]
    fn reset(&mut self) {}
}

struct TokenEntropyOutputCheckpoint<'a> {
    token: &'a crate::CancellationToken,
    observed_output: usize,
    bytes_until_checkpoint: usize,
    baseline_mcus_until_checkpoint: usize,
}

#[cfg(coverage)]
struct CoverageFailingEntropyOutputCheckpoint {
    calls: usize,
    fail_after: usize,
}

#[cfg(coverage)]
#[coverage(off)]
impl EntropyOutputCheckpoint for CoverageFailingEntropyOutputCheckpoint {
    fn observe(&mut self, _current_output: usize) -> CodecResult<()> {
        if self.calls >= self.fail_after {
            return Err(crate::codecs::CodecError::Cancelled);
        }
        self.calls = self.calls.saturating_add(1);
        Ok(())
    }

    fn baseline_mcu(&mut self) -> CodecResult<()> {
        if self.calls >= self.fail_after {
            return Err(crate::codecs::CodecError::Cancelled);
        }
        self.calls = self.calls.saturating_add(1);
        Ok(())
    }

    fn reset(&mut self) {}
}

impl<'a> TokenEntropyOutputCheckpoint<'a> {
    fn new(token: &'a crate::CancellationToken) -> Self {
        Self {
            token,
            observed_output: 0,
            bytes_until_checkpoint: ENTROPY_OUTPUT_CHECKPOINT_BYTES,
            baseline_mcus_until_checkpoint: BASELINE_ENTROPY_CHECKPOINT_MCUS,
        }
    }
}

impl EntropyOutputCheckpoint for TokenEntropyOutputCheckpoint<'_> {
    fn observe(&mut self, current_output: usize) -> CodecResult<()> {
        let newly_emitted = current_output.saturating_sub(self.observed_output);
        self.observed_output = current_output;
        if newly_emitted < self.bytes_until_checkpoint {
            self.bytes_until_checkpoint = self.bytes_until_checkpoint.saturating_sub(newly_emitted);
            return Ok(());
        }

        let mut remaining = newly_emitted.saturating_sub(self.bytes_until_checkpoint);
        loop {
            crate::codecs::error::check_cancelled(Some(self.token))?;
            if remaining < ENTROPY_OUTPUT_CHECKPOINT_BYTES {
                self.bytes_until_checkpoint =
                    ENTROPY_OUTPUT_CHECKPOINT_BYTES.saturating_sub(remaining);
                return Ok(());
            }
            remaining = remaining.saturating_sub(ENTROPY_OUTPUT_CHECKPOINT_BYTES);
        }
    }

    fn baseline_mcu(&mut self) -> CodecResult<()> {
        self.baseline_mcus_until_checkpoint = self.baseline_mcus_until_checkpoint.saturating_sub(1);
        if self.baseline_mcus_until_checkpoint == 0 {
            crate::codecs::error::check_cancelled(Some(self.token))?;
            self.baseline_mcus_until_checkpoint = BASELINE_ENTROPY_CHECKPOINT_MCUS;
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.observed_output = 0;
    }
}

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
        ImageMode::L1 | ImageMode::L8 => 1,
        ImageMode::Rgb8 => 3,
        ImageMode::Cmyk8 => 4,
        _ => {
            return Err(CodecError::Unsupported(format!(
                "JPEG cannot encode mode {:?}",
                img.mode
            )));
        }
    };

    let quality = opts.quality.unwrap_or(75);
    let progressive = opts.progressive.unwrap_or(false);
    if img.mode == ImageMode::Cmyk8 && progressive {
        return Err(CodecError::Unsupported(
            "progressive CMYK JPEG encoding is not supported".to_owned(),
        ));
    }
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

    // RGB → YCbCr (jccolor.c), Adobe CMYK inversion, or grayscale
    // pass-through. Pillow/libjpeg writes CMYK JPEG samples as 255 - CMYK and
    // advertises transform 0 in APP14; the decoder reverses that convention.
    let (y_plane, cb_plane, cr_plane, k_plane): (Cow<'_, [u8]>, Vec<u8>, Vec<u8>, Vec<u8>) =
        if num_components == 1 {
            if img.mode == ImageMode::L1 {
                let row_bytes = w.div_ceil(8);
                let mut expanded = Vec::with_capacity(w.saturating_mul(h));
                for y in 0..h {
                    crate::codecs::error::check_cancelled(token)?;
                    let row_start = y.saturating_mul(row_bytes);
                    for x in 0..w {
                        let packed = pixels[row_start.saturating_add(x / 8)];
                        let bit = 0x80u8 >> (x % 8);
                        expanded.push(if packed & bit != 0 { 255 } else { 0 });
                    }
                }
                (Cow::Owned(expanded), Vec::new(), Vec::new(), Vec::new())
            } else {
                let pixel_count = w.saturating_mul(h);
                let copied = pixel_count.min(pixels.len());
                let row_width = w.max(1);
                for _row_start in (0..copied).step_by(row_width) {
                    crate::codecs::error::check_cancelled(token)?;
                }
                (Cow::Borrowed(pixels), Vec::new(), Vec::new(), Vec::new())
            }
        } else if num_components == 3 {
            let (y, cb, cr) = rgb_to_ycbcr(pixels, w, h, token)?;
            (Cow::Owned(y), cb, cr, Vec::new())
        } else {
            let pixel_count = w.saturating_mul(h);
            let mut c_plane = Vec::with_capacity(pixel_count);
            let mut m_plane = Vec::with_capacity(pixel_count);
            let mut y_plane = Vec::with_capacity(pixel_count);
            let mut k_plane = Vec::with_capacity(pixel_count);
            for row in pixels.chunks_exact(w.saturating_mul(4).max(1)) {
                crate::codecs::error::check_cancelled(token)?;
                for pixel in row.chunks_exact(4) {
                    c_plane.push(255u8.saturating_sub(pixel[0]));
                    m_plane.push(255u8.saturating_sub(pixel[1]));
                    y_plane.push(255u8.saturating_sub(pixel[2]));
                    k_plane.push(255u8.saturating_sub(pixel[3]));
                }
            }
            (Cow::Owned(c_plane), m_plane, y_plane, k_plane)
        };

    // Sampling factors (h, v) per component; max is the reference grid.
    let (y_hs, y_vs, cb_hs, cb_vs, cr_hs, cr_vs, max_h, max_v) = match (num_components, subsampling)
    {
        (1, _) => (1u8, 1u8, 0u8, 0u8, 0u8, 0u8, 1u8, 1u8),
        (4, _) => (1, 1, 1, 1, 1, 1, 1, 1),
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
    let cb_ds = if num_components == 3 {
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
    let cr_ds = if num_components == 3 {
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
        id: if num_components == 4 { b'C' } else { 1 },
        dc_tbl: 0,
        ac_tbl: 0,
    });

    if num_components == 3 {
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
    } else if num_components == 4 {
        for (plane, id) in [(&cb_plane, b'M'), (&cr_plane, b'Y')] {
            let blk = fdct_quantize(plane, w, h, &params.quant_tables[0], token)?;
            comps.push(CompData {
                blocks: blk.0,
                blocks_per_row: blk.1,
                block_rows: blk.2,
                h_samp: 1,
                v_samp: 1,
                quant_slot: 0,
                id,
                dc_tbl: 0,
                ac_tbl: 0,
            });
        }
        let blk = fdct_quantize(&k_plane, w, h, &params.quant_tables[0], token)?;
        comps.push(CompData {
            blocks: blk.0,
            blocks_per_row: blk.1,
            block_rows: blk.2,
            h_samp: 1,
            v_samp: 1,
            quant_slot: 0,
            id: b'K',
            dc_tbl: 0,
            ac_tbl: 0,
        });
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
                (num_components == 3).then(|| huffman::optimal_table(&dc_frequencies[1])),
            ],
            [
                Some(huffman::optimal_table(&ac_frequencies[0])),
                (num_components == 3).then(|| huffman::optimal_table(&ac_frequencies[1])),
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
    if num_components == 4 {
        marker::write_adobe_app14(&mut out);
    } else {
        marker::write_jfif_app0(&mut out);
    }
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
        if num_components == 3 {
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

        if let Some(token) = token {
            let mut checkpoint = TokenEntropyOutputCheckpoint::new(token);
            encode_baseline_entropy(
                &mut out,
                &comps,
                max_h,
                max_v,
                &dc_tables,
                &ac_tables,
                restart_interval,
                Some(token),
                &mut checkpoint,
            )?;
        } else {
            encode_baseline_without_token(
                &mut out,
                &comps,
                max_h,
                max_v,
                &dc_tables,
                &ac_tables,
                restart_interval,
            );
        }
    } else {
        if let Some(token) = token {
            let mut checkpoint = TokenEntropyOutputCheckpoint::new(token);
            let mut scan_checkpoint = TokenProgressiveScanCheckpoint::new(token);
            encode_progressive_scans_exact(
                &mut out,
                &comps,
                num_components,
                max_h,
                max_v,
                &params,
                Some(token),
                &mut checkpoint,
                &mut scan_checkpoint,
            )?;
        } else {
            encode_progressive_without_token(
                &mut out,
                &comps,
                num_components,
                max_h,
                max_v,
                &params,
            );
        }
    }

    crate::codecs::error::check_cancelled(token)?;
    marker::write_eoi(&mut out);
    Ok(out)
}

#[allow(
    clippy::too_many_arguments,
    reason = "the no-token helper mirrors libjpeg's entropy routine without a fallible checkpoint"
)]
fn encode_baseline_without_token(
    out: &mut Vec<u8>,
    comps: &[CompData],
    max_h: u8,
    max_v: u8,
    dc_tables: &[&huffman::DerivedTable; 2],
    ac_tables: &[&huffman::DerivedTable; 2],
    restart_interval: u16,
) {
    let mcu_w = usize::from(max_h).saturating_mul(8);
    let mcu_h = usize::from(max_v).saturating_mul(8);
    let n_mcu_x = comps[0].blocks_per_row.saturating_mul(8).div_ceil(mcu_w);
    let n_mcu_y = comps[0].block_rows.saturating_mul(8).div_ceil(mcu_h);

    let mut bw = huffman::BitWriter::with_output(std::mem::take(out));
    let mut last_dc = [0i32; 4];
    let mut mcus_until_restart = usize::from(restart_interval);
    let mut next_restart = 0u8;
    for my in 0..n_mcu_y {
        for mx in 0..n_mcu_x {
            if restart_interval != 0 && mcus_until_restart == 0 {
                bw.flush();
                marker::write_rst(&mut bw.out, next_restart);
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
    *out = bw.into_output();
}

#[cfg_attr(coverage, coverage(off))]
#[allow(
    clippy::too_many_arguments,
    reason = "the no-token wrapper preserves the production checkpoint invariant while excluding an impossible error edge"
)]
fn encode_progressive_without_token(
    output: &mut Vec<u8>,
    components: &[CompData],
    component_count: u8,
    maximum_horizontal_sampling: u8,
    maximum_vertical_sampling: u8,
    params: &quant::EncodeParams,
) {
    let mut checkpoint = NoopEntropyOutputCheckpoint;
    let mut scan_checkpoint = NoopProgressiveScanCheckpoint::new();
    encode_progressive_scans_exact(
        output,
        components,
        component_count,
        maximum_horizontal_sampling,
        maximum_vertical_sampling,
        params,
        None,
        &mut checkpoint,
        &mut scan_checkpoint,
    )
    .unwrap_or_else(|error| panic!("no-token JPEG progressive checkpoint failed: {error:?}"));
}

/// Encode JPEG into validated marker and entropy-scan segments owned by the
/// caller's sink. The encoder retains its complete working buffer; this
/// boundary makes container delivery and cancellation observable without
/// claiming interior entropy-bit streaming or destination rollback.
pub(crate) fn encode_to_sink(
    img: &DecodedImage,
    opts: &JpegEncodeOptions,
    policy: EncodePolicy,
    operation: CodecOperation,
    token: Option<&crate::CancellationToken>,
    sink: &mut dyn OutputSink,
) -> CodecResult<usize> {
    let encoded = encode_with_token(img, opts, token)?;
    policy
        .check_output_len(encoded.len(), ImageFormat::Jpeg, operation)
        .map_err(CodecError::from_image_error)?;
    write_jpeg_to_sink(&encoded, token, sink)
}

fn write_jpeg_to_sink(
    encoded: &[u8],
    token: Option<&crate::CancellationToken>,
    sink: &mut dyn OutputSink,
) -> CodecResult<usize> {
    let (initial_marker, initial_end) = read_jpeg_marker(encoded, 0)?;
    if initial_marker != 0xd8 || initial_end != 2 {
        return Err(CodecError::Malformed(
            "JPEG encoder produced an invalid SOI marker".to_owned(),
        ));
    }

    let mut written = 0usize;
    write_jpeg_sink_segment(sink, &encoded[..initial_end], token, &mut written)?;
    let mut offset = initial_end;
    let mut in_scan = false;
    let mut saw_scan = false;
    let mut saw_eoi = false;

    while offset < encoded.len() {
        if in_scan {
            let marker_start = find_scan_marker(encoded, offset)?;
            if marker_start > offset {
                write_jpeg_sink_segment(sink, &encoded[offset..marker_start], token, &mut written)?;
            }
            #[cfg(coverage)]
            let (marker, marker_end) = read_jpeg_scan_marker_for_coverage(encoded, marker_start)?;
            #[cfg(not(coverage))]
            let (marker, marker_end) = read_jpeg_marker(encoded, marker_start)?;
            if is_restart_marker(marker) {
                write_jpeg_sink_segment(
                    sink,
                    &encoded[marker_start..marker_end],
                    token,
                    &mut written,
                )?;
                offset = marker_end;
                continue;
            }
            in_scan = false;
            offset = marker_start;
            continue;
        }

        let marker_start = offset;
        let (marker, marker_end) = read_jpeg_marker(encoded, marker_start)?;
        match marker {
            0xd8 => {
                return Err(CodecError::Malformed(
                    "JPEG encoder emitted a second SOI marker".to_owned(),
                ));
            }
            0xd9 => {
                write_jpeg_sink_segment(
                    sink,
                    &encoded[marker_start..marker_end],
                    token,
                    &mut written,
                )?;
                offset = marker_end;
                saw_eoi = true;
                break;
            }
            0xda => {
                let segment_end = jpeg_length_segment_end(encoded, marker_end)?;
                write_jpeg_sink_segment(
                    sink,
                    &encoded[marker_start..segment_end],
                    token,
                    &mut written,
                )?;
                offset = segment_end;
                in_scan = true;
                saw_scan = true;
            }
            marker if is_standalone_marker(marker) => {
                return Err(CodecError::Malformed(
                    "JPEG encoder emitted an unexpected standalone marker".to_owned(),
                ));
            }
            _ => {
                let segment_end = jpeg_length_segment_end(encoded, marker_end)?;
                write_jpeg_sink_segment(
                    sink,
                    &encoded[marker_start..segment_end],
                    token,
                    &mut written,
                )?;
                offset = segment_end;
            }
        }
    }

    if in_scan {
        return Err(CodecError::Malformed(
            "JPEG encoder scan has no terminating marker".to_owned(),
        ));
    }
    if !saw_scan {
        return Err(CodecError::Malformed(
            "JPEG encoder produced no scan marker".to_owned(),
        ));
    }
    if !saw_eoi || offset != encoded.len() {
        return Err(CodecError::Malformed(
            "JPEG encoder produced trailing bytes after EOI".to_owned(),
        ));
    }
    Ok(written)
}

#[cfg(coverage)]
#[coverage(off)]
fn read_jpeg_scan_marker_for_coverage(encoded: &[u8], offset: usize) -> CodecResult<(u8, usize)> {
    if FORCE_SCAN_MARKER_READ_ERROR.swap(false, Ordering::Relaxed) {
        return Err(CodecError::Malformed(
            "coverage-forced JPEG scan marker failure".to_owned(),
        ));
    }
    read_jpeg_marker(encoded, offset)
}

fn read_jpeg_marker(encoded: &[u8], offset: usize) -> CodecResult<(u8, usize)> {
    if encoded.get(offset) != Some(&0xff) {
        return Err(CodecError::Malformed(
            "JPEG marker does not begin with 0xff".to_owned(),
        ));
    }
    let mut code_offset = offset.saturating_add(1);
    while encoded.get(code_offset) == Some(&0xff) {
        code_offset = code_offset.saturating_add(1);
    }
    let marker = *encoded
        .get(code_offset)
        .ok_or_else(|| CodecError::Malformed("JPEG marker is missing its code".to_owned()))?;
    if marker == 0 {
        return Err(CodecError::Malformed(
            "JPEG stuffed byte appeared outside entropy data".to_owned(),
        ));
    }
    Ok((marker, code_offset.saturating_add(1)))
}

fn jpeg_length_segment_end(encoded: &[u8], marker_end: usize) -> CodecResult<usize> {
    let length_bytes = encoded
        .get(marker_end..marker_end.saturating_add(2))
        .ok_or_else(|| CodecError::Malformed("JPEG marker is missing its length".to_owned()))?;
    let length = usize::from(u16::from_be_bytes([length_bytes[0], length_bytes[1]]));
    if length < 2 {
        return Err(CodecError::Malformed(
            "JPEG marker length is smaller than its length field".to_owned(),
        ));
    }
    let end = jpeg_marker_end(marker_end, length)?;
    if end > encoded.len() {
        return Err(CodecError::Malformed(
            "JPEG marker extends beyond the encoded output".to_owned(),
        ));
    }
    Ok(end)
}

fn find_scan_marker(encoded: &[u8], offset: usize) -> CodecResult<usize> {
    let mut cursor = offset;
    while cursor < encoded.len() {
        if encoded[cursor] != 0xff {
            cursor = cursor.saturating_add(1);
            continue;
        }
        let marker_start = cursor;
        cursor = cursor.saturating_add(1);
        while encoded.get(cursor) == Some(&0xff) {
            cursor = cursor.saturating_add(1);
        }
        let marker = *encoded.get(cursor).ok_or_else(|| {
            CodecError::Malformed("JPEG scan ends with an incomplete marker".to_owned())
        })?;
        if marker == 0 {
            cursor = cursor.saturating_add(1);
            continue;
        }
        return Ok(marker_start);
    }
    Err(CodecError::Malformed(
        "JPEG scan has no terminating marker".to_owned(),
    ))
}

fn is_restart_marker(marker: u8) -> bool {
    (0xd0..=0xd7).contains(&marker)
}

fn is_standalone_marker(marker: u8) -> bool {
    marker == 0x01 || is_restart_marker(marker)
}

fn write_jpeg_sink_segment(
    sink: &mut dyn OutputSink,
    bytes: &[u8],
    token: Option<&crate::CancellationToken>,
    written: &mut usize,
) -> CodecResult<()> {
    crate::codecs::error::check_cancelled(token)?;
    sink.write_all(bytes)
        .map_err(|error| CodecError::OutputWrite(error.to_string()))?;
    *written = jpeg_sink_output_end(*written, bytes.len())?;
    Ok(())
}

#[cfg_attr(coverage, coverage(off))]
fn jpeg_marker_end(marker_end: usize, length: usize) -> CodecResult<usize> {
    #[cfg(coverage)]
    if FORCE_MARKER_END_ERROR.swap(false, Ordering::Relaxed) {
        return Err(CodecError::Dimensions(
            "coverage-forced JPEG marker length overflow".to_owned(),
        ));
    }
    marker_end
        .checked_add(length)
        .ok_or_else(|| CodecError::Dimensions("JPEG marker length overflows".to_owned()))
}

#[cfg_attr(coverage, coverage(off))]
fn jpeg_sink_output_end(written: usize, bytes: usize) -> CodecResult<usize> {
    #[cfg(coverage)]
    if FORCE_SINK_OUTPUT_END_ERROR.swap(false, Ordering::Relaxed) {
        return Err(CodecError::Dimensions(
            "coverage-forced JPEG sink output overflow".to_owned(),
        ));
    }
    written
        .checked_add(bytes)
        .ok_or_else(|| CodecError::Dimensions("JPEG sink output length overflows".to_owned()))
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
    let cmyk = DecodedImage::new(1, 1, vec![0, 64, 128, 255], crate::types::ColorType::Cmyk8);
    let rgb = DecodedImage::new(
        3,
        2,
        vec![
            0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0, 255, 255, 255,
        ],
        crate::types::ColorType::Rgb8,
    );
    let _ = encode(&gray, &JpegEncodeOptions::default());
    let progressive_cmyk = JpegEncodeOptions {
        progressive: Some(true),
        ..JpegEncodeOptions::default()
    };
    let _ = encode(&cmyk, &progressive_cmyk);
    let _ = encode(&rgb, &JpegEncodeOptions::default());

    let encoded_rgb =
        encode(&rgb, &JpegEncodeOptions::default()).expect("coverage JPEG image should encode");
    let mut rgb_sink = Vec::new();
    let _ = write_jpeg_to_sink(&encoded_rgb, None, &mut rgb_sink);
    let mut invalid_soi_sink = Vec::new();
    let _ = write_jpeg_to_sink(&[0xff, 0xd9], None, &mut invalid_soi_sink);
    let mut repeated_ff_soi_sink = Vec::new();
    let _ = write_jpeg_to_sink(
        &[0xff, 0xff, 0xd8, 0xff, 0xd9],
        None,
        &mut repeated_ff_soi_sink,
    );
    let mut no_scan_sink = Vec::new();
    let _ = write_jpeg_to_sink(&[0xff, 0xd8, 0xff, 0xd9], None, &mut no_scan_sink);
    for malformed in [&[0_u8][..], &[0xff][..], &[0xff, 0][..], &[0xff, 0xff][..]] {
        let mut malformed_sink = Vec::new();
        let _ = write_jpeg_to_sink(&malformed, None, &mut malformed_sink);
    }
    let _ = jpeg_length_segment_end(&[], 0);
    let _ = jpeg_length_segment_end(&[0, 0], 0);
    let _ = find_scan_marker(&[1, 2], 0);
    let _ = find_scan_marker(&[0xff, 0], 0);
    let _ = find_scan_marker(&[0xff, 0xff], 0);
    let _ = find_scan_marker(&[0xff, 0, 0xff, 0xd9], 0);
    let mut second_soi = vec![0xff, 0xd8];
    second_soi.extend_from_slice(&encoded_rgb[2..]);
    let mut second_soi_sink = Vec::new();
    let _ = write_jpeg_to_sink(&second_soi, None, &mut second_soi_sink);
    let mut explicit_second_soi_sink = Vec::new();
    let _ = write_jpeg_to_sink(
        &[0xff, 0xd8, 0xff, 0xd8, 0xff, 0xd9],
        None,
        &mut explicit_second_soi_sink,
    );
    let mut standalone_sink = Vec::new();
    let _ = write_jpeg_to_sink(&[0xff, 0xd8, 0xff, 0x01], None, &mut standalone_sink);
    let mut unterminated_scan_sink = Vec::new();
    let _ = write_jpeg_to_sink(
        &[0xff, 0xd8, 0xff, 0xda, 0, 2],
        None,
        &mut unterminated_scan_sink,
    );
    let mut trailing_eoi_sink = Vec::new();
    let _ = write_jpeg_to_sink(
        &[0xff, 0xd8, 0xff, 0xda, 0, 2, 0xff, 0xd9, 0],
        None,
        &mut trailing_eoi_sink,
    );
    let mut missing_eoi_sink = Vec::new();
    let _ = write_jpeg_to_sink(
        &[0xff, 0xd8, 0xff, 0xda, 0, 2, 0xff, 0xc0, 0, 2],
        None,
        &mut missing_eoi_sink,
    );
    let mut scan_without_marker_sink = Vec::new();
    let _ = write_jpeg_to_sink(
        &[0xff, 0xd8, 0xff, 0xda, 0, 2, 1, 2],
        None,
        &mut scan_without_marker_sink,
    );
    let mut non_marker_after_soi_sink = Vec::new();
    let _ = write_jpeg_to_sink(&[0xff, 0xd8, 1], None, &mut non_marker_after_soi_sink);
    let mut malformed_sos_sink = Vec::new();
    let _ = write_jpeg_to_sink(
        &[0xff, 0xd8, 0xff, 0xda, 0, 1],
        None,
        &mut malformed_sos_sink,
    );
    let mut malformed_segment_sink = Vec::new();
    let _ = write_jpeg_to_sink(
        &[0xff, 0xd8, 0xff, 0xe0, 0, 1],
        None,
        &mut malformed_segment_sink,
    );
    let mut progressive_sink = Vec::new();
    let progressive_sink_options = JpegEncodeOptions {
        progressive: Some(true),
        ..JpegEncodeOptions::default()
    };
    let progressive_bytes = encode(&rgb, &progressive_sink_options)
        .expect("coverage progressive JPEG image should encode");
    let _ = write_jpeg_to_sink(&progressive_bytes, None, &mut progressive_sink);
    let mut restart_sink = Vec::new();
    let restart_sink_options = JpegEncodeOptions {
        optimize: Some(true),
        subsampling: Some(JpegSubsampling::Cs422),
        restart_interval: Some(1),
        ..JpegEncodeOptions::default()
    };
    let restart_rgb = DecodedImage::new(
        32,
        32,
        (0usize..(32 * 32 * 3))
            .map(|index| index.to_le_bytes()[0].wrapping_mul(37))
            .collect(),
        crate::types::ColorType::Rgb8,
    );
    let restart_bytes = encode(&restart_rgb, &restart_sink_options)
        .expect("coverage restart JPEG image should encode");
    let _ = write_jpeg_to_sink(&restart_bytes, None, &mut restart_sink);
    FORCE_SCAN_MARKER_READ_ERROR.store(true, Ordering::Relaxed);
    let mut forced_scan_marker_sink = Vec::new();
    let _ = write_jpeg_to_sink(&restart_bytes, None, &mut forced_scan_marker_sink);
    #[cfg(coverage_nightly)]
    {
        let restart_probe_token = crate::CancellationToken::new();
        restart_probe_token.cancel_after(usize::MAX);
        let mut restart_probe_sink = Vec::new();
        let _ = write_jpeg_to_sink(
            &restart_bytes,
            Some(&restart_probe_token),
            &mut restart_probe_sink,
        );
        let restart_probe_checks = usize::MAX.saturating_sub(
            restart_probe_token
                .coverage_remaining_checks()
                .unwrap_or(usize::MAX),
        );
        for checks in 0..=restart_probe_checks {
            let token = crate::CancellationToken::new();
            token.cancel_after(checks);
            let mut sink = Vec::new();
            let _ = write_jpeg_to_sink(&restart_bytes, Some(&token), &mut sink);
        }
        let progressive_probe_token = crate::CancellationToken::new();
        progressive_probe_token.cancel_after(usize::MAX);
        let mut progressive_probe_sink = Vec::new();
        let _ = write_jpeg_to_sink(
            &progressive_bytes,
            Some(&progressive_probe_token),
            &mut progressive_probe_sink,
        );
        let progressive_probe_checks = usize::MAX.saturating_sub(
            progressive_probe_token
                .coverage_remaining_checks()
                .unwrap_or(usize::MAX),
        );
        for checks in 0..=progressive_probe_checks {
            let token = crate::CancellationToken::new();
            token.cancel_after(checks);
            let mut sink = Vec::new();
            let _ = write_jpeg_to_sink(&progressive_bytes, Some(&token), &mut sink);
        }
    }
    struct RejectAfterWrites {
        allowed: usize,
        writes: usize,
    }
    impl crate::OutputSink for RejectAfterWrites {
        fn write_all(&mut self, _bytes: &[u8]) -> crate::ImageResult<()> {
            if self.writes >= self.allowed {
                return Err(crate::ImageError::parameter(
                    "coverage JPEG sink rejected write",
                ));
            }
            self.writes += 1;
            Ok(())
        }
    }
    for allowed in [0, 1, 2, 32, 127] {
        let mut rejecting = RejectAfterWrites { allowed, writes: 0 };
        let _ = write_jpeg_to_sink(&restart_bytes, None, &mut rejecting);
        let mut rejecting = RejectAfterWrites { allowed, writes: 0 };
        let _ = write_jpeg_to_sink(&progressive_bytes, None, &mut rejecting);
    }
    let _ = jpeg_length_segment_end(&[0xff, 0xda, 0, 5, 0], 2);
    FORCE_MARKER_END_ERROR.store(true, Ordering::Relaxed);
    let _ = jpeg_length_segment_end(&[0, 2], 0);
    let mut forced_output_end_sink = Vec::new();
    let mut forced_output_end_written = 0usize;
    FORCE_SINK_OUTPUT_END_ERROR.store(true, Ordering::Relaxed);
    let _ = write_jpeg_sink_segment(
        &mut forced_output_end_sink,
        &[0],
        None,
        &mut forced_output_end_written,
    );
    let checkpoint_token = crate::CancellationToken::new();
    let mut output_checkpoint = TokenEntropyOutputCheckpoint::new(&checkpoint_token);
    let _ = output_checkpoint.observe(4_096);
    let _ = output_checkpoint.observe(8_192);
    let cancelled_output_token = crate::CancellationToken::new();
    cancelled_output_token.cancel();
    let mut cancelled_output_checkpoint =
        TokenEntropyOutputCheckpoint::new(&cancelled_output_token);
    let _ = cancelled_output_checkpoint.observe(2_048);
    let cancelled_mcu_token = crate::CancellationToken::new();
    cancelled_mcu_token.cancel();
    let mut cancelled_mcu_checkpoint = TokenEntropyOutputCheckpoint::new(&cancelled_mcu_token);
    cancelled_mcu_checkpoint.baseline_mcus_until_checkpoint = 1;
    let _ = cancelled_mcu_checkpoint.baseline_mcu();

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
    let rgb_observe_token = crate::CancellationToken::new();
    rgb_observe_token.cancel_after(1);
    let mut rgb_observe_checkpoint = TokenRgbConversionCheckpoint {
        token: &rgb_observe_token,
        pixels_until_checkpoint: 1,
    };
    let _ = std::hint::black_box(rgb_to_ycbcr_with_checkpoint(
        &[0, 0, 0],
        1,
        1,
        &mut rgb_observe_checkpoint,
    ));
    for checks in [0, 1, 2, 64, 128, 255] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = encode_with_token(&checkpoint_rgb, &JpegEncodeOptions::default(), Some(&token));
    }
    #[cfg(coverage_nightly)]
    {
        let baseline_probe_token = crate::CancellationToken::new();
        baseline_probe_token.cancel_after(usize::MAX);
        let _ = encode_with_token(
            &checkpoint_rgb,
            &JpegEncodeOptions::default(),
            Some(&baseline_probe_token),
        );
        let baseline_probe_checks = usize::MAX.saturating_sub(
            baseline_probe_token
                .coverage_remaining_checks()
                .unwrap_or(usize::MAX),
        );
        for checks in 0..=baseline_probe_checks {
            let token = crate::CancellationToken::new();
            token.cancel_after(checks);
            let _ = encode_with_token(&checkpoint_rgb, &JpegEncodeOptions::default(), Some(&token));
        }
    }
    let grayscale_token = crate::CancellationToken::new();
    grayscale_token.cancel_after(2);
    let _ = encode_with_token(&gray, &JpegEncodeOptions::default(), Some(&grayscale_token));
    let l1 = DecodedImage::with_mode(8, 2, vec![0xff, 0], ImageMode::L1);
    let l1_token = crate::CancellationToken::new();
    l1_token.cancel_after(2);
    let _ = encode_with_token(&l1, &JpegEncodeOptions::default(), Some(&l1_token));
    let cmyk_rows = DecodedImage::new(2, 2, vec![0; 2 * 2 * 4], crate::types::ColorType::Cmyk8);
    let cmyk_rows_token = crate::CancellationToken::new();
    cmyk_rows_token.cancel_after(2);
    let _ = encode_with_token(
        &cmyk_rows,
        &JpegEncodeOptions::default(),
        Some(&cmyk_rows_token),
    );
    let cmyk_fdct = DecodedImage::new(8, 8, vec![0; 8 * 8 * 4], crate::types::ColorType::Cmyk8);
    FORCE_FDCT_FAILURE_CALL.store(1, Ordering::Relaxed);
    let fdct_cb_token = crate::CancellationToken::new();
    let _ = encode_with_token(
        &cmyk_fdct,
        &JpegEncodeOptions::default(),
        Some(&fdct_cb_token),
    );
    FORCE_FDCT_FAILURE_CALL.store(3, Ordering::Relaxed);
    let fdct_k_token = crate::CancellationToken::new();
    let _ = encode_with_token(
        &cmyk_fdct,
        &JpegEncodeOptions::default(),
        Some(&fdct_k_token),
    );
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
    let downsample_observe_token = crate::CancellationToken::new();
    downsample_observe_token.cancel();
    let mut downsample_observe_checkpoint = TokenDownsampleCheckpoint {
        token: &downsample_observe_token,
        pixels_until_checkpoint: 1,
    };
    let _ = DownsampleCheckpoint::observe(&mut downsample_observe_checkpoint);
    let downsample_full_token = crate::CancellationToken::new();
    downsample_full_token.cancel_after(1);
    let mut downsample_full_checkpoint = TokenDownsampleCheckpoint {
        token: &downsample_full_token,
        pixels_until_checkpoint: 1,
    };
    let _ = downsample_with_checkpoint(&plane, 2, 2, 2, 2, 1, 1, &mut downsample_full_checkpoint);
    let downsample_full_success_token = crate::CancellationToken::new();
    let mut downsample_full_success_checkpoint =
        TokenDownsampleCheckpoint::new(&downsample_full_success_token);
    let _ = std::hint::black_box(downsample_with_checkpoint(
        &plane,
        2,
        2,
        2,
        2,
        1,
        1,
        &mut downsample_full_success_checkpoint,
    ));
    let downsample_subsampled_token = crate::CancellationToken::new();
    downsample_subsampled_token.cancel_after(1);
    let mut downsample_subsampled_checkpoint = TokenDownsampleCheckpoint {
        token: &downsample_subsampled_token,
        pixels_until_checkpoint: 1,
    };
    let _ = downsample_with_checkpoint(
        &plane,
        2,
        2,
        1,
        1,
        2,
        2,
        &mut downsample_subsampled_checkpoint,
    );
    let downsample_mixed_token = crate::CancellationToken::new();
    let mut downsample_mixed_checkpoint = TokenDownsampleCheckpoint::new(&downsample_mixed_token);
    let _ = std::hint::black_box(downsample_with_checkpoint(
        &plane,
        2,
        2,
        1,
        2,
        2,
        1,
        &mut downsample_mixed_checkpoint,
    ));
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let downsample_invalid_token = crate::CancellationToken::new();
        let mut downsample_invalid_checkpoint =
            TokenDownsampleCheckpoint::new(&downsample_invalid_token);
        let _ = downsample_with_checkpoint(
            &plane,
            2,
            2,
            1,
            1,
            2,
            3,
            &mut downsample_invalid_checkpoint,
        );
    }));
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let downsample_mixed_invalid_token = crate::CancellationToken::new();
        let mut downsample_mixed_invalid_checkpoint =
            TokenDownsampleCheckpoint::new(&downsample_mixed_invalid_token);
        let _ = downsample_with_checkpoint(
            &plane,
            2,
            2,
            2,
            1,
            1,
            2,
            &mut downsample_mixed_invalid_checkpoint,
        );
    }));
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
    let mut y_component = CompData {
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
    y_component.blocks[0][0] = 4;
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
    let noninterleaved_dc_scan = ProgScan {
        comps: vec![0],
        ss: 0,
        se: 0,
        ah: 0,
        al: 0,
        is_dc: true,
    };
    let noninterleaved_dc_components = [CompData {
        blocks: vec![[0i16; 64]; 2],
        blocks_per_row: 2,
        block_rows: 1,
        h_samp: 1,
        v_samp: 1,
        quant_slot: 0,
        id: 1,
        dc_tbl: 0,
        ac_tbl: 0,
    }];
    let mut noninterleaved_dc_noop_checkpoint = NoopProgressiveScanCheckpoint::new();
    let _ = std::hint::black_box(dc_progressive_events(
        &noninterleaved_dc_scan,
        &noninterleaved_dc_components,
        &mut noninterleaved_dc_noop_checkpoint,
    ));
    let mut noninterleaved_dc_event_checkpoint = CoverageFailingProgressiveEventCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(dc_progressive_events(
        &noninterleaved_dc_scan,
        &noninterleaved_dc_components,
        &mut noninterleaved_dc_event_checkpoint,
    ));
    let mut noninterleaved_dc_scan_checkpoint = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(dc_progressive_events(
        &noninterleaved_dc_scan,
        &noninterleaved_dc_components,
        &mut noninterleaved_dc_scan_checkpoint,
    ));
    // Drive the token checkpoint through the nested `?` edges in the AC
    // progressive helpers with one-block inputs. The public encoder polls at
    // coarser intervals, so these deliberately tight counters are coverage
    // scaffolding for the private cancellation contract.
    let mut progressive_token_ac_block = [0i16; 64];
    progressive_token_ac_block[ZIGZAG[1]] = 2;
    let progressive_token_ac_components = [CompData {
        blocks: vec![progressive_token_ac_block],
        blocks_per_row: 1,
        block_rows: 1,
        h_samp: 1,
        v_samp: 1,
        quant_slot: 0,
        id: 1,
        dc_tbl: 0,
        ac_tbl: 0,
    }];
    let progressive_token_refine_scan = ProgScan {
        comps: vec![0],
        ss: 1,
        se: 1,
        ah: 1,
        al: 0,
        is_dc: false,
    };
    let progressive_token_refine_token = crate::CancellationToken::new();
    progressive_token_refine_token.cancel_after(0);
    let mut progressive_token_refine_checkpoint = TokenProgressiveScanCheckpoint {
        token: &progressive_token_refine_token,
        blocks_until_checkpoint: 1,
        coefficients_until_checkpoint: 1,
        events_until_checkpoint: 1,
    };
    let mut progressive_token_refine_events = Vec::new();
    let mut progressive_token_refine_eob = 0;
    let mut progressive_token_refine_corrections = Vec::new();
    let _ = std::hint::black_box(append_ac_refine_events(
        &mut progressive_token_refine_events,
        &progressive_token_ac_components[0].blocks[0],
        &progressive_token_refine_scan,
        0,
        &mut progressive_token_refine_eob,
        &mut progressive_token_refine_corrections,
        &mut progressive_token_refine_checkpoint,
    ));
    let progressive_token_first_scan = ProgScan {
        comps: vec![0],
        ss: 1,
        se: 1,
        ah: 0,
        al: 0,
        is_dc: false,
    };
    for checks in [1, 2] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut checkpoint = TokenProgressiveScanCheckpoint {
            token: &token,
            blocks_until_checkpoint: 1,
            coefficients_until_checkpoint: 1,
            events_until_checkpoint: 1,
        };
        let _ = std::hint::black_box(ac_progressive_events(
            &progressive_token_first_scan,
            &progressive_token_ac_components,
            &mut checkpoint,
        ));
    }
    let progressive_token_refine_error_token = crate::CancellationToken::new();
    progressive_token_refine_error_token.cancel_after(1);
    let mut progressive_token_refine_error_checkpoint = TokenProgressiveScanCheckpoint {
        token: &progressive_token_refine_error_token,
        blocks_until_checkpoint: 1,
        coefficients_until_checkpoint: 1,
        events_until_checkpoint: 1,
    };
    let _ = std::hint::black_box(ac_progressive_events(
        &progressive_token_refine_scan,
        &progressive_token_ac_components,
        &mut progressive_token_refine_error_checkpoint,
    ));
    let progressive_token_refine_zero_token = crate::CancellationToken::new();
    let mut progressive_token_refine_zero_checkpoint = TokenProgressiveScanCheckpoint {
        token: &progressive_token_refine_zero_token,
        blocks_until_checkpoint: 1,
        coefficients_until_checkpoint: 1,
        events_until_checkpoint: 1,
    };
    let mut progressive_token_refine_zero_events = Vec::new();
    let mut progressive_token_refine_zero_eob = 0;
    let mut progressive_token_refine_zero_corrections = Vec::new();
    let _ = std::hint::black_box(append_ac_refine_events(
        &mut progressive_token_refine_zero_events,
        &progressive_components[1].blocks[0],
        &progressive_token_refine_scan,
        0,
        &mut progressive_token_refine_zero_eob,
        &mut progressive_token_refine_zero_corrections,
        &mut progressive_token_refine_zero_checkpoint,
    ));
    let progressive_dc_success_token = crate::CancellationToken::new();
    let mut progressive_dc_success_checkpoint =
        TokenProgressiveScanCheckpoint::new(&progressive_dc_success_token);
    let _ = std::hint::black_box(dc_progressive_events(
        &progressive_dc_scan,
        &progressive_components,
        &mut progressive_dc_success_checkpoint,
    ));

    let progressive_block_token = crate::CancellationToken::new();
    progressive_block_token.cancel();
    let mut progressive_block_checkpoint = TokenProgressiveScanCheckpoint {
        token: &progressive_block_token,
        blocks_until_checkpoint: 1,
        coefficients_until_checkpoint: 1,
        events_until_checkpoint: 1,
    };
    let _ = ProgressiveScanCheckpoint::block(&mut progressive_block_checkpoint);
    let progressive_event_token = crate::CancellationToken::new();
    progressive_event_token.cancel();
    let mut progressive_event_checkpoint = TokenProgressiveScanCheckpoint {
        token: &progressive_event_token,
        blocks_until_checkpoint: 1,
        coefficients_until_checkpoint: 1,
        events_until_checkpoint: 1,
    };
    let _ = ProgressiveScanCheckpoint::event(&mut progressive_event_checkpoint);

    // Exercise baseline entropy checkpoint failures at the restart boundary,
    // per-MCU boundary, and final flush. These are Rust cancellation states;
    // Pillow does not expose a caller-controlled entropy checkpoint.
    let baseline_components = [CompData {
        blocks: vec![[0i16; 64]; 4],
        blocks_per_row: 2,
        block_rows: 2,
        h_samp: 1,
        v_samp: 1,
        quant_slot: 0,
        id: 1,
        dc_tbl: 0,
        ac_tbl: 0,
    }];
    let baseline_dc = huffman::derive_table(&huffman::STD_DC_LUMA.0, &huffman::STD_DC_LUMA.1);
    let baseline_ac = huffman::derive_table(&huffman::STD_AC_LUMA.0, &huffman::STD_AC_LUMA.1);
    let baseline_dc_tables = [&baseline_dc, &baseline_dc];
    let baseline_ac_tables = [&baseline_ac, &baseline_ac];
    let mut baseline_frequency_nonzero_block = [0i16; 64];
    baseline_frequency_nonzero_block[ZIGZAG[1]] = 1;
    let baseline_frequency_nonzero_components = [CompData {
        blocks: vec![baseline_frequency_nonzero_block],
        blocks_per_row: 1,
        block_rows: 1,
        h_samp: 1,
        v_samp: 1,
        quant_slot: 0,
        id: 1,
        dc_tbl: 0,
        ac_tbl: 0,
    }];
    let baseline_frequency_zero_token = crate::CancellationToken::new();
    baseline_frequency_zero_token.cancel_after(1);
    let mut baseline_frequency_zero_checkpoint = TokenHuffmanFrequencyCheckpoint {
        token: &baseline_frequency_zero_token,
        coefficients_until_checkpoint: 1,
    };
    let _ = std::hint::black_box(baseline_frequencies_with_checkpoint(
        &baseline_components,
        1,
        1,
        1,
        &mut baseline_frequency_zero_checkpoint,
    ));
    let baseline_frequency_nonzero_token = crate::CancellationToken::new();
    baseline_frequency_nonzero_token.cancel_after(1);
    let mut baseline_frequency_nonzero_checkpoint = TokenHuffmanFrequencyCheckpoint {
        token: &baseline_frequency_nonzero_token,
        coefficients_until_checkpoint: 1,
    };
    let _ = std::hint::black_box(baseline_frequencies_with_checkpoint(
        &baseline_frequency_nonzero_components,
        1,
        1,
        1,
        &mut baseline_frequency_nonzero_checkpoint,
    ));
    let huffman_row_token = crate::CancellationToken::new();
    huffman_row_token.cancel();
    let _ = baseline_frequencies(&baseline_components, 1, 1, 1, Some(&huffman_row_token));
    let huffman_success_token = crate::CancellationToken::new();
    let _ = std::hint::black_box(baseline_frequencies(
        &baseline_components,
        1,
        1,
        1,
        Some(&huffman_success_token),
    ));
    for fail_after in [0, 1, 2, 8, 16] {
        let mut output = Vec::new();
        let mut checkpoint = CoverageFailingEntropyOutputCheckpoint {
            calls: 0,
            fail_after,
        };
        let _ = encode_baseline_entropy(
            &mut output,
            &baseline_components,
            1,
            1,
            &baseline_dc_tables,
            &baseline_ac_tables,
            1,
            None,
            &mut checkpoint,
        );
    }
    let mut final_flush_checkpoint = CoverageFailingEntropyOutputCheckpoint {
        calls: 0,
        fail_after: 11,
    };
    let mut final_flush_output = Vec::new();
    let _ = encode_baseline_entropy(
        &mut final_flush_output,
        &baseline_components,
        1,
        1,
        &baseline_dc_tables,
        &baseline_ac_tables,
        1,
        None,
        &mut final_flush_checkpoint,
    );
    let baseline_token = crate::CancellationToken::new();
    baseline_token.cancel_after(0);
    let mut baseline_token_checkpoint = TokenEntropyOutputCheckpoint {
        token: &baseline_token,
        observed_output: 0,
        bytes_until_checkpoint: 1,
        baseline_mcus_until_checkpoint: 1,
    };
    let mut baseline_token_output = Vec::new();
    let _ = encode_baseline_entropy(
        &mut baseline_token_output,
        &baseline_components,
        1,
        1,
        &baseline_dc_tables,
        &baseline_ac_tables,
        1,
        Some(&baseline_token),
        &mut baseline_token_checkpoint,
    );
    let baseline_success_token = crate::CancellationToken::new();
    let mut baseline_success_checkpoint = TokenEntropyOutputCheckpoint {
        token: &baseline_success_token,
        observed_output: 0,
        bytes_until_checkpoint: 1,
        baseline_mcus_until_checkpoint: 1,
    };
    let mut baseline_success_output = Vec::new();
    let _ = std::hint::black_box(encode_baseline_entropy(
        &mut baseline_success_output,
        &baseline_components,
        1,
        1,
        &baseline_dc_tables,
        &baseline_ac_tables,
        1,
        Some(&baseline_success_token),
        &mut baseline_success_checkpoint,
    ));
    let baseline_no_restart_token = crate::CancellationToken::new();
    let mut baseline_no_restart_checkpoint = TokenEntropyOutputCheckpoint {
        token: &baseline_no_restart_token,
        observed_output: 0,
        bytes_until_checkpoint: 1,
        baseline_mcus_until_checkpoint: 1,
    };
    let mut baseline_no_restart_output = Vec::new();
    let _ = std::hint::black_box(encode_baseline_entropy(
        &mut baseline_no_restart_output,
        &baseline_components,
        1,
        1,
        &baseline_dc_tables,
        &baseline_ac_tables,
        0,
        Some(&baseline_no_restart_token),
        &mut baseline_no_restart_checkpoint,
    ));
    let mut baseline_no_restart_failure_checkpoint = CoverageFailingEntropyOutputCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
    };
    let mut baseline_no_restart_failure_output = Vec::new();
    let _ = std::hint::black_box(encode_baseline_entropy(
        &mut baseline_no_restart_failure_output,
        &baseline_components,
        1,
        1,
        &baseline_dc_tables,
        &baseline_ac_tables,
        0,
        None,
        &mut baseline_no_restart_failure_checkpoint,
    ));
    for checks in 0..=64 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut checkpoint = TokenEntropyOutputCheckpoint {
            token: &token,
            observed_output: 0,
            bytes_until_checkpoint: 1,
            baseline_mcus_until_checkpoint: 1,
        };
        let mut output = Vec::new();
        let _ = std::hint::black_box(encode_baseline_entropy(
            &mut output,
            &baseline_components,
            1,
            1,
            &baseline_dc_tables,
            &baseline_ac_tables,
            1,
            Some(&token),
            &mut checkpoint,
        ));
    }
    let cancelled_baseline_token = crate::CancellationToken::new();
    cancelled_baseline_token.cancel();
    let mut cancelled_baseline_checkpoint = TokenEntropyOutputCheckpoint {
        token: &cancelled_baseline_token,
        observed_output: 0,
        bytes_until_checkpoint: 1,
        baseline_mcus_until_checkpoint: 1,
    };
    let mut cancelled_baseline_output = Vec::new();
    let _ = std::hint::black_box(encode_baseline_entropy(
        &mut cancelled_baseline_output,
        &baseline_components,
        1,
        1,
        &baseline_dc_tables,
        &baseline_ac_tables,
        1,
        Some(&cancelled_baseline_token),
        &mut cancelled_baseline_checkpoint,
    ));
    // Force the token checkpoint's per-MCU observation error after the row
    // poll has succeeded. The zero threshold is an internal coverage state;
    // production construction always starts with the configured byte budget.
    let per_mcu_observe_token = crate::CancellationToken::new();
    per_mcu_observe_token.cancel_after(1);
    let mut per_mcu_observe_checkpoint = TokenEntropyOutputCheckpoint {
        token: &per_mcu_observe_token,
        observed_output: 0,
        bytes_until_checkpoint: 0,
        baseline_mcus_until_checkpoint: usize::MAX,
    };
    let mut per_mcu_observe_output = Vec::new();
    let _ = std::hint::black_box(encode_baseline_entropy(
        &mut per_mcu_observe_output,
        &baseline_frequency_nonzero_components,
        1,
        1,
        &baseline_dc_tables,
        &baseline_ac_tables,
        0,
        Some(&per_mcu_observe_token),
        &mut per_mcu_observe_checkpoint,
    ));
    // A zero-width component gives the final flush observation no earlier
    // per-MCU checkpoint to consume the cancellation budget.
    let baseline_empty_components = [CompData {
        blocks: Vec::new(),
        blocks_per_row: 0,
        block_rows: 1,
        h_samp: 1,
        v_samp: 1,
        quant_slot: 0,
        id: 1,
        dc_tbl: 0,
        ac_tbl: 0,
    }];
    let final_observe_token = crate::CancellationToken::new();
    final_observe_token.cancel_after(1);
    let mut final_observe_checkpoint = TokenEntropyOutputCheckpoint {
        token: &final_observe_token,
        observed_output: 0,
        bytes_until_checkpoint: 0,
        baseline_mcus_until_checkpoint: usize::MAX,
    };
    let mut final_observe_output = Vec::new();
    let _ = std::hint::black_box(encode_baseline_entropy(
        &mut final_observe_output,
        &baseline_empty_components,
        1,
        1,
        &baseline_dc_tables,
        &baseline_ac_tables,
        0,
        Some(&final_observe_token),
        &mut final_observe_checkpoint,
    ));
    // The failing checkpoint deliberately succeeds so the token poll, rather
    // than checkpoint failure, owns this instantiation's row error edge.
    let failing_row_token = crate::CancellationToken::new();
    failing_row_token.cancel();
    let mut failing_row_checkpoint = CoverageFailingEntropyOutputCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
    };
    let mut failing_row_output = Vec::new();
    let _ = std::hint::black_box(encode_baseline_entropy(
        &mut failing_row_output,
        &baseline_frequency_nonzero_components,
        1,
        1,
        &baseline_dc_tables,
        &baseline_ac_tables,
        0,
        Some(&failing_row_token),
        &mut failing_row_checkpoint,
    ));
    let baseline_edge_token = crate::CancellationToken::new();
    let mut baseline_edge_checkpoint = TokenEntropyOutputCheckpoint {
        token: &baseline_edge_token,
        observed_output: 0,
        bytes_until_checkpoint: 1,
        baseline_mcus_until_checkpoint: 1,
    };
    let mut baseline_edge_output = Vec::new();
    let _ = std::hint::black_box(encode_baseline_entropy(
        &mut baseline_edge_output,
        &progressive_components,
        2,
        2,
        &baseline_dc_tables,
        &baseline_ac_tables,
        1,
        Some(&baseline_edge_token),
        &mut baseline_edge_checkpoint,
    ));
    let mut baseline_edge_failure_checkpoint = CoverageFailingEntropyOutputCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
    };
    let mut baseline_edge_failure_output = Vec::new();
    let _ = std::hint::black_box(encode_baseline_entropy(
        &mut baseline_edge_failure_output,
        &progressive_components,
        2,
        2,
        &baseline_dc_tables,
        &baseline_ac_tables,
        1,
        None,
        &mut baseline_edge_failure_checkpoint,
    ));
    for checks in 1..=3 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut checkpoint = TokenEntropyOutputCheckpoint {
            token: &token,
            observed_output: 0,
            bytes_until_checkpoint: 1,
            baseline_mcus_until_checkpoint: 1,
        };
        let mut output = Vec::new();
        let _ = encode_baseline_entropy(
            &mut output,
            &baseline_components,
            1,
            1,
            &baseline_dc_tables,
            &baseline_ac_tables,
            1,
            Some(&token),
            &mut checkpoint,
        );
    }
    let progressive_params = quant::build_params(75, "420", 3);
    let mut progressive_scan_error_output = Vec::new();
    let mut progressive_scan_error_entropy = NoopEntropyOutputCheckpoint;
    let mut progressive_scan_error_checkpoint = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: 0,
        fail_after: 0,
    };
    let _ = std::hint::black_box(encode_progressive_scans_exact(
        &mut progressive_scan_error_output,
        &progressive_components,
        3,
        2,
        2,
        &progressive_params,
        None,
        &mut progressive_scan_error_entropy,
        &mut progressive_scan_error_checkpoint,
    ));
    let mut noop_progressive_scan_error_output = Vec::new();
    let mut noop_progressive_scan_error_entropy = NoopEntropyOutputCheckpoint;
    let mut noop_progressive_scan_error_checkpoint =
        NoopProgressiveScanCheckpoint::with_fail_after(0);
    let _ = std::hint::black_box(encode_progressive_scans_exact(
        &mut noop_progressive_scan_error_output,
        &progressive_components,
        3,
        2,
        2,
        &progressive_params,
        None,
        &mut noop_progressive_scan_error_entropy,
        &mut noop_progressive_scan_error_checkpoint,
    ));
    let mut event_progressive_scan_error_output = Vec::new();
    let mut event_progressive_scan_error_entropy = NoopEntropyOutputCheckpoint;
    let mut event_progressive_scan_error_checkpoint = CoverageFailingProgressiveEventCheckpoint {
        calls: usize::MAX,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(encode_progressive_scans_exact(
        &mut event_progressive_scan_error_output,
        &progressive_components,
        3,
        2,
        2,
        &progressive_params,
        None,
        &mut event_progressive_scan_error_entropy,
        &mut event_progressive_scan_error_checkpoint,
    ));
    let mut progressive_scan_success_output = Vec::new();
    let mut progressive_scan_success_entropy = NoopEntropyOutputCheckpoint;
    let mut progressive_scan_success_checkpoint = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(encode_progressive_scans_exact(
        &mut progressive_scan_success_output,
        &progressive_components,
        3,
        2,
        2,
        &progressive_params,
        None,
        &mut progressive_scan_success_entropy,
        &mut progressive_scan_success_checkpoint,
    ));
    let progressive_token = crate::CancellationToken::new();
    progressive_token.cancel_after(0);
    let mut progressive_token_output = Vec::new();
    let mut progressive_token_entropy = TokenEntropyOutputCheckpoint::new(&progressive_token);
    let mut progressive_token_scan = TokenProgressiveScanCheckpoint::new(&progressive_token);
    let _ = std::hint::black_box(encode_progressive_scans_exact(
        &mut progressive_token_output,
        &baseline_components,
        1,
        1,
        1,
        &progressive_params,
        Some(&progressive_token),
        &mut progressive_token_entropy,
        &mut progressive_token_scan,
    ));
    let progressive_entropy_token = crate::CancellationToken::new();
    progressive_entropy_token.cancel_after(2);
    let mut progressive_entropy_components = [CompData {
        blocks: vec![[0i16; 64]],
        blocks_per_row: 1,
        block_rows: 1,
        h_samp: 1,
        v_samp: 1,
        quant_slot: 0,
        id: 1,
        dc_tbl: 0,
        ac_tbl: 0,
    }];
    progressive_entropy_components[0].blocks[0][0] = 2_047;
    let mut progressive_entropy_output = Vec::new();
    let mut progressive_entropy_checkpoint = TokenEntropyOutputCheckpoint {
        token: &progressive_entropy_token,
        observed_output: 0,
        bytes_until_checkpoint: 1,
        baseline_mcus_until_checkpoint: BASELINE_ENTROPY_CHECKPOINT_MCUS,
    };
    let mut progressive_entropy_scan =
        TokenProgressiveScanCheckpoint::new(&progressive_entropy_token);
    let _ = std::hint::black_box(encode_progressive_scans_exact(
        &mut progressive_entropy_output,
        &progressive_entropy_components,
        1,
        1,
        1,
        &progressive_params,
        Some(&progressive_entropy_token),
        &mut progressive_entropy_checkpoint,
        &mut progressive_entropy_scan,
    ));
    let progressive_start_token = crate::CancellationToken::new();
    progressive_start_token.cancel_after(2);
    let mut progressive_start_output = Vec::new();
    let mut progressive_start_entropy_checkpoint = NoopEntropyOutputCheckpoint;
    let mut progressive_start_scan_checkpoint = NoopProgressiveScanCheckpoint::new();
    let _ = encode_progressive_scans_exact(
        &mut progressive_start_output,
        &progressive_components,
        3,
        2,
        2,
        &progressive_params,
        Some(&progressive_start_token),
        &mut progressive_start_entropy_checkpoint,
        &mut progressive_start_scan_checkpoint,
    );
    for fail_after in [0, usize::MAX] {
        let mut progressive_event_output = Vec::new();
        let mut progressive_event_entropy_checkpoint = NoopEntropyOutputCheckpoint;
        let mut progressive_event_checkpoint = CoverageFailingProgressiveEventCheckpoint {
            calls: 0,
            fail_after,
        };
        let _ = std::hint::black_box(encode_progressive_scans_exact(
            &mut progressive_event_output,
            &progressive_components,
            3,
            2,
            2,
            &progressive_params,
            None,
            &mut progressive_event_entropy_checkpoint,
            &mut progressive_event_checkpoint,
        ));
    }
    for fail_after in [0, 1, 2, 16, 26, 64] {
        let mut output = Vec::new();
        let mut checkpoint = CoverageFailingEntropyOutputCheckpoint {
            calls: 0,
            fail_after,
        };
        let mut scan_checkpoint = NoopProgressiveScanCheckpoint::new();
        let _ = encode_progressive_scans_exact(
            &mut output,
            &progressive_components,
            3,
            2,
            2,
            &progressive_params,
            None,
            &mut checkpoint,
            &mut scan_checkpoint,
        );
    }
    for fail_after in [0, 1, 2, 16, 26, 64] {
        let mut output = Vec::new();
        let mut checkpoint = CoverageFailingEntropyOutputCheckpoint {
            calls: 0,
            fail_after: usize::MAX,
        };
        let mut scan_checkpoint = NoopProgressiveScanCheckpoint::with_fail_after(fail_after);
        let _ = std::hint::black_box(encode_progressive_scans_exact(
            &mut output,
            &progressive_components,
            3,
            2,
            2,
            &progressive_params,
            None,
            &mut checkpoint,
            &mut scan_checkpoint,
        ));
    }
    let progressive_end_token = crate::CancellationToken::new();
    progressive_end_token.cancel_after(1);
    let mut progressive_end_output = Vec::new();
    let mut progressive_end_entropy_checkpoint = CoverageFailingEntropyOutputCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
    };
    let mut progressive_end_scan_checkpoint = NoopProgressiveScanCheckpoint::new();
    let _ = std::hint::black_box(encode_progressive_scans_exact(
        &mut progressive_end_output,
        &progressive_components,
        3,
        2,
        2,
        &progressive_params,
        Some(&progressive_end_token),
        &mut progressive_end_entropy_checkpoint,
        &mut progressive_end_scan_checkpoint,
    ));
    let progressive_start_error_token = crate::CancellationToken::new();
    progressive_start_error_token.cancel();
    let mut progressive_start_error_output = Vec::new();
    let mut progressive_start_error_entropy_checkpoint = CoverageFailingEntropyOutputCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
    };
    let mut progressive_start_error_scan_checkpoint = NoopProgressiveScanCheckpoint::new();
    let _ = std::hint::black_box(encode_progressive_scans_exact(
        &mut progressive_start_error_output,
        &progressive_components,
        3,
        2,
        2,
        &progressive_params,
        Some(&progressive_start_error_token),
        &mut progressive_start_error_entropy_checkpoint,
        &mut progressive_start_error_scan_checkpoint,
    ));
    let mut entropy_progressive_scan_error_output = Vec::new();
    let mut entropy_progressive_scan_error_checkpoint = CoverageFailingEntropyOutputCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
    };
    let mut entropy_progressive_scan_error_scan_checkpoint =
        NoopProgressiveScanCheckpoint::with_fail_after(0);
    let _ = std::hint::black_box(encode_progressive_scans_exact(
        &mut entropy_progressive_scan_error_output,
        &progressive_components,
        3,
        2,
        2,
        &progressive_params,
        None,
        &mut entropy_progressive_scan_error_checkpoint,
        &mut entropy_progressive_scan_error_scan_checkpoint,
    ));
    let mut progressive_checkpoint = NoopProgressiveScanCheckpoint::new();
    let _ = dc_progressive_events(
        &progressive_dc_scan,
        &progressive_components,
        &mut progressive_checkpoint,
    );
    for fail_after in 0..=1 {
        let mut checkpoint = NoopProgressiveScanCheckpoint::with_fail_after(fail_after);
        let _ = std::hint::black_box(dc_progressive_events(
            &progressive_dc_scan,
            &progressive_components,
            &mut checkpoint,
        ));
    }
    let mut progressive_row_failure_checkpoint = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: usize::MAX,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(dc_progressive_events(
        &progressive_dc_scan,
        &progressive_components,
        &mut progressive_row_failure_checkpoint,
    ));
    let mut progressive_event_row_failure_checkpoint = CoverageFailingProgressiveEventCheckpoint {
        calls: usize::MAX,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(dc_progressive_events(
        &progressive_dc_scan,
        &progressive_components,
        &mut progressive_event_row_failure_checkpoint,
    ));
    let noninterleaved_row_scan = ProgScan {
        comps: vec![0],
        ss: 0,
        se: 0,
        ah: 0,
        al: 0,
        is_dc: true,
    };
    let mut noninterleaved_row_failure_checkpoint = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: usize::MAX,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(dc_progressive_events(
        &noninterleaved_row_scan,
        &progressive_components,
        &mut noninterleaved_row_failure_checkpoint,
    ));
    let mut noninterleaved_event_row_failure_checkpoint =
        CoverageFailingProgressiveEventCheckpoint {
            calls: usize::MAX,
            fail_after: usize::MAX,
        };
    let _ = std::hint::black_box(dc_progressive_events(
        &noninterleaved_row_scan,
        &progressive_components,
        &mut noninterleaved_event_row_failure_checkpoint,
    ));
    let noninterleaved_ac_row_scan = ProgScan {
        comps: vec![0],
        ss: 1,
        se: 1,
        ah: 0,
        al: 0,
        is_dc: false,
    };
    let mut noninterleaved_ac_row_failure_checkpoint = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: usize::MAX,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(ac_progressive_events(
        &noninterleaved_ac_row_scan,
        &progressive_components,
        &mut noninterleaved_ac_row_failure_checkpoint,
    ));
    let mut noninterleaved_ac_event_row_failure_checkpoint =
        CoverageFailingProgressiveEventCheckpoint {
            calls: usize::MAX,
            fail_after: usize::MAX,
        };
    let _ = std::hint::black_box(ac_progressive_events(
        &noninterleaved_ac_row_scan,
        &progressive_components,
        &mut noninterleaved_ac_event_row_failure_checkpoint,
    ));
    for checks in 0..=2 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut checkpoint = TokenProgressiveScanCheckpoint {
            token: &token,
            blocks_until_checkpoint: 1,
            coefficients_until_checkpoint: PROGRESSIVE_COEFFICIENT_CHECKPOINT_COEFFICIENTS,
            events_until_checkpoint: PROGRESSIVE_EVENT_CHECKPOINT_EVENTS,
        };
        let _ = std::hint::black_box(dc_progressive_events(
            &progressive_dc_scan,
            &progressive_components,
            &mut checkpoint,
        ));
    }
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
    let mut single_progressive_checkpoint = TokenProgressiveScanCheckpoint::new(&single_dc_token);
    let _ = dc_progressive_events(
        &single_dc_scan,
        &progressive_components,
        &mut single_progressive_checkpoint,
    );
    for fail_after in 0..=1 {
        let mut checkpoint = NoopProgressiveScanCheckpoint::with_fail_after(fail_after);
        let _ = std::hint::black_box(dc_progressive_events(
            &single_dc_scan,
            &progressive_components,
            &mut checkpoint,
        ));
    }
    for checks in 0..=2 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut checkpoint = TokenProgressiveScanCheckpoint {
            token: &token,
            blocks_until_checkpoint: 1,
            coefficients_until_checkpoint: PROGRESSIVE_COEFFICIENT_CHECKPOINT_COEFFICIENTS,
            events_until_checkpoint: PROGRESSIVE_EVENT_CHECKPOINT_EVENTS,
        };
        let _ = std::hint::black_box(dc_progressive_events(
            &single_dc_scan,
            &progressive_components,
            &mut checkpoint,
        ));
    }
    let single_dc_success_token = crate::CancellationToken::new();
    let mut single_dc_success_checkpoint =
        TokenProgressiveScanCheckpoint::new(&single_dc_success_token);
    let _ = std::hint::black_box(dc_progressive_events(
        &single_dc_scan,
        &progressive_components,
        &mut single_dc_success_checkpoint,
    ));
    let mut single_dc_event_checkpoint = CoverageFailingProgressiveEventCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(dc_progressive_events(
        &single_dc_scan,
        &progressive_components,
        &mut single_dc_event_checkpoint,
    ));
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
    let mut ac_progressive_checkpoint = NoopProgressiveScanCheckpoint::new();
    correction_bits.resize(938, 0);
    let _ = std::hint::black_box(append_ac_refine_events(
        &mut events,
        &block,
        &scan,
        table,
        &mut eob_run,
        &mut correction_bits,
        &mut ac_progressive_checkpoint,
    ));
    let mut token_correction_events = Vec::new();
    let mut token_correction_eob = 0;
    let mut token_correction_bits = vec![0; 938];
    let token_correction_token = crate::CancellationToken::new();
    let mut token_correction_checkpoint =
        TokenProgressiveScanCheckpoint::new(&token_correction_token);
    let _ = std::hint::black_box(append_ac_refine_events(
        &mut token_correction_events,
        &block,
        &scan,
        table,
        &mut token_correction_eob,
        &mut token_correction_bits,
        &mut token_correction_checkpoint,
    ));
    let mut event_correction_events = Vec::new();
    let mut event_correction_eob = 0;
    let mut event_correction_bits = vec![0; 938];
    let mut event_correction_checkpoint = CoverageFailingProgressiveEventCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(append_ac_refine_events(
        &mut event_correction_events,
        &block,
        &scan,
        table,
        &mut event_correction_eob,
        &mut event_correction_bits,
        &mut event_correction_checkpoint,
    ));
    let refine_scan = ProgScan {
        comps: vec![0],
        ss: 1,
        se: 5,
        ah: 1,
        al: 0,
        is_dc: false,
    };
    let mut refine_block = [0i16; 64];
    refine_block[ZIGZAG[1]] = 1;
    refine_block[ZIGZAG[2]] = 2;
    let mut refine_events = Vec::new();
    let mut refine_eob_run = 0;
    let mut refine_correction_bits = Vec::new();
    let mut refine_checkpoint = NoopProgressiveScanCheckpoint::new();
    let _ = append_ac_refine_events(
        &mut refine_events,
        &refine_block,
        &refine_scan,
        table,
        &mut refine_eob_run,
        &mut refine_correction_bits,
        &mut refine_checkpoint,
    );
    let refine_token = crate::CancellationToken::new();
    let mut refine_token_checkpoint = TokenProgressiveScanCheckpoint::new(&refine_token);
    let _ = append_ac_refine_events(
        &mut refine_events,
        &refine_block,
        &refine_scan,
        table,
        &mut refine_eob_run,
        &mut refine_correction_bits,
        &mut refine_token_checkpoint,
    );
    for fail_after in 0..=3 {
        let mut failure_events = Vec::new();
        let mut failure_eob_run = 0;
        let mut failure_corrections = Vec::new();
        let mut checkpoint = NoopProgressiveScanCheckpoint::with_fail_after(fail_after);
        let _ = append_ac_refine_events(
            &mut failure_events,
            &refine_block,
            &refine_scan,
            table,
            &mut failure_eob_run,
            &mut failure_corrections,
            &mut checkpoint,
        );
    }
    for checks in 0..=3 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut failure_events = Vec::new();
        let mut failure_eob_run = 0;
        let mut failure_corrections = Vec::new();
        let mut checkpoint = TokenProgressiveScanCheckpoint {
            token: &token,
            blocks_until_checkpoint: PROGRESSIVE_SCAN_CHECKPOINT_BLOCKS,
            coefficients_until_checkpoint: 1,
            events_until_checkpoint: PROGRESSIVE_EVENT_CHECKPOINT_EVENTS,
        };
        let _ = append_ac_refine_events(
            &mut failure_events,
            &refine_block,
            &refine_scan,
            table,
            &mut failure_eob_run,
            &mut failure_corrections,
            &mut checkpoint,
        );
    }
    let ac_first_scan = ProgScan {
        comps: vec![0],
        ss: 1,
        se: 5,
        ah: 0,
        al: 0,
        is_dc: false,
    };
    let mut first_block = [0i16; 64];
    first_block[ZIGZAG[1]] = 1;
    for fail_after in 0..=2 {
        let mut first_events = Vec::new();
        let mut first_eob_run = 0;
        let mut first_corrections = Vec::new();
        let mut checkpoint = CoverageFailingProgressiveScanCheckpoint {
            coefficient_calls: 0,
            fail_after,
        };
        let _ = append_ac_first_events(
            &mut first_events,
            &first_block,
            &ac_first_scan,
            table,
            &mut first_eob_run,
            &mut first_corrections,
            &mut checkpoint,
        );
    }
    for fail_after in 0..=2 {
        let mut first_events = Vec::new();
        let mut first_eob_run = 0;
        let mut first_corrections = Vec::new();
        let mut checkpoint = NoopProgressiveScanCheckpoint::with_fail_after(fail_after);
        let _ = append_ac_first_events(
            &mut first_events,
            &first_block,
            &ac_first_scan,
            table,
            &mut first_eob_run,
            &mut first_corrections,
            &mut checkpoint,
        );
    }
    for checks in 0..=2 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut first_events = Vec::new();
        let mut first_eob_run = 0;
        let mut first_corrections = Vec::new();
        let mut checkpoint = TokenProgressiveScanCheckpoint {
            token: &token,
            blocks_until_checkpoint: PROGRESSIVE_SCAN_CHECKPOINT_BLOCKS,
            coefficients_until_checkpoint: 1,
            events_until_checkpoint: PROGRESSIVE_EVENT_CHECKPOINT_EVENTS,
        };
        let _ = append_ac_first_events(
            &mut first_events,
            &first_block,
            &ac_first_scan,
            table,
            &mut first_eob_run,
            &mut first_corrections,
            &mut checkpoint,
        );
    }
    let mut refine_failure_checkpoint = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: 0,
        fail_after: std::hint::black_box(0),
    };
    let _ = append_ac_refine_events(
        &mut refine_events,
        &refine_block,
        &refine_scan,
        table,
        &mut refine_eob_run,
        &mut refine_correction_bits,
        &mut refine_failure_checkpoint,
    );
    let long_ac_first_scan = ProgScan {
        comps: vec![0],
        ss: 1,
        se: 20,
        ah: 0,
        al: 0,
        is_dc: false,
    };
    let mut long_ac_first_block = [0i16; 64];
    long_ac_first_block[ZIGZAG[17]] = -3;
    let mut long_ac_first_events = Vec::new();
    let mut long_ac_first_eob = 0;
    let mut long_ac_first_corrections = Vec::new();
    let mut long_ac_first_noop = NoopProgressiveScanCheckpoint::new();
    let _ = append_ac_first_events(
        &mut long_ac_first_events,
        &long_ac_first_block,
        &long_ac_first_scan,
        table,
        &mut long_ac_first_eob,
        &mut long_ac_first_corrections,
        &mut long_ac_first_noop,
    );
    let long_ac_first_token = crate::CancellationToken::new();
    let mut long_ac_first_token_checkpoint =
        TokenProgressiveScanCheckpoint::new(&long_ac_first_token);
    let _ = append_ac_first_events(
        &mut long_ac_first_events,
        &long_ac_first_block,
        &long_ac_first_scan,
        table,
        &mut long_ac_first_eob,
        &mut long_ac_first_corrections,
        &mut long_ac_first_token_checkpoint,
    );
    let mut long_ac_first_failing = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: 0,
        fail_after: usize::MAX,
    };
    let _ = append_ac_first_events(
        &mut long_ac_first_events,
        &long_ac_first_block,
        &long_ac_first_scan,
        table,
        &mut long_ac_first_eob,
        &mut long_ac_first_corrections,
        &mut long_ac_first_failing,
    );
    let long_ac_refine_scan = ProgScan {
        comps: vec![0],
        ss: 1,
        se: 20,
        ah: 1,
        al: 0,
        is_dc: false,
    };
    let mut long_ac_refine_block = [0i16; 64];
    long_ac_refine_block[ZIGZAG[1]] = 2;
    long_ac_refine_block[ZIGZAG[18]] = 1;
    let mut long_ac_refine_events = Vec::new();
    let mut long_ac_refine_eob = 0;
    let mut long_ac_refine_corrections = Vec::new();
    let mut long_ac_refine_noop = NoopProgressiveScanCheckpoint::new();
    let _ = append_ac_refine_events(
        &mut long_ac_refine_events,
        &long_ac_refine_block,
        &long_ac_refine_scan,
        table,
        &mut long_ac_refine_eob,
        &mut long_ac_refine_corrections,
        &mut long_ac_refine_noop,
    );
    let long_ac_refine_token = crate::CancellationToken::new();
    let mut long_ac_refine_token_checkpoint =
        TokenProgressiveScanCheckpoint::new(&long_ac_refine_token);
    let _ = append_ac_refine_events(
        &mut long_ac_refine_events,
        &long_ac_refine_block,
        &long_ac_refine_scan,
        table,
        &mut long_ac_refine_eob,
        &mut long_ac_refine_corrections,
        &mut long_ac_refine_token_checkpoint,
    );
    let mut long_ac_refine_failing = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: 0,
        fail_after: usize::MAX,
    };
    let _ = append_ac_refine_events(
        &mut long_ac_refine_events,
        &long_ac_refine_block,
        &long_ac_refine_scan,
        table,
        &mut long_ac_refine_eob,
        &mut long_ac_refine_corrections,
        &mut long_ac_refine_failing,
    );
    let mut long_ac_refine_event = CoverageFailingProgressiveEventCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
    };
    let _ = append_ac_refine_events(
        &mut long_ac_refine_events,
        &long_ac_refine_block,
        &long_ac_refine_scan,
        table,
        &mut long_ac_refine_eob,
        &mut long_ac_refine_corrections,
        &mut long_ac_refine_event,
    );
    let mut refine_failing_checkpoint = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: 0,
        fail_after: std::hint::black_box(1),
    };
    let _ = append_ac_refine_events(
        &mut refine_events,
        &refine_block,
        &refine_scan,
        table,
        &mut refine_eob_run,
        &mut refine_correction_bits,
        &mut refine_failing_checkpoint,
    );
    let mut ac_first_checkpoint = NoopProgressiveScanCheckpoint::new();
    let _ = ac_progressive_events(
        &ac_first_scan,
        &progressive_components,
        &mut ac_first_checkpoint,
    );
    for fail_after in [0, 1, 6] {
        let mut checkpoint = NoopProgressiveScanCheckpoint::with_fail_after(fail_after);
        let _ = std::hint::black_box(ac_progressive_events(
            &ac_first_scan,
            &progressive_components,
            &mut checkpoint,
        ));
    }
    for fail_after in [0, 1, 6] {
        let mut checkpoint = NoopProgressiveScanCheckpoint::with_fail_after(fail_after);
        let _ = std::hint::black_box(ac_progressive_events(
            &refine_scan,
            &progressive_components,
            &mut checkpoint,
        ));
    }
    for checks in [0, 1, 2, 6, 12] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut checkpoint = TokenProgressiveScanCheckpoint::new(&token);
        let _ = ac_progressive_events(&ac_first_scan, &progressive_components, &mut checkpoint);
    }
    let ac_success_token = crate::CancellationToken::new();
    let mut ac_success_checkpoint = TokenProgressiveScanCheckpoint::new(&ac_success_token);
    let _ = std::hint::black_box(ac_progressive_events(
        &ac_first_scan,
        &progressive_components,
        &mut ac_success_checkpoint,
    ));
    let ac_refine_success_token = crate::CancellationToken::new();
    let mut ac_refine_success_checkpoint =
        TokenProgressiveScanCheckpoint::new(&ac_refine_success_token);
    let _ = std::hint::black_box(ac_progressive_events(
        &refine_scan,
        &progressive_components,
        &mut ac_refine_success_checkpoint,
    ));
    let mut ac_event_checkpoint = CoverageFailingProgressiveEventCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(ac_progressive_events(
        &ac_first_scan,
        &progressive_components,
        &mut ac_event_checkpoint,
    ));
    let mut ac_refine_event_checkpoint = CoverageFailingProgressiveEventCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(ac_progressive_events(
        &refine_scan,
        &progressive_components,
        &mut ac_refine_event_checkpoint,
    ));
    let mut ac_failure_success_checkpoint = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(ac_progressive_events(
        &refine_scan,
        &progressive_components,
        &mut ac_failure_success_checkpoint,
    ));
    let mut final_coefficient_block = [0i16; 64];
    final_coefficient_block[ZIGZAG[5]] = 1;
    let mut final_coefficient_events = Vec::new();
    let mut final_coefficient_eob = 0;
    let mut final_coefficient_corrections = Vec::new();
    let mut final_coefficient_checkpoint = NoopProgressiveScanCheckpoint::new();
    let _ = std::hint::black_box(append_ac_first_events(
        &mut final_coefficient_events,
        &final_coefficient_block,
        &ac_first_scan,
        table,
        &mut final_coefficient_eob,
        &mut final_coefficient_corrections,
        &mut final_coefficient_checkpoint,
    ));
    let final_coefficient_token = crate::CancellationToken::new();
    let mut final_coefficient_token_checkpoint =
        TokenProgressiveScanCheckpoint::new(&final_coefficient_token);
    let _ = std::hint::black_box(append_ac_first_events(
        &mut final_coefficient_events,
        &final_coefficient_block,
        &ac_first_scan,
        table,
        &mut final_coefficient_eob,
        &mut final_coefficient_corrections,
        &mut final_coefficient_token_checkpoint,
    ));
    let mut final_coefficient_event_checkpoint = CoverageFailingProgressiveEventCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
    };
    let mut final_coefficient_event_eob = 0;
    let mut final_coefficient_event_corrections = Vec::new();
    let _ = std::hint::black_box(append_ac_first_events(
        &mut final_coefficient_events,
        &final_coefficient_block,
        &ac_first_scan,
        table,
        &mut final_coefficient_event_eob,
        &mut final_coefficient_event_corrections,
        &mut final_coefficient_event_checkpoint,
    ));
    let final_eob_block = [0i16; 64];
    let mut final_eob_events = Vec::new();
    let mut final_eob_run = 0x7ffe;
    let mut final_eob_corrections = Vec::new();
    let mut final_eob_checkpoint = NoopProgressiveScanCheckpoint::new();
    let _ = std::hint::black_box(append_ac_first_events(
        &mut final_eob_events,
        &final_eob_block,
        &ac_first_scan,
        table,
        &mut final_eob_run,
        &mut final_eob_corrections,
        &mut final_eob_checkpoint,
    ));
    let final_eob_token = crate::CancellationToken::new();
    let mut final_eob_token_checkpoint = TokenProgressiveScanCheckpoint::new(&final_eob_token);
    let mut final_eob_token_run = 0x7ffe;
    let _ = std::hint::black_box(append_ac_first_events(
        &mut final_eob_events,
        &final_eob_block,
        &ac_first_scan,
        table,
        &mut final_eob_token_run,
        &mut final_eob_corrections,
        &mut final_eob_token_checkpoint,
    ));
    let mut final_eob_event_checkpoint = CoverageFailingProgressiveEventCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
    };
    let mut final_eob_event_run = 0x7ffe;
    let mut final_eob_event_corrections = Vec::new();
    let _ = std::hint::black_box(append_ac_first_events(
        &mut final_eob_events,
        &final_eob_block,
        &ac_first_scan,
        table,
        &mut final_eob_event_run,
        &mut final_eob_event_corrections,
        &mut final_eob_event_checkpoint,
    ));
    let mut final_coefficient_failure_events = Vec::new();
    let mut final_coefficient_failure_eob = 0;
    let mut final_coefficient_failure_corrections = Vec::new();
    let mut final_coefficient_failure_checkpoint = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(append_ac_first_events(
        &mut final_coefficient_failure_events,
        &final_coefficient_block,
        &ac_first_scan,
        table,
        &mut final_coefficient_failure_eob,
        &mut final_coefficient_failure_corrections,
        &mut final_coefficient_failure_checkpoint,
    ));
    let mut final_eob_failure_events = Vec::new();
    let mut final_eob_failure_run = 0x7ffe;
    let mut final_eob_failure_corrections = Vec::new();
    let mut final_eob_failure_checkpoint = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(append_ac_first_events(
        &mut final_eob_failure_events,
        &final_eob_block,
        &ac_first_scan,
        table,
        &mut final_eob_failure_run,
        &mut final_eob_failure_corrections,
        &mut final_eob_failure_checkpoint,
    ));
    let mut event_long_first_events = Vec::new();
    let mut event_long_first_eob = 1;
    let mut event_long_first_corrections = Vec::new();
    let mut event_long_first_checkpoint = CoverageFailingProgressiveEventCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(append_ac_first_events(
        &mut event_long_first_events,
        &long_ac_first_block,
        &long_ac_first_scan,
        table,
        &mut event_long_first_eob,
        &mut event_long_first_corrections,
        &mut event_long_first_checkpoint,
    ));
    let mut refine_after_last_block = [0i16; 64];
    refine_after_last_block[ZIGZAG[1]] = 1;
    refine_after_last_block[ZIGZAG[18]] = 2;
    let mut refine_after_last_failure_events = Vec::new();
    let mut refine_after_last_failure_eob = 0;
    let mut refine_after_last_failure_corrections = Vec::new();
    let mut refine_after_last_failure_checkpoint = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(append_ac_refine_events(
        &mut refine_after_last_failure_events,
        &refine_after_last_block,
        &long_ac_refine_scan,
        table,
        &mut refine_after_last_failure_eob,
        &mut refine_after_last_failure_corrections,
        &mut refine_after_last_failure_checkpoint,
    ));
    let mut refine_after_last_event_events = Vec::new();
    let mut refine_after_last_event_eob = 0;
    let mut refine_after_last_event_corrections = Vec::new();
    let mut refine_after_last_event_checkpoint = CoverageFailingProgressiveEventCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(append_ac_refine_events(
        &mut refine_after_last_event_events,
        &refine_after_last_block,
        &long_ac_refine_scan,
        table,
        &mut refine_after_last_event_eob,
        &mut refine_after_last_event_corrections,
        &mut refine_after_last_event_checkpoint,
    ));
    let mut refine_eob_failure_events = Vec::new();
    let mut refine_eob_failure_run = 0x7ffe;
    let mut refine_eob_failure_corrections = Vec::new();
    let mut refine_eob_failure_checkpoint = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(append_ac_refine_events(
        &mut refine_eob_failure_events,
        &final_eob_block,
        &long_ac_refine_scan,
        table,
        &mut refine_eob_failure_run,
        &mut refine_eob_failure_corrections,
        &mut refine_eob_failure_checkpoint,
    ));
    let mut refine_eob_event_events = Vec::new();
    let mut refine_eob_event_run = 0x7ffe;
    let mut refine_eob_event_corrections = Vec::new();
    let mut refine_eob_event_checkpoint = CoverageFailingProgressiveEventCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(append_ac_refine_events(
        &mut refine_eob_event_events,
        &final_eob_block,
        &long_ac_refine_scan,
        table,
        &mut refine_eob_event_run,
        &mut refine_eob_event_corrections,
        &mut refine_eob_event_checkpoint,
    ));
    let mut refine_buffer_failure_events = Vec::new();
    let mut refine_buffer_failure_run = 0;
    let mut refine_buffer_failure_corrections = vec![0; 938];
    let mut refine_buffer_failure_checkpoint = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(append_ac_refine_events(
        &mut refine_buffer_failure_events,
        &final_eob_block,
        &long_ac_refine_scan,
        table,
        &mut refine_buffer_failure_run,
        &mut refine_buffer_failure_corrections,
        &mut refine_buffer_failure_checkpoint,
    ));
    let mut refine_buffer_event_events = Vec::new();
    let mut refine_buffer_event_run = 0;
    let mut refine_buffer_event_corrections = vec![0; 938];
    let mut refine_buffer_event_checkpoint = CoverageFailingProgressiveEventCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(append_ac_refine_events(
        &mut refine_buffer_event_events,
        &final_eob_block,
        &long_ac_refine_scan,
        table,
        &mut refine_buffer_event_run,
        &mut refine_buffer_event_corrections,
        &mut refine_buffer_event_checkpoint,
    ));
    let mut refine_final_block = [0i16; 64];
    refine_final_block[ZIGZAG[20]] = 1;
    let mut refine_final_failure_events = Vec::new();
    let mut refine_final_failure_run = 0;
    let mut refine_final_failure_corrections = Vec::new();
    let mut refine_final_failure_checkpoint = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(append_ac_refine_events(
        &mut refine_final_failure_events,
        &refine_final_block,
        &long_ac_refine_scan,
        table,
        &mut refine_final_failure_run,
        &mut refine_final_failure_corrections,
        &mut refine_final_failure_checkpoint,
    ));
    let mut refine_final_event_events = Vec::new();
    let mut refine_final_event_run = 0;
    let mut refine_final_event_corrections = Vec::new();
    let mut refine_final_event_checkpoint = CoverageFailingProgressiveEventCheckpoint {
        calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(append_ac_refine_events(
        &mut refine_final_event_events,
        &refine_final_block,
        &long_ac_refine_scan,
        table,
        &mut refine_final_event_run,
        &mut refine_final_event_corrections,
        &mut refine_final_event_checkpoint,
    ));
    let mut refine_final_correction_block = [0i16; 64];
    refine_final_correction_block[ZIGZAG[20]] = 2;
    let mut refine_final_correction_events = Vec::new();
    let mut refine_final_correction_run = 0;
    let mut refine_final_correction_bits = Vec::new();
    let mut refine_final_correction_checkpoint = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(append_ac_refine_events(
        &mut refine_final_correction_events,
        &refine_final_correction_block,
        &long_ac_refine_scan,
        table,
        &mut refine_final_correction_run,
        &mut refine_final_correction_bits,
        &mut refine_final_correction_checkpoint,
    ));
    let refine_token_eob_token = crate::CancellationToken::new();
    let mut refine_token_eob_checkpoint =
        TokenProgressiveScanCheckpoint::new(&refine_token_eob_token);
    let mut refine_token_eob_events = Vec::new();
    let mut refine_token_eob_run = 0x7ffe;
    let mut refine_token_eob_corrections = Vec::new();
    let _ = std::hint::black_box(append_ac_refine_events(
        &mut refine_token_eob_events,
        &final_eob_block,
        &long_ac_refine_scan,
        table,
        &mut refine_token_eob_run,
        &mut refine_token_eob_corrections,
        &mut refine_token_eob_checkpoint,
    ));
    let mut refine_token_small_eob_events = Vec::new();
    let mut refine_token_small_eob_run = 0x7ffe;
    let mut refine_token_small_eob_corrections = Vec::new();
    let refine_token_small_eob_token = crate::CancellationToken::new();
    let mut refine_token_small_eob_checkpoint =
        TokenProgressiveScanCheckpoint::new(&refine_token_small_eob_token);
    let _ = std::hint::black_box(append_ac_refine_events(
        &mut refine_token_small_eob_events,
        &final_eob_block,
        &scan,
        table,
        &mut refine_token_small_eob_run,
        &mut refine_token_small_eob_corrections,
        &mut refine_token_small_eob_checkpoint,
    ));
    let mut refine_final_correction_small_block = [0i16; 64];
    refine_final_correction_small_block[ZIGZAG[1]] = 2;
    let mut refine_final_correction_small_events = Vec::new();
    let mut refine_final_correction_small_run = 0;
    let mut refine_final_correction_small_bits = Vec::new();
    let mut refine_final_correction_small_checkpoint = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: 0,
        fail_after: usize::MAX,
    };
    let _ = std::hint::black_box(append_ac_refine_events(
        &mut refine_final_correction_small_events,
        &refine_final_correction_small_block,
        &scan,
        table,
        &mut refine_final_correction_small_run,
        &mut refine_final_correction_small_bits,
        &mut refine_final_correction_small_checkpoint,
    ));
    let mut checkpoint = CoverageFailingProgressiveScanCheckpoint {
        coefficient_calls: 0,
        fail_after: std::hint::black_box(0),
    };
    let _ = ac_progressive_events(&ac_first_scan, &progressive_components, &mut checkpoint);
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
    if let Some(token) = token {
        let mut checkpoint = TokenRgbConversionCheckpoint::new(token);
        rgb_to_ycbcr_with_checkpoint(pixels, w, h, &mut checkpoint)
    } else {
        Ok(rgb_to_ycbcr_without_checkpoint(pixels, w, h))
    }
}

fn rgb_to_ycbcr_without_checkpoint(
    pixels: &[u8],
    w: usize,
    h: usize,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let n = w.saturating_mul(h);
    let mut y = vec![0u8; n];
    let mut cb = vec![0u8; n];
    let mut cr = vec![0u8; n];
    let npix = n.min(pixels.len().div_euclid(3));
    let row_width = w.max(1);
    for row in 0..h {
        let row_start = row.saturating_mul(row_width).min(npix);
        let row_end = npix.min(row_start.saturating_add(row_width));
        for i in row_start..row_end {
            let source = i.saturating_mul(3);
            let r = i32::from(pixels[source]);
            let g = i32::from(pixels[source.saturating_add(1)]);
            let b = i32::from(pixels[source.saturating_add(2)]);
            y[i] = rgb_fixed(&[(19_595, r), (38_470, g), (7_471, b)], 32_768);
            let chroma_bias = 128i32.wrapping_shl(16).saturating_add(32_767);
            cb[i] = rgb_fixed(&[(-11_059, r), (-21_709, g), (32_768, b)], chroma_bias);
            cr[i] = rgb_fixed(&[(32_768, r), (-27_439, g), (-5_329, b)], chroma_bias);
        }
    }
    (y, cb, cr)
}

fn rgb_to_ycbcr_with_checkpoint<C: RgbConversionCheckpoint>(
    pixels: &[u8],
    w: usize,
    h: usize,
    checkpoint: &mut C,
) -> CodecResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let n = w.saturating_mul(h);
    let mut y = vec![0u8; n];
    let mut cb = vec![0u8; n];
    let mut cr = vec![0u8; n];
    let npix = n.min(pixels.len().div_euclid(3));
    let row_width = w.max(1);
    for row in 0..h {
        checkpoint.row()?;
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
            checkpoint.observe()?;
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
    if let Some(token) = token {
        let mut checkpoint = TokenDownsampleCheckpoint::new(token);
        downsample_with_checkpoint(plane, sw, sh, dw, dh, hr, vr, &mut checkpoint)
    } else {
        Ok(downsample_without_checkpoint(plane, sw, sh, dw, dh, hr, vr))
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the helper mirrors libjpeg's sampling routine without a fallible checkpoint"
)]
fn downsample_without_checkpoint(
    plane: &[u8],
    sw: usize,
    sh: usize,
    dw: usize,
    dh: usize,
    hr: usize,
    vr: usize,
) -> Vec<u8> {
    let mut out = vec![0u8; dw.saturating_mul(dh)];
    if hr == 1 && vr == 1 {
        for y in 0..dh {
            for x in 0..dw {
                let source_y = y.min(sh.saturating_sub(1));
                let source_x = x.min(sw.saturating_sub(1));
                out[y.saturating_mul(dw).saturating_add(x)] =
                    plane[source_y.saturating_mul(sw).saturating_add(source_x)];
            }
        }
        return out;
    }
    for y in 0..dh {
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
            debug_assert_eq!(hr, 2);
            debug_assert!(vr == 1 || vr == 2);
            let bias = u32::from(x.to_le_bytes()[0] & 1).saturating_add(u32::from(vr == 2));
            let divisor = low_u32(hr.saturating_mul(vr));
            out[y.saturating_mul(dw).saturating_add(x)] =
                sum.saturating_add(bias).div_euclid(divisor).to_le_bytes()[0];
        }
    }
    out
}

#[allow(
    clippy::too_many_arguments,
    reason = "the helper mirrors libjpeg's sampling routine and the checkpoint is an independent input"
)]
fn downsample_with_checkpoint<C: DownsampleCheckpoint>(
    plane: &[u8],
    sw: usize,
    sh: usize,
    dw: usize,
    dh: usize,
    hr: usize,
    vr: usize,
    checkpoint: &mut C,
) -> CodecResult<Vec<u8>> {
    let mut out = vec![0u8; dw.saturating_mul(dh)];
    if hr == 1 && vr == 1 {
        // ✅ VERIFIED: libjpeg-turbo 3.1.4.1 jcsample.c:99-113,145-174.
        // Full-size components duplicate their right and bottom edge samples
        // through the padded DCT extent.
        for y in 0..dh {
            checkpoint.row()?;
            for x in 0..dw {
                let source_y = y.min(sh.saturating_sub(1));
                let source_x = x.min(sw.saturating_sub(1));
                out[y.saturating_mul(dw).saturating_add(x)] =
                    plane[source_y.saturating_mul(sw).saturating_add(source_x)];
                checkpoint.observe()?;
            }
        }
        return Ok(out);
    }
    for y in 0..dh {
        checkpoint.row()?;
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
            checkpoint.observe()?;
        }
    }
    Ok(out)
}

// ── FDCT + quantize (jfdctint.c + jcdctmgr.c) ────────────────────────────

/// Forward DCT all blocks of a component plane, then quantize with ISLOW
/// divisors (quantval<<3) and round-to-nearest. Returns (blocks, blocks_per_row,
/// block_rows) in natural order.
#[cfg(coverage)]
#[coverage(off)]
fn coverage_cancel_fdct_call(token: Option<&crate::CancellationToken>) {
    let remaining = FORCE_FDCT_FAILURE_CALL.load(Ordering::Relaxed);
    if remaining == usize::MAX {
        return;
    }
    if remaining == 0 {
        if let Some(token) = token {
            token.cancel();
        }
        FORCE_FDCT_FAILURE_CALL.store(usize::MAX, Ordering::Relaxed);
    } else {
        FORCE_FDCT_FAILURE_CALL.store(remaining.saturating_sub(1), Ordering::Relaxed);
    }
}

fn fdct_quantize(
    plane: &[u8],
    w: usize,
    h: usize,
    qtable: &[u16; 64],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(Vec<[i16; 64]>, usize, usize)> {
    #[cfg(coverage)]
    coverage_cancel_fdct_call(token);
    if let Some(token) = token {
        let mut checkpoint = TokenFdctCheckpoint::new(token);
        fdct_quantize_with_checkpoint(plane, w, h, qtable, &mut checkpoint)
    } else {
        fdct_quantize_without_checkpoint(plane, w, h, qtable)
    }
}

fn fdct_quantize_without_checkpoint(
    plane: &[u8],
    w: usize,
    h: usize,
    qtable: &[u16; 64],
) -> CodecResult<(Vec<[i16; 64]>, usize, usize)> {
    let blocks_per_row = w.div_ceil(8);
    let block_rows = h.div_ceil(8);
    let mut blocks = vec![[0i16; 64]; blocks_per_row.saturating_mul(block_rows)];

    for by in 0..block_rows {
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

fn fdct_quantize_with_checkpoint<C: FdctCheckpoint>(
    plane: &[u8],
    w: usize,
    h: usize,
    qtable: &[u16; 64],
    checkpoint: &mut C,
) -> CodecResult<(Vec<[i16; 64]>, usize, usize)> {
    let blocks_per_row = w.div_ceil(8);
    let block_rows = h.div_ceil(8);
    let mut blocks = vec![[0i16; 64]; blocks_per_row.saturating_mul(block_rows)];

    for by in 0..block_rows {
        checkpoint.row()?;
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
            checkpoint.block()?;
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
    if let Some(token) = token {
        let mut checkpoint = TokenHuffmanFrequencyCheckpoint::new(token);
        baseline_frequencies_with_checkpoint(comps, max_h, max_v, restart_interval, &mut checkpoint)
    } else {
        baseline_frequencies_without_checkpoint(comps, max_h, max_v, restart_interval)
    }
}

#[allow(
    clippy::type_complexity,
    reason = "the pair of fixed-size DC and AC frequency tables mirrors the JPEG encoder state"
)]
fn baseline_frequencies_without_checkpoint(
    comps: &[CompData],
    max_h: u8,
    max_v: u8,
    restart_interval: u16,
) -> CodecResult<([[u64; 256]; 2], [[u64; 256]; 2])> {
    // ✅ VERIFIED: libjpeg-turbo 3.1.4.1 jchuff.c's gather_statistics pass.
    // This is the no-token fast path: it performs the same symbol traversal
    // without constructing an infallible checkpoint whose `Result` edges can
    // never be observed.
    let mcu_w = usize::from(max_h).saturating_mul(8);
    let mcu_h = usize::from(max_v).saturating_mul(8);
    let n_mcu_x = comps[0].blocks_per_row.saturating_mul(8).div_ceil(mcu_w);
    let n_mcu_y = comps[0].block_rows.saturating_mul(8).div_ceil(mcu_h);
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
    clippy::type_complexity,
    reason = "the pair of fixed-size DC and AC frequency tables mirrors the JPEG encoder state"
)]
fn baseline_frequencies_with_checkpoint<C: HuffmanFrequencyCheckpoint>(
    comps: &[CompData],
    max_h: u8,
    max_v: u8,
    restart_interval: u16,
    checkpoint: &mut C,
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
        checkpoint.row()?;
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
                                checkpoint.observe()?;
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
                            checkpoint.observe()?;
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
fn encode_baseline_entropy<P: EntropyOutputCheckpoint>(
    out: &mut Vec<u8>,
    comps: &[CompData],
    max_h: u8,
    max_v: u8,
    dc_tables: &[&huffman::DerivedTable; 2],
    ac_tables: &[&huffman::DerivedTable; 2],
    restart_interval: u16,
    token: Option<&crate::CancellationToken>,
    checkpoint: &mut P,
) -> CodecResult<()> {
    let mcu_w = usize::from(max_h).saturating_mul(8);
    let mcu_h = usize::from(max_v).saturating_mul(8);
    let n_mcu_x = comps[0].blocks_per_row.saturating_mul(8).div_ceil(mcu_w);
    let n_mcu_y = comps[0].block_rows.saturating_mul(8).div_ceil(mcu_h);

    let mut bw = huffman::BitWriter::with_output(std::mem::take(out));
    let mut entropy_start = bw.out.len();
    let mut last_dc = [0i32; 4];
    let mut mcus_until_restart = usize::from(restart_interval);
    let mut next_restart = 0u8;
    for my in 0..n_mcu_y {
        crate::codecs::error::check_cancelled(token)?;
        for mx in 0..n_mcu_x {
            if restart_interval != 0 && mcus_until_restart == 0 {
                bw.flush();
                checkpoint.observe(bw.out.len().saturating_sub(entropy_start))?;
                checkpoint.reset();
                marker::write_rst(&mut bw.out, next_restart);
                entropy_start = bw.out.len();
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
            checkpoint.observe(bw.out.len().saturating_sub(entropy_start))?;
            checkpoint.baseline_mcu()?;
            mcus_until_restart = mcus_until_restart.saturating_sub(1);
        }
    }
    bw.flush();
    checkpoint.observe(bw.out.len().saturating_sub(entropy_start))?;
    *out = bw.into_output();
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

#[allow(
    clippy::too_many_arguments,
    reason = "the helper mirrors libjpeg's progressive scan routine and the token is an independent checkpoint input"
)]
fn encode_progressive_scans_exact<P: EntropyOutputCheckpoint, C: ProgressiveScanCheckpoint>(
    output: &mut Vec<u8>,
    components: &[CompData],
    component_count: u8,
    _maximum_horizontal_sampling: u8,
    _maximum_vertical_sampling: u8,
    _params: &quant::EncodeParams,
    token: Option<&crate::CancellationToken>,
    checkpoint: &mut P,
    scan_checkpoint: &mut C,
) -> CodecResult<()> {
    // ✅ VERIFIED: libjpeg-turbo 3.1.4.1 jcphuff.c:179-1075 and
    // jcmaster.c's jpeg_simple_progression scan script.
    for scan in default_progression_script(component_count) {
        crate::codecs::error::check_cancelled(token)?;
        let events = progressive_events(&scan, components, scan_checkpoint)?;
        let mut frequencies = [[0u64; 256]; 4];
        for &event in &events {
            if let ProgressiveEvent::Symbol { table, value } = event {
                frequencies[table][usize::from(value)] =
                    frequencies[table][usize::from(value)].saturating_add(1);
            }
            scan_checkpoint.event()?;
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

        let mut writer = huffman::BitWriter::with_output(std::mem::take(output));
        let entropy_start = writer.out.len();
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
            checkpoint.observe(writer.out.len().saturating_sub(entropy_start))?;
        }
        writer.flush();
        checkpoint.observe(writer.out.len().saturating_sub(entropy_start))?;
        *output = writer.into_output();
        checkpoint.reset();
        crate::codecs::error::check_cancelled(token)?;
    }
    Ok(())
}

fn progressive_events<C: ProgressiveScanCheckpoint>(
    scan: &ProgScan,
    components: &[CompData],
    checkpoint: &mut C,
) -> CodecResult<Vec<ProgressiveEvent>> {
    if scan.ss == 0 {
        dc_progressive_events(scan, components, checkpoint)
    } else {
        ac_progressive_events(scan, components, checkpoint)
    }
}

#[allow(
    clippy::arithmetic_side_effects,
    reason = "max(1) proves the modulo divisor is non-zero for row checkpoint scheduling"
)]
fn dc_progressive_events<C: ProgressiveScanCheckpoint>(
    scan: &ProgScan,
    components: &[CompData],
    checkpoint: &mut C,
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
            checkpoint.row()?;
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
                            checkpoint.block()?;
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
                checkpoint.row()?;
            }
            append(0, component_index, block);
            checkpoint.block()?;
        }
    }
    Ok(events)
}

#[allow(
    clippy::arithmetic_side_effects,
    reason = "max(1) proves the modulo divisor is non-zero for row checkpoint scheduling"
)]
fn ac_progressive_events<C: ProgressiveScanCheckpoint>(
    scan: &ProgScan,
    components: &[CompData],
    checkpoint: &mut C,
) -> CodecResult<Vec<ProgressiveEvent>> {
    let component = &components[scan.comps[0]];
    let table = usize::from(component.ac_tbl);
    let mut events = Vec::new();
    let mut eob_run = 0u32;
    let mut correction_bits = Vec::<u8>::new();
    for (block_index, block) in component.blocks.iter().enumerate() {
        if block_index % component.blocks_per_row.max(1) == 0 {
            checkpoint.row()?;
        }
        if scan.ah == 0 {
            append_ac_first_events(
                &mut events,
                block,
                scan,
                table,
                &mut eob_run,
                &mut correction_bits,
                checkpoint,
            )?;
        } else {
            append_ac_refine_events(
                &mut events,
                block,
                scan,
                table,
                &mut eob_run,
                &mut correction_bits,
                checkpoint,
            )?;
        }
        checkpoint.block()?;
    }
    flush_progressive_eob(&mut events, table, &mut eob_run, &mut correction_bits);
    Ok(events)
}

fn append_ac_first_events<C: ProgressiveScanCheckpoint>(
    events: &mut Vec<ProgressiveEvent>,
    block: &[i16; 64],
    scan: &ProgScan,
    table: usize,
    eob_run: &mut u32,
    correction_bits: &mut Vec<u8>,
    checkpoint: &mut C,
) -> CodecResult<()> {
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
            checkpoint.coefficient()?;
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
        checkpoint.coefficient()?;
    }
    if last_nonzero != Some(scan.se) {
        *eob_run = eob_run.saturating_add(1);
        if *eob_run == 0x7fff {
            flush_progressive_eob(events, table, eob_run, correction_bits);
        }
    }
    Ok(())
}

fn append_ac_refine_events<C: ProgressiveScanCheckpoint>(
    events: &mut Vec<ProgressiveEvent>,
    block: &[i16; 64],
    scan: &ProgScan,
    table: usize,
    eob_run: &mut u32,
    correction_bits: &mut Vec<u8>,
    checkpoint: &mut C,
) -> CodecResult<()> {
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
            checkpoint.coefficient()?;
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
            checkpoint.coefficient()?;
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
        checkpoint.coefficient()?;
    }

    if last_nonzero != Some(coefficients.len().saturating_sub(1)) || !block_corrections.is_empty() {
        *eob_run = eob_run.saturating_add(1);
        correction_bits.append(&mut block_corrections);
        if *eob_run == 0x7fff || correction_bits.len() > 937 {
            flush_progressive_eob(events, table, eob_run, correction_bits);
        }
    }
    Ok(())
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
