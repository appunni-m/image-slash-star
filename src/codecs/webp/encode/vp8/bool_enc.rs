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

#![allow(dead_code)]

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
        let split = (self.range * i32::from(prob)) >> 8;

        if value {
            self.value += split + 1;
            self.range -= split + 1;
        } else {
            self.range = split;
        }

        if self.range < 127 {
            let mut shift = 0;
            while self.range < 127 {
                self.range = ((self.range + 1) << 1) - 1;
                shift += 1;
            }
            self.value <<= shift;
            self.nb_bits += shift;
            if self.nb_bits > 0 {
                self.flush();
            }
        }
    }

    fn flush(&mut self) {
        let shift = 8 + self.nb_bits;
        let bits = self.value >> shift;
        self.value -= bits << shift;
        self.nb_bits -= 8;
        if bits & 0xff == 0xff {
            self.run += 1;
            return;
        }
        if bits & 0x100 != 0 {
            if let Some(previous) = self.output.last_mut() {
                *previous = previous.wrapping_add(1);
            }
        }
        let delayed = if bits & 0x100 != 0 { 0x00 } else { 0xff };
        self.output.extend(std::iter::repeat_n(delayed, self.run));
        self.run = 0;
        self.output.push((bits & 0xff) as u8);
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
        self.encode_literal(0, (9 - self.nb_bits) as u8);
        self.nb_bits = 0;
        self.flush();
        self.output
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
            let bit = ((value >> i) & 1) != 0;
            self.encode_bool(128, bit);
        }
    }

    /// Encode a value using a VP8-style binary decision tree.
    ///
    /// # Tree format
    ///
    /// `tree` is a flat slice of `i8` pairs encoding a binary tree:
    /// `[left_0, right_0, left_1, right_1, ...]`.
    ///
    /// * A **positive** child value is the index of the next interior node
    ///   (multiply by 2 to get the child's index in the tree array).
    /// * A **negative** child value is a leaf: the decoded value is
    ///   `(-child) as u32`.
    ///
    /// `probs` provides the probability for each interior node.  There
    /// must be at least `tree.len() / 2` entries.
    ///
    /// # Traversal
    ///
    /// Starting at the root (node index 0), the function follows
    /// left/right branches until it reaches a leaf whose stored value
    /// matches `value`.
    pub fn encode_tree(&mut self, tree: &[i8], probs: &[u8], value: u32) {
        let mut node: usize = 0; // start at root
        loop {
            let prob = probs[node];
            let left = tree[2 * node] as i32;
            let right = tree[2 * node + 1] as i32;

            // Both children being leaves is VIRTUAL_ROOT (value comes
            // directly from the bit, not from the tree structure).
            let go_left = if left < 0 && right < 0 {
                // Both leaves: VP8 stores -(token+1); extract as -leaf-1
                // VP8 stores leaf as -(token+1); extract: token = -(leaf + 1)
                let left_val = (-(left + 1)) as u32;
                let _right_val = (-(right + 1)) as u32;
                value == left_val
            } else if left < 0 {
                // Left leaf only
                let left_val = (-(left + 1)) as u32;
                value == left_val
            } else if right < 0 {
                // Right leaf only — go left if value is NOT right leaf
                let right_val = (-(right + 1)) as u32;
                value != right_val
            } else {
                // Both interior — naive: go left (correct for VP8 token trees)
                true
            };

            self.encode_bool(prob, !go_left);

            if go_left {
                if left < 0 {
                    break;
                }
                // left is a DIRECT index into the tree array; convert to node number
                node = (left as usize) / 2;
            } else {
                if right < 0 {
                    break;
                }
                node = (right as usize) / 2;
            }
        }
    }

    /// Encode a signed value using VP8's signalling:
    ///
    /// 1. `encode_bool(128, abs > 0)` — whether the value is non-zero.
    /// 2. If non-zero: `encode_literal((abs - 1), 3)` — 3-bit magnitude.
    /// 3. If non-zero: `encode_bool(128, sign)` — sign bit.
    pub fn encode_signed(&mut self, value: i32) {
        let abs = value.unsigned_abs();
        self.encode_bool(128, abs > 0);
        if abs > 0 {
            self.encode_literal(abs - 1, 3);
            self.encode_bool(128, value < 0);
        }
    }

    // ------------------------------------------------------------------
    // Inspection helpers
    // ------------------------------------------------------------------

    /// Return the current number of emitted bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.output.len()
    }

    /// Return `true` if no bytes have been emitted yet.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.output.len() == 0
    }
}

impl Default for BoolEncoder {
    fn default() -> Self {
        Self::new()
    }
}

// -----------------------------------------------------------------------
// Standalone helpers
// -----------------------------------------------------------------------

/// Encode a signed value using VP8's encoding convention.
///
/// This is identical to `BoolEncoder::encode_signed` and is provided for
/// call sites that prefer a free function.
pub fn encode_signed(enc: &mut BoolEncoder, value: i32) {
    enc.encode_signed(value);
}
