//! Lossless WebP backward references, ported from libwebp 1.6.0
//! `src/enc/backward_references_enc.c`.

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
// Hash-chain positions, fixed-point entropy costs, and LZ77 interval arithmetic
// are the libwebp reference algorithm. Valid image geometry, bounded window
// sizes, and codec alphabets constrain these operations inside this module.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

const MIN_LENGTH: usize = 4;
const MAX_LENGTH: usize = (1 << 12) - 1;
const WINDOW_SIZE: usize = (1 << 20) - 120;
const HASH_BITS: usize = 18;
const HASH_SIZE: usize = 1 << HASH_BITS;
const HASH_MULTIPLIER_HI: u32 = 0xc6a4_a793;
const HASH_MULTIPLIER_LO: u32 = 0x5bd1_e996;
const COLOR_HASH_MUL: u32 = 0x1e35_a7bd;
const COST_CHECKPOINT_SYMBOLS: usize = 64;
const COST_CHECKPOINT_TOKENS: usize = 1_024;
const COST_MANAGER_CHECKPOINT_ENTRIES: usize = 1_024;
const COST_MANAGER_UPDATE_CHECKPOINT_ENTRIES: usize = 256;
const CACHE_CHECKPOINT_PIXELS: usize = 256;
const HASH_CHAIN_RUN_CHECKPOINT_PIXELS: usize = 256;
const HASH_CHAIN_RESULT_CHECKPOINT_PIXELS: usize = 256;
const HASH_CHAIN_CANDIDATE_CHECKPOINT_TRIALS: usize = 64;
const BOX_CHAIN_CANDIDATE_CHECKPOINT_OFFSETS: usize = 64;

type CheckpointToken<'a> = Option<&'a crate::CancellationToken>;
type CheckpointResult<T> = Result<T, super::EncodingError>;

#[inline]
fn checkpoint(token: CheckpointToken<'_>) -> CheckpointResult<()> {
    super::check_token(token)
}

#[inline]
fn checkpoint_cost_manager_work(
    token: CheckpointToken<'_>,
    work: &mut usize,
) -> CheckpointResult<()> {
    *work = work.saturating_add(1);
    if (*work).is_multiple_of(COST_MANAGER_CHECKPOINT_ENTRIES) {
        checkpoint(token)?;
    }
    Ok(())
}

#[inline]
fn checkpoint_cost_manager_update_work(
    token: CheckpointToken<'_>,
    work: &mut usize,
) -> CheckpointResult<()> {
    *work = work.saturating_add(1);
    if (*work).is_multiple_of(COST_MANAGER_UPDATE_CHECKPOINT_ENTRIES) {
        checkpoint(token)?;
    }
    Ok(())
}

#[inline]
fn checkpoint_hash_chain_candidate_work(
    token: &crate::CancellationToken,
    work: &mut usize,
) -> CheckpointResult<()> {
    *work = work.saturating_add(1);
    if (*work).is_multiple_of(HASH_CHAIN_CANDIDATE_CHECKPOINT_TRIALS) {
        checkpoint(Some(token))?;
    }
    Ok(())
}

#[inline]
fn checkpoint_box_chain_candidate_work(
    token: &crate::CancellationToken,
    work: &mut usize,
) -> CheckpointResult<()> {
    *work = work.saturating_add(1);
    if (*work).is_multiple_of(BOX_CHAIN_CANDIDATE_CHECKPOINT_OFFSETS) {
        checkpoint(Some(token))?;
    }
    Ok(())
}

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

fn match_length(
    pixels: &[u32],
    first: usize,
    second: usize,
    limit: usize,
    token: CheckpointToken<'_>,
) -> CheckpointResult<usize> {
    let mut length = 0;
    while length < limit && pixels[first + length] == pixels[second + length] {
        length += 1;
        if length.is_multiple_of(256) {
            checkpoint(token)?;
        }
    }
    Ok(length)
}

/// Builds the same best-distance/best-length table as `VP8LHashChainFill()`.
fn fill_hash_chain(
    pixels: &[u32],
    width: usize,
    quality: u32,
    token: CheckpointToken<'_>,
) -> CheckpointResult<Vec<(usize, usize)>> {
    let size = pixels.len();
    let mut result = vec![(0, 0); size];
    if size <= 2 {
        return Ok(result);
    }

    let mut first = vec![-1_i32; HASH_SIZE];
    let mut chain = vec![-1_i32; size];
    let mut position: usize = 0;
    let mut equal_pair = pixels[0] == pixels[1];
    while position < size - 2 {
        if position != 0 && position.is_multiple_of(1024) {
            checkpoint(token)?;
        }
        let next_equal_pair = pixels[position + 1] == pixels[position + 2];
        if equal_pair && next_equal_pair {
            let color = pixels[position];
            let mut run = 1;
            while position + run + 2 < size && pixels[position + run + 2] == color {
                run += 1;
                if run.is_multiple_of(256) {
                    checkpoint(token)?;
                }
            }
            if run > MAX_LENGTH {
                position += run - MAX_LENGTH;
                run = MAX_LENGTH;
            }
            if let Some(token) = token {
                let mut inserted = 0_usize;
                while run > 0 {
                    let key = (run as u32)
                        .wrapping_mul(HASH_MULTIPLIER_HI)
                        .wrapping_add(pixels[position].wrapping_mul(HASH_MULTIPLIER_LO));
                    let hash = (key >> (32 - HASH_BITS)) as usize;
                    chain[position] = first[hash];
                    first[hash] = position as i32;
                    position += 1;
                    run -= 1;
                    inserted += 1;
                    if inserted.is_multiple_of(HASH_CHAIN_RUN_CHECKPOINT_PIXELS) {
                        checkpoint(Some(token))?;
                    }
                }
            } else {
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
    let iterations = 8 + quality * quality / 128;
    let window_size = if quality > 75 {
        WINDOW_SIZE
    } else if quality > 50 {
        width << 8
    } else if quality > 25 {
        width << 6
    } else {
        width << 4
    }
    .min(WINDOW_SIZE);
    let mut base = size - 2;
    let mut candidate_work = 0_usize;
    while base > 0 {
        if base.is_multiple_of(1024) {
            checkpoint(token)?;
        }
        let max_length = MAX_LENGTH.min(size - 1 - base);
        let mut remaining = iterations;
        let mut best_length = 0;
        let mut best_distance = 0;
        let minimum = base.saturating_sub(window_size);

        if base >= width {
            let current = match_length(pixels, base - width, base, max_length, token)?;
            if current > best_length {
                best_length = current;
                best_distance = width;
            }
            remaining -= 1;
        }
        let current = match_length(pixels, base - 1, base, max_length, token)?;
        if current > best_length {
            best_length = current;
            best_distance = 1;
        }
        remaining -= 1;

        let mut candidate = chain[base];
        let good_enough = max_length.min(256);
        if let Some(token) = token {
            while candidate >= minimum as i32 && remaining > 1 && best_length < MAX_LENGTH {
                remaining -= 1;
                let candidate_index = candidate as usize;
                let mut reached_good_enough = false;
                if pixels[candidate_index + best_length] == pixels[base + best_length] {
                    let current =
                        match_length(pixels, candidate_index, base, max_length, Some(token))?;
                    if current > best_length {
                        best_length = current;
                        best_distance = base - candidate_index;
                        reached_good_enough = best_length >= good_enough;
                    }
                }
                checkpoint_hash_chain_candidate_work(token, &mut candidate_work)?;
                if reached_good_enough {
                    break;
                }
                candidate = chain[candidate_index];
            }
        } else {
            while candidate >= minimum as i32 && remaining > 1 && best_length < MAX_LENGTH {
                remaining -= 1;
                let candidate_index = candidate as usize;
                if pixels[candidate_index + best_length] == pixels[base + best_length] {
                    let current = match_length(pixels, candidate_index, base, max_length, None)?;
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
        }

        let mut maximum_base = base;
        let mut result_work = 0usize;
        loop {
            result[base] = (best_distance, best_length);
            base -= 1;
            result_work += 1;
            if let Some(token) = token
                && result_work.is_multiple_of(HASH_CHAIN_RESULT_CHECKPOINT_PIXELS)
            {
                checkpoint(Some(token))?;
            }
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
    Ok(result)
}

fn lz77(
    pixels: &[u32],
    width: usize,
    chain: &[(usize, usize)],
    token: CheckpointToken<'_>,
) -> CheckpointResult<Vec<Token>> {
    let mut refs = Vec::new();
    let mut position = 0;
    let mut last_check: isize = -1;
    let mut next_checkpoint = 1024;
    while position < pixels.len() {
        if position >= next_checkpoint {
            checkpoint(token)?;
            next_checkpoint = position.saturating_add(1024);
        }
        let (distance, initial_length) = chain[position];
        let mut length = initial_length;
        if length >= MIN_LENGTH {
            let mut maximum_reach = 0;
            let maximum_check = (position + length).min(pixels.len() - 1);
            last_check = last_check.max(position as isize);
            let check_start = last_check as usize + 1;
            for (offset, &(_, next_length)) in chain[check_start..=maximum_check].iter().enumerate()
            {
                let next = check_start + offset;
                if next >= next_checkpoint {
                    checkpoint(token)?;
                    next_checkpoint = next.saturating_add(1024);
                }
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
    Ok(refs)
}

fn rle(pixels: &[u32], width: usize, token: CheckpointToken<'_>) -> CheckpointResult<Vec<Token>> {
    let mut refs = vec![Token::Literal(pixels[0])];
    let mut position = 1;
    while position < pixels.len() {
        if position.is_multiple_of(1024) {
            checkpoint(token)?;
        }
        let maximum = MAX_LENGTH.min(pixels.len() - position);
        let run_length = match_length(pixels, position, position - 1, maximum, token)?;
        let previous_row_length = if position < width {
            0
        } else {
            match_length(pixels, position, position - width, maximum, token)?
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
    Ok(refs)
}

fn box_chain(
    pixels: &[u32],
    width: usize,
    best_chain: &[(usize, usize)],
    token: CheckpointToken<'_>,
) -> CheckpointResult<Vec<(usize, usize)>> {
    const WINDOW_OFFSETS_SIZE_MAX: usize = 32;

    let mut chain = vec![(0, 0); pixels.len()];
    if pixels.len() < 2 {
        return Ok(chain);
    }

    let mut counts = vec![1_u16; pixels.len()];
    for position in (0..pixels.len() - 1).rev() {
        if position.is_multiple_of(1024) {
            checkpoint(token)?;
        }
        if pixels[position] == pixels[position + 1] {
            counts[position] = counts[position + 1]
                .saturating_add(u16::from(usize::from(counts[position + 1]) != MAX_LENGTH));
        }
    }

    let mut offsets_by_code = [0_usize; WINDOW_OFFSETS_SIZE_MAX];
    for y in 0..=6 {
        for x in -6_isize..=6 {
            let offset = y * width;
            let Some(offset) = offset.checked_add_signed(x) else {
                continue;
            };
            if offset == 0 {
                continue;
            }
            let code = plane_code(width, offset) - 1;
            if code < WINDOW_OFFSETS_SIZE_MAX {
                offsets_by_code[code] = offset;
            }
        }
    }
    let window_offsets = offsets_by_code
        .into_iter()
        .filter(|&offset| offset != 0)
        .collect::<Vec<_>>();
    let window_offsets_new = window_offsets
        .iter()
        .copied()
        .filter(|&offset| {
            !window_offsets
                .iter()
                .any(|&other| offset == other.saturating_add(1))
        })
        .collect::<Vec<_>>();

    let mut previous_offset = 0;
    let mut previous_length = 0;
    let mut candidate_work = 0_usize;
    for position in 1..pixels.len() {
        if position.is_multiple_of(1024) {
            checkpoint(token)?;
        }
        let (mut best_offset, mut best_length) = best_chain[position];
        let recompute = best_length < MAX_LENGTH || !window_offsets.contains(&best_offset);
        if recompute {
            let use_previous = previous_length > 1 && previous_length < MAX_LENGTH;
            let offsets = if use_previous {
                &window_offsets_new
            } else {
                &window_offsets
            };
            best_length = if use_previous { previous_length - 1 } else { 0 };
            best_offset = if use_previous { previous_offset } else { 0 };
            if let Some(token) = token {
                for &offset in offsets {
                    checkpoint_box_chain_candidate_work(token, &mut candidate_work)?;
                    let Some(mut candidate) = position.checked_sub(offset) else {
                        continue;
                    };
                    if pixels[candidate] != pixels[position] {
                        continue;
                    }
                    let mut current = position;
                    let mut length = 0;
                    let mut next_checkpoint = 256;
                    loop {
                        let candidate_count = usize::from(counts[candidate]);
                        let current_count = usize::from(counts[current]);
                        if candidate_count != current_count {
                            length += candidate_count.min(current_count);
                            break;
                        }
                        length += candidate_count;
                        if length >= next_checkpoint {
                            checkpoint(Some(token))?;
                            next_checkpoint = length.saturating_add(256);
                        }
                        candidate += candidate_count;
                        current += current_count;
                        if length > MAX_LENGTH
                            || current >= pixels.len()
                            || pixels[candidate] != pixels[current]
                        {
                            break;
                        }
                    }
                    if length > best_length {
                        best_offset = offset;
                        best_length = length.min(MAX_LENGTH);
                        if length >= MAX_LENGTH {
                            break;
                        }
                    }
                }
            } else {
                for &offset in offsets {
                    let Some(mut candidate) = position.checked_sub(offset) else {
                        continue;
                    };
                    if pixels[candidate] != pixels[position] {
                        continue;
                    }
                    let mut current = position;
                    let mut length = 0;
                    let mut next_checkpoint = 256;
                    loop {
                        let candidate_count = usize::from(counts[candidate]);
                        let current_count = usize::from(counts[current]);
                        if candidate_count != current_count {
                            length += candidate_count.min(current_count);
                            break;
                        }
                        length += candidate_count;
                        if length >= next_checkpoint {
                            checkpoint(token)?;
                            next_checkpoint = length.saturating_add(256);
                        }
                        candidate += candidate_count;
                        current += current_count;
                        if length > MAX_LENGTH
                            || current >= pixels.len()
                            || pixels[candidate] != pixels[current]
                        {
                            break;
                        }
                    }
                    if length > best_length {
                        best_offset = offset;
                        best_length = length.min(MAX_LENGTH);
                        if length >= MAX_LENGTH {
                            break;
                        }
                    }
                }
            }
        }

        if best_length <= MIN_LENGTH {
            previous_offset = 0;
            previous_length = 0;
        } else {
            chain[position] = (best_offset, best_length);
            previous_offset = best_offset;
            previous_length = best_length;
        }
    }
    Ok(chain)
}

fn color_hash(pixel: u32, bits: u8) -> usize {
    (pixel.wrapping_mul(COLOR_HASH_MUL) >> (32 - bits)) as usize
}

fn populate_cache_without_checkpoint(
    pixels: &[u32],
    position: usize,
    length: usize,
    bits: u8,
    cache: &mut [u32],
) {
    if bits == 0 {
        return;
    }
    for &pixel in &pixels[position..position + length] {
        let key = color_hash(pixel, bits);
        cache[key] = pixel;
    }
}

fn populate_cache(
    pixels: &[u32],
    position: usize,
    length: usize,
    bits: u8,
    cache: &mut [u32],
    token: CheckpointToken<'_>,
) -> CheckpointResult<()> {
    if bits == 0 {
        return Ok(());
    }
    let Some(token) = token else {
        populate_cache_without_checkpoint(pixels, position, length, bits, cache);
        return Ok(());
    };
    for (index, &pixel) in pixels[position..position + length].iter().enumerate() {
        let key = color_hash(pixel, bits);
        cache[key] = pixel;
        if (index + 1).is_multiple_of(CACHE_CHECKPOINT_PIXELS) {
            checkpoint(Some(token))?;
        }
    }
    Ok(())
}

fn with_cache_without_checkpoint(pixels: &[u32], refs: &[Token], bits: u8) -> Vec<Token> {
    let mut cache = vec![0_u32; 1 << bits];
    let mut output = Vec::with_capacity(refs.len());
    let mut position: usize = 0;
    for &reference in refs {
        match reference {
            Token::Literal(pixel) => {
                let key = color_hash(pixel, bits);
                if cache[key] == pixel {
                    output.push(Token::Cache(key));
                } else {
                    output.push(reference);
                    cache[key] = pixel;
                }
                position += 1;
            }
            Token::Copy { length, .. } => {
                output.push(reference);
                populate_cache_without_checkpoint(pixels, position, length, bits, &mut cache);
                position += length;
            }
            Token::Cache(_) => unreachable!(),
        }
    }
    output
}

fn with_cache(
    pixels: &[u32],
    refs: &[Token],
    bits: u8,
    token: CheckpointToken<'_>,
) -> CheckpointResult<Vec<Token>> {
    if bits == 0 {
        return Ok(refs.to_vec());
    }
    let Some(token) = token else {
        return Ok(with_cache_without_checkpoint(pixels, refs, bits));
    };
    let mut cache = vec![0_u32; 1 << bits];
    let mut output = Vec::with_capacity(refs.len());
    let mut position: usize = 0;
    for &reference in refs {
        if position.is_multiple_of(1024) {
            checkpoint(Some(token))?;
        }
        match reference {
            Token::Literal(pixel) => {
                let key = color_hash(pixel, bits);
                if cache[key] == pixel {
                    output.push(Token::Cache(key));
                } else {
                    output.push(reference);
                    cache[key] = pixel;
                }
                position += 1;
            }
            Token::Copy { length, .. } => {
                output.push(reference);
                populate_cache(pixels, position, length, bits, &mut cache, Some(token))?;
                position += length;
            }
            Token::Cache(_) => unreachable!(),
        }
    }
    Ok(output)
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

pub(super) fn fast_slog(value: u32) -> u64 {
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

fn population_estimate_fixed_with_checkpoint(
    counts: &[u32],
    token: CheckpointToken<'_>,
) -> CheckpointResult<u64> {
    if token.is_none() {
        return Ok(population_estimate_fixed(counts));
    }
    let mut sum = 0_u32;
    let mut nonzero = 0;
    let mut maximum = 0;
    let mut entropy_sum = 0_u64;
    for (index, &count) in counts.iter().enumerate() {
        sum += count;
        if count != 0 {
            nonzero += 1;
            maximum = maximum.max(count);
            entropy_sum += fast_slog(count);
        }
        if (index + 1).is_multiple_of(COST_CHECKPOINT_SYMBOLS) {
            checkpoint(token)?;
        }
    }
    let entropy = fast_slog(sum) - entropy_sum;
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
            if end.is_multiple_of(COST_CHECKPOINT_SYMBOLS) {
                checkpoint(token)?;
            }
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
    Ok(refined + initial + (u64::from(extra) << 13))
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

fn estimated_bits_with_checkpoint(
    tokens: &[Token],
    cache_bits: u8,
    token: CheckpointToken<'_>,
) -> CheckpointResult<u64> {
    if token.is_none() {
        return Ok(estimated_bits(tokens, cache_bits));
    }
    let cache_size = if cache_bits == 0 { 0 } else { 1 << cache_bits };
    let mut green = vec![0_u32; 280 + cache_size];
    let mut red = [0_u32; 256];
    let mut blue = [0_u32; 256];
    let mut alpha = [0_u32; 256];
    let mut distance = [0_u32; 40];
    let mut extra = 0_u32;
    for (index, &item) in tokens.iter().enumerate() {
        match item {
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
            Token::Cache(cache_index) => green[280 + cache_index] += 1,
        }
        if (index + 1).is_multiple_of(COST_CHECKPOINT_TOKENS) {
            checkpoint(token)?;
        }
    }
    Ok(population_estimate_fixed_with_checkpoint(&green, token)?
        + population_estimate_fixed_with_checkpoint(&red, token)?
        + population_estimate_fixed_with_checkpoint(&blue, token)?
        + population_estimate_fixed_with_checkpoint(&alpha, token)?
        + population_estimate_fixed_with_checkpoint(&distance, token)?
        + (u64::from(extra) << 23))
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

fn cache_estimated_bits_with_checkpoint(
    tokens: &[Token],
    cache_bits: u8,
    token: CheckpointToken<'_>,
) -> CheckpointResult<u64> {
    if token.is_none() {
        return Ok(cache_estimated_bits(tokens, cache_bits));
    }
    let cache_size = if cache_bits == 0 { 0 } else { 1 << cache_bits };
    let mut green = vec![0_u32; 280 + cache_size];
    let mut red = [0_u32; 256];
    let mut blue = [0_u32; 256];
    let mut alpha = [0_u32; 256];
    for (index, &item) in tokens.iter().enumerate() {
        match item {
            Token::Literal(pixel) => {
                let [r, g, b, a] = super::channels(pixel);
                green[g] += 1;
                red[r] += 1;
                blue[b] += 1;
                alpha[a] += 1;
            }
            Token::Copy { length, .. } => green[256 + prefix(length).0] += 1,
            Token::Cache(cache_index) => green[280 + cache_index] += 1,
        }
        if (index + 1).is_multiple_of(COST_CHECKPOINT_TOKENS) {
            checkpoint(token)?;
        }
    }
    Ok(population_estimate_fixed_with_checkpoint(&green, token)?
        + population_estimate_fixed_with_checkpoint(&red, token)?
        + population_estimate_fixed_with_checkpoint(&blue, token)?
        + population_estimate_fixed_with_checkpoint(&alpha, token)?)
}

fn population_cost_in_place(counts: &mut [u32]) {
    let sum: u32 = counts.iter().sum();
    if counts.iter().filter(|&&count| count != 0).count() <= 1 {
        counts.fill(0);
        return;
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
    for count in counts {
        *count = log_sum - fast_log(*count);
    }
}

fn population_cost_in_place_with_checkpoint(
    counts: &mut [u32],
    token: CheckpointToken<'_>,
) -> CheckpointResult<()> {
    if token.is_none() {
        population_cost_in_place(counts);
        return Ok(());
    }
    let mut sum = 0_u32;
    let mut nonzero = 0;
    for (index, &count) in counts.iter().enumerate() {
        sum += count;
        nonzero += usize::from(count != 0);
        if (index + 1).is_multiple_of(COST_CHECKPOINT_SYMBOLS) {
            checkpoint(token)?;
        }
    }
    if nonzero <= 1 {
        counts.fill(0);
        return Ok(());
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
    for (index, count) in counts.iter_mut().enumerate() {
        *count = log_sum - fast_log(*count);
        if (index + 1).is_multiple_of(COST_CHECKPOINT_SYMBOLS) {
            checkpoint(token)?;
        }
    }
    Ok(())
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
    // Each population cost is bounded by the VP8L alphabet and fixed-point
    // scale, so the reference representation is guaranteed to fit `i32`.
    population_cost_in_place(&mut green);
    population_cost_in_place(&mut red);
    population_cost_in_place(&mut blue);
    population_cost_in_place(&mut alpha);
    population_cost_in_place(&mut distance);
    CostModel {
        green,
        red,
        blue,
        alpha,
        distance,
    }
}

fn cost_model_with_checkpoint(
    tokens: &[Token],
    cache_bits: u8,
    width: usize,
    token: CheckpointToken<'_>,
) -> CheckpointResult<CostModel> {
    if token.is_none() {
        return Ok(cost_model(tokens, cache_bits, width));
    }
    let cache_size = if cache_bits == 0 { 0 } else { 1 << cache_bits };
    let mut green = vec![0_u32; 280 + cache_size];
    let mut red = [0_u32; 256];
    let mut blue = [0_u32; 256];
    let mut alpha = [0_u32; 256];
    let mut distance = [0_u32; 40];
    for (index, &item) in tokens.iter().enumerate() {
        match item {
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
            Token::Cache(cache_index) => green[280 + cache_index] += 1,
        }
        if (index + 1).is_multiple_of(COST_CHECKPOINT_TOKENS) {
            checkpoint(token)?;
        }
    }
    // Each population cost is bounded by the VP8L alphabet and fixed-point
    // scale, so the reference representation is guaranteed to fit `i32`.
    population_cost_in_place_with_checkpoint(&mut green, token)?;
    population_cost_in_place_with_checkpoint(&mut red, token)?;
    population_cost_in_place_with_checkpoint(&mut blue, token)?;
    population_cost_in_place_with_checkpoint(&mut alpha, token)?;
    population_cost_in_place_with_checkpoint(&mut distance, token)?;
    Ok(CostModel {
        green,
        red,
        blue,
        alpha,
        distance,
    })
}

#[derive(Clone, Copy)]
struct CostInterval {
    cost: i64,
    start: usize,
    end: usize,
    position: usize,
}

// Interval splitting is revisited for many candidate positions. Keep its
// bounded working vectors attached to the manager so repeated candidates do
// not allocate and discard the same temporary state.
#[derive(Default)]
struct CostManagerScratch {
    boundaries: Vec<usize>,
    additions: Vec<CostInterval>,
    overlaps: Vec<CostInterval>,
    rebuilt: Vec<CostInterval>,
    merged: Vec<CostInterval>,
}

struct CostManager {
    costs: Vec<i64>,
    lengths: Vec<usize>,
    length_costs: Vec<i64>,
    length_intervals: Vec<(i64, usize, usize)>,
    intervals: Vec<CostInterval>,
    scratch: CostManagerScratch,
}

impl CostManager {
    fn new(pixel_count: usize, model: &CostModel) -> Self {
        const SCALE: i64 = 1 << 23;
        let cache_size = pixel_count.min(MAX_LENGTH);
        let mut length_costs = Vec::with_capacity(cache_size);
        for value in 0..cache_size {
            let (symbol, extra) = if value == 0 { (0, 0) } else { prefix(value) };
            length_costs.push(i64::from(model.green[256 + symbol]) + i64::from(extra) * SCALE);
        }
        let mut length_intervals = Vec::new();
        let mut start = 0;
        while start < length_costs.len() {
            let cost = length_costs[start];
            let mut end = start + 1;
            while end < length_costs.len() && length_costs[end] == cost {
                end += 1;
            }
            length_intervals.push((cost, start, end));
            start = end;
        }
        Self {
            costs: vec![i64::MAX; pixel_count],
            lengths: vec![1; pixel_count],
            length_costs,
            length_intervals,
            intervals: Vec::new(),
            scratch: CostManagerScratch::default(),
        }
    }

    fn new_with_checkpoint(
        pixel_count: usize,
        model: &CostModel,
        token: CheckpointToken<'_>,
    ) -> CheckpointResult<Self> {
        // The token-aware trace must not spend a full bounded match-length
        // table or equal-cost interval pass without a cooperative boundary.
        // Keep the no-token constructor separate so ordinary encoding retains
        // its original tight setup path.
        let Some(token) = token else {
            return Ok(Self::new(pixel_count, model));
        };
        const SCALE: i64 = 1 << 23;
        let cache_size = pixel_count.min(MAX_LENGTH);
        let mut length_costs = Vec::with_capacity(cache_size);
        for value in 0..cache_size {
            let (symbol, extra) = if value == 0 { (0, 0) } else { prefix(value) };
            length_costs.push(i64::from(model.green[256 + symbol]) + i64::from(extra) * SCALE);
            if (value + 1).is_multiple_of(COST_MANAGER_CHECKPOINT_ENTRIES) {
                checkpoint(Some(token))?;
            }
        }
        let mut length_intervals = Vec::new();
        let mut start = 0;
        while start < length_costs.len() {
            let cost = length_costs[start];
            let mut end = start + 1;
            while end < length_costs.len() && length_costs[end] == cost {
                end += 1;
                if end.is_multiple_of(COST_MANAGER_CHECKPOINT_ENTRIES) {
                    checkpoint(Some(token))?;
                }
            }
            length_intervals.push((cost, start, end));
            start = end;
        }
        // The capacity reservations remain ordinary fallible allocations;
        // this policy does not promise recoverable OOM behavior. Once those
        // tables exist, initialize their pixel-sized contents cooperatively so
        // a caller token cannot be hidden behind two bulk vec! fills. Keep the
        // no-token constructor above on its original tight path.
        let mut costs = Vec::with_capacity(pixel_count);
        let mut lengths = Vec::with_capacity(pixel_count);
        for index in 0..pixel_count {
            costs.push(i64::MAX);
            lengths.push(1);
            if (index + 1).is_multiple_of(COST_MANAGER_CHECKPOINT_ENTRIES) {
                checkpoint(Some(token))?;
            }
        }
        Ok(Self {
            costs,
            lengths,
            length_costs,
            length_intervals,
            intervals: Vec::new(),
            scratch: CostManagerScratch::default(),
        })
    }

    fn update(&mut self, index: usize, position: usize, cost: i64) {
        if self.costs[index] > cost {
            self.costs[index] = cost;
            self.lengths[index] = index - position + 1;
        }
    }

    fn update_at(&mut self, index: usize, clean: bool) {
        // Scan the small interval set by index so each pixel update does not
        // allocate a temporary applicable-interval vector.
        for interval_index in 0..self.intervals.len() {
            let interval = self.intervals[interval_index];
            if interval.start > index {
                break;
            }
            if interval.end > index {
                self.update(index, interval.position, interval.cost);
            }
        }
        if clean {
            self.intervals.retain(|interval| interval.end > index);
        }
    }

    fn update_at_with_checkpoint(
        &mut self,
        index: usize,
        clean: bool,
        token: CheckpointToken<'_>,
        work: &mut usize,
    ) -> CheckpointResult<()> {
        // Preserve the original tight interval scan when no caller token is
        // present. The token-aware path can otherwise inspect a large
        // interval set between its outer pixel checkpoints.
        let Some(token) = token else {
            self.update_at(index, clean);
            return Ok(());
        };

        for &interval in &self.intervals {
            if interval.start > index {
                break;
            }
            checkpoint_cost_manager_update_work(Some(token), work)?;
        }
        for interval_index in 0..self.intervals.len() {
            let interval = self.intervals[interval_index];
            if interval.start > index {
                break;
            }
            if interval.end > index {
                self.update(index, interval.position, interval.cost);
                checkpoint_cost_manager_update_work(Some(token), work)?;
            }
        }
        if clean {
            let mut retained = 0;
            for read_index in 0..self.intervals.len() {
                let interval = self.intervals[read_index];
                if interval.end > index {
                    self.intervals[retained] = interval;
                    retained += 1;
                }
                checkpoint_cost_manager_update_work(Some(token), work)?;
            }
            self.intervals.truncate(retained);
        }
        Ok(())
    }

    fn insert_min_interval(&mut self, candidate: CostInterval) {
        if candidate.start >= candidate.end {
            return;
        }
        if self.intervals.len() >= 500 {
            for index in candidate.start..candidate.end {
                self.update(index, candidate.position, candidate.cost);
            }
            return;
        }

        self.scratch.boundaries.clear();
        self.scratch
            .boundaries
            .extend([candidate.start, candidate.end]);
        for interval_index in 0..self.intervals.len() {
            let interval = self.intervals[interval_index];
            if interval.end > candidate.start && interval.start < candidate.end {
                self.scratch
                    .boundaries
                    .push(interval.start.max(candidate.start));
                self.scratch
                    .boundaries
                    .push(interval.end.min(candidate.end));
            }
        }
        self.scratch.boundaries.sort_unstable();
        self.scratch.boundaries.dedup();

        self.scratch.additions.clear();
        for window_index in 0..self.scratch.boundaries.len().saturating_sub(1) {
            let start = self.scratch.boundaries[window_index];
            let end = self.scratch.boundaries[window_index + 1];
            let existing = self
                .intervals
                .iter()
                .find(|interval| interval.start <= start && interval.end >= end);
            if existing.is_none_or(|interval| candidate.cost < interval.cost) {
                self.scratch.additions.push(CostInterval {
                    start,
                    end,
                    ..candidate
                });
            }
        }

        if self.scratch.additions.is_empty() {
            return;
        }
        let mut old = std::mem::take(&mut self.intervals);
        let mut rebuilt = std::mem::take(&mut self.scratch.rebuilt);
        let mut merged = std::mem::take(&mut self.scratch.merged);
        rebuilt.clear();
        merged.clear();
        for interval in old.drain(..) {
            self.scratch.overlaps.clear();
            for addition_index in 0..self.scratch.additions.len() {
                let addition = self.scratch.additions[addition_index];
                if addition.end > interval.start && addition.start < interval.end {
                    self.scratch.overlaps.push(addition);
                }
            }
            if self.scratch.overlaps.is_empty() {
                rebuilt.push(interval);
                continue;
            }
            let mut cursor = interval.start;
            for overlap_index in 0..self.scratch.overlaps.len() {
                let addition = self.scratch.overlaps[overlap_index];
                if cursor < addition.start {
                    rebuilt.push(CostInterval {
                        end: addition.start,
                        start: cursor,
                        ..interval
                    });
                }
                cursor = cursor.max(addition.end);
            }
            if cursor < interval.end {
                rebuilt.push(CostInterval {
                    start: cursor,
                    ..interval
                });
            }
        }
        rebuilt.append(&mut self.scratch.additions);
        rebuilt.sort_by_key(|interval| interval.start);
        for interval in rebuilt.drain(..) {
            if let Some(last) = merged.last_mut()
                && last.end == interval.start
                && last.cost == interval.cost
                && last.position == interval.position
            {
                last.end = interval.end;
            } else {
                merged.push(interval);
            }
        }
        self.intervals = merged;
        self.scratch.rebuilt = old;
        self.scratch.merged = rebuilt;
    }

    fn insert_min_interval_with_checkpoint(
        &mut self,
        candidate: CostInterval,
        token: CheckpointToken<'_>,
    ) -> CheckpointResult<()> {
        if candidate.start >= candidate.end {
            return Ok(());
        }
        if self.intervals.len() >= 500 {
            // The saturated fallback can cover the full bounded match length;
            // keep a long update range cooperatively interruptible.
            for (offset, index) in (candidate.start..candidate.end).enumerate() {
                self.update(index, candidate.position, candidate.cost);
                if (offset + 1).is_multiple_of(COST_MANAGER_CHECKPOINT_ENTRIES) {
                    checkpoint(token)?;
                }
            }
            return Ok(());
        }

        // The interval set is still below saturation, but splitting and
        // rebuilding it can compare hundreds of existing intervals against
        // hundreds of candidate windows. Keep this token-aware path bounded
        // without adding polling to the ordinary no-token encoder.
        let mut work = 0usize;
        self.scratch.boundaries.clear();
        self.scratch
            .boundaries
            .extend([candidate.start, candidate.end]);
        for interval_index in 0..self.intervals.len() {
            let interval = self.intervals[interval_index];
            checkpoint_cost_manager_work(token, &mut work)?;
            if interval.end > candidate.start && interval.start < candidate.end {
                self.scratch
                    .boundaries
                    .push(interval.start.max(candidate.start));
                self.scratch
                    .boundaries
                    .push(interval.end.min(candidate.end));
            }
        }
        self.scratch.boundaries.sort_unstable();
        self.scratch.boundaries.dedup();

        self.scratch.additions.clear();
        for window_index in 0..self.scratch.boundaries.len().saturating_sub(1) {
            let start = self.scratch.boundaries[window_index];
            let end = self.scratch.boundaries[window_index + 1];
            let mut existing = None;
            for interval_index in 0..self.intervals.len() {
                let interval = self.intervals[interval_index];
                checkpoint_cost_manager_work(token, &mut work)?;
                if interval.start <= start && interval.end >= end {
                    existing = Some(interval);
                    break;
                }
            }
            if existing.is_none_or(|interval| candidate.cost < interval.cost) {
                self.scratch.additions.push(CostInterval {
                    start,
                    end,
                    ..candidate
                });
            }
        }

        if self.scratch.additions.is_empty() {
            return Ok(());
        }
        let mut old = std::mem::take(&mut self.intervals);
        let mut rebuilt = std::mem::take(&mut self.scratch.rebuilt);
        let mut merged = std::mem::take(&mut self.scratch.merged);
        rebuilt.clear();
        merged.clear();
        for interval in old.drain(..) {
            self.scratch.overlaps.clear();
            for addition_index in 0..self.scratch.additions.len() {
                let addition = self.scratch.additions[addition_index];
                checkpoint_cost_manager_work(token, &mut work)?;
                if addition.end > interval.start && addition.start < interval.end {
                    self.scratch.overlaps.push(addition);
                }
            }
            if self.scratch.overlaps.is_empty() {
                rebuilt.push(interval);
                continue;
            }
            let mut cursor = interval.start;
            for overlap_index in 0..self.scratch.overlaps.len() {
                let addition = self.scratch.overlaps[overlap_index];
                if cursor < addition.start {
                    rebuilt.push(CostInterval {
                        end: addition.start,
                        start: cursor,
                        ..interval
                    });
                }
                cursor = cursor.max(addition.end);
            }
            if cursor < interval.end {
                rebuilt.push(CostInterval {
                    start: cursor,
                    ..interval
                });
            }
        }
        rebuilt.append(&mut self.scratch.additions);
        rebuilt.sort_by_key(|interval| interval.start);
        for interval in rebuilt.drain(..) {
            checkpoint_cost_manager_work(token, &mut work)?;
            if let Some(last) = merged.last_mut()
                && last.end == interval.start
                && last.cost == interval.cost
                && last.position == interval.position
            {
                last.end = interval.end;
            } else {
                merged.push(interval);
            }
        }
        self.intervals = merged;
        self.scratch.rebuilt = old;
        self.scratch.merged = rebuilt;
        Ok(())
    }

    fn push(&mut self, distance_cost: i64, position: usize, length: usize) {
        if length < 10 {
            for index in position..position + length {
                let cost = distance_cost + self.length_costs[index - position];
                self.update(index, position, cost);
            }
            return;
        }
        for interval_index in 0..self.length_intervals.len() {
            let (length_cost, relative_start, relative_end) = self.length_intervals[interval_index];
            if relative_start >= length {
                break;
            }
            self.insert_min_interval(CostInterval {
                cost: distance_cost + length_cost,
                start: position + relative_start,
                end: position + relative_end.min(length),
                position,
            });
        }
    }

    fn push_with_checkpoint(
        &mut self,
        distance_cost: i64,
        position: usize,
        length: usize,
        token: CheckpointToken<'_>,
    ) -> CheckpointResult<()> {
        // Preserve the original tight path when no caller token is present.
        if token.is_none() {
            self.push(distance_cost, position, length);
            return Ok(());
        }
        if length < 10 {
            for index in position..position + length {
                let cost = distance_cost + self.length_costs[index - position];
                self.update(index, position, cost);
            }
            return Ok(());
        }
        for interval_index in 0..self.length_intervals.len() {
            let (length_cost, relative_start, relative_end) = self.length_intervals[interval_index];
            if relative_start >= length {
                break;
            }
            self.insert_min_interval_with_checkpoint(
                CostInterval {
                    cost: distance_cost + length_cost,
                    start: position + relative_start,
                    end: position + relative_end.min(length),
                    position,
                },
                token,
            )?;
            if (interval_index + 1).is_multiple_of(COST_MANAGER_CHECKPOINT_ENTRIES) {
                checkpoint(token)?;
            }
        }
        Ok(())
    }
}

fn trace_backwards(
    pixels: &[u32],
    width: usize,
    chain: &[(usize, usize)],
    source: &[Token],
    cache_bits: u8,
    token: CheckpointToken<'_>,
) -> CheckpointResult<Vec<Token>> {
    if token.is_none() {
        return trace_backwards_impl::<false>(pixels, width, chain, source, cache_bits, token);
    }
    trace_backwards_impl::<true>(pixels, width, chain, source, cache_bits, token)
}

fn trace_backwards_impl<const FINE_TRACE: bool>(
    pixels: &[u32],
    width: usize,
    chain: &[(usize, usize)],
    source: &[Token],
    cache_bits: u8,
    token: CheckpointToken<'_>,
) -> CheckpointResult<Vec<Token>> {
    checkpoint(token)?;
    const SCALE: i64 = 1 << 23;
    let model = cost_model_with_checkpoint(source, cache_bits, width, token)?;
    let mut manager = CostManager::new_with_checkpoint(pixels.len(), &model, token)?;
    checkpoint(token)?;
    let mut cache = vec![0_u32; if cache_bits == 0 { 0 } else { 1 << cache_bits }];
    let mut update_work = 0usize;

    let mut add_literal = |position: usize, previous_cost: i64, manager: &mut CostManager| {
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
        if candidate < manager.costs[position] {
            manager.costs[position] = candidate;
            manager.lengths[position] = 1;
        }
    };

    add_literal(0, 0, &mut manager);
    let mut previous_offset = usize::MAX;
    let mut previous_length = usize::MAX;
    let mut offset_cost = 0_i64;
    let mut first_constant = false;
    let mut reach = 0_usize;

    const TRACE_CHECKPOINT_PIXELS: usize = 256;
    let mut next_checkpoint = if FINE_TRACE {
        TRACE_CHECKPOINT_PIXELS
    } else {
        1024
    };
    for position in 1..pixels.len() {
        if position >= next_checkpoint {
            checkpoint(token)?;
            next_checkpoint = position.saturating_add(if FINE_TRACE {
                TRACE_CHECKPOINT_PIXELS
            } else {
                1024
            });
        }
        let previous_cost = manager.costs[position - 1];
        let (distance, maximum_length) = chain[position];
        add_literal(position, previous_cost, &mut manager);

        if maximum_length >= 2 {
            if distance != previous_offset {
                let plane_distance = plane_code(width, distance);
                let (distance_symbol, distance_extra) = prefix(plane_distance);
                offset_cost =
                    i64::from(model.distance[distance_symbol]) + i64::from(distance_extra) * SCALE;
                first_constant = true;
                manager.push_with_checkpoint(
                    previous_cost + offset_cost,
                    position,
                    maximum_length,
                    token,
                )?;
            } else {
                if first_constant {
                    reach = position - 1 + previous_length - 1;
                    first_constant = false;
                }
                if position + maximum_length - 1 > reach {
                    let mut split = position;
                    let mut split_length = 0;
                    let mut split_checkpoint = split.saturating_add(256);
                    while split <= reach {
                        if split >= split_checkpoint {
                            checkpoint(token)?;
                            split_checkpoint = split.saturating_add(256);
                        }
                        let (next_offset, next_length) = chain[split + 1];
                        split_length = next_length;
                        if next_offset != distance {
                            split_length = chain[split].1;
                            break;
                        }
                        split += 1;
                    }
                    manager.update_at_with_checkpoint(split - 1, false, token, &mut update_work)?;
                    manager.update_at_with_checkpoint(split, false, token, &mut update_work)?;
                    manager.push_with_checkpoint(
                        manager.costs[split - 1] + offset_cost,
                        split,
                        split_length,
                        token,
                    )?;
                    reach = split + split_length - 1;
                }
            }
        }
        manager.update_at_with_checkpoint(position, true, token, &mut update_work)?;
        previous_offset = distance;
        previous_length = maximum_length;
    }

    let mut path = Vec::new();
    let mut end = pixels.len();
    if FINE_TRACE {
        let mut processed = 0_usize;
        let mut next_checkpoint = TRACE_CHECKPOINT_PIXELS;
        while end != 0 {
            let length = manager.lengths[end - 1];
            path.push(length);
            end -= length;
            processed = processed.saturating_add(length);
            while processed >= next_checkpoint {
                checkpoint(token)?;
                next_checkpoint = next_checkpoint.saturating_add(TRACE_CHECKPOINT_PIXELS);
            }
        }
    } else {
        while end != 0 {
            if end.is_multiple_of(1024) {
                checkpoint(token)?;
            }
            let length = manager.lengths[end - 1];
            path.push(length);
            end -= length;
        }
    }
    path.reverse();

    let mut output = Vec::with_capacity(path.len());
    let mut cache = vec![0_u32; if cache_bits == 0 { 0 } else { 1 << cache_bits }];
    let mut position: usize = 0;
    if FINE_TRACE {
        checkpoint(token)?;
        let mut next_checkpoint = TRACE_CHECKPOINT_PIXELS;
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
                populate_cache(pixels, position, length, cache_bits, &mut cache, token)?;
            }
            position += length;
            while position >= next_checkpoint {
                checkpoint(token)?;
                next_checkpoint = next_checkpoint.saturating_add(TRACE_CHECKPOINT_PIXELS);
            }
        }
    } else {
        for length in path {
            if position.is_multiple_of(1024) {
                checkpoint(token)?;
            }
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
                populate_cache(pixels, position, length, cache_bits, &mut cache, token)?;
            }
            position += length;
        }
    }
    Ok(output)
}

pub(super) fn candidates(
    pixels: &[u32],
    width: usize,
    allow_cache: bool,
    quality: u32,
    max_cache_bits: u8,
    token: CheckpointToken<'_>,
) -> CheckpointResult<Vec<(Vec<Token>, u8)>> {
    if pixels.is_empty() {
        return Ok(vec![(Vec::new(), 0)]);
    }
    let chain = fill_hash_chain(pixels, width, quality, token)?;
    let choose_cache = |source: Vec<Token>| -> CheckpointResult<(Vec<Token>, u8, u64)> {
        let maximum = if allow_cache { max_cache_bits } else { 0 };
        // The inclusive range always contains cache-bit value zero.
        let mut best = None;
        for bits in 0..=maximum {
            checkpoint(token)?;
            let cached = with_cache(pixels, &source, bits, token)?;
            let cost = cache_estimated_bits_with_checkpoint(&cached, bits, token)?;
            if best
                .as_ref()
                .is_none_or(|(_, _, best_cost)| cost < *best_cost)
            {
                best = Some((cached, bits, cost));
            }
        }
        let Some((tokens, bits, _)) = best else {
            unreachable!("the inclusive cache-bit range is never empty");
        };
        let cost = estimated_bits_with_checkpoint(&tokens, bits, token)?;
        checkpoint(token)?;
        Ok((tokens, bits, cost))
    };
    let improve = |mut candidate: (Vec<Token>, u8, u64),
                   source_chain: &[(usize, usize)]|
     -> CheckpointResult<(Vec<Token>, u8, u64)> {
        if quality >= 25 {
            checkpoint(token)?;
            let traced = trace_backwards(
                pixels,
                width,
                source_chain,
                &candidate.0,
                candidate.1,
                token,
            )?;
            let cost = estimated_bits_with_checkpoint(&traced, candidate.1, token)?;
            if cost < candidate.2 {
                candidate = (traced, candidate.1, cost);
            }
        }
        Ok(candidate)
    };

    let standard = choose_cache(lz77(pixels, width, &chain, token)?)?;
    let rle = choose_cache(rle(pixels, width, token)?)?;
    let mut primary = if standard.2 <= rle.2 {
        improve(standard, &chain)?
    } else {
        rle
    };
    let mut result = vec![(std::mem::take(&mut primary.0), primary.1)];

    // libwebp evaluates its low-distance "box" chain as a separate crunch
    // configuration for palette images containing at most sixteen colors.
    if allow_cache && max_cache_bits <= 4 {
        let chain = box_chain(pixels, width, &chain, token)?;
        let mut box_candidate =
            improve(choose_cache(lz77(pixels, width, &chain, token)?)?, &chain)?;
        result.push((std::mem::take(&mut box_candidate.0), box_candidate.1));
    }
    Ok(result)
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    assert!(matches!(
        candidates(&[], 1, true, 80, 0, None).map(|items| items.len()),
        Ok(1)
    ));
    let _ = candidates(&[0xff00_0000; MAX_LENGTH + 4], 1, false, 60, 0, None);
    let alternating = (0..MAX_LENGTH + 260)
        .map(|index| {
            if index % 2 == 0 {
                0xff00_0000
            } else {
                0xff00_0001
            }
        })
        .collect::<Vec<_>>();
    let _ = candidates(&alternating, 2, false, 100, 0, None);
    let long_periodic = (0..(MAX_LENGTH * 3 + 8))
        .map(|index| {
            if index % 2 == 0 {
                0xff00_0000
            } else {
                0xff00_0001
            }
        })
        .collect::<Vec<_>>();
    let _ = candidates(&long_periodic, 2, false, 100, 0, None);

    // Defensive optimizer-state model (TST-010), not Pillow parity evidence.
    // `box_chain` consumes a hash-chain heuristic whose retained offset/length
    // state cannot be selected independently through an encoded image. Both
    // states below are valid for this repeated input: the first is a maximum
    // low-distance match that needs no recomputation; the second is a maximum
    // non-window match that must be recomputed by the box-distance model.
    let long_uniform = vec![0xff00_0000; MAX_LENGTH * 3 + 8];
    let mut retained_chain = vec![(0, 0); long_uniform.len()];
    retained_chain[1] = (1, MAX_LENGTH);
    retained_chain[MAX_LENGTH + 1] = (MAX_LENGTH + 1, MAX_LENGTH);
    let _ = box_chain(&long_uniform, 1, &retained_chain, None);

    let _ = fast_slog(70_000);
    let _ = prefix(300);
    let mut population = [70_000, 1];
    population_cost_in_place(&mut population);
    let model = cost_model(&[Token::Literal(0xff00_0000)], 0, 1);
    let mut manager = CostManager::new(8, &model);
    manager.insert_min_interval(CostInterval {
        cost: 0,
        start: 1,
        end: 1,
        position: 0,
    });
    manager.intervals = vec![
        CostInterval {
            cost: 1,
            start: 0,
            end: 4,
            position: 0,
        };
        500
    ];
    manager.insert_min_interval(CostInterval {
        cost: 0,
        start: 0,
        end: 3,
        position: 0,
    });
    let mut manager = CostManager::new(8, &model);
    manager.insert_min_interval(CostInterval {
        cost: 5,
        start: 0,
        end: 2,
        position: 0,
    });
    manager.insert_min_interval(CostInterval {
        cost: 5,
        start: 2,
        end: 4,
        position: 0,
    });
    manager.insert_min_interval(CostInterval {
        cost: 6,
        start: 1,
        end: 3,
        position: 0,
    });
    let mut manager = CostManager::new(8, &model);
    manager.intervals = vec![
        CostInterval {
            cost: 5,
            start: 0,
            end: 1,
            position: 0,
        },
        CostInterval {
            cost: 5,
            start: 3,
            end: 4,
            position: 0,
        },
    ];
    manager.insert_min_interval(CostInterval {
        cost: 0,
        start: 0,
        end: 4,
        position: 0,
    });
    let mut manager = CostManager::new(8, &model);
    manager.insert_min_interval(CostInterval {
        cost: 1,
        start: 0,
        end: 1,
        position: 0,
    });
    manager.insert_min_interval(CostInterval {
        cost: 2,
        start: 1,
        end: 2,
        position: 0,
    });
    manager.insert_min_interval(CostInterval {
        cost: 2,
        start: 2,
        end: 3,
        position: 1,
    });
    manager.insert_min_interval(CostInterval {
        cost: 2,
        start: 4,
        end: 5,
        position: 1,
    });
    let _ = std::panic::catch_unwind(|| {
        let pixels = [0xff00_0000; 8];
        let chain = [
            (0, 0),
            (1, 3),
            (1, 5),
            (2, 4),
            (1, 2),
            (1, 2),
            (1, 1),
            (1, 1),
        ];
        let source = [
            Token::Literal(0xff00_0000),
            Token::Copy {
                distance: 1,
                length: 3,
            },
            Token::Copy {
                distance: 1,
                length: 5,
            },
        ];
        let _ = trace_backwards(&pixels, 1, &chain, &source, 0, None);
    });
    let _ = std::panic::catch_unwind(|| {
        let _ = with_cache(&[0xff00_0000], &[Token::Cache(0)], 1, None);
    });
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
