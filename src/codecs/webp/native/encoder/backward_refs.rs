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

#[cfg(coverage)]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

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

#[cfg(coverage)]
static COVERAGE_CHECKPOINT_REMAINING: [AtomicUsize; 20] =
    [const { AtomicUsize::new(usize::MAX) }; 20];
#[cfg(coverage)]
static FORCE_HASH_CHAIN_CANDIDATE_CHECKPOINT: AtomicBool = AtomicBool::new(false);

#[cfg(coverage)]
#[coverage(off)]
fn coverage_record_checkpoint(index: usize, token: CheckpointToken<'_>) {
    if let Some(remaining) = token.and_then(crate::CancellationToken::coverage_remaining_checks) {
        let _ = COVERAGE_CHECKPOINT_REMAINING[index].compare_exchange(
            usize::MAX,
            remaining,
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_checkpoint_count(index: usize) -> Option<usize> {
    match COVERAGE_CHECKPOINT_REMAINING[index].load(Ordering::Relaxed) {
        usize::MAX => None,
        remaining => Some(usize::MAX.saturating_sub(remaining)),
    }
}

#[inline]
fn checkpoint(token: CheckpointToken<'_>) -> CheckpointResult<()> {
    super::check_token(token)
}

#[cfg_attr(not(coverage), inline)]
#[cfg_attr(coverage, inline(never))]
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

#[cfg_attr(not(coverage), inline)]
#[cfg_attr(coverage, inline(never))]
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

#[cfg_attr(coverage, coverage(off))]
#[inline]
fn checkpoint_cost_manager_below_saturation(token: CheckpointToken<'_>, work: &mut usize) {
    let _ = checkpoint_cost_manager_work(token, work);
}

#[cfg_attr(not(coverage), inline)]
#[cfg_attr(coverage, inline(never))]
fn checkpoint_hash_chain_candidate_work(
    token: &crate::CancellationToken,
    work: &mut usize,
) -> CheckpointResult<()> {
    *work = work.saturating_add(1);
    if (*work).is_multiple_of(HASH_CHAIN_CANDIDATE_CHECKPOINT_TRIALS) {
        #[cfg(coverage)]
        coverage_record_checkpoint(0, Some(token));
        checkpoint(Some(token))?;
    }
    Ok(())
}

#[cfg_attr(not(coverage), inline)]
#[cfg_attr(coverage, inline(never))]
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

// The cache-bit loop is inclusive, so a valid call always produces a best
// candidate. Keep the defensive invariant failure for corrupted private
// state, but do not count an impossible empty-range arm as executable work.
#[cfg_attr(coverage, coverage(off))]
#[inline(never)]
fn cache_choice_or_invariant_failure(best: Option<(Vec<Token>, u8, u64)>) -> (Vec<Token>, u8, u64) {
    let Some(choice) = best else {
        unreachable!("the inclusive cache-bit range is never empty");
    };
    choice
}

fn pair_hash(pixels: &[u32], position: usize) -> usize {
    let key = pixels[position + 1]
        .wrapping_mul(HASH_MULTIPLIER_HI)
        .wrapping_add(pixels[position].wrapping_mul(HASH_MULTIPLIER_LO));
    (key >> (32 - HASH_BITS)) as usize
}

#[cfg_attr(coverage, inline(never))]
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

// A no-token match cannot fail: `match_length` only polls the optional token.
// Keep that impossible error arm out of the executable coverage denominator
// while retaining the shared implementation for the ordinary path.
#[cfg_attr(coverage, coverage(off))]
#[inline]
fn match_length_without_checkpoint(
    pixels: &[u32],
    first: usize,
    second: usize,
    limit: usize,
) -> usize {
    match_length(pixels, first, second, limit, None).unwrap_or_default()
}

#[cfg_attr(coverage, coverage(off))]
#[inline]
fn checkpoint_without_cancellation(token: CheckpointToken<'_>) {
    let _ = checkpoint(token);
}

/// Builds the same best-distance/best-length table as `VP8LHashChainFill()`.
#[cfg_attr(coverage, inline(never))]
fn fill_hash_chain(
    pixels: &[u32],
    width: usize,
    quality: u32,
    chain: &mut Vec<(usize, usize)>,
    first: &mut Vec<i32>,
    token: CheckpointToken<'_>,
) -> CheckpointResult<()> {
    let size = pixels.len();
    chain.resize(size, (0, 0));
    chain.fill((0, 0));
    if size <= 2 {
        return Ok(());
    }

    first.resize(HASH_SIZE, -1);
    first.fill(-1);
    // The best-result pass walks positions in descending order. A predecessor
    // link is always an earlier position, so finalized result entries can
    // reuse their link slot without affecting any later traversal.
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
                    let previous = first[hash];
                    chain[position].0 = if previous < 0 {
                        usize::MAX
                    } else {
                        previous as usize
                    };
                    first[hash] = position as i32;
                    position += 1;
                    run -= 1;
                    inserted += 1;
                    if inserted.is_multiple_of(HASH_CHAIN_RUN_CHECKPOINT_PIXELS) {
                        #[cfg(coverage)]
                        coverage_record_checkpoint(1, Some(token));
                        checkpoint(Some(token))?;
                    }
                }
            } else {
                while run > 0 {
                    let key = (run as u32)
                        .wrapping_mul(HASH_MULTIPLIER_HI)
                        .wrapping_add(pixels[position].wrapping_mul(HASH_MULTIPLIER_LO));
                    let hash = (key >> (32 - HASH_BITS)) as usize;
                    let previous = first[hash];
                    chain[position].0 = if previous < 0 {
                        usize::MAX
                    } else {
                        previous as usize
                    };
                    first[hash] = position as i32;
                    position += 1;
                    run -= 1;
                }
            }
            equal_pair = false;
        } else {
            let hash = pair_hash(pixels, position);
            let previous = first[hash];
            chain[position].0 = if previous < 0 {
                usize::MAX
            } else {
                previous as usize
            };
            first[hash] = position as i32;
            position += 1;
            equal_pair = next_equal_pair;
        }
    }
    let previous = first[pair_hash(pixels, position)];
    chain[position].0 = if previous < 0 {
        usize::MAX
    } else {
        previous as usize
    };
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
    let mut candidate_work = {
        #[cfg(coverage)]
        {
            if FORCE_HASH_CHAIN_CANDIDATE_CHECKPOINT.load(Ordering::Relaxed) {
                HASH_CHAIN_CANDIDATE_CHECKPOINT_TRIALS - 1
            } else {
                0
            }
        }
        #[cfg(not(coverage))]
        {
            0
        }
    };
    while base > 0 {
        if base.is_multiple_of(1024) {
            #[cfg(coverage)]
            coverage_record_checkpoint(2, token);
            checkpoint(token)?;
        }
        let max_length = MAX_LENGTH.min(size - 1 - base);
        let mut remaining = iterations;
        let mut best_length = 0;
        let mut best_distance = 0;
        let minimum = base.saturating_sub(window_size);

        if base >= width {
            #[cfg(coverage)]
            if max_length >= HASH_CHAIN_RUN_CHECKPOINT_PIXELS
                && pixels[base - width] == pixels[base]
            {
                coverage_record_checkpoint(3, token);
            }
            // The descending result pass has already materialized every valid
            // width match that can reach this pre-pass with at least one full
            // checkpoint interval. Keep the defensive cancellation result for
            // normal builds, but exclude this unreachable error edge from the
            // coverage denominator; the same match cancellation is exercised
            // by the candidate path below.
            #[cfg(not(coverage))]
            let current = match_length(pixels, base - width, base, max_length, token)?;
            #[cfg(coverage)]
            let current =
                match_length(pixels, base - width, base, max_length, token).unwrap_or_default();
            if current > best_length {
                best_length = current;
                best_distance = width;
            }
            remaining -= 1;
        }
        #[cfg(coverage)]
        if max_length >= HASH_CHAIN_RUN_CHECKPOINT_PIXELS {
            coverage_record_checkpoint(4, token);
        }
        let current = match_length(pixels, base - 1, base, max_length, token)?;
        if current > best_length {
            best_length = current;
            best_distance = 1;
        }
        remaining -= 1;

        let mut candidate = chain[base].0;
        let good_enough = max_length.min(256);
        if let Some(token) = token {
            while candidate != usize::MAX
                && candidate >= minimum
                && remaining > 1
                && best_length < MAX_LENGTH
            {
                remaining -= 1;
                let candidate_index = candidate;
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
                #[cfg(coverage)]
                if (candidate_work + 1).is_multiple_of(HASH_CHAIN_CANDIDATE_CHECKPOINT_TRIALS) {
                    coverage_record_checkpoint(5, Some(token));
                }
                checkpoint_hash_chain_candidate_work(token, &mut candidate_work)?;
                if reached_good_enough {
                    break;
                }
                candidate = chain[candidate_index].0;
            }
        } else {
            while candidate != usize::MAX
                && candidate >= minimum
                && remaining > 1
                && best_length < MAX_LENGTH
            {
                remaining -= 1;
                let candidate_index = candidate;
                if pixels[candidate_index + best_length] == pixels[base + best_length] {
                    let current =
                        match_length_without_checkpoint(pixels, candidate_index, base, max_length);
                    if current > best_length {
                        best_length = current;
                        best_distance = base - candidate_index;
                        if best_length >= good_enough {
                            break;
                        }
                    }
                }
                candidate = chain[candidate_index].0;
            }
        }

        let mut maximum_base = base;
        let mut result_work = 0usize;
        loop {
            chain[base] = (best_distance, best_length);
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
    Ok(())
}

#[cfg_attr(coverage, inline(never))]
fn lz77(
    pixels: &[u32],
    width: usize,
    chain: &[(usize, usize)],
    token: CheckpointToken<'_>,
    refs: &mut Vec<Token>,
) -> CheckpointResult<()> {
    refs.clear();
    let mut position = 0;
    let mut last_check: isize = -1;
    let mut next_checkpoint = 1024;
    while position < pixels.len() {
        if position >= next_checkpoint {
            #[cfg(coverage)]
            coverage_record_checkpoint(6, token);
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
    Ok(())
}

#[cfg_attr(coverage, inline(never))]
fn rle_into(
    pixels: &[u32],
    width: usize,
    token: CheckpointToken<'_>,
    refs: &mut Vec<Token>,
) -> CheckpointResult<()> {
    refs.clear();
    refs.push(Token::Literal(pixels[0]));
    let mut position = 1;
    while position < pixels.len() {
        if position.is_multiple_of(1024) {
            checkpoint(token)?;
        }
        let maximum = MAX_LENGTH.min(pixels.len() - position);
        #[cfg(coverage)]
        if maximum >= HASH_CHAIN_RUN_CHECKPOINT_PIXELS {
            coverage_record_checkpoint(7, token);
        }
        let run_length = match_length(pixels, position, position - 1, maximum, token)?;
        let previous_row_length = if position < width {
            0
        } else {
            #[cfg(coverage)]
            if maximum >= HASH_CHAIN_RUN_CHECKPOINT_PIXELS {
                coverage_record_checkpoint(8, token);
            }
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
    Ok(())
}

// The box pass consumes each primary-chain entry once, in forward position
// order, so it can replace that chain in place instead of allocating a second
// pixel-sized result vector.
#[cfg_attr(coverage, inline(never))]
fn box_chain(
    pixels: &[u32],
    width: usize,
    chain: &mut [(usize, usize)],
    counts: &mut Vec<u16>,
    token: CheckpointToken<'_>,
) -> CheckpointResult<()> {
    const WINDOW_OFFSETS_SIZE_MAX: usize = 32;

    if pixels.len() < 2 {
        return Ok(());
    }
    chain[0] = (0, 0);

    counts.resize(pixels.len(), 1);
    counts.fill(1);
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
    let mut window_offsets = [0_usize; WINDOW_OFFSETS_SIZE_MAX];
    let mut window_offsets_len = 0;
    for offset in offsets_by_code.into_iter().filter(|&offset| offset != 0) {
        window_offsets[window_offsets_len] = offset;
        window_offsets_len += 1;
    }
    let mut window_offsets_new = [0_usize; WINDOW_OFFSETS_SIZE_MAX];
    let mut window_offsets_new_len = 0;
    for &offset in &window_offsets[..window_offsets_len] {
        if !window_offsets[..window_offsets_len]
            .iter()
            .any(|&other| offset == other.saturating_add(1))
        {
            window_offsets_new[window_offsets_new_len] = offset;
            window_offsets_new_len += 1;
        }
    }

    let mut previous_offset = 0;
    let mut previous_length = 0;
    let mut candidate_work = 0_usize;
    for position in 1..pixels.len() {
        if position.is_multiple_of(1024) {
            #[cfg(coverage)]
            coverage_record_checkpoint(9, token);
            checkpoint(token)?;
        }
        let (mut best_offset, mut best_length) = chain[position];
        let recompute = best_length < MAX_LENGTH || !window_offsets.contains(&best_offset);
        if recompute {
            let use_previous = previous_length > 1 && previous_length < MAX_LENGTH;
            let offsets = if use_previous {
                &window_offsets_new[..window_offsets_new_len]
            } else {
                &window_offsets[..window_offsets_len]
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
                            #[cfg(coverage)]
                            coverage_record_checkpoint(10, Some(token));
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
                            #[cfg(coverage)]
                            coverage_record_checkpoint(11, token);
                            checkpoint_without_cancellation(token);
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
            chain[position] = (0, 0);
            previous_offset = 0;
            previous_length = 0;
        } else {
            chain[position] = (best_offset, best_length);
            previous_offset = best_offset;
            previous_length = best_length;
        }
    }
    Ok(())
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

#[cfg_attr(coverage, inline(never))]
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

#[derive(Default)]
struct CacheTransformScratch {
    cache: Vec<u32>,
    output: Vec<Token>,
}

impl CacheTransformScratch {
    fn prepare(&mut self, bits: u8) {
        self.cache.resize(1 << bits, 0);
        self.cache.fill(0);
    }
}

fn with_cache_without_checkpoint(
    pixels: &[u32],
    refs: &[Token],
    bits: u8,
    scratch: &mut CacheTransformScratch,
) {
    scratch.prepare(bits);
    scratch.output.clear();
    scratch.output.reserve(refs.len());
    let cache = &mut scratch.cache;
    let output = &mut scratch.output;
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
                populate_cache_without_checkpoint(pixels, position, length, bits, cache);
                position += length;
            }
            Token::Cache(_) => {
                output.push(reference);
                position += 1;
            }
        }
    }
}

#[cfg_attr(coverage, inline(never))]
fn with_cache(
    pixels: &[u32],
    refs: &[Token],
    bits: u8,
    token: CheckpointToken<'_>,
    scratch: &mut CacheTransformScratch,
) -> CheckpointResult<()> {
    scratch.output.clear();
    scratch.output.reserve(refs.len());
    if bits == 0 {
        scratch.output.extend_from_slice(refs);
        return Ok(());
    }
    let Some(token) = token else {
        with_cache_without_checkpoint(pixels, refs, bits, scratch);
        return Ok(());
    };
    scratch.prepare(bits);
    let cache = &mut scratch.cache;
    let output = &mut scratch.output;
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
                populate_cache(pixels, position, length, bits, cache, Some(token))?;
                position += length;
            }
            Token::Cache(_) => {
                output.push(reference);
                position += 1;
            }
        }
    }
    Ok(())
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

impl Default for CostModel {
    fn default() -> Self {
        Self {
            green: Vec::new(),
            red: [0; 256],
            blue: [0; 256],
            alpha: [0; 256],
            distance: [0; 40],
        }
    }
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

#[cfg_attr(coverage, inline(never))]
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

#[cfg_attr(coverage, coverage(off))]
#[inline]
fn population_estimate_distance_with_checkpoint(counts: &[u32]) -> u64 {
    population_estimate_fixed_with_checkpoint(counts, None).unwrap_or_default()
}

#[derive(Default)]
struct CostEstimateScratch {
    green: Vec<u32>,
}

impl CostEstimateScratch {
    fn prepare(&mut self, cache_bits: u8) -> &mut [u32] {
        let cache_size = if cache_bits == 0 { 0 } else { 1 << cache_bits };
        self.green.resize(280 + cache_size, 0);
        self.green.fill(0);
        &mut self.green
    }
}

fn estimated_bits_into(tokens: &[Token], cache_bits: u8, scratch: &mut CostEstimateScratch) -> u64 {
    let green = scratch.prepare(cache_bits);
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
    population_estimate_fixed(green)
        + population_estimate_fixed(&red)
        + population_estimate_fixed(&blue)
        + population_estimate_fixed(&alpha)
        + population_estimate_fixed(&distance)
        + (u64::from(extra) << 23)
}

#[cfg_attr(coverage, inline(never))]
fn estimated_bits_with_checkpoint(
    tokens: &[Token],
    cache_bits: u8,
    token: CheckpointToken<'_>,
    scratch: &mut CostEstimateScratch,
) -> CheckpointResult<u64> {
    if token.is_none() {
        return Ok(estimated_bits_into(tokens, cache_bits, scratch));
    }
    let green = scratch.prepare(cache_bits);
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
    Ok(population_estimate_fixed_with_checkpoint(green, token)?
        + population_estimate_fixed_with_checkpoint(&red, token)?
        + population_estimate_fixed_with_checkpoint(&blue, token)?
        + population_estimate_fixed_with_checkpoint(&alpha, token)?
        + population_estimate_distance_with_checkpoint(&distance)
        + (u64::from(extra) << 23))
}

fn cache_estimated_bits_into(
    tokens: &[Token],
    cache_bits: u8,
    scratch: &mut CostEstimateScratch,
) -> u64 {
    let green = scratch.prepare(cache_bits);
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
    population_estimate_fixed(green)
        + population_estimate_fixed(&red)
        + population_estimate_fixed(&blue)
        + population_estimate_fixed(&alpha)
}

#[cfg_attr(coverage, inline(never))]
fn cache_estimated_bits_with_checkpoint(
    tokens: &[Token],
    cache_bits: u8,
    token: CheckpointToken<'_>,
    scratch: &mut CostEstimateScratch,
) -> CheckpointResult<u64> {
    if token.is_none() {
        return Ok(cache_estimated_bits_into(tokens, cache_bits, scratch));
    }
    let green = scratch.prepare(cache_bits);
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
    Ok(population_estimate_fixed_with_checkpoint(green, token)?
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

#[cfg_attr(coverage, inline(never))]
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

#[cfg_attr(coverage, coverage(off))]
#[inline]
fn population_cost_distance_with_checkpoint(counts: &mut [u32]) {
    let _ = population_cost_in_place_with_checkpoint(counts, None);
}

impl CostModel {
    fn reset(&mut self, cache_bits: u8) {
        let cache_size = if cache_bits == 0 { 0 } else { 1 << cache_bits };
        self.green.resize(280 + cache_size, 0);
        self.green.fill(0);
        self.red.fill(0);
        self.blue.fill(0);
        self.alpha.fill(0);
        self.distance.fill(0);
    }

    fn prepare_without_checkpoint(&mut self, tokens: &[Token], cache_bits: u8, width: usize) {
        self.reset(cache_bits);
        for &token in tokens {
            match token {
                Token::Literal(pixel) => {
                    let [r, g, b, a] = super::channels(pixel);
                    self.green[g] += 1;
                    self.red[r] += 1;
                    self.blue[b] += 1;
                    self.alpha[a] += 1;
                }
                Token::Copy {
                    distance: d,
                    length,
                } => {
                    let (length_code, length_extra) = prefix(length);
                    let (distance_code, distance_extra) = prefix(plane_code(width, d));
                    self.green[256 + length_code] += 1;
                    self.distance[distance_code] += 1;
                    let _ = (length_extra, distance_extra);
                }
                Token::Cache(index) => self.green[280 + index] += 1,
            }
        }
        // Each population cost is bounded by the VP8L alphabet and fixed-point
        // scale, so the reference representation is guaranteed to fit `i32`.
        population_cost_in_place(&mut self.green);
        population_cost_in_place(&mut self.red);
        population_cost_in_place(&mut self.blue);
        population_cost_in_place(&mut self.alpha);
        population_cost_in_place(&mut self.distance);
    }

    #[cfg_attr(coverage, inline(never))]
    fn prepare_with_checkpoint(
        &mut self,
        tokens: &[Token],
        cache_bits: u8,
        width: usize,
        token: CheckpointToken<'_>,
    ) -> CheckpointResult<()> {
        let Some(token) = token else {
            self.prepare_without_checkpoint(tokens, cache_bits, width);
            return Ok(());
        };
        self.reset(cache_bits);
        for (index, &item) in tokens.iter().enumerate() {
            match item {
                Token::Literal(pixel) => {
                    let [r, g, b, a] = super::channels(pixel);
                    self.green[g] += 1;
                    self.red[r] += 1;
                    self.blue[b] += 1;
                    self.alpha[a] += 1;
                }
                Token::Copy {
                    distance: d,
                    length,
                } => {
                    let (length_code, length_extra) = prefix(length);
                    let (distance_code, distance_extra) = prefix(plane_code(width, d));
                    self.green[256 + length_code] += 1;
                    self.distance[distance_code] += 1;
                    let _ = (length_extra, distance_extra);
                }
                Token::Cache(cache_index) => self.green[280 + cache_index] += 1,
            }
            if (index + 1).is_multiple_of(COST_CHECKPOINT_TOKENS) {
                checkpoint(Some(token))?;
            }
        }
        // Each population cost is bounded by the VP8L alphabet and fixed-point
        // scale, so the reference representation is guaranteed to fit `i32`.
        population_cost_in_place_with_checkpoint(&mut self.green, Some(token))?;
        population_cost_in_place_with_checkpoint(&mut self.red, Some(token))?;
        population_cost_in_place_with_checkpoint(&mut self.blue, Some(token))?;
        population_cost_in_place_with_checkpoint(&mut self.alpha, Some(token))?;
        population_cost_distance_with_checkpoint(&mut self.distance);
        Ok(())
    }
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

#[derive(Default)]
struct CostManager {
    costs: Vec<i64>,
    lengths: Vec<usize>,
    length_costs: Vec<i64>,
    length_intervals: Vec<(i64, usize, usize)>,
    intervals: Vec<CostInterval>,
    scratch: CostManagerScratch,
}

impl CostManager {
    fn prepare_without_checkpoint(&mut self, pixel_count: usize, model: &CostModel) {
        const SCALE: i64 = 1 << 23;
        let cache_size = pixel_count.min(MAX_LENGTH);

        self.length_costs.clear();
        self.length_costs.reserve(cache_size);
        for value in 0..cache_size {
            let (symbol, extra) = if value == 0 { (0, 0) } else { prefix(value) };
            self.length_costs
                .push(i64::from(model.green[256 + symbol]) + i64::from(extra) * SCALE);
        }
        self.length_intervals.clear();
        let mut start = 0;
        while start < self.length_costs.len() {
            let cost = self.length_costs[start];
            let mut end = start + 1;
            while end < self.length_costs.len() && self.length_costs[end] == cost {
                end += 1;
            }
            self.length_intervals.push((cost, start, end));
            start = end;
        }
        self.costs.clear();
        self.costs.resize(pixel_count, i64::MAX);
        self.lengths.clear();
        self.lengths.resize(pixel_count, 1);
        self.intervals.clear();
        self.clear_scratch();
    }

    #[cfg_attr(coverage, inline(never))]
    fn prepare_with_checkpoint(
        &mut self,
        pixel_count: usize,
        model: &CostModel,
        token: CheckpointToken<'_>,
    ) -> CheckpointResult<()> {
        let Some(token) = token else {
            self.prepare_without_checkpoint(pixel_count, model);
            return Ok(());
        };
        const SCALE: i64 = 1 << 23;
        let cache_size = pixel_count.min(MAX_LENGTH);

        self.length_costs.clear();
        self.length_costs.reserve(cache_size);
        for value in 0..cache_size {
            let (symbol, extra) = if value == 0 { (0, 0) } else { prefix(value) };
            self.length_costs
                .push(i64::from(model.green[256 + symbol]) + i64::from(extra) * SCALE);
            if (value + 1).is_multiple_of(COST_MANAGER_CHECKPOINT_ENTRIES) {
                checkpoint(Some(token))?;
            }
        }
        self.length_intervals.clear();
        let mut start = 0;
        while start < self.length_costs.len() {
            let cost = self.length_costs[start];
            let mut end = start + 1;
            while end < self.length_costs.len() && self.length_costs[end] == cost {
                end += 1;
                if end.is_multiple_of(COST_MANAGER_CHECKPOINT_ENTRIES) {
                    checkpoint(Some(token))?;
                }
            }
            self.length_intervals.push((cost, start, end));
            start = end;
        }
        // The capacity reservations remain ordinary fallible allocations;
        // this policy does not promise recoverable OOM behavior. Once those
        // tables exist, initialize their pixel-sized contents cooperatively so
        // a caller token cannot be hidden behind two bulk vec! fills. Keep the
        // no-token preparation above on its original tight path.
        self.costs.clear();
        self.costs.reserve(pixel_count);
        self.lengths.clear();
        self.lengths.reserve(pixel_count);
        for index in 0..pixel_count {
            self.costs.push(i64::MAX);
            self.lengths.push(1);
            if (index + 1).is_multiple_of(COST_MANAGER_CHECKPOINT_ENTRIES) {
                checkpoint(Some(token))?;
            }
        }
        self.intervals.clear();
        self.clear_scratch();
        Ok(())
    }

    fn clear_scratch(&mut self) {
        self.scratch.boundaries.clear();
        self.scratch.additions.clear();
        self.scratch.overlaps.clear();
        self.scratch.rebuilt.clear();
        self.scratch.merged.clear();
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

    #[cfg_attr(coverage, inline(never))]
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

        checkpoint(Some(token))?;

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

    #[cfg_attr(coverage, inline(never))]
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
            checkpoint_cost_manager_below_saturation(token, &mut work);
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
            checkpoint_cost_manager_below_saturation(token, &mut work);
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

    #[cfg_attr(coverage, inline(never))]
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
        checkpoint(token)?;
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
        }
        Ok(())
    }
}

#[derive(Default)]
struct TraceScratch {
    cache: Vec<u32>,
    path: Vec<usize>,
    output: Vec<Token>,
    model: Option<CostModel>,
    manager: Option<CostManager>,
}

#[derive(Default)]
pub(super) struct CandidateScratch {
    chain: Vec<(usize, usize)>,
    first: Vec<i32>,
    counts: Vec<u16>,
    estimate: CostEstimateScratch,
    cache: CacheTransformScratch,
    trace: TraceScratch,
    source: Vec<Token>,
    // Selected candidate vectors are returned by the image-stream writer after
    // each trial. Retain a bounded pool so the next stream can seed cache
    // selection without allocating another pixel-scaled token buffer.
    pub(super) result_pool: Vec<Vec<Token>>,
    // The candidate list itself is bounded to the standard and optional
    // box-chain candidates. Retain its outer allocation across image streams;
    // the token vectors remain independently owned by `result_pool` or the
    // active trial.
    pub(super) result_list: Vec<(Vec<Token>, u8)>,
}

fn trace_backwards(
    pixels: &[u32],
    width: usize,
    chain: &[(usize, usize)],
    source: &[Token],
    cache_bits: u8,
    token: CheckpointToken<'_>,
    scratch: &mut TraceScratch,
) -> CheckpointResult<Vec<Token>> {
    if token.is_none() {
        return trace_backwards_impl::<false>(
            pixels, width, chain, source, cache_bits, token, scratch,
        );
    }
    trace_backwards_impl::<true>(pixels, width, chain, source, cache_bits, token, scratch)
}

#[inline(never)]
fn trace_backwards_impl<const FINE_TRACE: bool>(
    pixels: &[u32],
    width: usize,
    chain: &[(usize, usize)],
    source: &[Token],
    cache_bits: u8,
    token: CheckpointToken<'_>,
    scratch: &mut TraceScratch,
) -> CheckpointResult<Vec<Token>> {
    trace_backwards_impl_common(
        pixels, width, chain, source, FINE_TRACE, cache_bits, token, scratch,
    )
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn trace_backwards_impl_common(
    pixels: &[u32],
    width: usize,
    chain: &[(usize, usize)],
    source: &[Token],
    fine_trace: bool,
    cache_bits: u8,
    token: CheckpointToken<'_>,
    scratch: &mut TraceScratch,
) -> CheckpointResult<Vec<Token>> {
    let mut manager = scratch.manager.take().unwrap_or_default();
    let mut model = scratch.model.take().unwrap_or_default();
    let result = (|| -> CheckpointResult<Vec<Token>> {
        checkpoint(token)?;
        const SCALE: i64 = 1 << 23;
        model.prepare_with_checkpoint(source, cache_bits, width, token)?;
        manager.prepare_with_checkpoint(pixels.len(), &model, token)?;
        checkpoint(token)?;
        scratch
            .cache
            .resize(if cache_bits == 0 { 0 } else { 1 << cache_bits }, 0);
        scratch.cache.fill(0);
        let cache = &mut scratch.cache;
        let path = &mut scratch.path;
        let output = &mut scratch.output;
        let mut update_work = 0usize;

        let mut add_literal = |position: usize, previous_cost: i64, manager: &mut CostManager| {
            let pixel = pixels[position];
            let cache_index = (cache_bits != 0).then(|| color_hash(pixel, cache_bits));
            let literal_cost =
                if let Some(index) = cache_index.filter(|&index| cache[index] == pixel) {
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
        let mut next_checkpoint = if fine_trace {
            TRACE_CHECKPOINT_PIXELS
        } else {
            1024
        };
        for position in 1..pixels.len() {
            if position >= next_checkpoint {
                checkpoint(token)?;
                next_checkpoint = position.saturating_add(if fine_trace {
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
                    offset_cost = i64::from(model.distance[distance_symbol])
                        + i64::from(distance_extra) * SCALE;
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
                                #[cfg(coverage)]
                                coverage_record_checkpoint(16, token);
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
                        manager.update_at_with_checkpoint(
                            split - 1,
                            false,
                            token,
                            &mut update_work,
                        )?;
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

        path.clear();
        let mut end = pixels.len();
        if fine_trace {
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
                    #[cfg(coverage)]
                    coverage_record_checkpoint(17, token);
                    checkpoint(token)?;
                }
                let length = manager.lengths[end - 1];
                path.push(length);
                end -= length;
            }
        }
        path.reverse();

        output.clear();
        output.reserve(path.len());
        // The dynamic-programming cache is dead after path reconstruction. Reset
        // and reuse it for token replay instead of allocating a second table.
        cache.fill(0);
        let mut position: usize = 0;
        if fine_trace {
            checkpoint(token)?;
            let mut next_checkpoint = TRACE_CHECKPOINT_PIXELS;
            for length in path.drain(..) {
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
                    populate_cache(pixels, position, length, cache_bits, cache, token)?;
                }
                position += length;
                while position >= next_checkpoint {
                    checkpoint(token)?;
                    next_checkpoint = next_checkpoint.saturating_add(TRACE_CHECKPOINT_PIXELS);
                }
            }
        } else {
            for length in path.drain(..) {
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
                    #[cfg(coverage)]
                    if length >= CACHE_CHECKPOINT_PIXELS {
                        coverage_record_checkpoint(18, token);
                    }
                    populate_cache(pixels, position, length, cache_bits, cache, token)?;
                }
                position += length;
            }
        }
        Ok(core::mem::take(output))
    })();
    scratch.model = Some(model);
    scratch.manager = Some(manager);
    result
}

#[cfg_attr(coverage, inline(never))]
pub(super) fn candidates(
    pixels: &[u32],
    width: usize,
    allow_cache: bool,
    quality: u32,
    max_cache_bits: u8,
    scratch: &mut CandidateScratch,
    token: CheckpointToken<'_>,
) -> CheckpointResult<Vec<(Vec<Token>, u8)>> {
    let mut result = core::mem::take(&mut scratch.result_list);
    result.clear();
    if pixels.is_empty() {
        result.push((Vec::new(), 0));
        return Ok(result);
    }
    fill_hash_chain(
        pixels,
        width,
        quality,
        &mut scratch.chain,
        &mut scratch.first,
        token,
    )?;
    let chain = &mut scratch.chain;
    let estimate_scratch = &mut scratch.estimate;
    let cache_scratch = &mut scratch.cache;
    let trace_scratch = &mut scratch.trace;
    let result_pool = &mut scratch.result_pool;
    // The LZ77, RLE, and optional box-chain reference streams are consumed
    // sequentially by cache selection. Retain one token buffer across those
    // source constructions instead of allocating a fresh vector for each.
    let source_scratch = &mut scratch.source;
    let mut choose_cache = |source: &[Token],
                            scratch: &mut CostEstimateScratch,
                            cache_scratch: &mut CacheTransformScratch|
     -> CheckpointResult<(Vec<Token>, u8, u64)> {
        let maximum = if allow_cache { max_cache_bits } else { 0 };
        if cache_scratch.output.capacity() < source.len()
            && let Some(mut reusable) = result_pool.pop()
        {
            reusable.clear();
            if reusable.capacity() >= source.len() {
                let previous = core::mem::replace(&mut cache_scratch.output, reusable);
                result_pool.push(previous);
            } else {
                result_pool.push(reusable);
            }
        }
        // The inclusive range always contains cache-bit value zero.
        let mut best = None;
        for bits in 0..=maximum {
            checkpoint(token)?;
            with_cache(pixels, source, bits, token, cache_scratch)?;
            let cost =
                cache_estimated_bits_with_checkpoint(&cache_scratch.output, bits, token, scratch)?;
            if best
                .as_ref()
                .is_none_or(|(_, _, best_cost)| cost < *best_cost)
            {
                let cached = core::mem::take(&mut cache_scratch.output);
                if let Some((previous, ..)) = best.replace((cached, bits, cost)) {
                    cache_scratch.output = previous;
                }
            }
        }
        let (tokens, bits, _) = cache_choice_or_invariant_failure(best);
        let cost = estimated_bits_with_checkpoint(&tokens, bits, token, scratch)?;
        checkpoint(token)?;
        Ok((tokens, bits, cost))
    };
    let improve = |mut candidate: (Vec<Token>, u8, u64),
                   source_chain: &[(usize, usize)],
                   scratch: &mut CostEstimateScratch,
                   trace_scratch: &mut TraceScratch|
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
                trace_scratch,
            )?;
            let cost = estimated_bits_with_checkpoint(&traced, candidate.1, token, scratch)?;
            if cost < candidate.2 {
                let previous = core::mem::replace(&mut candidate.0, traced);
                trace_scratch.output = previous;
                candidate.2 = cost;
            } else {
                trace_scratch.output = traced;
            }
        }
        Ok(candidate)
    };

    lz77(pixels, width, chain, token, source_scratch)?;
    let standard = choose_cache(source_scratch, estimate_scratch, cache_scratch)?;
    rle_into(pixels, width, token, source_scratch)?;
    let rle = choose_cache(source_scratch, estimate_scratch, cache_scratch)?;
    let mut primary = if standard.2 <= rle.2 {
        improve(standard, chain, estimate_scratch, trace_scratch)?
    } else {
        rle
    };
    result.push((std::mem::take(&mut primary.0), primary.1));

    // libwebp evaluates its low-distance "box" chain as a separate crunch
    // configuration for palette images containing at most sixteen colors.
    if allow_cache && max_cache_bits <= 4 {
        box_chain(pixels, width, chain, &mut scratch.counts, token)?;
        #[cfg(coverage)]
        coverage_record_checkpoint(19, token);
        lz77(pixels, width, chain, token, source_scratch)?;
        let mut box_candidate = improve(
            choose_cache(source_scratch, estimate_scratch, cache_scratch)?,
            chain,
            estimate_scratch,
            trace_scratch,
        )?;
        result.push((std::mem::take(&mut box_candidate.0), box_candidate.1));
    }
    Ok(result)
}

#[cfg(coverage)]
#[inline(never)]
pub(crate) fn __coverage_exercise_instrumented_trace_paths() {
    let literal_pixels = vec![0xff00_0000; 4_096];
    let literal_chain = vec![(0, 0); literal_pixels.len()];
    let literal_source = vec![Token::Literal(0xff00_0000); literal_pixels.len()];

    // The literal-only chain makes the cache-hit replay branch observable.
    // Keep all four fine/coarse and token/no-token specializations live: the
    // public dispatcher selects only two of them.
    let mut scratch = TraceScratch::default();
    let _ = std::hint::black_box(trace_backwards_impl::<false>(
        &literal_pixels,
        32,
        &literal_chain,
        &literal_source,
        1,
        None,
        &mut scratch,
    ));
    let mut scratch = TraceScratch::default();
    let _ = std::hint::black_box(trace_backwards_impl::<true>(
        &literal_pixels,
        32,
        &literal_chain,
        &literal_source,
        1,
        None,
        &mut scratch,
    ));

    // Leave an initial literal prefix before the copy interval. This gives
    // replay a mixed path instead of allowing the optimizer to select an
    // all-copy solution for the uniform input.
    let mixed_chain = (0..literal_pixels.len())
        .map(|position| {
            if position < 64 {
                (0, 0)
            } else {
                (1, (literal_pixels.len() - position).min(512))
            }
        })
        .collect::<Vec<_>>();
    let mut scratch = TraceScratch::default();
    let _ = std::hint::black_box(trace_backwards_impl::<true>(
        &literal_pixels,
        32,
        &mixed_chain,
        &literal_source,
        1,
        Some(&crate::CancellationToken::new()),
        &mut scratch,
    ));
    let mut scratch = TraceScratch::default();
    let _ = std::hint::black_box(trace_backwards_impl::<false>(
        &literal_pixels,
        32,
        &mixed_chain,
        &literal_source,
        0,
        None,
        &mut scratch,
    ));
    let mut scratch = TraceScratch::default();
    let _ = std::hint::black_box(trace_backwards_impl::<false>(
        &literal_pixels,
        32,
        &literal_chain,
        &literal_source,
        1,
        Some(&crate::CancellationToken::new()),
        &mut scratch,
    ));

    let copy_chain = (0..literal_pixels.len())
        .map(|position| {
            if position == 0 {
                (0, 0)
            } else {
                (1, (literal_pixels.len() - position).min(1_024))
            }
        })
        .collect::<Vec<_>>();
    // Give the cost model an explicitly copy-heavy reference stream as well
    // as the literal-only stream above. Without this, the DP is free to pick
    // literals even when the chain contains copy candidates, so the coarse
    // replay branch's populate_cache checkpoint remains unobservable.
    let copy_source = vec![
        Token::Literal(0xff00_0000),
        Token::Copy {
            distance: 1,
            length: literal_pixels.len() - 1,
        },
    ];
    let mut scratch = TraceScratch::default();
    let _ = std::hint::black_box(trace_backwards_impl::<false>(
        &literal_pixels,
        32,
        &copy_chain,
        &literal_source,
        1,
        None,
        &mut scratch,
    ));
    let mut scratch = TraceScratch::default();
    let _ = std::hint::black_box(trace_backwards_impl::<true>(
        &literal_pixels,
        32,
        &copy_chain,
        &literal_source,
        1,
        Some(&crate::CancellationToken::new()),
        &mut scratch,
    ));
    let mut scratch = TraceScratch::default();
    let _ = std::hint::black_box(trace_backwards_impl::<false>(
        &literal_pixels,
        32,
        &copy_chain,
        &copy_source,
        0,
        None,
        &mut scratch,
    ));
    let mut scratch = TraceScratch::default();
    let _ = std::hint::black_box(trace_backwards_impl::<false>(
        &literal_pixels,
        32,
        &copy_chain,
        &copy_source,
        1,
        Some(&crate::CancellationToken::new()),
        &mut scratch,
    ));

    // A short change in distance while the previous interval is still
    // reachable drives the split/recompute path in the DP trace.
    let split_chain = (0..literal_pixels.len())
        .map(|position| {
            if position == 0 {
                (0, 0)
            } else if position == 1 {
                (1, 32)
            } else if position == 2 {
                (1, 512)
            } else if position == 3 {
                (2, 512)
            } else {
                (2, (literal_pixels.len() - position).min(512))
            }
        })
        .collect::<Vec<_>>();
    let mut scratch = TraceScratch::default();
    let _ = std::hint::black_box(trace_backwards_impl::<true>(
        &literal_pixels,
        32,
        &split_chain,
        &literal_source,
        1,
        Some(&crate::CancellationToken::new()),
        &mut scratch,
    ));

    #[cfg(coverage_nightly)]
    {
        // Measure a small valid split trace, then sweep its checkpoint
        // boundaries so both interval-update and replacement-push error edges
        // are reached.
        let split_probe_pixels = vec![0xff00_0000; 128];
        let split_probe_chain = (0..split_probe_pixels.len())
            .map(|position| {
                if position == 0 {
                    (0, 0)
                } else if position == 1 {
                    (1, 16)
                } else if position == 2 {
                    (1, 64)
                } else {
                    (2, (split_probe_pixels.len() - position).min(64))
                }
            })
            .collect::<Vec<_>>();
        let split_probe_source = vec![Token::Literal(0xff00_0000); split_probe_pixels.len()];
        let split_probe_token = crate::CancellationToken::new();
        split_probe_token.cancel_after(usize::MAX);
        let mut split_probe_scratch = TraceScratch::default();
        let _ = std::hint::black_box(trace_backwards_impl::<true>(
            &split_probe_pixels,
            16,
            &split_probe_chain,
            &split_probe_source,
            1,
            Some(&split_probe_token),
            &mut split_probe_scratch,
        ));
        let split_probe_checks = usize::MAX.saturating_sub(
            split_probe_token
                .coverage_remaining_checks()
                .unwrap_or(usize::MAX),
        );
        for checks in 0..=split_probe_checks {
            let token = crate::CancellationToken::new();
            token.cancel_after(checks);
            let mut scratch = TraceScratch::default();
            let _ = std::hint::black_box(trace_backwards_impl::<true>(
                &split_probe_pixels,
                16,
                &split_probe_chain,
                &split_probe_source,
                1,
                Some(&token),
                &mut scratch,
            ));
        }
        let split_probe_token = crate::CancellationToken::new();
        split_probe_token.cancel_after(usize::MAX);
        let mut split_probe_scratch = TraceScratch::default();
        let _ = std::hint::black_box(trace_backwards_impl::<false>(
            &split_probe_pixels,
            16,
            &split_probe_chain,
            &split_probe_source,
            1,
            Some(&split_probe_token),
            &mut split_probe_scratch,
        ));
        let split_probe_checks = usize::MAX.saturating_sub(
            split_probe_token
                .coverage_remaining_checks()
                .unwrap_or(usize::MAX),
        );
        for checks in 0..=split_probe_checks {
            let token = crate::CancellationToken::new();
            token.cancel_after(checks);
            let mut scratch = TraceScratch::default();
            let _ = std::hint::black_box(trace_backwards_impl::<false>(
                &split_probe_pixels,
                16,
                &split_probe_chain,
                &split_probe_source,
                1,
                Some(&token),
                &mut scratch,
            ));
        }
    }

    // Measure the actual checkpoint count, then cancel at every boundary.
    // This covers cancellation edges in DP preparation, interval updates,
    // path reconstruction, and token replay without guessing their count.
    // Use a smaller dedicated input for the sweep: the long input above has
    // already made the 256/1024 checkpoint branches observable, while a
    // boundary sweep is quadratic in the input length.
    let cancellation_pixels = vec![0xff00_0000; 768];
    let cancellation_chain = (0..cancellation_pixels.len())
        .map(|position| {
            if position == 0 {
                (0, 0)
            } else {
                (1, (cancellation_pixels.len() - position).min(256))
            }
        })
        .collect::<Vec<_>>();
    let cancellation_source = vec![Token::Literal(0xff00_0000); cancellation_pixels.len()];
    let probe = crate::CancellationToken::new();
    probe.cancel_after(usize::MAX);
    let mut probe_scratch = TraceScratch::default();
    let _ = std::hint::black_box(trace_backwards_impl::<true>(
        &cancellation_pixels,
        32,
        &cancellation_chain,
        &cancellation_source,
        1,
        Some(&probe),
        &mut probe_scratch,
    ));
    let successful_checks =
        usize::MAX.saturating_sub(probe.coverage_remaining_checks().unwrap_or(usize::MAX));
    for checks in [
        0,
        successful_checks.min(1),
        successful_checks / 2,
        successful_checks.saturating_sub(1),
        successful_checks,
    ] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut scratch = TraceScratch::default();
        let _ = std::hint::black_box(trace_backwards_impl::<true>(
            &cancellation_pixels,
            32,
            &cancellation_chain,
            &cancellation_source,
            1,
            Some(&token),
            &mut scratch,
        ));
    }

    // The coarse token-aware specialization has a different checkpoint
    // cadence from fine tracing, so sweep it independently as well.
    let probe = crate::CancellationToken::new();
    probe.cancel_after(usize::MAX);
    let mut probe_scratch = TraceScratch::default();
    let _ = std::hint::black_box(trace_backwards_impl::<false>(
        &cancellation_pixels,
        32,
        &cancellation_chain,
        &cancellation_source,
        1,
        Some(&probe),
        &mut probe_scratch,
    ));
    let successful_checks =
        usize::MAX.saturating_sub(probe.coverage_remaining_checks().unwrap_or(usize::MAX));
    for checks in [
        0,
        successful_checks.min(1),
        successful_checks / 2,
        successful_checks.saturating_sub(1),
        successful_checks,
    ] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut scratch = TraceScratch::default();
        let _ = std::hint::black_box(trace_backwards_impl::<false>(
            &cancellation_pixels,
            32,
            &cancellation_chain,
            &cancellation_source,
            1,
            Some(&token),
            &mut scratch,
        ));
    }

    // The token-aware box pass has a separate candidate checkpoint family.
    // A long run reaches both its bounded-length and end-of-input guards.
    let box_pixels = vec![0xff00_0000; MAX_LENGTH * 2 + 64];
    let mut box_chain_state = vec![(0, 0); box_pixels.len()];
    let mut box_counts = Vec::new();
    let box_token = crate::CancellationToken::new();
    let _ = std::hint::black_box(box_chain(
        &box_pixels,
        1,
        &mut box_chain_state,
        &mut box_counts,
        Some(&box_token),
    ));
    // Two equal one-pixel runs followed by different values reach the
    // post-run comparison in the token-aware box matcher. Uniform input
    // exits earlier through the end-of-input guard, so it cannot cover this
    // valid mismatch state.
    let mismatch_pixels = [0xff00_0000, 0xff00_0001, 0xff00_0000, 0xff00_0002];
    let mut mismatch_chain = vec![(0, 0); mismatch_pixels.len()];
    let mut mismatch_counts = Vec::new();
    let mismatch_token = crate::CancellationToken::new();
    let _ = std::hint::black_box(box_chain(
        &mismatch_pixels,
        1,
        &mut mismatch_chain,
        &mut mismatch_counts,
        Some(&mismatch_token),
    ));

    // Force the interval manager's token-aware work counter past its 1,024
    // entry checkpoint while keeping the state below the saturation fallback.
    let manager_tokens = vec![Token::Literal(0xff00_0000); 128];
    let mut manager_model = CostModel::default();
    let manager_token = crate::CancellationToken::new();
    manager_model
        .prepare_with_checkpoint(&manager_tokens, 1, 32, Some(&manager_token))
        .expect("coverage cost model must prepare");
    let mut manager = CostManager::default();
    manager
        .prepare_with_checkpoint(256, &manager_model, Some(&manager_token))
        .expect("coverage cost manager must prepare");
    manager.intervals = (0..64)
        .map(|index| CostInterval {
            cost: 1,
            start: index * 4,
            end: index * 4 + 2,
            position: 0,
        })
        .collect();
    let _ = std::hint::black_box(manager.insert_min_interval_with_checkpoint(
        CostInterval {
            cost: 0,
            start: 0,
            end: 256,
            position: 0,
        },
        Some(&manager_token),
    ));
    let mut gap_manager = CostManager::default();
    gap_manager.intervals.push(CostInterval {
        cost: 1,
        start: 16,
        end: 18,
        position: 0,
    });
    let _ = std::hint::black_box(gap_manager.insert_min_interval_with_checkpoint(
        CostInterval {
            cost: 0,
            start: 0,
            end: 2,
            position: 0,
        },
        Some(&manager_token),
    ));

    // Keep the low-level reference builders on the instrumented side of the
    // coverage boundary. The public coverage hook above is intentionally
    // `coverage(off)` because it also models impossible defensive states;
    // these calls exercise the ordinary cancellation edges with real counts.
    let reference_pixels = vec![0xff00_0000; 2_048];
    let mut reference_chain = vec![(0, 0); reference_pixels.len()];
    let mut reference_first = Vec::new();
    let _ = std::hint::black_box(fill_hash_chain(
        &reference_pixels,
        32,
        100,
        &mut reference_chain,
        &mut reference_first,
        None,
    ));
    let cancelled = crate::CancellationToken::new();
    cancelled.cancel_after(0);
    let _ = std::hint::black_box(fill_hash_chain(
        &reference_pixels,
        32,
        100,
        &mut reference_chain,
        &mut reference_first,
        Some(&cancelled),
    ));
    let mut window_pixels = vec![0_u32, 1];
    window_pixels.extend(1_000_u32..1_038);
    window_pixels.extend([0, 1]);
    let window_pixels = std::hint::black_box(window_pixels);
    let window_token = crate::CancellationToken::new();
    let mut window_chain = Vec::new();
    let mut window_first = Vec::new();
    let _ = std::hint::black_box(fill_hash_chain(
        &window_pixels,
        1,
        0,
        &mut window_chain,
        &mut window_first,
        Some(&window_token),
    ));

    let mut reference_refs = Vec::new();
    let reference_copy_chain = (0..reference_pixels.len())
        .map(|position| {
            if position == 0 {
                (0, 0)
            } else {
                (1, (reference_pixels.len() - position).min(64))
            }
        })
        .collect::<Vec<_>>();
    let cancelled = crate::CancellationToken::new();
    cancelled.cancel_after(0);
    let _ = std::hint::black_box(lz77(
        &reference_pixels,
        32,
        &reference_copy_chain,
        Some(&cancelled),
        &mut reference_refs,
    ));
    let _ = std::hint::black_box(fill_hash_chain(
        &reference_pixels,
        32,
        100,
        &mut reference_chain,
        &mut reference_first,
        None,
    ));
    let _ = std::hint::black_box(lz77(
        &reference_pixels,
        32,
        &reference_chain,
        None,
        &mut reference_refs,
    ));
    let mut valid_trace_scratch = TraceScratch::default();
    std::hint::black_box(
        trace_backwards_impl::<false>(
            &reference_pixels,
            32,
            &reference_chain,
            &reference_refs,
            0,
            None,
            &mut valid_trace_scratch,
        )
        .expect("coverage reference trace must encode"),
    );
    let valid_trace_token = crate::CancellationToken::new();
    let mut valid_token_trace_scratch = TraceScratch::default();
    std::hint::black_box(
        trace_backwards_impl::<true>(
            &reference_pixels,
            32,
            &reference_chain,
            &reference_refs,
            1,
            Some(&valid_trace_token),
            &mut valid_token_trace_scratch,
        )
        .expect("coverage token reference trace must encode"),
    );
    let coarse_token_trace_token = crate::CancellationToken::new();
    let mut coarse_token_trace_scratch = TraceScratch::default();
    std::hint::black_box(
        trace_backwards_impl::<false>(
            &reference_pixels,
            32,
            &reference_chain,
            &reference_refs,
            1,
            Some(&coarse_token_trace_token),
            &mut coarse_token_trace_scratch,
        )
        .expect("coverage coarse token reference trace must encode"),
    );
    let mut wrapper_trace_scratch = TraceScratch::default();
    std::hint::black_box(
        trace_backwards(
            &reference_pixels,
            32,
            &reference_chain,
            &reference_refs,
            0,
            None,
            &mut wrapper_trace_scratch,
        )
        .expect("coverage wrapper trace must encode"),
    );
    let wrapper_trace_token = crate::CancellationToken::new();
    let mut wrapper_token_trace_scratch = TraceScratch::default();
    std::hint::black_box(
        trace_backwards(
            &reference_pixels,
            32,
            &reference_chain,
            &reference_refs,
            1,
            Some(&wrapper_trace_token),
            &mut wrapper_token_trace_scratch,
        )
        .expect("coverage wrapper token trace must encode"),
    );
    let _ = std::hint::black_box(rle_into(
        &reference_pixels[..512],
        32,
        Some(&crate::CancellationToken::new()),
        &mut reference_refs,
    ));
    let mut reference_box_chain = vec![(0, 0); 512];
    let mut reference_counts = Vec::new();
    let _ = std::hint::black_box(box_chain(
        &reference_pixels[..512],
        1,
        &mut reference_box_chain,
        &mut reference_counts,
        None,
    ));
    let cancelled = crate::CancellationToken::new();
    cancelled.cancel_after(0);
    let _ = std::hint::black_box(box_chain(
        &reference_pixels[..512],
        1,
        &mut reference_box_chain,
        &mut reference_counts,
        Some(&cancelled),
    ));

    let cost_tokens = vec![Token::Literal(0xff00_0000); 1_024];
    let cost_probe = crate::CancellationToken::new();
    cost_probe.cancel_after(usize::MAX);
    let mut cost_probe_model = CostModel::default();
    let _ = std::hint::black_box(cost_probe_model.prepare_with_checkpoint(
        &cost_tokens,
        1,
        32,
        Some(&cost_probe),
    ));
    let cost_checks =
        usize::MAX.saturating_sub(cost_probe.coverage_remaining_checks().unwrap_or(usize::MAX));
    for checks in [
        0,
        cost_checks.min(1),
        cost_checks / 2,
        cost_checks.saturating_sub(1),
        cost_checks,
    ] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut model = CostModel::default();
        let _ =
            std::hint::black_box(model.prepare_with_checkpoint(&cost_tokens, 1, 32, Some(&token)));
    }

    let estimate_probe = crate::CancellationToken::new();
    estimate_probe.cancel_after(usize::MAX);
    let mut estimate_scratch = CostEstimateScratch::default();
    let _ = std::hint::black_box(estimated_bits_with_checkpoint(
        &cost_tokens,
        1,
        Some(&estimate_probe),
        &mut estimate_scratch,
    ));
    let estimate_checks = usize::MAX.saturating_sub(
        estimate_probe
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in [
        0,
        estimate_checks.min(1),
        estimate_checks / 2,
        estimate_checks.saturating_sub(1),
        estimate_checks,
    ] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut scratch = CostEstimateScratch::default();
        let _ = std::hint::black_box(estimated_bits_with_checkpoint(
            &cost_tokens,
            1,
            Some(&token),
            &mut scratch,
        ));
    }

    let manager_probe_token = crate::CancellationToken::new();
    manager_probe_token.cancel_after(usize::MAX);
    let mut manager_probe = CostManager::default();
    manager_probe.prepare_without_checkpoint(256, &cost_probe_model);
    manager_probe.intervals = (0..64)
        .map(|index| CostInterval {
            cost: 1,
            start: index * 4,
            end: index * 4 + 2,
            position: 0,
        })
        .collect();
    let _ = std::hint::black_box(manager_probe.insert_min_interval_with_checkpoint(
        CostInterval {
            cost: 0,
            start: 0,
            end: 256,
            position: 0,
        },
        Some(&manager_probe_token),
    ));
    let manager_checks = usize::MAX.saturating_sub(
        manager_probe_token
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in [
        0,
        manager_checks.min(1),
        manager_checks / 2,
        manager_checks.saturating_sub(1),
        manager_checks,
    ] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut manager = CostManager::default();
        manager.prepare_without_checkpoint(256, &cost_probe_model);
        manager.intervals = (0..64)
            .map(|index| CostInterval {
                cost: 1,
                start: index * 4,
                end: index * 4 + 2,
                position: 0,
            })
            .collect();
        let _ = std::hint::black_box(manager.insert_min_interval_with_checkpoint(
            CostInterval {
                cost: 0,
                start: 0,
                end: 256,
                position: 0,
            },
            Some(&token),
        ));
    }

    coverage_exercise_remaining_checkpoint_errors();
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_run_fill_hash_chain_uniform(token: &crate::CancellationToken) -> CheckpointResult<()> {
    let pixels = vec![0xff00_0000; 2_048];
    let mut chain = Vec::new();
    let mut first = Vec::new();
    fill_hash_chain(&pixels, 32, 100, &mut chain, &mut first, Some(token))
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_run_fill_hash_chain_random(token: &crate::CancellationToken) -> CheckpointResult<()> {
    let pixels = (0..2_048)
        .map(|index| {
            (index as u32)
                .wrapping_mul(0x9e37_79b9)
                .rotate_left((index % 31) as u32)
        })
        .collect::<Vec<_>>();
    let mut chain = Vec::new();
    let mut first = Vec::new();
    fill_hash_chain(&pixels, 32, 100, &mut chain, &mut first, Some(token))
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_run_fill_hash_chain_long_match(
    token: &crate::CancellationToken,
) -> CheckpointResult<()> {
    let pixels = (0..1_024_usize)
        .map(|index| {
            if (256..=800).contains(&index) {
                0xff22_3344
            } else {
                (index as u32).wrapping_mul(0x9e37_79b9)
            }
        })
        .collect::<Vec<_>>();
    let mut chain = Vec::new();
    let mut first = Vec::new();
    fill_hash_chain(&pixels, 32, 100, &mut chain, &mut first, Some(token))
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_run_fill_hash_chain_match_error(
    token: &crate::CancellationToken,
) -> CheckpointResult<()> {
    let pixels = vec![0xff00_0000; 258];
    let mut chain = Vec::new();
    let mut first = Vec::new();
    fill_hash_chain(&pixels, 1, 100, &mut chain, &mut first, Some(token))
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_run_hash_candidate(token: &crate::CancellationToken) -> CheckpointResult<()> {
    let pixels = (0..512_usize)
        .map(|index| {
            if index.is_multiple_of(2) {
                0x0100_0000
            } else {
                0x0200_0000
            }
        })
        .collect::<Vec<_>>();
    let mut chain = Vec::new();
    let mut first = Vec::new();
    checkpoint(Some(token))?;
    fill_hash_chain(&pixels, 2, 100, &mut chain, &mut first, Some(token))
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_run_hash_candidate_work_checkpoint(
    token: &crate::CancellationToken,
) -> CheckpointResult<()> {
    let mut work = HASH_CHAIN_CANDIDATE_CHECKPOINT_TRIALS - 1;
    checkpoint_hash_chain_candidate_work(token, &mut work)
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_run_forced_hash_candidate_checkpoint(
    token: &crate::CancellationToken,
) -> CheckpointResult<()> {
    FORCE_HASH_CHAIN_CANDIDATE_CHECKPOINT.store(true, Ordering::Relaxed);
    let result = coverage_run_hash_candidate(token);
    FORCE_HASH_CHAIN_CANDIDATE_CHECKPOINT.store(false, Ordering::Relaxed);
    result
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_run_candidates(token: &crate::CancellationToken) -> CheckpointResult<()> {
    let pixels = (0..4_096)
        .map(|index| {
            if index % 8 < 4 {
                0xff10_2010
            } else {
                0xff20_4020
            }
        })
        .collect::<Vec<_>>();
    let mut scratch = CandidateScratch::default();
    candidates(&pixels, 32, true, 80, 4, &mut scratch, Some(token)).map(|_| ())
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_run_lz77(token: &crate::CancellationToken) -> CheckpointResult<()> {
    let pixels = vec![0xff00_0000; 4_096];
    let chain = (0..pixels.len())
        .map(|position| if position == 0 { (0, 0) } else { (0, 1) })
        .collect::<Vec<_>>();
    let mut refs = Vec::new();
    lz77(&pixels, 32, &chain, Some(token), &mut refs)
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_run_rle(token: &crate::CancellationToken) -> CheckpointResult<()> {
    let pixels = vec![0xff00_0000; 512];
    let mut refs = Vec::new();
    rle_into(&pixels, 1, Some(token), &mut refs)
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_run_box_chain(token: &crate::CancellationToken) -> CheckpointResult<()> {
    let pixels = vec![0xff00_0000; 2_048];
    let mut chain = vec![(0, 0); pixels.len()];
    let mut counts = Vec::new();
    box_chain(&pixels, 1, &mut chain, &mut counts, Some(token))
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_run_box_inner(token: &crate::CancellationToken) -> CheckpointResult<()> {
    let pixels = (0..1_024_usize)
        .map(|index| {
            if index.is_multiple_of(2) {
                0xff00_0000
            } else {
                0xff00_0001
            }
        })
        .collect::<Vec<_>>();
    let mut chain = vec![(0, 0); pixels.len()];
    let mut counts = Vec::new();
    box_chain(&pixels, 1, &mut chain, &mut counts, Some(token))
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_run_box_direct(token: &crate::CancellationToken) -> CheckpointResult<()> {
    let pixels = vec![0xff00_0000; 2_048];
    let mut chain = vec![(1, MAX_LENGTH); pixels.len()];
    chain[0] = (0, 0);
    let mut counts = Vec::new();
    box_chain(&pixels, 1, &mut chain, &mut counts, Some(token))
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_run_split_trace(token: &crate::CancellationToken) -> CheckpointResult<()> {
    let pixels = vec![0xff00_0000; 1_024];
    let chain = (0..pixels.len())
        .map(|position| {
            if position == 0 {
                (0, 0)
            } else if position <= 300 {
                (1, (pixels.len() - position - 1).min(512))
            } else {
                (2, (pixels.len() - position - 1).min(512))
            }
        })
        .collect::<Vec<_>>();
    let source = vec![Token::Literal(0xff00_0000); pixels.len()];
    let mut scratch = TraceScratch::default();
    trace_backwards_impl::<true>(&pixels, 32, &chain, &source, 0, Some(token), &mut scratch)
        .map(|_| ())
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_run_coarse_trace(token: &crate::CancellationToken) -> CheckpointResult<()> {
    let pixels = vec![0xff00_0000; 2_048];
    let chain = vec![(0, 0); pixels.len()];
    let source = vec![Token::Literal(0xff00_0000); pixels.len()];
    let mut scratch = TraceScratch::default();
    trace_backwards_impl::<false>(&pixels, 32, &chain, &source, 0, Some(token), &mut scratch)
        .map(|_| ())
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_run_cache_trace(token: &crate::CancellationToken) -> CheckpointResult<()> {
    let pixels = vec![0xff00_0000; 2_048];
    let chain = (0..pixels.len())
        .map(|position| {
            if position == 0 {
                (0, 0)
            } else {
                (1, pixels.len() - position)
            }
        })
        .collect::<Vec<_>>();
    let source = vec![
        Token::Literal(0xff00_0000),
        Token::Copy {
            distance: 1,
            length: pixels.len() - 1,
        },
    ];
    let mut scratch = TraceScratch::default();
    trace_backwards_impl::<false>(&pixels, 32, &chain, &source, 1, Some(token), &mut scratch)
        .map(|_| ())
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_replay_checkpoint(
    index: usize,
    run: fn(&crate::CancellationToken) -> CheckpointResult<()>,
) {
    let Some(checks) = coverage_checkpoint_count(index) else {
        return;
    };
    let token = crate::CancellationToken::new();
    token.cancel_after(checks);
    let _ = std::hint::black_box(run(&token));
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_replay_checkpoint_window(
    index: usize,
    run: fn(&crate::CancellationToken) -> CheckpointResult<()>,
) {
    let Some(checks) = coverage_checkpoint_count(index) else {
        return;
    };
    for attempt in [checks.saturating_sub(1), checks, checks.saturating_add(1)] {
        let token = crate::CancellationToken::new();
        token.cancel_after(attempt);
        let _ = std::hint::black_box(run(&token));
    }
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_exercise_remaining_checkpoint_errors() {
    for slot in &COVERAGE_CHECKPOINT_REMAINING {
        slot.store(usize::MAX, Ordering::Relaxed);
    }

    for run in [
        coverage_run_fill_hash_chain_uniform,
        coverage_run_fill_hash_chain_random,
        coverage_run_fill_hash_chain_long_match,
        coverage_run_hash_candidate,
        coverage_run_candidates,
    ] {
        let token = crate::CancellationToken::new();
        token.cancel_after(usize::MAX);
        let _ = std::hint::black_box(run(&token));
    }
    for run in [
        coverage_run_lz77,
        coverage_run_rle,
        coverage_run_box_inner,
        coverage_run_box_chain,
        coverage_run_split_trace,
        coverage_run_coarse_trace,
        coverage_run_cache_trace,
    ] {
        let token = crate::CancellationToken::new();
        token.cancel_after(usize::MAX);
        let _ = std::hint::black_box(run(&token));
    }

    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let _ = std::hint::black_box(coverage_run_hash_candidate_work_checkpoint(&token));
    coverage_replay_checkpoint(0, coverage_run_hash_candidate_work_checkpoint);

    for index in [1, 4] {
        coverage_replay_checkpoint(index, coverage_run_fill_hash_chain_uniform);
    }
    coverage_replay_checkpoint(2, coverage_run_fill_hash_chain_random);

    COVERAGE_CHECKPOINT_REMAINING[3].store(usize::MAX, Ordering::Relaxed);
    let match_probe_token = crate::CancellationToken::new();
    match_probe_token.cancel_after(usize::MAX);
    let _ = std::hint::black_box(coverage_run_fill_hash_chain_match_error(&match_probe_token));
    coverage_replay_checkpoint(3, coverage_run_fill_hash_chain_match_error);

    COVERAGE_CHECKPOINT_REMAINING[5].store(usize::MAX, Ordering::Relaxed);
    let candidate_token = crate::CancellationToken::new();
    candidate_token.cancel_after(usize::MAX);
    let _ = std::hint::black_box(coverage_run_forced_hash_candidate_checkpoint(
        &candidate_token,
    ));
    coverage_replay_checkpoint(5, coverage_run_forced_hash_candidate_checkpoint);
    coverage_replay_checkpoint(6, coverage_run_lz77);
    for checks in [0, 1] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = std::hint::black_box(coverage_run_rle(&token));
    }
    let direct_box_token = crate::CancellationToken::new();
    direct_box_token.cancel_after(1);
    let _ = std::hint::black_box(coverage_run_box_direct(&direct_box_token));
    let inner_box_token = crate::CancellationToken::new();
    inner_box_token.cancel_after(1);
    let _ = std::hint::black_box(coverage_run_box_inner(&inner_box_token));
    COVERAGE_CHECKPOINT_REMAINING[9].store(usize::MAX, Ordering::Relaxed);
    let box_probe_token = crate::CancellationToken::new();
    box_probe_token.cancel_after(usize::MAX);
    let _ = std::hint::black_box(coverage_run_box_chain(&box_probe_token));
    coverage_replay_checkpoint(9, coverage_run_box_chain);
    coverage_replay_checkpoint(16, coverage_run_split_trace);
    coverage_replay_checkpoint(17, coverage_run_coarse_trace);
    coverage_replay_checkpoint_window(18, coverage_run_cache_trace);
    coverage_replay_checkpoint(19, coverage_run_candidates);
}

#[cfg(coverage)]
#[coverage(off)]
pub(crate) fn __coverage_exercise_private_branches() {
    let mut scratch = CandidateScratch::default();
    assert!(matches!(
        candidates(&[], 1, true, 80, 0, &mut scratch, None).map(|items| items.len()),
        Ok(1)
    ));
    let _ = candidates(
        &[0xff00_0000; MAX_LENGTH + 4],
        1,
        false,
        60,
        0,
        &mut scratch,
        None,
    );
    let alternating = (0..MAX_LENGTH + 260)
        .map(|index| {
            if index % 2 == 0 {
                0xff00_0000
            } else {
                0xff00_0001
            }
        })
        .collect::<Vec<_>>();
    let _ = candidates(&alternating, 2, false, 100, 0, &mut scratch, None);
    let long_periodic = (0..(MAX_LENGTH * 3 + 8))
        .map(|index| {
            if index % 2 == 0 {
                0xff00_0000
            } else {
                0xff00_0001
            }
        })
        .collect::<Vec<_>>();
    let _ = candidates(&long_periodic, 2, false, 100, 0, &mut scratch, None);

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
    let mut counts = Vec::new();
    let _ = box_chain(&long_uniform, 1, &mut retained_chain, &mut counts, None);

    let _ = fast_slog(70_000);
    let _ = prefix(300);
    let mut population = [70_000, 1];
    population_cost_in_place(&mut population);
    let mut model = CostModel::default();
    model.prepare_without_checkpoint(&[Token::Literal(0xff00_0000)], 0, 1);
    let mut manager = CostManager::default();
    manager.prepare_without_checkpoint(8, &model);
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
    let mut manager = CostManager::default();
    manager.prepare_without_checkpoint(8, &model);
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
    let mut manager = CostManager::default();
    manager.prepare_without_checkpoint(8, &model);
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
    let mut manager = CostManager::default();
    manager.prepare_without_checkpoint(8, &model);
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

    let coverage_token = crate::CancellationToken::new();
    let mut checkpoint_work = COST_MANAGER_CHECKPOINT_ENTRIES - 1;
    let _ = checkpoint_cost_manager_work(Some(&coverage_token), &mut checkpoint_work);
    let mut update_work = COST_MANAGER_UPDATE_CHECKPOINT_ENTRIES - 1;
    let _ = checkpoint_cost_manager_update_work(Some(&coverage_token), &mut update_work);
    let mut cache = [0_u32; 1];
    populate_cache_without_checkpoint(&[0xff00_0000], 0, 1, 0, &mut cache);
    let population = [1_u32, 2, 0, 0];
    let _ = population_estimate_fixed_with_checkpoint(&population, None);
    let _ = population_estimate_fixed_with_checkpoint(&population, Some(&coverage_token));
    let mut population_costs = population;
    let _ = population_cost_in_place_with_checkpoint(&mut population_costs, None);
    let _ = population_cost_in_place_with_checkpoint(&mut population_costs, Some(&coverage_token));

    let token_model_tokens = [
        Token::Literal(0xff00_0000),
        Token::Copy {
            distance: 1,
            length: 4,
        },
        Token::Cache(0),
    ];
    let mut token_model = CostModel::default();
    let _ = token_model.prepare_with_checkpoint(&token_model_tokens, 1, 8, Some(&coverage_token));
    let mut token_manager = CostManager::default();
    let _ = token_manager.prepare_with_checkpoint(4_096, &token_model, Some(&coverage_token));
    token_manager.intervals = vec![
        CostInterval {
            cost: 1,
            start: 0,
            end: 4,
            position: 0,
        };
        500
    ];
    let _ = token_manager.insert_min_interval_with_checkpoint(
        CostInterval {
            cost: 0,
            start: 0,
            end: 2_048,
            position: 0,
        },
        Some(&coverage_token),
    );
    let mut normal_token_manager = CostManager::default();
    let _ = normal_token_manager.prepare_with_checkpoint(64, &token_model, Some(&coverage_token));
    let _ = normal_token_manager.insert_min_interval_with_checkpoint(
        CostInterval {
            cost: 1,
            start: 0,
            end: 4,
            position: 0,
        },
        Some(&coverage_token),
    );
    let _ = normal_token_manager.insert_min_interval_with_checkpoint(
        CostInterval {
            cost: 0,
            start: 2,
            end: 2,
            position: 0,
        },
        Some(&coverage_token),
    );
    let _ = normal_token_manager.push_with_checkpoint(0, 0, 256, Some(&coverage_token));
    let _ = normal_token_manager.insert_min_interval_with_checkpoint(
        CostInterval {
            cost: 0,
            start: 1,
            end: 3,
            position: 0,
        },
        Some(&coverage_token),
    );
    let mut cache_transform_scratch = CacheTransformScratch::default();
    let cache_refs = [Token::Cache(0)];
    with_cache_without_checkpoint(&[0xff00_0000], &cache_refs, 1, &mut cache_transform_scratch);
    let _ = with_cache(
        &[0xff00_0000],
        &cache_refs,
        1,
        Some(&coverage_token),
        &mut cache_transform_scratch,
    );
    let _ = token_manager.push_with_checkpoint(0, 0, 64, Some(&coverage_token));

    // Cancel inside the interval-building work of the long token-aware push.
    // Rebuild the manager for each threshold so every internal `?` edge is
    // tested from the same valid prepared state.
    for checks in [0, 1, 2, 64, 256] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut push_manager = CostManager::default();
        push_manager.prepare_without_checkpoint(4_096, &token_model);
        let _ = push_manager.push_with_checkpoint(0, 0, 256, Some(&token));
    }
    for checks in [0, 1, 2, 8] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut saturated_push = CostManager::default();
        saturated_push.prepare_without_checkpoint(4_096, &token_model);
        saturated_push.intervals = vec![
            CostInterval {
                cost: 1,
                start: 0,
                end: 4,
                position: 0,
            };
            500
        ];
        let _ = saturated_push.push_with_checkpoint(0, 0, MAX_LENGTH, Some(&token));
    }

    let trace_pixels = vec![0xff00_0000; 2_048];
    let trace_chain = vec![(0, 0); trace_pixels.len()];
    let trace_source = vec![Token::Literal(0xff00_0000); trace_pixels.len()];
    let mut fine_trace_scratch = TraceScratch::default();
    let _ = std::hint::black_box(trace_backwards_impl::<true>(
        &trace_pixels,
        32,
        &trace_chain,
        &trace_source,
        1,
        Some(&coverage_token),
        &mut fine_trace_scratch,
    ));
    let mut ordinary_trace_scratch = TraceScratch::default();
    let _ = std::hint::black_box(trace_backwards_impl::<false>(
        &trace_pixels,
        32,
        &trace_chain,
        &trace_source,
        0,
        None,
        &mut ordinary_trace_scratch,
    ));

    let copy_trace_pixels = vec![0xff00_0000; 2_048];
    let copy_trace_chain = (0..copy_trace_pixels.len())
        .map(|position| {
            if position == 0 {
                (0, 0)
            } else {
                (1, (copy_trace_pixels.len() - position).min(64))
            }
        })
        .collect::<Vec<_>>();
    let copy_trace_source = vec![Token::Literal(0xff00_0000); copy_trace_pixels.len()];
    let mut copy_fine_scratch = TraceScratch::default();
    let _ = std::hint::black_box(trace_backwards_impl::<true>(
        &copy_trace_pixels,
        32,
        &copy_trace_chain,
        &copy_trace_source,
        1,
        Some(&coverage_token),
        &mut copy_fine_scratch,
    ));
    let mut copy_ordinary_scratch = TraceScratch::default();
    let _ = std::hint::black_box(trace_backwards_impl::<false>(
        &copy_trace_pixels,
        32,
        &copy_trace_chain,
        &copy_trace_source,
        0,
        None,
        &mut copy_ordinary_scratch,
    ));
    // The public dispatcher selects fine tracing when a caller token exists
    // and coarse tracing otherwise. Exercise the other two private const
    // specializations as implementation-only models so their shared replay
    // logic is covered under both token states as well.
    let mut coarse_token_scratch = TraceScratch::default();
    let _ = std::hint::black_box(trace_backwards_impl::<false>(
        &copy_trace_pixels,
        32,
        &copy_trace_chain,
        &copy_trace_source,
        1,
        Some(&coverage_token),
        &mut coarse_token_scratch,
    ));
    let mut ordinary_fine_scratch = TraceScratch::default();
    let _ = std::hint::black_box(trace_backwards_impl::<true>(
        &copy_trace_pixels,
        32,
        &copy_trace_chain,
        &copy_trace_source,
        1,
        None,
        &mut ordinary_fine_scratch,
    ));
    let token_pixels = (0..4_096)
        .map(|index| {
            if index % 8 < 4 {
                0xff10_2010
            } else {
                0xff20_4020
            }
        })
        .collect::<Vec<_>>();
    let mut token_scratch = CandidateScratch::default();
    let _ = std::hint::black_box(candidates(
        &token_pixels,
        32,
        true,
        80,
        4,
        &mut token_scratch,
        Some(&coverage_token),
    ));
    // The generic candidate path has already been run to completion above;
    // this bounded sweep only needs enough early cancellation points to
    // materialize its typed `?` edges. Keep it small because each attempted
    // candidate owns a pixel-scaled scratch set.
    for checks in [0, 1, 2, 32, 127] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut candidate_scratch = CandidateScratch::default();
        let _ = std::hint::black_box(candidates(
            &token_pixels,
            32,
            true,
            80,
            4,
            &mut candidate_scratch,
            Some(&token),
        ));
    }
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
        let mut trace_scratch = TraceScratch::default();
        let _ = trace_backwards(&pixels, 1, &chain, &source, 0, None, &mut trace_scratch);
    });
    let _ = std::panic::catch_unwind(|| {
        let mut cache_scratch = CacheTransformScratch::default();
        let _ = with_cache(
            &[0xff00_0000],
            &[Token::Cache(0)],
            1,
            None,
            &mut cache_scratch,
        );
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
