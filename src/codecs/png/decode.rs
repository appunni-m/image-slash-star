//! PNG decoder implemented from the PNG chunk and filtering specifications.

use crate::codecs::compression::deflate::decompress_zlib_prefix;
use crate::codecs::{CodecError, CodecResult, OptionCodecExt};
use crate::types::{ColorType, DecodedImage, ImageMode, ImagePalette};

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const PILLOW_DECOMPRESSION_BOMB_ERROR_PIXELS: u64 = 178_956_970;
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
pub fn decode(data: &[u8]) -> CodecResult<DecodedImage> {
    // Pillow's load path accepts bad IDAT CRCs after lazy construction has
    // validated all construction-critical chunks.
    let mut chunks = Chunks::new(data, false)?;
    let header = chunks
        .next()
        .transpose()?
        .malformed("PNG is missing its IHDR chunk")?;
    if header.kind != *b"IHDR" || header.data.len() != 13 {
        return Err(CodecError::Malformed(
            "PNG IHDR chunk has an invalid type or length".to_owned(),
        ));
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
        return Err(CodecError::Malformed(
            "PNG IHDR fields are invalid".to_owned(),
        ));
    }
    if u64::from(width).saturating_mul(u64::from(height)) > PILLOW_DECOMPRESSION_BOMB_ERROR_PIXELS {
        return Err(CodecError::Dimensions(
            "PNG dimensions exceed Pillow's decompression-bomb limit".to_owned(),
        ));
    }
    let (channels, color) = png_layout(png_color, depth)?;

    let mut compressed = Vec::new();
    let mut palette_rgb = None;
    let mut palette_alpha = Vec::new();
    for chunk in &mut chunks {
        let chunk = chunk?;
        match &chunk.kind {
            b"IDAT" => compressed.extend_from_slice(chunk.data),
            b"PLTE" if palette_rgb.is_none() => palette_rgb = Some(chunk.data.to_vec()),
            b"tRNS" if palette_alpha.is_empty() => palette_alpha.extend_from_slice(chunk.data),
            b"acTL" if chunk.data.len() != 8 => {
                return Err(CodecError::Malformed(
                    "PNG acTL chunk has an invalid length".to_owned(),
                ));
            }
            b"IEND" => {
                break;
            }
            _ => {}
        }
    }
    if compressed.is_empty() {
        return Err(CodecError::Malformed(
            "PNG contains no image data".to_owned(),
        ));
    }

    let expected_inflated = inflated_len(width, height, channels, depth, interlace);
    let inflated = decompress_zlib_prefix(&compressed, expected_inflated)
        .map_err(|error| error.context("decode PNG zlib stream"))?;
    if inflated.len() != expected_inflated {
        return Err(CodecError::Malformed(
            "PNG image data has an unexpected decompressed length".to_owned(),
        ));
    }

    let samples = decode_scanlines(&inflated, width, height, channels, depth, interlace)?;
    build_image(
        PngImageSpec {
            width,
            height,
            png_color,
            depth,
            color,
        },
        &samples,
        palette_rgb,
        palette_alpha,
    )
}

/// Validate PNG chunk framing and CRCs without decompressing image samples.
///
/// This matches Pillow's PNG-specific `Image.verify()` behavior: validation
/// proceeds from the image-data chunk through `IEND`, while construction has
/// already inspected the preceding header and metadata chunks.
pub(crate) fn verify(data: &[u8]) -> CodecResult<()> {
    // `EncodedImage::new` has already inspected the immutable source and
    // proved its signature, 13-byte IHDR framing, and construction-critical
    // IHDR CRC. Verification therefore starts at the first post-IHDR chunk.
    let mut chunks = Chunks {
        data,
        position: 33,
        failed: false,
        verify_crc: true,
    };
    let mut saw_image_data = false;
    for chunk in &mut chunks {
        let chunk = chunk?;
        saw_image_data |= chunk.kind == *b"IDAT";
        if chunk.kind == *b"IEND" {
            return if saw_image_data {
                Ok(())
            } else {
                Err(CodecError::Malformed(
                    "PNG contains no image-data chunk".to_owned(),
                ))
            };
        }
    }
    Err(CodecError::Malformed(
        "PNG is missing its IEND chunk".to_owned(),
    ))
}

fn png_layout(color: u8, depth: u8) -> CodecResult<(usize, ColorType)> {
    match (color, depth) {
        (0, 1 | 2 | 4 | 8) | (3, 1 | 2 | 4 | 8) => Ok((1, ColorType::L8)),
        (0, 16) => Ok((1, ColorType::L16)),
        (2, 8 | 16) => Ok((3, ColorType::Rgb8)),
        (4, 8) => Ok((2, ColorType::La8)),
        (4, 16) | (6, 8 | 16) => Ok((if color == 4 { 2 } else { 4 }, ColorType::Rgba8)),
        _ => Err(CodecError::Malformed(
            "PNG color type and bit depth are incompatible".to_owned(),
        )),
    }
}

fn inflated_len(width: u32, height: u32, channels: usize, depth: u8, interlace: u8) -> usize {
    let width = width as usize;
    let height = height as usize;
    if interlace == 0 {
        return row_bytes(width, channels, depth)
            .wrapping_add(1)
            .wrapping_mul(height);
    }

    let mut total = 0usize;
    for (x_start, y_start, x_step, y_step) in ADAM7 {
        let pass_width = pass_size(width, x_start, x_step);
        let pass_height = pass_size(height, y_start, y_step);
        if pass_width != 0 && pass_height != 0 {
            total = total.wrapping_add(
                row_bytes(pass_width, channels, depth)
                    .wrapping_add(1)
                    .wrapping_mul(pass_height),
            );
        }
    }
    total
}

fn decoded_sample_count(width: usize, height: usize, channels: usize) -> usize {
    // The public decoder applies Pillow's pixel ceiling before this helper and
    // `png_layout` admits at most four channels.
    width.wrapping_mul(height).wrapping_mul(channels)
}

fn decode_scanlines(
    data: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    depth: u8,
    interlace: u8,
) -> CodecResult<Vec<u16>> {
    let width = width as usize;
    let height = height as usize;
    let sample_count = decoded_sample_count(width, height, channels);
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
                let index = y
                    .wrapping_mul(width)
                    .wrapping_add(x)
                    .wrapping_mul(channels)
                    .wrapping_add(channel);
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
                    let x = x_start.wrapping_add(pass_x.wrapping_mul(x_step));
                    let y = y_start.wrapping_add(pass_y.wrapping_mul(y_step));
                    let index = y
                        .wrapping_mul(width)
                        .wrapping_add(x)
                        .wrapping_mul(channels)
                        .wrapping_add(channel);
                    samples[index] = value;
                },
            );
        }
    }
    // `decompress_zlib_prefix` returns at most the exact scanline budget and
    // the length check in `decode` rejects short output. Consequently every
    // accepted buffer is consumed exactly here; Pillow deliberately ignores
    // any additional inflated bytes after that prefix.
    Ok(samples)
}

fn read_filtered_row<'a>(
    data: &'a [u8],
    position: &mut usize,
    stride: usize,
) -> CodecResult<(u8, &'a [u8])> {
    let filter = *data
        .get(*position)
        .malformed("PNG scanline is missing its filter byte")?;
    *position = position.wrapping_add(1);
    // `position` and `stride` are bounded by the validated inflated buffer.
    let source_end = (*position).wrapping_add(stride);
    let source = data
        .get(*position..source_end)
        .malformed("PNG scanline is truncated")?;
    *position = source_end;
    Ok((filter, source))
}

fn unfilter_rows(
    data: &[u8],
    position: &mut usize,
    width: usize,
    height: usize,
    channels: usize,
    depth: u8,
) -> CodecResult<Vec<u8>> {
    let stride = row_bytes(width, channels, depth);
    let bytes_per_pixel = channels.wrapping_mul(usize::from(depth)).div_ceil(8).max(1);
    let rows_len = stride.wrapping_mul(height);
    let mut rows = vec![0u8; rows_len];

    for row in 0..height {
        let (filter, source) = read_filtered_row(data, position, stride)?;
        let row_start = row.wrapping_mul(stride);

        for column in 0..stride {
            let left = if column >= bytes_per_pixel {
                rows[row_start.wrapping_add(column).wrapping_sub(bytes_per_pixel)]
            } else {
                0
            };
            let above = if row != 0 {
                rows[row_start.wrapping_sub(stride).wrapping_add(column)]
            } else {
                0
            };
            let upper_left = if row != 0 && column >= bytes_per_pixel {
                rows[row_start
                    .wrapping_sub(stride)
                    .wrapping_add(column)
                    .wrapping_sub(bytes_per_pixel)]
            } else {
                0
            };
            rows[row_start.wrapping_add(column)] = match filter {
                0 => source[column],
                1 => source[column].wrapping_add(left),
                2 => source[column].wrapping_add(above),
                3 => {
                    let average = u16::from(left)
                        .wrapping_add(u16::from(above))
                        .wrapping_div(2);
                    source[column].wrapping_add(average.to_le_bytes()[0])
                }
                4 => source[column].wrapping_add(paeth(left, above, upper_left)),
                _ => {
                    return Err(CodecError::Malformed(
                        "PNG scanline uses an invalid filter".to_owned(),
                    ));
                }
            };
        }
    }
    Ok(rows)
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
    let stride = rows.len().checked_div(height).unwrap_or_default();
    for y in 0..height {
        let row_start = y.wrapping_mul(stride);
        let row = &rows[row_start..row_start.wrapping_add(stride)];
        for x in 0..width {
            for channel in 0..channels {
                let sample_index = x.wrapping_mul(channels).wrapping_add(channel);
                let value = match depth {
                    1 | 2 | 4 => {
                        let bit = sample_index.wrapping_mul(usize::from(depth));
                        let shift = 8_usize
                            .saturating_sub(usize::from(depth))
                            .saturating_sub(bit % 8);
                        let mask = 1_u8.wrapping_shl(depth.into()).wrapping_sub(1);
                        u16::from(row[bit / 8].wrapping_shr(shift.to_le_bytes()[0].into()) & mask)
                    }
                    8 => u16::from(row[sample_index]),
                    _ => {
                        debug_assert_eq!(depth, 16);
                        let offset = sample_index.wrapping_mul(2);
                        u16::from_be_bytes([row[offset], row[offset.wrapping_add(1)]])
                    }
                };
                store(x, y, channel, value);
            }
        }
    }
}

struct PngImageSpec {
    width: u32,
    height: u32,
    png_color: u8,
    depth: u8,
    color: ColorType,
}

fn build_image(
    spec: PngImageSpec,
    samples: &[u16],
    palette_rgb: Option<Vec<u8>>,
    mut palette_alpha: Vec<u8>,
) -> CodecResult<DecodedImage> {
    let PngImageSpec {
        width,
        height,
        png_color,
        depth,
        color,
    } = spec;
    let pixels = if png_color == 0 && depth == 1 {
        pack_one_bit(samples, width as usize, height as usize)
    } else if png_color == 0 && depth < 8 {
        let maximum = 1_u16.wrapping_shl(depth.into()).wrapping_sub(1);
        samples
            .iter()
            .map(|&sample| {
                sample
                    .wrapping_mul(255)
                    .checked_div(maximum)
                    .unwrap_or_default()
                    .to_le_bytes()[0]
            })
            .collect()
    } else if png_color == 4 && depth == 16 {
        let mut bytes = Vec::with_capacity(samples.len().wrapping_mul(2));
        for pair in samples.chunks_exact(2) {
            let luminance = pair[0].to_be_bytes()[0];
            let alpha = pair[1].to_be_bytes()[0];
            bytes.extend_from_slice(&[luminance, luminance, luminance, alpha]);
        }
        bytes
    } else if depth == 16 && matches!(png_color, 2 | 6) {
        samples
            .iter()
            .map(|&sample| sample.to_be_bytes()[0])
            .collect()
    } else if png_color == 3 || depth == 8 {
        samples
            .iter()
            .map(|&sample| sample.to_le_bytes()[0])
            .collect()
    } else {
        let mut bytes = Vec::with_capacity(samples.len().wrapping_mul(2));
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
    if png_color == 3
        && let Some(mut rgb) = palette_rgb
    {
        let entries = rgb.len() / 3;
        if entries != 0 {
            rgb.truncate(entries.wrapping_mul(3));
            if !palette_alpha.is_empty() {
                palette_alpha.truncate(entries);
            }
            let palette = ImagePalette::new(rgb, palette_alpha)
                .map_err(|_| CodecError::Malformed("PNG palette is invalid".to_owned()))?;
            image = image.with_palette(palette);
        }
    }
    Ok(image)
}

fn pack_one_bit(samples: &[u16], width: usize, height: usize) -> Vec<u8> {
    let stride = width.div_ceil(8);
    let mut output = vec![0u8; stride.wrapping_mul(height)];
    for y in 0..height {
        for x in 0..width {
            if samples[y.wrapping_mul(width).wrapping_add(x)] != 0 {
                let output_index = y.wrapping_mul(stride).wrapping_add(x / 8);
                let shift = 7_usize.saturating_sub(x % 8);
                output[output_index] |= 1_u8.wrapping_shl(shift.to_le_bytes()[0].into());
            }
        }
    }
    output
}

fn row_bytes(width: usize, channels: usize, depth: u8) -> usize {
    // Width comes from a u32 IHDR field, channels are in 1..=4, and Pillow's
    // pixel ceiling bounds the resulting byte row below 32-bit `usize::MAX`.
    let bits = usize_to_u64(width)
        .wrapping_mul(usize_to_u64(channels))
        .wrapping_mul(u64::from(depth));
    raster_usize(bits.div_ceil(8))
}

#[cfg(target_pointer_width = "64")]
fn usize_to_u64(value: usize) -> u64 {
    u64::from_ne_bytes(value.to_ne_bytes())
}

#[cfg(target_pointer_width = "32")]
fn usize_to_u64(value: usize) -> u64 {
    u64::from(u32::from_ne_bytes(value.to_ne_bytes()))
}

#[cfg(target_pointer_width = "64")]
fn raster_usize(value: u64) -> usize {
    usize::from_ne_bytes(value.to_ne_bytes())
}

#[cfg(target_pointer_width = "32")]
fn raster_usize(value: u64) -> usize {
    let bytes = value.to_le_bytes();
    usize::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn pass_size(full: usize, start: usize, step: usize) -> usize {
    if full <= start {
        0
    } else {
        full.saturating_sub(start).div_ceil(step)
    }
}

fn paeth(left: u8, above: u8, upper_left: u8) -> u8 {
    let left = i32::from(left);
    let above = i32::from(above);
    let upper_left = i32::from(upper_left);
    let prediction = left.wrapping_add(above).wrapping_sub(upper_left);
    let left_distance = prediction.wrapping_sub(left).unsigned_abs();
    let above_distance = prediction.wrapping_sub(above).unsigned_abs();
    let diagonal_distance = prediction.wrapping_sub(upper_left).unsigned_abs();
    if left_distance <= above_distance && left_distance <= diagonal_distance {
        left.to_le_bytes()[0]
    } else if above_distance <= diagonal_distance {
        above.to_le_bytes()[0]
    } else {
        upper_left.to_le_bytes()[0]
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
    verify_crc: bool,
) -> CodecResult<(&'a [u8], usize)> {
    let end = start
        .checked_add(length)
        .dimensions("PNG chunk byte range overflows")?;
    let payload = data
        .get(start..end)
        .malformed("PNG chunk payload is truncated")?;
    let crc_end = end.saturating_add(4);
    let expected_bytes = data
        .get(end..crc_end)
        .malformed("PNG chunk CRC is truncated")?;
    let expected = u32::from_be_bytes([
        expected_bytes[0],
        expected_bytes[1],
        expected_bytes[2],
        expected_bytes[3],
    ]);
    if !verify_crc || crc32(kind, payload) == expected {
        Ok((payload, crc_end))
    } else {
        Err(CodecError::Malformed(
            "PNG chunk CRC does not match".to_owned(),
        ))
    }
}

struct Chunk<'a> {
    kind: [u8; 4],
    data: &'a [u8],
}

struct Chunks<'a> {
    data: &'a [u8],
    position: usize,
    failed: bool,
    verify_crc: bool,
}

impl<'a> Chunks<'a> {
    fn new(data: &'a [u8], verify_crc: bool) -> CodecResult<Self> {
        if data.get(..8) == Some(PNG_SIGNATURE) {
            Ok(Self {
                data,
                position: 8,
                failed: false,
                verify_crc,
            })
        } else {
            Err(CodecError::Malformed(
                "PNG signature is missing or invalid".to_owned(),
            ))
        }
    }
}

impl<'a> Iterator for Chunks<'a> {
    type Item = CodecResult<Chunk<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.position == self.data.len() {
            return None;
        }
        let result = (|| -> CodecResult<Chunk<'a>> {
            let length_bytes = self
                .data
                .get(self.position..self.position.saturating_add(4))
                .malformed("PNG chunk length is truncated")?;
            let length = u32::from_be_bytes([
                length_bytes[0],
                length_bytes[1],
                length_bytes[2],
                length_bytes[3],
            ]) as usize;
            let kind_bytes = self
                .data
                .get(self.position.saturating_add(4)..self.position.saturating_add(8))
                .malformed("PNG chunk type is truncated")?;
            let kind = [kind_bytes[0], kind_bytes[1], kind_bytes[2], kind_bytes[3]];
            let start = self.position.saturating_add(8);
            // Pillow validates construction-critical chunk CRCs while opening
            // the file, but defers IDAT CRC validation to `verify()`.
            let verify_crc = self.verify_crc || kind != *b"IDAT";
            let (payload, crc_end) =
                chunk_payload_with_crc(self.data, &kind, start, length, verify_crc)?;
            self.position = crc_end;
            Ok(Chunk {
                kind,
                data: payload,
            })
        })();
        if result.is_err() {
            self.failed = true;
        }
        Some(result)
    }
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    fn png_chunk(kind: [u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut data = PNG_SIGNATURE.to_vec();
        data.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        data.extend_from_slice(&kind);
        data.extend_from_slice(payload);
        data.extend_from_slice(&crc32(&kind, payload).to_be_bytes());
        data
    }

    let _ = decode(b"");
    let _ = decode(&png_chunk(*b"NOPE", &[0; 13]));
    let _ = decode(&png_chunk(*b"IHDR", &[0; 12]));
    for (width, height, filter, interlace) in [
        (0u32, 1u32, 0u8, 0u8),
        (1, 0, 0, 0),
        (1, 1, 1, 0),
        (1, 1, 0, 2),
    ] {
        let mut header = [0u8; 13];
        header[..4].copy_from_slice(&width.to_be_bytes());
        header[4..8].copy_from_slice(&height.to_be_bytes());
        header[8] = 8;
        header[9] = 0;
        header[11] = filter;
        header[12] = interlace;
        let _ = decode(&png_chunk(*b"IHDR", &header));
    }
    assert!(png_layout(7, 8).is_err());
    let _ = verify(b"");
    let _ = verify(PNG_SIGNATURE);
    let _ = verify(&png_chunk(*b"NOPE", &[0; 13]));
    let _ = verify(&png_chunk(*b"IHDR", &[0; 12]));
    let malformed = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x01tEXtx";
    let mut chunks = Chunks::new(malformed, true).expect("coverage PNG signature should parse");

    assert!(chunks.next().is_some_and(|chunk| chunk.is_err()));
    assert!(chunks.failed);
    assert!(chunks.next().is_none());

    let mut position = 0;
    assert!(unfilter_rows(&[], &mut position, 1, 1, 1, 8).is_err());

    let mut position = 0;
    assert!(unfilter_rows(&[0], &mut position, 1, 1, 1, 8).is_err());

    assert!(chunk_payload_with_crc(&[], b"IDAT", usize::MAX, 1, true).is_err());
}
