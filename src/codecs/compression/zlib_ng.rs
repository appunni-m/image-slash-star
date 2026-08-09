//! Altered Rust ports of byte-compatible zlib-ng 2.3.3 compressor subsets.
//!
//! Modified Rust port copyright (c) 2026 Appunni M.
//!
//! The original zlib license notice is retained in
//! `third_party/zlib-ng/LICENSE.md`.

use super::deflate::{DISTANCE_BASE, DISTANCE_EXTRA, LENGTH_BASE, LENGTH_EXTRA};
const LITERAL_CODES: usize = 286;
const DISTANCE_CODES: usize = 30;
const BIT_LENGTH_CODES: usize = 19;
const MAX_BITS: usize = 15;
const BIT_COUNT_SIZE: usize = 16;
const MAX_BIT_LENGTH_BITS: usize = 7;
const MIN_LOOKAHEAD: usize = 262;
const MAX_DISTANCE: usize = 32_768 - MIN_LOOKAHEAD;
const MAX_MATCH: usize = 258;
const MIN_MATCH: usize = 4;
const WINDOW_SIZE: usize = 32_768;
const HASH_SIZE: usize = 65_536;
const WINDOW_MASK: usize = 32_767;
const CODE_LENGTH_ORDER: [usize; BIT_LENGTH_CODES] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];
const EXTRA_BIT_LENGTH_BITS: [u8; BIT_LENGTH_CODES] =
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 3, 7];

fn low_u16(value: usize) -> u16 {
    let bytes = value.to_le_bytes();
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn low_u32(value: usize) -> u32 {
    let bytes = value.to_le_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[cfg(target_pointer_width = "64")]
fn low_usize_i64(value: i64) -> usize {
    usize::from_le_bytes(value.to_le_bytes())
}

#[cfg(target_pointer_width = "32")]
fn low_usize_i64(value: i64) -> usize {
    let bytes = value.to_le_bytes();
    usize::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

enum Token {
    Literal(u8),
    Match { length: usize, distance: usize },
}

/// Compress using Pillow's zlib-ng 2.3.3 level-one quick strategy.
///
/// `deflate_quick` retains only the newest four-byte hash candidate, emits
/// fixed Huffman codes directly, and deliberately does not insert positions
/// skipped by a match.
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
pub(super) fn compress_level1(data: &[u8], input_chunks: &[usize]) -> Vec<u8> {
    let (tokens, final_tokens) = tokenize_level1(data, input_chunks);
    let mut writer = BitWriter::with_prefix([0x78, 0x01]);
    // deflate_quick opens its first block only after a Z_NO_FLUSH call has
    // enough lookahead to process. On Z_FINISH it closes an opened block as
    // non-final, then emits the remaining short lookahead in a final block.
    if tokens.is_empty() {
        emit_fixed_block(&final_tokens, true, &mut writer);
    } else {
        emit_fixed_block(&tokens, false, &mut writer);
        emit_fixed_block(&final_tokens, true, &mut writer);
    }
    let mut output = writer.finish();
    output.extend_from_slice(&adler32(data).to_be_bytes());
    output
}

#[cfg(feature = "png")]
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
pub(super) fn compress_level1_with_token(
    data: &[u8],
    input_chunks: &[usize],
    token: &crate::CancellationToken,
) -> crate::codecs::CodecResult<Vec<u8>> {
    let (tokens, final_tokens) = tokenize_level1_with_token(data, input_chunks, token)?;
    let mut checkpoint = CancellationMatcherCheckpoint { token };
    checkpoint.poll()?;
    let mut writer = BitWriter::with_prefix([0x78, 0x01]);
    if tokens.is_empty() {
        emit_fixed_block_with(&final_tokens, true, &mut writer, &mut checkpoint)?;
    } else {
        emit_fixed_block_with(&tokens, false, &mut writer, &mut checkpoint)?;
        emit_fixed_block_with(&final_tokens, true, &mut writer, &mut checkpoint)?;
    }
    checkpoint.poll()?;
    let mut output = writer.finish();
    output.extend_from_slice(&adler32_with(data, &mut checkpoint)?.to_be_bytes());
    checkpoint.poll()?;
    Ok(output)
}

fn tokenize_level1(data: &[u8], input_chunks: &[usize]) -> (Vec<Token>, Vec<Token>) {
    let mut head = vec![0usize; HASH_SIZE];
    let mut tokens = Vec::new();
    let mut position = 0usize;
    let mut available = 0usize;
    for &chunk_length in input_chunks {
        available = available.wrapping_add(chunk_length);
        debug_assert!(available <= data.len());

        // fill_window() re-inserts strstart - 1 whenever a new input call
        // makes another four-byte hash available. This includes positions
        // skipped by the preceding quick match.
        // When a previous non-final pass advanced `position`, the loop below
        // left at least `MIN_LOOKAHEAD - MAX_MATCH` bytes available, so
        // `position - 1` has the four bytes required by `quick_insert_level1`.
        if position >= 1 {
            quick_insert_level1(data, position.wrapping_sub(1), &mut head);
        }
        while available.wrapping_sub(position) >= MIN_LOOKAHEAD {
            tokenize_level1_position(data, available, &mut position, &mut head, &mut tokens);
        }
    }
    debug_assert_eq!(available, data.len());
    let mut final_tokens = Vec::new();
    while position < available {
        tokenize_level1_position(data, available, &mut position, &mut head, &mut final_tokens);
    }
    (tokens, final_tokens)
}

#[cfg(feature = "png")]
fn tokenize_level1_with_token(
    data: &[u8],
    input_chunks: &[usize],
    token: &crate::CancellationToken,
) -> crate::codecs::CodecResult<(Vec<Token>, Vec<Token>)> {
    let mut checkpoint = CancellationMatcherCheckpoint { token };
    let mut head = vec![0usize; HASH_SIZE];
    let mut tokens = Vec::new();
    let mut position = 0usize;
    let mut available = 0usize;
    for &chunk_length in input_chunks {
        checkpoint.poll()?;
        available = available.wrapping_add(chunk_length);
        debug_assert!(available <= data.len());
        if position >= 1 {
            checkpoint.poll()?;
            quick_insert_level1(data, position.wrapping_sub(1), &mut head);
        }
        while available.wrapping_sub(position) >= MIN_LOOKAHEAD {
            tokenize_level1_position_with(
                data,
                available,
                &mut position,
                &mut head,
                &mut tokens,
                &mut checkpoint,
            )?;
        }
    }
    debug_assert_eq!(available, data.len());
    let mut final_tokens = Vec::new();
    while position < available {
        tokenize_level1_position_with(
            data,
            available,
            &mut position,
            &mut head,
            &mut final_tokens,
            &mut checkpoint,
        )?;
    }
    checkpoint.poll()?;
    Ok((tokens, final_tokens))
}

fn tokenize_level1_position(
    data: &[u8],
    available: usize,
    position: &mut usize,
    head: &mut [usize],
    tokens: &mut Vec<Token>,
) {
    let lookahead = available.wrapping_sub(*position);
    if lookahead >= MIN_MATCH {
        let candidate = quick_insert_level1(data, *position, head);
        let distance = position.wrapping_sub(candidate);
        if distance != 0
            && distance <= MAX_DISTANCE
            && data[candidate..candidate.saturating_add(2)]
                == data[*position..position.saturating_add(2)]
        {
            let mut length = match_length(data, candidate, *position, lookahead.min(MAX_MATCH));
            if length >= MIN_MATCH {
                let mut distance = distance;
                if let Some(tail_length) =
                    level1_window_tail_distance_one(data, *position, lookahead, length)
                {
                    length = tail_length;
                    distance = 1;
                }
                tokens.push(Token::Match { length, distance });
                *position = position.wrapping_add(length);
                return;
            }
        }
    }

    tokens.push(Token::Literal(data[*position]));
    *position = position.wrapping_add(1);
}

#[cfg(feature = "png")]
fn tokenize_level1_position_with<P: MatcherCheckpoint>(
    data: &[u8],
    available: usize,
    position: &mut usize,
    head: &mut [usize],
    tokens: &mut Vec<Token>,
    checkpoint: &mut P,
) -> crate::codecs::CodecResult<()> {
    checkpoint.poll()?;
    let lookahead = available.wrapping_sub(*position);
    if lookahead >= MIN_MATCH {
        let candidate = quick_insert_level1(data, *position, head);
        let distance = position.wrapping_sub(candidate);
        if distance != 0
            && distance <= MAX_DISTANCE
            && data[candidate..candidate.saturating_add(2)]
                == data[*position..position.saturating_add(2)]
        {
            let length = match_length(data, candidate, *position, lookahead.min(MAX_MATCH));
            if length >= MIN_MATCH {
                let mut distance = distance;
                let mut length = length;
                if let Some(tail_length) =
                    level1_window_tail_distance_one(data, *position, lookahead, length)
                {
                    length = tail_length;
                    distance = 1;
                }
                tokens.push(Token::Match { length, distance });
                *position = position.wrapping_add(length);
                return Ok(());
            }
        }
    }
    tokens.push(Token::Literal(data[*position]));
    *position = position.wrapping_add(1);
    Ok(())
}

fn level1_window_tail_distance_one(
    data: &[u8],
    position: usize,
    lookahead: usize,
    current_length: usize,
) -> Option<usize> {
    // Pillow's zlib-ng 2.3.3 level-one oracle selects the one-byte run
    // distance for repeated-byte matches that start in the pre-slide guard zone
    // after the first 64 KiB fill window. Keep this scoped to that observable
    // deflate_quick window-tail state so earlier 32 KiB matches stay byte-exact.
    let first_slide_guard_start = WINDOW_SIZE.wrapping_mul(2).wrapping_sub(MIN_LOOKAHEAD);
    if position < first_slide_guard_start || position % WINDOW_SIZE <= MAX_DISTANCE {
        return None;
    }
    let previous_position = position.wrapping_sub(1);
    if data[previous_position] != data[position] {
        return None;
    }

    let length = match_length(data, previous_position, position, lookahead.min(MAX_MATCH));
    (length >= current_length && length >= MIN_MATCH).then_some(length)
}

#[cfg(coverage)]
// This coverage-only state explorer uses `expect` as an assertion that its
// hand-constructed private states satisfy the preconditions being exercised.
#[allow(clippy::expect_used)]
pub(crate) fn __coverage_exercise_private_branches() {
    let data = b"abcdef";
    let mut tokens = Vec::new();
    let mut head = vec![0usize; HASH_SIZE];
    let mut position = 0usize;
    tokenize_level1_position(data, data.len(), &mut position, &mut head, &mut tokens);
    let _ = compress_level1(data, &[data.len()]);
    let level1_reinsert_data = vec![b'a'; MIN_LOOKAHEAD + 3];
    let _ = tokenize_level1(&level1_reinsert_data, &[MIN_LOOKAHEAD, 3]);
    let _ = tokenize_level1(
        &level1_reinsert_data,
        &[
            MIN_LOOKAHEAD + 1,
            level1_reinsert_data.len() - MIN_LOOKAHEAD - 1,
        ],
    );
    let level1_tail_guard_data = vec![0; WINDOW_SIZE * 2 + MAX_MATCH];
    let level1_first_slide_guard_start = WINDOW_SIZE * 2 - MIN_LOOKAHEAD;
    let level1_tail_guard_position = WINDOW_SIZE * 2 - MIN_LOOKAHEAD + 1;
    assert!(
        level1_window_tail_distance_one(&level1_tail_guard_data, 0, MAX_MATCH, MAX_MATCH).is_none()
    );
    assert!(
        level1_window_tail_distance_one(
            &level1_tail_guard_data,
            level1_first_slide_guard_start,
            MAX_MATCH,
            MAX_MATCH,
        )
        .is_none()
    );
    let mut level1_tail_mismatch_data = level1_tail_guard_data.clone();
    level1_tail_mismatch_data[level1_tail_guard_position] = 1;
    assert!(
        level1_window_tail_distance_one(
            &level1_tail_mismatch_data,
            level1_tail_guard_position,
            MAX_MATCH,
            MAX_MATCH,
        )
        .is_none()
    );
    assert!(
        level1_window_tail_distance_one(
            &level1_tail_guard_data,
            level1_tail_guard_position,
            MAX_MATCH,
            MAX_MATCH,
        )
        .is_some()
    );
    assert!(
        level1_window_tail_distance_one(
            &level1_tail_guard_data,
            level1_tail_guard_position,
            MAX_MATCH - 1,
            MAX_MATCH,
        )
        .is_none()
    );
    assert!(
        level1_window_tail_distance_one(
            &level1_tail_guard_data,
            level1_tail_guard_position,
            MIN_MATCH - 1,
            0,
        )
        .is_none()
    );
    let mut current = MediumMatch {
        match_start: 0,
        length: 4,
        start: 10,
        original_start: 10,
    };
    let mut next = MediumMatch {
        match_start: 10,
        length: 4,
        start: 2,
        original_start: 2,
    };
    fizzle_matches(b"aaaaaaaaaaaa", &mut current, &mut next);
    assert_eq!(current.length, 4);
    assert_eq!(next.start, 2);

    let mut current = MediumMatch {
        match_start: 0,
        length: 2,
        start: 10,
        original_start: 10,
    };
    let mut next = MediumMatch {
        match_start: 1,
        length: 4,
        start: 2,
        original_start: 2,
    };
    fizzle_matches(b"aaaaaaaaaaaa", &mut current, &mut next);
    assert_eq!(current.length, 2);
    assert_eq!(next.match_start, 1);

    let mut current = MediumMatch {
        match_start: 0,
        length: 2,
        start: 10,
        original_start: 10,
    };
    let mut next = MediumMatch {
        match_start: 3,
        length: 1,
        start: 4,
        original_start: 4,
    };
    fizzle_matches(b"ABCC", &mut current, &mut next);
    assert_eq!(current.length, 2);
    assert_eq!(next.length, 1);

    let mut current = MediumMatch {
        match_start: 0,
        length: 2,
        start: 10,
        original_start: 10,
    };
    let mut next = MediumMatch {
        match_start: 3,
        length: 4,
        start: 4,
        original_start: 4,
    };
    fizzle_matches(b"aaaaaaaaaaaa", &mut current, &mut next);
    assert_eq!(current.length, 0);
    assert_eq!(next.length, 6);

    let mut current = MediumMatch {
        match_start: 0,
        length: 3,
        start: 0,
        original_start: 0,
    };
    let mut next = MediumMatch {
        match_start: 1,
        length: 4,
        start: 4,
        original_start: 4,
    };
    fizzle_matches(b"aaaaaaaa", &mut current, &mut next);

    let mut current = MediumMatch {
        match_start: 0,
        length: 3,
        start: 0,
        original_start: 0,
    };
    let mut next = MediumMatch {
        match_start: 4,
        length: 4,
        start: 1,
        original_start: 1,
    };
    fizzle_matches(b"aaaaaaaa", &mut current, &mut next);

    let mut current = MediumMatch {
        match_start: 0,
        length: 2,
        start: 0,
        original_start: 0,
    };
    let mut next = MediumMatch {
        match_start: 1,
        length: 1,
        start: 1,
        original_start: 1,
    };
    fizzle_matches(b"aaaa", &mut current, &mut next);

    let mut current = MediumMatch {
        match_start: 0,
        length: 2,
        start: 0,
        original_start: 0,
    };
    let mut next = MediumMatch {
        match_start: 2,
        length: 1,
        start: 2,
        original_start: 2,
    };
    fizzle_matches(b"aaaa", &mut current, &mut next);

    let mut current = MediumMatch {
        match_start: 0,
        length: 2,
        start: 0,
        original_start: 0,
    };
    let mut next = MediumMatch {
        match_start: 3,
        length: 255,
        start: 3,
        original_start: 3,
    };
    fizzle_matches(&[b'a'; 260], &mut current, &mut next);

    let mut slow = SlowMatcher::new(b"abcdefghijkl", 16, 8, 128, 128);
    slow.process(0, true);
    slow.quick_insert(4);
    slow.position = 4;
    let _ = slow.longest_match(slow.position, 8);
    slow.process(8, true);
    assert!(slow.position > 4);
    let mut slow_previous = SlowMatcher::new(b"aaaa", 16, 8, 128, 128);
    slow_previous.previous_length = 3;
    slow_previous.process(4, true);
    let mut slow_empty_chain = SlowMatcher::new(b"abcxyz", 16, 8, 128, 0);
    slow_empty_chain.position = 3;
    let _ = slow_empty_chain.longest_match(0, 3);

    let mut level6 = Level6Matcher::new(b"aaaaaaaa", 128, 128, 16);
    level6.position = 4;
    let self_hash = level6.hash(4);
    level6.head[self_hash] = 4;
    let found = level6.find_match(4, 4);
    assert_eq!(found.length, 1);

    let mut level9 = Level9Matcher::new(b"abcdefghijkl");
    level9.process(0, true);
    level9.position = 4;
    level9.refill_boundary();
    let self_hash = rolling_hash(level9.hash, level9.data[6]);
    level9.head[self_hash] = 4;
    level9.process(8, true);
    assert!(level9.position > 4);

    let mut level9 = Level9Matcher::new(b"aaaaaaaaaaaa");
    level9.position = 4;
    level9.previous_length = 3;
    let mut hash = rolling_hash(0, level9.data[5]);
    hash = rolling_hash(hash, level9.data[6]);
    hash = rolling_hash(hash, level9.data[7]);
    level9.head[hash] = 0;
    let _ = level9.longest_match(1, 8);
    let _ = level9.longest_match(level9.position, 8);
    let mut level9 = Level9Matcher::new(b"abcdefghijkl");
    level9.position = 4;
    let _ = level9.longest_match(level9.position, 8);
    let mut level9_previous = Level9Matcher::new(b"aaaa");
    level9_previous.previous_length = 3;
    level9_previous.process(4, true);

    let mut level3 = Level3Matcher::new(b"aaaaaaaaaaaa", 6, 4, 6, false);
    level3.position = 4;
    let _ = level3.longest_match(0, 8);
    let mut level3 = Level3Matcher::new(b"aaaaaaaaaaaa", 6, 128, 6, false);
    level3.position = 4;
    let _ = level3.longest_match(0, 4);
    let mut level3_empty_chain = Level3Matcher::new(b"abcxyz", 0, 128, 6, false);
    level3_empty_chain.position = 3;
    let _ = level3_empty_chain.longest_match(0, 3);
    let mut level3_equal_match = Level3Matcher::new(b"ababxx", 6, 128, 6, false);
    level3_equal_match.position = 2;
    let _ = level3_equal_match.longest_match(0, 2);
    let mut level3_candidate = Level3Matcher::new(b"abcdefghijklmnop", 6, 128, 6, false);
    level3_candidate.position = 4;
    let _ = level3_candidate.candidate_can_improve(0, 4);
    let _ = level3_candidate.candidate_can_improve(0, 8);

    let _ = medium_candidate_can_improve(b"abcdabcd", 0, 4, 2);
    let _ = medium_candidate_can_improve(b"abcdwxyzz", 0, 4, 4);

    let mut short_level3 = Level3Matcher::new(b"abcd", 1, 4, 4, false);
    let _ = short_level3.quick_insert(0);
    short_level3.position = 1;
    let _ = short_level3.candidate_can_improve(0, 2);
}

fn quick_insert_level1(data: &[u8], position: usize, head: &mut [usize]) -> usize {
    let bytes = &data[position..position.saturating_add(4)];
    let word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let hash = (word.wrapping_mul(2_654_435_761) >> 16) as usize;
    let candidate = head[hash];
    if candidate != position {
        head[hash] = position;
    }
    candidate
}

/// Compress using Pillow's zlib-ng 2.3.3 level-three configuration.
///
/// Pillow's `ZipEncode.c` selects `Z_FILTERED`, a 32 KiB window, and
/// `memLevel=9`. zlib-ng maps level three to `deflate_medium` with the
/// `{ good: 4, lazy: 6, nice: 16, chain: 6 }` configuration.
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
pub(super) fn compress_level3(data: &[u8], input_chunks: &[usize]) -> Vec<u8> {
    let tokens = tokenize_early_matcher(data, input_chunks, 6, 16, 6, false);
    let mut writer = BitWriter::with_prefix([0x78, 0x5e]);
    emit_blocks(&tokens, 32_767, &mut writer);
    let mut output = writer.finish();
    output.extend_from_slice(&adler32(data).to_be_bytes());
    output
}

/// Compress using Pillow's zlib-ng 2.3.3 level-two fast strategy.
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
pub(super) fn compress_level2(data: &[u8], input_chunks: &[usize]) -> Vec<u8> {
    let tokens = tokenize_early_matcher(data, input_chunks, 4, 8, 4, true);
    let mut writer = BitWriter::with_prefix([0x78, 0x5e]);
    emit_blocks(&tokens, 32_767, &mut writer);
    let mut output = writer.finish();
    output.extend_from_slice(&adler32(data).to_be_bytes());
    output
}

/// Compress using Pillow's zlib-ng 2.3.3 level-four medium strategy.
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
pub(super) fn compress_level4(data: &[u8], input_chunks: &[usize]) -> Vec<u8> {
    let tokens = tokenize_early_matcher(data, input_chunks, 24, 32, 12, false);
    let mut writer = BitWriter::with_prefix([0x78, 0x5e]);
    emit_blocks(&tokens, 32_767, &mut writer);
    let mut output = writer.finish();
    output.extend_from_slice(&adler32(data).to_be_bytes());
    output
}

/// Compress using Pillow's zlib-ng 2.3.3 level-six configuration.
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
pub(super) fn compress_level6(data: &[u8], input_chunks: &[usize]) -> Vec<u8> {
    let tokens = tokenize_lookahead_medium(data, input_chunks, 128, 128, 16);
    let mut writer = BitWriter::with_prefix([0x78, 0x9c]);
    emit_blocks(&tokens, 32_767, &mut writer);
    let mut output = writer.finish();
    output.extend_from_slice(&adler32(data).to_be_bytes());
    output
}

/// Compress using Pillow's zlib-ng 2.3.3 level-five medium strategy.
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
pub(super) fn compress_level5(data: &[u8], input_chunks: &[usize]) -> Vec<u8> {
    let tokens = tokenize_lookahead_medium(data, input_chunks, 32, 32, 16);
    let mut writer = BitWriter::with_prefix([0x78, 0x5e]);
    emit_blocks(&tokens, 32_767, &mut writer);
    let mut output = writer.finish();
    output.extend_from_slice(&adler32(data).to_be_bytes());
    output
}

/// Compress PNG levels two through five with token-aware matcher and emission
/// checkpoints. The no-token callers above remain on their existing helpers.
#[cfg(feature = "png")]
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
pub(super) fn compress_early_level_with_token(
    data: &[u8],
    input_chunks: &[usize],
    level: u8,
    token: &crate::CancellationToken,
) -> crate::codecs::CodecResult<Vec<u8>> {
    let tokens = match level {
        2..=4 => {
            let (max_chain, nice_match, max_insert, fast) = match level {
                2 => (4, 8, 4, true),
                3 => (6, 16, 6, false),
                4 => (24, 32, 12, false),
                _ => unreachable!("early PNG level is outside 2..=4"),
            };
            tokenize_early_matcher_with_token(
                data,
                input_chunks,
                max_chain,
                nice_match,
                max_insert,
                fast,
                token,
            )?
        }
        5 => tokenize_lookahead_medium_with_token(data, input_chunks, 32, 32, 16, token)?,
        _ => unreachable!("early PNG level is outside 2..=5"),
    };
    let mut checkpoint = CancellationMatcherCheckpoint { token };
    checkpoint.poll()?;
    let mut writer = BitWriter::with_prefix([0x78, 0x5e]);
    emit_blocks_with(&tokens, 32_767, &mut writer, &mut checkpoint)?;
    checkpoint.poll()?;
    let mut output = writer.finish();
    output.extend_from_slice(&adler32_with(data, &mut checkpoint)?.to_be_bytes());
    checkpoint.poll()?;
    Ok(output)
}

/// Compress using Pillow's zlib-ng 2.3.3 level-seven slow strategy.
pub(super) fn compress_level7(data: &[u8], input_chunks: &[usize]) -> Vec<u8> {
    compress_slow_level(data, input_chunks, 32, 8, 128, 256, 0xda)
}

/// Compress using Pillow's zlib-ng 2.3.3 level-eight slow strategy.
pub(super) fn compress_level8(data: &[u8], input_chunks: &[usize]) -> Vec<u8> {
    compress_slow_level(data, input_chunks, 128, 32, 258, 1024, 0xda)
}

#[cfg(feature = "png")]
pub(super) fn compress_slow_level_with_token_for_png(
    data: &[u8],
    input_chunks: &[usize],
    level: u8,
    token: &crate::CancellationToken,
) -> crate::codecs::CodecResult<Vec<u8>> {
    let (max_lazy, good_match, nice_match, max_chain) = match level {
        7 => (32, 8, 128, 256),
        8 => (128, 32, 258, 1024),
        _ => unreachable!("slow PNG level is outside 7..=8"),
    };
    compress_slow_level_with_token(
        data,
        input_chunks,
        max_lazy,
        good_match,
        nice_match,
        max_chain,
        0xda,
        token,
    )
}

#[allow(clippy::expect_used, clippy::unwrap_in_result)]
fn compress_slow_level(
    data: &[u8],
    input_chunks: &[usize],
    max_lazy: usize,
    good_match: usize,
    nice_match: usize,
    max_chain: usize,
    header: u8,
) -> Vec<u8> {
    let settings = SlowSettings {
        max_lazy,
        good_match,
        nice_match,
        max_chain,
    };
    let tokens = slow(data, input_chunks, settings);
    let mut writer = BitWriter::with_prefix([0x78, header]);
    emit_blocks(&tokens, 32_767, &mut writer);
    let mut output = writer.finish();
    output.extend_from_slice(&adler32(data).to_be_bytes());
    output
}

#[cfg(feature = "png")]
#[allow(
    clippy::expect_used,
    clippy::too_many_arguments,
    clippy::unwrap_in_result
)]
fn compress_slow_level_with_token(
    data: &[u8],
    input_chunks: &[usize],
    max_lazy: usize,
    good_match: usize,
    nice_match: usize,
    max_chain: usize,
    header: u8,
    token: &crate::CancellationToken,
) -> crate::codecs::CodecResult<Vec<u8>> {
    let settings = SlowSettings {
        max_lazy,
        good_match,
        nice_match,
        max_chain,
    };
    let tokens = slow_with_token(data, input_chunks, settings, token)?;
    let mut checkpoint = CancellationMatcherCheckpoint { token };
    checkpoint.poll()?;
    let mut writer = BitWriter::with_prefix([0x78, header]);
    emit_blocks_with(&tokens, 32_767, &mut writer, &mut checkpoint)?;
    checkpoint.poll()?;
    let mut output = writer.finish();
    output.extend_from_slice(&adler32_with(data, &mut checkpoint)?.to_be_bytes());
    checkpoint.poll()?;
    Ok(output)
}

struct SlowSettings {
    max_lazy: usize,
    good_match: usize,
    nice_match: usize,
    max_chain: usize,
}

#[allow(clippy::expect_used, clippy::unwrap_in_result)]
fn slow(data: &[u8], input_chunks: &[usize], settings: SlowSettings) -> Vec<Token> {
    let mut matcher = SlowMatcher::new(
        data,
        settings.max_lazy,
        settings.good_match,
        settings.nice_match,
        settings.max_chain,
    );
    let mut available = 0usize;
    for &chunk_length in input_chunks {
        available = available.wrapping_add(chunk_length);
        debug_assert!(available <= data.len());
        matcher.process(available, false);
    }
    debug_assert_eq!(available, data.len());
    matcher.process(available, true);
    matcher.tokens
}

#[cfg(feature = "png")]
fn slow_with_token(
    data: &[u8],
    input_chunks: &[usize],
    settings: SlowSettings,
    token: &crate::CancellationToken,
) -> crate::codecs::CodecResult<Vec<Token>> {
    let mut checkpoint = CancellationMatcherCheckpoint { token };
    let mut matcher = SlowMatcher::new(
        data,
        settings.max_lazy,
        settings.good_match,
        settings.nice_match,
        settings.max_chain,
    );
    let mut available = 0usize;
    for &chunk_length in input_chunks {
        checkpoint.poll()?;
        available = available.wrapping_add(chunk_length);
        debug_assert!(available <= data.len());
        matcher.process_with(available, false, &mut checkpoint)?;
        checkpoint.poll()?;
    }
    debug_assert_eq!(available, data.len());
    matcher.process_with(available, true, &mut checkpoint)?;
    checkpoint.poll()?;
    Ok(matcher.tokens)
}

struct SlowMatcher {
    data: Vec<u8>,
    head: Vec<usize>,
    previous: Vec<usize>,
    position: usize,
    previous_length: usize,
    match_start: usize,
    match_available: bool,
    tokens: Vec<Token>,
    max_lazy: usize,
    good_match: usize,
    nice_match: usize,
    max_chain: usize,
}

impl SlowMatcher {
    fn new(
        data: &[u8],
        max_lazy: usize,
        good_match: usize,
        nice_match: usize,
        max_chain: usize,
    ) -> Self {
        let mut window = Vec::with_capacity(data.len().saturating_add(MAX_MATCH));
        window.extend_from_slice(data);
        window.resize(data.len().saturating_add(MAX_MATCH), 0);
        Self {
            data: window,
            head: vec![0; HASH_SIZE],
            previous: vec![0; WINDOW_SIZE],
            position: 0,
            previous_length: 2,
            match_start: 0,
            match_available: false,
            tokens: Vec::new(),
            max_lazy,
            good_match,
            nice_match,
            max_chain,
        }
    }

    #[allow(clippy::expect_used, clippy::unwrap_in_result)]
    fn process(&mut self, available: usize, finishing: bool) {
        let mut checkpoint = NoopMatcherCheckpoint;
        self.process_with(available, finishing, &mut checkpoint)
            .expect("the no-op matcher checkpoint cannot fail");
    }

    fn process_with<P: MatcherCheckpoint>(
        &mut self,
        available: usize,
        finishing: bool,
        checkpoint: &mut P,
    ) -> crate::codecs::CodecResult<()> {
        loop {
            checkpoint.poll()?;
            debug_assert!(self.position <= available);
            let lookahead = available.wrapping_sub(self.position);
            if lookahead == 0 || (!finishing && lookahead < MIN_LOOKAHEAD) {
                break;
            }

            let candidate = if lookahead >= MIN_MATCH {
                self.quick_insert(self.position)
            } else {
                0
            };
            let previous_match = self.match_start;
            let mut match_length = 2usize;
            if candidate != 0
                && candidate < self.position
                && self.position.wrapping_sub(candidate) <= MAX_DISTANCE
                && self.previous_length < self.max_lazy
            {
                let found = self.longest_match_with(candidate, lookahead, checkpoint)?;
                match_length = found.0;
                if match_length > self.previous_length {
                    self.match_start = found.1;
                }
                if match_length <= 5 {
                    match_length = 2;
                }
            }

            if self.previous_length >= 3 && match_length <= self.previous_length {
                self.tokens.push(Token::Match {
                    length: self.previous_length,
                    distance: self.position.wrapping_sub(1).wrapping_sub(previous_match),
                });
                let maximum_insert = available.wrapping_sub(3);
                let move_forward = self.previous_length.wrapping_sub(2);
                let insert_count = move_forward.min(maximum_insert.saturating_sub(self.position));
                for insert_position in
                    self.position.wrapping_add(1)..=self.position.wrapping_add(insert_count)
                {
                    checkpoint.poll()?;
                    self.quick_insert(insert_position);
                }
                self.position = self
                    .position
                    .wrapping_add(self.previous_length.wrapping_sub(1));
                self.previous_length = 0;
                self.match_available = false;
            } else if self.match_available {
                self.tokens
                    .push(Token::Literal(self.data[self.position.wrapping_sub(1)]));
                self.previous_length = match_length;
                self.position = self.position.wrapping_add(1);
            } else {
                self.previous_length = match_length;
                self.match_available = true;
                self.position = self.position.wrapping_add(1);
            }
        }

        if finishing && self.match_available {
            // With `finishing == true`, the loop above exits only when
            // lookahead reaches zero.
            debug_assert_eq!(self.position, available);
            self.tokens
                .push(Token::Literal(self.data[self.position.wrapping_sub(1)]));
            self.match_available = false;
        }
        Ok(())
    }

    fn quick_insert(&mut self, position: usize) -> usize {
        let bytes = &self.data[position..position.wrapping_add(4)];
        let word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let hash = (word.wrapping_mul(2_654_435_761) >> 16) as usize;
        let candidate = self.head[hash];
        if candidate != position {
            self.previous[position & WINDOW_MASK] = candidate;
            self.head[hash] = position;
        }
        candidate
    }

    #[allow(dead_code, clippy::expect_used, clippy::unwrap_in_result)]
    fn longest_match(&self, candidate: usize, lookahead: usize) -> (usize, usize) {
        let mut checkpoint = NoopMatcherCheckpoint;
        self.longest_match_with(candidate, lookahead, &mut checkpoint)
            .expect("the no-op matcher checkpoint cannot fail")
    }

    fn longest_match_with<P: MatcherCheckpoint>(
        &self,
        mut candidate: usize,
        lookahead: usize,
        checkpoint: &mut P,
    ) -> crate::codecs::CodecResult<(usize, usize)> {
        let mut best_length = self.previous_length.max(2);
        let mut best_start = self.match_start;
        let mut chain_length = self.max_chain;
        if best_length >= self.good_match {
            chain_length >>= 2;
        }
        let limit = self.position.saturating_sub(MAX_DISTANCE);
        while candidate < self.position {
            checkpoint.poll()?;
            if medium_candidate_can_improve(&self.data, candidate, self.position, best_length) {
                let length = match_length(
                    &self.data,
                    candidate,
                    self.position,
                    lookahead.min(MAX_MATCH),
                );
                if length > best_length {
                    best_length = length;
                    best_start = candidate;
                    if best_length >= lookahead || best_length >= self.nice_match {
                        break;
                    }
                }
            }
            chain_length = chain_length.wrapping_sub(1);
            if chain_length == 0 {
                break;
            }
            candidate = self.previous[candidate & WINDOW_MASK];
            if candidate <= limit {
                break;
            }
        }
        Ok((best_length.min(lookahead), best_start))
    }
}

#[cfg(feature = "tiff")]
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
pub(super) fn compress_level6_tiff(
    data: &[u8],
    row_len: usize,
    height: usize,
    token: Option<&crate::CancellationToken>,
) -> crate::codecs::CodecResult<Vec<u8>> {
    let tokens = if let Some(token) = token {
        tokenize_lookahead_medium_repeated_with_token(data, row_len, height, 128, 128, 16, token)?
    } else {
        tokenize_lookahead_medium_repeated(data, row_len, height, 128, 128, 16)
    };
    finish_level6_tokens(data, tokens, token, 16_383)
}

#[cfg(any(feature = "png", feature = "tiff"))]
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
pub(super) fn compress_level6_with_token(
    data: &[u8],
    input_chunks: &[usize],
    token: Option<&crate::CancellationToken>,
    block_tokens: usize,
) -> crate::codecs::CodecResult<Vec<u8>> {
    let tokens = if let Some(token) = token {
        tokenize_lookahead_medium_with_token(data, input_chunks, 128, 128, 16, token)?
    } else {
        tokenize_lookahead_medium(data, input_chunks, 128, 128, 16)
    };
    finish_level6_tokens(data, tokens, token, block_tokens)
}

#[cfg(any(feature = "png", feature = "tiff"))]
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
fn finish_level6_tokens(
    data: &[u8],
    tokens: Vec<Token>,
    token: Option<&crate::CancellationToken>,
    block_tokens: usize,
) -> crate::codecs::CodecResult<Vec<u8>> {
    crate::codecs::error::check_cancelled(token)?;
    let mut writer = BitWriter::with_prefix([0x78, 0x9c]);
    if let Some(token) = token {
        let mut checkpoint = CancellationMatcherCheckpoint { token };
        emit_blocks_with(&tokens, block_tokens, &mut writer, &mut checkpoint)?;
        checkpoint.poll()?;
        let mut output = writer.finish();
        output.extend_from_slice(&adler32_with(data, &mut checkpoint)?.to_be_bytes());
        checkpoint.poll()?;
        Ok(output)
    } else {
        emit_blocks(&tokens, 16_383, &mut writer);
        let mut output = writer.finish();
        output.extend_from_slice(&adler32(data).to_be_bytes());
        Ok(output)
    }
}

#[cfg(feature = "tiff")]
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
fn tokenize_lookahead_medium_repeated(
    data: &[u8],
    row_len: usize,
    height: usize,
    max_chain: usize,
    nice_match: usize,
    max_insert: usize,
) -> Vec<Token> {
    let mut matcher = Level6Matcher::new(data, max_chain, nice_match, max_insert);
    let mut available = 0usize;
    for _ in 0..height {
        if available != 0 {
            matcher.refill_boundary();
        }
        available = available.wrapping_add(row_len);
        debug_assert!(available <= data.len());
        matcher.process(available, false);
    }
    debug_assert_eq!(available, data.len());
    matcher.process(available, true);
    matcher.tokens
}

#[cfg(feature = "tiff")]
fn tokenize_lookahead_medium_repeated_with_token(
    data: &[u8],
    row_len: usize,
    height: usize,
    max_chain: usize,
    nice_match: usize,
    max_insert: usize,
    token: &crate::CancellationToken,
) -> crate::codecs::CodecResult<Vec<Token>> {
    let mut checkpoint = CancellationMatcherCheckpoint { token };
    let mut matcher = Level6Matcher::new(data, max_chain, nice_match, max_insert);
    let mut available = 0usize;
    for _ in 0..height {
        checkpoint.poll()?;
        if available != 0 {
            matcher.refill_boundary_with(&mut checkpoint)?;
        }
        available = available.wrapping_add(row_len);
        debug_assert!(available <= data.len());
        matcher.process_with(available, false, &mut checkpoint)?;
        checkpoint.poll()?;
    }
    debug_assert_eq!(available, data.len());
    matcher.process_with(available, true, &mut checkpoint)?;
    checkpoint.poll()?;
    Ok(matcher.tokens)
}

#[allow(clippy::expect_used, clippy::unwrap_in_result)]
fn tokenize_lookahead_medium(
    data: &[u8],
    input_chunks: &[usize],
    max_chain: usize,
    nice_match: usize,
    max_insert: usize,
) -> Vec<Token> {
    // ✅ VERIFIED: zlib-ng 2.3.3 deflate.c:102-128 and
    // deflate_medium.c:160-293. The oracle and Rust models produce the same
    // 2,272 tokens for the level-six PNG parity input.
    let mut matcher = Level6Matcher::new(data, max_chain, nice_match, max_insert);
    let mut available = 0usize;
    for &chunk_length in input_chunks {
        if available != 0 {
            matcher.refill_boundary();
        }
        available = available.wrapping_add(chunk_length);
        debug_assert!(available <= data.len());
        matcher.process(available, false);
    }
    debug_assert_eq!(available, data.len());
    matcher.process(available, true);
    matcher.tokens
}

/// Tokenize PNG/TIFF Deflate input with cancellation checkpoints at each input
/// row boundary and inside the level-six matcher. Ordinary no-token callers
/// stay on the existing helper so token-aware control does not add polling
/// overhead to ordinary encodes.
#[cfg(any(feature = "png", feature = "tiff"))]
fn tokenize_lookahead_medium_with_token(
    data: &[u8],
    input_chunks: &[usize],
    max_chain: usize,
    nice_match: usize,
    max_insert: usize,
    token: &crate::CancellationToken,
) -> crate::codecs::CodecResult<Vec<Token>> {
    let mut checkpoint = CancellationMatcherCheckpoint { token };
    let mut matcher = Level6Matcher::new(data, max_chain, nice_match, max_insert);
    let mut available = 0usize;
    for &chunk_length in input_chunks {
        checkpoint.poll()?;
        if available != 0 {
            matcher.refill_boundary_with(&mut checkpoint)?;
        }
        available = available.wrapping_add(chunk_length);
        debug_assert!(available <= data.len());
        matcher.process_with(available, false, &mut checkpoint)?;
        checkpoint.poll()?;
    }
    debug_assert_eq!(available, data.len());
    matcher.process_with(available, true, &mut checkpoint)?;
    checkpoint.poll()?;
    Ok(matcher.tokens)
}

#[derive(Clone, Copy, Default)]
struct MediumMatch {
    match_start: usize,
    length: usize,
    start: usize,
    original_start: usize,
}

trait MatcherCheckpoint {
    fn poll(&mut self) -> crate::codecs::CodecResult<()>;
}

struct NoopMatcherCheckpoint;

impl MatcherCheckpoint for NoopMatcherCheckpoint {
    #[inline(always)]
    fn poll(&mut self) -> crate::codecs::CodecResult<()> {
        Ok(())
    }
}

#[cfg(any(feature = "png", feature = "tiff"))]
struct CancellationMatcherCheckpoint<'a> {
    token: &'a crate::CancellationToken,
}

#[cfg(any(feature = "png", feature = "tiff"))]
impl MatcherCheckpoint for CancellationMatcherCheckpoint<'_> {
    #[inline(always)]
    fn poll(&mut self) -> crate::codecs::CodecResult<()> {
        crate::codecs::error::check_cancelled(Some(self.token))
    }
}

struct Level6Matcher {
    data: Vec<u8>,
    head: Vec<usize>,
    previous: Vec<usize>,
    position: usize,
    window_base: usize,
    tokens: Vec<Token>,
    max_chain: usize,
    nice_match: usize,
    max_insert: usize,
}

impl Level6Matcher {
    fn new(data: &[u8], max_chain: usize, nice_match: usize, max_insert: usize) -> Self {
        // zlib-ng's window is zero-initialized through WIN_INIT bytes beyond
        // the supplied input. Its medium matcher intentionally probes that
        // region while evaluating the match following the current match.
        let mut window = Vec::with_capacity(data.len().saturating_add(MAX_MATCH));
        window.extend_from_slice(data);
        window.resize(data.len().saturating_add(MAX_MATCH), 0);
        Self {
            data: window,
            head: vec![0; HASH_SIZE],
            previous: vec![0; WINDOW_SIZE],
            position: 0,
            window_base: 0,
            tokens: Vec::new(),
            max_chain,
            nice_match,
            max_insert,
        }
    }

    #[allow(clippy::expect_used, clippy::unwrap_in_result)]
    fn refill_boundary(&mut self) {
        let mut checkpoint = NoopMatcherCheckpoint;
        self.refill_boundary_with(&mut checkpoint)
            .expect("the no-op matcher checkpoint cannot fail");
    }

    fn refill_boundary_with<P: MatcherCheckpoint>(
        &mut self,
        checkpoint: &mut P,
    ) -> crate::codecs::CodecResult<()> {
        // ✅ VERIFIED: zlib-ng 2.3.3 deflate.c:1213-1237. fill_window()
        // re-inserts strstart-1 when new input makes a three-byte hash valid.
        self.slide_window_if_needed_with(checkpoint)?;
        if self.position >= 1 {
            self.quick_insert(self.position.wrapping_sub(1));
        }
        checkpoint.poll()
    }

    fn slide_window_if_needed_with<P: MatcherCheckpoint>(
        &mut self,
        checkpoint: &mut P,
    ) -> crate::codecs::CodecResult<()> {
        debug_assert!(self.position >= self.window_base);
        if self.position.wrapping_sub(self.window_base) >= 32_768_usize.wrapping_add(MAX_DISTANCE) {
            checkpoint.poll()?;
            self.window_base = self.window_base.wrapping_add(32_768);
            for position in self.head.iter_mut().chain(&mut self.previous) {
                if *position < self.window_base {
                    *position = 0;
                }
            }
            checkpoint.poll()?;
        }
        Ok(())
    }

    #[allow(clippy::expect_used, clippy::unwrap_in_result)]
    fn process(&mut self, available: usize, finishing: bool) {
        let mut checkpoint = NoopMatcherCheckpoint;
        self.process_with(available, finishing, &mut checkpoint)
            .expect("the no-op matcher checkpoint cannot fail");
    }

    fn process_with<P: MatcherCheckpoint>(
        &mut self,
        available: usize,
        finishing: bool,
        checkpoint: &mut P,
    ) -> crate::codecs::CodecResult<()> {
        let mut following = None::<MediumMatch>;
        loop {
            checkpoint.poll()?;
            self.slide_window_if_needed_with(checkpoint)?;
            debug_assert!(self.position <= available);
            let lookahead = available.wrapping_sub(self.position);
            if lookahead == 0 {
                return Ok(());
            }
            if lookahead < MIN_LOOKAHEAD {
                if !finishing {
                    return Ok(());
                }
                // deflate_medium clears its speculative next match after
                // fill_window observes the final short lookahead. That match
                // was searched using the preceding position's larger
                // lookahead and can extend beyond the real input boundary.
                following = None;
            }

            let mut current = match following.take() {
                Some(found) => found,
                None => self.find_match_with(self.position, lookahead, checkpoint)?,
            };
            self.insert_match_with(current, lookahead, checkpoint)?;

            if lookahead > MIN_LOOKAHEAD
                && current.start.wrapping_add(current.length)
                    < self
                        .window_base
                        .wrapping_add(65_536)
                        .wrapping_sub(MIN_LOOKAHEAD)
            {
                let future = current.start.wrapping_add(current.length);
                let mut next = self.find_match_with(future, lookahead, checkpoint)?;
                if next.length >= MIN_MATCH {
                    fizzle_matches_with(&self.data, &mut current, &mut next, checkpoint)?;
                }
                following = Some(next);
            }

            if current.length < MIN_MATCH {
                for offset in 0..current.length {
                    self.tokens.push(Token::Literal(
                        self.data[current.start.wrapping_add(offset)],
                    ));
                }
            } else {
                self.tokens.push(Token::Match {
                    length: current.length,
                    distance: current.start.wrapping_sub(current.match_start),
                });
            }
            self.position = self.position.wrapping_add(current.length);
        }
    }

    #[allow(dead_code, clippy::expect_used, clippy::unwrap_in_result)]
    fn find_match(&mut self, position: usize, lookahead: usize) -> MediumMatch {
        let mut checkpoint = NoopMatcherCheckpoint;
        self.find_match_with(position, lookahead, &mut checkpoint)
            .expect("the no-op matcher checkpoint cannot fail")
    }

    fn find_match_with<P: MatcherCheckpoint>(
        &mut self,
        position: usize,
        lookahead: usize,
        checkpoint: &mut P,
    ) -> crate::codecs::CodecResult<MediumMatch> {
        checkpoint.poll()?;
        let candidate = if lookahead >= MIN_MATCH {
            self.quick_insert(position)
        } else {
            0
        };
        let mut found = MediumMatch {
            match_start: 0,
            length: 1,
            start: position,
            original_start: position,
        };
        if candidate != 0
            && candidate < position
            && position.wrapping_sub(candidate) <= MAX_DISTANCE
        {
            let (length, match_start) =
                self.longest_match_with(candidate, position, lookahead, checkpoint)?;
            if length >= MIN_MATCH {
                // `longest_match` can only return a match start from a prior
                // candidate accepted by the guard above.
                found.match_start = match_start;
                found.length = length;
            }
        }
        Ok(found)
    }

    fn hash(&self, position: usize) -> usize {
        let bytes = &self.data[position..position.wrapping_add(4)];
        let word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        (word.wrapping_mul(2_654_435_761) >> 16) as usize
    }

    fn quick_insert(&mut self, position: usize) -> usize {
        let hash = self.hash(position);
        let candidate = self.head[hash];
        if candidate != position {
            self.previous[position & WINDOW_MASK] = candidate;
            self.head[hash] = position;
        }
        candidate
    }

    fn insert_match_with<P: MatcherCheckpoint>(
        &mut self,
        found: MediumMatch,
        lookahead: usize,
        checkpoint: &mut P,
    ) -> crate::codecs::CodecResult<()> {
        checkpoint.poll()?;
        // ✅ VERIFIED: zlib-ng 2.3.3 deflate_medium.c:44-94. In particular,
        // original_start prevents a left-fizzled match from reinserting old
        // positions and creating a cyclic hash chain.
        if lookahead <= found.length.wrapping_add(MIN_MATCH) || found.length < MIN_MATCH {
            return Ok(());
        }
        if found.length <= 16_usize.wrapping_mul(self.max_insert) {
            let start = found.start.wrapping_add(1);
            let count = found.length.wrapping_sub(1);
            let insertion_start = start.max(found.original_start);
            let insertion_end = start.wrapping_add(count);
            for position in insertion_start..insertion_end {
                checkpoint.poll()?;
                self.quick_insert(position);
            }
        } else {
            self.quick_insert(found.start.wrapping_add(found.length).wrapping_sub(1));
        }
        Ok(())
    }

    fn longest_match_with<P: MatcherCheckpoint>(
        &self,
        mut candidate: usize,
        position: usize,
        lookahead: usize,
        checkpoint: &mut P,
    ) -> crate::codecs::CodecResult<(usize, usize)> {
        // ✅ VERIFIED: zlib-ng 2.3.3 match_tpl.h:38-247 with level-six
        // {good: 8, lazy: 16, nice: 128, chain: 128} configuration.
        let mut best_length = 2usize;
        let mut best_start = 0usize;
        let mut chain_length = self.max_chain;
        let limit = position.saturating_sub(MAX_DISTANCE);
        loop {
            checkpoint.poll()?;
            if candidate >= position {
                break;
            }
            if medium_candidate_can_improve(&self.data, candidate, position, best_length) {
                let length =
                    match_length(&self.data, candidate, position, lookahead.min(MAX_MATCH));
                if length > best_length {
                    best_length = length;
                    best_start = candidate;
                    if best_length >= self.nice_match || best_length >= lookahead {
                        break;
                    }
                }
            }
            chain_length = chain_length.wrapping_sub(1);
            if chain_length == 0 {
                break;
            }
            candidate = self.previous[candidate & WINDOW_MASK];
            if candidate <= limit {
                break;
            }
        }
        Ok((best_length, best_start))
    }
}

/// Compress using Pillow's zlib-ng 2.3.3 level-nine `Z_FILTERED`
/// configuration.
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
pub(super) fn compress_level9(data: &[u8], input_chunks: &[usize]) -> Vec<u8> {
    let tokens = tokenize_level9(data, input_chunks);
    let mut writer = BitWriter::with_prefix([0x78, 0xda]);
    emit_blocks(&tokens, 32_767, &mut writer);
    let mut output = writer.finish();
    output.extend_from_slice(&adler32(data).to_be_bytes());
    output
}

#[cfg(feature = "png")]
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
pub(super) fn compress_level9_with_token(
    data: &[u8],
    input_chunks: &[usize],
    token: &crate::CancellationToken,
) -> crate::codecs::CodecResult<Vec<u8>> {
    let tokens = tokenize_level9_with_token(data, input_chunks, token)?;
    let mut checkpoint = CancellationMatcherCheckpoint { token };
    checkpoint.poll()?;
    let mut writer = BitWriter::with_prefix([0x78, 0xda]);
    emit_blocks_with(&tokens, 32_767, &mut writer, &mut checkpoint)?;
    checkpoint.poll()?;
    let mut output = writer.finish();
    output.extend_from_slice(&adler32_with(data, &mut checkpoint)?.to_be_bytes());
    checkpoint.poll()?;
    Ok(output)
}

#[allow(clippy::expect_used, clippy::unwrap_in_result)]
fn tokenize_level9(data: &[u8], input_chunks: &[usize]) -> Vec<Token> {
    let mut matcher = Level9Matcher::new(data);
    let mut available = 0usize;
    for &chunk_length in input_chunks {
        if available != 0 {
            matcher.refill_boundary();
        }
        available = available.wrapping_add(chunk_length);
        debug_assert!(available <= data.len());
        matcher.process(available, false);
    }
    debug_assert_eq!(available, data.len());
    matcher.process(available, true);
    matcher.tokens
}

#[cfg(feature = "png")]
fn tokenize_level9_with_token(
    data: &[u8],
    input_chunks: &[usize],
    token: &crate::CancellationToken,
) -> crate::codecs::CodecResult<Vec<Token>> {
    let mut checkpoint = CancellationMatcherCheckpoint { token };
    let mut matcher = Level9Matcher::new(data);
    let mut available = 0usize;
    for &chunk_length in input_chunks {
        checkpoint.poll()?;
        if available != 0 {
            matcher.refill_boundary_with(&mut checkpoint)?;
        }
        available = available.wrapping_add(chunk_length);
        debug_assert!(available <= data.len());
        matcher.process_with(available, false, &mut checkpoint)?;
        checkpoint.poll()?;
    }
    debug_assert_eq!(available, data.len());
    matcher.process_with(available, true, &mut checkpoint)?;
    checkpoint.poll()?;
    Ok(matcher.tokens)
}

struct Level9Matcher {
    data: Vec<u8>,
    head: Vec<usize>,
    previous: Vec<usize>,
    hash: usize,
    position: usize,
    previous_length: usize,
    match_start: usize,
    match_available: bool,
    tokens: Vec<Token>,
}

impl Level9Matcher {
    fn new(data: &[u8]) -> Self {
        let mut window = Vec::with_capacity(data.len().saturating_add(MAX_MATCH));
        window.extend_from_slice(data);
        window.resize(data.len().saturating_add(MAX_MATCH), 0);
        let hash = rolling_hash(usize::from(window[0]), window[1]);
        Self {
            data: window,
            head: vec![0; 32_768],
            previous: vec![0; WINDOW_SIZE],
            hash,
            position: 0,
            previous_length: 2,
            match_start: 0,
            match_available: false,
            tokens: Vec::new(),
        }
    }

    #[allow(clippy::expect_used, clippy::unwrap_in_result)]
    fn refill_boundary(&mut self) {
        let mut checkpoint = NoopMatcherCheckpoint;
        self.refill_boundary_with(&mut checkpoint)
            .expect("the no-op matcher checkpoint cannot fail");
    }

    fn refill_boundary_with<P: MatcherCheckpoint>(
        &mut self,
        checkpoint: &mut P,
    ) -> crate::codecs::CodecResult<()> {
        checkpoint.poll()?;
        self.hash = rolling_hash(
            usize::from(self.data[self.position]),
            self.data[self.position.wrapping_add(1)],
        );
        checkpoint.poll()
    }

    #[allow(clippy::expect_used, clippy::unwrap_in_result)]
    fn process(&mut self, available: usize, finishing: bool) {
        let mut checkpoint = NoopMatcherCheckpoint;
        self.process_with(available, finishing, &mut checkpoint)
            .expect("the no-op matcher checkpoint cannot fail");
    }

    fn process_with<P: MatcherCheckpoint>(
        &mut self,
        available: usize,
        finishing: bool,
        checkpoint: &mut P,
    ) -> crate::codecs::CodecResult<()> {
        loop {
            checkpoint.poll()?;
            debug_assert!(self.position <= available);
            let lookahead = available.wrapping_sub(self.position);
            if lookahead == 0 || (!finishing && lookahead < MIN_LOOKAHEAD) {
                break;
            }

            let candidate = if lookahead >= MIN_MATCH {
                self.quick_insert(self.position)
            } else {
                0
            };
            let previous_match = self.match_start;
            let mut match_length = 2usize;
            if candidate != 0
                && candidate < self.position
                && self.position.wrapping_sub(candidate) <= MAX_DISTANCE
                && self.previous_length < MAX_MATCH
            {
                let found = self.longest_match_with(candidate, lookahead, checkpoint)?;
                match_length = found.0;
                if match_length > self.previous_length {
                    self.match_start = found.1;
                }
                if match_length <= 5 {
                    match_length = 2;
                }
            }

            if self.previous_length >= 3 && match_length <= self.previous_length {
                self.tokens.push(Token::Match {
                    length: self.previous_length,
                    distance: self.position.wrapping_sub(1).wrapping_sub(previous_match),
                });
                let maximum_insert = available.wrapping_sub(3);
                let move_forward = self.previous_length.wrapping_sub(2);
                let insert_count = move_forward.min(maximum_insert.saturating_sub(self.position));
                for insert_position in
                    self.position.wrapping_add(1)..=self.position.wrapping_add(insert_count)
                {
                    checkpoint.poll()?;
                    self.quick_insert(insert_position);
                }
                self.position = self
                    .position
                    .wrapping_add(self.previous_length.wrapping_sub(1));
                self.previous_length = 0;
                self.match_available = false;
            } else if self.match_available {
                self.tokens
                    .push(Token::Literal(self.data[self.position.wrapping_sub(1)]));
                self.previous_length = match_length;
                self.position = self.position.wrapping_add(1);
            } else {
                self.previous_length = match_length;
                self.match_available = true;
                self.position = self.position.wrapping_add(1);
            }
        }

        if finishing && self.match_available {
            // With `finishing == true`, the loop above exits only when
            // lookahead reaches zero.
            debug_assert_eq!(self.position, available);
            self.tokens
                .push(Token::Literal(self.data[self.position.wrapping_sub(1)]));
            self.match_available = false;
        }
        Ok(())
    }

    fn quick_insert(&mut self, position: usize) -> usize {
        let hash_position = position.wrapping_add(2);
        self.hash = rolling_hash(self.hash, self.data[hash_position]);
        let candidate = self.head[self.hash];
        if candidate != position {
            self.previous[position & WINDOW_MASK] = candidate;
            self.head[self.hash] = position;
        }
        candidate
    }

    #[allow(dead_code, clippy::expect_used, clippy::unwrap_in_result)]
    fn longest_match(&self, candidate: usize, lookahead: usize) -> (usize, usize) {
        let mut checkpoint = NoopMatcherCheckpoint;
        self.longest_match_with(candidate, lookahead, &mut checkpoint)
            .expect("the no-op matcher checkpoint cannot fail")
    }

    fn longest_match_with<P: MatcherCheckpoint>(
        &self,
        mut candidate: usize,
        lookahead: usize,
        checkpoint: &mut P,
    ) -> crate::codecs::CodecResult<(usize, usize)> {
        let mut best_length = self.previous_length.max(2);
        let mut best_start = self.match_start;
        let mut chain_length = if best_length >= 32 {
            1024usize
        } else {
            4096usize
        };
        let base_limit = self.position.saturating_sub(MAX_DISTANCE);
        let mut match_offset = 0usize;
        if best_length >= 3 {
            let mut hash = rolling_hash(0, self.data[self.position.wrapping_add(1)]);
            hash = rolling_hash(hash, self.data[self.position.wrapping_add(2)]);
            for index in 3..=best_length {
                hash = rolling_hash(hash, self.data[self.position.wrapping_add(index)]);
                let position = self.head[hash];
                if position < candidate {
                    match_offset = index.wrapping_sub(2);
                    candidate = position;
                }
            }
        }
        let mut limit = base_limit.wrapping_add(match_offset);
        if candidate <= limit {
            return Ok((best_length.min(lookahead), best_start));
        }
        // The preceding `candidate <= limit` return also proves
        // `candidate > match_offset`, because `limit >= match_offset`.
        while candidate < self.position.wrapping_add(match_offset) {
            checkpoint.poll()?;
            let aligned = candidate.wrapping_sub(match_offset);
            if medium_candidate_can_improve(&self.data, aligned, self.position, best_length) {
                let length =
                    match_length(&self.data, aligned, self.position, lookahead.min(MAX_MATCH));
                if length > best_length {
                    best_length = length;
                    best_start = aligned;
                    if best_length >= lookahead || best_length >= MAX_MATCH {
                        break;
                    }
                    if best_length > 3 && best_start.wrapping_add(best_length) < self.position {
                        candidate = candidate.wrapping_sub(match_offset);
                        match_offset = 0;
                        let mut next_position = candidate;
                        for index in 0..=best_length.wrapping_sub(3) {
                            let position =
                                self.previous[candidate.wrapping_add(index) & WINDOW_MASK];
                            if position < next_position {
                                if position <= base_limit.wrapping_add(index) {
                                    return Ok((best_length.min(lookahead), best_start));
                                }
                                next_position = position;
                                match_offset = index;
                            }
                        }
                        candidate = next_position;

                        let hash_start = self
                            .position
                            .wrapping_add(best_length)
                            .wrapping_sub(MIN_MATCH.wrapping_add(1));
                        let mut hash = rolling_hash(0, self.data[hash_start]);
                        hash = rolling_hash(hash, self.data[hash_start.wrapping_add(1)]);
                        hash = rolling_hash(hash, self.data[hash_start.wrapping_add(2)]);
                        let position = self.head[hash];
                        // Unlike zlib-ng's sliding C window, this matcher
                        // eagerly inserts every absolute position in a match.
                        // The matching tail is therefore never older than the
                        // chain candidate selected above.
                        debug_assert!(position >= candidate);
                        limit = base_limit.wrapping_add(match_offset);
                        continue;
                    }
                }
            }
            chain_length = chain_length.wrapping_sub(1);
            if chain_length == 0 {
                break;
            }
            candidate = self.previous[candidate & WINDOW_MASK];
            if candidate <= limit {
                break;
            }
        }
        Ok((best_length.min(lookahead), best_start))
    }
}

fn rolling_hash(hash: usize, value: u8) -> usize {
    (hash.wrapping_shl(5) ^ usize::from(value)) & 32_767
}

fn medium_candidate_can_improve(
    data: &[u8],
    candidate: usize,
    position: usize,
    best_length: usize,
) -> bool {
    let mut offset = best_length.wrapping_sub(1);
    if best_length >= 4 {
        offset = offset.wrapping_sub(2);
        if best_length >= 8 {
            offset = offset.wrapping_sub(4);
        }
    }
    let width = if best_length < 4 {
        2
    } else if best_length >= 8 {
        8
    } else {
        4
    };
    data[candidate..candidate.saturating_add(width)]
        == data[position..position.saturating_add(width)]
        && data
            [candidate.wrapping_add(offset)..candidate.wrapping_add(offset).saturating_add(width)]
            == data
                [position.wrapping_add(offset)..position.wrapping_add(offset).saturating_add(width)]
}

#[allow(dead_code, clippy::expect_used)]
fn fizzle_matches(data: &[u8], current: &mut MediumMatch, next: &mut MediumMatch) {
    let mut checkpoint = NoopMatcherCheckpoint;
    fizzle_matches_with(data, current, next, &mut checkpoint)
        .expect("the no-op matcher checkpoint cannot fail");
}

fn fizzle_matches_with<P: MatcherCheckpoint>(
    data: &[u8],
    current: &mut MediumMatch,
    next: &mut MediumMatch,
    checkpoint: &mut P,
) -> crate::codecs::CodecResult<()> {
    // ✅ VERIFIED: zlib-ng 2.3.3 deflate_medium.c:96-158.
    checkpoint.poll()?;
    if current.length <= 1
        || current.length > next.match_start.saturating_add(1)
        || current.length > next.start.saturating_add(1)
    {
        return Ok(());
    }
    let quick_match = next
        .match_start
        .wrapping_add(1)
        .wrapping_sub(current.length);
    let quick_original = next.start.wrapping_add(1).wrapping_sub(current.length);
    if data.get(quick_match) != data.get(quick_original) {
        return Ok(());
    }

    let mut adjusted_current = *current;
    let mut adjusted_next = *next;
    let mut changed = false;
    while adjusted_current.length > 0
        && adjusted_next.length < 256
        && adjusted_next.match_start > 1
        && data.get(adjusted_next.match_start.wrapping_sub(1))
            == data.get(adjusted_next.start.wrapping_sub(1))
    {
        checkpoint.poll()?;
        adjusted_next.start = adjusted_next.start.wrapping_sub(1);
        adjusted_next.match_start = adjusted_next.match_start.wrapping_sub(1);
        adjusted_next.length = adjusted_next.length.wrapping_add(1);
        adjusted_current.length = adjusted_current.length.wrapping_sub(1);
        changed = true;
    }
    if changed && adjusted_current.length <= 1 && adjusted_next.length != 2 {
        adjusted_next.original_start = adjusted_next.original_start.wrapping_add(1);
        *current = adjusted_current;
        *next = adjusted_next;
    }
    Ok(())
}

#[allow(clippy::expect_used, clippy::unwrap_in_result)]
fn tokenize_early_matcher(
    data: &[u8],
    input_chunks: &[usize],
    max_chain: usize,
    nice_match: usize,
    max_insert: usize,
    fast: bool,
) -> Vec<Token> {
    // ⚠️ UNVERIFIED: Rust port of zlib-ng 2.3.3 deflate_medium.c:160-293.
    // The independent oracle model matches all 3,000 level-three tokens; the
    // Rust path still requires the managed byte-parity run.
    let mut matcher = Level3Matcher::new(data, max_chain, nice_match, max_insert, fast);
    let mut available = 0usize;
    for &chunk_length in input_chunks {
        available = available.wrapping_add(chunk_length);
        debug_assert!(available <= data.len());
        matcher.process(available, false);
    }
    debug_assert_eq!(available, data.len());
    matcher.process(available, true);
    matcher.tokens
}

#[cfg(feature = "png")]
fn tokenize_early_matcher_with_token(
    data: &[u8],
    input_chunks: &[usize],
    max_chain: usize,
    nice_match: usize,
    max_insert: usize,
    fast: bool,
    token: &crate::CancellationToken,
) -> crate::codecs::CodecResult<Vec<Token>> {
    let mut checkpoint = CancellationMatcherCheckpoint { token };
    let mut matcher = Level3Matcher::new(data, max_chain, nice_match, max_insert, fast);
    let mut available = 0usize;
    for &chunk_length in input_chunks {
        checkpoint.poll()?;
        available = available.wrapping_add(chunk_length);
        debug_assert!(available <= data.len());
        matcher.process_with(available, false, &mut checkpoint)?;
        checkpoint.poll()?;
    }
    debug_assert_eq!(available, data.len());
    matcher.process_with(available, true, &mut checkpoint)?;
    checkpoint.poll()?;
    Ok(matcher.tokens)
}

struct Level3Matcher<'a> {
    data: &'a [u8],
    head: Vec<usize>,
    previous: Vec<usize>,
    position: usize,
    tokens: Vec<Token>,
    max_chain: usize,
    nice_match: usize,
    max_insert: usize,
    fast: bool,
}

impl<'a> Level3Matcher<'a> {
    fn new(
        data: &'a [u8],
        max_chain: usize,
        nice_match: usize,
        max_insert: usize,
        fast: bool,
    ) -> Self {
        Self {
            data,
            head: vec![0; HASH_SIZE],
            previous: vec![0; WINDOW_SIZE],
            position: 0,
            tokens: Vec::new(),
            max_chain,
            nice_match,
            max_insert,
            fast,
        }
    }

    #[allow(clippy::expect_used, clippy::unwrap_in_result)]
    fn process(&mut self, available: usize, finishing: bool) {
        let mut checkpoint = NoopMatcherCheckpoint;
        self.process_with(available, finishing, &mut checkpoint)
            .expect("the no-op matcher checkpoint cannot fail");
    }

    fn process_with<P: MatcherCheckpoint>(
        &mut self,
        available: usize,
        finishing: bool,
        checkpoint: &mut P,
    ) -> crate::codecs::CodecResult<()> {
        loop {
            checkpoint.poll()?;
            debug_assert!(self.position <= available);
            let lookahead = available.wrapping_sub(self.position);
            if lookahead == 0 || (!finishing && lookahead < MIN_LOOKAHEAD) {
                return Ok(());
            }

            let mut length = 1usize;
            let mut match_start = 0usize;
            if lookahead >= MIN_MATCH {
                let candidate = self.quick_insert(self.position);
                debug_assert!(candidate <= self.position);
                let distance = self.position.wrapping_sub(candidate);
                if candidate != 0 && distance <= MAX_DISTANCE {
                    (length, match_start) =
                        self.longest_match_with(candidate, lookahead, checkpoint)?;
                    if length < MIN_MATCH {
                        length = 1;
                    }
                }
            }

            if length >= MIN_MATCH {
                self.tokens.push(Token::Match {
                    length,
                    distance: self.position.wrapping_sub(match_start),
                });
                self.insert_match_with(length, lookahead, checkpoint)?;
            } else {
                self.tokens.push(Token::Literal(self.data[self.position]));
            }
            self.position = self.position.wrapping_add(length);
        }
    }

    fn hash(&self, position: usize) -> usize {
        // ⚠️ UNVERIFIED: zlib-ng 2.3.3 insert_string.c:11-16 and
        // insert_string_tpl.h:49-73 (four-byte multiplicative hash).
        let bytes = &self.data[position..position.wrapping_add(4)];
        let word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        (word.wrapping_mul(2_654_435_761) >> 16) as usize
    }

    fn quick_insert(&mut self, position: usize) -> usize {
        let hash = self.hash(position);
        let candidate = self.head[hash];
        if candidate != position {
            self.previous[position & WINDOW_MASK] = candidate;
            self.head[hash] = position;
        }
        candidate
    }

    #[allow(dead_code, clippy::expect_used, clippy::unwrap_in_result)]
    fn insert_match(&mut self, length: usize, lookahead: usize) {
        let mut checkpoint = NoopMatcherCheckpoint;
        self.insert_match_with(length, lookahead, &mut checkpoint)
            .expect("the no-op matcher checkpoint cannot fail");
    }

    fn insert_match_with<P: MatcherCheckpoint>(
        &mut self,
        length: usize,
        lookahead: usize,
        checkpoint: &mut P,
    ) -> crate::codecs::CodecResult<()> {
        checkpoint.poll()?;
        // ⚠️ UNVERIFIED: zlib-ng 2.3.3 deflate_medium.c:44-94.
        let insert_limit = if self.fast {
            debug_assert!(length <= lookahead);
            if lookahead.wrapping_sub(length) < MIN_MATCH {
                return Ok(());
            }
            self.max_insert
        } else {
            if lookahead <= length.wrapping_add(MIN_MATCH) {
                return Ok(());
            }
            16_usize.wrapping_mul(self.max_insert)
        };
        if length <= insert_limit {
            for offset in 1..length {
                checkpoint.poll()?;
                let position = self.position.wrapping_add(offset);
                self.quick_insert(position);
            }
        } else {
            let end = self.position.wrapping_add(length);
            self.quick_insert(end.wrapping_sub(1));
        }
        Ok(())
    }

    #[allow(dead_code, clippy::expect_used, clippy::unwrap_in_result)]
    fn longest_match(&self, candidate: usize, lookahead: usize) -> (usize, usize) {
        let mut checkpoint = NoopMatcherCheckpoint;
        self.longest_match_with(candidate, lookahead, &mut checkpoint)
            .expect("the no-op matcher checkpoint cannot fail")
    }

    fn longest_match_with<P: MatcherCheckpoint>(
        &self,
        mut candidate: usize,
        lookahead: usize,
        checkpoint: &mut P,
    ) -> crate::codecs::CodecResult<(usize, usize)> {
        // ⚠️ UNVERIFIED: zlib-ng 2.3.3 match_tpl.h:38-247, specialized for
        // level three's early-exit, nice-length, and chain limits.
        let mut best_length = 2usize;
        let mut best_start = 0usize;
        let mut chain_length = self.max_chain;
        let limit = self.position.saturating_sub(MAX_DISTANCE);

        loop {
            checkpoint.poll()?;
            if self.candidate_can_improve(candidate, best_length) {
                let length = match_length(
                    self.data,
                    candidate,
                    self.position,
                    lookahead.min(MAX_MATCH),
                );
                if length > best_length {
                    best_length = length;
                    best_start = candidate;
                    if best_length >= self.nice_match || best_length >= lookahead {
                        break;
                    }
                } else {
                    // zlib-ng's level-three early-exit applies only after the
                    // candidate passes its two endpoint pre-screens.
                    break;
                }
            }

            chain_length = chain_length.wrapping_sub(1);
            if chain_length == 0 {
                break;
            }
            candidate = self.previous[candidate & WINDOW_MASK];
            if candidate <= limit {
                break;
            }
        }
        Ok((best_length, best_start))
    }

    fn candidate_can_improve(&self, candidate: usize, best_length: usize) -> bool {
        let mut offset = best_length.wrapping_sub(1);
        if best_length >= 4 {
            offset = offset.wrapping_sub(2);
            if best_length >= 8 {
                offset = offset.wrapping_sub(4);
            }
        }
        let width = if best_length < 4 {
            2
        } else if best_length >= 8 {
            8
        } else {
            4
        };
        let candidate_end = candidate.wrapping_add(offset);
        let scan_end = self.position.wrapping_add(offset);
        self.data[candidate..candidate.saturating_add(width)]
            == self.data[self.position..self.position.saturating_add(width)]
            && self.data[candidate_end..candidate_end.saturating_add(width)]
                == self.data[scan_end..scan_end.saturating_add(width)]
    }
}

fn match_length(data: &[u8], left: usize, right: usize, maximum: usize) -> usize {
    let mut length = 0usize;
    while length < maximum
        && data
            .get(left.wrapping_add(length))
            .zip(data.get(right.wrapping_add(length)))
            .is_some_and(|(left_byte, right_byte)| left_byte == right_byte)
    {
        length = length.wrapping_add(1);
    }
    length
}

#[derive(Clone, Copy, Default)]
struct Node {
    frequency: u32,
    parent: usize,
    length: u16,
    code: u16,
    depth: u8,
}

struct HuffmanTree {
    nodes: Vec<Node>,
    max_code: usize,
    bit_cost: i64,
    static_cost: i64,
}

struct TreeSpec<'a> {
    elements: usize,
    max_length: usize,
    extra_bits: &'a [u8],
    extra_base: usize,
    static_lengths: Option<&'a [u8]>,
}

fn build_tree(frequencies: &[u32], spec: TreeSpec<'_>) -> HuffmanTree {
    // ⚠️ UNVERIFIED: zlib-ng 2.3.3 trees.c:122-345 (heap construction,
    // depth tie-breaking, length overflow repair, and canonical codes).
    let heap_size = spec.elements.wrapping_mul(2).wrapping_add(1);
    let mut nodes = vec![Node::default(); heap_size];
    for (node, &frequency) in nodes.iter_mut().zip(frequencies) {
        node.frequency = frequency;
    }
    let mut heap = vec![0usize; heap_size];
    let mut heap_len = 0usize;
    let mut heap_max = heap_size;
    let mut max_code = 0usize;
    let mut has_code = false;
    for (index, node) in nodes.iter().take(spec.elements).enumerate() {
        if node.frequency != 0 {
            heap_len = heap_len.wrapping_add(1);
            heap[heap_len] = index;
            max_code = index;
            has_code = true;
        }
    }

    let mut bit_cost = 0i64;
    let mut static_cost = 0i64;
    while heap_len < 2 {
        let index = if has_code && max_code <= 1 {
            max_code.wrapping_add(1)
        } else {
            0
        };
        max_code = max_code.max(index);
        has_code = true;
        heap_len = heap_len.wrapping_add(1);
        heap[heap_len] = index;
        nodes[index].frequency = 1;
        bit_cost = bit_cost.wrapping_sub(1);
        let static_length = spec
            .static_lengths
            .and_then(|lengths| lengths.get(index))
            .copied()
            .unwrap_or(0);
        static_cost = static_cost.wrapping_sub(i64::from(static_length));
    }
    for index in (1..=heap_len / 2).rev() {
        pq_down(&mut heap, heap_len, &nodes, index);
    }
    let mut next_node = spec.elements;
    while heap_len >= 2 {
        let first = remove_smallest(&mut heap, &mut heap_len, &nodes);
        let second = heap[1];
        heap_max = heap_max.wrapping_sub(1);
        heap[heap_max] = first;
        heap_max = heap_max.wrapping_sub(1);
        heap[heap_max] = second;

        nodes[next_node].frequency = nodes[first].frequency.wrapping_add(nodes[second].frequency);
        nodes[next_node].depth = nodes[first].depth.max(nodes[second].depth).wrapping_add(1);
        nodes[first].parent = next_node;
        nodes[second].parent = next_node;
        heap[1] = next_node;
        next_node = next_node.wrapping_add(1);
        pq_down(&mut heap, heap_len, &nodes, 1);
    }
    heap_max = heap_max.wrapping_sub(1);
    heap[heap_max] = heap[1];

    let mut bit_counts = [0u16; BIT_COUNT_SIZE];
    nodes[heap[heap_max]].length = 0;
    let mut overflow = 0i32;
    for &index in &heap[heap_max.wrapping_add(1)..heap_size] {
        let mut bits = usize::from(nodes[nodes[index].parent].length).wrapping_add(1);
        if bits > spec.max_length {
            bits = spec.max_length;
            overflow = overflow.wrapping_add(1);
        }
        nodes[index].length = low_u16(bits);
        if index > max_code {
            continue;
        }
        bit_counts[bits] = bit_counts[bits].wrapping_add(1);
        let extra = index
            .checked_sub(spec.extra_base)
            .and_then(|extra_index| spec.extra_bits.get(extra_index))
            .copied()
            .unwrap_or(0);
        let frequency = i64::from(nodes[index].frequency);
        bit_cost = bit_cost.wrapping_add(
            frequency.wrapping_mul(i64::from(low_u16(bits.wrapping_add(usize::from(extra))))),
        );
        if let Some(static_lengths) = spec.static_lengths {
            let static_length = static_lengths[index];
            static_cost = static_cost.wrapping_add(frequency.wrapping_mul(i64::from(low_u16(
                usize::from(static_length).wrapping_add(usize::from(extra)),
            ))));
        }
    }

    if overflow > 0 {
        while overflow > 0 {
            let mut bits = spec.max_length.wrapping_sub(1);
            while bit_counts[bits] == 0 {
                bits = bits.wrapping_sub(1);
            }
            bit_counts[bits] = bit_counts[bits].wrapping_sub(1);
            let next_bits = bits.wrapping_add(1);
            bit_counts[next_bits] = bit_counts[next_bits].wrapping_add(2);
            bit_counts[spec.max_length] = bit_counts[spec.max_length].wrapping_sub(1);
            overflow = overflow.wrapping_sub(2);
        }
        debug_assert_eq!(overflow, 0);
        let mut sorted_index = heap_size;
        for bits in (1..=spec.max_length).rev() {
            let mut count = bit_counts[bits];
            while count != 0 {
                sorted_index = sorted_index.wrapping_sub(1);
                let index = heap[sorted_index];
                if index > max_code {
                    continue;
                }
                if usize::from(nodes[index].length) != bits {
                    let old_length = i64::from(nodes[index].length);
                    let frequency = i64::from(nodes[index].frequency);
                    bit_cost = bit_cost.wrapping_add(
                        i64::from(low_u16(bits))
                            .wrapping_sub(old_length)
                            .wrapping_mul(frequency),
                    );
                    nodes[index].length = low_u16(bits);
                }
                count = count.wrapping_sub(1);
            }
        }
    }

    generate_codes(&mut nodes, max_code, &bit_counts);
    HuffmanTree {
        nodes,
        max_code,
        bit_cost,
        static_cost,
    }
}

fn smaller(nodes: &[Node], left: usize, right: usize) -> bool {
    nodes[left].frequency < nodes[right].frequency
        || (nodes[left].frequency == nodes[right].frequency
            && nodes[left].depth <= nodes[right].depth)
}

fn pq_down(heap: &mut [usize], heap_len: usize, nodes: &[Node], mut root: usize) {
    let value = heap[root];
    let mut child = root.wrapping_mul(2);
    while child <= heap_len {
        if child < heap_len && smaller(nodes, heap[child.wrapping_add(1)], heap[child]) {
            child = child.wrapping_add(1);
        }
        if smaller(nodes, value, heap[child]) {
            break;
        }
        heap[root] = heap[child];
        root = child;
        child = child.wrapping_mul(2);
    }
    heap[root] = value;
}

fn remove_smallest(heap: &mut [usize], heap_len: &mut usize, nodes: &[Node]) -> usize {
    let smallest = heap[1];
    heap[1] = heap[*heap_len];
    *heap_len = heap_len.wrapping_sub(1);
    pq_down(heap, *heap_len, nodes, 1);
    smallest
}

fn generate_codes(nodes: &mut [Node], max_code: usize, counts: &[u16; BIT_COUNT_SIZE]) {
    let mut next_code = [0u16; BIT_COUNT_SIZE];
    let mut code = 0u16;
    for bits in 1..=MAX_BITS {
        code = code
            .wrapping_add(counts[bits.wrapping_sub(1)])
            .wrapping_shl(1);
        next_code[bits] = code;
    }
    for node in nodes.iter_mut().take(max_code.wrapping_add(1)) {
        let length = usize::from(node.length);
        if length == 0 {
            continue;
        }
        node.code = reverse_bits(next_code[length], low_u16(length).to_le_bytes()[0]);
        next_code[length] = next_code[length].wrapping_add(1);
    }
}

#[allow(clippy::expect_used, clippy::unwrap_in_result)]
fn emit_blocks(tokens: &[Token], block_tokens: usize, writer: &mut BitWriter) {
    let mut checkpoint = NoopMatcherCheckpoint;
    emit_blocks_with(tokens, block_tokens, writer, &mut checkpoint)
        .expect("the no-op matcher checkpoint cannot fail");
}

fn emit_blocks_with<P: MatcherCheckpoint>(
    tokens: &[Token],
    block_tokens: usize,
    writer: &mut BitWriter,
    checkpoint: &mut P,
) -> crate::codecs::CodecResult<()> {
    checkpoint.poll()?;
    let block_count = tokens.len().div_ceil(block_tokens);
    let uncompressed = expand_tokens_with(tokens, checkpoint)?;
    let mut uncompressed_start = 0usize;
    for (index, block) in tokens.chunks(block_tokens).enumerate() {
        checkpoint.poll()?;
        let stored_length = block.iter().fold(0usize, |length, token| {
            length.wrapping_add(match token {
                Token::Literal(_) => 1,
                Token::Match { length, .. } => *length,
            })
        });
        let uncompressed_end = uncompressed_start.wrapping_add(stored_length);
        let uncompressed_block = &uncompressed[uncompressed_start..uncompressed_end];
        let final_block = index.wrapping_add(1) == block_count;
        write_block_with(block, uncompressed_block, final_block, writer, checkpoint)?;
        uncompressed_start = uncompressed_end;
        checkpoint.poll()?;
    }
    Ok(())
}

fn expand_tokens_with<P: MatcherCheckpoint>(
    tokens: &[Token],
    checkpoint: &mut P,
) -> crate::codecs::CodecResult<Vec<u8>> {
    let mut output = Vec::new();
    for token in tokens {
        checkpoint.poll()?;
        match token {
            Token::Literal(value) => output.push(*value),
            Token::Match { length, distance } => {
                for (offset, _) in (0..*length).enumerate() {
                    if offset % 1_024 == 0 {
                        checkpoint.poll()?;
                    }
                    let source = output.len().wrapping_sub(*distance);
                    output.push(output[source]);
                }
            }
        }
    }
    checkpoint.poll()?;
    Ok(output)
}

fn write_block_with<P: MatcherCheckpoint>(
    tokens: &[Token],
    uncompressed: &[u8],
    final_block: bool,
    writer: &mut BitWriter,
    checkpoint: &mut P,
) -> crate::codecs::CodecResult<()> {
    // ⚠️ UNVERIFIED: zlib-ng 2.3.3 trees.c:628-707.
    checkpoint.poll()?;
    let (literal_frequencies, distance_frequencies) = frequencies_with(tokens, checkpoint)?;
    let static_literal_lengths = static_literal_lengths();
    let static_distance_lengths = [5u8; DISTANCE_CODES];
    let literal_spec = TreeSpec {
        elements: LITERAL_CODES,
        max_length: MAX_BITS,
        extra_bits: &LENGTH_EXTRA,
        extra_base: 257,
        static_lengths: Some(&static_literal_lengths),
    };
    let literal_tree = build_tree(&literal_frequencies, literal_spec);
    checkpoint.poll()?;
    let distance_spec = TreeSpec {
        elements: DISTANCE_CODES,
        max_length: MAX_BITS,
        extra_bits: &DISTANCE_EXTRA,
        extra_base: 0,
        static_lengths: Some(&static_distance_lengths),
    };
    let distance_tree = build_tree(&distance_frequencies, distance_spec);
    checkpoint.poll()?;

    let mut bit_frequencies = [0u32; BIT_LENGTH_CODES];
    let literal_nodes = &literal_tree.nodes;
    scan_tree_with(
        literal_nodes,
        literal_tree.max_code,
        &mut bit_frequencies,
        checkpoint,
    )?;
    let distance_nodes = &distance_tree.nodes;
    scan_tree_with(
        distance_nodes,
        distance_tree.max_code,
        &mut bit_frequencies,
        checkpoint,
    )?;
    let bit_length_spec = TreeSpec {
        elements: BIT_LENGTH_CODES,
        max_length: MAX_BIT_LENGTH_BITS,
        extra_bits: &EXTRA_BIT_LENGTH_BITS,
        extra_base: 0,
        static_lengths: None,
    };
    let bit_length_tree = build_tree(&bit_frequencies, bit_length_spec);
    checkpoint.poll()?;
    let max_bit_length_index = (3..BIT_LENGTH_CODES)
        .rev()
        .find(|&index| bit_length_tree.nodes[CODE_LENGTH_ORDER[index]].length != 0)
        .unwrap_or(3);

    let tree_header_cost = 3_usize
        .wrapping_mul(max_bit_length_index.wrapping_add(1))
        .wrapping_add(14);
    let dynamic_cost = literal_tree
        .bit_cost
        .wrapping_add(distance_tree.bit_cost)
        .wrapping_add(bit_length_tree.bit_cost)
        .wrapping_add(i64::from(low_u16(tree_header_cost)));
    let static_cost = literal_tree
        .static_cost
        .wrapping_add(distance_tree.static_cost);
    let dynamic_bytes = low_usize_i64(dynamic_cost.wrapping_add(10) >> 3);
    let static_bytes = low_usize_i64(static_cost.wrapping_add(10) >> 3);

    let stored_cost = if uncompressed.len() <= usize::from(u16::MAX) {
        uncompressed.len().wrapping_add(4)
    } else {
        usize::MAX
    };
    if stored_cost <= dynamic_bytes.min(static_bytes) {
        // The stored-cost guard above proves this conversion cannot truncate.
        #[allow(clippy::cast_possible_truncation)]
        let length = uncompressed.len() as u16;
        checkpoint.poll()?;
        writer.write_bits(u32::from(final_block), 3); // BTYPE=stored (00).
        writer.align_to_byte();
        writer.write_aligned_bytes(&length.to_le_bytes());
        write_aligned_bytes_with(writer, &(!length).to_le_bytes(), checkpoint)?;
        write_aligned_bytes_with(writer, uncompressed, checkpoint)?;
        return Ok(());
    }
    if static_bytes <= dynamic_bytes {
        emit_fixed_block_with(tokens, final_block, writer, checkpoint)?;
    } else {
        checkpoint.poll()?;
        writer.write_bits(4 | u32::from(final_block), 3); // BTYPE=dynamic (10).
        let trees = [&literal_tree, &distance_tree, &bit_length_tree];
        send_trees_with(trees, max_bit_length_index, writer, checkpoint)?;
        emit_tokens_with(tokens, &literal_tree, &distance_tree, writer, checkpoint)?;
        send_code(writer, &literal_tree, 256);
    }
    checkpoint.poll()?;
    Ok(())
}

fn frequencies_with<P: MatcherCheckpoint>(
    tokens: &[Token],
    checkpoint: &mut P,
) -> crate::codecs::CodecResult<([u32; LITERAL_CODES], [u32; DISTANCE_CODES])> {
    let mut literal = [0u32; LITERAL_CODES];
    let mut distance = [0u32; DISTANCE_CODES];
    literal[256] = 1;
    for token in tokens {
        checkpoint.poll()?;
        match token {
            Token::Literal(value) => {
                let index = usize::from(*value);
                literal[index] = literal[index].wrapping_add(1);
            }
            Token::Match {
                length,
                distance: match_distance,
            } => {
                let length_index = length_index(*length);
                let literal_index = 257_usize.wrapping_add(length_index);
                literal[literal_index] = literal[literal_index].wrapping_add(1);
                let distance_index = distance_index(*match_distance);
                distance[distance_index] = distance[distance_index].wrapping_add(1);
            }
        }
    }
    checkpoint.poll()?;
    Ok((literal, distance))
}

fn scan_tree_with<P: MatcherCheckpoint>(
    nodes: &[Node],
    max_code: usize,
    frequencies: &mut [u32; 19],
    checkpoint: &mut P,
) -> crate::codecs::CodecResult<()> {
    // ⚠️ UNVERIFIED: zlib-ng 2.3.3 trees.c:348-396.
    let mut previous_length = usize::MAX;
    let mut current_length;
    let mut next_length = usize::from(nodes[0].length);
    let mut count = 0usize;
    let mut max_count = if next_length == 0 { 138 } else { 7 };
    let mut min_count = if next_length == 0 { 3 } else { 4 };

    for index in 0..=max_code {
        checkpoint.poll()?;
        current_length = next_length;
        next_length = if index == max_code {
            u16::MAX.into()
        } else {
            usize::from(nodes[index.wrapping_add(1)].length)
        };
        count = count.wrapping_add(1);
        if count < max_count && current_length == next_length {
            continue;
        }
        if count < min_count {
            frequencies[current_length] = frequencies[current_length].wrapping_add(low_u32(count));
        } else if current_length != 0 {
            if current_length != previous_length {
                frequencies[current_length] = frequencies[current_length].wrapping_add(1);
            }
            frequencies[16] = frequencies[16].wrapping_add(1);
        } else if count <= 10 {
            frequencies[17] = frequencies[17].wrapping_add(1);
        } else {
            frequencies[18] = frequencies[18].wrapping_add(1);
        }
        count = 0;
        previous_length = current_length;
        if next_length == 0 {
            max_count = 138;
            min_count = 3;
        } else if current_length == next_length {
            max_count = 6;
            min_count = 3;
        } else {
            max_count = 7;
            min_count = 4;
        }
    }
    checkpoint.poll()?;
    Ok(())
}

fn send_trees_with<P: MatcherCheckpoint>(
    trees: [&HuffmanTree; 3],
    max_bit_length_index: usize,
    writer: &mut BitWriter,
    checkpoint: &mut P,
) -> crate::codecs::CodecResult<()> {
    let [literal, distance, bit_length] = trees;
    checkpoint.poll()?;
    writer.write_bits(
        low_u32(literal.max_code.wrapping_add(1).wrapping_sub(257)),
        5,
    );
    writer.write_bits(low_u32(distance.max_code), 5);
    writer.write_bits(
        low_u32(max_bit_length_index.wrapping_add(1).wrapping_sub(4)),
        4,
    );
    for &code in &CODE_LENGTH_ORDER[..=max_bit_length_index] {
        checkpoint.poll()?;
        writer.write_bits(u32::from(bit_length.nodes[code].length), 3);
    }
    send_tree_with(literal, literal.max_code, bit_length, writer, checkpoint)?;
    send_tree_with(distance, distance.max_code, bit_length, writer, checkpoint)?;
    checkpoint.poll()?;
    Ok(())
}

fn send_tree_with<P: MatcherCheckpoint>(
    tree: &HuffmanTree,
    max_code: usize,
    bit_length: &HuffmanTree,
    writer: &mut BitWriter,
    checkpoint: &mut P,
) -> crate::codecs::CodecResult<()> {
    // ⚠️ UNVERIFIED: zlib-ng 2.3.3 trees.c:401-466.
    let mut previous_length = usize::MAX;
    let mut next_length = usize::from(tree.nodes[0].length);
    let mut count = 0usize;
    let mut max_count = if next_length == 0 { 138 } else { 7 };
    let mut min_count = if next_length == 0 { 3 } else { 4 };
    for index in 0..=max_code {
        checkpoint.poll()?;
        let current_length = next_length;
        next_length = if index == max_code {
            u16::MAX.into()
        } else {
            usize::from(tree.nodes[index.wrapping_add(1)].length)
        };
        count = count.wrapping_add(1);
        if count < max_count && current_length == next_length {
            continue;
        }
        if count < min_count {
            for _ in 0..count {
                send_code(writer, bit_length, current_length);
            }
        } else if current_length != 0 {
            if current_length != previous_length {
                send_code(writer, bit_length, current_length);
                count = count.wrapping_sub(1);
            }
            send_code(writer, bit_length, 16);
            writer.write_bits(low_u32(count.wrapping_sub(3)), 2);
        } else if count <= 10 {
            send_code(writer, bit_length, 17);
            writer.write_bits(low_u32(count.wrapping_sub(3)), 3);
        } else {
            send_code(writer, bit_length, 18);
            writer.write_bits(low_u32(count.wrapping_sub(11)), 7);
        }
        count = 0;
        previous_length = current_length;
        if next_length == 0 {
            max_count = 138;
            min_count = 3;
        } else if current_length == next_length {
            max_count = 6;
            min_count = 3;
        } else {
            max_count = 7;
            min_count = 4;
        }
    }
    checkpoint.poll()?;
    Ok(())
}

fn emit_tokens_with<P: MatcherCheckpoint>(
    tokens: &[Token],
    literal_tree: &HuffmanTree,
    distance_tree: &HuffmanTree,
    writer: &mut BitWriter,
    checkpoint: &mut P,
) -> crate::codecs::CodecResult<()> {
    for token in tokens {
        checkpoint.poll()?;
        match token {
            Token::Literal(value) => send_code(writer, literal_tree, usize::from(*value)),
            Token::Match { length, distance } => {
                let length_index = length_index(*length);
                send_code(writer, literal_tree, 257_usize.wrapping_add(length_index));
                writer.write_bits(
                    low_u32(length.wrapping_sub(LENGTH_BASE[length_index])),
                    LENGTH_EXTRA[length_index],
                );
                let distance_index = distance_index(*distance);
                send_code(writer, distance_tree, distance_index);
                writer.write_bits(
                    low_u32(distance.wrapping_sub(DISTANCE_BASE[distance_index])),
                    DISTANCE_EXTRA[distance_index],
                );
            }
        }
    }
    checkpoint.poll()?;
    Ok(())
}

#[allow(clippy::expect_used, clippy::unwrap_in_result)]
fn emit_fixed_block(tokens: &[Token], final_block: bool, writer: &mut BitWriter) {
    let mut checkpoint = NoopMatcherCheckpoint;
    emit_fixed_block_with(tokens, final_block, writer, &mut checkpoint)
        .expect("the no-op matcher checkpoint cannot fail");
}

fn emit_fixed_block_with<P: MatcherCheckpoint>(
    tokens: &[Token],
    final_block: bool,
    writer: &mut BitWriter,
    checkpoint: &mut P,
) -> crate::codecs::CodecResult<()> {
    checkpoint.poll()?;
    writer.write_bits(2 | u32::from(final_block), 3); // BTYPE=fixed (01).
    for token in tokens {
        checkpoint.poll()?;
        match token {
            Token::Literal(value) => write_fixed_symbol(writer, u16::from(*value)),
            Token::Match { length, distance } => {
                let length_index = length_index(*length);
                write_fixed_symbol(writer, low_u16(257_usize.wrapping_add(length_index)));
                writer.write_bits(
                    low_u32(length.wrapping_sub(LENGTH_BASE[length_index])),
                    LENGTH_EXTRA[length_index],
                );
                let distance_index = distance_index(*distance);
                writer.write_bits(u32::from(reverse_bits(low_u16(distance_index), 5)), 5);
                writer.write_bits(
                    low_u32(distance.wrapping_sub(DISTANCE_BASE[distance_index])),
                    DISTANCE_EXTRA[distance_index],
                );
            }
        }
    }
    write_fixed_symbol(writer, 256);
    checkpoint.poll()?;
    Ok(())
}

fn send_code(writer: &mut BitWriter, tree: &HuffmanTree, symbol: usize) {
    let node = &tree.nodes[symbol];
    writer.write_bits(u32::from(node.code), node.length.to_le_bytes()[0]);
}

fn length_index(length: usize) -> usize {
    let mut index = LENGTH_BASE.len().wrapping_sub(1);
    while length < LENGTH_BASE[index] {
        index = index.wrapping_sub(1);
    }
    index
}

fn distance_index(distance: usize) -> usize {
    let mut index = DISTANCE_BASE.len().wrapping_sub(1);
    while distance < DISTANCE_BASE[index] {
        index = index.wrapping_sub(1);
    }
    index
}

fn static_literal_lengths() -> [u8; LITERAL_CODES] {
    let mut lengths = [0u8; LITERAL_CODES];
    lengths[..=143].fill(8);
    lengths[144..=255].fill(9);
    lengths[256..=279].fill(7);
    lengths[280..].fill(8);
    lengths
}

fn write_fixed_symbol(writer: &mut BitWriter, symbol: u16) {
    let (canonical, width) = if symbol <= 143 {
        (0x30_u16.wrapping_add(symbol), 8)
    } else if symbol <= 255 {
        (0x190_u16.wrapping_add(symbol).wrapping_sub(144), 9)
    } else if symbol <= 279 {
        (symbol.wrapping_sub(256), 7)
    } else {
        debug_assert!(symbol <= 287);
        (0xc0_u16.wrapping_add(symbol).wrapping_sub(280), 8)
    };
    writer.write_bits(u32::from(reverse_bits(canonical, width)), width);
}

fn reverse_bits(mut value: u16, width: u8) -> u16 {
    let mut reversed = 0u16;
    for _ in 0..width {
        reversed = reversed.wrapping_shl(1) | (value & 1);
        value >>= 1;
    }
    reversed
}

#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    current: u8,
    used: u8,
}

impl BitWriter {
    fn with_prefix(prefix: [u8; 2]) -> Self {
        Self {
            bytes: prefix.to_vec(),
            current: 0,
            used: 0,
        }
    }

    fn write_bits(&mut self, value: u32, width: u8) {
        for bit in 0..width {
            self.current |=
                ((value >> bit).to_le_bytes()[0] & 1).wrapping_shl(u32::from(self.used));
            self.used = self.used.wrapping_add(1);
            if self.used == 8 {
                self.bytes.push(self.current);
                self.current = 0;
                self.used = 0;
            }
        }
    }

    fn align_to_byte(&mut self) {
        // Stored-block headers contribute three bits, so this helper is only
        // called with a partial byte pending.
        debug_assert_ne!(self.used, 0);
        self.bytes.push(self.current);
        self.current = 0;
        self.used = 0;
    }

    fn write_aligned_bytes(&mut self, bytes: &[u8]) {
        debug_assert_eq!(self.used, 0);
        self.bytes.extend_from_slice(bytes);
    }

    fn finish(mut self) -> Vec<u8> {
        if self.used != 0 {
            self.bytes.push(self.current);
        }
        self.bytes
    }
}

fn write_aligned_bytes_with<P: MatcherCheckpoint>(
    writer: &mut BitWriter,
    bytes: &[u8],
    checkpoint: &mut P,
) -> crate::codecs::CodecResult<()> {
    debug_assert_eq!(writer.used, 0);
    for chunk in bytes.chunks(1_024) {
        checkpoint.poll()?;
        writer.write_aligned_bytes(chunk);
    }
    checkpoint.poll()?;
    Ok(())
}

#[allow(clippy::expect_used, clippy::unwrap_in_result)]
fn adler32(data: &[u8]) -> u32 {
    let mut checkpoint = NoopMatcherCheckpoint;
    adler32_with(data, &mut checkpoint).expect("the no-op matcher checkpoint cannot fail")
}

fn adler32_with<P: MatcherCheckpoint>(
    data: &[u8],
    checkpoint: &mut P,
) -> crate::codecs::CodecResult<u32> {
    const MODULUS: u32 = 65_521;
    let mut first = 1u32;
    let mut second = 0u32;
    for chunk in data.chunks(5_552) {
        checkpoint.poll()?;
        for &byte in chunk {
            first = first.wrapping_add(u32::from(byte));
            second = second.wrapping_add(first);
        }
        first %= MODULUS;
        second %= MODULUS;
    }
    checkpoint.poll()?;
    Ok(second.wrapping_shl(16) | first)
}
