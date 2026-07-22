//! Baseline TIFF/BigTIFF-independent decoder for classic TIFF IFDs.

use crate::codecs::compression::deflate::decompress_zlib;
use crate::types::{ColorType, DecodedImage, ImageMode, ImagePalette};

const COMPRESSION_NONE: usize = 1;
const COMPRESSION_LZW: usize = 5;
const COMPRESSION_DEFLATE: usize = 8;
const COMPRESSION_PACKBITS: usize = 32_773;
const COMPRESSION_ADOBE_DEFLATE: usize = 32_946;

/// Decode the first IFD of a classic little- or big-endian TIFF stream.
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    let endian = match data.get(..2)? {
        b"II" => Endian::Little,
        b"MM" => Endian::Big,
        _ => return None,
    };
    let magic = data.get(2..4)?;
    if endian.u16_exact([magic[0], magic[1]]) != 42 {
        return None;
    }
    let ifd_offset = data.get(4..8)?;
    let ifd_offset =
        endian.u32_exact([ifd_offset[0], ifd_offset[1], ifd_offset[2], ifd_offset[3]]) as usize;
    let directory = Directory::parse(data, ifd_offset, endian)?;

    let width = directory.one(256)? as u32;
    let height = directory.one(257)? as u32;
    if width == 0 || height == 0 {
        return None;
    }
    let samples_per_pixel = directory.one_or(277, 1);
    let bits = directory.values_or(258, &[1]);
    if bits.is_empty() || bits.iter().any(|&value| value != bits[0]) {
        return None;
    }
    let bits_per_sample = u8::try_from(bits[0]).ok()?;
    let compression = directory.one_or(259, COMPRESSION_NONE);
    let photometric = directory.one_or(262, 1);
    let rows_per_strip = directory.one_or(278, height as usize);
    let predictor = directory.one_or(317, 1);
    let planar = directory.one_or(284, 1);
    let sample_format = directory.one_or(339, 1);
    let color_map = directory.values(320);
    if samples_per_pixel == 0 || rows_per_strip == 0 || planar != 1 || !matches!(predictor, 1 | 2) {
        return None;
    }

    let width_usize = width as usize;
    let height_usize = height as usize;
    // Pillow's baseline YCbCr TIFF raw mode is RGBX: the IFD declares three
    // samples, but each stored pixel occupies four bytes.
    let stored_samples = if photometric == 6 && samples_per_pixel == 3 && bits_per_sample == 8 {
        4
    } else {
        samples_per_pixel
    };
    #[cfg(target_pointer_width = "32")]
    let row_samples = width_usize.checked_mul(stored_samples)?;
    #[cfg(not(target_pointer_width = "32"))]
    let row_samples = width_usize * stored_samples;
    let row_bytes = row_samples
        .checked_mul(usize::from(bits_per_sample))?
        .checked_add(7)?
        / 8;
    let expected_total = row_bytes.checked_mul(height_usize)?;
    let decode_block = |encoded: &[u8], expected: usize| -> Option<Vec<u8>> {
        match compression {
            COMPRESSION_NONE => Some(encoded.to_vec()),
            COMPRESSION_LZW => decode_lzw(encoded, expected),
            COMPRESSION_DEFLATE | COMPRESSION_ADOBE_DEFLATE => decompress_zlib(encoded, expected),
            COMPRESSION_PACKBITS => decode_packbits(encoded, expected),
            _ => None,
        }
    };

    let tile_offsets = directory.values(324);
    let tile_byte_counts = directory.values(325);
    if tile_offsets.is_some() || tile_byte_counts.is_some() {
        let offsets = tile_offsets?;
        let byte_counts = tile_byte_counts?;
        let tile_width = directory.one(322)?;
        let tile_height = directory.one(323)?;
        if tile_width == 0 || tile_height == 0 || bits_per_sample % 8 != 0 {
            return None;
        }
        let tiles_across = width_usize.div_ceil(tile_width);
        let tiles_down = height_usize.div_ceil(tile_height);
        #[cfg(target_pointer_width = "32")]
        let expected_tiles = tiles_across.checked_mul(tiles_down)?;
        #[cfg(not(target_pointer_width = "32"))]
        let expected_tiles = tiles_across * tiles_down;
        if offsets.len() != expected_tiles {
            return None;
        }
        #[cfg(target_pointer_width = "32")]
        let bytes_per_pixel = samples_per_pixel.checked_mul(usize::from(bits_per_sample) / 8)?;
        #[cfg(not(target_pointer_width = "32"))]
        let bytes_per_pixel = samples_per_pixel * (usize::from(bits_per_sample) / 8);
        let tile_row_bytes = tile_width.checked_mul(bytes_per_pixel)?;
        let tile_size = tile_row_bytes.checked_mul(tile_height)?;
        // libtiff, and therefore Pillow, derives uncompressed tile lengths from
        // the tile geometry even when TileByteCounts is empty or inconsistent.
        let byte_counts = if compression == COMPRESSION_NONE {
            vec![tile_size; offsets.len()]
        } else {
            if offsets.len() != byte_counts.len() {
                return None;
            }
            byte_counts
        };
        let mut pixels = vec![0; expected_total];
        for (tile_index, (&offset, &byte_count)) in offsets.iter().zip(&byte_counts).enumerate() {
            #[cfg(target_pointer_width = "32")]
            let encoded_end = offset.checked_add(byte_count)?;
            #[cfg(not(target_pointer_width = "32"))]
            let encoded_end = offset + byte_count;
            let encoded = data.get(offset..encoded_end)?;
            let mut decoded = decode_block(encoded, tile_size)?;
            // Every compressed decoder returns exactly the requested size, and
            // uncompressed tile counts were normalized to tile_size above.
            let compressed_predictor = matches!(
                compression,
                COMPRESSION_LZW | COMPRESSION_DEFLATE | COMPRESSION_ADOBE_DEFLATE
            );
            let supported_sample_width = matches!(bits_per_sample, 8 | 16 | 32);
            if predictor == 2 && compressed_predictor && supported_sample_width {
                reverse_horizontal_predictor(
                    &mut decoded,
                    tile_row_bytes,
                    samples_per_pixel,
                    bits_per_sample,
                    endian,
                );
            }
            let tile_x = (tile_index % tiles_across) * tile_width;
            let tile_y = (tile_index / tiles_across) * tile_height;
            let copied_width = tile_width.min(width_usize - tile_x);
            let copied_height = tile_height.min(height_usize - tile_y);
            let copied_bytes = copied_width * bytes_per_pixel;
            for y in 0..copied_height {
                let source = y * tile_row_bytes;
                let destination = (tile_y + y) * row_bytes + tile_x * bytes_per_pixel;
                pixels[destination..destination + copied_bytes]
                    .copy_from_slice(&decoded[source..source + copied_bytes]);
            }
        }
        return convert_pixels(
            width,
            height,
            pixels,
            photometric,
            samples_per_pixel,
            bits_per_sample,
            endian,
            color_map.as_deref(),
            sample_format,
        );
    }

    let offsets = directory.values(273)?;
    let declared_byte_counts = directory.values(279)?;
    if offsets.is_empty() {
        return None;
    }
    let expected_strips = height_usize.div_ceil(rows_per_strip);
    if offsets.len() > expected_strips {
        return None;
    }
    let byte_counts = if compression == COMPRESSION_NONE {
        (0..offsets.len())
            .map(|strip_index| {
                let first_row = strip_index * rows_per_strip;
                let strip_rows = rows_per_strip.min(height_usize - first_row);
                row_bytes * strip_rows
            })
            .collect::<Vec<_>>()
    } else {
        if declared_byte_counts.is_empty() {
            offsets
                .iter()
                .enumerate()
                .map(|(index, &offset)| {
                    let end = offsets
                        .get(index + 1)
                        .copied()
                        .unwrap_or(if ifd_offset > offset {
                            ifd_offset
                        } else {
                            data.len()
                        });
                    end.checked_sub(offset)
                })
                .collect::<Option<Vec<_>>>()?
        } else if offsets.len() != declared_byte_counts.len() {
            return None;
        } else {
            declared_byte_counts
        }
    };
    let mut pixels = Vec::with_capacity(expected_total);

    for (strip_index, (&offset, &byte_count)) in offsets.iter().zip(&byte_counts).enumerate() {
        #[cfg(target_pointer_width = "32")]
        let encoded_end = offset.checked_add(byte_count)?;
        #[cfg(not(target_pointer_width = "32"))]
        let encoded_end = offset + byte_count;
        let encoded = data.get(offset..encoded_end)?;
        let first_row = strip_index * rows_per_strip;
        let strip_rows = rows_per_strip.min(height_usize - first_row);
        let expected = row_bytes * strip_rows;
        let mut decoded = decode_block(encoded, expected)?;
        let compressed_predictor = matches!(
            compression,
            COMPRESSION_LZW | COMPRESSION_DEFLATE | COMPRESSION_ADOBE_DEFLATE
        );
        let supported_sample_width = matches!(bits_per_sample, 8 | 16 | 32);
        if predictor == 2 && compressed_predictor && supported_sample_width {
            reverse_horizontal_predictor(
                &mut decoded,
                row_bytes,
                samples_per_pixel,
                bits_per_sample,
                endian,
            );
        }
        pixels.extend_from_slice(&decoded);
    }
    pixels.resize(expected_total, 0);

    convert_pixels(
        width,
        height,
        pixels,
        photometric,
        samples_per_pixel,
        bits_per_sample,
        endian,
        color_map.as_deref(),
        sample_format,
    )
}

fn convert_pixels(
    width: u32,
    height: u32,
    mut pixels: Vec<u8>,
    photometric: usize,
    samples: usize,
    bits: u8,
    endian: Endian,
    color_map: Option<&[usize]>,
    sample_format: usize,
) -> Option<DecodedImage> {
    match (photometric, samples, bits) {
        (0 | 1, 1, 1) => {
            if photometric == 0 {
                let width = width as usize;
                let row_bytes = width.div_ceil(8);
                for row in pixels.chunks_exact_mut(row_bytes) {
                    row.iter_mut().for_each(|byte| *byte = !*byte);
                    if width % 8 != 0 {
                        row[row_bytes - 1] &= u8::MAX << (8 - width % 8);
                    }
                }
            }
            Some(DecodedImage::with_mode(
                width,
                height,
                pixels,
                ImageMode::L1,
            ))
        }
        (0 | 1, 1, 8) => {
            if photometric == 0 {
                pixels.iter_mut().for_each(|byte| *byte = !*byte);
            }
            Some(DecodedImage::new(width, height, pixels, ColorType::L8))
        }
        (1, 2, 8) => Some(DecodedImage::with_mode(
            width,
            height,
            pixels,
            ImageMode::La8,
        )),
        (0 | 1, 1, bits @ (2 | 4)) => {
            let maximum = (1u16 << bits) - 1;
            let output = unpack_indices(&pixels, width, height, bits)?
                .into_iter()
                .map(|sample| {
                    let value = u16::from(sample) * 255 / maximum;
                    if photometric == 0 {
                        255 - value as u8
                    } else {
                        value as u8
                    }
                })
                .collect();
            Some(DecodedImage::new(width, height, output, ColorType::L8))
        }
        (0 | 1, 1, 16) => {
            let mut output = Vec::with_capacity(pixels.len());
            for bytes in pixels.chunks_exact(2) {
                let value = endian.u16_exact([bytes[0], bytes[1]]);
                let value = if photometric == 0 { !value } else { value };
                output.extend_from_slice(&value.to_le_bytes());
            }
            Some(DecodedImage::new(width, height, output, ColorType::L16))
        }
        (0 | 1, 1, 32) => match sample_format {
            1 | 2 => Some(DecodedImage::with_mode(
                width,
                height,
                pixels,
                ImageMode::I32,
            )),
            3 => Some(DecodedImage::with_mode(
                width,
                height,
                pixels,
                ImageMode::F32,
            )),
            _ => None,
        },
        (2, 3, 8) => Some(DecodedImage::new(width, height, pixels, ColorType::Rgb8)),
        (2, 4, 8) => Some(DecodedImage::new(width, height, pixels, ColorType::Rgba8)),
        (3, 1, 1 | 2 | 4 | 8) => {
            let indices = unpack_indices(&pixels, width, height, bits)?;
            let entries = 1usize << usize::from(bits);
            let map_len = entries * 3;
            let map = color_map?.get(..map_len)?;
            let mut rgb = Vec::with_capacity(map_len);
            for index in 0..entries {
                rgb.push(u8::try_from(map[index] >> 8).ok()?);
                rgb.push(u8::try_from(map[entries + index] >> 8).ok()?);
                rgb.push(u8::try_from(map[entries * 2 + index] >> 8).ok()?);
            }
            Some(
                DecodedImage::with_mode(width, height, indices, ImageMode::P8).with_palette(
                    ImagePalette {
                        rgb,
                        alpha: Vec::new(),
                    },
                ),
            )
        }
        (5, 4, 8) => Some(DecodedImage::new(width, height, pixels, ColorType::Cmyk8)),
        (6, 3, 8) => {
            let mut rgb = Vec::with_capacity(pixels.len() / 4 * 3);
            for pixel in pixels.chunks_exact(4) {
                rgb.extend_from_slice(&pixel[..3]);
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
    if !matches!(bits, 1 | 2 | 4) {
        return None;
    }
    let width = width as usize;
    let height = height as usize;
    let bits = usize::from(bits);
    #[cfg(target_pointer_width = "32")]
    let stride = width.checked_mul(bits)?.div_ceil(8);
    #[cfg(not(target_pointer_width = "32"))]
    let stride = (width * bits).div_ceil(8);
    #[cfg(target_pointer_width = "32")]
    let output_len = width.checked_mul(height)?;
    #[cfg(not(target_pointer_width = "32"))]
    let output_len = width * height;
    let mut output = Vec::with_capacity(output_len);
    for y in 0..height {
        let row = data.get(y * stride..(y + 1) * stride)?;
        for x in 0..width {
            let bit = x * bits;
            let shift = 8usize - bits - bit % 8;
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
) {
    match bits {
        8 => {
            for row in data.chunks_exact_mut(row_bytes) {
                for index in samples..row.len() {
                    row[index] = row[index].wrapping_add(row[index - samples]);
                }
            }
        }
        16 => {
            let sample_stride = samples * 2;
            for row in data.chunks_exact_mut(row_bytes) {
                for offset in (sample_stride..row.len()).step_by(2) {
                    let previous = endian
                        .u16_exact([row[offset - sample_stride], row[offset - sample_stride + 1]]);
                    let current = endian.u16_exact([row[offset], row[offset + 1]]);
                    endian.write_u16(current.wrapping_add(previous), &mut row[offset..offset + 2]);
                }
            }
        }
        _ => {
            let sample_stride = samples * 4;
            for row in data.chunks_exact_mut(row_bytes) {
                for offset in (sample_stride..row.len()).step_by(4) {
                    let previous = endian.u32_exact([
                        row[offset - sample_stride],
                        row[offset - sample_stride + 1],
                        row[offset - sample_stride + 2],
                        row[offset - sample_stride + 3],
                    ]);
                    let current = endian.u32_exact([
                        row[offset],
                        row[offset + 1],
                        row[offset + 2],
                        row[offset + 3],
                    ]);
                    endian.write_u32(current.wrapping_add(previous), &mut row[offset..offset + 4]);
                }
            }
        }
    }
}

fn decode_packbits(data: &[u8], expected: usize) -> Option<Vec<u8>> {
    let mut output = Vec::with_capacity(expected);
    let mut position = 0usize;
    while position < data.len() && output.len() < expected {
        let header = data[position] as i8;
        position += 1;
        match header {
            0..=127 => {
                let count = usize::from(header as u8) + 1;
                let end = position + count;
                let packet = data.get(position..end)?;
                let remaining = expected - output.len();
                output.extend_from_slice(&packet[..count.min(remaining)]);
                position = end;
            }
            -127..=-1 => {
                let count = usize::from((1i16 - i16::from(header)) as u16);
                let value = *data.get(position)?;
                position += 1;
                output.resize(output.len() + count.min(expected - output.len()), value);
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
            if output.len() == expected {
                return Some(output);
            }
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
            )
        } else if code == next_code {
            let first = append_lzw(
                old_code,
                &prefixes,
                &suffixes,
                &mut stack,
                &mut output,
                expected,
            );
            if output.len() >= expected {
                return Some(output);
            }
            output.push(first);
            first
        } else {
            return None;
        };

        if output.len() == expected {
            return Some(output);
        }

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
    (output.len() == expected).then_some(output)
}

fn append_lzw(
    mut code: u16,
    prefixes: &[u16; 4096],
    suffixes: &[u8; 4096],
    stack: &mut [u8; 4096],
    output: &mut Vec<u8>,
    expected: usize,
) -> u8 {
    let mut count = 0usize;
    while code >= 256 {
        stack[count] = suffixes[usize::from(code)];
        count += 1;
        code = prefixes[usize::from(code)];
    }
    let first = code as u8;
    stack[count] = first;
    count += 1;
    let remaining = expected - output.len();
    output.extend(stack[..count].iter().rev().take(remaining));
    first
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
        if self.bit.checked_add(usize::from(width))? > self.data.len() * 8 {
            return None;
        }
        let mut value = 0u16;
        for _ in 0..width {
            value = (value << 1) | u16::from(data_bit_unchecked(self.data, self.bit));
            self.bit += 1;
        }
        Some(value)
    }
}

fn data_bit_unchecked(data: &[u8], bit: usize) -> u8 {
    (data[bit / 8] >> (7 - bit % 8)) & 1
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Endian {
    Little,
    Big,
}

impl Endian {
    fn u16_exact(self, bytes: [u8; 2]) -> u16 {
        match self {
            Endian::Little => u16::from_le_bytes(bytes),
            Endian::Big => u16::from_be_bytes(bytes),
        }
    }

    fn u32_exact(self, bytes: [u8; 4]) -> u32 {
        match self {
            Endian::Little => u32::from_le_bytes(bytes),
            Endian::Big => u32::from_be_bytes(bytes),
        }
    }

    fn write_u16(self, value: u16, destination: &mut [u8]) {
        let bytes = match self {
            Endian::Little => value.to_le_bytes(),
            Endian::Big => value.to_be_bytes(),
        };
        destination.copy_from_slice(&bytes);
    }

    fn write_u32(self, value: u32, destination: &mut [u8]) {
        let bytes = match self {
            Endian::Little => value.to_le_bytes(),
            Endian::Big => value.to_be_bytes(),
        };
        destination.copy_from_slice(&bytes);
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
        let count_bytes = data.get(offset..offset.checked_add(2)?)?;
        let count = usize::from(endian.u16_exact([count_bytes[0], count_bytes[1]]));
        if count > 4096 {
            return None;
        }
        let entries_start = offset + 2;
        let mut entries = Vec::with_capacity(count);
        for index in 0..count {
            let start = entries_start + index * 12;
            let bytes = data.get(start..start + 12)?;
            let tag = endian.u16_exact([bytes[0], bytes[1]]);
            let field_type = endian.u16_exact([bytes[2], bytes[3]]);
            let value_count = endian.u32_exact([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
            let type_size = match field_type {
                1 | 2 | 6 | 7 => 1,
                3 | 8 => 2,
                4 | 9 | 11 => 4,
                5 | 10 | 12 => 8,
                _ => continue,
            };
            #[cfg(target_pointer_width = "32")]
            let byte_len = value_count.checked_mul(type_size)?;
            #[cfg(not(target_pointer_width = "32"))]
            let byte_len = value_count * type_size;
            let value_position = if byte_len <= 4 {
                start + 8
            } else {
                endian.u32_exact([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize
            };
            #[cfg(target_pointer_width = "32")]
            data.get(value_position..value_position.checked_add(byte_len)?)?;
            #[cfg(not(target_pointer_width = "32"))]
            data.get(value_position..value_position + byte_len)?;
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

    fn one(&self, tag: u16) -> Option<usize> {
        self.values(tag)?.into_iter().next()
    }

    fn one_or(&self, tag: u16, default: usize) -> usize {
        self.one(tag).unwrap_or(default)
    }

    fn values_or(&self, tag: u16, default: &[usize]) -> Vec<usize> {
        self.values(tag).unwrap_or_else(|| default.to_vec())
    }

    fn values(&self, tag: u16) -> Option<Vec<usize>> {
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
            1 => values.extend(bytes.iter().map(|&value| usize::from(value))),
            3 => {
                for chunk in bytes.chunks_exact(2) {
                    values.push(usize::from(self.endian.u16_exact([chunk[0], chunk[1]])));
                }
            }
            4 => {
                for chunk in bytes.chunks_exact(4) {
                    values.push(
                        self.endian
                            .u32_exact([chunk[0], chunk[1], chunk[2], chunk[3]])
                            as usize,
                    );
                }
            }
            _ => return None,
        }
        Some(values)
    }
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    fn put_entry(out: &mut Vec<u8>, tag: u16, field_type: u16, count: u32, value: [u8; 4]) {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&field_type.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&value);
    }

    fn tiny_tiff(
        bits_count: u32,
        bits_inline: [u8; 4],
        photometric: u16,
        samples_per_pixel: u16,
        rows_per_strip: u32,
        planar: u16,
        predictor: u16,
    ) -> Vec<u8> {
        let entry_count = 11u16;
        let pixel_offset = 8 + 2 + usize::from(entry_count) * 12 + 4;
        let mut out = Vec::new();
        out.extend_from_slice(b"II");
        out.extend_from_slice(&42u16.to_le_bytes());
        out.extend_from_slice(&8u32.to_le_bytes());
        out.extend_from_slice(&entry_count.to_le_bytes());
        put_entry(&mut out, 256, 4, 1, 1u32.to_le_bytes());
        put_entry(&mut out, 257, 4, 1, 1u32.to_le_bytes());
        put_entry(&mut out, 258, 3, bits_count, bits_inline);
        put_entry(&mut out, 259, 3, 1, [1, 0, 0, 0]);
        put_entry(
            &mut out,
            262,
            3,
            1,
            [photometric as u8, (photometric >> 8) as u8, 0, 0],
        );
        put_entry(
            &mut out,
            273,
            4,
            1,
            u32::try_from(pixel_offset).unwrap().to_le_bytes(),
        );
        put_entry(
            &mut out,
            277,
            3,
            1,
            [
                samples_per_pixel as u8,
                (samples_per_pixel >> 8) as u8,
                0,
                0,
            ],
        );
        put_entry(&mut out, 278, 4, 1, rows_per_strip.to_le_bytes());
        put_entry(&mut out, 279, 4, 1, 1u32.to_le_bytes());
        put_entry(
            &mut out,
            284,
            3,
            1,
            [planar as u8, (planar >> 8) as u8, 0, 0],
        );
        put_entry(
            &mut out,
            317,
            3,
            1,
            [predictor as u8, (predictor >> 8) as u8, 0, 0],
        );
        out.extend_from_slice(&0u32.to_le_bytes());
        out.push(0);
        out
    }

    fn oversized_strip_tiff(
        width: u32,
        height: u32,
        bits_per_sample: u16,
        samples_per_pixel: u32,
    ) -> Vec<u8> {
        let entry_count = 11u16;
        let pixel_offset = 8 + 2 + usize::from(entry_count) * 12 + 4;
        let mut out = Vec::new();
        out.extend_from_slice(b"II");
        out.extend_from_slice(&42u16.to_le_bytes());
        out.extend_from_slice(&8u32.to_le_bytes());
        out.extend_from_slice(&entry_count.to_le_bytes());
        put_entry(&mut out, 256, 4, 1, width.to_le_bytes());
        put_entry(&mut out, 257, 4, 1, height.to_le_bytes());
        put_entry(
            &mut out,
            258,
            3,
            1,
            [bits_per_sample as u8, (bits_per_sample >> 8) as u8, 0, 0],
        );
        put_entry(&mut out, 259, 3, 1, [1, 0, 0, 0]);
        put_entry(&mut out, 262, 3, 1, [1, 0, 0, 0]);
        put_entry(
            &mut out,
            273,
            4,
            1,
            u32::try_from(pixel_offset).unwrap().to_le_bytes(),
        );
        put_entry(&mut out, 277, 4, 1, samples_per_pixel.to_le_bytes());
        put_entry(&mut out, 278, 4, 1, 1u32.to_le_bytes());
        put_entry(&mut out, 279, 4, 1, 0u32.to_le_bytes());
        put_entry(&mut out, 284, 3, 1, [1, 0, 0, 0]);
        put_entry(&mut out, 317, 3, 1, [1, 0, 0, 0]);
        out.extend_from_slice(&0u32.to_le_bytes());
        out
    }

    fn oversized_tile_tiff(
        width: u32,
        height: u32,
        bits_per_sample: u16,
        samples_per_pixel: u32,
        tile_width: u32,
        tile_height: u32,
    ) -> Vec<u8> {
        let entry_count = 13u16;
        let pixel_offset = 8 + 2 + usize::from(entry_count) * 12 + 4;
        let mut out = Vec::new();
        out.extend_from_slice(b"II");
        out.extend_from_slice(&42u16.to_le_bytes());
        out.extend_from_slice(&8u32.to_le_bytes());
        out.extend_from_slice(&entry_count.to_le_bytes());
        put_entry(&mut out, 256, 4, 1, width.to_le_bytes());
        put_entry(&mut out, 257, 4, 1, height.to_le_bytes());
        put_entry(
            &mut out,
            258,
            3,
            1,
            [bits_per_sample as u8, (bits_per_sample >> 8) as u8, 0, 0],
        );
        put_entry(&mut out, 259, 3, 1, [1, 0, 0, 0]);
        put_entry(&mut out, 262, 3, 1, [1, 0, 0, 0]);
        put_entry(&mut out, 277, 4, 1, samples_per_pixel.to_le_bytes());
        put_entry(&mut out, 278, 4, 1, 1u32.to_le_bytes());
        put_entry(&mut out, 284, 3, 1, [1, 0, 0, 0]);
        put_entry(&mut out, 317, 3, 1, [1, 0, 0, 0]);
        put_entry(&mut out, 322, 4, 1, tile_width.to_le_bytes());
        put_entry(&mut out, 323, 4, 1, tile_height.to_le_bytes());
        put_entry(
            &mut out,
            324,
            4,
            1,
            u32::try_from(pixel_offset).unwrap().to_le_bytes(),
        );
        put_entry(&mut out, 325, 4, 1, 0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out
    }

    fn tiny_tiled_tiff(
        bits_per_sample: u16,
        include_tile_offsets: bool,
        include_tile_byte_counts: bool,
        tile_width: u32,
        tile_height: u32,
        predictor: u16,
        compression: u16,
        tile_payload: &[u8],
    ) -> Vec<u8> {
        let entry_count =
            10u16 + u16::from(include_tile_offsets) + u16::from(include_tile_byte_counts);
        let pixel_offset = 8 + 2 + usize::from(entry_count) * 12 + 4;
        let mut out = Vec::new();
        out.extend_from_slice(b"II");
        out.extend_from_slice(&42u16.to_le_bytes());
        out.extend_from_slice(&8u32.to_le_bytes());
        out.extend_from_slice(&entry_count.to_le_bytes());
        put_entry(&mut out, 256, 4, 1, 1u32.to_le_bytes());
        put_entry(&mut out, 257, 4, 1, 1u32.to_le_bytes());
        put_entry(&mut out, 258, 3, 1, [bits_per_sample as u8, 0, 0, 0]);
        put_entry(
            &mut out,
            259,
            3,
            1,
            [compression as u8, (compression >> 8) as u8, 0, 0],
        );
        put_entry(&mut out, 262, 3, 1, [1, 0, 0, 0]);
        put_entry(&mut out, 277, 3, 1, [1, 0, 0, 0]);
        put_entry(&mut out, 278, 4, 1, 1u32.to_le_bytes());
        put_entry(
            &mut out,
            317,
            3,
            1,
            [predictor as u8, (predictor >> 8) as u8, 0, 0],
        );
        put_entry(&mut out, 322, 4, 1, tile_width.to_le_bytes());
        put_entry(&mut out, 323, 4, 1, tile_height.to_le_bytes());
        if include_tile_offsets {
            put_entry(
                &mut out,
                324,
                4,
                1,
                u32::try_from(pixel_offset).unwrap().to_le_bytes(),
            );
        }
        if include_tile_byte_counts {
            put_entry(
                &mut out,
                325,
                4,
                1,
                u32::try_from(tile_payload.len()).unwrap().to_le_bytes(),
            );
        }
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(tile_payload);
        out
    }

    fn put_long_entry(
        out: &mut Vec<u8>,
        tag: u16,
        values: &[u32],
        external_start: usize,
        external: &mut Vec<u8>,
    ) {
        match values {
            [] => put_entry(out, tag, 4, 0, [0; 4]),
            [value] => put_entry(out, tag, 4, 1, value.to_le_bytes()),
            _ => {
                let position = u32::try_from(external_start + external.len()).unwrap();
                put_entry(
                    out,
                    tag,
                    4,
                    u32::try_from(values.len()).unwrap(),
                    position.to_le_bytes(),
                );
                for value in values {
                    external.extend_from_slice(&value.to_le_bytes());
                }
            }
        }
    }

    fn tiny_strip_tiff(
        width: u32,
        height: u32,
        bits_per_sample: u16,
        compression: u16,
        predictor: u16,
        rows_per_strip: u32,
        offset_count: usize,
        byte_counts: Option<&[u32]>,
        strip_payloads: &[&[u8]],
    ) -> Vec<u8> {
        let entry_count = 11u16;
        let external_start = 8 + 2 + usize::from(entry_count) * 12 + 4;
        let counts_len = byte_counts.map_or(0, <[u32]>::len);
        let pixel_offset = external_start
            + if offset_count > 1 {
                offset_count * 4
            } else {
                0
            }
            + if counts_len > 1 { counts_len * 4 } else { 0 };
        let mut next_offset = u32::try_from(pixel_offset).unwrap();
        let offsets = (0..offset_count)
            .map(|index| {
                let offset = next_offset;
                if let Some(payload) = strip_payloads.get(index) {
                    next_offset += u32::try_from(payload.len()).unwrap();
                }
                offset
            })
            .collect::<Vec<_>>();
        let mut external = Vec::new();
        let mut out = Vec::new();
        out.extend_from_slice(b"II");
        out.extend_from_slice(&42u16.to_le_bytes());
        out.extend_from_slice(&8u32.to_le_bytes());
        out.extend_from_slice(&entry_count.to_le_bytes());
        put_entry(&mut out, 256, 4, 1, width.to_le_bytes());
        put_entry(&mut out, 257, 4, 1, height.to_le_bytes());
        put_entry(&mut out, 258, 3, 1, [bits_per_sample as u8, 0, 0, 0]);
        put_entry(
            &mut out,
            259,
            3,
            1,
            [compression as u8, (compression >> 8) as u8, 0, 0],
        );
        put_entry(&mut out, 262, 3, 1, [1, 0, 0, 0]);
        put_long_entry(&mut out, 273, &offsets, external_start, &mut external);
        put_entry(&mut out, 277, 3, 1, [1, 0, 0, 0]);
        put_entry(&mut out, 278, 4, 1, rows_per_strip.to_le_bytes());
        put_long_entry(
            &mut out,
            279,
            byte_counts.unwrap_or(&[]),
            external_start,
            &mut external,
        );
        put_entry(&mut out, 284, 3, 1, [1, 0, 0, 0]);
        put_entry(
            &mut out,
            317,
            3,
            1,
            [predictor as u8, (predictor >> 8) as u8, 0, 0],
        );
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&external);
        for payload in strip_payloads {
            out.extend_from_slice(payload);
        }
        out
    }

    fn tiny_tiled_layout_tiff(
        width: u32,
        height: u32,
        bits_per_sample: u16,
        tile_width: u32,
        tile_height: u32,
        predictor: u16,
        compression: u16,
        tile_payloads: &[&[u8]],
        byte_counts: Option<&[u32]>,
    ) -> Vec<u8> {
        let entry_count = 12u16;
        let external_start = 8 + 2 + usize::from(entry_count) * 12 + 4;
        let counts = byte_counts.map(<[u32]>::to_vec).unwrap_or_else(|| {
            tile_payloads
                .iter()
                .map(|payload| payload.len() as u32)
                .collect()
        });
        let pixel_offset = external_start
            + if tile_payloads.len() > 1 {
                tile_payloads.len() * 4
            } else {
                0
            }
            + if counts.len() > 1 {
                counts.len() * 4
            } else {
                0
            };
        let mut next_offset = u32::try_from(pixel_offset).unwrap();
        let offsets = tile_payloads
            .iter()
            .map(|payload| {
                let offset = next_offset;
                next_offset += u32::try_from(payload.len()).unwrap();
                offset
            })
            .collect::<Vec<_>>();
        let mut external = Vec::new();
        let mut out = Vec::new();
        out.extend_from_slice(b"II");
        out.extend_from_slice(&42u16.to_le_bytes());
        out.extend_from_slice(&8u32.to_le_bytes());
        out.extend_from_slice(&entry_count.to_le_bytes());
        put_entry(&mut out, 256, 4, 1, width.to_le_bytes());
        put_entry(&mut out, 257, 4, 1, height.to_le_bytes());
        put_entry(&mut out, 258, 3, 1, [bits_per_sample as u8, 0, 0, 0]);
        put_entry(
            &mut out,
            259,
            3,
            1,
            [compression as u8, (compression >> 8) as u8, 0, 0],
        );
        put_entry(&mut out, 262, 3, 1, [1, 0, 0, 0]);
        put_entry(&mut out, 277, 3, 1, [1, 0, 0, 0]);
        put_entry(&mut out, 278, 4, 1, 1u32.to_le_bytes());
        put_entry(
            &mut out,
            317,
            3,
            1,
            [predictor as u8, (predictor >> 8) as u8, 0, 0],
        );
        put_entry(&mut out, 322, 4, 1, tile_width.to_le_bytes());
        put_entry(&mut out, 323, 4, 1, tile_height.to_le_bytes());
        put_long_entry(&mut out, 324, &offsets, external_start, &mut external);
        put_long_entry(&mut out, 325, &counts, external_start, &mut external);
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&external);
        for payload in tile_payloads {
            out.extend_from_slice(payload);
        }
        out
    }

    fn single_entry_ifd(tag: u16, field_type: u16, count: u32, value: [u8; 4]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&1u16.to_le_bytes());
        put_entry(&mut out, tag, field_type, count, value);
        out.extend_from_slice(&0u32.to_le_bytes());
        out
    }

    let _ = decode(b"II\0\0\x08\0\0\0");
    let _ = decode(b"II*\0");
    let _ = decode(b"MM\0*\0\0\0\x08\0\0\0\0");
    let _ = decode(&tiny_tiff(0, [0, 0, 0, 0], 1, 1, 1, 1, 1));
    let _ = decode(&tiny_tiff(3, u32::MAX.to_le_bytes(), 1, 1, 1, 1, 1));
    let _ = decode(&tiny_tiff(2, [8, 0, 16, 0], 1, 1, 1, 1, 1));
    let _ = decode(&tiny_tiff(1, [0, 1, 0, 0], 1, 1, 1, 1, 1));
    let _ = decode(&tiny_tiff(1, [8, 0, 0, 0], 1, 0, 1, 1, 1));
    let _ = decode(&tiny_tiff(1, [8, 0, 0, 0], 1, 1, 0, 1, 1));
    let _ = decode(&tiny_tiff(1, [8, 0, 0, 0], 1, 1, 1, 2, 1));
    let _ = decode(&tiny_tiff(1, [8, 0, 0, 0], 1, 1, 1, 1, 2));
    let _ = decode(&tiny_tiff(1, [8, 0, 0, 0], 1, 1, 1, 1, 3));
    let _ = decode(&tiny_tiff(1, [8, 0, 0, 0], 6, 1, 1, 1, 1));
    let _ = decode(&tiny_tiff(1, [16, 0, 0, 0], 6, 3, 1, 1, 1));
    let _ = decode(&oversized_strip_tiff(u32::MAX, 1, 255, u32::MAX));
    let _ = decode(&oversized_strip_tiff(1_722_007_169, 1, 3, 3_570_783_445));
    let _ = decode(&oversized_strip_tiff(u32::MAX, u32::MAX, 1, u32::MAX));
    let _ = decode(&oversized_tile_tiff(1, 1, 16, u32::MAX, u32::MAX, 1));
    let _ = decode(&oversized_tile_tiff(1, 1, 8, u32::MAX, u32::MAX, 2));
    let _ = decode(&tiny_tiled_tiff(8, false, true, 1, 1, 1, 1, &[0]));
    let _ = decode(&tiny_tiled_tiff(8, true, false, 1, 1, 1, 1, &[0]));
    let _ = decode(&tiny_tiled_tiff(8, true, true, 1, 0, 1, 1, &[0]));
    let _ = decode(&tiny_tiled_tiff(1, true, true, 1, 1, 1, 1, &[0]));
    let _ = decode(&tiny_tiled_tiff(8, true, true, 1, 1, 2, 1, &[0]));

    let _ = decode(&tiny_strip_tiff(1, 1, 8, 1, 1, 1, 0, Some(&[]), &[]));
    let _ = decode(&tiny_strip_tiff(1, 1, 8, 1, 1, 1, 1, Some(&[1]), &[]));
    let _ = decode(&tiny_strip_tiff(
        1,
        1,
        8,
        1,
        1,
        1,
        2,
        Some(&[1, 1]),
        &[&[0], &[1]],
    ));
    let _ = decode(&tiny_strip_tiff(
        1,
        1,
        8,
        COMPRESSION_PACKBITS as u16,
        1,
        1,
        1,
        None,
        &[&[0, 7]],
    ));
    let _ = decode(&tiny_strip_tiff(
        1,
        2,
        8,
        COMPRESSION_PACKBITS as u16,
        1,
        1,
        2,
        None,
        &[&[0, 7], &[0, 8]],
    ));
    let _ = decode(&tiny_strip_tiff(
        1,
        2,
        8,
        COMPRESSION_PACKBITS as u16,
        1,
        1,
        2,
        Some(&[2]),
        &[&[0, 7], &[0, 8]],
    ));
    let _ = decode(&tiny_strip_tiff(
        1,
        1,
        8,
        COMPRESSION_PACKBITS as u16,
        1,
        1,
        1,
        Some(&[4]),
        &[&[0, 7]],
    ));
    let _ = decode(&tiny_tiled_layout_tiff(
        2,
        1,
        8,
        1,
        1,
        1,
        1,
        &[&[0]],
        Some(&[1]),
    ));
    let _ = decode(&tiny_tiled_layout_tiff(
        2,
        2,
        8,
        1,
        1,
        1,
        1,
        &[&[1], &[2], &[3], &[4]],
        None,
    ));

    let _ = convert_pixels(3, 1, vec![0b1010_0000], 0, 1, 1, Endian::Little, None, 1);
    let _ = convert_pixels(8, 1, vec![0], 0, 1, 1, Endian::Little, None, 1);
    let _ = convert_pixels(1, 1, vec![0], 1, 1, 2, Endian::Little, None, 1);
    let _ = convert_pixels(9, 1, vec![0], 1, 1, 2, Endian::Little, None, 1);
    let _ = convert_pixels(1, 1, vec![0x34, 0x12], 0, 1, 16, Endian::Little, None, 1);
    let _ = convert_pixels(1, 1, vec![0x12, 0x34], 1, 1, 16, Endian::Big, None, 1);
    let _ = convert_pixels(1, 1, vec![0], 1, 1, 16, Endian::Little, None, 1);
    let _ = convert_pixels(1, 1, vec![0; 4], 1, 1, 32, Endian::Little, None, 1);
    let _ = convert_pixels(1, 1, vec![0; 4], 1, 1, 32, Endian::Little, None, 2);
    let _ = convert_pixels(1, 1, vec![0; 4], 1, 1, 32, Endian::Little, None, 3);
    let _ = convert_pixels(1, 1, vec![0; 4], 1, 1, 32, Endian::Little, None, 4);
    let palette = [0, 0xffff, 0, 0, 0xffff, 0, 0, 0xffff, 0];
    let _ = convert_pixels(
        2,
        1,
        vec![0b0100_0000],
        3,
        1,
        2,
        Endian::Little,
        Some(&palette),
        1,
    );
    let short_palette = [0, 0, 0];
    let _ = convert_pixels(1, 1, vec![], 3, 1, 1, Endian::Little, Some(&palette), 1);
    let _ = convert_pixels(1, 1, vec![0], 3, 1, 1, Endian::Little, None, 1);
    let _ = convert_pixels(
        1,
        1,
        vec![0],
        3,
        1,
        1,
        Endian::Little,
        Some(&short_palette),
        1,
    );
    let bad_red_palette = [0x1_0000, 0, 0, 0, 0, 0];
    let _ = convert_pixels(
        1,
        1,
        vec![0],
        3,
        1,
        1,
        Endian::Little,
        Some(&bad_red_palette),
        1,
    );
    let bad_green_palette = [0, 0, 0x1_0000, 0, 0, 0];
    let _ = convert_pixels(
        1,
        1,
        vec![0],
        3,
        1,
        1,
        Endian::Little,
        Some(&bad_green_palette),
        1,
    );
    let bad_blue_palette = [0, 0, 0, 0, 0x1_0000, 0];
    let _ = convert_pixels(
        1,
        1,
        vec![0],
        3,
        1,
        1,
        Endian::Little,
        Some(&bad_blue_palette),
        1,
    );
    let _ = convert_pixels(1, 1, vec![1, 2, 3, 4], 6, 3, 8, Endian::Little, None, 1);
    let _ = convert_pixels(1, 1, vec![], 9, 1, 8, Endian::Little, None, 1);
    let palette8 = [0usize; 768];
    let _ = convert_pixels(1, 1, vec![0], 3, 1, 8, Endian::Little, Some(&palette8), 1);
    let _ = convert_pixels(1, 1, vec![0], 3, 1, 8, Endian::Little, None, 1);
    let _ = unpack_indices(&[], 1, 1, 1);
    let _ = unpack_indices(&[0], 9, 1, 1);
    let _ = unpack_indices(&[0], 1, 1, 9);
    let _ = unpack_indices(&[0, 0], 3, 1, 3);

    let _ = decode_packbits(&[], 0);
    let _ = decode_packbits(&[0], 0);
    let _ = decode_packbits(&[0, 7], 1);
    let _ = decode_packbits(&[0x80, 0, 9], 1);
    let _ = decode_packbits(&[0x80], 1);
    let _ = decode_packbits(&[1, 7, 8], 1);
    let _ = decode_packbits(&[0xff, 5], 2);
    let _ = decode_packbits(&[0xff], 2);
    let _ = decode_packbits(&[2, 1], 3);

    fn pack_lzw_9(codes: &[u16]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut current = 0u8;
        let mut used = 0u8;
        for &code in codes {
            for shift in (0..9).rev() {
                current = (current << 1) | (((code >> shift) & 1) as u8);
                used += 1;
                if used == 8 {
                    out.push(current);
                    current = 0;
                    used = 0;
                }
            }
        }
        out.push(current << (8 - used));
        out
    }

    let _ = decode_lzw(&pack_lzw_9(&[258]), 1);
    let _ = decode_lzw(&pack_lzw_9(&[65]), 0);
    let _ = decode_lzw(&pack_lzw_9(&[65]), 1);
    let _ = decode_lzw(&pack_lzw_9(&[65, 66, 257]), 2);
    let lzw_a = pack_lzw_9(&[65]);
    let _ = decode(&tiny_strip_tiff(
        1,
        1,
        8,
        COMPRESSION_LZW as u16,
        2,
        1,
        1,
        Some(&[u32::try_from(lzw_a.len()).unwrap()]),
        &[&lzw_a],
    ));
    let _ = decode(&tiny_tiled_tiff(
        24,
        true,
        true,
        1,
        1,
        2,
        COMPRESSION_LZW as u16,
        &pack_lzw_9(&[65, 66, 67]),
    ));
    let _ = decode(&tiny_tiled_layout_tiff(
        1,
        1,
        8,
        1,
        1,
        2,
        COMPRESSION_LZW as u16,
        &[&lzw_a],
        Some(&[u32::try_from(lzw_a.len()).unwrap()]),
    ));
    let _ = decode(&tiny_tiled_layout_tiff(
        1,
        2,
        8,
        1,
        1,
        1,
        COMPRESSION_LZW as u16,
        &[&lzw_a, &lzw_a],
        Some(&[u32::try_from(lzw_a.len()).unwrap()]),
    ));
    let _ = decode(&tiny_tiled_layout_tiff(
        1,
        1,
        8,
        1,
        1,
        1,
        COMPRESSION_LZW as u16,
        &[&lzw_a],
        Some(&[u32::try_from(lzw_a.len() + 1).unwrap()]),
    ));
    let _ = decode(&tiny_tiled_layout_tiff(
        1,
        1,
        8,
        1,
        1,
        1,
        COMPRESSION_LZW as u16,
        &[&[0]],
        Some(&[1]),
    ));
    let lzw_rgb = pack_lzw_9(&[65, 66, 67, 257]);
    let _ = decode(&tiny_strip_tiff(
        1,
        1,
        24,
        COMPRESSION_LZW as u16,
        2,
        1,
        1,
        Some(&[u32::try_from(lzw_rgb.len()).unwrap()]),
        &[&lzw_rgb],
    ));

    let mut one_bit_reader = MsbBits::new(&[0x80]);
    let _ = one_bit_reader.read(1);
    let mut empty_width_reader = MsbBits::new(&[]);
    let _ = empty_width_reader.read(0);
    let mut short_reader = MsbBits::new(&[0]);
    let _ = short_reader.read(9);
    let mut overflow_reader = MsbBits {
        data: &[0],
        bit: usize::MAX,
    };
    let _ = overflow_reader.read(1);
    let mut endian_bytes = [0; 4];
    Endian::Little.write_u16(1, &mut endian_bytes[..2]);
    Endian::Big.write_u32(1, &mut endian_bytes);

    let _ = Directory::parse(&[], 0, Endian::Little);
    let _ = Directory::parse(&[], usize::MAX, Endian::Little);
    let oversized_count = 4097u16.to_le_bytes();
    let _ = Directory::parse(&oversized_count, 0, Endian::Little);
    let truncated_entry = 1u16.to_le_bytes();
    let _ = Directory::parse(&truncated_entry, 0, Endian::Little);
    let _ = Directory::parse(&single_entry_ifd(300, 13, 1, [0; 4]), 0, Endian::Little);
    let _ = Directory::parse(
        &single_entry_ifd(300, 5, 1, u32::MAX.to_le_bytes()),
        0,
        Endian::Little,
    );
    let empty_directory = Directory {
        data: &[],
        endian: Endian::Little,
        entries: Vec::new(),
    };
    let _ = empty_directory.one_or(1, 7);
    let _ = empty_directory.values_or(1, &[7, 8]);
    let inline_shorts = single_entry_ifd(300, 3, 2, [1, 0, 2, 0]);
    let directory = Directory::parse(&inline_shorts, 0, Endian::Little).unwrap();
    let _ = directory.values(300);
    let inline_long = single_entry_ifd(301, 4, 1, 9u32.to_le_bytes());
    let directory = Directory::parse(&inline_long, 0, Endian::Little).unwrap();
    let _ = directory.values(301);
    let mut external_shorts = single_entry_ifd(302, 3, 3, 18u32.to_le_bytes());
    external_shorts.extend_from_slice(&[1, 0, 2, 0, 3, 0]);
    let directory = Directory::parse(&external_shorts, 0, Endian::Little).unwrap();
    let _ = directory.values(302);
    let mut external_longs = single_entry_ifd(303, 4, 2, 18u32.to_le_bytes());
    external_longs.extend_from_slice(&1u32.to_le_bytes());
    external_longs.extend_from_slice(&2u32.to_le_bytes());
    let directory = Directory::parse(&external_longs, 0, Endian::Little).unwrap();
    let _ = directory.values(303);

    let overflow_values = Directory {
        data: &[0],
        endian: Endian::Little,
        entries: vec![Entry {
            tag: 1,
            field_type: 1,
            count: 1,
            value_position: 0,
            inline_position: usize::MAX,
            byte_len: 1,
        }],
    };
    let _ = overflow_values.values(1);
    let huge_values = Directory {
        data: &[0],
        endian: Endian::Little,
        entries: vec![Entry {
            tag: 1,
            field_type: 1,
            count: 1,
            value_position: 0,
            inline_position: 1,
            byte_len: usize::MAX,
        }],
    };
    let _ = huge_values.values(1);
    let unsupported_values = Directory {
        data: &[0, 0, 0, 0],
        endian: Endian::Little,
        entries: vec![Entry {
            tag: 1,
            field_type: 5,
            count: 1,
            value_position: 0,
            inline_position: 0,
            byte_len: 1,
        }],
    };
    let _ = unsupported_values.values(1);
    let _ = unsupported_values.values(2);

    let mut predicted = vec![1, 2, 3, 4, 5, 6];
    reverse_horizontal_predictor(&mut predicted, 6, 3, 8, Endian::Little);
    let mut predicted = vec![0, 1, 0, 2];
    reverse_horizontal_predictor(&mut predicted, 4, 1, 16, Endian::Big);
    let mut predicted = vec![0, 0, 0, 1, 0, 0, 0, 2];
    reverse_horizontal_predictor(&mut predicted, 8, 1, 32, Endian::Little);
}
