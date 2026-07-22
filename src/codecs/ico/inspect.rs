//! ICO/CUR directory and embedded-image inspection without pixel decoding.

use crate::types::{ImageFormat, ImageInfo, ImageMode};

const HEADER_SIZE: usize = 6;
const ENTRY_SIZE: usize = 16;

/// Inspect the same best-resolution entry selected by the ICO decoder.
pub fn inspect(data: &[u8]) -> Option<ImageInfo> {
    let header = data.get(..HEADER_SIZE)?;
    let reserved = u16::from_le_bytes([header[0], header[1]]);
    let kind = u16::from_le_bytes([header[2], header[3]]);
    let count = usize::from(u16::from_le_bytes([header[4], header[5]]));
    if reserved != 0 || !matches!(kind, 1 | 2) || count == 0 || count > 255 {
        return None;
    }
    let directory = data.get(HEADER_SIZE..HEADER_SIZE + count * ENTRY_SIZE)?;
    let mut best = &directory[..ENTRY_SIZE];
    let mut best_score = 0;
    for entry in directory.chunks_exact(ENTRY_SIZE) {
        let width = if entry[0] == 0 {
            256
        } else {
            u32::from(entry[0])
        };
        let height = if entry[1] == 0 {
            256
        } else {
            u32::from(entry[1])
        };
        let score = width.saturating_mul(height);
        if score > best_score {
            best = entry;
            best_score = score;
        }
    }

    let length = u32::from_le_bytes([best[8], best[9], best[10], best[11]]) as usize;
    let offset = u32::from_le_bytes([best[12], best[13], best[14], best[15]]) as usize;
    if length == 0 || offset == 0 {
        return None;
    }
    let payload = data.get(offset..offset.wrapping_add(length))?;
    let mut info = if payload.starts_with(b"\x89PNG\r\n\x1a\n") {
        crate::codecs::png::inspect::inspect(payload)?
    } else if kind == 2 {
        inspect_cursor_dib(payload)?
    } else {
        inspect_icon_dib(payload)?
    };
    info.format = ImageFormat::Ico;
    info.is_animated = false;
    info.frame_count = Some(1);
    Some(info)
}

fn inspect_cursor_dib(data: &[u8]) -> Option<ImageInfo> {
    let header = data.get(..40)?;
    let header_size = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
    if header_size < 40 || data.len() < header_size {
        return None;
    }
    let actual_height = i32::from_le_bytes([header[8], header[9], header[10], header[11]]) / 2;
    let bits = u16::from_le_bytes([header[14], header[15]]);
    let colors_used = u32::from_le_bytes([header[32], header[33], header[34], header[35]]);
    let palette_entries = if bits <= 8 {
        (if colors_used == 0 {
            1u32 << bits
        } else {
            colors_used
        }) as usize
    } else {
        0
    };
    let (file_size, file_size_bytes, pixel_offset_bytes) =
        super::decode::cur_bmp_prefix(data.len(), header_size, palette_entries)?;
    let mut bmp = Vec::with_capacity(file_size);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&file_size_bytes);
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&pixel_offset_bytes);
    bmp.extend_from_slice(data);
    bmp[22..26].copy_from_slice(&actual_height.to_le_bytes());
    crate::codecs::bmp::inspect::inspect(&bmp)
}

fn inspect_icon_dib(data: &[u8]) -> Option<ImageInfo> {
    let header = data.get(..40)?;
    let width = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
    let stored_height = u32::from_le_bytes([header[8], header[9], header[10], header[11]]);
    let height = stored_height / 2;
    let bits = u16::from_le_bytes([header[14], header[15]]);
    let colors_used = u32::from_le_bytes([header[32], header[33], header[34], header[35]]);
    if width == 0 || height == 0 || width > 16_384 || height > 16_384 {
        return None;
    }
    let indexed = matches!(bits, 1 | 4 | 8);
    let palette_entries = if indexed {
        if colors_used == 0 {
            1usize << bits
        } else {
            colors_used as usize
        }
    } else {
        0
    };
    let row_bytes = match bits {
        1 => (width as usize).div_ceil(8),
        4 => (width as usize).div_ceil(2),
        8 => width as usize,
        24 => width as usize * 3,
        32 => width as usize * 4,
        _ => return None,
    };
    let padded_row = (row_bytes + 3) & !3;
    let required = 40 + palette_entries * 4 + padded_row * height as usize;
    data.get(..required)?;
    Some(ImageInfo {
        format: ImageFormat::Ico,
        width,
        height,
        mode: ImageMode::Rgba8,
        bit_depth: bits as u8,
        palette: None,
        is_animated: false,
        frame_count: Some(1),
    })
}
