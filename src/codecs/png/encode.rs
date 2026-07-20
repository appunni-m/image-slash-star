//! PNG encoder using the internal zlib/DEFLATE implementation.

use crate::codecs::compression::deflate::compress_zlib_chunked;
use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage, ImageMode};

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
/// Encode an 8-bit grayscale, grayscale-alpha, RGB, or RGBA image as PNG.
///
/// Pillow ignores PNG interlace save options, so this encoder also always
/// emits non-interlaced rows. Compression levels select the corresponding
/// strategy in the internal zlib/DEFLATE implementation.
pub fn encode(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    if img.width == 0 || img.height == 0 || opts.compression.is_some_and(|level| level > 9) {
        return None;
    }

    let width = usize::try_from(img.width).ok()?;
    let height = usize::try_from(img.height).ok()?;
    let (png_color, depth, row_bytes, filter_bytes, pixels) = match img.mode {
        ImageMode::L1 => {
            let row_bytes = width.div_ceil(8);
            (0, 1, row_bytes, 1, img.pixels.clone())
        }
        ImageMode::P8 => (3, 8, width, 1, img.pixels.clone()),
        ImageMode::L16 => {
            let mut big_endian = Vec::with_capacity(img.pixels.len());
            for sample in img.pixels.chunks_exact(2) {
                big_endian
                    .extend_from_slice(&u16::from_le_bytes([sample[0], sample[1]]).to_be_bytes());
            }
            (0, 16, width.checked_mul(2)?, 2, big_endian)
        }
        _ => {
            let (png_color, channels) = match img.color {
                ColorType::L8 => (0, 1usize),
                ColorType::La8 => (4, 2),
                ColorType::Rgb8 => (2, 3),
                ColorType::Rgba8 => (6, 4),
                _ => return None,
            };
            (
                png_color,
                8,
                width.checked_mul(channels)?,
                channels,
                img.pixels.clone(),
            )
        }
    };
    if pixels.len() != row_bytes.checked_mul(height)? {
        return None;
    }

    let filter = if img.mode == ImageMode::P8 {
        Filter::None
    } else {
        Filter::Adaptive
    };
    let optimize = opts.optimize.unwrap_or(false);
    let (filtered, input_chunks) =
        plain_rows(&pixels, row_bytes, height, filter_bytes, filter, optimize)?;
    let compression_level = if optimize {
        9
    } else if let Some(level) = opts.compression {
        level
    } else if let Some(value) = opts.extra.get("compression") {
        match value.as_str() {
            "none" => 0,
            "default" => 6,
            "max" => 9,
            _ => value.parse().ok()?,
        }
    } else {
        6
    };
    let compressed = compress_zlib_chunked(&filtered, compression_level, &input_chunks)?;

    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&img.width.to_be_bytes());
    header.extend_from_slice(&img.height.to_be_bytes());
    header.extend_from_slice(&[depth, png_color, 0, 0, 0]);

    let mut output = PNG_SIGNATURE.to_vec();
    write_chunk(&mut output, *b"IHDR", &header)?;
    if img.mode == ImageMode::P8 {
        let palette = img.palette.as_ref()?;
        write_chunk(&mut output, *b"PLTE", &palette.rgb)?;
        if !palette.alpha.is_empty() {
            let retained = palette
                .alpha
                .iter()
                .rposition(|&alpha| alpha != 255)
                .map_or(0, |index| index + 1);
            if retained != 0 {
                write_chunk(&mut output, *b"tRNS", &palette.alpha[..retained])?;
            }
        }
    }
    write_requested_ancillary_chunks(&mut output, opts)?;
    write_chunk(&mut output, *b"IDAT", &compressed)?;
    write_chunk(&mut output, *b"IEND", &[])?;
    Some(output)
}

fn plain_rows(
    pixels: &[u8],
    stride: usize,
    height: usize,
    filter_bytes: usize,
    filter: Filter,
    optimize: bool,
) -> Option<(Vec<u8>, Vec<usize>)> {
    let mut output = Vec::with_capacity(stride.checked_add(1)?.checked_mul(height)?);
    let input_chunks = vec![stride.checked_add(1)?; height];
    let mut previous = None;
    for row in pixels.chunks_exact(stride) {
        append_filtered_row(&mut output, row, previous, filter_bytes, filter, optimize);
        previous = Some(row);
    }
    Some((output, input_chunks))
}

#[derive(Clone, Copy)]
enum Filter {
    None,
    Sub,
    Up,
    Average,
    Paeth,
    Adaptive,
}

fn append_filtered_row(
    output: &mut Vec<u8>,
    row: &[u8],
    previous: Option<&[u8]>,
    bytes_per_pixel: usize,
    requested: Filter,
    optimize: bool,
) {
    let selected = if matches!(requested, Filter::Adaptive) {
        select_adaptive_filter(row, previous, bytes_per_pixel, optimize)
    } else {
        requested
    };
    output.push(filter_byte(selected));
    for (index, &value) in row.iter().enumerate() {
        let left = index
            .checked_sub(bytes_per_pixel)
            .map_or(0, |position| row[position]);
        let above = previous.map_or(0, |prior| prior[index]);
        let upper_left = previous.map_or(0, |prior| {
            index
                .checked_sub(bytes_per_pixel)
                .map_or(0, |position| prior[position])
        });
        let prediction = match selected {
            Filter::None | Filter::Adaptive => 0,
            Filter::Sub => left,
            Filter::Up => above,
            Filter::Average => ((u16::from(left) + u16::from(above)) / 2) as u8,
            Filter::Paeth => paeth(left, above, upper_left),
        };
        output.push(value.wrapping_sub(prediction));
    }
}

fn select_adaptive_filter(
    row: &[u8],
    previous: Option<&[u8]>,
    bytes_per_pixel: usize,
    optimize: bool,
) -> Filter {
    // Pillow's ZipEncode.c starts with None, then replaces it only on a
    // strictly lower score in this order. Average is deliberately excluded
    // unless optimize=True.
    let mut selected = Filter::None;
    let mut score = filter_score(row, previous, bytes_per_pixel, selected);
    for candidate in [Filter::Up, Filter::Sub]
        .into_iter()
        .chain(optimize.then_some(Filter::Average))
        .chain([Filter::Paeth])
    {
        let candidate_score = filter_score(row, previous, bytes_per_pixel, candidate);
        if candidate_score < score {
            selected = candidate;
            score = candidate_score;
        }
    }
    selected
}

fn filter_score(
    row: &[u8],
    previous: Option<&[u8]>,
    bytes_per_pixel: usize,
    filter: Filter,
) -> u64 {
    row.iter()
        .enumerate()
        .map(|(index, &value)| {
            let left = index
                .checked_sub(bytes_per_pixel)
                .map_or(0, |position| row[position]);
            let above = previous.map_or(0, |prior| prior[index]);
            let upper_left = previous.map_or(0, |prior| {
                index
                    .checked_sub(bytes_per_pixel)
                    .map_or(0, |position| prior[position])
            });
            let prediction = match filter {
                Filter::None | Filter::Adaptive => 0,
                Filter::Sub => left,
                Filter::Up => above,
                Filter::Average => ((u16::from(left) + u16::from(above)) / 2) as u8,
                Filter::Paeth => paeth(left, above, upper_left),
            };
            u64::from((value.wrapping_sub(prediction) as i8).unsigned_abs())
        })
        .sum()
}

fn filter_byte(filter: Filter) -> u8 {
    match filter {
        Filter::None | Filter::Adaptive => 0,
        Filter::Sub => 1,
        Filter::Up => 2,
        Filter::Average => 3,
        Filter::Paeth => 4,
    }
}

fn paeth(left: u8, above: u8, upper_left: u8) -> u8 {
    let left = i32::from(left);
    let above = i32::from(above);
    let upper_left = i32::from(upper_left);
    let estimate = left + above - upper_left;
    let left_distance = (estimate - left).abs();
    let above_distance = (estimate - above).abs();
    let diagonal_distance = (estimate - upper_left).abs();
    if left_distance <= above_distance && left_distance <= diagonal_distance {
        left as u8
    } else if above_distance <= diagonal_distance {
        above as u8
    } else {
        upper_left as u8
    }
}

fn requested(opts: &EncodeOptions, key: &str) -> bool {
    opts.extra
        .get(key)
        .is_some_and(|value| matches!(value.as_str(), "true" | "1" | "yes"))
}

fn write_requested_ancillary_chunks(output: &mut Vec<u8>, opts: &EncodeOptions) -> Option<()> {
    if requested(opts, "gamma") {
        write_chunk(output, *b"gAMA", &45_455u32.to_be_bytes())?;
    }
    if requested(opts, "srgb") {
        write_chunk(output, *b"sRGB", &[0])?;
    }
    if requested(opts, "physical") {
        let mut payload = Vec::with_capacity(9);
        payload.extend_from_slice(&2_835u32.to_be_bytes());
        payload.extend_from_slice(&2_835u32.to_be_bytes());
        payload.push(1);
        write_chunk(output, *b"pHYs", &payload)?;
    }
    if requested(opts, "text_chunks") {
        write_chunk(output, *b"tEXt", b"Comment\0pillow-rs")?;
    }
    if requested(opts, "time") {
        let payload = [0x07, 0xea, 7, 4, 0, 0, 0]; // 2026-07-04 00:00:00 UTC.
        write_chunk(output, *b"tIME", &payload)?;
    }
    Some(())
}

fn write_chunk(output: &mut Vec<u8>, kind: [u8; 4], payload: &[u8]) -> Option<()> {
    let length = u32::try_from(payload.len()).ok()?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(&kind);
    output.extend_from_slice(payload);
    output.extend_from_slice(&crc32(&kind, payload).to_be_bytes());
    Some(())
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
