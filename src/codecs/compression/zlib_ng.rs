//! Altered Rust ports of byte-compatible zlib-ng 2.3.3 compressor subsets.
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
const LEVEL3_MAX_CHAIN: usize = 6;
const LEVEL3_NICE_MATCH: usize = 16;
const LEVEL3_MAX_INSERT: usize = 6;
const CODE_LENGTH_ORDER: [usize; BIT_LENGTH_CODES] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];
const EXTRA_BIT_LENGTH_BITS: [u8; BIT_LENGTH_CODES] =
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 3, 7];

#[derive(Clone, Copy)]
enum Token {
    Literal(u8),
    Match { length: usize, distance: usize },
}

/// Compress using Pillow's zlib-ng 2.3.3 level-three configuration.
///
/// Pillow's `ZipEncode.c` selects `Z_FILTERED`, a 32 KiB window, and
/// `memLevel=9`. zlib-ng maps level three to `deflate_medium` with the
/// `{ good: 4, lazy: 6, nice: 16, chain: 6 }` configuration.
pub(super) fn compress_level3(data: &[u8], input_chunks: &[usize]) -> Option<Vec<u8>> {
    let tokens = tokenize_level3(data, input_chunks)?;
    let mut output = vec![0x78, 0x5e];
    let mut writer = BitWriter::default();
    emit_blocks(&tokens, 32_767, &mut writer)?;
    output.extend_from_slice(&writer.finish());
    output.extend_from_slice(&adler32(data).to_be_bytes());
    Some(output)
}

/// Compress using Pillow's zlib-ng 2.3.3 level-six configuration.
pub(super) fn compress_level6(data: &[u8], input_chunks: &[usize]) -> Option<Vec<u8>> {
    let tokens = tokenize_level6(data, input_chunks)?;
    let mut output = vec![0x78, 0x9c];
    let mut writer = BitWriter::default();
    emit_blocks(&tokens, 32_767, &mut writer)?;
    output.extend_from_slice(&writer.finish());
    output.extend_from_slice(&adler32(data).to_be_bytes());
    Some(output)
}

pub(super) fn compress_level6_tiff(data: &[u8], input_chunks: &[usize]) -> Option<Vec<u8>> {
    let tokens = tokenize_level6(data, input_chunks)?;
    let mut output = vec![0x78, 0x9c];
    let mut writer = BitWriter::default();
    emit_blocks(&tokens, 16_383, &mut writer)?;
    output.extend_from_slice(&writer.finish());
    output.extend_from_slice(&adler32(data).to_be_bytes());
    Some(output)
}

fn tokenize_level6(data: &[u8], input_chunks: &[usize]) -> Option<Vec<Token>> {
    // ✅ VERIFIED: zlib-ng 2.3.3 deflate.c:102-128 and
    // deflate_medium.c:160-293. The oracle and Rust models produce the same
    // 2,272 tokens for the level-six PNG parity input.
    let mut matcher = Level6Matcher::new(data);
    let mut available = 0usize;
    for &chunk_length in input_chunks {
        if available != 0 {
            matcher.refill_boundary()?;
        }
        available = available.checked_add(chunk_length)?;
        if available > data.len() {
            return None;
        }
        matcher.process(available, false)?;
    }
    if available != data.len() {
        return None;
    }
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
    tokens: Vec<Token>,
}

impl Level6Matcher {
    fn new(data: &[u8]) -> Self {
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
            tokens: Vec::new(),
        }
    }

    fn refill_boundary(&mut self) -> Option<()> {
        // ✅ VERIFIED: zlib-ng 2.3.3 deflate.c:1213-1237. fill_window()
        // re-inserts strstart-1 when new input makes a three-byte hash valid.
        if self.position >= 1 {
            self.quick_insert(self.position - 1)?;
        }
        Some(())
    }

    fn process(&mut self, available: usize, finishing: bool) -> Option<()> {
        let mut following = None::<MediumMatch>;
        loop {
            let lookahead = available.checked_sub(self.position)?;
            if lookahead == 0 || (!finishing && lookahead < MIN_LOOKAHEAD) {
                return Some(());
            }

            let mut current = following
                .take()
                .map_or_else(|| self.find_match(self.position, lookahead), Some)?;
            self.insert_match(current, lookahead)?;

            if lookahead > MIN_LOOKAHEAD
                && current.start.checked_add(current.length)?
                    < 65_536usize.checked_sub(MIN_LOOKAHEAD)?
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
            if length >= MIN_MATCH && match_start < position {
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
        if found.length <= 16 * 16 {
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
        let mut chain_length = 128usize;
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
                    if best_length >= 128 || best_length >= lookahead {
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
        if available > data.len() {
            return None;
        }
        matcher.process(available, false)?;
    }
    if available != data.len() {
        return None;
    }
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
                if maximum_insert > self.position {
                    let insert_count = move_forward.min(maximum_insert - self.position);
                    for insert_position in
                        self.position.checked_add(1)?..=self.position.checked_add(insert_count)?
                    {
                        self.quick_insert(insert_position)?;
                    }
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

        if finishing && self.position == available && self.match_available {
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
        loop {
            if candidate < match_offset || candidate >= self.position.checked_add(match_offset)? {
                break;
            }
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

                        let hash_start = self.position.checked_add(best_length)?.checked_sub(4)?;
                        let mut hash = rolling_hash(0, *self.data.get(hash_start)?);
                        hash = rolling_hash(hash, *self.data.get(hash_start + 1)?);
                        hash = rolling_hash(hash, *self.data.get(hash_start + 2)?);
                        let position = *self.head.get(hash)?;
                        if position < candidate {
                            match_offset = best_length.checked_sub(4)?;
                            if position <= base_limit.checked_add(match_offset)? {
                                return Some((best_length.min(lookahead), best_start));
                            }
                            candidate = position;
                        }
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
    let limit = adjusted_next.start.saturating_sub(MAX_DISTANCE);
    let mut changed = false;
    while adjusted_current.length > 0
        && adjusted_next.start > limit
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

fn tokenize_level3(data: &[u8], input_chunks: &[usize]) -> Option<Vec<Token>> {
    // ⚠️ UNVERIFIED: Rust port of zlib-ng 2.3.3 deflate_medium.c:160-293.
    // The independent oracle model matches all 3,000 level-three tokens; the
    // Rust path still requires the managed byte-parity run.
    let mut matcher = Level3Matcher::new(data);
    let mut available = 0usize;
    for &chunk_length in input_chunks {
        available = available.checked_add(chunk_length)?;
        if available > data.len() {
            return None;
        }
        matcher.process(available, false)?;
    }
    if available != data.len() {
        return None;
    }
    matcher.process(available, true)?;
    Some(matcher.tokens)
}

struct Level3Matcher<'a> {
    data: &'a [u8],
    head: Vec<usize>,
    previous: Vec<usize>,
    position: usize,
    tokens: Vec<Token>,
}

impl<'a> Level3Matcher<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            head: vec![0; HASH_SIZE],
            previous: vec![0; WINDOW_MASK + 1],
            position: 0,
            tokens: Vec::new(),
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
        if lookahead <= length.checked_add(MIN_MATCH)? {
            return Some(());
        }
        if length <= 16 * LEVEL3_MAX_INSERT {
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
        let mut chain_length = LEVEL3_MAX_CHAIN;
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
                    if best_length >= LEVEL3_NICE_MATCH || best_length >= lookahead {
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
    let heap_size = spec.elements.checked_mul(2)?.checked_add(1)?;
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
        let current_max = max_code.unwrap_or(usize::MAX);
        let index = if current_max < 2 {
            current_max.checked_add(1)?
        } else {
            0
        };
        max_code = Some(index);
        heap_len += 1;
        heap[heap_len] = index;
        nodes[index].frequency = 1;
        bit_cost -= 1;
        if let Some(static_lengths) = spec.static_lengths {
            static_cost -= i64::from(*static_lengths.get(index)?);
        }
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
        if overflow != 0 {
            return None;
        }
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
        code = code.checked_add(counts[bits - 1])?.checked_shl(1)?;
        next_code[bits] = code;
    }
    for node in nodes.iter_mut().take(max_code + 1) {
        let length = usize::from(node.length);
        if length == 0 {
            continue;
        }
        node.code = reverse_bits(next_code[length], u8::try_from(length).ok()?);
        next_code[length] = next_code[length].checked_add(1)?;
    }
    Some(())
}

fn emit_blocks(tokens: &[Token], block_tokens: usize, writer: &mut BitWriter) -> Option<()> {
    let block_count = tokens.len().div_ceil(block_tokens);
    for (index, block) in tokens.chunks(block_tokens).enumerate() {
        let stored_length = block.iter().try_fold(0usize, |length, token| {
            length.checked_add(match token {
                Token::Literal(_) => 1,
                Token::Match { length, .. } => *length,
            })
        })?;
        emit_block(block, stored_length, index + 1 == block_count, writer)?;
    }
    Some(())
}

fn emit_block(
    tokens: &[Token],
    stored_length: usize,
    final_block: bool,
    writer: &mut BitWriter,
) -> Option<()> {
    // ⚠️ UNVERIFIED: zlib-ng 2.3.3 trees.c:628-707.
    let (literal_frequencies, distance_frequencies) = frequencies(tokens)?;
    let static_literal_lengths = static_literal_lengths();
    let static_distance_lengths = [5u8; DISTANCE_CODES];
    let literal_tree = build_tree(
        &literal_frequencies,
        TreeSpec {
            elements: LITERAL_CODES,
            max_length: MAX_BITS,
            extra_bits: &LENGTH_EXTRA,
            extra_base: 257,
            static_lengths: Some(&static_literal_lengths),
        },
    )?;
    let distance_tree = build_tree(
        &distance_frequencies,
        TreeSpec {
            elements: DISTANCE_CODES,
            max_length: MAX_BITS,
            extra_bits: &DISTANCE_EXTRA,
            extra_base: 0,
            static_lengths: Some(&static_distance_lengths),
        },
    )?;

    let mut bit_length_frequencies = [0u32; BIT_LENGTH_CODES];
    scan_tree(
        &literal_tree.nodes,
        literal_tree.max_code,
        &mut bit_length_frequencies,
    )?;
    scan_tree(
        &distance_tree.nodes,
        distance_tree.max_code,
        &mut bit_length_frequencies,
    )?;
    let bit_length_tree = build_tree(
        &bit_length_frequencies,
        TreeSpec {
            elements: BIT_LENGTH_CODES,
            max_length: MAX_BIT_LENGTH_BITS,
            extra_bits: &EXTRA_BIT_LENGTH_BITS,
            extra_base: 0,
            static_lengths: None,
        },
    )?;
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

    if stored_length.checked_add(4)? <= dynamic_bytes.min(static_bytes) {
        return None;
    }
    if static_bytes <= dynamic_bytes {
        emit_fixed_block(tokens, final_block, writer)?;
    } else {
        writer.write_bits(4 | u32::from(final_block), 3); // BTYPE=dynamic (10).
        send_all_trees(
            &literal_tree,
            &distance_tree,
            &bit_length_tree,
            max_bit_length_index,
            writer,
        )?;
        emit_tokens(tokens, &literal_tree, &distance_tree, writer)?;
        send_code(writer, &literal_tree, 256)?;
    }
    Some(())
}

fn frequencies(tokens: &[Token]) -> Option<([u32; LITERAL_CODES], [u32; DISTANCE_CODES])> {
    let mut literal = [0u32; LITERAL_CODES];
    let mut distance = [0u32; DISTANCE_CODES];
    literal[256] = 1;
    for &token in tokens {
        match token {
            Token::Literal(value) => {
                literal[usize::from(value)] = literal[usize::from(value)].checked_add(1)?;
            }
            Token::Match {
                length,
                distance: match_distance,
            } => {
                let length_index = length_index(length)?;
                literal[257 + length_index] = literal[257 + length_index].checked_add(1)?;
                let distance_index = distance_index(match_distance)?;
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
            usize::from(nodes.get(index + 1)?.length)
        };
        count += 1;
        if count < max_count && current_length == next_length {
            continue;
        }
        if count < min_count {
            frequencies[current_length] =
                frequencies[current_length].checked_add(u32::try_from(count).ok()?)?;
        } else if current_length != 0 {
            if current_length != previous_length {
                frequencies[current_length] = frequencies[current_length].checked_add(1)?;
            }
            frequencies[16] = frequencies[16].checked_add(1)?;
        } else if count <= 10 {
            frequencies[17] = frequencies[17].checked_add(1)?;
        } else {
            frequencies[18] = frequencies[18].checked_add(1)?;
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

fn send_all_trees(
    literal: &HuffmanTree,
    distance: &HuffmanTree,
    bit_length: &HuffmanTree,
    max_bit_length_index: usize,
    writer: &mut BitWriter,
) -> Option<()> {
    writer.write_bits(
        u32::try_from(literal.max_code.checked_add(1)?.checked_sub(257)?).ok()?,
        5,
    );
    writer.write_bits(u32::try_from(distance.max_code).ok()?, 5);
    writer.write_bits(
        u32::try_from(max_bit_length_index.checked_add(1)?.checked_sub(4)?).ok()?,
        4,
    );
    for &code in CODE_LENGTH_ORDER.get(..=max_bit_length_index)? {
        writer.write_bits(u32::from(bit_length.nodes.get(code)?.length), 3);
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
            usize::from(tree.nodes.get(index + 1)?.length)
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
            writer.write_bits(u32::try_from(count.checked_sub(3)?).ok()?, 2);
        } else if count <= 10 {
            send_code(writer, bit_length, 17)?;
            writer.write_bits(u32::try_from(count.checked_sub(3)?).ok()?, 3);
        } else {
            send_code(writer, bit_length, 18)?;
            writer.write_bits(u32::try_from(count.checked_sub(11)?).ok()?, 7);
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
    for &token in tokens {
        match token {
            Token::Literal(value) => send_code(writer, literal_tree, usize::from(value))?,
            Token::Match { length, distance } => {
                let length_index = length_index(length)?;
                send_code(writer, literal_tree, 257 + length_index)?;
                writer.write_bits(
                    u32::try_from(length.checked_sub(LENGTH_BASE[length_index])?).ok()?,
                    LENGTH_EXTRA[length_index],
                );
                let distance_index = distance_index(distance)?;
                send_code(writer, distance_tree, distance_index)?;
                writer.write_bits(
                    u32::try_from(distance.checked_sub(DISTANCE_BASE[distance_index])?).ok()?,
                    DISTANCE_EXTRA[distance_index],
                );
            }
        }
    }
    Some(())
}

fn emit_fixed_block(tokens: &[Token], final_block: bool, writer: &mut BitWriter) -> Option<()> {
    writer.write_bits(2 | u32::from(final_block), 3); // BTYPE=fixed (01).
    for &token in tokens {
        match token {
            Token::Literal(value) => write_fixed_symbol(writer, u16::from(value)),
            Token::Match { length, distance } => {
                let length_index = length_index(length)?;
                write_fixed_symbol(writer, u16::try_from(257 + length_index).ok()?);
                writer.write_bits(
                    u32::try_from(length.checked_sub(LENGTH_BASE[length_index])?).ok()?,
                    LENGTH_EXTRA[length_index],
                );
                let distance_index = distance_index(distance)?;
                writer.write_bits(
                    u32::from(reverse_bits(u16::try_from(distance_index).ok()?, 5)),
                    5,
                );
                writer.write_bits(
                    u32::try_from(distance.checked_sub(DISTANCE_BASE[distance_index])?).ok()?,
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
    let (canonical, width) = match symbol {
        0..=143 => (0x30 + symbol, 8),
        144..=255 => (0x190 + symbol - 144, 9),
        256..=279 => (symbol - 256, 7),
        280..=287 => (0xc0 + symbol - 280, 8),
        _ => return,
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
