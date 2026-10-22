//! BMP header and palette inspection without pixel decoding.

use crate::codecs::{CodecError, CodecResult, codec_add_end, need_from, need_slice};
use crate::types::{ImageFormat, ImageInfo, ImageMode, ImagePalette};

const FILE_HEADER_SIZE: usize = 14;
const CORE_HEADER_SIZE: u32 = 12;
const INFO_HEADER_SIZE: u32 = 40;
const BI_BITFIELDS: u32 = 3;

/// Inspect BMP dimensions, encoded depth, output mode, and indexed palette.
pub fn inspect(data: &[u8]) -> CodecResult<ImageInfo> {
    if need_slice(data, 0, 2, "truncated BMP file signature")
        .map_err(|error| error.at(0, "bmp_signature"))?
        != b"BM"
    {
        return Err(
            CodecError::Malformed("invalid BMP file signature".to_owned()).at(0, "bmp_signature"),
        );
    }

    let _data_offset = le_u32(data, 10)?;
    let header_size = le_u32(data, FILE_HEADER_SIZE)?;
    let header_end = FILE_HEADER_SIZE.saturating_add(header_size as usize);
    let (width, height, bit_depth, compression, colors_used, palette_entry_size, negative_width) =
        if header_size == CORE_HEADER_SIZE {
            let width = u32::from(le_u16(data, 18)?);
            let height = u32::from(le_u16(data, 20)?);
            // CORE headers have no compression selector that can outrank dimensions.
            if width == 0 || height == 0 {
                return Err(
                    CodecError::Malformed("BMP dimensions must be nonzero".to_owned())
                        .at(18, "bmp_dib_header"),
                );
            }
            (width, height, le_u16(data, 24)?, 0, 0, 3usize, false)
        } else if super::is_supported_info_dib_header_size(header_size) {
            let width = le_i32(data, 18)?;
            (
                width.cast_unsigned(),
                le_i32(data, 22)?.unsigned_abs(),
                le_u16(data, 28)?,
                le_u32(data, 30)?,
                le_u32(data, 46)?,
                4usize,
                width < 0,
            )
        } else {
            return Err(CodecError::Unsupported(format!(
                "unsupported BMP DIB header size {header_size}"
            ))
            .at(FILE_HEADER_SIZE as u64, "bmp_dib_header"));
        };

    let indexed = matches!(bit_depth, 1 | 4 | 8);
    if !matches!(bit_depth, 1 | 4 | 8 | 16 | 24 | 32) {
        let bit_depth_offset = if header_size == CORE_HEADER_SIZE {
            24
        } else {
            28
        };
        return Err(
            CodecError::Unsupported(format!("unsupported BMP pixel depth {bit_depth}"))
                .at(bit_depth_offset, "bmp_dib_header"),
        );
    }
    super::validate_compression(data, header_size, bit_depth, compression)?;
    // Pillow reads and validates BI_BITFIELDS masks before checking INFO
    // dimensions. Inspect the masks here, before returning dimension errors.
    let masks = bitfield_masks(data, header_size, bit_depth, compression)?;
    // INFO headers check zero dimensions after the depth and compression contract.
    if negative_width {
        return Err(
            CodecError::Dimensions("BMP width must be positive".to_owned())
                .at(18, "bmp_dib_header"),
        );
    }
    if width == 0 || height == 0 {
        return Err(
            CodecError::Malformed("BMP dimensions must be nonzero".to_owned())
                .at(18, "bmp_dib_header"),
        );
    }

    let alpha_mask = masks.map_or(0, |masks| masks.alpha);
    let palette_start = if header_size == INFO_HEADER_SIZE && compression == BI_BITFIELDS {
        header_end.saturating_add(12)
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
                index.to_le_bytes()[0]
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

    Ok(ImageInfo {
        format: ImageFormat::Bmp,
        width,
        height,
        mode,
        bit_depth: bit_depth.to_le_bytes()[0],
        palette,
        is_animated: false,
        frame_count: Some(1),
        frame_count_complete: true,
        cursor_hotspot: None,
        source: crate::types::SourceDescriptor::new(),
        source_color: crate::types::SourceColor::new(),
    })
}

fn bitfield_masks(
    data: &[u8],
    header_size: u32,
    bit_depth: u16,
    compression: u32,
) -> CodecResult<Option<super::BitfieldMasks>> {
    if compression != BI_BITFIELDS {
        return Ok(None);
    }
    let masks = super::BitfieldMasks {
        red: le_u32(data, 54)?,
        green: le_u32(data, 58)?,
        blue: le_u32(data, 62)?,
        alpha: if bit_depth == 32 && header_size >= 56 {
            le_u32(data, 66)?
        } else {
            0
        },
    };
    super::normalize_bitfield_masks(bit_depth, masks).map(Some)
}

fn read_palette(
    data: &[u8],
    start: usize,
    count: usize,
    entry_size: usize,
) -> CodecResult<Vec<[u8; 4]>> {
    let available = need_from(data, start, "BMP palette begins beyond the input")
        .map_err(|error| error.at(u64::try_from(start).unwrap_or(u64::MAX), "bmp_palette"))?;
    let count = count.min(available.len().div_euclid(entry_size));
    let byte_len = count.saturating_mul(entry_size);
    let bytes = &available[..byte_len];
    let mut palette = Vec::with_capacity(count);
    for entry in bytes.chunks_exact(entry_size) {
        let mut color = [0; 4];
        color[..entry_size].copy_from_slice(entry);
        palette.push(color);
    }
    Ok(palette)
}

fn le_u16(data: &[u8], offset: usize) -> CodecResult<u16> {
    let bytes = need_slice(
        data,
        offset,
        codec_add_end(offset, 2),
        "truncated BMP 16-bit field",
    )
    .map_err(|error| error.at(u64::try_from(offset).unwrap_or(u64::MAX), "bmp_field"))?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn le_u32(data: &[u8], offset: usize) -> CodecResult<u32> {
    let bytes = need_slice(
        data,
        offset,
        codec_add_end(offset, 4),
        "truncated BMP 32-bit field",
    )
    .map_err(|error| error.at(u64::try_from(offset).unwrap_or(u64::MAX), "bmp_field"))?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn le_i32(data: &[u8], offset: usize) -> CodecResult<i32> {
    let bytes = need_slice(
        data,
        offset,
        codec_add_end(offset, 4),
        "truncated BMP signed field",
    )
    .map_err(|error| error.at(u64::try_from(offset).unwrap_or(u64::MAX), "bmp_field"))?;
    Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}
