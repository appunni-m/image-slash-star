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
                let key = (run as u32)
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

fn rle(pixels: &[u32], width: usize) -> Vec<Token> {
    let mut refs = vec![Token::Literal(pixels[0])];
    let mut position = 1;
    while position < pixels.len() {
        let maximum = MAX_LENGTH.min(pixels.len() - position);
        let run_length = match_length(pixels, position, position - 1, maximum);
        let previous_row_length = if position < width {
            0
        } else {
            match_length(pixels, position, position - width, maximum)
        };
        if run_length >= previous_row_length && run_length >= MIN_LENGTH {
            refs.push(Token::Copy {
                distance: 1,
                length: run_length,
            });
            position += run_length;
        } else if previous_row_length >= MIN_LENGTH {
            refs.push(Token::Copy {
                distance: width,
                length: previous_row_length,
            });
            position += previous_row_length;
        } else {
            refs.push(Token::Literal(pixels[position]));
            position += 1;
        }
    }
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

struct CostModel {
    green: Vec<u32>,
    red: [u32; 256],
    blue: [u32; 256],
    alpha: [u32; 256],
    distance: [u32; 40],
}

fn population_estimate(counts: &[u32]) -> f64 {
    let sum: u32 = counts.iter().sum();
    let nonzero = counts.iter().filter(|&&count| count != 0).count();
    let maximum = counts.iter().copied().max().unwrap_or(0);
    let entropy = if sum == 0 {
        0.0
    } else {
        counts
            .iter()
            .filter(|&&count| count != 0)
            .map(|&count| f64::from(count) * (f64::from(sum) / f64::from(count)).log2())
            .sum()
    };
    let refined = match nonzero {
        0 | 1 => 0.0,
        2 => (99.0 * f64::from(sum) + entropy) / 100.0,
        _ => {
            let mix = if nonzero == 3 {
                950.0
            } else if nonzero == 4 {
                700.0
            } else {
                627.0
            };
            let minimum = (mix * f64::from(2 * sum - maximum) + (1000.0 - mix) * entropy) / 1000.0;
            entropy.max(minimum)
        }
    };

    let mut counts_by_kind = [0_u32; 2];
    let mut streaks = [[0_u32; 2]; 2];
    let mut start = 0;
    while start < counts.len() {
        let value = counts[start];
        let mut end = start + 1;
        while end < counts.len() && counts[end] == value {
            end += 1;
        }
        let kind = usize::from(value != 0);
        let long = usize::from(end - start > 3);
        counts_by_kind[kind] += u32::from(long != 0);
        streaks[kind][long] += (end - start) as u32;
        start = end;
    }
    let extra = counts_by_kind[0] * 1600
        + 240 * streaks[0][1]
        + counts_by_kind[1] * 2640
        + 720 * streaks[1][1]
        + 1840 * streaks[0][0]
        + 3360 * streaks[1][0];
    refined + 47.9 + f64::from(extra) / 1024.0
}

fn fast_slog(value: u32) -> u64 {
    if value < 256 {
        (f64::from(value) * f64::from(value).log2() * f64::from(1_u32 << 23)).round_ties_even()
            as u64
    } else if value < 65_536 {
        let log_count = value.ilog2() - 7;
        let scale = 1_u32 << log_count;
        let reduced = value >> log_count;
        let reduced_log =
            (f64::from(reduced).log2() * f64::from(1_u32 << 23)).round_ties_even() as u32;
        u64::from(value) * u64::from(reduced_log + (log_count << 23))
            + 12_102_203_u64 * u64::from(value & (scale - 1))
    } else {
        (12_102_203.161_561_485 * f64::from(value) * f64::from(value).ln() + 0.5) as u64
    }
}

fn population_estimate_fixed(counts: &[u32]) -> u64 {
    let sum: u32 = counts.iter().sum();
    let nonzero = counts.iter().filter(|&&count| count != 0).count();
    let maximum = counts.iter().copied().max().unwrap_or(0);
    let entropy = fast_slog(sum)
        - counts
            .iter()
            .copied()
            .filter(|&count| count != 0)
            .map(fast_slog)
            .sum::<u64>();
    let div_round = |value: u64, divisor: u64| (value + divisor / 2) / divisor;
    let refined = match nonzero {
        0 | 1 => 0,
        2 => div_round(99 * (u64::from(sum) << 23) + entropy, 100),
        _ => {
            let mix = if nonzero == 3 {
                950
            } else if nonzero == 4 {
                700
            } else {
                627
            };
            let minimum = div_round(
                mix * (u64::from(2 * sum - maximum) << 23) + (1000 - mix) * entropy,
                1000,
            );
            entropy.max(minimum)
        }
    };

    let mut counts_by_kind = [0_u32; 2];
    let mut streaks = [[0_u32; 2]; 2];
    let mut start = 0;
    while start < counts.len() {
        let value = counts[start];
        let mut end = start + 1;
        while end < counts.len() && counts[end] == value {
            end += 1;
        }
        let kind = usize::from(value != 0);
        let long = usize::from(end - start > 3);
        counts_by_kind[kind] += u32::from(long != 0);
        streaks[kind][long] += (end - start) as u32;
        start = end;
    }
    let extra = counts_by_kind[0] * 1600
        + 240 * streaks[0][1]
        + counts_by_kind[1] * 2640
        + 720 * streaks[1][1]
        + 1840 * streaks[0][0]
        + 3360 * streaks[1][0];
    let initial = (57_u64 << 23) - div_round(91_u64 << 23, 10);
    refined + initial + (u64::from(extra) << 13)
}

pub(super) fn estimated_bits(tokens: &[Token], cache_bits: u8) -> u64 {
    let cache_size = if cache_bits == 0 { 0 } else { 1 << cache_bits };
    let mut green = vec![0_u32; 280 + cache_size];
    let mut red = [0_u32; 256];
    let mut blue = [0_u32; 256];
    let mut alpha = [0_u32; 256];
    let mut distance = [0_u32; 40];
    let mut extra = 0_u32;
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
                let (length_symbol, length_extra) = prefix(length);
                let (distance_symbol, distance_extra) = prefix(d);
                green[256 + length_symbol] += 1;
                distance[distance_symbol] += 1;
                extra += u32::from(length_extra + distance_extra);
            }
            Token::Cache(index) => green[280 + index] += 1,
        }
    }
    population_estimate_fixed(&green)
        + population_estimate_fixed(&red)
        + population_estimate_fixed(&blue)
        + population_estimate_fixed(&alpha)
        + population_estimate_fixed(&distance)
        + (u64::from(extra) << 23)
}

fn cache_estimated_bits(tokens: &[Token], cache_bits: u8) -> u64 {
    let cache_size = if cache_bits == 0 { 0 } else { 1 << cache_bits };
    let mut green = vec![0_u32; 280 + cache_size];
    let mut red = [0_u32; 256];
    let mut blue = [0_u32; 256];
    let mut alpha = [0_u32; 256];
    for &token in tokens {
        match token {
            Token::Literal(pixel) => {
                let [r, g, b, a] = super::channels(pixel);
                green[g] += 1;
                red[r] += 1;
                blue[b] += 1;
                alpha[a] += 1;
            }
            Token::Copy { length, .. } => green[256 + prefix(length).0] += 1,
            Token::Cache(index) => green[280 + index] += 1,
        }
    }
    population_estimate_fixed(&green)
        + population_estimate_fixed(&red)
        + population_estimate_fixed(&blue)
        + population_estimate_fixed(&alpha)
}

fn population_cost(counts: &[u32]) -> Vec<u32> {
    let sum: u32 = counts.iter().sum();
    if counts.iter().filter(|&&count| count != 0).count() <= 1 {
        return vec![0; counts.len()];
    }
    let fast_log = |value: u32| -> u32 {
        if value == 0 {
            0
        } else if value < 256 {
            (f64::from(value).log2() * f64::from(1_u32 << 23)).round() as u32
        } else if value < 65_536 {
            let log_count = value.ilog2() - 7;
            let scale = 1_u32 << log_count;
            let reduced = value >> log_count;
            let mut result = (f64::from(reduced).log2() * f64::from(1_u32 << 23)).round() as u32
                + (log_count << 23);
            if value >= 4096 {
                let correction = 12_102_203_u64 * u64::from(value & (scale - 1));
                result += ((correction + u64::from(value) / 2) / u64::from(value)) as u32;
            }
            result
        } else {
            (12_102_203.161_561_485 * f64::from(value).ln() + 0.5) as u32
        }
    };
    let log_sum = fast_log(sum);
    counts
        .iter()
        .map(|&count| log_sum - fast_log(count))
        .collect()
}

fn cost_model(tokens: &[Token], cache_bits: u8, width: usize) -> CostModel {
    let cache_size = if cache_bits == 0 { 0 } else { 1 << cache_bits };
    let mut green = vec![0_u32; 280 + cache_size];
    let mut red = [0_u32; 256];
    let mut blue = [0_u32; 256];
    let mut alpha = [0_u32; 256];
    let mut distance = [0_u32; 40];
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
                let (distance_code, distance_extra) = prefix(plane_code(width, d));
                green[256 + length_code] += 1;
                distance[distance_code] += 1;
                let _ = (length_extra, distance_extra);
            }
            Token::Cache(index) => green[280 + index] += 1,
        }
    }
    CostModel {
        green: population_cost(&green),
        red: population_cost(&red).try_into().unwrap(),
        blue: population_cost(&blue).try_into().unwrap(),
        alpha: population_cost(&alpha).try_into().unwrap(),
        distance: population_cost(&distance).try_into().unwrap(),
    }
}

fn trace_backwards(
    pixels: &[u32],
    width: usize,
    chain: &[(usize, usize)],
    source: &[Token],
    cache_bits: u8,
) -> Vec<Token> {
    const SCALE: i64 = 1 << 23;
    let model = cost_model(source, cache_bits, width);
    let mut costs = vec![i64::MAX; pixels.len()];
    let mut lengths = vec![1_usize; pixels.len()];
    let mut cache = vec![0_u32; if cache_bits == 0 { 0 } else { 1 << cache_bits }];

    for position in 0..pixels.len() {
        let previous_cost = if position == 0 {
            0
        } else {
            costs[position - 1]
        };
        let pixel = pixels[position];
        let cache_index = (cache_bits != 0).then(|| color_hash(pixel, cache_bits));
        let literal_cost = if let Some(index) = cache_index.filter(|&index| cache[index] == pixel) {
            (i64::from(model.green[280 + index]) * 68 + 50) / 100
        } else {
            if let Some(index) = cache_index {
                cache[index] = pixel;
            }
            let [red, green, blue, alpha] = super::channels(pixel);
            let cost = i64::from(model.green[green])
                + i64::from(model.red[red])
                + i64::from(model.blue[blue])
                + i64::from(model.alpha[alpha]);
            (cost * 82 + 50) / 100
        };
        let candidate = previous_cost + literal_cost;
        if candidate < costs[position] {
            costs[position] = candidate;
            lengths[position] = 1;
        }

        let (distance, maximum_length) = chain[position];
        if maximum_length >= 2 {
            let plane_distance = plane_code(width, distance);
            let (distance_symbol, distance_extra) = prefix(plane_distance);
            let distance_cost =
                i64::from(model.distance[distance_symbol]) + i64::from(distance_extra) * SCALE;
            for length in 1..=maximum_length {
                let end = position + length - 1;
                let (length_symbol, length_extra) = prefix(length);
                let candidate = previous_cost
                    + distance_cost
                    + i64::from(model.green[256 + length_symbol])
                    + i64::from(length_extra) * SCALE;
                if candidate < costs[end] {
                    costs[end] = candidate;
                    lengths[end] = length;
                }
            }
        }
    }

    let mut path = Vec::new();
    let mut end = pixels.len();
    while end != 0 {
        let length = lengths[end - 1];
        path.push(length);
        end -= length;
    }
    path.reverse();

    let mut output = Vec::with_capacity(path.len());
    let mut cache = vec![0_u32; if cache_bits == 0 { 0 } else { 1 << cache_bits }];
    let mut position = 0;
    for length in path {
        if length == 1 {
            let pixel = pixels[position];
            if cache_bits != 0 {
                let index = color_hash(pixel, cache_bits);
                if cache[index] == pixel {
                    output.push(Token::Cache(index));
                } else {
                    cache[index] = pixel;
                    output.push(Token::Literal(pixel));
                }
            } else {
                output.push(Token::Literal(pixel));
            }
        } else {
            output.push(Token::Copy {
                distance: chain[position].0,
                length,
            });
            if cache_bits != 0 {
                for &pixel in &pixels[position..position + length] {
                    let index = color_hash(pixel, cache_bits);
                    cache[index] = pixel;
                }
            }
        }
        position += length;
    }
    output
}

pub(super) fn trace(pixels: &[u32], width: usize, source: &[Token], cache_bits: u8) -> Vec<Token> {
    let chain = fill_hash_chain(pixels, width);
    trace_backwards(pixels, width, &chain, source, cache_bits)
}

pub(super) fn candidates(pixels: &[u32], width: usize, allow_cache: bool) -> Vec<(Vec<Token>, u8)> {
    if pixels.is_empty() {
        return vec![(Vec::new(), 0)];
    }
    let chain = fill_hash_chain(pixels, width);
    let refs = lz77(pixels, width, &chain);
    let refs_rle = rle(pixels, width);
    if !allow_cache {
        return vec![(refs, 0), (refs_rle, 0)];
    }
    let candidates: Vec<_> = [refs, refs_rle]
        .into_iter()
        .map(|source| {
            (0..=11)
                .map(|bits| {
                    let cached = with_cache(pixels, &source, bits);
                    let cost = cache_estimated_bits(&cached, bits);
                    (cached, bits, cost)
                })
                .min_by_key(|candidate| candidate.2)
                .map(|(tokens, bits, _)| (tokens, bits))
                .unwrap()
        })
        .collect();
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
