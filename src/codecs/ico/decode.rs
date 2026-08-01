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
//! Reference: <https://en.wikipedia.org/wiki/ICO_(file_format)>

use crate::codecs::{CodecError, CodecResult, OptionCodecExt, need_slice, terminalize};
use crate::types::{ColorType, CursorHotspot, DecodedImage};

/// ICO header size: 6 bytes
const ICO_HEADER_SIZE: usize = 6;

/// Directory entry size: 16 bytes
const ICO_DIR_ENTRY_SIZE: usize = 16;

/// Decode an ICO image from raw bytes.
///
/// Returns the best icon entry or a classified container/payload failure.
pub fn decode(
    data: &[u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(DecodedImage, Option<usize>)> {
    crate::codecs::error::check_cancelled(token)?;
    // ICO header: reserved(2) + type(2) + count(2)
    if data.len() < ICO_HEADER_SIZE {
        return Err(CodecError::NeedMore {
            minimum: ICO_HEADER_SIZE,
            message: "truncated ICO header".to_owned(),
        }
        .at(0, "ico_header"));
    }

    let reserved = u16::from_le_bytes([data[0], data[1]]);
    let icon_type = u16::from_le_bytes([data[2], data[3]]);
    let count = u16::from_le_bytes([data[4], data[5]]) as usize;

    // Reserved should be 0; type 1 = ICO, type 2 = CUR
    if reserved != 0 {
        return Err(
            CodecError::Malformed("ICO reserved header field is nonzero".to_owned())
                .at(0, "ico_header"),
        );
    }
    if icon_type != 1 && icon_type != 2 {
        return Err(CodecError::Malformed(
            "ICO container type is neither icon nor cursor".to_owned(),
        )
        .at(2, "ico_header"));
    }
    if count == 0 || count > 255 {
        return Err(CodecError::Malformed(
            "ICO directory count is empty or exceeds Pillow's limit".to_owned(),
        )
        .at(4, "ico_header"));
    }

    // Read all directory entries
    let entries_start = ICO_HEADER_SIZE;
    let entries_end = entries_start.saturating_add(count.saturating_mul(ICO_DIR_ENTRY_SIZE));
    if data.len() < entries_end {
        return Err(CodecError::NeedMore {
            minimum: entries_end,
            message: "truncated ICO directory".to_owned(),
        }
        .at(
            u64::try_from(entries_start).unwrap_or(u64::MAX),
            "ico_directory",
        ));
    }

    // Find the best entry: prefer 256x256, then largest image
    let mut best_idx = 0;
    let mut best_score: u32 = 0;

    for i in 0..count {
        crate::codecs::error::check_cancelled(token)?;
        let entry_offset = entries_start.saturating_add(i.saturating_mul(ICO_DIR_ENTRY_SIZE));
        let entry = &data[entry_offset..entry_offset.saturating_add(ICO_DIR_ENTRY_SIZE)];

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

    // Decode the best entry and retain the two CUR hotspot fields that occupy
    // the ICO plane/bit-depth positions.
    let image = decode_entry(data, best_idx, icon_type == 2, token)?;
    if icon_type == 2 {
        let entry_offset =
            ICO_HEADER_SIZE.saturating_add(best_idx.saturating_mul(ICO_DIR_ENTRY_SIZE));
        let entry = &data[entry_offset..entry_offset.saturating_add(ICO_DIR_ENTRY_SIZE)];
        return Ok((
            image.with_cursor_hotspot(CursorHotspot {
                x: u16::from_le_bytes([entry[4], entry[5]]),
                y: u16::from_le_bytes([entry[6], entry[7]]),
            }),
            None,
        ));
    }
    // ICO/CUR directories do not declare a total extent; trailing bytes are
    // ignored and the complete input remains the source.
    Ok((image, None))
}

/// Measure the encoded metadata extent: the file header plus the complete
/// entry directory. Entry payload bytes are pixel data by this contract.
pub(crate) fn metadata_bytes(data: &[u8]) -> CodecResult<u64> {
    if data.len() < ICO_HEADER_SIZE {
        return Err(CodecError::NeedMore {
            minimum: ICO_HEADER_SIZE,
            message: "truncated ICO header".to_owned(),
        }
        .at(0, "ico_header"));
    }
    let count = u64::from(u16::from_le_bytes([data[4], data[5]]));
    Ok(6u64.saturating_add(count.saturating_mul(ICO_DIR_ENTRY_SIZE as u64)))
}

/// Decode a single ICO directory entry by index.
fn decode_entry(
    data: &[u8],
    index: usize,
    cursor: bool,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<DecodedImage> {
    crate::codecs::error::check_cancelled(token)?;
    let entry_offset = ICO_HEADER_SIZE.saturating_add(index.saturating_mul(ICO_DIR_ENTRY_SIZE));
    // `decode(, None)` validates the complete directory and selects `index` from
    // `0..count`, so this private slice is bounded before dispatch.
    let entry = &data[entry_offset..entry_offset.saturating_add(ICO_DIR_ENTRY_SIZE)];

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
    let data_size_u32 = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]);
    let data_size = data_size_u32 as usize;
    let data_offset = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;

    // Validate bounds
    if data_size == 0 {
        return Err(
            CodecError::Malformed("ICO directory entry has an empty size".to_owned()).at(
                u64::try_from(entry_offset.saturating_add(8)).unwrap_or(u64::MAX),
                "ico_entry",
            ),
        );
    }
    if data_offset == 0 {
        return Err(
            CodecError::Malformed("ICO directory entry has an empty offset".to_owned()).at(
                u64::try_from(entry_offset.saturating_add(12)).unwrap_or(u64::MAX),
                "ico_entry",
            ),
        );
    }
    let entry_data_start = data_offset;
    let entry_data_end = entry_data_start.saturating_add(data_size);

    let entry_data = need_slice(
        data,
        entry_data_start,
        entry_data_end,
        "ICO entry payload is out of bounds",
    )
    .map_err(|error| {
        error.at(
            u64::try_from(entry_data_start).unwrap_or(u64::MAX),
            "ico_entry",
        )
    })?;

    // Check if the entry data is PNG (magic: 0x89 0x50 0x4E 0x47)
    if entry_data.len() >= 8 && entry_data[0..4] == [0x89, 0x50, 0x4E, 0x47] {
        // Decode as PNG
        #[cfg(feature = "png")]
        {
            crate::codecs::png::decode::decode(entry_data, token)
                .map(|(image, _, _)| image)
                .map_err(terminalize)
                .map_err(|error| {
                    error.at(u64::try_from(data_offset).unwrap_or(u64::MAX), "ico_png")
                })
        }
        #[cfg(not(feature = "png"))]
        {
            Err(
                CodecError::Unsupported("ICO PNG entry requires the PNG feature".to_owned())
                    .at(u64::try_from(data_offset).unwrap_or(u64::MAX), "ico_png"),
            )
        }
    } else {
        // BMP/DIB data inside ICO
        // ICO BMP data starts with a BITMAPINFOHEADER (40 bytes) at offset 0,
        // but without the standard BMP file header (no "BM" signature).
        // We extract the pixel data manually.
        if cursor {
            decode_cur_bmp(entry_data, data_size_u32)
                .map_err(terminalize)
                .map_err(|error| {
                    error.at(
                        u64::try_from(data_offset).unwrap_or(u64::MAX),
                        "ico_cur_dib",
                    )
                })
        } else {
            decode_ico_bmp(entry_data, entry)
                .map_err(terminalize)
                .map_err(|error| {
                    error.at(u64::try_from(data_offset).unwrap_or(u64::MAX), "ico_dib")
                })
        }
    }
}

/// Decode a CUR DIB using Pillow's BMP semantics: retain its indexed mode and
/// read only the XOR plane represented by half of the stored DIB height.
fn decode_cur_bmp(data: &[u8], declared_len: u32) -> CodecResult<DecodedImage> {
    let header_size_bytes = data.get(..4).malformed("truncated CUR DIB header size")?;
    let header_size_u32 = u32::from_le_bytes([
        header_size_bytes[0],
        header_size_bytes[1],
        header_size_bytes[2],
        header_size_bytes[3],
    ]);
    let header_size = header_size_u32 as usize;
    if header_size < 40 {
        return Err(CodecError::Unsupported(
            "unsupported CUR DIB header size".to_owned(),
        ));
    }
    if data.len() < header_size {
        return Err(CodecError::Malformed("truncated CUR DIB header".to_owned()));
    }
    let header = &data[..40];
    let stored_height = i32::from_le_bytes([header[8], header[9], header[10], header[11]]);
    let actual_height = stored_height / 2;
    let bits = u16::from_le_bytes([header[14], header[15]]);
    let colors_used = u32::from_le_bytes([header[32], header[33], header[34], header[35]]);
    let palette_entries = if bits <= 8 {
        if colors_used == 0 {
            1u32 << bits
        } else {
            colors_used
        }
    } else {
        0
    };
    let (file_size_bytes, pixel_offset_bytes) =
        cur_bmp_prefix(declared_len, header_size_u32, palette_entries)?;
    let mut bmp = Vec::with_capacity(data.len());
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&file_size_bytes);
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&pixel_offset_bytes);
    bmp.extend_from_slice(data);
    // `data.len() >= header_size >= 40`, so the synthetic BMP is always at
    // least 54 bytes (`14 + data.len()`), and the height field is present.
    bmp[22..26].copy_from_slice(&actual_height.to_le_bytes());
    crate::codecs::bmp::decode::decode(&bmp, None).map(|(image, _)| image)
}

pub(super) fn cur_bmp_prefix(
    data_len: u32,
    header_size: u32,
    palette_entries: u32,
) -> CodecResult<([u8; 4], [u8; 4])> {
    // The delegated BMP parser reads but does not use `bfSize`. Preserve its
    // exact value for every representable CUR payload; wrapping only affects a
    // synthetic header for a payload whose size plus the 14-byte wrapper is
    // itself not representable in the on-disk u32 field.
    let file_size_bytes = data_len.wrapping_add(14).to_le_bytes();
    let pixel_offset = 14u64
        .wrapping_add(u64::from(header_size))
        .wrapping_add(u64::from(palette_entries).wrapping_mul(4));
    let pixel_offset_bytes = match u32::try_from(pixel_offset) {
        Ok(pixel_offset) => pixel_offset.to_le_bytes(),
        Err(_) => {
            return Err(CodecError::Unsupported(
                "unsupported CUR BMP palette size".to_owned(),
            ));
        }
    };
    Ok((file_size_bytes, pixel_offset_bytes))
}

/// Decode an embedded BMP/DIB entry inside an ICO file.
///
/// ICO-embedded BMP data differs from standalone BMPs:
///   - No "BM" file header (starts directly with BITMAPINFOHEADER)
///   - Pixel data is uncompressed and stored in a specific layout
fn decode_ico_bmp(data: &[u8], _entry: &[u8]) -> CodecResult<DecodedImage> {
    if data.len() < 40 {
        return Err(CodecError::Malformed("truncated ICO DIB header".to_owned()));
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
        return Err(CodecError::Malformed(
            "ICO DIB dimensions are empty or exceed the supported bounds".to_owned(),
        ));
    }

    match bpp {
        32 => decode_ico_bmp_32bpp(data, width, actual_height),
        24 => decode_ico_bmp_24bpp(data, width, actual_height),
        8 => decode_ico_bmp_8bpp(data, width, actual_height, colors_used),
        4 => decode_ico_bmp_4bpp(data, width, actual_height, colors_used),
        1 => decode_ico_bmp_1bpp(data, width, actual_height, colors_used),
        _ => Err(CodecError::Unsupported(format!(
            "unsupported ICO BMP pixel depth {bpp}"
        ))),
    }
}

/// Decode a 32-bit BGRA ICO BMP entry (4 bytes/pixel).
fn decode_ico_bmp_32bpp(data: &[u8], width: u32, height: u32) -> CodecResult<DecodedImage> {
    let header_size = 40_usize;
    let row_size = (width as usize).wrapping_mul(4);
    // Each row is padded to a multiple of 4 bytes
    let padded_row = row_size.wrapping_add(3) & !3;
    let pixel_data_size = padded_row.wrapping_mul(height as usize);

    let pixel_start = header_size;
    let pixel_end = pixel_start.wrapping_add(pixel_data_size);
    let pixels_raw = data
        .get(pixel_start..pixel_end)
        .malformed("truncated 32-bit ICO bitmap")?;

    let mut pixels = Vec::with_capacity(row_size.wrapping_mul(height as usize));

    // ICO BMP stores rows bottom-up; we flip to top-down
    for y in (0..height as usize).rev() {
        let row_start = y.wrapping_mul(padded_row);
        let row_end = row_start.wrapping_add(row_size);
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

    Ok(DecodedImage::new(width, height, pixels, ColorType::Rgba8))
}

/// Decode a 24-bit BGR ICO BMP entry (3 bytes/pixel).
fn decode_ico_bmp_24bpp(data: &[u8], width: u32, height: u32) -> CodecResult<DecodedImage> {
    let header_size = 40_usize;
    let row_size = (width as usize).wrapping_mul(3);
    let padded_row = row_size.wrapping_add(3) & !3;
    let pixel_data_size = padded_row.wrapping_mul(height as usize);

    let pixel_start = header_size;
    let pixel_end = pixel_start.wrapping_add(pixel_data_size);
    let pixels_raw = data
        .get(pixel_start..pixel_end)
        .malformed("truncated 24-bit ICO bitmap")?;

    // Pillow IcoImagePlugin reads the padded AND mask from the end of the DIB
    // entry. Its BMP writer may emit fewer explicit mask bytes, in which case
    // this deliberately overlaps the tail of the XOR bitmap as Pillow does.
    // A valid 24-bit XOR plane is always larger than its mask, so the slice is
    // present once `pixels_raw` above succeeded. Pillow overlaps the XOR tail
    // when explicit mask bytes are omitted.
    let (mask, mask_row_size) = ico_and_mask_after_xor(data, width, height);
    let mut pixels = Vec::with_capacity(
        (width as usize)
            .wrapping_mul(height as usize)
            .wrapping_mul(4),
    );

    for y in (0..height as usize).rev() {
        let row_start = y.wrapping_mul(padded_row);
        let row_end = row_start.wrapping_add(row_size);
        let row = &pixels_raw[row_start..row_end];

        for (x, chunk) in row.chunks(3).enumerate() {
            let b = chunk[0];
            let g = chunk[1];
            let r = chunk[2];
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
            let byte = mask[y.wrapping_mul(mask_row_size).wrapping_add(x / 8)];
            let transparent = byte & (0x80 >> (x % 8)) != 0;
            pixels.push(if transparent { 0 } else { 255 });
        }
    }

    Ok(DecodedImage::new(width, height, pixels, ColorType::Rgba8))
}

/// Decode an 8-bit indexed ICO BMP entry (palette + indices).
fn decode_ico_bmp_8bpp(
    data: &[u8],
    width: u32,
    height: u32,
    colors_used: u32,
) -> CodecResult<DecodedImage> {
    let header_size = 40_usize;
    let color_count = (if colors_used == 0 { 256 } else { colors_used }) as usize;
    #[cfg(target_pointer_width = "64")]
    let palette_size = ico_palette_bytes(color_count);
    #[cfg(not(target_pointer_width = "64"))]
    let palette_size = ico_palette_bytes(color_count)?;
    let palette_end = header_size.saturating_add(palette_size);

    let row_size = width as usize;
    let padded_row = row_size.wrapping_add(3) & !3;
    let pixel_data_size = padded_row.wrapping_mul(height as usize);

    let pixel_start = palette_end;
    let pixel_end = pixel_start.saturating_add(pixel_data_size);
    let pixels_raw = data
        .get(pixel_start..pixel_end)
        .malformed("truncated 8-bit ICO bitmap")?;

    // Read palette (BGRA → RGBA)
    // The validated XOR plane begins at `palette_end`, so reaching it proves
    // the complete preceding palette range is present.
    let palette_raw = &data[header_size..palette_end];
    let mut palette = Vec::with_capacity(color_count);
    for i in 0..color_count {
        let offset = i.wrapping_mul(4);
        let b = palette_raw[offset];
        let g = palette_raw[offset.wrapping_add(1)];
        let r = palette_raw[offset.wrapping_add(2)];
        palette.push([r, g, b]);
    }
    let mut pixels = Vec::with_capacity(
        (width as usize)
            .wrapping_mul(height as usize)
            .wrapping_mul(4),
    );
    let (mask, mask_row_size) = ico_and_mask_after_xor(data, width, height);

    for y in (0..height as usize).rev() {
        let row_start = y.wrapping_mul(padded_row);
        let row_end = row_start.wrapping_add(row_size);
        let row = &pixels_raw[row_start..row_end];

        for (x, &idx) in row.iter().enumerate() {
            // Pillow's 8-bit BMP decoder maps indices beyond an explicitly
            // shortened `ColorsUsed` table to black. Its 1-bit and 4-bit
            // decoder configurations reject the analogous declaration.
            let color = palette.get(idx as usize).copied().unwrap_or([0; 3]);
            pixels.push(color[0]);
            pixels.push(color[1]);
            pixels.push(color[2]);
            pixels.push(mask_alpha(mask, mask_row_size, x, y));
        }
    }

    Ok(DecodedImage::new(width, height, pixels, ColorType::Rgba8))
}

/// Decode a 4-bit indexed ICO BMP entry.
fn decode_ico_bmp_4bpp(
    data: &[u8],
    width: u32,
    height: u32,
    colors_used: u32,
) -> CodecResult<DecodedImage> {
    let header_size = 40_usize;
    let color_count = (if colors_used == 0 { 16 } else { colors_used }) as usize;
    #[cfg(target_pointer_width = "64")]
    let palette_size = ico_palette_bytes(color_count);
    #[cfg(not(target_pointer_width = "64"))]
    let palette_size = ico_palette_bytes(color_count)?;
    let palette_end = header_size.saturating_add(palette_size);

    // 4bpp: 2 pixels per byte
    let row_bytes = (width as usize).div_ceil(2);
    let padded_row = row_bytes.wrapping_add(3) & !3;
    let pixel_data_size = padded_row.wrapping_mul(height as usize);

    let pixel_start = palette_end;
    let pixel_end = pixel_start.saturating_add(pixel_data_size);
    let pixels_raw = data
        .get(pixel_start..pixel_end)
        .malformed("truncated 4-bit ICO bitmap")?;

    // Read palette
    // The validated XOR plane begins at `palette_end`, so reaching it proves
    // the complete preceding palette range is present.
    let palette_raw = &data[header_size..palette_end];
    let mut palette = Vec::with_capacity(color_count);
    for i in 0..color_count {
        let offset = i.wrapping_mul(4);
        let b = palette_raw[offset];
        let g = palette_raw[offset.wrapping_add(1)];
        let r = palette_raw[offset.wrapping_add(2)];
        palette.push([r, g, b]);
    }
    validate_4bit_palette_references(pixels_raw, width, height, padded_row, color_count)?;

    let mut pixels = Vec::with_capacity(
        (width as usize)
            .wrapping_mul(height as usize)
            .wrapping_mul(4),
    );
    let (mask, mask_row_size) = ico_and_mask_after_xor(data, width, height);

    for y in (0..height as usize).rev() {
        let row_start = y.wrapping_mul(padded_row);
        let row_end = row_start.wrapping_add(row_bytes);
        let row = &pixels_raw[row_start..row_end];

        let mut col = 0;
        for &byte in row {
            let hi = (byte >> 4) & 0x0F;
            let lo = byte & 0x0F;
            // The complete XOR plane was validated against this palette above.
            let color = palette[hi as usize];
            pixels.push(color[0]);
            pixels.push(color[1]);
            pixels.push(color[2]);
            pixels.push(mask_alpha(mask, mask_row_size, col, y));
            col = col.wrapping_add(1);
            if col < width as usize {
                let color = palette[lo as usize];
                pixels.push(color[0]);
                pixels.push(color[1]);
                pixels.push(color[2]);
                pixels.push(mask_alpha(mask, mask_row_size, col, y));
            }
            col = col.wrapping_add(1);
        }
    }

    Ok(DecodedImage::new(width, height, pixels, ColorType::Rgba8))
}

/// Decode a 1-bit indexed ICO BMP entry.
fn decode_ico_bmp_1bpp(
    data: &[u8],
    width: u32,
    height: u32,
    colors_used: u32,
) -> CodecResult<DecodedImage> {
    let header_size = 40_usize;
    let color_count = (if colors_used == 0 { 2 } else { colors_used }) as usize;
    #[cfg(target_pointer_width = "64")]
    let palette_size = ico_palette_bytes(color_count);
    #[cfg(not(target_pointer_width = "64"))]
    let palette_size = ico_palette_bytes(color_count)?;
    let palette_end = header_size.saturating_add(palette_size);

    // 1bpp: 8 pixels per byte
    let row_bytes = (width as usize).div_ceil(8);
    let padded_row = row_bytes.wrapping_add(3) & !3;
    let pixel_data_size = padded_row.wrapping_mul(height as usize);

    let pixel_start = palette_end;
    let pixel_end = pixel_start.saturating_add(pixel_data_size);
    let pixels_raw = data
        .get(pixel_start..pixel_end)
        .malformed("truncated 1-bit ICO bitmap")?;

    // Read palette
    // The validated XOR plane begins at `palette_end`, so reaching it proves
    // the complete preceding palette range is present.
    let palette_raw = &data[header_size..palette_end];
    let mut palette = Vec::with_capacity(color_count);
    for i in 0..color_count {
        let offset = i.wrapping_mul(4);
        let b = palette_raw[offset];
        let g = palette_raw[offset.wrapping_add(1)];
        let r = palette_raw[offset.wrapping_add(2)];
        palette.push([r, g, b]);
    }
    validate_1bit_palette_references(pixels_raw, width, height, padded_row, color_count)?;

    let mut pixels = Vec::with_capacity(
        (width as usize)
            .wrapping_mul(height as usize)
            .wrapping_mul(4),
    );
    let (mask, mask_row_size) = ico_and_mask_after_xor(data, width, height);

    for y in (0..height as usize).rev() {
        let row_start = y.wrapping_mul(padded_row);
        let row_end = row_start.wrapping_add(row_bytes);
        let row = &pixels_raw[row_start..row_end];

        let mut col = 0;
        for &byte in row {
            for bit in (0..8).rev() {
                if col >= width as usize {
                    break;
                }
                let idx = ((byte >> bit) & 1) as usize;
                // The complete XOR plane was validated against this palette above.
                let color = palette[idx];
                pixels.push(color[0]);
                pixels.push(color[1]);
                pixels.push(color[2]);
                pixels.push(mask_alpha(mask, mask_row_size, col, y));
                col = col.wrapping_add(1);
            }
        }
    }

    Ok(DecodedImage::new(width, height, pixels, ColorType::Rgba8))
}

pub(super) fn validate_4bit_palette_references(
    pixels: &[u8],
    width: u32,
    height: u32,
    padded_row: usize,
    palette_len: usize,
) -> CodecResult<()> {
    for y in 0..height as usize {
        let row_start = y.wrapping_mul(padded_row);
        let row = &pixels[row_start..row_start.wrapping_add(padded_row)];
        for x in 0..width as usize {
            let byte = row[x / 2];
            let index = if x % 2 == 0 { byte >> 4 } else { byte & 0x0f };
            if usize::from(index) >= palette_len {
                return Err(CodecError::Malformed(
                    "4-bit ICO pixel references a missing palette entry".to_owned(),
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_1bit_palette_references(
    pixels: &[u8],
    width: u32,
    height: u32,
    padded_row: usize,
    palette_len: usize,
) -> CodecResult<()> {
    for y in 0..height as usize {
        let row_start = y.wrapping_mul(padded_row);
        let row = &pixels[row_start..row_start.wrapping_add(padded_row)];
        for x in 0..width as usize {
            // `x % 8` is in `0..=7`; wrapping subtraction expresses the
            // proven bound without introducing a fallible arithmetic path.
            let shift = 7usize.wrapping_sub(x % 8);
            let index = (row[x / 8] >> shift) & 1;
            if usize::from(index) >= palette_len {
                return Err(CodecError::Malformed(
                    "1-bit ICO pixel references a missing palette entry".to_owned(),
                ));
            }
        }
    }
    Ok(())
}

fn ico_and_mask_after_xor(data: &[u8], width: u32, height: u32) -> (&[u8], usize) {
    let row_size = (width as usize).div_ceil(32).wrapping_mul(4);
    let size = row_size.wrapping_mul(height as usize);
    // Every caller first validates a complete padded XOR plane. At 1, 4, 8,
    // and 24 bits per pixel, that plane's padded row is always at least as
    // wide as this one-bit AND-mask row, so `data.len() >= size`.
    let start = data.len().wrapping_sub(size);
    (&data[start..], row_size)
}

#[cfg(target_pointer_width = "64")]
fn ico_palette_bytes(color_count: usize) -> usize {
    // ICO stores the palette count as u32, so four-byte entries cannot
    // overflow a 64-bit address-space index.
    color_count.wrapping_mul(4)
}

#[cfg(not(target_pointer_width = "64"))]
fn ico_palette_bytes(color_count: usize) -> CodecResult<usize> {
    match color_count.checked_mul(4) {
        Some(bytes) => Ok(bytes),
        None => Err(CodecError::Unsupported(
            "unsupported ICO palette size".to_owned(),
        )),
    }
}

fn mask_alpha(mask: &[u8], row_size: usize, x: usize, y: usize) -> u8 {
    let transparent = mask[y.wrapping_mul(row_size).wrapping_add(x / 8)] & (0x80 >> (x % 8)) != 0;
    if transparent { 0 } else { 255 }
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    assert!(decode(b"", None).is_err());
    assert!(decode(&[1, 0, 1, 0, 1, 0], None).is_err());
    assert!(decode(&[0, 0, 0, 0, 1, 0], None).is_err());
    assert!(decode(&[0, 0, 1, 0, 0, 0], None).is_err());
    let _ = metadata_bytes(b"");
    let _ = metadata_bytes(&[0, 0, 1, 0, 2, 0]);

    let mut too_many = Vec::new();
    too_many.extend_from_slice(&0u16.to_le_bytes());
    too_many.extend_from_slice(&1u16.to_le_bytes());
    too_many.extend_from_slice(&256u16.to_le_bytes());
    assert!(decode(&too_many, None).is_err());

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
    assert!(decode(&two_entries, None).is_err());
    let fixture = include_bytes!("../../../tests/fixtures/input/images/ico/16x16.ico");
    for checks in 0..=4 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = decode(fixture, Some(&token));
    }

    let mut zero_size = two_entries.clone();
    zero_size[14..18].copy_from_slice(&0u32.to_le_bytes());
    assert!(decode_entry(&zero_size, 0, false, None).is_err());
    let mut zero_offset = two_entries.clone();
    zero_offset[18..22].copy_from_slice(&0u32.to_le_bytes());
    assert!(decode_entry(&zero_offset, 0, false, None).is_err());

    let short_payload = &two_entries[..39];
    assert!(decode_entry(short_payload, 0, false, None).is_err());
    assert!(decode_cur_bmp(&[], 0).is_err());
    assert!(decode_cur_bmp(&[39, 0, 0, 0], 4).is_err());
    assert!(decode_cur_bmp(&[40, 0, 0, 0], 4).is_err());
    let cur_dib = indexed_dib(1, 1, 8, 2, &[1]);
    assert!(decode_cur_bmp(&cur_dib, cur_dib.len() as u32).is_ok());
    let mut cur_oversized_palette = vec![0u8; 40];
    cur_oversized_palette[0..4].copy_from_slice(&40u32.to_le_bytes());
    cur_oversized_palette[8..12].copy_from_slice(&2i32.to_le_bytes());
    cur_oversized_palette[14..16].copy_from_slice(&8u16.to_le_bytes());
    cur_oversized_palette[32..36].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(decode_cur_bmp(&cur_oversized_palette, cur_oversized_palette.len() as u32).is_err());

    for (width, stored_height) in [(0u32, 2u32), (1, 0), (16_385, 2), (1, 32_770)] {
        let mut dib = vec![0u8; 40];
        dib[0..4].copy_from_slice(&40u32.to_le_bytes());
        dib[4..8].copy_from_slice(&width.to_le_bytes());
        dib[8..12].copy_from_slice(&stored_height.to_le_bytes());
        dib[14..16].copy_from_slice(&32u16.to_le_bytes());
        assert!(decode_ico_bmp(&dib, &[]).is_err());
    }

    let dib24 = dib24(1, 1, &[0, 0, 255], &[0x80]);
    assert!(decode_ico_bmp_24bpp(&dib24, 1, 1).is_ok());

    let dib8 = indexed_dib(1, 1, 8, 3, &[0]);
    assert!(decode_ico_bmp_8bpp(&dib8, 1, 1, 3).is_ok());
    let dib8_masked = indexed_dib_with_mask(2, 1, 8, 3, &[0, 1], &[0x40]);
    assert!(decode_ico_bmp_8bpp(&dib8_masked, 2, 1, 3).is_ok());
    let dib8_default_palette = indexed_dib(1, 1, 8, 256, &[0]);
    assert!(decode_ico_bmp_8bpp(&dib8_default_palette, 1, 1, 0).is_ok());

    let dib4 = indexed_dib(3, 1, 4, 3, &[0x12, 0]);
    assert!(decode_ico_bmp_4bpp(&dib4, 3, 1, 3).is_ok());
    let dib4_even = indexed_dib(4, 1, 4, 3, &[0x12, 0x10]);
    assert!(decode_ico_bmp_4bpp(&dib4_even, 4, 1, 3).is_ok());
    let dib4_masked = indexed_dib_with_mask(2, 1, 4, 3, &[0x12], &[0x40]);
    assert!(decode_ico_bmp_4bpp(&dib4_masked, 2, 1, 3).is_ok());
    let dib4_default_palette = indexed_dib(1, 1, 4, 16, &[0]);
    assert!(decode_ico_bmp_4bpp(&dib4_default_palette, 1, 1, 0).is_ok());

    let dib1 = indexed_dib(1, 1, 1, 2, &[0x80]);
    assert!(decode_ico_bmp_1bpp(&dib1, 1, 1, 2).is_ok());
    let dib1_masked = indexed_dib_with_mask(2, 1, 1, 2, &[0x80], &[0x40]);
    assert!(decode_ico_bmp_1bpp(&dib1_masked, 2, 1, 2).is_ok());
    let dib1_default_palette = indexed_dib(1, 1, 1, 2, &[0x80]);
    assert!(decode_ico_bmp_1bpp(&dib1_default_palette, 1, 1, 0).is_ok());
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
