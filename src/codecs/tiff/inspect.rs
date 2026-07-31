//! Classic TIFF IFD inspection without strip or tile decompression.

use super::decode::{Directory, Endian};
use crate::codecs::{CodecError, CodecResult, OptionCodecExt};
use crate::types::{ImageFormat, ImageInfo, ImageMode, ImagePalette};

const PILLOW_DECOMPRESSION_BOMB_ERROR_PIXELS: u64 = 178_956_970;

/// Inspect the first TIFF page and count the complete IFD chain.
pub fn inspect(data: &[u8]) -> CodecResult<ImageInfo> {
    let (endian, first_offset) = super::decode::parse_header(data)?;
    let directory = Directory::parse(data, first_offset, endian)?;

    let (width, height) = validate_primary_dimensions(&directory)?;
    verify_directory(&directory)?;
    let samples = directory.one_or(277, 1);
    let bits = directory.values_or(258, &[1]);
    if bits.is_empty() || bits.iter().any(|&value| value != bits[0]) {
        return Err(CodecError::Malformed(
            "invalid TIFF bits-per-sample values".to_owned(),
        ));
    }
    let bit_depth = bits[0];
    let photometric = directory.one_or(262, 1);
    let sample_format = directory.one_or(339, 1);
    let color_map = directory.values(320);
    let (layout, palette) = layout_and_palette(
        photometric,
        samples,
        bit_depth,
        sample_format,
        color_map.as_deref(),
    )?;
    let (frame_count, complete_chain) = count_directories(data, first_offset, endian);

    Ok(ImageInfo {
        format: ImageFormat::Tiff,
        width,
        height,
        mode: layout.mode(),
        bit_depth: bit_depth.to_le_bytes()[0],
        palette,
        is_animated: frame_count > 1,
        frame_count: complete_chain.then_some(frame_count),
        cursor_hotspot: None,
    })
}

pub(super) fn verify_directory(directory: &Directory<'_>) -> CodecResult<()> {
    let compression = directory.one_or(259, 1);
    if !matches!(compression, 1 | 5 | 8 | 32_773 | 32_946) {
        return Err(CodecError::Malformed(
            "unsupported TIFF compression tag".to_owned(),
        ));
    }
    for tag in [273, 324] {
        if matches!(directory.field_type(tag), Some(1 | 3 | 4))
            && directory
                .values(tag)
                .is_some_and(|values| values.is_empty())
        {
            return Err(CodecError::Malformed(
                "TIFF strip or tile offsets are empty".to_owned(),
            ));
        }
    }
    let tiled = directory.field_type(324).is_some() || directory.field_type(325).is_some();
    if tiled {
        for tag in [322, 323] {
            if directory.one(tag).is_none() {
                return Err(CodecError::Dimensions(
                    "TIFF tile dimensions are missing".to_owned(),
                ));
            }
        }
    } else if directory.field_type(273).is_none() {
        return Err(CodecError::Malformed(
            "TIFF strip offsets are missing".to_owned(),
        ));
    }
    Ok(())
}

pub(super) fn validate_primary_dimensions(directory: &Directory<'_>) -> CodecResult<(u32, u32)> {
    let width = required_dimension(directory, 256, "width")?;
    let height = required_dimension(directory, 257, "height")?;
    if width == 0 || height == 0 {
        return Err(CodecError::Malformed(
            "TIFF dimensions must be nonzero".to_owned(),
        ));
    }
    if u64::from(width).saturating_mul(u64::from(height)) > PILLOW_DECOMPRESSION_BOMB_ERROR_PIXELS {
        return Err(CodecError::Dimensions(
            "TIFF dimensions exceed Pillow's decompression-bomb limit".to_owned(),
        ));
    }
    Ok((width, height))
}

fn required_dimension(directory: &Directory<'_>, tag: u16, name: &'static str) -> CodecResult<u32> {
    directory
        .one(tag)
        .map(bounded_u32)
        .ok_or_else(|| CodecError::Malformed(format!("TIFF {name} is missing")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TiffLayout {
    Bilevel { invert: bool },
    Gray2 { invert: bool },
    Gray4 { invert: bool },
    Gray8 { invert: bool },
    Gray16,
    GrayAlpha8,
    I32,
    F32,
    Rgb8,
    Rgba8,
    Palette { bits: u8 },
    Cmyk8,
    Ycbcr8,
}

impl TiffLayout {
    pub(super) const fn mode(self) -> ImageMode {
        match self {
            Self::Bilevel { .. } => ImageMode::L1,
            Self::Gray2 { .. } | Self::Gray4 { .. } | Self::Gray8 { .. } => ImageMode::L8,
            Self::Gray16 => ImageMode::L16,
            Self::GrayAlpha8 => ImageMode::La8,
            Self::I32 => ImageMode::I32,
            Self::F32 => ImageMode::F32,
            Self::Rgb8 | Self::Ycbcr8 => ImageMode::Rgb8,
            Self::Rgba8 => ImageMode::Rgba8,
            Self::Palette { .. } => ImageMode::P8,
            Self::Cmyk8 => ImageMode::Cmyk8,
        }
    }
}

pub(super) fn layout_and_palette(
    photometric: usize,
    samples: usize,
    bits: usize,
    sample_format: usize,
    color_map: Option<&[usize]>,
) -> CodecResult<(TiffLayout, Option<ImagePalette>)> {
    let invert = photometric == 0;
    let layout = match (photometric, samples, bits) {
        (0 | 1, 1, 1) => TiffLayout::Bilevel { invert },
        (0 | 1, 1, 2) => TiffLayout::Gray2 { invert },
        (0 | 1, 1, 4) => TiffLayout::Gray4 { invert },
        (0 | 1, 1, 8) => TiffLayout::Gray8 { invert },
        (0 | 1, 1, 16) => TiffLayout::Gray16,
        (0 | 1, 1, 32) => match sample_format {
            1 | 2 => TiffLayout::I32,
            3 => TiffLayout::F32,
            _ => {
                return Err(CodecError::Malformed(
                    "unsupported TIFF sample format".to_owned(),
                ));
            }
        },
        (1, 2, 8) => TiffLayout::GrayAlpha8,
        (2, 3, 8) => TiffLayout::Rgb8,
        (2, 4, 8) => TiffLayout::Rgba8,
        (3, 1, bits @ (1 | 2 | 4 | 8)) => TiffLayout::Palette {
            bits: bits.to_le_bytes()[0],
        },
        (5, 4, 8) => TiffLayout::Cmyk8,
        (6, 3, 8) => TiffLayout::Ycbcr8,
        _ => {
            return Err(CodecError::Malformed(
                "unsupported TIFF photometric/sample layout".to_owned(),
            ));
        }
    };
    let palette = if let TiffLayout::Palette { bits } = layout {
        let entries = 1usize << usize::from(bits);
        let map = color_map
            .malformed("indexed TIFF contains no color map")?
            .get(..entries.wrapping_mul(3));
        if let Some(map) = map {
            let mut rgb = Vec::with_capacity(entries.wrapping_mul(3));
            for index in 0..entries {
                rgb.push(map[index].wrapping_shr(8).to_le_bytes()[0]);
                rgb.push(
                    map[entries.saturating_add(index)]
                        .wrapping_shr(8)
                        .to_le_bytes()[0],
                );
                rgb.push(
                    map[entries.saturating_mul(2).saturating_add(index)]
                        .wrapping_shr(8)
                        .to_le_bytes()[0],
                );
            }
            Some(ImagePalette {
                rgb,
                alpha: Vec::new(),
            })
        } else {
            None
        }
    } else {
        None
    };
    Ok((layout, palette))
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
        let directory = match Directory::parse(data, offset, endian) {
            Ok(directory) => directory,
            Err(_) => {
                complete = false;
                break;
            }
        };
        offset = directory.next_offset();
    }
    (bounded_u32(seen.len()), complete)
}

fn bounded_u32(value: usize) -> u32 {
    let bytes = value.to_le_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}
