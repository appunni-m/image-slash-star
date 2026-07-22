//! ICO decoder — parses ICO container format and delegates to PNG or BMP decoders.
//!
//! ICO (Icon) files store one or more icon images in a container that references
//! either embedded PNG data or BMP/DIB data for each entry. This decoder:
//!
//! 1. Parses the ICO header to get the entry count.
//! 2. Reads the directory entries (each 16 bytes).
//! 3. Selects the entry with the largest resolution (preferring 256x256).
//! 4. Dispatches to the PNG decoder if the entry data starts with the PNG
//!    signature, or attempts BMP/DIB decoding otherwise.
//!
//! Reference: https://en.wikipedia.org/wiki/ICO_(file_format)

use crate::types::{ColorType, DecodedImage};

/// ICO header size: 6 bytes
const ICO_HEADER_SIZE: usize = 6;

/// Directory entry size: 16 bytes
const ICO_DIR_ENTRY_SIZE: usize = 16;

/// Decode an ICO image from raw bytes.
///
/// Returns `Some(DecodedImage)` for the best icon entry found, or `None` if
/// the data is not valid ICO or no entry could be decoded.
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    // ICO header: reserved(2) + type(2) + count(2)
    if data.len() < ICO_HEADER_SIZE {
        return None;
    }

    let reserved = u16::from_le_bytes([data[0], data[1]]);
    let icon_type = u16::from_le_bytes([data[2], data[3]]);
    let count = u16::from_le_bytes([data[4], data[5]]) as usize;

    // Reserved should be 0; type 1 = ICO, type 2 = CUR
    if reserved != 0 {
        return None;
    }
    if icon_type != 1 && icon_type != 2 {
        return None;
    }
    if count == 0 || count > 255 {
        return None;
    }

    // Read all directory entries
    let entries_start = ICO_HEADER_SIZE;
    let entries_end = entries_start + count * ICO_DIR_ENTRY_SIZE;
    if data.len() < entries_end {
        return None;
    }

    // Find the best entry: prefer 256x256, then largest image
    let mut best_idx = 0;
    let mut best_score: u32 = 0;

    for i in 0..count {
        let entry_offset = entries_start + i * ICO_DIR_ENTRY_SIZE;
        let entry = &data[entry_offset..entry_offset + ICO_DIR_ENTRY_SIZE];

        let w = entry[0] as u32;
        let h = entry[1] as u32;
        // Width/height of 0 means 256 pixels
        let actual_w = if w == 0 { 256 } else { w };
        let actual_h = if h == 0 { 256 } else { h };

        let score = actual_w.saturating_mul(actual_h);
        if score > best_score {
            best_score = score;
            best_idx = i;
        }
    }

    // Decode the best entry
    decode_entry(data, best_idx, icon_type == 2)
}

/// Decode a single ICO directory entry by index.
fn decode_entry(data: &[u8], index: usize, cursor: bool) -> Option<DecodedImage> {
    let entry_offset = ICO_HEADER_SIZE + index * ICO_DIR_ENTRY_SIZE;
    let entry = &data[entry_offset..entry_offset + ICO_DIR_ENTRY_SIZE];

    // Directory entry fields:
    //   byte 0:    width (0 = 256)
    //   byte 1:    height (0 = 256)
    //   byte 2:    palette colors (0 if >= 256)
    //   byte 3:    reserved (0)
    //   bytes 4-5: color planes (should be 0 or 1)
    //   bytes 6-7: bits per pixel
    //   bytes 8-11: size of entry data in bytes
    //   bytes 12-15: offset of entry data from start of file
    let _w = entry[0];
    let _h = entry[1];
    let _palette = entry[2];
    let _reserved = entry[3];
    let _planes = u16::from_le_bytes([entry[4], entry[5]]);
    let _bpp = u16::from_le_bytes([entry[6], entry[7]]);
    let data_size = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize;
    let data_offset = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;

    // Validate bounds
    if data_size == 0 || data_offset == 0 {
        return None;
    }
    let entry_data_start = data_offset;
    let entry_data_end = entry_data_start + data_size;

    let entry_data = data.get(entry_data_start..entry_data_end)?;

    // Check if the entry data is PNG (magic: 0x89 0x50 0x4E 0x47)
    if entry_data.len() >= 8 && entry_data[0..4] == [0x89, 0x50, 0x4E, 0x47] {
        // Decode as PNG
        #[cfg(feature = "png")]
        {
            crate::codecs::png::decode::decode(entry_data)
        }
        #[cfg(not(feature = "png"))]
        {
            None
        }
    } else {
        // BMP/DIB data inside ICO
        // ICO BMP data starts with a BITMAPINFOHEADER (40 bytes) at offset 0,
        // but without the standard BMP file header (no "BM" signature).
        // We extract the pixel data manually.
        if cursor {
            decode_cur_bmp(entry_data)
        } else {
            decode_ico_bmp(entry_data, entry)
        }
    }
}

/// Decode a CUR DIB using Pillow's BMP semantics: retain its indexed mode and
/// read only the XOR plane represented by half of the stored DIB height.
fn decode_cur_bmp(data: &[u8]) -> Option<DecodedImage> {
    let header_size_bytes = data.get(..4)?;
    let header_size = u32::from_le_bytes([
        header_size_bytes[0],
        header_size_bytes[1],
        header_size_bytes[2],
        header_size_bytes[3],
    ]) as usize;
    if header_size < 40 || data.len() < header_size {
        return None;
    }
    let header = &data[..40];
    let stored_height = i32::from_le_bytes([header[8], header[9], header[10], header[11]]);
    let actual_height = stored_height / 2;
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
        cur_bmp_prefix(data.len(), header_size, palette_entries)?;
    let mut bmp = Vec::with_capacity(file_size);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&file_size_bytes);
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&pixel_offset_bytes);
    bmp.extend_from_slice(data);
    // `data.len() >= header_size >= 40`, so the synthetic BMP is always at
    // least 54 bytes (`14 + data.len()`), and the height field is present.
    bmp[22..26].copy_from_slice(&actual_height.to_le_bytes());
    crate::codecs::bmp::decode::decode(&bmp)
}

fn cur_bmp_prefix(
    data_len: usize,
    header_size: usize,
    palette_entries: usize,
) -> Option<(usize, [u8; 4], [u8; 4])> {
    let pixel_offset = 14usize
        .checked_add(header_size)?
        .checked_add(palette_entries.checked_mul(4)?)?;
    let file_size = 14usize.checked_add(data_len)?;
    Some((
        file_size,
        u32::try_from(file_size).ok()?.to_le_bytes(),
        u32::try_from(pixel_offset).ok()?.to_le_bytes(),
    ))
}

/// Decode an embedded BMP/DIB entry inside an ICO file.
///
/// ICO-embedded BMP data differs from standalone BMPs:
///   - No "BM" file header (starts directly with BITMAPINFOHEADER)
///   - Pixel data is uncompressed and stored in a specific layout
fn decode_ico_bmp(data: &[u8], _entry: &[u8]) -> Option<DecodedImage> {
    if data.len() < 40 {
        return None;
    }

    // BITMAPINFOHEADER fields
    let _header_size = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let width = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let height = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);

    // ICO height is doubled in BMP header (AND mask row is included)
    let actual_height = height / 2;

    let _planes = u16::from_le_bytes([data[12], data[13]]);
    let bpp = u16::from_le_bytes([data[14], data[15]]);
    let _compression = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
    let _image_size = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
    let colors_used = u32::from_le_bytes([data[32], data[33], data[34], data[35]]);

    if width == 0 || actual_height == 0 || width > 16384 || actual_height > 16384 {
        return None;
    }

    match bpp {
        32 => decode_ico_bmp_32bpp(data, width, actual_height),
        24 => decode_ico_bmp_24bpp(data, width, actual_height),
        8 => decode_ico_bmp_8bpp(data, width, actual_height, colors_used),
        4 => decode_ico_bmp_4bpp(data, width, actual_height, colors_used),
        1 => decode_ico_bmp_1bpp(data, width, actual_height, colors_used),
        _ => None,
    }
}

/// Decode a 32-bit BGRA ICO BMP entry (4 bytes/pixel).
fn decode_ico_bmp_32bpp(data: &[u8], width: u32, height: u32) -> Option<DecodedImage> {
    let header_size = 40;
    let row_size = width as usize * 4;
    // Each row is padded to a multiple of 4 bytes
    let padded_row = (row_size + 3) & !3;
    let pixel_data_size = padded_row * height as usize;

    let pixel_start = header_size;
    let pixel_end = pixel_start + pixel_data_size;
    let pixels_raw = data.get(pixel_start..pixel_end)?;

    let mut pixels = Vec::with_capacity(row_size * height as usize);

    // ICO BMP stores rows bottom-up; we flip to top-down
    for y in (0..height as usize).rev() {
        let row_start = y * padded_row;
        let row_end = row_start + row_size;
        let row = &pixels_raw[row_start..row_end];

        // BGRA → RGBA conversion
        for chunk in row.chunks(4) {
            let b = chunk[0];
            let g = chunk[1];
            let r = chunk[2];
            let a = chunk[3];
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
            pixels.push(a);
        }
    }

    Some(DecodedImage::new(width, height, pixels, ColorType::Rgba8))
}

/// Decode a 24-bit BGR ICO BMP entry (3 bytes/pixel).
fn decode_ico_bmp_24bpp(data: &[u8], width: u32, height: u32) -> Option<DecodedImage> {
    let header_size = 40;
    let row_size = width as usize * 3;
    let padded_row = (row_size + 3) & !3;
    let pixel_data_size = padded_row * height as usize;

    let pixel_start = header_size;
    let pixel_end = pixel_start + pixel_data_size;
    let pixels_raw = data.get(pixel_start..pixel_end)?;

    // Pillow IcoImagePlugin reads the padded AND mask from the end of the DIB
    // entry. Its BMP writer may emit fewer explicit mask bytes, in which case
    // this deliberately overlaps the tail of the XOR bitmap as Pillow does.
    // A valid 24-bit XOR plane is always larger than its mask, so the slice is
    // present once `pixels_raw` above succeeded. Pillow overlaps the XOR tail
    // when explicit mask bytes are omitted.
    let (mask, mask_row_size) = ico_and_mask_after_xor(data, width, height);
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);

    for y in (0..height as usize).rev() {
        let row_start = y * padded_row;
        let row_end = row_start + row_size;
        let row = &pixels_raw[row_start..row_end];

        for (x, chunk) in row.chunks(3).enumerate() {
            let b = chunk[0];
            let g = chunk[1];
            let r = chunk[2];
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
            let byte = mask[y * mask_row_size + x / 8];
            let transparent = byte & (0x80 >> (x % 8)) != 0;
            pixels.push(if transparent { 0 } else { 255 });
        }
    }

    Some(DecodedImage::new(width, height, pixels, ColorType::Rgba8))
}

/// Decode an 8-bit indexed ICO BMP entry (palette + indices).
fn decode_ico_bmp_8bpp(
    data: &[u8],
    width: u32,
    height: u32,
    colors_used: u32,
) -> Option<DecodedImage> {
    let header_size = 40;
    let color_count = (if colors_used == 0 { 256 } else { colors_used }) as usize;
    let palette_size = ico_palette_bytes(color_count);
    #[cfg(not(target_pointer_width = "64"))]
    let palette_size = palette_size?;
    let palette_end = header_size + palette_size;

    let row_size = width as usize;
    let padded_row = (row_size + 3) & !3;
    let pixel_data_size = padded_row * height as usize;

    let pixel_start = palette_end;
    let pixel_end = pixel_start + pixel_data_size;
    let pixels_raw = data.get(pixel_start..pixel_end)?;

    // Read palette (BGRA → RGBA)
    let palette_raw = &data[header_size..palette_end];
    let mut palette = Vec::with_capacity(color_count);
    for i in 0..color_count {
        let offset = i * 4;
        let b = palette_raw[offset];
        let g = palette_raw[offset + 1];
        let r = palette_raw[offset + 2];
        palette.push([r, g, b]);
    }

    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    let (mask, mask_row_size) = ico_and_mask_after_xor(data, width, height);

    for y in (0..height as usize).rev() {
        let row_start = y * padded_row;
        let row_end = row_start + row_size;
        let row = &pixels_raw[row_start..row_end];

        for (x, &idx) in row.iter().enumerate() {
            let color = palette[idx as usize];
            pixels.push(color[0]);
            pixels.push(color[1]);
            pixels.push(color[2]);
            pixels.push(mask_alpha(mask, mask_row_size, x, y));
        }
    }

    Some(DecodedImage::new(width, height, pixels, ColorType::Rgba8))
}

/// Decode a 4-bit indexed ICO BMP entry.
fn decode_ico_bmp_4bpp(
    data: &[u8],
    width: u32,
    height: u32,
    colors_used: u32,
) -> Option<DecodedImage> {
    let header_size = 40;
    let color_count = (if colors_used == 0 { 16 } else { colors_used }) as usize;
    let palette_size = ico_palette_bytes(color_count);
    #[cfg(not(target_pointer_width = "64"))]
    let palette_size = palette_size?;
    let palette_end = header_size + palette_size;

    // 4bpp: 2 pixels per byte
    let row_bytes = (width as usize).div_ceil(2);
    let padded_row = (row_bytes + 3) & !3;
    let pixel_data_size = padded_row * height as usize;

    let pixel_start = palette_end;
    let pixel_end = pixel_start + pixel_data_size;
    let pixels_raw = data.get(pixel_start..pixel_end)?;

    // Read palette
    let palette_raw = &data[header_size..palette_end];
    let mut palette = Vec::with_capacity(color_count);
    for i in 0..color_count {
        let offset = i * 4;
        let b = palette_raw[offset];
        let g = palette_raw[offset + 1];
        let r = palette_raw[offset + 2];
        palette.push([r, g, b]);
    }

    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    let (mask, mask_row_size) = ico_and_mask_after_xor(data, width, height);

    for y in (0..height as usize).rev() {
        let row_start = y * padded_row;
        let row_end = row_start + row_bytes;
        let row = &pixels_raw[row_start..row_end];

        let mut col = 0;
        for &byte in row {
            let hi = (byte >> 4) & 0x0F;
            let lo = byte & 0x0F;
            let color = palette[hi as usize];
            pixels.push(color[0]);
            pixels.push(color[1]);
            pixels.push(color[2]);
            pixels.push(mask_alpha(mask, mask_row_size, col, y));
            col += 1;
            if col < width as usize {
                let color = palette[lo as usize];
                pixels.push(color[0]);
                pixels.push(color[1]);
                pixels.push(color[2]);
                pixels.push(mask_alpha(mask, mask_row_size, col, y));
            }
            col += 1;
        }
    }

    Some(DecodedImage::new(width, height, pixels, ColorType::Rgba8))
}

/// Decode a 1-bit indexed ICO BMP entry.
fn decode_ico_bmp_1bpp(
    data: &[u8],
    width: u32,
    height: u32,
    colors_used: u32,
) -> Option<DecodedImage> {
    let header_size = 40;
    let color_count = (if colors_used == 0 { 2 } else { colors_used }) as usize;
    let palette_size = ico_palette_bytes(color_count);
    #[cfg(not(target_pointer_width = "64"))]
    let palette_size = palette_size?;
    let palette_end = header_size + palette_size;

    // 1bpp: 8 pixels per byte
    let row_bytes = (width as usize).div_ceil(8);
    let padded_row = (row_bytes + 3) & !3;
    let pixel_data_size = padded_row * height as usize;

    let pixel_start = palette_end;
    let pixel_end = pixel_start + pixel_data_size;
    let pixels_raw = data.get(pixel_start..pixel_end)?;

    // Read palette
    let palette_raw = &data[header_size..palette_end];
    let mut palette = Vec::with_capacity(color_count);
    for i in 0..color_count {
        let offset = i * 4;
        let b = palette_raw[offset];
        let g = palette_raw[offset + 1];
        let r = palette_raw[offset + 2];
        palette.push([r, g, b]);
    }

    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    let (mask, mask_row_size) = ico_and_mask_after_xor(data, width, height);

    for y in (0..height as usize).rev() {
        let row_start = y * padded_row;
        let row_end = row_start + row_bytes;
        let row = &pixels_raw[row_start..row_end];

        let mut col = 0;
        for &byte in row {
            for bit in (0..8).rev() {
                if col >= width as usize {
                    break;
                }
                let idx = ((byte >> bit) & 1) as usize;
                let color = palette[idx];
                pixels.push(color[0]);
                pixels.push(color[1]);
                pixels.push(color[2]);
                pixels.push(mask_alpha(mask, mask_row_size, col, y));
                col += 1;
            }
        }
    }

    Some(DecodedImage::new(width, height, pixels, ColorType::Rgba8))
}

fn ico_and_mask_after_xor(data: &[u8], width: u32, height: u32) -> (&[u8], usize) {
    let row_size = (width as usize).div_ceil(32) * 4;
    let size = row_size * height as usize;
    debug_assert!(data.len() >= size);
    (&data[data.len() - size..], row_size)
}

#[cfg(target_pointer_width = "64")]
fn ico_palette_bytes(color_count: usize) -> usize {
    color_count * 4
}

#[cfg(not(target_pointer_width = "64"))]
fn ico_palette_bytes(color_count: usize) -> Option<usize> {
    color_count.checked_mul(4)
}

fn mask_alpha(mask: &[u8], row_size: usize, x: usize, y: usize) -> u8 {
    let transparent = mask[y * row_size + x / 8] & (0x80 >> (x % 8)) != 0;
    if transparent { 0 } else { 255 }
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let mut too_many = Vec::new();
    too_many.extend_from_slice(&0u16.to_le_bytes());
    too_many.extend_from_slice(&1u16.to_le_bytes());
    too_many.extend_from_slice(&256u16.to_le_bytes());
    assert!(decode(&too_many).is_none());

    let mut two_entries = Vec::new();
    two_entries.extend_from_slice(&0u16.to_le_bytes());
    two_entries.extend_from_slice(&1u16.to_le_bytes());
    two_entries.extend_from_slice(&2u16.to_le_bytes());
    two_entries.extend_from_slice(&[16, 16, 0, 0]);
    two_entries.extend_from_slice(&1u16.to_le_bytes());
    two_entries.extend_from_slice(&32u16.to_le_bytes());
    two_entries.extend_from_slice(&1u32.to_le_bytes());
    two_entries.extend_from_slice(&38u32.to_le_bytes());
    two_entries.extend_from_slice(&[8, 8, 0, 0]);
    two_entries.extend_from_slice(&1u16.to_le_bytes());
    two_entries.extend_from_slice(&32u16.to_le_bytes());
    two_entries.extend_from_slice(&1u32.to_le_bytes());
    two_entries.extend_from_slice(&38u32.to_le_bytes());
    two_entries.push(0);
    assert!(decode(&two_entries).is_none());

    let mut zero_size = two_entries.clone();
    zero_size[14..18].copy_from_slice(&0u32.to_le_bytes());
    assert!(decode_entry(&zero_size, 0, false).is_none());
    let mut zero_offset = two_entries.clone();
    zero_offset[18..22].copy_from_slice(&0u32.to_le_bytes());
    assert!(decode_entry(&zero_offset, 0, false).is_none());

    let short_payload = &two_entries[..39];
    assert!(decode_entry(short_payload, 0, false).is_none());
    assert!(decode_cur_bmp(&[]).is_none());
    assert!(decode_cur_bmp(&[39, 0, 0, 0]).is_none());
    assert!(decode_cur_bmp(&[40, 0, 0, 0]).is_none());
    let cur_dib = indexed_dib(1, 1, 8, 2, &[1]);
    assert!(decode_cur_bmp(&cur_dib).is_some());
    let mut cur_oversized_palette = vec![0u8; 40];
    cur_oversized_palette[0..4].copy_from_slice(&40u32.to_le_bytes());
    cur_oversized_palette[8..12].copy_from_slice(&2i32.to_le_bytes());
    cur_oversized_palette[14..16].copy_from_slice(&8u16.to_le_bytes());
    cur_oversized_palette[32..36].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(decode_cur_bmp(&cur_oversized_palette).is_none());
    assert!(cur_bmp_prefix(1, usize::MAX, 0).is_none());
    assert!(cur_bmp_prefix(1, usize::MAX - 20, 4).is_none());
    assert!(cur_bmp_prefix(1, 0, usize::MAX).is_none());
    assert!(cur_bmp_prefix(usize::MAX, 0, 0).is_none());
    assert!(cur_bmp_prefix(u32::MAX as usize, 0, 0).is_none());

    for (width, stored_height) in [(0u32, 2u32), (1, 0), (16_385, 2), (1, 32_770)] {
        let mut dib = vec![0u8; 40];
        dib[0..4].copy_from_slice(&40u32.to_le_bytes());
        dib[4..8].copy_from_slice(&width.to_le_bytes());
        dib[8..12].copy_from_slice(&stored_height.to_le_bytes());
        dib[14..16].copy_from_slice(&32u16.to_le_bytes());
        assert!(decode_ico_bmp(&dib, &[]).is_none());
    }

    let dib24 = dib24(1, 1, &[0, 0, 255], &[0x80]);
    assert!(decode_ico_bmp_24bpp(&dib24, 1, 1).is_some());

    let dib8 = indexed_dib(1, 1, 8, 3, &[0]);
    assert!(decode_ico_bmp_8bpp(&dib8, 1, 1, 3).is_some());
    let dib8_masked = indexed_dib_with_mask(2, 1, 8, 3, &[0, 1], &[0x40]);
    assert!(decode_ico_bmp_8bpp(&dib8_masked, 2, 1, 3).is_some());
    let dib8_default_palette = indexed_dib(1, 1, 8, 256, &[0]);
    assert!(decode_ico_bmp_8bpp(&dib8_default_palette, 1, 1, 0).is_some());

    let dib4 = indexed_dib(3, 1, 4, 3, &[0x12, 0]);
    assert!(decode_ico_bmp_4bpp(&dib4, 3, 1, 3).is_some());
    let dib4_even = indexed_dib(4, 1, 4, 3, &[0x12, 0x10]);
    assert!(decode_ico_bmp_4bpp(&dib4_even, 4, 1, 3).is_some());
    let dib4_masked = indexed_dib_with_mask(2, 1, 4, 3, &[0x12], &[0x40]);
    assert!(decode_ico_bmp_4bpp(&dib4_masked, 2, 1, 3).is_some());
    let dib4_default_palette = indexed_dib(1, 1, 4, 16, &[0]);
    assert!(decode_ico_bmp_4bpp(&dib4_default_palette, 1, 1, 0).is_some());

    let dib1 = indexed_dib(1, 1, 1, 2, &[0x80]);
    assert!(decode_ico_bmp_1bpp(&dib1, 1, 1, 2).is_some());
    let dib1_masked = indexed_dib_with_mask(2, 1, 1, 2, &[0x80], &[0x40]);
    assert!(decode_ico_bmp_1bpp(&dib1_masked, 2, 1, 2).is_some());
    let dib1_default_palette = indexed_dib(1, 1, 1, 2, &[0x80]);
    assert!(decode_ico_bmp_1bpp(&dib1_default_palette, 1, 1, 0).is_some());
}

#[cfg(coverage)]
fn indexed_dib(width: u32, height: u32, bpp: u16, colors: u32, xor: &[u8]) -> Vec<u8> {
    indexed_dib_with_mask(width, height, bpp, colors, xor, &[])
}

#[cfg(coverage)]
fn indexed_dib_with_mask(
    width: u32,
    height: u32,
    bpp: u16,
    colors: u32,
    xor: &[u8],
    and_mask: &[u8],
) -> Vec<u8> {
    let palette_entries = usize::try_from(colors).expect("coverage palette fits usize");
    let row_bytes = (width as usize * usize::from(bpp)).div_ceil(8);
    let padded_row = (row_bytes + 3) & !3;
    let mask_row = (width as usize).div_ceil(32) * 4;
    let mut dib = vec![0u8; 40];
    dib[0..4].copy_from_slice(&40u32.to_le_bytes());
    dib[4..8].copy_from_slice(&width.to_le_bytes());
    dib[8..12].copy_from_slice(&(height * 2).to_le_bytes());
    dib[12..14].copy_from_slice(&1u16.to_le_bytes());
    dib[14..16].copy_from_slice(&bpp.to_le_bytes());
    dib[32..36].copy_from_slice(&colors.to_le_bytes());
    for index in 0..palette_entries {
        let value = u8::try_from(index).expect("coverage palette value fits u8");
        dib.extend_from_slice(&[value, value, value, 0]);
    }
    let mut xor_plane = vec![0u8; padded_row * height as usize];
    xor_plane[..xor.len()].copy_from_slice(xor);
    dib.extend_from_slice(&xor_plane);
    let mut mask_plane = vec![0u8; mask_row * height as usize];
    mask_plane[..and_mask.len()].copy_from_slice(and_mask);
    dib.extend_from_slice(&mask_plane);
    dib
}

#[cfg(coverage)]
fn dib24(width: u32, height: u32, xor: &[u8], and_mask: &[u8]) -> Vec<u8> {
    let row_bytes = width as usize * 3;
    let padded_row = (row_bytes + 3) & !3;
    let mask_row = (width as usize).div_ceil(32) * 4;
    let mut dib = vec![0u8; 40];
    dib[0..4].copy_from_slice(&40u32.to_le_bytes());
    dib[4..8].copy_from_slice(&width.to_le_bytes());
    dib[8..12].copy_from_slice(&(height * 2).to_le_bytes());
    dib[12..14].copy_from_slice(&1u16.to_le_bytes());
    dib[14..16].copy_from_slice(&24u16.to_le_bytes());
    let mut xor_plane = vec![0u8; padded_row * height as usize];
    xor_plane[..xor.len()].copy_from_slice(xor);
    dib.extend_from_slice(&xor_plane);
    let mut mask_plane = vec![0u8; mask_row * height as usize];
    mask_plane[..and_mask.len()].copy_from_slice(and_mask);
    dib.extend_from_slice(&mask_plane);
    dib
}
