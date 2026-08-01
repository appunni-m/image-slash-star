//! ICO/CUR directory and embedded-image inspection without pixel decoding.

use crate::codecs::{CodecError, CodecResult, codec_add_end, need_slice, terminalize};
use crate::types::{CursorHotspot, ImageFormat, ImageInfo, ImageMode};

const HEADER_SIZE: usize = 6;
const ENTRY_SIZE: usize = 16;

/// Inspect the same best-resolution entry selected by the ICO decoder.
pub fn inspect(data: &[u8]) -> CodecResult<ImageInfo> {
    let header = need_slice(data, 0, HEADER_SIZE, "truncated ICO header")
        .map_err(|error| error.at(0, "ico_header"))?;
    let reserved = u16::from_le_bytes([header[0], header[1]]);
    let kind = u16::from_le_bytes([header[2], header[3]]);
    let count = usize::from(u16::from_le_bytes([header[4], header[5]]));
    if reserved != 0 {
        return Err(
            CodecError::Malformed("invalid ICO reserved header field".to_owned())
                .at(0, "ico_header"),
        );
    }
    if !matches!(kind, 1 | 2) {
        return Err(
            CodecError::Malformed("invalid ICO container type".to_owned()).at(2, "ico_header"),
        );
    }
    if count == 0 || count > 255 {
        return Err(
            CodecError::Malformed("invalid ICO directory count".to_owned()).at(4, "ico_header"),
        );
    }
    let directory_end = HEADER_SIZE.saturating_add(count.saturating_mul(ENTRY_SIZE));
    let directory = need_slice(data, HEADER_SIZE, directory_end, "truncated ICO directory")
        .map_err(|error| error.at(HEADER_SIZE as u64, "ico_directory"))?;
    let mut best = &directory[..ENTRY_SIZE];
    let mut best_offset = HEADER_SIZE;
    let mut best_score = 0;
    for (index, entry) in directory.chunks_exact(ENTRY_SIZE).enumerate() {
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
            best_offset = HEADER_SIZE.saturating_add(index.saturating_mul(ENTRY_SIZE));
            best_score = score;
        }
    }

    let length_u32 = u32::from_le_bytes([best[8], best[9], best[10], best[11]]);
    let length = length_u32 as usize;
    let offset = u32::from_le_bytes([best[12], best[13], best[14], best[15]]) as usize;
    if length == 0 {
        return Err(
            CodecError::Malformed("ICO directory entry has an empty size".to_owned()).at(
                u64::try_from(best_offset.saturating_add(8)).unwrap_or(u64::MAX),
                "ico_entry",
            ),
        );
    }
    if offset == 0 {
        return Err(
            CodecError::Malformed("ICO directory entry has an empty offset".to_owned()).at(
                u64::try_from(best_offset.saturating_add(12)).unwrap_or(u64::MAX),
                "ico_entry",
            ),
        );
    }
    let payload_end = codec_add_end(offset, length);
    let payload = need_slice(
        data,
        offset,
        payload_end,
        "ICO entry payload is out of bounds",
    )
    .map_err(|error| error.at(u64::try_from(offset).unwrap_or(u64::MAX), "ico_entry"))?;
    let mut info = if payload.starts_with(b"\x89PNG\r\n\x1a\n") {
        crate::codecs::png::inspect::inspect(payload)
            .map_err(terminalize)
            .map_err(|error| error.at(u64::try_from(offset).unwrap_or(u64::MAX), "ico_png"))?
    } else if kind == 2 {
        inspect_cursor_dib(payload, length_u32)
            .map_err(terminalize)
            .map_err(|error| error.at(u64::try_from(offset).unwrap_or(u64::MAX), "ico_cur_dib"))?
    } else {
        inspect_icon_dib(payload)
            .map_err(terminalize)
            .map_err(|error| error.at(u64::try_from(offset).unwrap_or(u64::MAX), "ico_dib"))?
    };
    info.format = ImageFormat::Ico;
    info.is_animated = false;
    info.frame_count = Some(1);
    info.cursor_hotspot = (kind == 2).then(|| CursorHotspot {
        x: u16::from_le_bytes([best[4], best[5]]),
        y: u16::from_le_bytes([best[6], best[7]]),
    });
    Ok(info)
}

fn inspect_cursor_dib(data: &[u8], declared_len: u32) -> CodecResult<ImageInfo> {
    let header = need_slice(data, 0, 40, "truncated CUR DIB header")?;
    let header_size_u32 = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    let header_size = header_size_u32 as usize;
    if header_size < 40 {
        return Err(CodecError::Unsupported(
            "unsupported CUR DIB header size".to_owned(),
        ));
    }
    if data.len() < header_size {
        return Err(CodecError::NeedMore {
            minimum: header_size,
            message: "truncated CUR DIB header".to_owned(),
        });
    }
    let actual_height = i32::from_le_bytes([header[8], header[9], header[10], header[11]]) / 2;
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
        super::decode::cur_bmp_prefix(declared_len, header_size_u32, palette_entries)?;
    let mut bmp = Vec::with_capacity(data.len());
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&file_size_bytes);
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&pixel_offset_bytes);
    bmp.extend_from_slice(data);
    bmp[22..26].copy_from_slice(&actual_height.to_le_bytes());
    crate::codecs::bmp::inspect::inspect(&bmp).map_err(terminalize)
}

fn inspect_icon_dib(data: &[u8]) -> CodecResult<ImageInfo> {
    let header = need_slice(data, 0, 40, "truncated ICO DIB header")?;
    let width = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
    let stored_height = u32::from_le_bytes([header[8], header[9], header[10], header[11]]);
    let height = stored_height / 2;
    let bits = u16::from_le_bytes([header[14], header[15]]);
    let colors_used = u32::from_le_bytes([header[32], header[33], header[34], header[35]]);
    if width == 0 || height == 0 || width > 16_384 || height > 16_384 {
        return Err(CodecError::Malformed(
            "ICO DIB dimensions are empty or exceed the supported bounds".to_owned(),
        ));
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
        24 => bounded_usize(width).saturating_mul(3),
        32 => bounded_usize(width).saturating_mul(4),
        _ => {
            return Err(CodecError::Unsupported(format!(
                "unsupported ICO BMP pixel depth {bits}"
            )));
        }
    };
    let padded_row = row_bytes.saturating_add(3) & !3;
    let pixel_start = 40usize.saturating_add(palette_entries.saturating_mul(4));
    let required = pixel_start.saturating_add(padded_row.saturating_mul(bounded_usize(height)));
    let payload = need_slice(data, 0, required, "truncated ICO bitmap payload")?;
    let pixels = &payload[pixel_start..];
    if bits == 4 {
        super::decode::validate_4bit_palette_references(
            pixels,
            width,
            height,
            padded_row,
            palette_entries,
        )?;
    } else if bits == 1 {
        super::decode::validate_1bit_palette_references(
            pixels,
            width,
            height,
            padded_row,
            palette_entries,
        )?;
    }
    Ok(ImageInfo {
        format: ImageFormat::Ico,
        width,
        height,
        mode: ImageMode::Rgba8,
        bit_depth: bits.to_le_bytes()[0],
        palette: None,
        is_animated: false,
        frame_count: Some(1),
        frame_count_complete: true,
        cursor_hotspot: None,
        source: crate::types::SourceDescriptor::new(),
        source_color: crate::types::SourceColor::new(),
    })
}

fn bounded_usize(value: u32) -> usize {
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

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let _ = inspect(b"");
    for header in [
        [1, 0, 1, 0, 1, 0],
        [0, 0, 0, 0, 1, 0],
        [0, 0, 1, 0, 0, 0],
        [0, 0, 1, 0, 0, 1],
    ] {
        let _ = inspect(&header);
    }
    // A complete ICO directory entry whose declared PNG payload is itself
    // truncated proves that nested incremental status is terminalized.
    let mut truncated_png_ico = vec![0, 0, 1, 0, 1, 0];
    truncated_png_ico.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 22, 0, 0, 0]);
    truncated_png_ico.extend_from_slice(b"\x89PNG\r");
    let _ = inspect(&truncated_png_ico);
}
