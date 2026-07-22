//! BMP header and palette inspection without pixel decoding.

use crate::types::{ImageFormat, ImageInfo, ImageMode, ImagePalette};

const FILE_HEADER_SIZE: usize = 14;
const CORE_HEADER_SIZE: u32 = 12;
const INFO_HEADER_SIZE: u32 = 40;
const BI_RLE8: u32 = 1;
const BI_RLE4: u32 = 2;
const BI_BITFIELDS: u32 = 3;

/// Inspect BMP dimensions, encoded depth, output mode, and indexed palette.
pub fn inspect(data: &[u8]) -> Option<ImageInfo> {
    if data.get(..2)? != b"BM" {
        return None;
    }

    let _data_offset = le_u32(data, 10)?;
    let header_size = le_u32(data, FILE_HEADER_SIZE)?;
    #[cfg(target_pointer_width = "64")]
    let header_end = FILE_HEADER_SIZE + header_size as usize;
    #[cfg(not(target_pointer_width = "64"))]
    let header_end = FILE_HEADER_SIZE.checked_add(header_size as usize)?;
    let (width, height, bit_depth, compression, colors_used, palette_entry_size) =
        if header_size == CORE_HEADER_SIZE {
            (
                u32::from(le_u16(data, 18)?),
                u32::from(le_u16(data, 20)?),
                le_u16(data, 24)?,
                0,
                0,
                3usize,
            )
        } else if header_size >= INFO_HEADER_SIZE {
            let width = le_i32(data, 18)?;
            if width <= 0 {
                return None;
            }
            (
                width as u32,
                le_i32(data, 22)?.unsigned_abs(),
                le_u16(data, 28)?,
                le_u32(data, 30)?,
                le_u32(data, 46)?,
                4usize,
            )
        } else {
            return None;
        };

    if width == 0 || height == 0 || width > 16_384 || height > 16_384 {
        return None;
    }
    let indexed = matches!(bit_depth, 1 | 4 | 8);
    if !matches!(bit_depth, 1 | 4 | 8 | 16 | 24 | 32)
        || matches!(compression, BI_RLE8 | BI_RLE4) && !indexed
    {
        return None;
    }

    let alpha_mask = bitfield_alpha(data, header_size, bit_depth, compression)?;
    let palette_start = if header_size == INFO_HEADER_SIZE && compression == BI_BITFIELDS {
        #[cfg(target_pointer_width = "64")]
        let start = header_end + 12;
        #[cfg(not(target_pointer_width = "64"))]
        let start = header_end.checked_add(12)?;
        start
    } else {
        header_end
    };
    let palette_count = if colors_used != 0 {
        colors_used as usize
    } else if indexed {
        1usize << bit_depth
    } else {
        0
    };
    let palette = read_palette(data, palette_start, palette_count, palette_entry_size)?;
    let grayscale_palette = !palette.is_empty()
        && palette.iter().enumerate().all(|(index, entry)| {
            let expected = if palette.len() == 2 {
                if index == 0 { 0 } else { 255 }
            } else {
                index as u8
            };
            entry[0] == expected && entry[1] == expected && entry[2] == expected
        });
    let mode = if bit_depth == 1 {
        if grayscale_palette {
            ImageMode::L1
        } else {
            ImageMode::P8
        }
    } else if matches!(bit_depth, 4 | 8) {
        if grayscale_palette {
            ImageMode::L8
        } else {
            ImageMode::P8
        }
    } else if matches!(bit_depth, 16 | 24) {
        ImageMode::Rgb8
    } else if compression == BI_BITFIELDS && alpha_mask != 0 {
        ImageMode::Rgba8
    } else {
        ImageMode::Rgb8
    };
    let palette = (mode == ImageMode::P8).then(|| {
        let mut rgb = Vec::with_capacity(palette.len().wrapping_mul(3));
        for entry in palette {
            rgb.extend_from_slice(&[entry[2], entry[1], entry[0]]);
        }
        ImagePalette {
            rgb,
            alpha: Vec::new(),
        }
    });

    Some(ImageInfo {
        format: ImageFormat::Bmp,
        width,
        height,
        mode,
        bit_depth: bit_depth as u8,
        palette,
        is_animated: false,
        frame_count: Some(1),
    })
}

fn bitfield_alpha(data: &[u8], header_size: u32, bit_depth: u16, compression: u32) -> Option<u32> {
    if compression != BI_BITFIELDS {
        return Some(0);
    }
    if le_u32(data, 54)? == 0 || le_u32(data, 58)? == 0 || le_u32(data, 62)? == 0 {
        return None;
    }
    if bit_depth == 32 && header_size >= 56 {
        le_u32(data, 66)
    } else {
        Some(0)
    }
}

fn read_palette(
    data: &[u8],
    start: usize,
    count: usize,
    entry_size: usize,
) -> Option<Vec<[u8; 4]>> {
    let available = data.get(start..)?;
    if count > available.len() / entry_size {
        return None;
    }
    let byte_len = count * entry_size;
    let bytes = &available[..byte_len];
    let mut palette = Vec::with_capacity(count);
    for entry in bytes.chunks_exact(entry_size) {
        let mut color = [0; 4];
        color[..entry_size].copy_from_slice(entry);
        palette.push(color);
    }
    Some(palette)
}

fn le_u16(data: &[u8], offset: usize) -> Option<u16> {
    let bytes = data.get(offset..offset.wrapping_add(2))?;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn le_u32(data: &[u8], offset: usize) -> Option<u32> {
    let bytes = data.get(offset..offset.wrapping_add(4))?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn le_i32(data: &[u8], offset: usize) -> Option<i32> {
    let bytes = data.get(offset..offset.wrapping_add(4))?;
    Some(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}
