//! Encoding of WebP images.

#![warn(clippy::all)]
#![deny(
    clippy::clone_on_copy,
    clippy::expect_used,
    clippy::large_enum_variant,
    clippy::map_unwrap_or,
    clippy::needless_borrow,
    clippy::needless_collect,
    clippy::needless_range_loop,
    clippy::redundant_clone,
    clippy::todo,
    clippy::unnecessary_cast,
    clippy::unnecessary_to_owned,
    clippy::unwrap_in_result,
    clippy::unwrap_used
)]
// VP8L bit packing, fixed-point costs, Huffman construction, and image
// geometry are reference-codec arithmetic. Validated dimensions and bounded
// alphabets constrain the operations in this module.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

mod backward_refs;
pub(super) mod cross_color;
mod histogram;
pub(super) mod predictor;

#[cfg(coverage)]
use core::sync::atomic::{AtomicUsize, Ordering};

/// Color type of the image.
///
/// Note that the WebP format doesn't have a concept of color type. All images are encoded as RGBA
/// and some decoders may treat them as such. This enum is used to indicate the color type of the
/// input data provided to the encoder, which can help improve compression ratio.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ColorType {
    /// Opaque image with a red, green, and blue byte per pixel.
    Rgb8,
    /// Image with a red, green, blue, and alpha byte per pixel.
    Rgba8,
}

/// Error encountered while encoding lossless WebP data.
#[derive(Debug)]
pub enum EncodingError {
    InvalidDimensions,
    Cancelled,
    WorkBudgetExceeded { maximum: u64, observed: u64 },
}

fn check_token(token: Option<&crate::CancellationToken>) -> Result<(), EncodingError> {
    match token {
        None => Ok(()),
        Some(token) => match token.poll() {
            crate::cancel::PollResult::Continue => Ok(()),
            crate::cancel::PollResult::Cancelled => Err(EncodingError::Cancelled),
            crate::cancel::PollResult::WorkBudgetExceeded {
                maximum, observed, ..
            } => Err(EncodingError::WorkBudgetExceeded { maximum, observed }),
        },
    }
}

#[cfg(coverage)]
static COVERAGE_NESTED_METADATA_REMAINING: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static COVERAGE_FRAME_CROSS_REMAINING: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static COVERAGE_FRAME_COLOR_REMAINING: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static COVERAGE_FRAME_PREDICTOR_REMAINING: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static COVERAGE_TOKEN_STREAM_HISTOGRAM_REMAINING: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static COVERAGE_TOKEN_STREAM_CANCEL_AT_OPTIMIZE: AtomicUsize = AtomicUsize::new(0);
#[cfg(coverage)]
static COVERAGE_TOKEN_STREAM_META_PIXEL_REMAINING: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static COVERAGE_IMAGE_STREAM_SUFFIX_REMAINING: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static COVERAGE_FRAME_SUBTRACT_FIRST_REMAINING: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static COVERAGE_FRAME_SUBTRACT_SECOND_REMAINING: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static COVERAGE_FRAME_CROSS_FIRST_REMAINING: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static COVERAGE_FRAME_CROSS_SECOND_REMAINING: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static COVERAGE_FRAME_CROSS_THIRD_REMAINING: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static COVERAGE_ALPHA_COMPRESSED_REMAINING: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static COVERAGE_ALPHA_UNCOMPRESSED_REMAINING: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static COVERAGE_ENCODER_FRAME_REMAINING: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static COVERAGE_ENCODER_CHUNK_REMAINING: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static COVERAGE_ENCODER_FINAL_REMAINING: AtomicUsize = AtomicUsize::new(usize::MAX);

#[cfg(coverage)]
#[coverage(off)]
fn coverage_record_nested_metadata(token: Option<&crate::CancellationToken>) {
    if let Some(token) = token {
        let remaining = token.coverage_remaining_checks().unwrap_or(usize::MAX);
        let _ = COVERAGE_NESTED_METADATA_REMAINING.compare_exchange(
            usize::MAX,
            remaining,
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_record_frame_boundary(slot: &AtomicUsize, token: Option<&crate::CancellationToken>) {
    if let Some(token) = token {
        let remaining = token.coverage_remaining_checks().unwrap_or(usize::MAX);
        let _ = slot.compare_exchange(usize::MAX, remaining, Ordering::Relaxed, Ordering::Relaxed);
    }
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_record_remaining(slot: &AtomicUsize, remaining: Option<usize>) {
    if let Some(remaining) = remaining {
        let _ = slot.compare_exchange(usize::MAX, remaining, Ordering::Relaxed, Ordering::Relaxed);
    }
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_record_token_remaining(slot: &AtomicUsize, token: Option<&crate::CancellationToken>) {
    coverage_record_remaining(
        slot,
        token.and_then(crate::CancellationToken::coverage_remaining_checks),
    );
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_cancel_token_at_optimize(token: Option<&crate::CancellationToken>) {
    if COVERAGE_TOKEN_STREAM_CANCEL_AT_OPTIMIZE.swap(0, Ordering::Relaxed) != 0 {
        if let Some(token) = token {
            token.cancel_after(0);
        }
    }
}

const VP8L_OUTPUT_CHECKPOINT_BYTES: usize = 1_024;
const VP8L_TRANSFORM_CHECKPOINT_PIXELS: usize = 1_024;
const VP8L_GRAYSCALE_CHECKPOINT_PIXELS: usize = 1_024;
const VP8L_ENTROPY_ANALYSIS_CHECKPOINT_PIXELS: usize = 1_024;
const VP8L_ALPHA_CLEANUP_CHECKPOINT_PIXELS: usize = 1_024;
const VP8L_PIXEL_CONVERSION_CHECKPOINT_PIXELS: usize = 1_024;
const VP8L_ALPHA_CHANNEL_CHECKPOINT_PIXELS: usize = 1_024;
const VP8L_MAX_PALETTE_ENTRIES: usize = 256;
const WEBP_ALPHA_PALETTE_CHECKPOINT_PIXELS: usize = 1_024;
const WEBP_ALPHA_PALETTE_PACKING_CHECKPOINT_PIXELS: usize = 1_024;
const VP8L_PALETTE_CHECKPOINT_PIXELS: usize = 1_024;
const VP8L_PALETTE_PACKING_CHECKPOINT_PIXELS: usize = 1_024;
const VP8L_8_BITSTREAM_CHECKPOINT_BITS: usize = 8;
const VP8L_16_BITSTREAM_CHECKPOINT_BITS: usize = 16;
const VP8L_32_BITSTREAM_CHECKPOINT_BITS: usize = 32;
const VP8L_64_BITSTREAM_CHECKPOINT_BITS: usize = 64;
const VP8L_128_BITSTREAM_CHECKPOINT_BITS: usize = 128;
const VP8L_256_BITSTREAM_CHECKPOINT_BITS: usize = 256;
const VP8L_512_BITSTREAM_CHECKPOINT_BITS: usize = 512;
const VP8L_1024_BITSTREAM_CHECKPOINT_BITS: usize = 1_024;
const VP8L_2048_BITSTREAM_CHECKPOINT_BITS: usize = 2_048;
const VP8L_4096_BITSTREAM_CHECKPOINT_BITS: usize = 4_096;
const VP8L_8192_BITSTREAM_CHECKPOINT_BITS: usize = 8_192;
const VP8L_16384_BITSTREAM_CHECKPOINT_BITS: usize = 16_384;
const VP8L_32768_BITSTREAM_CHECKPOINT_BITS: usize = 32_768;
const VP8L_HUFFMAN_TREE_CHECKPOINT_NODES: usize = 64;
const VP8L_HUFFMAN_TOKEN_CHECKPOINTS: usize = 16;
// The largest VP8L alphabet has 281 leaves; a depth-first binary traversal
// never needs more than one pending entry per leaf, so this fixed stack avoids
// a heap allocation while retaining ample space for malformed defensive input.
const VP8L_HUFFMAN_STACK_ENTRIES: usize = 512;
const VP8L_65536_BITSTREAM_CHECKPOINT_BITS: usize = 65_536;
const VP8L_131072_BITSTREAM_CHECKPOINT_BITS: usize = 131_072;
const VP8L_262144_BITSTREAM_CHECKPOINT_BITS: usize = 262_144;
const VP8L_524288_BITSTREAM_CHECKPOINT_BITS: usize = 524_288;
const VP8L_1048576_BITSTREAM_CHECKPOINT_BITS: usize = 1_048_576;
const VP8L_2097152_BITSTREAM_CHECKPOINT_BITS: usize = 2_097_152;
const VP8L_HUFFMAN_CHECKPOINT_SYMBOLS: usize = 64;
const VP8L_HISTOGRAM_SAMPLING_CHECKPOINT_SYMBOLS: usize = 1_024;
const VP8L_TOKEN_STREAM_CHECKPOINT_PIXELS: usize = 256;
const WEBP_PALETTE_CHECKPOINT_VALUES: usize = 64;
const WEBP_ALPHA_PALETTE_CHECKPOINT_VALUES: usize = 64;

struct AlphaPalette {
    values: [u8; VP8L_MAX_PALETTE_ENTRIES],
    len: usize,
}

trait BitWriterCheckpoint: Clone {
    fn checkpoint_bits(&mut self, written: usize) -> Result<(), EncodingError>;
    fn checkpoint_output_bytes(&mut self, emitted: usize) -> Result<(), EncodingError>;

    #[cfg(coverage)]
    #[coverage(off)]
    fn coverage_remaining(&self) -> Option<usize> {
        None
    }
}

#[derive(Clone, Copy)]
struct NoopBitWriterCheckpoint {
    #[cfg(coverage)]
    fail_after: usize,
}

#[allow(clippy::derivable_impls)]
impl Default for NoopBitWriterCheckpoint {
    fn default() -> Self {
        Self {
            #[cfg(coverage)]
            fail_after: usize::MAX,
        }
    }
}

impl NoopBitWriterCheckpoint {
    #[inline(always)]
    fn checkpoint_event(&mut self) -> Result<(), EncodingError> {
        #[cfg(coverage)]
        {
            if self.fail_after == 0 {
                return Err(EncodingError::Cancelled);
            }
            self.fail_after = self.fail_after.saturating_sub(1);
        }
        Ok(())
    }
}

impl BitWriterCheckpoint for NoopBitWriterCheckpoint {
    #[inline(always)]
    fn checkpoint_bits(&mut self, _written: usize) -> Result<(), EncodingError> {
        self.checkpoint_event()
    }

    #[inline(always)]
    fn checkpoint_output_bytes(&mut self, _emitted: usize) -> Result<(), EncodingError> {
        self.checkpoint_event()
    }

    #[cfg(coverage)]
    #[coverage(off)]
    fn coverage_remaining(&self) -> Option<usize> {
        Some(self.fail_after)
    }
}

#[derive(Clone, Copy)]
struct TokenBitWriterCheckpoint<'a> {
    token: &'a crate::CancellationToken,
    written_bits: usize,
    output_bytes: usize,
}

impl BitWriterCheckpoint for TokenBitWriterCheckpoint<'_> {
    #[cfg_attr(coverage, inline(never))]
    fn checkpoint_bits(&mut self, written: usize) -> Result<(), EncodingError> {
        let previous = self.written_bits;
        self.written_bits = self.written_bits.saturating_add(written);
        // Every logical interval counts the same written bits. Walk the
        // finest 8-bit intervals once and nest the larger intervals so a
        // token-aware write does not rescan the same range.
        let mut previous_8_interval = previous / VP8L_8_BITSTREAM_CHECKPOINT_BITS;
        let current_8_interval = self.written_bits / VP8L_8_BITSTREAM_CHECKPOINT_BITS;
        while previous_8_interval < current_8_interval {
            previous_8_interval = previous_8_interval.saturating_add(1);
            check_token(Some(self.token))?;
            if previous_8_interval.is_multiple_of(
                VP8L_16_BITSTREAM_CHECKPOINT_BITS / VP8L_8_BITSTREAM_CHECKPOINT_BITS,
            ) {
                check_token(Some(self.token))?;
                if previous_8_interval.is_multiple_of(
                    VP8L_32_BITSTREAM_CHECKPOINT_BITS / VP8L_8_BITSTREAM_CHECKPOINT_BITS,
                ) {
                    check_token(Some(self.token))?;
                    if previous_8_interval.is_multiple_of(
                        VP8L_64_BITSTREAM_CHECKPOINT_BITS / VP8L_8_BITSTREAM_CHECKPOINT_BITS,
                    ) {
                        check_token(Some(self.token))?;
                        if previous_8_interval.is_multiple_of(
                            VP8L_128_BITSTREAM_CHECKPOINT_BITS / VP8L_8_BITSTREAM_CHECKPOINT_BITS,
                        ) {
                            check_token(Some(self.token))?;
                            if previous_8_interval.is_multiple_of(
                                VP8L_256_BITSTREAM_CHECKPOINT_BITS
                                    / VP8L_8_BITSTREAM_CHECKPOINT_BITS,
                            ) {
                                check_token(Some(self.token))?;
                                if previous_8_interval.is_multiple_of(
                                    VP8L_512_BITSTREAM_CHECKPOINT_BITS
                                        / VP8L_8_BITSTREAM_CHECKPOINT_BITS,
                                ) {
                                    check_token(Some(self.token))?;
                                    if previous_8_interval.is_multiple_of(
                                        VP8L_1024_BITSTREAM_CHECKPOINT_BITS
                                            / VP8L_8_BITSTREAM_CHECKPOINT_BITS,
                                    ) {
                                        check_token(Some(self.token))?;
                                        if previous_8_interval.is_multiple_of(
                                            VP8L_2048_BITSTREAM_CHECKPOINT_BITS
                                                / VP8L_8_BITSTREAM_CHECKPOINT_BITS,
                                        ) {
                                            check_token(Some(self.token))?;
                                            if previous_8_interval.is_multiple_of(
                                                VP8L_4096_BITSTREAM_CHECKPOINT_BITS
                                                    / VP8L_8_BITSTREAM_CHECKPOINT_BITS,
                                            ) {
                                                check_token(Some(self.token))?;
                                                if previous_8_interval.is_multiple_of(
                                                    VP8L_8192_BITSTREAM_CHECKPOINT_BITS
                                                        / VP8L_8_BITSTREAM_CHECKPOINT_BITS,
                                                ) {
                                                    check_token(Some(self.token))?;
                                                    if previous_8_interval.is_multiple_of(
                                                        VP8L_16384_BITSTREAM_CHECKPOINT_BITS
                                                            / VP8L_8_BITSTREAM_CHECKPOINT_BITS,
                                                    ) {
                                                        check_token(Some(self.token))?;
                                                        if previous_8_interval.is_multiple_of(
                                                            VP8L_32768_BITSTREAM_CHECKPOINT_BITS
                                                                / VP8L_8_BITSTREAM_CHECKPOINT_BITS,
                                                        ) {
                                                            check_token(Some(self.token))?;
                                                            if previous_8_interval.is_multiple_of(
                                                                VP8L_65536_BITSTREAM_CHECKPOINT_BITS
                                                                    / VP8L_8_BITSTREAM_CHECKPOINT_BITS,
                                                            ) {
                                                                check_token(Some(self.token))?;
                                                                if previous_8_interval.is_multiple_of(
                                                                    VP8L_131072_BITSTREAM_CHECKPOINT_BITS
                                                                        / VP8L_8_BITSTREAM_CHECKPOINT_BITS,
                                                                ) {
                                                                    check_token(Some(self.token))?;
                                                                    if previous_8_interval.is_multiple_of(
                                                                        VP8L_262144_BITSTREAM_CHECKPOINT_BITS
                                                                            / VP8L_8_BITSTREAM_CHECKPOINT_BITS,
                                                                    ) {
                                                                        check_token(Some(self.token))?;
                                                                        if previous_8_interval.is_multiple_of(
                                                                            VP8L_524288_BITSTREAM_CHECKPOINT_BITS
                                                                                / VP8L_8_BITSTREAM_CHECKPOINT_BITS,
                                                                        ) {
                                                                            check_token(Some(self.token))?;
                                                                            if previous_8_interval.is_multiple_of(
                                                                                VP8L_1048576_BITSTREAM_CHECKPOINT_BITS
                                                                                    / VP8L_8_BITSTREAM_CHECKPOINT_BITS,
                                                                            ) {
                                                                                check_token(Some(self.token))?;
                                                                                if previous_8_interval.is_multiple_of(
                                                                                    VP8L_2097152_BITSTREAM_CHECKPOINT_BITS
                                                                                        / VP8L_8_BITSTREAM_CHECKPOINT_BITS,
                                                                                ) {
                                                                                    check_token(Some(self.token))?;
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    #[cfg_attr(coverage, inline(never))]
    fn checkpoint_output_bytes(&mut self, emitted: usize) -> Result<(), EncodingError> {
        let previous = self.output_bytes;
        self.output_bytes = self.output_bytes.saturating_add(emitted);
        let mut previous_interval = previous / VP8L_OUTPUT_CHECKPOINT_BYTES;
        let current_interval = self.output_bytes / VP8L_OUTPUT_CHECKPOINT_BYTES;
        while previous_interval < current_interval {
            previous_interval = previous_interval.saturating_add(1);
            check_token(Some(self.token))?;
        }
        Ok(())
    }

    #[cfg(coverage)]
    #[coverage(off)]
    fn coverage_remaining(&self) -> Option<usize> {
        self.token.coverage_remaining_checks()
    }
}

struct BitWriter<'a, C: BitWriterCheckpoint> {
    writer: &'a mut Vec<u8>,
    buffer: u64,
    nbits: u8,
    checkpoint: C,
}

impl<C: BitWriterCheckpoint> BitWriter<'_, C> {
    #[inline(always)]
    fn write_bits_unchecked(&mut self, bits: u64, nbits: u8) {
        debug_assert!(nbits <= 64);

        self.buffer |= bits << self.nbits;
        self.nbits += nbits;

        if self.nbits >= 64 {
            self.writer.extend_from_slice(&self.buffer.to_le_bytes());
            self.nbits -= 64;
            self.buffer = bits.checked_shr(u32::from(nbits - self.nbits)).unwrap_or(0);
        }
        debug_assert!(self.nbits < 64);
    }

    #[inline(always)]
    fn write_bits(&mut self, bits: u64, nbits: u8) -> Result<(), EncodingError> {
        let before = self.writer.len();
        self.write_bits_unchecked(bits, nbits);
        self.checkpoint.checkpoint_bits(usize::from(nbits))?;
        self.checkpoint
            .checkpoint_output_bytes(self.writer.len().saturating_sub(before))
    }

    #[inline(always)]
    fn flush_unchecked(&mut self) {
        if !self.nbits.is_multiple_of(8) {
            self.write_bits_unchecked(0, 8 - self.nbits % 8);
        }
        if self.nbits > 0 {
            self.writer
                .extend_from_slice(&self.buffer.to_le_bytes()[..self.nbits as usize / 8]);
            self.buffer = 0;
            self.nbits = 0;
        }
    }

    #[inline(always)]
    fn flush(&mut self) -> Result<(), EncodingError> {
        let before = self.writer.len();
        self.flush_unchecked();
        self.checkpoint
            .checkpoint_output_bytes(self.writer.len().saturating_sub(before))
    }
}

#[derive(Clone, Copy)]
enum HuffmanEncodingNode {
    Leaf(usize),
    Branch(usize, usize),
}

#[derive(Clone, Copy)]
struct WeightedHuffmanEncodingNode {
    count: u32,
    sort_value: isize,
    node: usize,
}

// Every pop occurs while at least two nodes remain.
#[allow(clippy::unwrap_used)]
fn build_huffman_tree(
    frequencies: &[u32],
    lengths: &mut [u8],
    codes: &mut [u16],
    scratch: &mut HuffmanTreeScratch<'_>,
    length_limit: u8,
    token: Option<&crate::CancellationToken>,
) -> Result<bool, EncodingError> {
    check_token(token)?;
    assert_eq!(frequencies.len(), lengths.len());
    assert_eq!(frequencies.len(), codes.len());

    let optimized = &mut *scratch.optimized_frequencies;
    let nodes = &mut *scratch.nodes;
    let node_sort_scratch = &mut *scratch.node_sort_scratch;
    let node_arena = &mut *scratch.node_arena;
    nodes.clear();
    node_sort_scratch.clear();
    node_arena.clear();
    optimized.clear();
    optimized.extend_from_slice(frequencies);
    optimize_huffman_for_rle_with_checkpoint(optimized, scratch.huffman_rle_good, token)?;
    let optimized_symbol_count = if let Some(token) = token {
        let mut count = 0_usize;
        for (index, &frequency) in optimized.iter().enumerate() {
            if (index + 1).is_multiple_of(VP8L_HUFFMAN_CHECKPOINT_SYMBOLS) {
                check_token(Some(token))?;
            }
            count += usize::from(frequency != 0);
        }
        count
    } else {
        optimized
            .iter()
            .filter(|&&frequency| frequency != 0)
            .count()
    };
    if optimized_symbol_count <= 1 {
        lengths.fill(0);
        codes.fill(0);
        if let Some(symbol) = optimized.iter().position(|&frequency| frequency != 0) {
            lengths[symbol] = 1;
        }
        return Ok(false);
    }
    let mut count_min = 1_u32;
    loop {
        check_token(token)?;
        nodes.clear();
        node_arena.clear();
        if let Some(token) = token {
            for (value, &frequency) in optimized.iter().enumerate() {
                if (value + 1).is_multiple_of(VP8L_HUFFMAN_CHECKPOINT_SYMBOLS) {
                    check_token(Some(token))?;
                }
                if frequency != 0 {
                    let node = node_arena.len();
                    node_arena.push(HuffmanEncodingNode::Leaf(value));
                    nodes.push(WeightedHuffmanEncodingNode {
                        count: frequency.max(count_min),
                        sort_value: value as isize,
                        node,
                    });
                }
            }
        } else {
            for (value, &frequency) in optimized.iter().enumerate() {
                if frequency != 0 {
                    let node = node_arena.len();
                    node_arena.push(HuffmanEncodingNode::Leaf(value));
                    nodes.push(WeightedHuffmanEncodingNode {
                        count: frequency.max(count_min),
                        sort_value: value as isize,
                        node,
                    });
                }
            }
        }
        if let Some(token) = token {
            // The token-aware path keeps the stable ordering of the original
            // sort with a bounded bottom-up merge sort. A large fixed alphabet
            // can otherwise spend an entire comparison sort between the
            // surrounding tree checkpoints; keeping O(n log n) here also
            // avoids turning cancellation-aware encoding into a quadratic
            // slow path.
            let mut comparisons = 0_usize;
            node_sort_scratch.clone_from(nodes);
            let mut width = 1_usize;
            while width < nodes.len() {
                let mut start = 0_usize;
                while start < nodes.len() {
                    let middle = start.saturating_add(width).min(nodes.len());
                    let end = middle.saturating_add(width).min(nodes.len());
                    let mut left = start;
                    let mut right = middle;
                    for slot in &mut node_sort_scratch[start..end] {
                        let take_left =
                            if left == middle {
                                false
                            } else if right == end {
                                true
                            } else {
                                comparisons = comparisons.saturating_add(1);
                                if comparisons.is_multiple_of(VP8L_HUFFMAN_TREE_CHECKPOINT_NODES) {
                                    check_token(Some(token))?;
                                }
                                nodes[right].count.cmp(&nodes[left].count).then_with(|| {
                                    nodes[left].sort_value.cmp(&nodes[right].sort_value)
                                }) != core::cmp::Ordering::Greater
                            };
                        if take_left {
                            *slot = nodes[left];
                            left += 1;
                        } else {
                            *slot = nodes[right];
                            right += 1;
                        }
                    }
                    start = end;
                }
                core::mem::swap(nodes, node_sort_scratch);
                width = width.saturating_mul(2);
            }
        } else {
            nodes.sort_by(|left, right| {
                right
                    .count
                    .cmp(&left.count)
                    .then_with(|| left.sort_value.cmp(&right.sort_value))
            });
        }
        while nodes.len() > 1 {
            check_token(token)?;
            let left = nodes.pop().unwrap();
            let right = nodes.pop().unwrap();
            let count = left.count + right.count;
            let node = node_arena.len();
            node_arena.push(HuffmanEncodingNode::Branch(left.node, right.node));
            let position = if token.is_some() {
                let mut position = nodes.len();
                for (index, node) in nodes.iter().enumerate() {
                    if index.is_multiple_of(VP8L_HUFFMAN_TREE_CHECKPOINT_NODES) {
                        check_token(token)?;
                    }
                    if node.count <= count {
                        position = index;
                        break;
                    }
                }
                position
            } else {
                nodes
                    .iter()
                    .position(|node| node.count <= count)
                    .unwrap_or(nodes.len())
            };
            nodes.insert(
                position,
                WeightedHuffmanEncodingNode {
                    count,
                    sort_value: -1,
                    node,
                },
            );
        }

        lengths.fill(0);
        let mut stack = [(0usize, 0_u8); VP8L_HUFFMAN_STACK_ENTRIES];
        let mut stack_len = 1;
        stack[0] = (nodes[0].node, 0_u8);
        while stack_len != 0 {
            stack_len -= 1;
            let (node, depth) = stack[stack_len];
            check_token(token)?;
            match node_arena[node] {
                HuffmanEncodingNode::Leaf(value) => lengths[value] = depth,
                HuffmanEncodingNode::Branch(left, right) => {
                    debug_assert!(stack_len + 2 <= VP8L_HUFFMAN_STACK_ENTRIES);
                    stack[stack_len] = (right, depth + 1);
                    stack_len += 1;
                    stack[stack_len] = (left, depth + 1);
                    stack_len += 1;
                }
            }
        }
        // The token-aware path keeps the fixed-alphabet depth scan
        // interruptible without adding a callback to the ordinary path.
        let maximum_length = if let Some(token) = token {
            let mut maximum = 0_u8;
            for (index, &length) in lengths.iter().enumerate() {
                maximum = maximum.max(length);
                if (index + 1).is_multiple_of(VP8L_HUFFMAN_CHECKPOINT_SYMBOLS) {
                    check_token(Some(token))?;
                }
            }
            maximum
        } else {
            lengths.iter().copied().max().unwrap_or(0)
        };
        if maximum_length <= length_limit {
            break;
        }
        count_min *= 2;
    }

    nodes.clear();
    node_sort_scratch.clear();
    node_arena.clear();

    // Assign codes
    codes.fill(0);
    let mut code = 0u32;
    if let Some(token) = token {
        let mut scanned = 0_usize;
        for len in 1..=length_limit {
            check_token(Some(token))?;
            for (i, &length) in lengths.iter().enumerate() {
                if length == len {
                    codes[i] = (code as u16).reverse_bits() >> (16 - len);
                    code += 1;
                }
                scanned += 1;
                if scanned.is_multiple_of(VP8L_HUFFMAN_CHECKPOINT_SYMBOLS) {
                    check_token(Some(token))?;
                }
            }
            code <<= 1;
        }
    } else {
        for len in 1..=length_limit {
            for (i, &length) in lengths.iter().enumerate() {
                if length == len {
                    codes[i] = (code as u16).reverse_bits() >> (16 - len);
                    code += 1;
                }
            }
            code <<= 1;
        }
    }
    Ok(true)
}

fn optimize_huffman_for_rle(counts: &mut [u32], good: &mut Vec<bool>) {
    let Some(length) = counts.iter().rposition(|&count| count != 0).map(|i| i + 1) else {
        return;
    };
    good.resize(length, false);
    good.fill(false);
    let mut symbol = counts[0];
    let mut stride = 0;
    for i in 0..=length {
        if i == length || counts[i] != symbol {
            if (symbol == 0 && stride >= 5) || (symbol != 0 && stride >= 7) {
                good[i - stride..i].fill(true);
            }
            stride = 1;
            if i != length {
                symbol = counts[i];
            }
        } else {
            stride += 1;
        }
    }

    stride = 0;
    let mut limit = counts[0];
    let mut sum = 0_u32;
    for i in 0..=length {
        if i == length || good[i] || (i != 0 && good[i - 1]) || counts[i].abs_diff(limit) >= 4 {
            if stride >= 4 || (stride >= 3 && sum == 0) {
                let mut count = (sum + stride as u32 / 2) / stride as u32;
                count = count.max(1);
                if sum == 0 {
                    count = 0;
                }
                counts[i - stride..i].fill(count);
            }
            stride = 0;
            sum = 0;
            limit = if i + 3 < length {
                (counts[i] + counts[i + 1] + counts[i + 2] + counts[i + 3] + 2) / 4
            } else if i < length {
                counts[i]
            } else {
                0
            };
        }
        stride += 1;
        if i != length {
            sum += counts[i];
            if stride >= 4 {
                limit = (sum + stride as u32 / 2) / stride as u32;
            }
        }
    }
}

fn optimize_huffman_for_rle_with_checkpoint(
    counts: &mut [u32],
    good: &mut Vec<bool>,
    token: Option<&crate::CancellationToken>,
) -> Result<(), EncodingError> {
    if token.is_none() {
        optimize_huffman_for_rle(counts, good);
        return Ok(());
    }
    let length = {
        let mut scanned = 0_usize;
        let mut last_nonzero = None;
        for (index, &count) in counts.iter().enumerate().rev() {
            scanned += 1;
            if scanned.is_multiple_of(VP8L_HUFFMAN_CHECKPOINT_SYMBOLS) {
                check_token(token)?;
            }
            if count != 0 {
                last_nonzero = Some(index + 1);
                break;
            }
        }
        let Some(length) = last_nonzero else {
            return Ok(());
        };
        length
    };
    good.resize(length, false);
    good.fill(false);
    let mut symbol = counts[0];
    let mut stride = 0;
    for i in 0..=length {
        if i.is_multiple_of(VP8L_HUFFMAN_CHECKPOINT_SYMBOLS) {
            check_token(token)?;
        }
        if i == length || counts[i] != symbol {
            if (symbol == 0 && stride >= 5) || (symbol != 0 && stride >= 7) {
                for (index, value) in good[i - stride..i].iter_mut().enumerate() {
                    *value = true;
                    if (index + 1).is_multiple_of(VP8L_HUFFMAN_CHECKPOINT_SYMBOLS) {
                        check_token(token)?;
                    }
                }
            }
            stride = 1;
            if i != length {
                symbol = counts[i];
            }
        } else {
            stride += 1;
        }
    }

    stride = 0;
    let mut limit = counts[0];
    let mut sum = 0_u32;
    for i in 0..=length {
        if i.is_multiple_of(VP8L_HUFFMAN_CHECKPOINT_SYMBOLS) {
            check_token(token)?;
        }
        if i == length || good[i] || (i != 0 && good[i - 1]) || counts[i].abs_diff(limit) >= 4 {
            if stride >= 4 || (stride >= 3 && sum == 0) {
                let mut count = (sum + stride as u32 / 2) / stride as u32;
                count = count.max(1);
                if sum == 0 {
                    count = 0;
                }
                for (index, value) in counts[i - stride..i].iter_mut().enumerate() {
                    *value = count;
                    if (index + 1).is_multiple_of(VP8L_HUFFMAN_CHECKPOINT_SYMBOLS) {
                        check_token(token)?;
                    }
                }
            }
            stride = 0;
            sum = 0;
            limit = if i + 3 < length {
                (counts[i] + counts[i + 1] + counts[i + 2] + counts[i + 3] + 2) / 4
            } else if i < length {
                counts[i]
            } else {
                0
            };
        }
        stride += 1;
        if i != length {
            sum += counts[i];
            if stride >= 4 {
                limit = (sum + stride as u32 / 2) / stride as u32;
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct HuffmanToken {
    code: u8,
    extra: u8,
}

struct HuffmanTreeScratch<'a> {
    huffman_tokens: &'a mut Vec<HuffmanToken>,
    optimized_frequencies: &'a mut Vec<u32>,
    huffman_rle_good: &'a mut Vec<bool>,
    nodes: &'a mut Vec<WeightedHuffmanEncodingNode>,
    node_sort_scratch: &'a mut Vec<WeightedHuffmanEncodingNode>,
    node_arena: &'a mut Vec<HuffmanEncodingNode>,
}

fn compressed_huffman_tokens_into(lengths: &[u8], tokens: &mut Vec<HuffmanToken>) {
    tokens.clear();
    let mut previous = 8;
    let mut i = 0;
    while i < lengths.len() {
        let value = lengths[i];
        let mut end = i + 1;
        while end < lengths.len() && lengths[end] == value {
            end += 1;
        }
        let mut repetitions = end - i;
        if value == 0 {
            // A run always contains at least one value. The long-run case
            // subtracts 138 only when that leaves a non-zero remainder, and
            // every shorter case exits immediately.
            loop {
                if repetitions < 3 {
                    tokens.extend((0..repetitions).map(|_| HuffmanToken { code: 0, extra: 0 }));
                    break;
                } else if repetitions < 11 {
                    tokens.push(HuffmanToken {
                        code: 17,
                        extra: (repetitions - 3) as u8,
                    });
                    break;
                } else if repetitions < 139 {
                    tokens.push(HuffmanToken {
                        code: 18,
                        extra: (repetitions - 11) as u8,
                    });
                    break;
                } else {
                    tokens.push(HuffmanToken {
                        code: 18,
                        extra: 0x7f,
                    });
                    repetitions -= 138;
                }
            }
        } else {
            if value != previous {
                tokens.push(HuffmanToken {
                    code: value,
                    extra: 0,
                });
                repetitions -= 1;
            }
            while repetitions != 0 {
                if repetitions < 3 {
                    tokens.extend((0..repetitions).map(|_| HuffmanToken {
                        code: value,
                        extra: 0,
                    }));
                    break;
                } else if repetitions < 7 {
                    tokens.push(HuffmanToken {
                        code: 16,
                        extra: (repetitions - 3) as u8,
                    });
                    break;
                } else {
                    tokens.push(HuffmanToken { code: 16, extra: 3 });
                    repetitions -= 6;
                }
            }
            previous = value;
        }
        i = end;
    }
}

#[cfg_attr(coverage, inline(never))]
fn compressed_huffman_tokens_with_checkpoint(
    lengths: &[u8],
    tokens: &mut Vec<HuffmanToken>,
    token: Option<&crate::CancellationToken>,
) -> Result<(), EncodingError> {
    tokens.clear();
    if token.is_none() {
        compressed_huffman_tokens_into(lengths, tokens);
        return Ok(());
    }
    let mut emitted_tokens = 0_usize;
    let mut emit = |huffman_token: HuffmanToken| -> Result<(), EncodingError> {
        tokens.push(huffman_token);
        emitted_tokens += 1;
        if emitted_tokens.is_multiple_of(VP8L_HUFFMAN_TOKEN_CHECKPOINTS) {
            check_token(token)?;
        }
        Ok(())
    };
    let mut previous = 8;
    let mut next_checkpoint = VP8L_HUFFMAN_CHECKPOINT_SYMBOLS;
    let mut i = 0;
    while i < lengths.len() {
        let value = lengths[i];
        let mut end = i + 1;
        while end < lengths.len() && lengths[end] == value {
            end += 1;
            // Poll while scanning a long equal-length run instead of waiting
            // for the run to finish. The no-token path returns to the
            // original helper above, so this finer boundary is only paid by
            // caller-controlled work-budget encodes.
            while end >= next_checkpoint {
                check_token(token)?;
                next_checkpoint = next_checkpoint.saturating_add(VP8L_HUFFMAN_CHECKPOINT_SYMBOLS);
            }
        }
        while end >= next_checkpoint {
            check_token(token)?;
            next_checkpoint = next_checkpoint.saturating_add(VP8L_HUFFMAN_CHECKPOINT_SYMBOLS);
        }
        let mut repetitions = end - i;
        if value == 0 {
            // A run always contains at least one value. The long-run case
            // subtracts 138 only when that leaves a non-zero remainder, and
            // every shorter case exits immediately.
            loop {
                if repetitions < 3 {
                    for _ in 0..repetitions {
                        emit(HuffmanToken { code: 0, extra: 0 })?;
                    }
                    break;
                } else if repetitions < 11 {
                    let huffman_token = HuffmanToken {
                        code: 17,
                        extra: (repetitions - 3) as u8,
                    };
                    emit(huffman_token)?;
                    break;
                } else if repetitions < 139 {
                    let huffman_token = HuffmanToken {
                        code: 18,
                        extra: (repetitions - 11) as u8,
                    };
                    emit(huffman_token)?;
                    break;
                } else {
                    let huffman_token = HuffmanToken {
                        code: 18,
                        extra: 0x7f,
                    };
                    emit(huffman_token)?;
                    repetitions -= 138;
                }
            }
        } else {
            if value != previous {
                let huffman_token = HuffmanToken {
                    code: value,
                    extra: 0,
                };
                emit(huffman_token)?;
                repetitions -= 1;
            }
            while repetitions != 0 {
                if repetitions < 3 {
                    for _ in 0..repetitions {
                        let huffman_token = HuffmanToken {
                            code: value,
                            extra: 0,
                        };
                        emit(huffman_token)?;
                    }
                    break;
                } else if repetitions < 7 {
                    let huffman_token = HuffmanToken {
                        code: 16,
                        extra: (repetitions - 3) as u8,
                    };
                    emit(huffman_token)?;
                    break;
                } else {
                    emit(HuffmanToken { code: 16, extra: 3 })?;
                    repetitions -= 6;
                }
            }
            previous = value;
        }
        i = end;
    }
    Ok(())
}

#[cfg_attr(coverage, inline(never))]
fn write_huffman_tree<C: BitWriterCheckpoint>(
    w: &mut BitWriter<'_, C>,
    frequencies: &[u32],
    lengths: &mut [u8],
    codes: &mut [u16],
    scratch: &mut HuffmanTreeScratch<'_>,
    token: Option<&crate::CancellationToken>,
) -> Result<(), EncodingError> {
    build_huffman_tree(frequencies, lengths, codes, scratch, 15, token)?;
    let mut symbols = [0_usize; 3];
    let symbol_count = if let Some(token) = token {
        let mut count = 0;
        for (index, &length) in lengths.iter().enumerate() {
            if length != 0 {
                symbols[count] = index;
                count += 1;
                if count == symbols.len() {
                    break;
                }
            }
            if (index + 1).is_multiple_of(VP8L_HUFFMAN_CHECKPOINT_SYMBOLS) {
                check_token(Some(token))?;
            }
        }
        count
    } else {
        let mut count = 0;
        for (index, &length) in lengths.iter().enumerate() {
            if length != 0 {
                symbols[count] = index;
                count += 1;
                if count == symbols.len() {
                    break;
                }
            }
        }
        count
    };
    if symbol_count <= 2 && symbols[..symbol_count].iter().all(|&symbol| symbol < 256) {
        let first = symbols[0];
        w.write_bits(1, 1)?;
        w.write_bits(u64::from(symbol_count == 2), 1)?;
        if first <= 1 {
            w.write_bits(0, 1)?;
            w.write_bits(first as u64, 1)?;
        } else {
            w.write_bits(1, 1)?;
            w.write_bits(first as u64, 8)?;
        }
        if symbol_count == 2 {
            w.write_bits(symbols[1] as u64, 8)?;
        }
        lengths.fill(0);
        codes.fill(0);
        if symbol_count == 2 {
            lengths[symbols[0]] = 1;
            lengths[symbols[1]] = 1;
            codes[symbols[1]] = 1;
        }
        return Ok(());
    }
    compressed_huffman_tokens_with_checkpoint(lengths, scratch.huffman_tokens, token)?;
    let mut code_length_lengths = [0u8; 19];
    let mut code_length_codes = [0u16; 19];
    let mut code_length_frequencies = [0u32; 19];
    if let Some(token) = token {
        for (index, huffman_token) in scratch.huffman_tokens.iter().enumerate() {
            code_length_frequencies[usize::from(huffman_token.code)] += 1;
            if (index + 1).is_multiple_of(VP8L_HUFFMAN_TOKEN_CHECKPOINTS) {
                check_token(Some(token))?;
            }
        }
    } else {
        for huffman_token in scratch.huffman_tokens.iter() {
            code_length_frequencies[usize::from(huffman_token.code)] += 1;
        }
    }
    build_huffman_tree(
        &code_length_frequencies,
        &mut code_length_lengths,
        &mut code_length_codes,
        scratch,
        7,
        token,
    )?;
    const CODE_LENGTH_ORDER: [usize; 19] = [
        17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
    ];

    // Write the huffman tree
    w.write_bits(0, 1)?; // normal huffman tree
    let mut codes_to_store = 19;
    while codes_to_store > 4 && code_length_lengths[CODE_LENGTH_ORDER[codes_to_store - 1]] == 0 {
        check_token(token)?;
        codes_to_store -= 1;
    }
    w.write_bits((codes_to_store - 4) as u64, 4)?;
    for &symbol in &CODE_LENGTH_ORDER[..codes_to_store] {
        w.write_bits(u64::from(code_length_lengths[symbol]), 3)?;
    }

    if code_length_lengths
        .iter()
        .filter(|&&length| length != 0)
        .count()
        <= 1
    {
        code_length_lengths.fill(0);
        code_length_codes.fill(0);
    }
    let mut trimmed_length = scratch.huffman_tokens.len();
    let mut trailing_zero_bits = 0;
    // The normal-tree path always emits at least one non-zero code-length
    // token before trailing zero-repeat tokens.
    if token.is_some() {
        loop {
            let huffman_token = scratch.huffman_tokens[trimmed_length - 1];
            if !matches!(huffman_token.code, 0 | 17 | 18) {
                break;
            }
            trimmed_length -= 1;
            trailing_zero_bits += usize::from(code_length_lengths[usize::from(huffman_token.code)]);
            trailing_zero_bits += match huffman_token.code {
                17 => 3,
                18 => 7,
                _ => 0,
            };
        }
    } else {
        loop {
            let huffman_token = scratch.huffman_tokens[trimmed_length - 1];
            if !matches!(huffman_token.code, 0 | 17 | 18) {
                break;
            }
            trimmed_length -= 1;
            trailing_zero_bits += usize::from(code_length_lengths[usize::from(huffman_token.code)]);
            trailing_zero_bits += match huffman_token.code {
                17 => 3,
                18 => 7,
                _ => 0,
            };
        }
    }
    let write_trimmed = trailing_zero_bits > 12;
    w.write_bits(u64::from(write_trimmed), 1)?;
    let token_count = if write_trimmed {
        if trimmed_length == 2 {
            w.write_bits(0, 5)?;
        } else {
            let nbits = (trimmed_length - 2).ilog2() as usize;
            let pairs = nbits / 2 + 1;
            w.write_bits((pairs - 1) as u64, 3)?;
            w.write_bits((trimmed_length - 2) as u64, (pairs * 2) as u8)?;
        }
        trimmed_length
    } else {
        scratch.huffman_tokens.len()
    };
    for (index, huffman_token) in scratch.huffman_tokens[..token_count].iter().enumerate() {
        // Bit/output checkpoints already bound the emitted bitstream. Keep
        // this structural token walk cooperative at the same 16-entry
        // interval used by the code-length frequency and trim scans instead
        // of paying one cancellation poll for every token.
        if (index + 1).is_multiple_of(VP8L_HUFFMAN_TOKEN_CHECKPOINTS) || index + 1 == token_count {
            check_token(token)?;
        }
        let symbol = usize::from(huffman_token.code);
        let code = u64::from(code_length_codes[symbol]);
        let code_length = code_length_lengths[symbol];
        w.write_bits(code, code_length)?;
        let bits = match huffman_token.code {
            16 => 2,
            17 => 3,
            18 => 7,
            _ => 0,
        };
        w.write_bits(u64::from(huffman_token.extra), bits)?;
    }
    Ok(())
}

const fn length_to_symbol(len: usize) -> (usize, u8) {
    if len <= 4 {
        return (len - 1, 0);
    }
    let len = len - 1;
    let highest_bit = len.ilog2() as usize;
    let second_highest_bit = (len >> (highest_bit - 1)) & 1;
    let extra_bits = highest_bit - 1;
    let symbol = 2 * highest_bit + second_highest_bit;
    (symbol, extra_bits as u8)
}

#[inline]
fn channels(pixel: u32) -> [usize; 4] {
    [
        ((pixel >> 16) & 0xff) as usize,
        ((pixel >> 8) & 0xff) as usize,
        (pixel & 0xff) as usize,
        (pixel >> 24) as usize,
    ]
}

fn write_image_stream<C: BitWriterCheckpoint>(
    w: &mut BitWriter<'_, C>,
    pixels: &[u32],
    width: usize,
    write_meta_huffman_bit: bool,
    output_scratch: &mut Vec<u8>,
    token_scratch: &mut TokenStreamScratch,
    token: Option<&crate::CancellationToken>,
) -> Result<(), EncodingError> {
    write_image_stream_configured_with_scratch(
        w,
        pixels,
        width,
        write_meta_huffman_bit,
        3,
        80,
        11,
        output_scratch,
        token_scratch,
        token,
    )
}

#[allow(clippy::too_many_arguments, clippy::unwrap_used)]
#[cfg_attr(coverage, inline(never))]
fn write_image_stream_configured_with_scratch<C: BitWriterCheckpoint>(
    w: &mut BitWriter<'_, C>,
    pixels: &[u32],
    width: usize,
    write_meta_huffman_bit: bool,
    histogram_bits: u8,
    quality: u32,
    max_cache_bits: u8,
    output_scratch: &mut Vec<u8>,
    token_scratch: &mut TokenStreamScratch,
    token: Option<&crate::CancellationToken>,
) -> Result<(), EncodingError> {
    check_token(token)?;
    let mut candidates = backward_refs::candidates(
        pixels,
        width,
        write_meta_huffman_bit,
        quality,
        max_cache_bits,
        &mut token_scratch.candidates,
        token,
    )?;

    // Candidate trials share the already-emitted prefix. Keeping that prefix
    // out of each trial avoids an O(prefix × candidate-count) copy/allocation;
    // leave the parent writer in place until the winning suffix is selected.
    let result = (|| -> Result<(), EncodingError> {
        let initial_byte_length = w.writer.len();
        let initial_buffer = w.buffer;
        let initial_nbits = w.nbits;
        let initial_checkpoint = w.checkpoint.clone();
        let mut best: Option<(usize, Vec<u8>, u64, u8, C)> = None;
        for (tokens, cache_bits) in candidates.drain(..) {
            output_scratch.clear();
            let (byte_length, buffer, nbits, checkpoint) = {
                let mut trial = BitWriter {
                    writer: output_scratch,
                    buffer: initial_buffer,
                    nbits: initial_nbits,
                    checkpoint: initial_checkpoint.clone(),
                };
                write_token_stream(
                    &mut trial,
                    pixels,
                    width,
                    &tokens,
                    TokenStreamConfig {
                        write_meta_huffman_bit,
                        cache_bits,
                        histogram_bits,
                        quality,
                    },
                    token_scratch,
                    token,
                )?;
                let checkpoint = trial.checkpoint.clone();
                (
                    initial_byte_length
                        .saturating_add(trial.writer.len())
                        .saturating_add(usize::from(trial.nbits).div_ceil(8)),
                    trial.buffer,
                    trial.nbits,
                    checkpoint,
                )
            };
            if best
                .as_ref()
                .is_none_or(|(best_length, ..)| byte_length < *best_length)
            {
                let suffix = core::mem::take(output_scratch);
                if let Some((_, previous_suffix, ..)) =
                    best.replace((byte_length, suffix, buffer, nbits, checkpoint))
                {
                    *output_scratch = previous_suffix;
                }
            }
            token_scratch.candidates.result_pool.push(tokens);
        }
        // At most the standard and optional box-chain candidates are emitted
        // for one stream. Keep the pool bounded even when the cache scratch
        // already has sufficient capacity and did not consume a pooled vector.
        token_scratch.candidates.result_pool.truncate(2);
        let (_, mut suffix, buffer, nbits, checkpoint) = best.unwrap();
        w.writer.reserve(suffix.len());
        // The ordinary path has no caller-visible copy checkpoint. Move the
        // winning suffix into the parent writer instead of copying it; keep
        // the token-aware path's chunked copy and cancellation behavior.
        match token {
            None => w.writer.append(&mut suffix),
            Some(token) => {
                #[cfg(coverage)]
                coverage_record_token_remaining(
                    &COVERAGE_IMAGE_STREAM_SUFFIX_REMAINING,
                    Some(token),
                );
                extend_bytes_with_checkpoint(w.writer, &suffix, Some(token))?
            }
        }
        *output_scratch = suffix;
        w.buffer = buffer;
        w.nbits = nbits;
        w.checkpoint = checkpoint;
        Ok(())
    })();
    // `drain` leaves the result-list allocation available for the next image
    // stream. Restore it even when a token-aware trial is cancelled or fails.
    token_scratch.candidates.result_list = candidates;
    result
}

struct GroupCodes {
    lengths: [Vec<u8>; 5],
    codes: [Vec<u16>; 5],
}

impl Default for GroupCodes {
    fn default() -> Self {
        Self {
            lengths: core::array::from_fn(|_| Vec::new()),
            codes: core::array::from_fn(|_| Vec::new()),
        }
    }
}

impl GroupCodes {
    fn prepare(&mut self, populations: &[Vec<u32>; 5]) {
        for (channel, population) in populations.iter().enumerate() {
            let population_len = population.len();
            self.lengths[channel].resize(population_len, 0);
            self.lengths[channel].fill(0);
            self.codes[channel].resize(population_len, 0);
            self.codes[channel].fill(0);
        }
    }
}

#[derive(Default)]
struct TokenStreamScratch {
    groups: Vec<GroupCodes>,
    candidates: backward_refs::CandidateScratch,
    histogram: histogram::HistogramScratch,
    huffman_tokens: Vec<HuffmanToken>,
    optimized_frequencies: Vec<u32>,
    huffman_rle_good: Vec<bool>,
    meta_pixels: Vec<u32>,
    huffman_nodes: Vec<WeightedHuffmanEncodingNode>,
    huffman_node_sort_scratch: Vec<WeightedHuffmanEncodingNode>,
    huffman_node_arena: Vec<HuffmanEncodingNode>,
    meta_output: Vec<u8>,
    // Multi-group token streams encode a metadata image once per candidate.
    // Retain the nested stream scratch so its bounded buffers survive the
    // outer candidate loop; the metadata stream disables further recursion.
    meta_stream: Option<Box<TokenStreamScratch>>,
}

#[derive(Default)]
struct ImageStreamScratch {
    output: Vec<u8>,
    tokens: TokenStreamScratch,
    predictor: predictor::PredictorScratch,
    cross_color: cross_color::CrossColorScratch,
    // The packed ALPH transform image is consumed before the next frame can
    // use this encoder, so retain its capacity without retaining frame output.
    alpha_packed: Vec<u32>,
}

#[derive(Clone, Copy)]
struct TokenStreamConfig {
    write_meta_huffman_bit: bool,
    cache_bits: u8,
    histogram_bits: u8,
    quality: u32,
}

#[cfg_attr(coverage, inline(never))]
fn optimize_sampling(
    symbols: &mut Vec<u16>,
    full_width: usize,
    full_height: usize,
    input_bits: u8,
    maximum_bits: u8,
    token: Option<&crate::CancellationToken>,
) -> Result<u8, EncodingError> {
    let mut width = full_width.div_ceil(1 << input_bits);
    let mut height = full_height.div_ceil(1 << input_bits);
    let mut best_bits = input_bits;

    while best_bits < maximum_bits {
        check_token(token)?;
        let new_square_size = 1 << (best_bits + 1 - input_bits);
        let square_size = 1 << (best_bits - input_bits);
        let rows_match = if let Some(token) = token {
            let mut comparisons = 0_usize;
            let mut rows_match = true;
            'rows: for y in (0..height)
                .step_by(new_square_size)
                .take_while(|&y| y + square_size < height)
            {
                let left = &symbols[y * width..(y + 1) * width];
                let right = &symbols[(y + square_size) * width..(y + square_size + 1) * width];
                for (&left, &right) in left.iter().zip(right.iter()) {
                    let equal = left == right;
                    comparisons += 1;
                    if comparisons.is_multiple_of(VP8L_HISTOGRAM_SAMPLING_CHECKPOINT_SYMBOLS) {
                        check_token(Some(token))?;
                    }
                    if !equal {
                        rows_match = false;
                        break 'rows;
                    }
                }
            }
            rows_match
        } else {
            (0..height)
                .step_by(new_square_size)
                .take_while(|&y| y + square_size < height)
                .all(|y| {
                    symbols[y * width..(y + 1) * width]
                        == symbols[(y + square_size) * width..(y + square_size + 1) * width]
                })
        };
        if !rows_match {
            break;
        }
        best_bits += 1;
    }
    if best_bits == input_bits {
        return Ok(input_bits);
    }

    while best_bits > input_bits {
        check_token(token)?;
        let square_size = 1 << (best_bits - input_bits);
        let columns_match = if let Some(token) = token {
            let mut comparisons = 0_usize;
            let mut columns_match = true;
            'rows: for y in 0..height {
                for x in (0..width).step_by(square_size) {
                    let first = symbols[y * width + x];
                    for column in (x + 1)..(x + square_size).min(width) {
                        let equal = symbols[y * width + column] == first;
                        comparisons += 1;
                        if comparisons.is_multiple_of(VP8L_HISTOGRAM_SAMPLING_CHECKPOINT_SYMBOLS) {
                            check_token(Some(token))?;
                        }
                        if !equal {
                            columns_match = false;
                            break 'rows;
                        }
                    }
                }
            }
            columns_match
        } else {
            (0..height).all(|y| {
                (0..width).step_by(square_size).all(|x| {
                    let first = symbols[y * width + x];
                    (x + 1..(x + square_size).min(width))
                        .all(|column| symbols[y * width + column] == first)
                })
            })
        };
        if columns_match {
            break;
        }
        best_bits -= 1;
    }
    if best_bits == input_bits {
        return Ok(input_bits);
    }

    let old_width = width;
    let square_size = 1 << (best_bits - input_bits);
    width = full_width.div_ceil(1 << best_bits);
    height = full_height.div_ceil(1 << best_bits);
    if let Some(token) = token {
        let mut copied = 0_usize;
        for y in 0..height {
            if y.is_multiple_of(64) {
                check_token(Some(token))?;
            }
            for x in 0..width {
                symbols[y * width + x] = symbols[square_size * (y * old_width + x)];
                copied += 1;
                if copied.is_multiple_of(VP8L_HISTOGRAM_SAMPLING_CHECKPOINT_SYMBOLS) {
                    check_token(Some(token))?;
                }
            }
        }
    } else {
        for y in 0..height {
            for x in 0..width {
                symbols[y * width + x] = symbols[square_size * (y * old_width + x)];
            }
        }
    }
    symbols.truncate(width * height);
    Ok(best_bits)
}

fn write_group<C: BitWriterCheckpoint>(
    w: &mut BitWriter<'_, C>,
    populations: &[Vec<u32>; 5],
    group: &mut GroupCodes,
    scratch: &mut HuffmanTreeScratch<'_>,
    token: Option<&crate::CancellationToken>,
) -> Result<(), EncodingError> {
    group.prepare(populations);
    for (channel, population) in populations.iter().enumerate() {
        check_token(token)?;
        let channel_lengths = &mut group.lengths[channel];
        let channel_codes = &mut group.codes[channel];
        write_huffman_tree(
            w,
            population,
            channel_lengths,
            channel_codes,
            scratch,
            token,
        )?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct TokenStreamReferenceContext<'a> {
    width: usize,
    multiple_groups: bool,
    symbols: &'a [u16],
    encoded_histogram_bits: u8,
    tile_width: usize,
    groups: &'a [GroupCodes],
}

#[inline(always)]
fn write_token_reference<C: BitWriterCheckpoint>(
    w: &mut BitWriter<'_, C>,
    reference: backward_refs::Token,
    position: usize,
    context: TokenStreamReferenceContext<'_>,
) -> Result<usize, EncodingError> {
    let group_index = if context.multiple_groups {
        let x = position % context.width;
        let y = position / context.width;
        usize::from(
            context.symbols[(y >> context.encoded_histogram_bits) * context.tile_width
                + (x >> context.encoded_histogram_bits)],
        )
    } else {
        0
    };
    let lengths = &context.groups[group_index].lengths;
    let codes = &context.groups[group_index].codes;
    match reference {
        backward_refs::Token::Literal(pixel) => {
            let [red, green, blue, alpha] = channels(pixel);
            let green_length = lengths[0][green];
            let red_length = lengths[1][red];
            let blue_length = lengths[2][blue];
            let alpha_length = lengths[3][alpha];
            let code = u64::from(codes[0][green])
                | (u64::from(codes[1][red]) << green_length)
                | (u64::from(codes[2][blue]) << (green_length + red_length))
                | (u64::from(codes[3][alpha]) << (green_length + red_length + blue_length));
            w.write_bits(code, green_length + red_length + blue_length + alpha_length)?;
            Ok(1)
        }
        backward_refs::Token::Copy { distance, length } => {
            let (symbol, extra_bits) = length_to_symbol(length);
            let symbol = 256 + symbol;
            w.write_bits(u64::from(codes[0][symbol]), lengths[0][symbol])?;
            w.write_bits(((length - 1) & ((1 << extra_bits) - 1)) as u64, extra_bits)?;
            let distance = backward_refs::plane_code(context.width, distance);
            let (symbol, extra_bits) = length_to_symbol(distance);
            w.write_bits(u64::from(codes[4][symbol]), lengths[4][symbol])?;
            let distance_extra_bits = ((distance - 1) & ((1 << extra_bits) - 1)) as u64;
            w.write_bits(distance_extra_bits, extra_bits)?;
            Ok(length)
        }
        backward_refs::Token::Cache(index) => {
            let symbol = 280 + index;
            w.write_bits(u64::from(codes[0][symbol]), lengths[0][symbol])?;
            Ok(1)
        }
    }
}

#[cfg_attr(coverage, inline(never))]
fn write_token_stream<C: BitWriterCheckpoint>(
    w: &mut BitWriter<'_, C>,
    pixels: &[u32],
    width: usize,
    tokens: &[backward_refs::Token],
    config: TokenStreamConfig,
    scratch: &mut TokenStreamScratch,
    token: Option<&crate::CancellationToken>,
) -> Result<(), EncodingError> {
    check_token(token)?;
    let TokenStreamConfig {
        write_meta_huffman_bit,
        cache_bits,
        histogram_bits,
        quality,
    } = config;
    w.write_bits(u64::from(cache_bits != 0), 1)?;
    if cache_bits != 0 {
        w.write_bits(u64::from(cache_bits), 4)?;
    }
    let height = pixels.len() / width;
    let (symbols, histograms) = if write_meta_huffman_bit {
        histogram::cluster(
            tokens,
            (width, height),
            cache_bits,
            quality,
            histogram_bits,
            &mut scratch.histogram,
            token,
        )?
    } else {
        histogram::cluster(
            tokens,
            (width, height),
            cache_bits,
            quality,
            31,
            &mut scratch.histogram,
            token,
        )?
    };
    check_token(token)?;
    let multiple_groups = write_meta_huffman_bit && histograms.len() > 1;
    let mut encoded_histogram_bits = histogram_bits;
    if write_meta_huffman_bit {
        w.write_bits(u64::from(multiple_groups), 1)?;
        if multiple_groups {
            #[cfg(coverage)]
            coverage_cancel_token_at_optimize(token);
            encoded_histogram_bits =
                optimize_sampling(symbols, width, height, histogram_bits, 9, token)?;
            #[cfg(coverage)]
            coverage_record_remaining(
                &COVERAGE_TOKEN_STREAM_HISTOGRAM_REMAINING,
                w.checkpoint.coverage_remaining(),
            );
            w.write_bits(u64::from(encoded_histogram_bits - 2), 3)?;
            // Meta-pixel materialization scales with the retained histogram
            // tile map after sampling. Keep the ordinary no-token map
            // unchanged and poll only the caller-controlled path.
            {
                let meta_pixels = &mut scratch.meta_pixels;
                meta_pixels.clear();
                meta_pixels.reserve(symbols.len());
                if let Some(token) = token {
                    let mut symbols_until_checkpoint = VP8L_HISTOGRAM_SAMPLING_CHECKPOINT_SYMBOLS;
                    for &symbol in symbols.iter() {
                        meta_pixels.push(u32::from(symbol) << 8);
                        symbols_until_checkpoint = symbols_until_checkpoint.saturating_sub(1);
                        if symbols_until_checkpoint == 0 {
                            #[cfg(coverage)]
                            coverage_record_token_remaining(
                                &COVERAGE_TOKEN_STREAM_META_PIXEL_REMAINING,
                                Some(token),
                            );
                            check_token(Some(token))?;
                            symbols_until_checkpoint = VP8L_HISTOGRAM_SAMPLING_CHECKPOINT_SYMBOLS;
                        }
                    }
                } else {
                    meta_pixels.extend(symbols.iter().map(|&symbol| u32::from(symbol) << 8));
                }
            }
            let meta_width = width.div_ceil(1 << encoded_histogram_bits);
            let meta_scratch = scratch
                .meta_stream
                .get_or_insert_with(|| Box::new(TokenStreamScratch::default()));
            #[cfg(coverage)]
            coverage_record_nested_metadata(token);
            write_image_stream_configured_with_scratch(
                w,
                &scratch.meta_pixels,
                meta_width,
                false,
                3,
                quality,
                0,
                &mut scratch.meta_output,
                meta_scratch.as_mut(),
                token,
            )?;
        }
    }
    let group_count = histograms.len();
    let group_scratch = &mut scratch.groups;
    let mut huffman_scratch = HuffmanTreeScratch {
        huffman_tokens: &mut scratch.huffman_tokens,
        optimized_frequencies: &mut scratch.optimized_frequencies,
        huffman_rle_good: &mut scratch.huffman_rle_good,
        nodes: &mut scratch.huffman_nodes,
        node_sort_scratch: &mut scratch.huffman_node_sort_scratch,
        node_arena: &mut scratch.huffman_node_arena,
    };
    if group_scratch.len() < group_count {
        group_scratch.resize_with(group_count, GroupCodes::default);
    }
    for (group, histogram) in group_scratch
        .iter_mut()
        .take(group_count)
        .zip(histograms.iter())
    {
        check_token(token)?;
        write_group(
            w,
            &histogram.populations,
            group,
            &mut huffman_scratch,
            token,
        )?;
    }

    let tile_width = width.div_ceil(1 << encoded_histogram_bits);
    let reference_context = TokenStreamReferenceContext {
        width,
        multiple_groups,
        symbols,
        encoded_histogram_bits,
        tile_width,
        groups: &group_scratch[..group_count],
    };
    if let Some(token) = token {
        let mut position = 0;
        let mut next_checkpoint = VP8L_TOKEN_STREAM_CHECKPOINT_PIXELS;
        for &reference in tokens {
            let consumed = write_token_reference(w, reference, position, reference_context)?;
            position += consumed;
            while position >= next_checkpoint {
                check_token(Some(token))?;
                next_checkpoint =
                    next_checkpoint.saturating_add(VP8L_TOKEN_STREAM_CHECKPOINT_PIXELS);
            }
        }
    } else {
        let mut position = 0;
        for &reference in tokens {
            position += write_token_reference(w, reference, position, reference_context)?;
        }
    }
    Ok(())
}

fn subtract_pixels(color: u32, previous: u32) -> u32 {
    let alpha = (color >> 24).wrapping_sub(previous >> 24) & 0xff;
    let red = ((color >> 16) & 0xff).wrapping_sub((previous >> 16) & 0xff) & 0xff;
    let green = ((color >> 8) & 0xff).wrapping_sub((previous >> 8) & 0xff) & 0xff;
    let blue = (color & 0xff).wrapping_sub(previous & 0xff) & 0xff;
    alpha << 24 | red << 16 | green << 8 | blue
}

fn palette_color_distance(color: u32, previous: u32) -> u32 {
    let difference = subtract_pixels(color, previous);
    let component_distance = |value: u32| value.min(256 - value);
    let rgb = component_distance(difference & 0xff)
        + component_distance((difference >> 8) & 0xff)
        + component_distance((difference >> 16) & 0xff);
    9 * rgb + component_distance(difference >> 24)
}

// Each searched suffix starts at an index strictly below `sortable_length`.
#[allow(clippy::unwrap_used)]
fn minimize_palette_deltas(palette: &mut [u32]) {
    let mut signs = 0_u8;
    let mut previous = 0_u32;
    for &color in palette.iter() {
        let difference = subtract_pixels(color, previous);
        for (shift, positive, negative) in [(16, 1, 2), (8, 8, 16), (0, 64, 128)] {
            let component = ((difference >> shift) & 0xff) as u8;
            if component != 0 {
                signs |= if component < 0x80 { positive } else { negative };
            }
        }
        previous = color;
    }
    if signs & (signs << 1) == 0 {
        return;
    }
    let mut sortable_length = palette.len();
    if sortable_length > 17 && palette[0] == 0 {
        sortable_length -= 1;
        palette.swap(0, sortable_length);
    }
    previous = 0;
    for index in 0..sortable_length {
        let (offset, _) = palette[index..sortable_length]
            .iter()
            .enumerate()
            .map(|(offset, &color)| (offset, palette_color_distance(color, previous)))
            .min_by_key(|&(_, distance)| distance)
            .unwrap();
        palette.swap(index, index + offset);
        previous = palette[index];
    }
}

// The palette has at most 256 entries, but each nearest-delta selection can
// still scan a large suffix. This token-aware path preserves the no-token
// ordering and tie behavior while bounding both palette passes.
#[cfg_attr(coverage, inline(never))]
fn minimize_palette_deltas_with_checkpoint(
    palette: &mut [u32],
    token: &crate::CancellationToken,
) -> Result<(), EncodingError> {
    let mut signs = 0_u8;
    let mut previous = 0_u32;
    for (index, &color) in palette.iter().enumerate() {
        if index.is_multiple_of(WEBP_PALETTE_CHECKPOINT_VALUES) {
            check_token(Some(token))?;
        }
        let difference = subtract_pixels(color, previous);
        for (shift, positive, negative) in [(16, 1, 2), (8, 8, 16), (0, 64, 128)] {
            let component = ((difference >> shift) & 0xff) as u8;
            if component != 0 {
                signs |= if component < 0x80 { positive } else { negative };
            }
        }
        previous = color;
    }
    if signs & (signs << 1) == 0 {
        return Ok(());
    }
    let mut sortable_length = palette.len();
    if sortable_length > 17 && palette[0] == 0 {
        sortable_length -= 1;
        palette.swap(0, sortable_length);
    }
    previous = 0;
    for index in 0..sortable_length {
        if index.is_multiple_of(WEBP_PALETTE_CHECKPOINT_VALUES) {
            check_token(Some(token))?;
        }
        let mut best_offset = 0;
        let mut best_distance = u32::MAX;
        let mut candidates_until_checkpoint = WEBP_PALETTE_CHECKPOINT_VALUES;
        for (offset, &color) in palette[index..sortable_length].iter().enumerate() {
            let distance = palette_color_distance(color, previous);
            if distance < best_distance {
                best_offset = offset;
                best_distance = distance;
            }
            candidates_until_checkpoint = candidates_until_checkpoint.saturating_sub(1);
            if candidates_until_checkpoint == 0 {
                check_token(Some(token))?;
                candidates_until_checkpoint = WEBP_PALETTE_CHECKPOINT_VALUES;
            }
        }
        palette.swap(index, index + best_offset);
        previous = palette[index];
    }
    Ok(())
}

// Palette construction proves that every encoded pixel belongs to this
// table. The token-aware lookup keeps that invariant and the no-token
// position/tie behavior while bounding the potentially repeated linear scan.
#[inline]
fn palette_index_with_checkpoint(
    palette: &[u32],
    color: u32,
    token: &crate::CancellationToken,
) -> Result<usize, EncodingError> {
    let mut candidates_until_checkpoint = WEBP_PALETTE_CHECKPOINT_VALUES;
    for (index, &entry) in palette.iter().enumerate() {
        let matches = entry == color;
        candidates_until_checkpoint = candidates_until_checkpoint.saturating_sub(1);
        if candidates_until_checkpoint == 0 {
            check_token(Some(token))?;
            candidates_until_checkpoint = WEBP_PALETTE_CHECKPOINT_VALUES;
        }
        if matches {
            return Ok(index);
        }
    }
    unreachable!("palette construction must retain every encoded color")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EntropyMode {
    Direct,
    Spatial,
    SubtractGreen,
    SpatialSubtractGreen,
    Palette,
}

fn pixels_are_grayscale(pixels: &[u32]) -> bool {
    pixels.iter().all(|&pixel| {
        let red = (pixel >> 16) & 0xff;
        let green = (pixel >> 8) & 0xff;
        let blue = pixel & 0xff;
        red == green && green == blue
    })
}

fn pixels_are_grayscale_with_checkpoint(
    pixels: &[u32],
    token: Option<&crate::CancellationToken>,
) -> Result<bool, EncodingError> {
    if token.is_none() {
        return Ok(pixels_are_grayscale(pixels));
    }
    for (index, &pixel) in pixels.iter().enumerate() {
        let red = (pixel >> 16) & 0xff;
        let green = (pixel >> 8) & 0xff;
        let blue = pixel & 0xff;
        if red != green || green != blue {
            return Ok(false);
        }
        if (index + 1).is_multiple_of(VP8L_GRAYSCALE_CHECKPOINT_PIXELS) {
            check_token(token)?;
        }
    }
    Ok(true)
}

// Keep pixel accumulation separate from token scheduling so the wide-row
// token path can add a post-chunk poll while the no-token traversal remains
// a direct loop over the existing heap-backed histogram table.
#[inline(always)]
fn accumulate_entropy_pixel(
    histograms: &mut [[u32; 256]],
    pixels: &[u32],
    width: usize,
    y: usize,
    x: usize,
    previous_pixel: &mut u32,
) {
    const ALPHA: usize = 0;
    const ALPHA_PREDICTED: usize = 1;
    const GREEN: usize = 2;
    const GREEN_PREDICTED: usize = 3;
    const RED: usize = 4;
    const RED_PREDICTED: usize = 5;
    const BLUE: usize = 6;
    const BLUE_PREDICTED: usize = 7;
    const RED_SUB_GREEN: usize = 8;
    const RED_PREDICTED_SUB_GREEN: usize = 9;
    const BLUE_SUB_GREEN: usize = 10;
    const BLUE_PREDICTED_SUB_GREEN: usize = 11;
    const PALETTE: usize = 12;

    let pixel = pixels[y * width + x];
    let difference = subtract_pixels(pixel, *previous_pixel);
    *previous_pixel = pixel;
    if difference == 0 || (y != 0 && pixel == pixels[(y - 1) * width + x]) {
        return;
    }

    histograms[ALPHA][((pixel >> 24) & 0xff) as usize] += 1;
    histograms[RED][((pixel >> 16) & 0xff) as usize] += 1;
    histograms[GREEN][((pixel >> 8) & 0xff) as usize] += 1;
    histograms[BLUE][(pixel & 0xff) as usize] += 1;
    histograms[ALPHA_PREDICTED][((difference >> 24) & 0xff) as usize] += 1;
    histograms[RED_PREDICTED][((difference >> 16) & 0xff) as usize] += 1;
    histograms[GREEN_PREDICTED][((difference >> 8) & 0xff) as usize] += 1;
    histograms[BLUE_PREDICTED][(difference & 0xff) as usize] += 1;

    let green = ((pixel >> 8) & 0xff) as u8;
    let red = ((pixel >> 16) & 0xff) as u8;
    let blue = (pixel & 0xff) as u8;
    histograms[RED_SUB_GREEN][usize::from(red.wrapping_sub(green))] += 1;
    histograms[BLUE_SUB_GREEN][usize::from(blue.wrapping_sub(green))] += 1;

    let predicted_green = ((difference >> 8) & 0xff) as u8;
    let predicted_red = ((difference >> 16) & 0xff) as u8;
    let predicted_blue = (difference & 0xff) as u8;
    histograms[RED_PREDICTED_SUB_GREEN]
        [usize::from(predicted_red.wrapping_sub(predicted_green))] += 1;
    histograms[BLUE_PREDICTED_SUB_GREEN]
        [usize::from(predicted_blue.wrapping_sub(predicted_green))] += 1;

    let hash = ((((u64::from(pixel) + u64::from(pixel >> 19)) * 0x39c5_fba7) & 0xffff_ffff) >> 24)
        as usize;
    histograms[PALETTE][hash] += 1;
}

// The direct entropy mode makes the mode table non-empty.
#[allow(clippy::unwrap_used)]
fn analyze_entropy(
    pixels: &[u32],
    width: usize,
    height: usize,
    palette_size: Option<usize>,
    transform_bits: u8,
    token: Option<&crate::CancellationToken>,
) -> Result<(EntropyMode, bool), EncodingError> {
    if palette_size.is_some_and(|size| size <= 16) {
        return Ok((EntropyMode::Palette, true));
    }
    const ALPHA: usize = 0;
    const ALPHA_PREDICTED: usize = 1;
    const GREEN: usize = 2;
    const GREEN_PREDICTED: usize = 3;
    const RED: usize = 4;
    const RED_PREDICTED: usize = 5;
    const BLUE: usize = 6;
    const BLUE_PREDICTED: usize = 7;
    const RED_SUB_GREEN: usize = 8;
    const RED_PREDICTED_SUB_GREEN: usize = 9;
    const BLUE_SUB_GREEN: usize = 10;
    const BLUE_PREDICTED_SUB_GREEN: usize = 11;
    const PALETTE: usize = 12;
    // Entropy analysis always uses the fixed 13-channel, 8-bit alphabet. Keep
    // this function-local table on the stack instead of allocating a heap
    // vector whose shape cannot vary and whose contents never escape.
    let mut histograms = [[0_u32; 256]; 13];
    let mut previous_pixel = pixels[0];
    if let Some(token) = token {
        if width > VP8L_ENTROPY_ANALYSIS_CHECKPOINT_PIXELS {
            let mut scanned_pixels = 0_usize;
            for y in 0..height {
                if y.is_multiple_of(16) {
                    check_token(Some(token))?;
                }
                for x in 0..width {
                    if x.is_multiple_of(1024) {
                        check_token(Some(token))?;
                    }
                    accumulate_entropy_pixel(
                        &mut histograms,
                        pixels,
                        width,
                        y,
                        x,
                        &mut previous_pixel,
                    );
                    scanned_pixels += 1;
                    if scanned_pixels.is_multiple_of(VP8L_ENTROPY_ANALYSIS_CHECKPOINT_PIXELS) {
                        check_token(Some(token))?;
                    }
                }
            }
        } else {
            for y in 0..height {
                if y.is_multiple_of(16) {
                    check_token(Some(token))?;
                }
                for x in 0..width {
                    if x.is_multiple_of(1024) {
                        check_token(Some(token))?;
                    }
                    accumulate_entropy_pixel(
                        &mut histograms,
                        pixels,
                        width,
                        y,
                        x,
                        &mut previous_pixel,
                    );
                }
            }
        }
    } else {
        for y in 0..height {
            for x in 0..width {
                accumulate_entropy_pixel(&mut histograms, pixels, width, y, x, &mut previous_pixel);
            }
        }
    }
    for category in [
        RED_PREDICTED_SUB_GREEN,
        BLUE_PREDICTED_SUB_GREEN,
        RED_PREDICTED,
        GREEN_PREDICTED,
        BLUE_PREDICTED,
        ALPHA_PREDICTED,
    ] {
        histograms[category][0] += 1;
    }
    // A caller work budget must also cover this fixed-alphabet entropy pass.
    // Pillow has no equivalent caller token or work-budget result, so this is
    // Rust-only work-control evidence rather than a parity surface.
    let mut costs = [0_u64; 13];
    for (histogram, cost) in histograms.iter().zip(costs.iter_mut()) {
        *cost = histogram::bits_entropy_with_checkpoint(histogram, token)?;
    }
    check_token(token)?;
    let transform_width = width.div_ceil(1 << transform_bits);
    let transform_height = height.div_ceil(1 << transform_bits);
    let fast_log = |value: u32| (f64::from(value).log2() * f64::from(1_u32 << 23)).round() as u64;
    let mut modes = [
        (
            EntropyMode::Direct,
            costs[ALPHA] + costs[RED] + costs[GREEN] + costs[BLUE],
        ),
        (
            EntropyMode::Spatial,
            costs[ALPHA_PREDICTED]
                + costs[RED_PREDICTED]
                + costs[GREEN_PREDICTED]
                + costs[BLUE_PREDICTED]
                + (transform_width * transform_height) as u64 * fast_log(14),
        ),
        (
            EntropyMode::SubtractGreen,
            costs[ALPHA] + costs[RED_SUB_GREEN] + costs[GREEN] + costs[BLUE_SUB_GREEN],
        ),
        (
            EntropyMode::SpatialSubtractGreen,
            costs[ALPHA_PREDICTED]
                + costs[RED_PREDICTED_SUB_GREEN]
                + costs[GREEN_PREDICTED]
                + costs[BLUE_PREDICTED_SUB_GREEN]
                + (transform_width * transform_height) as u64 * fast_log(24),
        ),
        (EntropyMode::Palette, 0),
    ];
    let mode_count = if let Some(size) = palette_size {
        modes[4] = (
            EntropyMode::Palette,
            costs[PALETTE] + ((size as u64 * 8) << 23),
        );
        5
    } else {
        4
    };
    let mode = modes
        .iter()
        .take(mode_count)
        .min_by_key(|&(_, cost)| cost)
        .map(|&(mode, _)| mode)
        .unwrap();
    let (red_histogram, blue_histogram) = match mode {
        EntropyMode::Direct | EntropyMode::Palette => (RED, BLUE),
        EntropyMode::Spatial => (RED_PREDICTED, BLUE_PREDICTED),
        EntropyMode::SubtractGreen => (RED_SUB_GREEN, BLUE_SUB_GREEN),
        EntropyMode::SpatialSubtractGreen => (RED_PREDICTED_SUB_GREEN, BLUE_PREDICTED_SUB_GREEN),
    };
    let red_and_blue_zero = (1..256).all(|index| {
        histograms[red_histogram][index] == 0 && histograms[blue_histogram][index] == 0
    });
    Ok((mode, red_and_blue_zero))
}

fn subtract_green(pixels: &mut [u32]) {
    for pixel in pixels {
        let green = (*pixel >> 8) & 0xff;
        let red = ((*pixel >> 16) & 0xff).wrapping_sub(green) & 0xff;
        let blue = (*pixel & 0xff).wrapping_sub(green) & 0xff;
        *pixel = (*pixel & 0xff00_ff00) | (red << 16) | blue;
    }
}

fn subtract_green_with_checkpoint(
    pixels: &mut [u32],
    token: Option<&crate::CancellationToken>,
) -> Result<(), EncodingError> {
    let mut pixels_until_checkpoint = VP8L_TRANSFORM_CHECKPOINT_PIXELS;
    for pixel in pixels {
        let green = (*pixel >> 8) & 0xff;
        let red = ((*pixel >> 16) & 0xff).wrapping_sub(green) & 0xff;
        let blue = (*pixel & 0xff).wrapping_sub(green) & 0xff;
        *pixel = (*pixel & 0xff00_ff00) | (red << 16) | blue;
        pixels_until_checkpoint = pixels_until_checkpoint.saturating_sub(1);
        if pixels_until_checkpoint == 0 {
            check_token(token)?;
            pixels_until_checkpoint = VP8L_TRANSFORM_CHECKPOINT_PIXELS;
        }
    }
    Ok(())
}

// The palette is constructed from the same pixel set being packed.
#[allow(clippy::unwrap_used)]
#[cfg_attr(coverage, inline(never))]
fn apply_palette<C: BitWriterCheckpoint>(
    w: &mut BitWriter<'_, C>,
    pixels: &mut [u32],
    width: usize,
    height: usize,
    mut palette: Vec<u32>,
    scratch: &mut ImageStreamScratch,
    token: Option<&crate::CancellationToken>,
) -> Result<(), EncodingError> {
    check_token(token)?;
    if let Some(token) = token {
        minimize_palette_deltas_with_checkpoint(&mut palette, token)?;
    } else {
        minimize_palette_deltas(&mut palette);
    }
    let encoded_length = if palette.len() > 17 && palette.last() == Some(&0) {
        palette.len() - 1
    } else {
        palette.len()
    };
    w.write_bits(1, 1)?;
    w.write_bits(3, 2)?;
    w.write_bits((encoded_length - 1) as u64, 8)?;
    let mut previous = 0;
    // Palette mode is selected only for palettes with at most 256 entries.
    // Keep the source palette intact for the later index-packing pass while
    // avoiding a heap allocation for this bounded transformed view.
    let mut palette_delta = [0_u32; 256];
    for (index, &color) in palette[..encoded_length].iter().enumerate() {
        let difference = subtract_pixels(color, previous);
        previous = color;
        palette_delta[index] = difference;
    }
    write_image_stream_configured_with_scratch(
        w,
        &palette_delta[..encoded_length],
        encoded_length,
        false,
        3,
        20,
        0,
        &mut scratch.output,
        &mut scratch.tokens,
        token,
    )?;

    let packing_bits = match palette.len() {
        0..=2 => 3,
        3..=4 => 2,
        5..=16 => 1,
        _ => 0,
    };
    let pixels_per_group = 1 << packing_bits;
    let bits_per_pixel = 8 >> packing_bits;
    let packed_width = width.div_ceil(pixels_per_group);
    let packed_len = packed_width * height;
    // The source buffer is no longer needed after palette packing. Packing
    // left-to-right into its prefix is safe because each destination index is
    // at or before the source group being read; for one-pixel groups the
    // source value is read before the same slot is overwritten.
    if let Some(token) = token {
        let mut source_pixels_until_checkpoint = VP8L_PALETTE_PACKING_CHECKPOINT_PIXELS;
        for row_index in 0..height {
            if row_index.is_multiple_of(64) {
                check_token(Some(token))?;
            }
            let row_start = row_index * width;
            let packed_row_start = row_index * packed_width;
            for (group_index, group_start) in (0..width).step_by(pixels_per_group).enumerate() {
                let mut packed_pixel = 0xff00_0000_u32;
                let group_len = (width - group_start).min(pixels_per_group);
                for index in 0..group_len {
                    let color = pixels[row_start + group_start + index];
                    let palette_index = palette_index_with_checkpoint(&palette, color, token)?;
                    packed_pixel |= (palette_index as u32) << (8 + bits_per_pixel * index);
                }
                pixels[packed_row_start + group_index] = packed_pixel;
                source_pixels_until_checkpoint =
                    source_pixels_until_checkpoint.saturating_sub(group_len);
                if source_pixels_until_checkpoint == 0 {
                    check_token(Some(token))?;
                    source_pixels_until_checkpoint = VP8L_PALETTE_PACKING_CHECKPOINT_PIXELS;
                }
            }
        }
    } else {
        for row_index in 0..height {
            let row_start = row_index * width;
            let packed_row_start = row_index * packed_width;
            for (group_index, group_start) in (0..width).step_by(pixels_per_group).enumerate() {
                let mut packed_pixel = 0xff00_0000_u32;
                let group_len = (width - group_start).min(pixels_per_group);
                for index in 0..group_len {
                    let color = pixels[row_start + group_start + index];
                    let palette_index = palette.iter().position(|&entry| entry == color).unwrap();
                    packed_pixel |= (palette_index as u32) << (8 + bits_per_pixel * index);
                }
                pixels[packed_row_start + group_index] = packed_pixel;
            }
        }
    }
    w.write_bits(0, 1)?;
    let maximum_cache_bits = (usize::BITS - palette.len().leading_zeros()) as u8;
    write_image_stream_configured_with_scratch(
        w,
        &pixels[..packed_len],
        packed_width,
        true,
        5,
        80,
        maximum_cache_bits,
        &mut scratch.output,
        &mut scratch.tokens,
        token,
    )
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(coverage, inline(never))]
fn encode_frame_stream<C: BitWriterCheckpoint>(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    is_alpha: bool,
    entropy_mode: EntropyMode,
    red_and_blue_zero: bool,
    transform_bits: u8,
    palette: Vec<u32>,
    token: Option<&crate::CancellationToken>,
    checkpoint: C,
    scratch: &mut ImageStreamScratch,
) -> Result<Vec<u8>, EncodingError> {
    // Nested image-stream trials leave one bounded suffix buffer in the
    // stream scratch. Recycle that allocation for the final VP8L frame so
    // sequential frames do not allocate a second transient output vector;
    // nested trials refill the scratch buffer after it is taken.
    let mut frame = core::mem::take(&mut scratch.output);
    frame.clear();
    {
        let w = &mut BitWriter {
            writer: &mut frame,
            buffer: 0,
            nbits: 0,
            checkpoint,
        };
        w.write_bits(0x2f, 8)?; // signature
        w.write_bits(u64::from(width) - 1, 14)?;
        w.write_bits(u64::from(height) - 1, 14)?;

        w.write_bits(u64::from(is_alpha), 1)?; // alpha used
        w.write_bits(0x0, 3)?; // version

        if entropy_mode == EntropyMode::Palette {
            apply_palette(
                w,
                pixels,
                width as usize,
                height as usize,
                palette,
                scratch,
                token,
            )?;
        } else {
            let grayscale = pixels_are_grayscale_with_checkpoint(pixels, token)?;
            let use_subtract_green = matches!(
                entropy_mode,
                EntropyMode::SubtractGreen | EntropyMode::SpatialSubtractGreen
            );
            let use_predictor = matches!(
                entropy_mode,
                EntropyMode::Spatial | EntropyMode::SpatialSubtractGreen
            );
            if use_subtract_green {
                #[cfg(coverage)]
                coverage_record_remaining(
                    &COVERAGE_FRAME_SUBTRACT_FIRST_REMAINING,
                    w.checkpoint.coverage_remaining(),
                );
                w.write_bits(1, 1)?;
                #[cfg(coverage)]
                coverage_record_remaining(
                    &COVERAGE_FRAME_SUBTRACT_SECOND_REMAINING,
                    w.checkpoint.coverage_remaining(),
                );
                w.write_bits(2, 2)?;
                if token.is_some() {
                    subtract_green_with_checkpoint(pixels, token)?;
                } else {
                    subtract_green(pixels);
                }
                check_token(token)?;
            }

            if use_predictor {
                let predictor_bits = if grayscale {
                    predictor::apply_fixed(
                        pixels,
                        width as usize,
                        height as usize,
                        transform_bits,
                        12,
                        &mut scratch.predictor,
                        token,
                    )?
                } else {
                    #[cfg(coverage)]
                    coverage_record_frame_boundary(&COVERAGE_FRAME_PREDICTOR_REMAINING, token);
                    predictor::select_and_apply(
                        pixels,
                        width as usize,
                        height as usize,
                        transform_bits,
                        &mut scratch.predictor,
                        token,
                    )?
                };
                w.write_bits(1, 1)?;
                w.write_bits(0, 2)?;
                w.write_bits(u64::from(predictor_bits - 2), 3)?;
                let predictor_width =
                    (width as usize + (1 << predictor_bits) - 1) >> predictor_bits;
                write_image_stream(
                    w,
                    scratch.predictor.modes(),
                    predictor_width,
                    false,
                    &mut scratch.output,
                    &mut scratch.tokens,
                    token,
                )?;
            }

            if use_predictor && !red_and_blue_zero {
                #[cfg(coverage)]
                coverage_record_frame_boundary(&COVERAGE_FRAME_CROSS_REMAINING, token);
                let color_bits = cross_color::select_and_apply(
                    pixels,
                    width as usize,
                    height as usize,
                    transform_bits,
                    80,
                    &mut scratch.cross_color,
                    token,
                )?;
                #[cfg(coverage)]
                coverage_record_remaining(
                    &COVERAGE_FRAME_CROSS_FIRST_REMAINING,
                    w.checkpoint.coverage_remaining(),
                );
                w.write_bits(1, 1)?;
                #[cfg(coverage)]
                coverage_record_remaining(
                    &COVERAGE_FRAME_CROSS_SECOND_REMAINING,
                    w.checkpoint.coverage_remaining(),
                );
                w.write_bits(1, 2)?;
                #[cfg(coverage)]
                coverage_record_remaining(
                    &COVERAGE_FRAME_CROSS_THIRD_REMAINING,
                    w.checkpoint.coverage_remaining(),
                );
                w.write_bits(u64::from(color_bits - 2), 3)?;
                let color_width = (width as usize + (1 << color_bits) - 1) >> color_bits;
                #[cfg(coverage)]
                coverage_record_frame_boundary(&COVERAGE_FRAME_COLOR_REMAINING, token);
                write_image_stream(
                    w,
                    scratch.cross_color.image(),
                    color_width,
                    false,
                    &mut scratch.output,
                    &mut scratch.tokens,
                    token,
                )?;
            }

            w.write_bits(0, 1)?; // transforms done
            write_image_stream(
                w,
                pixels,
                width as usize,
                true,
                &mut scratch.output,
                &mut scratch.tokens,
                token,
            )?;
        }

        w.flush()?;
    }
    check_token(token)?;
    Ok(frame)
}

#[cfg_attr(coverage, inline(never))]
fn collect_palette(
    pixels: &[u32],
    token: Option<&crate::CancellationToken>,
) -> Result<Vec<u32>, EncodingError> {
    if let Some(token) = token {
        let mut palette = std::collections::BTreeSet::new();
        let mut pixels_until_checkpoint = VP8L_PALETTE_CHECKPOINT_PIXELS;
        for &pixel in pixels {
            palette.insert(pixel);
            pixels_until_checkpoint = pixels_until_checkpoint.saturating_sub(1);
            if pixels_until_checkpoint == 0 {
                check_token(Some(token))?;
                pixels_until_checkpoint = VP8L_PALETTE_CHECKPOINT_PIXELS;
            }
        }
        // The ordered drain is also O(unique-color-count), which can remain
        // image-scaled after the source scan has finished. Keep the complete
        // drain for the caller-controlled path so its work-budget cadence
        // remains stable; the ordinary path can stop at the palette cutoff.
        let mut values = Vec::with_capacity(palette.len());
        let mut values_until_checkpoint = VP8L_PALETTE_CHECKPOINT_PIXELS;
        for value in palette {
            values.push(value);
            values_until_checkpoint = values_until_checkpoint.saturating_sub(1);
            if values_until_checkpoint == 0 {
                check_token(Some(token))?;
                values_until_checkpoint = VP8L_PALETTE_CHECKPOINT_PIXELS;
            }
        }
        Ok(values)
    } else {
        let mut palette = std::collections::BTreeSet::new();
        for &pixel in pixels {
            palette.insert(pixel);
            if palette.len() > VP8L_MAX_PALETTE_ENTRIES {
                // More than 256 colors can never select palette mode. Keep a
                // sorted 257-entry sentinel so the caller can distinguish
                // that case without retaining or ordering the rest of the
                // image's unique colors; this vector is ignored by every
                // non-palette stream. The token-aware path deliberately keeps
                // its complete ordered drain for the established work-budget
                // contract below.
                return Ok(palette.into_iter().collect());
            }
        }
        Ok(palette.into_iter().collect())
    }
}

// Materializing the native ARGB pixels is itself an O(pixel-count) stage. Keep
// the existing no-token maps byte-for-byte and add bounded polling only to the
// caller-controlled path; Pillow has no equivalent caller work budget.
fn convert_pixels(
    data: &[u8],
    color: ColorType,
    token: Option<&crate::CancellationToken>,
) -> Result<Vec<u32>, EncodingError> {
    let bytes_per_pixel = match color {
        ColorType::Rgb8 => 3,
        ColorType::Rgba8 => 4,
    };
    if let Some(token) = token {
        let mut pixels = Vec::with_capacity(data.len() / bytes_per_pixel);
        let mut pixels_until_checkpoint = VP8L_PIXEL_CONVERSION_CHECKPOINT_PIXELS;
        match color {
            ColorType::Rgb8 => {
                for pixel in data.as_chunks::<3>().0 {
                    pixels.push(
                        0xff00_0000
                            | (u32::from(pixel[0]) << 16)
                            | (u32::from(pixel[1]) << 8)
                            | u32::from(pixel[2]),
                    );
                    pixels_until_checkpoint = pixels_until_checkpoint.saturating_sub(1);
                    if pixels_until_checkpoint == 0 {
                        check_token(Some(token))?;
                        pixels_until_checkpoint = VP8L_PIXEL_CONVERSION_CHECKPOINT_PIXELS;
                    }
                }
            }
            ColorType::Rgba8 => {
                for pixel in data.as_chunks::<4>().0 {
                    pixels.push(
                        (u32::from(pixel[3]) << 24)
                            | (u32::from(pixel[0]) << 16)
                            | (u32::from(pixel[1]) << 8)
                            | u32::from(pixel[2]),
                    );
                    pixels_until_checkpoint = pixels_until_checkpoint.saturating_sub(1);
                    if pixels_until_checkpoint == 0 {
                        check_token(Some(token))?;
                        pixels_until_checkpoint = VP8L_PIXEL_CONVERSION_CHECKPOINT_PIXELS;
                    }
                }
            }
        }
        Ok(pixels)
    } else {
        Ok(match color {
            ColorType::Rgb8 => data
                .as_chunks::<3>()
                .0
                .iter()
                .map(|pixel| {
                    0xff00_0000
                        | (u32::from(pixel[0]) << 16)
                        | (u32::from(pixel[1]) << 8)
                        | u32::from(pixel[2])
                })
                .collect(),
            ColorType::Rgba8 => data
                .as_chunks::<4>()
                .0
                .iter()
                .map(|pixel| {
                    (u32::from(pixel[3]) << 24)
                        | (u32::from(pixel[0]) << 16)
                        | (u32::from(pixel[1]) << 8)
                        | u32::from(pixel[2])
                })
                .collect(),
        })
    }
}

/// Encode image data with the indicated color type.
///
/// # Panics
///
/// Panics if the image data is not of the indicated dimensions.
#[cfg_attr(coverage, inline(never))]
fn encode_frame(
    data: &[u8],
    width: u32,
    height: u32,
    color: ColorType,
    token: Option<&crate::CancellationToken>,
    scratch: &mut ImageStreamScratch,
) -> Result<Vec<u8>, EncodingError> {
    check_token(token)?;
    let (is_alpha, bytes_per_pixel) = match color {
        ColorType::Rgb8 => (false, 3),
        ColorType::Rgba8 => (true, 4),
    };

    assert_eq!(
        (u64::from(width) * u64::from(height)).saturating_mul(bytes_per_pixel),
        data.len() as u64
    );

    if width == 0 || width > 16384 || height == 0 || height > 16384 {
        return Err(EncodingError::InvalidDimensions);
    }

    let mut pixels = convert_pixels(data, color, token)?;
    check_token(token)?;

    // Pillow's lossless WebP path uses libwebp's default `exact=false`.
    // libwebp therefore replaces hidden RGB values of fully transparent
    // pixels with transparent black before selecting any transforms.
    if is_alpha {
        if let Some(token) = token {
            let mut pixels_until_checkpoint = VP8L_ALPHA_CLEANUP_CHECKPOINT_PIXELS;
            for pixel in &mut pixels {
                if *pixel >> 24 == 0 {
                    *pixel = 0;
                }
                pixels_until_checkpoint = pixels_until_checkpoint.saturating_sub(1);
                if pixels_until_checkpoint == 0 {
                    check_token(Some(token))?;
                    pixels_until_checkpoint = VP8L_ALPHA_CLEANUP_CHECKPOINT_PIXELS;
                }
            }
        } else {
            for pixel in &mut pixels {
                if *pixel >> 24 == 0 {
                    *pixel = 0;
                }
            }
        }
    }
    check_token(token)?;

    let palette = collect_palette(&pixels, token)?;
    let palette_size = (palette.len() <= VP8L_MAX_PALETTE_ENTRIES).then_some(palette.len());
    let transform_bits = if palette_size.is_some() { 5 } else { 3 };
    let (entropy_mode, red_and_blue_zero) = analyze_entropy(
        &pixels,
        width as usize,
        height as usize,
        palette_size,
        transform_bits,
        token,
    )?;
    check_token(token)?;

    match token {
        Some(token) => encode_frame_stream(
            &mut pixels,
            width,
            height,
            is_alpha,
            entropy_mode,
            red_and_blue_zero,
            transform_bits,
            palette,
            Some(token),
            TokenBitWriterCheckpoint {
                token,
                written_bits: 0,
                output_bytes: 0,
            },
            scratch,
        ),
        None => encode_frame_stream(
            &mut pixels,
            width,
            height,
            is_alpha,
            entropy_mode,
            red_and_blue_zero,
            transform_bits,
            palette,
            None,
            NoopBitWriterCheckpoint::default(),
            scratch,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(coverage, inline(never))]
fn encode_alpha_stream<C: BitWriterCheckpoint>(
    palette_delta: &[u32],
    palette_len: usize,
    packed: &[u32],
    packed_width: usize,
    alpha: &[u8],
    output_scratch: &mut Vec<u8>,
    token_scratch: &mut TokenStreamScratch,
    token: Option<&crate::CancellationToken>,
    checkpoint: C,
) -> Result<Vec<u8>, EncodingError> {
    // The nested candidate writer leaves one bounded suffix buffer in
    // `output_scratch`. Recycle that allocation for the final ALPH bitstream
    // instead of allocating a fresh output vector for every sequential frame;
    // nested trials refill the scratch buffer after this vector is taken.
    let mut encoded = core::mem::take(output_scratch);
    encoded.clear();
    let mut writer = BitWriter {
        writer: &mut encoded,
        buffer: 0,
        nbits: 0,
        checkpoint,
    };
    writer.write_bits(1, 1)?; // transform present
    writer.write_bits(3, 2)?; // color-indexing transform
    writer.write_bits((palette_len - 1) as u64, 8)?;
    write_image_stream_configured_with_scratch(
        &mut writer,
        palette_delta,
        palette_len,
        false,
        3,
        20,
        0,
        output_scratch,
        token_scratch,
        token,
    )?;

    writer.write_bits(0, 1)?; // transforms done
    write_image_stream_configured_with_scratch(
        &mut writer,
        packed,
        packed_width,
        true,
        5,
        32,
        2,
        output_scratch,
        token_scratch,
        token,
    )?;
    writer.flush()?;

    // The ordinary path already knows which ALPH representation is shorter.
    // Avoid materializing both candidates: reuse the encoded allocation for
    // either winner, replacing its contents with the raw plane when that is
    // shorter and inserting the one-byte ALPH header in place when compression
    // wins.
    // Keep the token-aware path below unchanged because its per-candidate copy
    // checkpoints are part of the existing Rust-only work-budget contract.
    if token.is_none() {
        if alpha.len() <= encoded.len() {
            if alpha.len() == encoded.len() {
                encoded.reserve(1);
            }
            encoded.clear();
            encoded.push(0); // no compression, no filtering, no preprocessing
            encoded.extend_from_slice(alpha);
            return Ok(encoded);
        }

        let encoded_length = encoded.len();
        encoded.reserve(1);
        encoded.push(0);
        encoded.copy_within(..encoded_length, 1);
        encoded[0] = 1; // lossless compression, no filtering, no preprocessing
        return Ok(encoded);
    }

    let mut compressed = Vec::with_capacity(encoded.len() + 1);
    compressed.push(1); // lossless compression, no filtering, no preprocessing
    #[cfg(coverage)]
    coverage_record_token_remaining(&COVERAGE_ALPHA_COMPRESSED_REMAINING, token);
    extend_bytes_with_checkpoint(&mut compressed, &encoded, token)?;

    let mut uncompressed = Vec::with_capacity(alpha.len() + 1);
    uncompressed.push(0); // no compression, no filtering, no preprocessing
    #[cfg(coverage)]
    coverage_record_token_remaining(&COVERAGE_ALPHA_UNCOMPRESSED_REMAINING, token);
    extend_bytes_with_checkpoint(&mut uncompressed, alpha, token)?;

    check_token(token)?;
    Ok(if uncompressed.len() <= compressed.len() {
        uncompressed
    } else {
        compressed
    })
}

// Alpha output may be copied from either the compressed VP8L stream or the
// raw alpha plane. The lossless VP8L RIFF container uses the same helper for
// its complete frame payload. Keep ordinary paths as single bulk copies, but
// bound caller-controlled copies at the same 1,024-byte output interval used
// by the bit writer. Pillow has no caller token or typed work-budget result for
// these copies, so this is Rust-only work-control evidence.
#[cfg_attr(coverage, inline(never))]
fn extend_bytes_with_checkpoint(
    output: &mut Vec<u8>,
    source: &[u8],
    token: Option<&crate::CancellationToken>,
) -> Result<(), EncodingError> {
    let Some(token) = token else {
        output.extend_from_slice(source);
        return Ok(());
    };
    for chunk in source.chunks(VP8L_OUTPUT_CHECKPOINT_BYTES) {
        output.extend_from_slice(chunk);
        if chunk.len() == VP8L_OUTPUT_CHECKPOINT_BYTES {
            check_token(Some(token))?;
        }
    }
    Ok(())
}

#[cfg_attr(coverage, inline(never))]
fn collect_alpha_palette(
    alpha: &[u8],
    token: Option<&crate::CancellationToken>,
) -> Result<AlphaPalette, EncodingError> {
    // Alpha values have a fixed 8-bit alphabet. A presence table preserves
    // BTreeSet's sorted, unique result while keeping the bounded membership
    // workspace on the stack; the returned fixed palette remains the single
    // owned representation required by the later delta/index passes.
    let mut present = [false; 256];
    let mut palette_len = 0;
    let mut samples_until_checkpoint = WEBP_ALPHA_PALETTE_CHECKPOINT_PIXELS;
    for &value in alpha {
        let slot = &mut present[usize::from(value)];
        if !*slot {
            *slot = true;
            palette_len += 1;
        }
        samples_until_checkpoint = samples_until_checkpoint.saturating_sub(1);
        if samples_until_checkpoint == 0 {
            check_token(token)?;
            samples_until_checkpoint = WEBP_ALPHA_PALETTE_CHECKPOINT_PIXELS;
        }
    }

    let mut palette = AlphaPalette {
        values: [0; VP8L_MAX_PALETTE_ENTRIES],
        len: palette_len,
    };
    let mut palette_index = 0;
    for (value, &is_present) in present.iter().enumerate() {
        if is_present {
            palette.values[palette_index] = value as u8;
            palette_index += 1;
        }
    }
    Ok(palette)
}

// Every sorted palette suffix is non-empty by construction.
#[cfg(coverage)]
#[allow(clippy::unwrap_used)]
pub(crate) fn encode_alpha(
    alpha: &[u8],
    width: u32,
    height: u32,
    token: Option<&crate::CancellationToken>,
) -> Result<Vec<u8>, EncodingError> {
    let mut scratch = ImageStreamScratch::default();
    encode_alpha_with_scratch(alpha, width, height, &mut scratch, token)
}

#[allow(clippy::unwrap_used)]
#[cfg_attr(coverage, inline(never))]
fn encode_alpha_with_scratch(
    alpha: &[u8],
    width: u32,
    height: u32,
    scratch: &mut ImageStreamScratch,
    token: Option<&crate::CancellationToken>,
) -> Result<Vec<u8>, EncodingError> {
    check_token(token)?;
    assert_eq!(alpha.len(), width as usize * height as usize);

    let mut palette_values = collect_alpha_palette(alpha, token)?;
    let palette_len = palette_values.len;
    let palette_values = &mut palette_values.values[..palette_len];
    let mut signs = 0u8;
    let mut predicted = 0u8;
    for (index, &value) in palette_values.iter().enumerate() {
        if index.is_multiple_of(64) {
            check_token(token)?;
        }
        let delta = value.wrapping_sub(predicted);
        if delta != 0 {
            signs |= if delta < 0x80 { 1 } else { 2 };
        }
        predicted = value;
    }
    if signs == 3 {
        let mut sortable_len = palette_values.len();
        if sortable_len > 17 && palette_values[0] == 0 {
            sortable_len -= 1;
            palette_values.swap(0, sortable_len);
        }
        predicted = 0;
        if let Some(token) = token {
            for index in 0..sortable_len {
                if index.is_multiple_of(WEBP_ALPHA_PALETTE_CHECKPOINT_VALUES) {
                    check_token(Some(token))?;
                }
                let mut candidates_until_checkpoint = WEBP_ALPHA_PALETTE_CHECKPOINT_VALUES;
                let mut best_offset = 0;
                let mut best_distance = u8::MAX;
                for (offset, &value) in palette_values[index..sortable_len].iter().enumerate() {
                    let delta = value.wrapping_sub(predicted);
                    let distance = delta.min(0u8.wrapping_sub(delta));
                    if distance < best_distance {
                        best_offset = offset;
                        best_distance = distance;
                    }
                    candidates_until_checkpoint = candidates_until_checkpoint.saturating_sub(1);
                    if candidates_until_checkpoint == 0 {
                        check_token(Some(token))?;
                        candidates_until_checkpoint = WEBP_ALPHA_PALETTE_CHECKPOINT_VALUES;
                    }
                }
                palette_values.swap(index, index + best_offset);
                predicted = palette_values[index];
            }
        } else {
            for index in 0..sortable_len {
                let (offset, _) = palette_values[index..sortable_len]
                    .iter()
                    .enumerate()
                    .map(|(offset, &value)| {
                        let delta = value.wrapping_sub(predicted);
                        (offset, delta.min(0u8.wrapping_sub(delta)))
                    })
                    .min_by_key(|&(_, distance)| distance)
                    .unwrap();
                palette_values.swap(index, index + offset);
                predicted = palette_values[index];
            }
        }
    }
    let mut palette_indices = [0u8; 256];
    for (index, &value) in palette_values.iter().enumerate() {
        if index.is_multiple_of(64) {
            check_token(token)?;
        }
        palette_indices[usize::from(value)] = index as u8;
    }
    // Alpha palettes are bounded to the 8-bit palette alphabet. Keep the
    // sorted source values available for index lookup without allocating a
    // second heap vector for their transformed view.
    let mut palette_delta = [0_u32; 256];
    let mut previous = 0u32;
    for (index, &value) in palette_values.iter().enumerate() {
        if index.is_multiple_of(64) {
            check_token(token)?;
        }
        let pixel = u32::from(value) << 8;
        let alpha = (pixel >> 24).wrapping_sub(previous >> 24) & 0xff;
        let red = ((pixel >> 16) & 0xff).wrapping_sub((previous >> 16) & 0xff) & 0xff;
        let green = ((pixel >> 8) & 0xff).wrapping_sub((previous >> 8) & 0xff) & 0xff;
        let blue = (pixel & 0xff).wrapping_sub(previous & 0xff) & 0xff;
        palette_delta[index] = alpha << 24 | red << 16 | green << 8 | blue;
        previous = pixel;
    }

    let xbits = match palette_len {
        0..=2 => 3,
        3..=4 => 2,
        5..=16 => 1,
        _ => 0,
    };
    let pixels_per_group = 1usize << xbits;
    let bits_per_pixel = 8 >> xbits;
    let packed_width = width.div_ceil(pixels_per_group as u32) as usize;
    let packed_len = packed_width * height as usize;
    {
        let packed = &mut scratch.alpha_packed;
        packed.clear();
        packed.reserve(packed_len);
        if let Some(token) = token {
            check_token(Some(token))?;
            let mut source_pixels_until_checkpoint = WEBP_ALPHA_PALETTE_PACKING_CHECKPOINT_PIXELS;
            for row in alpha.chunks_exact(width as usize) {
                for group in row.chunks(pixels_per_group) {
                    let mut pixel = 0xff00_0000u32;
                    for (index, &value) in group.iter().enumerate() {
                        let palette_index = u32::from(palette_indices[usize::from(value)]);
                        pixel |= palette_index << (8 + bits_per_pixel * index);
                    }
                    packed.push(pixel);
                    source_pixels_until_checkpoint =
                        source_pixels_until_checkpoint.saturating_sub(group.len());
                    if source_pixels_until_checkpoint == 0 {
                        check_token(Some(token))?;
                        source_pixels_until_checkpoint =
                            WEBP_ALPHA_PALETTE_PACKING_CHECKPOINT_PIXELS;
                    }
                }
            }
        } else {
            for row in alpha.chunks_exact(width as usize) {
                for group in row.chunks(pixels_per_group) {
                    let mut pixel = 0xff00_0000u32;
                    for (index, &value) in group.iter().enumerate() {
                        let palette_index = u32::from(palette_indices[usize::from(value)]);
                        pixel |= palette_index << (8 + bits_per_pixel * index);
                    }
                    packed.push(pixel);
                }
            }
        }
    }

    let packed = &scratch.alpha_packed;
    match token {
        Some(token) => encode_alpha_stream(
            &palette_delta[..palette_len],
            palette_len,
            packed,
            packed_width,
            alpha,
            &mut scratch.output,
            &mut scratch.tokens,
            Some(token),
            TokenBitWriterCheckpoint {
                token,
                written_bits: 0,
                output_bytes: 0,
            },
        ),
        None => encode_alpha_stream(
            &palette_delta[..palette_len],
            palette_len,
            packed,
            packed_width,
            alpha,
            &mut scratch.output,
            &mut scratch.tokens,
            None,
            NoopBitWriterCheckpoint::default(),
        ),
    }
}

const fn chunk_size(inner_bytes: usize) -> u32 {
    if inner_bytes % 2 == 1 {
        (inner_bytes + 1) as u32 + 8
    } else {
        inner_bytes as u32 + 8
    }
}

#[cfg_attr(coverage, inline(never))]
fn write_chunk(
    output: &mut Vec<u8>,
    name: &[u8],
    data: &[u8],
    token: Option<&crate::CancellationToken>,
) -> Result<(), EncodingError> {
    debug_assert!(name.len() == 4);

    output.extend_from_slice(name);
    output.extend_from_slice(&(data.len() as u32).to_le_bytes());
    extend_bytes_with_checkpoint(output, data, token)?;
    if data.len() % 2 == 1 {
        output.push(0);
    }
    Ok(())
}

/// WebP Encoder.
pub struct WebPEncoder {
    // Lossy RGBA ALPH encoding consumes the extracted alpha channel before the
    // next sequential frame can use this encoder. Retain only its capacity;
    // the logical channel is cleared after every encode attempt.
    alpha_scratch: Vec<u8>,
    // Lossless animation frames are encoded sequentially. Retain the bounded
    // VP8L transform, histogram, token, and bitstream scratch between frames;
    // each returned frame owns its output bytes independently.
    scratch: Option<ImageStreamScratch>,
}

impl WebPEncoder {
    /// Create a new in-memory lossless encoder.
    ///
    /// Only supports "VP8L" lossless encoding.
    pub const fn new() -> Self {
        Self {
            alpha_scratch: Vec::new(),
            scratch: None,
        }
    }

    /// Encode a lossy WebP alpha substream after reusing the RGBA alpha
    /// extraction workspace for the next sequential frame.
    pub(crate) fn encode_alpha_from_rgba_with_token(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
        token: Option<&crate::CancellationToken>,
    ) -> Result<Vec<u8>, EncodingError> {
        self.alpha_scratch.clear();
        self.alpha_scratch.reserve(rgba.len() / 4);
        let result = if let Some(token) = token {
            let mut pixels_until_checkpoint = VP8L_ALPHA_CHANNEL_CHECKPOINT_PIXELS;
            (|| {
                for pixel in rgba.as_chunks::<4>().0 {
                    self.alpha_scratch.push(pixel[3]);
                    pixels_until_checkpoint = pixels_until_checkpoint.saturating_sub(1);
                    if pixels_until_checkpoint == 0 {
                        check_token(Some(token))?;
                        pixels_until_checkpoint = VP8L_ALPHA_CHANNEL_CHECKPOINT_PIXELS;
                    }
                }
                Ok(())
            })()
        } else {
            self.alpha_scratch
                .extend(rgba.as_chunks::<4>().0.iter().map(|pixel| pixel[3]));
            Ok(())
        };
        let result = result.and_then(|()| {
            let scratch = self.scratch.get_or_insert_with(ImageStreamScratch::default);
            encode_alpha_with_scratch(&self.alpha_scratch, width, height, scratch, token)
        });
        self.alpha_scratch.clear();
        result
    }

    /// Encode image data while polling an optional cooperative work token.
    ///
    /// The encoder retains bounded scratch so sequential animation frames can
    /// reuse its VP8L working storage without sharing their output buffers.
    #[cfg_attr(coverage, inline(never))]
    pub(crate) fn encode_with_token(
        &mut self,
        data: &[u8],
        width: u32,
        height: u32,
        color: ColorType,
        token: Option<&crate::CancellationToken>,
    ) -> Result<Vec<u8>, EncodingError> {
        let scratch = self.scratch.get_or_insert_with(ImageStreamScratch::default);
        let mut frame = encode_frame(data, width, height, color, token, scratch)?;
        #[cfg(coverage)]
        coverage_record_token_remaining(&COVERAGE_ENCODER_FRAME_REMAINING, token);
        check_token(token)?;

        // The ordinary path has no caller-visible checkpoint during the final
        // container copy. Reuse the completed frame allocation by shifting it
        // behind the RIFF/VP8L headers; keep the token-aware copy below so its
        // existing output-copy checkpoints and cancellation behavior remain
        // unchanged.
        if token.is_none() {
            let frame_length = frame.len();
            let padding = frame_length % 2;
            frame.reserve(20 + padding);
            frame.resize(frame_length + 20, 0);
            frame.copy_within(..frame_length, 20);
            frame[..4].copy_from_slice(b"RIFF");
            frame[4..8].copy_from_slice(&(chunk_size(frame_length) + 4).to_le_bytes());
            frame[8..12].copy_from_slice(b"WEBP");
            frame[12..16].copy_from_slice(b"VP8L");
            frame[16..20].copy_from_slice(&(frame_length as u32).to_le_bytes());
            if padding != 0 {
                frame.push(0);
            }
            return Ok(frame);
        }

        let mut output = Vec::with_capacity(frame.len().saturating_add(20));
        output.extend_from_slice(b"RIFF");
        output.extend_from_slice(&(chunk_size(frame.len()) + 4).to_le_bytes());
        output.extend_from_slice(b"WEBP");
        #[cfg(coverage)]
        coverage_record_token_remaining(&COVERAGE_ENCODER_CHUNK_REMAINING, token);
        write_chunk(&mut output, b"VP8L", &frame, token)?;
        #[cfg(coverage)]
        coverage_record_token_remaining(&COVERAGE_ENCODER_FINAL_REMAINING, token);
        check_token(token)?;
        Ok(output)
    }
}

#[cfg(coverage)]
#[inline(never)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
fn __coverage_exercise_instrumented_generic_paths() {
    let direct_token = crate::CancellationToken::new();

    // Seed the nested checkpoint counters at their last interval. One small
    // write then observes every bit interval without a 2 MiB synthetic write.
    let mut boundary_checkpoint = TokenBitWriterCheckpoint {
        token: &direct_token,
        written_bits: VP8L_2097152_BITSTREAM_CHECKPOINT_BITS - VP8L_8_BITSTREAM_CHECKPOINT_BITS,
        output_bytes: VP8L_OUTPUT_CHECKPOINT_BYTES - 1,
    };
    let _ =
        std::hint::black_box(boundary_checkpoint.checkpoint_bits(VP8L_8_BITSTREAM_CHECKPOINT_BITS));
    let _ = std::hint::black_box(boundary_checkpoint.checkpoint_output_bytes(1));

    let direct_rle_lengths = [
        0_u8, 0, 1, 0, 0, 0, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    ];
    let mut direct_rle_tokens = Vec::new();
    let _ = std::hint::black_box(compressed_huffman_tokens_with_checkpoint(
        &direct_rle_lengths,
        &mut direct_rle_tokens,
        Some(&direct_token),
    ));

    let mut sampling_symbols = vec![1_u16; 64 * 64];
    let _ = std::hint::black_box(optimize_sampling(
        &mut sampling_symbols,
        64,
        64,
        0,
        6,
        Some(&direct_token),
    ));

    let mut direct_palette = (0..256)
        .map(|index| {
            let value = (index as u32).wrapping_mul(0x0103_0507);
            0xff00_0000 | (value & 0x00ff_ffff)
        })
        .collect::<Vec<_>>();
    let _ = std::hint::black_box(minimize_palette_deltas_with_checkpoint(
        &mut direct_palette,
        &direct_token,
    ));

    let mut direct_chunk = Vec::new();
    let _ = std::hint::black_box(write_chunk(
        &mut direct_chunk,
        b"TEST",
        &[0_u8; VP8L_OUTPUT_CHECKPOINT_BYTES],
        Some(&direct_token),
    ));

    let direct_alpha = (0..2_048).map(|index| index as u8).collect::<Vec<_>>();
    let _ = std::hint::black_box(encode_alpha(&direct_alpha, 64, 32, Some(&direct_token)));

    let direct_rgba = (0..64 * 32)
        .flat_map(|index| {
            let value = index as u8;
            [value, value.wrapping_mul(3), value.wrapping_mul(7), value]
        })
        .collect::<Vec<_>>();
    let mut direct_encoder = WebPEncoder::new();
    std::hint::black_box(
        direct_encoder
            .encode_with_token(&direct_rgba, 64, 32, ColorType::Rgba8, Some(&direct_token))
            .expect("instrumented token WebP frame must encode"),
    );

    // A varied meta-pixel map keeps histogram clustering in the multiple-group
    // case. The 1,024-symbol checkpoint is otherwise unreachable in the
    // small direct probes above.
    let meta_pixels = (0..64 * 32)
        .map(|index| {
            let value = index as u32;
            0xff00_0000
                | ((value & 0xff) << 16)
                | (((value.wrapping_mul(3)) & 0xff) << 8)
                | (value.wrapping_mul(7) & 0xff)
        })
        .collect::<Vec<_>>();
    let mut meta_bytes = Vec::new();
    let mut meta_writer = BitWriter {
        writer: &mut meta_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut meta_scratch = ImageStreamScratch::default();
    let _ = std::hint::black_box(write_image_stream(
        &mut meta_writer,
        &meta_pixels,
        64,
        true,
        &mut meta_scratch.output,
        &mut meta_scratch.tokens,
        Some(&direct_token),
    ));
    let _ = std::hint::black_box(meta_writer.flush());

    let large_meta_width = 256_usize;
    let large_meta_height = 128_usize;
    let large_meta_pixels = (0..large_meta_width * large_meta_height)
        .map(|index| {
            let value = (index as u32).wrapping_mul(0x45d9_f3b);
            0xff00_0000 | (value & 0x00ff_ffff)
        })
        .collect::<Vec<_>>();
    let large_meta_tokens = large_meta_pixels
        .iter()
        .copied()
        .map(backward_refs::Token::Literal)
        .collect::<Vec<_>>();
    let mut large_meta_bytes = Vec::new();
    let mut large_meta_writer = BitWriter {
        writer: &mut large_meta_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut large_meta_scratch = TokenStreamScratch::default();
    let _ = std::hint::black_box(write_token_stream(
        &mut large_meta_writer,
        &large_meta_pixels,
        large_meta_width,
        &large_meta_tokens,
        TokenStreamConfig {
            write_meta_huffman_bit: true,
            cache_bits: 0,
            histogram_bits: 2,
            quality: 100,
        },
        &mut large_meta_scratch,
        Some(&direct_token),
    ));
    let _ = large_meta_writer.flush();

    let dense_frequencies = (0..256)
        .map(|index| ((index * 37) % 251 + 1) as u32)
        .collect::<Vec<_>>();
    let mut huffman_bytes = Vec::new();
    let mut huffman_writer = BitWriter {
        writer: &mut huffman_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut huffman_lengths = vec![0; 256];
    let mut huffman_codes = vec![0; 256];
    let mut huffman_tokens = Vec::new();
    let mut optimized_frequencies = Vec::new();
    let mut huffman_rle_good = Vec::new();
    let mut huffman_nodes = Vec::new();
    let mut huffman_node_sort_scratch = Vec::new();
    let mut huffman_node_arena = Vec::new();
    let mut huffman_scratch = HuffmanTreeScratch {
        huffman_tokens: &mut huffman_tokens,
        optimized_frequencies: &mut optimized_frequencies,
        huffman_rle_good: &mut huffman_rle_good,
        nodes: &mut huffman_nodes,
        node_sort_scratch: &mut huffman_node_sort_scratch,
        node_arena: &mut huffman_node_arena,
    };
    for fail_after in 0..=2_048 {
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: NoopBitWriterCheckpoint { fail_after },
        };
        let mut lengths = vec![0; 256];
        let mut codes = vec![0; 256];
        let _ = std::hint::black_box(write_huffman_tree(
            &mut writer,
            &dense_frequencies,
            &mut lengths,
            &mut codes,
            &mut huffman_scratch,
            None,
        ));
    }
    let trimmed_frequencies = {
        let mut frequencies = vec![0; 256];
        frequencies[..4].fill(1);
        frequencies
    };
    for fail_after in 0..=128 {
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: NoopBitWriterCheckpoint { fail_after },
        };
        let mut lengths = vec![0; 256];
        let mut codes = vec![0; 256];
        let _ = std::hint::black_box(write_huffman_tree(
            &mut writer,
            &trimmed_frequencies,
            &mut lengths,
            &mut codes,
            &mut huffman_scratch,
            None,
        ));
    }
    for checks in 0..=512 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: NoopBitWriterCheckpoint::default(),
        };
        let mut lengths = vec![0; 256];
        let mut codes = vec![0; 256];
        let _ = std::hint::black_box(write_huffman_tree(
            &mut writer,
            &dense_frequencies,
            &mut lengths,
            &mut codes,
            &mut huffman_scratch,
            Some(&token),
        ));
    }
    for checks in 0..=512 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: TokenBitWriterCheckpoint {
                token: &token,
                written_bits: 0,
                output_bytes: 0,
            },
        };
        let mut lengths = vec![0; 256];
        let mut codes = vec![0; 256];
        let _ = std::hint::black_box(write_huffman_tree(
            &mut writer,
            &dense_frequencies,
            &mut lengths,
            &mut codes,
            &mut huffman_scratch,
            Some(&token),
        ));
    }
    let palette_token = crate::CancellationToken::new();
    let mut live_noop_bytes = Vec::new();
    let mut live_noop_writer = BitWriter {
        writer: &mut live_noop_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut live_noop_lengths = vec![0; 256];
    let mut live_noop_codes = vec![0; 256];
    std::hint::black_box(
        write_huffman_tree(
            &mut live_noop_writer,
            &dense_frequencies,
            &mut live_noop_lengths,
            &mut live_noop_codes,
            &mut huffman_scratch,
            Some(&palette_token),
        )
        .expect("instrumented token Huffman coverage input must encode"),
    );
    let _ = std::hint::black_box(live_noop_writer.flush());

    let mut live_token_bytes = Vec::new();
    let mut live_token_writer = BitWriter {
        writer: &mut live_token_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &palette_token,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    let mut live_token_lengths = vec![0; 256];
    let mut live_token_codes = vec![0; 256];
    std::hint::black_box(
        write_huffman_tree(
            &mut live_token_writer,
            &dense_frequencies,
            &mut live_token_lengths,
            &mut live_token_codes,
            &mut huffman_scratch,
            Some(&palette_token),
        )
        .expect("instrumented bit-token Huffman coverage input must encode"),
    );
    let _ = std::hint::black_box(live_token_writer.flush());

    let mut trimmed_token_bytes = Vec::new();
    let mut trimmed_token_writer = BitWriter {
        writer: &mut trimmed_token_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &palette_token,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    let mut trimmed_token_lengths = vec![0; 256];
    let mut trimmed_token_codes = vec![0; 256];
    std::hint::black_box(
        write_huffman_tree(
            &mut trimmed_token_writer,
            &trimmed_frequencies,
            &mut trimmed_token_lengths,
            &mut trimmed_token_codes,
            &mut huffman_scratch,
            Some(&palette_token),
        )
        .expect("instrumented trimmed bit-token Huffman coverage input must encode"),
    );
    let _ = std::hint::black_box(trimmed_token_writer.flush());

    for initial_bits in 0..8 {
        let trimmed_writer_probe_token = crate::CancellationToken::new();
        trimmed_writer_probe_token.cancel_after(usize::MAX);
        let trimmed_function_probe_token = crate::CancellationToken::new();
        let mut trimmed_writer_probe_bytes = Vec::new();
        let mut trimmed_writer_probe = BitWriter {
            writer: &mut trimmed_writer_probe_bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: TokenBitWriterCheckpoint {
                token: &trimmed_writer_probe_token,
                written_bits: initial_bits,
                output_bytes: 0,
            },
        };
        let mut trimmed_writer_probe_lengths = vec![0; 256];
        let mut trimmed_writer_probe_codes = vec![0; 256];
        let _ = std::hint::black_box(write_huffman_tree(
            &mut trimmed_writer_probe,
            &trimmed_frequencies,
            &mut trimmed_writer_probe_lengths,
            &mut trimmed_writer_probe_codes,
            &mut huffman_scratch,
            Some(&trimmed_function_probe_token),
        ));
        let trimmed_writer_checks = usize::MAX.saturating_sub(
            trimmed_writer_probe_token
                .coverage_remaining_checks()
                .unwrap_or(usize::MAX),
        );
        for fail_after in 0..=trimmed_writer_checks {
            let writer_token = crate::CancellationToken::new();
            writer_token.cancel_after(fail_after);
            let function_token = crate::CancellationToken::new();
            let mut bytes = Vec::new();
            let mut writer = BitWriter {
                writer: &mut bytes,
                buffer: 0,
                nbits: 0,
                checkpoint: TokenBitWriterCheckpoint {
                    token: &writer_token,
                    written_bits: initial_bits,
                    output_bytes: 0,
                },
            };
            let mut lengths = vec![0; 256];
            let mut codes = vec![0; 256];
            let _ = std::hint::black_box(write_huffman_tree(
                &mut writer,
                &trimmed_frequencies,
                &mut lengths,
                &mut codes,
                &mut huffman_scratch,
                Some(&function_token),
            ));
        }
    }

    let mut direct_reference_group = GroupCodes::default();
    direct_reference_group.lengths[0].resize(280, 1);
    direct_reference_group.codes[0].resize(280, 0);
    direct_reference_group.lengths[4].resize(2, 1);
    direct_reference_group.codes[4].resize(2, 0);
    let direct_reference_groups = [direct_reference_group];
    let direct_reference_context = TokenStreamReferenceContext {
        width: 8,
        multiple_groups: false,
        symbols: &[],
        encoded_histogram_bits: 0,
        tile_width: 1,
        groups: &direct_reference_groups,
    };
    let direct_reference_token = crate::CancellationToken::new();
    direct_reference_token.cancel_after(0);
    let mut direct_reference_bytes = Vec::new();
    let mut direct_reference_writer = BitWriter {
        writer: &mut direct_reference_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &direct_reference_token,
            written_bits: 14,
            output_bytes: 0,
        },
    };
    let _ = std::hint::black_box(write_token_reference(
        &mut direct_reference_writer,
        backward_refs::Token::Copy {
            distance: 1,
            length: 4,
        },
        0,
        direct_reference_context,
    ));

    let huffman_probe_token = crate::CancellationToken::new();
    huffman_probe_token.cancel_after(usize::MAX);
    let mut huffman_probe_bytes = Vec::new();
    let mut huffman_probe_writer = BitWriter {
        writer: &mut huffman_probe_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut huffman_probe_lengths = vec![0; 256];
    let mut huffman_probe_codes = vec![0; 256];
    let _ = std::hint::black_box(write_huffman_tree(
        &mut huffman_probe_writer,
        &dense_frequencies,
        &mut huffman_probe_lengths,
        &mut huffman_probe_codes,
        &mut huffman_scratch,
        Some(&huffman_probe_token),
    ));
    let huffman_noop_checks = usize::MAX.saturating_sub(
        huffman_probe_token
            .coverage_remaining_checks()
            .expect("coverage token must retain its remaining checks"),
    );
    for checks in 0..=huffman_noop_checks {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: NoopBitWriterCheckpoint::default(),
        };
        let mut lengths = vec![0; 256];
        let mut codes = vec![0; 256];
        let _ = std::hint::black_box(write_huffman_tree(
            &mut writer,
            &dense_frequencies,
            &mut lengths,
            &mut codes,
            &mut huffman_scratch,
            Some(&token),
        ));
    }

    let huffman_bit_probe_token = crate::CancellationToken::new();
    huffman_bit_probe_token.cancel_after(usize::MAX);
    let mut huffman_bit_probe_bytes = Vec::new();
    let mut huffman_bit_probe_writer = BitWriter {
        writer: &mut huffman_bit_probe_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &huffman_bit_probe_token,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    let mut huffman_bit_probe_lengths = vec![0; 256];
    let mut huffman_bit_probe_codes = vec![0; 256];
    let _ = std::hint::black_box(write_huffman_tree(
        &mut huffman_bit_probe_writer,
        &dense_frequencies,
        &mut huffman_bit_probe_lengths,
        &mut huffman_bit_probe_codes,
        &mut huffman_scratch,
        Some(&huffman_bit_probe_token),
    ));
    let huffman_bit_checks = usize::MAX.saturating_sub(
        huffman_bit_probe_token
            .coverage_remaining_checks()
            .expect("coverage token must retain its remaining checks"),
    );
    for checks in 0..=huffman_bit_checks {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: TokenBitWriterCheckpoint {
                token: &token,
                written_bits: 0,
                output_bytes: 0,
            },
        };
        let mut lengths = vec![0; 256];
        let mut codes = vec![0; 256];
        let _ = std::hint::black_box(write_huffman_tree(
            &mut writer,
            &dense_frequencies,
            &mut lengths,
            &mut codes,
            &mut huffman_scratch,
            Some(&token),
        ));
    }
    std::hint::black_box(
        write_huffman_tree(
            &mut huffman_writer,
            &dense_frequencies,
            &mut huffman_lengths,
            &mut huffman_codes,
            &mut huffman_scratch,
            None,
        )
        .expect("instrumented Huffman coverage input must encode"),
    );
    let _ = std::hint::black_box(huffman_writer.flush());

    let palette = (0..20)
        .map(|index| {
            let value = (index * 11) as u32;
            0xff00_0000 | (value << 16) | ((value ^ 0x55) << 8) | (value ^ 0xaa)
        })
        .collect::<Vec<_>>();
    for fail_after in 0..=2_048 {
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: NoopBitWriterCheckpoint { fail_after },
        };
        let mut pixels = (0..(64 * 32))
            .map(|index| palette[index % palette.len()])
            .collect::<Vec<_>>();
        let mut scratch = ImageStreamScratch::default();
        let _ = std::hint::black_box(apply_palette(
            &mut writer,
            &mut pixels,
            64,
            32,
            palette.clone(),
            &mut scratch,
            None,
        ));
    }
    let mut palette_bytes = Vec::new();
    let mut palette_writer = BitWriter {
        writer: &mut palette_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut palette_pixels = (0..(64 * 32))
        .map(|index| palette[index % palette.len()])
        .collect::<Vec<_>>();
    let mut palette_scratch = ImageStreamScratch::default();
    std::hint::black_box(
        apply_palette(
            &mut palette_writer,
            &mut palette_pixels,
            64,
            32,
            palette,
            &mut palette_scratch,
            None,
        )
        .expect("instrumented palette coverage input must encode"),
    );
    let _ = std::hint::black_box(palette_writer.flush());

    let token_palette = (0..20)
        .map(|index| {
            let value = (index * 11) as u32;
            0xff00_0000 | (value << 16) | ((value ^ 0x55) << 8) | (value ^ 0xaa)
        })
        .collect::<Vec<_>>();
    let live_token = crate::CancellationToken::new();
    let mut token_palette_pixels = (0..(64 * 32))
        .map(|index| token_palette[index % token_palette.len()])
        .collect::<Vec<_>>();
    let mut token_palette_bytes = Vec::new();
    let mut token_palette_writer = BitWriter {
        writer: &mut token_palette_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    std::hint::black_box(
        apply_palette(
            &mut token_palette_writer,
            &mut token_palette_pixels,
            64,
            32,
            token_palette.clone(),
            &mut palette_scratch,
            Some(&live_token),
        )
        .expect("instrumented token palette coverage input must encode"),
    );
    let _ = std::hint::black_box(token_palette_writer.flush());

    let mut token_palette_pixels = (0..(64 * 32))
        .map(|index| token_palette[index % token_palette.len()])
        .collect::<Vec<_>>();
    let mut token_palette_bytes = Vec::new();
    let mut token_palette_writer = BitWriter {
        writer: &mut token_palette_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &live_token,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    std::hint::black_box(
        apply_palette(
            &mut token_palette_writer,
            &mut token_palette_pixels,
            64,
            32,
            token_palette.clone(),
            &mut palette_scratch,
            Some(&live_token),
        )
        .expect("instrumented bit-token palette coverage input must encode"),
    );
    let _ = std::hint::black_box(token_palette_writer.flush());

    let apply_probe_token = crate::CancellationToken::new();
    apply_probe_token.cancel_after(usize::MAX);
    let mut apply_probe_bytes = Vec::new();
    let mut apply_probe_writer = BitWriter {
        writer: &mut apply_probe_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut apply_probe_pixels = (0..(64 * 32))
        .map(|index| token_palette[index % token_palette.len()])
        .collect::<Vec<_>>();
    let mut apply_probe_scratch = ImageStreamScratch::default();
    let _ = std::hint::black_box(apply_palette(
        &mut apply_probe_writer,
        &mut apply_probe_pixels,
        64,
        32,
        token_palette.clone(),
        &mut apply_probe_scratch,
        Some(&apply_probe_token),
    ));
    let apply_noop_checks = usize::MAX.saturating_sub(
        apply_probe_token
            .coverage_remaining_checks()
            .expect("coverage token must retain its remaining checks"),
    );
    for checks in 0..=apply_noop_checks {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: NoopBitWriterCheckpoint::default(),
        };
        let mut pixels = (0..(64 * 32))
            .map(|index| token_palette[index % token_palette.len()])
            .collect::<Vec<_>>();
        let mut scratch = ImageStreamScratch::default();
        let _ = std::hint::black_box(apply_palette(
            &mut writer,
            &mut pixels,
            64,
            32,
            token_palette.clone(),
            &mut scratch,
            Some(&token),
        ));
    }

    let apply_bit_probe_token = crate::CancellationToken::new();
    apply_bit_probe_token.cancel_after(usize::MAX);
    let mut apply_bit_probe_bytes = Vec::new();
    let mut apply_bit_probe_writer = BitWriter {
        writer: &mut apply_bit_probe_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &apply_bit_probe_token,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    let mut apply_bit_probe_pixels = (0..(64 * 32))
        .map(|index| token_palette[index % token_palette.len()])
        .collect::<Vec<_>>();
    let mut apply_bit_probe_scratch = ImageStreamScratch::default();
    let _ = std::hint::black_box(apply_palette(
        &mut apply_bit_probe_writer,
        &mut apply_bit_probe_pixels,
        64,
        32,
        token_palette.clone(),
        &mut apply_bit_probe_scratch,
        Some(&apply_bit_probe_token),
    ));
    let apply_bit_checks = usize::MAX.saturating_sub(
        apply_bit_probe_token
            .coverage_remaining_checks()
            .expect("coverage token must retain its remaining checks"),
    );
    for checks in 0..=apply_bit_checks {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: TokenBitWriterCheckpoint {
                token: &token,
                written_bits: 0,
                output_bytes: 0,
            },
        };
        let mut pixels = (0..(64 * 32))
            .map(|index| token_palette[index % token_palette.len()])
            .collect::<Vec<_>>();
        let mut scratch = ImageStreamScratch::default();
        let _ = std::hint::black_box(apply_palette(
            &mut writer,
            &mut pixels,
            64,
            32,
            token_palette.clone(),
            &mut scratch,
            Some(&token),
        ));
    }

    let alpha_palette_delta = (0..20)
        .map(|index| 0xff00_0000 | ((index as u32) << 8))
        .collect::<Vec<_>>();
    let alpha_packed = (0..64)
        .map(|index| 0xff00_0000 | (index as u32))
        .collect::<Vec<_>>();
    let alpha = (0..64).map(|index| (index * 17) as u8).collect::<Vec<_>>();
    for fail_after in 0..=2_048 {
        let mut output = Vec::new();
        let mut token_scratch = TokenStreamScratch::default();
        let _ = std::hint::black_box(encode_alpha_stream(
            &alpha_palette_delta,
            alpha_palette_delta.len(),
            &alpha_packed,
            8,
            &alpha,
            &mut output,
            &mut token_scratch,
            None,
            NoopBitWriterCheckpoint { fail_after },
        ));
    }
    let mut alpha_output = Vec::new();
    let mut alpha_scratch = TokenStreamScratch::default();
    std::hint::black_box(
        encode_alpha_stream(
            &alpha_palette_delta,
            alpha_palette_delta.len(),
            &alpha_packed,
            8,
            &alpha,
            &mut alpha_output,
            &mut alpha_scratch,
            None,
            NoopBitWriterCheckpoint::default(),
        )
        .expect("instrumented alpha coverage input must encode"),
    );

    let live_alpha_token = crate::CancellationToken::new();
    let mut token_alpha_output = Vec::new();
    let mut token_alpha_scratch = TokenStreamScratch::default();
    std::hint::black_box(
        encode_alpha_stream(
            &alpha_palette_delta,
            alpha_palette_delta.len(),
            &alpha_packed,
            8,
            &alpha,
            &mut token_alpha_output,
            &mut token_alpha_scratch,
            Some(&live_alpha_token),
            NoopBitWriterCheckpoint::default(),
        )
        .expect("instrumented token alpha coverage input must encode"),
    );
    let mut token_alpha_output = Vec::new();
    let mut token_alpha_scratch = TokenStreamScratch::default();
    std::hint::black_box(
        encode_alpha_stream(
            &alpha_palette_delta,
            alpha_palette_delta.len(),
            &alpha_packed,
            8,
            &alpha,
            &mut token_alpha_output,
            &mut token_alpha_scratch,
            Some(&live_alpha_token),
            TokenBitWriterCheckpoint {
                token: &live_alpha_token,
                written_bits: 0,
                output_bytes: 0,
            },
        )
        .expect("instrumented bit-token alpha coverage input must encode"),
    );

    let alpha_probe_token = crate::CancellationToken::new();
    alpha_probe_token.cancel_after(usize::MAX);
    let mut alpha_probe_output = Vec::new();
    let mut alpha_probe_scratch = TokenStreamScratch::default();
    let _ = std::hint::black_box(encode_alpha_stream(
        &alpha_palette_delta,
        alpha_palette_delta.len(),
        &alpha_packed,
        8,
        &alpha,
        &mut alpha_probe_output,
        &mut alpha_probe_scratch,
        Some(&alpha_probe_token),
        NoopBitWriterCheckpoint::default(),
    ));
    let alpha_noop_checks = usize::MAX.saturating_sub(
        alpha_probe_token
            .coverage_remaining_checks()
            .expect("coverage token must retain its remaining checks"),
    );
    for checks in 0..=alpha_noop_checks {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut output = Vec::new();
        let mut token_scratch = TokenStreamScratch::default();
        let _ = std::hint::black_box(encode_alpha_stream(
            &alpha_palette_delta,
            alpha_palette_delta.len(),
            &alpha_packed,
            8,
            &alpha,
            &mut output,
            &mut token_scratch,
            Some(&token),
            NoopBitWriterCheckpoint::default(),
        ));
    }

    let alpha_bit_probe_token = crate::CancellationToken::new();
    alpha_bit_probe_token.cancel_after(usize::MAX);
    let mut alpha_bit_probe_output = Vec::new();
    let mut alpha_bit_probe_scratch = TokenStreamScratch::default();
    let _ = std::hint::black_box(encode_alpha_stream(
        &alpha_palette_delta,
        alpha_palette_delta.len(),
        &alpha_packed,
        8,
        &alpha,
        &mut alpha_bit_probe_output,
        &mut alpha_bit_probe_scratch,
        Some(&alpha_bit_probe_token),
        TokenBitWriterCheckpoint {
            token: &alpha_bit_probe_token,
            written_bits: 0,
            output_bytes: 0,
        },
    ));
    let alpha_bit_checks = usize::MAX.saturating_sub(
        alpha_bit_probe_token
            .coverage_remaining_checks()
            .expect("coverage token must retain its remaining checks"),
    );
    for checks in 0..=alpha_bit_checks {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut output = Vec::new();
        let mut token_scratch = TokenStreamScratch::default();
        let _ = std::hint::black_box(encode_alpha_stream(
            &alpha_palette_delta,
            alpha_palette_delta.len(),
            &alpha_packed,
            8,
            &alpha,
            &mut output,
            &mut token_scratch,
            Some(&token),
            TokenBitWriterCheckpoint {
                token: &token,
                written_bits: 0,
                output_bytes: 0,
            },
        ));
    }

    for initial_bits in [7, 6] {
        let token = crate::CancellationToken::new();
        token.cancel_after(0);
        let mut output = Vec::new();
        let mut token_scratch = TokenStreamScratch::default();
        let _ = std::hint::black_box(encode_alpha_stream(
            &alpha_palette_delta,
            alpha_palette_delta.len(),
            &alpha_packed,
            8,
            &alpha,
            &mut output,
            &mut token_scratch,
            Some(&token),
            TokenBitWriterCheckpoint {
                token: &token,
                written_bits: initial_bits,
                output_bytes: 0,
            },
        ));
    }

    let empty_alpha = [];
    let mut empty_alpha_output = Vec::new();
    let mut empty_alpha_scratch = TokenStreamScratch::default();
    let _ = std::hint::black_box(encode_alpha_stream(
        &alpha_palette_delta,
        alpha_palette_delta.len(),
        &alpha_packed,
        8,
        &empty_alpha,
        &mut empty_alpha_output,
        &mut empty_alpha_scratch,
        None,
        NoopBitWriterCheckpoint::default(),
    ));
    let empty_alpha_token = crate::CancellationToken::new();
    let mut empty_alpha_token_output = Vec::new();
    let mut empty_alpha_token_scratch = TokenStreamScratch::default();
    let _ = std::hint::black_box(encode_alpha_stream(
        &alpha_palette_delta,
        alpha_palette_delta.len(),
        &alpha_packed,
        8,
        &empty_alpha,
        &mut empty_alpha_token_output,
        &mut empty_alpha_token_scratch,
        None,
        TokenBitWriterCheckpoint {
            token: &empty_alpha_token,
            written_bits: 0,
            output_bytes: 0,
        },
    ));

    let large_alpha = vec![0_u8; 64 * 1024];
    let mut large_alpha_output = Vec::new();
    let mut large_alpha_scratch = TokenStreamScratch::default();
    let _ = std::hint::black_box(encode_alpha_stream(
        &alpha_palette_delta,
        alpha_palette_delta.len(),
        &alpha_packed,
        8,
        &large_alpha,
        &mut large_alpha_output,
        &mut large_alpha_scratch,
        None,
        NoopBitWriterCheckpoint::default(),
    ));
    let large_alpha_checkpoint_token = crate::CancellationToken::new();
    let mut large_alpha_checkpoint_output = Vec::new();
    let mut large_alpha_checkpoint_scratch = TokenStreamScratch::default();
    let _ = std::hint::black_box(encode_alpha_stream(
        &alpha_palette_delta,
        alpha_palette_delta.len(),
        &alpha_packed,
        8,
        &large_alpha,
        &mut large_alpha_checkpoint_output,
        &mut large_alpha_checkpoint_scratch,
        None,
        TokenBitWriterCheckpoint {
            token: &large_alpha_checkpoint_token,
            written_bits: 0,
            output_bytes: 0,
        },
    ));
    let large_alpha_noop_token = crate::CancellationToken::new();
    let mut large_alpha_noop_token_output = Vec::new();
    let mut large_alpha_noop_token_scratch = TokenStreamScratch::default();
    let _ = std::hint::black_box(encode_alpha_stream(
        &alpha_palette_delta,
        alpha_palette_delta.len(),
        &alpha_packed,
        8,
        &large_alpha,
        &mut large_alpha_noop_token_output,
        &mut large_alpha_noop_token_scratch,
        Some(&large_alpha_noop_token),
        NoopBitWriterCheckpoint::default(),
    ));
    let large_alpha_token = crate::CancellationToken::new();
    let mut large_alpha_token_output = Vec::new();
    let mut large_alpha_token_scratch = TokenStreamScratch::default();
    let _ = std::hint::black_box(encode_alpha_stream(
        &alpha_palette_delta,
        alpha_palette_delta.len(),
        &alpha_packed,
        8,
        &large_alpha,
        &mut large_alpha_token_output,
        &mut large_alpha_token_scratch,
        Some(&large_alpha_token),
        TokenBitWriterCheckpoint {
            token: &large_alpha_token,
            written_bits: 0,
            output_bytes: 0,
        },
    ));

    #[cfg(coverage_nightly)]
    {
        // The ordinary ALPH path compares the raw plane against the encoded
        // candidate. First measure that candidate, then feed an equally sized
        // raw plane to reach the exact-size branch for both checkpoint types.
        let mut compressed_length = 0usize;
        for length in 0..=16_384 {
            let alpha = vec![0_u8; length];
            let mut ordinary_probe_output = Vec::new();
            let mut ordinary_probe_scratch = TokenStreamScratch::default();
            let encoded = encode_alpha_stream(
                &alpha_palette_delta,
                alpha_palette_delta.len(),
                &alpha_packed,
                8,
                &alpha,
                &mut ordinary_probe_output,
                &mut ordinary_probe_scratch,
                None,
                NoopBitWriterCheckpoint::default(),
            )
            .unwrap_or_default();
            if encoded.first().copied() == Some(1) {
                compressed_length = encoded.len().saturating_sub(1);
                break;
            }
        }
        let equal_alpha = vec![0_u8; compressed_length];
        let mut equal_output = Vec::new();
        let mut equal_scratch = TokenStreamScratch::default();
        let _ = std::hint::black_box(encode_alpha_stream(
            &alpha_palette_delta,
            alpha_palette_delta.len(),
            &alpha_packed,
            8,
            &equal_alpha,
            &mut equal_output,
            &mut equal_scratch,
            None,
            NoopBitWriterCheckpoint::default(),
        ));
        let equal_token = crate::CancellationToken::new();
        let mut equal_token_output = Vec::new();
        let mut equal_token_scratch = TokenStreamScratch::default();
        let _ = std::hint::black_box(encode_alpha_stream(
            &alpha_palette_delta,
            alpha_palette_delta.len(),
            &alpha_packed,
            8,
            &equal_alpha,
            &mut equal_token_output,
            &mut equal_token_scratch,
            None,
            TokenBitWriterCheckpoint {
                token: &equal_token,
                written_bits: 0,
                output_bytes: 0,
            },
        ));
    }

    for fail_after in 0..=2_048 {
        let mut pixels = vec![0xff40_4040; 16 * 16];
        let mut scratch = ImageStreamScratch::default();
        let _ = std::hint::black_box(encode_frame_stream(
            &mut pixels,
            16,
            16,
            false,
            EntropyMode::Spatial,
            true,
            1,
            Vec::new(),
            None,
            NoopBitWriterCheckpoint { fail_after },
            &mut scratch,
        ));
    }
    let mut pixels = vec![0xff40_4040; 16 * 16];
    let mut scratch = ImageStreamScratch::default();
    std::hint::black_box(
        encode_frame_stream(
            &mut pixels,
            16,
            16,
            false,
            EntropyMode::Spatial,
            true,
            1,
            Vec::new(),
            None,
            NoopBitWriterCheckpoint::default(),
            &mut scratch,
        )
        .expect("instrumented frame coverage input must encode"),
    );
    let live_frame_token = crate::CancellationToken::new();
    let mut token_pixels = vec![0xff40_4040; 16 * 16];
    let mut token_scratch = ImageStreamScratch::default();
    std::hint::black_box(
        encode_frame_stream(
            &mut token_pixels,
            16,
            16,
            false,
            EntropyMode::Spatial,
            true,
            1,
            Vec::new(),
            Some(&live_frame_token),
            NoopBitWriterCheckpoint::default(),
            &mut token_scratch,
        )
        .expect("instrumented token frame coverage input must encode"),
    );
    let mut token_pixels = vec![0xff40_4040; 16 * 16];
    let mut token_scratch = ImageStreamScratch::default();
    std::hint::black_box(
        encode_frame_stream(
            &mut token_pixels,
            16,
            16,
            false,
            EntropyMode::Spatial,
            true,
            1,
            Vec::new(),
            Some(&live_frame_token),
            TokenBitWriterCheckpoint {
                token: &live_frame_token,
                written_bits: 0,
                output_bytes: 0,
            },
            &mut token_scratch,
        )
        .expect("instrumented bit-token frame coverage input must encode"),
    );

    let frame_probe_token = crate::CancellationToken::new();
    frame_probe_token.cancel_after(usize::MAX);
    let mut frame_probe_pixels = vec![0xff40_4040; 16 * 16];
    let mut frame_probe_scratch = ImageStreamScratch::default();
    let _ = std::hint::black_box(encode_frame_stream(
        &mut frame_probe_pixels,
        16,
        16,
        false,
        EntropyMode::Spatial,
        true,
        1,
        Vec::new(),
        Some(&frame_probe_token),
        NoopBitWriterCheckpoint::default(),
        &mut frame_probe_scratch,
    ));
    let frame_noop_checks = usize::MAX.saturating_sub(
        frame_probe_token
            .coverage_remaining_checks()
            .expect("coverage token must retain its remaining checks"),
    );
    for checks in 0..=frame_noop_checks {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut pixels = vec![0xff40_4040; 16 * 16];
        let mut scratch = ImageStreamScratch::default();
        let _ = std::hint::black_box(encode_frame_stream(
            &mut pixels,
            16,
            16,
            false,
            EntropyMode::Spatial,
            true,
            1,
            Vec::new(),
            Some(&token),
            NoopBitWriterCheckpoint::default(),
            &mut scratch,
        ));
    }

    let frame_bit_probe_token = crate::CancellationToken::new();
    frame_bit_probe_token.cancel_after(usize::MAX);
    let mut frame_bit_probe_pixels = vec![0xff40_4040; 16 * 16];
    let mut frame_bit_probe_scratch = ImageStreamScratch::default();
    let _ = std::hint::black_box(encode_frame_stream(
        &mut frame_bit_probe_pixels,
        16,
        16,
        false,
        EntropyMode::Spatial,
        true,
        1,
        Vec::new(),
        Some(&frame_bit_probe_token),
        TokenBitWriterCheckpoint {
            token: &frame_bit_probe_token,
            written_bits: 0,
            output_bytes: 0,
        },
        &mut frame_bit_probe_scratch,
    ));
    let frame_bit_checks = usize::MAX.saturating_sub(
        frame_bit_probe_token
            .coverage_remaining_checks()
            .expect("coverage token must retain its remaining checks"),
    );
    for checks in 0..=frame_bit_checks {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut pixels = vec![0xff40_4040; 16 * 16];
        let mut scratch = ImageStreamScratch::default();
        let _ = std::hint::black_box(encode_frame_stream(
            &mut pixels,
            16,
            16,
            false,
            EntropyMode::Spatial,
            true,
            1,
            Vec::new(),
            Some(&token),
            TokenBitWriterCheckpoint {
                token: &token,
                written_bits: 0,
                output_bytes: 0,
            },
            &mut scratch,
        ));
    }

    // The subtract-green branch has a token checkpoint inside the transform,
    // but the existing frame sweeps use Spatial mode and therefore never
    // reach the outer `?` at that call site. Measure a small real subtract-
    // green frame, then replay its token checkpoints with the Noop writer so
    // the generic frame specialization observes the transform error.
    let subtract_frame_probe_token = crate::CancellationToken::new();
    subtract_frame_probe_token.cancel_after(usize::MAX);
    let mut subtract_frame_probe_pixels = vec![0xff40_4040; 32 * 32];
    let mut subtract_frame_probe_scratch = ImageStreamScratch::default();
    let _ = std::hint::black_box(encode_frame_stream(
        &mut subtract_frame_probe_pixels,
        32,
        32,
        false,
        EntropyMode::SubtractGreen,
        true,
        1,
        Vec::new(),
        Some(&subtract_frame_probe_token),
        NoopBitWriterCheckpoint::default(),
        &mut subtract_frame_probe_scratch,
    ));
    let subtract_frame_checks = usize::MAX.saturating_sub(
        subtract_frame_probe_token
            .coverage_remaining_checks()
            .expect("coverage token must retain its remaining checks"),
    );
    for checks in 0..=subtract_frame_checks {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut pixels = vec![0xff40_4040; 32 * 32];
        let mut scratch = ImageStreamScratch::default();
        let _ = std::hint::black_box(encode_frame_stream(
            &mut pixels,
            32,
            32,
            false,
            EntropyMode::SubtractGreen,
            true,
            1,
            Vec::new(),
            Some(&token),
            NoopBitWriterCheckpoint::default(),
            &mut scratch,
        ));
    }

    let frame_header_token = crate::CancellationToken::new();
    frame_header_token.cancel_after(4);
    let mut frame_header_pixels = vec![0xff40_4040; 16 * 16];
    let mut frame_header_scratch = ImageStreamScratch::default();
    let _ = std::hint::black_box(encode_frame_stream(
        &mut frame_header_pixels,
        16,
        16,
        false,
        EntropyMode::Spatial,
        true,
        1,
        Vec::new(),
        Some(&frame_header_token),
        TokenBitWriterCheckpoint {
            token: &frame_header_token,
            written_bits: 3,
            output_bytes: 0,
        },
        &mut frame_header_scratch,
    ));

    let grayscale_token = crate::CancellationToken::new();
    let _ = std::hint::black_box(pixels_are_grayscale_with_checkpoint(
        &[0xff20_2021],
        Some(&grayscale_token),
    ));

    coverage_exercise_remaining_encoder_errors();
}

#[cfg(coverage)]
#[coverage(off)]
#[inline(never)]
#[allow(dead_code)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
fn coverage_exercise_remaining_encoder_errors_exploration() {
    let multi_group_width = 32_usize;
    let multi_group_height = 32_usize;
    let multi_group_pixels = (0..multi_group_width * multi_group_height)
        .map(|index| {
            let x = index % multi_group_width;
            let y = index / multi_group_width;
            let tile = (x / 4) + (y / 4) * 8;
            let value = (tile as u32).wrapping_mul(0x1f3d_5b79);
            0xff00_0000 | (value & 0x00ff_ffff)
        })
        .collect::<Vec<_>>();
    let multi_group_tokens = multi_group_pixels
        .iter()
        .copied()
        .map(backward_refs::Token::Literal)
        .collect::<Vec<_>>();

    // The function token and the writer token are independent work budgets.
    // Keeping the writer checkpoint live isolates the polling edge inside the
    // meta-pixel materialization loop.
    let metadata_token_probe = crate::CancellationToken::new();
    metadata_token_probe.cancel_after(usize::MAX);
    let mut metadata_probe_bytes = Vec::new();
    let mut metadata_probe_writer = BitWriter {
        writer: &mut metadata_probe_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut metadata_probe_scratch = TokenStreamScratch::default();
    let _ = write_token_stream(
        &mut metadata_probe_writer,
        &multi_group_pixels,
        multi_group_width,
        &multi_group_tokens,
        TokenStreamConfig {
            write_meta_huffman_bit: true,
            cache_bits: 0,
            histogram_bits: 2,
            quality: 100,
        },
        &mut metadata_probe_scratch,
        Some(&metadata_token_probe),
    );
    let metadata_checks = usize::MAX.saturating_sub(
        metadata_token_probe
            .coverage_remaining_checks()
            .expect("coverage token must retain its remaining checks"),
    );
    for checks in 0..=metadata_checks {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: NoopBitWriterCheckpoint::default(),
        };
        let mut scratch = TokenStreamScratch::default();
        let _ = write_token_stream(
            &mut writer,
            &multi_group_pixels,
            multi_group_width,
            &multi_group_tokens,
            TokenStreamConfig {
                write_meta_huffman_bit: true,
                cache_bits: 0,
                histogram_bits: 2,
                quality: 100,
            },
            &mut scratch,
            Some(&token),
        );
    }

    // A token-aware writer has a separate checkpoint token. Sweep the measured
    // writer checks to reach the histogram-header `write_bits` error edge.
    let writer_token_probe = crate::CancellationToken::new();
    writer_token_probe.cancel_after(usize::MAX);
    let live_metadata_token = crate::CancellationToken::new();
    let mut writer_probe_bytes = Vec::new();
    let mut writer_probe = BitWriter {
        writer: &mut writer_probe_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &writer_token_probe,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    let mut writer_probe_scratch = TokenStreamScratch::default();
    let _ = write_token_stream(
        &mut writer_probe,
        &multi_group_pixels,
        multi_group_width,
        &multi_group_tokens,
        TokenStreamConfig {
            write_meta_huffman_bit: true,
            cache_bits: 0,
            histogram_bits: 2,
            quality: 100,
        },
        &mut writer_probe_scratch,
        Some(&live_metadata_token),
    );
    let writer_checks = usize::MAX.saturating_sub(
        writer_token_probe
            .coverage_remaining_checks()
            .expect("coverage token must retain its remaining checks"),
    );
    for checks in 0..=writer_checks {
        let writer_token = crate::CancellationToken::new();
        writer_token.cancel_after(checks);
        let live_token = crate::CancellationToken::new();
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: TokenBitWriterCheckpoint {
                token: &writer_token,
                written_bits: 0,
                output_bytes: 0,
            },
        };
        let mut scratch = TokenStreamScratch::default();
        let _ = write_token_stream(
            &mut writer,
            &multi_group_pixels,
            multi_group_width,
            &multi_group_tokens,
            TokenStreamConfig {
                write_meta_huffman_bit: true,
                cache_bits: 0,
                histogram_bits: 2,
                quality: 100,
            },
            &mut scratch,
            Some(&live_token),
        );
    }

    let grayscale_pixels = vec![0xff40_4040; 32 * 32];
    let grayscale_token = crate::CancellationToken::new();
    grayscale_token.cancel_after(0);
    let mut grayscale_pixels = grayscale_pixels.clone();
    let mut grayscale_scratch = ImageStreamScratch::default();
    let _ = encode_frame_stream(
        &mut grayscale_pixels,
        32,
        32,
        false,
        EntropyMode::Spatial,
        true,
        1,
        Vec::new(),
        Some(&grayscale_token),
        NoopBitWriterCheckpoint::default(),
        &mut grayscale_scratch,
    );
    let grayscale_writer_token = crate::CancellationToken::new();
    let mut grayscale_pixels = grayscale_pixels.clone();
    let mut grayscale_scratch = ImageStreamScratch::default();
    let _ = encode_frame_stream(
        &mut grayscale_pixels,
        32,
        32,
        false,
        EntropyMode::Spatial,
        true,
        1,
        Vec::new(),
        Some(&grayscale_token),
        TokenBitWriterCheckpoint {
            token: &grayscale_writer_token,
            written_bits: 0,
            output_bytes: 0,
        },
        &mut grayscale_scratch,
    );

    let non_grayscale_frame = (0..16 * 16)
        .map(|index| {
            let value = index as u32;
            0xff00_0000 | ((value & 0xff) << 16) | (((value * 3) & 0xff) << 8) | value
        })
        .collect::<Vec<_>>();
    for fail_after in 0..=512 {
        let mut pixels = non_grayscale_frame.clone();
        let mut scratch = ImageStreamScratch::default();
        let _ = encode_frame_stream(
            &mut pixels,
            16,
            16,
            false,
            EntropyMode::SubtractGreen,
            false,
            1,
            Vec::new(),
            None,
            NoopBitWriterCheckpoint { fail_after },
            &mut scratch,
        );
    }
    for checks in 0..=512 {
        let writer_token = crate::CancellationToken::new();
        writer_token.cancel_after(checks);
        let mut pixels = non_grayscale_frame.clone();
        let mut scratch = ImageStreamScratch::default();
        let _ = encode_frame_stream(
            &mut pixels,
            16,
            16,
            false,
            EntropyMode::SubtractGreen,
            false,
            1,
            Vec::new(),
            None,
            TokenBitWriterCheckpoint {
                token: &writer_token,
                written_bits: 0,
                output_bytes: 0,
            },
            &mut scratch,
        );
    }
    for fail_after in 0..=512 {
        let mut pixels = non_grayscale_frame.clone();
        let mut scratch = ImageStreamScratch::default();
        let _ = encode_frame_stream(
            &mut pixels,
            16,
            16,
            false,
            EntropyMode::SpatialSubtractGreen,
            false,
            1,
            Vec::new(),
            None,
            NoopBitWriterCheckpoint { fail_after },
            &mut scratch,
        );
    }
    for checks in 0..=512 {
        let writer_token = crate::CancellationToken::new();
        writer_token.cancel_after(checks);
        let mut pixels = non_grayscale_frame.clone();
        let mut scratch = ImageStreamScratch::default();
        let _ = encode_frame_stream(
            &mut pixels,
            16,
            16,
            false,
            EntropyMode::SpatialSubtractGreen,
            false,
            1,
            Vec::new(),
            None,
            TokenBitWriterCheckpoint {
                token: &writer_token,
                written_bits: 0,
                output_bytes: 0,
            },
            &mut scratch,
        );
    }

    let alpha_palette_delta = (0..256)
        .map(|index| {
            let value = (index as u32).wrapping_mul(0x45d9_f3b);
            0xff00_0000 | (value & 0x00ff_ffff)
        })
        .collect::<Vec<_>>();
    let alpha_packed = (0..4_096)
        .map(|index| {
            let value = (index as u32).wrapping_mul(0x9e37_79b9);
            0xff00_0000 | (value & 0x00ff_ffff)
        })
        .collect::<Vec<_>>();
    let alpha = (0..4_096)
        .map(|index| (index as u8).wrapping_mul(37))
        .collect::<Vec<_>>();
    let alpha_probe_token = crate::CancellationToken::new();
    alpha_probe_token.cancel_after(usize::MAX);
    let mut alpha_probe_output = Vec::new();
    let mut alpha_probe_scratch = TokenStreamScratch::default();
    let _ = encode_alpha_stream(
        &alpha_palette_delta,
        alpha_palette_delta.len(),
        &alpha_packed,
        64,
        &alpha,
        &mut alpha_probe_output,
        &mut alpha_probe_scratch,
        Some(&alpha_probe_token),
        NoopBitWriterCheckpoint::default(),
    );
    let alpha_checks = usize::MAX.saturating_sub(
        alpha_probe_token
            .coverage_remaining_checks()
            .expect("coverage token must retain its remaining checks"),
    );
    for checks in 0..=alpha_checks {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut output = Vec::new();
        let mut scratch = TokenStreamScratch::default();
        let _ = encode_alpha_stream(
            &alpha_palette_delta,
            alpha_palette_delta.len(),
            &alpha_packed,
            64,
            &alpha,
            &mut output,
            &mut scratch,
            Some(&token),
            NoopBitWriterCheckpoint::default(),
        );
    }

    let rgba = (0..(128 * 128))
        .flat_map(|index| {
            let value = (index as u32).wrapping_mul(0x9e37_79b9);
            [
                value as u8,
                (value >> 8) as u8,
                (value >> 16) as u8,
                (value >> 24) as u8,
            ]
        })
        .collect::<Vec<_>>();
    let encode_probe_token = crate::CancellationToken::new();
    encode_probe_token.cancel_after(usize::MAX);
    let mut encode_probe = WebPEncoder::new();
    let _ =
        encode_probe.encode_with_token(&rgba, 64, 64, ColorType::Rgba8, Some(&encode_probe_token));
    let encode_checks = usize::MAX.saturating_sub(
        encode_probe_token
            .coverage_remaining_checks()
            .expect("coverage token must retain its remaining checks"),
    );
    // The probe's final polls are the only ones relevant to the wrapper's
    // post-frame check and RIFF/VP8L copy. Keep this window narrow: sweeping
    // every earlier pixel checkpoint would repeat the complete encoder work.
    for checks in encode_checks.saturating_sub(12)..=encode_checks {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut encoder = WebPEncoder::new();
        let _ = encoder.encode_with_token(&rgba, 64, 64, ColorType::Rgba8, Some(&token));
    }
}

#[cfg(coverage)]
#[inline(never)]
fn coverage_exercise_remaining_encoder_errors() {
    let width = 128_usize;
    let height = 128_usize;
    let pixels = (0..width * height)
        .map(|index| {
            let x = index % width;
            let y = index / width;
            let tile = (x / 4) + (y / 4) * 32;
            let value = (tile as u32).wrapping_mul(0x1f3d_5b79);
            0xff00_0000 | (value & 0x00ff_ffff)
        })
        .collect::<Vec<_>>();
    let tokens = pixels
        .iter()
        .copied()
        .map(backward_refs::Token::Literal)
        .collect::<Vec<_>>();
    let config = TokenStreamConfig {
        write_meta_huffman_bit: true,
        cache_bits: 0,
        histogram_bits: 2,
        quality: 100,
    };

    COVERAGE_TOKEN_STREAM_HISTOGRAM_REMAINING.store(usize::MAX, Ordering::Relaxed);
    let mut bytes = Vec::new();
    let mut writer = BitWriter {
        writer: &mut bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut scratch = TokenStreamScratch::default();
    let _ = write_token_stream(
        &mut writer,
        &pixels,
        width,
        &tokens,
        config,
        &mut scratch,
        None,
    );

    COVERAGE_TOKEN_STREAM_CANCEL_AT_OPTIMIZE.store(1, Ordering::Relaxed);
    let optimize_error_token = crate::CancellationToken::new();
    let mut bytes = Vec::new();
    let mut writer = BitWriter {
        writer: &mut bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut scratch = TokenStreamScratch::default();
    let optimize_result = std::hint::black_box(write_token_stream(
        &mut writer,
        &pixels,
        width,
        &tokens,
        config,
        &mut scratch,
        Some(&optimize_error_token),
    ));
    assert!(optimize_result.is_err());
    assert_eq!(
        COVERAGE_TOKEN_STREAM_CANCEL_AT_OPTIMIZE.load(Ordering::Relaxed),
        0,
        "the multi-group token-stream probe did not reach optimize_sampling"
    );
    COVERAGE_TOKEN_STREAM_CANCEL_AT_OPTIMIZE.store(0, Ordering::Relaxed);

    let histogram_fail_after = usize::MAX
        .saturating_sub(COVERAGE_TOKEN_STREAM_HISTOGRAM_REMAINING.load(Ordering::Relaxed));
    let mut bytes = Vec::new();
    let mut writer = BitWriter {
        writer: &mut bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint {
            fail_after: histogram_fail_after,
        },
    };
    let mut scratch = TokenStreamScratch::default();
    let _ = write_token_stream(
        &mut writer,
        &pixels,
        width,
        &tokens,
        config,
        &mut scratch,
        None,
    );

    COVERAGE_TOKEN_STREAM_HISTOGRAM_REMAINING.store(usize::MAX, Ordering::Relaxed);
    let writer_probe_token = crate::CancellationToken::new();
    writer_probe_token.cancel_after(usize::MAX);
    let mut bytes = Vec::new();
    let mut writer = BitWriter {
        writer: &mut bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &writer_probe_token,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    let mut scratch = TokenStreamScratch::default();
    let _ = write_token_stream(
        &mut writer,
        &pixels,
        width,
        &tokens,
        config,
        &mut scratch,
        None,
    );
    let writer_fail_after = usize::MAX
        .saturating_sub(COVERAGE_TOKEN_STREAM_HISTOGRAM_REMAINING.load(Ordering::Relaxed));
    let writer_token = crate::CancellationToken::new();
    writer_token.cancel_after(writer_fail_after);
    let mut bytes = Vec::new();
    let mut writer = BitWriter {
        writer: &mut bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &writer_token,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    let mut scratch = TokenStreamScratch::default();
    let _ = write_token_stream(
        &mut writer,
        &pixels,
        width,
        &tokens,
        config,
        &mut scratch,
        None,
    );

    COVERAGE_TOKEN_STREAM_META_PIXEL_REMAINING.store(usize::MAX, Ordering::Relaxed);
    let metadata_probe_token = crate::CancellationToken::new();
    metadata_probe_token.cancel_after(usize::MAX);
    let mut bytes = Vec::new();
    let mut writer = BitWriter {
        writer: &mut bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut scratch = TokenStreamScratch::default();
    let _ = write_token_stream(
        &mut writer,
        &pixels,
        width,
        &tokens,
        config,
        &mut scratch,
        Some(&metadata_probe_token),
    );
    let metadata_fail_after = usize::MAX
        .saturating_sub(COVERAGE_TOKEN_STREAM_META_PIXEL_REMAINING.load(Ordering::Relaxed));
    let metadata_token = crate::CancellationToken::new();
    metadata_token.cancel_after(metadata_fail_after);
    let mut bytes = Vec::new();
    let mut writer = BitWriter {
        writer: &mut bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut scratch = TokenStreamScratch::default();
    let _ = write_token_stream(
        &mut writer,
        &pixels,
        width,
        &tokens,
        config,
        &mut scratch,
        Some(&metadata_token),
    );

    COVERAGE_IMAGE_STREAM_SUFFIX_REMAINING.store(usize::MAX, Ordering::Relaxed);
    let suffix_pixels = (0..width * height)
        .map(|index| {
            let value = (index as u32)
                .wrapping_mul(0x9e37_79b9)
                .rotate_left((index % 29) as u32);
            0xff00_0000 | (value & 0x00ff_ffff)
        })
        .collect::<Vec<_>>();
    let suffix_probe_token = crate::CancellationToken::new();
    suffix_probe_token.cancel_after(usize::MAX);
    let mut suffix_bytes = Vec::new();
    let mut suffix_writer = BitWriter {
        writer: &mut suffix_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut suffix_output_scratch = Vec::new();
    let mut suffix_token_scratch = TokenStreamScratch::default();
    let _ = write_image_stream_configured_with_scratch(
        &mut suffix_writer,
        &suffix_pixels,
        width,
        true,
        2,
        100,
        0,
        &mut suffix_output_scratch,
        &mut suffix_token_scratch,
        Some(&suffix_probe_token),
    );
    let suffix_checks =
        usize::MAX.saturating_sub(COVERAGE_IMAGE_STREAM_SUFFIX_REMAINING.load(Ordering::Relaxed));
    let suffix_token = crate::CancellationToken::new();
    suffix_token.cancel_after(suffix_checks);
    let mut suffix_bytes = Vec::new();
    let mut suffix_writer = BitWriter {
        writer: &mut suffix_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut suffix_output_scratch = Vec::new();
    let mut suffix_token_scratch = TokenStreamScratch::default();
    let _ = write_image_stream_configured_with_scratch(
        &mut suffix_writer,
        &suffix_pixels,
        width,
        true,
        2,
        100,
        0,
        &mut suffix_output_scratch,
        &mut suffix_token_scratch,
        Some(&suffix_token),
    );
    let copy_probe_token = crate::CancellationToken::new();
    copy_probe_token.cancel_after(usize::MAX);
    let mut copy_probe_bytes = Vec::new();
    let mut copy_probe_writer = BitWriter {
        writer: &mut copy_probe_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &copy_probe_token,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    let mut copy_probe_output_scratch = Vec::new();
    let mut copy_probe_token_scratch = TokenStreamScratch::default();
    let _ = write_image_stream_configured_with_scratch(
        &mut copy_probe_writer,
        &suffix_pixels,
        width,
        true,
        2,
        100,
        0,
        &mut copy_probe_output_scratch,
        &mut copy_probe_token_scratch,
        Some(&copy_probe_token),
    );
    let copy_checks = usize::MAX.saturating_sub(
        copy_probe_token
            .coverage_remaining_checks()
            .expect("coverage token must retain its remaining checks"),
    );
    for checks in copy_checks.saturating_sub(4)..=copy_checks.saturating_add(4) {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: TokenBitWriterCheckpoint {
                token: &token,
                written_bits: 0,
                output_bytes: 0,
            },
        };
        let mut output_scratch = Vec::new();
        let mut token_scratch = TokenStreamScratch::default();
        let _ = write_image_stream_configured_with_scratch(
            &mut writer,
            &suffix_pixels,
            width,
            true,
            2,
            100,
            0,
            &mut output_scratch,
            &mut token_scratch,
            Some(&token),
        );
    }

    let grayscale = vec![0xff40_4040; 32 * 32];
    let grayscale_token = crate::CancellationToken::new();
    grayscale_token.cancel_after(0);
    let mut grayscale_pixels = grayscale.clone();
    let mut scratch = ImageStreamScratch::default();
    let _ = encode_frame_stream(
        &mut grayscale_pixels,
        32,
        32,
        false,
        EntropyMode::Spatial,
        true,
        1,
        Vec::new(),
        Some(&grayscale_token),
        NoopBitWriterCheckpoint::default(),
        &mut scratch,
    );
    let grayscale_writer_token = crate::CancellationToken::new();
    let mut grayscale_pixels = grayscale;
    let mut scratch = ImageStreamScratch::default();
    let _ = encode_frame_stream(
        &mut grayscale_pixels,
        32,
        32,
        false,
        EntropyMode::Spatial,
        true,
        1,
        Vec::new(),
        Some(&grayscale_token),
        TokenBitWriterCheckpoint {
            token: &grayscale_writer_token,
            written_bits: 0,
            output_bytes: 0,
        },
        &mut scratch,
    );

    let frame_pixels = (0..16 * 16)
        .map(|index| {
            let value = index as u32;
            0xff00_0000 | ((value & 0xff) << 16) | (((value * 3) & 0xff) << 8) | value
        })
        .collect::<Vec<_>>();
    let run_noop = |mode: EntropyMode, fail_after: usize| {
        let mut pixels = frame_pixels.clone();
        let mut scratch = ImageStreamScratch::default();
        let _ = encode_frame_stream(
            &mut pixels,
            16,
            16,
            false,
            mode,
            false,
            1,
            Vec::new(),
            None,
            NoopBitWriterCheckpoint { fail_after },
            &mut scratch,
        );
    };
    let run_token = |mode: EntropyMode, checks: usize| {
        let writer_token = crate::CancellationToken::new();
        writer_token.cancel_after(checks);
        let mut pixels = frame_pixels.clone();
        let mut scratch = ImageStreamScratch::default();
        let _ = encode_frame_stream(
            &mut pixels,
            16,
            16,
            false,
            mode,
            false,
            1,
            Vec::new(),
            None,
            TokenBitWriterCheckpoint {
                token: &writer_token,
                written_bits: 0,
                output_bytes: 0,
            },
            &mut scratch,
        );
    };

    let frame_targets = [
        (
            &COVERAGE_FRAME_SUBTRACT_FIRST_REMAINING,
            EntropyMode::SubtractGreen,
        ),
        (
            &COVERAGE_FRAME_SUBTRACT_SECOND_REMAINING,
            EntropyMode::SubtractGreen,
        ),
        (
            &COVERAGE_FRAME_CROSS_FIRST_REMAINING,
            EntropyMode::SpatialSubtractGreen,
        ),
        (
            &COVERAGE_FRAME_CROSS_SECOND_REMAINING,
            EntropyMode::SpatialSubtractGreen,
        ),
        (
            &COVERAGE_FRAME_CROSS_THIRD_REMAINING,
            EntropyMode::SpatialSubtractGreen,
        ),
    ];
    for &(slot, mode) in &frame_targets {
        slot.store(usize::MAX, Ordering::Relaxed);
        run_noop(mode, usize::MAX);
        let fail_after = usize::MAX.saturating_sub(slot.load(Ordering::Relaxed));
        run_noop(mode, fail_after);
        slot.store(usize::MAX, Ordering::Relaxed);
        let probe_token = crate::CancellationToken::new();
        probe_token.cancel_after(usize::MAX);
        let mut pixels = frame_pixels.clone();
        let mut scratch = ImageStreamScratch::default();
        let _ = encode_frame_stream(
            &mut pixels,
            16,
            16,
            false,
            mode,
            false,
            1,
            Vec::new(),
            None,
            TokenBitWriterCheckpoint {
                token: &probe_token,
                written_bits: 0,
                output_bytes: 0,
            },
            &mut scratch,
        );
        let checks = usize::MAX.saturating_sub(slot.load(Ordering::Relaxed));
        run_token(mode, checks);
    }

    let alpha_palette = (0..256)
        .map(|index| {
            let value = (index as u32).wrapping_mul(0x45d9_f3b);
            0xff00_0000 | (value & 0x00ff_ffff)
        })
        .collect::<Vec<_>>();
    let packed = (0..4_096)
        .map(|index| {
            let value = (index as u32).wrapping_mul(0x9e37_79b9);
            0xff00_0000 | (value & 0x00ff_ffff)
        })
        .collect::<Vec<_>>();
    let alpha = (0..4_096)
        .map(|index| (index as u8).wrapping_mul(37))
        .collect::<Vec<_>>();
    let alpha_probe_token = crate::CancellationToken::new();
    alpha_probe_token.cancel_after(usize::MAX);
    COVERAGE_ALPHA_COMPRESSED_REMAINING.store(usize::MAX, Ordering::Relaxed);
    COVERAGE_ALPHA_UNCOMPRESSED_REMAINING.store(usize::MAX, Ordering::Relaxed);
    let mut output = Vec::new();
    let mut scratch = TokenStreamScratch::default();
    let _ = encode_alpha_stream(
        &alpha_palette,
        alpha_palette.len(),
        &packed,
        64,
        &alpha,
        &mut output,
        &mut scratch,
        Some(&alpha_probe_token),
        NoopBitWriterCheckpoint::default(),
    );
    let compressed_checks =
        usize::MAX.saturating_sub(COVERAGE_ALPHA_COMPRESSED_REMAINING.load(Ordering::Relaxed));
    let compressed_token = crate::CancellationToken::new();
    compressed_token.cancel_after(compressed_checks);
    let mut output = Vec::new();
    let mut scratch = TokenStreamScratch::default();
    let _ = encode_alpha_stream(
        &alpha_palette,
        alpha_palette.len(),
        &packed,
        64,
        &alpha,
        &mut output,
        &mut scratch,
        Some(&compressed_token),
        NoopBitWriterCheckpoint::default(),
    );
    let uncompressed_checks =
        usize::MAX.saturating_sub(COVERAGE_ALPHA_UNCOMPRESSED_REMAINING.load(Ordering::Relaxed));
    let uncompressed_token = crate::CancellationToken::new();
    uncompressed_token.cancel_after(uncompressed_checks);
    let mut output = Vec::new();
    let mut scratch = TokenStreamScratch::default();
    let _ = encode_alpha_stream(
        &alpha_palette,
        alpha_palette.len(),
        &packed,
        64,
        &alpha,
        &mut output,
        &mut scratch,
        Some(&uncompressed_token),
        NoopBitWriterCheckpoint::default(),
    );

    let rgba = (0..(128 * 128))
        .flat_map(|index| {
            let value = (index as u32).wrapping_mul(0x9e37_79b9);
            [
                value as u8,
                (value >> 8) as u8,
                (value >> 16) as u8,
                (value >> 24) as u8,
            ]
        })
        .collect::<Vec<_>>();
    COVERAGE_ENCODER_FRAME_REMAINING.store(usize::MAX, Ordering::Relaxed);
    COVERAGE_ENCODER_CHUNK_REMAINING.store(usize::MAX, Ordering::Relaxed);
    COVERAGE_ENCODER_FINAL_REMAINING.store(usize::MAX, Ordering::Relaxed);
    let probe_token = crate::CancellationToken::new();
    probe_token.cancel_after(usize::MAX);
    let mut encoder = WebPEncoder::new();
    let _ = encoder.encode_with_token(&rgba, 128, 128, ColorType::Rgba8, Some(&probe_token));
    let frame_checks =
        usize::MAX.saturating_sub(COVERAGE_ENCODER_FRAME_REMAINING.load(Ordering::Relaxed));
    let frame_token = crate::CancellationToken::new();
    frame_token.cancel_after(frame_checks);
    let mut encoder = WebPEncoder::new();
    let _ = encoder.encode_with_token(&rgba, 128, 128, ColorType::Rgba8, Some(&frame_token));
    let chunk_checks =
        usize::MAX.saturating_sub(COVERAGE_ENCODER_CHUNK_REMAINING.load(Ordering::Relaxed));
    let chunk_token = crate::CancellationToken::new();
    chunk_token.cancel_after(chunk_checks);
    let mut encoder = WebPEncoder::new();
    let _ = encoder.encode_with_token(&rgba, 128, 128, ColorType::Rgba8, Some(&chunk_token));
    let final_checks =
        usize::MAX.saturating_sub(COVERAGE_ENCODER_FINAL_REMAINING.load(Ordering::Relaxed));
    let final_token = crate::CancellationToken::new();
    final_token.cancel_after(final_checks);
    let mut encoder = WebPEncoder::new();
    let _ = encoder.encode_with_token(&rgba, 128, 128, ColorType::Rgba8, Some(&final_token));
}

#[cfg(coverage)]
#[inline(never)]
pub(crate) fn __coverage_exercise_instrumented_paths() {
    __coverage_exercise_instrumented_generic_paths();
    backward_refs::__coverage_exercise_instrumented_trace_paths();
    histogram::__coverage_exercise_instrumented_checkpoint_errors();
}

#[cfg(coverage)]
#[coverage(off)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
pub(crate) fn __coverage_exercise_private_branches() {
    backward_refs::__coverage_exercise_private_branches();
    cross_color::__coverage_exercise_private_branches();
    histogram::__coverage_exercise_private_branches();
    predictor::__coverage_exercise_private_branches();

    let _ = length_to_symbol(4);
    let _ = length_to_symbol(300);
    let _ = channels(0x1122_3344);
    let _ = chunk_size(3);
    let _ = chunk_size(4);
    let mut compressed_tokens = Vec::new();
    compressed_huffman_tokens_into(&[0; 300], &mut compressed_tokens);
    let coverage_token = crate::CancellationToken::new();
    let cancelled_token = crate::CancellationToken::new();
    cancelled_token.cancel();
    assert!(matches!(
        check_token(Some(&cancelled_token)),
        Err(EncodingError::Cancelled)
    ));
    let mut checkpoint = TokenBitWriterCheckpoint {
        token: &coverage_token,
        written_bits: 0,
        output_bytes: 0,
    };
    let _ = checkpoint.checkpoint_bits(VP8L_2097152_BITSTREAM_CHECKPOINT_BITS);
    let _ = checkpoint.checkpoint_output_bytes(VP8L_OUTPUT_CHECKPOINT_BYTES);

    // The nested bit intervals are ordinary success-path structure above;
    // measure one boundary at a time to drive each nested cancellation arm
    // without replaying a two-million-bit stream for every schedule.
    for threshold in [
        VP8L_2048_BITSTREAM_CHECKPOINT_BITS,
        VP8L_4096_BITSTREAM_CHECKPOINT_BITS,
        VP8L_8192_BITSTREAM_CHECKPOINT_BITS,
        VP8L_16384_BITSTREAM_CHECKPOINT_BITS,
        VP8L_32768_BITSTREAM_CHECKPOINT_BITS,
        VP8L_65536_BITSTREAM_CHECKPOINT_BITS,
        VP8L_131072_BITSTREAM_CHECKPOINT_BITS,
        VP8L_262144_BITSTREAM_CHECKPOINT_BITS,
        VP8L_524288_BITSTREAM_CHECKPOINT_BITS,
        VP8L_1048576_BITSTREAM_CHECKPOINT_BITS,
        VP8L_2097152_BITSTREAM_CHECKPOINT_BITS,
    ] {
        let probe_token = crate::CancellationToken::new();
        probe_token.cancel_after(usize::MAX);
        let mut probe = TokenBitWriterCheckpoint {
            token: &probe_token,
            written_bits: threshold.saturating_sub(VP8L_8_BITSTREAM_CHECKPOINT_BITS),
            output_bytes: 0,
        };
        let _ = probe.checkpoint_bits(VP8L_8_BITSTREAM_CHECKPOINT_BITS);
        let calls = usize::MAX.saturating_sub(
            probe_token
                .coverage_remaining_checks()
                .unwrap_or(usize::MAX),
        );
        for checks in 0..=calls {
            let token = crate::CancellationToken::new();
            token.cancel_after(checks);
            let mut checkpoint = TokenBitWriterCheckpoint {
                token: &token,
                written_bits: threshold.saturating_sub(VP8L_8_BITSTREAM_CHECKPOINT_BITS),
                output_bytes: 0,
            };
            let _ = checkpoint.checkpoint_bits(VP8L_8_BITSTREAM_CHECKPOINT_BITS);
        }
    }
    let output_probe_token = crate::CancellationToken::new();
    output_probe_token.cancel_after(usize::MAX);
    let mut output_probe = TokenBitWriterCheckpoint {
        token: &output_probe_token,
        written_bits: 0,
        output_bytes: 0,
    };
    let _ = output_probe.checkpoint_output_bytes(VP8L_OUTPUT_CHECKPOINT_BYTES);
    let output_calls = usize::MAX.saturating_sub(
        output_probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=output_calls {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut checkpoint = TokenBitWriterCheckpoint {
            token: &token,
            written_bits: 0,
            output_bytes: 0,
        };
        let _ = checkpoint.checkpoint_output_bytes(VP8L_OUTPUT_CHECKPOINT_BYTES);
    }
    let mut token_rle_lengths = Vec::new();
    for &(value, count) in &[
        (0_u8, 2_usize),
        (1, 1),
        (0, 5),
        (2, 4),
        (0, 20),
        (3, 8),
        (0, 150),
        (4, 2),
        (5, 8),
    ] {
        for _ in 0..count {
            token_rle_lengths.push(value);
        }
    }
    let mut token_rle_tokens = Vec::new();
    let _ = compressed_huffman_tokens_with_checkpoint(
        &token_rle_lengths,
        &mut token_rle_tokens,
        Some(&coverage_token),
    );
    let rle_probe_token = crate::CancellationToken::new();
    rle_probe_token.cancel_after(usize::MAX);
    let mut rle_probe_tokens = Vec::new();
    let _ = compressed_huffman_tokens_with_checkpoint(
        &token_rle_lengths,
        &mut rle_probe_tokens,
        Some(&rle_probe_token),
    );
    let rle_calls = usize::MAX.saturating_sub(
        rle_probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=rle_calls {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut tokens = Vec::new();
        let _ = std::hint::black_box(compressed_huffman_tokens_with_checkpoint(
            &token_rle_lengths,
            &mut tokens,
            Some(&token),
        ));
        std::hint::black_box(tokens.len());
    }
    for (prefix_len, value, repetitions) in [
        (15_usize, 0_u8, 2_usize),
        (15, 0, 3),
        (15, 0, 11),
        (15, 0, 139),
        (14, 15, 4),
        (14, 15, 8),
    ] {
        let mut lengths = (1..=prefix_len as u8).collect::<Vec<_>>();
        lengths.extend(std::iter::repeat_n(value, repetitions));
        let probe_token = crate::CancellationToken::new();
        probe_token.cancel_after(usize::MAX);
        let mut probe_tokens = Vec::new();
        let _ = compressed_huffman_tokens_with_checkpoint(
            &lengths,
            &mut probe_tokens,
            Some(&probe_token),
        );
        let calls = usize::MAX.saturating_sub(
            probe_token
                .coverage_remaining_checks()
                .unwrap_or(usize::MAX),
        );
        for checks in 0..=calls {
            let token = crate::CancellationToken::new();
            token.cancel_after(checks);
            let mut tokens = Vec::new();
            let _ = compressed_huffman_tokens_with_checkpoint(&lengths, &mut tokens, Some(&token));
        }
    }
    let mut odd_chunk = Vec::new();
    let _ = write_chunk(&mut odd_chunk, b"ODD!", &[1, 2, 3], None);
    let mut even_chunk = Vec::new();
    let _ = write_chunk(&mut even_chunk, b"EVEN", &[1, 2, 3, 4], None);

    let mut tree_bytes = Vec::new();
    let mut tree_writer = BitWriter {
        writer: &mut tree_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut lengths = vec![0; 4];
    let mut codes = vec![0; 4];
    let mut huffman_tokens = Vec::new();
    let mut optimized_frequencies = Vec::new();
    let mut huffman_rle_good = Vec::new();
    let mut huffman_nodes = Vec::new();
    let mut huffman_node_sort_scratch = Vec::new();
    let mut huffman_node_arena = Vec::new();
    let mut huffman_scratch = HuffmanTreeScratch {
        huffman_tokens: &mut huffman_tokens,
        optimized_frequencies: &mut optimized_frequencies,
        huffman_rle_good: &mut huffman_rle_good,
        nodes: &mut huffman_nodes,
        node_sort_scratch: &mut huffman_node_sort_scratch,
        node_arena: &mut huffman_node_arena,
    };
    let _ = write_huffman_tree(
        &mut tree_writer,
        &[1, 0, 0, 0],
        &mut lengths,
        &mut codes,
        &mut huffman_scratch,
        None,
    );
    let _ = tree_writer.flush();

    let compact_token = crate::CancellationToken::new();
    let mut compact_tree_bytes = Vec::new();
    let mut compact_tree_writer = BitWriter {
        writer: &mut compact_tree_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &compact_token,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    let mut compact_lengths = vec![0; 256];
    let mut compact_codes = vec![0; 256];
    let mut compact_frequencies = vec![0; 256];
    compact_frequencies[2] = 1;
    compact_frequencies[3] = 1;
    let _ = write_huffman_tree(
        &mut compact_tree_writer,
        &compact_frequencies,
        &mut compact_lengths,
        &mut compact_codes,
        &mut huffman_scratch,
        Some(&compact_token),
    );
    let _ = compact_tree_writer.flush();

    let no_cancel_huffman_token = crate::CancellationToken::new();
    let single_failure_token = crate::CancellationToken::new();
    single_failure_token.cancel_after(0);
    let mut single_failure_bytes = Vec::new();
    let mut single_failure_writer = BitWriter {
        writer: &mut single_failure_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &single_failure_token,
            written_bits: 5,
            output_bytes: 0,
        },
    };
    let mut single_failure_frequencies = vec![0; 256];
    single_failure_frequencies[128] = 1;
    let mut single_failure_lengths = vec![0; 256];
    let mut single_failure_codes = vec![0; 256];
    let single_failure_result = std::hint::black_box(write_huffman_tree(
        &mut single_failure_writer,
        &single_failure_frequencies,
        &mut single_failure_lengths,
        &mut single_failure_codes,
        &mut huffman_scratch,
        Some(&no_cancel_huffman_token),
    ));
    assert!(single_failure_result.is_err());

    for written_bits in [7, 6] {
        let token = crate::CancellationToken::new();
        token.cancel_after(0);
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: TokenBitWriterCheckpoint {
                token: &token,
                written_bits,
                output_bytes: 0,
            },
        };
        let mut pixels = vec![0xff00_0000];
        let mut scratch = ImageStreamScratch::default();
        let _ = apply_palette(
            &mut writer,
            &mut pixels,
            1,
            1,
            vec![0xff00_0000],
            &mut scratch,
            None,
        );
    }

    let huffman_probe_token = crate::CancellationToken::new();
    huffman_probe_token.cancel_after(usize::MAX);
    let mut huffman_probe_counts = (0..128)
        .map(|index| (index % 3 + 1) as u32)
        .collect::<Vec<_>>();
    let mut huffman_probe_good = Vec::new();
    let _ = optimize_huffman_for_rle_with_checkpoint(
        &mut huffman_probe_counts,
        &mut huffman_probe_good,
        Some(&huffman_probe_token),
    );
    let huffman_calls = usize::MAX.saturating_sub(
        huffman_probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=huffman_calls {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut counts = (0..128)
            .map(|index| (index % 3 + 1) as u32)
            .collect::<Vec<_>>();
        let mut good = Vec::new();
        let _ = optimize_huffman_for_rle_with_checkpoint(&mut counts, &mut good, Some(&token));
    }

    let mut token_tree_bytes = Vec::new();
    let mut token_tree_writer = BitWriter {
        writer: &mut token_tree_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &coverage_token,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    let mut token_tree_lengths = vec![0; 256];
    let mut token_tree_codes = vec![0; 256];
    let mut token_tree_frequencies = vec![0; 256];
    token_tree_frequencies[0] = 3;
    token_tree_frequencies[1] = 1;
    token_tree_frequencies[128] = 1;
    token_tree_frequencies[255] = 7;
    std::hint::black_box(
        write_huffman_tree(
            &mut token_tree_writer,
            &token_tree_frequencies,
            &mut token_tree_lengths,
            &mut token_tree_codes,
            &mut huffman_scratch,
            Some(&coverage_token),
        )
        .expect("token-aware huffman tree coverage input must encode"),
    );
    let _ = token_tree_writer.flush();
    std::hint::black_box(&token_tree_bytes);

    let mut dense_tree_bytes = Vec::new();
    let mut dense_tree_writer = BitWriter {
        writer: &mut dense_tree_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &coverage_token,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    let mut dense_tree_lengths = vec![0; 256];
    let mut dense_tree_codes = vec![0; 256];
    let dense_tree_frequencies = (0..256)
        .map(|index| ((index * 37) % 251 + 1) as u32)
        .collect::<Vec<_>>();
    std::hint::black_box(
        write_huffman_tree(
            &mut dense_tree_writer,
            &dense_tree_frequencies,
            &mut dense_tree_lengths,
            &mut dense_tree_codes,
            &mut huffman_scratch,
            Some(&coverage_token),
        )
        .expect("dense token-aware huffman tree coverage input must encode"),
    );
    let _ = dense_tree_writer.flush();
    std::hint::black_box(&dense_tree_bytes);

    let mut trimmed_tree_bytes = Vec::new();
    let mut trimmed_tree_writer = BitWriter {
        writer: &mut trimmed_tree_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut trimmed_lengths = vec![0; 256];
    let mut trimmed_codes = vec![0; 256];
    let mut trimmed_frequencies = vec![0; 256];
    trimmed_frequencies[..4].fill(1);
    let _ = write_huffman_tree(
        &mut trimmed_tree_writer,
        &trimmed_frequencies,
        &mut trimmed_lengths,
        &mut trimmed_codes,
        &mut huffman_scratch,
        None,
    );
    let _ = trimmed_tree_writer.flush();

    let mut token_trimmed_tree_bytes = Vec::new();
    let mut token_trimmed_tree_writer = BitWriter {
        writer: &mut token_trimmed_tree_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &coverage_token,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    let mut token_trimmed_lengths = vec![0; 256];
    let mut token_trimmed_codes = vec![0; 256];
    std::hint::black_box(
        write_huffman_tree(
            &mut token_trimmed_tree_writer,
            &trimmed_frequencies,
            &mut token_trimmed_lengths,
            &mut token_trimmed_codes,
            &mut huffman_scratch,
            Some(&coverage_token),
        )
        .expect("trimmed token-aware huffman tree coverage input must encode"),
    );
    let _ = token_trimmed_tree_writer.flush();
    std::hint::black_box(&token_trimmed_tree_bytes);

    for (written_bits, checks) in [(7, 7), (6, 6)] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: TokenBitWriterCheckpoint {
                token: &token,
                written_bits,
                output_bytes: 0,
            },
        };
        let mut lengths = vec![0; 256];
        let mut codes = vec![0; 256];
        let _ = std::hint::black_box(write_huffman_tree(
            &mut writer,
            &trimmed_frequencies,
            &mut lengths,
            &mut codes,
            &mut huffman_scratch,
            Some(&no_cancel_huffman_token),
        ));
    }

    let mut ordinary_token_tree_bytes = Vec::new();
    let mut ordinary_token_tree_writer = BitWriter {
        writer: &mut ordinary_token_tree_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &coverage_token,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    let mut ordinary_token_lengths = vec![0; 256];
    let mut ordinary_token_codes = vec![0; 256];
    std::hint::black_box(
        write_huffman_tree(
            &mut ordinary_token_tree_writer,
            &trimmed_frequencies,
            &mut ordinary_token_lengths,
            &mut ordinary_token_codes,
            &mut huffman_scratch,
            None,
        )
        .expect("ordinary token-writer huffman tree coverage input must encode"),
    );
    let _ = ordinary_token_tree_writer.flush();

    let mut single_token_tree_frequencies = vec![0; 256];
    single_token_tree_frequencies[128] = 1;
    let mut single_token_tree_lengths = vec![0; 256];
    let mut single_token_tree_codes = vec![0; 256];
    std::hint::black_box(
        write_huffman_tree(
            &mut ordinary_token_tree_writer,
            &single_token_tree_frequencies,
            &mut single_token_tree_lengths,
            &mut single_token_tree_codes,
            &mut huffman_scratch,
            None,
        )
        .expect("single-symbol token-writer huffman tree coverage input must encode"),
    );
    let _ = ordinary_token_tree_writer.flush();
    std::hint::black_box(&ordinary_token_tree_bytes);

    let mut dense_success_bytes = Vec::new();
    let mut dense_success_writer = BitWriter {
        writer: &mut dense_success_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut dense_success_lengths = vec![0; 256];
    let mut dense_success_codes = vec![0; 256];
    std::hint::black_box(
        write_huffman_tree(
            &mut dense_success_writer,
            &dense_tree_frequencies,
            &mut dense_success_lengths,
            &mut dense_success_codes,
            &mut huffman_scratch,
            None,
        )
        .expect("dense no-op Huffman coverage input must encode"),
    );
    let _ = std::hint::black_box(dense_success_writer.flush());

    let populations = [
        vec![1; 281],
        vec![1; 256],
        vec![1; 256],
        vec![1; 256],
        vec![1; 40],
    ];
    let mut group_bytes = Vec::new();
    let mut group_writer = BitWriter {
        writer: &mut group_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut group = GroupCodes::default();
    let _ = write_group(
        &mut group_writer,
        &populations,
        &mut group,
        &mut huffman_scratch,
        None,
    );
    let _ = group_writer.flush();

    let mut row_mismatch = vec![0_u16; 2 * 2_048];
    row_mismatch[2_048 + 1_024] = 1;
    let _ = optimize_sampling(&mut row_mismatch, 2_048, 2, 0, 1, Some(&coverage_token));

    let mut column_mismatch = vec![0_u16; 2 * 32_768];
    column_mismatch[1_367] = 1;
    column_mismatch[32_768 + 1_367] = 1;
    let _ = optimize_sampling(&mut column_mismatch, 32_768, 2, 0, 2, Some(&coverage_token));

    let sampling_probe_token = crate::CancellationToken::new();
    sampling_probe_token.cancel_after(usize::MAX);
    let mut sampling_probe_symbols = vec![0_u16; 2 * 2_048];
    let _ = optimize_sampling(
        &mut sampling_probe_symbols,
        2_048,
        2,
        0,
        1,
        Some(&sampling_probe_token),
    );
    let sampling_calls = usize::MAX.saturating_sub(
        sampling_probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=sampling_calls {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut symbols = vec![0_u16; 2 * 2_048];
        let _ = optimize_sampling(&mut symbols, 2_048, 2, 0, 1, Some(&token));
    }

    let mut sampled_copy = vec![0_u16; 128 * 128];
    for y in 0..128 {
        for x in 0..128 {
            sampled_copy[y * 128 + x] = ((x / 2) % 16) as u16;
        }
    }
    let _ = optimize_sampling(&mut sampled_copy, 128, 128, 0, 1, Some(&coverage_token));

    let mut token_bytes = Vec::new();
    let mut token_writer = BitWriter {
        writer: &mut token_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut token_scratch = TokenStreamScratch::default();
    let _ = write_token_stream(
        &mut token_writer,
        &[0xff00_0000; 8],
        8,
        &[
            backward_refs::Token::Literal(0xff00_0000),
            backward_refs::Token::Copy {
                distance: 1,
                length: 4,
            },
            backward_refs::Token::Literal(0xff00_0000),
            backward_refs::Token::Literal(0xff00_0000),
            backward_refs::Token::Literal(0xff00_0000),
        ],
        TokenStreamConfig {
            write_meta_huffman_bit: false,
            cache_bits: 0,
            histogram_bits: 3,
            quality: 1,
        },
        &mut token_scratch,
        None,
    );
    let _ = token_writer.flush();

    let no_op_tokens = [
        backward_refs::Token::Literal(0xff00_0000),
        backward_refs::Token::Copy {
            distance: 1,
            length: 4,
        },
        backward_refs::Token::Literal(0xff00_0001),
        backward_refs::Token::Literal(0xff00_0002),
    ];
    for fail_after in [0, 1, 2, 64, 512, 2_048] {
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: NoopBitWriterCheckpoint { fail_after },
        };
        let mut scratch = TokenStreamScratch::default();
        let _ = std::hint::black_box(write_token_stream(
            &mut writer,
            &[0xff00_0000; 8],
            8,
            &no_op_tokens,
            TokenStreamConfig {
                write_meta_huffman_bit: false,
                cache_bits: 0,
                histogram_bits: 3,
                quality: 1,
            },
            &mut scratch,
            None,
        ));
    }
    let mut token_reference_probe_bytes = Vec::new();
    let mut token_reference_probe_writer = BitWriter {
        writer: &mut token_reference_probe_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut token_reference_probe_scratch = TokenStreamScratch::default();
    let _ = write_token_stream(
        &mut token_reference_probe_writer,
        &[0xff00_0000; 8],
        8,
        &no_op_tokens,
        TokenStreamConfig {
            write_meta_huffman_bit: false,
            cache_bits: 0,
            histogram_bits: 3,
            quality: 1,
        },
        &mut token_reference_probe_scratch,
        Some(&coverage_token),
    );
    let token_reference_write_calls =
        usize::MAX.saturating_sub(token_reference_probe_writer.checkpoint.fail_after);
    for fail_after in token_reference_write_calls.saturating_sub(1)..=token_reference_write_calls {
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: NoopBitWriterCheckpoint { fail_after },
        };
        let mut scratch = TokenStreamScratch::default();
        let _ = std::hint::black_box(write_token_stream(
            &mut writer,
            &[0xff00_0000; 8],
            8,
            &no_op_tokens,
            TokenStreamConfig {
                write_meta_huffman_bit: false,
                cache_bits: 0,
                histogram_bits: 3,
                quality: 1,
            },
            &mut scratch,
            Some(&coverage_token),
        ));
    }
    let meta_width = 128_usize;
    let meta_height = 128_usize;
    let mut meta_pixels = Vec::with_capacity(meta_width * meta_height);
    let mut meta_tokens = Vec::with_capacity(meta_width * meta_height);
    for index in 0..meta_width * meta_height {
        let x = index % meta_width;
        let y = index / meta_width;
        let tile = (x / 8) + (y / 8) * 16;
        let pixel = 0xff00_0000
            | ((tile as u32) << 16)
            | (((255 - tile) as u32) << 8)
            | (tile as u32 ^ 0x55);
        meta_pixels.push(pixel);
        meta_tokens.push(backward_refs::Token::Literal(pixel));
    }
    for fail_after in [0, 1, 32, 128, 512] {
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: NoopBitWriterCheckpoint { fail_after },
        };
        let mut scratch = TokenStreamScratch::default();
        let _ = std::hint::black_box(write_token_stream(
            &mut writer,
            &meta_pixels,
            meta_width,
            &meta_tokens,
            TokenStreamConfig {
                write_meta_huffman_bit: true,
                cache_bits: 1,
                histogram_bits: 3,
                quality: 0,
            },
            &mut scratch,
            None,
        ));
    }
    let mut meta_bytes = Vec::new();
    let mut meta_writer = BitWriter {
        writer: &mut meta_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &coverage_token,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    let mut meta_scratch = TokenStreamScratch::default();
    write_token_stream(
        &mut meta_writer,
        &meta_pixels,
        meta_width,
        &meta_tokens,
        TokenStreamConfig {
            write_meta_huffman_bit: true,
            cache_bits: 1,
            histogram_bits: 3,
            quality: 0,
        },
        &mut meta_scratch,
        Some(&coverage_token),
    )
    .expect("token-aware metadata stream coverage input must encode");
    let _ = meta_writer.flush();

    let wide_meta_width = 33_usize;
    let wide_meta_height = 33_usize;
    let wide_meta_pixels = (0..wide_meta_width * wide_meta_height)
        .map(|index| {
            let value = (index as u32).wrapping_mul(0x45d9_f3b);
            0xff00_0000 | (value & 0x00ff_ffff)
        })
        .collect::<Vec<_>>();
    let wide_meta_tokens = wide_meta_pixels
        .iter()
        .copied()
        .map(backward_refs::Token::Literal)
        .collect::<Vec<_>>();
    let mut wide_meta_bytes = Vec::new();
    let mut wide_meta_writer = BitWriter {
        writer: &mut wide_meta_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &coverage_token,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    let mut wide_meta_scratch = TokenStreamScratch::default();
    write_token_stream(
        &mut wide_meta_writer,
        &wide_meta_pixels,
        wide_meta_width,
        &wide_meta_tokens,
        TokenStreamConfig {
            write_meta_huffman_bit: true,
            cache_bits: 0,
            histogram_bits: 0,
            quality: 100,
        },
        &mut wide_meta_scratch,
        Some(&coverage_token),
    )
    .expect("wide token-aware metadata stream coverage input must encode");
    let _ = wide_meta_writer.flush();

    let mut wide_meta_noop_bytes = Vec::new();
    let mut wide_meta_noop_writer = BitWriter {
        writer: &mut wide_meta_noop_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut wide_meta_noop_scratch = TokenStreamScratch::default();
    let _ = std::hint::black_box(write_token_stream(
        &mut wide_meta_noop_writer,
        &wide_meta_pixels,
        wide_meta_width,
        &wide_meta_tokens,
        TokenStreamConfig {
            write_meta_huffman_bit: true,
            cache_bits: 0,
            histogram_bits: 0,
            quality: 100,
        },
        &mut wide_meta_noop_scratch,
        Some(&coverage_token),
    ));
    let _ = wide_meta_noop_writer.flush();

    // Sweep token polls on a tiny, deliberately diverse multi-group metadata
    // stream. A no-op outer writer leaves cancellation available to the
    // nested image stream, whose error returns through the caller's `?` edge.
    let nested_width = 8;
    let nested_pixels = vec![
        0xff00_0000,
        0xff00_0000,
        0xff00_0000,
        0xff00_0000,
        0xff00_00ff,
        0xff00_00ff,
        0xff00_00ff,
        0xff00_00ff,
    ];
    let nested_tokens = nested_pixels
        .iter()
        .copied()
        .map(backward_refs::Token::Literal)
        .collect::<Vec<_>>();
    for checks in [0, 1, 32, 128, 512] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: NoopBitWriterCheckpoint::default(),
        };
        let mut scratch = TokenStreamScratch::default();
        let _ = write_token_stream(
            &mut writer,
            &nested_pixels,
            nested_width,
            &nested_tokens,
            TokenStreamConfig {
                write_meta_huffman_bit: true,
                cache_bits: 0,
                histogram_bits: 2,
                quality: 0,
            },
            &mut scratch,
            Some(&token),
        );
    }

    for checks in [0, 1, 32, 256, 1_024, 2_047] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: TokenBitWriterCheckpoint {
                token: &token,
                written_bits: 0,
                output_bytes: 0,
            },
        };
        let mut scratch = TokenStreamScratch::default();
        let _ = write_token_stream(
            &mut writer,
            &wide_meta_pixels,
            wide_meta_width,
            &wide_meta_tokens,
            TokenStreamConfig {
                write_meta_huffman_bit: true,
                cache_bits: 0,
                histogram_bits: 0,
                quality: 100,
            },
            &mut scratch,
            Some(&token),
        );
    }

    // Use a no-op bit-writer checkpoint so cancellation can reach the nested
    // metadata stream itself instead of being consumed by the outer bit
    // writer first.
    for checks in [0, 1, 32, 256, 1_024, 4_096] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: NoopBitWriterCheckpoint::default(),
        };
        let mut scratch = TokenStreamScratch::default();
        let _ = write_token_stream(
            &mut writer,
            &wide_meta_pixels,
            wide_meta_width,
            &wide_meta_tokens,
            TokenStreamConfig {
                write_meta_huffman_bit: true,
                cache_bits: 0,
                histogram_bits: 0,
                quality: 100,
            },
            &mut scratch,
            Some(&token),
        );
    }

    let medium_meta_width = 11_usize;
    let medium_meta_height = 10_usize;
    let medium_meta_pixels = (0..medium_meta_width * medium_meta_height)
        .map(|index| {
            let value = (index as u32).wrapping_mul(0x9e37_79b9);
            0xff00_0000 | (value & 0x00ff_ffff)
        })
        .collect::<Vec<_>>();
    let medium_meta_tokens = medium_meta_pixels
        .iter()
        .copied()
        .map(backward_refs::Token::Literal)
        .collect::<Vec<_>>();
    for checks in [0, 1, 32, 256, 1_024, 4_095] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: TokenBitWriterCheckpoint {
                token: &token,
                written_bits: 0,
                output_bytes: 0,
            },
        };
        let mut scratch = TokenStreamScratch::default();
        let _ = write_token_stream(
            &mut writer,
            &medium_meta_pixels,
            medium_meta_width,
            &medium_meta_tokens,
            TokenStreamConfig {
                write_meta_huffman_bit: true,
                cache_bits: 0,
                histogram_bits: 0,
                quality: 100,
            },
            &mut scratch,
            Some(&token),
        );
    }

    let mut ordinary_meta_bytes = Vec::new();
    let mut ordinary_meta_writer = BitWriter {
        writer: &mut ordinary_meta_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut ordinary_meta_scratch = TokenStreamScratch::default();
    let _ = write_token_stream(
        &mut ordinary_meta_writer,
        &meta_pixels,
        meta_width,
        &meta_tokens,
        TokenStreamConfig {
            write_meta_huffman_bit: true,
            cache_bits: 1,
            histogram_bits: 3,
            quality: 0,
        },
        &mut ordinary_meta_scratch,
        None,
    );
    let _ = ordinary_meta_writer.flush();

    let mut token_meta_noop_bytes = Vec::new();
    let mut token_meta_noop_writer = BitWriter {
        writer: &mut token_meta_noop_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut token_meta_noop_scratch = TokenStreamScratch::default();
    let _ = std::hint::black_box(write_token_stream(
        &mut token_meta_noop_writer,
        &meta_pixels,
        meta_width,
        &meta_tokens,
        TokenStreamConfig {
            write_meta_huffman_bit: true,
            cache_bits: 1,
            histogram_bits: 3,
            quality: 0,
        },
        &mut token_meta_noop_scratch,
        Some(&coverage_token),
    ));
    let _ = token_meta_noop_writer.flush();

    // Keep the writer-checkpoint type while omitting the outer cancellation
    // token. This reaches the ordinary meta-pixel materialization closure for
    // the token-aware writer specialization.
    let mut token_ordinary_meta_bytes = Vec::new();
    let mut token_ordinary_meta_writer = BitWriter {
        writer: &mut token_ordinary_meta_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &coverage_token,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    let mut token_ordinary_meta_scratch = TokenStreamScratch::default();
    let _ = write_token_stream(
        &mut token_ordinary_meta_writer,
        &meta_pixels,
        meta_width,
        &meta_tokens,
        TokenStreamConfig {
            write_meta_huffman_bit: true,
            cache_bits: 1,
            histogram_bits: 3,
            quality: 0,
        },
        &mut token_ordinary_meta_scratch,
        None,
    );
    let _ = token_ordinary_meta_writer.flush();

    // Exercise the token-aware writer's nested metadata stream with tile
    // histograms that remain distinct under several clustering thresholds.
    // The ordinary writer already covers this call site; these deliberately
    // varied tiles keep the `TokenBitWriterCheckpoint` specialization on the
    // multiple-group path as well.
    let multi_group_width = 32_usize;
    let multi_group_height = 32_usize;
    let multi_group_pixels = (0..multi_group_width * multi_group_height)
        .map(|index| {
            let x = index % multi_group_width;
            let y = index / multi_group_width;
            let tile = (x / 4) + (y / 4) * 8;
            let value = (tile as u32).wrapping_mul(0x1f3d_5b79);
            0xff00_0000 | (value & 0x00ff_ffff)
        })
        .collect::<Vec<_>>();
    let multi_group_tokens = multi_group_pixels
        .iter()
        .copied()
        .map(backward_refs::Token::Literal)
        .collect::<Vec<_>>();
    for (quality, histogram_bits) in [(100, 2)] {
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: TokenBitWriterCheckpoint {
                token: &coverage_token,
                written_bits: 0,
                output_bytes: 0,
            },
        };
        let mut scratch = TokenStreamScratch::default();
        let _ = std::hint::black_box(write_token_stream(
            &mut writer,
            &multi_group_pixels,
            multi_group_width,
            &multi_group_tokens,
            TokenStreamConfig {
                write_meta_huffman_bit: true,
                cache_bits: 0,
                histogram_bits,
                quality,
            },
            &mut scratch,
            Some(&coverage_token),
        ));
        let _ = writer.flush();
    }

    // Measure this small token-aware stream so the targeted late cancellation
    // window below can reach the nested metadata image stream.
    COVERAGE_NESTED_METADATA_REMAINING.store(usize::MAX, Ordering::Relaxed);
    let stream_probe_token = crate::CancellationToken::new();
    stream_probe_token.cancel_after(usize::MAX);
    let stream_probe_writer_token = crate::CancellationToken::new();
    let mut stream_probe_bytes = Vec::new();
    let mut stream_probe_writer = BitWriter {
        writer: &mut stream_probe_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &stream_probe_writer_token,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    let mut stream_probe_scratch = TokenStreamScratch::default();
    let _ = write_token_stream(
        &mut stream_probe_writer,
        &multi_group_pixels,
        multi_group_width,
        &multi_group_tokens,
        TokenStreamConfig {
            write_meta_huffman_bit: true,
            cache_bits: 0,
            histogram_bits: 2,
            quality: 100,
        },
        &mut stream_probe_scratch,
        Some(&stream_probe_token),
    );
    let nested_metadata_checks =
        usize::MAX.saturating_sub(COVERAGE_NESTED_METADATA_REMAINING.load(Ordering::Relaxed));
    for checks in
        nested_metadata_checks.saturating_sub(2)..=nested_metadata_checks.saturating_add(2)
    {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let writer_token = crate::CancellationToken::new();
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: TokenBitWriterCheckpoint {
                token: &writer_token,
                written_bits: 0,
                output_bytes: 0,
            },
        };
        let mut scratch = TokenStreamScratch::default();
        let _ = write_token_stream(
            &mut writer,
            &multi_group_pixels,
            multi_group_width,
            &multi_group_tokens,
            TokenStreamConfig {
                write_meta_huffman_bit: true,
                cache_bits: 0,
                histogram_bits: 2,
                quality: 100,
            },
            &mut scratch,
            Some(&token),
        );
    }

    let mut palette = (0..20)
        .map(|index| {
            let value = ((index * 37) & 0xff) as u32;
            0xff00_0000
                | (value << 16)
                | (((255_u32.wrapping_sub(value)) & 0xff) << 8)
                | (value ^ 0x55)
        })
        .collect::<Vec<_>>();
    palette[0] = 0;
    minimize_palette_deltas(&mut palette);
    let mut nonzero_first_palette = (0..20)
        .map(|index| {
            let value = ((index * 29 + 7) & 0xff) as u32;
            0xff00_0000 | (value << 16) | (((value ^ 0xa5) & 0xff) << 8) | value
        })
        .collect::<Vec<_>>();
    minimize_palette_deltas(&mut nonzero_first_palette);
    let palette_probe_token = crate::CancellationToken::new();
    palette_probe_token.cancel_after(usize::MAX);
    let mut palette_probe = (0..128)
        .map(|index| {
            let value = ((index * 37) & 0xff) as u32;
            0xff00_0000
                | (value << 16)
                | (((255_u32.wrapping_sub(value)) & 0xff) << 8)
                | (value ^ 0x55)
        })
        .collect::<Vec<_>>();
    palette_probe[0] = 0;
    let _ = minimize_palette_deltas_with_checkpoint(&mut palette_probe, &palette_probe_token);
    let palette_calls = usize::MAX.saturating_sub(
        palette_probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=palette_calls {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut palette = (0..128)
            .map(|index| {
                let value = ((index * 37) & 0xff) as u32;
                0xff00_0000
                    | (value << 16)
                    | (((255_u32.wrapping_sub(value)) & 0xff) << 8)
                    | (value ^ 0x55)
            })
            .collect::<Vec<_>>();
        palette[0] = 0;
        let _ = minimize_palette_deltas_with_checkpoint(&mut palette, &token);
    }

    let mut subtract_probe_pixels = (0..2_048)
        .map(|index| {
            let value = index as u32;
            0xff00_0000 | ((value & 0xff) << 16) | (((value * 3) & 0xff) << 8) | value
        })
        .collect::<Vec<_>>();
    let subtract_probe_token = crate::CancellationToken::new();
    subtract_probe_token.cancel_after(usize::MAX);
    let _ = subtract_green_with_checkpoint(&mut subtract_probe_pixels, Some(&subtract_probe_token));
    let subtract_calls = usize::MAX.saturating_sub(
        subtract_probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=subtract_calls {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut pixels = (0..2_048)
            .map(|index| {
                let value = index as u32;
                0xff00_0000 | ((value & 0xff) << 16) | (((value * 3) & 0xff) << 8) | value
            })
            .collect::<Vec<_>>();
        let _ = subtract_green_with_checkpoint(&mut pixels, Some(&token));
    }

    let collect_probe_pixels = (0..2_048)
        .map(|index| {
            let value = index as u32;
            0xff00_0000 | ((value & 0xff) << 16) | (((value * 5) & 0xff) << 8) | value
        })
        .collect::<Vec<_>>();
    let collect_probe_token = crate::CancellationToken::new();
    collect_probe_token.cancel_after(usize::MAX);
    let _ = collect_palette(&collect_probe_pixels, Some(&collect_probe_token));
    let collect_calls = usize::MAX.saturating_sub(
        collect_probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=collect_calls {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = collect_palette(&collect_probe_pixels, Some(&token));
    }

    let convert_probe_data = (0..2_048)
        .flat_map(|index| {
            let value = index as u8;
            [value, value.wrapping_mul(3), value.wrapping_mul(5)]
        })
        .collect::<Vec<_>>();
    let convert_probe_token = crate::CancellationToken::new();
    convert_probe_token.cancel_after(usize::MAX);
    let _ = convert_pixels(
        &convert_probe_data,
        ColorType::Rgb8,
        Some(&convert_probe_token),
    );
    let convert_calls = usize::MAX.saturating_sub(
        convert_probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=convert_calls {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = convert_pixels(&convert_probe_data, ColorType::Rgb8, Some(&token));
    }
    let convert_rgba_probe_data = (0..2_048)
        .flat_map(|index| {
            let value = index as u8;
            [value, value.wrapping_mul(3), value.wrapping_mul(5), value]
        })
        .collect::<Vec<_>>();
    let convert_rgba_probe_token = crate::CancellationToken::new();
    convert_rgba_probe_token.cancel_after(usize::MAX);
    let _ = convert_pixels(
        &convert_rgba_probe_data,
        ColorType::Rgba8,
        Some(&convert_rgba_probe_token),
    );
    let convert_rgba_calls = usize::MAX.saturating_sub(
        convert_rgba_probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=convert_rgba_calls {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = convert_pixels(&convert_rgba_probe_data, ColorType::Rgba8, Some(&token));
    }

    let extend_probe_data = vec![0_u8; VP8L_OUTPUT_CHECKPOINT_BYTES * 2];
    let extend_probe_token = crate::CancellationToken::new();
    extend_probe_token.cancel_after(usize::MAX);
    let mut extend_probe_output = Vec::new();
    let _ = extend_bytes_with_checkpoint(
        &mut extend_probe_output,
        &extend_probe_data,
        Some(&extend_probe_token),
    );
    let extend_calls = usize::MAX.saturating_sub(
        extend_probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=extend_calls {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut output = Vec::new();
        let _ = extend_bytes_with_checkpoint(&mut output, &extend_probe_data, Some(&token));
    }
    let chunk_probe_token = crate::CancellationToken::new();
    chunk_probe_token.cancel_after(usize::MAX);
    let mut chunk_probe_output = Vec::new();
    let _ = write_chunk(
        &mut chunk_probe_output,
        b"TEST",
        &extend_probe_data,
        Some(&chunk_probe_token),
    );
    let chunk_calls = usize::MAX.saturating_sub(
        chunk_probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=chunk_calls {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut output = Vec::new();
        let _ = write_chunk(&mut output, b"TEST", &extend_probe_data, Some(&token));
    }

    let mut apply_probe_pixels = (0..128)
        .map(|index| {
            let value = index as u32;
            0xff00_0000 | ((value & 0xff) << 16) | (((value * 3) & 0xff) << 8) | value
        })
        .collect::<Vec<_>>();
    let apply_probe_palette = apply_probe_pixels.clone();
    let apply_probe_token = crate::CancellationToken::new();
    apply_probe_token.cancel_after(usize::MAX);
    let mut apply_probe_output = Vec::new();
    let mut apply_probe_writer = BitWriter {
        writer: &mut apply_probe_output,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut apply_probe_scratch = ImageStreamScratch::default();
    let _ = apply_palette(
        &mut apply_probe_writer,
        &mut apply_probe_pixels,
        128,
        1,
        apply_probe_palette.clone(),
        &mut apply_probe_scratch,
        Some(&apply_probe_token),
    );
    let apply_calls = usize::MAX.saturating_sub(
        apply_probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=apply_calls {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut pixels = apply_probe_palette.clone();
        let mut output = Vec::new();
        let mut writer = BitWriter {
            writer: &mut output,
            buffer: 0,
            nbits: 0,
            checkpoint: NoopBitWriterCheckpoint::default(),
        };
        let mut scratch = ImageStreamScratch::default();
        let _ = apply_palette(
            &mut writer,
            &mut pixels,
            128,
            1,
            apply_probe_palette.clone(),
            &mut scratch,
            Some(&token),
        );
    }

    let alpha_probe = (0..128)
        .map(|index| {
            if index % 2 == 0 {
                index as u8
            } else {
                255 - index as u8
            }
        })
        .collect::<Vec<_>>();
    let alpha_probe_token = crate::CancellationToken::new();
    alpha_probe_token.cancel_after(usize::MAX);
    let _ = encode_alpha(
        &alpha_probe,
        alpha_probe.len() as u32,
        1,
        Some(&alpha_probe_token),
    );
    let alpha_calls = usize::MAX.saturating_sub(
        alpha_probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=alpha_calls {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = encode_alpha(&alpha_probe, alpha_probe.len() as u32, 1, Some(&token));
    }
    let alpha_sort_probe = (0..128)
        .map(|index| match index % 3 {
            0 => 0,
            1 => 200,
            _ => 201,
        })
        .collect::<Vec<_>>();
    let alpha_sort_probe_token = crate::CancellationToken::new();
    alpha_sort_probe_token.cancel_after(usize::MAX);
    let _ = encode_alpha(
        &alpha_sort_probe,
        alpha_sort_probe.len() as u32,
        1,
        Some(&alpha_sort_probe_token),
    );
    let alpha_sort_calls = usize::MAX.saturating_sub(
        alpha_sort_probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=alpha_sort_calls {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = encode_alpha(
            &alpha_sort_probe,
            alpha_sort_probe.len() as u32,
            1,
            Some(&token),
        );
    }
    let alpha_three = [0_u8, 200, 201];
    let alpha_five = [0_u8, 200, 201, 202, 203];
    let _ = encode_alpha(&alpha_three, 3, 1, None);
    let _ = encode_alpha(&alpha_five, 5, 1, None);
    let alpha_checkpoint_probe = (0..1_024)
        .map(|index| (index as u8).wrapping_mul(37))
        .collect::<Vec<_>>();
    let alpha_success_token = crate::CancellationToken::new();
    alpha_success_token.cancel_after(usize::MAX);
    let _ = std::hint::black_box(collect_alpha_palette(
        &alpha_checkpoint_probe,
        Some(&alpha_success_token),
    ));
    let alpha_failure_token = crate::CancellationToken::new();
    alpha_failure_token.cancel_after(0);
    let _ = std::hint::black_box(collect_alpha_palette(
        &alpha_checkpoint_probe,
        Some(&alpha_failure_token),
    ));
    let alpha_frame_failure_token = crate::CancellationToken::new();
    alpha_frame_failure_token.cancel_after(1);
    let mut alpha_frame_scratch = ImageStreamScratch::default();
    let _ = std::hint::black_box(encode_alpha_with_scratch(
        &alpha_checkpoint_probe,
        alpha_checkpoint_probe.len() as u32,
        1,
        &mut alpha_frame_scratch,
        Some(&alpha_frame_failure_token),
    ));
    let entropy_pixels = [0xff10_2010, 0xff20_4020, 0xff30_6030, 0xff40_8040];
    let _ = analyze_entropy(&entropy_pixels, 2, 2, None, 1, None);
    let wide_entropy_pixels = (0..(2 * 1_025))
        .map(|index| {
            let value = index as u32;
            0xff00_0000 | ((value & 0xff) << 16) | (((value * 3) & 0xff) << 8) | value * 7 & 0xff
        })
        .collect::<Vec<_>>();
    let entropy_probe_token = crate::CancellationToken::new();
    entropy_probe_token.cancel_after(usize::MAX);
    let _ = std::hint::black_box(analyze_entropy(
        &wide_entropy_pixels,
        1_025,
        2,
        None,
        1,
        Some(&entropy_probe_token),
    ));
    let entropy_probe_calls = usize::MAX.saturating_sub(
        entropy_probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=entropy_probe_calls {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = std::hint::black_box(analyze_entropy(
            &wide_entropy_pixels,
            1_025,
            2,
            None,
            1,
            Some(&token),
        ));
    }

    // A tiny direct frame lets one measured cancellation schedule reach the
    // post-palette and post-entropy checkpoints without replaying a full
    // encoder image. The 1,024-pixel RGBA probe separately reaches alpha
    // cleanup's first checkpoint.
    let frame_probe_data = vec![
        0, 0, 0, 0, 32, 64, 96, 255, 64, 128, 192, 255, 96, 192, 32, 0,
    ];
    let frame_probe_token = crate::CancellationToken::new();
    frame_probe_token.cancel_after(usize::MAX);
    let mut frame_probe_scratch = ImageStreamScratch::default();
    let _ = std::hint::black_box(encode_frame(
        &frame_probe_data,
        2,
        2,
        ColorType::Rgba8,
        Some(&frame_probe_token),
        &mut frame_probe_scratch,
    ));
    let frame_probe_calls = usize::MAX.saturating_sub(
        frame_probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=frame_probe_calls {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut scratch = ImageStreamScratch::default();
        let _ = std::hint::black_box(encode_frame(
            &frame_probe_data,
            2,
            2,
            ColorType::Rgba8,
            Some(&token),
            &mut scratch,
        ));
    }
    let alpha_frame_data = (0..1_024)
        .flat_map(|index| {
            let value = index as u8;
            [value, value.wrapping_mul(3), value.wrapping_mul(5), 0]
        })
        .collect::<Vec<_>>();
    for checks in [3, 4] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut scratch = ImageStreamScratch::default();
        let _ = std::hint::black_box(encode_frame(
            &alpha_frame_data,
            1_024,
            1,
            ColorType::Rgba8,
            Some(&token),
            &mut scratch,
        ));
    }
    let grayscale_pixels = vec![0xff20_2020; 2_048];
    let _ = pixels_are_grayscale_with_checkpoint(&grayscale_pixels, Some(&coverage_token));
    let grayscale_probe_token = crate::CancellationToken::new();
    grayscale_probe_token.cancel_after(usize::MAX);
    let _ = pixels_are_grayscale_with_checkpoint(&grayscale_pixels, Some(&grayscale_probe_token));
    let grayscale_calls = usize::MAX.saturating_sub(
        grayscale_probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=grayscale_calls {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = pixels_are_grayscale_with_checkpoint(&grayscale_pixels, Some(&token));
    }
    let mut non_grayscale_pixels = grayscale_pixels.clone();
    non_grayscale_pixels[1_024] = 0xff20_211f;
    let _ = pixels_are_grayscale_with_checkpoint(&non_grayscale_pixels, Some(&coverage_token));
    let mut red_mismatch_pixels = grayscale_pixels;
    red_mismatch_pixels[0] = 0xff21_2020;
    let _ = pixels_are_grayscale_with_checkpoint(&red_mismatch_pixels, Some(&coverage_token));

    let mut transform_pixels = (0..(64 * 64))
        .map(|index| {
            let value = index as u32;
            0xff00_0000 | ((value & 0xff) << 16) | (((value * 3) & 0xff) << 8) | (value * 7 & 0xff)
        })
        .collect::<Vec<_>>();
    let mut transform_scratch = ImageStreamScratch::default();
    encode_frame_stream(
        &mut transform_pixels,
        64,
        64,
        false,
        EntropyMode::SpatialSubtractGreen,
        false,
        1,
        Vec::new(),
        Some(&coverage_token),
        TokenBitWriterCheckpoint {
            token: &coverage_token,
            written_bits: 0,
            output_bytes: 0,
        },
        &mut transform_scratch,
    )
    .expect("token-aware transform stream coverage input must encode");

    let mut grayscale_transform_pixels = vec![0xff40_4040; 64 * 64];
    let mut grayscale_transform_scratch = ImageStreamScratch::default();
    encode_frame_stream(
        &mut grayscale_transform_pixels,
        64,
        64,
        false,
        EntropyMode::Spatial,
        true,
        1,
        Vec::new(),
        Some(&coverage_token),
        TokenBitWriterCheckpoint {
            token: &coverage_token,
            written_bits: 0,
            output_bytes: 0,
        },
        &mut grayscale_transform_scratch,
    )
    .expect("token-aware grayscale predictor coverage input must encode");

    let grayscale_frame = vec![0xff40_4040; 16 * 16];
    let non_grayscale_frame = (0..16 * 16)
        .map(|index| {
            let value = index as u32;
            0xff00_0000 | ((value & 0xff) << 16) | (((value * 3) & 0xff) << 8) | value
        })
        .collect::<Vec<_>>();

    let palette_frame_palette = vec![0xff00_0000, 0xff00_00ff, 0xffff_0000, 0xff00_ff00];
    let palette_frame_pixels = (0..16 * 16)
        .map(|index| palette_frame_palette[index % palette_frame_palette.len()])
        .collect::<Vec<_>>();
    for fail_after in 0..=512 {
        let mut pixels = palette_frame_pixels.clone();
        let mut scratch = ImageStreamScratch::default();
        let _ = encode_frame_stream(
            &mut pixels,
            16,
            16,
            false,
            EntropyMode::Palette,
            true,
            1,
            palette_frame_palette.clone(),
            None,
            NoopBitWriterCheckpoint { fail_after },
            &mut scratch,
        );
    }

    COVERAGE_FRAME_CROSS_REMAINING.store(usize::MAX, Ordering::Relaxed);
    COVERAGE_FRAME_COLOR_REMAINING.store(usize::MAX, Ordering::Relaxed);
    COVERAGE_FRAME_PREDICTOR_REMAINING.store(usize::MAX, Ordering::Relaxed);
    let frame_probe_token = crate::CancellationToken::new();
    frame_probe_token.cancel_after(usize::MAX);
    let frame_probe_writer_token = crate::CancellationToken::new();
    let mut frame_probe_pixels = non_grayscale_frame.clone();
    let mut frame_probe_scratch = ImageStreamScratch::default();
    let _ = encode_frame_stream(
        &mut frame_probe_pixels,
        16,
        16,
        false,
        EntropyMode::SpatialSubtractGreen,
        false,
        1,
        Vec::new(),
        Some(&frame_probe_token),
        TokenBitWriterCheckpoint {
            token: &frame_probe_writer_token,
            written_bits: 0,
            output_bytes: 0,
        },
        &mut frame_probe_scratch,
    );
    let frame_cross_checks =
        usize::MAX.saturating_sub(COVERAGE_FRAME_CROSS_REMAINING.load(Ordering::Relaxed));
    let frame_color_checks =
        usize::MAX.saturating_sub(COVERAGE_FRAME_COLOR_REMAINING.load(Ordering::Relaxed));
    let frame_predictor_checks =
        usize::MAX.saturating_sub(COVERAGE_FRAME_PREDICTOR_REMAINING.load(Ordering::Relaxed));
    for checks in frame_cross_checks.saturating_sub(2)..=frame_cross_checks.saturating_add(2) {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let writer_token = crate::CancellationToken::new();
        let mut pixels = non_grayscale_frame.clone();
        let mut scratch = ImageStreamScratch::default();
        let _ = encode_frame_stream(
            &mut pixels,
            16,
            16,
            false,
            EntropyMode::SpatialSubtractGreen,
            false,
            1,
            Vec::new(),
            Some(&token),
            TokenBitWriterCheckpoint {
                token: &writer_token,
                written_bits: 0,
                output_bytes: 0,
            },
            &mut scratch,
        );
    }
    for checks in frame_color_checks.saturating_sub(2)..=frame_color_checks.saturating_add(2) {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let writer_token = crate::CancellationToken::new();
        let mut pixels = non_grayscale_frame.clone();
        let mut scratch = ImageStreamScratch::default();
        let _ = encode_frame_stream(
            &mut pixels,
            16,
            16,
            false,
            EntropyMode::SpatialSubtractGreen,
            false,
            1,
            Vec::new(),
            Some(&token),
            TokenBitWriterCheckpoint {
                token: &writer_token,
                written_bits: 0,
                output_bytes: 0,
            },
            &mut scratch,
        );
    }
    for checks in
        frame_predictor_checks.saturating_sub(2)..=frame_predictor_checks.saturating_add(2)
    {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let writer_token = crate::CancellationToken::new();
        let mut pixels = non_grayscale_frame.clone();
        let mut scratch = ImageStreamScratch::default();
        let _ = encode_frame_stream(
            &mut pixels,
            16,
            16,
            false,
            EntropyMode::SpatialSubtractGreen,
            false,
            1,
            Vec::new(),
            Some(&token),
            TokenBitWriterCheckpoint {
                token: &writer_token,
                written_bits: 0,
                output_bytes: 0,
            },
            &mut scratch,
        );
    }

    for checks in [0, 1, 32, 256, 1_023] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut pixels = grayscale_frame.clone();
        let mut scratch = ImageStreamScratch::default();
        let _ = encode_frame_stream(
            &mut pixels,
            16,
            16,
            false,
            EntropyMode::Spatial,
            true,
            1,
            Vec::new(),
            Some(&token),
            TokenBitWriterCheckpoint {
                token: &token,
                written_bits: 0,
                output_bytes: 0,
            },
            &mut scratch,
        );
        let mut pixels = non_grayscale_frame.clone();
        let mut scratch = ImageStreamScratch::default();
        let _ = encode_frame_stream(
            &mut pixels,
            16,
            16,
            false,
            EntropyMode::SpatialSubtractGreen,
            false,
            1,
            Vec::new(),
            Some(&token),
            TokenBitWriterCheckpoint {
                token: &token,
                written_bits: 0,
                output_bytes: 0,
            },
            &mut scratch,
        );
    }

    // The outer bit writer is deliberately non-cancellable here. This
    // isolates cancellation in cross-color selection and in its following
    // color image stream, which are separate encoder work units.
    for checks in [0, 1, 32, 256, 1_024] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut pixels = non_grayscale_frame.clone();
        let mut scratch = ImageStreamScratch::default();
        let _ = encode_frame_stream(
            &mut pixels,
            16,
            16,
            false,
            EntropyMode::SpatialSubtractGreen,
            false,
            1,
            Vec::new(),
            Some(&token),
            NoopBitWriterCheckpoint::default(),
            &mut scratch,
        );
    }
    #[cfg(coverage_nightly)]
    {
        let cross_probe_token = crate::CancellationToken::new();
        cross_probe_token.cancel_after(usize::MAX);
        let mut cross_probe_pixels = non_grayscale_frame.clone();
        let mut cross_probe_scratch = ImageStreamScratch::default();
        let _ = encode_frame_stream(
            &mut cross_probe_pixels,
            16,
            16,
            false,
            EntropyMode::SpatialSubtractGreen,
            false,
            1,
            Vec::new(),
            Some(&cross_probe_token),
            NoopBitWriterCheckpoint::default(),
            &mut cross_probe_scratch,
        );
        let cross_probe_checks = usize::MAX.saturating_sub(
            cross_probe_token
                .coverage_remaining_checks()
                .unwrap_or(usize::MAX),
        );
        for checks in 0..=cross_probe_checks {
            let token = crate::CancellationToken::new();
            token.cancel_after(checks);
            let mut pixels = non_grayscale_frame.clone();
            let mut scratch = ImageStreamScratch::default();
            let _ = encode_frame_stream(
                &mut pixels,
                16,
                16,
                false,
                EntropyMode::SpatialSubtractGreen,
                false,
                1,
                Vec::new(),
                Some(&token),
                NoopBitWriterCheckpoint::default(),
                &mut scratch,
            );
        }
    }

    let mut no_op_success_pixels = vec![0xff40_4040; 16 * 16];
    let mut no_op_success_scratch = ImageStreamScratch::default();
    std::hint::black_box(
        encode_frame_stream(
            &mut no_op_success_pixels,
            16,
            16,
            false,
            EntropyMode::Spatial,
            true,
            1,
            Vec::new(),
            None,
            NoopBitWriterCheckpoint::default(),
            &mut no_op_success_scratch,
        )
        .expect("no-op frame coverage input must encode"),
    );

    // The token-aware subtract-green branch is exercised by normal frames,
    // but the no-token specialization is a separate monomorphization. Keep a
    // small varied frame here so its direct subtract-green fallback is also
    // represented in the strict all-feature coverage contract.
    let mut no_token_subtract_pixels = (0..16 * 16)
        .map(|index| {
            let value = index as u32;
            0xff00_0000 | ((value & 0xff) << 16) | (((value * 3) & 0xff) << 8) | value
        })
        .collect::<Vec<_>>();
    let mut no_token_subtract_scratch = ImageStreamScratch::default();
    std::hint::black_box(
        encode_frame_stream(
            &mut no_token_subtract_pixels,
            16,
            16,
            false,
            EntropyMode::SubtractGreen,
            false,
            1,
            Vec::new(),
            None,
            NoopBitWriterCheckpoint::default(),
            &mut no_token_subtract_scratch,
        )
        .expect("no-token subtract-green coverage input must encode"),
    );

    let no_token_checkpoint_token = crate::CancellationToken::new();
    let mut no_token_token_pixels = no_token_subtract_pixels.clone();
    let mut no_token_token_scratch = ImageStreamScratch::default();
    std::hint::black_box(
        encode_frame_stream(
            &mut no_token_token_pixels,
            16,
            16,
            false,
            EntropyMode::SubtractGreen,
            false,
            1,
            Vec::new(),
            None,
            TokenBitWriterCheckpoint {
                token: &no_token_checkpoint_token,
                written_bits: 0,
                output_bytes: 0,
            },
            &mut no_token_token_scratch,
        )
        .expect("no-token token-checkpoint coverage input must encode"),
    );

    let small_cross_frame = (0..64)
        .map(|index| {
            let value = index as u32;
            0xff00_0000 | ((value & 0xff) << 16) | (((value * 5) & 0xff) << 8) | value
        })
        .collect::<Vec<_>>();
    for checks in [0, 1, 32, 256, 1_024, 2_048] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut pixels = small_cross_frame.clone();
        let mut scratch = ImageStreamScratch::default();
        let _ = encode_frame_stream(
            &mut pixels,
            8,
            8,
            false,
            EntropyMode::Spatial,
            false,
            2,
            Vec::new(),
            Some(&token),
            NoopBitWriterCheckpoint::default(),
            &mut scratch,
        );
    }
    let mut palette_bytes = Vec::new();
    let mut palette_writer = BitWriter {
        writer: &mut palette_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut palette = (0..18)
        .map(|index| 0xff00_0000 | ((index as u32) << 16))
        .collect::<Vec<_>>();
    palette.push(0);
    let mut palette_pixels = [0xff00_0000, 0xff01_0000, 0xff02_0000, 0xff03_0000];
    let mut palette_scratch = ImageStreamScratch::default();
    let _ = apply_palette(
        &mut palette_writer,
        &mut palette_pixels,
        2,
        2,
        palette,
        &mut palette_scratch,
        None,
    );
    let _ = palette_writer.flush();

    let mut palette_trim_bytes = Vec::new();
    let mut palette_trim_writer = BitWriter {
        writer: &mut palette_trim_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut palette_trim_pixels = [0; 4];
    let _ = apply_palette(
        &mut palette_trim_writer,
        &mut palette_trim_pixels,
        2,
        2,
        vec![0; 18],
        &mut palette_scratch,
        None,
    );
    let _ = palette_trim_writer.flush();

    let mut palette4_bytes = Vec::new();
    let mut palette4_writer = BitWriter {
        writer: &mut palette4_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut palette4_pixels = [0xff00_0000, 0xff01_0000, 0xff02_0000, 0xff03_0000];
    let _ = apply_palette(
        &mut palette4_writer,
        &mut palette4_pixels,
        2,
        2,
        vec![0xff00_0000, 0xff01_0000, 0xff02_0000, 0xff03_0000],
        &mut palette_scratch,
        None,
    );
    let _ = palette4_writer.flush();

    let mut palette16_bytes = Vec::new();
    let mut palette16_writer = BitWriter {
        writer: &mut palette16_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let palette16 = (0..16)
        .map(|index| 0xff00_0000 | ((index as u32) << 16))
        .collect::<Vec<_>>();
    let mut palette16_pixels = [0xff00_0000, 0xff01_0000, 0xff02_0000, 0xff03_0000];
    let _ = apply_palette(
        &mut palette16_writer,
        &mut palette16_pixels,
        2,
        2,
        palette16,
        &mut palette_scratch,
        None,
    );
    let _ = palette16_writer.flush();

    let missing_palette = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = palette_index_with_checkpoint(&[0], 1, &coverage_token);
    }));
    let _ = missing_palette;

    let token_palette = (0..20)
        .map(|index| {
            let value = (index * 11) as u32;
            0xff00_0000 | (value << 16) | ((value ^ 0x55) << 8) | (value ^ 0xaa)
        })
        .collect::<Vec<_>>();
    let mut rich_success_pixels = (0..(64 * 32))
        .map(|index| token_palette[index % token_palette.len()])
        .collect::<Vec<_>>();
    let mut rich_success_bytes = Vec::new();
    let mut rich_success_writer = BitWriter {
        writer: &mut rich_success_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint::default(),
    };
    let mut rich_success_scratch = ImageStreamScratch::default();
    std::hint::black_box(
        apply_palette(
            &mut rich_success_writer,
            &mut rich_success_pixels,
            64,
            32,
            token_palette.clone(),
            &mut rich_success_scratch,
            None,
        )
        .expect("rich no-op palette coverage input must encode"),
    );
    let _ = std::hint::black_box(rich_success_writer.flush());
    let mut token_palette_pixels = (0..(64 * 32))
        .map(|index| token_palette[index % token_palette.len()])
        .collect::<Vec<_>>();
    let mut token_palette_bytes = Vec::new();
    let mut token_palette_writer = BitWriter {
        writer: &mut token_palette_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &coverage_token,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    std::hint::black_box(
        apply_palette(
            &mut token_palette_writer,
            &mut token_palette_pixels,
            64,
            32,
            token_palette,
            &mut palette_scratch,
            Some(&coverage_token),
        )
        .expect("token-aware palette coverage input must encode"),
    );
    let _ = token_palette_writer.flush();
    std::hint::black_box(&token_palette_bytes);

    let ordinary_palette = vec![0xff00_0000, 0xff00_0001];
    let mut ordinary_palette_pixels = vec![ordinary_palette[0], ordinary_palette[1]];
    let mut ordinary_palette_bytes = Vec::new();
    let mut ordinary_palette_writer = BitWriter {
        writer: &mut ordinary_palette_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: TokenBitWriterCheckpoint {
            token: &coverage_token,
            written_bits: 0,
            output_bytes: 0,
        },
    };
    let _ = apply_palette(
        &mut ordinary_palette_writer,
        &mut ordinary_palette_pixels,
        2,
        1,
        ordinary_palette,
        &mut palette_scratch,
        None,
    );
    let _ = ordinary_palette_writer.flush();

    for fail_after in [0, 1, 2, 64, 512, 2_048] {
        let mut bytes = Vec::new();
        let mut writer = BitWriter {
            writer: &mut bytes,
            buffer: 0,
            nbits: 0,
            checkpoint: NoopBitWriterCheckpoint { fail_after },
        };
        let mut pixels = [0xff00_0000, 0xff01_0000, 0xff02_0000, 0xff03_0000];
        let mut scratch = ImageStreamScratch::default();
        let _ = std::hint::black_box(apply_palette(
            &mut writer,
            &mut pixels,
            2,
            2,
            vec![0xff00_0000, 0xff01_0000, 0xff02_0000, 0xff03_0000],
            &mut scratch,
            None,
        ));
    }

    let alpha = [
        0, 255, 1, 254, 2, 253, 3, 252, 4, 251, 5, 250, 6, 249, 7, 248, 8, 247, 9, 246,
    ];
    let _ = encode_alpha(&alpha, alpha.len() as u32, 1, None);
    let short_alpha = [
        0, 255, 1, 254, 2, 253, 3, 252, 4, 251, 5, 250, 6, 249, 7, 248, 8,
    ];
    let _ = encode_alpha(&short_alpha, short_alpha.len() as u32, 1, None);
    let nonzero_alpha = [
        1, 255, 2, 254, 3, 253, 4, 252, 5, 251, 6, 250, 7, 249, 8, 248, 9, 247, 10, 246,
    ];
    let _ = encode_alpha(&nonzero_alpha, nonzero_alpha.len() as u32, 1, None);
    let two_value_alpha = [0, 255, 0, 255];
    let _ = encode_alpha(&two_value_alpha, two_value_alpha.len() as u32, 1, None);
    let alpha_values = [
        0_u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 255,
    ];

    for fail_after in [0, 1, 2, 64, 512, 2_048] {
        let mut output = Vec::new();
        let mut token_scratch = TokenStreamScratch::default();
        let _ = std::hint::black_box(encode_alpha_stream(
            &[0xff00_0000],
            1,
            &[0xff00_0000, 0xff00_0000, 0xff00_0000, 0xff00_0000],
            2,
            &[0, 255, 0, 255],
            &mut output,
            &mut token_scratch,
            None,
            NoopBitWriterCheckpoint { fail_after },
        ));
    }
    let rich_alpha_palette_delta = (0..20)
        .map(|index| 0xff00_0000 | ((index as u32) << 8))
        .collect::<Vec<_>>();
    let rich_alpha_packed = (0..64)
        .map(|index| 0xff00_0000 | (index as u32))
        .collect::<Vec<_>>();
    let rich_alpha = (0..64).map(|index| (index * 17) as u8).collect::<Vec<_>>();
    let mut rich_alpha_success_output = Vec::new();
    let mut rich_alpha_success_scratch = TokenStreamScratch::default();
    std::hint::black_box(
        encode_alpha_stream(
            &rich_alpha_palette_delta,
            rich_alpha_palette_delta.len(),
            &rich_alpha_packed,
            8,
            &rich_alpha,
            &mut rich_alpha_success_output,
            &mut rich_alpha_success_scratch,
            None,
            NoopBitWriterCheckpoint::default(),
        )
        .expect("rich no-op alpha coverage input must encode"),
    );

    let token_alpha = (0..2_048)
        .map(|index| alpha_values[index % alpha_values.len()])
        .collect::<Vec<_>>();
    encode_alpha(
        &token_alpha,
        token_alpha.len() as u32,
        1,
        Some(&coverage_token),
    )
    .expect("token-aware alpha coverage input must encode");
    let flat_token_alpha = vec![0_u8; 2_048];
    encode_alpha(
        &flat_token_alpha,
        flat_token_alpha.len() as u32,
        1,
        Some(&coverage_token),
    )
    .expect("flat token-aware alpha coverage input must encode");

    for checks in [0, 1, 32, 256, 1_023] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let alpha = (0..20).map(|index| (index * 13) as u8).collect::<Vec<_>>();
        let _ = encode_alpha(&alpha, alpha.len() as u32, 1, Some(&token));
    }

    for length in [1, 2, 20, 64, 256] {
        for pattern in 0..4 {
            let alpha = (0..length)
                .map(|index: usize| match pattern {
                    0 => 0,
                    1 => (index & 1) as u8 * 255,
                    2 => (index.wrapping_mul(37) & 0xff) as u8,
                    _ => ((index.wrapping_mul(73) ^ (index >> 2)) & 0xff) as u8,
                })
                .collect::<Vec<_>>();
            let _ = encode_alpha(&alpha, length as u32, 1, None);
        }
    }

    let mut encoder = WebPEncoder::new();
    encoder
        .encode_with_token(&[], 0, 1, ColorType::Rgb8, None)
        .expect_err("zero-width WebP must be rejected");
    let mut encoder = WebPEncoder::new();
    encoder
        .encode_with_token(&[], 1, 0, ColorType::Rgb8, None)
        .expect_err("zero-height WebP must be rejected");
    let mut encoder = WebPEncoder::new();
    encoder
        .encode_with_token(&vec![0; 16_385 * 3], 16_385, 1, ColorType::Rgb8, None)
        .expect_err("too-wide WebP must be rejected");
    let mut encoder = WebPEncoder::new();
    encoder
        .encode_with_token(&vec![0; 16_385 * 3], 1, 16_385, ColorType::Rgb8, None)
        .expect_err("too-tall WebP must be rejected");

    let rgb = [0, 0, 0];
    let mut encoder = WebPEncoder::new();
    encoder
        .encode_with_token(&rgb, 1, 1, ColorType::Rgb8, None)
        .expect("one-pixel in-memory WebP must encode");
}
