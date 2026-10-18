//! Classic TIFF IFD inspection without strip or tile decompression.

use super::decode::{Directory, Endian};
use crate::codecs::{CodecError, CodecResult, OptionCodecExt};
use crate::types::{ImageFormat, ImageInfo, ImageMode, ImagePalette, SourceDescriptor};

const PILLOW_DECOMPRESSION_BOMB_ERROR_PIXELS: u64 = 178_956_970;

/// Inspect the first TIFF page and count the complete IFD chain.
pub fn inspect(data: &[u8]) -> CodecResult<ImageInfo> {
    inspect_inner(data, false)
}

/// Inspect only the first TIFF page without traversing the IFD chain.
///
/// `frame_count_complete` is `true` only when the first directory has no next
/// offset, so multipage files report an incomplete count.
pub fn inspect_basic(data: &[u8]) -> CodecResult<ImageInfo> {
    inspect_inner(data, true)
}

fn inspect_inner(data: &[u8], basic: bool) -> CodecResult<ImageInfo> {
    let (endian, first_offset) = super::decode::parse_header(data)?;
    let directory = Directory::parse(data, first_offset, endian)?;
    super::decode::validate_fill_order(&directory)?;

    let (width, height) = validate_primary_dimensions(&directory)?;
    verify_directory(&directory)?;
    let samples = directory.one_or(277, 1);
    verify_separate_planar_offset_count(&directory, width, height, samples)?;
    let bits = directory.values_or(258, &[1]);
    if bits.is_empty() || bits.iter().any(|&value| value != bits[0]) {
        return Err(CodecError::Malformed(
            "invalid TIFF bits-per-sample values".to_owned(),
        ));
    }
    let bit_depth = bits[0];
    let compression = directory.one_or(259, 1);
    let photometric = directory.one_or(262, 1);
    let sample_format = directory.one_or(339, 1);
    let color_map = directory.values(320);
    let extra_samples = directory.values(338);
    let ycbcr_parameters = if photometric == 6 && compression != 1 {
        Some(ycbcr_parameters(&directory)?)
    } else {
        None
    };
    let (layout, palette) = layout_and_palette(TiffLayoutParameters {
        photometric,
        samples,
        bits: bit_depth,
        sample_format,
        compression,
        color_map: color_map.as_deref(),
        extra_samples: extra_samples.as_deref().unwrap_or_default(),
        planar: directory.one_or(284, 1),
        ycbcr_subsampling: ycbcr_parameters.map(|parameters| parameters.subsampling),
        ycbcr_conversion: ycbcr_parameters.map(|parameters| parameters.conversion),
    })?;
    verify_ignored_raw_band_count(&directory, width, height, layout)?;
    let (frame_count, complete_chain) = if basic {
        (1u32, directory.next_offset() == 0)
    } else {
        count_directories(data, first_offset, endian)
    };
    let alpha = extra_samples
        .as_deref()
        .and_then(super::decode::source_alpha_from_extra_samples);
    let mut source = SourceDescriptor::new().with_byte_order(endian.source_byte_order());
    if let Some(alpha) = alpha {
        source = source.with_alpha(alpha);
    }

    Ok(ImageInfo {
        format: ImageFormat::Tiff,
        width,
        height,
        mode: layout.mode(),
        bit_depth: bit_depth.to_le_bytes()[0],
        palette,
        is_animated: frame_count > 1,
        frame_count: complete_chain.then_some(frame_count),
        frame_count_complete: complete_chain,
        cursor_hotspot: None,
        source,
        source_color: crate::types::SourceColor::new(),
    })
}

fn verify_separate_planar_offset_count(
    directory: &Directory<'_>,
    width: u32,
    height: u32,
    samples: usize,
) -> CodecResult<()> {
    if directory.one_or(284, 1) != 2 || samples <= 1 {
        return Ok(());
    }
    verify_planar_offset_count(directory, width, height, samples)
}

fn verify_planar_offset_count(
    directory: &Directory<'_>,
    width: u32,
    height: u32,
    samples: usize,
) -> CodecResult<()> {
    // `verify_directory` has already rejected unmatched tile-offset/count tags.
    if directory.field_type(324).is_some() {
        let tile_width = directory
            .one(322)
            .dimensions("TIFF tile width is missing")?;
        let tile_height = directory
            .one(323)
            .dimensions("TIFF tile height is missing")?;
        if tile_width == 0 || tile_height == 0 {
            return Ok(());
        }
        let tiles_across = (width as usize).div_ceil(tile_width);
        let tiles_down = (height as usize).div_ceil(tile_height);
        let expected_tiles = tiles_across
            .checked_mul(tiles_down)
            .dimensions("TIFF separate-planar tile count overflows")?;
        let expected_offsets = expected_tiles
            .checked_mul(samples)
            .dimensions("TIFF separate-planar tile offset count overflows")?;
        let tile_offset_count = directory
            .value_count(324)
            .malformed("TIFF tile offsets are missing")?;
        if tile_offset_count > expected_offsets {
            return Err(CodecError::Malformed(
                "TIFF separate-planar tile offset count exceeds its geometry".to_owned(),
            ));
        }
        return Ok(());
    }

    let rows_per_strip = directory.one_or(278, height as usize);
    if rows_per_strip == 0 {
        return Ok(());
    }

    let strip_offset_count = directory
        .value_count(273)
        .malformed("TIFF strip offsets are missing")?;
    let strips_per_plane = (height as usize).div_ceil(rows_per_strip);
    let expected_strips = strips_per_plane
        .checked_mul(samples)
        .dimensions("TIFF separate-planar strip count overflows")?;
    if strip_offset_count > expected_strips {
        return Err(CodecError::Malformed(
            "TIFF separate-planar strip count exceeds its geometry".to_owned(),
        ));
    }

    Ok(())
}

/// Pillow removes unspecified separate bands from its raw-mode string, but
/// still walks every declared offset. An offset for an ignored band therefore
/// makes raw image opening fail; compressed loading ignores that band instead.
pub(super) fn verify_ignored_raw_band_count(
    directory: &Directory<'_>,
    width: u32,
    height: u32,
    layout: TiffLayout,
) -> CodecResult<()> {
    if directory.one_or(259, 1) == 1 && directory.one_or(284, 1) == 2 {
        let retained_samples = match layout {
            TiffLayout::GrayIgnored8 => 1,
            TiffLayout::RgbIgnored8 => 3,
            _ => return Ok(()),
        };
        verify_planar_offset_count(directory, width, height, retained_samples)?;
    }
    Ok(())
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
            && directory.value_count(tag).is_some_and(|count| count == 0)
        {
            return Err(CodecError::Malformed(
                "TIFF strip or tile offsets are empty".to_owned(),
            ));
        }
    }
    let tile_offsets_present = directory.field_type(324).is_some();
    let tile_byte_counts_present = directory.field_type(325).is_some();
    if tile_offsets_present != tile_byte_counts_present {
        return Err(CodecError::Malformed(
            "TIFF tile offsets and byte counts must be present together".to_owned(),
        ));
    }
    let tiled = tile_offsets_present || tile_byte_counts_present;
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum TiffLayout {
    Bilevel {
        invert: bool,
    },
    Gray2 {
        invert: bool,
    },
    Gray4 {
        invert: bool,
    },
    Gray8 {
        invert: bool,
    },
    Gray16,
    GrayAlpha8,
    GrayIgnored8,
    I32,
    F32,
    Rgb8,
    RgbIgnored8,
    Rgba8,
    Palette {
        bits: u8,
    },
    Cmyk8,
    Ycbcr8,
    YcbcrFull8 {
        conversion: YcbcrConversion,
    },
    YcbcrSubsampled8 {
        conversion: YcbcrConversion,
        subsampling: (usize, usize),
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct YcbcrConversion {
    pub(super) coefficients: [(i64, i64); 3],
    pub(super) reference_black_white: [(i64, i64); 6],
    pub(super) floating_coefficients: Option<[f64; 3]>,
    pub(super) floating_reference_black_white: Option<[f64; 6]>,
    pub(super) invalid_coefficients: bool,
    pub(super) is_default: bool,
}

#[derive(Debug, Clone, Copy)]
struct YcbcrNumericValues<const N: usize> {
    rational: [(i64, i64); N],
    floating: Option<[f64; N]>,
    invalid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct YcbcrParameters {
    pub(super) subsampling: (usize, usize),
    pub(super) conversion: YcbcrConversion,
}

impl TiffLayout {
    pub(super) const fn mode(self) -> ImageMode {
        match self {
            Self::Bilevel { .. } => ImageMode::L1,
            Self::Gray2 { .. } | Self::Gray4 { .. } | Self::Gray8 { .. } | Self::GrayIgnored8 => {
                ImageMode::L8
            }
            Self::Gray16 => ImageMode::L16,
            Self::GrayAlpha8 => ImageMode::La8,
            Self::I32 => ImageMode::I32,
            Self::F32 => ImageMode::F32,
            Self::Rgb8
            | Self::RgbIgnored8
            | Self::Ycbcr8
            | Self::YcbcrFull8 { .. }
            | Self::YcbcrSubsampled8 { .. } => ImageMode::Rgb8,
            Self::Rgba8 => ImageMode::Rgba8,
            Self::Palette { .. } => ImageMode::P8,
            Self::Cmyk8 => ImageMode::Cmyk8,
        }
    }
}

pub(super) struct TiffLayoutParameters<'a> {
    pub(super) photometric: usize,
    pub(super) samples: usize,
    pub(super) bits: usize,
    pub(super) sample_format: usize,
    pub(super) compression: usize,
    pub(super) color_map: Option<&'a [usize]>,
    pub(super) extra_samples: &'a [usize],
    pub(super) planar: usize,
    pub(super) ycbcr_subsampling: Option<(usize, usize)>,
    pub(super) ycbcr_conversion: Option<YcbcrConversion>,
}

pub(super) fn layout_and_palette(
    parameters: TiffLayoutParameters<'_>,
) -> CodecResult<(TiffLayout, Option<ImagePalette>)> {
    let TiffLayoutParameters {
        photometric,
        samples,
        bits,
        sample_format,
        compression,
        color_map,
        extra_samples,
        planar,
        ycbcr_subsampling,
        ycbcr_conversion,
    } = parameters;
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
        (1, 2, 8) if extra_samples == [2] => TiffLayout::GrayAlpha8,
        (1, 2, 8) if planar == 2 && extra_samples == [0] => TiffLayout::GrayIgnored8,
        (2, 3, 8) => TiffLayout::Rgb8,
        (2, 4, 8) if extra_samples == [0] => TiffLayout::RgbIgnored8,
        (2, 4, 8) => TiffLayout::Rgba8,
        (3, 1, bits @ (1 | 2 | 4 | 8)) => TiffLayout::Palette {
            bits: bits.to_le_bytes()[0],
        },
        (5, 4, 8) => TiffLayout::Cmyk8,
        (6, 3, 8) if compression == 1 => TiffLayout::Ycbcr8,
        (6, 3, 8) => {
            let conversion = ycbcr_conversion.malformed("TIFF YCbCr conversion is missing")?;
            let subsampling = ycbcr_subsampling.unwrap_or((2, 2));
            if subsampling == (1, 1) {
                TiffLayout::YcbcrFull8 { conversion }
            } else {
                TiffLayout::YcbcrSubsampled8 {
                    conversion,
                    subsampling,
                }
            }
        }
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

/// Read the YCbCr subsampling and positioning tags used by Pillow's TIFF path.
///
/// Compressed TIFF YCbCr data is converted using default 1-by-1 triples or
/// default 2-by-2 clumps. Uncompressed files retain Pillow's legacy RGBX raw
/// interpretation. Libtiff's RGB conversion ignores the positioning tag and
/// uses one chroma pair for each clump.
pub(super) fn ycbcr_parameters(directory: &Directory<'_>) -> CodecResult<YcbcrParameters> {
    const DEFAULT_COEFFICIENTS: [(i64, i64); 3] = [(299, 1000), (587, 1000), (114, 1000)];
    const DEFAULT_REFERENCE_BLACK_WHITE: [(i64, i64); 6] =
        [(0, 1), (255, 1), (128, 1), (255, 1), (128, 1), (255, 1)];
    let coefficients = read_ycbcr_rationals(directory, 529, DEFAULT_COEFFICIENTS)?;
    let reference_black_white =
        read_ycbcr_rationals(directory, 532, DEFAULT_REFERENCE_BLACK_WHITE)?;

    let subsampling = match directory.field_type(530) {
        Some(1 | 3 | 4 | 6 | 8 | 9) if directory.value_count(530) == Some(2) => {
            let values = directory.values(530).ok_or_else(|| {
                CodecError::Malformed("TIFF YCbCr subsampling is malformed".to_owned())
            })?;
            match values.as_slice() {
                // Pillow ignores negative signed tag values and uses its 2-by-2
                // default. Signed directory accessors represent those values
                // with usize::MAX so they cannot enter decode geometry.
                [horizontal, vertical] if *horizontal != usize::MAX && *vertical != usize::MAX => {
                    (*horizontal, *vertical)
                }
                _ => (2, 2),
            }
        }
        _ => (2, 2),
    };

    let is_default = coefficients.floating.is_none()
        && reference_black_white.floating.is_none()
        && rational_values_match_default(&coefficients.rational, &DEFAULT_COEFFICIENTS)
        && rational_values_match_default(
            &reference_black_white.rational,
            &DEFAULT_REFERENCE_BLACK_WHITE,
        );
    Ok(YcbcrParameters {
        subsampling,
        conversion: YcbcrConversion {
            coefficients: coefficients.rational,
            reference_black_white: reference_black_white.rational,
            floating_coefficients: coefficients.floating,
            floating_reference_black_white: reference_black_white.floating,
            invalid_coefficients: coefficients.invalid,
            is_default,
        },
    })
}

fn read_ycbcr_rationals<const N: usize>(
    directory: &Directory<'_>,
    tag: u16,
    defaults: [(i64, i64); N],
) -> CodecResult<YcbcrNumericValues<N>> {
    let Some(field_type) = directory.field_type(tag) else {
        return Ok(YcbcrNumericValues {
            rational: defaults,
            floating: None,
            invalid: false,
        });
    };
    if directory.field_count(tag) != Some(N) {
        return Ok(YcbcrNumericValues {
            rational: defaults,
            floating: None,
            invalid: false,
        });
    }
    if directory.value_count(tag) != Some(N) {
        return Ok(YcbcrNumericValues {
            rational: defaults,
            floating: None,
            invalid: false,
        });
    }
    if matches!(field_type, 11 | 12) {
        let mut values = [0.0; N];
        for (index, value) in values.iter_mut().enumerate() {
            *value = directory.floating_value(tag, index).ok_or_else(|| {
                CodecError::Malformed(format!("TIFF YCbCr tag {tag} is truncated"))
            })?;
        }
        return Ok(YcbcrNumericValues {
            rational: defaults,
            floating: Some(values),
            invalid: false,
        });
    }
    let mut values = defaults;
    match field_type {
        5 | 10 => {
            for (index, value) in values.iter_mut().enumerate() {
                let (numerator, denominator) = directory.rational(tag, index).ok_or_else(|| {
                    CodecError::Malformed(format!("TIFF YCbCr tag {tag} is truncated"))
                })?;
                *value = (numerator, denominator);
            }
        }
        3 | 4 => {
            if tag == 529 && field_type == 3 {
                return Ok(YcbcrNumericValues {
                    rational: defaults,
                    floating: None,
                    invalid: true,
                });
            }
            let numeric_values = directory.values(tag).ok_or_else(|| {
                CodecError::Malformed(format!("TIFF YCbCr tag {tag} is truncated"))
            })?;
            for (value, numeric_value) in values.iter_mut().zip(numeric_values) {
                let numerator = i64::try_from(numeric_value).map_err(|_| {
                    CodecError::Malformed(format!("TIFF YCbCr tag {tag} value exceeds i64"))
                })?;
                *value = (numerator, 1);
            }
        }
        _ => {
            return Ok(YcbcrNumericValues {
                rational: defaults,
                floating: None,
                invalid: tag == 529,
            });
        }
    }
    Ok(YcbcrNumericValues {
        rational: values,
        floating: None,
        invalid: false,
    })
}

fn rational_values_match_default<const N: usize>(
    actual: &[(i64, i64); N],
    expected: &[(i64, i64); N],
) -> bool {
    actual.iter().zip(expected).all(
        |(&(numerator, denominator), &(expected_numerator, expected_denominator))| {
            let actual_is_zero = numerator == 0 || denominator == 0;
            let expected_is_zero = expected_numerator == 0 || expected_denominator == 0;
            if actual_is_zero || expected_is_zero {
                actual_is_zero && expected_is_zero
            } else {
                i128::from(numerator).checked_mul(i128::from(expected_denominator))
                    == i128::from(expected_numerator).checked_mul(i128::from(denominator))
            }
        },
    )
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
