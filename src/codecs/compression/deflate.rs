//! RFC 1950 zlib wrapper and RFC 1951 DEFLATE implementation.

use super::{CompressionResult, malformed, parameter};
use crate::codecs::error::CodecError;

pub(super) const LENGTH_BASE: [usize; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
pub(super) const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
pub(super) const DISTANCE_BASE: [usize; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
pub(super) const DISTANCE_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const CODE_LENGTH_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// Inflate the requested prefix of a zlib stream.
///
/// Pillow's PNG and TIFF decoders stop once their raster buffer is full, so
/// extra inflated bytes and the remainder of that zlib stream are deliberately
/// ignored.
pub(crate) fn decompress_zlib_prefix(data: &[u8], max_output: usize) -> CompressionResult<Vec<u8>> {
    decompress_zlib_with_limit(data, max_output)
}

#[cfg(coverage)]
#[allow(clippy::expect_used)]
pub(crate) fn __coverage_exercise_private_branches() {
    let _ = decompress_zlib_prefix(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00], 1);
    let _ = decompress_zlib_prefix(&[0x88, 0x00, 0x00, 0x00, 0x00, 0x00], 1);
    let _ = decompress_zlib_prefix(&[0x78, 0x00, 0x00, 0x00, 0x00, 0x00], 1);
    let _ = decompress_zlib_prefix(&[0x78, 0x20, 0x00, 0x00, 0x00, 0x00], 1);
    let _ = decompress_zlib_prefix(
        &[0x78, 0x01, 0x73, 0x04, 0x02, 0x00, 0x00, 0x00, 0x00, 0x01],
        8,
    );

    assert!(compress_zlib_chunked(&[], 10, &[]).is_err());

    assert_eq!(write_stored_block(&mut Vec::new(), &[], true), Ok(()));
    let oversized = vec![0; usize::from(u16::MAX) + 1];
    assert!(write_stored_block(&mut Vec::new(), &oversized, false).is_err());

    let mut bits = BitReader::new(&[]);
    assert_eq!(bits.read(0), Ok(0));
    assert!(bits.read(1).is_err());

    let mut bits = BitReader::new(&[]);
    let mut output = Vec::new();
    let _ = decode_stored(&mut bits, &mut output, 0);

    let mut bits = BitReader::new(&[0, 0]);
    let mut output = Vec::new();
    let _ = decode_stored(&mut bits, &mut output, 0);

    let mut bits = BitReader::new(&[0, 0, 0xff, 0xff]);
    let mut output = vec![0];
    assert!(decode_stored(&mut bits, &mut output, 0).is_err());

    let mut bits = BitReader::new(&[1, 0, 0xfe, 0xff]);
    let mut output = Vec::new();
    let _ = decode_stored(&mut bits, &mut output, 0);

    let mut bits = BitReader::new(&[1, 0, 0xfe, 0xff]);
    let mut output = Vec::new();
    assert!(decode_stored(&mut bits, &mut output, 1).is_err());

    let mut repeated = vec![0];
    assert!(extend_repeated(&mut repeated, 0, usize::MAX, usize::MAX).is_err());

    assert!(Huffman::from_lengths(&[]).is_err());
    assert!(Huffman::from_lengths(&[0]).is_err());
    let too_many_codes = vec![1u8; usize::from(u16::MAX) + 1];
    assert!(Huffman::from_lengths(&too_many_codes).is_err());

    let single = Huffman::from_lengths(&[1]).expect("coverage huffman should build");
    let mut bits = BitReader::new(&[1]);
    assert!(single.decode(&mut bits).is_err());

    let _ = decompress_zlib_prefix(&[0x78, 0x01, 0, 0, 0, 0], 1);
    let mut bits = BitReader {
        data: &[0],
        bit_position: 7,
    };
    assert!(bits.read(2).is_err());
    let mut bits = BitReader {
        data: &[],
        bit_position: usize::MAX,
    };
    assert!(bits.read(1).is_err());

    let mut bits = BitReader::new(&[]);
    assert!(read_dynamic_tables(&mut bits).is_err());
    let mut bits = BitReader::new(&[0]);
    assert!(read_dynamic_tables(&mut bits).is_err());
    let mut bits = BitReader {
        data: &[0, 0],
        bit_position: 2,
    };
    assert!(read_dynamic_tables(&mut bits).is_err());
    let mut bits = BitReader::new(&[0, 0, 0, 0]);
    assert!(read_dynamic_tables(&mut bits).is_err());
    let mut bits = BitReader {
        data: &[0, 0],
        bit_position: 5,
    };
    assert!(read_dynamic_tables(&mut bits).is_err());

    let symbol_16 = huffman_with_symbol(16);
    let mut bits = BitReader::new(&[0]);
    assert!(read_dynamic_code_lengths(&mut bits, &symbol_16, 1).is_err());

    let zero_then_16 = huffman_with_symbols(&[(0, 1), (16, 1)]);
    let mut bits = BitReader {
        data: &[0b0100_0000],
        bit_position: 5,
    };
    assert!(read_dynamic_code_lengths(&mut bits, &zero_then_16, 4).is_err());
    let mut bits = BitReader::new(&[0b0000_0010]);
    assert!(read_dynamic_code_lengths(&mut bits, &zero_then_16, 2).is_err());

    let symbol_17 = huffman_with_symbol(17);
    let mut bits = BitReader {
        data: &[0],
        bit_position: 7,
    };
    assert!(read_dynamic_code_lengths(&mut bits, &symbol_17, 1).is_err());
    let mut bits = BitReader::new(&[0]);
    assert!(read_dynamic_code_lengths(&mut bits, &symbol_17, 1).is_err());

    let symbol_18 = huffman_with_symbol(18);
    let mut bits = BitReader {
        data: &[0],
        bit_position: 7,
    };
    assert!(read_dynamic_code_lengths(&mut bits, &symbol_18, 1).is_err());

    let zero_lengths = vec![0; 258];
    assert!(build_dynamic_tables(&zero_lengths, 257).is_err());
    let mut no_distance_lengths = vec![0; 258];
    no_distance_lengths[0] = 1;
    assert!(build_dynamic_tables(&no_distance_lengths, 257).is_err());

    let literal_zero = huffman_with_symbol(0);
    let literal_end = huffman_with_symbol(256);
    let literal_match = huffman_with_symbol(257);
    let literal_extra = huffman_with_symbol(265);
    let distance_zero = huffman_with_symbol(0);
    let distance_two_bit = Huffman::from_lengths(&[2]).expect("coverage huffman should build");
    let distance_extra = huffman_with_symbol(4);
    let distance_reserved = huffman_with_symbol(30);

    let mut bits = BitReader::new(&[0]);
    let mut output = Vec::new();
    let _ = decode_compressed(&mut bits, &literal_zero, &distance_zero, &mut output, 0);

    let mut bits = BitReader::new(&[0]);
    let mut output = Vec::new();
    let _ = decode_compressed(&mut bits, &literal_end, &distance_zero, &mut output, 1);

    let mut bits = BitReader::new(&[0]);
    let mut output = Vec::new();
    assert!(
        decode_compressed(
            &mut bits,
            &literal_match,
            &distance_reserved,
            &mut output,
            8,
        )
        .is_err()
    );

    let mut bits = BitReader::new(&[0]);
    let mut output = Vec::new();
    assert!(decode_compressed(&mut bits, &literal_match, &distance_zero, &mut output, 8,).is_err());

    let mut bits = BitReader::new(&[0]);
    let mut output = vec![7];
    let _ = decode_compressed(&mut bits, &literal_match, &distance_zero, &mut output, 1);

    let mut bits = BitReader {
        data: &[0],
        bit_position: 6,
    };
    let mut output = vec![7];
    assert!(
        decode_compressed(&mut bits, &literal_match, &distance_two_bit, &mut output, 8,).is_err()
    );

    let mut bits = BitReader::new(&[0]);
    let mut output = vec![7, 8];
    assert!(decode_compressed(&mut bits, &literal_match, &distance_zero, &mut output, 1,).is_err());

    let mut bits = BitReader {
        data: &[0],
        bit_position: 7,
    };
    let mut output = Vec::new();
    assert!(decode_compressed(&mut bits, &literal_extra, &distance_zero, &mut output, 8,).is_err());

    let mut bits = BitReader {
        data: &[0],
        bit_position: 6,
    };
    let mut output = vec![7, 8, 9, 10];
    assert!(
        decode_compressed(&mut bits, &literal_match, &distance_extra, &mut output, 16,).is_err()
    );

    let mut overflowing_codes = vec![1u8; usize::from(u16::MAX)];
    overflowing_codes.extend_from_slice(&[2, 2]);
    assert!(Huffman::from_lengths(&overflowing_codes).is_err());
}

#[cfg(coverage)]
#[allow(clippy::expect_used)]
fn huffman_with_symbol(symbol: usize) -> Huffman {
    let mut lengths = vec![0; symbol + 1];
    lengths[symbol] = 1;
    Huffman::from_lengths(&lengths).expect("coverage huffman should build")
}

#[cfg(coverage)]
#[allow(clippy::expect_used)]
fn huffman_with_symbols(symbols: &[(usize, u8)]) -> Huffman {
    let mut max_symbol: Option<usize> = None;
    for &(symbol, _) in symbols {
        max_symbol = Some(match max_symbol {
            Some(maximum) => maximum.max(symbol),
            None => symbol,
        });
    }
    let max_symbol = max_symbol.expect("coverage huffman should have symbols");
    let mut lengths = vec![0; max_symbol.saturating_add(1)];
    for &(symbol, length) in symbols {
        lengths[symbol] = length;
    }
    Huffman::from_lengths(&lengths).expect("coverage huffman should build")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DecodeStatus {
    Complete,
    OutputFull,
}

// The fixed RFC 1951 distance table is statically valid.
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
fn decompress_zlib_with_limit(data: &[u8], max_output: usize) -> CompressionResult<Vec<u8>> {
    if data.len() < 6 {
        return Err(CodecError::NeedMore {
            minimum: 6,
            message:
                "invalid compressed stream: zlib stream is shorter than its header and trailer"
                    .to_owned(),
        });
    }
    let cmf = data[0];
    let flg = data[1];
    if cmf & 0x0f != 8
        || cmf >> 4 > 7
        || u16::from(cmf)
            .saturating_mul(256)
            .saturating_add(u16::from(flg))
            .rem_euclid(31)
            != 0
        || flg & 0x20 != 0
    {
        return Err(malformed(
            "zlib header is invalid or requests a preset dictionary",
        ));
    }

    let payload_end = data.len().saturating_sub(4);
    let mut bits = BitReader::new(&data[2..payload_end]);
    let mut output = Vec::with_capacity(max_output.min(65_536));
    loop {
        let block_header = bits.read(3)?;
        let final_block = block_header & 1 != 0;
        let status = match block_header >> 1 {
            0 => decode_stored(&mut bits, &mut output, max_output)?,
            1 => {
                let literal = fixed_literal_table();
                let distance =
                    Huffman::from_lengths(&[5; 32]).expect("fixed DEFLATE distance table is valid");
                decode_compressed(&mut bits, &literal, &distance, &mut output, max_output)?
            }
            2 => {
                let (literal, distance) = read_dynamic_tables(&mut bits)?;
                decode_compressed(&mut bits, &literal, &distance, &mut output, max_output)?
            }
            _ => return Err(malformed("DEFLATE block type is reserved")),
        };
        if status == DecodeStatus::OutputFull {
            return Ok(output);
        }
        if final_block {
            break;
        }
    }

    let trailer = &data[payload_end..];
    let expected = u32::from_be_bytes([trailer[0], trailer[1], trailer[2], trailer[3]]);
    if adler32(&output) != expected {
        return Err(malformed("zlib Adler-32 checksum does not match"));
    }
    Ok(output)
}

/// Compress TIFF scanlines with zlib-ng's default memLevel-eight buffer.
#[cfg(feature = "tiff")]
pub(crate) fn compress_zlib_tiff(data: &[u8], input_chunks: &[usize]) -> Vec<u8> {
    debug_assert_eq!(
        input_chunks
            .iter()
            .fold(0usize, |total, &length| total.wrapping_add(length)),
        data.len()
    );
    super::zlib_ng::compress_level6_tiff(data, input_chunks)
}

/// Compress a sequence of input calls as one zlib stream.
///
/// The chunk lengths model callers such as Pillow's PNG encoder, which feeds
/// one complete filtered scanline to zlib-ng at a time. Input-call boundaries
/// are observable at level zero because zlib-ng emits a stored block when its
/// buffered input first reaches the 32 KiB window size.
#[cfg(any(feature = "png", feature = "tiff"))]
pub(crate) fn compress_zlib_chunked(
    data: &[u8],
    level: u8,
    input_chunks: &[usize],
) -> CompressionResult<Vec<u8>> {
    debug_assert_eq!(
        input_chunks
            .iter()
            .fold(0usize, |total, &length| total.wrapping_add(length)),
        data.len()
    );
    match level {
        0 => compress_zlib_stored_chunked(data, input_chunks),
        1 => Ok(super::zlib_ng::compress_level1(data, input_chunks)),
        2 => Ok(super::zlib_ng::compress_level2(data, input_chunks)),
        3 => Ok(super::zlib_ng::compress_level3(data, input_chunks)),
        4 => Ok(super::zlib_ng::compress_level4(data, input_chunks)),
        5 => Ok(super::zlib_ng::compress_level5(data, input_chunks)),
        6 => Ok(super::zlib_ng::compress_level6(data, input_chunks)),
        7 => Ok(super::zlib_ng::compress_level7(data, input_chunks)),
        8 => Ok(super::zlib_ng::compress_level8(data, input_chunks)),
        9 => Ok(super::zlib_ng::compress_level9(data, input_chunks)),
        _ => Err(parameter("compression level is outside 0..=9")),
    }
}

#[cfg(any(feature = "png", feature = "tiff"))]
fn compress_zlib_stored_chunked(data: &[u8], input_chunks: &[usize]) -> CompressionResult<Vec<u8>> {
    const MIN_BLOCK: usize = 32_768;
    const MAX_STORED: usize = 65_535;

    let mut output = vec![0x78, 0x01];
    let mut pending_start = 0usize;
    let mut input_end = 0usize;
    for &input_len in input_chunks {
        input_end = input_end.wrapping_add(input_len);
        while input_end.saturating_sub(pending_start) >= MIN_BLOCK {
            let maximum_end = pending_start.saturating_add(MAX_STORED);
            let block_end = input_end.min(maximum_end);
            write_stored_block_bounded(&mut output, &data[pending_start..block_end], false);
            pending_start = block_end;
        }
    }
    write_stored_block_bounded(&mut output, &data[pending_start..], true);
    output.extend_from_slice(&adler32(data).to_be_bytes());
    Ok(output)
}

#[cfg(all(coverage, any(feature = "png", feature = "tiff")))]
fn write_stored_block(
    output: &mut Vec<u8>,
    block: &[u8],
    final_block: bool,
) -> CompressionResult<()> {
    let Ok(len) = u16::try_from(block.len()) else {
        return Err(parameter("stored DEFLATE block exceeds 65,535 bytes"));
    };
    write_stored_block_with_len(output, block, final_block, len);
    Ok(())
}

#[cfg(any(feature = "png", feature = "tiff"))]
fn write_stored_block_bounded(output: &mut Vec<u8>, block: &[u8], final_block: bool) {
    debug_assert!(u16::try_from(block.len()).is_ok());
    write_stored_block_with_len(output, block, final_block, low_u16(block.len()));
}

#[cfg(any(feature = "png", feature = "tiff"))]
fn write_stored_block_with_len(output: &mut Vec<u8>, block: &[u8], final_block: bool, len: u16) {
    output.push(u8::from(final_block));
    output.extend_from_slice(&len.to_le_bytes());
    output.extend_from_slice(&(!len).to_le_bytes());
    output.extend_from_slice(block);
}

fn decode_stored(
    bits: &mut BitReader<'_>,
    output: &mut Vec<u8>,
    max_output: usize,
) -> CompressionResult<DecodeStatus> {
    bits.align_to_byte();
    let len = low_u16(bounded_usize(bits.read(16)?));
    let complement = low_u16(bounded_usize(bits.read(16)?));
    if len != !complement {
        return Err(malformed("stored DEFLATE length complement does not match"));
    }
    let Some(available) = max_output.checked_sub(output.len()) else {
        return Err(malformed("inflated output exceeds its configured limit"));
    };
    let copied = usize::from(len).min(available);
    for _ in 0..copied {
        output.push(bits.read(8)?.to_le_bytes()[0]);
    }
    if copied < usize::from(len) {
        Ok(DecodeStatus::OutputFull)
    } else {
        Ok(DecodeStatus::Complete)
    }
}

// The fixed RFC 1951 literal table is statically valid.
#[allow(clippy::expect_used)]
fn fixed_literal_table() -> Huffman {
    let mut lengths = vec![0; 288];
    lengths[0..144].fill(8);
    lengths[144..256].fill(9);
    lengths[256..280].fill(7);
    lengths[280..288].fill(8);
    Huffman::from_lengths(&lengths).expect("fixed DEFLATE literal table is valid")
}

fn read_dynamic_tables(bits: &mut BitReader<'_>) -> CompressionResult<(Huffman, Huffman)> {
    let literal_count = bounded_usize(bits.read(5)?).saturating_add(257);
    let distance_count = bounded_usize(bits.read(5)?).saturating_add(1);
    let code_length_count = bounded_usize(bits.read(4)?).saturating_add(4);
    let mut code_lengths = [0u8; 19];
    for &symbol in &CODE_LENGTH_ORDER[..code_length_count] {
        code_lengths[symbol] = bits.read(3)?.to_le_bytes()[0];
    }
    let code_length_table = Huffman::from_lengths(&code_lengths)?;

    let total = literal_count.saturating_add(distance_count);
    let lengths = read_dynamic_code_lengths(bits, &code_length_table, total)?;
    build_dynamic_tables(&lengths, literal_count)
}

fn read_dynamic_code_lengths(
    bits: &mut BitReader<'_>,
    code_length_table: &Huffman,
    total: usize,
) -> CompressionResult<Vec<u8>> {
    let mut lengths = Vec::with_capacity(total);
    while lengths.len() < total {
        let symbol = code_length_table.decode(bits)?;
        match symbol {
            symbol @ 0..=15 => lengths.push(symbol.to_le_bytes()[0]),
            16 => {
                let Some(&previous) = lengths.last() else {
                    return Err(malformed("repeat code appears before any code length"));
                };
                let repeat = bounded_usize(bits.read(2)?).saturating_add(3);
                extend_repeated(&mut lengths, previous, repeat, total)?;
            }
            17 => {
                let repeat = bounded_usize(bits.read(3)?).saturating_add(3);
                extend_repeated(&mut lengths, 0, repeat, total)?;
            }
            _ => {
                // The code-length alphabet has exactly 19 symbols.
                debug_assert_eq!(symbol, 18);
                let repeat = bounded_usize(bits.read(7)?).saturating_add(11);
                extend_repeated(&mut lengths, 0, repeat, total)?;
            }
        }
    }
    Ok(lengths)
}

fn build_dynamic_tables(
    lengths: &[u8],
    literal_count: usize,
) -> CompressionResult<(Huffman, Huffman)> {
    let literal = Huffman::from_lengths(&lengths[..literal_count])?;
    let distance = Huffman::from_lengths(&lengths[literal_count..])?;
    Ok((literal, distance))
}

fn extend_repeated(
    lengths: &mut Vec<u8>,
    value: u8,
    repeat: usize,
    limit: usize,
) -> CompressionResult<()> {
    let Some(new_len) = lengths.len().checked_add(repeat) else {
        return Err(malformed("repeated code lengths overflow usize"));
    };
    if new_len > limit {
        return Err(malformed("repeated code lengths exceed their table"));
    }
    lengths.resize(new_len, value);
    Ok(())
}

fn decode_compressed(
    bits: &mut BitReader<'_>,
    literal: &Huffman,
    distance: &Huffman,
    output: &mut Vec<u8>,
    max_output: usize,
) -> CompressionResult<DecodeStatus> {
    loop {
        match literal.decode(bits)? {
            byte @ 0..=255 => {
                if output.len() >= max_output {
                    return Ok(DecodeStatus::OutputFull);
                }
                output.push(byte.to_le_bytes()[0]);
            }
            256 => return Ok(DecodeStatus::Complete),
            symbol @ 257..=285 => {
                let length_index = usize::from(symbol.saturating_sub(257));
                let length = LENGTH_BASE[length_index]
                    .saturating_add(bounded_usize(bits.read(LENGTH_EXTRA[length_index])?));
                let distance_symbol = distance.decode(bits)?;
                if distance_symbol >= 30 {
                    return Err(malformed("distance symbol is reserved"));
                }
                let distance_index = usize::from(distance_symbol);
                let backwards = DISTANCE_BASE[distance_index]
                    .saturating_add(bounded_usize(bits.read(DISTANCE_EXTRA[distance_index])?));
                if backwards > output.len() {
                    return Err(malformed("back-reference precedes the inflated output"));
                }
                let Some(available) = max_output.checked_sub(output.len()) else {
                    return Err(malformed("inflated output exceeds its configured limit"));
                };
                let copied = length.min(available);
                for _ in 0..copied {
                    let source = output.len().saturating_sub(backwards);
                    output.push(output[source]);
                }
                if copied < length {
                    return Ok(DecodeStatus::OutputFull);
                }
            }
            _ => return Err(malformed("literal/length symbol is reserved")),
        }
    }
}

fn adler32(data: &[u8]) -> u32 {
    const MODULUS: u32 = 65_521;
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in data {
        a = a.saturating_add(u32::from(byte)).rem_euclid(MODULUS);
        b = b.saturating_add(a).rem_euclid(MODULUS);
    }
    b.wrapping_shl(16) | a
}

struct Huffman {
    entries: Vec<HuffmanEntry>,
    maximum_length: u8,
}

struct HuffmanEntry {
    reversed_code: u16,
    length: u8,
    symbol: u16,
}

impl Huffman {
    fn from_lengths(lengths: &[u8]) -> CompressionResult<Self> {
        let Some(maximum_length) = lengths.iter().copied().max() else {
            return Err(malformed("Huffman table is empty"));
        };
        if maximum_length == 0 {
            return Err(malformed("Huffman table has no symbols"));
        }

        let mut counts = [0u16; 16];
        for &length in lengths {
            if length != 0 {
                // Every production alphabet is bounded by the 288-symbol
                // literal/length alphabet, and oversized private inputs are
                // rejected below before entries are retained.
                counts[usize::from(length)] = counts[usize::from(length)].saturating_add(1);
            }
        }

        if lengths.len() > 288 {
            return Err(malformed("Huffman table exceeds the DEFLATE alphabet"));
        }

        let mut next_codes = [0u32; 16];
        let mut code = 0u32;
        for length in 1usize..=15 {
            code = code
                .saturating_add(u32::from(counts[length.saturating_sub(1)]))
                .wrapping_shl(1);
            next_codes[length] = code;
        }

        let mut entries = Vec::new();
        for (symbol, &length) in lengths.iter().enumerate() {
            if length == 0 {
                continue;
            }
            let canonical = next_codes[usize::from(length)];
            if canonical >= 1u32.wrapping_shl(u32::from(length)) {
                return Err(malformed("Huffman code lengths are oversubscribed"));
            }
            next_codes[usize::from(length)] = canonical.saturating_add(1);
            let canonical_bytes = canonical.to_le_bytes();
            entries.push(HuffmanEntry {
                reversed_code: reverse_low_bits(
                    u16::from_le_bytes([canonical_bytes[0], canonical_bytes[1]]),
                    length,
                ),
                length,
                // DEFLATE has at most 288 literal/length symbols.
                symbol: low_u16(symbol),
            });
        }
        Ok(Self {
            entries,
            maximum_length,
        })
    }

    fn decode(&self, bits: &mut BitReader<'_>) -> CompressionResult<u16> {
        let mut code = 0u16;
        for length in 1..=self.maximum_length {
            code |= low_u16(bounded_usize(bits.read(1)?))
                .wrapping_shl(u32::from(length.saturating_sub(1)));
            for entry in &self.entries {
                if entry.length == length && entry.reversed_code == code {
                    return Ok(entry.symbol);
                }
            }
        }
        Err(malformed("Huffman code is not present in the table"))
    }
}

fn reverse_low_bits(mut value: u16, width: u8) -> u16 {
    let mut reversed = 0u16;
    for _ in 0..width {
        reversed = reversed.wrapping_shl(1) | (value & 1);
        value = value.wrapping_shr(1);
    }
    reversed
}

struct BitReader<'a> {
    data: &'a [u8],
    bit_position: usize,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            bit_position: 0,
        }
    }

    fn read(&mut self, width: u8) -> CompressionResult<u32> {
        if width == 0 {
            return Ok(0);
        }
        let byte_position = self.bit_position.div_euclid(8);
        let bit_offset = self.bit_position.rem_euclid(8);
        // `width` is a byte, so this sum is at most 262 and cannot overflow.
        // Comparing the small required byte count avoids forming `len * 8`,
        // which can overflow on 32-bit WASM for otherwise valid large slices.
        let required_bytes = bit_offset.saturating_add(usize::from(width)).div_ceil(8);
        if self.data.len().saturating_sub(byte_position) < required_bytes {
            return Err(CodecError::NeedMore {
                minimum: self.data.len().saturating_add(1),
                message: "invalid compressed stream: DEFLATE bit read exceeds the input".to_owned(),
            });
        }
        let mut value = 0u32;
        for shift in 0..width {
            let byte = self.data[self.bit_position.div_euclid(8)];
            value |= u32::from(
                byte.wrapping_shr(self.bit_position.rem_euclid(8).to_le_bytes()[0].into()) & 1,
            )
            .wrapping_shl(u32::from(shift));
            self.bit_position = self.bit_position.saturating_add(1);
        }
        Ok(value)
    }

    fn align_to_byte(&mut self) {
        self.bit_position = self.bit_position.div_ceil(8).saturating_mul(8);
    }
}

fn bounded_usize(value: u32) -> usize {
    #[cfg(target_pointer_width = "64")]
    {
        let [a, b, c, d] = value.to_le_bytes();
        usize::from_le_bytes([a, b, c, d, 0, 0, 0, 0])
    }
    #[cfg(target_pointer_width = "32")]
    {
        usize::from_le_bytes(value.to_le_bytes())
    }
}

fn low_u16(value: usize) -> u16 {
    let [a, b, ..] = value.to_le_bytes();
    u16::from_le_bytes([a, b])
}
