//! ICO encoder — wraps a BMP DIB inside an ICO container.
//!
//! ICO (Icon) files store one or more images. This encoder writes a single
//! 32-bit BGRA entry, wrapping the pixel data in a BITMAPINFOHEADER + AND
//! mask. Supports RGBA8 (4 bytes/pixel), RGB8 (converts to RGBA), and L8
//! (converts to RGBA).
use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage};
/// Encode a `DecodedImage` as an ICO file (single 32-bit BGRA entry).
///
/// The output is a valid ICO container with one directory entry pointing to
/// BMP/DIB data:
/// - ICO header (6 bytes)
/// - Directory entry (16 bytes)
/// - BITMAPINFOHEADER (40 bytes)
/// - BGRA pixel data (bottom-up)
/// - AND mask (all zeros, for full opacity)
///
/// Supported input color types: `Rgba8`, `Rgb8`, `L8`. Other types return
/// `None`.
pub fn encode(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    let w = img.width as usize;
    let h = img.height as usize;
    if w == 0 || h == 0 || w > 256 || h > 256 {
        return None;
    }
    if opts.extra.get("entry_type").map(String::as_str) == Some("bmp") {
        return encode_bmp_entry(img, opts);
    }
    encode_png_entries(img, opts)
}

fn encode_png_entries(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    let mut sizes = if let Some(value) = opts.extra.get("sizes") {
        parse_sizes(value)?
    } else {
        [16, 24, 32, 48, 64, 128, 256]
            .into_iter()
            .map(|size| (size, size))
            .collect()
    };
    sizes.retain(|&(width, height)| {
        width <= img.width as usize
            && height <= img.height as usize
            && width <= 256
            && height <= 256
    });
    sizes.sort_unstable();
    sizes.dedup();
    if sizes.is_empty() {
        return None;
    }

    let mut frames = Vec::with_capacity(sizes.len());
    for &(width, height) in &sizes {
        let frame = if (width, height) == (img.width as usize, img.height as usize) {
            img.clone()
        } else {
            resize_lanczos(img, width, height)?
        };
        frames.push(crate::codecs::png::encode::encode(
            &frame,
            &EncodeOptions::default(),
        )?);
    }

    let directory_bytes = sizes.len().checked_mul(16)?;
    let mut offset = 6usize.checked_add(directory_bytes)?;
    let total = frames
        .iter()
        .try_fold(offset, |length, frame| length.checked_add(frame.len()))?;
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(&[0, 0, 1, 0]);
    output.extend_from_slice(&u16::try_from(sizes.len()).ok()?.to_le_bytes());
    for (&(width, height), frame) in sizes.iter().zip(&frames) {
        output.push(if width == 256 {
            0
        } else {
            u8::try_from(width).ok()?
        });
        output.push(if height == 256 {
            0
        } else {
            u8::try_from(height).ok()?
        });
        output.extend_from_slice(&[0, 0, 0, 0]);
        output.extend_from_slice(&32u16.to_le_bytes());
        output.extend_from_slice(&u32::try_from(frame.len()).ok()?.to_le_bytes());
        output.extend_from_slice(&u32::try_from(offset).ok()?.to_le_bytes());
        offset = offset.checked_add(frame.len())?;
    }
    for frame in frames {
        output.extend_from_slice(&frame);
    }
    Some(output)
}

fn resize_lanczos(img: &DecodedImage, width: usize, height: usize) -> Option<DecodedImage> {
    let channels = match img.color {
        ColorType::L8 => 1,
        ColorType::Rgb8 => 3,
        ColorType::Rgba8 => 4,
        _ => return None,
    };
    let source_width = usize::try_from(img.width).ok()?;
    let source_height = usize::try_from(img.height).ok()?;
    let mut pixels = img.pixels.clone();
    if channels == 4 {
        for pixel in pixels.chunks_exact_mut(4) {
            let alpha = u32::from(pixel[3]);
            for channel in &mut pixel[..3] {
                let product = u32::from(*channel).checked_mul(alpha)?.checked_add(128)?;
                *channel = u8::try_from(((product >> 8) + product) >> 8).ok()?;
            }
        }
    }

    let horizontal = resample_axis(&pixels, source_width, source_height, width, channels, true)?;
    let mut resized = resample_axis(&horizontal, width, source_height, height, channels, false)?;
    if channels == 4 {
        for pixel in resized.chunks_exact_mut(4) {
            let alpha = u32::from(pixel[3]);
            if alpha != 0 && alpha != 255 {
                for channel in &mut pixel[..3] {
                    *channel = u8::try_from((255 * u32::from(*channel) / alpha).min(255)).ok()?;
                }
            }
        }
    }
    Some(DecodedImage::new(
        u32::try_from(width).ok()?,
        u32::try_from(height).ok()?,
        resized,
        img.color,
    ))
}

fn resample_axis(
    pixels: &[u8],
    width: usize,
    height: usize,
    output_size: usize,
    channels: usize,
    horizontal: bool,
) -> Option<Vec<u8>> {
    let input_size = if horizontal { width } else { height };
    let coefficients = lanczos_coefficients(input_size, output_size)?;
    let (output_width, output_height) = if horizontal {
        (output_size, height)
    } else {
        (width, output_size)
    };
    let mut output = vec![
        0;
        output_width
            .checked_mul(output_height)?
            .checked_mul(channels)?
    ];
    for y in 0..output_height {
        for x in 0..output_width {
            let coefficient = &coefficients[if horizontal { x } else { y }];
            for channel in 0..channels {
                let mut sum = 1i64 << 21;
                for (index, &weight) in coefficient.weights.iter().enumerate() {
                    let source_x = if horizontal {
                        coefficient.start + index
                    } else {
                        x
                    };
                    let source_y = if horizontal {
                        y
                    } else {
                        coefficient.start + index
                    };
                    let source = (source_y.checked_mul(width)?.checked_add(source_x)?)
                        .checked_mul(channels)?
                        .checked_add(channel)?;
                    sum = sum.checked_add(i64::from(*pixels.get(source)?) * i64::from(weight))?;
                }
                let value = (sum >> 22).clamp(0, 255) as u8;
                let target = (y.checked_mul(output_width)?.checked_add(x)?)
                    .checked_mul(channels)?
                    .checked_add(channel)?;
                output[target] = value;
            }
        }
    }
    Some(output)
}

struct Coefficients {
    start: usize,
    weights: Vec<i32>,
}

fn lanczos_coefficients(input: usize, output: usize) -> Option<Vec<Coefficients>> {
    let scale = input as f64 / output as f64;
    let filter_scale = scale.max(1.0);
    let support = 3.0 * filter_scale;
    let mut coefficients = Vec::with_capacity(output);
    for out in 0..output {
        let center = (out as f64 + 0.5) * scale;
        let start = ((center - support + 0.5) as isize).max(0) as usize;
        let end = ((center + support + 0.5) as usize).min(input);
        let mut weights = Vec::with_capacity(end.checked_sub(start)?);
        let mut total = 0.0;
        for source in start..end {
            let distance = (source as f64 - center + 0.5) / filter_scale;
            let weight = lanczos(distance);
            weights.push(weight);
            total += weight;
        }
        let weights = weights
            .into_iter()
            .map(|weight| {
                let normalized = weight / total * ((1u64 << 22) as f64);
                if normalized < 0.0 {
                    (normalized - 0.5) as i32
                } else {
                    (normalized + 0.5) as i32
                }
            })
            .collect();
        coefficients.push(Coefficients { start, weights });
    }
    Some(coefficients)
}

fn lanczos(value: f64) -> f64 {
    if !(-3.0..3.0).contains(&value) {
        return 0.0;
    }
    sinc(value) * sinc(value / 3.0)
}

fn sinc(mut value: f64) -> f64 {
    if value == 0.0 {
        return 1.0;
    }
    value *= std::f64::consts::PI;
    value.sin() / value
}

fn encode_bmp_entry(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    let width = usize::try_from(img.width).ok()?;
    let height = usize::try_from(img.height).ok()?;
    let (bits, row_bytes, pixels) = match img.color {
        ColorType::Rgb8 => {
            let row_bytes = width.checked_mul(3)?.next_multiple_of(4);
            let mut pixels = Vec::with_capacity(row_bytes.checked_mul(height)?);
            for row in img.pixels.chunks_exact(width.checked_mul(3)?).rev() {
                for pixel in row.chunks_exact(3) {
                    pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
                }
                pixels.resize(pixels.len().checked_add(row_bytes - width * 3)?, 0);
            }
            (24u16, row_bytes, pixels)
        }
        ColorType::Rgba8 => {
            let row_bytes = width.checked_mul(4)?;
            let mut pixels = Vec::with_capacity(row_bytes.checked_mul(height)?);
            for row in img.pixels.chunks_exact(row_bytes).rev() {
                for pixel in row.chunks_exact(4) {
                    pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
                }
            }
            (32u16, row_bytes, pixels)
        }
        _ => return None,
    };
    let pixel_bytes = row_bytes.checked_mul(height)?;
    if pixels.len() != pixel_bytes {
        return None;
    }

    // Pillow 12.2.0 IcoImagePlugin.py:137-190 leaves `size` bound to the
    // final requested/default size when it writes a non-32-bit AND mask.
    // With the default size list this is 256x256 even for a 16x16 frame.
    let mask_dimensions = opts
        .extra
        .get("sizes")
        .and_then(|value| parse_last_size(value))
        .unwrap_or((256, 256));
    let mask_row_bytes = mask_dimensions.0.div_ceil(8);
    let mask_bytes = if bits == 32 {
        0
    } else {
        mask_row_bytes.checked_mul(mask_dimensions.1)?
    };
    let dib_bytes = 40usize.checked_add(pixel_bytes)?.checked_add(mask_bytes)?;
    let mut output = Vec::with_capacity(22usize.checked_add(dib_bytes)?);
    output.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
    output.push(if width == 256 {
        0
    } else {
        u8::try_from(width).ok()?
    });
    output.push(if height == 256 {
        0
    } else {
        u8::try_from(height).ok()?
    });
    output.extend_from_slice(&[0, 0, 0, 0]);
    output.extend_from_slice(&bits.to_le_bytes());
    output.extend_from_slice(&u32::try_from(dib_bytes).ok()?.to_le_bytes());
    output.extend_from_slice(&22u32.to_le_bytes());

    output.extend_from_slice(&40u32.to_le_bytes());
    output.extend_from_slice(&img.width.to_le_bytes());
    output.extend_from_slice(&img.height.checked_mul(2)?.to_le_bytes());
    output.extend_from_slice(&1u16.to_le_bytes());
    output.extend_from_slice(&bits.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&u32::try_from(pixel_bytes).ok()?.to_le_bytes());
    output.extend_from_slice(&3_780i32.to_le_bytes());
    output.extend_from_slice(&3_780i32.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&pixels);
    output.resize(output.len().checked_add(mask_bytes)?, 0);
    Some(output)
}

fn parse_last_size(value: &str) -> Option<(usize, usize)> {
    parse_sizes(value)?.pop()
}

fn parse_sizes(value: &str) -> Option<Vec<(usize, usize)>> {
    let numbers = value
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(str::parse::<usize>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if numbers.len() % 2 != 0 {
        return None;
    }
    Some(
        numbers
            .chunks_exact(2)
            .map(|pair| (pair[0], pair[1]))
            .collect(),
    )
}
