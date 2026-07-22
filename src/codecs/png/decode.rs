//! PNG decoder implemented from the PNG chunk and filtering specifications.

use crate::codecs::compression::deflate::decompress_zlib_prefix;
use crate::types::{ColorType, DecodedImage, ImageMode, ImagePalette};

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const ADAM7: [(usize, usize, usize, usize); 7] = [
    (0, 0, 8, 8),
    (4, 0, 8, 8),
    (0, 4, 4, 8),
    (2, 0, 4, 4),
    (0, 2, 2, 4),
    (1, 0, 2, 2),
    (0, 1, 1, 2),
];

/// Decode the first image represented by a PNG or APNG stream.
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    let mut chunks = Chunks::new(data)?;
    let header = chunks.next()?;
    if header.kind != *b"IHDR" || header.data.len() != 13 {
        return None;
    }

    let width = u32::from_be_bytes([
        header.data[0],
        header.data[1],
        header.data[2],
        header.data[3],
    ]);
    let height = u32::from_be_bytes([
        header.data[4],
        header.data[5],
        header.data[6],
        header.data[7],
    ]);
    let depth = header.data[8];
    let png_color = header.data[9];
    let _compression = header.data[10];
    let filter = header.data[11];
    let interlace = header.data[12];
    if width == 0 || height == 0 || filter != 0 || interlace > 1 {
        return None;
    }
    let (channels, color) = png_layout(png_color, depth)?;

    let mut compressed = Vec::new();
    let mut palette_rgb = None;
    let mut palette_alpha = Vec::new();
    let mut saw_end = false;
    for chunk in &mut chunks {
        match &chunk.kind {
            b"IDAT" => compressed.extend_from_slice(chunk.data),
            b"PLTE" if palette_rgb.is_none() => palette_rgb = Some(chunk.data.to_vec()),
            b"tRNS" if palette_alpha.is_empty() => palette_alpha.extend_from_slice(chunk.data),
            b"IEND" => {
                saw_end = true;
                break;
            }
            _ => {}
        }
    }
    if compressed.is_empty() || (!saw_end && chunks.failed) {
        return None;
    }

    let expected_inflated = inflated_len(width, height, channels, depth, interlace)?;
    let inflated = decompress_zlib_prefix(&compressed, expected_inflated)?;
    if inflated.len() != expected_inflated {
        return None;
    }

    let samples = decode_scanlines(&inflated, width, height, channels, depth, interlace)?;
    build_image(
        width,
        height,
        png_color,
        depth,
        color,
        &samples,
        palette_rgb,
        palette_alpha,
    )
}

fn png_layout(color: u8, depth: u8) -> Option<(usize, ColorType)> {
    match (color, depth) {
        (0, 1 | 2 | 4 | 8) | (3, 1 | 2 | 4 | 8) => Some((1, ColorType::L8)),
        (0, 16) => Some((1, ColorType::L16)),
        (2, 8 | 16) => Some((3, ColorType::Rgb8)),
        (4, 8) => Some((2, ColorType::La8)),
        (4, 16) | (6, 8 | 16) => Some((if color == 4 { 2 } else { 4 }, ColorType::Rgba8)),
        _ => None,
    }
}

fn inflated_len(
    width: u32,
    height: u32,
    channels: usize,
    depth: u8,
    interlace: u8,
) -> Option<usize> {
    let width = width as usize;
    let height = height as usize;
    if interlace == 0 {
        return (row_bytes(width, channels, depth)? + 1).checked_mul(height);
    }

    let mut total = 0usize;
    for (x_start, y_start, x_step, y_step) in ADAM7 {
        let pass_width = pass_size(width, x_start, x_step);
        let pass_height = pass_size(height, y_start, y_step);
        if pass_width != 0 && pass_height != 0 {
            total = total.checked_add(
                (row_bytes(pass_width, channels, depth)? + 1).checked_mul(pass_height)?,
            )?;
        }
    }
    Some(total)
}

fn decoded_sample_count(width: usize, height: usize, channels: usize) -> Option<usize> {
    width.checked_mul(height)?.checked_mul(channels)
}

fn decode_scanlines(
    data: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    depth: u8,
    interlace: u8,
) -> Option<Vec<u16>> {
    let width = width as usize;
    let height = height as usize;
    let sample_count = decoded_sample_count(width, height, channels)?;
    let mut samples = vec![0u16; sample_count];
    let mut position = 0usize;

    if interlace == 0 {
        let rows = unfilter_rows(data, &mut position, width, height, channels, depth)?;
        unpack_into(
            &rows,
            width,
            height,
            channels,
            depth,
            |x, y, channel, value| {
                let index = (y * width + x) * channels + channel;
                samples[index] = value;
            },
        );
    } else {
        for (x_start, y_start, x_step, y_step) in ADAM7 {
            let pass_width = pass_size(width, x_start, x_step);
            let pass_height = pass_size(height, y_start, y_step);
            if pass_width == 0 || pass_height == 0 {
                continue;
            }
            let rows = unfilter_rows(
                data,
                &mut position,
                pass_width,
                pass_height,
                channels,
                depth,
            )?;
            unpack_into(
                &rows,
                pass_width,
                pass_height,
                channels,
                depth,
                |pass_x, pass_y, channel, value| {
                    let x = x_start + pass_x * x_step;
                    let y = y_start + pass_y * y_step;
                    let index = (y * width + x) * channels + channel;
                    samples[index] = value;
                },
            );
        }
    }
    (position == data.len()).then_some(samples)
}

fn read_filtered_row<'a>(
    data: &'a [u8],
    position: &mut usize,
    stride: usize,
) -> Option<(u8, &'a [u8])> {
    let filter = *data.get(*position)?;
    *position += 1;
    let source_end = (*position).checked_add(stride)?;
    let source = data.get(*position..source_end)?;
    *position = source_end;
    Some((filter, source))
}

fn unfilter_rows(
    data: &[u8],
    position: &mut usize,
    width: usize,
    height: usize,
    channels: usize,
    depth: u8,
) -> Option<Vec<u8>> {
    let stride = row_bytes(width, channels, depth)?;
    let bytes_per_pixel = channels.checked_mul(usize::from(depth))?.div_ceil(8).max(1);
    let mut rows = vec![0u8; stride.checked_mul(height)?];

    for row in 0..height {
        let (filter, source) = read_filtered_row(data, position, stride)?;
        let row_start = row * stride;

        for column in 0..stride {
            let left = if column >= bytes_per_pixel {
                rows[row_start + column - bytes_per_pixel]
            } else {
                0
            };
            let above = if row != 0 {
                rows[row_start - stride + column]
            } else {
                0
            };
            let upper_left = if row != 0 && column >= bytes_per_pixel {
                rows[row_start - stride + column - bytes_per_pixel]
            } else {
                0
            };
            rows[row_start + column] = match filter {
                0 => source[column],
                1 => source[column].wrapping_add(left),
                2 => source[column].wrapping_add(above),
                3 => source[column].wrapping_add(((u16::from(left) + u16::from(above)) / 2) as u8),
                4 => source[column].wrapping_add(paeth(left, above, upper_left)),
                _ => return None,
            };
        }
    }
    Some(rows)
}

fn unpack_into<F>(
    rows: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    depth: u8,
    mut store: F,
) where
    F: FnMut(usize, usize, usize, u16),
{
    let stride = rows.len() / height;
    for y in 0..height {
        let row = &rows[y * stride..(y + 1) * stride];
        for x in 0..width {
            for channel in 0..channels {
                let sample_index = x * channels + channel;
                let value = match depth {
                    1 | 2 | 4 => {
                        let bit = sample_index * usize::from(depth);
                        let shift = 8 - usize::from(depth) - bit % 8;
                        u16::from((row[bit / 8] >> shift) & ((1u8 << depth) - 1))
                    }
                    8 => u16::from(row[sample_index]),
                    _ => {
                        debug_assert_eq!(depth, 16);
                        let offset = sample_index * 2;
                        u16::from_be_bytes([row[offset], row[offset + 1]])
                    }
                };
                store(x, y, channel, value);
            }
        }
    }
}

fn build_image(
    width: u32,
    height: u32,
    png_color: u8,
    depth: u8,
    color: ColorType,
    samples: &[u16],
    palette_rgb: Option<Vec<u8>>,
    mut palette_alpha: Vec<u8>,
) -> Option<DecodedImage> {
    let pixels = if png_color == 0 && depth == 1 {
        pack_one_bit(samples, width as usize, height as usize)
    } else if png_color == 0 && depth < 8 {
        let maximum = (1u16 << depth) - 1;
        samples
            .iter()
            .map(|&sample| ((sample * 255) / maximum) as u8)
            .collect()
    } else if png_color == 4 && depth == 16 {
        let mut bytes = Vec::with_capacity(samples.len() * 2);
        for pair in samples.chunks_exact(2) {
            let luminance = (pair[0] >> 8) as u8;
            let alpha = (pair[1] >> 8) as u8;
            bytes.extend_from_slice(&[luminance, luminance, luminance, alpha]);
        }
        bytes
    } else if depth == 16 && matches!(png_color, 2 | 6) {
        samples.iter().map(|&sample| (sample >> 8) as u8).collect()
    } else if png_color == 3 || depth == 8 {
        samples.iter().map(|&sample| sample as u8).collect()
    } else {
        let mut bytes = Vec::with_capacity(samples.len() * 2);
        for &sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    };
    let mode = match (png_color, depth) {
        (0, 1) => ImageMode::L1,
        (3, _) => ImageMode::P8,
        _ => color.into(),
    };
    let mut image = DecodedImage::with_mode(width, height, pixels, mode);
    if png_color == 3 {
        if let Some(mut rgb) = palette_rgb {
            let entries = rgb.len() / 3;
            if entries != 0 {
                rgb.truncate(entries * 3);
                if !palette_alpha.is_empty() {
                    palette_alpha.truncate(entries);
                }
                image = image.with_palette(ImagePalette::new(rgb, palette_alpha).ok()?);
            }
        }
    }
    Some(image)
}

fn pack_one_bit(samples: &[u16], width: usize, height: usize) -> Vec<u8> {
    let stride = width.div_ceil(8);
    let mut output = vec![0u8; stride * height];
    for y in 0..height {
        for x in 0..width {
            if samples[y * width + x] != 0 {
                output[y * stride + x / 8] |= 1 << (7 - x % 8);
            }
        }
    }
    output
}

fn row_bytes(width: usize, channels: usize, depth: u8) -> Option<usize> {
    width
        .checked_mul(channels)?
        .checked_mul(usize::from(depth))?
        .checked_add(7)
        .map(|bits| bits / 8)
}

fn pass_size(full: usize, start: usize, step: usize) -> usize {
    if full <= start {
        0
    } else {
        (full - start).div_ceil(step)
    }
}

fn paeth(left: u8, above: u8, upper_left: u8) -> u8 {
    let left = i32::from(left);
    let above = i32::from(above);
    let upper_left = i32::from(upper_left);
    let prediction = left + above - upper_left;
    let left_distance = (prediction - left).unsigned_abs();
    let above_distance = (prediction - above).unsigned_abs();
    let diagonal_distance = (prediction - upper_left).unsigned_abs();
    if left_distance <= above_distance && left_distance <= diagonal_distance {
        left as u8
    } else if above_distance <= diagonal_distance {
        above as u8
    } else {
        upper_left as u8
    }
}

fn crc32(kind: &[u8; 4], data: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in kind.iter().chain(data) {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

fn chunk_payload_with_crc<'a>(
    data: &'a [u8],
    kind: &[u8; 4],
    start: usize,
    length: usize,
) -> Option<(&'a [u8], usize)> {
    let end = start.checked_add(length)?;
    let payload = data.get(start..end)?;
    let crc_end = end + 4;
    let expected_bytes = data.get(end..crc_end)?;
    let expected = u32::from_be_bytes([
        expected_bytes[0],
        expected_bytes[1],
        expected_bytes[2],
        expected_bytes[3],
    ]);
    (crc32(kind, payload) == expected).then_some((payload, crc_end))
}

struct Chunk<'a> {
    kind: [u8; 4],
    data: &'a [u8],
}

struct Chunks<'a> {
    data: &'a [u8],
    position: usize,
    failed: bool,
}

impl<'a> Chunks<'a> {
    fn new(data: &'a [u8]) -> Option<Self> {
        (data.get(..8)? == PNG_SIGNATURE).then_some(Self {
            data,
            position: 8,
            failed: false,
        })
    }
}

impl<'a> Iterator for Chunks<'a> {
    type Item = Chunk<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.position == self.data.len() {
            return None;
        }
        let result = (|| {
            let length_bytes = self.data.get(self.position..self.position + 4)?;
            let length = u32::from_be_bytes([
                length_bytes[0],
                length_bytes[1],
                length_bytes[2],
                length_bytes[3],
            ]) as usize;
            let kind_bytes = self.data.get(self.position + 4..self.position + 8)?;
            let kind = [kind_bytes[0], kind_bytes[1], kind_bytes[2], kind_bytes[3]];
            let start = self.position + 8;
            let (payload, crc_end) = chunk_payload_with_crc(self.data, &kind, start, length)?;
            self.position = crc_end;
            Some(Chunk {
                kind,
                data: payload,
            })
        })();
        if result.is_none() {
            self.failed = true;
        }
        result
    }
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let malformed = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x01tEXtx";
    let mut chunks = Chunks::new(malformed).expect("coverage PNG signature should parse");

    assert!(chunks.next().is_none());
    assert!(chunks.failed);
    assert!(chunks.next().is_none());

    assert_eq!(row_bytes(usize::MAX, 2, 8), None);
    assert_eq!(row_bytes(usize::MAX / 8 + 1, 1, 8), None);
    assert_eq!(row_bytes(usize::MAX - 6, 1, 1), None);

    assert_eq!(inflated_len(1, 1, usize::MAX, 8, 0), None);
    assert_eq!(inflated_len(8, 8, usize::MAX, 8, 1), None);
    assert_eq!(inflated_len(u32::MAX, u32::MAX, 2, 8, 0), None);
    assert_eq!(inflated_len(u32::MAX, u32::MAX, 64, 16, 1), None);
    assert_eq!(decoded_sample_count(usize::MAX, 2, 1), None);
    assert_eq!(decode_scanlines(&[], u32::MAX, u32::MAX, 4, 8, 0), None);

    let mut position = 0;
    assert_eq!(unfilter_rows(&[], &mut position, usize::MAX, 1, 1, 8), None);
    assert_eq!(unfilter_rows(&[], &mut position, 0, 0, usize::MAX, 8), None);
    assert_eq!(
        unfilter_rows(&[], &mut position, usize::MAX / 8, 9, 1, 8),
        None
    );

    let mut position = 0;
    assert_eq!(unfilter_rows(&[], &mut position, 1, 1, 1, 8), None);

    let mut position = 0;
    assert_eq!(unfilter_rows(&[0], &mut position, 1, 1, 1, 8), None);

    let mut position = 0;
    assert!(read_filtered_row(&[0], &mut position, usize::MAX).is_none());

    assert!(chunk_payload_with_crc(&[], b"IDAT", usize::MAX, 1).is_none());
}
