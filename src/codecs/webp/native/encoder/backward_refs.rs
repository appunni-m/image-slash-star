// Lossless WebP backward references, ported from libwebp 1.6.0
// `src/enc/backward_references_enc.c`.

const MIN_LENGTH: usize = 4;
const MAX_LENGTH: usize = 4096;
const WINDOW_SIZE: usize = (1 << 20) - 120;
const HASH_BITS: usize = 18;
const HASH_SIZE: usize = 1 << HASH_BITS;
const HASH_MULTIPLIER_HI: u32 = 0xc6a4_a793;
const HASH_MULTIPLIER_LO: u32 = 0x5bd1_e996;
const COLOR_HASH_MUL: u32 = 0x1e35_a7bd;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Token {
    Literal(u32),
    Copy { distance: usize, length: usize },
    Cache(usize),
}

fn pair_hash(pixels: &[u32], position: usize) -> usize {
    let key = pixels[position + 1]
        .wrapping_mul(HASH_MULTIPLIER_HI)
        .wrapping_add(pixels[position].wrapping_mul(HASH_MULTIPLIER_LO));
    (key >> (32 - HASH_BITS)) as usize
}

fn match_length(pixels: &[u32], first: usize, second: usize, limit: usize) -> usize {
    let mut length = 0;
    while length < limit && pixels[first + length] == pixels[second + length] {
        length += 1;
    }
    length
}

/// Builds the same best-distance/best-length table as `VP8LHashChainFill()` at
/// Pillow's lossless quality=80, method=4 settings.
fn fill_hash_chain(pixels: &[u32], width: usize) -> Vec<(usize, usize)> {
    let size = pixels.len();
    let mut result = vec![(0, 0); size];
    if size <= 2 {
        return result;
    }

    let mut first = vec![-1_i32; HASH_SIZE];
    let mut chain = vec![-1_i32; size];
    let mut position = 0;
    let mut equal_pair = pixels[0] == pixels[1];
    while position < size - 2 {
        let next_equal_pair = pixels[position + 1] == pixels[position + 2];
        if equal_pair && next_equal_pair {
            let color = pixels[position];
            let mut run = 1;
            while position + run + 2 < size && pixels[position + run + 2] == color {
                run += 1;
            }
            if run > MAX_LENGTH {
                position += run - MAX_LENGTH;
                run = MAX_LENGTH;
            }
            while run > 0 {
                let key = pixels[position + 1]
                    .wrapping_mul(HASH_MULTIPLIER_HI)
                    .wrapping_add(pixels[position].wrapping_mul(HASH_MULTIPLIER_LO));
                let hash = (key >> (32 - HASH_BITS)) as usize;
                chain[position] = first[hash];
                first[hash] = position as i32;
                position += 1;
                run -= 1;
            }
            equal_pair = false;
        } else {
            let hash = pair_hash(pixels, position);
            chain[position] = first[hash];
            first[hash] = position as i32;
            position += 1;
            equal_pair = next_equal_pair;
        }
    }
    chain[position] = first[pair_hash(pixels, position)];

    let iterations = 8 + 80 * 80 / 128;
    let mut base = size - 2;
    while base > 0 {
        let max_length = MAX_LENGTH.min(size - 1 - base);
        let mut remaining = iterations;
        let mut best_length = 0;
        let mut best_distance = 0;
        let minimum = base.saturating_sub(WINDOW_SIZE);

        if base >= width {
            let current = match_length(pixels, base - width, base, max_length);
            if current > best_length {
                best_length = current;
                best_distance = width;
            }
            remaining -= 1;
        }
        let current = match_length(pixels, base - 1, base, max_length);
        if current > best_length {
            best_length = current;
            best_distance = 1;
        }
        remaining -= 1;

        let mut candidate = chain[base];
        let good_enough = max_length.min(256);
        while candidate >= minimum as i32 && remaining > 1 && best_length < MAX_LENGTH {
            remaining -= 1;
            let candidate_index = candidate as usize;
            if pixels[candidate_index + best_length] == pixels[base + best_length] {
                let current = match_length(pixels, candidate_index, base, max_length);
                if current > best_length {
                    best_length = current;
                    best_distance = base - candidate_index;
                    if best_length >= good_enough {
                        break;
                    }
                }
            }
            candidate = chain[candidate_index];
        }

        let mut maximum_base = base;
        loop {
            result[base] = (best_distance, best_length);
            base -= 1;
            if best_distance == 0
                || base == 0
                || base < best_distance
                || pixels[base - best_distance] != pixels[base]
                || (best_length == MAX_LENGTH
                    && best_distance != 1
                    && base + MAX_LENGTH < maximum_base)
            {
                break;
            }
            if best_length < MAX_LENGTH {
                best_length += 1;
                maximum_base = base;
            }
        }
    }
    result
}

fn lz77(pixels: &[u32], width: usize, chain: &[(usize, usize)]) -> Vec<Token> {
    let mut refs = Vec::new();
    let mut position = 0;
    let mut last_check: isize = -1;
    while position < pixels.len() {
        let (distance, initial_length) = chain[position];
        let mut length = initial_length;
        if length >= MIN_LENGTH {
            let mut maximum_reach = 0;
            let maximum_check = (position + length).min(pixels.len() - 1);
            last_check = last_check.max(position as isize);
            for next in (last_check as usize + 1)..=maximum_check {
                let next_length = chain[next].1;
                let reach = next
                    + if next_length >= MIN_LENGTH {
                        next_length
                    } else {
                        1
                    };
                if reach > maximum_reach {
                    length = next - position;
                    maximum_reach = reach;
                    if maximum_reach >= pixels.len() {
                        break;
                    }
                }
            }
        } else {
            length = 1;
        }
        if length == 1 {
            refs.push(Token::Literal(pixels[position]));
        } else {
            refs.push(Token::Copy { distance, length });
        }
        position += length;
    }
    let _ = width;
    refs
}

fn color_hash(pixel: u32, bits: u8) -> usize {
    (pixel.wrapping_mul(COLOR_HASH_MUL) >> (32 - bits)) as usize
}

fn with_cache(pixels: &[u32], refs: &[Token], bits: u8) -> Vec<Token> {
    if bits == 0 {
        return refs.to_vec();
    }
    let mut cache = vec![0_u32; 1 << bits];
    let mut output = Vec::with_capacity(refs.len());
    let mut position = 0;
    for &token in refs {
        match token {
            Token::Literal(pixel) => {
                let key = color_hash(pixel, bits);
                if cache[key] == pixel {
                    output.push(Token::Cache(key));
                } else {
                    output.push(token);
                    cache[key] = pixel;
                }
                position += 1;
            }
            Token::Copy { length, .. } => {
                output.push(token);
                for &pixel in &pixels[position..position + length] {
                    let key = color_hash(pixel, bits);
                    cache[key] = pixel;
                }
                position += length;
            }
            Token::Cache(_) => unreachable!(),
        }
    }
    output
}

fn prefix(value: usize) -> (usize, u8) {
    if value <= 4 {
        return (value - 1, 0);
    }
    let value = value - 1;
    let highest = value.ilog2() as usize;
    let second = (value >> (highest - 1)) & 1;
    (2 * highest + second, (highest - 1) as u8)
}

fn estimated_cost(tokens: &[Token], cache_bits: u8) -> f64 {
    let cache_size = if cache_bits == 0 { 0 } else { 1 << cache_bits };
    let mut green = vec![0_u32; 280 + cache_size];
    let mut red = [0_u32; 256];
    let mut blue = [0_u32; 256];
    let mut alpha = [0_u32; 256];
    let mut distance = [0_u32; 40];
    let mut extra = 0_u64;
    for &token in tokens {
        match token {
            Token::Literal(pixel) => {
                let [r, g, b, a] = super::channels(pixel);
                green[g] += 1;
                red[r] += 1;
                blue[b] += 1;
                alpha[a] += 1;
            }
            Token::Copy {
                distance: d,
                length,
            } => {
                let (length_code, length_extra) = prefix(length);
                let (distance_code, distance_extra) = prefix(d);
                green[256 + length_code] += 1;
                distance[distance_code] += 1;
                extra += u64::from(length_extra + distance_extra);
            }
            Token::Cache(index) => green[280 + index] += 1,
        }
    }
    fn entropy(values: &[u32]) -> f64 {
        let total: u32 = values.iter().sum();
        if total == 0 {
            return 0.0;
        }
        let total = f64::from(total);
        values
            .iter()
            .filter(|&&value| value != 0)
            .map(|&value| {
                let value = f64::from(value);
                value * (total / value).log2()
            })
            .sum()
    }
    entropy(&green)
        + entropy(&red)
        + entropy(&blue)
        + entropy(&alpha)
        + entropy(&distance)
        + extra as f64
}

pub(super) fn candidates(pixels: &[u32], width: usize, allow_cache: bool) -> Vec<(Vec<Token>, u8)> {
    if pixels.is_empty() {
        return vec![(Vec::new(), 0)];
    }
    let chain = fill_hash_chain(pixels, width);
    let refs = lz77(pixels, width, &chain);
    if !allow_cache {
        return vec![(refs, 0)];
    }
    let mut candidates = vec![(refs.clone(), 0)];
    for bits in 1..=11 {
        candidates.push((with_cache(pixels, &refs, bits), bits));
    }
    candidates
}

const PLANE_TO_CODE: [u8; 128] = [
    96, 73, 55, 39, 23, 13, 5, 1, 255, 255, 255, 255, 255, 255, 255, 255, 101, 78, 58, 42, 26, 16,
    8, 2, 0, 3, 9, 17, 27, 43, 59, 79, 102, 86, 62, 46, 32, 20, 10, 6, 4, 7, 11, 21, 33, 47, 63,
    87, 105, 90, 70, 52, 37, 28, 18, 14, 12, 15, 19, 29, 38, 53, 71, 91, 110, 99, 82, 66, 48, 35,
    30, 24, 22, 25, 31, 36, 49, 67, 83, 100, 115, 108, 94, 76, 64, 50, 44, 40, 34, 41, 45, 51, 65,
    77, 95, 109, 118, 113, 103, 92, 80, 68, 60, 56, 54, 57, 61, 69, 81, 93, 104, 114, 119, 116,
    111, 106, 97, 88, 84, 74, 72, 75, 85, 89, 98, 107, 112, 117,
];

pub(super) fn plane_code(width: usize, distance: usize) -> usize {
    let y = distance / width;
    let x = distance - y * width;
    if x <= 8 && y < 8 {
        usize::from(PLANE_TO_CODE[y * 16 + 8 - x]) + 1
    } else if x + 8 > width && y < 7 {
        usize::from(PLANE_TO_CODE[(y + 1) * 16 + 8 + width - x]) + 1
    } else {
        distance + 120
    }
}
