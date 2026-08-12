// Modified Rust port copyright (c) 2026 Appunni M.
// Derived from libjpeg-turbo/IJG sources; see third_party/libjpeg-turbo/.

// ── JPEG Huffman encoding (libjpeg-turbo 3.1.4.1 jchuff.c / jcphuff.c) ────
//
// Bit writer + standard Huffman tables for baseline and progressive encoding.

use std::sync::OnceLock;

/// Standard DC luminance/chrominance and AC luminance/chrominance Huffman
/// tables (jcparam.c std_huff_tables).  Counts (BITS) and values (HUFFVAL).
pub(crate) const STD_DC_LUMA: ([u8; 16], [u8; 12]) = (
    [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
);

pub(crate) const STD_DC_CHROMA: ([u8; 16], [u8; 12]) = (
    [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
);

pub(crate) const STD_AC_LUMA: ([u8; 16], [u8; 162]) = (
    [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d],
    [
        0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61,
        0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52,
        0xd1, 0xf0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25,
        0x26, 0x27, 0x28, 0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45,
        0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64,
        0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83,
        0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99,
        0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6,
        0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3,
        0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8,
        0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa,
    ],
);

pub(crate) const STD_AC_CHROMA: ([u8; 16], [u8; 162]) = (
    [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77],
    [
        0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61,
        0x71, 0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33,
        0x52, 0xf0, 0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18,
        0x19, 0x1a, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44,
        0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63,
        0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a,
        0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97,
        0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4,
        0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca,
        0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7,
        0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa,
    ],
);

/// A derived Huffman code table: (code, length) for each of the 256 symbols.
pub(crate) struct DerivedTable {
    /// code[symbol] and code length len[symbol] (0 = unused).
    pub codes: [u32; 256],
    pub lengths: [u8; 256],
}

/// Largest signed quantized AC coefficient covered by the precombined
/// standard-table lookup. Keeping the table to the common small-coefficient
/// domain lets all sixteen JPEG zero-run states fit in the same footprint as
/// the former run-zero-only lookup. Values outside this range use the ordinary
/// category/magnitude path.
pub(crate) const COEFFICIENT_READY_LIMIT: i32 = 127;

pub(crate) const COEFFICIENT_READY_RUNS: usize = 16;

pub(crate) const COEFFICIENT_READY_WIDTH: usize = (COEFFICIENT_READY_LIMIT as usize)
    .saturating_mul(2)
    .saturating_add(1);

/// A standard AC Huffman code already followed by the coefficient magnitude
/// bits. The low byte stores the total width and the upper bits store the
/// combined value.
#[allow(
    clippy::large_stack_arrays,
    reason = "the fixed tables live in static read-only storage"
)]
pub(crate) struct CoefficientReadyTable {
    pub(crate) entries: [u32; COEFFICIENT_READY_WIDTH * COEFFICIENT_READY_RUNS],
}

#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the bounded coefficient domain makes every conversion exact"
)]
const fn coefficient_category(value: i32) -> u32 {
    let mut magnitude = if value < 0 {
        (-value as i64) as u32
    } else {
        value as u32
    };
    let mut category = 0u32;
    while magnitude != 0 {
        magnitude >>= 1;
        category += 1;
    }
    category
}

#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::large_stack_arrays,
    reason = "the bounded table builder executes at compile time over exact static domains"
)]
const fn build_coefficient_ready_table(
    bits: &[u8; 16],
    values: &[u8; 162],
) -> CoefficientReadyTable {
    let mut codes = [0u32; 256];
    let mut lengths = [0u8; 256];
    let mut code = 0u32;
    let mut value_index = 0usize;
    let mut code_length = 1usize;
    while code_length <= 16 {
        let mut count = 0usize;
        while count < bits[code_length - 1] as usize {
            let symbol = values[value_index] as usize;
            codes[symbol] = code;
            lengths[symbol] = code_length as u8;
            code += 1;
            value_index += 1;
            count += 1;
        }
        code <<= 1;
        code_length += 1;
    }

    let mut entries = [0u32; COEFFICIENT_READY_WIDTH * COEFFICIENT_READY_RUNS];
    let mut run = 0usize;
    while run < COEFFICIENT_READY_RUNS {
        let mut value = -COEFFICIENT_READY_LIMIT;
        while value <= COEFFICIENT_READY_LIMIT {
            let value_index = (value + COEFFICIENT_READY_LIMIT) as usize;
            let category = coefficient_category(value);
            if category != 0 {
                let symbol = run * 16 + category as usize;
                if lengths[symbol] != 0 {
                    let mask = (1u32 << category).wrapping_sub(1);
                    let emitted = (if value < 0 { value - 1 } else { value }) as u32;
                    let magnitude = emitted & mask;
                    let combined = (codes[symbol] << category) | magnitude;
                    let total_width = lengths[symbol] as u32 + category;
                    let index = run * COEFFICIENT_READY_WIDTH + value_index;
                    entries[index] = combined << 8 | total_width;
                }
            }
            value += 1;
        }
        run += 1;
    }
    CoefficientReadyTable { entries }
}

#[allow(
    clippy::large_stack_arrays,
    reason = "the fixed tables live in static read-only storage"
)]
static STANDARD_AC_LUMA_COEFFICIENT_READY: CoefficientReadyTable =
    build_coefficient_ready_table(&STD_AC_LUMA.0, &STD_AC_LUMA.1);

#[allow(
    clippy::large_stack_arrays,
    reason = "the fixed tables live in static read-only storage"
)]
static STANDARD_AC_CHROMA_COEFFICIENT_READY: CoefficientReadyTable =
    build_coefficient_ready_table(&STD_AC_CHROMA.0, &STD_AC_CHROMA.1);

pub(crate) fn standard_ac_luma_coefficient_ready() -> &'static CoefficientReadyTable {
    &STANDARD_AC_LUMA_COEFFICIENT_READY
}

pub(crate) fn standard_ac_chroma_coefficient_ready() -> &'static CoefficientReadyTable {
    &STANDARD_AC_CHROMA_COEFFICIENT_READY
}

/// Standard derived encoder tables in DC-luma, DC-chroma, AC-luma,
/// AC-chroma order. They are immutable for the life of the process, so warm
/// production calls should not rebuild the canonical 256-entry maps.
pub(crate) fn standard_derived_tables() -> &'static [DerivedTable; 4] {
    static TABLES: OnceLock<[DerivedTable; 4]> = OnceLock::new();
    TABLES.get_or_init(|| {
        [
            derive_table(&STD_DC_LUMA.0, &STD_DC_LUMA.1),
            derive_table(&STD_DC_CHROMA.0, &STD_DC_CHROMA.1),
            derive_table(&STD_AC_LUMA.0, &STD_AC_LUMA.1),
            derive_table(&STD_AC_CHROMA.0, &STD_AC_CHROMA.1),
        ]
    })
}

/// JPEG-compliant optimal Huffman table and its derived encoder lookup.
pub(crate) struct OptimalTable {
    pub bits: [u8; 16],
    pub values: Vec<u8>,
    pub derived: DerivedTable,
}

/// Build libjpeg's length-limited optimal table from observed symbol counts.
pub(crate) fn optimal_table(frequencies: &[u64; 256]) -> OptimalTable {
    // ✅ VERIFIED: libjpeg-turbo 3.1.4.1 jchuff.c:947-1110
    const MAX_CODE_LENGTH: usize = 32;
    const SENTINEL_FREQUENCY: u64 = 1_000_000_001;

    let mut source = [0u64; 257];
    source[..256].copy_from_slice(frequencies);
    source[256] = 1;

    let mut nonzero_symbols = Vec::new();
    let mut working = Vec::new();
    for (symbol, &frequency) in source.iter().enumerate() {
        if frequency != 0 {
            nonzero_symbols.push(symbol);
            working.push(frequency);
        }
    }

    let count = working.len();
    let mut code_size = vec![0usize; count];
    let mut others = vec![None::<usize>; count];
    loop {
        let mut smallest = None::<usize>;
        let mut next_smallest = None::<usize>;
        let mut smallest_frequency = 1_000_000_000u64;
        let mut next_frequency = 1_000_000_000u64;
        for (index, &frequency) in working.iter().enumerate() {
            if frequency <= next_frequency {
                if frequency <= smallest_frequency {
                    next_smallest = smallest;
                    next_frequency = smallest_frequency;
                    smallest = Some(index);
                    smallest_frequency = frequency;
                } else {
                    next_smallest = Some(index);
                    next_frequency = frequency;
                }
            }
        }
        let (Some(mut first), Some(mut second)) = (smallest, next_smallest) else {
            break;
        };

        working[first] = working[first].saturating_add(working[second]);
        working[second] = SENTINEL_FREQUENCY;
        code_size[first] = code_size[first].saturating_add(1);
        while let Some(next) = others[first] {
            first = next;
            code_size[first] = code_size[first].saturating_add(1);
        }
        others[first] = Some(second);
        code_size[second] = code_size[second].saturating_add(1);
        while let Some(next) = others[second] {
            second = next;
            code_size[second] = code_size[second].saturating_add(1);
        }
    }

    let mut length_counts = [0u16; MAX_CODE_LENGTH + 2];
    for &length in &code_size {
        length_counts[length] = length_counts[length].saturating_add(1);
    }
    let mut positions = [0usize; MAX_CODE_LENGTH + 1];
    let mut position = 0usize;
    for length in 1..=MAX_CODE_LENGTH {
        positions[length] = position;
        position = position.saturating_add(usize::from(length_counts[length]));
    }

    for length in (17..=MAX_CODE_LENGTH).rev() {
        while length_counts[length] != 0 {
            let mut prefix = length.saturating_sub(2);
            while length_counts[prefix] == 0 {
                prefix = prefix.saturating_sub(1);
            }
            length_counts[length] = length_counts[length].saturating_sub(2);
            let shorter = length.saturating_sub(1);
            length_counts[shorter] = length_counts[shorter].saturating_add(1);
            let longer_prefix = prefix.saturating_add(1);
            length_counts[longer_prefix] = length_counts[longer_prefix].saturating_add(2);
            length_counts[prefix] = length_counts[prefix].saturating_sub(1);
        }
    }

    let mut longest = 16usize;
    while length_counts[longest] == 0 {
        longest = longest.saturating_sub(1);
    }
    length_counts[longest] = length_counts[longest].saturating_sub(1);

    let mut bits = [0u8; 16];
    for (target, &value) in bits.iter_mut().zip(&length_counts[1..=16]) {
        *target = value.to_le_bytes()[0];
    }
    let value_count: usize = bits.iter().map(|&value| usize::from(value)).sum();
    let mut values = vec![0u8; value_count];
    for index in 0..count.saturating_sub(1) {
        let length = code_size[index];
        let target = positions[length];
        values[target] = nonzero_symbols[index].to_le_bytes()[0];
        positions[length] = target.saturating_add(1);
    }
    let derived = derive_table(&bits, &values);
    OptimalTable {
        bits,
        values,
        derived,
    }
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let mut frequencies = [0u64; 256];
    for (index, frequency) in frequencies.iter_mut().take(18).enumerate() {
        *frequency = 1u64 << index;
    }
    let table = optimal_table(&frequencies);
    assert!(!table.values.is_empty());
}

/// Derive canonical Huffman codes from BITS/HUFFVAL (jcphuff.c jpeg_make_c_derived_tbl).
pub(crate) fn derive_table(bits: &[u8; 16], huffval: &[u8]) -> DerivedTable {
    let mut codes = [0u32; 256];
    let mut lengths = [0u8; 256];

    // Generate canonical codes: for each code length l (1..16), assign the
    // next code value to the next symbol in huffval order.
    let mut code: u32 = 0;
    let mut idx = 0usize;
    for l in 1..=16usize {
        for _ in 0..usize::from(bits[l.saturating_sub(1)]) {
            debug_assert!(idx < huffval.len());
            let sym = usize::from(huffval[idx]);
            codes[sym] = code;
            lengths[sym] = l.to_le_bytes()[0];
            code = code.saturating_add(1);
            idx = idx.saturating_add(1);
        }
        code = code.wrapping_shl(1);
    }

    DerivedTable { codes, lengths }
}

/// Bit writer that accumulates bits MSB-first, with 0xFF byte stuffing.
pub(crate) struct BitWriter {
    pub out: Vec<u8>,
    buf: u64,
    bits: u32,
}

impl BitWriter {
    pub(crate) fn with_output(out: Vec<u8>) -> Self {
        BitWriter {
            out,
            buf: 0,
            bits: 0,
        }
    }

    pub(crate) fn into_output(self) -> Vec<u8> {
        self.out
    }

    /// Write `len` bits of `code` (MSB-first).
    #[allow(
        clippy::arithmetic_side_effects,
        clippy::cast_possible_truncation,
        reason = "JPEG bit-buffer invariants bound the shifts, pending bits, and emitted byte"
    )]
    #[inline(always)]
    pub(crate) fn write_bits(&mut self, code: u32, len: u8) {
        debug_assert!((1..=32).contains(&len));
        let length = u32::from(len);
        if self.bits + length > 64 {
            self.flush_bytes();
        }
        let mask = 1u64.wrapping_shl(length).wrapping_sub(1);
        self.buf = (self.buf << length) | (u64::from(code) & mask);
        self.bits += length;
        if self.bits == 64 {
            self.flush_bytes();
        }
    }

    /// Write bits whose value is already known to fit within `len` bits.
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "JPEG bit-buffer invariants bound the shifts and pending bit count"
    )]
    #[inline(always)]
    pub(crate) fn write_bounded_bits(&mut self, code: u32, len: u8) {
        debug_assert!((1..=32).contains(&len));
        let length = u32::from(len);
        debug_assert!(u64::from(code) < 1u64.wrapping_shl(length));
        if self.bits + length > 64 {
            self.flush_bytes();
        }
        self.buf = (self.buf << length) | u64::from(code);
        self.bits += length;
        if self.bits == 64 {
            self.flush_bytes();
        }
    }

    /// Flush complete bytes from the reservoir, applying JPEG 0xFF stuffing.
    #[allow(
        clippy::arithmetic_side_effects,
        clippy::cast_possible_truncation,
        reason = "the accumulator contains at most 64 pending bits and emitted values are bytes"
    )]
    #[inline(always)]
    fn flush_bytes(&mut self) {
        if self.bits == 64 {
            let bytes = self.buf.to_be_bytes();
            if !bytes.contains(&0xFF) {
                self.out.extend_from_slice(&bytes);
                self.bits = 0;
                self.buf = 0;
                return;
            }
        }

        while self.bits >= 8 {
            self.bits -= 8;
            let byte = (self.buf >> self.bits) as u8;
            self.out.push(byte);
            if byte == 0xFF {
                self.out.push(0x00); // byte stuffing
            }
        }

        let pending_mask = 1u64.wrapping_shl(self.bits).saturating_sub(1);
        self.buf &= pending_mask;
    }

    /// Flush remaining bits, padding with 1s (IJG: pad with 1 bits to byte boundary).
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "the writer maintains a bounded 64-bit reservoir and byte-aligned padding"
    )]
    pub(crate) fn flush(&mut self) {
        if self.bits > 0 {
            let pad = (8 - self.bits % 8) % 8;
            let mask = 1u64.wrapping_shl(pad).saturating_sub(1);
            self.buf = (self.buf << pad) | mask;
            self.bits += pad;
            self.flush_bytes();
        }
    }
}
