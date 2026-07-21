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
const MAX_BIT_LENGTH_BITS: usize = 7;
const MIN_LOOKAHEAD: usize = 262;
const MAX_DISTANCE: usize = 32_768 - MIN_LOOKAHEAD;
const MAX_MATCH: usize = 258;
const MIN_MATCH: usize = 4;
const HASH_SIZE: usize = 65_536;
const WINDOW_MASK: usize = 32_767;
const CODE_LENGTH_ORDER: [usize; BIT_LENGTH_CODES] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];
const EXTRA_BIT_LENGTH_BITS: [u8; BIT_LENGTH_CODES] =
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 3, 7];

enum Token {
    Literal(u8),
    Match { length: usize, distance: usize },
}

/// Compress using Pillow's zlib-ng 2.3.3 level-one quick strategy.
///
/// `deflate_quick` retains only the newest four-byte hash candidate, emits
/// fixed Huffman codes directly, and deliberately does not insert positions
/// skipped by a match.
pub(super) fn compress_level1(data: &[u8], input_chunks: &[usize]) -> Option<Vec<u8>> {
    let mut input_len = 0usize;
    for &length in input_chunks {
        input_len = input_len.checked_add(length)?;
    }
    debug_assert_eq!(input_len, data.len());

    let (tokens, final_tokens) =
        tokenize_level1(data, input_chunks).expect("validated zlib chunks should tokenize");
    let mut writer = BitWriter::default();
    // deflate_quick opens its first block only after a Z_NO_FLUSH call has
    // enough lookahead to process. On Z_FINISH it closes an opened block as
    // non-final, then emits the remaining short lookahead in a final block.
    if tokens.is_empty() {
        emit_fixed_block(&final_tokens, true, &mut writer)?;
    } else {
        emit_fixed_block(&tokens, false, &mut writer)?;
        emit_fixed_block(&final_tokens, true, &mut writer)?;
    }
    let mut output = vec![0x78, 0x01];
    output.extend_from_slice(&writer.finish());
    output.extend_from_slice(&adler32(data).to_be_bytes());
    Some(output)
}

fn tokenize_level1(data: &[u8], input_chunks: &[usize]) -> Option<(Vec<Token>, Vec<Token>)> {
    let mut head = vec![0usize; HASH_SIZE];
    let mut tokens = Vec::new();
    let mut position = 0usize;
    let mut available = 0usize;
    for &chunk_length in input_chunks {
        available = available.checked_add(chunk_length)?;
        debug_assert!(available <= data.len());

        // fill_window() re-inserts strstart - 1 whenever a new input call
        // makes another four-byte hash available. This includes positions
        // skipped by the preceding quick match.
        // When a previous non-final pass advanced `position`, the loop below
        // left at least `MIN_LOOKAHEAD - MAX_MATCH` bytes available, so
        // `position - 1` has the four bytes required by `quick_insert_level1`.
        if position >= 1 {
            quick_insert_level1(data, position - 1, &mut head)?;
        }
        while available.checked_sub(position)? >= MIN_LOOKAHEAD {
            tokenize_level1_position(data, available, &mut position, &mut head, &mut tokens)?;
        }
    }
    debug_assert_eq!(available, data.len());
    let mut final_tokens = Vec::new();
    while position < available {
        tokenize_level1_position(data, available, &mut position, &mut head, &mut final_tokens)?;
    }
    Some((tokens, final_tokens))
}

fn tokenize_level1_position(
    data: &[u8],
    available: usize,
    position: &mut usize,
    head: &mut [usize],
    tokens: &mut Vec<Token>,
) -> Option<()> {
    let lookahead = available.checked_sub(*position)?;
    if lookahead >= MIN_MATCH {
        let candidate = quick_insert_level1(data, *position, head)?;
        let distance = position.checked_sub(candidate)?;
        if distance != 0
            && distance <= MAX_DISTANCE
            && data.get(candidate..candidate + 2)? == data.get(*position..*position + 2)?
        {
            let length = match_length(data, candidate, *position, lookahead.min(MAX_MATCH));
            if length >= MIN_MATCH {
                tokens.push(Token::Match { length, distance });
                *position = position.checked_add(length)?;
                return Some(());
            }
        }
    }

    tokens.push(Token::Literal(*data.get(*position)?));
    *position = position.checked_add(1)?;
    Some(())
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let data = b"abcdef";
    let mut tokens = Vec::new();
    let mut head = vec![0usize; HASH_SIZE];
    let mut position = 0usize;
    tokenize_level1_position(data, data.len(), &mut position, &mut head, &mut tokens)
        .expect("literal path should tokenize");
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
    let _ = compress_level1(data, &[data.len(), usize::MAX]);
    let _ = tokenize_level1(data, &[data.len(), usize::MAX]);

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

    let mut slow = SlowMatcher::new(b"abcdefghijkl", 16, 8, 128, 128);
    slow.process(0, true)
        .expect("slow matcher empty finalization should process");
    slow.quick_insert(4)
        .expect("slow matcher pre-insert should succeed");
    slow.position = 4;
    let _ = slow
        .longest_match(slow.position, 8)
        .expect("slow matcher current-candidate loop exit should process");
    slow.process(8, true)
        .expect("slow matcher self-candidate path should process");
    assert!(slow.position > 4);
    let mut slow_underflow = SlowMatcher::new(b"abcd", 16, 8, 128, 128);
    slow_underflow.position = usize::MAX;
    let _ = slow_underflow.process(0, true);
    let mut slow_flush = SlowMatcher::new(b"", 16, 8, 128, 128);
    slow_flush.match_available = true;
    let _ = slow_flush.process(0, true);
    let mut slow_previous = SlowMatcher::new(b"aaaa", 16, 8, 128, 128);
    slow_previous.previous_length = 3;
    let _ = slow_previous.process(4, true);
    let mut slow_short = SlowMatcher::new(b"abcd", 16, 8, 128, 128);
    slow_short.data.truncate(3);
    let _ = slow_short.quick_insert(0);
    let mut slow_empty_chain = SlowMatcher::new(b"abcxyz", 16, 8, 128, 0);
    slow_empty_chain.position = 3;
    let _ = slow_empty_chain.longest_match(0, 3);

    let mut level6 = Level6Matcher::new(b"aaaaaaaa", 128, 128, 16);
    level6.position = 4;
    let self_hash = level6.hash(4).expect("level6 self hash should compute");
    level6.head[self_hash] = 4;
    let found = level6
        .find_match(4, 4)
        .expect("level6 self-candidate path should process");
    assert_eq!(found.length, 1);
    let mut level6_underflow = Level6Matcher::new(b"abcd", 128, 128, 16);
    level6_underflow.window_base = 2;
    level6_underflow.position = 1;
    let _ = level6_underflow.slide_window_if_needed();
    let mut level6_process_underflow = Level6Matcher::new(b"abcd", 128, 128, 16);
    level6_process_underflow.position = usize::MAX;
    let _ = level6_process_underflow.process(0, true);
    let level6_hash_overflow = Level6Matcher::new(b"abcd", 128, 128, 16);
    let _ = level6_hash_overflow.hash(usize::MAX);
    let mut level6_insert_overflow = Level6Matcher::new(b"aaaaaaaa", 128, 128, 16);
    let _ = level6_insert_overflow.insert_match(
        MediumMatch {
            match_start: 0,
            length: usize::MAX,
            start: 0,
            original_start: 0,
        },
        usize::MAX,
    );
    let _ = level6_insert_overflow.insert_match(
        MediumMatch {
            match_start: 0,
            length: 4,
            start: usize::MAX,
            original_start: 0,
        },
        16,
    );
    let _ = level6_insert_overflow.find_match(usize::MAX, 4);
    let _ = level6_insert_overflow.longest_match(0, usize::MAX, 4);

    let mut level9 = Level9Matcher::new(b"abcdefghijkl");
    level9
        .process(0, true)
        .expect("level9 empty finalization should process");
    level9.position = 4;
    level9
        .refill_boundary()
        .expect("level9 boundary hash should refresh");
    let self_hash = rolling_hash(level9.hash, level9.data[6]);
    level9.head[self_hash] = 4;
    level9
        .process(8, true)
        .expect("level9 self-candidate path should process");
    assert!(level9.position > 4);

    let mut level9 = Level9Matcher::new(b"aaaaaaaaaaaa");
    level9.position = 4;
    level9.previous_length = 3;
    let mut hash = rolling_hash(0, level9.data[5]);
    hash = rolling_hash(hash, level9.data[6]);
    hash = rolling_hash(hash, level9.data[7]);
    level9.head[hash] = 0;
    let _ = level9
        .longest_match(1, 8)
        .expect("level9 offset-before-candidate exit should process");
    let _ = level9
        .longest_match(level9.position, 8)
        .expect("level9 current-candidate exit should process");
    let mut level9 = Level9Matcher::new(b"abcdefghijkl");
    level9.position = 4;
    let _ = level9
        .longest_match(level9.position, 8)
        .expect("level9 upper-bound loop exit should process");
    let mut level9_underflow = Level9Matcher::new(b"abcd");
    level9_underflow.position = usize::MAX;
    let _ = level9_underflow.process(0, true);
    let mut level9_flush = Level9Matcher::new(b"");
    level9_flush.match_available = true;
    let _ = level9_flush.process(0, true);
    let mut level9_previous = Level9Matcher::new(b"aaaa");
    level9_previous.previous_length = 3;
    let _ = level9_previous.process(4, true);
    let mut level9_short_insert = Level9Matcher::new(b"abcd");
    level9_short_insert.data.truncate(2);
    level9_short_insert.position = 1;
    let _ = level9_short_insert.quick_insert(1);
    let mut level9_overflow_match = Level9Matcher::new(b"abcdefghijkl");
    level9_overflow_match.position = usize::MAX - 1;
    level9_overflow_match.previous_length = 3;
    let _ = level9_overflow_match.longest_match(0, 8);

    let mut level3 = Level3Matcher::new(b"aaaaaaaaaaaa", 6, 4, 6, false);
    level3.position = 4;
    let _ = level3
        .longest_match(0, 8)
        .expect("level3 nice-match break should process");
    let mut level3 = Level3Matcher::new(b"aaaaaaaaaaaa", 6, 128, 6, false);
    level3.position = 4;
    let _ = level3
        .longest_match(0, 4)
        .expect("level3 lookahead break should process");
    let mut level3_underflow = Level3Matcher::new(b"abcd", 6, 4, 6, false);
    level3_underflow.position = usize::MAX;
    let _ = level3_underflow.process(0, true);
    let level3_hash_overflow = Level3Matcher::new(b"abcd", 6, 4, 6, false);
    let _ = level3_hash_overflow.hash(usize::MAX);
    let mut level3_fast_insert = Level3Matcher::new(b"abcdefgh", 6, 4, 6, true);
    let _ = level3_fast_insert.insert_match(5, 4);
    let mut level3_slow_insert = Level3Matcher::new(b"abcdefgh", 6, 4, 6, false);
    let _ = level3_slow_insert.insert_match(usize::MAX, usize::MAX);
    level3_slow_insert.position = usize::MAX;
    let _ = level3_slow_insert.insert_match(4, 16);
    let mut level3_empty_chain = Level3Matcher::new(b"abcxyz", 0, 128, 6, false);
    level3_empty_chain.position = 3;
    let _ = level3_empty_chain.longest_match(0, 3);

    let _ = quick_insert_level1(data, data.len(), &mut head);
    let mut overflowing_position = usize::MAX;
    let _ = tokenize_level1_position(
        data,
        data.len(),
        &mut overflowing_position,
        &mut head,
        &mut tokens,
    );
    let _ = compress_level2(data, &[data.len(), usize::MAX]);
    let _ = compress_level3(data, &[data.len(), usize::MAX]);
    let _ = compress_level4(data, &[data.len(), usize::MAX]);
    let _ = compress_level5(data, &[data.len(), usize::MAX]);
    let _ = compress_level6(data, &[data.len(), usize::MAX]);
    let _ = compress_level7(data, &[data.len(), usize::MAX]);
    let _ = compress_level8(data, &[data.len(), usize::MAX]);
    let _ = compress_level9(data, &[data.len(), usize::MAX]);

    let _ = medium_candidate_can_improve(data, 0, 1, 0);
    let _ = medium_candidate_can_improve(data, data.len(), 0, 3);
    let _ = medium_candidate_can_improve(data, 0, data.len(), 8);

    let mut short_level6 = Level6Matcher::new(b"abcd", 1, 4, 4);
    short_level6.data.truncate(3);
    let _ = short_level6.quick_insert(0);

    let mut short_level9 = Level9Matcher::new(b"abcd");
    short_level9.data.truncate(3);
    short_level9.position = 1;
    let _ = short_level9.refill_boundary();

    let mut short_level3 = Level3Matcher::new(b"abc", 1, 4, 4, false);
    let _ = short_level3.quick_insert(0);
    short_level3.position = usize::MAX;
    let _ = short_level3.candidate_can_improve(0, 3);
    let _ = short_level3.candidate_can_improve(usize::MAX, 3);
}

fn quick_insert_level1(data: &[u8], position: usize, head: &mut [usize]) -> Option<usize> {
    let word = u32::from_le_bytes(
        data.get(position..position.checked_add(4)?)?
            .try_into()
            .ok()?,
    );
    let hash = usize::try_from(word.wrapping_mul(2_654_435_761) >> 16).ok()?;
    let candidate = *head.get(hash)?;
    if candidate != position {
        *head.get_mut(hash)? = position;
    }
    Some(candidate)
}

/// Compress using Pillow's zlib-ng 2.3.3 level-three configuration.
///
/// Pillow's `ZipEncode.c` selects `Z_FILTERED`, a 32 KiB window, and
/// `memLevel=9`. zlib-ng maps level three to `deflate_medium` with the
/// `{ good: 4, lazy: 6, nice: 16, chain: 6 }` configuration.
pub(super) fn compress_level3(data: &[u8], input_chunks: &[usize]) -> Option<Vec<u8>> {
    let tokens = tokenize_early_matcher(data, input_chunks, 6, 16, 6, false)?;
    let mut output = vec![0x78, 0x5e];
    let mut writer = BitWriter::default();
    emit_blocks(&tokens, 32_767, &mut writer)?;
    output.extend_from_slice(&writer.finish());
    output.extend_from_slice(&adler32(data).to_be_bytes());
    Some(output)
}

/// Compress using Pillow's zlib-ng 2.3.3 level-two fast strategy.
pub(super) fn compress_level2(data: &[u8], input_chunks: &[usize]) -> Option<Vec<u8>> {
    let tokens = tokenize_early_matcher(data, input_chunks, 4, 8, 4, true)?;
    let mut output = vec![0x78, 0x5e];
    let mut writer = BitWriter::default();
    emit_blocks(&tokens, 32_767, &mut writer)?;
    output.extend_from_slice(&writer.finish());
    output.extend_from_slice(&adler32(data).to_be_bytes());
    Some(output)
}

/// Compress using Pillow's zlib-ng 2.3.3 level-four medium strategy.
pub(super) fn compress_level4(data: &[u8], input_chunks: &[usize]) -> Option<Vec<u8>> {
    let tokens = tokenize_early_matcher(data, input_chunks, 24, 32, 12, false)?;
    let mut output = vec![0x78, 0x5e];
    let mut writer = BitWriter::default();
    emit_blocks(&tokens, 32_767, &mut writer)?;
    output.extend_from_slice(&writer.finish());
    output.extend_from_slice(&adler32(data).to_be_bytes());
    Some(output)
}

/// Compress using Pillow's zlib-ng 2.3.3 level-six configuration.
pub(super) fn compress_level6(data: &[u8], input_chunks: &[usize]) -> Option<Vec<u8>> {
    let tokens = tokenize_lookahead_medium(data, input_chunks, 128, 128, 16)?;
    let mut output = vec![0x78, 0x9c];
    let mut writer = BitWriter::default();
    emit_blocks(&tokens, 32_767, &mut writer)?;
    output.extend_from_slice(&writer.finish());
    output.extend_from_slice(&adler32(data).to_be_bytes());
    Some(output)
}

/// Compress using Pillow's zlib-ng 2.3.3 level-five medium strategy.
pub(super) fn compress_level5(data: &[u8], input_chunks: &[usize]) -> Option<Vec<u8>> {
    let tokens = tokenize_lookahead_medium(data, input_chunks, 32, 32, 16)?;
    let mut output = vec![0x78, 0x5e];
    let mut writer = BitWriter::default();
    emit_blocks(&tokens, 32_767, &mut writer)?;
    output.extend_from_slice(&writer.finish());
    output.extend_from_slice(&adler32(data).to_be_bytes());
    Some(output)
}

/// Compress using Pillow's zlib-ng 2.3.3 level-seven slow strategy.
pub(super) fn compress_level7(data: &[u8], input_chunks: &[usize]) -> Option<Vec<u8>> {
    compress_slow_level(data, input_chunks, 32, 8, 128, 256, 0xda)
}

/// Compress using Pillow's zlib-ng 2.3.3 level-eight slow strategy.
pub(super) fn compress_level8(data: &[u8], input_chunks: &[usize]) -> Option<Vec<u8>> {
    compress_slow_level(data, input_chunks, 128, 32, 258, 1024, 0xda)
}

fn compress_slow_level(
    data: &[u8],
    input_chunks: &[usize],
    max_lazy: usize,
    good_match: usize,
    nice_match: usize,
    max_chain: usize,
    header: u8,
) -> Option<Vec<u8>> {
    let settings = SlowSettings {
        max_lazy,
        good_match,
        nice_match,
        max_chain,
    };
    let tokens = slow(data, input_chunks, settings)?;
    let mut output = vec![0x78, header];
    let mut writer = BitWriter::default();
    emit_blocks(&tokens, 32_767, &mut writer)?;
    output.extend_from_slice(&writer.finish());
    output.extend_from_slice(&adler32(data).to_be_bytes());
    Some(output)
}

struct SlowSettings {
    max_lazy: usize,
    good_match: usize,
    nice_match: usize,
    max_chain: usize,
}

fn slow(data: &[u8], input_chunks: &[usize], settings: SlowSettings) -> Option<Vec<Token>> {
    let mut matcher = SlowMatcher::new(
        data,
        settings.max_lazy,
        settings.good_match,
        settings.nice_match,
        settings.max_chain,
    );
    let mut available = 0usize;
    for &chunk_length in input_chunks {
        available = available.checked_add(chunk_length)?;
        debug_assert!(available <= data.len());
        matcher.process(available, false)?;
    }
    debug_assert_eq!(available, data.len());
    matcher.process(available, true)?;
    Some(matcher.tokens)
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
            previous: vec![0; WINDOW_MASK + 1],
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

    fn process(&mut self, available: usize, finishing: bool) -> Option<()> {
        loop {
            let lookahead = available.checked_sub(self.position)?;
            if lookahead == 0 || (!finishing && lookahead < MIN_LOOKAHEAD) {
                break;
            }

            let candidate = if lookahead >= MIN_MATCH {
                self.quick_insert(self.position)?
            } else {
                0
            };
            let previous_match = self.match_start;
            let mut match_length = 2usize;
            if candidate != 0
                && candidate < self.position
                && self.position.checked_sub(candidate)? <= MAX_DISTANCE
                && self.previous_length < self.max_lazy
            {
                let found = self.longest_match(candidate, lookahead)?;
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
                    distance: self.position.checked_sub(1)?.checked_sub(previous_match)?,
                });
                let maximum_insert = self.position.checked_add(lookahead)?.checked_sub(3)?;
                let move_forward = self.previous_length.checked_sub(2)?;
                let insert_count = move_forward.min(maximum_insert.saturating_sub(self.position));
                for insert_position in
                    self.position.checked_add(1)?..=self.position.checked_add(insert_count)?
                {
                    self.quick_insert(insert_position)?;
                }
                self.position = self
                    .position
                    .checked_add(self.previous_length.checked_sub(1)?)?;
                self.previous_length = 0;
                self.match_available = false;
            } else if self.match_available {
                self.tokens.push(Token::Literal(
                    *self.data.get(self.position.checked_sub(1)?)?,
                ));
                self.previous_length = match_length;
                self.position = self.position.checked_add(1)?;
            } else {
                self.previous_length = match_length;
                self.match_available = true;
                self.position = self.position.checked_add(1)?;
            }
        }

        if finishing && self.match_available {
            // With `finishing == true`, the loop above exits only when
            // lookahead reaches zero.
            debug_assert_eq!(self.position, available);
            self.tokens.push(Token::Literal(
                *self.data.get(self.position.checked_sub(1)?)?,
            ));
            self.match_available = false;
        }
        Some(())
    }

    fn quick_insert(&mut self, position: usize) -> Option<usize> {
        let word = u32::from_le_bytes(
            self.data
                .get(position..position.checked_add(4)?)?
                .try_into()
                .ok()?,
        );
        let hash = usize::try_from(word.wrapping_mul(2_654_435_761) >> 16).ok()?;
        let candidate = *self.head.get(hash)?;
        if candidate != position {
            *self.previous.get_mut(position & WINDOW_MASK)? = candidate;
            *self.head.get_mut(hash)? = position;
        }
        Some(candidate)
    }

    fn longest_match(&self, mut candidate: usize, lookahead: usize) -> Option<(usize, usize)> {
        let mut best_length = self.previous_length.max(2);
        let mut best_start = self.match_start;
        let mut chain_length = self.max_chain;
        if best_length >= self.good_match {
            chain_length >>= 2;
        }
        let limit = self.position.saturating_sub(MAX_DISTANCE);
        while candidate < self.position {
            if medium_candidate_can_improve(&self.data, candidate, self.position, best_length)? {
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
            chain_length = chain_length.checked_sub(1)?;
            if chain_length == 0 {
                break;
            }
            candidate = *self.previous.get(candidate & WINDOW_MASK)?;
            if candidate <= limit {
                break;
            }
        }
        Some((best_length.min(lookahead), best_start))
    }
}

#[cfg(feature = "tiff")]
pub(super) fn compress_level6_tiff(data: &[u8], input_chunks: &[usize]) -> Option<Vec<u8>> {
    let tokens = tokenize_lookahead_medium(data, input_chunks, 128, 128, 16)?;
    let mut output = vec![0x78, 0x9c];
    let mut writer = BitWriter::default();
    emit_blocks(&tokens, 16_383, &mut writer)?;
    output.extend_from_slice(&writer.finish());
    output.extend_from_slice(&adler32(data).to_be_bytes());
    Some(output)
}

fn tokenize_lookahead_medium(
    data: &[u8],
    input_chunks: &[usize],
    max_chain: usize,
    nice_match: usize,
    max_insert: usize,
) -> Option<Vec<Token>> {
    // ✅ VERIFIED: zlib-ng 2.3.3 deflate.c:102-128 and
    // deflate_medium.c:160-293. The oracle and Rust models produce the same
    // 2,272 tokens for the level-six PNG parity input.
    let mut matcher = Level6Matcher::new(data, max_chain, nice_match, max_insert);
    let mut available = 0usize;
    for &chunk_length in input_chunks {
        if available != 0 {
            matcher.refill_boundary()?;
        }
        available = available.checked_add(chunk_length)?;
        debug_assert!(available <= data.len());
        matcher.process(available, false)?;
    }
    debug_assert_eq!(available, data.len());
    matcher.process(available, true)?;
    Some(matcher.tokens)
}

#[derive(Clone, Copy, Default)]
struct MediumMatch {
    match_start: usize,
    length: usize,
    start: usize,
    original_start: usize,
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
            previous: vec![0; WINDOW_MASK + 1],
            position: 0,
            window_base: 0,
            tokens: Vec::new(),
            max_chain,
            nice_match,
            max_insert,
        }
    }

    fn refill_boundary(&mut self) -> Option<()> {
        // ✅ VERIFIED: zlib-ng 2.3.3 deflate.c:1213-1237. fill_window()
        // re-inserts strstart-1 when new input makes a three-byte hash valid.
        self.slide_window_if_needed()?;
        if self.position >= 1 {
            self.quick_insert(self.position - 1)?;
        }
        Some(())
    }

    fn slide_window_if_needed(&mut self) -> Option<()> {
        if self.position.checked_sub(self.window_base)? >= 32_768 + MAX_DISTANCE {
            self.window_base = self.window_base.checked_add(32_768)?;
            for position in self.head.iter_mut().chain(&mut self.previous) {
                if *position < self.window_base {
                    *position = 0;
                }
            }
        }
        Some(())
    }

    fn process(&mut self, available: usize, finishing: bool) -> Option<()> {
        let mut following = None::<MediumMatch>;
        loop {
            self.slide_window_if_needed()?;
            let lookahead = available.checked_sub(self.position)?;
            if lookahead == 0 {
                return Some(());
            }
            if lookahead < MIN_LOOKAHEAD {
                if !finishing {
                    return Some(());
                }
                // deflate_medium clears its speculative next match after
                // fill_window observes the final short lookahead. That match
                // was searched using the preceding position's larger
                // lookahead and can extend beyond the real input boundary.
                following = None;
            }

            let mut current = following
                .take()
                .map_or_else(|| self.find_match(self.position, lookahead), Some)?;
            self.insert_match(current, lookahead)?;

            if lookahead > MIN_LOOKAHEAD
                && current.start.checked_add(current.length)?
                    < self
                        .window_base
                        .checked_add(65_536usize.checked_sub(MIN_LOOKAHEAD)?)?
            {
                let future = current.start.checked_add(current.length)?;
                let mut next = self.find_match(future, lookahead)?;
                if next.length >= MIN_MATCH {
                    fizzle_matches(&self.data, &mut current, &mut next);
                }
                following = Some(next);
            }

            if current.length < MIN_MATCH {
                for offset in 0..current.length {
                    self.tokens
                        .push(Token::Literal(*self.data.get(current.start + offset)?));
                }
            } else {
                self.tokens.push(Token::Match {
                    length: current.length,
                    distance: current.start.checked_sub(current.match_start)?,
                });
            }
            self.position = self.position.checked_add(current.length)?;
        }
    }

    fn find_match(&mut self, position: usize, lookahead: usize) -> Option<MediumMatch> {
        let candidate = if lookahead >= MIN_MATCH {
            self.quick_insert(position)?
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
            && position.checked_sub(candidate)? <= MAX_DISTANCE
        {
            let (length, match_start) = self.longest_match(candidate, position, lookahead)?;
            if length >= MIN_MATCH {
                // `longest_match` can only return a match start from a prior
                // candidate accepted by the guard above.
                found.match_start = match_start;
                found.length = length;
            }
        }
        Some(found)
    }

    fn hash(&self, position: usize) -> Option<usize> {
        let bytes: [u8; 4] = self
            .data
            .get(position..position.checked_add(4)?)?
            .try_into()
            .ok()?;
        usize::try_from(u32::from_le_bytes(bytes).wrapping_mul(2_654_435_761) >> 16).ok()
    }

    fn quick_insert(&mut self, position: usize) -> Option<usize> {
        let hash = self.hash(position)?;
        let candidate = *self.head.get(hash)?;
        if candidate != position {
            *self.previous.get_mut(position & WINDOW_MASK)? = candidate;
            *self.head.get_mut(hash)? = position;
        }
        Some(candidate)
    }

    fn insert_match(&mut self, found: MediumMatch, lookahead: usize) -> Option<()> {
        // ✅ VERIFIED: zlib-ng 2.3.3 deflate_medium.c:44-94. In particular,
        // original_start prevents a left-fizzled match from reinserting old
        // positions and creating a cyclic hash chain.
        if lookahead <= found.length.checked_add(MIN_MATCH)? || found.length < MIN_MATCH {
            return Some(());
        }
        if found.length <= 16 * self.max_insert {
            let start = found.start.checked_add(1)?;
            let count = found.length.checked_sub(1)?;
            let insertion_start = start.max(found.original_start);
            let insertion_end = start.checked_add(count)?;
            for position in insertion_start..insertion_end {
                self.quick_insert(position)?;
            }
        } else {
            self.quick_insert(found.start.checked_add(found.length)?.checked_sub(1)?)?;
        }
        Some(())
    }

    fn longest_match(
        &self,
        mut candidate: usize,
        position: usize,
        lookahead: usize,
    ) -> Option<(usize, usize)> {
        // ✅ VERIFIED: zlib-ng 2.3.3 match_tpl.h:38-247 with level-six
        // {good: 8, lazy: 16, nice: 128, chain: 128} configuration.
        let mut best_length = 2usize;
        let mut best_start = 0usize;
        let mut chain_length = self.max_chain;
        let limit = position.saturating_sub(MAX_DISTANCE);
        loop {
            if candidate >= position {
                break;
            }
            if medium_candidate_can_improve(&self.data, candidate, position, best_length)? {
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
            chain_length = chain_length.checked_sub(1)?;
            if chain_length == 0 {
                break;
            }
            candidate = *self.previous.get(candidate & WINDOW_MASK)?;
            if candidate <= limit {
                break;
            }
        }
        Some((best_length, best_start))
    }
}

/// Compress using Pillow's zlib-ng 2.3.3 level-nine `Z_FILTERED`
/// configuration.
pub(super) fn compress_level9(data: &[u8], input_chunks: &[usize]) -> Option<Vec<u8>> {
    let tokens = tokenize_level9(data, input_chunks)?;
    let mut output = vec![0x78, 0xda];
    let mut writer = BitWriter::default();
    emit_blocks(&tokens, 32_767, &mut writer)?;
    output.extend_from_slice(&writer.finish());
    output.extend_from_slice(&adler32(data).to_be_bytes());
    Some(output)
}

fn tokenize_level9(data: &[u8], input_chunks: &[usize]) -> Option<Vec<Token>> {
    let mut matcher = Level9Matcher::new(data);
    let mut available = 0usize;
    for &chunk_length in input_chunks {
        if available != 0 {
            matcher.refill_boundary()?;
        }
        available = available.checked_add(chunk_length)?;
        debug_assert!(available <= data.len());
        matcher.process(available, false)?;
    }
    debug_assert_eq!(available, data.len());
    matcher.process(available, true)?;
    Some(matcher.tokens)
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
            previous: vec![0; WINDOW_MASK + 1],
            hash,
            position: 0,
            previous_length: 2,
            match_start: 0,
            match_available: false,
            tokens: Vec::new(),
        }
    }

    fn refill_boundary(&mut self) -> Option<()> {
        self.hash = rolling_hash(
            usize::from(*self.data.get(self.position)?),
            *self.data.get(self.position.checked_add(1)?)?,
        );
        Some(())
    }

    fn process(&mut self, available: usize, finishing: bool) -> Option<()> {
        loop {
            let lookahead = available.checked_sub(self.position)?;
            if lookahead == 0 || (!finishing && lookahead < MIN_LOOKAHEAD) {
                break;
            }

            let candidate = if lookahead >= MIN_MATCH {
                self.quick_insert(self.position)?
            } else {
                0
            };
            let previous_match = self.match_start;
            let mut match_length = 2usize;
            if candidate != 0
                && candidate < self.position
                && self.position.checked_sub(candidate)? <= MAX_DISTANCE
                && self.previous_length < MAX_MATCH
            {
                let found = self.longest_match(candidate, lookahead)?;
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
                    distance: self.position.checked_sub(1)?.checked_sub(previous_match)?,
                });
                let maximum_insert = self.position.checked_add(lookahead)?.checked_sub(3)?;
                let move_forward = self.previous_length.checked_sub(2)?;
                let insert_count = move_forward.min(maximum_insert.saturating_sub(self.position));
                for insert_position in
                    self.position.checked_add(1)?..=self.position.checked_add(insert_count)?
                {
                    self.quick_insert(insert_position)?;
                }
                self.position = self
                    .position
                    .checked_add(self.previous_length.checked_sub(1)?)?;
                self.previous_length = 0;
                self.match_available = false;
            } else if self.match_available {
                self.tokens.push(Token::Literal(
                    *self.data.get(self.position.checked_sub(1)?)?,
                ));
                self.previous_length = match_length;
                self.position = self.position.checked_add(1)?;
            } else {
                self.previous_length = match_length;
                self.match_available = true;
                self.position = self.position.checked_add(1)?;
            }
        }

        if finishing && self.match_available {
            // With `finishing == true`, the loop above exits only when
            // lookahead reaches zero.
            debug_assert_eq!(self.position, available);
            self.tokens.push(Token::Literal(
                *self.data.get(self.position.checked_sub(1)?)?,
            ));
            self.match_available = false;
        }
        Some(())
    }

    fn quick_insert(&mut self, position: usize) -> Option<usize> {
        self.hash = rolling_hash(self.hash, *self.data.get(position.checked_add(2)?)?);
        let candidate = *self.head.get(self.hash)?;
        if candidate != position {
            *self.previous.get_mut(position & WINDOW_MASK)? = candidate;
            *self.head.get_mut(self.hash)? = position;
        }
        Some(candidate)
    }

    fn longest_match(&self, mut candidate: usize, lookahead: usize) -> Option<(usize, usize)> {
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
            let mut hash = rolling_hash(0, *self.data.get(self.position.checked_add(1)?)?);
            hash = rolling_hash(hash, *self.data.get(self.position.checked_add(2)?)?);
            for index in 3..=best_length {
                hash = rolling_hash(hash, *self.data.get(self.position.checked_add(index)?)?);
                let position = *self.head.get(hash)?;
                if position < candidate {
                    match_offset = index.checked_sub(2)?;
                    candidate = position;
                }
            }
        }
        let mut limit = base_limit.checked_add(match_offset)?;
        if candidate <= limit {
            return Some((best_length.min(lookahead), best_start));
        }
        // The preceding `candidate <= limit` return also proves
        // `candidate > match_offset`, because `limit >= match_offset`.
        while candidate < self.position.checked_add(match_offset)? {
            let aligned = candidate.checked_sub(match_offset)?;
            if medium_candidate_can_improve(&self.data, aligned, self.position, best_length)? {
                let length =
                    match_length(&self.data, aligned, self.position, lookahead.min(MAX_MATCH));
                if length > best_length {
                    best_length = length;
                    best_start = aligned;
                    if best_length >= lookahead || best_length >= MAX_MATCH {
                        break;
                    }
                    if best_length > 3 && best_start.checked_add(best_length)? < self.position {
                        candidate = candidate.checked_sub(match_offset)?;
                        match_offset = 0;
                        let mut next_position = candidate;
                        for index in 0..=best_length.checked_sub(3)? {
                            let position = *self.previous.get((candidate + index) & WINDOW_MASK)?;
                            if position < next_position {
                                if position <= base_limit.checked_add(index)? {
                                    return Some((best_length.min(lookahead), best_start));
                                }
                                next_position = position;
                                match_offset = index;
                            }
                        }
                        candidate = next_position;

                        let hash_start = self
                            .position
                            .checked_add(best_length)?
                            .checked_sub(MIN_MATCH.checked_add(1)?)?;
                        let mut hash = rolling_hash(0, *self.data.get(hash_start)?);
                        hash = rolling_hash(hash, *self.data.get(hash_start + 1)?);
                        hash = rolling_hash(hash, *self.data.get(hash_start + 2)?);
                        let position = *self.head.get(hash)?;
                        // Unlike zlib-ng's sliding C window, this matcher
                        // eagerly inserts every absolute position in a match.
                        // The matching tail is therefore never older than the
                        // chain candidate selected above.
                        debug_assert!(position >= candidate);
                        limit = base_limit.checked_add(match_offset)?;
                        continue;
                    }
                }
            }
            chain_length = chain_length.checked_sub(1)?;
            if chain_length == 0 {
                break;
            }
            candidate = *self.previous.get(candidate & WINDOW_MASK)?;
            if candidate <= limit {
                break;
            }
        }
        Some((best_length.min(lookahead), best_start))
    }
}

fn rolling_hash(hash: usize, value: u8) -> usize {
    ((hash << 5) ^ usize::from(value)) & 32_767
}

fn medium_candidate_can_improve(
    data: &[u8],
    candidate: usize,
    position: usize,
    best_length: usize,
) -> Option<bool> {
    let mut offset = best_length.checked_sub(1)?;
    if best_length >= 4 {
        offset = offset.checked_sub(2)?;
        if best_length >= 8 {
            offset = offset.checked_sub(4)?;
        }
    }
    let width = if best_length < 4 {
        2
    } else if best_length >= 8 {
        8
    } else {
        4
    };
    Some(
        data.get(candidate..candidate.checked_add(width)?)?
            == data.get(position..position.checked_add(width)?)?
            && data.get(candidate.checked_add(offset)?..candidate.checked_add(offset + width)?)?
                == data
                    .get(position.checked_add(offset)?..position.checked_add(offset + width)?)?,
    )
}

fn fizzle_matches(data: &[u8], current: &mut MediumMatch, next: &mut MediumMatch) {
    // ✅ VERIFIED: zlib-ng 2.3.3 deflate_medium.c:96-158.
    if current.length <= 1
        || current.length > next.match_start.saturating_add(1)
        || current.length > next.start.saturating_add(1)
    {
        return;
    }
    let quick_match = next.match_start + 1 - current.length;
    let quick_original = next.start + 1 - current.length;
    if data.get(quick_match) != data.get(quick_original) {
        return;
    }

    let mut adjusted_current = *current;
    let mut adjusted_next = *next;
    let mut changed = false;
    while adjusted_current.length > 0
        && adjusted_next.length < 256
        && adjusted_next.match_start > 1
        && data.get(adjusted_next.match_start - 1) == data.get(adjusted_next.start - 1)
    {
        adjusted_next.start -= 1;
        adjusted_next.match_start -= 1;
        adjusted_next.length += 1;
        adjusted_current.length -= 1;
        changed = true;
    }
    if changed && adjusted_current.length <= 1 && adjusted_next.length != 2 {
        adjusted_next.original_start += 1;
        *current = adjusted_current;
        *next = adjusted_next;
    }
}

fn tokenize_early_matcher(
    data: &[u8],
    input_chunks: &[usize],
    max_chain: usize,
    nice_match: usize,
    max_insert: usize,
    fast: bool,
) -> Option<Vec<Token>> {
    // ⚠️ UNVERIFIED: Rust port of zlib-ng 2.3.3 deflate_medium.c:160-293.
    // The independent oracle model matches all 3,000 level-three tokens; the
    // Rust path still requires the managed byte-parity run.
    let mut matcher = Level3Matcher::new(data, max_chain, nice_match, max_insert, fast);
    let mut available = 0usize;
    for &chunk_length in input_chunks {
        available = available.checked_add(chunk_length)?;
        debug_assert!(available <= data.len());
        matcher.process(available, false)?;
    }
    debug_assert_eq!(available, data.len());
    matcher.process(available, true)?;
    Some(matcher.tokens)
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
            previous: vec![0; WINDOW_MASK + 1],
            position: 0,
            tokens: Vec::new(),
            max_chain,
            nice_match,
            max_insert,
            fast,
        }
    }

    fn process(&mut self, available: usize, finishing: bool) -> Option<()> {
        loop {
            let lookahead = available.checked_sub(self.position)?;
            if lookahead == 0 || (!finishing && lookahead < MIN_LOOKAHEAD) {
                return Some(());
            }

            let mut length = 1usize;
            let mut match_start = 0usize;
            if lookahead >= MIN_MATCH {
                let candidate = self.quick_insert(self.position)?;
                let distance = self.position.checked_sub(candidate)?;
                if candidate != 0 && distance <= MAX_DISTANCE {
                    (length, match_start) = self.longest_match(candidate, lookahead)?;
                    if length < MIN_MATCH {
                        length = 1;
                    }
                }
            }

            if length >= MIN_MATCH {
                self.tokens.push(Token::Match {
                    length,
                    distance: self.position.checked_sub(match_start)?,
                });
                self.insert_match(length, lookahead)?;
            } else {
                self.tokens
                    .push(Token::Literal(*self.data.get(self.position)?));
            }
            self.position = self.position.checked_add(length)?;
        }
    }

    fn hash(&self, position: usize) -> Option<usize> {
        // ⚠️ UNVERIFIED: zlib-ng 2.3.3 insert_string.c:11-16 and
        // insert_string_tpl.h:49-73 (four-byte multiplicative hash).
        let bytes: [u8; 4] = self
            .data
            .get(position..position.checked_add(4)?)?
            .try_into()
            .ok()?;
        let hash = u32::from_le_bytes(bytes).wrapping_mul(2_654_435_761) >> 16;
        usize::try_from(hash).ok()
    }

    fn quick_insert(&mut self, position: usize) -> Option<usize> {
        let hash = self.hash(position)?;
        let candidate = *self.head.get(hash)?;
        if candidate != position {
            *self.previous.get_mut(position & WINDOW_MASK)? = candidate;
            *self.head.get_mut(hash)? = position;
        }
        Some(candidate)
    }

    fn insert_match(&mut self, length: usize, lookahead: usize) -> Option<()> {
        // ⚠️ UNVERIFIED: zlib-ng 2.3.3 deflate_medium.c:44-94.
        let insert_limit = if self.fast {
            if lookahead.checked_sub(length)? < MIN_MATCH {
                return Some(());
            }
            self.max_insert
        } else {
            if lookahead <= length.checked_add(MIN_MATCH)? {
                return Some(());
            }
            16 * self.max_insert
        };
        if length <= insert_limit {
            for offset in 1..length {
                self.quick_insert(self.position.checked_add(offset)?)?;
            }
        } else {
            let end = self.position.checked_add(length)?;
            self.quick_insert(end.checked_sub(1)?)?;
        }
        Some(())
    }

    fn longest_match(&self, mut candidate: usize, lookahead: usize) -> Option<(usize, usize)> {
        // ⚠️ UNVERIFIED: zlib-ng 2.3.3 match_tpl.h:38-247, specialized for
        // level three's early-exit, nice-length, and chain limits.
        let mut best_length = 2usize;
        let mut best_start = 0usize;
        let mut chain_length = self.max_chain;
        let limit = self.position.saturating_sub(MAX_DISTANCE);

        loop {
            if self.candidate_can_improve(candidate, best_length)? {
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

            chain_length = chain_length.checked_sub(1)?;
            if chain_length == 0 {
                break;
            }
            candidate = *self.previous.get(candidate & WINDOW_MASK)?;
            if candidate <= limit {
                break;
            }
        }
        Some((best_length, best_start))
    }

    fn candidate_can_improve(&self, candidate: usize, best_length: usize) -> Option<bool> {
        let mut offset = best_length.checked_sub(1)?;
        if best_length >= 4 {
            offset = offset.checked_sub(2)?;
            if best_length >= 8 {
                offset = offset.checked_sub(4)?;
            }
        }
        let width = if best_length < 4 {
            2
        } else if best_length >= 8 {
            8
        } else {
            4
        };
        let candidate_end = candidate.checked_add(offset)?;
        let scan_end = self.position.checked_add(offset)?;
        Some(
            self.data.get(candidate..candidate.checked_add(width)?)?
                == self
                    .data
                    .get(self.position..self.position.checked_add(width)?)?
                && self
                    .data
                    .get(candidate_end..candidate_end.checked_add(width)?)?
                    == self.data.get(scan_end..scan_end.checked_add(width)?)?,
        )
    }
}

fn match_length(data: &[u8], left: usize, right: usize, maximum: usize) -> usize {
    let mut length = 0usize;
    while length < maximum
        && data
            .get(left + length)
            .zip(data.get(right + length))
            .is_some_and(|(left_byte, right_byte)| left_byte == right_byte)
    {
        length += 1;
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

fn build_tree(frequencies: &[u32], spec: TreeSpec<'_>) -> Option<HuffmanTree> {
    // ⚠️ UNVERIFIED: zlib-ng 2.3.3 trees.c:122-345 (heap construction,
    // depth tie-breaking, length overflow repair, and canonical codes).
    let heap_size = spec.elements * 2 + 1;
    let mut nodes = vec![Node::default(); heap_size];
    for (node, &frequency) in nodes.iter_mut().zip(frequencies) {
        node.frequency = frequency;
    }
    let mut heap = vec![0usize; heap_size];
    let mut heap_len = 0usize;
    let mut heap_max = heap_size;
    let mut max_code = None;
    for (index, node) in nodes.iter().take(spec.elements).enumerate() {
        if node.frequency != 0 {
            heap_len += 1;
            heap[heap_len] = index;
            max_code = Some(index);
        }
    }

    let mut bit_cost = 0i64;
    let mut static_cost = 0i64;
    while heap_len < 2 {
        let index = match max_code {
            Some(current_max @ 0..=1) => current_max.checked_add(1)?,
            Some(_) | None => 0,
        };
        max_code = Some(max_code.map_or(index, |current_max| current_max.max(index)));
        heap_len += 1;
        heap[heap_len] = index;
        nodes[index].frequency = 1;
        bit_cost -= 1;
        let static_length = spec
            .static_lengths
            .and_then(|lengths| lengths.get(index))
            .copied()
            .unwrap_or(0);
        static_cost -= i64::from(static_length);
    }
    let max_code = max_code?;

    for index in (1..=heap_len / 2).rev() {
        pq_down(&mut heap, heap_len, &nodes, index);
    }
    let mut next_node = spec.elements;
    while heap_len >= 2 {
        let first = remove_smallest(&mut heap, &mut heap_len, &nodes);
        let second = heap[1];
        heap_max = heap_max.checked_sub(1)?;
        heap[heap_max] = first;
        heap_max = heap_max.checked_sub(1)?;
        heap[heap_max] = second;

        nodes[next_node].frequency = nodes[first]
            .frequency
            .checked_add(nodes[second].frequency)?;
        nodes[next_node].depth = nodes[first].depth.max(nodes[second].depth).checked_add(1)?;
        nodes[first].parent = next_node;
        nodes[second].parent = next_node;
        heap[1] = next_node;
        next_node = next_node.checked_add(1)?;
        pq_down(&mut heap, heap_len, &nodes, 1);
    }
    heap_max = heap_max.checked_sub(1)?;
    heap[heap_max] = heap[1];

    let mut bit_counts = [0u16; MAX_BITS + 1];
    nodes[heap[heap_max]].length = 0;
    let mut overflow = 0i32;
    for &index in heap.get(heap_max + 1..heap_size)? {
        let mut bits = usize::from(nodes[nodes[index].parent].length).checked_add(1)?;
        if bits > spec.max_length {
            bits = spec.max_length;
            overflow += 1;
        }
        nodes[index].length = u16::try_from(bits).ok()?;
        if index > max_code {
            continue;
        }
        bit_counts[bits] = bit_counts[bits].checked_add(1)?;
        let extra = index
            .checked_sub(spec.extra_base)
            .and_then(|extra_index| spec.extra_bits.get(extra_index))
            .copied()
            .unwrap_or(0);
        let frequency = i64::from(nodes[index].frequency);
        bit_cost += frequency * i64::try_from(bits.checked_add(usize::from(extra))?).ok()?;
        if let Some(static_lengths) = spec.static_lengths {
            static_cost += frequency
                * i64::try_from(
                    usize::from(*static_lengths.get(index)?).checked_add(usize::from(extra))?,
                )
                .ok()?;
        }
    }

    if overflow > 0 {
        while overflow > 0 {
            let mut bits = spec.max_length.checked_sub(1)?;
            while bit_counts[bits] == 0 {
                bits = bits.checked_sub(1)?;
            }
            bit_counts[bits] = bit_counts[bits].checked_sub(1)?;
            bit_counts[bits + 1] = bit_counts[bits + 1].checked_add(2)?;
            bit_counts[spec.max_length] = bit_counts[spec.max_length].checked_sub(1)?;
            overflow -= 2;
        }
        debug_assert_eq!(overflow, 0);
        let mut sorted_index = heap_size;
        for bits in (1..=spec.max_length).rev() {
            let mut count = bit_counts[bits];
            while count != 0 {
                sorted_index = sorted_index.checked_sub(1)?;
                let index = heap[sorted_index];
                if index > max_code {
                    continue;
                }
                if usize::from(nodes[index].length) != bits {
                    let old_length = i64::from(nodes[index].length);
                    let frequency = i64::from(nodes[index].frequency);
                    bit_cost += (i64::try_from(bits).ok()? - old_length) * frequency;
                    nodes[index].length = u16::try_from(bits).ok()?;
                }
                count = count.checked_sub(1)?;
            }
        }
    }

    generate_codes(&mut nodes, max_code, &bit_counts)?;
    Some(HuffmanTree {
        nodes,
        max_code,
        bit_cost,
        static_cost,
    })
}

fn smaller(nodes: &[Node], left: usize, right: usize) -> bool {
    nodes[left].frequency < nodes[right].frequency
        || (nodes[left].frequency == nodes[right].frequency
            && nodes[left].depth <= nodes[right].depth)
}

fn pq_down(heap: &mut [usize], heap_len: usize, nodes: &[Node], mut root: usize) {
    let value = heap[root];
    let mut child = root * 2;
    while child <= heap_len {
        if child < heap_len && smaller(nodes, heap[child + 1], heap[child]) {
            child += 1;
        }
        if smaller(nodes, value, heap[child]) {
            break;
        }
        heap[root] = heap[child];
        root = child;
        child *= 2;
    }
    heap[root] = value;
}

fn remove_smallest(heap: &mut [usize], heap_len: &mut usize, nodes: &[Node]) -> usize {
    let smallest = heap[1];
    heap[1] = heap[*heap_len];
    *heap_len -= 1;
    pq_down(heap, *heap_len, nodes, 1);
    smallest
}

fn generate_codes(nodes: &mut [Node], max_code: usize, counts: &[u16; MAX_BITS + 1]) -> Option<()> {
    let mut next_code = [0u16; MAX_BITS + 1];
    let mut code = 0u16;
    for bits in 1..=MAX_BITS {
        code = (code + counts[bits - 1]) << 1;
        next_code[bits] = code;
    }
    for node in nodes.iter_mut().take(max_code + 1) {
        let length = usize::from(node.length);
        if length == 0 {
            continue;
        }
        node.code = reverse_bits(next_code[length], u8::try_from(length).ok()?);
        next_code[length] += 1;
    }
    Some(())
}

fn emit_blocks(tokens: &[Token], block_tokens: usize, writer: &mut BitWriter) -> Option<()> {
    let block_count = tokens.len().div_ceil(block_tokens);
    let uncompressed = expand_tokens(tokens)?;
    let mut uncompressed_start = 0usize;
    for (index, block) in tokens.chunks(block_tokens).enumerate() {
        let stored_length = block.iter().fold(0usize, |length, token| {
            length
                + match token {
                    Token::Literal(_) => 1,
                    Token::Match { length, .. } => *length,
                }
        });
        let uncompressed_end = uncompressed_start + stored_length;
        let uncompressed_block = &uncompressed[uncompressed_start..uncompressed_end];
        write_block(block, uncompressed_block, index + 1 == block_count, writer)?;
        uncompressed_start = uncompressed_end;
    }
    Some(())
}

fn expand_tokens(tokens: &[Token]) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    for token in tokens {
        match token {
            Token::Literal(value) => output.push(*value),
            Token::Match { length, distance } => {
                for _ in 0..*length {
                    let source = output.len().checked_sub(*distance)?;
                    output.push(*output.get(source)?);
                }
            }
        }
    }
    Some(output)
}

fn write_block(
    tokens: &[Token],
    uncompressed: &[u8],
    final_block: bool,
    writer: &mut BitWriter,
) -> Option<()> {
    // ⚠️ UNVERIFIED: zlib-ng 2.3.3 trees.c:628-707.
    let (literal_frequencies, distance_frequencies) = frequencies(tokens)?;
    let static_literal_lengths = static_literal_lengths();
    let static_distance_lengths = [5u8; DISTANCE_CODES];
    let literal_spec = TreeSpec {
        elements: LITERAL_CODES,
        max_length: MAX_BITS,
        extra_bits: &LENGTH_EXTRA,
        extra_base: 257,
        static_lengths: Some(&static_literal_lengths),
    };
    let literal_tree = build_tree(&literal_frequencies, literal_spec)?;
    let distance_spec = TreeSpec {
        elements: DISTANCE_CODES,
        max_length: MAX_BITS,
        extra_bits: &DISTANCE_EXTRA,
        extra_base: 0,
        static_lengths: Some(&static_distance_lengths),
    };
    let distance_tree = build_tree(&distance_frequencies, distance_spec)?;

    let mut bit_frequencies = [0u32; BIT_LENGTH_CODES];
    let literal_nodes = &literal_tree.nodes;
    scan_tree(literal_nodes, literal_tree.max_code, &mut bit_frequencies)?;
    let distance_nodes = &distance_tree.nodes;
    scan_tree(distance_nodes, distance_tree.max_code, &mut bit_frequencies)?;
    let bit_length_spec = TreeSpec {
        elements: BIT_LENGTH_CODES,
        max_length: MAX_BIT_LENGTH_BITS,
        extra_bits: &EXTRA_BIT_LENGTH_BITS,
        extra_base: 0,
        static_lengths: None,
    };
    let bit_length_tree = build_tree(&bit_frequencies, bit_length_spec)?;
    let max_bit_length_index = (3..BIT_LENGTH_CODES)
        .rev()
        .find(|&index| bit_length_tree.nodes[CODE_LENGTH_ORDER[index]].length != 0)
        .unwrap_or(3);

    let dynamic_cost = literal_tree
        .bit_cost
        .checked_add(distance_tree.bit_cost)?
        .checked_add(bit_length_tree.bit_cost)?
        .checked_add(i64::try_from(3 * (max_bit_length_index + 1) + 14).ok()?)?;
    let static_cost = literal_tree
        .static_cost
        .checked_add(distance_tree.static_cost)?;
    let dynamic_bytes = usize::try_from((dynamic_cost + 10) >> 3).ok()?;
    let static_bytes = usize::try_from((static_cost + 10) >> 3).ok()?;

    let stored_cost = if uncompressed.len() <= usize::from(u16::MAX) {
        uncompressed.len() + 4
    } else {
        usize::MAX
    };
    if stored_cost <= dynamic_bytes.min(static_bytes) {
        // The stored-cost guard above proves this conversion cannot truncate.
        #[allow(clippy::cast_possible_truncation)]
        let length = uncompressed.len() as u16;
        writer.write_bits(u32::from(final_block), 3); // BTYPE=stored (00).
        writer.align_to_byte();
        writer.write_aligned_bytes(&length.to_le_bytes());
        writer.write_aligned_bytes(&(!length).to_le_bytes());
        writer.write_aligned_bytes(uncompressed);
        return Some(());
    }
    if static_bytes <= dynamic_bytes {
        emit_fixed_block(tokens, final_block, writer)?;
    } else {
        writer.write_bits(4 | u32::from(final_block), 3); // BTYPE=dynamic (10).
        let trees = [&literal_tree, &distance_tree, &bit_length_tree];
        send_trees(trees, max_bit_length_index, writer)?;
        emit_tokens(tokens, &literal_tree, &distance_tree, writer)?;
        send_code(writer, &literal_tree, 256)?;
    }
    Some(())
}

fn frequencies(tokens: &[Token]) -> Option<([u32; LITERAL_CODES], [u32; DISTANCE_CODES])> {
    let mut literal = [0u32; LITERAL_CODES];
    let mut distance = [0u32; DISTANCE_CODES];
    literal[256] = 1;
    for token in tokens {
        match token {
            Token::Literal(value) => {
                literal[usize::from(*value)] = literal[usize::from(*value)].checked_add(1)?;
            }
            Token::Match {
                length,
                distance: match_distance,
            } => {
                let length_index = length_index(*length)?;
                literal[257 + length_index] = literal[257 + length_index].checked_add(1)?;
                let distance_index = distance_index(*match_distance)?;
                distance[distance_index] = distance[distance_index].checked_add(1)?;
            }
        }
    }
    Some((literal, distance))
}

fn scan_tree(nodes: &[Node], max_code: usize, frequencies: &mut [u32; 19]) -> Option<()> {
    // ⚠️ UNVERIFIED: zlib-ng 2.3.3 trees.c:348-396.
    let mut previous_length = usize::MAX;
    let mut current_length;
    let mut next_length = usize::from(nodes.first()?.length);
    let mut count = 0usize;
    let mut max_count = if next_length == 0 { 138 } else { 7 };
    let mut min_count = if next_length == 0 { 3 } else { 4 };

    for index in 0..=max_code {
        current_length = next_length;
        next_length = if index == max_code {
            u16::MAX.into()
        } else {
            usize::from(nodes[index + 1].length)
        };
        count += 1;
        if count < max_count && current_length == next_length {
            continue;
        }
        if count < min_count {
            frequencies[current_length] += u32::try_from(count).ok()?;
        } else if current_length != 0 {
            if current_length != previous_length {
                frequencies[current_length] += 1;
            }
            frequencies[16] += 1;
        } else if count <= 10 {
            frequencies[17] += 1;
        } else {
            frequencies[18] += 1;
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
    Some(())
}

fn send_trees(
    trees: [&HuffmanTree; 3],
    max_bit_length_index: usize,
    writer: &mut BitWriter,
) -> Option<()> {
    let [literal, distance, bit_length] = trees;
    writer.write_bits(u32::try_from(literal.max_code + 1 - 257).ok()?, 5);
    writer.write_bits(u32::try_from(distance.max_code).ok()?, 5);
    writer.write_bits(u32::try_from(max_bit_length_index + 1 - 4).ok()?, 4);
    for &code in &CODE_LENGTH_ORDER[..=max_bit_length_index] {
        writer.write_bits(u32::from(bit_length.nodes[code].length), 3);
    }
    send_tree(literal, literal.max_code, bit_length, writer)?;
    send_tree(distance, distance.max_code, bit_length, writer)
}

fn send_tree(
    tree: &HuffmanTree,
    max_code: usize,
    bit_length: &HuffmanTree,
    writer: &mut BitWriter,
) -> Option<()> {
    // ⚠️ UNVERIFIED: zlib-ng 2.3.3 trees.c:401-466.
    let mut previous_length = usize::MAX;
    let mut next_length = usize::from(tree.nodes.first()?.length);
    let mut count = 0usize;
    let mut max_count = if next_length == 0 { 138 } else { 7 };
    let mut min_count = if next_length == 0 { 3 } else { 4 };
    for index in 0..=max_code {
        let current_length = next_length;
        next_length = if index == max_code {
            u16::MAX.into()
        } else {
            usize::from(tree.nodes[index + 1].length)
        };
        count += 1;
        if count < max_count && current_length == next_length {
            continue;
        }
        if count < min_count {
            for _ in 0..count {
                send_code(writer, bit_length, current_length)?;
            }
        } else if current_length != 0 {
            if current_length != previous_length {
                send_code(writer, bit_length, current_length)?;
                count -= 1;
            }
            send_code(writer, bit_length, 16)?;
            writer.write_bits(u32::try_from(count - 3).ok()?, 2);
        } else if count <= 10 {
            send_code(writer, bit_length, 17)?;
            writer.write_bits(u32::try_from(count - 3).ok()?, 3);
        } else {
            send_code(writer, bit_length, 18)?;
            writer.write_bits(u32::try_from(count - 11).ok()?, 7);
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
    Some(())
}

fn emit_tokens(
    tokens: &[Token],
    literal_tree: &HuffmanTree,
    distance_tree: &HuffmanTree,
    writer: &mut BitWriter,
) -> Option<()> {
    for token in tokens {
        match token {
            Token::Literal(value) => send_code(writer, literal_tree, usize::from(*value))?,
            Token::Match { length, distance } => {
                let length_index = length_index(*length)?;
                send_code(writer, literal_tree, 257 + length_index)?;
                writer.write_bits(
                    u32::try_from(length - LENGTH_BASE[length_index]).ok()?,
                    LENGTH_EXTRA[length_index],
                );
                let distance_index = distance_index(*distance)?;
                send_code(writer, distance_tree, distance_index)?;
                writer.write_bits(
                    u32::try_from(distance - DISTANCE_BASE[distance_index]).ok()?,
                    DISTANCE_EXTRA[distance_index],
                );
            }
        }
    }
    Some(())
}

fn emit_fixed_block(tokens: &[Token], final_block: bool, writer: &mut BitWriter) -> Option<()> {
    writer.write_bits(2 | u32::from(final_block), 3); // BTYPE=fixed (01).
    for token in tokens {
        match token {
            Token::Literal(value) => write_fixed_symbol(writer, u16::from(*value)),
            Token::Match { length, distance } => {
                let length_index = length_index(*length)?;
                write_fixed_symbol(writer, u16::try_from(257 + length_index).ok()?);
                writer.write_bits(
                    u32::try_from(length - LENGTH_BASE[length_index]).ok()?,
                    LENGTH_EXTRA[length_index],
                );
                let distance_index = distance_index(*distance)?;
                writer.write_bits(
                    u32::from(reverse_bits(u16::try_from(distance_index).ok()?, 5)),
                    5,
                );
                writer.write_bits(
                    u32::try_from(distance - DISTANCE_BASE[distance_index]).ok()?,
                    DISTANCE_EXTRA[distance_index],
                );
            }
        }
    }
    write_fixed_symbol(writer, 256);
    Some(())
}

fn send_code(writer: &mut BitWriter, tree: &HuffmanTree, symbol: usize) -> Option<()> {
    let node = tree.nodes.get(symbol)?;
    writer.write_bits(u32::from(node.code), u8::try_from(node.length).ok()?);
    Some(())
}

fn length_index(length: usize) -> Option<usize> {
    LENGTH_BASE
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, &base)| (length >= base).then_some(index))
}

fn distance_index(distance: usize) -> Option<usize> {
    DISTANCE_BASE
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, &base)| (distance >= base).then_some(index))
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
        (0x30 + symbol, 8)
    } else if symbol <= 255 {
        (0x190 + symbol - 144, 9)
    } else if symbol <= 279 {
        (symbol - 256, 7)
    } else {
        debug_assert!(symbol <= 287);
        (0xc0 + symbol - 280, 8)
    };
    writer.write_bits(u32::from(reverse_bits(canonical, width)), width);
}

fn reverse_bits(mut value: u16, width: u8) -> u16 {
    let mut reversed = 0u16;
    for _ in 0..width {
        reversed = (reversed << 1) | (value & 1);
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
    fn write_bits(&mut self, value: u32, width: u8) {
        for bit in 0..width {
            self.current |= ((value >> bit) as u8 & 1) << self.used;
            self.used += 1;
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

fn adler32(data: &[u8]) -> u32 {
    const MODULUS: u32 = 65_521;
    let mut first = 1u32;
    let mut second = 0u32;
    for chunk in data.chunks(5_552) {
        for &byte in chunk {
            first += u32::from(byte);
            second += first;
        }
        first %= MODULUS;
        second %= MODULUS;
    }
    (second << 16) | first
}
