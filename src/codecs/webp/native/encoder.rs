//! Encoding of WebP images.
use std::io::{self, Write};

mod backward_refs;
pub(super) mod cross_color;
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

struct BitWriter<W> {
    writer: W,
    buffer: u64,
    nbits: u8,
}

impl<W: Write> BitWriter<W> {
    fn write_bits(&mut self, bits: u64, nbits: u8) -> io::Result<()> {
        debug_assert!(nbits <= 64);

        self.buffer |= bits << self.nbits;
        self.nbits += nbits;

        if self.nbits >= 64 {
            self.writer.write_all(&self.buffer.to_le_bytes())?;
            self.nbits -= 64;
            self.buffer = bits.checked_shr(u32::from(nbits - self.nbits)).unwrap_or(0);
        }
        debug_assert!(self.nbits < 64);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.nbits % 8 != 0 {
            self.write_bits(0, 8 - self.nbits % 8)?;
        }
        if self.nbits > 0 {
            self.writer
                .write_all(&self.buffer.to_le_bytes()[..self.nbits as usize / 8])
                .unwrap();
            self.buffer = 0;
            self.nbits = 0;
        }
        Ok(())
    }
}

fn write_single_entry_huffman_tree<W: Write>(w: &mut BitWriter<W>, symbol: u8) -> io::Result<()> {
    w.write_bits(1, 2)?;
    if symbol <= 1 {
        w.write_bits(0, 1)?;
        w.write_bits(u64::from(symbol), 1)?;
    } else {
        w.write_bits(1, 1)?;
        w.write_bits(u64::from(symbol), 8)?;
    }
    Ok(())
}

fn build_huffman_tree(
    frequencies: &[u32],
    lengths: &mut [u8],
    codes: &mut [u16],
    length_limit: u8,
) -> bool {
    assert_eq!(frequencies.len(), lengths.len());
    assert_eq!(frequencies.len(), codes.len());

    if frequencies.iter().filter(|&&f| f > 0).count() <= 1 {
        lengths.fill(0);
        codes.fill(0);
        return false;
    }

    #[derive(Clone)]
    enum Node {
        Leaf(usize),
        Branch(Box<Node>, Box<Node>),
    }
    #[derive(Clone)]
    struct WeightedNode {
        count: u32,
        sort_value: usize,
        node: Node,
    }

    let mut optimized = frequencies.to_vec();
    optimize_huffman_for_rle(&mut optimized);
    let mut count_min = 1_u32;
    loop {
        let mut nodes = optimized
            .iter()
            .enumerate()
            .filter(|&(_, &frequency)| frequency != 0)
            .map(|(value, &frequency)| WeightedNode {
                count: frequency.max(count_min),
                sort_value: value,
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
                    sort_value: usize::MAX,
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
            while repetitions != 0 {
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

fn write_huffman_tree<W: Write>(
    w: &mut BitWriter<W>,
    frequencies: &[u32],
    lengths: &mut [u8],
    codes: &mut [u16],
) -> io::Result<()> {
    let symbols = frequencies
        .iter()
        .enumerate()
        .filter_map(|(symbol, &frequency)| (frequency != 0).then_some(symbol))
        .take(3)
        .collect::<Vec<_>>();
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
            lengths.fill(0);
            codes.fill(0);
            lengths[symbols[0]] = 1;
            lengths[symbols[1]] = 1;
            codes[symbols[1]] = 1;
        }
        return Ok(());
    }
    if !build_huffman_tree(frequencies, lengths, codes, 15) {
        let symbol = frequencies
            .iter()
            .position(|&frequency| frequency > 0)
            .unwrap_or(0);
        return write_single_entry_huffman_tree(w, symbol as u8);
    }
    let tokens = compressed_huffman_tokens(lengths);
    let mut code_length_lengths = [0u8; 19];
    let mut code_length_codes = [0u16; 19];
    let mut code_length_frequencies = [0u32; 19];
    for token in &tokens {
        code_length_frequencies[usize::from(token.code)] += 1;
    }
    let code_length_tree_is_nontrivial = build_huffman_tree(
        &code_length_frequencies,
        &mut code_length_lengths,
        &mut code_length_codes,
        7,
    );
    if !code_length_tree_is_nontrivial {
        let symbol = code_length_frequencies
            .iter()
            .position(|&frequency| frequency != 0)
            .unwrap_or(0);
        code_length_lengths[symbol] = 1;
    }
    const CODE_LENGTH_ORDER: [usize; 19] = [
        17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
    ];

    // Write the huffman tree
    w.write_bits(0, 1)?; // normal huffman tree
    let mut codes_to_store = 19;
    while codes_to_store > 4 && code_length_lengths[CODE_LENGTH_ORDER[codes_to_store - 1]] == 0 {
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
    let mut trimmed_length = tokens.len();
    let mut trailing_zero_bits = 0;
    while trimmed_length > 0 {
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
    let write_trimmed = trimmed_length > 1 && trailing_zero_bits > 12;
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
        tokens.len()
    };
    for token in &tokens[..token_count] {
        let symbol = usize::from(token.code);
        w.write_bits(
            u64::from(code_length_codes[symbol]),
            code_length_lengths[symbol],
        )?;
        let bits = match token.code {
            16 => 2,
            17 => 3,
            18 => 7,
            _ => 0,
        };
        w.write_bits(u64::from(token.extra), bits)?;
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

fn write_image_stream<W: Write>(
    w: &mut BitWriter<W>,
    pixels: &[u32],
    width: usize,
    write_meta_huffman_bit: bool,
) -> io::Result<()> {
    write_image_stream_configured(w, pixels, width, write_meta_huffman_bit, 80, 11)
}

fn write_image_stream_configured<W: Write>(
    w: &mut BitWriter<W>,
    pixels: &[u32],
    width: usize,
    write_meta_huffman_bit: bool,
    quality: u32,
    max_cache_bits: u8,
) -> io::Result<()> {
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
    let (mut tokens, cache_bits) = candidates
        .into_iter()
        .min_by_key(|(tokens, cache_bits)| token_cost(tokens, *cache_bits))
        .unwrap();
    if write_meta_huffman_bit && quality >= 25 {
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
    )
}

fn write_token_stream<W: Write>(
    w: &mut BitWriter<W>,
    _pixels: &[u32],
    width: usize,
    write_meta_huffman_bit: bool,
    tokens: &[backward_refs::Token],
    cache_bits: u8,
) -> io::Result<()> {
    w.write_bits(u64::from(cache_bits != 0), 1)?;
    if cache_bits != 0 {
        w.write_bits(u64::from(cache_bits), 4)?;
    }
    if write_meta_huffman_bit {
        w.write_bits(0, 1)?; // one global Huffman group
    }

    let mut frequencies0 = [0_u32; 256];
    let cache_size = if cache_bits == 0 { 0 } else { 1 << cache_bits };
    let mut frequencies1 = vec![0_u32; 280 + cache_size];
    let mut frequencies2 = [0_u32; 256];
    let mut frequencies3 = [0_u32; 256];
    let mut frequencies4 = [0_u32; 40];
    for &token in tokens {
        match token {
            backward_refs::Token::Literal(pixel) => {
                let [red, green, blue, alpha] = channels(pixel);
                frequencies0[red] += 1;
                frequencies1[green] += 1;
                frequencies2[blue] += 1;
                frequencies3[alpha] += 1;
            }
            backward_refs::Token::Copy { distance, length } => {
                let (symbol, _) = length_to_symbol(length);
                frequencies1[256 + symbol] += 1;
                let distance = backward_refs::plane_code(width, distance);
                let (symbol, _) = length_to_symbol(distance);
                frequencies4[symbol] += 1;
            }
            backward_refs::Token::Cache(index) => frequencies1[280 + index] += 1,
        }
    }

    let mut lengths0 = [0_u8; 256];
    let mut lengths1 = vec![0_u8; frequencies1.len()];
    let mut lengths2 = [0_u8; 256];
    let mut lengths3 = [0_u8; 256];
    let mut lengths4 = [0_u8; 40];
    let mut codes0 = [0_u16; 256];
    let mut codes1 = vec![0_u16; frequencies1.len()];
    let mut codes2 = [0_u16; 256];
    let mut codes3 = [0_u16; 256];
    let mut codes4 = [0_u16; 40];
    write_huffman_tree(w, &frequencies1, &mut lengths1, &mut codes1)?;
    write_huffman_tree(w, &frequencies0, &mut lengths0, &mut codes0)?;
    write_huffman_tree(w, &frequencies2, &mut lengths2, &mut codes2)?;
    write_huffman_tree(w, &frequencies3, &mut lengths3, &mut codes3)?;
    write_huffman_tree(w, &frequencies4, &mut lengths4, &mut codes4)?;

    for &token in tokens {
        match token {
            backward_refs::Token::Literal(pixel) => {
                let [red, green, blue, alpha] = channels(pixel);
                let green_length = lengths1[green];
                let red_length = lengths0[red];
                let blue_length = lengths2[blue];
                let alpha_length = lengths3[alpha];
                let code = u64::from(codes1[green])
                    | (u64::from(codes0[red]) << green_length)
                    | (u64::from(codes2[blue]) << (green_length + red_length))
                    | (u64::from(codes3[alpha]) << (green_length + red_length + blue_length));
                w.write_bits(code, green_length + red_length + blue_length + alpha_length)?;
            }
            backward_refs::Token::Copy { distance, length } => {
                let (symbol, extra_bits) = length_to_symbol(length);
                let symbol = 256 + symbol;
                w.write_bits(u64::from(codes1[symbol]), lengths1[symbol])?;
                w.write_bits(((length - 1) & ((1 << extra_bits) - 1)) as u64, extra_bits)?;
                let distance = backward_refs::plane_code(width, distance);
                let (symbol, extra_bits) = length_to_symbol(distance);
                w.write_bits(u64::from(codes4[symbol]), lengths4[symbol])?;
                w.write_bits(
                    ((distance - 1) & ((1 << extra_bits) - 1)) as u64,
                    extra_bits,
                )?;
            }
            backward_refs::Token::Cache(index) => {
                let symbol = 280 + index;
                w.write_bits(u64::from(codes1[symbol]), lengths1[symbol])?;
            }
        }
    }
    Ok(())
}

/// Encode image data with the indicated color type.
///
/// # Panics
///
/// Panics if the image data is not of the indicated dimensions.
fn encode_frame<W: Write>(
    writer: W,
    data: &[u8],
    width: u32,
    height: u32,
    color: ColorType,
) -> Result<(), EncodingError> {
    let w = &mut BitWriter {
        writer,
        buffer: 0,
        nbits: 0,
    };

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

    w.write_bits(0x2f, 8)?; // signature
    w.write_bits(u64::from(width) - 1, 14)?;
    w.write_bits(u64::from(height) - 1, 14)?;

    w.write_bits(u64::from(is_alpha), 1)?; // alpha used
    w.write_bits(0x0, 3)?; // version

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

    let grayscale = pixels.iter().all(|&pixel| {
        let red = (pixel >> 16) & 0xff;
        let green = (pixel >> 8) & 0xff;
        let blue = pixel & 0xff;
        red == green && green == blue
    });
    if grayscale {
        w.write_bits(1, 1)?;
        w.write_bits(2, 2)?;
        for pixel in &mut pixels {
            let alpha = *pixel & 0xff00_0000;
            let green = *pixel & 0x0000_ff00;
            *pixel = alpha | green;
        }
    }

    let (predictor_map, predictor_bits) = if grayscale {
        predictor::apply_fixed(&mut pixels, width as usize, height as usize, 3, 12)
    } else {
        predictor::select_and_apply(&mut pixels, width as usize, height as usize, 3)
    };
    w.write_bits(1, 1)?;
    w.write_bits(0, 2)?;
    w.write_bits(u64::from(predictor_bits - 2), 3)?;
    let predictor_width = (width as usize + (1 << predictor_bits) - 1) >> predictor_bits;
    write_image_stream(w, &predictor_map, predictor_width, false)?;

    if !grayscale {
        let (color_map, color_bits) =
            cross_color::select_and_apply(&mut pixels, width as usize, height as usize, 3, 80);
        w.write_bits(1, 1)?;
        w.write_bits(1, 2)?;
        w.write_bits(u64::from(color_bits - 2), 3)?;
        let color_width = (width as usize + (1 << color_bits) - 1) >> color_bits;
        write_image_stream(w, &color_map, color_width, false)?;
    }

    w.write_bits(0, 1)?; // transforms done
    write_image_stream(w, &pixels, width as usize, true)?;

    w.flush()?;
    Ok(())
}

pub(crate) fn encode_alpha(alpha: &[u8], width: u32, height: u32) -> io::Result<Vec<u8>> {
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
    writer.write_bits(1, 1)?; // transform present
    writer.write_bits(3, 2)?; // color-indexing transform
    writer.write_bits((palette.len() - 1) as u64, 8)?;
    write_image_stream_configured(&mut writer, &palette_delta, palette.len(), false, 20, 0)?;

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

    writer.write_bits(0, 1)?; // transforms done
    write_image_stream_configured(&mut writer, &packed, packed_width, true, 32, 2)?;
    writer.flush()?;

    let mut chunk = Vec::with_capacity(encoded.len() + 1);
    chunk.push(1); // lossless compression, no filtering, no preprocessing
    chunk.extend_from_slice(&encoded);
    Ok(chunk)
}

const fn chunk_size(inner_bytes: usize) -> u32 {
    if inner_bytes % 2 == 1 {
        (inner_bytes + 1) as u32 + 8
    } else {
        inner_bytes as u32 + 8
    }
}

fn write_chunk<W: Write>(mut w: W, name: &[u8], data: &[u8]) -> io::Result<()> {
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
        let mut frame = Vec::new();
        encode_frame(&mut frame, data, width, height, color)?;

        self.writer.write_all(b"RIFF")?;
        self.writer
            .write_all(&(chunk_size(frame.len()) + 4).to_le_bytes())?;
        self.writer.write_all(b"WEBP")?;
        write_chunk(&mut self.writer, b"VP8L", &frame)?;

        Ok(())
    }
}
