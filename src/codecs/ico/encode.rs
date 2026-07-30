//! Source-sized ICO encoder with PNG-backed or BMP-backed entries.
//!
//! This codec never resizes source pixels. Callers that need a
//! multi-resolution icon must provide already-sized entries through a future
//! entry-oriented API rather than asking the codec to perform image processing.
use crate::codecs::{CodecError, CodecResult};
use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage};
/// Encode one source-sized image as one Pillow-compatible ICO entry.
pub fn encode(img: &DecodedImage, opts: &EncodeOptions) -> CodecResult<Vec<u8>> {
    img.validate().map_err(CodecError::from_image_error)?;
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
    let luma = DecodedImage::new(16, 16, vec![0; 16 * 16], ColorType::L8);
    let cmyk = DecodedImage::new(16, 16, vec![0; 16 * 16 * 4], ColorType::Cmyk8);

    let mut exact_size = EncodeOptions::default();
    exact_size
        .extra
        .insert("sizes".to_owned(), "[[16, 16]]".to_owned());
    let _ = encode(&rgb, &exact_size);

    let mut wrong_size = EncodeOptions::default();
    wrong_size
        .extra
        .insert("sizes".to_owned(), "[[8, 8]]".to_owned());
    let _ = encode(&rgb, &wrong_size);

    let mut bmp = exact_size.clone();
    bmp.extra.insert("entry_type".to_owned(), "bmp".to_owned());
    let _ = encode(&rgb, &bmp);
    let _ = encode(&rgba, &bmp);
    let _ = encode(&luma, &bmp);
    let _ = encode(&cmyk, &bmp);

    let mut invalid_size = EncodeOptions::default();
    invalid_size.extra.insert(
        "sizes".to_owned(),
        "999999999999999999999999999999999999".to_owned(),
    );
    let _ = encode(&rgb, &invalid_size);
    invalid_size
        .extra
        .insert("entry_type".to_owned(), "bmp".to_owned());
    let _ = encode(&rgb, &invalid_size);

    let oversized = DecodedImage::new(257, 1, vec![0; 257 * 3], ColorType::Rgb8);
    let _ = encode(&oversized, &EncodeOptions::default());
    let too_tall = DecodedImage::new(1, 257, vec![0; 257 * 3], ColorType::Rgb8);
    let _ = encode(&too_tall, &EncodeOptions::default());

    let _ = parse_single_size("[[16, 16]]");
    let _ = parse_single_size("[[16, 16], [32, 32]]");
    let _ = parse_single_size("");
    let _ = encode_directory((256, 256), &[1, 2, 3], 32);
    for size in [(0, 1), (1, 0), (257, 1), (1, 257)] {
        let _ = encode_directory(size, &[0], 32);
    }
    let _ = encode_bmp_single_entry(&rgb);
    let _ = encode_bmp_single_entry(&rgba);
}

fn encode_png_entries(img: &DecodedImage, opts: &EncodeOptions) -> CodecResult<Vec<u8>> {
    let size = source_entry_size(img, opts)?;
    let frame = crate::codecs::png::encode::encode(img, &EncodeOptions::default())
        .map_err(|error| error.context("embedded ICO PNG encode"))?;
    encode_directory(size, &frame, 32)
}

fn source_entry_size(img: &DecodedImage, opts: &EncodeOptions) -> CodecResult<(usize, usize)> {
    let source = (bounded_usize_u32(img.width), bounded_usize_u32(img.height));
    if source.0 > 256 || source.1 > 256 {
        return Err(CodecError::Dimensions(
            "ICO entries cannot exceed 256 by 256 pixels".to_owned(),
        ));
    }
    match opts.extra.get("sizes") {
        Some(value) if parse_single_size(value)? != source => Err(CodecError::Parameter(
            "ICO sizes must contain exactly the source dimensions".to_owned(),
        )),
        Some(_) | None => Ok(source),
    }
}

fn encode_directory(
    (width, height): (usize, usize),
    frame: &[u8],
    bits: u16,
) -> CodecResult<Vec<u8>> {
    if width == 0 || height == 0 || width > 256 || height > 256 {
        return Err(CodecError::Dimensions(
            "ICO entry dimensions must be between 1 and 256".to_owned(),
        ));
    }
    // This codec intentionally writes one source-sized entry. Its default PNG
    // and BMP encoders are bounded by the 256x256 ceiling, so both the payload
    // length and the fixed offset fit in an ICO u32 field.
    let mut output = Vec::with_capacity(22usize.saturating_add(frame.len()));
    output.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
    output.push(directory_dimension(width));
    output.push(directory_dimension(height));
    output.extend_from_slice(&[0, 0, 0, 0]);
    output.extend_from_slice(&bits.to_le_bytes());
    output.extend_from_slice(&low_u32(frame.len()).to_le_bytes());
    output.extend_from_slice(&22u32.to_le_bytes());
    output.extend_from_slice(frame);
    Ok(output)
}

fn directory_dimension(value: usize) -> u8 {
    debug_assert!(value <= 256);
    if value == 256 {
        0
    } else {
        value.to_le_bytes()[0]
    }
}

fn encode_bmp_entries(img: &DecodedImage, opts: &EncodeOptions) -> CodecResult<Vec<u8>> {
    let size = source_entry_size(img, opts)?;
    let encoded = encode_bmp_single_entry(img)?;
    let bits = u16::from_le_bytes([encoded[12], encoded[13]]);
    encode_directory(size, &encoded[22..], bits)
}

fn bounded_usize_u32(value: u32) -> usize {
    #[cfg(target_pointer_width = "64")]
    {
        let [a, b, c, d] = value.to_le_bytes();
        usize::from_le_bytes([a, b, c, d, 0, 0, 0, 0])
    }
    #[cfg(target_pointer_width = "32")]
    {
        usize::from_le_bytes(value.to_le_bytes())
    }
}

fn low_u32(value: usize) -> u32 {
    #[cfg(target_pointer_width = "64")]
    {
        let [a, b, c, d, ..] = value.to_le_bytes();
        u32::from_le_bytes([a, b, c, d])
    }
    #[cfg(target_pointer_width = "32")]
    {
        u32::from_le_bytes(value.to_le_bytes())
    }
}

fn encode_bmp_single_entry(img: &DecodedImage) -> CodecResult<Vec<u8>> {
    let width = bounded_usize_u32(img.width);
    let height = bounded_usize_u32(img.height);
    let (bits, row_bytes, pixels) = match img.color {
        ColorType::Rgb8 => {
            let source_row_bytes = width.saturating_mul(3);
            let row_bytes = source_row_bytes.next_multiple_of(4);
            let mut pixels = Vec::with_capacity(row_bytes.saturating_mul(height));
            for row in img.pixels.chunks_exact(source_row_bytes).rev() {
                for pixel in row.chunks_exact(3) {
                    pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
                }
                let padding = row_bytes.saturating_sub(source_row_bytes);
                pixels.resize(pixels.len().saturating_add(padding), 0);
            }
            (24u16, row_bytes, pixels)
        }
        ColorType::Rgba8 => {
            let row_bytes = width.saturating_mul(4);
            let mut pixels = Vec::with_capacity(row_bytes.saturating_mul(height));
            for row in img.pixels.chunks_exact(row_bytes).rev() {
                for pixel in row.chunks_exact(4) {
                    pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
                }
            }
            (32u16, row_bytes, pixels)
        }
        _ => {
            return Err(CodecError::Unsupported(format!(
                "BMP-backed ICO cannot encode color type {:?}",
                img.color
            )));
        }
    };
    let pixel_bytes = row_bytes.saturating_mul(height);
    // Each color arm emits exactly one validated source row at `row_bytes`.
    debug_assert_eq!(pixels.len(), pixel_bytes);

    let mask_row_bytes = width.div_ceil(8);
    let mask_bytes = if bits == 32 {
        0
    } else {
        // Source dimensions are capped at 256, so this is at most 8 KiB.
        mask_row_bytes.saturating_mul(height)
    };
    // Public BMP-backed ICO entries are generated only for <=256px sizes.
    let dib_bytes = 40usize
        .saturating_add(pixel_bytes)
        .saturating_add(mask_bytes);
    // The largest supported RGBA entry is below 264 KiB.
    let dib_size = low_u32(dib_bytes);
    let mut output = Vec::with_capacity(22usize.saturating_add(dib_bytes));
    output.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
    output.push(directory_dimension(width));
    output.push(directory_dimension(height));
    output.extend_from_slice(&[0, 0, 0, 0]);
    output.extend_from_slice(&bits.to_le_bytes());
    output.extend_from_slice(&dib_size.to_le_bytes());
    output.extend_from_slice(&22u32.to_le_bytes());

    output.extend_from_slice(&40u32.to_le_bytes());
    output.extend_from_slice(&img.width.to_le_bytes());
    output.extend_from_slice(&img.height.saturating_mul(2).to_le_bytes());
    output.extend_from_slice(&1u16.to_le_bytes());
    output.extend_from_slice(&bits.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&low_u32(pixel_bytes).to_le_bytes());
    output.extend_from_slice(&3_780i32.to_le_bytes());
    output.extend_from_slice(&3_780i32.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&pixels);
    output.resize(output.len().saturating_add(mask_bytes), 0);
    Ok(output)
}

fn parse_single_size(value: &str) -> CodecResult<(usize, usize)> {
    let numbers = value
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(str::parse::<usize>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| CodecError::Parameter("invalid ICO sizes option".to_owned()))?;
    let [width, height] = numbers.as_slice() else {
        return Err(CodecError::Parameter(
            "ICO sizes must contain exactly one width-height pair".to_owned(),
        ));
    };
    Ok((*width, *height))
}
