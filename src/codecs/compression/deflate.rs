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

    let payload_end = data.len().checked_sub(4)?;
    let mut bits = BitReader::new(data.get(2..payload_end)?);
    let mut output = Vec::with_capacity(max_output.min(64 * 1024));
    loop {
        let final_block = bits.read(1)? != 0;
        match bits.read(2)? {
            0 => decode_stored(&mut bits, &mut output, max_output)?,
            1 => {
                let literal = fixed_literal_table()?;
                let distance = Huffman::from_lengths(&[5; 32])?;
                decode_compressed(&mut bits, &literal, &distance, &mut output, max_output)?;
            }
            2 => {
                let (literal, distance) = read_dynamic_tables(&mut bits)?;
                decode_compressed(&mut bits, &literal, &distance, &mut output, max_output)?;
            }
            _ => return None,
        }
        if final_block {
            break;
        }
    }

    let expected = u32::from_be_bytes(data.get(payload_end..)?.try_into().ok()?);
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
    if level > 9 {
        return None;
    }
    let input_len = input_chunks
        .iter()
        .try_fold(0usize, |total, &length| total.checked_add(length))?;
    if input_len != data.len() {
        return None;
    }
    if level == 0 {
        return compress_zlib_stored_chunked(data, input_chunks);
    }
    if level == 3 {
        return super::zlib_ng::compress_level3(data, input_chunks);
    }
    if level == 6 {
        return super::zlib_ng::compress_level6(data, input_chunks);
    }

    let mut output = zlib_header(level);
    let mut writer = DeflateWriter::default();
    writer.write_bits(1, 1); // BFINAL
    writer.write_bits(1, 2); // BTYPE=fixed Huffman (LSB first: 01)
    encode_fixed_payload(data, level, &mut writer)?;
    write_fixed_symbol(&mut writer, 256);
    output.extend_from_slice(&writer.finish());
    output.extend_from_slice(&adler32(data).to_be_bytes());
    Some(output)
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
        while input_end.checked_sub(pending_start)? >= MIN_BLOCK {
            let maximum_end = pending_start.checked_add(MAX_STORED)?;
            let block_end = input_end.min(maximum_end);
            write_stored_block(&mut output, data.get(pending_start..block_end)?, false)?;
            pending_start = block_end;
        }
    }
    write_stored_block(&mut output, data.get(pending_start..)?, true)?;
    output.extend_from_slice(&adler32(data).to_be_bytes());
    Some(output)
}

#[cfg(any(feature = "png", feature = "tiff"))]
fn write_stored_block(output: &mut Vec<u8>, block: &[u8], final_block: bool) -> Option<()> {
    output.push(u8::from(final_block));
    let len = u16::try_from(block.len()).ok()?;
    output.extend_from_slice(&len.to_le_bytes());
    output.extend_from_slice(&(!len).to_le_bytes());
    output.extend_from_slice(block);
    Some(())
}

#[cfg(any(feature = "png", feature = "tiff"))]
fn zlib_header(level: u8) -> Vec<u8> {
    let cmf = 0x78u8;
    let compression_class = match level {
        0..=1 => 0u8,
        2..=5 => 1,
        6..=7 => 2,
        _ => 3,
    };
    let mut flg = compression_class << 6;
    let remainder = (u16::from(cmf) * 256 + u16::from(flg)) % 31;
    if remainder != 0 {
        flg = flg.wrapping_add(u8::try_from(31 - remainder).unwrap_or(0));
    }
    vec![cmf, flg]
}

#[cfg(any(feature = "png", feature = "tiff"))]
fn encode_fixed_payload(data: &[u8], level: u8, writer: &mut DeflateWriter) -> Option<()> {
    const HASH_SIZE: usize = 1 << 16;
    const WINDOW: usize = 32_768;
    let max_match = match level {
        1..=2 => 32,
        3..=5 => 96,
        _ => 258,
    };
    let mut previous = vec![usize::MAX; HASH_SIZE];
    let mut position = 0usize;
    while position < data.len() {
        let candidate = hash_at(data, position).map(|hash| previous[hash]);
        let match_len = candidate
            .filter(|&start| start != usize::MAX && position - start <= WINDOW)
            .map_or(0, |start| match_length(data, start, position, max_match));

        if match_len >= 3 {
            let distance = position.checked_sub(candidate?)?;
            write_length_distance(writer, match_len, distance)?;
            let end = position.checked_add(match_len)?;
            while position < end {
                if let Some(hash) = hash_at(data, position) {
                    previous[hash] = position;
                }
                position += 1;
            }
        } else {
            write_fixed_symbol(writer, u16::from(data[position]));
            if let Some(hash) = hash_at(data, position) {
                previous[hash] = position;
            }
            position += 1;
        }
    }
    Some(())
}

#[cfg(any(feature = "png", feature = "tiff"))]
fn hash_at(data: &[u8], position: usize) -> Option<usize> {
    let bytes = data.get(position..position.checked_add(3)?)?;
    Some(
        ((usize::from(bytes[0]) * 251 + usize::from(bytes[1])) * 251 + usize::from(bytes[2]))
            & 0xffff,
    )
}

#[cfg(any(feature = "png", feature = "tiff"))]
fn match_length(data: &[u8], left: usize, right: usize, maximum: usize) -> usize {
    let available = data.len().saturating_sub(right).min(maximum);
    let mut length = 0usize;
    while length < available && data[left + length] == data[right + length] {
        length += 1;
    }
    length
}

#[cfg(any(feature = "png", feature = "tiff"))]
fn write_length_distance(writer: &mut DeflateWriter, length: usize, distance: usize) -> Option<()> {
    let length_index = LENGTH_BASE
        .iter()
        .enumerate()
        .rev()
        .find(|&(_, &base)| length >= base)?
        .0;
    let length_symbol = 257u16.checked_add(u16::try_from(length_index).ok()?)?;
    write_fixed_symbol(writer, length_symbol);
    let length_extra = LENGTH_EXTRA[length_index];
    writer.write_bits(
        u32::try_from(length.checked_sub(LENGTH_BASE[length_index])?).ok()?,
        length_extra,
    );

    let distance_index = DISTANCE_BASE
        .iter()
        .enumerate()
        .rev()
        .find(|&(_, &base)| distance >= base)?
        .0;
    writer.write_bits(
        reverse_low_bits(u16::try_from(distance_index).ok()?, 5).into(),
        5,
    );
    let distance_extra = DISTANCE_EXTRA[distance_index];
    writer.write_bits(
        u32::try_from(distance.checked_sub(DISTANCE_BASE[distance_index])?).ok()?,
        distance_extra,
    );
    Some(())
}

#[cfg(any(feature = "png", feature = "tiff"))]
fn write_fixed_symbol(writer: &mut DeflateWriter, symbol: u16) {
    let (canonical, width) = match symbol {
        0..=143 => (0x30 + symbol, 8),
        144..=255 => (0x190 + symbol - 144, 9),
        256..=279 => (symbol - 256, 7),
        280..=287 => (0xc0 + symbol - 280, 8),
        _ => return,
    };
    writer.write_bits(u32::from(reverse_low_bits(canonical, width)), width);
}

#[cfg(any(feature = "png", feature = "tiff"))]
#[derive(Default)]
struct DeflateWriter {
    bytes: Vec<u8>,
    current: u8,
    used: u8,
}

#[cfg(any(feature = "png", feature = "tiff"))]
impl DeflateWriter {
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

fn decode_stored(bits: &mut BitReader<'_>, output: &mut Vec<u8>, max_output: usize) -> Option<()> {
    bits.align_to_byte();
    let len = bits.read(16)? as u16;
    let complement = bits.read(16)? as u16;
    if len != !complement || output.len().checked_add(usize::from(len))? > max_output {
        return None;
    }
    for _ in 0..len {
        output.push(bits.read(8)? as u8);
    }
    Some(())
}

fn fixed_literal_table() -> Option<Huffman> {
    let mut lengths = vec![0; 288];
    lengths[0..144].fill(8);
    lengths[144..256].fill(9);
    lengths[256..280].fill(7);
    lengths[280..288].fill(8);
    Huffman::from_lengths(&lengths)
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

    let total = literal_count.checked_add(distance_count)?;
    let mut lengths = Vec::with_capacity(total);
    while lengths.len() < total {
        match code_length_table.decode(bits)? {
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
            18 => {
                let repeat = bits.read(7)? as usize + 11;
                extend_repeated(&mut lengths, 0, repeat, total)?;
            }
            _ => return None,
        }
    }

    let literal = Huffman::from_lengths(lengths.get(..literal_count)?)?;
    let distance = Huffman::from_lengths(lengths.get(literal_count..)?)?;
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
) -> Option<()> {
    loop {
        match literal.decode(bits)? {
            byte @ 0..=255 => {
                if output.len() >= max_output {
                    return None;
                }
                output.push(byte as u8);
            }
            256 => return Some(()),
            symbol @ 257..=285 => {
                let length_index = usize::from(symbol - 257);
                let length = LENGTH_BASE[length_index]
                    .checked_add(bits.read(LENGTH_EXTRA[length_index])? as usize)?;
                let distance_symbol = distance.decode(bits)?;
                if distance_symbol >= 30 {
                    return None;
                }
                let distance_index = usize::from(distance_symbol);
                let backwards = DISTANCE_BASE[distance_index]
                    .checked_add(bits.read(DISTANCE_EXTRA[distance_index])? as usize)?;
                if backwards == 0
                    || backwards > output.len()
                    || output.len().checked_add(length)? > max_output
                {
                    return None;
                }
                for _ in 0..length {
                    let source = output.len().checked_sub(backwards)?;
                    output.push(*output.get(source)?);
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
        if maximum_length == 0 || maximum_length > 15 {
            return None;
        }

        let mut counts = [0u16; 16];
        for &length in lengths {
            if length > 15 {
                return None;
            }
            if length != 0 {
                counts[usize::from(length)] = counts[usize::from(length)].checked_add(1)?;
            }
        }

        let mut next_codes = [0u16; 16];
        let mut code = 0u16;
        for length in 1..=15 {
            code = code.checked_add(counts[length - 1])?.checked_shl(1)?;
            next_codes[length] = code;
        }

        let mut entries = Vec::new();
        for (symbol, &length) in lengths.iter().enumerate() {
            if length == 0 {
                continue;
            }
            let canonical = next_codes[usize::from(length)];
            next_codes[usize::from(length)] = canonical.checked_add(1)?;
            if canonical >= (1u16 << length) {
                return None;
            }
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
        if end > self.data.len().checked_mul(8)? {
            return None;
        }
        let mut value = 0u32;
        for shift in 0..width {
            let byte = *self.data.get(self.bit_position / 8)?;
            value |= u32::from((byte >> (self.bit_position % 8)) & 1) << shift;
            self.bit_position += 1;
        }
        Some(value)
    }

    fn align_to_byte(&mut self) {
        self.bit_position = self.bit_position.div_ceil(8) * 8;
    }
}
