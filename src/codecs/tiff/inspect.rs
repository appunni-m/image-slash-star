//! Classic TIFF IFD inspection without strip or tile decompression.

use super::decode::{Directory, Endian};
use crate::types::{ImageFormat, ImageInfo, ImageMode, ImagePalette};

/// Inspect the first TIFF page and count the complete IFD chain.
pub fn inspect(data: &[u8]) -> Option<ImageInfo> {
    let endian = match data.get(..2)? {
        b"II" => Endian::Little,
        b"MM" => Endian::Big,
        _ => return None,
    };
    let magic = data.get(2..4)?;
    if endian.u16_exact([magic[0], magic[1]]) != 42 {
        return None;
    }
    let offset = data.get(4..8)?;
    let first_offset = endian.u32_exact([offset[0], offset[1], offset[2], offset[3]]) as usize;
    let directory = Directory::parse(data, first_offset, endian)?;

    let width = directory.one(256)? as u32;
    let height = directory.one(257)? as u32;
    if width == 0 || height == 0 {
        return None;
    }
    let samples = directory.one_or(277, 1);
    let bits = directory.values_or(258, &[1]);
    if bits.is_empty() || bits.iter().any(|&value| value != bits[0]) {
        return None;
    }
    let bit_depth = bits[0];
    let photometric = directory.one_or(262, 1);
    let sample_format = directory.one_or(339, 1);
    let (mode, palette) = mode_and_palette(
        photometric,
        samples,
        bit_depth,
        sample_format,
        directory.values(320).as_deref(),
    )?;
    let (frame_count, complete_chain) = count_directories(data, first_offset, endian);

    Some(ImageInfo {
        format: ImageFormat::Tiff,
        width,
        height,
        mode,
        bit_depth: bit_depth as u8,
        palette,
        is_animated: frame_count > 1,
        frame_count: complete_chain.then_some(frame_count),
    })
}

fn mode_and_palette(
    photometric: usize,
    samples: usize,
    bits: usize,
    sample_format: usize,
    color_map: Option<&[usize]>,
) -> Option<(ImageMode, Option<ImagePalette>)> {
    let mode = match (photometric, samples, bits) {
        (0 | 1, 1, 1) => ImageMode::L1,
        (0 | 1, 1, 2 | 4 | 8) => ImageMode::L8,
        (0 | 1, 1, 16) => ImageMode::L16,
        (0 | 1, 1, 32) => match sample_format {
            1 | 2 => ImageMode::I32,
            3 => ImageMode::F32,
            _ => return None,
        },
        (1, 2, 8) => ImageMode::La8,
        (2, 3, 8) | (6, 3, 8) => ImageMode::Rgb8,
        (2, 4, 8) => ImageMode::Rgba8,
        (3, 1, 1 | 2 | 4 | 8) => ImageMode::P8,
        (5, 4, 8) => ImageMode::Cmyk8,
        _ => return None,
    };
    let palette = if mode == ImageMode::P8 {
        let entries = 1usize << bits;
        let map = color_map?;
        map.get(..entries.wrapping_mul(3)).map(|map| {
            let mut rgb = Vec::with_capacity(entries.wrapping_mul(3));
            for index in 0..entries {
                rgb.push((map[index] >> 8) as u8);
                rgb.push((map[entries + index] >> 8) as u8);
                rgb.push((map[entries * 2 + index] >> 8) as u8);
            }
            ImagePalette {
                rgb,
                alpha: Vec::new(),
            }
        })
    } else {
        None
    };
    Some((mode, palette))
}

fn count_directories(data: &[u8], first_offset: usize, endian: Endian) -> (u32, bool) {
    let mut offset = first_offset;
    let mut seen = Vec::new();
    let mut complete = true;
    while offset != 0 {
        if seen.contains(&offset) {
            break;
        }
        seen.push(offset);
        let Some(directory) = Directory::parse(data, offset, endian) else {
            complete = false;
            break;
        };
        offset = directory.next_offset();
    }
    (seen.len() as u32, complete)
}
