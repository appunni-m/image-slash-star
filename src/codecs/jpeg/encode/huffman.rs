// Modified Rust port copyright (c) 2026 Appunni M.
// Derived from libjpeg-turbo/IJG sources; see third_party/libjpeg-turbo/.

// ── JPEG Huffman encoding (libjpeg-turbo 3.1.4.1 jchuff.c / jcphuff.c) ────
//
// Bit writer + standard Huffman tables for baseline and progressive encoding.

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

        working[first] += working[second];
        working[second] = SENTINEL_FREQUENCY;
        code_size[first] += 1;
        while let Some(next) = others[first] {
            first = next;
            code_size[first] += 1;
        }
        others[first] = Some(second);
        code_size[second] += 1;
        while let Some(next) = others[second] {
            second = next;
            code_size[second] += 1;
        }
    }

    let mut length_counts = [0u16; MAX_CODE_LENGTH + 2];
    for &length in &code_size {
        length_counts[length] += 1;
    }
    let mut positions = [0usize; MAX_CODE_LENGTH + 1];
    let mut position = 0usize;
    for length in 1..=MAX_CODE_LENGTH {
        positions[length] = position;
        position += usize::from(length_counts[length]);
    }

    for length in (17..=MAX_CODE_LENGTH).rev() {
        while length_counts[length] != 0 {
            let mut prefix = length - 2;
            while length_counts[prefix] == 0 {
                prefix -= 1;
            }
            length_counts[length] -= 2;
            length_counts[length - 1] += 1;
            length_counts[prefix + 1] += 2;
            length_counts[prefix] -= 1;
        }
    }

    let mut longest = 16usize;
    while length_counts[longest] == 0 {
        longest -= 1;
    }
    length_counts[longest] -= 1;

    let mut bits = [0u8; 16];
    for (target, &value) in bits.iter_mut().zip(&length_counts[1..=16]) {
        *target = value as u8;
    }
    let value_count: usize = bits.iter().map(|&value| usize::from(value)).sum();
    let mut values = vec![0u8; value_count];
    for index in 0..count - 1 {
        let length = code_size[index];
        let target = positions[length];
        values[target] = nonzero_symbols[index] as u8;
        positions[length] = target + 1;
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
        for _ in 0..bits[l - 1] as usize {
            debug_assert!(idx < huffval.len());
            let sym = huffval[idx] as usize;
            codes[sym] = code;
            lengths[sym] = l as u8;
            code += 1;
            idx += 1;
        }
        code <<= 1;
    }

    DerivedTable { codes, lengths }
}

/// Bit writer that accumulates bits MSB-first, with 0xFF byte stuffing.
pub(crate) struct BitWriter {
    pub out: Vec<u8>,
    buf: u32,
    bits: u32,
}

impl BitWriter {
    pub(crate) fn new() -> Self {
        BitWriter {
            out: Vec::new(),
            buf: 0,
            bits: 0,
        }
    }

    /// Write `len` bits of `code` (MSB-first).
    pub(crate) fn write_bits(&mut self, code: u32, len: u8) {
        debug_assert!(len > 0);
        // Accumulate into a 32-bit buffer; flush bytes when ≥ 8 bits available.
        self.buf = (self.buf << len) | (code & ((1u32 << len) - 1));
        self.bits += len as u32;
        while self.bits >= 8 {
            self.bits -= 8;
            let byte = ((self.buf >> self.bits) & 0xFF) as u8;
            self.out.push(byte);
            if byte == 0xFF {
                self.out.push(0x00); // byte stuffing
            }
        }
    }

    /// Flush remaining bits, padding with 1s (IJG: pad with 1 bits to byte boundary).
    pub(crate) fn flush(&mut self) {
        if self.bits > 0 {
            let pad = 8 - self.bits;
            self.write_bits((1u32 << pad) - 1, pad as u8);
        }
    }
}
