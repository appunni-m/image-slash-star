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
    if writes_one_source_sized_png(img, opts) {
        return encode_png_entry(img);
    }
    // Convert pixel data to BGRA (we always write 32-bit ICO entries)
    let bgra = match img.color {
        ColorType::Rgba8 => convert_rgba_to_bgra(&img.pixels),
        ColorType::Rgb8 => convert_rgb_to_bgra(&img.pixels),
        ColorType::L8 => convert_l8_to_bgra(&img.pixels),
        _ => return None,
    };
    // ICO header + directory entry size
    let header_size = 6;
    let dir_entry_size = 16;
    let data_offset = header_size + dir_entry_size;
    // BMP BITMAPINFOHEADER (40 bytes)
    let dib_header_size = 40u32;
    // Pixel data: each row is 4 bytes per pixel, bottom-up
    let row_bytes = w * 4;
    let pixel_data_size = row_bytes * h;
    // AND mask: 1 bit per pixel, each row padded to 4-byte boundary
    let and_mask_row_bytes = w.div_ceil(32) * 4;
    let and_mask_size = and_mask_row_bytes * h;
    // ICO BMP data: DIB header + pixels + AND mask
    let bmp_data_size = dib_header_size as usize + pixel_data_size + and_mask_size;
    let total_size = data_offset + bmp_data_size;
    let mut data = Vec::with_capacity(total_size);
    // --- ICO header (6 bytes) ---
    data.extend_from_slice(&[0u8; 2]); // reserved
    data.extend_from_slice(&1u16.to_le_bytes()); // type = ICO (1)
    data.extend_from_slice(&1u16.to_le_bytes()); // count = 1
    // --- Directory entry (16 bytes) ---
    // Width/height: 0 means 256; otherwise actual value
    if w == 256 {
        data.push(0);
    } else {
        data.push(w as u8);
    }
    if h == 256 {
        data.push(0);
    } else {
        data.push(h as u8);
    }
    data.push(0); // colors (0 = >= 256)
    data.push(0); // reserved
    data.extend_from_slice(&1u16.to_le_bytes()); // color planes
    data.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
    data.extend_from_slice(&(bmp_data_size as u32).to_le_bytes()); // size of BMP data
    data.extend_from_slice(&(data_offset as u32).to_le_bytes()); // offset of BMP data
    // --- BMP data: BITMAPINFOHEADER (40 bytes) ---
    data.extend_from_slice(&dib_header_size.to_le_bytes()); // biSize
    data.extend_from_slice(&(w as u32).to_le_bytes()); // biWidth
    // ICO convention: height is doubled to include AND mask rows
    data.extend_from_slice(&((h as u32) * 2).to_le_bytes()); // biHeight
    data.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    data.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    data.extend_from_slice(&0u32.to_le_bytes()); // biCompression (BI_RGB)
    data.extend_from_slice(&0u32.to_le_bytes()); // biSizeImage
    data.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
    data.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
    data.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
    data.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant
    // --- Pixel data (bottom-up BGRA) ---
    for y in (0..h).rev() {
        let row_start = y * row_bytes;
        data.extend_from_slice(&bgra[row_start..row_start + row_bytes]);
    }
    // --- AND mask (all zeros = fully opaque) ---
    for _ in 0..h {
        for _ in 0..and_mask_row_bytes {
            data.push(0);
        }
    }
    Some(data)
}

fn writes_one_source_sized_png(img: &DecodedImage, opts: &EncodeOptions) -> bool {
    if let Some(value) = opts.extra.get("sizes") {
        return parse_sizes(value).is_some_and(|sizes| {
            sizes.len() == 1 && sizes[0] == (img.width as usize, img.height as usize)
        });
    }
    const DEFAULT_SIZES: [usize; 7] = [16, 24, 32, 48, 64, 128, 256];
    DEFAULT_SIZES
        .into_iter()
        .filter(|&size| size <= img.width as usize && size <= img.height as usize)
        .count()
        == 1
}

fn encode_png_entry(img: &DecodedImage) -> Option<Vec<u8>> {
    // Pillow 12.2.0 IcoImagePlugin.py:175-178 delegates the frame to the PNG
    // writer without forwarding ICO options.
    let png = crate::codecs::png::encode::encode(img, &EncodeOptions::default())?;
    let mut output = Vec::with_capacity(22usize.checked_add(png.len())?);
    output.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
    output.push(if img.width == 256 {
        0
    } else {
        u8::try_from(img.width).ok()?
    });
    output.push(if img.height == 256 {
        0
    } else {
        u8::try_from(img.height).ok()?
    });
    output.extend_from_slice(&[0, 0, 0, 0]);
    output.extend_from_slice(&32u16.to_le_bytes());
    output.extend_from_slice(&u32::try_from(png.len()).ok()?.to_le_bytes());
    output.extend_from_slice(&22u32.to_le_bytes());
    output.extend_from_slice(&png);
    Some(output)
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
/// Convert RGBA8 pixels (R,G,B,A) to BGRA (B,G,R,A).
fn convert_rgba_to_bgra(pixels: &[u8]) -> Vec<u8> {
    let mut bgra = Vec::with_capacity(pixels.len());
    for chunk in pixels.chunks_exact(4) {
        bgra.push(chunk[2]); // B
        bgra.push(chunk[1]); // G
        bgra.push(chunk[0]); // R
        bgra.push(chunk[3]); // A
    }
    bgra
}
/// Convert RGB8 pixels (R,G,B) to BGRA (B,G,R,A=255).
fn convert_rgb_to_bgra(pixels: &[u8]) -> Vec<u8> {
    let pixel_count = pixels.len() / 3;
    let mut bgra = Vec::with_capacity(pixel_count * 4);
    for chunk in pixels.chunks_exact(3) {
        bgra.push(chunk[2]); // B
        bgra.push(chunk[1]); // G
        bgra.push(chunk[0]); // R
        bgra.push(255); // A (fully opaque)
    }
    bgra
}
/// Convert L8 pixels (grayscale) to BGRA (B=G=R=lum, A=255).
fn convert_l8_to_bgra(pixels: &[u8]) -> Vec<u8> {
    let mut bgra = Vec::with_capacity(pixels.len() * 4);
    for &lum in pixels {
        bgra.push(lum); // B
        bgra.push(lum); // G
        bgra.push(lum); // R
        bgra.push(255); // A
    }
    bgra
}
