//! VP8 boolean entropy encoder (RFC 6386 Section 7).
//!
//! VP8 uses a custom binary arithmetic coder that encodes one bit at a time
//! using 8-bit fixed-point probabilities (0–255, where 0 = 0% false,
//! 255 ≈ 100% false).  This module implements the encoder counterpart of
//! the boolean entropy decoder described in the specification.
//!
//! This is the byte-exact writer used by libwebp 1.6.0. It stores `range - 1`
//! and splits the interval as follows:
//!
//! ```text
//! split = range * prob >> 8
//!
//! if value == false:  range = split
//! if value == true:   value += split + 1; range -= split + 1
//! ```
//!
//! Renormalization follows libwebp's `kNorm`/`kNewRange` transformation.
//! Pending `0xff` bytes are delayed so a later carry can resolve the whole run.
//!
//! # Safety
//!
//! Zero `unsafe` code.

/// VP8 boolean entropy encoder.
///
/// Encodes a sequence of boolean values into a byte stream using
/// 8-bit fixed-point probabilities.
pub struct BoolEncoder {
    /// Current interval width minus one.
    range: i32,
    /// Pending arithmetic-coded value.
    value: i32,
    /// Number of delayed `0xff` bytes awaiting carry resolution.
    run: usize,
    /// Number of pending bits.
    nb_bits: i32,
    output: Vec<u8>,
}

impl BoolEncoder {
    /// Create a new bool encoder with initial state.
    ///
    /// * `value` = 0
    /// * `range` = 254 (`255 - 1`)
    /// * `nb_bits` = -8
    /// * `output` = empty
    pub fn new() -> Self {
        Self {
            range: 254,
            value: 0,
            run: 0,
            nb_bits: -8,
            output: Vec::new(),
        }
    }

    /// Encode a single boolean value.
    ///
    /// # Parameters
    ///
    /// * `prob` — 8-bit probability of the value being `false` (0–255).
    ///   0 means `false` is impossible; 255 means `false` is nearly certain.
    /// * `value` — `false` (0) or `true` (1) to encode.
    pub fn encode_bool(&mut self, prob: u8, value: bool) {
        let split = self.range.wrapping_mul(i32::from(prob)).wrapping_shr(8);

        if value {
            let upper_interval = split.saturating_add(1);
            self.value = self.value.wrapping_add(upper_interval);
            self.range = self.range.wrapping_sub(upper_interval);
        } else {
            self.range = split;
        }

        if self.range < 127 {
            let mut shift = 0i32;
            while self.range < 127 {
                self.range = self.range.wrapping_add(1).wrapping_shl(1).wrapping_sub(1);
                shift = shift.saturating_add(1);
            }
            self.value = self.value.wrapping_shl(shift.cast_unsigned());
            self.nb_bits = self.nb_bits.saturating_add(shift);
            if self.nb_bits > 0 {
                self.flush();
            }
        }
    }

    /// Encode one boolean value and report newly emitted bytes to a caller.
    ///
    /// The ordinary [`Self::encode_bool`] path remains allocation- and
    /// callback-free. VP8's token-aware checkpoint controllers use this
    /// variant to account for interior boolean-bitstream output without
    /// changing the byte-exact arithmetic coding.
    pub(crate) fn encode_bool_with_checkpoint<E, F>(
        &mut self,
        prob: u8,
        value: bool,
        checkpoint: &mut F,
    ) -> Result<(), E>
    where
        F: FnMut(usize) -> Result<(), E>,
    {
        let before = self.output.len();
        self.encode_bool(prob, value);
        let emitted = self.output.len().saturating_sub(before);
        if emitted != 0 {
            checkpoint(emitted)?;
        }
        Ok(())
    }

    fn flush(&mut self) {
        let shift = self.nb_bits.saturating_add(8);
        let bits = self.value.wrapping_shr(shift.cast_unsigned());
        self.value = self
            .value
            .wrapping_sub(bits.wrapping_shl(shift.cast_unsigned()));
        self.nb_bits = self.nb_bits.saturating_sub(8);
        if bits & 0xff == 0xff {
            self.run = self.run.saturating_add(1);
            return;
        }
        if bits & 0x100 != 0
            && let Some(previous) = self.output.last_mut()
        {
            *previous = previous.wrapping_add(1);
        }
        let delayed = if bits & 0x100 != 0 { 0x00 } else { 0xff };
        self.output.extend(std::iter::repeat_n(delayed, self.run));
        self.run = 0;
        self.output.push(bits.to_le_bytes()[0]);
    }

    fn flush_with_checkpoint<E, F>(&mut self, checkpoint: &mut F) -> Result<(), E>
    where
        F: FnMut(usize) -> Result<(), E>,
    {
        let before = self.output.len();
        self.flush();
        let emitted = self.output.len().saturating_sub(before);
        if emitted != 0 {
            checkpoint(emitted)?;
        }
        Ok(())
    }

    /// Flush remaining state and return the encoded byte stream.
    ///
    /// Writes `9 - nb_bits` uniform zero bits and performs one final flush,
    /// matching `VP8BitWriterFinish` in libwebp 1.6.0.
    ///
    /// # Returns
    ///
    /// The encoded byte vector (the caller takes ownership).
    pub fn finish(mut self) -> Vec<u8> {
        self.encode_literal(0, 9i32.saturating_sub(self.nb_bits).to_le_bytes()[0]);
        self.nb_bits = 0;
        self.flush();
        self.output
    }

    /// Finish the stream while reporting newly emitted bytes.
    pub(crate) fn finish_with_checkpoint<E, F>(mut self, mut checkpoint: F) -> Result<Vec<u8>, E>
    where
        F: FnMut(usize) -> Result<(), E>,
    {
        let bits = 9i32.saturating_sub(self.nb_bits).to_le_bytes()[0].min(32);
        for _ in 0..bits {
            self.encode_bool_with_checkpoint(128, false, &mut checkpoint)?;
        }
        self.nb_bits = 0;
        self.flush_with_checkpoint(&mut checkpoint)?;
        Ok(self.output)
    }

    // ------------------------------------------------------------------
    // Convenience encoding helpers
    // ------------------------------------------------------------------

    /// Encode a literal value using MSb-first bit encoding (matching image_webp's
    /// [`ArithmeticDecoder::read_literal`] which accumulates bits with
    /// `v = (v << 1) + bit`).
    ///
    /// Each bit is encoded with `prob = 128` (uniform 50/50).
    ///
    /// # Parameters
    ///
    /// * `value` — Unsigned value to encode.
    /// * `bits`  — Number of bits to encode (0..32).
    pub fn encode_literal(&mut self, value: u32, bits: u8) {
        debug_assert!(bits <= 32, "encode_literal: bits must be ≤ 32");
        let n = bits.min(32);
        for i in (0..n).rev() {
            let bit = (value.wrapping_shr(u32::from(i)) & 1) != 0;
            self.encode_bool(128, bit);
        }
    }
}

impl Default for BoolEncoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let mut encoder = BoolEncoder {
        range: 254,
        value: 0x10000,
        run: 1,
        nb_bits: 0,
        output: Vec::new(),
    };
    encoder.flush();
    assert_eq!(encoder.output, [0x00, 0x00]);
}
