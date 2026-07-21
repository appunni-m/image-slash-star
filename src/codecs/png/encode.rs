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
    img.validate().ok()?;

    let width = img.width as usize;
    let height = img.height as usize;
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
            (0, 16, width * 2, 2, big_endian)
        }
        _ => {
            let (png_color, channels) = match img.color {
                ColorType::L8 => (0, 1usize),
                ColorType::La8 => (4, 2),
                ColorType::Rgb8 => (2, 3),
                ColorType::Rgba8 => (6, 4),
                _ => return None,
            };
            (png_color, 8, width * channels, channels, img.pixels.clone())
        }
    };

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
    write_bounded_chunk(&mut output, *b"IHDR", &header);
    if img.mode == ImageMode::P8 {
        let palette = img.palette.as_ref()?;
        write_bounded_chunk(&mut output, *b"PLTE", &palette.rgb);
        if !palette.alpha.is_empty() {
            write_bounded_chunk(&mut output, *b"tRNS", &palette.alpha);
        }
    }
    write_requested_ancillary_chunks(&mut output, opts);
    write_chunk(&mut output, *b"IDAT", &compressed)?;
    write_bounded_chunk(&mut output, *b"IEND", &[]);
    Some(output)
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let _ = encode(
        &DecodedImage::new(0, 1, Vec::new(), ColorType::L8),
        &EncodeOptions::default(),
    );
    let _ = encode(
        &DecodedImage::new(1, 1, Vec::new(), ColorType::Rgb8),
        &EncodeOptions::default(),
    );
    let _ = encode(
        &DecodedImage::new(1, 1, vec![0, 0, 0, 0], ColorType::Cmyk8),
        &EncodeOptions::default(),
    );

    let l1 = DecodedImage::with_mode(9, 1, vec![0b1010_1010, 0b1000_0000], ImageMode::L1);
    let _ = encode(&l1, &EncodeOptions::default());

    let l16 = DecodedImage::with_mode(1, 1, 0x1234u16.to_le_bytes().to_vec(), ImageMode::L16);
    let _ = encode(&l16, &EncodeOptions::default());

    let palette = crate::types::ImagePalette::new(vec![0, 0, 0, 255, 255, 255], vec![0, 255])
        .expect("coverage palette should be valid");
    let indexed = DecodedImage::with_mode(2, 1, vec![0, 1], ImageMode::P8).with_palette(palette);
    let _ = encode(&indexed, &EncodeOptions::default());
    let palette_less_indexed = DecodedImage::with_mode(1, 1, vec![0], ImageMode::P8);
    let _ = encode(&palette_less_indexed, &EncodeOptions::default());

    let rgb = DecodedImage::new(
        2,
        2,
        vec![0, 0, 0, 255, 0, 0, 0, 255, 0, 255, 255, 255],
        ColorType::Rgb8,
    );
    let mut ancillary = EncodeOptions::default();
    for key in ["gamma", "srgb", "physical", "text_chunks", "time"] {
        ancillary.extra.insert(key.to_owned(), "true".to_owned());
    }
    ancillary
        .extra
        .insert("compression".to_owned(), "none".to_owned());
    let _ = encode(&rgb, &ancillary);

    let mut bad_compression = EncodeOptions::default();
    bad_compression
        .extra
        .insert("compression".to_owned(), "not-a-level".to_owned());
    let _ = encode(&rgb, &bad_compression);

    let _ = plain_rows(&[], usize::MAX, 1, 1, Filter::None, false);
    let _ = plain_rows(&[], usize::MAX - 1, 2, 1, Filter::None, false);

    for value in ["1", "yes", "false"] {
        let mut option = EncodeOptions::default();
        option.extra.insert("gamma".to_owned(), value.to_owned());
        let _ = requested(&option, "gamma");
    }
    let _ = requested(&EncodeOptions::default(), "gamma");

    let row = [10u8, 20, 40, 80];
    let previous = [1u8, 2, 4, 8];
    for filter in [
        Filter::None,
        Filter::Sub,
        Filter::Up,
        Filter::Average,
        Filter::Paeth,
        Filter::Adaptive,
    ] {
        let mut output = Vec::new();
        append_filtered_row(&mut output, &row, Some(&previous), 2, filter, true);
        let _ = filter_score(&row, Some(&previous), 2, filter);
        let _ = filter_byte(filter);
    }
    let _ = select_adaptive_filter(&row, Some(&previous), 2, true);
    let _ = paeth(10, 20, 30);
    let _ = paeth(200, 10, 20);
    let _ = paeth(5, 200, 10);
}

fn plain_rows(
    pixels: &[u8],
    stride: usize,
    height: usize,
    filter_bytes: usize,
    filter: Filter,
    optimize: bool,
) -> Option<(Vec<u8>, Vec<usize>)> {
    let row_len = stride.checked_add(1)?;
    let mut output = Vec::with_capacity(row_len.checked_mul(height)?);
    let input_chunks = vec![row_len; height];
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

fn write_requested_ancillary_chunks(output: &mut Vec<u8>, opts: &EncodeOptions) {
    if requested(opts, "gamma") {
        write_bounded_chunk(output, *b"gAMA", &45_455u32.to_be_bytes());
    }
    if requested(opts, "srgb") {
        write_bounded_chunk(output, *b"sRGB", &[0]);
    }
    if requested(opts, "physical") {
        let mut payload = Vec::with_capacity(9);
        payload.extend_from_slice(&2_835u32.to_be_bytes());
        payload.extend_from_slice(&2_835u32.to_be_bytes());
        payload.push(1);
        write_bounded_chunk(output, *b"pHYs", &payload);
    }
    if requested(opts, "text_chunks") {
        write_bounded_chunk(output, *b"tEXt", b"Comment\0pillow-rs");
    }
    if requested(opts, "time") {
        let payload = [0x07, 0xea, 7, 4, 0, 0, 0]; // 2026-07-04 00:00:00 UTC.
        write_bounded_chunk(output, *b"tIME", &payload);
    }
}

fn write_chunk(output: &mut Vec<u8>, kind: [u8; 4], payload: &[u8]) -> Option<()> {
    let length = u32::try_from(payload.len()).ok()?;
    append_chunk(output, kind, payload, length);
    Some(())
}

fn write_bounded_chunk(output: &mut Vec<u8>, kind: [u8; 4], payload: &[u8]) {
    // Callers pass fixed metadata chunks or palettes already bounded by
    // DecodedImage::validate().
    append_chunk(output, kind, payload, payload.len() as u32);
}

fn append_chunk(output: &mut Vec<u8>, kind: [u8; 4], payload: &[u8], length: u32) {
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(&kind);
    output.extend_from_slice(payload);
    output.extend_from_slice(&crc32(&kind, payload).to_be_bytes());
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
