//! Baseline decoder for classic TIFF IFD payloads.

use super::inspect::TiffLayout;
use crate::SequenceDecodeBudget;
use crate::codecs::compression::deflate::{
    decompress_zlib_prefix, decompress_zlib_prefix_with_status_and_token,
};
use crate::codecs::{CodecError, CodecResult, OptionCodecExt, need_slice};
use crate::types::{
    ColorType, DecodedFrame, DecodedImage, DecodedSequence, FrameBlend, FrameDisposal,
    FrameDuration, ImageMode, ImagePalette, SourceAlpha, SourceByteOrder, SourceColor,
    SourceDescriptor,
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
            loop_count: None,
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
        #[allow(clippy::arithmetic_side_effects)]
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
    #[allow(clippy::arithmetic_side_effects)]
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
    #[allow(clippy::arithmetic_side_effects)]
    let directory_end = ifd_offset
        .saturating_add(2)
        .saturating_add(directory.entries.len().saturating_mul(12))
        .saturating_add(4);
    validate_decode_field_types(&directory)?;

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
    let color_map = directory.values(320);
    if rows_per_strip == 0 {
        return Err(CodecError::Dimensions(
            "TIFF rows per strip must be nonzero".to_owned(),
        ));
    }
    if samples_per_pixel == 0 || planar != 1 || !matches!(predictor, 1 | 2) {
        return Err(CodecError::Malformed(
            "TIFF sample, strip, planar, or predictor fields are invalid".to_owned(),
        ));
    }
    let (layout, palette) = super::inspect::layout_and_palette(
        photometric,
        samples_per_pixel,
        usize::from(bits_per_sample),
        sample_format,
        color_map.as_deref(),
    )?;
    let alpha = directory
        .values(338)
        .as_deref()
        .and_then(source_alpha_from_extra_samples);
    if let Some(budget) = budget {
        budget
            .reserve_later_frame(layout.mode(), width, height)
            .map_err(CodecError::LimitExceeded)?;
    }

    let width_usize = width as usize;
    let height_usize = height as usize;
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
    #[allow(clippy::arithmetic_side_effects)]
    let row_samples = width_usize * stored_samples;
    #[allow(clippy::arithmetic_side_effects)]
    let row_bits = row_samples * usize::from(bits_per_sample);
    let row_bytes = row_bits.div_ceil(8);
    #[allow(clippy::arithmetic_side_effects)]
    let expected_total = row_bytes * height_usize;

    let tile_offsets = directory.values(324);
    let tile_byte_counts = directory.values(325);
    if tile_offsets.is_some() || tile_byte_counts.is_some() {
        let offsets = tile_offsets.malformed("TIFF tile offsets are missing")?;
        let byte_counts = tile_byte_counts.malformed("TIFF tile byte counts are missing")?;
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
        if bits_per_sample % 8 != 0 {
            return Err(CodecError::Malformed(
                "TIFF tile sample width is invalid".to_owned(),
            ));
        }
        let tiles_across = width_usize.div_ceil(tile_width);
        let tiles_down = height_usize.div_ceil(tile_height);
        #[cfg(target_pointer_width = "32")]
        let expected_tiles = tiles_across
            .checked_mul(tiles_down)
            .dimensions("TIFF tile count overflows")?;
        #[cfg(not(target_pointer_width = "32"))]
        let expected_tiles = tiles_across.wrapping_mul(tiles_down);
        if offsets.len() != expected_tiles {
            return Err(CodecError::Malformed(
                "TIFF tile offset count does not match its geometry".to_owned(),
            ));
        }
        // The validated layout has at most four 32-bit samples.
        #[allow(clippy::arithmetic_side_effects)]
        let bytes_per_pixel = samples_per_pixel * (usize::from(bits_per_sample) / 8);
        #[cfg(target_pointer_width = "32")]
        let tile_row_bytes = tile_width
            .checked_mul(bytes_per_pixel)
            .malformed("TIFF tile row byte size overflows")?;
        #[cfg(not(target_pointer_width = "32"))]
        #[allow(clippy::arithmetic_side_effects)]
        let tile_row_bytes = tile_width * bytes_per_pixel;
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
                .dimensions("TIFF tile byte range overflows")?;
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
            let mut decoded = decode_block(compression, encoded, tile_size, token)
                .map_err(|error| error.at(offset as u64, "tiff_tile"))?;
            // Every compressed decoder returns exactly the requested size, and
            // uncompressed tile counts were normalized to tile_size above.
            if uses_horizontal_predictor(predictor, compression, bits_per_sample) {
                reverse_horizontal_predictor(
                    &mut decoded,
                    tile_row_bytes,
                    samples_per_pixel,
                    bits_per_sample,
                    endian,
                );
            }
            let tile_column = tile_index.checked_rem(tiles_across).unwrap_or_default();
            let tile_row = tile_index.checked_div(tiles_across).unwrap_or_default();
            let tile_x = tile_column.wrapping_mul(tile_width);
            let tile_y = tile_row.wrapping_mul(tile_height);
            let copied_width = tile_width.min(width_usize.saturating_sub(tile_x));
            let copied_height = tile_height.min(height_usize.saturating_sub(tile_y));
            let copied_bytes = copied_width.wrapping_mul(bytes_per_pixel);
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
            convert_pixels((width, height), pixels, layout, endian, palette, alpha)
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
    // StripByteCounts is absent. Compressed strips also use the existing
    // offset-boundary derivation for an absent or empty tag.
    let declared_byte_counts = directory.values(279);
    if offsets.is_empty() {
        return Err(CodecError::Malformed(
            "TIFF strip offsets are empty".to_owned(),
        ));
    }
    let expected_strips = height_usize.div_ceil(rows_per_strip);
    if offsets.len() > expected_strips {
        return Err(CodecError::Malformed(
            "TIFF contains more strips than its geometry permits".to_owned(),
        ));
    }
    let byte_counts = if compression == COMPRESSION_NONE {
        (0..offsets.len())
            .map(|strip_index| {
                let first_row = strip_index.wrapping_mul(rows_per_strip);
                let strip_rows = rows_per_strip.min(height_usize.saturating_sub(first_row));
                row_bytes.wrapping_mul(strip_rows)
            })
            .collect::<Vec<_>>()
    } else {
        let declared_byte_counts = declared_byte_counts.unwrap_or_default();
        if declared_byte_counts.is_empty() {
            offsets
                .iter()
                .enumerate()
                .map(|(index, &offset)| {
                    let end = offsets.get(index.wrapping_add(1)).copied().unwrap_or(
                        if ifd_offset > offset {
                            ifd_offset
                        } else {
                            data.len()
                        },
                    );
                    end.checked_sub(offset)
                        .malformed("TIFF strip offsets are not monotonic")
                })
                .collect::<CodecResult<Vec<_>>>()?
        } else if offsets.len() != declared_byte_counts.len() {
            return Err(CodecError::Malformed(
                "TIFF strip offset and byte-count lengths differ".to_owned(),
            ));
        } else {
            declared_byte_counts
        }
    };
    let mut pixels = Vec::with_capacity(expected_total);

    for (strip_index, (&offset, &byte_count)) in offsets.iter().zip(&byte_counts).enumerate() {
        crate::codecs::error::check_cancelled(token)?;
        #[cfg(target_pointer_width = "32")]
        let encoded_end = offset
            .checked_add(byte_count)
            .dimensions("TIFF strip byte range overflows")?;
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
        let expected = row_bytes.wrapping_mul(strip_rows);
        let mut decoded = decode_block(compression, encoded, expected, token)
            .map_err(|error| error.at(offset as u64, "tiff_strip"))?;
        if uses_horizontal_predictor(predictor, compression, bits_per_sample) {
            reverse_horizontal_predictor(
                &mut decoded,
                row_bytes,
                samples_per_pixel,
                bits_per_sample,
                endian,
            );
        }
        pixels.extend_from_slice(&decoded);
    }
    pixels.resize(expected_total, 0);

    Ok((
        convert_pixels((width, height), pixels, layout, endian, palette, alpha)
            .with_opaque_blocks(std::mem::take(&mut directory.opaque_blocks))
            .with_metadata(std::mem::take(&mut directory.metadata)),
        next_offset,
        directory_end,
    ))
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
        COMPRESSION_NONE => Ok(encoded.to_vec()),
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
            inflated.map_err(|error| error.context("decode TIFF Deflate stream"))
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

fn convert_pixels(
    dimensions: (u32, u32),
    mut pixels: Vec<u8>,
    layout: TiffLayout,
    endian: Endian,
    palette: Option<ImagePalette>,
    alpha: Option<SourceAlpha>,
) -> DecodedImage {
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
        TiffLayout::GrayAlpha8 => DecodedImage::with_mode(width, height, pixels, ImageMode::La8),
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
            for bytes in pixels.chunks_exact(2) {
                let value = endian.u16_exact([bytes[0], bytes[1]]);
                output.extend_from_slice(&value.to_le_bytes());
            }
            DecodedImage::new(width, height, output, ColorType::L16)
        }
        TiffLayout::I32 => DecodedImage::with_mode(width, height, pixels, ImageMode::I32),
        TiffLayout::F32 => DecodedImage::with_mode(width, height, pixels, ImageMode::F32),
        TiffLayout::Rgb8 => DecodedImage::new(width, height, pixels, ColorType::Rgb8),
        TiffLayout::Rgba8 => DecodedImage::new(width, height, pixels, ColorType::Rgba8),
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
        TiffLayout::Ycbcr8 => {
            let mut rgb = Vec::with_capacity((pixels.len() / 4).wrapping_mul(3));
            for pixel in pixels.chunks_exact(4) {
                rgb.extend_from_slice(&pixel[..3]);
            }
            DecodedImage::new(width, height, rgb, ColorType::Rgb8)
        }
    };
    let mut descriptor = SourceDescriptor::new().with_byte_order(endian.source_byte_order());
    if let Some(alpha) = alpha {
        descriptor = descriptor.with_alpha(alpha);
    }
    image.with_source_descriptor(descriptor)
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

fn decode_packbits(data: &[u8], expected: usize) -> CodecResult<Vec<u8>> {
    let mut output = Vec::with_capacity(expected);
    let mut position = 0usize;
    while position < data.len() && output.len() < expected {
        let header = data[position] as i8;
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
        let header = data[position] as i8;
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

    pub(super) fn u32_exact(self, bytes: [u8; 4]) -> u32 {
        match self {
            Endian::Little => u32::from_le_bytes(bytes),
            Endian::Big => u32::from_be_bytes(bytes),
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
    next_offset: usize,
    pub(super) opaque_blocks: Vec<crate::types::OpaqueBlock>,
    pub(super) metadata: Vec<crate::types::OpaqueMetadata>,
}

#[derive(Clone, Copy)]
struct Field {
    tag: u16,
    field_type: u16,
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
}

/// Tags the TIFF model interprets and therefore never retains as container
/// records.
const INTERPRETED_TAGS: [u16; 18] = [
    256, 257, 258, 259, 262, 273, 277, 278, 279, 284, 317, 320, 322, 323, 324, 325, 338, 339,
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
    for tag in [273, 278, 279, 284, 317, 324, 325] {
        if directory
            .field_type(tag)
            .is_some_and(|field_type| !matches!(field_type, 1 | 3 | 4))
        {
            return Err(CodecError::Malformed(format!(
                "TIFF tag {tag} has an unsupported field type"
            )));
        }
    }
    Ok(())
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
            fields.push(Field { tag, field_type });
            let numeric_kind = match field_type {
                1 => Some(NumericKind::Bytes),
                3 => Some(NumericKind::Shorts),
                4 => Some(NumericKind::Longs),
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
        Ok(Self {
            data,
            endian,
            fields,
            entries,
            next_offset: endian.u32_exact([next[0], next[1], next[2], next[3]]) as usize,
            opaque_blocks,
            metadata,
        })
    }

    pub(super) fn next_offset(&self) -> usize {
        self.next_offset
    }

    pub(super) fn one(&self, tag: u16) -> Option<usize> {
        self.values(tag)
            .and_then(|values| values.into_iter().next())
    }

    pub(super) fn one_or(&self, tag: u16, default: usize) -> usize {
        self.one(tag).unwrap_or(default)
    }

    pub(super) fn values_or(&self, tag: u16, default: &[usize]) -> Vec<usize> {
        self.values(tag).unwrap_or_else(|| default.to_vec())
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
            NumericKind::Shorts => {
                for chunk in bytes.chunks_exact(2) {
                    values.push(usize::from(self.endian.u16_exact([chunk[0], chunk[1]])));
                }
            }
            NumericKind::Longs => {
                for chunk in bytes.chunks_exact(4) {
                    values.push(
                        self.endian
                            .u32_exact([chunk[0], chunk[1], chunk[2], chunk[3]])
                            as usize,
                    );
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
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    assert!(decode_page(b"", 0, None).is_err());
    assert!(decode_page(b"II", 0, None).is_err());
    let mut bad_page =
        include_bytes!("../../../tests/fixtures/input/images/tiff/1bit.tiff").to_vec();
    bad_page[106..110].copy_from_slice(&2000u32.to_le_bytes());
    assert!(decode_page(&bad_page, 1, None).is_err());
    let mut cyclic_page =
        include_bytes!("../../../tests/fixtures/input/images/tiff/1bit.tiff").to_vec();
    cyclic_page[106..110].copy_from_slice(&8u32.to_le_bytes());
    assert!(decode_page(&cyclic_page, 1, None).is_err());

    // No committed TIFF fixture declares associated (premultiplied) alpha;
    // exercise every tag-338 mapping arm so the semantic space stays covered.
    assert_eq!(
        source_alpha_from_extra_samples(&[1]),
        Some(SourceAlpha::Premultiplied)
    );
    assert_eq!(
        source_alpha_from_extra_samples(&[2]),
        Some(SourceAlpha::Straight)
    );
    assert_eq!(source_alpha_from_extra_samples(&[0]), None);
    assert_eq!(source_alpha_from_extra_samples(&[]), None);

    assert!(decode(b"", None).is_err());
    assert!(decode(b"II", None).is_err());
    assert!(decode(b"ZZ\0\0\0\0\0\0", None).is_err());
    let fixture = include_bytes!("../../../tests/fixtures/input/images/tiff/1bit.tiff");
    for checks in 0..=6 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = decode(fixture, Some(&token));
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = decode_sequence(
            fixture,
            &mut SequenceDecodeBudget::default_for(crate::ImageFormat::Tiff),
            Some(&token),
        );
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = decode_page(fixture, 0, Some(&token));
    }
    let _ = metadata_bytes(b"");
    let _ = metadata_bytes(b"II");
    let _ = metadata_bytes(b"II\x2a\0\0\0\0\0\0");
    let mut empty_ifd = b"II\x2a\0\0\0\x08\0\0\0\0\0".to_vec();
    empty_ifd.extend_from_slice(&[0u8; 16]);
    let _ = metadata_bytes(&empty_ifd);
    let _ = metadata_bytes(&[b'I', b'I', 0x2a, 0, 0, 0, 0, 8, 0xff]);
    // A self-referencing IFD chain exercises the cycle guard: patch the
    // classic `1bit.tiff` chain terminator (at 106) back to the first IFD.
    let mut cyclic = include_bytes!("../../../tests/fixtures/input/images/tiff/1bit.tiff").to_vec();
    cyclic[106..110].copy_from_slice(&8u32.to_le_bytes());
    let _ = metadata_bytes(&cyclic);
    // Strip offset/count arrays of different lengths exercise the mismatch
    // guard: double tag 279's declared count (entry at 82, count at 86).
    let mut mismatched = cyclic.clone();
    mismatched[86..90].copy_from_slice(&2u32.to_le_bytes());
    let _ = metadata_bytes(&mismatched);

    fn put_entry(out: &mut Vec<u8>, tag: u16, field_type: u16, count: u32, value: [u8; 4]) {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&field_type.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&value);
    }

    fn tiny_tiff(
        bits_count: u32,
        bits_inline: [u8; 4],
        photometric: u16,
        samples_per_pixel: u16,
        rows_per_strip: u32,
        planar: u16,
        predictor: u16,
    ) -> Vec<u8> {
        let entry_count = 11u16;
        let pixel_offset = 8 + 2 + usize::from(entry_count) * 12 + 4;
        let mut out = Vec::new();
        out.extend_from_slice(b"II");
        out.extend_from_slice(&42u16.to_le_bytes());
        out.extend_from_slice(&8u32.to_le_bytes());
        out.extend_from_slice(&entry_count.to_le_bytes());
        put_entry(&mut out, 256, 4, 1, 1u32.to_le_bytes());
        put_entry(&mut out, 257, 4, 1, 1u32.to_le_bytes());
        put_entry(&mut out, 258, 3, bits_count, bits_inline);
        put_entry(&mut out, 259, 3, 1, [1, 0, 0, 0]);
        put_entry(
            &mut out,
            262,
            3,
            1,
            [photometric as u8, (photometric >> 8) as u8, 0, 0],
        );
        put_entry(
            &mut out,
            273,
            4,
            1,
            u32::try_from(pixel_offset).unwrap().to_le_bytes(),
        );
        put_entry(
            &mut out,
            277,
            3,
            1,
            [
                samples_per_pixel as u8,
                (samples_per_pixel >> 8) as u8,
                0,
                0,
            ],
        );
        put_entry(&mut out, 278, 4, 1, rows_per_strip.to_le_bytes());
        put_entry(&mut out, 279, 4, 1, 1u32.to_le_bytes());
        put_entry(
            &mut out,
            284,
            3,
            1,
            [planar as u8, (planar >> 8) as u8, 0, 0],
        );
        put_entry(
            &mut out,
            317,
            3,
            1,
            [predictor as u8, (predictor >> 8) as u8, 0, 0],
        );
        out.extend_from_slice(&0u32.to_le_bytes());
        out.push(0);
        out
    }

    fn tiny_tiled_tiff(
        bits_per_sample: u16,
        include_tile_offsets: bool,
        include_tile_byte_counts: bool,
        tile_width: u32,
        tile_height: u32,
        predictor: u16,
        compression: u16,
        tile_payload: &[u8],
    ) -> Vec<u8> {
        let entry_count =
            10u16 + u16::from(include_tile_offsets) + u16::from(include_tile_byte_counts);
        let pixel_offset = 8 + 2 + usize::from(entry_count) * 12 + 4;
        let mut out = Vec::new();
        out.extend_from_slice(b"II");
        out.extend_from_slice(&42u16.to_le_bytes());
        out.extend_from_slice(&8u32.to_le_bytes());
        out.extend_from_slice(&entry_count.to_le_bytes());
        put_entry(&mut out, 256, 4, 1, 1u32.to_le_bytes());
        put_entry(&mut out, 257, 4, 1, 1u32.to_le_bytes());
        put_entry(&mut out, 258, 3, 1, [bits_per_sample as u8, 0, 0, 0]);
        put_entry(
            &mut out,
            259,
            3,
            1,
            [compression as u8, (compression >> 8) as u8, 0, 0],
        );
        put_entry(&mut out, 262, 3, 1, [1, 0, 0, 0]);
        put_entry(&mut out, 277, 3, 1, [1, 0, 0, 0]);
        put_entry(&mut out, 278, 4, 1, 1u32.to_le_bytes());
        put_entry(
            &mut out,
            317,
            3,
            1,
            [predictor as u8, (predictor >> 8) as u8, 0, 0],
        );
        put_entry(&mut out, 322, 4, 1, tile_width.to_le_bytes());
        put_entry(&mut out, 323, 4, 1, tile_height.to_le_bytes());
        if include_tile_offsets {
            put_entry(
                &mut out,
                324,
                4,
                1,
                u32::try_from(pixel_offset).unwrap().to_le_bytes(),
            );
        }
        if include_tile_byte_counts {
            put_entry(
                &mut out,
                325,
                4,
                1,
                u32::try_from(tile_payload.len()).unwrap().to_le_bytes(),
            );
        }
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(tile_payload);
        out
    }

    fn put_long_entry(
        out: &mut Vec<u8>,
        tag: u16,
        values: &[u32],
        external_start: usize,
        external: &mut Vec<u8>,
    ) {
        match values {
            [] => put_entry(out, tag, 4, 0, [0; 4]),
            [value] => put_entry(out, tag, 4, 1, value.to_le_bytes()),
            _ => {
                let position = u32::try_from(external_start + external.len()).unwrap();
                put_entry(
                    out,
                    tag,
                    4,
                    u32::try_from(values.len()).unwrap(),
                    position.to_le_bytes(),
                );
                for value in values {
                    external.extend_from_slice(&value.to_le_bytes());
                }
            }
        }
    }

    fn tiny_strip_tiff(
        width: u32,
        height: u32,
        bits_per_sample: u16,
        compression: u16,
        predictor: u16,
        rows_per_strip: u32,
        offset_count: usize,
        byte_counts: Option<&[u32]>,
        strip_payloads: &[&[u8]],
    ) -> Vec<u8> {
        let entry_count = 11u16;
        let external_start = 8 + 2 + usize::from(entry_count) * 12 + 4;
        let counts_len = byte_counts.map_or(0, <[u32]>::len);
        let pixel_offset = external_start
            + if offset_count > 1 {
                offset_count * 4
            } else {
                0
            }
            + if counts_len > 1 { counts_len * 4 } else { 0 };
        let mut next_offset = u32::try_from(pixel_offset).unwrap();
        let offsets = (0..offset_count)
            .map(|index| {
                let offset = next_offset;
                if let Some(payload) = strip_payloads.get(index) {
                    next_offset += u32::try_from(payload.len()).unwrap();
                }
                offset
            })
            .collect::<Vec<_>>();
        let mut external = Vec::new();
        let mut out = Vec::new();
        out.extend_from_slice(b"II");
        out.extend_from_slice(&42u16.to_le_bytes());
        out.extend_from_slice(&8u32.to_le_bytes());
        out.extend_from_slice(&entry_count.to_le_bytes());
        put_entry(&mut out, 256, 4, 1, width.to_le_bytes());
        put_entry(&mut out, 257, 4, 1, height.to_le_bytes());
        put_entry(&mut out, 258, 3, 1, [bits_per_sample as u8, 0, 0, 0]);
        put_entry(
            &mut out,
            259,
            3,
            1,
            [compression as u8, (compression >> 8) as u8, 0, 0],
        );
        put_entry(&mut out, 262, 3, 1, [1, 0, 0, 0]);
        put_long_entry(&mut out, 273, &offsets, external_start, &mut external);
        put_entry(&mut out, 277, 3, 1, [1, 0, 0, 0]);
        put_entry(&mut out, 278, 4, 1, rows_per_strip.to_le_bytes());
        put_long_entry(
            &mut out,
            279,
            byte_counts.unwrap_or(&[]),
            external_start,
            &mut external,
        );
        put_entry(&mut out, 284, 3, 1, [1, 0, 0, 0]);
        put_entry(
            &mut out,
            317,
            3,
            1,
            [predictor as u8, (predictor >> 8) as u8, 0, 0],
        );
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&external);
        for payload in strip_payloads {
            out.extend_from_slice(payload);
        }
        out
    }

    fn tiny_tiled_layout_tiff(
        width: u32,
        height: u32,
        bits_per_sample: u16,
        tile_width: u32,
        tile_height: u32,
        predictor: u16,
        compression: u16,
        tile_payloads: &[&[u8]],
        byte_counts: Option<&[u32]>,
    ) -> Vec<u8> {
        let entry_count = 12u16;
        let external_start = 8 + 2 + usize::from(entry_count) * 12 + 4;
        let counts = byte_counts.map(<[u32]>::to_vec).unwrap_or_else(|| {
            tile_payloads
                .iter()
                .map(|payload| payload.len() as u32)
                .collect()
        });
        let pixel_offset = external_start
            + if tile_payloads.len() > 1 {
                tile_payloads.len() * 4
            } else {
                0
            }
            + if counts.len() > 1 {
                counts.len() * 4
            } else {
                0
            };
        let mut next_offset = u32::try_from(pixel_offset).unwrap();
        let offsets = tile_payloads
            .iter()
            .map(|payload| {
                let offset = next_offset;
                next_offset += u32::try_from(payload.len()).unwrap();
                offset
            })
            .collect::<Vec<_>>();
        let mut external = Vec::new();
        let mut out = Vec::new();
        out.extend_from_slice(b"II");
        out.extend_from_slice(&42u16.to_le_bytes());
        out.extend_from_slice(&8u32.to_le_bytes());
        out.extend_from_slice(&entry_count.to_le_bytes());
        put_entry(&mut out, 256, 4, 1, width.to_le_bytes());
        put_entry(&mut out, 257, 4, 1, height.to_le_bytes());
        put_entry(&mut out, 258, 3, 1, [bits_per_sample as u8, 0, 0, 0]);
        put_entry(
            &mut out,
            259,
            3,
            1,
            [compression as u8, (compression >> 8) as u8, 0, 0],
        );
        put_entry(&mut out, 262, 3, 1, [1, 0, 0, 0]);
        put_entry(&mut out, 277, 3, 1, [1, 0, 0, 0]);
        put_entry(&mut out, 278, 4, 1, 1u32.to_le_bytes());
        put_entry(
            &mut out,
            317,
            3,
            1,
            [predictor as u8, (predictor >> 8) as u8, 0, 0],
        );
        put_entry(&mut out, 322, 4, 1, tile_width.to_le_bytes());
        put_entry(&mut out, 323, 4, 1, tile_height.to_le_bytes());
        put_long_entry(&mut out, 324, &offsets, external_start, &mut external);
        put_long_entry(&mut out, 325, &counts, external_start, &mut external);
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&external);
        for payload in tile_payloads {
            out.extend_from_slice(payload);
        }
        out
    }

    fn single_entry_ifd(tag: u16, field_type: u16, count: u32, value: [u8; 4]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&1u16.to_le_bytes());
        put_entry(&mut out, tag, field_type, count, value);
        out.extend_from_slice(&0u32.to_le_bytes());
        out
    }

    let _ = decode(b"II\0\0\x08\0\0\0", None);
    let _ = decode(b"II*\0", None);
    let _ = decode(b"MM\0*\0\0\0\x08\0\0\0\0", None);
    let _ = decode(&tiny_tiff(0, [0, 0, 0, 0], 1, 1, 1, 1, 1), None);
    let _ = decode(&tiny_tiff(3, u32::MAX.to_le_bytes(), 1, 1, 1, 1, 1), None);
    let _ = decode(&tiny_tiff(2, [8, 0, 16, 0], 1, 1, 1, 1, 1), None);
    let _ = decode(&tiny_tiff(1, [0, 1, 0, 0], 1, 1, 1, 1, 1), None);
    let _ = decode(&tiny_tiff(1, [8, 0, 0, 0], 1, 0, 1, 1, 1), None);
    let _ = decode(&tiny_tiff(1, [8, 0, 0, 0], 1, 1, 0, 1, 1), None);
    let _ = decode(&tiny_tiff(1, [8, 0, 0, 0], 1, 1, 1, 2, 1), None);
    let _ = decode(&tiny_tiff(1, [8, 0, 0, 0], 1, 1, 1, 1, 2), None);
    let _ = decode(&tiny_tiff(1, [8, 0, 0, 0], 1, 1, 1, 1, 3), None);
    let _ = decode(&tiny_tiff(1, [8, 0, 0, 0], 6, 1, 1, 1, 1), None);
    let _ = decode(&tiny_tiff(1, [16, 0, 0, 0], 6, 3, 1, 1, 1), None);
    let _ = decode(
        &tiny_strip_tiff(0, 1, 8, 1, 1, 1, 1, Some(&[1]), &[&[0]]),
        None,
    );
    let _ = decode(
        &tiny_strip_tiff(1, 1, 8, 99, 1, 1, 1, Some(&[1]), &[&[0]]),
        None,
    );
    let _ = decode(&tiny_tiled_tiff(8, false, true, 1, 1, 1, 1, &[0]), None);
    let _ = decode(&tiny_tiled_tiff(8, true, false, 1, 1, 1, 1, &[0]), None);
    let _ = decode(&tiny_tiled_tiff(8, true, true, 0, 1, 1, 1, &[0]), None);
    let _ = decode(&tiny_tiled_tiff(8, true, true, 1, 0, 1, 1, &[0]), None);
    let _ = decode(&tiny_tiled_tiff(1, true, true, 1, 1, 1, 1, &[0]), None);
    let _ = decode(&tiny_tiled_tiff(8, true, true, 1, 1, 2, 1, &[0]), None);

    let _ = decode(&tiny_strip_tiff(1, 1, 8, 1, 1, 1, 0, Some(&[]), &[]), None);
    let _ = decode(&tiny_strip_tiff(1, 1, 8, 1, 1, 1, 1, Some(&[1]), &[]), None);
    let _ = decode(
        &tiny_strip_tiff(1, 1, 8, 1, 1, 1, 2, Some(&[1, 1]), &[&[0], &[1]]),
        None,
    );
    let _ = decode(
        &tiny_strip_tiff(
            1,
            1,
            8,
            COMPRESSION_PACKBITS as u16,
            1,
            1,
            1,
            None,
            &[&[0, 7]],
        ),
        None,
    );
    let _ = decode(
        &tiny_strip_tiff(
            1,
            2,
            8,
            COMPRESSION_PACKBITS as u16,
            1,
            1,
            2,
            None,
            &[&[0, 7], &[0, 8]],
        ),
        None,
    );
    let _ = decode(
        &tiny_strip_tiff(
            1,
            2,
            8,
            COMPRESSION_PACKBITS as u16,
            1,
            1,
            2,
            Some(&[2]),
            &[&[0, 7], &[0, 8]],
        ),
        None,
    );
    let _ = decode(
        &tiny_strip_tiff(
            1,
            1,
            8,
            COMPRESSION_PACKBITS as u16,
            1,
            1,
            1,
            Some(&[4]),
            &[&[0, 7]],
        ),
        None,
    );
    let _ = decode(
        &tiny_tiled_layout_tiff(2, 1, 8, 1, 1, 1, 1, &[&[0]], Some(&[1])),
        None,
    );
    let _ = decode(
        &tiny_tiled_layout_tiff(2, 2, 8, 1, 1, 1, 1, &[&[1], &[2], &[3], &[4]], None),
        None,
    );

    let _ = decode_packbits(&[], 0);
    let _ = decode_packbits(&[0], 0);
    let _ = decode_packbits(&[0, 7], 1);
    let _ = decode_packbits(&[0x80, 0, 9], 1);
    let _ = decode_packbits(&[0x80], 1);
    let _ = decode_packbits(&[1, 7, 8], 1);
    let _ = decode_packbits(&[0xff, 5], 2);
    let _ = decode_packbits(&[0xff], 2);
    let _ = decode_packbits(&[2, 1], 3);
    let token = crate::CancellationToken::new();
    let _ = decode_packbits_with_token(&[0x80, 0, 9], 1, &token);
    let token = crate::CancellationToken::new();
    let _ = decode_packbits_with_token(&[0, 7, 9], 1, &token);
    let token = crate::CancellationToken::new();
    let _ = decode_packbits_with_token(&[1, 7], 1, &token);
    let token = crate::CancellationToken::new();
    let _ = decode_packbits_with_token(&[0xff], 1, &token);
    let token = crate::CancellationToken::new();
    let _ = decode_packbits_with_token(&[0x80], 1, &token);

    fn pack_lzw_9(codes: &[u16]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut current = 0u8;
        let mut used = 0u8;
        for &code in codes {
            for shift in (0..9).rev() {
                current = (current << 1) | (((code >> shift) & 1) as u8);
                used += 1;
                if used == 8 {
                    out.push(current);
                    current = 0;
                    used = 0;
                }
            }
        }
        out.push(current << (8 - used));
        out
    }

    fn pack_lzw_variable(codes: &[u16]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut current = 0u8;
        let mut used = 0u8;
        let mut width = 9u8;
        let mut next_code = 258u16;
        let mut previous = false;
        for &code in codes {
            for shift in (0..width).rev() {
                current = (current << 1) | (((code >> shift) & 1) as u8);
                used = used.wrapping_add(1);
                if used == 8 {
                    out.push(current);
                    current = 0;
                    used = 0;
                }
            }
            if previous {
                next_code = next_code.wrapping_add(1);
                if width < 12 && next_code == 1_u16.wrapping_shl(width.into()).wrapping_sub(1) {
                    width = width.wrapping_add(1);
                }
            }
            previous = true;
        }
        out.push(current.wrapping_shl(u32::from(8_u8.wrapping_sub(used))));
        out
    }

    let _ = decode_lzw(&pack_lzw_9(&[258]), 1);
    let _ = decode_lzw(&pack_lzw_9(&[65]), 0);
    let _ = decode_lzw(&pack_lzw_9(&[65]), 1);
    let _ = decode_lzw(&pack_lzw_9(&[65, 66, 257]), 2);
    let lzw_a = pack_lzw_9(&[65]);
    let token = crate::CancellationToken::new();
    let _ = decode_lzw_with_token(&[], 1, &token);
    let token = crate::CancellationToken::new();
    let _ = decode_lzw_with_token(&pack_lzw_9(&[257]), 1, &token);
    let token = crate::CancellationToken::new();
    let _ = decode_lzw_with_token(&pack_lzw_9(&[258]), 1, &token);
    let token = crate::CancellationToken::new();
    let _ = decode_lzw_with_token(&pack_lzw_9(&[65, 258]), 2, &token);
    let token = crate::CancellationToken::new();
    let _ = decode_lzw_with_token(&pack_lzw_9(&[65]), 0, &token);
    let token = crate::CancellationToken::new();
    let _ = decode_lzw_with_token(&pack_lzw_9(&[65, 300]), 2, &token);
    let width_probe = vec![65u16; 2_000];
    let _ = pack_lzw_variable(&width_probe);
    let mut growth_codes = vec![65u16, 66];
    growth_codes.extend(259..=1282);
    let growth = pack_lzw_variable(&growth_codes);
    let token = crate::CancellationToken::new();
    token.cancel_after(1_026);
    let _ = decode_lzw_with_token(&growth, 525_826, &token);
    let mut repeated_growth = growth_codes.clone();
    repeated_growth.push(1282);
    let repeated_growth = pack_lzw_variable(&repeated_growth);
    for checks in 1_027..=1_032 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = decode_lzw_with_token(&repeated_growth, 526_851, &token);
    }
    let dictionary_saturation =
        include_bytes!("../../../tests/fixtures/input/images/tiff/lzw_dictionary_saturation.tiff");
    let token = crate::CancellationToken::new();
    let _ = decode(dictionary_saturation, Some(&token));
    let mut prefixes = [0u16; 4096];
    let suffixes = [0u8; 4096];
    let mut stack = [0u8; 4096];
    for value in 1..=1024u16 {
        prefixes[usize::from(value)] = value - 1;
    }
    let token = crate::CancellationToken::new();
    let _ = append_lzw_with_token(
        1024,
        &prefixes,
        &suffixes,
        &mut stack,
        &mut Vec::new(),
        1025,
        &token,
    );
    let token = crate::CancellationToken::new();
    token.cancel();
    let _ = append_lzw_with_token(
        1024,
        &prefixes,
        &suffixes,
        &mut stack,
        &mut Vec::new(),
        1025,
        &token,
    );
    for checks in 0..=6 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = decode(
            &tiny_tiled_layout_tiff(
                1,
                1,
                8,
                1,
                1,
                1,
                COMPRESSION_LZW as u16,
                &[&lzw_a],
                Some(&[u32::try_from(lzw_a.len()).unwrap()]),
            ),
            Some(&token),
        );
    }
    let _ = decode(
        &tiny_strip_tiff(
            1,
            1,
            8,
            COMPRESSION_LZW as u16,
            2,
            1,
            1,
            Some(&[u32::try_from(lzw_a.len()).unwrap()]),
            &[&lzw_a],
        ),
        None,
    );
    let _ = decode(
        &tiny_tiled_tiff(
            24,
            true,
            true,
            1,
            1,
            2,
            COMPRESSION_LZW as u16,
            &pack_lzw_9(&[65, 66, 67]),
        ),
        None,
    );
    let _ = decode(
        &tiny_tiled_layout_tiff(
            1,
            1,
            8,
            1,
            1,
            2,
            COMPRESSION_LZW as u16,
            &[&lzw_a],
            Some(&[u32::try_from(lzw_a.len()).unwrap()]),
        ),
        None,
    );
    let _ = decode(
        &tiny_tiled_layout_tiff(
            1,
            2,
            8,
            1,
            1,
            1,
            COMPRESSION_LZW as u16,
            &[&lzw_a, &lzw_a],
            Some(&[u32::try_from(lzw_a.len()).unwrap()]),
        ),
        None,
    );
    let _ = decode(
        &tiny_tiled_layout_tiff(
            1,
            1,
            8,
            1,
            1,
            1,
            COMPRESSION_LZW as u16,
            &[&lzw_a],
            Some(&[u32::try_from(lzw_a.len() + 1).unwrap()]),
        ),
        None,
    );
    let _ = decode(
        &tiny_tiled_layout_tiff(
            1,
            1,
            8,
            1,
            1,
            1,
            COMPRESSION_LZW as u16,
            &[&[0]],
            Some(&[1]),
        ),
        None,
    );
    let lzw_rgb = pack_lzw_9(&[65, 66, 67, 257]);
    let _ = decode(
        &tiny_strip_tiff(
            1,
            1,
            24,
            COMPRESSION_LZW as u16,
            2,
            1,
            1,
            Some(&[u32::try_from(lzw_rgb.len()).unwrap()]),
            &[&lzw_rgb],
        ),
        None,
    );

    let mut one_bit_reader = MsbBits::new(&[0x80]);
    let _ = one_bit_reader.read(1);
    let mut short_reader = MsbBits::new(&[0]);
    let _ = short_reader.read(9);
    let mut endian_bytes = [0; 4];
    Endian::Little.write_u16(1, &mut endian_bytes[..2]);
    Endian::Big.write_u32(1, &mut endian_bytes);

    let _ = Directory::parse(&[], 0, Endian::Little);
    let _ = Directory::parse(&[], usize::MAX, Endian::Little);
    let oversized_count = 4097u16.to_le_bytes();
    let _ = Directory::parse(&oversized_count, 0, Endian::Little);
    let truncated_entry = 1u16.to_le_bytes();
    let _ = Directory::parse(&truncated_entry, 0, Endian::Little);
    let _ = Directory::parse(&single_entry_ifd(300, 13, 1, [0; 4]), 0, Endian::Little);
    let _ = Directory::parse(
        &single_entry_ifd(300, 5, 1, u32::MAX.to_le_bytes()),
        0,
        Endian::Little,
    );
    let empty_directory = Directory {
        data: &[],
        endian: Endian::Little,
        fields: Vec::new(),
        entries: Vec::new(),
        next_offset: 0,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
    };
    let _ = empty_directory.one_or(1, 7);
    let _ = empty_directory.values_or(1, &[7, 8]);
    let inline_shorts = single_entry_ifd(300, 3, 2, [1, 0, 2, 0]);
    let directory = Directory::parse(&inline_shorts, 0, Endian::Little).unwrap();
    let _ = directory.values(300);
    let inline_long = single_entry_ifd(301, 4, 1, 9u32.to_le_bytes());
    let directory = Directory::parse(&inline_long, 0, Endian::Little).unwrap();
    let _ = directory.values(301);
    let mut external_shorts = single_entry_ifd(302, 3, 3, 18u32.to_le_bytes());
    external_shorts.extend_from_slice(&[1, 0, 2, 0, 3, 0]);
    let directory = Directory::parse(&external_shorts, 0, Endian::Little).unwrap();
    let _ = directory.values(302);
    let mut external_longs = single_entry_ifd(303, 4, 2, 18u32.to_le_bytes());
    external_longs.extend_from_slice(&1u32.to_le_bytes());
    external_longs.extend_from_slice(&2u32.to_le_bytes());
    let directory = Directory::parse(&external_longs, 0, Endian::Little).unwrap();
    let _ = directory.values(303);

    let mut predicted = vec![1, 2, 3, 4, 5, 6];
    reverse_horizontal_predictor(&mut predicted, 6, 3, 8, Endian::Little);
    let mut predicted = vec![0, 1, 0, 2];
    reverse_horizontal_predictor(&mut predicted, 4, 1, 16, Endian::Big);
    let mut predicted = vec![0, 0, 0, 1, 0, 0, 0, 2];
    reverse_horizontal_predictor(&mut predicted, 8, 1, 32, Endian::Little);
}
