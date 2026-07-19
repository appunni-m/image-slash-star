//! Baseline TIFF/BigTIFF-independent decoder for classic TIFF IFDs.

use crate::compression::deflate::decompress_zlib;
use crate::types::{ColorType, DecodedImage};

const COMPRESSION_NONE: u64 = 1;
const COMPRESSION_LZW: u64 = 5;
const COMPRESSION_DEFLATE: u64 = 8;
const COMPRESSION_PACKBITS: u64 = 32_773;
const COMPRESSION_ADOBE_DEFLATE: u64 = 32_946;

/// Decode the first IFD of a classic little- or big-endian TIFF stream.
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    let endian = match data.get(..2)? {
        b"II" => Endian::Little,
        b"MM" => Endian::Big,
        _ => return None,
    };
    if endian.u16(data.get(2..4)?)? != 42 {
        return None;
    }
    let ifd_offset = usize::try_from(endian.u32(data.get(4..8)?)?).ok()?;
    let directory = Directory::parse(data, ifd_offset, endian)?;

    let width = u32::try_from(directory.one(256)?).ok()?;
    let height = u32::try_from(directory.one(257)?).ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    let samples_per_pixel = usize::try_from(directory.one_or(277, 1)).ok()?;
    let bits = directory.values_or(258, &[1])?;
    if bits.is_empty() || bits.iter().any(|&value| value != bits[0]) {
        return None;
    }
    let bits_per_sample = u8::try_from(bits[0]).ok()?;
    let compression = directory.one_or(259, COMPRESSION_NONE);
    let photometric = directory.one_or(262, 1);
    let rows_per_strip = usize::try_from(directory.one_or(278, u64::from(height))).ok()?;
    let predictor = directory.one_or(317, 1);
    let planar = directory.one_or(284, 1);
    if rows_per_strip == 0 || planar != 1 || !matches!(predictor, 1 | 2) {
        return None;
    }

    let offsets = directory.values(273)?;
    let byte_counts = directory.values(279)?;
    if offsets.is_empty() || offsets.len() != byte_counts.len() {
        return None;
    }

    let width_usize = usize::try_from(width).ok()?;
    let height_usize = usize::try_from(height).ok()?;
    let row_bytes = width_usize
        .checked_mul(samples_per_pixel)?
        .checked_mul(usize::from(bits_per_sample))?
        .checked_add(7)?
        / 8;
    let expected_total = row_bytes.checked_mul(height_usize)?;
    let mut pixels = Vec::with_capacity(expected_total);

    for (strip_index, (&offset, &byte_count)) in offsets.iter().zip(&byte_counts).enumerate() {
        let start = usize::try_from(offset).ok()?;
        let count = usize::try_from(byte_count).ok()?;
        let encoded = data.get(start..start.checked_add(count)?)?;
        let first_row = strip_index.checked_mul(rows_per_strip)?;
        if first_row >= height_usize {
            return None;
        }
        let strip_rows = rows_per_strip.min(height_usize - first_row);
        let expected = row_bytes.checked_mul(strip_rows)?;
        let mut decoded = match compression {
            COMPRESSION_NONE => encoded.to_vec(),
            COMPRESSION_LZW => decode_lzw(encoded, expected)?,
            COMPRESSION_DEFLATE | COMPRESSION_ADOBE_DEFLATE => decompress_zlib(encoded, expected)?,
            COMPRESSION_PACKBITS => decode_packbits(encoded, expected)?,
            _ => return None,
        };
        if decoded.len() != expected {
            return None;
        }
        if predictor == 2 {
            reverse_horizontal_predictor(
                &mut decoded,
                row_bytes,
                samples_per_pixel,
                bits_per_sample,
                endian,
            )?;
        }
        pixels.extend_from_slice(&decoded);
    }
    if pixels.len() != expected_total {
        return None;
    }

    convert_pixels(
        width,
        height,
        pixels,
        photometric,
        samples_per_pixel,
        bits_per_sample,
        endian,
    )
}

fn convert_pixels(
    width: u32,
    height: u32,
    mut pixels: Vec<u8>,
    photometric: u64,
    samples: usize,
    bits: u8,
    endian: Endian,
) -> Option<DecodedImage> {
    match (photometric, samples, bits) {
        (0 | 1, 1, 1) => {
            if photometric == 0 {
                pixels.iter_mut().for_each(|byte| *byte = !*byte);
            }
            Some(DecodedImage::new(width, height, pixels, ColorType::L8))
        }
        (0 | 1, 1, 8) => {
            if photometric == 0 {
                pixels.iter_mut().for_each(|byte| *byte = !*byte);
            }
            Some(DecodedImage::new(width, height, pixels, ColorType::L8))
        }
        (0 | 1, 1, 16) => {
            let mut output = Vec::with_capacity(pixels.len() / 2);
            for bytes in pixels.chunks_exact(2) {
                let value = endian.u16(bytes)?;
                let high = (value >> 8) as u8;
                output.push(if photometric == 0 { !high } else { high });
            }
            Some(DecodedImage::new(width, height, output, ColorType::L8))
        }
        (2, 3, 8) => Some(DecodedImage::new(width, height, pixels, ColorType::Rgb8)),
        (2, 4, 8) => Some(DecodedImage::new(width, height, pixels, ColorType::Rgba8)),
        (3, 1, 1 | 2 | 4 | 8) => {
            let indices = unpack_indices(&pixels, width, height, bits)?;
            Some(DecodedImage::new(width, height, indices, ColorType::L8))
        }
        (5, 4, 8) => {
            let mut rgb = Vec::with_capacity(pixels.len() / 4 * 3);
            for cmyk in pixels.chunks_exact(4) {
                let black = 255u32 - u32::from(cmyk[3]);
                rgb.push(((255 - u32::from(cmyk[0])) * black / 255) as u8);
                rgb.push(((255 - u32::from(cmyk[1])) * black / 255) as u8);
                rgb.push(((255 - u32::from(cmyk[2])) * black / 255) as u8);
            }
            Some(DecodedImage::new(width, height, rgb, ColorType::Rgb8))
        }
        (6, 3, 8) => {
            let mut rgb = Vec::with_capacity(pixels.len());
            for ycbcr in pixels.chunks_exact(3) {
                let y = f32::from(ycbcr[0]);
                let cb = f32::from(ycbcr[1]) - 128.0;
                let cr = f32::from(ycbcr[2]) - 128.0;
                rgb.push((y + 1.402 * cr).round().clamp(0.0, 255.0) as u8);
                rgb.push(
                    (y - 0.344_136 * cb - 0.714_136 * cr)
                        .round()
                        .clamp(0.0, 255.0) as u8,
                );
                rgb.push((y + 1.772 * cb).round().clamp(0.0, 255.0) as u8);
            }
            Some(DecodedImage::new(width, height, rgb, ColorType::Rgb8))
        }
        _ => None,
    }
}

fn unpack_indices(data: &[u8], width: u32, height: u32, bits: u8) -> Option<Vec<u8>> {
    if bits == 8 {
        return Some(data.to_vec());
    }
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    let stride = width.checked_mul(usize::from(bits))?.div_ceil(8);
    let mut output = Vec::with_capacity(width.checked_mul(height)?);
    for y in 0..height {
        let row = data.get(y * stride..(y + 1) * stride)?;
        for x in 0..width {
            let bit = x.checked_mul(usize::from(bits))?;
            let shift = 8usize
                .checked_sub(usize::from(bits))?
                .checked_sub(bit % 8)?;
            output.push((row[bit / 8] >> shift) & ((1u8 << bits) - 1));
        }
    }
    Some(output)
}

fn reverse_horizontal_predictor(
    data: &mut [u8],
    row_bytes: usize,
    samples: usize,
    bits: u8,
    endian: Endian,
) -> Option<()> {
    match bits {
        8 => {
            for row in data.chunks_exact_mut(row_bytes) {
                for index in samples..row.len() {
                    row[index] = row[index].wrapping_add(row[index - samples]);
                }
            }
        }
        16 => {
            let sample_stride = samples.checked_mul(2)?;
            for row in data.chunks_exact_mut(row_bytes) {
                for offset in (sample_stride..row.len()).step_by(2) {
                    let previous =
                        endian.u16(row.get(offset - sample_stride..offset - sample_stride + 2)?)?;
                    let current = endian.u16(row.get(offset..offset + 2)?)?;
                    endian.write_u16(
                        current.wrapping_add(previous),
                        row.get_mut(offset..offset + 2)?,
                    )?;
                }
            }
        }
        _ => return None,
    }
    Some(())
}

fn decode_packbits(data: &[u8], expected: usize) -> Option<Vec<u8>> {
    let mut output = Vec::with_capacity(expected);
    let mut position = 0usize;
    while position < data.len() && output.len() < expected {
        let header = *data.get(position)? as i8;
        position += 1;
        match header {
            0..=127 => {
                let count = usize::from(header as u8) + 1;
                let end = position.checked_add(count)?;
                if output.len().checked_add(count)? > expected {
                    return None;
                }
                output.extend_from_slice(data.get(position..end)?);
                position = end;
            }
            -127..=-1 => {
                let count = usize::from((1i16 - i16::from(header)) as u16);
                let value = *data.get(position)?;
                position += 1;
                if output.len().checked_add(count)? > expected {
                    return None;
                }
                output.resize(output.len() + count, value);
            }
            -128 => {}
        }
    }
    (output.len() == expected).then_some(output)
}

fn decode_lzw(data: &[u8], expected: usize) -> Option<Vec<u8>> {
    const CLEAR: u16 = 256;
    const END: u16 = 257;
    const LIMIT: usize = 4096;
    let mut prefixes = [0u16; LIMIT];
    let mut suffixes = [0u8; LIMIT];
    for value in 0..256u16 {
        suffixes[usize::from(value)] = value as u8;
    }
    let mut stack = [0u8; LIMIT];
    let mut reader = MsbBits::new(data);
    let mut output = Vec::with_capacity(expected);
    let mut width = 9u8;
    let mut next_code = 258u16;
    let mut previous = None;

    while let Some(code) = reader.read(width) {
        if code == CLEAR {
            width = 9;
            next_code = 258;
            previous = None;
            continue;
        }
        if code == END {
            return (output.len() == expected).then_some(output);
        }
        let Some(old_code) = previous else {
            if code >= CLEAR || output.len() >= expected {
                return None;
            }
            output.push(code as u8);
            previous = Some(code);
            continue;
        };

        let first = if code < next_code {
            append_lzw(
                code,
                &prefixes,
                &suffixes,
                &mut stack,
                &mut output,
                expected,
            )?
        } else if code == next_code {
            let first = append_lzw(
                old_code,
                &prefixes,
                &suffixes,
                &mut stack,
                &mut output,
                expected,
            )?;
            if output.len() >= expected {
                return None;
            }
            output.push(first);
            first
        } else {
            return None;
        };

        if usize::from(next_code) < LIMIT {
            prefixes[usize::from(next_code)] = old_code;
            suffixes[usize::from(next_code)] = first;
            next_code += 1;
            if width < 12 && next_code == (1u16 << width) - 1 {
                width += 1;
            }
        }
        previous = Some(code);
    }
    None
}

fn append_lzw(
    mut code: u16,
    prefixes: &[u16; 4096],
    suffixes: &[u8; 4096],
    stack: &mut [u8; 4096],
    output: &mut Vec<u8>,
    expected: usize,
) -> Option<u8> {
    let mut count = 0usize;
    while code >= 256 {
        if usize::from(code) >= 4096 || count >= stack.len() {
            return None;
        }
        stack[count] = suffixes[usize::from(code)];
        count += 1;
        code = prefixes[usize::from(code)];
    }
    let first = code as u8;
    stack[count] = first;
    count += 1;
    if output.len().checked_add(count)? > expected {
        return None;
    }
    output.extend(stack[..count].iter().rev());
    Some(first)
}

struct MsbBits<'a> {
    data: &'a [u8],
    bit: usize,
}

impl<'a> MsbBits<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, bit: 0 }
    }

    fn read(&mut self, width: u8) -> Option<u16> {
        if self.bit.checked_add(usize::from(width))? > self.data.len().checked_mul(8)? {
            return None;
        }
        let mut value = 0u16;
        for _ in 0..width {
            value = (value << 1) | u16::from((data_bit(self.data, self.bit))?);
            self.bit += 1;
        }
        Some(value)
    }
}

fn data_bit(data: &[u8], bit: usize) -> Option<u8> {
    Some((data.get(bit / 8)? >> (7 - bit % 8)) & 1)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Endian {
    Little,
    Big,
}

impl Endian {
    fn u16(self, bytes: &[u8]) -> Option<u16> {
        let bytes: [u8; 2] = bytes.try_into().ok()?;
        Some(match self {
            Endian::Little => u16::from_le_bytes(bytes),
            Endian::Big => u16::from_be_bytes(bytes),
        })
    }

    fn u32(self, bytes: &[u8]) -> Option<u32> {
        let bytes: [u8; 4] = bytes.try_into().ok()?;
        Some(match self {
            Endian::Little => u32::from_le_bytes(bytes),
            Endian::Big => u32::from_be_bytes(bytes),
        })
    }

    fn write_u16(self, value: u16, destination: &mut [u8]) -> Option<()> {
        let bytes = match self {
            Endian::Little => value.to_le_bytes(),
            Endian::Big => value.to_be_bytes(),
        };
        destination.get_mut(..2)?.copy_from_slice(&bytes);
        Some(())
    }
}

struct Directory<'a> {
    data: &'a [u8],
    endian: Endian,
    entries: Vec<Entry>,
}

struct Entry {
    tag: u16,
    field_type: u16,
    count: usize,
    value_position: usize,
    inline_position: usize,
    byte_len: usize,
}

impl<'a> Directory<'a> {
    fn parse(data: &'a [u8], offset: usize, endian: Endian) -> Option<Self> {
        let count = usize::from(endian.u16(data.get(offset..offset.checked_add(2)?)?)?);
        if count > 4096 {
            return None;
        }
        let mut entries = Vec::with_capacity(count);
        for index in 0..count {
            let start = offset.checked_add(2)?.checked_add(index.checked_mul(12)?)?;
            let bytes = data.get(start..start.checked_add(12)?)?;
            let tag = endian.u16(&bytes[0..2])?;
            let field_type = endian.u16(&bytes[2..4])?;
            let value_count = usize::try_from(endian.u32(&bytes[4..8])?).ok()?;
            let type_size = match field_type {
                1 | 2 | 6 | 7 => 1,
                3 | 8 => 2,
                4 | 9 | 11 => 4,
                5 | 10 | 12 => 8,
                _ => return None,
            };
            let byte_len = value_count.checked_mul(type_size)?;
            let value_position = if byte_len <= 4 {
                start.checked_add(8)?
            } else {
                usize::try_from(endian.u32(&bytes[8..12])?).ok()?
            };
            data.get(value_position..value_position.checked_add(byte_len)?)?;
            entries.push(Entry {
                tag,
                field_type,
                count: value_count,
                value_position,
                inline_position: start + 8,
                byte_len,
            });
        }
        Some(Self {
            data,
            endian,
            entries,
        })
    }

    fn one(&self, tag: u16) -> Option<u64> {
        self.values(tag)?.into_iter().next()
    }

    fn one_or(&self, tag: u16, default: u64) -> u64 {
        self.one(tag).unwrap_or(default)
    }

    fn values_or(&self, tag: u16, default: &[u64]) -> Option<Vec<u64>> {
        self.values(tag).or_else(|| Some(default.to_vec()))
    }

    fn values(&self, tag: u16) -> Option<Vec<u64>> {
        let entry = self.entries.iter().find(|entry| entry.tag == tag)?;
        let position = if entry.byte_len <= 4 {
            entry.inline_position
        } else {
            entry.value_position
        };
        let bytes = self
            .data
            .get(position..position.checked_add(entry.byte_len)?)?;
        let mut values = Vec::with_capacity(entry.count);
        match entry.field_type {
            1 => values.extend(bytes.iter().map(|&value| u64::from(value))),
            3 => {
                for chunk in bytes.chunks_exact(2) {
                    values.push(u64::from(self.endian.u16(chunk)?));
                }
            }
            4 => {
                for chunk in bytes.chunks_exact(4) {
                    values.push(u64::from(self.endian.u32(chunk)?));
                }
            }
            _ => return None,
        }
        Some(values)
    }
}
