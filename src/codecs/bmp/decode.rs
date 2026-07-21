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

#[derive(Copy, Clone, Eq, PartialEq)]
enum BmpBitDepth {
    One,
    Four,
    Eight,
    Sixteen,
    TwentyFour,
    ThirtyTwo,
}

impl BmpBitDepth {
    fn from_raw(value: u16) -> Option<Self> {
        Some(match value {
            1 => Self::One,
            4 => Self::Four,
            8 => Self::Eight,
            16 => Self::Sixteen,
            24 => Self::TwentyFour,
            32 => Self::ThirtyTwo,
            _ => return None,
        })
    }

    fn bits_per_pixel(self) -> u16 {
        match self {
            Self::One => 1,
            Self::Four => 4,
            Self::Eight => 8,
            Self::Sixteen => 16,
            Self::TwentyFour => 24,
            Self::ThirtyTwo => 32,
        }
    }

    fn is_indexed(self) -> bool {
        matches!(self, Self::One | Self::Four | Self::Eight)
    }
}

// ---------------------------------------------------------------------------
// Palette reading
// ---------------------------------------------------------------------------

/// Read a Windows RGBQUAD or OS/2 RGBTRIPLE palette.
fn read_palette(r: &mut Cursor<&[u8]>, count: u32, entry_bytes: usize) -> Option<Vec<[u8; 4]>> {
    debug_assert!(matches!(entry_bytes, 3 | 4));
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
    debug_assert_ne!(mask, 0);
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

fn decode_rle(
    data: &[u8],
    width: usize,
    height: usize,
    rle4: bool,
    stream_offset: usize,
) -> Option<Vec<u8>> {
    // Pillow 12.2.0 BmpRleDecoder builds one linear index stream. In
    // particular, its RLE4 absolute mode reads floor(pixel_count / 2) bytes,
    // so an unpaired final nibble is deliberately omitted and later row
    // padding supplies a zero. Preserve that observable behavior exactly.
    let destination_length = width.checked_mul(height)?;
    let mut output = Vec::with_capacity(destination_length);
    let mut x = 0usize;
    let mut position = 0usize;

    while output.len() < destination_length {
        let count = usize::from(*data.get(position)?);
        let value = *data.get(position.checked_add(1)?)?;
        position = position.checked_add(2)?;

        if count != 0 {
            let pixel_count = count.min(width.saturating_sub(x));
            if rle4 {
                let first = value >> 4;
                let second = value & 0x0f;
                output.extend((0..pixel_count).map(|index| {
                    if index.is_multiple_of(2) {
                        first
                    } else {
                        second
                    }
                }));
            } else {
                output.resize(output.len().checked_add(pixel_count)?, value);
            }
            x = x.checked_add(pixel_count)?;
            continue;
        }

        match value {
            0 => {
                let remainder = output.len() % width;
                if remainder != 0 {
                    output.resize(output.len().checked_add(width - remainder)?, 0);
                }
                x = 0;
            }
            // The loop exits before reading EOB once the declared canvas is
            // complete. Reaching EOB here therefore means Pillow would report
            // insufficient image data rather than pad the missing pixels.
            1 => return None,
            2 => {
                let right = usize::from(*data.get(position)?);
                let up = usize::from(*data.get(position.checked_add(1)?)?);
                position = position.checked_add(2)?;
                let skipped = up.checked_mul(width)?.checked_add(right)?;
                output.resize(output.len().checked_add(skipped)?, 0);
                x = output.len() % width;
            }
            absolute_pixels => {
                let byte_count = if rle4 {
                    usize::from(absolute_pixels) / 2
                } else {
                    usize::from(absolute_pixels)
                };
                let end = position.checked_add(byte_count)?;
                let literal = data.get(position..end)?;
                if rle4 {
                    for &byte in literal {
                        output.extend_from_slice(&[byte >> 4, byte & 0x0f]);
                    }
                } else {
                    output.extend_from_slice(literal);
                }
                position = end;
                x = x.checked_add(usize::from(absolute_pixels))?;

                if stream_offset.checked_add(position)? % 2 != 0 {
                    position = position.checked_add(1)?;
                }
            }
        }
    }

    output.resize(destination_length, 0);
    output.truncate(destination_length);
    Some(output)
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
    let (width, height, bit_depth_raw, compression, colors_used, palette_entry_bytes) =
        if header_size == 12 {
            // OS/2 1.x BITMAPCOREHEADER uses unsigned 16-bit dimensions and
            // RGBTRIPLE palette entries.
            let width = i32::from(read_u16_le(&mut r)?);
            let height = i32::from(read_u16_le(&mut r)?);
            let _planes = read_u16_le(&mut r)?;
            let bit_depth = read_u16_le(&mut r)?;
            (width, height, bit_depth, 0, 0, 3usize)
        } else if header_size >= 40 {
            let width = read_i32_le(&mut r)?;
            let height = read_i32_le(&mut r)?;
            let _planes = read_u16_le(&mut r)?;
            let bit_depth = read_u16_le(&mut r)?;
            let compression = read_u32_le(&mut r)?;
            let _image_size = read_u32_le(&mut r)?;
            let _x_pels = read_i32_le(&mut r)?;
            let _y_pels = read_i32_le(&mut r)?;
            let colors_used = read_u32_le(&mut r)?;
            let _colors_important = read_u32_le(&mut r)?;
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
    if h == 0 || w > 16_384 || h > 16_384 {
        return None;
    }
    let bit_depth = BmpBitDepth::from_raw(bit_depth_raw)?;
    if matches!(compression, 1 | 2) && !bit_depth.is_indexed() {
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
                let a0 = if bit_depth == BmpBitDepth::ThirtyTwo && header_size >= 56 {
                    read_u32_le(&mut r).unwrap_or(0)
                } else {
                    0
                };
                (r0, g0, b0, a0)
            }
        }
    } else {
        // Default RGB555 masks for 16-bit BI_RGB.
        if bit_depth == BmpBitDepth::Sixteen {
            (0x7C00, 0x03E0, 0x001F, 0)
        } else {
            (0, 0, 0, 0)
        }
    };
    if compression == 3 && (rm == 0 || gm == 0 || bm == 0) {
        return None;
    }

    // --- Skip any remaining DIB header bytes to reach palette area ---
    let dib_end = 14u64 + header_size as u64;
    if r.position() < dib_end {
        r.seek(SeekFrom::Start(dib_end)).ok()?;
    }

    // --- Palette (color table) ---
    let pal_count = if colors_used > 0 {
        colors_used
    } else if bit_depth.is_indexed() {
        1u32 << bit_depth.bits_per_pixel()
    } else {
        0
    };

    let palette = if pal_count > 0 {
        read_palette(&mut r, pal_count, palette_entry_bytes)?
    } else {
        Vec::new()
    };
    let palette_is_grayscale = !palette.is_empty()
        && palette.iter().enumerate().all(|(index, entry)| {
            let expected = if palette.len() == 2 {
                if index == 0 { 0 } else { 255 }
            } else {
                index as u8
            };
            entry[0] == expected && entry[1] == expected && entry[2] == expected
        });

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
            decode_rle(
                &remaining,
                width_usize,
                height_usize,
                false,
                usize::try_from(data_offset).ok()?,
            )?,
            width_usize,
            top_down,
        )
    } else if compression == 2 {
        // BI_RLE4 — return raw palette indices
        let mut remaining = Vec::new();
        r.read_to_end(&mut remaining).ok()?;
        orient_index_rows(
            decode_rle(
                &remaining,
                width_usize,
                height_usize,
                true,
                usize::try_from(data_offset).ok()?,
            )?,
            width_usize,
            top_down,
        )
    } else {
        // BI_RGB or BI_BITFIELDS — uncompressed scanlines
        let stride = row_size(bit_depth.bits_per_pixel(), w);
        let mut raw = vec![0u8; stride * height_usize];
        r.read_exact(&mut raw).ok()?;

        match bit_depth {
            BmpBitDepth::One => {
                if palette_is_grayscale {
                    // Pillow retains packed bytes only for its canonical
                    // black/white palette and exposes mode `1`.
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
                } else {
                    // A noncanonical two-color palette remains mode `P`, so
                    // Pillow expands each packed bit to one palette index.
                    let mut out = Vec::with_capacity(width_usize * height_usize);
                    for row in 0..height_usize {
                        let src_row = if top_down {
                            row
                        } else {
                            height_usize - 1 - row
                        };
                        let offset = src_row * stride;
                        for col in 0..width_usize {
                            out.push((raw[offset + col / 8] >> (7 - col % 8)) & 1);
                        }
                    }
                    out
                }
            }
            BmpBitDepth::Four => {
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
            BmpBitDepth::Eight => {
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
            BmpBitDepth::Sixteen => {
                // 16 bpp — RGB555 or BI_BITFIELDS
                let mut out = Vec::with_capacity(width_usize * height_usize * 3);
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
                    }
                }
                out
            }
            BmpBitDepth::TwentyFour => {
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
            BmpBitDepth::ThirtyTwo => {
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
        }
    };

    // Determine output color type
    let color = if bit_depth.is_indexed() {
        ColorType::L8
    } else if matches!(bit_depth, BmpBitDepth::Sixteen | BmpBitDepth::ThirtyTwo) && am != 0 {
        ColorType::Rgba8
    } else {
        ColorType::Rgb8
    };

    let mode = match bit_depth {
        BmpBitDepth::One => {
            if palette_is_grayscale {
                ImageMode::L1
            } else {
                ImageMode::P8
            }
        }
        BmpBitDepth::Four | BmpBitDepth::Eight => {
            if palette_is_grayscale {
                ImageMode::L8
            } else {
                ImageMode::P8
            }
        }
        BmpBitDepth::Sixteen | BmpBitDepth::TwentyFour | BmpBitDepth::ThirtyTwo => color.into(),
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

fn orient_index_rows(mut pixels: Vec<u8>, width: usize, top_down: bool) -> Vec<u8> {
    if top_down {
        return pixels;
    }
    debug_assert_ne!(width, 0);
    debug_assert!(pixels.len().is_multiple_of(width));
    let height = pixels.len() / width;
    for top in 0..height / 2 {
        let bottom = height - top - 1;
        for x in 0..width {
            pixels.swap(top * width + x, bottom * width + x);
        }
    }
    pixels
}
