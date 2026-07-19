//! BMP decoder — pure Rust, no external crate.
//!
//! Handles:
//! - BITMAPFILEHEADER + BITMAPINFOHEADER (and larger DIB headers)
//! - Bit depths: 1, 4, 8, 16 (RGB555 / BI_BITFIELDS), 24, 32 (RGBA)
//! - Compression: BI_RGB (none), BI_RLE8, BI_RLE4, BI_BITFIELDS
//! - Bottom‑up (positive height) and top‑down (negative height)
//! - 4‑byte row padding
//! - Palette (color table)

use crate::types::{ColorType, DecodedImage, ImageMode, ImagePalette};
use std::io::{Cursor, Read, Seek, SeekFrom};

// ---------------------------------------------------------------------------
// Little‑endian helpers
// ---------------------------------------------------------------------------

fn read_u16_le<R: Read>(r: &mut R) -> Option<u16> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b).ok()?;
    Some(u16::from_le_bytes(b))
}

fn read_u32_le<R: Read>(r: &mut R) -> Option<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b).ok()?;
    Some(u32::from_le_bytes(b))
}

fn read_i32_le<R: Read>(r: &mut R) -> Option<i32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b).ok()?;
    Some(i32::from_le_bytes(b))
}

/// Row size in bytes (padded to 4‑byte boundary).
fn row_size(bits_per_pixel: u16, width: u32) -> usize {
    (((bits_per_pixel as u64) * (width as u64)).div_ceil(32) * 4) as usize
}

// ---------------------------------------------------------------------------
// Palette reading
// ---------------------------------------------------------------------------

/// Read a Windows RGBQUAD or OS/2 RGBTRIPLE palette.
fn read_palette(r: &mut Cursor<&[u8]>, count: u32, entry_bytes: usize) -> Option<Vec<[u8; 4]>> {
    if !matches!(entry_bytes, 3 | 4) {
        return None;
    }
    let mut pal = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let mut entry = [0u8; 4];
        r.read_exact(&mut entry[..entry_bytes]).ok()?; // B, G, R, [reserved]
        pal.push(entry);
    }
    Some(pal)
}

// ---------------------------------------------------------------------------
// Channel extraction from bit‑mask (for BI_BITFIELDS)
// ---------------------------------------------------------------------------

fn extract_channel(pixel: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0;
    }
    let shift = mask.trailing_zeros();
    let width = (mask >> shift).count_ones();
    let value = (pixel & mask) >> shift;
    if width >= 8 {
        (value >> (width - 8)) as u8
    } else {
        let max_val = (1u32 << width) - 1;
        ((value * 255) / max_val) as u8
    }
}

// ---------------------------------------------------------------------------
// RLE8 decoder
// ---------------------------------------------------------------------------

fn decode_rle8(data: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
    let mut out = vec![0u8; width * height];
    let mut x = 0usize;
    let mut y = 0usize;
    let mut i = 0usize;

    while i + 1 < data.len() {
        let count = data[i] as usize;
        let value = data[i + 1];
        i += 2;

        if count > 0 {
            // Encoded mode: repeat `value` `count` times
            for _ in 0..count {
                if x < width {
                    out[y * width + x] = value;
                }
                x += 1;
            }
        } else {
            // Escape sequences
            match value {
                0 => {
                    // End of line
                    x = 0;
                    y += 1;
                    if y >= height {
                        break;
                    }
                }
                1 => break, // End of bitmap
                2 => {
                    // Delta
                    if i + 1 >= data.len() {
                        return None;
                    }
                    let dx = data[i] as usize;
                    let dy = data[i + 1] as usize;
                    i += 2;
                    x += dx;
                    y += dy;
                    if y >= height {
                        break;
                    }
                }
                _ => {
                    // Absolute mode: `value` literal bytes follow, padded to word boundary
                    let abs_len = value as usize;
                    if i + abs_len > data.len() {
                        return None;
                    }
                    for j in 0..abs_len {
                        if x < width {
                            out[y * width + x] = data[i + j];
                        }
                        x += 1;
                    }
                    i += abs_len;
                    // Pad to 16-bit boundary
                    if abs_len % 2 == 1 {
                        i += 1;
                    }
                }
            }
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// RLE4 decoder
// ---------------------------------------------------------------------------

fn decode_rle4(data: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
    let mut out = vec![0u8; width * height];
    let mut x = 0usize;
    let mut y = 0usize;
    let mut i = 0usize;

    while i + 1 < data.len() {
        let count = data[i] as usize;
        let value = data[i + 1];
        i += 2;

        if count > 0 {
            // Encoded mode: the two nibbles of `value` are repeated `count` times
            let hi = (value >> 4) & 0x0F;
            let lo = value & 0x0F;
            for position in 0..count {
                if x < width {
                    out[y * width + x] = if position % 2 == 0 { hi } else { lo };
                }
                x += 1;
            }
        } else {
            // Escape sequences
            match value {
                0 => {
                    // End of line
                    x = 0;
                    y += 1;
                    if y >= height {
                        break;
                    }
                }
                1 => break, // End of bitmap
                2 => {
                    // Delta
                    if i + 1 >= data.len() {
                        return None;
                    }
                    let dx = data[i] as usize;
                    let dy = data[i + 1] as usize;
                    i += 2;
                    x += dx;
                    y += dy;
                    if y >= height {
                        break;
                    }
                }
                _ => {
                    // Absolute mode: `value` nibbles follow
                    let nibble_count = value as usize;
                    let byte_count = nibble_count.div_ceil(2);
                    if i + byte_count > data.len() {
                        return None;
                    }
                    for j in 0..nibble_count {
                        let byte = data[i + j / 2];
                        let nibble = if j % 2 == 0 {
                            (byte >> 4) & 0x0F
                        } else {
                            byte & 0x0F
                        };
                        if x < width {
                            out[y * width + x] = nibble;
                        }
                        x += 1;
                    }
                    i += byte_count;
                    // Pad to 16-bit boundary
                    if byte_count % 2 == 1 {
                        i += 1;
                    }
                }
            }
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Main decoder
// ---------------------------------------------------------------------------

pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    let mut r = Cursor::new(data);

    // --- BITMAPFILEHEADER (14 bytes) ---
    let mut magic = [0u8; 2];
    r.read_exact(&mut magic).ok()?;
    if &magic != b"BM" {
        return None;
    }
    let _file_size = read_u32_le(&mut r)?; // bytes 2-5
    r.seek(SeekFrom::Current(4)).ok()?; // bytes 6-9 (reserved)
    let data_offset = read_u32_le(&mut r)? as u64; // bytes 10-13

    // --- DIB header ---
    let header_size = read_u32_le(&mut r)?;
    let (width, height, bit_depth, compression, colors_used, palette_entry_bytes) =
        if header_size == 12 {
            // OS/2 1.x BITMAPCOREHEADER uses unsigned 16-bit dimensions and
            // RGBTRIPLE palette entries.
            let width = i32::from(read_u16_le(&mut r)?);
            let height = i32::from(read_u16_le(&mut r)?);
            let planes = read_u16_le(&mut r)?;
            let bit_depth = read_u16_le(&mut r)?;
            if planes != 1 {
                return None;
            }
            (width, height, bit_depth, 0, 0, 3usize)
        } else if header_size >= 40 {
            let width = read_i32_le(&mut r)?;
            let height = read_i32_le(&mut r)?;
            let planes = read_u16_le(&mut r)?;
            let bit_depth = read_u16_le(&mut r)?;
            let compression = read_u32_le(&mut r)?;
            let _image_size = read_u32_le(&mut r)?;
            let _x_pels = read_i32_le(&mut r)?;
            let _y_pels = read_i32_le(&mut r)?;
            let colors_used = read_u32_le(&mut r)?;
            let _colors_important = read_u32_le(&mut r)?;
            if planes != 1 {
                return None;
            }
            (width, height, bit_depth, compression, colors_used, 4usize)
        } else {
            return None;
        };

    // After the standard 40-byte fields, the cursor is at position 14 + 40 = 54.
    let pos_after_standard = r.position();

    // Top-down if height is negative.
    let top_down = height < 0;
    let h = height.unsigned_abs();
    if width <= 0 {
        return None;
    }
    let w = u32::try_from(width).ok()?;
    if w == 0 || h == 0 || w > 16_384 || h > 16_384 {
        return None;
    }

    // --- BI_BITFIELDS masks ---
    let (rm, gm, bm, am): (u32, u32, u32, u32) = if compression == 3 {
        match header_size {
            40 => {
                // BITMAPINFOHEADER BI_BITFIELDS defines three color masks.
                // Pillow does not promote an optional fourth DWORD in this
                // legacy layout to an alpha channel; alpha is authoritative
                // only in V4/V5 headers.
                r.seek(SeekFrom::Start(pos_after_standard)).ok()?;
                let r0 = read_u32_le(&mut r)?;
                let g0 = read_u32_le(&mut r)?;
                let b0 = read_u32_le(&mut r)?;
                (r0, g0, b0, 0)
            }
            _ => {
                // For V4/V5 headers the masks are embedded at known offsets.
                // V4 offsets (from DIB start): red=40, green=44, blue=48, alpha=52
                r.seek(SeekFrom::Start(14 + 40)).ok()?;
                let r0 = read_u32_le(&mut r)?;
                let g0 = read_u32_le(&mut r)?;
                let b0 = read_u32_le(&mut r)?;
                let a0 = if bit_depth == 32 && header_size >= 56 {
                    read_u32_le(&mut r).unwrap_or(0)
                } else {
                    0
                };
                (r0, g0, b0, a0)
            }
        }
    } else {
        // Default RGB555 masks for 16-bit BI_RGB.
        if bit_depth == 16 {
            (0x7C00, 0x03E0, 0x001F, 0)
        } else {
            (0, 0, 0, 0)
        }
    };

    // --- Skip any remaining DIB header bytes to reach palette area ---
    let dib_end = 14u64 + header_size as u64;
    if r.position() < dib_end {
        r.seek(SeekFrom::Start(dib_end)).ok()?;
    }

    // --- Palette (color table) ---
    let pal_count = if colors_used > 0 {
        colors_used
    } else if bit_depth <= 8 {
        1u32 << bit_depth
    } else {
        0
    };

    let palette = if pal_count > 0 {
        read_palette(&mut r, pal_count, palette_entry_bytes)?
    } else {
        Vec::new()
    };

    // --- Seek to pixel data ---
    if r.position() != data_offset {
        r.seek(SeekFrom::Start(data_offset)).ok()?;
    }

    // ------------------------------------------------------------------
    // Pixel decoding
    // ------------------------------------------------------------------
    let width_usize = w as usize;
    let height_usize = h as usize;

    let pixels: Vec<u8> = if compression == 1 {
        // BI_RLE8 — return raw palette indices
        let mut remaining = Vec::new();
        r.read_to_end(&mut remaining).ok()?;
        orient_index_rows(
            decode_rle8(&remaining, width_usize, height_usize)?,
            width_usize,
            top_down,
        )?
    } else if compression == 2 {
        // BI_RLE4 — return raw palette indices
        let mut remaining = Vec::new();
        r.read_to_end(&mut remaining).ok()?;
        orient_index_rows(
            decode_rle4(&remaining, width_usize, height_usize)?,
            width_usize,
            top_down,
        )?
    } else {
        // BI_RGB or BI_BITFIELDS — uncompressed scanlines
        let stride = row_size(bit_depth, w);
        let mut raw = vec![0u8; stride * height_usize];
        r.read_exact(&mut raw).ok()?;

        match bit_depth {
            1 => {
                // 1 bpp — packed bits, skip stride padding (PIL mode '1' parity)
                let packed_per_row = width_usize.div_ceil(8);
                let mut out = Vec::with_capacity(packed_per_row * height_usize);
                for row in 0..height_usize {
                    let src_row = if top_down {
                        row
                    } else {
                        height_usize - 1 - row
                    };
                    let offset = src_row * stride;
                    out.extend_from_slice(&raw[offset..offset + packed_per_row]);
                }
                out
            }
            4 => {
                // 4 bpp — expand nibbles to full-byte indices
                let mut out = Vec::with_capacity(width_usize * height_usize);
                for row in 0..height_usize {
                    let src_row = if top_down {
                        row
                    } else {
                        height_usize - 1 - row
                    };
                    let offset = src_row * stride;
                    for col in 0..width_usize {
                        let byte = raw[offset + col / 2];
                        let idx = if col % 2 == 0 {
                            (byte >> 4) & 0x0F
                        } else {
                            byte & 0x0F
                        };
                        out.push(idx);
                    }
                }
                out
            }
            8 => {
                // 8 bpp — raw palette indices, skip stride padding (PIL mode 'P' parity)
                let mut out = Vec::with_capacity(width_usize * height_usize);
                for row in 0..height_usize {
                    let src_row = if top_down {
                        row
                    } else {
                        height_usize - 1 - row
                    };
                    let offset = src_row * stride;
                    out.extend_from_slice(&raw[offset..offset + width_usize]);
                }
                out
            }
            16 => {
                // 16 bpp — RGB555 or BI_BITFIELDS
                let channels = if am == 0 { 3 } else { 4 };
                let mut out = Vec::with_capacity(width_usize * height_usize * channels);
                for row in 0..height_usize {
                    let src_row = if top_down {
                        row
                    } else {
                        height_usize - 1 - row
                    };
                    let offset = src_row * stride;
                    for col in 0..width_usize {
                        let lo = raw[offset + col * 2] as u32;
                        let hi = raw[offset + col * 2 + 1] as u32;
                        let pixel = lo | (hi << 8);
                        let rv = extract_channel(pixel, rm);
                        let gv = extract_channel(pixel, gm);
                        let bv = extract_channel(pixel, bm);
                        out.extend_from_slice(&[rv, gv, bv]);
                        if am != 0 {
                            out.push(extract_channel(pixel, am));
                        }
                    }
                }
                out
            }
            24 => {
                // 24 bpp — BGR order
                let mut out = Vec::with_capacity(width_usize * height_usize * 3);
                for row in 0..height_usize {
                    let src_row = if top_down {
                        row
                    } else {
                        height_usize - 1 - row
                    };
                    let offset = src_row * stride;
                    for col in 0..width_usize {
                        let b = raw[offset + col * 3];
                        let g = raw[offset + col * 3 + 1];
                        let r = raw[offset + col * 3 + 2];
                        out.extend_from_slice(&[r, g, b]);
                    }
                }
                out
            }
            32 => {
                // BI_RGB treats byte four as padding. V4/V5 BI_BITFIELDS files
                // with an alpha mask expose that channel as Pillow RGBA.
                let channels = if compression == 3 && am != 0 { 4 } else { 3 };
                let mut out = Vec::with_capacity(width_usize * height_usize * channels);
                for row in 0..height_usize {
                    let src_row = if top_down {
                        row
                    } else {
                        height_usize - 1 - row
                    };
                    let offset = src_row * stride;
                    for col in 0..width_usize {
                        let start = offset + col * 4;
                        if compression == 3 {
                            let pixel =
                                u32::from_le_bytes(raw.get(start..start + 4)?.try_into().ok()?);
                            out.extend_from_slice(&[
                                extract_channel(pixel, rm),
                                extract_channel(pixel, gm),
                                extract_channel(pixel, bm),
                            ]);
                            if am != 0 {
                                out.push(extract_channel(pixel, am));
                            }
                        } else {
                            out.extend_from_slice(&[raw[start + 2], raw[start + 1], raw[start]]);
                        }
                    }
                }
                out
            }
            _ => return None,
        }
    };

    // Determine output color type
    let color = if bit_depth <= 8 || compression == 1 || compression == 2 {
        ColorType::L8
    } else if matches!(bit_depth, 16 | 32) && am != 0 {
        ColorType::Rgba8
    } else {
        ColorType::Rgb8
    };

    let mode = match bit_depth {
        1 => ImageMode::L1,
        2 | 4 | 8 => ImageMode::P8,
        _ => color.into(),
    };
    let mut image = DecodedImage::with_mode(w, h, pixels, mode);
    if mode == ImageMode::P8 {
        let mut rgb = Vec::with_capacity(palette.len().checked_mul(3)?);
        for entry in palette {
            rgb.extend_from_slice(&[entry[2], entry[1], entry[0]]);
        }
        image = image.with_palette(ImagePalette::new(rgb, Vec::new()).ok()?);
    }
    Some(image)
}

fn orient_index_rows(mut pixels: Vec<u8>, width: usize, top_down: bool) -> Option<Vec<u8>> {
    if top_down {
        return Some(pixels);
    }
    if width == 0 || !pixels.len().is_multiple_of(width) {
        return None;
    }
    let height = pixels.len() / width;
    for top in 0..height / 2 {
        let bottom = height.checked_sub(top)?.checked_sub(1)?;
        for x in 0..width {
            pixels.swap(
                top.checked_mul(width)?.checked_add(x)?,
                bottom.checked_mul(width)?.checked_add(x)?,
            );
        }
    }
    Some(pixels)
}
