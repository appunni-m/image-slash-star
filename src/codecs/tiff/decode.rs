//! Baseline decoder for classic TIFF IFD payloads.

use super::inspect::{TiffLayout, TiffLayoutParameters};
use crate::SequenceDecodeBudget;
use crate::codecs::compression::deflate::{
    decompress_zlib_prefix, decompress_zlib_prefix_with_status_and_token,
};
use crate::codecs::{CodecError, CodecResult, OptionCodecExt, need_slice};
use crate::types::{
    AnimationLoop, ColorType, DecodedFrame, DecodedImage, DecodedSequence, FrameBlend,
    FrameDisposal, FrameDuration, ImageMode, ImagePalette, SourceAlpha, SourceByteOrder,
    SourceColor, SourceDescriptor,
};

const COMPRESSION_NONE: usize = 1;
const COMPRESSION_LZW: usize = 5;
const COMPRESSION_DEFLATE: usize = 8;
const COMPRESSION_PACKBITS: usize = 32_773;
const COMPRESSION_ADOBE_DEFLATE: usize = 32_946;

/// Decode the first IFD of a classic little- or big-endian TIFF stream.
pub fn decode(
    data: &[u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(DecodedImage, usize)> {
    crate::codecs::error::check_cancelled(token)?;
    let (endian, ifd_offset) = parse_header(data)?;
    decode_ifd(data, ifd_offset, endian, None, token)
        .map(|(image, _, directory_end)| (image, directory_end))
}

/// Decode exactly one page by walking the classic IFD chain to its directory.
///
/// Only the selected IFD's pixels are decompressed, so later pages are not
/// materialized. The returned consumed extent is that page's directory end.
pub(crate) fn decode_page(
    data: &[u8],
    page_index: u32,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(DecodedImage, usize)> {
    crate::codecs::error::check_cancelled(token)?;
    let (endian, first_offset) = parse_header(data)?;
    let mut offset = first_offset;
    let mut seen = Vec::new();
    let mut index = 0u32;
    while offset != 0 && !seen.contains(&offset) {
        crate::codecs::error::check_cancelled(token)?;
        seen.push(offset);
        let (image, next_offset, directory_end) = decode_ifd(data, offset, endian, None, token)?;
        if index == page_index {
            return Ok((image, directory_end));
        }
        index = index.saturating_add(1);
        offset = next_offset;
    }
    Err(CodecError::Parameter(format!(
        "TIFF page index {page_index} is out of range"
    )))
}

/// Decode every unique IFD in the classic TIFF main-directory chain.
pub fn decode_sequence(
    data: &[u8],
    budget: &mut SequenceDecodeBudget,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(DecodedSequence, usize)> {
    crate::codecs::error::check_cancelled(token)?;
    let (endian, first_offset) = parse_header(data)?;
    let mut offset = first_offset;
    let mut seen = Vec::new();
    let mut frames = Vec::new();
    let mut width = 0;
    let mut height = 0;
    let mut consumed = 0;
    while offset != 0 && !seen.contains(&offset) {
        crate::codecs::error::check_cancelled(token)?;
        seen.push(offset);
        let (image, next_offset, directory_end) = decode_ifd(
            data,
            offset,
            endian,
            if frames.is_empty() {
                None
            } else {
                Some(&mut *budget)
            },
            token,
        )?;
        consumed = directory_end;
        width = width.max(image.width);
        height = height.max(image.height);
        frames.push(DecodedFrame::source_rectangle(
            image,
            0,
            0,
            FrameDuration::ZERO,
            FrameDisposal::Unspecified,
            FrameBlend::Unspecified,
            false,
        ));
        offset = next_offset;
    }
    if frames.is_empty() {
        return Err(CodecError::Malformed(
            "TIFF contains no image directory".to_owned(),
        ));
    }
    Ok((
        DecodedSequence {
            width,
            height,
            frames,
            loop_count: AnimationLoop::Unspecified,
            background: None,
            kind: crate::types::SequenceKind::UntimedPages,
            opaque_blocks: Vec::new(),
            metadata: Vec::new(),
            source_color: SourceColor::new(),
        },
        consumed,
    ))
}

/// Measure the encoded metadata extent: the consumed main-chain IFD bytes
/// minus the encoded strip/tile payload bytes declared by each directory.
pub(crate) fn metadata_bytes(data: &[u8]) -> CodecResult<u64> {
    let (endian, first_offset) = parse_header(data)?;
    let mut offset = first_offset;
    let mut seen = Vec::new();
    let mut pixel = 0u64;
    let mut consumed = 0usize;
    let mut payload_end = 0usize;
    while offset != 0 && !seen.contains(&offset) {
        seen.push(offset);
        let directory = Directory::parse(data, offset, endian)
            .map_err(|error| error.at(offset as u64, "tiff_ifd"))?;
        #[allow(
            clippy::arithmetic_side_effects,
            reason = "Saturating sums deliberately bound the scanned IFD and payload extents."
        )]
        let directory_end = offset
            .saturating_add(2)
            .saturating_add(directory.entries.len().saturating_mul(12))
            .saturating_add(4);
        consumed = directory_end;
        for (offsets_tag, byte_counts_tag) in [(273u16, 279u16), (324u16, 325u16)] {
            if let (Some(offsets), Some(byte_counts)) = (
                directory.values(offsets_tag),
                directory.values(byte_counts_tag),
            ) && offsets.len() == byte_counts.len()
            {
                for (&byte_offset, &byte_count) in offsets.iter().zip(&byte_counts) {
                    pixel = pixel.saturating_add(byte_count as u64);
                    payload_end = payload_end.max(byte_offset.saturating_add(byte_count));
                }
            }
        }
        offset = directory.next_offset();
    }
    if consumed == 0 {
        return Err(CodecError::Malformed(
            "TIFF contains no image directory".to_owned(),
        ));
    }
    // Strip/tile payloads can extend beyond the final IFD, so the container
    // extent is the later of the IFD chain end and the payload ranges.
    // `pixel` is the sum of strip/tile payload bytes inside the extent.
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "Metadata accounting subtracts the declared strip and tile payload total from the container extent."
    )]
    let metadata = consumed.max(payload_end) as u64 - pixel;
    Ok(metadata)
}

fn decode_ifd(
    data: &[u8],
    ifd_offset: usize,
    endian: Endian,
    budget: Option<&mut SequenceDecodeBudget>,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(DecodedImage, usize, usize)> {
    crate::codecs::error::check_cancelled(token)?;
    let mut directory = Directory::parse(data, ifd_offset, endian)
        .map_err(|error| error.at(ifd_offset as u64, "tiff_ifd"))?;
    let next_offset = directory.next_offset();
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "The supported layout and inspected dimensions bound this directory-end calculation."
    )]
    let directory_end = ifd_offset
        .saturating_add(2)
        .saturating_add(directory.entries.len().saturating_mul(12))
        .saturating_add(4);
    validate_decode_field_types(&directory)?;
    let fill_order = validate_fill_order(&directory)?;

    let (width, height) = super::inspect::validate_primary_dimensions(&directory)?;
    let samples_per_pixel = directory.one_or(277, 1);
    let bits = directory.values_or(258, &[1]);
    if bits.is_empty() || bits.iter().any(|&value| value != bits[0]) {
        return Err(CodecError::Malformed(
            "TIFF bits-per-sample values are invalid".to_owned(),
        ));
    }
    let bits_per_sample = u8::try_from(bits[0]).map_err(|error| {
        CodecError::Malformed(format!("TIFF bit depth exceeds format limits: {error}"))
    })?;
    let compression = directory.one_or(259, COMPRESSION_NONE);
    let photometric = directory.one_or(262, 1);
    let rows_per_strip = directory.one_or(278, height as usize);
    let predictor = directory.one_or(317, 1);
    let planar = directory.one_or(284, 1);
    let sample_format = directory.one_or(339, 1);
    let ycbcr_parameters = if photometric == 6 && compression != COMPRESSION_NONE {
        Some(super::inspect::ycbcr_parameters(&directory)?)
    } else {
        None
    };
    let color_map = directory.values(320);
    if rows_per_strip == 0 {
        return Err(CodecError::Dimensions(
            "TIFF rows per strip must be nonzero".to_owned(),
        ));
    }
    if samples_per_pixel == 0 || !matches!(planar, 1 | 2) || !matches!(predictor, 1 | 2) {
        return Err(CodecError::Malformed(
            "TIFF sample, strip, planar, or predictor fields are invalid".to_owned(),
        ));
    }
    let separate_planar = planar == 2 && samples_per_pixel > 1;
    let extra_samples = directory.values(338);
    let (layout, palette) = super::inspect::layout_and_palette(TiffLayoutParameters {
        photometric,
        samples: samples_per_pixel,
        bits: usize::from(bits_per_sample),
        sample_format,
        compression,
        color_map: color_map.as_deref(),
        extra_samples: extra_samples.as_deref().unwrap_or_default(),
        planar,
        ycbcr_subsampling: ycbcr_parameters.map(|parameters| parameters.subsampling),
        ycbcr_conversion: ycbcr_parameters.map(|parameters| parameters.conversion),
    })?;
    if fill_order == 2
        && compression == COMPRESSION_NONE
        && matches!(layout, TiffLayout::Palette { bits: 4 })
    {
        return Err(CodecError::Malformed(
            "TIFF packed 4-bit palette has no supported FillOrder 2 raw mode".to_owned(),
        ));
    }
    let reverse_packed_fill_order = compression == COMPRESSION_NONE
        && fill_order == 2
        && matches!(
            layout,
            TiffLayout::Bilevel { .. } | TiffLayout::Gray2 { .. }
        );
    super::inspect::verify_ignored_raw_band_count(&directory, width, height, layout)?;
    if layout == TiffLayout::GrayAlpha8 && separate_planar && compression == COMPRESSION_NONE {
        return Err(CodecError::Malformed(
            "TIFF raw separate grayscale-alpha band is invalid".to_owned(),
        ));
    }
    let source_alpha = extra_samples
        .as_deref()
        .and_then(source_alpha_from_extra_samples);
    let associated_rgba =
        layout == TiffLayout::Rgba8 && source_alpha == Some(SourceAlpha::Premultiplied);
    let raw_separate_associated_alpha =
        associated_rgba && separate_planar && compression == COMPRESSION_NONE;
    let decoded_planes = match layout {
        TiffLayout::GrayIgnored8 => 1,
        TiffLayout::RgbIgnored8 => 3,
        _ => samples_per_pixel,
    };
    let alpha = TiffAlpha {
        source: source_alpha,
        // Pillow's raw per-band importer leaves missing alpha planes at zero
        // without normalizing the RGB bands. Its compressed importer and
        // contiguous raw importer normalize associated RGBA after assembly.
        unassociate: associated_rgba && !raw_separate_associated_alpha,
        // Pillow's compressed separate importer copies through RGBA band
        // unpackers. LA stores alpha in the fourth internal byte, so copying
        // its second source plane into G leaves the exported alpha at zero.
        clear_gray_alpha: layout == TiffLayout::GrayAlpha8 && separate_planar,
    };
    if let Some(budget) = budget {
        budget
            .reserve_later_frame(layout.mode(), width, height)
            .map_err(CodecError::LimitExceeded)?;
    }

    let width_usize = width as usize;
    let height_usize = height as usize;
    let ycbcr_subsampling = match layout {
        TiffLayout::YcbcrSubsampled8 { subsampling, .. } => Some(subsampling),
        _ => None,
    };
    if ycbcr_subsampling.is_some_and(|subsampling| {
        !matches!(
            subsampling,
            (1, 1) | (1, 2) | (2, 1) | (2, 2) | (4, 1) | (4, 2) | (4, 4)
        )
    }) {
        return Err(CodecError::Malformed(
            "unsupported TIFF YCbCr subsampling".to_owned(),
        ));
    }
    if matches!(
        layout,
        TiffLayout::YcbcrFull8 { .. } | TiffLayout::YcbcrSubsampled8 { .. }
    ) && planar != 1
    {
        return Err(CodecError::Malformed(
            "unsupported separate-planar TIFF YCbCr conversion".to_owned(),
        ));
    }
    if matches!(layout, TiffLayout::YcbcrSubsampled8 { .. }) {
        let tiled = directory.field_type(324).is_some() || directory.field_type(325).is_some();
        if predictor != 1 || tiled {
            return Err(CodecError::Malformed(
                "unsupported TIFF subsampled YCbCr storage layout".to_owned(),
            ));
        }
        let vertical_subsampling = ycbcr_subsampling.map(|(_, vertical)| vertical);
        if vertical_subsampling.is_some_and(|vertical| {
            height_usize > rows_per_strip && !rows_per_strip.is_multiple_of(vertical)
        }) {
            return Err(CodecError::Malformed(
                "TIFF YCbCr strip boundaries must align to subsampled rows".to_owned(),
            ));
        }
    }
    // Pillow's baseline YCbCr TIFF raw mode is RGBX: the IFD declares three
    // samples, but each stored pixel occupies four bytes.
    let stored_samples = if layout == TiffLayout::Ycbcr8 {
        4
    } else {
        samples_per_pixel
    };
    // `layout_and_palette` limits a supported layout to four stored bytes per
    // pixel. Pillow's decompression-bomb ceiling limits the complete raster to
    // 178,956,970 pixels, so these products fit 32-bit and 64-bit `usize`.
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "Validated dimensions and a maximum of four samples bound this row-sample product."
    )]
    let row_samples = width_usize * stored_samples;
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "Validated dimensions, sample count, and bit depth bound this row-bit product."
    )]
    let row_bits = row_samples * usize::from(bits_per_sample);
    let row_bytes = if let Some((horizontal, vertical)) = ycbcr_subsampling {
        let clump_bytes = horizontal
            .checked_mul(vertical)
            .and_then(|luma_samples| luma_samples.checked_add(2))
            .dimensions("TIFF YCbCr clump byte size overflows")?;
        width_usize
            .div_ceil(horizontal)
            .checked_mul(clump_bytes)
            .dimensions("TIFF YCbCr clump row byte size overflows")?
    } else {
        row_bits.div_ceil(8)
    };
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "The validated raster ceiling bounds the row-byte and height product."
    )]
    let expected_total = if let Some((_, vertical)) = ycbcr_subsampling {
        row_bytes
            .checked_mul(height_usize.div_ceil(vertical))
            .dimensions("TIFF YCbCr clump buffer size overflows")?
    } else {
        row_bytes * height_usize
    };

    let tile_offsets = directory.values(324);
    let tile_byte_counts = directory.values(325);
    if tile_offsets.is_some() || tile_byte_counts.is_some() {
        let offsets = tile_offsets.malformed("TIFF tile offsets are missing")?;
        let byte_counts = match tile_byte_counts {
            Some(byte_counts) => byte_counts,
            None if compression == COMPRESSION_NONE
                && matches!(directory.field_type(325), None | Some(2)) =>
            {
                Vec::new()
            }
            None => {
                return Err(CodecError::Malformed(
                    "TIFF tile byte counts are missing".to_owned(),
                ));
            }
        };
        let tile_width = directory
            .one(322)
            .dimensions("TIFF tile width is missing")?;
        let tile_height = directory
            .one(323)
            .dimensions("TIFF tile height is missing")?;
        if tile_width == 0 || tile_height == 0 {
            return Err(CodecError::Dimensions(
                "TIFF tile dimensions must be nonzero".to_owned(),
            ));
        }
        let packed_samples = bits_per_sample < 8;
        // `layout_and_palette` only accepts packed samples at 1, 2, or 4 bits
        // with one sample per pixel, so these tile-layout checks are redundant.
        let tiles_across = width_usize.div_ceil(tile_width);
        let tiles_down = height_usize.div_ceil(tile_height);
        let expected_tiles = tiles_across
            .checked_mul(tiles_down)
            .dimensions("TIFF tile count overflows")?;
        if !separate_planar && offsets.len() != expected_tiles {
            return Err(CodecError::Malformed(
                "TIFF tile offset count does not match its geometry".to_owned(),
            ));
        }
        if separate_planar {
            let pixels = decode_separate_planar_tiles(
                data,
                SeparatePlanarTileLayout {
                    offsets: &offsets,
                    declared_byte_counts: &byte_counts,
                    width: width_usize,
                    height: height_usize,
                    tiles_across,
                    tiles_down,
                    tile_width,
                    tile_height,
                    samples_per_pixel,
                    decoded_planes,
                    output_samples: stored_samples,
                    output_row_bytes: row_bytes,
                    output_len: expected_total,
                    compression,
                    predictor,
                    bits_per_sample,
                    endian,
                    raw_associated_alpha: raw_separate_associated_alpha,
                },
                token,
            )?;
            return Ok((
                convert_pixels_for_token(
                    (width, height),
                    pixels,
                    layout,
                    endian,
                    palette,
                    alpha,
                    token,
                )?
                .with_opaque_blocks(std::mem::take(&mut directory.opaque_blocks))
                .with_metadata(std::mem::take(&mut directory.metadata)),
                next_offset,
                directory_end,
            ));
        }
        // Packed layouts use bit offsets. Keep the existing byte-aligned
        // arithmetic unchanged for the common 8-bit and wider path.
        let (tile_row_bytes, bytes_per_pixel) = if packed_samples {
            let tile_row_bits = tile_width
                .checked_mul(stored_samples)
                .and_then(|samples| samples.checked_mul(usize::from(bits_per_sample)))
                .dimensions("TIFF packed tile row bit size overflows")?;
            (tile_row_bits.div_ceil(8), 0)
        } else {
            // The validated layout has at most four 32-bit samples.
            #[allow(
                clippy::arithmetic_side_effects,
                reason = "The validated layout has at most four 32-bit samples per pixel."
            )]
            let bytes_per_pixel = stored_samples * (usize::from(bits_per_sample) / 8);
            #[cfg(target_pointer_width = "32")]
            let tile_row_bytes = tile_width
                .checked_mul(bytes_per_pixel)
                .malformed("TIFF tile row byte size overflows")?;
            #[cfg(not(target_pointer_width = "32"))]
            #[allow(
                clippy::arithmetic_side_effects,
                reason = "On 64-bit targets the u32 tile width times the validated pixel stride fits usize."
            )]
            let tile_row_bytes = tile_width * bytes_per_pixel;
            (tile_row_bytes, bytes_per_pixel)
        };
        let tile_size = tile_row_bytes
            .checked_mul(tile_height)
            .malformed("TIFF tile byte size overflows")?;
        // libtiff, and therefore Pillow, derives uncompressed tile lengths from
        // the tile geometry even when TileByteCounts is empty or inconsistent.
        let byte_counts = if compression == COMPRESSION_NONE {
            vec![tile_size; offsets.len()]
        } else {
            if offsets.len() != byte_counts.len() {
                return Err(CodecError::Malformed(
                    "TIFF tile offset and byte-count lengths differ".to_owned(),
                ));
            }
            byte_counts
        };
        let mut pixels = vec![0; expected_total];
        for (tile_index, (&offset, &byte_count)) in offsets.iter().zip(&byte_counts).enumerate() {
            crate::codecs::error::check_cancelled(token)?;
            #[cfg(target_pointer_width = "32")]
            let encoded_end = offset
                .checked_add(byte_count)
                .malformed("TIFF tile payload is out of bounds")?;
            #[cfg(not(target_pointer_width = "32"))]
            let encoded_end = offset.saturating_add(byte_count);
            let encoded = if encoded_end <= data.len() {
                &data[offset..encoded_end]
            } else {
                return Err(CodecError::NeedMore {
                    minimum: encoded_end,
                    message: "TIFF tile payload is out of bounds".to_owned(),
                });
            };
            let tile_column = tile_index.checked_rem(tiles_across).unwrap_or_default();
            let tile_row = tile_index.checked_div(tiles_across).unwrap_or_default();
            let tile_x = tile_column.wrapping_mul(tile_width);
            let tile_y = tile_row.wrapping_mul(tile_height);
            let copied_width = tile_width.min(width_usize.saturating_sub(tile_x));
            let copied_height = tile_height.min(height_usize.saturating_sub(tile_y));
            if packed_samples {
                let decoded;
                let tile_data = if compression == COMPRESSION_NONE {
                    encoded
                } else {
                    decoded = decode_block(compression, encoded, tile_size, token)
                        .map_err(|error| error.at(offset as u64, "tiff_tile"))?;
                    &decoded
                };
                let placement = PackedTilePlacement {
                    tile_row_bytes,
                    row_bytes,
                    tile_x,
                    tile_y,
                    copied_width,
                    copied_height,
                    bits_per_sample,
                };
                if reverse_packed_fill_order {
                    copy_packed_tile_into::<true>(&mut pixels, tile_data, placement, token)
                } else {
                    copy_packed_tile_into::<false>(&mut pixels, tile_data, placement, token)
                }
                .map_err(|error| error.at(offset as u64, "tiff_tile"))?;
                continue;
            }
            let copied_bytes = copied_width.wrapping_mul(bytes_per_pixel);
            if compression == COMPRESSION_NONE {
                copy_raw_tile_into(
                    &mut pixels,
                    encoded,
                    RawTilePlacement {
                        tile_row_bytes,
                        row_bytes,
                        tile_x,
                        tile_y,
                        bytes_per_pixel,
                        copied_height,
                        copied_bytes,
                    },
                    token,
                )
                .map_err(|error| error.at(offset as u64, "tiff_tile"))?;
                continue;
            }
            let mut decoded = decode_block(compression, encoded, tile_size, token)
                .map_err(|error| error.at(offset as u64, "tiff_tile"))?;
            // Every compressed decoder returns exactly the requested size, and
            // uncompressed tile counts were normalized to tile_size above.
            if uses_horizontal_predictor(predictor, compression, bits_per_sample) {
                if let Some(token) = token {
                    reverse_horizontal_predictor_with_token(
                        &mut decoded,
                        tile_row_bytes,
                        samples_per_pixel,
                        bits_per_sample,
                        endian,
                        token,
                    )
                    .map_err(|error| error.at(offset as u64, "tiff_tile"))?;
                } else {
                    reverse_horizontal_predictor(
                        &mut decoded,
                        tile_row_bytes,
                        samples_per_pixel,
                        bits_per_sample,
                        endian,
                    );
                }
            }
            for y in 0..copied_height {
                let source = y.wrapping_mul(tile_row_bytes);
                let destination = tile_y
                    .wrapping_add(y)
                    .wrapping_mul(row_bytes)
                    .wrapping_add(tile_x.wrapping_mul(bytes_per_pixel));
                pixels[destination..destination.wrapping_add(copied_bytes)]
                    .copy_from_slice(&decoded[source..source.wrapping_add(copied_bytes)]);
            }
        }
        return Ok((
            convert_pixels_for_token(
                (width, height),
                pixels,
                layout,
                endian,
                palette,
                alpha,
                token,
            )?
            .with_opaque_blocks(std::mem::take(&mut directory.opaque_blocks))
            .with_metadata(std::mem::take(&mut directory.metadata)),
            next_offset,
            directory_end,
        ));
    }

    let offsets = directory
        .values(273)
        .malformed("TIFF strip offsets are missing")?;
    // Pillow derives uncompressed strip sizes from the raster layout when
    // StripByteCounts is absent. For compressed strips, a missing, empty, or
    // zero-valued count uses the next strip boundary (or the file boundary).
    let declared_byte_counts = directory.values(279);
    if offsets.is_empty() {
        return Err(CodecError::Malformed(
            "TIFF strip offsets are empty".to_owned(),
        ));
    }
    if separate_planar {
        let pixels = decode_separate_planar_strips(
            data,
            SeparatePlanarStripLayout {
                offsets: &offsets,
                declared_byte_counts: declared_byte_counts.as_deref(),
                width: width_usize,
                height: height_usize,
                rows_per_strip,
                samples_per_pixel,
                decoded_planes,
                output_samples: stored_samples,
                output_row_bytes: row_bytes,
                output_len: expected_total,
                compression,
                predictor,
                bits_per_sample,
                endian,
                raw_associated_alpha: raw_separate_associated_alpha,
            },
            token,
        )?;
        return Ok((
            convert_pixels_for_token(
                (width, height),
                pixels,
                layout,
                endian,
                palette,
                alpha,
                token,
            )?
            .with_opaque_blocks(std::mem::take(&mut directory.opaque_blocks))
            .with_metadata(std::mem::take(&mut directory.metadata)),
            next_offset,
            directory_end,
        ));
    }
    let expected_strips = height_usize.div_ceil(rows_per_strip);
    if offsets.len() > expected_strips && compression != COMPRESSION_LZW {
        return Err(CodecError::Malformed(
            "TIFF contains more strips than its geometry permits".to_owned(),
        ));
    }
    let derive_compressed_byte_count = |index: usize, offset: usize| {
        let end = offsets
            .get(index.wrapping_add(1))
            .copied()
            .unwrap_or(if ifd_offset > offset {
                ifd_offset
            } else {
                data.len()
            });
        end.checked_sub(offset)
            .malformed("TIFF strip offsets are not monotonic")
    };
    let byte_counts = if compression == COMPRESSION_NONE {
        (0..offsets.len())
            .map(|strip_index| {
                let first_row = strip_index.wrapping_mul(rows_per_strip);
                let strip_rows = rows_per_strip.min(height_usize.saturating_sub(first_row));
                let stored_rows = ycbcr_subsampling
                    .map_or(strip_rows, |(_, vertical)| strip_rows.div_ceil(vertical));
                row_bytes.wrapping_mul(stored_rows)
            })
            .collect::<Vec<_>>()
    } else {
        let declared_byte_counts = declared_byte_counts.unwrap_or_default();
        if declared_byte_counts.is_empty() {
            offsets
                .iter()
                .enumerate()
                .map(|(index, &offset)| derive_compressed_byte_count(index, offset))
                .collect::<CodecResult<Vec<_>>>()?
        } else if offsets.len() != declared_byte_counts.len() {
            return Err(CodecError::Malformed(
                "TIFF strip offset and byte-count lengths differ".to_owned(),
            ));
        } else {
            declared_byte_counts
        }
    };
    let mut pixels = Vec::new();
    #[cfg(coverage)]
    let reservation = if crate::coverage_support::take_fault_point(
        crate::coverage_support::CoverageFaultPoint::TiffStripAssemblyReservation,
    ) {
        usize::MAX
    } else {
        expected_total
    };
    #[cfg(not(coverage))]
    let reservation = expected_total;
    pixels
        .try_reserve_exact(reservation)
        .map_err(|_| CodecError::Dimensions("unable to allocate TIFF strip buffer".to_owned()))?;

    for (strip_index, (&offset, &declared_byte_count)) in
        offsets.iter().zip(&byte_counts).enumerate()
    {
        crate::codecs::error::check_cancelled(token)?;
        let byte_count = if compression != COMPRESSION_NONE && declared_byte_count == 0 {
            derive_compressed_byte_count(strip_index, offset)?
        } else {
            declared_byte_count
        };
        #[cfg(target_pointer_width = "32")]
        let encoded_end = offset
            .checked_add(byte_count)
            .malformed("TIFF strip payload is out of bounds")?;
        #[cfg(not(target_pointer_width = "32"))]
        let encoded_end = offset.saturating_add(byte_count);
        let encoded = if encoded_end <= data.len() {
            &data[offset..encoded_end]
        } else {
            return Err(CodecError::NeedMore {
                minimum: encoded_end,
                message: "TIFF strip payload is out of bounds".to_owned(),
            });
        };
        let first_row = strip_index.wrapping_mul(rows_per_strip);
        let strip_rows = rows_per_strip.min(height_usize.saturating_sub(first_row));
        let stored_rows =
            ycbcr_subsampling.map_or(strip_rows, |(_, vertical)| strip_rows.div_ceil(vertical));
        let expected = row_bytes.wrapping_mul(stored_rows);
        // The geometry check above rejects extra strips except for LZW;
        // those declared, bounded payloads do not contribute image rows.
        if strip_rows == 0 {
            continue;
        }
        if compression == COMPRESSION_NONE {
            copy_raw_into(&mut pixels, encoded, token)
                .map_err(|error| error.at(offset as u64, "tiff_strip"))?;
            continue;
        }
        let mut decoded = decode_block(compression, encoded, expected, token)
            .map_err(|error| error.at(offset as u64, "tiff_strip"))?;
        if uses_horizontal_predictor(predictor, compression, bits_per_sample) {
            if let Some(token) = token {
                reverse_horizontal_predictor_with_token(
                    &mut decoded,
                    row_bytes,
                    samples_per_pixel,
                    bits_per_sample,
                    endian,
                    token,
                )
                .map_err(|error| error.at(offset as u64, "tiff_strip"))?;
            } else {
                reverse_horizontal_predictor(
                    &mut decoded,
                    row_bytes,
                    samples_per_pixel,
                    bits_per_sample,
                    endian,
                );
            }
        }
        pixels.extend_from_slice(&decoded);
    }
    pixels.resize(expected_total, 0);
    if reverse_packed_fill_order {
        reverse_tiff_fill_order(&mut pixels, token)?;
    }

    Ok((
        convert_pixels_for_token(
            (width, height),
            pixels,
            layout,
            endian,
            palette,
            alpha,
            token,
        )?
        .with_opaque_blocks(std::mem::take(&mut directory.opaque_blocks))
        .with_metadata(std::mem::take(&mut directory.metadata)),
        next_offset,
        directory_end,
    ))
}

struct SeparatePlanarStripLayout<'a> {
    offsets: &'a [usize],
    declared_byte_counts: Option<&'a [usize]>,
    width: usize,
    height: usize,
    rows_per_strip: usize,
    samples_per_pixel: usize,
    decoded_planes: usize,
    output_samples: usize,
    output_row_bytes: usize,
    output_len: usize,
    compression: usize,
    predictor: usize,
    bits_per_sample: u8,
    endian: Endian,
    raw_associated_alpha: bool,
}

fn decode_separate_planar_strips(
    data: &[u8],
    layout: SeparatePlanarStripLayout<'_>,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    let strips_per_plane = layout.height.div_ceil(layout.rows_per_strip);
    let expected_strips = strips_per_plane
        .checked_mul(layout.samples_per_pixel)
        .dimensions("TIFF separate-planar strip count overflows")?;
    if layout.offsets.len() > expected_strips {
        return Err(CodecError::Malformed(
            "TIFF separate-planar strip count exceeds its geometry".to_owned(),
        ));
    }
    if layout.raw_associated_alpha && layout.offsets.len() > strips_per_plane.saturating_mul(3) {
        return Err(CodecError::Malformed(
            "TIFF raw separate associated-alpha band is invalid".to_owned(),
        ));
    }

    let declared_byte_counts = layout
        .declared_byte_counts
        .filter(|counts| !counts.is_empty());
    if layout.compression != COMPRESSION_NONE
        && declared_byte_counts.is_none_or(|counts| counts.len() != layout.offsets.len())
    {
        return Err(CodecError::Malformed(
            "TIFF strip offset and byte-count lengths differ".to_owned(),
        ));
    }

    let mut pixels = vec![0; layout.output_len];
    // Compressed Pillow loading decodes only retained bands. The ignored
    // trailing planes must not contribute payload errors or output samples.
    let decoded_strips = strips_per_plane
        .checked_mul(layout.decoded_planes)
        .dimensions("TIFF retained separate-planar strip count overflows")?;
    for (index, &offset) in layout.offsets.iter().take(decoded_strips).enumerate() {
        crate::codecs::error::check_cancelled(token)?;
        let plane = index
            .checked_div(strips_per_plane)
            .dimensions("TIFF separate-planar strip plane is invalid")?;
        let strip_in_plane = index
            .checked_rem(strips_per_plane)
            .dimensions("TIFF separate-planar strip index is invalid")?;
        let first_row = strip_in_plane
            .checked_mul(layout.rows_per_strip)
            .dimensions("TIFF separate-planar strip row offset overflows")?;
        let strip_rows = layout
            .rows_per_strip
            .min(layout.height.saturating_sub(first_row));
        let expected = layout
            .width
            .checked_mul(strip_rows)
            .dimensions("TIFF separate-planar strip size overflows")?;
        let byte_count = match (layout.compression, declared_byte_counts) {
            (COMPRESSION_NONE, _) => expected,
            (_, Some(counts)) => counts[index],
            (_, None) => 0,
        };
        let encoded_end = offset
            .checked_add(byte_count)
            .malformed("TIFF strip payload is out of bounds")?;
        let encoded = if encoded_end <= data.len() {
            &data[offset..encoded_end]
        } else {
            return Err(CodecError::NeedMore {
                minimum: encoded_end,
                message: "TIFF strip payload is out of bounds".to_owned(),
            });
        };
        let mut decoded = if layout.compression == COMPRESSION_NONE {
            None
        } else {
            Some(
                decode_block(layout.compression, encoded, expected, token)
                    .map_err(|error| error.at(offset as u64, "tiff_strip"))?,
            )
        };
        if let Some(decoded) = decoded.as_mut()
            && uses_horizontal_predictor(
                layout.predictor,
                layout.compression,
                layout.bits_per_sample,
            )
        {
            if let Some(token) = token {
                reverse_horizontal_predictor_with_token(
                    decoded,
                    layout.width,
                    1,
                    layout.bits_per_sample,
                    layout.endian,
                    token,
                )
                .map_err(|error| error.at(offset as u64, "tiff_strip"))?;
            } else {
                reverse_horizontal_predictor(
                    decoded,
                    layout.width,
                    1,
                    layout.bits_per_sample,
                    layout.endian,
                );
            }
        }

        let decoded = decoded.as_deref().unwrap_or(encoded);
        for (row_index, samples) in decoded.chunks(layout.width).enumerate() {
            crate::codecs::error::check_cancelled(token)?;
            let row = first_row
                .checked_add(row_index)
                .dimensions("TIFF separate-planar output row overflows")?;
            let destination_start = row
                .checked_mul(layout.output_row_bytes)
                .dimensions("TIFF separate-planar output offset overflows")?;
            let destination_end = destination_start
                .checked_add(layout.output_row_bytes)
                .dimensions("TIFF separate-planar output range overflows")?;
            let destination = &mut pixels[destination_start..destination_end];
            for (pixel, &sample) in destination
                .chunks_exact_mut(layout.output_samples)
                .zip(samples)
            {
                pixel[plane] = sample;
            }
        }
    }
    Ok(pixels)
}

struct SeparatePlanarTileLayout<'a> {
    offsets: &'a [usize],
    declared_byte_counts: &'a [usize],
    width: usize,
    height: usize,
    tiles_across: usize,
    tiles_down: usize,
    tile_width: usize,
    tile_height: usize,
    samples_per_pixel: usize,
    decoded_planes: usize,
    output_samples: usize,
    output_row_bytes: usize,
    output_len: usize,
    compression: usize,
    predictor: usize,
    bits_per_sample: u8,
    endian: Endian,
    raw_associated_alpha: bool,
}

fn decode_separate_planar_tiles(
    data: &[u8],
    layout: SeparatePlanarTileLayout<'_>,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    let tiles_per_plane = layout
        .tiles_across
        .checked_mul(layout.tiles_down)
        .dimensions("TIFF separate-planar tile count overflows")?;
    let expected_offsets = tiles_per_plane
        .checked_mul(layout.samples_per_pixel)
        .dimensions("TIFF separate-planar tile offset count overflows")?;
    if layout.offsets.len() > expected_offsets {
        return Err(CodecError::Malformed(
            "TIFF separate-planar tile offset count exceeds its geometry".to_owned(),
        ));
    }
    if layout.raw_associated_alpha && layout.offsets.len() > tiles_per_plane.saturating_mul(3) {
        return Err(CodecError::Malformed(
            "TIFF raw separate associated-alpha band is invalid".to_owned(),
        ));
    }

    let sample_bytes = usize::from(layout.bits_per_sample) / 8;
    let tile_row_bytes = layout
        .tile_width
        .checked_mul(sample_bytes)
        .dimensions("TIFF separate-planar tile row byte size overflows")?;
    let tile_size = tile_row_bytes
        .checked_mul(layout.tile_height)
        .dimensions("TIFF separate-planar tile size overflows")?;
    let byte_counts = if layout.compression == COMPRESSION_NONE {
        vec![tile_size; layout.offsets.len()]
    } else {
        if layout.offsets.len() != layout.declared_byte_counts.len() {
            return Err(CodecError::Malformed(
                "TIFF tile offset and byte-count lengths differ".to_owned(),
            ));
        }
        layout.declared_byte_counts.to_vec()
    };

    let output_sample_bytes = layout
        .output_samples
        .checked_mul(sample_bytes)
        .dimensions("TIFF separate-planar pixel stride overflows")?;
    let mut pixels = vec![0; layout.output_len];
    let decoded_tiles = tiles_per_plane
        .checked_mul(layout.decoded_planes)
        .dimensions("TIFF retained separate-planar tile count overflows")?;
    for (index, (&offset, &byte_count)) in layout
        .offsets
        .iter()
        .zip(&byte_counts)
        .take(decoded_tiles)
        .enumerate()
    {
        crate::codecs::error::check_cancelled(token)?;
        let plane = index
            .checked_div(tiles_per_plane)
            .dimensions("TIFF separate-planar tile plane is invalid")?;
        let tile_index = index
            .checked_rem(tiles_per_plane)
            .dimensions("TIFF separate-planar tile index is invalid")?;
        let tile_column = tile_index
            .checked_rem(layout.tiles_across)
            .dimensions("TIFF separate-planar tile column is invalid")?;
        let tile_row = tile_index
            .checked_div(layout.tiles_across)
            .dimensions("TIFF separate-planar tile row is invalid")?;
        let tile_x = tile_column
            .checked_mul(layout.tile_width)
            .dimensions("TIFF separate-planar tile x offset overflows")?;
        let tile_y = tile_row
            .checked_mul(layout.tile_height)
            .dimensions("TIFF separate-planar tile y offset overflows")?;
        let copied_width = layout.tile_width.min(layout.width.saturating_sub(tile_x));
        let copied_height = layout.tile_height.min(layout.height.saturating_sub(tile_y));
        let encoded_end = offset
            .checked_add(byte_count)
            .malformed("TIFF tile payload is out of bounds")?;
        let encoded = if encoded_end <= data.len() {
            &data[offset..encoded_end]
        } else {
            return Err(CodecError::NeedMore {
                minimum: encoded_end,
                message: "TIFF tile payload is out of bounds".to_owned(),
            });
        };
        let mut decoded = if layout.compression == COMPRESSION_NONE {
            None
        } else {
            Some(
                decode_block(layout.compression, encoded, tile_size, token)
                    .map_err(|error| error.at(offset as u64, "tiff_tile"))?,
            )
        };
        if let Some(decoded) = decoded.as_mut()
            && uses_horizontal_predictor(
                layout.predictor,
                layout.compression,
                layout.bits_per_sample,
            )
        {
            if let Some(token) = token {
                reverse_horizontal_predictor_with_token(
                    decoded,
                    tile_row_bytes,
                    1,
                    layout.bits_per_sample,
                    layout.endian,
                    token,
                )
                .map_err(|error| error.at(offset as u64, "tiff_tile"))?;
            } else {
                reverse_horizontal_predictor(
                    decoded,
                    tile_row_bytes,
                    1,
                    layout.bits_per_sample,
                    layout.endian,
                );
            }
        }

        let decoded = decoded.as_deref().unwrap_or(encoded);
        for y in 0..copied_height {
            crate::codecs::error::check_cancelled(token)?;
            let source_row = y
                .checked_mul(tile_row_bytes)
                .dimensions("TIFF separate-planar tile source row overflows")?;
            let destination_row = tile_y
                .checked_add(y)
                .and_then(|row| row.checked_mul(layout.output_row_bytes))
                .dimensions("TIFF separate-planar tile destination row overflows")?;
            for x in 0..copied_width {
                let source = x
                    .checked_mul(sample_bytes)
                    .and_then(|sample| source_row.checked_add(sample))
                    .dimensions("TIFF separate-planar tile source offset overflows")?;
                let source_end = source
                    .checked_add(sample_bytes)
                    .dimensions("TIFF separate-planar tile source range overflows")?;
                let column = tile_x
                    .checked_add(x)
                    .dimensions("TIFF separate-planar tile column overflows")?;
                let pixel_offset = column
                    .checked_mul(output_sample_bytes)
                    .dimensions("TIFF separate-planar tile pixel offset overflows")?;
                let channel_offset = plane
                    .checked_mul(sample_bytes)
                    .dimensions("TIFF separate-planar tile channel offset overflows")?;
                let destination = destination_row
                    .checked_add(pixel_offset)
                    .and_then(|pixel| pixel.checked_add(channel_offset))
                    .dimensions("TIFF separate-planar tile destination offset overflows")?;
                let destination_end = destination
                    .checked_add(sample_bytes)
                    .dimensions("TIFF separate-planar tile destination range overflows")?;
                pixels[destination..destination_end].copy_from_slice(&decoded[source..source_end]);
            }
        }
    }
    Ok(pixels)
}

/// Parse every TIFF signature registered by Pillow over a classic-IFD payload.
///
/// Pillow accepts the two canonical classic signatures, their legacy
/// byte-swapped magic variants, and both BigTIFF signatures during format
/// detection. Its pinned parser selects BigTIFF only when header byte two is
/// `43`, so `II+\0` reaches an unsupported directory layout while `MM\0+`
/// continues through the classic big-endian parser. Genuine BigTIFF directory
/// layout is outside the current contract.
pub(super) fn parse_header(data: &[u8]) -> CodecResult<(Endian, usize)> {
    let endian = match need_slice(data, 0, 2, "TIFF byte-order marker is truncated")? {
        b"II" => Endian::Little,
        b"MM" => Endian::Big,
        _ => {
            return Err(CodecError::Malformed(
                "TIFF byte-order marker is invalid".to_owned(),
            ));
        }
    };
    let magic = need_slice(data, 2, 4, "TIFF magic is truncated")?;
    let registered = match endian {
        Endian::Little => matches!(magic, b"\x2a\x00" | b"\x00\x2a" | b"\x2b\x00"),
        Endian::Big => matches!(magic, b"\x00\x2a" | b"\x2a\x00" | b"\x00\x2b"),
    };
    if !registered {
        return Err(CodecError::Malformed("TIFF magic is invalid".to_owned()));
    }
    if endian == Endian::Little && magic == b"\x2b\x00" {
        return Err(CodecError::Malformed(
            "TIFF BigTIFF directory layout is unsupported".to_owned(),
        ));
    }
    let ifd_offset = need_slice(data, 4, 8, "TIFF directory offset is truncated")?;
    let ifd_offset =
        endian.u32_exact([ifd_offset[0], ifd_offset[1], ifd_offset[2], ifd_offset[3]]) as usize;
    Ok((endian, ifd_offset))
}

fn decode_block(
    compression: usize,
    encoded: &[u8],
    expected: usize,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    match compression {
        COMPRESSION_LZW => match token {
            Some(token) => decode_lzw_with_token(encoded, expected, token),
            None => decode_lzw(encoded, expected),
        },
        COMPRESSION_DEFLATE | COMPRESSION_ADOBE_DEFLATE => {
            let inflated = match token {
                Some(token) => {
                    decompress_zlib_prefix_with_status_and_token(encoded, expected, Some(token))
                        .map(|(output, _)| output)
                }
                None => decompress_zlib_prefix(encoded, expected),
            };
            inflated
                .map_err(crate::codecs::error::terminalize)
                .map_err(|error| error.context("decode TIFF Deflate stream"))
        }
        COMPRESSION_PACKBITS => match token {
            Some(token) => decode_packbits_with_token(encoded, expected, token),
            None => decode_packbits(encoded, expected),
        },
        _ => Err(CodecError::Malformed(
            "TIFF compression method is unsupported".to_owned(),
        )),
    }
}

/// Normalize uncompressed packed bytes to TIFF's default high-bit-first order.
fn reverse_tiff_fill_order(
    bytes: &mut [u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    for chunk in bytes.chunks_mut(1_024) {
        crate::codecs::error::check_cancelled(token)?;
        for byte in chunk {
            *byte = byte.reverse_bits();
        }
    }
    Ok(())
}

fn copy_raw_into(
    output: &mut Vec<u8>,
    data: &[u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    if let Some(token) = token {
        for chunk in data.chunks(1_024) {
            crate::codecs::error::check_cancelled(Some(token))?;
            output.extend_from_slice(chunk);
        }
    } else {
        output.extend_from_slice(data);
    }
    Ok(())
}

struct RawTilePlacement {
    tile_row_bytes: usize,
    row_bytes: usize,
    tile_x: usize,
    tile_y: usize,
    bytes_per_pixel: usize,
    copied_height: usize,
    copied_bytes: usize,
}

fn copy_raw_tile_into(
    output: &mut [u8],
    data: &[u8],
    placement: RawTilePlacement,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    if let Some(token) = token {
        let mut chunk_start = 0usize;
        for chunk in data.chunks(1_024) {
            crate::codecs::error::check_cancelled(Some(token))?;
            let chunk_end = chunk_start.wrapping_add(chunk.len());
            for y in 0..placement.copied_height {
                let row_start = y.wrapping_mul(placement.tile_row_bytes);
                let row_end = row_start.wrapping_add(placement.copied_bytes);
                let source_start = row_start.max(chunk_start);
                let source_end = row_end.min(chunk_end);
                if source_start < source_end {
                    let source_offset = source_start.wrapping_sub(chunk_start);
                    let destination = placement
                        .tile_y
                        .wrapping_add(y)
                        .wrapping_mul(placement.row_bytes)
                        .wrapping_add(placement.tile_x.wrapping_mul(placement.bytes_per_pixel))
                        .wrapping_add(source_start.wrapping_sub(row_start));
                    let length = source_end.wrapping_sub(source_start);
                    output[destination..destination.wrapping_add(length)]
                        .copy_from_slice(&chunk[source_offset..source_offset.wrapping_add(length)]);
                }
            }
            chunk_start = chunk_end;
        }
    } else {
        for y in 0..placement.copied_height {
            let source = y.wrapping_mul(placement.tile_row_bytes);
            let destination = placement
                .tile_y
                .wrapping_add(y)
                .wrapping_mul(placement.row_bytes)
                .wrapping_add(placement.tile_x.wrapping_mul(placement.bytes_per_pixel));
            output[destination..destination.wrapping_add(placement.copied_bytes)]
                .copy_from_slice(&data[source..source.wrapping_add(placement.copied_bytes)]);
        }
    }
    Ok(())
}

struct PackedTilePlacement {
    tile_row_bytes: usize,
    row_bytes: usize,
    tile_x: usize,
    tile_y: usize,
    copied_width: usize,
    copied_height: usize,
    bits_per_sample: u8,
}

fn copy_packed_tile_into<const LSB_FIRST: bool>(
    output: &mut [u8],
    data: &[u8],
    placement: PackedTilePlacement,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    // The sole caller dispatches only parsed 1-, 2-, and 4-bit TIFF layouts.
    let bits = usize::from(placement.bits_per_sample);
    let source_bits = placement
        .copied_width
        .checked_mul(bits)
        .dimensions("TIFF packed tile source row bit size overflows")?;
    let source_row_bytes = source_bits.div_ceil(8);

    for row in 0..placement.copied_height {
        crate::codecs::error::check_cancelled(token)?;
        let source_start = row
            .checked_mul(placement.tile_row_bytes)
            .dimensions("TIFF packed tile source offset overflows")?;
        let source_end = source_start
            .checked_add(source_row_bytes)
            .dimensions("TIFF packed tile source range overflows")?;
        let source_row = data
            .get(source_start..source_end)
            .ok_or_else(|| CodecError::Malformed("TIFF packed tile row is truncated".to_owned()))?;

        let destination_row_index = placement
            .tile_y
            .checked_add(row)
            .dimensions("TIFF packed tile destination row overflows")?;
        let destination_start = destination_row_index
            .checked_mul(placement.row_bytes)
            .dimensions("TIFF packed tile destination offset overflows")?;
        let destination_end = destination_start
            .checked_add(placement.row_bytes)
            .dimensions("TIFF packed tile destination range overflows")?;
        let destination_row = output
            .get_mut(destination_start..destination_end)
            .ok_or_else(|| {
                CodecError::Malformed("TIFF packed tile destination row is invalid".to_owned())
            })?;
        copy_packed_tile_row::<LSB_FIRST>(
            destination_row,
            source_row,
            placement.tile_x,
            placement.copied_width,
            bits,
            token,
        )?;
    }
    Ok(())
}

fn copy_packed_tile_row<const LSB_FIRST: bool>(
    destination: &mut [u8],
    source: &[u8],
    tile_x: usize,
    sample_count: usize,
    bits: usize,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    let destination_start_bit = tile_x
        .checked_mul(bits)
        .dimensions("TIFF packed tile destination bit offset overflows")?;
    let copied_bits = sample_count
        .checked_mul(bits)
        .dimensions("TIFF packed tile copy bit size overflows")?;
    let _destination_end_bit = destination_start_bit
        .checked_add(copied_bits)
        .dimensions("TIFF packed tile destination bit range overflows")?;
    // The caller clips this range to the image row and slices a complete source
    // row before dispatch, so repeating those bounds checks here is redundant.

    if destination_start_bit.is_multiple_of(8) {
        let destination_start = destination_start_bit / 8;
        let whole_bytes = copied_bits / 8;
        let destination_full_end = destination_start
            .checked_add(whole_bytes)
            .dimensions("TIFF packed tile destination range overflows")?;
        if let Some(token) = token {
            for (source_chunk, destination_chunk) in source[..whole_bytes]
                .chunks(1_024)
                .zip(destination[destination_start..destination_full_end].chunks_mut(1_024))
            {
                crate::codecs::error::check_cancelled(Some(token))?;
                if LSB_FIRST {
                    for (source_byte, destination_byte) in
                        source_chunk.iter().zip(destination_chunk)
                    {
                        *destination_byte = source_byte.reverse_bits();
                    }
                } else {
                    destination_chunk.copy_from_slice(source_chunk);
                }
            }
        } else if LSB_FIRST {
            for (source_byte, destination_byte) in source[..whole_bytes]
                .iter()
                .zip(&mut destination[destination_start..destination_full_end])
            {
                *destination_byte = source_byte.reverse_bits();
            }
        } else {
            destination[destination_start..destination_full_end]
                .copy_from_slice(&source[..whole_bytes]);
        }

        let tail_bits = copied_bits % 8;
        if tail_bits != 0 {
            let tail_shift = 8usize
                .checked_sub(tail_bits)
                .and_then(|shift| u32::try_from(shift).ok())
                .malformed("TIFF packed tile tail shift is invalid")?;
            let tail_mask = u8::MAX.wrapping_shl(tail_shift);
            let source_tail = if LSB_FIRST {
                source[whole_bytes].reverse_bits()
            } else {
                source[whole_bytes]
            };
            let destination_tail = &mut destination[destination_full_end];
            *destination_tail = (*destination_tail & !tail_mask) | (source_tail & tail_mask);
        }
        return Ok(());
    }

    let sample_shift = u32::try_from(bits)
        .map_err(|_| CodecError::Malformed("TIFF packed sample width is invalid".to_owned()))?;
    let sample_mask = 1_u8.wrapping_shl(sample_shift).wrapping_sub(1);
    for sample_index in 0..sample_count {
        if sample_index.is_multiple_of(1_024) {
            crate::codecs::error::check_cancelled(token)?;
        }
        let source_bit = sample_index
            .checked_mul(bits)
            .dimensions("TIFF packed tile source bit offset overflows")?;
        let source_shift = if LSB_FIRST {
            u32::try_from(source_bit % 8).map_err(|_| {
                CodecError::Malformed("TIFF packed source sample shift is invalid".to_owned())
            })?
        } else {
            8usize
                .checked_sub(bits)
                .and_then(|shift| shift.checked_sub(source_bit % 8))
                .and_then(|shift| u32::try_from(shift).ok())
                .malformed("TIFF packed source sample shift is invalid")?
        };
        let value = source[source_bit / 8].wrapping_shr(source_shift) & sample_mask;
        let value = if LSB_FIRST {
            // FillOrder=2 stores each packed sample's bits in reverse order.
            // Reverse the sample value when copying into a non-byte-aligned
            // destination; the byte-aligned path reverses complete bytes.
            value
                .reverse_bits()
                .wrapping_shr(8_u32.wrapping_sub(sample_shift))
        } else {
            value
        };
        let destination_bit = destination_start_bit
            .checked_add(source_bit)
            .dimensions("TIFF packed tile destination bit offset overflows")?;
        let destination_byte = destination_bit / 8;
        let destination_shift = 8usize
            .checked_sub(bits)
            .and_then(|shift| shift.checked_sub(destination_bit % 8))
            .and_then(|shift| u32::try_from(shift).ok())
            .malformed("TIFF packed destination sample shift is invalid")?;
        let sample_bits = sample_mask.wrapping_shl(destination_shift);
        destination[destination_byte] =
            (destination[destination_byte] & !sample_bits) | value.wrapping_shl(destination_shift);
    }
    Ok(())
}

fn uses_horizontal_predictor(predictor: usize, compression: usize, bits: u8) -> bool {
    matches!(
        (predictor, compression, bits),
        (
            2,
            COMPRESSION_LZW | COMPRESSION_DEFLATE | COMPRESSION_ADOBE_DEFLATE,
            8 | 16 | 32
        )
    )
}

/// Keep source provenance separate from Pillow's normalized transfer samples.
#[derive(Clone, Copy)]
struct TiffAlpha {
    source: Option<SourceAlpha>,
    unassociate: bool,
    clear_gray_alpha: bool,
}

/// Normalize one associated RGBA pixel exactly as Pillow's RGBa unpacker does.
fn unassociate_rgba_pixel(pixel: &mut [u8; 4]) {
    match pixel[3] {
        0 => pixel.fill(0),
        255 => {}
        alpha => {
            for sample in &mut pixel[..3] {
                // Both operands are bytes, so the product fits u16. This arm
                // has a nonzero denominator; clipping also handles stored
                // colors greater than their declared alpha.
                #[expect(
                    clippy::arithmetic_side_effects,
                    reason = "A byte times 255 is at most 65025; this match arm bounds alpha to 1..=254."
                )]
                let normalized = u16::from(*sample) * 255 / u16::from(alpha);
                *sample = normalized.min(255).to_le_bytes()[0];
            }
        }
    }
}

fn convert_pixels(
    dimensions: (u32, u32),
    mut pixels: Vec<u8>,
    layout: TiffLayout,
    endian: Endian,
    palette: Option<ImagePalette>,
    alpha: TiffAlpha,
) -> CodecResult<DecodedImage> {
    let (width, height) = dimensions;
    let image = match layout {
        TiffLayout::Bilevel { invert } => {
            if invert {
                let width = width as usize;
                let row_bytes = width.div_ceil(8);
                for row in pixels.chunks_exact_mut(row_bytes) {
                    row.iter_mut().for_each(|byte| *byte = !*byte);
                    if !width.is_multiple_of(8) {
                        let last = row_bytes.wrapping_sub(1);
                        let shift = 8_usize.saturating_sub(width % 8);
                        row[last] &= u8::MAX.wrapping_shl(shift.to_le_bytes()[0].into());
                    }
                }
            }
            DecodedImage::with_mode(width, height, pixels, ImageMode::L1)
        }
        TiffLayout::Gray8 { invert } => {
            if invert {
                pixels.iter_mut().for_each(|byte| *byte = !*byte);
            }
            DecodedImage::new(width, height, pixels, ColorType::L8)
        }
        TiffLayout::GrayAlpha8 => {
            if alpha.clear_gray_alpha {
                for pixel in pixels.as_chunks_mut::<2>().0 {
                    pixel[1] = 0;
                }
            }
            DecodedImage::with_mode(width, height, pixels, ImageMode::La8)
        }
        TiffLayout::GrayIgnored8 => {
            compact_extra_samples(&mut pixels, width as usize, 2, None)?;
            DecodedImage::new(width, height, pixels, ColorType::L8)
        }
        TiffLayout::Gray2 { invert } | TiffLayout::Gray4 { invert } => {
            let bits = if matches!(layout, TiffLayout::Gray2 { .. }) {
                2
            } else {
                4
            };
            let maximum = 1_u16.wrapping_shl(bits.into()).wrapping_sub(1);
            let output = unpack_indices(&pixels, width, height, bits)
                .into_iter()
                .map(|sample| {
                    let value = u16::from(sample)
                        .wrapping_mul(255)
                        .checked_div(maximum)
                        .unwrap_or_default()
                        .to_le_bytes()[0];
                    if invert {
                        255_u8.wrapping_sub(value)
                    } else {
                        value
                    }
                })
                .collect();
            DecodedImage::new(width, height, output, ColorType::L8)
        }
        TiffLayout::Gray16 => {
            let mut output = Vec::with_capacity(pixels.len());
            for bytes in pixels.as_chunks::<2>().0 {
                let value = endian.u16_exact([bytes[0], bytes[1]]);
                output.extend_from_slice(&value.to_le_bytes());
            }
            DecodedImage::new(width, height, output, ColorType::L16)
        }
        TiffLayout::I32 => DecodedImage::with_mode(width, height, pixels, ImageMode::I32),
        TiffLayout::F32 => DecodedImage::with_mode(width, height, pixels, ImageMode::F32),
        TiffLayout::Rgb8 => DecodedImage::new(width, height, pixels, ColorType::Rgb8),
        TiffLayout::Rgba8 => {
            if alpha.unassociate {
                for pixel in pixels.as_chunks_mut::<4>().0 {
                    unassociate_rgba_pixel(pixel);
                }
            }
            DecodedImage::new(width, height, pixels, ColorType::Rgba8)
        }
        TiffLayout::Palette { bits } => {
            let indices = unpack_indices(&pixels, width, height, bits);
            let image = DecodedImage::with_mode(width, height, indices, ImageMode::P8);
            if let Some(palette) = palette {
                image.with_palette(palette)
            } else {
                image
            }
        }
        TiffLayout::Cmyk8 => DecodedImage::new(width, height, pixels, ColorType::Cmyk8),
        TiffLayout::RgbIgnored8 => {
            compact_extra_samples(&mut pixels, width as usize, 4, None)?;
            DecodedImage::new(width, height, pixels, ColorType::Rgb8)
        }
        TiffLayout::Ycbcr8 => {
            let mut rgb = Vec::with_capacity((pixels.len() / 4).wrapping_mul(3));
            for pixel in pixels.as_chunks::<4>().0 {
                rgb.extend_from_slice(&pixel[..3]);
            }
            DecodedImage::new(width, height, rgb, ColorType::Rgb8)
        }
        TiffLayout::YcbcrFull8 { conversion } => {
            convert_ycbcr_full_resolution(&mut pixels, (width, height), conversion, None)?;
            DecodedImage::new(width, height, pixels, ColorType::Rgb8)
        }
        TiffLayout::YcbcrSubsampled8 {
            conversion,
            subsampling,
        } => DecodedImage::new(
            width,
            height,
            convert_ycbcr_subsampled((width, height), &pixels, subsampling, conversion, None)?,
            ColorType::Rgb8,
        ),
    };
    let mut descriptor = SourceDescriptor::new().with_byte_order(endian.source_byte_order());
    if let Some(alpha) = alpha.source {
        descriptor = descriptor.with_alpha(alpha);
    }
    Ok(image.with_source_descriptor(descriptor))
}

/// Remove one ignored trailing sample using the checked assembly buffer.
///
/// Output positions precede their source positions, so overlapping copies
/// cannot overwrite a later stored pixel. The allocation and its capacity are
/// retained; only the initialized output length changes.
fn compact_extra_samples(
    pixels: &mut Vec<u8>,
    width: usize,
    stored_samples: usize,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    let retained_samples = match stored_samples {
        2 => 1,
        4 => 3,
        _ => {
            return Err(CodecError::Malformed(
                "TIFF ignored sample stride is invalid".to_owned(),
            ));
        }
    };
    let row_bytes = width
        .checked_mul(stored_samples)
        .dimensions("TIFF ignored sample row size overflows")?;
    if row_bytes == 0 || !pixels.len().is_multiple_of(row_bytes) {
        return Err(CodecError::Malformed(
            "TIFF ignored sample assembly size is invalid".to_owned(),
        ));
    }
    let mut destination = 0usize;
    for source in (0..pixels.len()).step_by(stored_samples) {
        if token.is_some() && (source.is_multiple_of(row_bytes) || source.is_multiple_of(1_024)) {
            crate::codecs::error::check_cancelled(token)?;
        }
        let end = source
            .checked_add(retained_samples)
            .dimensions("TIFF retained sample range overflows")?;
        pixels.copy_within(source..end, destination);
        destination = destination
            .checked_add(retained_samples)
            .dimensions("TIFF retained sample position overflows")?;
    }
    pixels.truncate(destination);
    Ok(())
}

fn convert_pixels_for_token(
    dimensions: (u32, u32),
    pixels: Vec<u8>,
    layout: TiffLayout,
    endian: Endian,
    palette: Option<ImagePalette>,
    alpha: TiffAlpha,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<DecodedImage> {
    match token {
        Some(token) => {
            convert_pixels_with_token(dimensions, pixels, layout, endian, palette, alpha, token)
        }
        None => convert_pixels(dimensions, pixels, layout, endian, palette, alpha),
    }
}

fn convert_pixels_with_token(
    dimensions: (u32, u32),
    mut pixels: Vec<u8>,
    layout: TiffLayout,
    endian: Endian,
    palette: Option<ImagePalette>,
    alpha: TiffAlpha,
    token: &crate::CancellationToken,
) -> CodecResult<DecodedImage> {
    let (width, height) = dimensions;
    let width_usize = width as usize;
    let image = match layout {
        TiffLayout::Bilevel { invert } => {
            if invert {
                let row_bytes = width_usize.div_ceil(8);
                let mut processed = 0usize;
                for row in pixels.chunks_exact_mut(row_bytes) {
                    crate::codecs::error::check_cancelled(Some(token))?;
                    for byte in row.iter_mut() {
                        *byte = !*byte;
                        processed = processed.saturating_add(1);
                        if processed.is_multiple_of(1_024) {
                            crate::codecs::error::check_cancelled(Some(token))?;
                        }
                    }
                    if !width_usize.is_multiple_of(8) {
                        let last = row_bytes.wrapping_sub(1);
                        let shift = 8_usize.saturating_sub(width_usize % 8);
                        row[last] &= u8::MAX.wrapping_shl(shift.to_le_bytes()[0].into());
                    }
                }
            }
            DecodedImage::with_mode(width, height, pixels, ImageMode::L1)
        }
        TiffLayout::Gray8 { invert } => {
            if invert {
                let mut processed = 0usize;
                for row in pixels.chunks_exact_mut(width_usize) {
                    crate::codecs::error::check_cancelled(Some(token))?;
                    for byte in row.iter_mut() {
                        *byte = !*byte;
                        processed = processed.saturating_add(1);
                        if processed.is_multiple_of(1_024) {
                            crate::codecs::error::check_cancelled(Some(token))?;
                        }
                    }
                }
            }
            DecodedImage::new(width, height, pixels, ColorType::L8)
        }
        TiffLayout::GrayAlpha8 => {
            if alpha.clear_gray_alpha {
                for chunk in pixels.chunks_mut(1_024) {
                    crate::codecs::error::check_cancelled(Some(token))?;
                    for pixel in chunk.as_chunks_mut::<2>().0 {
                        pixel[1] = 0;
                    }
                }
            }
            DecodedImage::with_mode(width, height, pixels, ImageMode::La8)
        }
        TiffLayout::GrayIgnored8 => {
            compact_extra_samples(&mut pixels, width_usize, 2, Some(token))?;
            DecodedImage::new(width, height, pixels, ColorType::L8)
        }
        TiffLayout::Gray2 { invert } | TiffLayout::Gray4 { invert } => {
            let bits = if matches!(layout, TiffLayout::Gray2 { .. }) {
                2
            } else {
                4
            };
            let maximum = 1_u16.wrapping_shl(bits.into()).wrapping_sub(1);
            let indices = unpack_indices_with_token(&pixels, width, height, bits, token)?;
            let mut output = Vec::with_capacity(indices.len());
            let mut processed = 0usize;
            for sample in indices {
                let value = u16::from(sample)
                    .wrapping_mul(255)
                    .checked_div(maximum)
                    .unwrap_or_default()
                    .to_le_bytes()[0];
                output.push(if invert {
                    255_u8.wrapping_sub(value)
                } else {
                    value
                });
                processed = processed.saturating_add(1);
                if processed.is_multiple_of(1_024) {
                    crate::codecs::error::check_cancelled(Some(token))?;
                }
            }
            DecodedImage::new(width, height, output, ColorType::L8)
        }
        TiffLayout::Gray16 => {
            let row_bytes = width_usize.wrapping_mul(2);
            let mut output = Vec::with_capacity(pixels.len());
            let mut processed = 0usize;
            for row in pixels.chunks_exact(row_bytes) {
                crate::codecs::error::check_cancelled(Some(token))?;
                for bytes in row.as_chunks::<2>().0 {
                    let value = endian.u16_exact([bytes[0], bytes[1]]);
                    output.extend_from_slice(&value.to_le_bytes());
                    processed = processed.saturating_add(2);
                    if processed.is_multiple_of(1_024) {
                        crate::codecs::error::check_cancelled(Some(token))?;
                    }
                }
            }
            DecodedImage::new(width, height, output, ColorType::L16)
        }
        TiffLayout::I32 => DecodedImage::with_mode(width, height, pixels, ImageMode::I32),
        TiffLayout::F32 => DecodedImage::with_mode(width, height, pixels, ImageMode::F32),
        TiffLayout::Rgb8 => DecodedImage::new(width, height, pixels, ColorType::Rgb8),
        TiffLayout::Rgba8 => {
            if alpha.unassociate {
                for chunk in pixels.chunks_mut(1_024) {
                    crate::codecs::error::check_cancelled(Some(token))?;
                    for pixel in chunk.as_chunks_mut::<4>().0 {
                        unassociate_rgba_pixel(pixel);
                    }
                }
            }
            DecodedImage::new(width, height, pixels, ColorType::Rgba8)
        }
        TiffLayout::Palette { bits } => {
            let indices = unpack_indices_with_token(&pixels, width, height, bits, token)?;
            let image = DecodedImage::with_mode(width, height, indices, ImageMode::P8);
            if let Some(palette) = palette {
                image.with_palette(palette)
            } else {
                image
            }
        }
        TiffLayout::Cmyk8 => DecodedImage::new(width, height, pixels, ColorType::Cmyk8),
        TiffLayout::RgbIgnored8 => {
            compact_extra_samples(&mut pixels, width_usize, 4, Some(token))?;
            DecodedImage::new(width, height, pixels, ColorType::Rgb8)
        }
        TiffLayout::Ycbcr8 => {
            let row_bytes = width_usize.wrapping_mul(4);
            let mut rgb = Vec::with_capacity((pixels.len() / 4).wrapping_mul(3));
            let mut processed = 0usize;
            for row in pixels.chunks_exact(row_bytes) {
                crate::codecs::error::check_cancelled(Some(token))?;
                for pixel in row.as_chunks::<4>().0 {
                    rgb.extend_from_slice(&pixel[..3]);
                    processed = processed.saturating_add(4);
                    if processed.is_multiple_of(1_024) {
                        crate::codecs::error::check_cancelled(Some(token))?;
                    }
                }
            }
            DecodedImage::new(width, height, rgb, ColorType::Rgb8)
        }
        TiffLayout::YcbcrFull8 { conversion } => {
            convert_ycbcr_full_resolution(&mut pixels, (width, height), conversion, Some(token))?;
            DecodedImage::new(width, height, pixels, ColorType::Rgb8)
        }
        TiffLayout::YcbcrSubsampled8 {
            conversion,
            subsampling,
        } => DecodedImage::new(
            width,
            height,
            convert_ycbcr_subsampled(
                (width, height),
                &pixels,
                subsampling,
                conversion,
                Some(token),
            )?,
            ColorType::Rgb8,
        ),
    };
    let mut descriptor = SourceDescriptor::new().with_byte_order(endian.source_byte_order());
    if let Some(alpha) = alpha.source {
        descriptor = descriptor.with_alpha(alpha);
    }
    Ok(image.with_source_descriptor(descriptor))
}

/// Expand supported contiguous TIFF YCbCr clumps using Pillow/libtiff's RGB path.
///
/// The 2-by-2 layout stays on its existing specialized path. Pillow-compatible
/// 1-by-2 clumps use a separate fixed-size path to keep the common case fast.
fn convert_ycbcr_subsampled(
    dimensions: (u32, u32),
    samples: &[u8],
    subsampling: (usize, usize),
    conversion: super::inspect::YcbcrConversion,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    match subsampling {
        (1, 2) => convert_ycbcr_subsampled_1x2(dimensions, samples, conversion, token),
        (2, 2) => convert_ycbcr_subsampled_2x2(dimensions, samples, conversion, token),
        _ => convert_ycbcr_subsampled_general(dimensions, samples, subsampling, conversion, token),
    }
}

/// Expand other Pillow-supported YCbCr clump dimensions without affecting the
/// specialized 1-by-2 and 2-by-2 paths.
fn convert_ycbcr_subsampled_general(
    dimensions: (u32, u32),
    samples: &[u8],
    subsampling: (usize, usize),
    conversion: super::inspect::YcbcrConversion,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    match YcbcrTables::new(conversion)?.as_ref() {
        Some(tables) => convert_ycbcr_subsampled_general_with(
            dimensions,
            samples,
            subsampling,
            token,
            |y, cb, cr| tables.to_rgb(y, cb, cr),
        ),
        None => convert_ycbcr_subsampled_general_with(
            dimensions,
            samples,
            subsampling,
            token,
            ycbcr_to_rgb_pixel,
        ),
    }
}

fn convert_ycbcr_subsampled_general_with(
    dimensions: (u32, u32),
    samples: &[u8],
    subsampling: (usize, usize),
    token: Option<&crate::CancellationToken>,
    convert_pixel: impl Fn(u8, u8, u8) -> [u8; 3],
) -> CodecResult<Vec<u8>> {
    let (width, height) = dimensions;
    let width = width as usize;
    let height = height as usize;
    let (horizontal, vertical) = subsampling;
    if horizontal == 0 || vertical == 0 {
        return Err(CodecError::Malformed(
            "unsupported TIFF YCbCr subsampling".to_owned(),
        ));
    }

    let clump_columns = width.div_ceil(horizontal);
    let clump_rows = height.div_ceil(vertical);
    let luma_samples_per_clump = horizontal
        .checked_mul(vertical)
        .dimensions("TIFF YCbCr clump size overflows")?;
    let clump_size = luma_samples_per_clump
        .checked_add(2)
        .dimensions("TIFF YCbCr clump size overflows")?;
    let clump_count = clump_columns
        .checked_mul(clump_rows)
        .dimensions("TIFF YCbCr clump count overflows")?;
    let expected_sample_len = clump_count
        .checked_mul(clump_size)
        .dimensions("TIFF YCbCr clump buffer size overflows")?;
    #[cfg(coverage)]
    let fault_injected = crate::coverage_support::take_fault_point(
        crate::coverage_support::CoverageFaultPoint::TiffYcbcrClumpLengthMismatch,
    );
    #[cfg(not(coverage))]
    let fault_injected = false;
    let available_len = if fault_injected {
        samples.len().saturating_sub(1)
    } else {
        samples.len()
    };
    let (available_samples, _) = samples.split_at(available_len);
    if available_samples.len() != expected_sample_len {
        return Err(CodecError::Malformed(
            "TIFF YCbCr clump buffer has an invalid length".to_owned(),
        ));
    }

    let output_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(3))
        .dimensions("TIFF YCbCr RGB buffer size overflows")?;
    let mut rgb = Vec::new();
    rgb.try_reserve_exact(output_len).map_err(|_| {
        CodecError::Dimensions("unable to allocate TIFF YCbCr RGB buffer".to_owned())
    })?;
    rgb.resize(output_len, 0);

    let mut processed = 0usize;
    for clump_y in 0..clump_rows {
        crate::codecs::error::check_cancelled(token)?;
        let first_y = clump_y
            .checked_mul(vertical)
            .dimensions("TIFF YCbCr row offset overflows")?;
        for clump_x in 0..clump_columns {
            let clump_index = clump_y
                .checked_mul(clump_columns)
                .and_then(|row| row.checked_add(clump_x))
                .dimensions("TIFF YCbCr clump offset overflows")?;
            let clump_start = clump_index
                .checked_mul(clump_size)
                .dimensions("TIFF YCbCr clump offset overflows")?;
            let clump_end = clump_start
                .checked_add(clump_size)
                .dimensions("TIFF YCbCr clump offset overflows")?;
            let clump = available_samples
                .get(clump_start..clump_end)
                .ok_or_else(|| CodecError::Malformed("TIFF YCbCr clump is missing".to_owned()))?;
            let chroma_cb_index = luma_samples_per_clump;
            let chroma_cr_index = chroma_cb_index
                .checked_add(1)
                .dimensions("TIFF YCbCr chroma offset overflows")?;
            let cb = *clump.get(chroma_cb_index).ok_or_else(|| {
                CodecError::Malformed("TIFF YCbCr Cb sample is missing".to_owned())
            })?;
            let cr = *clump.get(chroma_cr_index).ok_or_else(|| {
                CodecError::Malformed("TIFF YCbCr Cr sample is missing".to_owned())
            })?;
            let first_x = clump_x
                .checked_mul(horizontal)
                .dimensions("TIFF YCbCr column offset overflows")?;

            for local_y in 0..vertical {
                let y = first_y
                    .checked_add(local_y)
                    .dimensions("TIFF YCbCr row offset overflows")?;
                if y >= height {
                    continue;
                }
                for local_x in 0..horizontal {
                    let x = first_x
                        .checked_add(local_x)
                        .dimensions("TIFF YCbCr column offset overflows")?;
                    if x >= width {
                        continue;
                    }
                    let luma_index = local_y
                        .checked_mul(horizontal)
                        .and_then(|row| row.checked_add(local_x))
                        .dimensions("TIFF YCbCr luma offset overflows")?;
                    let y_sample = *clump.get(luma_index).ok_or_else(|| {
                        CodecError::Malformed("TIFF YCbCr luma sample is missing".to_owned())
                    })?;
                    let pixel = convert_pixel(y_sample, cb, cr);
                    let pixel_index = y
                        .checked_mul(width)
                        .and_then(|row| row.checked_add(x))
                        .and_then(|pixel| pixel.checked_mul(3))
                        .dimensions("TIFF YCbCr RGB offset overflows")?;
                    let output_end = pixel_index
                        .checked_add(3)
                        .dimensions("TIFF YCbCr RGB offset overflows")?;
                    let output = rgb.get_mut(pixel_index..output_end).ok_or_else(|| {
                        CodecError::Malformed("TIFF YCbCr RGB pixel is out of bounds".to_owned())
                    })?;
                    output.copy_from_slice(&pixel);
                    processed = processed.saturating_add(1);
                    if processed.is_multiple_of(1_024) {
                        crate::codecs::error::check_cancelled(token)?;
                    }
                }
            }
        }
    }

    Ok(rgb)
}

/// Expand contiguous TIFF YCbCr 2-by-2 clumps using Pillow/libtiff's RGB path.
///
/// Each clump stores four luma samples followed by one Cb and one Cr sample.
/// Libtiff applies the clump chroma pair to each luma sample and uses its
/// 16-bit fixed-point default YCbCr conversion coefficients.
fn convert_ycbcr_subsampled_2x2(
    dimensions: (u32, u32),
    samples: &[u8],
    conversion: super::inspect::YcbcrConversion,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    match YcbcrTables::new(conversion)?.as_ref() {
        Some(tables) => {
            convert_ycbcr_subsampled_2x2_with(dimensions, samples, token, |y, cb, cr| {
                tables.to_rgb(y, cb, cr)
            })
        }
        None => convert_ycbcr_subsampled_2x2_with(dimensions, samples, token, ycbcr_to_rgb_pixel),
    }
}

fn convert_ycbcr_subsampled_2x2_with(
    dimensions: (u32, u32),
    samples: &[u8],
    token: Option<&crate::CancellationToken>,
    convert_pixel: impl Fn(u8, u8, u8) -> [u8; 3],
) -> CodecResult<Vec<u8>> {
    let (width, height) = dimensions;
    let width = width as usize;
    let height = height as usize;
    let clump_columns = width.div_ceil(2);
    let clump_rows = height.div_ceil(2);
    let clump_count = clump_columns
        .checked_mul(clump_rows)
        .dimensions("TIFF YCbCr clump count overflows")?;
    #[cfg(coverage)]
    let fault_injected = crate::coverage_support::take_fault_point(
        crate::coverage_support::CoverageFaultPoint::TiffYcbcrClumpLengthMismatch,
    );
    #[cfg(not(coverage))]
    let fault_injected = false;
    let available_len = if fault_injected {
        samples.len().saturating_sub(1)
    } else {
        samples.len()
    };
    let (available_samples, _) = samples.split_at(available_len);
    let expected_sample_len = clump_count
        .checked_mul(6)
        .dimensions("TIFF YCbCr clump buffer size overflows")?;
    if available_samples.len() != expected_sample_len {
        return Err(CodecError::Malformed(
            "TIFF YCbCr clump buffer has an invalid length".to_owned(),
        ));
    }
    let (clumps, _) = available_samples.as_chunks::<6>();
    let output_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(3))
        .dimensions("TIFF YCbCr RGB buffer size overflows")?;
    let mut rgb = Vec::new();
    #[cfg(coverage)]
    let reservation = if crate::coverage_support::take_fault_point(
        crate::coverage_support::CoverageFaultPoint::TiffYcbcrRgbOutputReservation,
    ) {
        usize::MAX
    } else {
        output_len
    };
    #[cfg(not(coverage))]
    let reservation = output_len;
    rgb.try_reserve_exact(reservation).map_err(|_| {
        CodecError::Dimensions("unable to allocate TIFF YCbCr RGB buffer".to_owned())
    })?;
    rgb.resize(output_len, 0);

    let mut processed = 0usize;
    for clump_y in 0..clump_rows {
        crate::codecs::error::check_cancelled(token)?;
        let first_y = clump_y
            .checked_mul(2)
            .dimensions("TIFF YCbCr row offset overflows")?;
        for clump_x in 0..clump_columns {
            let clump_index = clump_y
                .checked_mul(clump_columns)
                .and_then(|row| row.checked_add(clump_x))
                .dimensions("TIFF YCbCr clump offset overflows")?;
            let clump = clumps
                .get(clump_index)
                .ok_or_else(|| CodecError::Malformed("TIFF YCbCr clump is missing".to_owned()))?;
            let first_x = clump_x
                .checked_mul(2)
                .dimensions("TIFF YCbCr column offset overflows")?;
            let cb = clump[4];
            let cr = clump[5];
            let y_samples = [[clump[0], clump[1]], [clump[2], clump[3]]];
            for (local_y, row) in y_samples.iter().enumerate() {
                let y = first_y
                    .checked_add(local_y)
                    .dimensions("TIFF YCbCr row offset overflows")?;
                if y >= height {
                    continue;
                }
                for (local_x, &y_sample) in row.iter().enumerate() {
                    let x = first_x
                        .checked_add(local_x)
                        .dimensions("TIFF YCbCr column offset overflows")?;
                    if x >= width {
                        continue;
                    }
                    let pixel = convert_pixel(y_sample, cb, cr);
                    let pixel_index = y
                        .checked_mul(width)
                        .and_then(|row| row.checked_add(x))
                        .and_then(|pixel| pixel.checked_mul(3))
                        .dimensions("TIFF YCbCr RGB offset overflows")?;
                    let output_end = pixel_index
                        .checked_add(3)
                        .dimensions("TIFF YCbCr RGB offset overflows")?;
                    let output = rgb.get_mut(pixel_index..output_end).ok_or_else(|| {
                        CodecError::Malformed("TIFF YCbCr RGB pixel is out of bounds".to_owned())
                    })?;
                    output.copy_from_slice(&pixel);
                    processed = processed.saturating_add(1);
                    if processed.is_multiple_of(1_024) {
                        crate::codecs::error::check_cancelled(token)?;
                    }
                }
            }
        }
    }
    Ok(rgb)
}

/// Expand 1-by-2 clumps, each containing two luma samples and one chroma pair.
fn convert_ycbcr_subsampled_1x2(
    dimensions: (u32, u32),
    samples: &[u8],
    conversion: super::inspect::YcbcrConversion,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    match YcbcrTables::new(conversion)?.as_ref() {
        Some(tables) => {
            convert_ycbcr_subsampled_1x2_with(dimensions, samples, token, |y, cb, cr| {
                tables.to_rgb(y, cb, cr)
            })
        }
        None => convert_ycbcr_subsampled_1x2_with(dimensions, samples, token, ycbcr_to_rgb_pixel),
    }
}

fn convert_ycbcr_subsampled_1x2_with(
    dimensions: (u32, u32),
    samples: &[u8],
    token: Option<&crate::CancellationToken>,
    convert_pixel: impl Fn(u8, u8, u8) -> [u8; 3],
) -> CodecResult<Vec<u8>> {
    let (width, height) = dimensions;
    let width = width as usize;
    let height = height as usize;
    let clump_rows = height.div_ceil(2);
    let clump_count = width
        .checked_mul(clump_rows)
        .dimensions("TIFF YCbCr clump count overflows")?;
    #[cfg(coverage)]
    let fault_injected = crate::coverage_support::take_fault_point(
        crate::coverage_support::CoverageFaultPoint::TiffYcbcrClumpLengthMismatch,
    );
    #[cfg(not(coverage))]
    let fault_injected = false;
    let available_len = if fault_injected {
        samples.len().saturating_sub(1)
    } else {
        samples.len()
    };
    let (available_samples, _) = samples.split_at(available_len);
    let expected_sample_len = clump_count
        .checked_mul(4)
        .dimensions("TIFF YCbCr clump buffer size overflows")?;
    if available_samples.len() != expected_sample_len {
        return Err(CodecError::Malformed(
            "TIFF YCbCr clump buffer has an invalid length".to_owned(),
        ));
    }
    let (clumps, _) = available_samples.as_chunks::<4>();
    let output_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(3))
        .dimensions("TIFF YCbCr RGB buffer size overflows")?;
    let mut rgb = Vec::new();
    #[cfg(coverage)]
    let reservation = if crate::coverage_support::take_fault_point(
        crate::coverage_support::CoverageFaultPoint::TiffYcbcrRgbOutputReservation,
    ) {
        usize::MAX
    } else {
        output_len
    };
    #[cfg(not(coverage))]
    let reservation = output_len;
    rgb.try_reserve_exact(reservation).map_err(|_| {
        CodecError::Dimensions("unable to allocate TIFF YCbCr RGB buffer".to_owned())
    })?;
    rgb.resize(output_len, 0);

    let mut processed = 0usize;
    for clump_y in 0..clump_rows {
        crate::codecs::error::check_cancelled(token)?;
        let first_y = clump_y
            .checked_mul(2)
            .dimensions("TIFF YCbCr row offset overflows")?;
        for x in 0..width {
            let clump_index = clump_y
                .checked_mul(width)
                .and_then(|row| row.checked_add(x))
                .dimensions("TIFF YCbCr clump offset overflows")?;
            let [first_luma, second_luma, cb, cr] = *clumps
                .get(clump_index)
                .ok_or_else(|| CodecError::Malformed("TIFF YCbCr clump is missing".to_owned()))?;
            for (local_y, y_sample) in [first_luma, second_luma].into_iter().enumerate() {
                let y = first_y
                    .checked_add(local_y)
                    .dimensions("TIFF YCbCr row offset overflows")?;
                if y >= height {
                    continue;
                }
                let pixel = convert_pixel(y_sample, cb, cr);
                let pixel_index = y
                    .checked_mul(width)
                    .and_then(|row| row.checked_add(x))
                    .and_then(|pixel| pixel.checked_mul(3))
                    .dimensions("TIFF YCbCr RGB offset overflows")?;
                let output_end = pixel_index
                    .checked_add(3)
                    .dimensions("TIFF YCbCr RGB offset overflows")?;
                let output = rgb.get_mut(pixel_index..output_end).ok_or_else(|| {
                    CodecError::Malformed("TIFF YCbCr RGB pixel is out of bounds".to_owned())
                })?;
                output.copy_from_slice(&pixel);
                processed = processed.saturating_add(1);
                if processed.is_multiple_of(1_024) {
                    crate::codecs::error::check_cancelled(token)?;
                }
            }
        }
    }
    Ok(rgb)
}

/// Convert full-resolution YCbCr triples into RGB in place.
fn convert_ycbcr_full_resolution(
    samples: &mut [u8],
    dimensions: (u32, u32),
    conversion: super::inspect::YcbcrConversion,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    match YcbcrTables::new(conversion)?.as_ref() {
        Some(tables) => {
            convert_ycbcr_full_resolution_with(samples, dimensions, token, |y, cb, cr| {
                tables.to_rgb(y, cb, cr)
            })
        }
        None => convert_ycbcr_full_resolution_with(samples, dimensions, token, ycbcr_to_rgb_pixel),
    }
}

fn convert_ycbcr_full_resolution_with(
    samples: &mut [u8],
    dimensions: (u32, u32),
    token: Option<&crate::CancellationToken>,
    convert_pixel: impl Fn(u8, u8, u8) -> [u8; 3],
) -> CodecResult<()> {
    let (width, height) = dimensions;
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .dimensions("TIFF YCbCr pixel count overflows")?;
    let expected_len = pixel_count
        .checked_mul(3)
        .dimensions("TIFF YCbCr sample buffer size overflows")?;
    #[cfg(coverage)]
    let fault_injected = crate::coverage_support::take_fault_point(
        crate::coverage_support::CoverageFaultPoint::TiffYcbcrFullResolutionLengthMismatch,
    );
    #[cfg(not(coverage))]
    let fault_injected = false;
    let available_len = if fault_injected {
        samples.len().saturating_sub(1)
    } else {
        samples.len()
    };
    let (samples, _) = samples.split_at_mut(available_len);
    if samples.len() != expected_len {
        return Err(CodecError::Malformed(
            "TIFF YCbCr sample buffer has an invalid length".to_owned(),
        ));
    }

    let (pixels, _) = samples.as_chunks_mut::<3>();
    for (index, pixel) in pixels.iter_mut().enumerate() {
        if index.is_multiple_of(1_024) {
            crate::codecs::error::check_cancelled(token)?;
        }
        let [y, cb, cr] = [pixel[0], pixel[1], pixel[2]];
        pixel.copy_from_slice(&convert_pixel(y, cb, cr));
    }
    Ok(())
}

/// Precomputed 8-bit conversion tables for non-default TIFF YCbCr tags.
struct YcbcrTables {
    red_from_cr: [i32; 256],
    green_from_cb: [i32; 256],
    green_from_cr: [i32; 256],
    blue_from_cb: [i32; 256],
    luma: [i32; 256],
}

impl YcbcrTables {
    /// Build libtiff-compatible tables once for a page, outside its pixel loop.
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "Validated TIFF rationals and libtiff's [-4096, 4096] table bounds keep all fixed-point products inside i32."
    )]
    fn new(conversion: super::inspect::YcbcrConversion) -> CodecResult<Option<Self>> {
        if conversion.invalid_coefficients {
            return Err(CodecError::Malformed(
                "TIFF YCbCr coefficients have an unsupported field type".to_owned(),
            ));
        }

        if conversion.is_default {
            return Ok(None);
        }

        let coefficients = conversion.floating_coefficients.map_or_else(
            || conversion.coefficients.map(rational_to_f32),
            |values| values.map(ycbcr_float_to_f32),
        );
        let references = conversion.floating_reference_black_white.map_or_else(
            || conversion.reference_black_white.map(rational_to_f32),
            |values| values.map(ycbcr_float_to_f32),
        );

        if coefficients[1] == 0.0 {
            return Err(CodecError::Malformed(
                "TIFF YCbCr green coefficient must be nonzero".to_owned(),
            ));
        }

        let reference_exceeds_limit = conversion.floating_reference_black_white.map_or_else(
            || {
                conversion
                    .reference_black_white
                    .iter()
                    .copied()
                    .any(ycbcr_reference_exceeds_conversion_limit)
            },
            |values| {
                values
                    .iter()
                    .any(|value| !value.is_finite() || *value >= 2_147_483_584.0)
            },
        );
        if reference_exceeds_limit {
            return Err(CodecError::Malformed(
                "TIFF YCbCr reference value exceeds conversion limits".to_owned(),
            ));
        }

        if conversion
            .floating_coefficients
            .is_some_and(|values| values.iter().any(|value| value.is_nan()))
        {
            return Err(CodecError::Malformed(
                "TIFF YCbCr coefficients contain NaN".to_owned(),
            ));
        }
        let [red_luma, green_luma, blue_luma] = coefficients;
        // libtiff maps zero-numerator and zero-denominator rationals to zero.

        let red_factor = 2.0_f32 - 2.0_f32 * red_luma;
        let green_from_red_factor = red_luma * red_factor / green_luma;
        let blue_factor = 2.0_f32 - 2.0_f32 * blue_luma;
        let green_from_blue_factor = blue_luma * blue_factor / green_luma;
        let red_scale = fixed_16_16(red_factor);
        let green_from_red_scale = -fixed_16_16(green_from_red_factor);
        let blue_scale = fixed_16_16(blue_factor);
        let green_from_blue_scale = -fixed_16_16(green_from_blue_factor);

        let mut tables = Self {
            red_from_cr: [0; 256],
            green_from_cb: [0; 256],
            green_from_cr: [0; 256],
            blue_from_cb: [0; 256],
            luma: [0; 256],
        };
        for sample_code in u8::MIN..=u8::MAX {
            let index = usize::from(sample_code);
            let sample = i32::from(sample_code);
            let chroma_code = sample - 128;
            let cr = libtiff_code_to_value(
                chroma_code,
                references[4] - 128.0,
                references[5] - 128.0,
                127.0,
            )?;
            let cb = libtiff_code_to_value(
                chroma_code,
                references[2] - 128.0,
                references[3] - 128.0,
                127.0,
            )?;
            tables.red_from_cr[index] = (red_scale * cr + (1 << 15)) >> 16;
            tables.blue_from_cb[index] = (blue_scale * cb + (1 << 15)) >> 16;
            tables.green_from_cr[index] = green_from_red_scale * cr;
            tables.green_from_cb[index] = green_from_blue_scale * cb + (1 << 15);
            tables.luma[index] =
                libtiff_code_to_value(sample, references[0], references[1], 255.0)?;
        }
        Ok(Some(tables))
    }

    /// Convert one pixel using only the precomputed component tables.
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "Table construction bounds each component to [-4096, 4096] and all fixed-point channel sums fit i32."
    )]
    fn to_rgb(&self, y: u8, cb: u8, cr: u8) -> [u8; 3] {
        let y = self.luma[usize::from(y)];
        let cb_index = usize::from(cb);
        let cr_index = usize::from(cr);
        let red = (y + self.red_from_cr[cr_index]).clamp(0, 255);
        let green = (y + ((self.green_from_cb[cb_index] + self.green_from_cr[cr_index]) >> 16))
            .clamp(0, 255);
        let blue = (y + self.blue_from_cb[cb_index]).clamp(0, 255);
        [
            red.to_le_bytes()[0],
            green.to_le_bytes()[0],
            blue.to_le_bytes()[0],
        ]
    }
}

fn rational_to_f32((numerator, denominator): (i64, i64)) -> f32 {
    if numerator == 0 || denominator == 0 {
        0.0
    } else {
        numerator as f32 / denominator as f32
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "libtiff builds YCbCr conversion tables from FLOAT and DOUBLE tag values using f32 arithmetic"
)]
fn ycbcr_float_to_f32(value: f64) -> f32 {
    value as f32
}

fn ycbcr_reference_exceeds_conversion_limit(rational: (i64, i64)) -> bool {
    // Pillow's libtiff path rejects ratios at 2^31 - 64 during pixel loading.
    const REFERENCE_LIMIT: i128 = 2_147_483_584;
    let (numerator, denominator) = rational;
    if numerator == 0 || denominator == 0 {
        return false;
    }
    let mut numerator = i128::from(numerator);
    let mut denominator = i128::from(denominator);
    if numerator < 0 && denominator < 0 {
        let Some(positive_numerator) = numerator.checked_neg() else {
            return false;
        };
        let Some(positive_denominator) = denominator.checked_neg() else {
            return false;
        };
        numerator = positive_numerator;
        denominator = positive_denominator;
    } else if numerator <= 0 || denominator <= 0 {
        return false;
    }
    REFERENCE_LIMIT
        .checked_mul(denominator)
        .is_some_and(|limit| numerator >= limit)
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Clamping to [0, 2] bounds the rounded fixed-point value to 131,073, which fits i32."
)]
fn fixed_16_16(value: f32) -> i32 {
    ((value.clamp(0.0, 2.0) * (1 << 16) as f32) + 0.5).trunc() as i32
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Table construction bounds reference values before integer conversion, and the final conversion clamps to [-4096, 4096]."
)]
fn libtiff_code_to_value(
    code: i32,
    black: f32,
    white: f32,
    component_range: f32,
) -> CodecResult<i32> {
    let black_code = black.trunc() as i32;
    let code_offset = code.checked_sub(black_code).ok_or_else(|| {
        CodecError::Malformed("TIFF YCbCr sample offset exceeds conversion limits".to_owned())
    })?;
    let denominator = if white - black == 0.0 {
        1.0
    } else {
        white - black
    };
    let value = code_offset as f32 * component_range / denominator;
    Ok(value.clamp(-4096.0, 4096.0).trunc() as i32)
}

/// Apply libtiff's bounded 16-bit fixed-point conversion to one 8-bit sample.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "8-bit samples bound the fixed-point products and sums well within i32."
)]
fn ycbcr_to_rgb_pixel(y: u8, cb: u8, cr: u8) -> [u8; 3] {
    const RED_FROM_CR: i32 = 91_881;
    const GREEN_FROM_CR: i32 = -46_802;
    const BLUE_FROM_CB: i32 = 116_130;
    const GREEN_FROM_CB: i32 = -22_553;
    const ROUNDING: i32 = 1 << 15;
    const SHIFT: u32 = 16;

    let luma = i32::from(y);
    let blue_difference = i32::from(cb) - 128;
    let red_difference = i32::from(cr) - 128;
    let red = (luma + ((RED_FROM_CR * red_difference + ROUNDING) >> SHIFT)).clamp(0, 255);
    let green = (luma
        + ((GREEN_FROM_CB * blue_difference + ROUNDING + GREEN_FROM_CR * red_difference) >> SHIFT))
        .clamp(0, 255);
    let blue = (luma + ((BLUE_FROM_CB * blue_difference + ROUNDING) >> SHIFT)).clamp(0, 255);
    [
        red.to_le_bytes()[0],
        green.to_le_bytes()[0],
        blue.to_le_bytes()[0],
    ]
}

/// Map TIFF tag 338 (`ExtraSamples`) to the declared source alpha semantics.
///
/// TIFF defines 1 as associated (premultiplied) and 2 as unassociated
/// (straight) alpha; 0 and absent values remain unspecified.
pub(super) fn source_alpha_from_extra_samples(values: &[usize]) -> Option<SourceAlpha> {
    match values.first() {
        Some(1) => Some(SourceAlpha::Premultiplied),
        Some(2) => Some(SourceAlpha::Straight),
        _ => None,
    }
}

fn unpack_indices(data: &[u8], width: u32, height: u32, bits: u8) -> Vec<u8> {
    if bits == 8 {
        return data.to_vec();
    }
    // `TiffLayout::Palette` admits only packed depths 1, 2, 4, or the
    // separately returned byte-aligned depth 8.
    let width = width as usize;
    let height = height as usize;
    let bits = usize::from(bits);
    let stride = width.wrapping_mul(bits).div_ceil(8);
    let output_len = width.wrapping_mul(height);
    debug_assert_eq!(data.len(), stride.wrapping_mul(height));
    let mut output = Vec::with_capacity(output_len);
    for y in 0..height {
        let row_start = y.wrapping_mul(stride);
        let row = &data[row_start..row_start.wrapping_add(stride)];
        for x in 0..width {
            let bit = x.wrapping_mul(bits);
            let shift = 8_usize.saturating_sub(bits).saturating_sub(bit % 8);
            let mask = 1_u8
                .wrapping_shl(bits.to_le_bytes()[0].into())
                .wrapping_sub(1);
            output.push(row[bit / 8].wrapping_shr(shift.to_le_bytes()[0].into()) & mask);
        }
    }
    output
}

fn unpack_indices_with_token(
    data: &[u8],
    width: u32,
    height: u32,
    bits: u8,
    token: &crate::CancellationToken,
) -> CodecResult<Vec<u8>> {
    let width = width as usize;
    let height = height as usize;
    if bits == 8 {
        let mut output = Vec::with_capacity(width.wrapping_mul(height));
        let mut processed = 0usize;
        for row in data.chunks_exact(width) {
            crate::codecs::error::check_cancelled(Some(token))?;
            for &sample in row {
                output.push(sample);
                processed = processed.saturating_add(1);
                if processed.is_multiple_of(1_024) {
                    crate::codecs::error::check_cancelled(Some(token))?;
                }
            }
        }
        return Ok(output);
    }
    let bits = usize::from(bits);
    let stride = width.wrapping_mul(bits).div_ceil(8);
    let output_len = width.wrapping_mul(height);
    debug_assert_eq!(data.len(), stride.wrapping_mul(height));
    let mut output = Vec::with_capacity(output_len);
    let mut processed = 0usize;
    for y in 0..height {
        crate::codecs::error::check_cancelled(Some(token))?;
        let row_start = y.wrapping_mul(stride);
        let row = &data[row_start..row_start.wrapping_add(stride)];
        for x in 0..width {
            let bit = x.wrapping_mul(bits);
            let shift = 8_usize.saturating_sub(bits).saturating_sub(bit % 8);
            let mask = 1_u8
                .wrapping_shl(bits.to_le_bytes()[0].into())
                .wrapping_sub(1);
            output.push(row[bit / 8].wrapping_shr(shift.to_le_bytes()[0].into()) & mask);
            processed = processed.saturating_add(1);
            if processed.is_multiple_of(1_024) {
                crate::codecs::error::check_cancelled(Some(token))?;
            }
        }
    }
    Ok(output)
}

fn reverse_horizontal_predictor(
    data: &mut [u8],
    row_bytes: usize,
    samples: usize,
    bits: u8,
    endian: Endian,
) {
    match bits {
        8 => {
            for row in data.chunks_exact_mut(row_bytes) {
                for index in samples..row.len() {
                    row[index] = row[index].wrapping_add(row[index.wrapping_sub(samples)]);
                }
            }
        }
        16 => {
            let sample_stride = samples.wrapping_mul(2);
            for row in data.chunks_exact_mut(row_bytes) {
                for offset in (sample_stride..row.len()).step_by(2) {
                    let previous_offset = offset.wrapping_sub(sample_stride);
                    let previous = endian
                        .u16_exact([row[previous_offset], row[previous_offset.wrapping_add(1)]]);
                    let current = endian.u16_exact([row[offset], row[offset.wrapping_add(1)]]);
                    endian.write_u16(
                        current.wrapping_add(previous),
                        &mut row[offset..offset.wrapping_add(2)],
                    );
                }
            }
        }
        _ => {
            let sample_stride = samples.wrapping_mul(4);
            for row in data.chunks_exact_mut(row_bytes) {
                for offset in (sample_stride..row.len()).step_by(4) {
                    let previous_offset = offset.wrapping_sub(sample_stride);
                    let previous = endian.u32_exact([
                        row[previous_offset],
                        row[previous_offset.wrapping_add(1)],
                        row[previous_offset.wrapping_add(2)],
                        row[previous_offset.wrapping_add(3)],
                    ]);
                    let current = endian.u32_exact([
                        row[offset],
                        row[offset.wrapping_add(1)],
                        row[offset.wrapping_add(2)],
                        row[offset.wrapping_add(3)],
                    ]);
                    endian.write_u32(
                        current.wrapping_add(previous),
                        &mut row[offset..offset.wrapping_add(4)],
                    );
                }
            }
        }
    }
}

fn reverse_horizontal_predictor_with_token(
    data: &mut [u8],
    row_bytes: usize,
    samples: usize,
    bits: u8,
    endian: Endian,
    token: &crate::CancellationToken,
) -> CodecResult<()> {
    match bits {
        8 => {
            for row in data.chunks_exact_mut(row_bytes) {
                crate::codecs::error::check_cancelled(Some(token))?;
                let mut processed = 0usize;
                for index in samples..row.len() {
                    row[index] = row[index].wrapping_add(row[index.wrapping_sub(samples)]);
                    processed = processed.saturating_add(1);
                    if processed.is_multiple_of(1_024) {
                        crate::codecs::error::check_cancelled(Some(token))?;
                    }
                }
            }
        }
        16 => {
            let sample_stride = samples.wrapping_mul(2);
            for row in data.chunks_exact_mut(row_bytes) {
                crate::codecs::error::check_cancelled(Some(token))?;
                let mut processed = 0usize;
                for offset in (sample_stride..row.len()).step_by(2) {
                    let previous_offset = offset.wrapping_sub(sample_stride);
                    let previous = endian
                        .u16_exact([row[previous_offset], row[previous_offset.wrapping_add(1)]]);
                    let current = endian
                        .u16_exact([row[offset], row[offset.wrapping_add(1)]])
                        .wrapping_add(previous);
                    endian.write_u16(current, &mut row[offset..offset.wrapping_add(2)]);
                    processed = processed.saturating_add(2);
                    if processed.is_multiple_of(1_024) {
                        crate::codecs::error::check_cancelled(Some(token))?;
                    }
                }
            }
        }
        _ => {
            let sample_stride = samples.wrapping_mul(4);
            for row in data.chunks_exact_mut(row_bytes) {
                crate::codecs::error::check_cancelled(Some(token))?;
                let mut processed = 0usize;
                for offset in (sample_stride..row.len()).step_by(4) {
                    let previous_offset = offset.wrapping_sub(sample_stride);
                    let previous = endian.u32_exact([
                        row[previous_offset],
                        row[previous_offset.wrapping_add(1)],
                        row[previous_offset.wrapping_add(2)],
                        row[previous_offset.wrapping_add(3)],
                    ]);
                    let current = endian
                        .u32_exact([
                            row[offset],
                            row[offset.wrapping_add(1)],
                            row[offset.wrapping_add(2)],
                            row[offset.wrapping_add(3)],
                        ])
                        .wrapping_add(previous);
                    endian.write_u32(current, &mut row[offset..offset.wrapping_add(4)]);
                    processed = processed.saturating_add(4);
                    if processed.is_multiple_of(1_024) {
                        crate::codecs::error::check_cancelled(Some(token))?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn decode_packbits(data: &[u8], expected: usize) -> CodecResult<Vec<u8>> {
    let mut output = Vec::with_capacity(expected);
    let mut position = 0usize;
    while position < data.len() && output.len() < expected {
        let header = data[position].cast_signed();
        position = position.wrapping_add(1);
        match header {
            0..=127 => {
                let count = usize::from(header.cast_unsigned()).wrapping_add(1);
                let end = position.saturating_add(count);
                let packet = data
                    .get(position..end)
                    .malformed("TIFF PackBits literal packet is truncated")?;
                let remaining = expected.saturating_sub(output.len());
                output.extend_from_slice(&packet[..count.min(remaining)]);
                position = end;
            }
            -127..=-1 => {
                let count = usize::from(1_i16.wrapping_sub(i16::from(header)).cast_unsigned());
                let value = *data
                    .get(position)
                    .malformed("TIFF PackBits repeat packet is truncated")?;
                position = position.wrapping_add(1);
                output.resize(
                    output
                        .len()
                        .wrapping_add(count.min(expected.saturating_sub(output.len()))),
                    value,
                );
            }
            -128 => {}
        }
    }
    if output.len() == expected {
        Ok(output)
    } else {
        Err(CodecError::Malformed(
            "TIFF PackBits stream ended before the expected output".to_owned(),
        ))
    }
}

fn decode_packbits_with_token(
    data: &[u8],
    expected: usize,
    token: &crate::CancellationToken,
) -> CodecResult<Vec<u8>> {
    let mut output = Vec::with_capacity(expected);
    let mut position = 0usize;
    while position < data.len() && output.len() < expected {
        crate::codecs::error::check_cancelled(Some(token))?;
        let header = data[position].cast_signed();
        position = position.wrapping_add(1);
        match header {
            0..=127 => {
                let count = usize::from(header.cast_unsigned()).wrapping_add(1);
                let end = position.saturating_add(count);
                let packet = data
                    .get(position..end)
                    .malformed("TIFF PackBits literal packet is truncated")?;
                let remaining = expected.saturating_sub(output.len());
                output.extend_from_slice(&packet[..count.min(remaining)]);
                position = end;
            }
            -127..=-1 => {
                let count = usize::from(1_i16.wrapping_sub(i16::from(header)).cast_unsigned());
                let value = *data
                    .get(position)
                    .malformed("TIFF PackBits repeat packet is truncated")?;
                position = position.wrapping_add(1);
                output.resize(
                    output
                        .len()
                        .wrapping_add(count.min(expected.saturating_sub(output.len()))),
                    value,
                );
            }
            -128 => {}
        }
    }
    if output.len() == expected {
        Ok(output)
    } else {
        Err(CodecError::Malformed(
            "TIFF PackBits stream ended before the expected output".to_owned(),
        ))
    }
}

fn decode_lzw(data: &[u8], expected: usize) -> CodecResult<Vec<u8>> {
    const CLEAR: u16 = 256;
    const END: u16 = 257;
    const LIMIT: usize = 4096;
    let mut prefixes = [0u16; LIMIT];
    let mut suffixes = [0u8; LIMIT];
    for value in 0..256u16 {
        suffixes[usize::from(value)] = value.to_le_bytes()[0];
    }
    let mut stack = [0u8; LIMIT];
    let mut reader = MsbBits::new(data);
    let mut output = Vec::with_capacity(expected);
    let mut width = 9u8;
    let mut next_code = 258u16;
    let mut previous = None;

    loop {
        let code = reader.read(width)?;
        if code == CLEAR {
            width = 9;
            next_code = 258;
            previous = None;
            continue;
        }
        if code == END {
            // Every write returns immediately when it reaches `expected`, so
            // observing END here necessarily means the stream ended early.
            return Err(CodecError::Malformed(
                "TIFF LZW end code preceded the expected output".to_owned(),
            ));
        }
        let Some(old_code) = previous else {
            if code >= CLEAR || output.len() >= expected {
                return Err(CodecError::Malformed(
                    "TIFF LZW stream starts with an invalid code".to_owned(),
                ));
            }
            output.push(code.to_le_bytes()[0]);
            if output.len() == expected {
                return Ok(output);
            }
            previous = Some(code);
            continue;
        };

        let first = if code < next_code {
            append_lzw(
                code,
                &prefixes,
                &suffixes,
                &mut stack,
                &mut output,
                expected,
            )
        } else if code == next_code {
            let first = append_lzw(
                old_code,
                &prefixes,
                &suffixes,
                &mut stack,
                &mut output,
                expected,
            );
            if output.len() >= expected {
                return Ok(output);
            }
            output.push(first);
            first
        } else {
            return Err(CodecError::Malformed(
                "TIFF LZW code exceeds the current dictionary".to_owned(),
            ));
        };

        if output.len() == expected {
            return Ok(output);
        }

        if usize::from(next_code) < LIMIT {
            prefixes[usize::from(next_code)] = old_code;
            suffixes[usize::from(next_code)] = first;
            next_code = next_code.wrapping_add(1);
            if width < 12 && next_code == 1_u16.wrapping_shl(width.into()).wrapping_sub(1) {
                width = width.wrapping_add(1);
            }
        }
        previous = Some(code);
    }
}

fn decode_lzw_with_token(
    data: &[u8],
    expected: usize,
    token: &crate::CancellationToken,
) -> CodecResult<Vec<u8>> {
    const CLEAR: u16 = 256;
    const END: u16 = 257;
    const LIMIT: usize = 4096;
    let mut prefixes = [0u16; LIMIT];
    let mut suffixes = [0u8; LIMIT];
    for value in 0..256u16 {
        suffixes[usize::from(value)] = value.to_le_bytes()[0];
    }
    let mut stack = [0u8; LIMIT];
    let mut reader = MsbBits::new(data);
    let mut output = Vec::with_capacity(expected);
    let mut width = 9u8;
    let mut next_code = 258u16;
    let mut previous = None;

    loop {
        crate::codecs::error::check_cancelled(Some(token))?;
        let code = reader.read(width)?;
        if code == CLEAR {
            width = 9;
            next_code = 258;
            previous = None;
            continue;
        }
        if code == END {
            return Err(CodecError::Malformed(
                "TIFF LZW end code preceded the expected output".to_owned(),
            ));
        }
        let Some(old_code) = previous else {
            if code >= CLEAR || output.len() >= expected {
                return Err(CodecError::Malformed(
                    "TIFF LZW stream starts with an invalid code".to_owned(),
                ));
            }
            output.push(code.to_le_bytes()[0]);
            if output.len() == expected {
                return Ok(output);
            }
            previous = Some(code);
            continue;
        };

        let first = if code < next_code {
            append_lzw_with_token(
                code,
                &prefixes,
                &suffixes,
                &mut stack,
                &mut output,
                expected,
                token,
            )?
        } else if code == next_code {
            let first = append_lzw_with_token(
                old_code,
                &prefixes,
                &suffixes,
                &mut stack,
                &mut output,
                expected,
                token,
            )?;
            if output.len() >= expected {
                return Ok(output);
            }
            output.push(first);
            first
        } else {
            return Err(CodecError::Malformed(
                "TIFF LZW code exceeds the current dictionary".to_owned(),
            ));
        };

        if output.len() == expected {
            return Ok(output);
        }

        if usize::from(next_code) < LIMIT {
            prefixes[usize::from(next_code)] = old_code;
            suffixes[usize::from(next_code)] = first;
            next_code = next_code.wrapping_add(1);
            if width < 12 && next_code == 1_u16.wrapping_shl(width.into()).wrapping_sub(1) {
                width = width.wrapping_add(1);
            }
        }
        previous = Some(code);
    }
}

fn append_lzw(
    mut code: u16,
    prefixes: &[u16; 4096],
    suffixes: &[u8; 4096],
    stack: &mut [u8; 4096],
    output: &mut Vec<u8>,
    expected: usize,
) -> u8 {
    let mut count = 0usize;
    while code >= 256 {
        // New dictionary entries only reference an older code. The prefix
        // graph is therefore acyclic and has fewer than 4096 entries.
        stack[count] = suffixes[usize::from(code)];
        count = count.wrapping_add(1);
        code = prefixes[usize::from(code)];
    }
    let first = code.to_le_bytes()[0];
    stack[count] = first;
    count = count.wrapping_add(1);
    let remaining = expected.saturating_sub(output.len());
    output.extend(stack[..count].iter().rev().take(remaining));
    first
}

fn append_lzw_with_token(
    mut code: u16,
    prefixes: &[u16; 4096],
    suffixes: &[u8; 4096],
    stack: &mut [u8; 4096],
    output: &mut Vec<u8>,
    expected: usize,
    token: &crate::CancellationToken,
) -> CodecResult<u8> {
    let mut count = 0usize;
    while code >= 256 {
        stack[count] = suffixes[usize::from(code)];
        count = count.wrapping_add(1);
        code = prefixes[usize::from(code)];
    }
    let first = code.to_le_bytes()[0];
    stack[count] = first;
    count = count.wrapping_add(1);
    let remaining = expected.saturating_sub(output.len());
    for (index, &value) in stack[..count].iter().rev().take(remaining).enumerate() {
        output.push(value);
        if index.saturating_add(1).is_multiple_of(1_024) {
            crate::codecs::error::check_cancelled(Some(token))?;
        }
    }
    Ok(first)
}

struct MsbBits<'a> {
    data: &'a [u8],
    bit: usize,
}

impl<'a> MsbBits<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, bit: 0 }
    }

    fn read(&mut self, width: u8) -> CodecResult<u16> {
        debug_assert!(width <= 12);
        let end = self.bit.wrapping_add(usize::from(width));
        if end > self.data.len().saturating_mul(8) {
            return Err(CodecError::Malformed(
                "TIFF LZW stream ended before the expected output".to_owned(),
            ));
        }
        let mut value = 0u16;
        for _ in 0..width {
            value = value.wrapping_shl(1) | u16::from(data_bit_unchecked(self.data, self.bit));
            self.bit = self.bit.wrapping_add(1);
        }
        Ok(value)
    }
}

fn data_bit_unchecked(data: &[u8], bit: usize) -> u8 {
    data[bit / 8].wrapping_shr(7_usize.saturating_sub(bit % 8).to_le_bytes()[0].into()) & 1
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Endian {
    Little,
    Big,
}

impl Endian {
    pub(super) const fn source_byte_order(self) -> SourceByteOrder {
        match self {
            Self::Little => SourceByteOrder::Little,
            Self::Big => SourceByteOrder::Big,
        }
    }

    pub(super) fn u16_exact(self, bytes: [u8; 2]) -> u16 {
        match self {
            Endian::Little => u16::from_le_bytes(bytes),
            Endian::Big => u16::from_be_bytes(bytes),
        }
    }

    fn i16_exact(self, bytes: [u8; 2]) -> i16 {
        match self {
            Endian::Little => i16::from_le_bytes(bytes),
            Endian::Big => i16::from_be_bytes(bytes),
        }
    }

    pub(super) fn u32_exact(self, bytes: [u8; 4]) -> u32 {
        match self {
            Endian::Little => u32::from_le_bytes(bytes),
            Endian::Big => u32::from_be_bytes(bytes),
        }
    }

    fn u64_exact(self, bytes: [u8; 8]) -> u64 {
        match self {
            Endian::Little => u64::from_le_bytes(bytes),
            Endian::Big => u64::from_be_bytes(bytes),
        }
    }

    fn i32_exact(self, bytes: [u8; 4]) -> i32 {
        match self {
            Endian::Little => i32::from_le_bytes(bytes),
            Endian::Big => i32::from_be_bytes(bytes),
        }
    }

    fn write_u16(self, value: u16, destination: &mut [u8]) {
        let bytes = match self {
            Endian::Little => value.to_le_bytes(),
            Endian::Big => value.to_be_bytes(),
        };
        destination.copy_from_slice(&bytes);
    }

    fn write_u32(self, value: u32, destination: &mut [u8]) {
        let bytes = match self {
            Endian::Little => value.to_le_bytes(),
            Endian::Big => value.to_be_bytes(),
        };
        destination.copy_from_slice(&bytes);
    }
}

pub(super) struct Directory<'a> {
    data: &'a [u8],
    endian: Endian,
    fields: Vec<Field>,
    entries: Vec<Entry>,
    rationals: Vec<RationalEntry>,
    floating_entries: Vec<FloatingEntry>,
    fill_order_rational: (usize, (u32, u32)),
    next_offset: usize,
    pub(super) opaque_blocks: Vec<crate::types::OpaqueBlock>,
    pub(super) metadata: Vec<crate::types::OpaqueMetadata>,
}

#[derive(Clone, Copy)]
struct Field {
    tag: u16,
    field_type: u16,
    count: usize,
}

struct RationalEntry {
    tag: u16,
    count: usize,
    value_position: usize,
}

struct FloatingEntry {
    tag: u16,
    field_type: u16,
    count: usize,
    value_position: usize,
}

struct Entry {
    tag: u16,
    numeric_kind: NumericKind,
    count: usize,
    value_position: usize,
    inline_position: usize,
    byte_len: usize,
}

#[derive(Clone, Copy)]
enum NumericKind {
    Bytes,
    Shorts,
    Longs,
    SignedBytes,
    SignedShorts,
    SignedLongs,
}

fn signed_integer_as_usize(value: i64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

/// Tags the TIFF model interprets and therefore never retains as container
/// records.
const INTERPRETED_TAGS: [u16; 19] = [
    256, 257, 258, 259, 262, 266, 273, 277, 278, 279, 284, 317, 320, 322, 323, 324, 325, 338, 339,
];

/// Tags the model classifies as known metadata and retains as raw metadata
/// records.
const KNOWN_METADATA_TAGS: [u16; 6] = [270, 305, 306, 315, 33_432, 34_675];

fn validate_baseline_field_type(tag: u16, field_type: u16) -> CodecResult<()> {
    let dimension_name = match tag {
        256 => Some("width"),
        257 => Some("height"),
        322 => Some("tile width"),
        323 => Some("tile height"),
        _ => None,
    };
    if let Some(name) = dimension_name {
        if !matches!(field_type, 3 | 4) {
            return Err(CodecError::Dimensions(format!(
                "TIFF {name} has an invalid field type"
            )));
        }
    } else if matches!(tag, 258 | 259 | 262 | 277 | 320 | 339) && !matches!(field_type, 1 | 3 | 4) {
        return Err(CodecError::Malformed(format!(
            "TIFF tag {tag} has unsupported field type {field_type}"
        )));
    }
    Ok(())
}

fn validate_decode_field_types(directory: &Directory<'_>) -> CodecResult<()> {
    let raw_tiles = directory.field_type(324).is_some()
        && directory.one_or(259, COMPRESSION_NONE) == COMPRESSION_NONE;
    for tag in [273, 278, 279, 284, 317, 324, 325] {
        let field_type = directory.field_type(tag);
        let pillow_ignores_ascii_raw_tile_counts = tag == 325 && raw_tiles && field_type == Some(2);
        if field_type.is_some_and(|field_type| !matches!(field_type, 1 | 3 | 4))
            && !pillow_ignores_ascii_raw_tile_counts
        {
            return Err(CodecError::Malformed(format!(
                "TIFF tag {tag} has an unsupported field type"
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_fill_order(directory: &Directory<'_>) -> CodecResult<usize> {
    let Some(field_type) = directory.field_type(266) else {
        return Ok(1);
    };
    let fill_order = match field_type {
        3 | 4 => directory.one_or(266, 1),
        5 => {
            let (count, (numerator, denominator)) = directory.fill_order_rational;
            if count == 0 {
                return Ok(1);
            }
            if denominator == 0 {
                return Err(CodecError::Malformed(
                    "TIFF FillOrder rational denominator is zero".to_owned(),
                ));
            }
            let numerator = u64::from(numerator);
            let denominator = u64::from(denominator);
            if numerator == denominator {
                1
            } else if numerator == denominator * 2 {
                2
            } else {
                return Err(CodecError::Malformed(
                    "TIFF FillOrder has an unsupported value".to_owned(),
                ));
            }
        }
        _ => {
            return Err(CodecError::Malformed(
                "TIFF FillOrder has an unsupported field type".to_owned(),
            ));
        }
    };
    if !matches!(fill_order, 1 | 2) {
        return Err(CodecError::Malformed(
            "TIFF FillOrder has an unsupported value".to_owned(),
        ));
    }
    Ok(fill_order)
}

impl<'a> Directory<'a> {
    pub(super) fn parse(data: &'a [u8], offset: usize, endian: Endian) -> CodecResult<Self> {
        let count_end = offset
            .checked_add(2)
            .dimensions("TIFF directory entry-count range overflows")?;
        let count_bytes = need_slice(
            data,
            offset,
            count_end,
            "TIFF directory entry count is truncated",
        )?;
        let count = usize::from(endian.u16_exact([count_bytes[0], count_bytes[1]]));
        if count > 4096 {
            return Err(CodecError::Malformed(
                "TIFF directory contains too many entries".to_owned(),
            ));
        }
        let entries_start = offset.saturating_add(2);
        let mut fields = Vec::with_capacity(count);
        let mut entries = Vec::with_capacity(count);
        let mut rationals = Vec::new();
        let mut floating_entries = Vec::new();
        let mut fill_order_rational = None;
        let mut opaque_blocks = Vec::new();
        let mut metadata = Vec::new();
        for index in 0..count {
            let start = entries_start.saturating_add(index.saturating_mul(12));
            let bytes = need_slice(
                data,
                start,
                start.saturating_add(12),
                "TIFF directory entry is truncated",
            )?;
            let tag = endian.u16_exact([bytes[0], bytes[1]]);
            let field_type = endian.u16_exact([bytes[2], bytes[3]]);
            validate_baseline_field_type(tag, field_type)?;
            let value_count = endian.u32_exact([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
            fields.push(Field {
                tag,
                field_type,
                count: value_count,
            });
            let numeric_kind = match field_type {
                1 => Some(NumericKind::Bytes),
                3 => Some(NumericKind::Shorts),
                4 => Some(NumericKind::Longs),
                6 if tag == 530 => Some(NumericKind::SignedBytes),
                8 if tag == 530 => Some(NumericKind::SignedShorts),
                9 if tag == 530 => Some(NumericKind::SignedLongs),
                _ => None,
            };
            let type_size = match field_type {
                1 | 2 | 6 | 7 => 1,
                3 | 8 => 2,
                4 | 9 | 11 => 4,
                5 | 10 | 12 => 8,
                _ => continue,
            };
            #[cfg(target_pointer_width = "32")]
            let byte_len = value_count
                .checked_mul(type_size)
                .dimensions("TIFF directory value byte length overflows")?;
            #[cfg(not(target_pointer_width = "32"))]
            let byte_len = value_count.wrapping_mul(type_size);
            let value_position = if byte_len <= 4 {
                start.saturating_add(8)
            } else {
                endian.u32_exact([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize
            };
            #[cfg(target_pointer_width = "32")]
            {
                let value_end = value_position
                    .checked_add(byte_len)
                    .dimensions("TIFF directory value range overflows")?;
                need_slice(
                    data,
                    value_position,
                    value_end,
                    "TIFF directory value is out of bounds",
                )?;
            }
            #[cfg(not(target_pointer_width = "32"))]
            need_slice(
                data,
                value_position,
                value_position.saturating_add(byte_len),
                "TIFF directory value is out of bounds",
            )?;
            let raw_value = data[value_position..value_position.saturating_add(byte_len)].to_vec();
            if matches!(field_type, 5 | 10) {
                rationals.push(RationalEntry {
                    tag,
                    count: value_count,
                    value_position,
                });
            }
            if matches!(tag, 529 | 532) && matches!(field_type, 11 | 12) {
                floating_entries.push(FloatingEntry {
                    tag,
                    field_type,
                    count: value_count,
                    value_position,
                });
            }
            if tag == 266 && field_type == 5 {
                let rational = if value_count == 0 {
                    (0, 0)
                } else {
                    let value = raw_value.get(..8).ok_or_else(|| {
                        CodecError::Malformed(
                            "TIFF FillOrder rational value is truncated".to_owned(),
                        )
                    })?;
                    (
                        endian.u32_exact([value[0], value[1], value[2], value[3]]),
                        endian.u32_exact([value[4], value[5], value[6], value[7]]),
                    )
                };
                // Pillow keeps the last value when a TIFF directory repeats a tag.
                if let Some(previous) = fill_order_rational.as_mut() {
                    *previous = (value_count, rational);
                } else {
                    fill_order_rational = Some((value_count, rational));
                }
            }
            if !INTERPRETED_TAGS.contains(&tag) {
                let mut kind = [0u8; 2];
                endian.write_u16(tag, &mut kind);
                if KNOWN_METADATA_TAGS.contains(&tag) {
                    metadata.push(crate::types::OpaqueMetadata {
                        kind: kind.to_vec(),
                        data: raw_value,
                    });
                } else {
                    // TIFF defines no safe-to-copy bit; unknown tags are
                    // ignorable by baseline readers, so copying one cannot
                    // change interpreted semantics.
                    opaque_blocks.push(crate::types::OpaqueBlock {
                        kind: kind.to_vec(),
                        data: raw_value,
                        safe_to_copy: true,
                    });
                }
            }
            if let Some(numeric_kind) = numeric_kind {
                entries.push(Entry {
                    tag,
                    numeric_kind,
                    count: value_count,
                    value_position,
                    inline_position: start.saturating_add(8),
                    byte_len,
                });
            }
        }
        let next_position = entries_start.saturating_add(count.saturating_mul(12));
        let next = need_slice(
            data,
            next_position,
            next_position.saturating_add(4),
            "TIFF next-directory offset is truncated",
        )?;
        let mut directory = Self {
            data,
            endian,
            fields,
            entries,
            rationals,
            floating_entries,
            fill_order_rational: fill_order_rational.unwrap_or_default(),
            next_offset: endian.u32_exact([next[0], next[1], next[2], next[3]]) as usize,
            opaque_blocks,
            metadata,
        };
        if directory.one_or(262, 1) == 6 && directory.one_or(259, 1) != COMPRESSION_NONE {
            directory.opaque_blocks.retain(|block| {
                let Some(tag_bytes) = block.kind.get(..2) else {
                    return true;
                };
                let tag = endian.u16_exact([tag_bytes[0], tag_bytes[1]]);
                !matches!(tag, 529..=532)
            });
        }
        Ok(directory)
    }

    pub(super) fn next_offset(&self) -> usize {
        self.next_offset
    }

    pub(super) fn one(&self, tag: u16) -> Option<usize> {
        let entry = self.entries.iter().find(|entry| entry.tag == tag)?;
        if entry.count == 0 {
            return None;
        }
        let position = if entry.byte_len <= 4 {
            entry.inline_position
        } else {
            entry.value_position
        };
        match entry.numeric_kind {
            NumericKind::Bytes => self.data.get(position).copied().map(usize::from),
            NumericKind::SignedBytes => self
                .data
                .get(position)
                .copied()
                .map(|value| signed_integer_as_usize(i64::from(i8::from_ne_bytes([value])))),
            NumericKind::Shorts => {
                let bytes = self.data.get(position..position.saturating_add(2))?;
                Some(usize::from(self.endian.u16_exact([bytes[0], bytes[1]])))
            }
            NumericKind::SignedShorts => {
                let bytes = self.data.get(position..position.saturating_add(2))?;
                Some(signed_integer_as_usize(i64::from(
                    self.endian.i16_exact([bytes[0], bytes[1]]),
                )))
            }
            NumericKind::Longs => {
                let bytes = self.data.get(position..position.saturating_add(4))?;
                Some(
                    self.endian
                        .u32_exact([bytes[0], bytes[1], bytes[2], bytes[3]])
                        as usize,
                )
            }
            NumericKind::SignedLongs => {
                let bytes = self.data.get(position..position.saturating_add(4))?;
                Some(signed_integer_as_usize(i64::from(
                    self.endian
                        .i32_exact([bytes[0], bytes[1], bytes[2], bytes[3]]),
                )))
            }
        }
    }

    pub(super) fn one_or(&self, tag: u16, default: usize) -> usize {
        self.one(tag).unwrap_or(default)
    }

    pub(super) fn values_or(&self, tag: u16, default: &[usize]) -> Vec<usize> {
        self.values(tag).unwrap_or_else(|| default.to_vec())
    }

    pub(super) fn value_count(&self, tag: u16) -> Option<usize> {
        let field = self.fields.iter().find(|field| field.tag == tag)?;
        if matches!(field.field_type, 1 | 3 | 4)
            || (tag == 530 && matches!(field.field_type, 6 | 8 | 9))
            || (matches!(tag, 529 | 532) && matches!(field.field_type, 5 | 10 | 11 | 12))
        {
            Some(field.count)
        } else {
            None
        }
    }

    pub(super) fn field_count(&self, tag: u16) -> Option<usize> {
        self.fields
            .iter()
            .find(|field| field.tag == tag)
            .map(|field| field.count)
    }

    pub(super) fn values(&self, tag: u16) -> Option<Vec<usize>> {
        let entry = self.entries.iter().find(|entry| entry.tag == tag)?;
        let position = if entry.byte_len <= 4 {
            entry.inline_position
        } else {
            entry.value_position
        };
        let bytes = &self.data[position..position.saturating_add(entry.byte_len)];
        let mut values = Vec::with_capacity(entry.count);
        match entry.numeric_kind {
            NumericKind::Bytes => values.extend(bytes.iter().map(|&value| usize::from(value))),
            NumericKind::SignedBytes => values.extend(
                bytes
                    .iter()
                    .map(|&value| signed_integer_as_usize(i64::from(i8::from_ne_bytes([value])))),
            ),
            NumericKind::Shorts => {
                for chunk in bytes.as_chunks::<2>().0 {
                    values.push(usize::from(self.endian.u16_exact([chunk[0], chunk[1]])));
                }
            }
            NumericKind::SignedShorts => {
                for chunk in bytes.as_chunks::<2>().0 {
                    values.push(signed_integer_as_usize(i64::from(
                        self.endian.i16_exact([chunk[0], chunk[1]]),
                    )));
                }
            }
            NumericKind::Longs => {
                for chunk in bytes.as_chunks::<4>().0 {
                    values.push(
                        self.endian
                            .u32_exact([chunk[0], chunk[1], chunk[2], chunk[3]])
                            as usize,
                    );
                }
            }
            NumericKind::SignedLongs => {
                for chunk in bytes.as_chunks::<4>().0 {
                    values.push(signed_integer_as_usize(i64::from(
                        self.endian
                            .i32_exact([chunk[0], chunk[1], chunk[2], chunk[3]]),
                    )));
                }
            }
        }
        Some(values)
    }

    pub(super) fn field_type(&self, tag: u16) -> Option<u16> {
        self.fields
            .iter()
            .find(|field| field.tag == tag)
            .map(|field| field.field_type)
    }

    pub(super) fn rational(&self, tag: u16, index: usize) -> Option<(i64, i64)> {
        let entry = self.rationals.iter().find(|entry| entry.tag == tag)?;
        if index >= entry.count {
            return None;
        }
        let position = entry.value_position.checked_add(index.checked_mul(8)?)?;
        let value_end = position.checked_add(8)?;
        let value = self.data.get(position..value_end)?;
        let numerator_bytes = [value[0], value[1], value[2], value[3]];
        let denominator_bytes = [value[4], value[5], value[6], value[7]];
        // libtiff reads SRATIONAL numerators as signed but denominators as unsigned.
        if self.field_type(tag) == Some(10) {
            Some((
                i64::from(self.endian.i32_exact(numerator_bytes)),
                i64::from(self.endian.u32_exact(denominator_bytes)),
            ))
        } else {
            Some((
                i64::from(self.endian.u32_exact(numerator_bytes)),
                i64::from(self.endian.u32_exact(denominator_bytes)),
            ))
        }
    }

    pub(super) fn floating_value(&self, tag: u16, index: usize) -> Option<f64> {
        let entry = self
            .floating_entries
            .iter()
            .find(|entry| entry.tag == tag)?;
        if index >= entry.count {
            return None;
        }
        match entry.field_type {
            11 => {
                let position = entry.value_position.checked_add(index.checked_mul(4)?)?;
                let value = self.data.get(position..position.checked_add(4)?)?;
                let bits = self
                    .endian
                    .u32_exact([value[0], value[1], value[2], value[3]]);
                Some(f64::from(f32::from_bits(bits)))
            }
            12 => {
                let position = entry.value_position.checked_add(index.checked_mul(8)?)?;
                let value = self.data.get(position..position.checked_add(8)?)?;
                let bits = self.endian.u64_exact([
                    value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
                ]);
                Some(f64::from_bits(bits))
            }
            _ => None,
        }
    }
}
