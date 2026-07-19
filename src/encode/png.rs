//! PNG encoder using the internal zlib/DEFLATE implementation.

use crate::compression::deflate::compress_zlib;
use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage};

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

/// Encode an 8-bit grayscale, grayscale-alpha, RGB, or RGBA image as PNG.
///
/// `interlace = true` emits Adam7 passes. Compression levels are accepted for
/// API compatibility; the current internal compressor emits deterministic
/// stored DEFLATE blocks while the compressed strategies are implemented.
pub fn encode(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    let (png_color, channels) = match img.color {
        ColorType::L8 => (0, 1usize),
        ColorType::La8 => (4, 2),
        ColorType::Rgb8 => (2, 3),
        ColorType::Rgba8 => (6, 4),
        _ => return None,
    };
    if img.width == 0 || img.height == 0 || opts.compression.is_some_and(|level| level > 9) {
        return None;
    }

    let width = usize::try_from(img.width).ok()?;
    let height = usize::try_from(img.height).ok()?;
    let expected = width.checked_mul(height)?.checked_mul(channels)?;
    if img.pixels.len() != expected {
        return None;
    }

    let interlaced = opts.interlace.unwrap_or(false);
    let filter = Filter::parse(opts.extra.get("filter").map(String::as_str))?;
    let filtered = if interlaced {
        adam7_rows(&img.pixels, width, height, channels, filter)?
    } else {
        plain_rows(&img.pixels, width, height, channels, filter)?
    };
    let compression_level = opts
        .compression
        .or_else(|| {
            opts.extra
                .get("compression")
                .and_then(|value| match value.as_str() {
                    "none" => Some(0),
                    "default" => Some(6),
                    "max" => Some(9),
                    _ => value.parse().ok(),
                })
        })
        .unwrap_or(6);
    let compressed = compress_zlib(&filtered, compression_level)?;

    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&img.width.to_be_bytes());
    header.extend_from_slice(&img.height.to_be_bytes());
    header.extend_from_slice(&[8, png_color, 0, 0, u8::from(interlaced)]);

    let mut output = PNG_SIGNATURE.to_vec();
    write_chunk(&mut output, *b"IHDR", &header)?;
    write_requested_ancillary_chunks(&mut output, opts)?;
    write_chunk(&mut output, *b"IDAT", &compressed)?;
    write_chunk(&mut output, *b"IEND", &[])?;
    Some(output)
}

fn plain_rows(
    pixels: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    filter: Filter,
) -> Option<Vec<u8>> {
    let stride = width.checked_mul(channels)?;
    let mut output = Vec::with_capacity(stride.checked_add(1)?.checked_mul(height)?);
    let mut previous = None;
    for row in pixels.chunks_exact(stride) {
        append_filtered_row(&mut output, row, previous, channels, filter);
        previous = Some(row);
    }
    Some(output)
}

fn adam7_rows(
    pixels: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    filter: Filter,
) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    for (x_start, y_start, x_step, y_step) in ADAM7 {
        if width <= x_start || height <= y_start {
            continue;
        }
        let mut previous = None::<Vec<u8>>;
        for y in (y_start..height).step_by(y_step) {
            let pass_width = (width - x_start).div_ceil(x_step);
            let mut row = Vec::with_capacity(pass_width.checked_mul(channels)?);
            for x in (x_start..width).step_by(x_step) {
                let start = (y.checked_mul(width)?.checked_add(x)?).checked_mul(channels)?;
                row.extend_from_slice(pixels.get(start..start + channels)?);
            }
            append_filtered_row(&mut output, &row, previous.as_deref(), channels, filter);
            previous = Some(row);
        }
    }
    Some(output)
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

impl Filter {
    fn parse(value: Option<&str>) -> Option<Self> {
        match value {
            None | Some("none" | "0") => Some(Self::None),
            Some("sub" | "1") => Some(Self::Sub),
            Some("up" | "2") => Some(Self::Up),
            Some("average" | "avg" | "3") => Some(Self::Average),
            Some("paeth" | "4") => Some(Self::Paeth),
            Some("adaptive") => Some(Self::Adaptive),
            Some(_) => None,
        }
    }
}

fn append_filtered_row(
    output: &mut Vec<u8>,
    row: &[u8],
    previous: Option<&[u8]>,
    bytes_per_pixel: usize,
    requested: Filter,
) {
    let selected = if matches!(requested, Filter::Adaptive) {
        [
            Filter::None,
            Filter::Sub,
            Filter::Up,
            Filter::Average,
            Filter::Paeth,
        ]
        .into_iter()
        .min_by_key(|&candidate| filter_score(row, previous, bytes_per_pixel, candidate))
        .unwrap_or(Filter::None)
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
        write_chunk(output, *b"tEXt", b"Comment\0pillow-rs-image")?;
    }
    if requested(opts, "time") {
        let payload = [0x07, 0xb2, 1, 1, 0, 0, 0]; // 1970-01-01 00:00:00 UTC.
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

#[cfg(test)]
mod tests {
    use super::encode;
    use crate::decode::png::decode;
    use crate::encode_options::EncodeOptions;
    use crate::types::{ColorType, DecodedImage};

    #[test]
    fn native_png_roundtrips_plain_and_adam7_rgba() {
        let width = 17;
        let height = 19;
        let pixels = (0..width * height * 4)
            .map(|index| ((index * 29 + index / 7) & 0xff) as u8)
            .collect::<Vec<_>>();
        let image = DecodedImage::new(width, height, pixels.clone(), ColorType::Rgba8);

        for interlace in [false, true] {
            let options = EncodeOptions {
                interlace: Some(interlace),
                ..EncodeOptions::default()
            };
            let encoded = encode(&image, &options).expect("PNG should encode");
            let decoded = decode(&encoded).expect("native PNG should decode");
            assert_eq!(decoded.pixels, pixels);
        }
    }
}
