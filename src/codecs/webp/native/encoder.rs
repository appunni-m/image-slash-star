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
    match crate::codecs::error::check_cancelled(token) {
        Ok(()) => Ok(()),
        Err(crate::codecs::CodecError::Cancelled) => Err(EncodingError::Cancelled),
        Err(crate::codecs::CodecError::WorkBudgetExceeded { maximum, observed }) => {
            Err(EncodingError::WorkBudgetExceeded { maximum, observed })
        }
        Err(error) => unreachable!("token polling returned an unexpected error: {error:?}"),
    }
}

const VP8L_OUTPUT_CHECKPOINT_BYTES: usize = 1_024;
const VP8L_TRANSFORM_CHECKPOINT_PIXELS: usize = 1_024;
const VP8L_GRAYSCALE_CHECKPOINT_PIXELS: usize = 1_024;
const VP8L_ENTROPY_ANALYSIS_CHECKPOINT_PIXELS: usize = 1_024;
const VP8L_ALPHA_CLEANUP_CHECKPOINT_PIXELS: usize = 1_024;
const VP8L_PIXEL_CONVERSION_CHECKPOINT_PIXELS: usize = 1_024;
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

trait BitWriterCheckpoint: Clone {
    fn checkpoint_bits(&mut self, written: usize) -> Result<(), EncodingError>;
    fn checkpoint_output_bytes(&mut self, emitted: usize) -> Result<(), EncodingError>;
}

#[derive(Clone, Copy, Default)]
struct NoopBitWriterCheckpoint;

impl BitWriterCheckpoint for NoopBitWriterCheckpoint {
    #[inline(always)]
    fn checkpoint_bits(&mut self, _written: usize) -> Result<(), EncodingError> {
        Ok(())
    }

    #[inline(always)]
    fn checkpoint_output_bytes(&mut self, _emitted: usize) -> Result<(), EncodingError> {
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct TokenBitWriterCheckpoint<'a> {
    token: &'a crate::CancellationToken,
    written_bits: usize,
    output_bytes: usize,
}

impl BitWriterCheckpoint for TokenBitWriterCheckpoint<'_> {
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

// Every pop occurs while at least two nodes remain.
#[allow(clippy::unwrap_used)]
fn build_huffman_tree(
    frequencies: &[u32],
    lengths: &mut [u8],
    codes: &mut [u16],
    length_limit: u8,
    token: Option<&crate::CancellationToken>,
) -> Result<bool, EncodingError> {
    check_token(token)?;
    assert_eq!(frequencies.len(), lengths.len());
    assert_eq!(frequencies.len(), codes.len());

    #[derive(Clone)]
    enum Node {
        Leaf(usize),
        Branch(Box<Node>, Box<Node>),
    }
    #[derive(Clone)]
    struct WeightedNode {
        count: u32,
        sort_value: isize,
        node: Node,
    }

    let mut optimized = frequencies.to_vec();
    optimize_huffman_for_rle_with_checkpoint(&mut optimized, token)?;
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
        let mut nodes = if let Some(token) = token {
            let mut nodes = Vec::new();
            for (value, &frequency) in optimized.iter().enumerate() {
                if (value + 1).is_multiple_of(VP8L_HUFFMAN_CHECKPOINT_SYMBOLS) {
                    check_token(Some(token))?;
                }
                if frequency != 0 {
                    nodes.push(WeightedNode {
                        count: frequency.max(count_min),
                        sort_value: value as isize,
                        node: Node::Leaf(value),
                    });
                }
            }
            nodes
        } else {
            optimized
                .iter()
                .enumerate()
                .filter(|&(_, &frequency)| frequency != 0)
                .map(|(value, &frequency)| WeightedNode {
                    count: frequency.max(count_min),
                    sort_value: value as isize,
                    node: Node::Leaf(value),
                })
                .collect::<Vec<_>>()
        };
        if let Some(token) = token {
            // The token-aware path keeps the stable ordering of the original
            // sort with a bounded bottom-up merge sort. A large fixed alphabet
            // can otherwise spend an entire comparison sort between the
            // surrounding tree checkpoints; keeping O(n log n) here also
            // avoids turning cancellation-aware encoding into a quadratic
            // slow path.
            let mut comparisons = 0_usize;
            let mut scratch = nodes.clone();
            let mut width = 1_usize;
            while width < nodes.len() {
                let mut start = 0_usize;
                while start < nodes.len() {
                    let middle = start.saturating_add(width).min(nodes.len());
                    let end = middle.saturating_add(width).min(nodes.len());
                    let mut left = start;
                    let mut right = middle;
                    for slot in &mut scratch[start..end] {
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
                            *slot = nodes[left].clone();
                            left += 1;
                        } else {
                            *slot = nodes[right].clone();
                            right += 1;
                        }
                    }
                    start = end;
                }
                core::mem::swap(&mut nodes, &mut scratch);
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
                WeightedNode {
                    count,
                    sort_value: -1,
                    node: Node::Branch(Box::new(left.node), Box::new(right.node)),
                },
            );
        }

        lengths.fill(0);
        let mut stack = vec![(&nodes[0].node, 0_u8)];
        while let Some((node, depth)) = stack.pop() {
            check_token(token)?;
            match node {
                Node::Leaf(value) => lengths[*value] = depth,
                Node::Branch(left, right) => {
                    stack.push((right, depth + 1));
                    stack.push((left, depth + 1));
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

fn optimize_huffman_for_rle(counts: &mut [u32]) {
    let Some(length) = counts.iter().rposition(|&count| count != 0).map(|i| i + 1) else {
        return;
    };
    let mut good = vec![false; length];
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
    token: Option<&crate::CancellationToken>,
) -> Result<(), EncodingError> {
    if token.is_none() {
        optimize_huffman_for_rle(counts);
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
    let mut good = vec![false; length];
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
                    emit(HuffmanToken {
                        code: 17,
                        extra: (repetitions - 3) as u8,
                    })?;
                    break;
                } else if repetitions < 139 {
                    emit(HuffmanToken {
                        code: 18,
                        extra: (repetitions - 11) as u8,
                    })?;
                    break;
                } else {
                    emit(HuffmanToken {
                        code: 18,
                        extra: 0x7f,
                    })?;
                    repetitions -= 138;
                }
            }
        } else {
            if value != previous {
                emit(HuffmanToken {
                    code: value,
                    extra: 0,
                })?;
                repetitions -= 1;
            }
            while repetitions != 0 {
                if repetitions < 3 {
                    for _ in 0..repetitions {
                        emit(HuffmanToken {
                            code: value,
                            extra: 0,
                        })?;
                    }
                    break;
                } else if repetitions < 7 {
                    emit(HuffmanToken {
                        code: 16,
                        extra: (repetitions - 3) as u8,
                    })?;
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

fn write_huffman_tree<C: BitWriterCheckpoint>(
    w: &mut BitWriter<'_, C>,
    frequencies: &[u32],
    lengths: &mut [u8],
    codes: &mut [u16],
    huffman_tokens: &mut Vec<HuffmanToken>,
    token: Option<&crate::CancellationToken>,
) -> Result<(), EncodingError> {
    build_huffman_tree(frequencies, lengths, codes, 15, token)?;
    let symbols = if let Some(token) = token {
        let mut symbols = Vec::with_capacity(3);
        for (index, &length) in lengths.iter().enumerate() {
            if length != 0 {
                symbols.push(index);
                if symbols.len() == 3 {
                    break;
                }
            }
            if (index + 1).is_multiple_of(VP8L_HUFFMAN_CHECKPOINT_SYMBOLS) {
                check_token(Some(token))?;
            }
        }
        symbols
    } else {
        lengths
            .iter()
            .enumerate()
            .filter_map(|(symbol, &length)| (length != 0).then_some(symbol))
            .take(3)
            .collect::<Vec<_>>()
    };
    if symbols.len() <= 2 && symbols.iter().all(|&symbol| symbol < 256) {
        let first = symbols.first().copied().unwrap_or(0);
        w.write_bits(1, 1)?;
        w.write_bits(u64::from(symbols.len() == 2), 1)?;
        if first <= 1 {
            w.write_bits(0, 1)?;
            w.write_bits(first as u64, 1)?;
        } else {
            w.write_bits(1, 1)?;
            w.write_bits(first as u64, 8)?;
        }
        if symbols.len() == 2 {
            w.write_bits(symbols[1] as u64, 8)?;
        }
        lengths.fill(0);
        codes.fill(0);
        if symbols.len() == 2 {
            lengths[symbols[0]] = 1;
            lengths[symbols[1]] = 1;
            codes[symbols[1]] = 1;
        }
        return Ok(());
    }
    compressed_huffman_tokens_with_checkpoint(lengths, huffman_tokens, token)?;
    let mut code_length_lengths = [0u8; 19];
    let mut code_length_codes = [0u16; 19];
    let mut code_length_frequencies = [0u32; 19];
    if let Some(token) = token {
        for (index, huffman_token) in huffman_tokens.iter().enumerate() {
            code_length_frequencies[usize::from(huffman_token.code)] += 1;
            if (index + 1).is_multiple_of(VP8L_HUFFMAN_TOKEN_CHECKPOINTS) {
                check_token(Some(token))?;
            }
        }
    } else {
        for huffman_token in huffman_tokens.iter() {
            code_length_frequencies[usize::from(huffman_token.code)] += 1;
        }
    }
    build_huffman_tree(
        &code_length_frequencies,
        &mut code_length_lengths,
        &mut code_length_codes,
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
    let mut trimmed_length = huffman_tokens.len();
    let mut trailing_zero_bits = 0;
    // The normal-tree path always emits at least one non-zero code-length
    // token before trailing zero-repeat tokens.
    if let Some(token) = token {
        let mut trimmed_tokens = 0usize;
        loop {
            let huffman_token = huffman_tokens[trimmed_length - 1];
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
            trimmed_tokens += 1;
            if trimmed_tokens.is_multiple_of(VP8L_HUFFMAN_TOKEN_CHECKPOINTS) {
                check_token(Some(token))?;
            }
        }
    } else {
        loop {
            let huffman_token = huffman_tokens[trimmed_length - 1];
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
        huffman_tokens.len()
    };
    for (index, huffman_token) in huffman_tokens[..token_count].iter().enumerate() {
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
    token: Option<&crate::CancellationToken>,
) -> Result<(), EncodingError> {
    write_image_stream_configured(w, pixels, width, write_meta_huffman_bit, 3, 80, 11, token)
}

// `backward_refs::candidates` always returns at least one crunch configuration.
#[allow(clippy::too_many_arguments, clippy::unwrap_used)]
fn write_image_stream_configured<C: BitWriterCheckpoint>(
    w: &mut BitWriter<'_, C>,
    pixels: &[u32],
    width: usize,
    write_meta_huffman_bit: bool,
    histogram_bits: u8,
    quality: u32,
    max_cache_bits: u8,
    token: Option<&crate::CancellationToken>,
) -> Result<(), EncodingError> {
    let candidates = backward_refs::candidates(
        pixels,
        width,
        write_meta_huffman_bit,
        quality,
        max_cache_bits,
        token,
    )?;

    // Candidate trials share the already-emitted prefix. Keeping that prefix
    // out of each trial avoids an O(prefix × candidate-count) copy/allocation;
    // leave the parent writer in place until the winning suffix is selected.
    let initial_byte_length = w.writer.len();
    let initial_buffer = w.buffer;
    let initial_nbits = w.nbits;
    let initial_checkpoint = w.checkpoint.clone();
    let mut best: Option<(usize, Vec<u8>, u64, u8, C)> = None;
    let mut scratch = Vec::new();
    let mut token_scratch = TokenStreamScratch::default();
    for (tokens, cache_bits) in candidates {
        scratch.clear();
        let (byte_length, buffer, nbits, checkpoint) = {
            let mut trial = BitWriter {
                writer: &mut scratch,
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
                &mut token_scratch,
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
            let suffix = core::mem::take(&mut scratch);
            if let Some((_, previous_suffix, ..)) =
                best.replace((byte_length, suffix, buffer, nbits, checkpoint))
            {
                scratch = previous_suffix;
            }
        }
    }
    let (_, suffix, buffer, nbits, checkpoint) = best.unwrap();
    w.writer.reserve(suffix.len());
    extend_bytes_with_checkpoint(w.writer, &suffix, token)?;
    w.buffer = buffer;
    w.nbits = nbits;
    w.checkpoint = checkpoint;
    Ok(())
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
    huffman_tokens: Vec<HuffmanToken>,
}

#[derive(Clone, Copy)]
struct TokenStreamConfig {
    write_meta_huffman_bit: bool,
    cache_bits: u8,
    histogram_bits: u8,
    quality: u32,
}

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
    huffman_tokens: &mut Vec<HuffmanToken>,
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
            huffman_tokens,
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
    let (mut symbols, histograms) = if write_meta_huffman_bit {
        histogram::cluster(
            tokens,
            width,
            height,
            cache_bits,
            quality,
            histogram_bits,
            token,
        )?
    } else {
        histogram::cluster(tokens, width, height, cache_bits, quality, 31, token)?
    };
    check_token(token)?;
    let multiple_groups = write_meta_huffman_bit && histograms.len() > 1;
    let mut encoded_histogram_bits = histogram_bits;
    if write_meta_huffman_bit {
        w.write_bits(u64::from(multiple_groups), 1)?;
        if multiple_groups {
            encoded_histogram_bits =
                optimize_sampling(&mut symbols, width, height, histogram_bits, 9, token)?;
            w.write_bits(u64::from(encoded_histogram_bits - 2), 3)?;
            // Meta-pixel materialization scales with the retained histogram
            // tile map after sampling. Keep the ordinary no-token map
            // unchanged and poll only the caller-controlled path.
            let meta_pixels = if let Some(token) = token {
                let mut meta_pixels = Vec::with_capacity(symbols.len());
                let mut symbols_until_checkpoint = VP8L_HISTOGRAM_SAMPLING_CHECKPOINT_SYMBOLS;
                for &symbol in &symbols {
                    meta_pixels.push(u32::from(symbol) << 8);
                    symbols_until_checkpoint = symbols_until_checkpoint.saturating_sub(1);
                    if symbols_until_checkpoint == 0 {
                        check_token(Some(token))?;
                        symbols_until_checkpoint = VP8L_HISTOGRAM_SAMPLING_CHECKPOINT_SYMBOLS;
                    }
                }
                meta_pixels
            } else {
                symbols
                    .iter()
                    .map(|&symbol| u32::from(symbol) << 8)
                    .collect::<Vec<_>>()
            };
            let meta_width = width.div_ceil(1 << encoded_histogram_bits);
            write_image_stream_configured(
                w,
                &meta_pixels,
                meta_width,
                false,
                3,
                quality,
                0,
                token,
            )?;
        }
    }
    let group_count = histograms.len();
    let group_scratch = &mut scratch.groups;
    let huffman_tokens = &mut scratch.huffman_tokens;
    if group_scratch.len() < group_count {
        group_scratch.resize_with(group_count, GroupCodes::default);
    }
    for (group, histogram) in group_scratch.iter_mut().take(group_count).zip(&histograms) {
        check_token(token)?;
        write_group(w, &histogram.populations, group, huffman_tokens, token)?;
    }

    let tile_width = width.div_ceil(1 << encoded_histogram_bits);
    let reference_context = TokenStreamReferenceContext {
        width,
        multiple_groups,
        symbols: &symbols,
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
    let mut histograms = vec![[0_u32; 256]; 13];
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
    let costs = histograms
        .iter()
        .map(|histogram| histogram::bits_entropy_with_checkpoint(histogram, token))
        .collect::<Result<Vec<_>, _>>()?;
    check_token(token)?;
    let transform_width = width.div_ceil(1 << transform_bits);
    let transform_height = height.div_ceil(1 << transform_bits);
    let fast_log = |value: u32| (f64::from(value).log2() * f64::from(1_u32 << 23)).round() as u64;
    let mut modes = vec![
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
    ];
    if let Some(size) = palette_size {
        modes.push((
            EntropyMode::Palette,
            costs[PALETTE] + ((size as u64 * 8) << 23),
        ));
    }
    let mode = modes
        .into_iter()
        .min_by_key(|&(_, cost)| cost)
        .map(|(mode, _)| mode)
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
fn apply_palette<C: BitWriterCheckpoint>(
    w: &mut BitWriter<'_, C>,
    pixels: &[u32],
    width: usize,
    height: usize,
    mut palette: Vec<u32>,
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
    let palette_delta = palette[..encoded_length]
        .iter()
        .map(|&color| {
            let difference = subtract_pixels(color, previous);
            previous = color;
            difference
        })
        .collect::<Vec<_>>();
    write_image_stream_configured(w, &palette_delta, encoded_length, false, 3, 20, 0, token)?;

    let packing_bits = match palette.len() {
        0..=2 => 3,
        3..=4 => 2,
        5..=16 => 1,
        _ => 0,
    };
    let pixels_per_group = 1 << packing_bits;
    let bits_per_pixel = 8 >> packing_bits;
    let packed_width = width.div_ceil(pixels_per_group);
    let mut packed = Vec::with_capacity(packed_width * height);
    if let Some(token) = token {
        let mut source_pixels_until_checkpoint = VP8L_PALETTE_PACKING_CHECKPOINT_PIXELS;
        for (row_index, row) in pixels.chunks_exact(width).enumerate() {
            if row_index.is_multiple_of(64) {
                check_token(Some(token))?;
            }
            for group in row.chunks(pixels_per_group) {
                let mut packed_pixel = 0xff00_0000_u32;
                for (index, &color) in group.iter().enumerate() {
                    let palette_index = palette_index_with_checkpoint(&palette, color, token)?;
                    packed_pixel |= (palette_index as u32) << (8 + bits_per_pixel * index);
                }
                packed.push(packed_pixel);
                source_pixels_until_checkpoint =
                    source_pixels_until_checkpoint.saturating_sub(group.len());
                if source_pixels_until_checkpoint == 0 {
                    check_token(Some(token))?;
                    source_pixels_until_checkpoint = VP8L_PALETTE_PACKING_CHECKPOINT_PIXELS;
                }
            }
        }
    } else {
        for row in pixels.chunks_exact(width) {
            for group in row.chunks(pixels_per_group) {
                let mut packed_pixel = 0xff00_0000_u32;
                for (index, &color) in group.iter().enumerate() {
                    let palette_index = palette.iter().position(|&entry| entry == color).unwrap();
                    packed_pixel |= (palette_index as u32) << (8 + bits_per_pixel * index);
                }
                packed.push(packed_pixel);
            }
        }
    }
    w.write_bits(0, 1)?;
    let maximum_cache_bits = (usize::BITS - palette.len().leading_zeros()) as u8;
    write_image_stream_configured(
        w,
        &packed,
        packed_width,
        true,
        5,
        80,
        maximum_cache_bits,
        token,
    )
}

#[allow(clippy::too_many_arguments)]
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
) -> Result<Vec<u8>, EncodingError> {
    let mut frame = Vec::new();
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
            apply_palette(w, pixels, width as usize, height as usize, palette, token)?;
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
                w.write_bits(1, 1)?;
                w.write_bits(2, 2)?;
                if token.is_some() {
                    subtract_green_with_checkpoint(pixels, token)?;
                } else {
                    subtract_green(pixels);
                }
                check_token(token)?;
            }

            if use_predictor {
                let (predictor_map, predictor_bits) = if grayscale {
                    predictor::apply_fixed(
                        pixels,
                        width as usize,
                        height as usize,
                        transform_bits,
                        12,
                        token,
                    )?
                } else {
                    predictor::select_and_apply(
                        pixels,
                        width as usize,
                        height as usize,
                        transform_bits,
                        token,
                    )?
                };
                w.write_bits(1, 1)?;
                w.write_bits(0, 2)?;
                w.write_bits(u64::from(predictor_bits - 2), 3)?;
                let predictor_width =
                    (width as usize + (1 << predictor_bits) - 1) >> predictor_bits;
                write_image_stream(w, &predictor_map, predictor_width, false, token)?;
            }

            if use_predictor && !red_and_blue_zero {
                let (color_map, color_bits) = cross_color::select_and_apply(
                    pixels,
                    width as usize,
                    height as usize,
                    transform_bits,
                    80,
                    token,
                )?;
                w.write_bits(1, 1)?;
                w.write_bits(1, 2)?;
                w.write_bits(u64::from(color_bits - 2), 3)?;
                let color_width = (width as usize + (1 << color_bits) - 1) >> color_bits;
                write_image_stream(w, &color_map, color_width, false, token)?;
            }

            w.write_bits(0, 1)?; // transforms done
            write_image_stream(w, pixels, width as usize, true, token)?;
        }

        w.flush()?;
    }
    check_token(token)?;
    Ok(frame)
}

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
        // image-scaled after the source scan has finished. Keep the ordinary
        // no-token collect unchanged and poll only the caller-controlled path.
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
        Ok(pixels
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect())
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
                for pixel in data.chunks_exact(3) {
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
                for pixel in data.chunks_exact(4) {
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
                .chunks_exact(3)
                .map(|pixel| {
                    0xff00_0000
                        | (u32::from(pixel[0]) << 16)
                        | (u32::from(pixel[1]) << 8)
                        | u32::from(pixel[2])
                })
                .collect(),
            ColorType::Rgba8 => data
                .chunks_exact(4)
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
fn encode_frame(
    data: &[u8],
    width: u32,
    height: u32,
    color: ColorType,
    token: Option<&crate::CancellationToken>,
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
    let palette_size = (palette.len() <= 256).then_some(palette.len());
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
            NoopBitWriterCheckpoint,
        ),
    }
}

fn encode_alpha_stream<C: BitWriterCheckpoint>(
    palette_delta: &[u32],
    palette_len: usize,
    packed: &[u32],
    packed_width: usize,
    alpha: &[u8],
    token: Option<&crate::CancellationToken>,
    checkpoint: C,
) -> Result<Vec<u8>, EncodingError> {
    let mut encoded = Vec::new();
    let mut writer = BitWriter {
        writer: &mut encoded,
        buffer: 0,
        nbits: 0,
        checkpoint,
    };
    writer.write_bits(1, 1)?; // transform present
    writer.write_bits(3, 2)?; // color-indexing transform
    writer.write_bits((palette_len - 1) as u64, 8)?;
    write_image_stream_configured(
        &mut writer,
        palette_delta,
        palette_len,
        false,
        3,
        20,
        0,
        token,
    )?;

    writer.write_bits(0, 1)?; // transforms done
    write_image_stream_configured(&mut writer, packed, packed_width, true, 5, 32, 2, token)?;
    writer.flush()?;

    let mut compressed = Vec::with_capacity(encoded.len() + 1);
    compressed.push(1); // lossless compression, no filtering, no preprocessing
    extend_bytes_with_checkpoint(&mut compressed, &encoded, token)?;

    let mut uncompressed = Vec::with_capacity(alpha.len() + 1);
    uncompressed.push(0); // no compression, no filtering, no preprocessing
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

fn collect_alpha_palette(
    alpha: &[u8],
    token: Option<&crate::CancellationToken>,
) -> Result<Vec<u8>, EncodingError> {
    if let Some(token) = token {
        let mut palette = std::collections::BTreeSet::new();
        let mut samples_until_checkpoint = WEBP_ALPHA_PALETTE_CHECKPOINT_PIXELS;
        for &value in alpha {
            palette.insert(value);
            samples_until_checkpoint = samples_until_checkpoint.saturating_sub(1);
            if samples_until_checkpoint == 0 {
                check_token(Some(token))?;
                samples_until_checkpoint = WEBP_ALPHA_PALETTE_CHECKPOINT_PIXELS;
            }
        }
        Ok(palette.into_iter().collect())
    } else {
        Ok(alpha
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect())
    }
}

// Every sorted palette suffix is non-empty by construction.
#[allow(clippy::unwrap_used)]
pub(crate) fn encode_alpha(
    alpha: &[u8],
    width: u32,
    height: u32,
    token: Option<&crate::CancellationToken>,
) -> Result<Vec<u8>, EncodingError> {
    check_token(token)?;
    assert_eq!(alpha.len(), width as usize * height as usize);

    let mut palette_values = collect_alpha_palette(alpha, token)?;
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
    let palette = palette_values
        .iter()
        .map(|&value| u32::from(value) << 8)
        .collect::<Vec<_>>();
    let mut palette_indices = [0u8; 256];
    for (index, &value) in palette_values.iter().enumerate() {
        if index.is_multiple_of(64) {
            check_token(token)?;
        }
        palette_indices[usize::from(value)] = index as u8;
    }
    let mut palette_delta = Vec::with_capacity(palette.len());
    let mut previous = 0u32;
    for (index, &pixel) in palette.iter().enumerate() {
        if index.is_multiple_of(64) {
            check_token(token)?;
        }
        let alpha = (pixel >> 24).wrapping_sub(previous >> 24) & 0xff;
        let red = ((pixel >> 16) & 0xff).wrapping_sub((previous >> 16) & 0xff) & 0xff;
        let green = ((pixel >> 8) & 0xff).wrapping_sub((previous >> 8) & 0xff) & 0xff;
        let blue = (pixel & 0xff).wrapping_sub(previous & 0xff) & 0xff;
        palette_delta.push(alpha << 24 | red << 16 | green << 8 | blue);
        previous = pixel;
    }

    let xbits = match palette.len() {
        0..=2 => 3,
        3..=4 => 2,
        5..=16 => 1,
        _ => 0,
    };
    let pixels_per_group = 1usize << xbits;
    let bits_per_pixel = 8 >> xbits;
    let packed_width = width.div_ceil(pixels_per_group as u32) as usize;
    let mut packed = Vec::with_capacity(packed_width * height as usize);
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
                    source_pixels_until_checkpoint = WEBP_ALPHA_PALETTE_PACKING_CHECKPOINT_PIXELS;
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

    match token {
        Some(token) => encode_alpha_stream(
            &palette_delta,
            palette.len(),
            &packed,
            packed_width,
            alpha,
            Some(token),
            TokenBitWriterCheckpoint {
                token,
                written_bits: 0,
                output_bytes: 0,
            },
        ),
        None => encode_alpha_stream(
            &palette_delta,
            palette.len(),
            &packed,
            packed_width,
            alpha,
            None,
            NoopBitWriterCheckpoint,
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
pub struct WebPEncoder;

impl WebPEncoder {
    /// Create a new in-memory lossless encoder.
    ///
    /// Only supports "VP8L" lossless encoding.
    pub const fn new() -> Self {
        Self
    }

    /// Encode image data while polling an optional cooperative work token.
    pub(crate) fn encode_with_token(
        self,
        data: &[u8],
        width: u32,
        height: u32,
        color: ColorType,
        token: Option<&crate::CancellationToken>,
    ) -> Result<Vec<u8>, EncodingError> {
        let frame = encode_frame(data, width, height, color, token)?;
        check_token(token)?;

        let mut output = Vec::with_capacity(frame.len().saturating_add(20));
        output.extend_from_slice(b"RIFF");
        output.extend_from_slice(&(chunk_size(frame.len()) + 4).to_le_bytes());
        output.extend_from_slice(b"WEBP");
        write_chunk(&mut output, b"VP8L", &frame, token)?;
        check_token(token)?;
        Ok(output)
    }
}

#[cfg(coverage)]
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
    let mut odd_chunk = Vec::new();
    let _ = write_chunk(&mut odd_chunk, b"ODD!", &[1, 2, 3], None);
    let mut even_chunk = Vec::new();
    let _ = write_chunk(&mut even_chunk, b"EVEN", &[1, 2, 3, 4], None);

    let mut tree_bytes = Vec::new();
    let mut tree_writer = BitWriter {
        writer: &mut tree_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint,
    };
    let mut lengths = vec![0; 4];
    let mut codes = vec![0; 4];
    let mut huffman_tokens = Vec::new();
    let _ = write_huffman_tree(
        &mut tree_writer,
        &[1, 0, 0, 0],
        &mut lengths,
        &mut codes,
        &mut huffman_tokens,
        None,
    );
    let _ = tree_writer.flush();

    let mut trimmed_tree_bytes = Vec::new();
    let mut trimmed_tree_writer = BitWriter {
        writer: &mut trimmed_tree_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint,
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
        &mut huffman_tokens,
        None,
    );
    let _ = trimmed_tree_writer.flush();

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
        checkpoint: NoopBitWriterCheckpoint,
    };
    let mut group = GroupCodes::default();
    let _ = write_group(
        &mut group_writer,
        &populations,
        &mut group,
        &mut huffman_tokens,
        None,
    );
    let _ = group_writer.flush();

    let mut token_bytes = Vec::new();
    let mut token_writer = BitWriter {
        writer: &mut token_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint,
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
    let entropy_pixels = [0xff10_2010, 0xff20_4020, 0xff30_6030, 0xff40_8040];
    let _ = analyze_entropy(&entropy_pixels, 2, 2, None, 1, None);
    let mut palette_bytes = Vec::new();
    let mut palette_writer = BitWriter {
        writer: &mut palette_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint,
    };
    let mut palette = (0..18)
        .map(|index| 0xff00_0000 | ((index as u32) << 16))
        .collect::<Vec<_>>();
    palette.push(0);
    let _ = apply_palette(
        &mut palette_writer,
        &[0xff00_0000, 0xff01_0000, 0xff02_0000, 0xff03_0000],
        2,
        2,
        palette,
        None,
    );
    let _ = palette_writer.flush();

    let mut palette_trim_bytes = Vec::new();
    let mut palette_trim_writer = BitWriter {
        writer: &mut palette_trim_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint,
    };
    let _ = apply_palette(&mut palette_trim_writer, &[0; 4], 2, 2, vec![0; 18], None);
    let _ = palette_trim_writer.flush();

    let mut palette4_bytes = Vec::new();
    let mut palette4_writer = BitWriter {
        writer: &mut palette4_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint,
    };
    let _ = apply_palette(
        &mut palette4_writer,
        &[0xff00_0000, 0xff01_0000, 0xff02_0000, 0xff03_0000],
        2,
        2,
        vec![0xff00_0000, 0xff01_0000, 0xff02_0000, 0xff03_0000],
        None,
    );
    let _ = palette4_writer.flush();

    let mut palette16_bytes = Vec::new();
    let mut palette16_writer = BitWriter {
        writer: &mut palette16_bytes,
        buffer: 0,
        nbits: 0,
        checkpoint: NoopBitWriterCheckpoint,
    };
    let palette16 = (0..16)
        .map(|index| 0xff00_0000 | ((index as u32) << 16))
        .collect::<Vec<_>>();
    let _ = apply_palette(
        &mut palette16_writer,
        &[0xff00_0000, 0xff01_0000, 0xff02_0000, 0xff03_0000],
        2,
        2,
        palette16,
        None,
    );
    let _ = palette16_writer.flush();

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

    WebPEncoder::new()
        .encode_with_token(&[], 0, 1, ColorType::Rgb8, None)
        .expect_err("zero-width WebP must be rejected");
    WebPEncoder::new()
        .encode_with_token(&[], 1, 0, ColorType::Rgb8, None)
        .expect_err("zero-height WebP must be rejected");
    WebPEncoder::new()
        .encode_with_token(&vec![0; 16_385 * 3], 16_385, 1, ColorType::Rgb8, None)
        .expect_err("too-wide WebP must be rejected");
    WebPEncoder::new()
        .encode_with_token(&vec![0; 16_385 * 3], 1, 16_385, ColorType::Rgb8, None)
        .expect_err("too-tall WebP must be rejected");

    let rgb = [0, 0, 0];
    WebPEncoder::new()
        .encode_with_token(&rgb, 1, 1, ColorType::Rgb8, None)
        .expect("one-pixel in-memory WebP must encode");
}
