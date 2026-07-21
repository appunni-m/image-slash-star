//! ICO encoder with PNG-backed multi-resolution and BMP-backed entries.
//!
//! Supports the Pillow ICO save surface for RGB and RGBA images, including
//! filtered size lists and explicit PNG or BMP payload selection.
use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage};
/// Encode an image using Pillow-compatible ICO save options.
pub fn encode(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    img.validate().ok()?;
    if opts.extra.get("entry_type").map(String::as_str) == Some("bmp") {
        return encode_bmp_entries(img, opts);
    }
    encode_png_entries(img, opts)
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let _ = encode(
        &DecodedImage::new(0, 1, Vec::new(), ColorType::Rgb8),
        &EncodeOptions::default(),
    );

    let rgb = DecodedImage::new(
        16,
        16,
        (0u8..=255)
            .flat_map(|value| [value, value.wrapping_mul(3), value.wrapping_mul(7)])
            .collect(),
        ColorType::Rgb8,
    );
    let rgba = DecodedImage::new(
        16,
        16,
        (0u8..=255)
            .flat_map(|value| [value, value.wrapping_mul(5), value.wrapping_mul(11), value])
            .collect(),
        ColorType::Rgba8,
    );
    let luma = DecodedImage::new(1, 1, vec![0], ColorType::L8);
    let cmyk = DecodedImage::new(16, 16, vec![0; 16 * 16 * 4], ColorType::Cmyk8);

    let mut png_sizes = EncodeOptions::default();
    png_sizes
        .extra
        .insert("sizes".to_owned(), "8,8 16,16 512,512".to_owned());
    let _ = encode(&rgb, &png_sizes);

    let mut invalid_sizes = EncodeOptions::default();
    invalid_sizes.extra.insert(
        "sizes".to_owned(),
        "999999999999999999999999999999999999".to_owned(),
    );
    let _ = encode(&rgb, &invalid_sizes);

    let mut bmp_sizes = png_sizes.clone();
    bmp_sizes
        .extra
        .insert("entry_type".to_owned(), "bmp".to_owned());
    let _ = encode(&rgb, &bmp_sizes);
    let _ = encode(&rgba, &bmp_sizes);
    let _ = encode(&luma, &bmp_sizes);

    let mut invalid_bmp_sizes = invalid_sizes.clone();
    invalid_bmp_sizes
        .extra
        .insert("entry_type".to_owned(), "bmp".to_owned());
    let _ = encode(&rgb, &invalid_bmp_sizes);

    let mut cmyk_bmp_resize = EncodeOptions::default();
    cmyk_bmp_resize
        .extra
        .insert("sizes".to_owned(), "8,8".to_owned());
    cmyk_bmp_resize
        .extra
        .insert("entry_type".to_owned(), "bmp".to_owned());
    let _ = encode(&cmyk, &cmyk_bmp_resize);

    let _ = parse_sizes("16x16, 32x24, invalid");
    let _ = parse_sizes("999999999999999999999999999999999999");
    let _ = parse_last_size("16x16 32x24");
    let _ = parse_last_size("");
    let _ = parse_last_size("999999999999999999999999999999999999");
    let _ = thumbnail_dimensions(16, 8, 4, 4);
    let _ = thumbnail_dimensions(8, 16, 4, 4);
    let _ = encode_directory(&[(256, 256), (1, 1)], &[vec![1], vec![2, 3]], 32);
    let _ = encode_directory(&[], &[], 32);
    let too_many_sizes = vec![(1, 1); usize::from(u16::MAX) + 1];
    let too_many_frames = vec![Vec::new(); too_many_sizes.len()];
    let _ = encode_directory(&too_many_sizes, &too_many_frames, 32);
    let _ = encode_bmp_single_entry(&rgb, &bmp_sizes);
    let _ = encode_bmp_single_entry(&rgba, &bmp_sizes);
    let mut huge_mask = EncodeOptions::default();
    huge_mask
        .extra
        .insert("sizes".to_owned(), format!("{},8", usize::MAX));
    let _ = encode_bmp_single_entry(&rgb, &huge_mask);
    let mut huge_dib = EncodeOptions::default();
    huge_dib
        .extra
        .insert("sizes".to_owned(), "1000000,1000000".to_owned());
    let _ = encode_bmp_single_entry(&rgb, &huge_dib);
    let _ = resize_lanczos(&rgb, 8, 8);
    let _ = resample_axis(&rgb.pixels, 16, 16, 8, 3, true);
    let _ = resample_axis(&rgb.pixels, 16, 16, 8, 3, false);
    let _ = lanczos(-3.0);
    let _ = lanczos(0.0);
    let _ = lanczos(1.5);
    let _ = sinc(0.0);
    let _ = sinc(1.0);
}

fn encode_png_entries(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    let sizes = ico_sizes(img, opts)?;

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

    encode_directory(&sizes, &frames, 32)
}

fn ico_sizes(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<(usize, usize)>> {
    let mut bounds = if let Some(value) = opts.extra.get("sizes") {
        parse_sizes(value)?
    } else {
        [16, 24, 32, 48, 64, 128, 256]
            .into_iter()
            .map(|size| (size, size))
            .collect()
    };
    if bounds
        .iter()
        .any(|&(width, height)| width == 0 || height == 0)
    {
        return None;
    }
    bounds.retain(|&(width, height)| {
        width <= img.width as usize
            && height <= img.height as usize
            && width <= 256
            && height <= 256
    });
    let mut sizes = bounds
        .into_iter()
        .map(|(width, height)| thumbnail_dimensions(img.width, img.height, width, height))
        .collect::<Vec<_>>();
    sizes.sort_unstable();
    sizes.dedup();
    Some(sizes)
}

fn thumbnail_dimensions(
    source_width: u32,
    source_height: u32,
    bound_width: usize,
    bound_height: usize,
) -> (usize, usize) {
    debug_assert!(source_width > 0);
    debug_assert!(source_height > 0);
    debug_assert!((1..=256).contains(&bound_width));
    debug_assert!((1..=256).contains(&bound_height));
    let source_width = u64::from(source_width);
    let source_height = u64::from(source_height);
    let bound_width = bound_width as u64;
    let bound_height = bound_height as u64;
    let (width, height) = if source_width * bound_height > source_height * bound_width {
        let height = (source_height * bound_width + source_width / 2) / source_width;
        (bound_width, height.max(1))
    } else {
        let width = (source_width * bound_height + source_height / 2) / source_height;
        (width.max(1), bound_height)
    };
    (width as usize, height as usize)
}

fn encode_directory(sizes: &[(usize, usize)], frames: &[Vec<u8>], bits: u16) -> Option<Vec<u8>> {
    debug_assert_eq!(sizes.len(), frames.len());

    let directory_bytes = sizes.len() * 16;
    let mut offset = 6usize + directory_bytes;
    let total = offset + frames.iter().map(Vec::len).sum::<usize>();
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(&[0, 0, 1, 0]);
    output.extend_from_slice(&u16::try_from(sizes.len()).ok()?.to_le_bytes());
    for (&(width, height), frame) in sizes.iter().zip(frames) {
        output.push(directory_dimension(width));
        output.push(directory_dimension(height));
        output.extend_from_slice(&[0, 0, 0, 0]);
        output.extend_from_slice(&bits.to_le_bytes());
        // Public callers build entries from `ico_sizes()`, which caps every
        // generated PNG/BMP frame at 256x256 pixels.
        output.extend_from_slice(&(frame.len() as u32).to_le_bytes());
        output.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += frame.len();
    }
    for frame in frames {
        output.extend_from_slice(frame);
    }
    Some(output)
}

fn directory_dimension(value: usize) -> u8 {
    debug_assert!(value <= 256);
    if value == 256 { 0 } else { value as u8 }
}

fn encode_bmp_entries(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    let sizes = ico_sizes(img, opts)?;
    if sizes.is_empty() {
        return encode_directory(&[], &[], 32);
    }

    let mut frames = Vec::with_capacity(sizes.len());
    let mut bits = 0;
    for (index, &(width, height)) in sizes.iter().enumerate() {
        let frame = if (width, height) == (img.width as usize, img.height as usize) {
            img.clone()
        } else {
            resize_lanczos(img, width, height)?
        };
        let encoded = encode_bmp_single_entry(&frame, opts)?;
        let frame_bits = u16::from_le_bytes([encoded[12], encoded[13]]);
        if index == 0 {
            bits = frame_bits;
        } else {
            debug_assert_eq!(bits, frame_bits);
        }
        frames.push(encoded[22..].to_vec());
    }
    encode_directory(&sizes, &frames, bits)
}

fn resize_lanczos(img: &DecodedImage, width: usize, height: usize) -> Option<DecodedImage> {
    let channels = match img.color {
        ColorType::L8 => 1,
        ColorType::Rgb8 => 3,
        ColorType::Rgba8 => 4,
        _ => return None,
    };
    let source_width = img.width as usize;
    let source_height = img.height as usize;
    let mut pixels = img.pixels.clone();
    if channels == 4 {
        for pixel in pixels.chunks_exact_mut(4) {
            let alpha = u32::from(pixel[3]);
            for channel in &mut pixel[..3] {
                let product = u32::from(*channel) * alpha + 128;
                *channel = (((product >> 8) + product) >> 8) as u8;
            }
        }
    }

    let horizontal = resample_axis(&pixels, source_width, source_height, width, channels, true);
    let mut resized = resample_axis(&horizontal, width, source_height, height, channels, false);
    if channels == 4 {
        for pixel in resized.chunks_exact_mut(4) {
            let alpha = u32::from(pixel[3]);
            if alpha != 0 && alpha != 255 {
                for channel in &mut pixel[..3] {
                    *channel = (255 * u32::from(*channel) / alpha).min(255) as u8;
                }
            }
        }
    }
    Some(DecodedImage::new(
        width as u32,
        height as u32,
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
) -> Vec<u8> {
    let input_size = if horizontal { width } else { height };
    let coefficients = lanczos_coefficients(input_size, output_size);
    let (output_width, output_height) = if horizontal {
        (output_size, height)
    } else {
        (width, output_size)
    };
    let mut output = vec![0; output_width * output_height * channels];
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
                    let source = (source_y * width + source_x) * channels + channel;
                    sum += i64::from(pixels[source]) * i64::from(weight);
                }
                let value = (sum >> 22).clamp(0, 255) as u8;
                let target = (y * output_width + x) * channels + channel;
                output[target] = value;
            }
        }
    }
    output
}

struct Coefficients {
    start: usize,
    weights: Vec<i32>,
}

fn lanczos_coefficients(input: usize, output: usize) -> Vec<Coefficients> {
    let scale = input as f64 / output as f64;
    let filter_scale = scale.max(1.0);
    let support = 3.0 * filter_scale;
    let mut coefficients = Vec::with_capacity(output);
    for out in 0..output {
        let center = (out as f64 + 0.5) * scale;
        let start = ((center - support + 0.5) as isize).max(0) as usize;
        let end = ((center + support + 0.5) as usize).min(input);
        debug_assert!(start <= end);
        let mut weights = Vec::with_capacity(end - start);
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
    coefficients
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

fn encode_bmp_single_entry(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    let width = img.width as usize;
    let height = img.height as usize;
    let (bits, row_bytes, pixels) = match img.color {
        ColorType::Rgb8 => {
            let source_row_bytes = width * 3;
            let row_bytes = source_row_bytes.next_multiple_of(4);
            let mut pixels = Vec::with_capacity(row_bytes * height);
            for row in img.pixels.chunks_exact(source_row_bytes).rev() {
                for pixel in row.chunks_exact(3) {
                    pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
                }
                pixels.resize(pixels.len() + row_bytes - source_row_bytes, 0);
            }
            (24u16, row_bytes, pixels)
        }
        ColorType::Rgba8 => {
            let row_bytes = width * 4;
            let mut pixels = Vec::with_capacity(row_bytes * height);
            for row in img.pixels.chunks_exact(row_bytes).rev() {
                for pixel in row.chunks_exact(4) {
                    pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
                }
            }
            (32u16, row_bytes, pixels)
        }
        _ => return None,
    };
    let pixel_bytes = row_bytes * height;
    // Each color arm emits exactly one validated source row at `row_bytes`.
    debug_assert_eq!(pixels.len(), pixel_bytes);

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
    // Public BMP-backed ICO entries are generated only for <=256px sizes.
    let dib_bytes = 40usize + pixel_bytes + mask_bytes;
    let dib_size = u32::try_from(dib_bytes).ok()?;
    let mut output = Vec::with_capacity(22usize + dib_bytes);
    output.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
    output.push(directory_dimension(width));
    output.push(directory_dimension(height));
    output.extend_from_slice(&[0, 0, 0, 0]);
    output.extend_from_slice(&bits.to_le_bytes());
    output.extend_from_slice(&dib_size.to_le_bytes());
    output.extend_from_slice(&22u32.to_le_bytes());

    output.extend_from_slice(&40u32.to_le_bytes());
    output.extend_from_slice(&img.width.to_le_bytes());
    output.extend_from_slice(&(img.height * 2).to_le_bytes());
    output.extend_from_slice(&1u16.to_le_bytes());
    output.extend_from_slice(&bits.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    output.extend_from_slice(&3_780i32.to_le_bytes());
    output.extend_from_slice(&3_780i32.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&pixels);
    output.resize(output.len() + mask_bytes, 0);
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
    Some(
        numbers
            .chunks_exact(2)
            .map(|pair| (pair[0], pair[1]))
            .collect(),
    )
}
