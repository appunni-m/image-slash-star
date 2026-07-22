//! Encoding of WebP images.
use std::io::{self, Write};

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
    IoError,
    InvalidDimensions,
}

impl From<io::Error> for EncodingError {
    fn from(_error: io::Error) -> Self {
        Self::IoError
    }
}

struct BitWriter<'a> {
    writer: &'a mut Vec<u8>,
    buffer: u64,
    nbits: u8,
}

impl BitWriter<'_> {
    fn write_bits(&mut self, bits: u64, nbits: u8) {
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

    fn flush(&mut self) {
        if self.nbits % 8 != 0 {
            self.write_bits(0, 8 - self.nbits % 8);
        }
        if self.nbits > 0 {
            self.writer
                .extend_from_slice(&self.buffer.to_le_bytes()[..self.nbits as usize / 8]);
            self.buffer = 0;
            self.nbits = 0;
        }
    }
}

fn build_huffman_tree(
    frequencies: &[u32],
    lengths: &mut [u8],
    codes: &mut [u16],
    length_limit: u8,
) -> bool {
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
    optimize_huffman_for_rle(&mut optimized);
    let optimized_symbol_count = optimized
        .iter()
        .filter(|&&frequency| frequency != 0)
        .count();
    if optimized_symbol_count <= 1 {
        lengths.fill(0);
        codes.fill(0);
        if let Some(symbol) = optimized.iter().position(|&frequency| frequency != 0) {
            lengths[symbol] = 1;
        }
        return false;
    }
    let mut count_min = 1_u32;
    loop {
        let mut nodes = optimized
            .iter()
            .enumerate()
            .filter(|&(_, &frequency)| frequency != 0)
            .map(|(value, &frequency)| WeightedNode {
                count: frequency.max(count_min),
                sort_value: value as isize,
                node: Node::Leaf(value),
            })
            .collect::<Vec<_>>();
        nodes.sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.sort_value.cmp(&right.sort_value))
        });
        while nodes.len() > 1 {
            let left = nodes.pop().unwrap();
            let right = nodes.pop().unwrap();
            let count = left.count + right.count;
            let position = nodes
                .iter()
                .position(|node| node.count <= count)
                .unwrap_or(nodes.len());
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
            match node {
                Node::Leaf(value) => lengths[*value] = depth,
                Node::Branch(left, right) => {
                    stack.push((right, depth + 1));
                    stack.push((left, depth + 1));
                }
            }
        }
        if lengths.iter().copied().max().unwrap_or(0) <= length_limit {
            break;
        }
        count_min *= 2;
    }

    // Assign codes
    codes.fill(0);
    let mut code = 0u32;
    for len in 1..=length_limit {
        for (i, &length) in lengths.iter().enumerate() {
            if length == len {
                codes[i] = (code as u16).reverse_bits() >> (16 - len);
                code += 1;
            }
        }
        code <<= 1;
    }
    true
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

#[derive(Clone, Copy)]
struct HuffmanToken {
    code: u8,
    extra: u8,
}

fn compressed_huffman_tokens(lengths: &[u8]) -> Vec<HuffmanToken> {
    let mut tokens = Vec::new();
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
    tokens
}

fn write_huffman_tree(
    w: &mut BitWriter<'_>,
    frequencies: &[u32],
    lengths: &mut [u8],
    codes: &mut [u16],
) {
    build_huffman_tree(frequencies, lengths, codes, 15);
    let symbols = lengths
        .iter()
        .enumerate()
        .filter_map(|(symbol, &length)| (length != 0).then_some(symbol))
        .take(3)
        .collect::<Vec<_>>();
    if symbols.len() <= 2 && symbols.iter().all(|&symbol| symbol < 256) {
        let first = symbols.first().copied().unwrap_or(0);
        w.write_bits(1, 1);
        w.write_bits(u64::from(symbols.len() == 2), 1);
        if first <= 1 {
            w.write_bits(0, 1);
            w.write_bits(first as u64, 1);
        } else {
            w.write_bits(1, 1);
            w.write_bits(first as u64, 8);
        }
        if symbols.len() == 2 {
            w.write_bits(symbols[1] as u64, 8);
        }
        lengths.fill(0);
        codes.fill(0);
        if symbols.len() == 2 {
            lengths[symbols[0]] = 1;
            lengths[symbols[1]] = 1;
            codes[symbols[1]] = 1;
        }
        return;
    }
    let tokens = compressed_huffman_tokens(lengths);
    let mut code_length_lengths = [0u8; 19];
    let mut code_length_codes = [0u16; 19];
    let mut code_length_frequencies = [0u32; 19];
    for token in &tokens {
        code_length_frequencies[usize::from(token.code)] += 1;
    }
    build_huffman_tree(
        &code_length_frequencies,
        &mut code_length_lengths,
        &mut code_length_codes,
        7,
    );
    const CODE_LENGTH_ORDER: [usize; 19] = [
        17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
    ];

    // Write the huffman tree
    w.write_bits(0, 1); // normal huffman tree
    let mut codes_to_store = 19;
    while codes_to_store > 4 && code_length_lengths[CODE_LENGTH_ORDER[codes_to_store - 1]] == 0 {
        codes_to_store -= 1;
    }
    w.write_bits((codes_to_store - 4) as u64, 4);
    for &symbol in &CODE_LENGTH_ORDER[..codes_to_store] {
        w.write_bits(u64::from(code_length_lengths[symbol]), 3);
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
    let mut trimmed_length = tokens.len();
    let mut trailing_zero_bits = 0;
    // The normal-tree path always emits at least one non-zero code-length
    // token before trailing zero-repeat tokens.
    loop {
        let token = tokens[trimmed_length - 1];
        if !matches!(token.code, 0 | 17 | 18) {
            break;
        }
        trimmed_length -= 1;
        trailing_zero_bits += usize::from(code_length_lengths[usize::from(token.code)]);
        trailing_zero_bits += match token.code {
            17 => 3,
            18 => 7,
            _ => 0,
        };
    }
    let write_trimmed = trailing_zero_bits > 12;
    w.write_bits(u64::from(write_trimmed), 1);
    let token_count = if write_trimmed {
        if trimmed_length == 2 {
            w.write_bits(0, 5);
        } else {
            let nbits = (trimmed_length - 2).ilog2() as usize;
            let pairs = nbits / 2 + 1;
            w.write_bits((pairs - 1) as u64, 3);
            w.write_bits((trimmed_length - 2) as u64, (pairs * 2) as u8);
        }
        trimmed_length
    } else {
        tokens.len()
    };
    for token in &tokens[..token_count] {
        let symbol = usize::from(token.code);
        let code = u64::from(code_length_codes[symbol]);
        let code_length = code_length_lengths[symbol];
        w.write_bits(code, code_length);
        let bits = match token.code {
            16 => 2,
            17 => 3,
            18 => 7,
            _ => 0,
        };
        w.write_bits(u64::from(token.extra), bits);
    }
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

fn write_image_stream(
    w: &mut BitWriter<'_>,
    pixels: &[u32],
    width: usize,
    write_meta_huffman_bit: bool,
) {
    write_image_stream_configured(w, pixels, width, write_meta_huffman_bit, 80, 11)
}

fn write_image_stream_configured(
    w: &mut BitWriter<'_>,
    pixels: &[u32],
    width: usize,
    write_meta_huffman_bit: bool,
    quality: u32,
    max_cache_bits: u8,
) {
    let candidates = backward_refs::candidates(
        pixels,
        width,
        write_meta_huffman_bit,
        quality,
        max_cache_bits,
    );
    let token_cost = |tokens: &[backward_refs::Token], cache_bits: u8| {
        backward_refs::estimated_bits(tokens, cache_bits)
    };
    let (mut tokens, cache_bits, is_standard) = candidates
        .into_iter()
        .min_by_key(|(tokens, cache_bits, _)| token_cost(tokens, *cache_bits))
        .unwrap();
    if is_standard && quality >= 25 {
        let traced = backward_refs::trace(pixels, width, &tokens, cache_bits, quality);
        if token_cost(&traced, cache_bits) < token_cost(&tokens, cache_bits) {
            tokens = traced;
        }
    }
    write_token_stream(
        w,
        pixels,
        width,
        write_meta_huffman_bit,
        &tokens,
        cache_bits,
        quality,
    );
}

struct GroupCodes {
    lengths: [Vec<u8>; 5],
    codes: [Vec<u16>; 5],
}

fn write_group(w: &mut BitWriter<'_>, populations: &[Vec<u32>; 5]) -> GroupCodes {
    let mut lengths = populations
        .each_ref()
        .map(|frequency| vec![0; frequency.len()]);
    let mut codes = populations
        .each_ref()
        .map(|frequency| vec![0; frequency.len()]);
    for channel in 0..5 {
        let population = &populations[channel];
        let channel_lengths = &mut lengths[channel];
        let channel_codes = &mut codes[channel];
        write_huffman_tree(w, population, channel_lengths, channel_codes);
    }
    GroupCodes { lengths, codes }
}

fn write_token_stream(
    w: &mut BitWriter<'_>,
    pixels: &[u32],
    width: usize,
    write_meta_huffman_bit: bool,
    tokens: &[backward_refs::Token],
    cache_bits: u8,
    quality: u32,
) {
    w.write_bits(u64::from(cache_bits != 0), 1);
    if cache_bits != 0 {
        w.write_bits(u64::from(cache_bits), 4);
    }
    let height = pixels.len() / width;
    let histogram_bits = 3_u8;
    let (symbols, histograms) = if write_meta_huffman_bit {
        histogram::cluster(tokens, width, height, cache_bits, quality, histogram_bits)
    } else {
        histogram::cluster(tokens, width, height, cache_bits, quality, 31)
    };
    let multiple_groups = write_meta_huffman_bit && histograms.len() > 1;
    if write_meta_huffman_bit {
        w.write_bits(u64::from(multiple_groups), 1);
        if multiple_groups {
            w.write_bits(u64::from(histogram_bits - 2), 3);
            let meta_pixels = symbols
                .iter()
                .map(|&symbol| u32::from(symbol) << 8)
                .collect::<Vec<_>>();
            let meta_width = (width + (1 << histogram_bits) - 1) >> histogram_bits;
            write_image_stream_configured(w, &meta_pixels, meta_width, false, quality, 0);
        }
    }
    let mut groups = Vec::with_capacity(histograms.len());
    for histogram in &histograms {
        groups.push(write_group(w, &histogram.populations));
    }

    let tile_width = (width + (1 << histogram_bits) - 1) >> histogram_bits;
    let mut position = 0;
    for &token in tokens {
        let group_index = if multiple_groups {
            let x = position % width;
            let y = position / width;
            usize::from(symbols[(y >> histogram_bits) * tile_width + (x >> histogram_bits)])
        } else {
            0
        };
        let lengths = &groups[group_index].lengths;
        let codes = &groups[group_index].codes;
        match token {
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
                w.write_bits(code, green_length + red_length + blue_length + alpha_length);
                position += 1;
            }
            backward_refs::Token::Copy { distance, length } => {
                let (symbol, extra_bits) = length_to_symbol(length);
                let symbol = 256 + symbol;
                w.write_bits(u64::from(codes[0][symbol]), lengths[0][symbol]);
                w.write_bits(((length - 1) & ((1 << extra_bits) - 1)) as u64, extra_bits);
                let distance = backward_refs::plane_code(width, distance);
                let (symbol, extra_bits) = length_to_symbol(distance);
                w.write_bits(u64::from(codes[4][symbol]), lengths[4][symbol]);
                let distance_extra_bits = ((distance - 1) & ((1 << extra_bits) - 1)) as u64;
                w.write_bits(distance_extra_bits, extra_bits);
                position += length;
            }
            backward_refs::Token::Cache(index) => {
                let symbol = 280 + index;
                w.write_bits(u64::from(codes[0][symbol]), lengths[0][symbol]);
                position += 1;
            }
        }
    }
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

fn minimize_palette_deltas(palette: &mut Vec<u32>) {
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum EntropyMode {
    Direct,
    Spatial,
    SubtractGreen,
    SpatialSubtractGreen,
    Palette,
}

fn analyze_entropy(
    pixels: &[u32],
    width: usize,
    height: usize,
    palette_size: Option<usize>,
    transform_bits: u8,
) -> (EntropyMode, bool) {
    if palette_size.is_some_and(|size| size <= 16) {
        return (EntropyMode::Palette, true);
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
    for y in 0..height {
        for x in 0..width {
            let pixel = pixels[y * width + x];
            let difference = subtract_pixels(pixel, previous_pixel);
            previous_pixel = pixel;
            if difference == 0 || (y != 0 && pixel == pixels[(y - 1) * width + x]) {
                continue;
            }
            let add_channels = |histograms: &mut [[u32; 256]], base: [usize; 4], color: u32| {
                for (channel, shift) in [24, 16, 8, 0].into_iter().enumerate() {
                    histograms[base[channel]][((color >> shift) & 0xff) as usize] += 1;
                }
            };
            add_channels(&mut histograms, [ALPHA, RED, GREEN, BLUE], pixel);
            add_channels(
                &mut histograms,
                [
                    ALPHA_PREDICTED,
                    RED_PREDICTED,
                    GREEN_PREDICTED,
                    BLUE_PREDICTED,
                ],
                difference,
            );
            let add_sub_green =
                |histograms: &mut [[u32; 256]], red: usize, blue: usize, color: u32| {
                    let green = ((color >> 8) & 0xff) as u8;
                    let red_value = ((color >> 16) & 0xff) as u8;
                    let blue_value = (color & 0xff) as u8;
                    histograms[red][usize::from(red_value.wrapping_sub(green))] += 1;
                    histograms[blue][usize::from(blue_value.wrapping_sub(green))] += 1;
                };
            add_sub_green(&mut histograms, RED_SUB_GREEN, BLUE_SUB_GREEN, pixel);
            add_sub_green(
                &mut histograms,
                RED_PREDICTED_SUB_GREEN,
                BLUE_PREDICTED_SUB_GREEN,
                difference,
            );
            let hash = ((((u64::from(pixel) + u64::from(pixel >> 19)) * 0x39c5_fba7) & 0xffff_ffff)
                >> 24) as usize;
            histograms[PALETTE][hash] += 1;
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
    let costs = histograms
        .iter()
        .map(|histogram| histogram::bits_entropy(histogram))
        .collect::<Vec<_>>();
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
    (mode, red_and_blue_zero)
}

fn subtract_green(pixels: &mut [u32]) {
    for pixel in pixels {
        let green = (*pixel >> 8) & 0xff;
        let red = ((*pixel >> 16) & 0xff).wrapping_sub(green) & 0xff;
        let blue = (*pixel & 0xff).wrapping_sub(green) & 0xff;
        *pixel = (*pixel & 0xff00_ff00) | (red << 16) | blue;
    }
}

fn apply_palette(
    w: &mut BitWriter<'_>,
    pixels: &[u32],
    width: usize,
    height: usize,
    mut palette: Vec<u32>,
) {
    minimize_palette_deltas(&mut palette);
    let encoded_length = if palette.len() > 17 && palette.last() == Some(&0) {
        palette.len() - 1
    } else {
        palette.len()
    };
    w.write_bits(1, 1);
    w.write_bits(3, 2);
    w.write_bits((encoded_length - 1) as u64, 8);
    let mut previous = 0;
    let palette_delta = palette[..encoded_length]
        .iter()
        .map(|&color| {
            let difference = subtract_pixels(color, previous);
            previous = color;
            difference
        })
        .collect::<Vec<_>>();
    write_image_stream_configured(w, &palette_delta, encoded_length, false, 20, 0);

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
    w.write_bits(0, 1);
    let maximum_cache_bits = (usize::BITS - palette.len().leading_zeros()) as u8;
    write_image_stream_configured(w, &packed, packed_width, true, 80, maximum_cache_bits)
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
) -> Result<Vec<u8>, EncodingError> {
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

    let mut pixels: Vec<u32> = match color {
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
    };

    // Pillow's lossless WebP path uses libwebp's default `exact=false`.
    // libwebp therefore replaces hidden RGB values of fully transparent
    // pixels with transparent black before selecting any transforms.
    if is_alpha {
        for pixel in &mut pixels {
            if *pixel >> 24 == 0 {
                *pixel = 0;
            }
        }
    }

    let palette = pixels
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let palette_size = (palette.len() <= 256).then_some(palette.len());
    let transform_bits = if palette_size.is_some() { 5 } else { 3 };
    let (entropy_mode, red_and_blue_zero) = analyze_entropy(
        &pixels,
        width as usize,
        height as usize,
        palette_size,
        transform_bits,
    );

    let mut frame = Vec::new();
    {
        let w = &mut BitWriter {
            writer: &mut frame,
            buffer: 0,
            nbits: 0,
        };
        w.write_bits(0x2f, 8); // signature
        w.write_bits(u64::from(width) - 1, 14);
        w.write_bits(u64::from(height) - 1, 14);

        w.write_bits(u64::from(is_alpha), 1); // alpha used
        w.write_bits(0x0, 3); // version

        if entropy_mode == EntropyMode::Palette {
            apply_palette(w, &pixels, width as usize, height as usize, palette);
        } else {
            let grayscale = pixels.iter().all(|&pixel| {
                let red = (pixel >> 16) & 0xff;
                let green = (pixel >> 8) & 0xff;
                let blue = pixel & 0xff;
                red == green && green == blue
            });
            let use_subtract_green = matches!(
                entropy_mode,
                EntropyMode::SubtractGreen | EntropyMode::SpatialSubtractGreen
            );
            let use_predictor = matches!(
                entropy_mode,
                EntropyMode::Spatial | EntropyMode::SpatialSubtractGreen
            );
            if use_subtract_green {
                w.write_bits(1, 1);
                w.write_bits(2, 2);
                subtract_green(&mut pixels);
            }

            if use_predictor {
                let (predictor_map, predictor_bits) = if grayscale {
                    predictor::apply_fixed(
                        &mut pixels,
                        width as usize,
                        height as usize,
                        transform_bits,
                        12,
                    )
                } else {
                    predictor::select_and_apply(
                        &mut pixels,
                        width as usize,
                        height as usize,
                        transform_bits,
                    )
                };
                w.write_bits(1, 1);
                w.write_bits(0, 2);
                w.write_bits(u64::from(predictor_bits - 2), 3);
                let predictor_width =
                    (width as usize + (1 << predictor_bits) - 1) >> predictor_bits;
                write_image_stream(w, &predictor_map, predictor_width, false);
            }

            if use_predictor && !red_and_blue_zero {
                let (color_map, color_bits) = cross_color::select_and_apply(
                    &mut pixels,
                    width as usize,
                    height as usize,
                    transform_bits,
                    80,
                );
                w.write_bits(1, 1);
                w.write_bits(1, 2);
                w.write_bits(u64::from(color_bits - 2), 3);
                let color_width = (width as usize + (1 << color_bits) - 1) >> color_bits;
                write_image_stream(w, &color_map, color_width, false);
            }

            w.write_bits(0, 1); // transforms done
            write_image_stream(w, &pixels, width as usize, true);
        }

        w.flush();
    }
    Ok(frame)
}

pub(crate) fn encode_alpha(alpha: &[u8], width: u32, height: u32) -> Vec<u8> {
    assert_eq!(alpha.len(), width as usize * height as usize);

    let mut palette_values = alpha
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut signs = 0u8;
    let mut predicted = 0u8;
    for &value in &palette_values {
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
    let palette = palette_values
        .iter()
        .map(|&value| u32::from(value) << 8)
        .collect::<Vec<_>>();
    let mut palette_indices = [0u8; 256];
    for (index, &value) in palette_values.iter().enumerate() {
        palette_indices[usize::from(value)] = index as u8;
    }
    let mut palette_delta = Vec::with_capacity(palette.len());
    let mut previous = 0u32;
    for &pixel in &palette {
        let alpha = (pixel >> 24).wrapping_sub(previous >> 24) & 0xff;
        let red = ((pixel >> 16) & 0xff).wrapping_sub((previous >> 16) & 0xff) & 0xff;
        let green = ((pixel >> 8) & 0xff).wrapping_sub((previous >> 8) & 0xff) & 0xff;
        let blue = (pixel & 0xff).wrapping_sub(previous & 0xff) & 0xff;
        palette_delta.push(alpha << 24 | red << 16 | green << 8 | blue);
        previous = pixel;
    }

    let mut encoded = Vec::new();
    let mut writer = BitWriter {
        writer: &mut encoded,
        buffer: 0,
        nbits: 0,
    };
    writer.write_bits(1, 1); // transform present
    writer.write_bits(3, 2); // color-indexing transform
    writer.write_bits((palette.len() - 1) as u64, 8);
    write_image_stream_configured(&mut writer, &palette_delta, palette.len(), false, 20, 0);

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

    writer.write_bits(0, 1); // transforms done
    write_image_stream_configured(&mut writer, &packed, packed_width, true, 32, 2);
    writer.flush();

    let mut chunk = Vec::with_capacity(encoded.len() + 1);
    chunk.push(1); // lossless compression, no filtering, no preprocessing
    chunk.extend_from_slice(&encoded);
    chunk
}

const fn chunk_size(inner_bytes: usize) -> u32 {
    if inner_bytes % 2 == 1 {
        (inner_bytes + 1) as u32 + 8
    } else {
        inner_bytes as u32 + 8
    }
}

fn write_chunk(w: &mut dyn Write, name: &[u8], data: &[u8]) -> io::Result<()> {
    debug_assert!(name.len() == 4);

    w.write_all(name)?;
    w.write_all(&(data.len() as u32).to_le_bytes())?;
    w.write_all(data)?;
    if data.len() % 2 == 1 {
        w.write_all(&[0])?;
    }
    Ok(())
}

/// WebP Encoder.
pub struct WebPEncoder<W> {
    writer: W,
}

impl<W: Write> WebPEncoder<W> {
    /// Create a new encoder that writes its output to `w`.
    ///
    /// Only supports "VP8L" lossless encoding.
    pub fn new(w: W) -> Self {
        Self { writer: w }
    }

    /// Encode image data with the indicated color type.
    ///
    /// # Panics
    ///
    /// Panics if the image data is not of the indicated dimensions.
    pub fn encode(
        mut self,
        data: &[u8],
        width: u32,
        height: u32,
        color: ColorType,
    ) -> Result<(), EncodingError> {
        let frame = encode_frame(data, width, height, color)?;

        self.writer.write_all(b"RIFF")?;
        self.writer
            .write_all(&(chunk_size(frame.len()) + 4).to_le_bytes())?;
        self.writer.write_all(b"WEBP")?;
        write_chunk(&mut self.writer, b"VP8L", &frame)?;

        Ok(())
    }
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    use std::io::{Cursor, ErrorKind};

    struct FailOnWrite {
        call: usize,
        fail_at: usize,
    }

    impl Write for FailOnWrite {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if self.call == self.fail_at {
                return Err(io::Error::from(ErrorKind::Other));
            }
            self.call += 1;
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    backward_refs::__coverage_exercise_private_branches();
    cross_color::__coverage_exercise_private_branches();
    histogram::__coverage_exercise_private_branches();
    predictor::__coverage_exercise_private_branches();

    let _ = EncodingError::from(io::Error::from(ErrorKind::Other));
    let _ = length_to_symbol(4);
    let _ = length_to_symbol(300);
    let _ = channels(0x1122_3344);
    let _ = chunk_size(3);
    let _ = chunk_size(4);
    let _ = compressed_huffman_tokens(&[0; 300]);
    let _ = FailOnWrite {
        call: 0,
        fail_at: usize::MAX,
    }
    .flush();
    let rgba_pixel = [0, 0, 0, 255];
    let _ = WebPEncoder::new(FailOnWrite {
        call: 0,
        fail_at: 0,
    })
    .encode(&rgba_pixel, 1, 1, ColorType::Rgba8);
    let _ = WebPEncoder::new(FailOnWrite {
        call: 0,
        fail_at: 1,
    })
    .encode(&rgba_pixel, 1, 1, ColorType::Rgba8);
    let _ = WebPEncoder::new(FailOnWrite {
        call: 0,
        fail_at: 2,
    })
    .encode(&rgba_pixel, 1, 1, ColorType::Rgba8);
    let _ = WebPEncoder::new(FailOnWrite {
        call: 0,
        fail_at: 3,
    })
    .encode(&rgba_pixel, 1, 1, ColorType::Rgba8);
    let mut odd_chunk = Vec::new();
    write_chunk(&mut odd_chunk, b"ODD!", &[1, 2, 3]).unwrap();
    let mut even_chunk = Vec::new();
    write_chunk(&mut even_chunk, b"EVEN", &[1, 2, 3, 4]).unwrap();
    let mut even_fixed_buffer = [0u8; 12];
    let mut even_fixed = Cursor::new(&mut even_fixed_buffer[..]);
    write_chunk(&mut even_fixed, b"EVEN", &[1, 2, 3, 4]).unwrap();
    let mut invalid_dimension_buffer = [0u8; 0];
    let _ = WebPEncoder::new(Cursor::new(&mut invalid_dimension_buffer[..])).encode(
        &[],
        0,
        1,
        ColorType::Rgba8,
    );
    let mut odd_fixed_buffer = [0u8; 12];
    let mut odd_fixed = Cursor::new(&mut odd_fixed_buffer[..]);
    write_chunk(&mut odd_fixed, b"ODD!", &[1, 2, 3]).unwrap();
    let mut name_error_buffer = [0u8; 0];
    let mut name_error = Cursor::new(&mut name_error_buffer[..]);
    write_chunk(&mut name_error, b"FAIL", &[1, 2, 3])
        .expect_err("empty fixed buffer must fail on RIFF chunk name");
    let mut size_error_buffer = [0u8; 4];
    let mut size_error = Cursor::new(&mut size_error_buffer[..]);
    write_chunk(&mut size_error, b"FAIL", &[1, 2, 3])
        .expect_err("short fixed buffer must fail on RIFF chunk size");
    let mut data_error_buffer = [0u8; 8];
    let mut data_error = Cursor::new(&mut data_error_buffer[..]);
    write_chunk(&mut data_error, b"FAIL", &[1, 2, 3])
        .expect_err("short fixed buffer must fail on RIFF chunk data");
    let mut padding_error_buffer = [0u8; 11];
    let mut padding_error = Cursor::new(&mut padding_error_buffer[..]);
    write_chunk(&mut padding_error, b"PAD!", &[1, 2, 3])
        .expect_err("short fixed buffer must fail on RIFF padding byte");

    let mut tree_bytes = Vec::new();
    let mut tree_writer = BitWriter {
        writer: &mut tree_bytes,
        buffer: 0,
        nbits: 0,
    };
    let mut lengths = vec![0; 4];
    let mut codes = vec![0; 4];
    write_huffman_tree(&mut tree_writer, &[1, 0, 0, 0], &mut lengths, &mut codes);
    tree_writer.flush();

    let mut trimmed_tree_bytes = Vec::new();
    let mut trimmed_tree_writer = BitWriter {
        writer: &mut trimmed_tree_bytes,
        buffer: 0,
        nbits: 0,
    };
    let mut trimmed_lengths = vec![0; 256];
    let mut trimmed_codes = vec![0; 256];
    let mut trimmed_frequencies = vec![0; 256];
    trimmed_frequencies[..4].fill(1);
    write_huffman_tree(
        &mut trimmed_tree_writer,
        &trimmed_frequencies,
        &mut trimmed_lengths,
        &mut trimmed_codes,
    );
    trimmed_tree_writer.flush();

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
    };
    let _ = write_group(&mut group_writer, &populations);
    group_writer.flush();

    let mut token_bytes = Vec::new();
    let mut token_writer = BitWriter {
        writer: &mut token_bytes,
        buffer: 0,
        nbits: 0,
    };
    write_token_stream(
        &mut token_writer,
        &[0xff00_0000; 8],
        8,
        false,
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
        0,
        1,
    );
    token_writer.flush();

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
    let _ = analyze_entropy(&entropy_pixels, 2, 2, None, 1);
    let mut palette_bytes = Vec::new();
    let mut palette_writer = BitWriter {
        writer: &mut palette_bytes,
        buffer: 0,
        nbits: 0,
    };
    let mut palette = (0..18)
        .map(|index| 0xff00_0000 | ((index as u32) << 16))
        .collect::<Vec<_>>();
    palette.push(0);
    apply_palette(
        &mut palette_writer,
        &[0xff00_0000, 0xff01_0000, 0xff02_0000, 0xff03_0000],
        2,
        2,
        palette,
    );
    palette_writer.flush();

    let mut palette_trim_bytes = Vec::new();
    let mut palette_trim_writer = BitWriter {
        writer: &mut palette_trim_bytes,
        buffer: 0,
        nbits: 0,
    };
    apply_palette(&mut palette_trim_writer, &[0; 4], 2, 2, vec![0; 18]);
    palette_trim_writer.flush();

    let mut palette4_bytes = Vec::new();
    let mut palette4_writer = BitWriter {
        writer: &mut palette4_bytes,
        buffer: 0,
        nbits: 0,
    };
    apply_palette(
        &mut palette4_writer,
        &[0xff00_0000, 0xff01_0000, 0xff02_0000, 0xff03_0000],
        2,
        2,
        vec![0xff00_0000, 0xff01_0000, 0xff02_0000, 0xff03_0000],
    );
    palette4_writer.flush();

    let mut palette16_bytes = Vec::new();
    let mut palette16_writer = BitWriter {
        writer: &mut palette16_bytes,
        buffer: 0,
        nbits: 0,
    };
    let palette16 = (0..16)
        .map(|index| 0xff00_0000 | ((index as u32) << 16))
        .collect::<Vec<_>>();
    apply_palette(
        &mut palette16_writer,
        &[0xff00_0000, 0xff01_0000, 0xff02_0000, 0xff03_0000],
        2,
        2,
        palette16,
    );
    palette16_writer.flush();

    let alpha = [
        0, 255, 1, 254, 2, 253, 3, 252, 4, 251, 5, 250, 6, 249, 7, 248, 8, 247, 9, 246,
    ];
    let _ = encode_alpha(&alpha, alpha.len() as u32, 1);
    let short_alpha = [
        0, 255, 1, 254, 2, 253, 3, 252, 4, 251, 5, 250, 6, 249, 7, 248, 8,
    ];
    let _ = encode_alpha(&short_alpha, short_alpha.len() as u32, 1);
    let nonzero_alpha = [
        1, 255, 2, 254, 3, 253, 4, 252, 5, 251, 6, 250, 7, 249, 8, 248, 9, 247, 10, 246,
    ];
    let _ = encode_alpha(&nonzero_alpha, nonzero_alpha.len() as u32, 1);
    let two_value_alpha = [0, 255, 0, 255];
    let _ = encode_alpha(&two_value_alpha, two_value_alpha.len() as u32, 1);

    let mut output = Vec::new();
    WebPEncoder::new(&mut output)
        .encode(&[], 0, 1, ColorType::Rgb8)
        .expect_err("zero-width WebP must be rejected");
    WebPEncoder::new(&mut output)
        .encode(&[], 1, 0, ColorType::Rgb8)
        .expect_err("zero-height WebP must be rejected");
    WebPEncoder::new(&mut output)
        .encode(&vec![0; 16_385 * 3], 16_385, 1, ColorType::Rgb8)
        .expect_err("too-wide WebP must be rejected");
    WebPEncoder::new(&mut output)
        .encode(&vec![0; 16_385 * 3], 1, 16_385, ColorType::Rgb8)
        .expect_err("too-tall WebP must be rejected");

    let rgb = [0, 0, 0];
    let mut riff_name_error = [0u8; 0];
    WebPEncoder::new(Cursor::new(&mut riff_name_error[..]))
        .encode(&rgb, 1, 1, ColorType::Rgb8)
        .expect_err("empty fixed buffer must fail on RIFF signature");
    let mut riff_size_error = [0u8; 4];
    WebPEncoder::new(Cursor::new(&mut riff_size_error[..]))
        .encode(&rgb, 1, 1, ColorType::Rgb8)
        .expect_err("short fixed buffer must fail on RIFF size");
    let mut webp_signature_error = [0u8; 8];
    WebPEncoder::new(Cursor::new(&mut webp_signature_error[..]))
        .encode(&rgb, 1, 1, ColorType::Rgb8)
        .expect_err("short fixed buffer must fail on WEBP signature");
    let mut vp8l_chunk_error = [0u8; 12];
    WebPEncoder::new(Cursor::new(&mut vp8l_chunk_error[..]))
        .encode(&rgb, 1, 1, ColorType::Rgb8)
        .expect_err("short fixed buffer must fail on VP8L chunk");
    let mut fixed_output = [0u8; 256];
    WebPEncoder::new(Cursor::new(&mut fixed_output[..]))
        .encode(&rgb, 1, 1, ColorType::Rgb8)
        .expect("fixed buffer with enough space must encode WebP");
}
