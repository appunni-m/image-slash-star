// Modified Rust port copyright (c) 2026 Appunni M.
// Derived from libjpeg-turbo/IJG sources; see third_party/libjpeg-turbo/.

// ── IJG-faithful Derived Huffman Table (libjpeg-turbo 3.1.4.1 jdhuff.c) ──
//
// Port of jpeg_make_d_derived_tbl + HUFF_DECODE macro + jpeg_huff_decode.
//
// Uses a lookahead table (HUFF_LOOKAHEAD=10, 1,024 entries) for fast-path decode
// of codes ≤10 bits, with a bit-by-bit fallback for longer codes.
//
// Table entry format (matching IJG d_derived_tbl.lookup):
//   (nb << 8) | symbol      for codes ≤ HUFF_LOOKAHEAD
//   (HUFF_LOOKAHEAD+1) << 8  for codes > HUFF_LOOKAHEAD

#[cfg(target_arch = "aarch64")]
use super::super::encode::huffman::{STD_AC_CHROMA, STD_AC_LUMA};
use super::bit_reader::BitReader;
#[cfg(target_arch = "aarch64")]
use super::bit_reader::FastBitReader;
use crate::codecs::{CodecError, CodecResult};
use std::sync::{Arc, OnceLock};

const HUFF_LOOKAHEAD: u32 = 10;
const HUFF_LOOKAHEAD_SENTINEL: u16 = 0x0B00;
#[cfg(target_arch = "aarch64")]
const AC_PAIR_LOOKAHEAD: u32 = 12;
#[cfg(target_arch = "aarch64")]
const AC_PAIR_TABLE_SIZE: usize = 1 << AC_PAIR_LOOKAHEAD;
#[cfg(target_arch = "aarch64")]
type AcGeneralPairTable = [u32; AC_PAIR_TABLE_SIZE];

#[cfg(target_arch = "aarch64")]
const STD_AC_LUMA_GENERAL_PAIR_TABLE: AcGeneralPairTable =
    build_ac_general_pair_table(&STD_AC_LUMA.0, &STD_AC_LUMA.1);
#[cfg(target_arch = "aarch64")]
const STD_AC_CHROMA_GENERAL_PAIR_TABLE: AcGeneralPairTable =
    build_ac_general_pair_table(&STD_AC_CHROMA.0, &STD_AC_CHROMA.1);

#[derive(Debug, Clone)]
pub(super) struct HuffTable {
    /// Lookahead table: indexed by the next ten bits of input.
    /// Entry = (code_length << 8) | symbol, or the long-code sentinel.
    lookup: [u16; 1 << HUFF_LOOKAHEAD],
    /// Original Huffman symbol values (for slow-path index calculation).
    values: Vec<u8>,
    /// maxcode[l]: largest Huffman code of length l, or -1 if none.
    /// maxcode[17] is the sentinel (0x7FFFFFFF) ensuring termination.
    maxcode: [i32; 18],
    /// valoffset[l]: huffval[] index of 1st symbol of length l, minus the
    /// smallest code of length l.  Used in slow path: symbol = values[code + valoffset[l]].
    valoffset: [i32; 18],
    /// Two complete nonzero AC symbols decoded from one twelve-bit window.
    /// This is attached only to the exact standard luminance/chrominance AC
    /// tables; custom tables continue through the ordinary decoder.
    #[cfg(target_arch = "aarch64")]
    general_pair_table: Option<&'static AcGeneralPairTable>,
}

pub(super) type HuffTableStorage = Arc<HuffTable>;

pub(super) fn store_huff_table(table: HuffTable) -> HuffTableStorage {
    Arc::new(table)
}

fn standard_huff_table(
    table_class: u8,
    counts: &[u8; 16],
    values: &[u8],
) -> Option<HuffTableStorage> {
    use super::super::encode::huffman::{STD_AC_CHROMA, STD_AC_LUMA, STD_DC_CHROMA, STD_DC_LUMA};

    fn cached(
        slot: &OnceLock<Arc<HuffTable>>,
        counts: &'static [u8; 16],
        values: &'static [u8],
    ) -> Arc<HuffTable> {
        Arc::clone(slot.get_or_init(|| Arc::new(HuffTable::build(counts, values))))
    }

    static DC_LUMA: OnceLock<Arc<HuffTable>> = OnceLock::new();
    static DC_CHROMA: OnceLock<Arc<HuffTable>> = OnceLock::new();
    static AC_LUMA: OnceLock<Arc<HuffTable>> = OnceLock::new();
    static AC_CHROMA: OnceLock<Arc<HuffTable>> = OnceLock::new();

    match (table_class, counts, values) {
        (0, counts, values) if counts == &STD_DC_LUMA.0 && values == STD_DC_LUMA.1 => {
            Some(cached(&DC_LUMA, &STD_DC_LUMA.0, &STD_DC_LUMA.1))
        }
        (0, counts, values) if counts == &STD_DC_CHROMA.0 && values == STD_DC_CHROMA.1 => {
            Some(cached(&DC_CHROMA, &STD_DC_CHROMA.0, &STD_DC_CHROMA.1))
        }
        (1, counts, values) if counts == &STD_AC_LUMA.0 && values == STD_AC_LUMA.1 => {
            Some(cached(&AC_LUMA, &STD_AC_LUMA.0, &STD_AC_LUMA.1))
        }
        (1, counts, values) if counts == &STD_AC_CHROMA.0 && values == STD_AC_CHROMA.1 => {
            Some(cached(&AC_CHROMA, &STD_AC_CHROMA.0, &STD_AC_CHROMA.1))
        }
        _ => None,
    }
}

pub(super) fn build_huff_table(
    table_class: u8,
    counts: &[u8; 16],
    values: &[u8],
) -> HuffTableStorage {
    standard_huff_table(table_class, counts, values)
        .unwrap_or_else(|| store_huff_table(HuffTable::build(counts, values)))
}

impl HuffTable {
    // ── jpeg_make_d_derived_tbl ───────────────────────────────────────────

    /// Build a derived Huffman table from DHT marker data.
    /// `counts[l-1]` = number of codes of length l (1..16).
    /// `values` = symbol values in the order they appear in the DHT segment.
    pub(super) fn build(counts: &[u8; 16], values: &[u8]) -> Self {
        let numsymbols = values.len();
        #[cfg(target_arch = "aarch64")]
        let general_pair_table = standard_ac_general_pair_table(counts, values);

        // ── Generate Huffman codes (Figure F.15: code generation) ──
        let mut huffcode: Vec<i32> = vec![0; numsymbols];
        let mut code: i32 = 0;
        let mut p = 0usize;

        for l in 1usize..=16 {
            let cnt = usize::from(counts[l.saturating_sub(1)]);
            for _ in 0..cnt {
                huffcode[p] = code;
                code = code.saturating_add(1);
                p = p.saturating_add(1);
            }
            code = code.wrapping_shl(1);
        }

        // Validate codes: each code < 2^length
        p = 0;
        for l in 1usize..=16 {
            let cnt = usize::from(counts[l.saturating_sub(1)]);
            for _ in 0..cnt {
                if i64::from(huffcode[p]) >= 1i64.wrapping_shl(u32::from(l.to_le_bytes()[0])) {
                    // Bad table — return a minimal valid table
                    return HuffTable::empty();
                }
                p = p.saturating_add(1);
            }
        }

        // ── Build maxcode / valoffset (Figure F.15) ──
        let mut maxcode = [-1i32; 18];
        let mut valoffset = [0i32; 18];
        p = 0;
        for l in 1usize..=16 {
            if counts[l.saturating_sub(1)] > 0 {
                valoffset[l] = bounded_i32(p).saturating_sub(huffcode[p]);
                p = p.saturating_add(usize::from(counts[l.saturating_sub(1)]));
                maxcode[l] = huffcode[p.saturating_sub(1)];
            }
            // else: maxcode[l] stays -1 (no codes of this length)
        }
        valoffset[17] = 0;
        maxcode[17] = 0x7FFFFFi32; // IJG sentinel: 0xFFFFFL ensures termination

        // ── Build lookahead table ──
        let mut lookup = [HUFF_LOOKAHEAD_SENTINEL; 1 << HUFF_LOOKAHEAD];

        p = 0;
        for l in 1..=HUFF_LOOKAHEAD {
            let count_index = usize::from(l.saturating_sub(1).to_le_bytes()[0]);
            let cnt = usize::from(counts[count_index]);
            for _ in 0..cnt {
                // Left-justify the code followed by all possible bit sequences
                let lookbits = usize::from(bounded_u16(
                    huffcode[p].wrapping_shl(HUFF_LOOKAHEAD.saturating_sub(l)),
                ));
                let entry: u16 =
                    u16::from(l.to_le_bytes()[0]).wrapping_shl(8) | u16::from(values[p]);
                let fill_count = 1usize.wrapping_shl(HUFF_LOOKAHEAD.saturating_sub(l));
                for ctr in 0..fill_count {
                    let idx = lookbits.saturating_add(ctr);
                    lookup[idx] = entry;
                }
                p = p.saturating_add(1);
            }
        }

        HuffTable {
            lookup,
            values: values.to_vec(),
            maxcode,
            valoffset,
            #[cfg(target_arch = "aarch64")]
            general_pair_table,
        }
    }

    /// Return a minimal valid table (used when input table data is corrupt).
    fn empty() -> Self {
        let mut maxcode = [-1i32; 18];
        maxcode[17] = 0x7FFFFFi32;
        HuffTable {
            lookup: [HUFF_LOOKAHEAD_SENTINEL; 1 << HUFF_LOOKAHEAD],
            values: vec![0],
            maxcode,
            valoffset: [0i32; 18],
            #[cfg(target_arch = "aarch64")]
            general_pair_table: None,
        }
    }

    // ── HUFF_DECODE macro, inlined as a method ───────────────────────────

    /// Decode one Huffman symbol from the bit stream.
    /// Returns a malformed-data error if the entropy stream is exhausted or corrupt.
    ///
    /// Implements the IJG HUFF_DECODE macro:
    ///   1. Fast path: PEEK_BITS(HUFF_LOOKAHEAD), index lookup table.
    ///   2. If code ≤ HUFF_LOOKAHEAD bits: DROP_BITS(nb), return symbol.
    ///   3. Slow path: jpeg_huff_decode — bit-by-bit traversal.
    pub(super) fn decode(&self, br: &mut BitReader) -> CodecResult<u8> {
        // IJG HUFF_DECODE asks jpeg_fill_bit_buffer for lookahead with
        // nbits=0. Near a marker/end of segment, that means "prefetch if
        // bytes exist, but do not synthesize warning zero bits just to satisfy
        // the fast lookup table."
        if br.bits_left() < HUFF_LOOKAHEAD {
            br.fill(0);
        }
        if br.bits_left() < HUFF_LOOKAHEAD {
            // Not enough bits for lookahead — go directly to slow path.
            return self.decode_slow(br, 1);
        }

        let look = usize::from(bounded_u16(br.peek_bits(HUFF_LOOKAHEAD).cast_signed()));
        let entry = self.lookup[look];
        let nb = u32::from(entry >> 8); // code length, or HUFF_LOOKAHEAD+1 if too long

        if nb <= HUFF_LOOKAHEAD {
            // Fast path: code fits in lookahead
            br.drop_bits(nb);
            Ok(entry.to_le_bytes()[0])
        } else {
            // Slow path: code is > HUFF_LOOKAHEAD bits. IJG passes the
            // lookup-table sentinel (HUFF_LOOKAHEAD + 1), so the first slow
            // GET_BITS consumes the already-peeked 9-bit prefix.
            self.decode_slow(br, nb)
        }
    }

    /// Decode through the fully inlined marker-aware reader used by the
    /// AArch64 baseline fast path.
    #[cfg(target_arch = "aarch64")]
    #[inline(always)]
    pub(super) fn decode_fast(&self, br: &mut FastBitReader) -> CodecResult<u8> {
        if br.bits_left() < HUFF_LOOKAHEAD {
            br.fill(0);
        }
        if br.bits_left() < HUFF_LOOKAHEAD {
            return self.decode_slow_fast(br, 1);
        }

        let entry = self.lookup[br.peek_bits(HUFF_LOOKAHEAD) as usize];
        let width = u32::from(entry >> 8);
        if width <= HUFF_LOOKAHEAD {
            br.drop_bits(width);
            Ok(entry.to_le_bytes()[0])
        } else {
            self.decode_slow_fast(br, width)
        }
    }

    /// Peek two complete nonzero AC symbols. A miss leaves the reader
    /// untouched so custom tables and short entropy tails retain the normal
    /// one-symbol behavior.
    #[cfg(target_arch = "aarch64")]
    #[inline(always)]
    pub(super) fn peek_general_pair_fast(
        &self,
        br: &mut FastBitReader,
    ) -> Option<(u8, u32, u8, u32, u32)> {
        let table = self.general_pair_table?;
        if br.bits_left() < AC_PAIR_LOOKAHEAD {
            br.fill(0);
        }
        if br.bits_left() < AC_PAIR_LOOKAHEAD {
            return None;
        }
        let window = br.peek_bits(AC_PAIR_LOOKAHEAD);
        let entry = table[window as usize];
        if entry == 0 {
            return None;
        }

        let first_symbol = entry.to_le_bytes()[0];
        let first_total = (entry >> 8) & 0x0F;
        let second_symbol = (entry >> 12).to_le_bytes()[0];
        let second_total = (entry >> 20) & 0x0F;
        let first_size = u32::from(first_symbol & 0x0F);
        let second_size = u32::from(second_symbol & 0x0F);
        // The table builder only emits complete nonzero symbols whose two
        // codes fit in the twelve-bit lookahead window. Keep those invariants
        // explicit here: a malformed table entry simply falls back to the
        // ordinary one-symbol decoder instead of relying on a potentially
        // overflowing shift or subtraction.
        let combined = first_total.checked_add(second_total)?;
        let first_shift = AC_PAIR_LOOKAHEAD.checked_sub(first_total)?;
        let second_shift = AC_PAIR_LOOKAHEAD.checked_sub(combined)?;
        let first_mask = 1u32.checked_shl(first_size)?.saturating_sub(1);
        let second_mask = 1u32.checked_shl(second_size)?.saturating_sub(1);
        let first_amplitude = (window >> first_shift) & first_mask;
        let second_amplitude = (window >> second_shift) & second_mask;
        Some((
            first_symbol,
            first_amplitude,
            second_symbol,
            second_amplitude,
            combined,
        ))
    }

    // ── jpeg_huff_decode ─────────────────────────────────────────────────

    /// Slow-path Huffman decode: bit-by-bit traversal up to 16 bits.
    /// `min_bits` is the starting bit count (typically 1 after fast-path miss).
    fn decode_slow(&self, br: &mut BitReader, min_bits: u32) -> CodecResult<u8> {
        // IJG: int l = min_bits; CHECK_BIT_BUFFER(*state, l, return -1);
        //      code = GET_BITS(l);
        let min = min_bits.max(1);
        if !br.ensure(min) {
            return Err(CodecError::Malformed(
                "truncated JPEG Huffman code".to_owned(),
            ));
        }
        let mut code = br.get_bits(min).cast_signed();
        let mut l = usize::from(min.to_le_bytes()[0]);

        // IJG: while (code > htbl->maxcode[l]) {
        //        code <<= 1; CHECK_BIT_BUFFER(1); code |= GET_BITS(1); l++; }
        while code > self.maxcode[l] {
            br.ensure(1);
            code = code.wrapping_shl(1) | br.get_bits(1).cast_signed();
            l = l.saturating_add(1);
        }

        if l > 16 {
            // ✅ FIX: Match libjpeg-turbo jdhuff.c `jpeg_huff_decode`.
            //    With garbage entropy, IJG consumes through the sentinel
            //    length, warns, and returns a fake zero symbol instead of
            //    aborting the image. Keep synthetic empty tables fatal; those
            //    represent invalid DHT input that libjpeg rejects before
            //    entropy decode.
            return if self.maxcode[1..=16].iter().any(|&max| max >= 0) {
                Ok(0)
            } else {
                Err(CodecError::Malformed(
                    "invalid empty JPEG Huffman table".to_owned(),
                ))
            };
        }

        // IJG: return htbl->pub->huffval[code + htbl->valoffset[l]];
        let idx = wrapping_usize(code.saturating_add(self.valoffset[l]));
        if idx < self.values.len() {
            Ok(self.values[idx])
        } else {
            Err(CodecError::Malformed(
                "JPEG Huffman code references a missing symbol".to_owned(),
            ))
        }
    }

    #[cfg(target_arch = "aarch64")]
    #[inline(always)]
    fn decode_slow_fast(&self, br: &mut FastBitReader, min_bits: u32) -> CodecResult<u8> {
        let min = min_bits.max(1);
        if !br.ensure(min) {
            return Err(CodecError::Malformed(
                "truncated JPEG Huffman code".to_owned(),
            ));
        }
        let mut code = br.get_bits(min) as i32;
        let mut length = min as usize;
        while code > self.maxcode[length] {
            // The initial ensure pads an exhausted entropy segment to the
            // 49-bit IJG reservoir. JPEG Huffman lengths are at most 16, so
            // every continuation bit is available without another fallible
            // boundary; this matches the scalar decoder's padding rule.
            br.ensure(1);
            code = code.wrapping_shl(1) | br.get_bits(1) as i32;
            length = length.saturating_add(1);
        }
        if length > 16 {
            return if self.maxcode[1..=16].iter().any(|&max| max >= 0) {
                Ok(0)
            } else {
                Err(CodecError::Malformed(
                    "invalid empty JPEG Huffman table".to_owned(),
                ))
            };
        }
        let index = wrapping_usize(code.saturating_add(self.valoffset[length]));
        self.values.get(index).copied().ok_or_else(|| {
            CodecError::Malformed("JPEG Huffman code references a missing symbol".to_owned())
        })
    }
}

#[cfg(target_arch = "aarch64")]
fn standard_ac_general_pair_table(
    counts: &[u8; 16],
    values: &[u8],
) -> Option<&'static AcGeneralPairTable> {
    if counts == &STD_AC_LUMA.0 && values == STD_AC_LUMA.1 {
        #[cfg(coverage)]
        {
            // LLVM coverage does not observe the compile-time evaluation that
            // initializes the production table. Rebuild the complete luma
            // table from runtime-opaque inputs and compare every entry so the
            // measured path validates the same algorithm without changing the
            // non-coverage fast path.
            let bits = std::hint::black_box(STD_AC_LUMA.0);
            let values = std::hint::black_box(STD_AC_LUMA.1);
            let rebuilt = build_ac_general_pair_table(&bits, &values);
            assert!(
                std::hint::black_box(rebuilt)
                    == std::hint::black_box(STD_AC_LUMA_GENERAL_PAIR_TABLE),
                "runtime AC-pair table differs from its compile-time artifact"
            );
        }
        Some(&STD_AC_LUMA_GENERAL_PAIR_TABLE)
    } else if counts == &STD_AC_CHROMA.0 && values == STD_AC_CHROMA.1 {
        Some(&STD_AC_CHROMA_GENERAL_PAIR_TABLE)
    } else {
        None
    }
}

/// Build a compact standard-table lookup for two consecutive nonzero AC
/// symbols, including their amplitude payloads.
#[cfg(target_arch = "aarch64")]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::large_stack_arrays,
    reason = "constant construction is bounded by the JPEG symbol and twelve-bit table domains"
)]
#[cfg_attr(coverage, inline(never))]
const fn build_ac_general_pair_table(bits: &[u8; 16], values: &[u8; 162]) -> AcGeneralPairTable {
    let mut codes = [0u16; 256];
    let mut lengths = [0u8; 256];
    let mut code = 0u16;
    let mut value_index = 0usize;
    let mut code_length = 1usize;
    while code_length <= 16 {
        let mut count = 0usize;
        while count < bits[code_length - 1] as usize {
            let symbol = values[value_index] as usize;
            codes[symbol] = code;
            lengths[symbol] = code_length as u8;
            code = code.wrapping_add(1);
            value_index += 1;
            count += 1;
        }
        code = code.wrapping_shl(1);
        code_length += 1;
    }

    let mut table = [0u32; AC_PAIR_TABLE_SIZE];
    let mut first_run = 0usize;
    while first_run < 16 {
        let mut first_size = 1usize;
        while first_size <= 15 {
            let first_symbol = (first_run << 4) | first_size;
            let first_huffman_length = lengths[first_symbol] as usize;
            let first_total = first_huffman_length + first_size;
            if first_huffman_length != 0 && first_total <= AC_PAIR_LOOKAHEAD as usize {
                let mut second_run = 0usize;
                while second_run < 16 {
                    let mut second_size = 1usize;
                    while second_size <= 15 {
                        let second_symbol = (second_run << 4) | second_size;
                        let second_huffman_length = lengths[second_symbol] as usize;
                        let second_total = second_huffman_length + second_size;
                        if second_huffman_length != 0
                            && first_total + second_total <= AC_PAIR_LOOKAHEAD as usize
                        {
                            let mut first_amplitude = 0usize;
                            while first_amplitude < (1usize << first_size) {
                                let mut second_amplitude = 0usize;
                                while second_amplitude < (1usize << second_size) {
                                    let first_prefix = (codes[first_symbol] as usize) << first_size
                                        | first_amplitude;
                                    let second_prefix = (first_prefix << second_huffman_length)
                                        | codes[second_symbol] as usize;
                                    let prefix = (second_prefix << second_size) | second_amplitude;
                                    let remaining =
                                        AC_PAIR_LOOKAHEAD as usize - first_total - second_total;
                                    let base = prefix << remaining;
                                    let entry = (first_symbol as u32)
                                        | ((first_total as u32) << 8)
                                        | ((second_symbol as u32) << 12)
                                        | ((second_total as u32) << 20);
                                    let mut suffix = 0usize;
                                    while suffix < (1usize << remaining) {
                                        table[base | suffix] = entry;
                                        suffix += 1;
                                    }
                                    second_amplitude += 1;
                                }
                                first_amplitude += 1;
                            }
                        }
                        second_size += 1;
                    }
                    second_run += 1;
                }
            }
            first_size += 1;
        }
        first_run += 1;
    }
    table
}

fn bounded_i32(value: usize) -> i32 {
    let bytes = value.to_le_bytes();
    i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn bounded_u16(value: i32) -> u16 {
    let bytes = value.to_le_bytes();
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn wrapping_usize(value: i32) -> usize {
    #[cfg(target_pointer_width = "64")]
    {
        usize::from_le_bytes(i64::from(value).to_le_bytes())
    }
    #[cfg(target_pointer_width = "32")]
    {
        usize::from_le_bytes(value.to_le_bytes())
    }
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let empty = [];
    let mut table = HuffTable {
        lookup: [HUFF_LOOKAHEAD_SENTINEL; 1 << HUFF_LOOKAHEAD],
        values: Vec::new(),
        maxcode: {
            let mut maxcode = [-1; 18];
            maxcode[1] = 0;
            maxcode[17] = 0x7FFFFF;
            maxcode
        },
        valoffset: [0; 18],
        #[cfg(target_arch = "aarch64")]
        general_pair_table: None,
    };

    let mut br = BitReader::new(&empty, 0, 0);
    assert!(table.decode_slow(&mut br, 64).is_err());

    let data = [0x00];
    let mut br = BitReader::new(&data, 0, data.len());
    assert!(table.decode_slow(&mut br, 1).is_err());

    table.values.push(0);
    // A leading one keeps the synthetic table on the sentinel path.  Using
    // 0xFF here would be interpreted as a marker by BitReader's JPEG padding
    // rules and would never reach the scalar `l > 16` compatibility branch.
    let mut br = BitReader::new(&[0x80], 0, 1);
    assert_eq!(table.decode_slow(&mut br, 1), Ok(0));
    let empty_table = HuffTable::build(&[0; 16], &[]);
    let mut br = BitReader::new(&[0xFF; 16], 0, 16);
    assert!(empty_table.decode_slow(&mut br, 1).is_err());

    use super::super::encode::huffman::{STD_AC_CHROMA, STD_AC_LUMA, STD_DC_CHROMA, STD_DC_LUMA};
    let _ = build_huff_table(0, &STD_DC_LUMA.0, &STD_DC_LUMA.1);
    let _ = build_huff_table(0, &STD_DC_CHROMA.0, &STD_DC_CHROMA.1);
    let _ = build_huff_table(1, &STD_AC_LUMA.0, &STD_AC_LUMA.1);
    let _ = build_huff_table(1, &STD_AC_CHROMA.0, &STD_AC_CHROMA.1);
    let _ = build_huff_table(2, &STD_DC_LUMA.0, &STD_DC_LUMA.1);
    let mut wrong_dc_luma_values = STD_DC_LUMA.1;
    wrong_dc_luma_values[0] ^= 1;
    let mut wrong_dc_chroma_values = STD_DC_CHROMA.1;
    wrong_dc_chroma_values[0] ^= 1;
    let mut wrong_ac_luma_values = STD_AC_LUMA.1;
    wrong_ac_luma_values[0] ^= 1;
    let mut wrong_ac_chroma_values = STD_AC_CHROMA.1;
    wrong_ac_chroma_values[0] ^= 1;
    let _ = build_huff_table(0, &STD_DC_LUMA.0, &wrong_dc_luma_values);
    let _ = build_huff_table(0, &STD_DC_CHROMA.0, &wrong_dc_chroma_values);
    let _ = build_huff_table(1, &STD_AC_LUMA.0, &wrong_ac_luma_values);
    let _ = build_huff_table(1, &STD_AC_CHROMA.0, &wrong_ac_chroma_values);

    #[cfg(target_arch = "aarch64")]
    {
        use super::bit_reader::FastBitReader;

        let mut fast_empty = FastBitReader::new(&[], 0, 0);
        assert!(table.decode_slow_fast(&mut fast_empty, 50).is_err());

        let mut sentinel_maxcode = [-1; 18];
        sentinel_maxcode[1] = 0;
        sentinel_maxcode[17] = 0x7FFFFF;
        let sentinel_table = HuffTable {
            lookup: [HUFF_LOOKAHEAD_SENTINEL; 1 << HUFF_LOOKAHEAD],
            values: Vec::new(),
            maxcode: sentinel_maxcode,
            valoffset: [0; 18],
            general_pair_table: None,
        };
        let mut fast = FastBitReader::new(&[0; 16], 0, 16);
        assert_eq!(sentinel_table.decode_slow_fast(&mut fast, 11), Ok(0));

        let mut missing_symbol = FastBitReader::new(&[0; 2], 0, 2);
        assert!(
            sentinel_table
                .decode_slow_fast(&mut missing_symbol, 1)
                .is_err()
        );

        assert!(standard_ac_general_pair_table(&STD_AC_LUMA.0, &STD_AC_LUMA.1).is_some());
        assert!(standard_ac_general_pair_table(&STD_AC_CHROMA.0, &STD_AC_CHROMA.1).is_some());
        assert!(standard_ac_general_pair_table(&STD_AC_LUMA.0, &wrong_ac_luma_values).is_none());
        assert!(
            standard_ac_general_pair_table(&STD_AC_CHROMA.0, &wrong_ac_chroma_values).is_none()
        );
        assert!(standard_ac_general_pair_table(&[0; 16], &[]).is_none());
    }
}
