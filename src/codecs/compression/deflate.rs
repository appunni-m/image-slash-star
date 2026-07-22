//! RFC 1950 zlib wrapper and RFC 1951 DEFLATE implementation.

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

/// Inflate a zlib stream while enforcing an exact output-size ceiling.
pub(crate) fn decompress_zlib(data: &[u8], max_output: usize) -> Option<Vec<u8>> {
    decompress_zlib_with_limit(data, max_output, false)
}

/// Inflate the requested prefix of a zlib stream.
///
/// Pillow's PNG decoder stops once its scanline buffer is full, so extra
/// inflated bytes and the remainder of that zlib stream are deliberately
/// ignored. TIFF decoding continues to use [`decompress_zlib`] and validates
/// the complete stream.
pub(crate) fn decompress_zlib_prefix(data: &[u8], max_output: usize) -> Option<Vec<u8>> {
    decompress_zlib_with_limit(data, max_output, true)
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let _ = decompress_zlib(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00], 1);
    let _ = decompress_zlib(&[0x88, 0x00, 0x00, 0x00, 0x00, 0x00], 1);
    let _ = decompress_zlib(&[0x78, 0x00, 0x00, 0x00, 0x00, 0x00], 1);
    let _ = decompress_zlib(&[0x78, 0x20, 0x00, 0x00, 0x00, 0x00], 1);
    let _ = decompress_zlib(
        &[0x78, 0x01, 0x73, 0x04, 0x02, 0x00, 0x00, 0x00, 0x00, 0x01],
        8,
    );

    assert_eq!(compress_zlib_chunked(&[], 0, &[usize::MAX, 1]), None);
    assert_eq!(compress_zlib_chunked(&[], 0, &[1]), None);

    assert_eq!(write_stored_block(&mut Vec::new(), &[], true), Some(()));
    let oversized = vec![0; usize::from(u16::MAX) + 1];
    assert_eq!(write_stored_block(&mut Vec::new(), &oversized, false), None);

    let mut bits = BitReader::new(&[]);
    assert_eq!(bits.read(0), Some(0));
    assert_eq!(bits.read(1), None);

    let mut bits = BitReader::new(&[]);
    let mut output = Vec::new();
    let _ = decode_stored(&mut bits, &mut output, 0, false);

    let mut bits = BitReader::new(&[0, 0]);
    let mut output = Vec::new();
    let _ = decode_stored(&mut bits, &mut output, 0, false);

    let mut bits = BitReader::new(&[0, 0, 0xff, 0xff]);
    let mut output = vec![0];
    assert!(decode_stored(&mut bits, &mut output, 0, false).is_none());

    let mut bits = BitReader::new(&[1, 0, 0xfe, 0xff]);
    let mut output = Vec::new();
    let _ = decode_stored(&mut bits, &mut output, 0, true);

    let mut bits = BitReader::new(&[1, 0, 0xfe, 0xff]);
    let mut output = Vec::new();
    assert!(decode_stored(&mut bits, &mut output, 1, false).is_none());

    let mut repeated = vec![0];
    assert_eq!(
        extend_repeated(&mut repeated, 0, usize::MAX, usize::MAX),
        None
    );

    assert!(Huffman::from_lengths(&[]).is_none());
    assert!(Huffman::from_lengths(&[0]).is_none());
    let too_many_codes = vec![1u8; usize::from(u16::MAX) + 1];
    assert!(Huffman::from_lengths(&too_many_codes).is_none());
    let mut too_large_symbol = vec![0u8; usize::from(u16::MAX) + 2];
    *too_large_symbol
        .last_mut()
        .expect("coverage vector is non-empty") = 1;
    assert!(Huffman::from_lengths(&too_large_symbol).is_none());

    let single = Huffman::from_lengths(&[1]).expect("coverage huffman should build");
    let mut bits = BitReader::new(&[1]);
    assert_eq!(single.decode(&mut bits), None);

    let _ = decompress_zlib(&[0x78, 0x01, 0, 0, 0, 0], 1);
    let _ = compress_zlib_stored_chunked(&[], &[32_767, usize::MAX]);

    let mut bits = BitReader {
        data: &[0],
        bit_position: 7,
    };
    assert_eq!(bits.read(2), None);
    let mut bits = BitReader {
        data: &[],
        bit_position: usize::MAX,
    };
    assert_eq!(bits.read(1), None);

    let mut bits = BitReader::new(&[]);
    assert!(read_dynamic_tables(&mut bits).is_none());
    let mut bits = BitReader::new(&[0]);
    assert!(read_dynamic_tables(&mut bits).is_none());
    let mut bits = BitReader {
        data: &[0, 0],
        bit_position: 2,
    };
    assert!(read_dynamic_tables(&mut bits).is_none());
    let mut bits = BitReader::new(&[0, 0, 0, 0]);
    assert!(read_dynamic_tables(&mut bits).is_none());
    let mut bits = BitReader {
        data: &[0, 0],
        bit_position: 5,
    };
    assert!(read_dynamic_tables(&mut bits).is_none());

    let symbol_16 = huffman_with_symbol(16);
    let mut bits = BitReader::new(&[0]);
    assert!(read_dynamic_code_lengths(&mut bits, &symbol_16, 1).is_none());

    let zero_then_16 = huffman_with_symbols(&[(0, 1), (16, 1)]);
    let mut bits = BitReader {
        data: &[0b0100_0000],
        bit_position: 5,
    };
    assert!(read_dynamic_code_lengths(&mut bits, &zero_then_16, 4).is_none());
    let mut bits = BitReader::new(&[0b0000_0010]);
    assert!(read_dynamic_code_lengths(&mut bits, &zero_then_16, 2).is_none());

    let symbol_17 = huffman_with_symbol(17);
    let mut bits = BitReader {
        data: &[0],
        bit_position: 7,
    };
    assert!(read_dynamic_code_lengths(&mut bits, &symbol_17, 1).is_none());
    let mut bits = BitReader::new(&[0]);
    assert!(read_dynamic_code_lengths(&mut bits, &symbol_17, 1).is_none());

    let symbol_18 = huffman_with_symbol(18);
    let mut bits = BitReader {
        data: &[0],
        bit_position: 7,
    };
    assert!(read_dynamic_code_lengths(&mut bits, &symbol_18, 1).is_none());

    let zero_lengths = vec![0; 258];
    assert!(build_dynamic_tables(&zero_lengths, 257).is_none());
    let mut no_distance_lengths = vec![0; 258];
    no_distance_lengths[0] = 1;
    assert!(build_dynamic_tables(&no_distance_lengths, 257).is_none());

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
    let _ = decode_compressed(
        &mut bits,
        &literal_zero,
        &distance_zero,
        &mut output,
        0,
        true,
    );

    let mut bits = BitReader::new(&[0]);
    let mut output = Vec::new();
    assert!(
        decode_compressed(
            &mut bits,
            &literal_zero,
            &distance_zero,
            &mut output,
            0,
            false
        )
        .is_none()
    );

    let mut bits = BitReader::new(&[0]);
    let mut output = Vec::new();
    let _ = decode_compressed(
        &mut bits,
        &literal_end,
        &distance_zero,
        &mut output,
        1,
        false,
    );

    let mut bits = BitReader::new(&[0]);
    let mut output = Vec::new();
    assert!(
        decode_compressed(
            &mut bits,
            &literal_match,
            &distance_reserved,
            &mut output,
            8,
            false,
        )
        .is_none()
    );

    let mut bits = BitReader::new(&[0]);
    let mut output = Vec::new();
    assert!(
        decode_compressed(
            &mut bits,
            &literal_match,
            &distance_zero,
            &mut output,
            8,
            false,
        )
        .is_none()
    );

    let mut bits = BitReader::new(&[0]);
    let mut output = vec![7];
    let _ = decode_compressed(
        &mut bits,
        &literal_match,
        &distance_zero,
        &mut output,
        1,
        true,
    );

    let mut bits = BitReader::new(&[0]);
    let mut output = vec![7];
    assert!(
        decode_compressed(
            &mut bits,
            &literal_match,
            &distance_zero,
            &mut output,
            1,
            false,
        )
        .is_none()
    );

    let mut bits = BitReader {
        data: &[0],
        bit_position: 6,
    };
    let mut output = vec![7];
    assert!(
        decode_compressed(
            &mut bits,
            &literal_match,
            &distance_two_bit,
            &mut output,
            8,
            false,
        )
        .is_none()
    );

    let mut bits = BitReader::new(&[0]);
    let mut output = vec![7, 8];
    assert!(
        decode_compressed(
            &mut bits,
            &literal_match,
            &distance_zero,
            &mut output,
            1,
            false,
        )
        .is_none()
    );

    let mut bits = BitReader {
        data: &[0],
        bit_position: 7,
    };
    let mut output = Vec::new();
    assert!(
        decode_compressed(
            &mut bits,
            &literal_extra,
            &distance_zero,
            &mut output,
            8,
            false
        )
        .is_none()
    );

    let mut bits = BitReader {
        data: &[0],
        bit_position: 6,
    };
    let mut output = vec![7, 8, 9, 10];
    assert!(
        decode_compressed(
            &mut bits,
            &literal_match,
            &distance_extra,
            &mut output,
            16,
            false,
        )
        .is_none()
    );

    let mut overflowing_codes = vec![1u8; usize::from(u16::MAX)];
    overflowing_codes.extend_from_slice(&[2, 2]);
    assert!(Huffman::from_lengths(&overflowing_codes).is_none());
}

#[cfg(coverage)]
fn huffman_with_symbol(symbol: usize) -> Huffman {
    let mut lengths = vec![0; symbol + 1];
    lengths[symbol] = 1;
    Huffman::from_lengths(&lengths).expect("coverage huffman should build")
}

#[cfg(coverage)]
fn huffman_with_symbols(symbols: &[(usize, u8)]) -> Huffman {
    let max_symbol = symbols
        .iter()
        .map(|&(symbol, _)| symbol)
        .max()
        .expect("coverage huffman should have symbols");
    let mut lengths = vec![0; max_symbol + 1];
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

fn decompress_zlib_with_limit(
    data: &[u8],
    max_output: usize,
    allow_trailing_output: bool,
) -> Option<Vec<u8>> {
    if data.len() < 6 {
        return None;
    }
    let cmf = data[0];
    let flg = data[1];
    if cmf & 0x0f != 8
        || cmf >> 4 > 7
        || (u16::from(cmf) * 256 + u16::from(flg)) % 31 != 0
        || flg & 0x20 != 0
    {
        return None;
    }

    let payload_end = data.len() - 4;
    let mut bits = BitReader::new(&data[2..payload_end]);
    let mut output = Vec::with_capacity(max_output.min(64 * 1024));
    loop {
        let block_header = bits.read(3)?;
        let final_block = block_header & 1 != 0;
        let status = match block_header >> 1 {
            0 => decode_stored(&mut bits, &mut output, max_output, allow_trailing_output)?,
            1 => {
                let literal = fixed_literal_table();
                let distance =
                    Huffman::from_lengths(&[5; 32]).expect("fixed DEFLATE distance table is valid");
                decode_compressed(
                    &mut bits,
                    &literal,
                    &distance,
                    &mut output,
                    max_output,
                    allow_trailing_output,
                )?
            }
            2 => {
                let (literal, distance) = read_dynamic_tables(&mut bits)?;
                decode_compressed(
                    &mut bits,
                    &literal,
                    &distance,
                    &mut output,
                    max_output,
                    allow_trailing_output,
                )?
            }
            _ => return None,
        };
        if status == DecodeStatus::OutputFull {
            return Some(output);
        }
        if final_block {
            break;
        }
    }

    let trailer = &data[payload_end..];
    let expected = u32::from_be_bytes([trailer[0], trailer[1], trailer[2], trailer[3]]);
    (adler32(&output) == expected).then_some(output)
}

/// Compress TIFF scanlines with zlib-ng's default memLevel-eight buffer.
#[cfg(feature = "tiff")]
pub(crate) fn compress_zlib_tiff(data: &[u8], input_chunks: &[usize]) -> Option<Vec<u8>> {
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
) -> Option<Vec<u8>> {
    let input_len = input_chunks
        .iter()
        .try_fold(0usize, |total, &length| total.checked_add(length))?;
    if input_len != data.len() {
        return None;
    }
    match level {
        0 => compress_zlib_stored_chunked(data, input_chunks),
        1 => super::zlib_ng::compress_level1(data, input_chunks),
        2 => super::zlib_ng::compress_level2(data, input_chunks),
        3 => super::zlib_ng::compress_level3(data, input_chunks),
        4 => super::zlib_ng::compress_level4(data, input_chunks),
        5 => super::zlib_ng::compress_level5(data, input_chunks),
        6 => super::zlib_ng::compress_level6(data, input_chunks),
        7 => super::zlib_ng::compress_level7(data, input_chunks),
        8 => super::zlib_ng::compress_level8(data, input_chunks),
        9 => super::zlib_ng::compress_level9(data, input_chunks),
        _ => None,
    }
}

#[cfg(any(feature = "png", feature = "tiff"))]
fn compress_zlib_stored_chunked(data: &[u8], input_chunks: &[usize]) -> Option<Vec<u8>> {
    const MIN_BLOCK: usize = 32_768;
    const MAX_STORED: usize = u16::MAX as usize;

    let mut output = vec![0x78, 0x01];
    let mut pending_start = 0usize;
    let mut input_end = 0usize;
    for &input_len in input_chunks {
        input_end = input_end.checked_add(input_len)?;
        while input_end - pending_start >= MIN_BLOCK {
            let maximum_end = pending_start + MAX_STORED;
            let block_end = input_end.min(maximum_end);
            write_stored_block_bounded(&mut output, &data[pending_start..block_end], false);
            pending_start = block_end;
        }
    }
    write_stored_block_bounded(&mut output, &data[pending_start..], true);
    output.extend_from_slice(&adler32(data).to_be_bytes());
    Some(output)
}

#[cfg(all(coverage, any(feature = "png", feature = "tiff")))]
fn write_stored_block(output: &mut Vec<u8>, block: &[u8], final_block: bool) -> Option<()> {
    let len = u16::try_from(block.len()).ok()?;
    write_stored_block_with_len(output, block, final_block, len);
    Some(())
}

#[cfg(any(feature = "png", feature = "tiff"))]
fn write_stored_block_bounded(output: &mut Vec<u8>, block: &[u8], final_block: bool) {
    debug_assert!(u16::try_from(block.len()).is_ok());
    write_stored_block_with_len(output, block, final_block, block.len() as u16);
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
    allow_trailing_output: bool,
) -> Option<DecodeStatus> {
    bits.align_to_byte();
    let len = bits.read(16)? as u16;
    let complement = bits.read(16)? as u16;
    if len != !complement {
        return None;
    }
    let available = max_output.checked_sub(output.len())?;
    let copied = usize::from(len).min(available);
    for _ in 0..copied {
        output.push(bits.read(8)? as u8);
    }
    if copied < usize::from(len) {
        allow_trailing_output.then_some(DecodeStatus::OutputFull)
    } else {
        Some(DecodeStatus::Complete)
    }
}

fn fixed_literal_table() -> Huffman {
    let mut lengths = vec![0; 288];
    lengths[0..144].fill(8);
    lengths[144..256].fill(9);
    lengths[256..280].fill(7);
    lengths[280..288].fill(8);
    Huffman::from_lengths(&lengths).expect("fixed DEFLATE literal table is valid")
}

fn read_dynamic_tables(bits: &mut BitReader<'_>) -> Option<(Huffman, Huffman)> {
    let literal_count = bits.read(5)? as usize + 257;
    let distance_count = bits.read(5)? as usize + 1;
    let code_length_count = bits.read(4)? as usize + 4;
    let mut code_lengths = [0u8; 19];
    for &symbol in &CODE_LENGTH_ORDER[..code_length_count] {
        code_lengths[symbol] = bits.read(3)? as u8;
    }
    let code_length_table = Huffman::from_lengths(&code_lengths)?;

    let total = literal_count + distance_count;
    let lengths = read_dynamic_code_lengths(bits, &code_length_table, total)?;
    build_dynamic_tables(&lengths, literal_count)
}

fn read_dynamic_code_lengths(
    bits: &mut BitReader<'_>,
    code_length_table: &Huffman,
    total: usize,
) -> Option<Vec<u8>> {
    let mut lengths = Vec::with_capacity(total);
    while lengths.len() < total {
        let symbol = code_length_table.decode(bits)?;
        match symbol {
            symbol @ 0..=15 => lengths.push(symbol as u8),
            16 => {
                let previous = *lengths.last()?;
                let repeat = bits.read(2)? as usize + 3;
                extend_repeated(&mut lengths, previous, repeat, total)?;
            }
            17 => {
                let repeat = bits.read(3)? as usize + 3;
                extend_repeated(&mut lengths, 0, repeat, total)?;
            }
            _ => {
                // The code-length alphabet has exactly 19 symbols.
                debug_assert_eq!(symbol, 18);
                let repeat = bits.read(7)? as usize + 11;
                extend_repeated(&mut lengths, 0, repeat, total)?;
            }
        }
    }
    Some(lengths)
}

fn build_dynamic_tables(lengths: &[u8], literal_count: usize) -> Option<(Huffman, Huffman)> {
    let literal = Huffman::from_lengths(&lengths[..literal_count])?;
    let distance = Huffman::from_lengths(&lengths[literal_count..])?;
    Some((literal, distance))
}

fn extend_repeated(lengths: &mut Vec<u8>, value: u8, repeat: usize, limit: usize) -> Option<()> {
    if lengths.len().checked_add(repeat)? > limit {
        return None;
    }
    lengths.resize(lengths.len() + repeat, value);
    Some(())
}

fn decode_compressed(
    bits: &mut BitReader<'_>,
    literal: &Huffman,
    distance: &Huffman,
    output: &mut Vec<u8>,
    max_output: usize,
    allow_trailing_output: bool,
) -> Option<DecodeStatus> {
    loop {
        match literal.decode(bits)? {
            byte @ 0..=255 => {
                if output.len() >= max_output {
                    return allow_trailing_output.then_some(DecodeStatus::OutputFull);
                }
                output.push(byte as u8);
            }
            256 => return Some(DecodeStatus::Complete),
            symbol @ 257..=285 => {
                let length_index = usize::from(symbol - 257);
                let length =
                    LENGTH_BASE[length_index] + bits.read(LENGTH_EXTRA[length_index])? as usize;
                let distance_symbol = distance.decode(bits)?;
                if distance_symbol >= 30 {
                    return None;
                }
                let distance_index = usize::from(distance_symbol);
                let backwards = DISTANCE_BASE[distance_index]
                    + bits.read(DISTANCE_EXTRA[distance_index])? as usize;
                if backwards > output.len() {
                    return None;
                }
                let available = max_output.checked_sub(output.len())?;
                let copied = length.min(available);
                for _ in 0..copied {
                    let source = output.len() - backwards;
                    output.push(output[source]);
                }
                if copied < length {
                    return allow_trailing_output.then_some(DecodeStatus::OutputFull);
                }
            }
            _ => return None,
        }
    }
}

fn adler32(data: &[u8]) -> u32 {
    const MODULUS: u32 = 65_521;
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in data {
        a = (a + u32::from(byte)) % MODULUS;
        b = (b + a) % MODULUS;
    }
    (b << 16) | a
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
    fn from_lengths(lengths: &[u8]) -> Option<Self> {
        let maximum_length = lengths.iter().copied().max()?;
        if maximum_length == 0 {
            return None;
        }

        let mut counts = [0u16; 16];
        for &length in lengths {
            if length != 0 {
                counts[usize::from(length)] = counts[usize::from(length)].checked_add(1)?;
            }
        }

        let mut next_codes = [0u16; 16];
        let mut code = 0u16;
        for length in 1..=15 {
            code = code.checked_add(counts[length - 1])? << 1;
            next_codes[length] = code;
        }

        let mut entries = Vec::new();
        for (symbol, &length) in lengths.iter().enumerate() {
            if length == 0 {
                continue;
            }
            let canonical = next_codes[usize::from(length)];
            if canonical >= (1u16 << length) {
                return None;
            }
            next_codes[usize::from(length)] = canonical + 1;
            entries.push(HuffmanEntry {
                reversed_code: reverse_low_bits(canonical, length),
                length,
                symbol: u16::try_from(symbol).ok()?,
            });
        }
        Some(Self {
            entries,
            maximum_length,
        })
    }

    fn decode(&self, bits: &mut BitReader<'_>) -> Option<u16> {
        let mut code = 0u16;
        for length in 1..=self.maximum_length {
            code |= (bits.read(1)? as u16) << (length - 1);
            if let Some(entry) = self
                .entries
                .iter()
                .find(|entry| entry.length == length && entry.reversed_code == code)
            {
                return Some(entry.symbol);
            }
        }
        None
    }
}

fn reverse_low_bits(mut value: u16, width: u8) -> u16 {
    let mut reversed = 0u16;
    for _ in 0..width {
        reversed = (reversed << 1) | (value & 1);
        value >>= 1;
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

    fn read(&mut self, width: u8) -> Option<u32> {
        if width == 0 {
            return Some(0);
        }
        let end = self.bit_position.checked_add(usize::from(width))?;
        #[cfg(target_pointer_width = "64")]
        let bit_len = self.data.len() * 8;
        #[cfg(not(target_pointer_width = "64"))]
        let bit_len = self.data.len().checked_mul(8)?;
        if end > bit_len {
            return None;
        }
        let mut value = 0u32;
        for shift in 0..width {
            let byte = self.data[self.bit_position / 8];
            value |= u32::from((byte >> (self.bit_position % 8)) & 1) << shift;
            self.bit_position += 1;
        }
        Some(value)
    }

    fn align_to_byte(&mut self) {
        self.bit_position = self.bit_position.div_ceil(8) * 8;
    }
}
