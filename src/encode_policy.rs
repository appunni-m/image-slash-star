//! Caller-selected policy for encoded-output admission.

use crate::{CodecOperation, ImageError, ImageFormat, ImageResult, ResourceLimit};

/// Policy applied to complete encoded output before it is returned or written.
///
/// An absent limit is unlimited for backward compatibility. The output-result
/// bound does not account for transient allocations or recoverable out-of-
/// memory failure. `max_work_units` counts the deterministic cooperative
/// checkpoints reached by an encode, including TIFF Deflate input-row,
/// level-six matcher, expansion, Huffman, bitstream, and checksum intervals,
/// JPEG RGB-to-YCbCr conversion and chroma-downsample output intervals after
/// each 1,024 converted or produced pixels, baseline entropy traversal after
/// each 1,024 MCUs, and entropy-output intervals after
/// each 1,024 emitted entropy bytes, JPEG optimized-Huffman frequency
/// coefficients after each 1,024 coefficients, progressive scan block slots
/// after each 1,024 blocks, progressive scan coefficient items after each
/// 1,024 coefficients, and progressive scan-event frequency items after each
/// 1,024 events, the PNG adaptive-filter
/// and filtered-row checkpoints charged after each 1,024 row bytes, PNG
/// stored-block copy checkpoints charged after each 1,024 copied bytes, BMP
/// row-conversion checkpoints charged after each 1,024 pixels, lossy WebP VP8
/// RGB/RGBA-to-YUV conversion items and RGBA transparent-area cleanup after
/// each 1,024 scanned or flattened pixels, RGBA alpha-palette source
/// collection and index packing after each 1,024 source pixels, required
/// padded-plane edge replication, filter-edge adjustment, analysis histogram
/// construction after each 64 completed 4×4 blocks, analysis after each 1,024
/// macroblocks, segment-clustering alpha-domain chunks after each 64 values,
/// segment-assignment macroblocks after each 1,024 items, and
/// mode-selection batches after each 64 completed macroblocks (roughly 1,024
/// luma blocks), method-6 trellis
/// quantization coefficient candidates and path-reconstruction nodes, plus
/// intra4 squared-error pixels, spectral-distortion weighted-transform
/// row/column passes, residual-cost coefficients, and the remaining
/// analysis/partition stages, 8-bit, 16-bit, 32-bit, 64-bit,
/// 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit,
/// 32,768-bit, 65,536-bit, 131,072-bit, and 262,144-bit logical and
/// 16,384-boolean first-partition-bit intervals,
/// 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, and 1,048,576-bit logical and 16,384-boolean coefficient-bit intervals,
/// and 1,024-byte boolean-bitstream output intervals, including pending VP8
/// boolean-output runs drained in those same chunks, and WebP L1/P8/L8/La8/CMYK
/// source-mode preparation and RGBA alpha/RGB extraction after each 1,024
/// source pixels, plus lossless WebP VP8L RGB/RGBA source-pixel
/// materialization, image-palette construction, and palette-mode index packing
/// after each 1,024 source pixels, plus RGBA
/// hidden-RGB cleanup after each 1,024 scanned pixels,
/// predictor source-snapshot copying, predictor/cross-color/entropy/transform,
/// VP8L meta-histogram sampling row/column comparisons and symbol compaction
/// after each 1,024 symbols,
/// bounded backward-reference cost/length-table initialization and setup after
/// each 1,024 entries, token-aware cost-manager interval-update and cleanup
/// scans after each 256 cumulative interval entries, repeated-run hash-chain insertion
/// after each 256 pixels, long backward-reference result backfills after each
/// 256 entries, and saturated cost-interval scans,
/// entropy-analysis, histogram/Huffman, 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 16,384-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, and 1,048,576-bit logical bitstream, 1,024-byte bitstream-output,
/// and token-stream stages, GIF RGB
/// quantization input/index intervals, high-color RGB nearest-palette candidate
/// ordering and scan intervals, fixed 1,024-cell RGBA FASTOCTREE
/// copy/subtraction/lookup intervals, and LZW input-symbol intervals; it is a
/// work-control bound, not a CPU-time or allocation guarantee.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EncodePolicy {
    max_output_bytes: Option<u64>,
    max_work_units: Option<u64>,
}

impl EncodePolicy {
    /// Create the unlimited compatibility policy.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_output_bytes: None,
            max_work_units: None,
        }
    }

    /// Return the maximum admitted encoded-output length.
    #[must_use]
    pub const fn max_output_bytes(self) -> Option<u64> {
        self.max_output_bytes
    }

    /// Set the inclusive maximum admitted encoded-output length.
    #[must_use]
    pub const fn with_max_output_bytes(mut self, maximum: u64) -> Self {
        self.max_output_bytes = Some(maximum);
        self
    }

    /// Return the maximum number of cooperative encode checkpoints admitted.
    #[must_use]
    pub const fn max_work_units(self) -> Option<u64> {
        self.max_work_units
    }

    /// Set the inclusive maximum number of cooperative encode checkpoints.
    ///
    /// One work unit is charged at each documented cancellation checkpoint.
    /// PNG long-row filtering charges additional checkpoints after each 1,024
    /// filtered bytes while adaptive candidates are scored or emitted. PNG
    /// stored-block copying charges an additional checkpoint after each 1,024
    /// copied bytes. BMP row conversion charges additional checkpoints after
    /// each 1,024 pixels. JPEG RGB-to-YCbCr conversion charges an additional
    /// checkpoint after each 1,024 pixels, JPEG chroma downsampling charges an
    /// additional checkpoint after each 1,024 output pixels, baseline entropy
    /// traversal charges an additional checkpoint after each 1,024 MCUs, and JPEG entropy
    /// coding charges an additional checkpoint after each 1,024 emitted entropy
    /// bytes, and optimized baseline Huffman frequency gathering charges an
    /// additional checkpoint after each 1,024 AC coefficients. Progressive
    /// scan event generation charges an additional checkpoint after each 1,024
    /// block slots, progressive scan coefficient traversal charges an additional
    /// checkpoint after each 1,024 coefficients, and progressive scan-event
    /// frequency gathering charges an additional checkpoint after each 1,024
    /// events. TIFF
    /// Deflate charges input-row and level-six matcher candidate, insertion,
    /// fizzle, window, and position intervals plus expansion, Huffman,
    /// bitstream, stored-block, and checksum intervals. Lossy
    /// WebP VP8 encoding charges checkpoints after each 1,024 RGB/RGBA-to-YUV
    /// conversion items and each 1,024 scanned or flattened RGBA
    /// transparent-area cleanup pixels, and after each 1,024 RGBA alpha-palette
    /// source pixels during source collection and index packing, after each
    /// required padded-plane item, analysis histogram batch of 64 completed
    /// 4×4 blocks, analysis macroblock, segment-clustering alpha-domain chunk
    /// of 64 values, segment-assignment macroblock,
    /// mode-selection batch of 64 completed macroblocks (roughly 1,024 luma
    /// blocks), method-6 trellis quantization coefficient candidates and
    /// path-reconstruction nodes, squared-error pixels, spectral-distortion
    /// weighted-transform row/column passes, residual-cost coefficients, and
    /// after each 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, and 262,144-bit
    /// logical and 16,384-boolean first-partition interval, after each 8-bit, 16-bit, 32-bit, 64-bit, 128-bit,
    /// 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, and 1,048,576-bit logical and 16,384-boolean coefficient-bit interval, and between its
    /// major analysis, mode-selection, probability, and bitstream stages; VP8L
    /// encoding charges checkpoints during L1/P8/L8/La8/CMYK source-mode
    /// preparation and RGBA alpha/RGB extraction after each 1,024 source
    /// pixels, plus RGB/RGBA source-pixel materialization, image-palette
    /// construction, and palette-mode index packing after each 1,024 source
    /// pixels, plus RGBA hidden-RGB cleanup
    /// after each 1,024 scanned pixels, predictor image-width tile-row copies,
    /// predictor tile scans, cross-color, entropy,
    /// transform, bounded backward-reference cost/length-table initialization
    /// and setup after each 1,024 entries, token-aware cost-manager interval-
    /// update and cleanup scans after each 256 cumulative interval entries,
    /// repeated-run hash-chain insertion after each 256 pixels and long
    /// backward-reference result backfills after each 256 entries,
    /// palette-index lookup candidate scans after each 64 palette entries,
    /// Huffman RLE preparation and in-run code-length scans after each 64
    /// symbols, canonical-code assignment scans after each 64 symbols,
    /// histogram/Huffman, 8-bit, 16-bit, 32-bit, 64-bit, 128-bit,
    /// 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 16,384-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, and 1,048,576-bit logical bitstream intervals, 1,024-byte bitstream-output, and
    /// token-stream intervals. GIF RGB/RGBA
    /// palette quantization charges after
    /// each 1,024 pixels while preparing palette/index data; high-color RGB
    /// median-cut preparation additionally charges around hash/order setup,
    /// axis ordering, split stages, and 1,024-item partition intervals, and
    /// its nearest-palette candidate ordering and scan charge every 1,024
    /// work items; RGBA
    /// FASTOCTREE preparation additionally charges after each 1,024-cell, bucket,
    /// lookup-entry, or bucket-sort operation interval; and GIF LZW encoding
    /// charges an interval for each input symbol considered by its dictionary
    /// pass.
    /// Exhaustion returns [`ImageError::LimitExceeded`] before that checkpoint
    /// performs further codec work.
    #[must_use]
    pub const fn with_max_work_units(mut self, maximum: u64) -> Self {
        self.max_work_units = Some(maximum);
        self
    }

    pub(crate) fn check_output(
        self,
        bytes: &[u8],
        format: ImageFormat,
        operation: CodecOperation,
    ) -> ImageResult<()> {
        self.check_output_len(bytes.len(), format, operation)
    }

    pub(crate) fn check_output_len(
        self,
        observed_length: usize,
        format: ImageFormat,
        operation: CodecOperation,
    ) -> ImageResult<()> {
        let Some(maximum) = self.max_output_bytes else {
            return Ok(());
        };
        let observed = u64::try_from(observed_length).unwrap_or(u64::MAX);
        if observed > maximum {
            return Err(ImageError::LimitExceeded {
                format: Some(format),
                operation,
                resource: ResourceLimit::EncodedOutputBytes,
                maximum,
                observed,
            });
        }
        Ok(())
    }
}
