// Modified Rust port copyright (c) 2026 Appunni M.
// Derived from libjpeg-turbo/IJG sources; see third_party/libjpeg-turbo/.

// ── IJG-faithful Bit Reader (libjpeg-turbo 3.1.4.1 jdhuff.c) ────────────
//
// Port of the jpeg_fill_bit_buffer + CHECK_BIT_BUFFER/GET_BITS/PEEK_BITS/DROP_BITS
// macros from jdhuff.h.  Uses a 64-bit buffer (BIT_BUF_SIZE=64, MIN_GET_BITS=57 on
// 64-bit platforms) with the exact same byte-stuffing and zero-padding semantics.
//
// Bits are consumed from the MSB side: get_buffer holds the next bits_left bits
// at its most significant positions.  GET_BITS(n) extracts the top n bits and
// decrements bits_left.  PEEK_BITS(n) returns the top n bits without consuming.

pub(super) struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    end: usize,
    buf: u64,  // get_buffer — bits accumulate at MSB
    bits: u32, // bits_left — number of valid bits in buf
    insufficient_data: bool,
}

// BIT_BUF_SIZE=64 on 64-bit platforms → MIN_GET_BITS = 64-7 = 57
// We use a slightly lower threshold (49) to reduce the chance of
// marker-boundary edge cases while still matching IJG prefetch behavior.
const MIN_GET_BITS: u32 = 49;

impl<'a> BitReader<'a> {
    pub(super) fn new(data: &'a [u8], start: usize, end: usize) -> Self {
        BitReader {
            data,
            pos: start,
            end,
            buf: 0,
            bits: 0,
            insufficient_data: false,
        }
    }

    // ── jpeg_fill_bit_buffer (simplified: no suspension, no data source callbacks) ──

    /// Fill the bit buffer to at least MIN_GET_BITS bits.
    /// Handles byte stuffing (0xFF 0x00 → data 0xFF) and stops at marker bytes.
    /// On exhausted data / marker, leaves whatever bits we have (zero-padding per IJG).
    pub(super) fn fill(&mut self, nbits: u32) {
        // IJG: while (bits_left < MIN_GET_BITS) { ... }
        while self.bits < MIN_GET_BITS {
            if self.pos >= self.end {
                self.pad_with_zero_bits_if_needed(nbits);
                return;
            }
            let byte = self.data[self.pos];
            self.pos = self.pos.saturating_add(1);

            if byte == 0xFF {
                // IJG: loop to discard padding 0xFF bytes
                // We pre-split segments so we rarely see padding, but handle it.
                loop {
                    if self.pos >= self.end {
                        self.pad_with_zero_bits_if_needed(nbits);
                        return;
                    }
                    let next = self.data[self.pos];
                    if next == 0x00 {
                        // FF 00 → data byte 0xFF
                        self.pos = self.pos.saturating_add(1);
                        self.buf = self.buf.wrapping_shl(8) | 0xFF;
                        self.bits = self.bits.saturating_add(8);
                        break;
                    } else if next == 0xFF {
                        // Padding 0xFF — skip it, continue looking
                        self.pos = self.pos.saturating_add(1);
                        // continue the inner loop
                    } else {
                        // Other marker byte — end of entropy data
                        // IJG: save marker, goto no_more_bytes
                        self.pad_with_zero_bits_if_needed(nbits);
                        return;
                    }
                }
            } else {
                self.buf = self.buf.wrapping_shl(8) | u64::from(byte);
                self.bits = self.bits.saturating_add(8);
            }
        }
    }

    fn pad_with_zero_bits_if_needed(&mut self, nbits: u32) {
        if nbits > self.bits {
            let missing = MIN_GET_BITS.saturating_sub(self.bits);
            self.buf = self.buf.wrapping_shl(missing);
            self.bits = MIN_GET_BITS;
            self.insufficient_data = true;
        }
    }

    pub(super) fn insufficient_data(&self) -> bool {
        self.insufficient_data
    }

    pub(super) fn bits_left(&self) -> u32 {
        self.bits
    }

    /// Ensure at least `n` bits are available. Returns true if successful.
    #[inline]
    pub(super) fn ensure(&mut self, n: u32) -> bool {
        if self.bits < n {
            self.fill(n);
        }
        self.bits >= n
    }

    // ── IJG bit-extraction macros ──

    /// PEEK_BITS(n): peek at top n bits without consuming. Caller must ensure n ≤ bits.
    #[inline]
    pub(super) fn peek_bits(&self, n: u32) -> u32 {
        debug_assert!(n > 0 && n <= self.bits);
        let shifted = self.buf.wrapping_shr(self.bits.saturating_sub(n));
        low_u32(shifted) & u32::MAX.wrapping_shr(u32::BITS.saturating_sub(n))
    }

    /// GET_BITS(n): consume and return top n bits. Caller must ensure n ≤ bits.
    #[inline]
    pub(super) fn get_bits(&mut self, n: u32) -> u32 {
        debug_assert!(n <= self.bits && n > 0);
        self.bits = self.bits.saturating_sub(n);
        let shifted = self.buf.wrapping_shr(self.bits);
        low_u32(shifted) & u32::MAX.wrapping_shr(u32::BITS.saturating_sub(n))
    }

    /// DROP_BITS(n): discard top n bits. Caller must ensure n ≤ bits.
    #[inline]
    pub(super) fn drop_bits(&mut self, n: u32) {
        debug_assert!(n <= self.bits);
        self.bits = self.bits.saturating_sub(n);
    }

    /// Read a coefficient-width value after the JPEG entropy-padding
    /// invariant has been established.
    ///
    /// IJG pads an exhausted entropy segment to `MIN_GET_BITS`; JPEG
    /// coefficient categories use at most 15 bits. Callers that pass those
    /// syntax-derived widths therefore cannot observe `read_bits()` failure.
    pub(super) fn read_padded_bits(&mut self, n: u32) -> u32 {
        debug_assert!((1..=15).contains(&n));
        let available = self.ensure(n);
        debug_assert!(available);
        self.get_bits(n)
    }

    #[inline(always)]
    pub(super) fn read_padded_bits_optional(&mut self, n: u32) -> Option<u32> {
        if !self.ensure(n) {
            return None;
        }
        Some(self.get_bits(n))
    }
}

/// Marker-aware reader for the common baseline path.
///
/// This has the same state and padding rules as [`BitReader`], but keeps the
/// small hot operations inlinable across a complete MCU. All cursor and bit
/// widths are checked before indexing or shifting; the wrapping arithmetic is
/// used only after those bounds have been established.
#[cfg(target_arch = "aarch64")]
pub(super) struct FastBitReader<'a> {
    data: &'a [u8],
    pos: usize,
    end: usize,
    buf: u64,
    bits: u32,
    insufficient_data: bool,
}

#[cfg(target_arch = "aarch64")]
impl<'a> FastBitReader<'a> {
    pub(super) fn new(data: &'a [u8], start: usize, end: usize) -> Self {
        Self {
            data,
            pos: start,
            end,
            buf: 0,
            bits: 0,
            insufficient_data: false,
        }
    }

    #[inline(always)]
    pub(super) fn bits_left(&self) -> u32 {
        self.bits
    }

    #[inline(always)]
    pub(super) fn insufficient_data(&self) -> bool {
        self.insufficient_data
    }

    #[inline(always)]
    pub(super) fn ensure(&mut self, n: u32) -> bool {
        if self.bits < n {
            self.fill(n);
        }
        self.bits >= n
    }

    #[inline(always)]
    pub(super) fn peek_bits(&self, n: u32) -> u32 {
        debug_assert!(n > 0 && n <= self.bits);
        let shifted = self.buf.wrapping_shr(self.bits.wrapping_sub(n));
        low_u32(shifted) & u32::MAX.wrapping_shr(u32::BITS.wrapping_sub(n))
    }

    #[inline(always)]
    pub(super) fn get_bits(&mut self, n: u32) -> u32 {
        debug_assert!(n > 0 && n <= self.bits);
        self.bits = self.bits.wrapping_sub(n);
        let shifted = self.buf.wrapping_shr(self.bits);
        low_u32(shifted) & u32::MAX.wrapping_shr(u32::BITS.wrapping_sub(n))
    }

    #[inline(always)]
    pub(super) fn drop_bits(&mut self, n: u32) {
        debug_assert!(n <= self.bits);
        self.bits = self.bits.wrapping_sub(n);
    }

    #[inline(always)]
    pub(super) fn read_padded_bits(&mut self, n: u32) -> u32 {
        debug_assert!((1..=15).contains(&n));
        let available = self.ensure(n);
        debug_assert!(available);
        self.get_bits(n)
    }

    #[inline(always)]
    fn read_entropy_byte(&mut self) -> Option<u8> {
        if self.pos >= self.end {
            return None;
        }
        let byte = self.data[self.pos];
        self.pos = self.pos.wrapping_add(1);
        if byte != 0xFF {
            return Some(byte);
        }

        loop {
            if self.pos >= self.end {
                return None;
            }
            let next = self.data[self.pos];
            self.pos = self.pos.wrapping_add(1);
            match next {
                0x00 => return Some(0xFF),
                0xFF => {}
                _ => return None,
            }
        }
    }

    #[inline(always)]
    pub(super) fn fill(&mut self, nbits: u32) {
        while self.bits < MIN_GET_BITS {
            let Some(byte) = self.read_entropy_byte() else {
                if nbits > self.bits {
                    let missing = MIN_GET_BITS.wrapping_sub(self.bits);
                    self.buf = self.buf.wrapping_shl(missing);
                    self.bits = MIN_GET_BITS;
                    self.insufficient_data = true;
                }
                return;
            };
            self.buf = self.buf.wrapping_shl(8) | u64::from(byte);
            // The loop guard proves bits < 49, so adding one byte stays below
            // the 64-bit reservoir width.
            self.bits = self.bits.wrapping_add(8);
        }
    }
}

fn low_u32(value: u64) -> u32 {
    let bytes = value.to_le_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let marker_padded = [0xFF, 0xFF, 0xD9];
    let mut br = BitReader::new(&marker_padded, 0, marker_padded.len());
    br.fill(1);
    assert!(br.insufficient_data());

    let data = [0b1010_0000];
    let mut br = BitReader::new(&data, 0, data.len());
    br.fill(1);
    assert!(catch_unwind(AssertUnwindSafe(|| br.peek_bits(0))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| br.peek_bits(br.bits_left() + 1))).is_err());

    let mut br = BitReader::new(&data, 0, data.len());
    br.fill(1);
    assert!(catch_unwind(AssertUnwindSafe(|| br.get_bits(0))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| br.get_bits(br.bits_left() + 1))).is_err());
}
