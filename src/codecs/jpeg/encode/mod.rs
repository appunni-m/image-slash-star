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
pub(crate) mod huffman;
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
use wide::bytemuck::{cast, pod_read_unaligned};
use wide::{i16x8, i32x4, i32x8, u8x16, u16x8};

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

type PreparedJpegPlanes<'a> = (Cow<'a, [u8]>, Vec<u8>, Vec<u8>, Vec<u8>, bool);

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

/// Per-coefficient integer division metadata shared by every block in one
/// component. JPEG quantization divisors are fixed for an encode, so paying
/// for division once here avoids repeating it for every transformed value.
struct FdctQuantizer {
    divisors: [u32; 64],
    reciprocals: [u32; 64],
}

impl FdctQuantizer {
    fn new(qtable: &[u16; 64]) -> Self {
        let mut divisors = [0u32; 64];
        let mut reciprocals = [0u32; 64];
        for coefficient in 0usize..64 {
            let divisor = u32::from(qtable[coefficient]).saturating_mul(8);
            divisors[coefficient] = divisor;
            reciprocals[coefficient] = reciprocal_divisor(divisor);
        }
        Self {
            divisors,
            reciprocals,
        }
    }
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
    let y_quantizer = FdctQuantizer::new(&params.quant_tables[0]);
    let chroma_quantizer =
        (num_components == 3).then(|| FdctQuantizer::new(&params.quant_tables[1]));

    // The common baseline 4:2:0 path converts one MCU row directly into its
    // packed FDCT/entropy pipeline. This keeps large RGB inputs from making
    // and rereading three whole-image component planes.
    if token.is_none()
        && num_components == 3
        && !progressive
        && !optimize
        && subsampling == "420"
        && quality > 25
        && w != 0
        && h != 0
        && w.saturating_mul(h) >= 1024
    {
        let chroma_quantizer = chroma_quantizer
            .as_ref()
            .ok_or_else(|| CodecError::Malformed("missing JPEG chroma quantizer".to_owned()))?;
        return encode_baseline_420_mcu_row_streaming(
            Baseline420Source::Rgb(pixels),
            w,
            h,
            &params,
            &y_quantizer,
            chroma_quantizer,
            opts,
        );
    }

    // RGB → YCbCr (jccolor.c), Adobe CMYK inversion, or grayscale
    // pass-through. Pillow/libjpeg writes CMYK JPEG samples as 255 - CMYK and
    // advertises transform 0 in APP14; the decoder reverses that convention.
    let (y_plane, mut cb_plane, mut cr_plane, k_plane, chroma_already_downsampled):
        PreparedJpegPlanes<'_> = if num_components == 1 {
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
            (
                Cow::Owned(expanded),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                false,
            )
        } else {
            let pixel_count = w.saturating_mul(h);
            let copied = pixel_count.min(pixels.len());
            let row_width = w.max(1);
            for _row_start in (0..copied).step_by(row_width) {
                crate::codecs::error::check_cancelled(token)?;
            }
            (
                Cow::Borrowed(pixels),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                false,
            )
        }
    } else if num_components == 3 {
        if token.is_none() && subsampling == "420" {
            let (y, cb, cr) = crate::codecs::jpeg::kernels::rgb_to_ycbcr_420_batch(pixels, w, h);
            (Cow::Owned(y), cb, cr, Vec::new(), true)
        } else {
            let (y, cb, cr) = rgb_to_ycbcr(pixels, w, h, token)?;
            (Cow::Owned(y), cb, cr, Vec::new(), false)
        }
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
        (Cow::Owned(c_plane), m_plane, y_plane, k_plane, false)
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
    let cb_ds = if chroma_already_downsampled || (num_components == 3 && cb_w == w && cb_h == h) {
        std::mem::take(&mut cb_plane)
    } else if num_components == 3 {
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
    let cr_ds = if chroma_already_downsampled || (num_components == 3 && cr_w == w && cr_h == h) {
        std::mem::take(&mut cr_plane)
    } else if num_components == 3 {
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

    // Baseline CMYK keeps four adjacent blocks per component in safe packed
    // FDCT form, then writes one C/M/Y/K block packet per MCU.
    if token.is_none()
        && num_components == 4
        && !progressive
        && !optimize
        && w != 0
        && h != 0
        && (w.saturating_mul(h) >= 1024 || (w.is_multiple_of(32) && h.is_multiple_of(8)))
        && y_plane.len() == w.saturating_mul(h)
        && cb_plane.len() == y_plane.len()
        && cr_plane.len() == y_plane.len()
        && k_plane.len() == y_plane.len()
    {
        return encode_baseline_cmyk_block_row_streaming(
            &y_plane,
            &cb_plane,
            &cr_plane,
            &k_plane,
            w,
            h,
            &params,
            &y_quantizer,
            opts,
        );
    }

    // Baseline grayscale streams four blocks from the safe packed FDCT into
    // independent entropy reservoirs without materializing a coefficient
    // image first. Tiny unaligned images keep the simpler generic path.
    if token.is_none()
        && num_components == 1
        && !progressive
        && !optimize
        && w != 0
        && h != 0
        && (w.saturating_mul(h) >= 1024 || (w.is_multiple_of(32) && h.is_multiple_of(8)))
        && y_plane.len() == w.saturating_mul(h)
    {
        return encode_baseline_grayscale_block_row_streaming(
            &y_plane,
            w,
            h,
            &params,
            &y_quantizer,
            opts,
        );
    }

    // Baseline 4:2:2 can stream four MCUs at a time from safe SIMD
    // FDCT packets into entropy output, avoiding whole-image coefficient
    // planes and their coefficient-major-to-block-major transposes.
    if token.is_none()
        && num_components == 3
        && !progressive
        && !optimize
        && subsampling == "422"
        && w != 0
        && h != 0
        && (w.saturating_mul(h) >= 1024 || (w.is_multiple_of(64) && h.is_multiple_of(8)))
        && y_plane.len() == w.saturating_mul(h)
        && cb_w == w.div_ceil(16).saturating_mul(8)
        && cb_h == h
        && cr_w == cb_w
        && cr_h == cb_h
        && cb_ds.len() == cb_w.saturating_mul(cb_h)
        && cr_ds.len() == cr_w.saturating_mul(cr_h)
    {
        let chroma_quantizer = chroma_quantizer
            .as_ref()
            .ok_or_else(|| CodecError::Malformed("missing JPEG chroma quantizer".to_owned()))?;
        return encode_baseline_422_block_row_streaming(
            &y_plane,
            &cb_ds,
            &cr_ds,
            w,
            h,
            cb_w,
            cb_h,
            &params,
            &y_quantizer,
            chroma_quantizer,
            opts,
        );
    }

    // Baseline 4:4:4 can keep each four-block SIMD packet in its
    // native coefficient-major layout until entropy output. This avoids
    // materializing and transposing three whole-image coefficient planes.
    if token.is_none()
        && num_components == 3
        && !progressive
        && !optimize
        && subsampling == "444"
        && w != 0
        && h != 0
        && (w.saturating_mul(h) >= 1024 || (w.is_multiple_of(32) && h.is_multiple_of(8)))
        && y_plane.len() == w.saturating_mul(h)
        && cb_w == w
        && cb_h == h
        && cr_w == w
        && cr_h == h
        && cb_ds.len() == w.saturating_mul(h)
        && cr_ds.len() == w.saturating_mul(h)
    {
        let chroma_quantizer = chroma_quantizer
            .as_ref()
            .ok_or_else(|| CodecError::Malformed("missing JPEG chroma quantizer".to_owned()))?;
        return encode_baseline_444_block_row_streaming(
            &y_plane,
            &cb_ds,
            &cr_ds,
            w,
            h,
            &params,
            &y_quantizer,
            chroma_quantizer,
            opts,
        );
    }

    // Sparse low-quality 4:2:0 data is faster after the fused whole-plane
    // conversion, while retaining the same packed transform/entropy backend.
    if token.is_none()
        && num_components == 3
        && !progressive
        && !optimize
        && subsampling == "420"
        && quality <= 25
        && w != 0
        && h != 0
        && w.saturating_mul(h) >= 1024
        && y_plane.len() == w.saturating_mul(h)
        && cb_w == w.div_ceil(16).saturating_mul(8)
        && cb_h == h.div_ceil(2)
        && cr_w == cb_w
        && cr_h == cb_h
        && cb_ds.len() == cb_w.saturating_mul(cb_h)
        && cr_ds.len() == cr_w.saturating_mul(cr_h)
    {
        let chroma_quantizer = chroma_quantizer
            .as_ref()
            .ok_or_else(|| CodecError::Malformed("missing JPEG chroma quantizer".to_owned()))?;
        return encode_baseline_420_mcu_row_streaming(
            Baseline420Source::Planes {
                y: &y_plane,
                cb: &cb_ds,
                cr: &cr_ds,
                chroma_width: cb_w,
                chroma_height: cb_h,
            },
            w,
            h,
            &params,
            &y_quantizer,
            chroma_quantizer,
            opts,
        );
    }

    // Prepare per-component quantized coefficient blocks (natural order).
    let mut comps: Vec<CompData> = Vec::with_capacity(usize::from(num_components));

    // Y
    let y_blocks = fdct_quantize(&y_plane, y_w, y_h, &y_quantizer, token)?;
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
            let quantizer = chroma_quantizer
                .as_ref()
                .ok_or_else(|| CodecError::Malformed("missing JPEG chroma quantizer".to_owned()))?;
            let blk = fdct_quantize(plane, cw, ch, quantizer, token)?;
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
            let blk = fdct_quantize(plane, w, h, &y_quantizer, token)?;
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
        let blk = fdct_quantize(&k_plane, w, h, &y_quantizer, token)?;
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
    let standard_tables = huffman::standard_derived_tables();
    let dc_luma = &standard_tables[0];
    let dc_chroma = &standard_tables[1];
    let ac_luma = &standard_tables[2];
    let ac_chroma = &standard_tables[3];
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
            .map_or(dc_luma, |table| &table.derived),
        optimized_dc[1]
            .as_ref()
            .map_or(dc_chroma, |table| &table.derived),
    ];
    let ac_tables = [
        optimized_ac[0]
            .as_ref()
            .map_or(ac_luma, |table| &table.derived),
        optimized_ac[1]
            .as_ref()
            .map_or(ac_chroma, |table| &table.derived),
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
        } else if !optimize
            && restart_interval == 0
            && baseline_420_independent_entropy_is_compatible(&comps, max_h, max_v)
        {
            encode_baseline_420_independent_entropy(&mut out, &comps, &dc_tables, &ac_tables);
        } else if !optimize
            && restart_interval == 0
            && baseline_422_independent_entropy_is_compatible(&comps, max_h, max_v)
        {
            encode_baseline_422_independent_entropy(&mut out, &comps, &dc_tables, &ac_tables);
        } else if !optimize
            && restart_interval == 0
            && baseline_444_independent_entropy_is_compatible(&comps, max_h, max_v)
        {
            encode_baseline_444_independent_entropy(&mut out, &comps, &dc_tables, &ac_tables);
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
                            bw.write_bounded_bits(dc_tbl.codes[0], dc_tbl.lengths[0]);
                            bw.write_bounded_bits(ac_tbl.codes[0], ac_tbl.lengths[0]);
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
    fdct::__coverage_exercise_private_branches();

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
    let small_rgb = DecodedImage::new(
        32,
        16,
        (0usize..(32 * 16 * 3))
            .map(|index| index.to_le_bytes()[0].wrapping_mul(29))
            .collect(),
        crate::types::ColorType::Rgb8,
    );
    for subsampling in [
        JpegSubsampling::Cs420,
        JpegSubsampling::Cs422,
        JpegSubsampling::Cs444,
    ] {
        let options = JpegEncodeOptions {
            subsampling: Some(subsampling),
            ..JpegEncodeOptions::default()
        };
        let _ = encode(&small_rgb, &options);
    }
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
    let large_rgb = DecodedImage::new(
        33,
        35,
        vec![128; 33 * 35 * 3],
        crate::types::ColorType::Rgb8,
    );
    for subsampling in [JpegSubsampling::Cs422, JpegSubsampling::Cs444] {
        let mut streaming = JpegEncodeOptions {
            subsampling: Some(subsampling),
            restart_interval: Some(1),
            exif: Some(vec![0x45, 0x78, 0x69, 0x66, 0]),
            ..JpegEncodeOptions::default()
        };
        let _ = encode(&large_rgb, &streaming);
        streaming.restart_interval = Some(70_000);
        let _ = encode(&large_rgb, &streaming);
    }
    let large_gray = DecodedImage::new(33, 35, vec![128; 33 * 35], crate::types::ColorType::L8);
    let mut large_gray_options = JpegEncodeOptions {
        restart_interval: Some(1),
        exif: Some(vec![0x45, 0x78, 0x69, 0x66, 0]),
        ..JpegEncodeOptions::default()
    };
    let _ = encode(&large_gray, &large_gray_options);
    large_gray_options.restart_interval = Some(70_000);
    let _ = encode(&large_gray, &large_gray_options);
    let large_cmyk = DecodedImage::new(
        33,
        35,
        vec![128; 33 * 35 * 4],
        crate::types::ColorType::Cmyk8,
    );
    let mut large_cmyk_options = JpegEncodeOptions {
        restart_interval: Some(1),
        exif: Some(vec![0x45, 0x78, 0x69, 0x66, 0]),
        ..JpegEncodeOptions::default()
    };
    let _ = encode(&large_cmyk, &large_cmyk_options);
    large_cmyk_options.restart_interval = Some(70_000);
    let _ = encode(&large_cmyk, &large_cmyk_options);
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
    let wide_plane = vec![10u8; 32 * 2];
    let _ = downsample(&wide_plane, 32, 2, 16, 1, 2, 1, None);
    let _ = downsample(&wide_plane, 32, 2, 16, 1, 2, 2, None);
    let _ = downsample(&[], 0, 0, 0, 0, 1, 1, None);
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
    crate::codecs::jpeg::kernels::rgb_to_ycbcr_batch(pixels, w, h)
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
            // ✅ VERIFIED: libjpeg-turbo 3.1.4.1 jccolor.c:214-243 and
            // jccolext.c:37-73. Chroma includes CENTERJSAMPLE before descaling;
            // the prior port accidentally added 128 before, rather than after,
            // the 16-bit fixed-point scale.
            let (y_sample, cb_sample, cr_sample) = crate::codecs::jpeg::kernels::rgb_to_ycbcr_pixel(
                pixels[source],
                pixels[source.saturating_add(1)],
                pixels[source.saturating_add(2)],
            );
            y[i] = y_sample;
            cb[i] = cb_sample;
            cr[i] = cr_sample;
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
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 {
        return vec![0u8; dw.saturating_mul(dh)];
    }

    if hr == 1 && vr == 1 {
        let mut out = Vec::with_capacity(dw.saturating_mul(dh));
        for y in 0..dh {
            let source_y = y.min(sh.saturating_sub(1));
            for x in 0..dw {
                let source_x = x.min(sw.saturating_sub(1));
                out.push(plane[source_y.saturating_mul(sw).saturating_add(source_x)]);
            }
        }
        return out;
    }

    if hr == 2 && (vr == 1 || vr == 2) {
        return downsample_two_to_one(plane, sw, sh, dw, dh, vr);
    }

    let mut out = vec![0u8; dw.saturating_mul(dh)];
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
    clippy::arithmetic_side_effects,
    reason = "validated JPEG dimensions bound two-row sums and source coordinates"
)]
#[inline]
fn downsample_two_to_one(
    plane: &[u8],
    source_width: usize,
    source_height: usize,
    destination_width: usize,
    destination_height: usize,
    vertical_rows: usize,
) -> Vec<u8> {
    debug_assert!(vertical_rows == 1 || vertical_rows == 2);
    let mut output = Vec::with_capacity(destination_width.saturating_mul(destination_height));
    for y in 0usize..destination_height {
        let source_y = (y * vertical_rows).min(source_height - 1);
        let next_y = (source_y + usize::from(vertical_rows == 2)).min(source_height - 1);
        let row0 = source_y * source_width;
        let row1 = next_y * source_width;
        let mut x = 0usize;
        while x + 8 <= destination_width && x * 2 + 16 <= source_width {
            let source_x = x * 2;
            let source0: &[u8; 16] = plane[row0 + source_x..row0 + source_x + 16]
                .try_into()
                .unwrap_or_else(|_| unreachable!("validated h2 source batch is not 16 bytes"));
            let averaged = if vertical_rows == 2 {
                let source1: &[u8; 16] = plane[row1 + source_x..row1 + source_x + 16]
                    .try_into()
                    .unwrap_or_else(|_| {
                        unreachable!("validated h2v2 source batch is not 16 bytes")
                    });
                crate::codecs::jpeg::kernels::downsample_h2v2_eight(source0, source1)
            } else {
                crate::codecs::jpeg::kernels::downsample_h2v1_eight(source0)
            };
            output.extend_from_slice(&averaged);
            x += 8;
        }
        while x < destination_width {
            let source_x = (x * 2).min(source_width - 1);
            let next_x = (source_x + 1).min(source_width - 1);
            let mut sum = u32::from(plane[row0 + source_x]) + u32::from(plane[row0 + next_x]);
            if vertical_rows == 2 {
                sum += u32::from(plane[row1 + source_x]) + u32::from(plane[row1 + next_x]);
            }
            let bias = u32::from(x.to_le_bytes()[0] & 1) + u32::from(vertical_rows == 2);
            let shift = u32::from(vertical_rows == 2) + 1;
            output.push(((sum + bias) >> shift).to_le_bytes()[0]);
            x += 1;
        }
    }
    output
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

/// Encode baseline Adobe CMYK as four packed component block rows.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::too_many_arguments,
    reason = "four validated component planes plus quantizer and marker options are explicit inputs"
)]
fn encode_baseline_cmyk_block_row_streaming(
    c_plane: &[u8],
    m_plane: &[u8],
    y_plane: &[u8],
    k_plane: &[u8],
    width: usize,
    height: usize,
    params: &quant::EncodeParams,
    quantizer: &FdctQuantizer,
    options: &JpegEncodeOptions,
) -> CodecResult<Vec<u8>> {
    debug_assert!(width != 0);
    debug_assert!(height != 0);
    let pixel_count = width.saturating_mul(height);
    debug_assert_eq!(c_plane.len(), pixel_count);
    debug_assert_eq!(m_plane.len(), pixel_count);
    debug_assert_eq!(y_plane.len(), pixel_count);
    debug_assert_eq!(k_plane.len(), pixel_count);
    debug_assert_eq!(params.quant_tables.len(), 1);

    let mcu_columns = width.div_ceil(8);
    let restart_rows = options.restart_interval.unwrap_or(0);
    let restart_interval = if restart_rows == 0 {
        0
    } else {
        let interval = usize::try_from(restart_rows)
            .unwrap_or(usize::MAX)
            .saturating_mul(mcu_columns);
        u16::try_from(interval).map_err(|_| {
            CodecError::Parameter("JPEG restart interval exceeds 65535 MCUs".to_owned())
        })?
    };

    let standard_tables = huffman::standard_derived_tables();
    let luma_dc = &standard_tables[0];
    let luma_ac = &standard_tables[2];
    let luma_ready = huffman::standard_ac_luma_coefficient_ready();

    let mut output = Vec::with_capacity(
        width
            .saturating_mul(height)
            .saturating_mul(2)
            .saturating_add(512),
    );
    marker::write_soi(&mut output);
    marker::write_adobe_app14(&mut output);
    if let Some(exif) = options.exif.as_deref() {
        marker::write_exif_app1(&mut output, exif)?;
    }
    marker::write_dqt(&mut output, 0, &params.quant_tables[0]);
    marker::write_sof(
        &mut output,
        0xC0,
        low_u16(width),
        low_u16(height),
        &[
            (b'C', 1, 1, 0),
            (b'M', 1, 1, 0),
            (b'Y', 1, 1, 0),
            (b'K', 1, 1, 0),
        ],
    );
    marker::write_dht(
        &mut output,
        0,
        0,
        &huffman::STD_DC_LUMA.0,
        &huffman::STD_DC_LUMA.1,
    );
    marker::write_dht(
        &mut output,
        1,
        0,
        &huffman::STD_AC_LUMA.0,
        &huffman::STD_AC_LUMA.1,
    );
    if restart_interval != 0 {
        marker::write_dri(&mut output, restart_interval);
    }
    marker::write_sos(
        &mut output,
        &[(b'C', 0, 0), (b'M', 0, 0), (b'Y', 0, 0), (b'K', 0, 0)],
        0,
        63,
        0,
        0,
    );

    let mcu_rows = height.div_ceil(8);
    let mcu_groups = mcu_columns.div_ceil(4);
    let mut coefficients = [[0i16; 256]; 4];
    let mut samples = [i32x4::ZERO; 64];
    let mut writers = [RawBlockWriter::new(); 4];
    let mut scan = RawScanWriter::new(&mut output);
    let mut previous_dc = [0i32; 4];
    let mut mcus_until_restart = usize::from(restart_interval);
    let mut next_restart = 0u8;

    for mcu_y in 0usize..mcu_rows {
        for mcu_group in 0usize..mcu_groups {
            if restart_interval != 0 && mcus_until_restart == 0 {
                scan.finish();
                marker::write_rst(&mut output, next_restart);
                next_restart = next_restart.saturating_add(1) & 7;
                scan = RawScanWriter::new(&mut output);
                previous_dc = [0; 4];
                mcus_until_restart = usize::from(restart_interval);
            }

            let first_mcu = mcu_group.saturating_mul(4);
            for (plane, coefficient_group) in [c_plane, m_plane, y_plane, k_plane]
                .into_iter()
                .zip(&mut coefficients)
            {
                load_fdct_samples_four_into(plane, width, height, first_mcu, mcu_y, &mut samples);
                fdct_quantize_four_coefficient_major(&mut samples, quantizer, coefficient_group);
            }

            let group_mcus = mcu_columns.saturating_sub(first_mcu).min(4);
            for lane in 0usize..group_mcus {
                let blocks = [
                    CoefficientMajorBlock {
                        group: &coefficients[0],
                        lane,
                    },
                    CoefficientMajorBlock {
                        group: &coefficients[1],
                        lane,
                    },
                    CoefficientMajorBlock {
                        group: &coefficients[2],
                        lane,
                    },
                    CoefficientMajorBlock {
                        group: &coefficients[3],
                        lane,
                    },
                ];
                let dc = blocks.map(|block| block.coefficient(0));
                encode_four_coefficient_major_raw_blocks(
                    &mut writers,
                    blocks,
                    [
                        dc[0] - previous_dc[0],
                        dc[1] - previous_dc[1],
                        dc[2] - previous_dc[2],
                        dc[3] - previous_dc[3],
                    ],
                    luma_dc,
                    luma_ac,
                    luma_dc,
                    luma_ac,
                    luma_ready,
                    luma_ready,
                );
                for writer in &writers {
                    writer.append_to(&mut scan);
                }
                previous_dc = dc;
                mcus_until_restart = mcus_until_restart.saturating_sub(1);
            }
        }
    }
    scan.finish();
    marker::write_eoi(&mut output);
    Ok(output)
}

/// Encode baseline grayscale as a four-block transform/entropy pipeline.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::too_many_arguments,
    reason = "validated planes, quantizer, and marker options are explicit inputs"
)]
fn encode_baseline_grayscale_block_row_streaming(
    plane: &[u8],
    width: usize,
    height: usize,
    params: &quant::EncodeParams,
    quantizer: &FdctQuantizer,
    options: &JpegEncodeOptions,
) -> CodecResult<Vec<u8>> {
    debug_assert!(width != 0);
    debug_assert!(height != 0);
    debug_assert_eq!(plane.len(), width.saturating_mul(height));
    debug_assert_eq!(params.quant_tables.len(), 1);

    let block_columns = width.div_ceil(8);
    let restart_rows = options.restart_interval.unwrap_or(0);
    let restart_interval = if restart_rows == 0 {
        0
    } else {
        let interval = usize::try_from(restart_rows)
            .unwrap_or(usize::MAX)
            .saturating_mul(block_columns);
        u16::try_from(interval).map_err(|_| {
            CodecError::Parameter("JPEG restart interval exceeds 65535 MCUs".to_owned())
        })?
    };

    let standard_tables = huffman::standard_derived_tables();
    let luma_dc = &standard_tables[0];
    let luma_ac = &standard_tables[2];
    let luma_ready = huffman::standard_ac_luma_coefficient_ready();

    let mut output = Vec::with_capacity(width.saturating_mul(height).saturating_add(512));
    marker::write_soi(&mut output);
    marker::write_jfif_app0(&mut output);
    if let Some(exif) = options.exif.as_deref() {
        marker::write_exif_app1(&mut output, exif)?;
    }
    marker::write_dqt(&mut output, 0, &params.quant_tables[0]);
    marker::write_sof(
        &mut output,
        0xC0,
        low_u16(width),
        low_u16(height),
        &[(1, 1, 1, 0)],
    );
    marker::write_dht(
        &mut output,
        0,
        0,
        &huffman::STD_DC_LUMA.0,
        &huffman::STD_DC_LUMA.1,
    );
    marker::write_dht(
        &mut output,
        1,
        0,
        &huffman::STD_AC_LUMA.0,
        &huffman::STD_AC_LUMA.1,
    );
    if restart_interval != 0 {
        marker::write_dri(&mut output, restart_interval);
    }
    marker::write_sos(&mut output, &[(1, 0, 0)], 0, 63, 0, 0);

    let block_rows = height.div_ceil(8);
    let block_groups = block_columns.div_ceil(4);
    let mut coefficients = [0i16; 256];
    let mut samples = [i32x4::ZERO; 64];
    let mut writers = [RawBlockWriter::new(); 4];
    let mut scan = RawScanWriter::new(&mut output);
    let mut previous_dc = 0i32;
    let mut mcus_until_restart = usize::from(restart_interval);
    let mut next_restart = 0u8;

    for block_y in 0usize..block_rows {
        for block_group in 0usize..block_groups {
            if restart_interval != 0 && mcus_until_restart == 0 {
                scan.finish();
                marker::write_rst(&mut output, next_restart);
                next_restart = next_restart.saturating_add(1) & 7;
                scan = RawScanWriter::new(&mut output);
                previous_dc = 0;
                mcus_until_restart = usize::from(restart_interval);
            }

            let first_block = block_group.saturating_mul(4);
            load_fdct_samples_four_into(plane, width, height, first_block, block_y, &mut samples);
            fdct_quantize_four_coefficient_major(&mut samples, quantizer, &mut coefficients);
            let blocks: [CoefficientMajorBlock<'_>; 4] =
                std::array::from_fn(|lane| CoefficientMajorBlock {
                    group: &coefficients,
                    lane,
                });
            let dc = blocks.map(|block| block.coefficient(0));
            let differences = [
                dc[0] - previous_dc,
                dc[1] - dc[0],
                dc[2] - dc[1],
                dc[3] - dc[2],
            ];
            encode_four_coefficient_major_raw_blocks(
                &mut writers,
                blocks,
                differences,
                luma_dc,
                luma_ac,
                luma_dc,
                luma_ac,
                luma_ready,
                luma_ready,
            );
            let group_blocks = block_columns.saturating_sub(first_block).min(4);
            for writer in &writers[..group_blocks] {
                writer.append_to(&mut scan);
            }
            previous_dc = dc[group_blocks.saturating_sub(1)];
            mcus_until_restart = mcus_until_restart.saturating_sub(group_blocks);
        }
    }
    scan.finish();
    marker::write_eoi(&mut output);
    Ok(output)
}

/// Encode baseline RGB 4:2:2 as a block-row transform/entropy
/// pipeline. Two four-block luma packets and one packet per chroma component
/// form four MCUs without whole-image coefficient storage or transposes.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::too_many_arguments,
    reason = "validated planes, quantizers, and marker options are explicit inputs"
)]
fn encode_baseline_422_block_row_streaming(
    y_plane: &[u8],
    cb_plane: &[u8],
    cr_plane: &[u8],
    width: usize,
    height: usize,
    chroma_width: usize,
    chroma_height: usize,
    params: &quant::EncodeParams,
    y_quantizer: &FdctQuantizer,
    chroma_quantizer: &FdctQuantizer,
    options: &JpegEncodeOptions,
) -> CodecResult<Vec<u8>> {
    debug_assert!(width != 0);
    debug_assert!(height != 0);
    debug_assert_eq!(chroma_width, width.div_ceil(16).saturating_mul(8));
    debug_assert_eq!(chroma_height, height);
    debug_assert_eq!(params.quant_tables.len(), 2);

    let mcu_columns = width.div_ceil(16);
    let restart_rows = options.restart_interval.unwrap_or(0);
    let restart_interval = if restart_rows == 0 {
        0
    } else {
        let interval = usize::try_from(restart_rows)
            .unwrap_or(usize::MAX)
            .saturating_mul(mcu_columns);
        u16::try_from(interval).map_err(|_| {
            CodecError::Parameter("JPEG restart interval exceeds 65535 MCUs".to_owned())
        })?
    };

    let standard_tables = huffman::standard_derived_tables();
    let luma_dc = &standard_tables[0];
    let chroma_dc = &standard_tables[1];
    let luma_ac = &standard_tables[2];
    let chroma_ac = &standard_tables[3];

    let mut output = Vec::with_capacity(width.saturating_mul(height).saturating_add(1024));
    marker::write_soi(&mut output);
    marker::write_jfif_app0(&mut output);
    if let Some(exif) = options.exif.as_deref() {
        marker::write_exif_app1(&mut output, exif)?;
    }
    marker::write_dqt(&mut output, 0, &params.quant_tables[0]);
    marker::write_dqt(&mut output, 1, &params.quant_tables[1]);
    marker::write_sof(
        &mut output,
        0xC0,
        low_u16(width),
        low_u16(height),
        &[(1, 2, 1, 0), (2, 1, 1, 1), (3, 1, 1, 1)],
    );
    marker::write_dht(
        &mut output,
        0,
        0,
        &huffman::STD_DC_LUMA.0,
        &huffman::STD_DC_LUMA.1,
    );
    marker::write_dht(
        &mut output,
        1,
        0,
        &huffman::STD_AC_LUMA.0,
        &huffman::STD_AC_LUMA.1,
    );
    marker::write_dht(
        &mut output,
        0,
        1,
        &huffman::STD_DC_CHROMA.0,
        &huffman::STD_DC_CHROMA.1,
    );
    marker::write_dht(
        &mut output,
        1,
        1,
        &huffman::STD_AC_CHROMA.0,
        &huffman::STD_AC_CHROMA.1,
    );
    if restart_interval != 0 {
        marker::write_dri(&mut output, restart_interval);
    }
    marker::write_sos(&mut output, &[(1, 0, 0), (2, 1, 1), (3, 1, 1)], 0, 63, 0, 0);

    let mcu_rows = height.div_ceil(8);
    let y_block_columns = width.div_ceil(8);
    let mcu_groups_per_row = mcu_columns.div_ceil(4);
    let mut coefficient_groups = [[0i16; 256]; 4];
    let mut samples = [i32x4::ZERO; 64];
    let luma_ready = huffman::standard_ac_luma_coefficient_ready();
    let chroma_ready = huffman::standard_ac_chroma_coefficient_ready();
    let mut mcu_writers = [RawBlockWriter::new(); 4];
    let mut scan = RawScanWriter::new(&mut output);
    let mut previous_dc = [0i32; 3];
    let mut mcus_until_restart = usize::from(restart_interval);
    let mut next_restart = 0u8;

    for mcu_y in 0usize..mcu_rows {
        for mcu_group in 0usize..mcu_groups_per_row {
            let y_block_x = mcu_group.saturating_mul(8);
            load_fdct_samples_four_into(y_plane, width, height, y_block_x, mcu_y, &mut samples);
            fdct_quantize_four_coefficient_major(
                &mut samples,
                y_quantizer,
                &mut coefficient_groups[0],
            );
            load_fdct_samples_four_into(
                y_plane,
                width,
                height,
                y_block_x.saturating_add(4),
                mcu_y,
                &mut samples,
            );
            fdct_quantize_four_coefficient_major(
                &mut samples,
                y_quantizer,
                &mut coefficient_groups[1],
            );

            let chroma_block_x = mcu_group.saturating_mul(4);
            load_fdct_samples_four_into(
                cb_plane,
                chroma_width,
                chroma_height,
                chroma_block_x,
                mcu_y,
                &mut samples,
            );
            fdct_quantize_four_coefficient_major(
                &mut samples,
                chroma_quantizer,
                &mut coefficient_groups[2],
            );
            load_fdct_samples_four_into(
                cr_plane,
                chroma_width,
                chroma_height,
                chroma_block_x,
                mcu_y,
                &mut samples,
            );
            fdct_quantize_four_coefficient_major(
                &mut samples,
                chroma_quantizer,
                &mut coefficient_groups[3],
            );

            let first_mcu_x = mcu_group.saturating_mul(4);
            let group_mcus = mcu_columns.saturating_sub(first_mcu_x).min(4);
            for mcu_lane in 0usize..group_mcus {
                if restart_interval != 0 && mcus_until_restart == 0 {
                    scan.finish();
                    marker::write_rst(&mut output, next_restart);
                    next_restart = next_restart.saturating_add(1) & 7;
                    scan = RawScanWriter::new(&mut output);
                    previous_dc = [0; 3];
                    mcus_until_restart = usize::from(restart_interval);
                }
                let y_group = mcu_lane / 2;
                let y_lane = (mcu_lane % 2).saturating_mul(2);
                let blocks = [
                    CoefficientMajorBlock {
                        group: &coefficient_groups[y_group],
                        lane: y_lane,
                    },
                    CoefficientMajorBlock {
                        group: &coefficient_groups[y_group],
                        lane: y_lane.saturating_add(1),
                    },
                    CoefficientMajorBlock {
                        group: &coefficient_groups[2],
                        lane: mcu_lane,
                    },
                    CoefficientMajorBlock {
                        group: &coefficient_groups[3],
                        lane: mcu_lane,
                    },
                ];
                let dc = blocks.map(|block| block.coefficient(0));
                let second_luma_present = first_mcu_x
                    .saturating_add(mcu_lane)
                    .saturating_mul(2)
                    .saturating_add(1)
                    < y_block_columns;
                let differences = [
                    dc[0] - previous_dc[0],
                    if second_luma_present {
                        dc[1] - dc[0]
                    } else {
                        0
                    },
                    dc[2] - previous_dc[1],
                    dc[3] - previous_dc[2],
                ];
                if second_luma_present {
                    encode_four_coefficient_major_raw_blocks(
                        &mut mcu_writers,
                        blocks,
                        differences,
                        luma_dc,
                        luma_ac,
                        chroma_dc,
                        chroma_ac,
                        luma_ready,
                        chroma_ready,
                    );
                } else {
                    encode_four_coefficient_major_edge_raw_blocks(
                        &mut mcu_writers,
                        blocks,
                        differences,
                        luma_dc,
                        luma_ac,
                        chroma_dc,
                        chroma_ac,
                        luma_ready,
                        chroma_ready,
                    );
                }
                for writer in &mcu_writers {
                    writer.append_to(&mut scan);
                }
                previous_dc = [
                    if second_luma_present { dc[1] } else { dc[0] },
                    dc[2],
                    dc[3],
                ];
                mcus_until_restart = mcus_until_restart.saturating_sub(1);
            }
        }
    }
    scan.finish();
    marker::write_eoi(&mut output);
    Ok(output)
}

/// Encode baseline RGB 4:4:4 as a block-row transform/entropy
/// pipeline. Four blocks from each component remain coefficient-major from
/// the safe SIMD FDCT through entropy coding, so no whole-image coefficient
/// planes or coefficient-major-to-block-major transposes are needed.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::too_many_arguments,
    reason = "validated planes, quantizers, and marker options are explicit inputs"
)]
fn encode_baseline_444_block_row_streaming(
    y_plane: &[u8],
    cb_plane: &[u8],
    cr_plane: &[u8],
    width: usize,
    height: usize,
    params: &quant::EncodeParams,
    y_quantizer: &FdctQuantizer,
    chroma_quantizer: &FdctQuantizer,
    options: &JpegEncodeOptions,
) -> CodecResult<Vec<u8>> {
    debug_assert!(width != 0);
    debug_assert!(height != 0);
    debug_assert_eq!(y_plane.len(), width.saturating_mul(height));
    debug_assert_eq!(cb_plane.len(), y_plane.len());
    debug_assert_eq!(cr_plane.len(), y_plane.len());
    debug_assert_eq!(params.quant_tables.len(), 2);

    let mcu_columns = width.div_ceil(8);
    let restart_rows = options.restart_interval.unwrap_or(0);
    let restart_interval = if restart_rows == 0 {
        0
    } else {
        let interval = usize::try_from(restart_rows)
            .unwrap_or(usize::MAX)
            .saturating_mul(mcu_columns);
        u16::try_from(interval).map_err(|_| {
            CodecError::Parameter("JPEG restart interval exceeds 65535 MCUs".to_owned())
        })?
    };

    let standard_tables = huffman::standard_derived_tables();
    let luma_dc = &standard_tables[0];
    let chroma_dc = &standard_tables[1];
    let luma_ac = &standard_tables[2];
    let chroma_ac = &standard_tables[3];

    let mut output = Vec::with_capacity(
        width
            .saturating_mul(height)
            .saturating_mul(2)
            .saturating_add(1024),
    );
    marker::write_soi(&mut output);
    marker::write_jfif_app0(&mut output);
    if let Some(exif) = options.exif.as_deref() {
        marker::write_exif_app1(&mut output, exif)?;
    }
    marker::write_dqt(&mut output, 0, &params.quant_tables[0]);
    marker::write_dqt(&mut output, 1, &params.quant_tables[1]);
    marker::write_sof(
        &mut output,
        0xC0,
        low_u16(width),
        low_u16(height),
        &[(1, 1, 1, 0), (2, 1, 1, 1), (3, 1, 1, 1)],
    );
    marker::write_dht(
        &mut output,
        0,
        0,
        &huffman::STD_DC_LUMA.0,
        &huffman::STD_DC_LUMA.1,
    );
    marker::write_dht(
        &mut output,
        1,
        0,
        &huffman::STD_AC_LUMA.0,
        &huffman::STD_AC_LUMA.1,
    );
    marker::write_dht(
        &mut output,
        0,
        1,
        &huffman::STD_DC_CHROMA.0,
        &huffman::STD_DC_CHROMA.1,
    );
    marker::write_dht(
        &mut output,
        1,
        1,
        &huffman::STD_AC_CHROMA.0,
        &huffman::STD_AC_CHROMA.1,
    );
    if restart_interval != 0 {
        marker::write_dri(&mut output, restart_interval);
    }
    marker::write_sos(&mut output, &[(1, 0, 0), (2, 1, 1), (3, 1, 1)], 0, 63, 0, 0);

    let block_rows = height.div_ceil(8);
    let block_groups_per_row = mcu_columns.div_ceil(4);
    let mut coefficient_groups = [[0i16; 256]; 3];
    let mut samples = [i32x4::ZERO; 64];
    let luma_ready = huffman::standard_ac_luma_coefficient_ready();
    let chroma_ready = huffman::standard_ac_chroma_coefficient_ready();
    let mut mcu_writers = [RawBlockWriter::new(); 3];
    let mut scan = RawScanWriter::new(&mut output);
    let mut previous_dc = [0i32; 3];
    let mut mcus_until_restart = usize::from(restart_interval);
    let mut next_restart = 0u8;

    for block_y in 0usize..block_rows {
        for block_group in 0usize..block_groups_per_row {
            let block_x = block_group.saturating_mul(4);
            load_fdct_samples_four_into(y_plane, width, height, block_x, block_y, &mut samples);
            fdct_quantize_four_coefficient_major(
                &mut samples,
                y_quantizer,
                &mut coefficient_groups[0],
            );
            load_fdct_samples_four_into(cb_plane, width, height, block_x, block_y, &mut samples);
            fdct_quantize_four_coefficient_major(
                &mut samples,
                chroma_quantizer,
                &mut coefficient_groups[1],
            );
            load_fdct_samples_four_into(cr_plane, width, height, block_x, block_y, &mut samples);
            fdct_quantize_four_coefficient_major(
                &mut samples,
                chroma_quantizer,
                &mut coefficient_groups[2],
            );

            let first_mcu_x = block_group.saturating_mul(4);
            let group_mcus = mcu_columns.saturating_sub(first_mcu_x).min(4);
            for lane in 0usize..group_mcus {
                if restart_interval != 0 && mcus_until_restart == 0 {
                    scan.finish();
                    marker::write_rst(&mut output, next_restart);
                    next_restart = next_restart.saturating_add(1) & 7;
                    scan = RawScanWriter::new(&mut output);
                    previous_dc = [0; 3];
                    mcus_until_restart = usize::from(restart_interval);
                }
                let blocks = [
                    CoefficientMajorBlock {
                        group: &coefficient_groups[0],
                        lane,
                    },
                    CoefficientMajorBlock {
                        group: &coefficient_groups[1],
                        lane,
                    },
                    CoefficientMajorBlock {
                        group: &coefficient_groups[2],
                        lane,
                    },
                ];
                let dc = blocks.map(|block| block.coefficient(0));
                encode_three_coefficient_major_raw_blocks(
                    &mut mcu_writers,
                    blocks,
                    [
                        dc[0] - previous_dc[0],
                        dc[1] - previous_dc[1],
                        dc[2] - previous_dc[2],
                    ],
                    luma_dc,
                    luma_ac,
                    chroma_dc,
                    chroma_ac,
                    luma_ready,
                    chroma_ready,
                );
                for writer in &mcu_writers {
                    writer.append_to(&mut scan);
                }
                previous_dc = dc;
                mcus_until_restart = mcus_until_restart.saturating_sub(1);
            }
        }
    }
    scan.finish();
    marker::write_eoi(&mut output);
    Ok(output)
}

struct Rgb420McuRowBuffers<'a> {
    y: &'a mut [u8],
    cb: &'a mut [u8],
    cr: &'a mut [u8],
}

#[expect(
    clippy::arithmetic_side_effects,
    reason = "validated RGB dimensions and fixed 16-pixel packets bound every source and row index"
)]
fn convert_rgb_420_mcu_row(
    pixels: &[u8],
    width: usize,
    height: usize,
    mcu_y: usize,
    padded_width: usize,
    buffers: &mut Rgb420McuRowBuffers<'_>,
) {
    debug_assert!(width != 0);
    debug_assert!(height != 0);
    debug_assert!(padded_width.is_multiple_of(16));
    debug_assert!(padded_width >= width);
    debug_assert_eq!(buffers.y.len(), padded_width.saturating_mul(16));
    let chroma_width = padded_width / 2;
    debug_assert_eq!(buffers.cb.len(), chroma_width.saturating_mul(8));
    debug_assert_eq!(buffers.cr.len(), buffers.cb.len());

    for pair in 0usize..8 {
        let first_source_y = mcu_y
            .saturating_mul(16)
            .saturating_add(pair.saturating_mul(2))
            .min(height.saturating_sub(1));
        let second_source_y = first_source_y
            .saturating_add(1)
            .min(height.saturating_sub(1));
        let first_source_row = first_source_y.saturating_mul(width).saturating_mul(3);
        let second_source_row = second_source_y.saturating_mul(width).saturating_mul(3);
        let first_y_output = pair.saturating_mul(2).saturating_mul(padded_width);
        let second_y_output = first_y_output.saturating_add(padded_width);
        let chroma_output = pair.saturating_mul(chroma_width);

        for x in (0usize..padded_width).step_by(16) {
            let (y_first, y_second, cb, cr) = if x.saturating_add(16) <= width {
                let first_start = first_source_row.saturating_add(x.saturating_mul(3));
                let second_start = second_source_row.saturating_add(x.saturating_mul(3));
                let first: &[u8; 48] = pixels[first_start..first_start.saturating_add(48)]
                    .try_into()
                    .unwrap_or_else(|_| unreachable!("validated RGB MCU packet is 48 bytes"));
                let second: &[u8; 48] = pixels[second_start..second_start.saturating_add(48)]
                    .try_into()
                    .unwrap_or_else(|_| unreachable!("validated RGB MCU packet is 48 bytes"));
                crate::codecs::jpeg::kernels::rgb_to_ycbcr_420_packet(first, second)
            } else {
                let mut first = [0u8; 48];
                let mut second = [0u8; 48];
                for lane in 0usize..16 {
                    let source_x = x.saturating_add(lane).min(width.saturating_sub(1));
                    let first_start = first_source_row.saturating_add(source_x.saturating_mul(3));
                    let second_start = second_source_row.saturating_add(source_x.saturating_mul(3));
                    let destination = lane.saturating_mul(3);
                    first[destination..destination.saturating_add(3)]
                        .copy_from_slice(&pixels[first_start..first_start.saturating_add(3)]);
                    second[destination..destination.saturating_add(3)]
                        .copy_from_slice(&pixels[second_start..second_start.saturating_add(3)]);
                }
                crate::codecs::jpeg::kernels::rgb_to_ycbcr_420_packet(&first, &second)
            };
            buffers.y[first_y_output.saturating_add(x)..first_y_output.saturating_add(x + 16)]
                .copy_from_slice(&y_first);
            buffers.y[second_y_output.saturating_add(x)..second_y_output.saturating_add(x + 16)]
                .copy_from_slice(&y_second);
            let chroma_x = x / 2;
            buffers.cb[chroma_output.saturating_add(chroma_x)
                ..chroma_output.saturating_add(chroma_x + 8)]
                .copy_from_slice(&cb);
            buffers.cr[chroma_output.saturating_add(chroma_x)
                ..chroma_output.saturating_add(chroma_x + 8)]
                .copy_from_slice(&cr);
        }
    }
}

#[derive(Clone, Copy)]
enum Baseline420Source<'a> {
    Rgb(&'a [u8]),
    Planes {
        y: &'a [u8],
        cb: &'a [u8],
        cr: &'a [u8],
        chroma_width: usize,
        chroma_height: usize,
    },
}

#[allow(
    clippy::arithmetic_side_effects,
    clippy::too_many_arguments,
    reason = "the exact six-block 4:2:0 scan packet and its image-edge geometry are explicit inputs"
)]
#[inline(always)]
fn encode_baseline_420_mcu_pair(
    pair_mcu_x: usize,
    mcu_y: usize,
    mcu_columns: usize,
    y_block_columns: usize,
    y_block_rows: usize,
    y_top: &[i16; 256],
    y_bottom: &[i16; 256],
    cb_group: &[i16; 256],
    cr_group: &[i16; 256],
    chroma_lane: usize,
    writers: &mut [RawBlockWriter; 6],
    scan: &mut RawScanWriter<'_>,
    previous_dc: [i32; 3],
    luma_dc: &huffman::DerivedTable,
    luma_ac: &huffman::DerivedTable,
    chroma_dc: &huffman::DerivedTable,
    chroma_ac: &huffman::DerivedTable,
    luma_ready: &huffman::CoefficientReadyTable,
    chroma_ready: &huffman::CoefficientReadyTable,
    trim_trailing_zeros: bool,
) -> ([i32; 3], usize) {
    let first_blocks = [
        CoefficientMajorBlock {
            group: y_top,
            lane: 0,
        },
        CoefficientMajorBlock {
            group: y_top,
            lane: 1,
        },
        CoefficientMajorBlock {
            group: y_bottom,
            lane: 0,
        },
        CoefficientMajorBlock {
            group: y_bottom,
            lane: 1,
        },
        CoefficientMajorBlock {
            group: cb_group,
            lane: chroma_lane,
        },
        CoefficientMajorBlock {
            group: cr_group,
            lane: chroma_lane,
        },
    ];
    let first_values = first_blocks.map(|block| block.coefficient(0));
    let first_y_column = pair_mcu_x.saturating_mul(2);
    let first_y_row = mcu_y.saturating_mul(2);
    let first_present = [
        first_y_column < y_block_columns && first_y_row < y_block_rows,
        first_y_column.saturating_add(1) < y_block_columns && first_y_row < y_block_rows,
        first_y_column < y_block_columns && first_y_row.saturating_add(1) < y_block_rows,
        first_y_column.saturating_add(1) < y_block_columns
            && first_y_row.saturating_add(1) < y_block_rows,
        true,
        true,
    ];
    let (first_differences, predictors_after_first) =
        mcu_dc_differences(first_values, first_present, previous_dc);
    if first_present.iter().all(|&present| present) {
        encode_six_coefficient_major_raw_blocks(
            writers,
            first_blocks,
            first_differences,
            luma_dc,
            luma_ac,
            chroma_dc,
            chroma_ac,
            luma_ready,
            chroma_ready,
            trim_trailing_zeros,
        );
    } else {
        encode_six_coefficient_major_edge_raw_blocks(
            writers,
            first_blocks,
            first_differences,
            first_present,
            luma_dc,
            luma_ac,
            chroma_dc,
            chroma_ac,
            luma_ready,
            chroma_ready,
        );
    }
    for writer in &*writers {
        writer.append_to(scan);
    }

    if pair_mcu_x.saturating_add(1) >= mcu_columns {
        return (predictors_after_first, 1);
    }

    let second_blocks = [
        CoefficientMajorBlock {
            group: y_top,
            lane: 2,
        },
        CoefficientMajorBlock {
            group: y_top,
            lane: 3,
        },
        CoefficientMajorBlock {
            group: y_bottom,
            lane: 2,
        },
        CoefficientMajorBlock {
            group: y_bottom,
            lane: 3,
        },
        CoefficientMajorBlock {
            group: cb_group,
            lane: chroma_lane.saturating_add(1),
        },
        CoefficientMajorBlock {
            group: cr_group,
            lane: chroma_lane.saturating_add(1),
        },
    ];
    let second_values = second_blocks.map(|block| block.coefficient(0));
    let second_y_column = pair_mcu_x.saturating_add(1).saturating_mul(2);
    let second_present = [
        second_y_column < y_block_columns && first_y_row < y_block_rows,
        second_y_column.saturating_add(1) < y_block_columns && first_y_row < y_block_rows,
        second_y_column < y_block_columns && first_y_row.saturating_add(1) < y_block_rows,
        second_y_column.saturating_add(1) < y_block_columns
            && first_y_row.saturating_add(1) < y_block_rows,
        true,
        true,
    ];
    let (second_differences, predictors_after_second) =
        mcu_dc_differences(second_values, second_present, predictors_after_first);
    if second_present.iter().all(|&present| present) {
        encode_six_coefficient_major_raw_blocks(
            writers,
            second_blocks,
            second_differences,
            luma_dc,
            luma_ac,
            chroma_dc,
            chroma_ac,
            luma_ready,
            chroma_ready,
            trim_trailing_zeros,
        );
    } else {
        encode_six_coefficient_major_edge_raw_blocks(
            writers,
            second_blocks,
            second_differences,
            second_present,
            luma_dc,
            luma_ac,
            chroma_dc,
            chroma_ac,
            luma_ready,
            chroma_ready,
        );
    }
    for writer in &*writers {
        writer.append_to(scan);
    }
    (predictors_after_second, 2)
}

/// Encode baseline RGB 4:2:0 as one transform/entropy row pipeline.
/// Four blocks remain coefficient-major from the safe SIMD FDCT through the
/// six-way entropy kernel, avoiding whole-image coefficient storage and the
/// coefficient-major-to-block-major transpose.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::too_many_arguments,
    reason = "validated RGB pixels, quantizers, and marker options are explicit inputs"
)]
fn encode_baseline_420_mcu_row_streaming(
    source: Baseline420Source<'_>,
    width: usize,
    height: usize,
    params: &quant::EncodeParams,
    y_quantizer: &FdctQuantizer,
    chroma_quantizer: &FdctQuantizer,
    options: &JpegEncodeOptions,
) -> CodecResult<Vec<u8>> {
    debug_assert!(width != 0);
    debug_assert!(height != 0);
    match source {
        Baseline420Source::Rgb(pixels) => {
            debug_assert_eq!(pixels.len(), width.saturating_mul(height).saturating_mul(3));
        }
        Baseline420Source::Planes {
            y,
            cb,
            cr,
            chroma_width,
            chroma_height,
        } => {
            debug_assert_eq!(y.len(), width.saturating_mul(height));
            debug_assert_eq!(chroma_width, width.div_ceil(16).saturating_mul(8));
            debug_assert_eq!(chroma_height, height.div_ceil(2));
            debug_assert_eq!(cb.len(), chroma_width.saturating_mul(chroma_height));
            debug_assert_eq!(cr.len(), cb.len());
        }
    }
    debug_assert_eq!(params.quant_tables.len(), 2);

    let standard_tables = huffman::standard_derived_tables();
    let luma_dc = &standard_tables[0];
    let chroma_dc = &standard_tables[1];
    let luma_ac = &standard_tables[2];
    let chroma_ac = &standard_tables[3];
    let mcu_columns = width.div_ceil(16);
    let restart_rows = options.restart_interval.unwrap_or(0);
    let restart_interval = if restart_rows == 0 {
        0
    } else {
        let interval = usize::try_from(restart_rows)
            .unwrap_or(usize::MAX)
            .saturating_mul(mcu_columns);
        u16::try_from(interval).map_err(|_| {
            CodecError::Parameter("JPEG restart interval exceeds 65535 MCUs".to_owned())
        })?
    };

    let mut output = Vec::with_capacity(width.saturating_mul(height).saturating_add(1024));
    marker::write_soi(&mut output);
    marker::write_jfif_app0(&mut output);
    if let Some(exif) = options.exif.as_deref() {
        marker::write_exif_app1(&mut output, exif)?;
    }
    marker::write_dqt(&mut output, 0, &params.quant_tables[0]);
    marker::write_dqt(&mut output, 1, &params.quant_tables[1]);
    marker::write_sof(
        &mut output,
        0xC0,
        low_u16(width),
        low_u16(height),
        &[(1, 2, 2, 0), (2, 1, 1, 1), (3, 1, 1, 1)],
    );
    marker::write_dht(
        &mut output,
        0,
        0,
        &huffman::STD_DC_LUMA.0,
        &huffman::STD_DC_LUMA.1,
    );
    marker::write_dht(
        &mut output,
        1,
        0,
        &huffman::STD_AC_LUMA.0,
        &huffman::STD_AC_LUMA.1,
    );
    marker::write_dht(
        &mut output,
        0,
        1,
        &huffman::STD_DC_CHROMA.0,
        &huffman::STD_DC_CHROMA.1,
    );
    marker::write_dht(
        &mut output,
        1,
        1,
        &huffman::STD_AC_CHROMA.0,
        &huffman::STD_AC_CHROMA.1,
    );
    if restart_interval != 0 {
        marker::write_dri(&mut output, restart_interval);
    }
    marker::write_sos(&mut output, &[(1, 0, 0), (2, 1, 1), (3, 1, 1)], 0, 63, 0, 0);

    let mcu_rows = height.div_ceil(16);
    let y_block_columns = width.div_ceil(8);
    let y_block_rows = height.div_ceil(8);
    let y_groups_per_row = mcu_columns.div_ceil(2);
    let chroma_groups_per_row = mcu_columns.div_ceil(4);
    let y_bottom_offset = y_groups_per_row;
    let cb_offset = y_groups_per_row.saturating_mul(2);
    let cr_offset = cb_offset.saturating_add(chroma_groups_per_row);
    let group_count = cr_offset.saturating_add(chroma_groups_per_row);
    let mut row_groups = vec![[0i16; 256]; group_count];
    let padded_width = mcu_columns.saturating_mul(16);
    let chroma_width = padded_width / 2;
    let (mut y_strip, mut cb_strip, mut cr_strip) = match source {
        Baseline420Source::Rgb(_) => (
            vec![0u8; padded_width.saturating_mul(16)],
            vec![0u8; chroma_width.saturating_mul(8)],
            vec![0u8; chroma_width.saturating_mul(8)],
        ),
        Baseline420Source::Planes { .. } => (Vec::new(), Vec::new(), Vec::new()),
    };
    let mut samples = [i32x4::ZERO; 64];
    let luma_ready = huffman::standard_ac_luma_coefficient_ready();
    let chroma_ready = huffman::standard_ac_chroma_coefficient_ready();
    let trim_trailing_zeros = options.quality.unwrap_or(75) <= 25;
    let mut mcu_writers = [RawBlockWriter::new(); 6];
    let mut scan = RawScanWriter::new(&mut output);
    let mut previous_dc = [0i32; 3];
    let mut mcus_until_restart = usize::from(restart_interval);
    let mut next_restart = 0u8;

    for mcu_y in 0usize..mcu_rows {
        let (
            y_source,
            y_source_width,
            y_source_height,
            y_block_row,
            cb_source,
            cr_source,
            chroma_source_width,
            chroma_source_height,
            chroma_block_row,
        ) = match source {
            Baseline420Source::Rgb(pixels) => {
                let mut buffers = Rgb420McuRowBuffers {
                    y: &mut y_strip,
                    cb: &mut cb_strip,
                    cr: &mut cr_strip,
                };
                convert_rgb_420_mcu_row(pixels, width, height, mcu_y, padded_width, &mut buffers);
                (
                    &y_strip[..],
                    padded_width,
                    16,
                    0,
                    &cb_strip[..],
                    &cr_strip[..],
                    chroma_width,
                    8,
                    0,
                )
            }
            Baseline420Source::Planes {
                y,
                cb,
                cr,
                chroma_width,
                chroma_height,
            } => (
                y,
                width,
                height,
                mcu_y.saturating_mul(2),
                cb,
                cr,
                chroma_width,
                chroma_height,
                mcu_y,
            ),
        };
        for mcu_x in (0usize..mcu_columns).step_by(2) {
            let group_column = mcu_x / 2;
            let y_block_x = mcu_x.saturating_mul(2);
            load_fdct_samples_four_into(
                y_source,
                y_source_width,
                y_source_height,
                y_block_x,
                y_block_row,
                &mut samples,
            );
            fdct_quantize_four_coefficient_major(
                &mut samples,
                y_quantizer,
                &mut row_groups[group_column],
            );

            load_fdct_samples_four_into(
                y_source,
                y_source_width,
                y_source_height,
                y_block_x,
                y_block_row.saturating_add(1),
                &mut samples,
            );
            fdct_quantize_four_coefficient_major(
                &mut samples,
                y_quantizer,
                &mut row_groups[y_bottom_offset.saturating_add(group_column)],
            );
        }

        for chroma_group in 0usize..chroma_groups_per_row {
            let chroma_block_x = chroma_group.saturating_mul(4);
            load_fdct_samples_four_into(
                cb_source,
                chroma_source_width,
                chroma_source_height,
                chroma_block_x,
                chroma_block_row,
                &mut samples,
            );
            fdct_quantize_four_coefficient_major(
                &mut samples,
                chroma_quantizer,
                &mut row_groups[cb_offset.saturating_add(chroma_group)],
            );

            load_fdct_samples_four_into(
                cr_source,
                chroma_source_width,
                chroma_source_height,
                chroma_block_x,
                chroma_block_row,
                &mut samples,
            );
            fdct_quantize_four_coefficient_major(
                &mut samples,
                chroma_quantizer,
                &mut row_groups[cr_offset.saturating_add(chroma_group)],
            );
        }

        for pair_mcu_x in (0usize..mcu_columns).step_by(2) {
            if restart_interval != 0 && mcus_until_restart == 0 {
                scan.finish();
                marker::write_rst(&mut output, next_restart);
                next_restart = next_restart.saturating_add(1) & 7;
                scan = RawScanWriter::new(&mut output);
                previous_dc = [0; 3];
                mcus_until_restart = usize::from(restart_interval);
            }
            let y_group_column = pair_mcu_x / 2;
            let y_top = &row_groups[y_group_column];
            let y_bottom = &row_groups[y_bottom_offset.saturating_add(y_group_column)];
            let chroma_group_column = pair_mcu_x / 4;
            let cb_group = &row_groups[cb_offset.saturating_add(chroma_group_column)];
            let cr_group = &row_groups[cr_offset.saturating_add(chroma_group_column)];
            let chroma_lane = pair_mcu_x % 4;
            let (predictors_after_pair, encoded_mcus) = encode_baseline_420_mcu_pair(
                pair_mcu_x,
                mcu_y,
                mcu_columns,
                y_block_columns,
                y_block_rows,
                y_top,
                y_bottom,
                cb_group,
                cr_group,
                chroma_lane,
                &mut mcu_writers,
                &mut scan,
                previous_dc,
                luma_dc,
                luma_ac,
                chroma_dc,
                chroma_ac,
                luma_ready,
                chroma_ready,
                trim_trailing_zeros,
            );
            previous_dc = predictors_after_pair;
            mcus_until_restart = mcus_until_restart.saturating_sub(encoded_mcus);
        }
    }
    scan.finish();
    marker::write_eoi(&mut output);
    Ok(output)
}

#[expect(
    clippy::arithmetic_side_effects,
    reason = "validated byte samples stay in the signed i16 range after the JPEG level shift"
)]
#[inline(always)]
fn store_fdct_sample_row_four(source: &[u8; 32], destination: usize, output: &mut [i32x4; 64]) {
    const LEVEL_SHIFT: i16x8 = i16x8::new([128; 8]);
    let first = pod_read_unaligned::<u8x16>(&source[..16]);
    let second = pod_read_unaligned::<u8x16>(&source[16..]);
    let blocks = [
        i32x8::from(cast::<u16x8, i16x8>(u16x8::from_u8x16_low(first)) - LEVEL_SHIFT).to_array(),
        i32x8::from(cast::<u16x8, i16x8>(u16x8::from_u8x16_high(first)) - LEVEL_SHIFT).to_array(),
        i32x8::from(cast::<u16x8, i16x8>(u16x8::from_u8x16_low(second)) - LEVEL_SHIFT).to_array(),
        i32x8::from(cast::<u16x8, i16x8>(u16x8::from_u8x16_high(second)) - LEVEL_SHIFT).to_array(),
    ];
    for column in 0usize..8 {
        output[destination.saturating_add(column)] = i32x4::new([
            blocks[0][column],
            blocks[1][column],
            blocks[2][column],
            blocks[3][column],
        ]);
    }
}

#[inline(always)]
fn load_fdct_samples_four_into(
    plane: &[u8],
    width: usize,
    height: usize,
    block_x: usize,
    block_y: usize,
    output: &mut [i32x4; 64],
) {
    let source_x = block_x.saturating_mul(8);
    let source_y = block_y.saturating_mul(8);
    if source_x.saturating_add(32) <= width {
        for row in 0usize..8 {
            let source_row = source_y.saturating_add(row).min(height.saturating_sub(1));
            let source_start = source_row.saturating_mul(width).saturating_add(source_x);
            let source = &plane[source_start..source_start.saturating_add(32)];
            let source: &[u8; 32] = source
                .try_into()
                .unwrap_or_else(|_| unreachable!("validated FDCT packet row is 32 bytes"));
            store_fdct_sample_row_four(source, row.saturating_mul(8), output);
        }
        return;
    }

    for row in 0usize..8 {
        let sample_y = source_y.saturating_add(row).min(height.saturating_sub(1));
        let row_start = sample_y.saturating_mul(width);
        let copy_start = source_x.min(width.saturating_sub(1));
        let copy_count = width.saturating_sub(copy_start).min(32);
        let edge = plane[row_start.saturating_add(width.saturating_sub(1))];
        let mut samples = [edge; 32];
        samples[..copy_count]
            .copy_from_slice(&plane[row_start.saturating_add(copy_start)..][..copy_count]);
        store_fdct_sample_row_four(&samples, row.saturating_mul(8), output);
    }
}

#[allow(
    clippy::arithmetic_side_effects,
    reason = "four fixed lanes and 64 JPEG coefficients exactly index the output packet"
)]
#[inline(always)]
fn fdct_quantize_four_coefficient_major(
    samples: &mut [i32x4; 64],
    quantizer: &FdctQuantizer,
    output: &mut [i16; 256],
) {
    fdct::fdct_islow_four_coefficient_major_packed(samples);
    for (coefficient, values) in samples.iter().copied().enumerate() {
        let values = values.to_array();
        for lane in 0usize..4 {
            output[coefficient * 4 + lane] = quantize_coefficient(
                values[lane],
                quantizer.divisors[coefficient],
                quantizer.reciprocals[coefficient],
            );
        }
    }
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
    quantizer: &FdctQuantizer,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(Vec<[i16; 64]>, usize, usize)> {
    #[cfg(coverage)]
    coverage_cancel_fdct_call(token);
    if let Some(token) = token {
        let mut checkpoint = TokenFdctCheckpoint::new(token);
        fdct_quantize_with_checkpoint(plane, w, h, quantizer, &mut checkpoint)
    } else {
        fdct_quantize_without_checkpoint(plane, w, h, quantizer)
    }
}

fn fdct_quantize_without_checkpoint(
    plane: &[u8],
    w: usize,
    h: usize,
    quantizer: &FdctQuantizer,
) -> CodecResult<(Vec<[i16; 64]>, usize, usize)> {
    let blocks_per_row = w.div_ceil(8);
    let block_rows = h.div_ceil(8);
    let mut blocks = vec![[0i16; 64]; blocks_per_row.saturating_mul(block_rows)];

    for by in 0..block_rows {
        for bx_start in (0usize..blocks_per_row).step_by(4) {
            let block_count = blocks_per_row.saturating_sub(bx_start).min(4);
            let mut sample_batch: [[i32; 64]; 4] = std::array::from_fn(|lane| {
                let bx = bx_start
                    .saturating_add(lane)
                    .min(blocks_per_row.saturating_sub(1));
                load_fdct_samples(plane, w, h, bx, by)
            });

            fdct::fdct_islow_four(&mut sample_batch);
            for (lane, samples) in sample_batch.iter().take(block_count).enumerate() {
                let quantized = std::array::from_fn(|coefficient| {
                    quantize_coefficient(
                        samples[coefficient],
                        quantizer.divisors[coefficient],
                        quantizer.reciprocals[coefficient],
                    )
                });
                let block_index = by
                    .saturating_mul(blocks_per_row)
                    .saturating_add(bx_start)
                    .saturating_add(lane);
                blocks[block_index] = quantized;
            }
        }
    }
    Ok((blocks, blocks_per_row, block_rows))
}

fn fdct_quantize_with_checkpoint<C: FdctCheckpoint>(
    plane: &[u8],
    w: usize,
    h: usize,
    quantizer: &FdctQuantizer,
    checkpoint: &mut C,
) -> CodecResult<(Vec<[i16; 64]>, usize, usize)> {
    let blocks_per_row = w.div_ceil(8);
    let block_rows = h.div_ceil(8);
    let mut blocks = vec![[0i16; 64]; blocks_per_row.saturating_mul(block_rows)];

    for by in 0..block_rows {
        checkpoint.row()?;
        for bx in 0..blocks_per_row {
            let mut samples = load_fdct_samples(plane, w, h, bx, by);
            fdct::fdct_islow(&mut samples);
            // Quantize in natural order: divisor = quantval[i] << 3.
            let quantized = std::array::from_fn(|coefficient| {
                quantize_coefficient(
                    samples[coefficient],
                    quantizer.divisors[coefficient],
                    quantizer.reciprocals[coefficient],
                )
            });
            blocks[by.saturating_mul(blocks_per_row).saturating_add(bx)] = quantized;
            checkpoint.block()?;
        }
    }
    Ok((blocks, blocks_per_row, block_rows))
}

#[allow(
    clippy::arithmetic_side_effects,
    reason = "the complete-block branch and replicated edge coordinates are bounded by validated image dimensions"
)]
#[inline]
fn load_fdct_samples(plane: &[u8], w: usize, h: usize, bx: usize, by: usize) -> [i32; 64] {
    let mut samples = [0i32; 64];
    let x = bx.saturating_mul(8);
    let y = by.saturating_mul(8);

    if x.saturating_add(8) <= w && y.saturating_add(8) <= h {
        for row in 0usize..8 {
            let source_start = y.saturating_add(row).saturating_mul(w).saturating_add(x);
            let source = &plane[source_start..source_start.saturating_add(8)];
            let destination_start = row.saturating_mul(8);
            for (output, &sample) in samples[destination_start..destination_start + 8]
                .iter_mut()
                .zip(source)
            {
                *output = i32::from(sample) - 128;
            }
        }
        return samples;
    }

    for row in 0usize..8 {
        for column in 0usize..8 {
            let source_y = y.saturating_add(row).min(h.saturating_sub(1));
            let source_x = x.saturating_add(column).min(w.saturating_sub(1));
            samples[row.saturating_mul(8).saturating_add(column)] =
                i32::from(plane[source_y.saturating_mul(w).saturating_add(source_x)]) - 128;
        }
    }
    samples
}

/// Build a fixed-point reciprocal whose high product word is exact or one
/// correction away from division by a positive JPEG quantization divisor.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "JPEG quantization divisors are positive and at most 2040"
)]
#[inline]
fn reciprocal_divisor(divisor: u32) -> u32 {
    debug_assert!(divisor != 0);
    let numerator = (1u64 << 32) + u64::from(divisor) - 1;
    (numerator / u64::from(divisor)) as u32
}

/// Round one transformed coefficient exactly as libjpeg's integer quantizer,
/// using multiply-high plus bounded correction instead of integer division.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    reason = "JPEG FDCT bounds keep the numerator, corrected quotient, and signed result in range"
)]
#[inline(always)]
fn quantize_coefficient(value: i32, divisor: u32, reciprocal: u32) -> i16 {
    debug_assert!(divisor != 0);
    let numerator = value.unsigned_abs() + (divisor >> 1);
    let mut quotient = ((u64::from(numerator) * u64::from(reciprocal)) >> 32) as u32;

    if quotient * divisor > numerator {
        quotient -= 1;
    }
    if (quotient + 1) * divisor <= numerator {
        quotient += 1;
    }

    let magnitude = quotient as i32;
    let signed = if value < 0 { -magnitude } else { magnitude };
    signed as i16
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
                            bw.write_bounded_bits(dc_tbl.codes[0], dc_tbl.lengths[0]);
                            bw.write_bounded_bits(ac_tbl.codes[0], ac_tbl.lengths[0]);
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

/// Maximum complete 64-bit words emitted by one unpadded baseline block.
/// The legal 8-bit baseline coefficient domain fits below this 2,048-bit
/// bound, including worst-case Huffman and magnitude widths.
const RAW_BLOCK_WORD_CAPACITY: usize = 32;

/// One unstuffed, unpadded block bitstream. Six of these writers allow the
/// processor to advance the independent Y0/Y1/Y2/Y3/Cb/Cr entropy chains in
/// parallel before they are concatenated in exact JPEG scan order.
#[derive(Clone, Copy)]
struct RawBlockWriter {
    words: [u64; RAW_BLOCK_WORD_CAPACITY],
    word_count: usize,
    reservoir: u64,
    valid_bits: u8,
}

impl RawBlockWriter {
    const fn new() -> Self {
        Self {
            words: [0; RAW_BLOCK_WORD_CAPACITY],
            word_count: 0,
            reservoir: 0,
            valid_bits: 0,
        }
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.word_count = 0;
        self.reservoir = 0;
        self.valid_bits = 0;
    }

    #[allow(
        clippy::arithmetic_side_effects,
        reason = "validated JPEG append widths and the reservoir invariant bound every shift"
    )]
    #[inline(always)]
    fn write(&mut self, code: u64, width: u8) {
        debug_assert!((1..=32).contains(&width));
        debug_assert!(code < 1u64.wrapping_shl(u32::from(width)));
        let available = 64u8.saturating_sub(self.valid_bits);
        if width <= available {
            self.reservoir = (self.reservoir << width) | code;
            self.valid_bits += width;
            if self.valid_bits == 64 {
                self.publish_word();
            }
            return;
        }

        let suffix_width = width.saturating_sub(available);
        let prefix = code.wrapping_shr(u32::from(suffix_width));
        self.reservoir = (self.reservoir << available) | prefix;
        self.valid_bits = 64;
        self.publish_word();
        let suffix_mask = 1u64.wrapping_shl(u32::from(suffix_width)).saturating_sub(1);
        self.reservoir = code & suffix_mask;
        self.valid_bits = suffix_width;
    }

    #[inline(always)]
    fn publish_word(&mut self) {
        assert!(
            self.word_count < RAW_BLOCK_WORD_CAPACITY,
            "baseline JPEG block exceeded raw entropy capacity"
        );
        self.words[self.word_count] = self.reservoir;
        self.word_count = self.word_count.saturating_add(1);
        self.reservoir = 0;
        self.valid_bits = 0;
    }

    #[inline(always)]
    fn append_to(&self, scan: &mut RawScanWriter<'_>) {
        for &word in &self.words[..self.word_count] {
            scan.write(word, 64);
        }
        if self.valid_bits != 0 {
            scan.write(self.reservoir, self.valid_bits);
        }
    }
}

/// Complete entropy scan. It joins the six independent block streams before
/// applying the one scan-level pad and JPEG 0xFF byte stuffing.
struct RawScanWriter<'a> {
    output: &'a mut Vec<u8>,
    reservoir: u64,
    valid_bits: u8,
}

impl<'a> RawScanWriter<'a> {
    fn new(output: &'a mut Vec<u8>) -> Self {
        Self {
            output,
            reservoir: 0,
            valid_bits: 0,
        }
    }

    #[allow(
        clippy::arithmetic_side_effects,
        reason = "block streams are validated at construction and each append is at most 64 bits"
    )]
    #[inline(always)]
    fn write(&mut self, code: u64, width: u8) {
        debug_assert!((1..=64).contains(&width));
        debug_assert!(width == 64 || code < 1u64.wrapping_shl(u32::from(width)));
        let available = 64u8.saturating_sub(self.valid_bits);
        if width <= available {
            self.reservoir = if width == 64 {
                debug_assert_eq!(self.valid_bits, 0);
                code
            } else {
                (self.reservoir << width) | code
            };
            self.valid_bits += width;
            if self.valid_bits == 64 {
                self.publish_word();
            }
            return;
        }

        let suffix_width = width.saturating_sub(available);
        let prefix = code.wrapping_shr(u32::from(suffix_width));
        self.reservoir = (self.reservoir << available) | prefix;
        self.valid_bits = 64;
        self.publish_word();
        let suffix_mask = 1u64.wrapping_shl(u32::from(suffix_width)).saturating_sub(1);
        self.reservoir = code & suffix_mask;
        self.valid_bits = suffix_width;
    }

    #[inline(always)]
    fn publish_word(&mut self) {
        let bytes = self.reservoir.to_be_bytes();
        if !bytes.contains(&0xFF) {
            self.output.extend_from_slice(&bytes);
        } else {
            for byte in bytes {
                self.output.push(byte);
                if byte == 0xFF {
                    self.output.push(0);
                }
            }
        }
        self.reservoir = 0;
        self.valid_bits = 0;
    }

    #[allow(
        clippy::arithmetic_side_effects,
        clippy::cast_possible_truncation,
        reason = "the final reservoir contains at most 64 bits and is padded to a byte boundary"
    )]
    fn finish(mut self) {
        if self.valid_bits == 0 {
            return;
        }

        let padding = (8 - self.valid_bits % 8) % 8;
        self.reservoir =
            (self.reservoir << padding) | 1u64.wrapping_shl(u32::from(padding)).saturating_sub(1);
        self.valid_bits += padding;
        if self.valid_bits == 64 {
            self.publish_word();
            return;
        }

        while self.valid_bits >= 8 {
            self.valid_bits -= 8;
            let byte = (self.reservoir >> self.valid_bits) as u8;
            self.output.push(byte);
            if byte == 0xFF {
                self.output.push(0);
            }
        }
    }
}

#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "baseline JPEG bounds DC differences, categories, and combined symbol widths"
)]
#[inline(always)]
fn encode_raw_dc(writer: &mut RawBlockWriter, difference: i32, table: &huffman::DerivedTable) {
    let width = jpeg_nbits(difference);
    let index = bounded_usize(width);
    let huffman_code = table.codes[index];
    let huffman_width = table.lengths[index];
    if width == 0 {
        writer.write(u64::from(huffman_code), huffman_width);
    } else {
        writer.write(
            u64::from((huffman_code << width) | mag_bits(difference, width)),
            huffman_width.saturating_add(width as u8),
        );
    }
}

#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "baseline JPEG bounds zero runs, categories, and ready-table indices"
)]
#[inline(always)]
fn encode_raw_ac_value(
    writer: &mut RawBlockWriter,
    coefficient: i32,
    run: &mut u32,
    table: &huffman::DerivedTable,
    ready: &huffman::CoefficientReadyTable,
) {
    if coefficient == 0 {
        *run = run.saturating_add(1);
        return;
    }

    while *run >= 16 {
        writer.write(u64::from(table.codes[0xF0]), table.lengths[0xF0]);
        *run -= 16;
    }

    if (-huffman::COEFFICIENT_READY_LIMIT..=huffman::COEFFICIENT_READY_LIMIT).contains(&coefficient)
    {
        let value_width = huffman::COEFFICIENT_READY_LIMIT as usize * 2 + 1;
        let index =
            *run as usize * value_width + (coefficient + huffman::COEFFICIENT_READY_LIMIT) as usize;
        let packed = ready.entries[index];
        if packed != 0 {
            writer.write(u64::from(packed >> 8), packed as u8);
            *run = 0;
            return;
        }
    }

    let width = jpeg_nbits(coefficient);
    let symbol = bounded_usize((*run << 4) | width);
    let huffman_code = table.codes[symbol];
    let huffman_width = table.lengths[symbol];
    writer.write(
        u64::from((huffman_code << width) | mag_bits(coefficient, width)),
        huffman_width.saturating_add(width as u8),
    );
    *run = 0;
}

#[inline(always)]
fn finish_raw_ac(writer: &mut RawBlockWriter, run: u32, table: &huffman::DerivedTable) {
    if run != 0 {
        writer.write(u64::from(table.codes[0]), table.lengths[0]);
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the six independent block chains require separate luma and chroma tables"
)]
#[inline(always)]
fn encode_six_raw_blocks(
    writers: &mut [RawBlockWriter; 6],
    blocks: [&[i16; 64]; 6],
    dc_differences: [i32; 6],
    luma_dc: &huffman::DerivedTable,
    luma_ac: &huffman::DerivedTable,
    chroma_dc: &huffman::DerivedTable,
    chroma_ac: &huffman::DerivedTable,
    luma_ready: &huffman::CoefficientReadyTable,
    chroma_ready: &huffman::CoefficientReadyTable,
) {
    let [writer0, writer1, writer2, writer3, writer4, writer5] = writers;
    writer0.reset();
    writer1.reset();
    writer2.reset();
    writer3.reset();
    writer4.reset();
    writer5.reset();
    encode_raw_dc(writer0, dc_differences[0], luma_dc);
    encode_raw_dc(writer1, dc_differences[1], luma_dc);
    encode_raw_dc(writer2, dc_differences[2], luma_dc);
    encode_raw_dc(writer3, dc_differences[3], luma_dc);
    encode_raw_dc(writer4, dc_differences[4], chroma_dc);
    encode_raw_dc(writer5, dc_differences[5], chroma_dc);

    let mut run0 = 0u32;
    let mut run1 = 0u32;
    let mut run2 = 0u32;
    let mut run3 = 0u32;
    let mut run4 = 0u32;
    let mut run5 = 0u32;
    for &coefficient_index in &ZIGZAG[1..] {
        encode_raw_ac_value(
            writer0,
            i32::from(blocks[0][coefficient_index]),
            &mut run0,
            luma_ac,
            luma_ready,
        );
        encode_raw_ac_value(
            writer1,
            i32::from(blocks[1][coefficient_index]),
            &mut run1,
            luma_ac,
            luma_ready,
        );
        encode_raw_ac_value(
            writer2,
            i32::from(blocks[2][coefficient_index]),
            &mut run2,
            luma_ac,
            luma_ready,
        );
        encode_raw_ac_value(
            writer3,
            i32::from(blocks[3][coefficient_index]),
            &mut run3,
            luma_ac,
            luma_ready,
        );
        encode_raw_ac_value(
            writer4,
            i32::from(blocks[4][coefficient_index]),
            &mut run4,
            chroma_ac,
            chroma_ready,
        );
        encode_raw_ac_value(
            writer5,
            i32::from(blocks[5][coefficient_index]),
            &mut run5,
            chroma_ac,
            chroma_ready,
        );
    }
    finish_raw_ac(writer0, run0, luma_ac);
    finish_raw_ac(writer1, run1, luma_ac);
    finish_raw_ac(writer2, run2, luma_ac);
    finish_raw_ac(writer3, run3, luma_ac);
    finish_raw_ac(writer4, run4, chroma_ac);
    finish_raw_ac(writer5, run5, chroma_ac);
}

/// One logical block viewed through a lane of the four-block FDCT's native
/// coefficient-major output.
#[derive(Clone, Copy)]
struct CoefficientMajorBlock<'a> {
    group: &'a [i16; 256],
    lane: usize,
}

impl CoefficientMajorBlock<'_> {
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "JPEG coefficient indices are below 64 and lanes are below four"
    )]
    #[inline(always)]
    fn coefficient(self, natural_index: usize) -> i32 {
        debug_assert!(natural_index < 64);
        debug_assert!(self.lane < 4);
        i32::from(self.group[natural_index * 4 + self.lane])
    }
}

#[inline(always)]
fn mcu_dc_differences(
    values: [i32; 6],
    present: [bool; 6],
    mut predictors: [i32; 3],
) -> ([i32; 6], [i32; 3]) {
    let mut differences = [0i32; 6];
    for block in 0usize..4 {
        if present[block] {
            differences[block] = values[block].saturating_sub(predictors[0]);
            predictors[0] = values[block];
        }
    }
    for block in 4usize..6 {
        if present[block] {
            let component = block.saturating_sub(3);
            differences[block] = values[block].saturating_sub(predictors[component]);
            predictors[component] = values[block];
        }
    }
    (differences, predictors)
}

#[allow(
    clippy::too_many_arguments,
    reason = "partial edge MCUs require six presence flags and separate luma/chroma tables"
)]
#[inline(always)]
fn encode_six_coefficient_major_edge_raw_blocks(
    writers: &mut [RawBlockWriter; 6],
    blocks: [CoefficientMajorBlock<'_>; 6],
    dc_differences: [i32; 6],
    present: [bool; 6],
    luma_dc: &huffman::DerivedTable,
    luma_ac: &huffman::DerivedTable,
    chroma_dc: &huffman::DerivedTable,
    chroma_ac: &huffman::DerivedTable,
    luma_ready: &huffman::CoefficientReadyTable,
    chroma_ready: &huffman::CoefficientReadyTable,
) {
    let [writer0, writer1, writer2, writer3, writer4, writer5] = writers;
    writer0.reset();
    writer1.reset();
    writer2.reset();
    writer3.reset();
    writer4.reset();
    writer5.reset();
    for (writer, difference, is_present) in [
        (&mut *writer0, dc_differences[0], present[0]),
        (&mut *writer1, dc_differences[1], present[1]),
        (&mut *writer2, dc_differences[2], present[2]),
        (&mut *writer3, dc_differences[3], present[3]),
    ] {
        encode_raw_dc(writer, difference, luma_dc);
        if !is_present {
            writer.write(u64::from(luma_ac.codes[0]), luma_ac.lengths[0]);
        }
    }
    for (writer, difference, is_present) in [
        (&mut *writer4, dc_differences[4], present[4]),
        (&mut *writer5, dc_differences[5], present[5]),
    ] {
        encode_raw_dc(writer, difference, chroma_dc);
        if !is_present {
            writer.write(u64::from(chroma_ac.codes[0]), chroma_ac.lengths[0]);
        }
    }

    let mut run0 = 0u32;
    let mut run1 = 0u32;
    let mut run2 = 0u32;
    let mut run3 = 0u32;
    let mut run4 = 0u32;
    let mut run5 = 0u32;
    for &coefficient_index in &ZIGZAG[1..] {
        if present[0] {
            encode_raw_ac_value(
                writer0,
                blocks[0].coefficient(coefficient_index),
                &mut run0,
                luma_ac,
                luma_ready,
            );
        }
        if present[1] {
            encode_raw_ac_value(
                writer1,
                blocks[1].coefficient(coefficient_index),
                &mut run1,
                luma_ac,
                luma_ready,
            );
        }
        if present[2] {
            encode_raw_ac_value(
                writer2,
                blocks[2].coefficient(coefficient_index),
                &mut run2,
                luma_ac,
                luma_ready,
            );
        }
        if present[3] {
            encode_raw_ac_value(
                writer3,
                blocks[3].coefficient(coefficient_index),
                &mut run3,
                luma_ac,
                luma_ready,
            );
        }
        if present[4] {
            encode_raw_ac_value(
                writer4,
                blocks[4].coefficient(coefficient_index),
                &mut run4,
                chroma_ac,
                chroma_ready,
            );
        }
        if present[5] {
            encode_raw_ac_value(
                writer5,
                blocks[5].coefficient(coefficient_index),
                &mut run5,
                chroma_ac,
                chroma_ready,
            );
        }
    }
    if present[0] {
        finish_raw_ac(writer0, run0, luma_ac);
    }
    if present[1] {
        finish_raw_ac(writer1, run1, luma_ac);
    }
    if present[2] {
        finish_raw_ac(writer2, run2, luma_ac);
    }
    if present[3] {
        finish_raw_ac(writer3, run3, luma_ac);
    }
    if present[4] {
        finish_raw_ac(writer4, run4, chroma_ac);
    }
    if present[5] {
        finish_raw_ac(writer5, run5, chroma_ac);
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the six independent block chains require separate luma and chroma tables"
)]
#[inline(always)]
fn encode_six_coefficient_major_raw_blocks(
    writers: &mut [RawBlockWriter; 6],
    blocks: [CoefficientMajorBlock<'_>; 6],
    dc_differences: [i32; 6],
    luma_dc: &huffman::DerivedTable,
    luma_ac: &huffman::DerivedTable,
    chroma_dc: &huffman::DerivedTable,
    chroma_ac: &huffman::DerivedTable,
    luma_ready: &huffman::CoefficientReadyTable,
    chroma_ready: &huffman::CoefficientReadyTable,
    trim_trailing_zeros: bool,
) {
    let [writer0, writer1, writer2, writer3, writer4, writer5] = writers;
    writer0.reset();
    writer1.reset();
    writer2.reset();
    writer3.reset();
    writer4.reset();
    writer5.reset();
    encode_raw_dc(writer0, dc_differences[0], luma_dc);
    encode_raw_dc(writer1, dc_differences[1], luma_dc);
    encode_raw_dc(writer2, dc_differences[2], luma_dc);
    encode_raw_dc(writer3, dc_differences[3], luma_dc);
    encode_raw_dc(writer4, dc_differences[4], chroma_dc);
    encode_raw_dc(writer5, dc_differences[5], chroma_dc);

    let mut run0 = 0u32;
    let mut run1 = 0u32;
    let mut run2 = 0u32;
    let mut run3 = 0u32;
    let mut run4 = 0u32;
    let mut run5 = 0u32;
    let mut last_position = 63usize;
    if trim_trailing_zeros {
        while last_position != 0 {
            let natural_index = ZIGZAG[last_position];
            if blocks
                .iter()
                .any(|block| block.coefficient(natural_index) != 0)
            {
                break;
            }
            last_position = last_position.saturating_sub(1);
        }
    }
    for &coefficient_index in &ZIGZAG[1..=last_position] {
        encode_raw_ac_value(
            writer0,
            blocks[0].coefficient(coefficient_index),
            &mut run0,
            luma_ac,
            luma_ready,
        );
        encode_raw_ac_value(
            writer1,
            blocks[1].coefficient(coefficient_index),
            &mut run1,
            luma_ac,
            luma_ready,
        );
        encode_raw_ac_value(
            writer2,
            blocks[2].coefficient(coefficient_index),
            &mut run2,
            luma_ac,
            luma_ready,
        );
        encode_raw_ac_value(
            writer3,
            blocks[3].coefficient(coefficient_index),
            &mut run3,
            luma_ac,
            luma_ready,
        );
        encode_raw_ac_value(
            writer4,
            blocks[4].coefficient(coefficient_index),
            &mut run4,
            chroma_ac,
            chroma_ready,
        );
        encode_raw_ac_value(
            writer5,
            blocks[5].coefficient(coefficient_index),
            &mut run5,
            chroma_ac,
            chroma_ready,
        );
    }
    let omitted = u32::try_from(63usize.saturating_sub(last_position)).unwrap_or(0);
    run0 = run0.saturating_add(omitted);
    run1 = run1.saturating_add(omitted);
    run2 = run2.saturating_add(omitted);
    run3 = run3.saturating_add(omitted);
    run4 = run4.saturating_add(omitted);
    run5 = run5.saturating_add(omitted);
    finish_raw_ac(writer0, run0, luma_ac);
    finish_raw_ac(writer1, run1, luma_ac);
    finish_raw_ac(writer2, run2, luma_ac);
    finish_raw_ac(writer3, run3, luma_ac);
    finish_raw_ac(writer4, run4, chroma_ac);
    finish_raw_ac(writer5, run5, chroma_ac);
}

#[allow(
    clippy::too_many_arguments,
    reason = "a partial 4:2:2 edge MCU requires separate luma and chroma tables"
)]
#[inline(always)]
fn encode_four_coefficient_major_edge_raw_blocks(
    writers: &mut [RawBlockWriter; 4],
    blocks: [CoefficientMajorBlock<'_>; 4],
    dc_differences: [i32; 4],
    luma_dc: &huffman::DerivedTable,
    luma_ac: &huffman::DerivedTable,
    chroma_dc: &huffman::DerivedTable,
    chroma_ac: &huffman::DerivedTable,
    luma_ready: &huffman::CoefficientReadyTable,
    chroma_ready: &huffman::CoefficientReadyTable,
) {
    let [writer0, writer1, writer2, writer3] = writers;
    writer0.reset();
    writer1.reset();
    writer2.reset();
    writer3.reset();
    encode_raw_dc(writer0, dc_differences[0], luma_dc);
    encode_raw_dc(writer1, dc_differences[1], luma_dc);
    writer1.write(u64::from(luma_ac.codes[0]), luma_ac.lengths[0]);
    encode_raw_dc(writer2, dc_differences[2], chroma_dc);
    encode_raw_dc(writer3, dc_differences[3], chroma_dc);

    let mut run0 = 0u32;
    let mut run2 = 0u32;
    let mut run3 = 0u32;
    for &coefficient_index in &ZIGZAG[1..] {
        encode_raw_ac_value(
            writer0,
            blocks[0].coefficient(coefficient_index),
            &mut run0,
            luma_ac,
            luma_ready,
        );
        encode_raw_ac_value(
            writer2,
            blocks[2].coefficient(coefficient_index),
            &mut run2,
            chroma_ac,
            chroma_ready,
        );
        encode_raw_ac_value(
            writer3,
            blocks[3].coefficient(coefficient_index),
            &mut run3,
            chroma_ac,
            chroma_ready,
        );
    }
    finish_raw_ac(writer0, run0, luma_ac);
    finish_raw_ac(writer2, run2, chroma_ac);
    finish_raw_ac(writer3, run3, chroma_ac);
}

#[allow(
    clippy::too_many_arguments,
    reason = "the four independent block chains require separate luma and chroma tables"
)]
#[inline(always)]
fn encode_four_coefficient_major_raw_blocks(
    writers: &mut [RawBlockWriter; 4],
    blocks: [CoefficientMajorBlock<'_>; 4],
    dc_differences: [i32; 4],
    luma_dc: &huffman::DerivedTable,
    luma_ac: &huffman::DerivedTable,
    chroma_dc: &huffman::DerivedTable,
    chroma_ac: &huffman::DerivedTable,
    luma_ready: &huffman::CoefficientReadyTable,
    chroma_ready: &huffman::CoefficientReadyTable,
) {
    let [writer0, writer1, writer2, writer3] = writers;
    writer0.reset();
    writer1.reset();
    writer2.reset();
    writer3.reset();
    encode_raw_dc(writer0, dc_differences[0], luma_dc);
    encode_raw_dc(writer1, dc_differences[1], luma_dc);
    encode_raw_dc(writer2, dc_differences[2], chroma_dc);
    encode_raw_dc(writer3, dc_differences[3], chroma_dc);

    let mut run0 = 0u32;
    let mut run1 = 0u32;
    let mut run2 = 0u32;
    let mut run3 = 0u32;
    for &coefficient_index in &ZIGZAG[1..] {
        encode_raw_ac_value(
            writer0,
            blocks[0].coefficient(coefficient_index),
            &mut run0,
            luma_ac,
            luma_ready,
        );
        encode_raw_ac_value(
            writer1,
            blocks[1].coefficient(coefficient_index),
            &mut run1,
            luma_ac,
            luma_ready,
        );
        encode_raw_ac_value(
            writer2,
            blocks[2].coefficient(coefficient_index),
            &mut run2,
            chroma_ac,
            chroma_ready,
        );
        encode_raw_ac_value(
            writer3,
            blocks[3].coefficient(coefficient_index),
            &mut run3,
            chroma_ac,
            chroma_ready,
        );
    }
    finish_raw_ac(writer0, run0, luma_ac);
    finish_raw_ac(writer1, run1, luma_ac);
    finish_raw_ac(writer2, run2, chroma_ac);
    finish_raw_ac(writer3, run3, chroma_ac);
}

#[allow(
    clippy::too_many_arguments,
    reason = "the three independent block chains require separate luma and chroma tables"
)]
#[inline(always)]
fn encode_three_coefficient_major_raw_blocks(
    writers: &mut [RawBlockWriter; 3],
    blocks: [CoefficientMajorBlock<'_>; 3],
    dc_differences: [i32; 3],
    luma_dc: &huffman::DerivedTable,
    luma_ac: &huffman::DerivedTable,
    chroma_dc: &huffman::DerivedTable,
    chroma_ac: &huffman::DerivedTable,
    luma_ready: &huffman::CoefficientReadyTable,
    chroma_ready: &huffman::CoefficientReadyTable,
) {
    let [writer0, writer1, writer2] = writers;
    writer0.reset();
    writer1.reset();
    writer2.reset();
    encode_raw_dc(writer0, dc_differences[0], luma_dc);
    encode_raw_dc(writer1, dc_differences[1], chroma_dc);
    encode_raw_dc(writer2, dc_differences[2], chroma_dc);

    let mut run0 = 0u32;
    let mut run1 = 0u32;
    let mut run2 = 0u32;
    for &coefficient_index in &ZIGZAG[1..] {
        encode_raw_ac_value(
            writer0,
            blocks[0].coefficient(coefficient_index),
            &mut run0,
            luma_ac,
            luma_ready,
        );
        encode_raw_ac_value(
            writer1,
            blocks[1].coefficient(coefficient_index),
            &mut run1,
            chroma_ac,
            chroma_ready,
        );
        encode_raw_ac_value(
            writer2,
            blocks[2].coefficient(coefficient_index),
            &mut run2,
            chroma_ac,
            chroma_ready,
        );
    }
    finish_raw_ac(writer0, run0, luma_ac);
    finish_raw_ac(writer1, run1, chroma_ac);
    finish_raw_ac(writer2, run2, chroma_ac);
}

#[allow(
    clippy::too_many_arguments,
    reason = "the four independent block chains require separate luma and chroma tables"
)]
#[inline(always)]
fn encode_four_raw_blocks(
    writers: &mut [RawBlockWriter; 4],
    blocks: [&[i16; 64]; 4],
    dc_differences: [i32; 4],
    luma_dc: &huffman::DerivedTable,
    luma_ac: &huffman::DerivedTable,
    chroma_dc: &huffman::DerivedTable,
    chroma_ac: &huffman::DerivedTable,
    luma_ready: &huffman::CoefficientReadyTable,
    chroma_ready: &huffman::CoefficientReadyTable,
) {
    let [writer0, writer1, writer2, writer3] = writers;
    writer0.reset();
    writer1.reset();
    writer2.reset();
    writer3.reset();
    encode_raw_dc(writer0, dc_differences[0], luma_dc);
    encode_raw_dc(writer1, dc_differences[1], luma_dc);
    encode_raw_dc(writer2, dc_differences[2], chroma_dc);
    encode_raw_dc(writer3, dc_differences[3], chroma_dc);

    let mut run0 = 0u32;
    let mut run1 = 0u32;
    let mut run2 = 0u32;
    let mut run3 = 0u32;
    for &coefficient_index in &ZIGZAG[1..] {
        encode_raw_ac_value(
            writer0,
            i32::from(blocks[0][coefficient_index]),
            &mut run0,
            luma_ac,
            luma_ready,
        );
        encode_raw_ac_value(
            writer1,
            i32::from(blocks[1][coefficient_index]),
            &mut run1,
            luma_ac,
            luma_ready,
        );
        encode_raw_ac_value(
            writer2,
            i32::from(blocks[2][coefficient_index]),
            &mut run2,
            chroma_ac,
            chroma_ready,
        );
        encode_raw_ac_value(
            writer3,
            i32::from(blocks[3][coefficient_index]),
            &mut run3,
            chroma_ac,
            chroma_ready,
        );
    }
    finish_raw_ac(writer0, run0, luma_ac);
    finish_raw_ac(writer1, run1, luma_ac);
    finish_raw_ac(writer2, run2, chroma_ac);
    finish_raw_ac(writer3, run3, chroma_ac);
}

#[allow(
    clippy::too_many_arguments,
    reason = "the three independent block chains require separate luma and chroma tables"
)]
#[inline(always)]
fn encode_three_raw_blocks(
    writers: &mut [RawBlockWriter; 3],
    blocks: [&[i16; 64]; 3],
    dc_differences: [i32; 3],
    luma_dc: &huffman::DerivedTable,
    luma_ac: &huffman::DerivedTable,
    chroma_dc: &huffman::DerivedTable,
    chroma_ac: &huffman::DerivedTable,
    luma_ready: &huffman::CoefficientReadyTable,
    chroma_ready: &huffman::CoefficientReadyTable,
) {
    let [writer0, writer1, writer2] = writers;
    writer0.reset();
    writer1.reset();
    writer2.reset();
    encode_raw_dc(writer0, dc_differences[0], luma_dc);
    encode_raw_dc(writer1, dc_differences[1], chroma_dc);
    encode_raw_dc(writer2, dc_differences[2], chroma_dc);

    let mut run0 = 0u32;
    let mut run1 = 0u32;
    let mut run2 = 0u32;
    for &coefficient_index in &ZIGZAG[1..] {
        encode_raw_ac_value(
            writer0,
            i32::from(blocks[0][coefficient_index]),
            &mut run0,
            luma_ac,
            luma_ready,
        );
        encode_raw_ac_value(
            writer1,
            i32::from(blocks[1][coefficient_index]),
            &mut run1,
            chroma_ac,
            chroma_ready,
        );
        encode_raw_ac_value(
            writer2,
            i32::from(blocks[2][coefficient_index]),
            &mut run2,
            chroma_ac,
            chroma_ready,
        );
    }
    finish_raw_ac(writer0, run0, luma_ac);
    finish_raw_ac(writer1, run1, chroma_ac);
    finish_raw_ac(writer2, run2, chroma_ac);
}

fn baseline_422_independent_entropy_is_compatible(
    components: &[CompData],
    maximum_horizontal_sampling: u8,
    maximum_vertical_sampling: u8,
) -> bool {
    let [y, cb, cr] = components else {
        return false;
    };
    maximum_horizontal_sampling == 2
        && maximum_vertical_sampling == 1
        && (y.h_samp, y.v_samp) == (2, 1)
        && (cb.h_samp, cb.v_samp) == (1, 1)
        && (cr.h_samp, cr.v_samp) == (1, 1)
        && !y.blocks.is_empty()
        && y.blocks_per_row.is_multiple_of(2)
        && cb.blocks_per_row.saturating_mul(2) == y.blocks_per_row
        && cb.block_rows == y.block_rows
        && cr.blocks_per_row == cb.blocks_per_row
        && cr.block_rows == cb.block_rows
}

#[expect(
    clippy::arithmetic_side_effects,
    reason = "quantized JPEG DC coefficients are i16 values, so predictor deltas fit in i32"
)]
fn encode_baseline_422_independent_entropy(
    output: &mut Vec<u8>,
    components: &[CompData],
    dc_tables: &[&huffman::DerivedTable; 2],
    ac_tables: &[&huffman::DerivedTable; 2],
) {
    let [y, cb, cr] = components else {
        unreachable!("the 4:2:2 fast-path guard requires three components");
    };
    let mcu_columns = y.blocks_per_row / 2;
    let mcu_rows = y.block_rows;
    let luma_ready = huffman::standard_ac_luma_coefficient_ready();
    let chroma_ready = huffman::standard_ac_chroma_coefficient_ready();
    let mut writers = [RawBlockWriter::new(); 4];
    let mut scan = RawScanWriter::new(output);
    let mut previous_dc = [0i32; 3];

    for mcu_y in 0..mcu_rows {
        for mcu_x in 0..mcu_columns {
            let y_base = mcu_y
                .saturating_mul(y.blocks_per_row)
                .saturating_add(mcu_x.saturating_mul(2));
            let chroma_index = mcu_y
                .saturating_mul(cb.blocks_per_row)
                .saturating_add(mcu_x);
            let blocks = [
                &y.blocks[y_base],
                &y.blocks[y_base.saturating_add(1)],
                &cb.blocks[chroma_index],
                &cr.blocks[chroma_index],
            ];
            let dc = blocks.map(|block| i32::from(block[0]));
            encode_four_raw_blocks(
                &mut writers,
                blocks,
                [
                    dc[0] - previous_dc[0],
                    dc[1] - dc[0],
                    dc[2] - previous_dc[1],
                    dc[3] - previous_dc[2],
                ],
                dc_tables[0],
                ac_tables[0],
                dc_tables[1],
                ac_tables[1],
                luma_ready,
                chroma_ready,
            );
            for writer in &writers {
                writer.append_to(&mut scan);
            }
            previous_dc = [dc[1], dc[2], dc[3]];
        }
    }
    scan.finish();
}

fn baseline_444_independent_entropy_is_compatible(
    components: &[CompData],
    maximum_horizontal_sampling: u8,
    maximum_vertical_sampling: u8,
) -> bool {
    let [y, cb, cr] = components else {
        return false;
    };
    maximum_horizontal_sampling == 1
        && maximum_vertical_sampling == 1
        && (y.h_samp, y.v_samp) == (1, 1)
        && (cb.h_samp, cb.v_samp) == (1, 1)
        && (cr.h_samp, cr.v_samp) == (1, 1)
        && !y.blocks.is_empty()
        && cb.blocks_per_row == y.blocks_per_row
        && cb.block_rows == y.block_rows
        && cr.blocks_per_row == y.blocks_per_row
        && cr.block_rows == y.block_rows
}

#[expect(
    clippy::arithmetic_side_effects,
    reason = "quantized JPEG DC coefficients are i16 values, so predictor deltas fit in i32"
)]
fn encode_baseline_444_independent_entropy(
    output: &mut Vec<u8>,
    components: &[CompData],
    dc_tables: &[&huffman::DerivedTable; 2],
    ac_tables: &[&huffman::DerivedTable; 2],
) {
    let [y, cb, cr] = components else {
        unreachable!("the 4:4:4 fast-path guard requires three components");
    };
    let luma_ready = huffman::standard_ac_luma_coefficient_ready();
    let chroma_ready = huffman::standard_ac_chroma_coefficient_ready();
    let mut writers = [RawBlockWriter::new(); 3];
    let mut scan = RawScanWriter::new(output);
    let mut previous_dc = [0i32; 3];

    for block_index in 0..y.blocks.len() {
        let blocks = [
            &y.blocks[block_index],
            &cb.blocks[block_index],
            &cr.blocks[block_index],
        ];
        let dc = blocks.map(|block| i32::from(block[0]));
        encode_three_raw_blocks(
            &mut writers,
            blocks,
            [
                dc[0] - previous_dc[0],
                dc[1] - previous_dc[1],
                dc[2] - previous_dc[2],
            ],
            dc_tables[0],
            ac_tables[0],
            dc_tables[1],
            ac_tables[1],
            luma_ready,
            chroma_ready,
        );
        for writer in &writers {
            writer.append_to(&mut scan);
        }
        previous_dc = dc;
    }
    scan.finish();
}

fn baseline_420_independent_entropy_is_compatible(
    components: &[CompData],
    maximum_horizontal_sampling: u8,
    maximum_vertical_sampling: u8,
) -> bool {
    let [y, cb, cr] = components else {
        return false;
    };
    maximum_horizontal_sampling == 2
        && maximum_vertical_sampling == 2
        && (y.h_samp, y.v_samp) == (2, 2)
        && (cb.h_samp, cb.v_samp) == (1, 1)
        && (cr.h_samp, cr.v_samp) == (1, 1)
        && !y.blocks.is_empty()
        && y.blocks_per_row.is_multiple_of(2)
        && y.block_rows.is_multiple_of(2)
        && cb.blocks_per_row.saturating_mul(2) == y.blocks_per_row
        && cb.block_rows.saturating_mul(2) == y.block_rows
        && cr.blocks_per_row == cb.blocks_per_row
        && cr.block_rows == cb.block_rows
}

#[allow(
    clippy::arithmetic_side_effects,
    clippy::too_many_arguments,
    reason = "i16 DC differences fit i32 and the six-block scan order is explicit"
)]
fn encode_baseline_420_independent_entropy(
    output: &mut Vec<u8>,
    components: &[CompData],
    dc_tables: &[&huffman::DerivedTable; 2],
    ac_tables: &[&huffman::DerivedTable; 2],
) {
    let [y, cb, cr] = components else {
        unreachable!("the 4:2:0 fast-path guard requires three components");
    };
    let mcu_columns = y.blocks_per_row / 2;
    let mcu_rows = y.block_rows / 2;
    let luma_ready = huffman::standard_ac_luma_coefficient_ready();
    let chroma_ready = huffman::standard_ac_chroma_coefficient_ready();
    let mut mcu_writers = [RawBlockWriter::new(); 6];
    let mut scan = RawScanWriter::new(output);
    let mut previous_dc = [0i32; 3];

    for mcu_y in 0..mcu_rows {
        for mcu_x in 0..mcu_columns {
            let y_row = mcu_y.saturating_mul(2);
            let y_column = mcu_x.saturating_mul(2);
            let y_base = y_row
                .saturating_mul(y.blocks_per_row)
                .saturating_add(y_column);
            let y_next_row = y_base.saturating_add(y.blocks_per_row);
            let y_blocks = [
                &y.blocks[y_base],
                &y.blocks[y_base.saturating_add(1)],
                &y.blocks[y_next_row],
                &y.blocks[y_next_row.saturating_add(1)],
            ];
            let y_values = y_blocks.map(|block| i32::from(block[0]));
            let y_differences = [
                y_values[0] - previous_dc[0],
                y_values[1] - y_values[0],
                y_values[2] - y_values[1],
                y_values[3] - y_values[2],
            ];
            previous_dc[0] = y_values[3];

            let chroma_index = mcu_y
                .saturating_mul(cb.blocks_per_row)
                .saturating_add(mcu_x);
            let chroma_blocks = [&cb.blocks[chroma_index], &cr.blocks[chroma_index]];
            let chroma_values = chroma_blocks.map(|block| i32::from(block[0]));
            let chroma_differences = [
                chroma_values[0] - previous_dc[1],
                chroma_values[1] - previous_dc[2],
            ];
            previous_dc[1] = chroma_values[0];
            previous_dc[2] = chroma_values[1];

            encode_six_raw_blocks(
                &mut mcu_writers,
                [
                    y_blocks[0],
                    y_blocks[1],
                    y_blocks[2],
                    y_blocks[3],
                    chroma_blocks[0],
                    chroma_blocks[1],
                ],
                [
                    y_differences[0],
                    y_differences[1],
                    y_differences[2],
                    y_differences[3],
                    chroma_differences[0],
                    chroma_differences[1],
                ],
                dc_tables[0],
                ac_tables[0],
                dc_tables[1],
                ac_tables[1],
                luma_ready,
                chroma_ready,
            );
            for writer in &mcu_writers {
                writer.append_to(&mut scan);
            }
        }
    }
    scan.finish();
}

/// Encode one 8×8 block: DC difference + AC run/length in zigzag order.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "baseline JPEG bounds coefficient categories, symbol widths, and the 63-position AC scan"
)]
#[inline(always)]
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
    let dc_code = dc_tbl.codes[nbits_index];
    let dc_length = dc_tbl.lengths[nbits_index];
    if nbits > 0 {
        let magnitude = mag_bits(diff, nbits);
        bw.write_bounded_bits(
            dc_code.wrapping_shl(nbits) | magnitude,
            dc_length + nbits.to_le_bytes()[0],
        );
    } else {
        bw.write_bounded_bits(dc_code, dc_length);
    }

    let mut coefficient_index = 1usize;
    while coefficient_index < 64 {
        let mut coefficient = i32::from(block[ZIGZAG[coefficient_index]]);
        if coefficient != 0 {
            loop {
                let width = jpeg_nbits(coefficient);
                let symbol = bounded_usize(width);
                let code = ac_tbl.codes[symbol];
                let length = ac_tbl.lengths[symbol];
                let magnitude = mag_bits(coefficient, width);
                bw.write_bounded_bits(
                    code.wrapping_shl(width) | magnitude,
                    length + width.to_le_bytes()[0],
                );
                coefficient_index += 1;
                if coefficient_index == 64 {
                    return;
                }
                coefficient = i32::from(block[ZIGZAG[coefficient_index]]);
                if coefficient == 0 {
                    break;
                }
            }
            continue;
        }

        let mut run = 1u32;
        coefficient_index += 1;
        while coefficient_index < 64 {
            coefficient = i32::from(block[ZIGZAG[coefficient_index]]);
            if coefficient != 0 {
                break;
            }
            run += 1;
            coefficient_index += 1;
        }
        if coefficient_index == 64 {
            bw.write_bounded_bits(ac_tbl.codes[0], ac_tbl.lengths[0]);
            return;
        }

        while run >= 16 {
            bw.write_bounded_bits(ac_tbl.codes[0xF0], ac_tbl.lengths[0xF0]);
            run -= 16;
        }
        let width = jpeg_nbits(coefficient);
        let symbol = bounded_usize(run.wrapping_shl(4) | width);
        let code = ac_tbl.codes[symbol];
        let length = ac_tbl.lengths[symbol];
        let magnitude = mag_bits(coefficient, width);
        bw.write_bounded_bits(
            code.wrapping_shl(width) | magnitude,
            length + width.to_le_bytes()[0],
        );
        coefficient_index += 1;
    }
}

/// Number of bits needed to represent |v| (JPEG_NBITS).  nbits(0)=0.
fn jpeg_nbits(v: i32) -> u32 {
    let magnitude = v.unsigned_abs();
    if magnitude == 0 {
        0
    } else {
        32u32.saturating_sub(magnitude.leading_zeros())
    }
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
