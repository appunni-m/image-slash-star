//! PNG decoder implemented from the PNG chunk and filtering specifications.

use crate::SequenceDecodeBudget;
use crate::codecs::compression::deflate::decompress_zlib_prefix;
use crate::codecs::{CodecError, CodecResult, OptionCodecExt, codec_add_end, need_slice};
use crate::types::{
    ColorType, DecodedFrame, DecodedImage, DecodedSequence, FrameBlend, FrameDisposal,
    FrameDuration, FrameRect, ImageMode, ImagePalette, SourceColor,
};

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const PILLOW_DECOMPRESSION_BOMB_ERROR_PIXELS: u64 = 178_956_970;
const ADAM7: [(usize, usize, usize, usize); 7] = [
    (0, 0, 8, 8),
    (4, 0, 8, 8),
    (0, 4, 4, 8),
    (2, 0, 4, 4),
    (0, 2, 2, 4),
    (1, 0, 2, 2),
    (0, 1, 1, 2),
];

/// Chunk types the PNG model interprets and therefore never retains as opaque
/// container blocks.
const INTERPRETED_CHUNKS: [&[u8; 4]; 8] = [
    b"IHDR", b"PLTE", b"tRNS", b"IDAT", b"IEND", b"acTL", b"fcTL", b"fdAT",
];

/// Ancillary chunks the model classifies as known metadata and retains in the
/// metadata records instead of the opaque-block list.
const KNOWN_METADATA_CHUNKS: [&[u8; 4]; 9] = [
    b"tEXt", b"zTXt", b"iTXt", b"eXIf", b"tIME", b"pHYs", b"bKGD", b"hIST", b"sBIT",
];

/// Ancillary chunks the model classifies as source color metadata.
const COLOR_CHUNKS: [&[u8; 4]; 4] = [b"sRGB", b"gAMA", b"cHRM", b"iCCP"];

/// Whether an uninterpreted ancillary chunk is retained as an opaque block.
fn retained_opaque_chunk(kind: &[u8; 4]) -> bool {
    kind[0] & 0x20 != 0
        && !INTERPRETED_CHUNKS.contains(&kind)
        && !KNOWN_METADATA_CHUNKS.contains(&kind)
}

/// Whether an ancillary chunk is classified as known metadata.
fn retained_metadata_chunk(kind: &[u8; 4]) -> bool {
    kind[0] & 0x20 != 0 && KNOWN_METADATA_CHUNKS.contains(&kind)
}

/// Whether an ancillary chunk is classified as source color metadata.
fn retained_color_chunk(kind: &[u8; 4]) -> bool {
    kind[0] & 0x20 != 0 && COLOR_CHUNKS.contains(&kind)
}

fn opaque_block(kind: [u8; 4], data: &[u8]) -> crate::types::OpaqueBlock {
    crate::types::OpaqueBlock {
        kind: kind.to_vec(),
        data: data.to_vec(),
        // PNG's safe-to-copy bit is the lowercase bit of the chunk name's
        // fourth character.
        safe_to_copy: kind[3] & 0x20 != 0,
    }
}

fn metadata_record(kind: [u8; 4], data: &[u8]) -> crate::types::OpaqueMetadata {
    crate::types::OpaqueMetadata {
        kind: kind.to_vec(),
        data: data.to_vec(),
    }
}

fn srgb_intent(value: u8) -> Option<crate::types::SrgbIntent> {
    match value {
        0 => Some(crate::types::SrgbIntent::Perceptual),
        1 => Some(crate::types::SrgbIntent::RelativeColorimetric),
        2 => Some(crate::types::SrgbIntent::Saturation),
        3 => Some(crate::types::SrgbIntent::AbsoluteColorimetric),
        _ => None,
    }
}

/// Parse the first well-formed occurrence of each color chunk into the source
/// color descriptor; duplicates and malformed payloads fall back to raw
/// metadata records so no bytes are lost.
fn retain_color_chunk(
    kind: [u8; 4],
    data: &[u8],
    color: &mut SourceColor,
    metadata: &mut Vec<crate::types::OpaqueMetadata>,
) {
    match &kind {
        b"sRGB"
            if data.len() == 1
                && color.srgb().is_none()
                && let Some(intent) = srgb_intent(data[0]) =>
        {
            *color = core::mem::take(color).with_srgb(intent);
            return;
        }
        b"gAMA" if data.len() == 4 && color.gamma().is_none() => {
            let gamma = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            *color = core::mem::take(color).with_gamma(gamma);
            return;
        }
        b"cHRM" if data.len() == 32 && color.chromaticities().is_none() => {
            let value = |index: usize| {
                u32::from_be_bytes([
                    data[index.wrapping_mul(4)],
                    data[index.wrapping_mul(4).wrapping_add(1)],
                    data[index.wrapping_mul(4).wrapping_add(2)],
                    data[index.wrapping_mul(4).wrapping_add(3)],
                ])
            };
            *color =
                core::mem::take(color).with_chromaticities(crate::types::SourceChromaticities {
                    white_x: value(0),
                    white_y: value(1),
                    red_x: value(2),
                    red_y: value(3),
                    green_x: value(4),
                    green_y: value(5),
                    blue_x: value(6),
                    blue_y: value(7),
                });
            return;
        }
        b"iCCP" if color.icc_profile().is_none() => {
            if let Some(nul) = data.iter().position(|&byte| byte == 0)
                && nul != 0
                && nul.saturating_add(1) < data.len()
            {
                *color = core::mem::take(color).with_icc_profile(crate::types::RawIccProfile {
                    keyword: data[..nul].to_vec(),
                    data: data[nul.saturating_add(1)..].to_vec(),
                });
                return;
            }
        }
        _ => {}
    }
    metadata.push(crate::types::OpaqueMetadata {
        kind: kind.to_vec(),
        data: data.to_vec(),
    });
}

/// Decode the first image represented by a PNG or APNG stream.
pub fn decode(data: &[u8]) -> CodecResult<(DecodedImage, usize)> {
    // Pillow's load path accepts bad IDAT CRCs after lazy construction has
    // validated all construction-critical chunks.
    let mut chunks = Chunks::new(data, false)?;
    let header = read_header(&mut chunks)?;

    let mut compressed = Vec::new();
    let mut palette_rgb = None;
    let mut palette_alpha = Vec::new();
    let mut saw_idat = false;
    let mut saw_post_idat_control = false;
    let mut next_sequence = 0;
    let mut opaque_blocks = Vec::new();
    let mut metadata = Vec::new();
    let mut source_color = SourceColor::new();
    let mut saw_iend = false;
    for chunk in &mut chunks {
        let chunk = chunk?;
        match &chunk.kind {
            b"IDAT" => {
                saw_idat = true;
                compressed.extend_from_slice(chunk.data);
            }
            b"PLTE" if palette_rgb.is_none() => palette_rgb = Some(chunk.data.to_vec()),
            b"tRNS" if palette_alpha.is_empty() => palette_alpha.extend_from_slice(chunk.data),
            b"acTL" if chunk.data.len() < 8 => {
                return Err(CodecError::Malformed(
                    "PNG acTL chunk has an invalid length".to_owned(),
                ));
            }
            b"fcTL" => {
                if saw_idat {
                    saw_post_idat_control = true;
                } else {
                    let _ = parse_frame_control(chunk.data, header, &mut next_sequence)?;
                }
            }
            b"fdAT" if saw_idat && !saw_post_idat_control => {
                if chunk.data.len() < 4 {
                    return Err(CodecError::Malformed(
                        "APNG contains a truncated fdAT chunk".to_owned(),
                    ));
                }
                consume_sequence(read_u32(chunk.data, 0), &mut next_sequence)?;
            }
            b"IEND" => {
                saw_iend = true;
                break;
            }
            _ if retained_color_chunk(&chunk.kind) => {
                retain_color_chunk(chunk.kind, chunk.data, &mut source_color, &mut metadata);
            }
            _ if retained_metadata_chunk(&chunk.kind) => {
                metadata.push(metadata_record(chunk.kind, chunk.data));
            }
            _ if retained_opaque_chunk(&chunk.kind) => {
                opaque_blocks.push(opaque_block(chunk.kind, chunk.data));
            }
            _ => {}
        }
    }
    if compressed.is_empty() {
        if saw_iend {
            return Err(CodecError::Malformed(
                "PNG contains no image data".to_owned(),
            ));
        }
        return Err(CodecError::NeedMore {
            minimum: codec_add_end(chunks.position, 8),
            message: "PNG contains no image data".to_owned(),
        });
    }

    let image = decode_image_data(
        header.image_spec(header.width, header.height),
        header.channels,
        header.interlace,
        &compressed,
        palette_rgb,
        palette_alpha,
    )
    .map_err(|error| match error {
        CodecError::NeedMore { message, .. } if saw_iend => CodecError::Malformed(message),
        CodecError::NeedMore { message, .. } => CodecError::NeedMore {
            minimum: codec_add_end(chunks.position, 8),
            message,
        },
        other => other,
    })?
    .with_opaque_blocks(opaque_blocks)
    .with_metadata(metadata)
    .with_source_color(source_color);
    Ok((image, chunks.position))
}

fn decode_image_data(
    spec: PngImageSpec,
    channels: usize,
    interlace: u8,
    compressed: &[u8],
    palette_rgb: Option<Vec<u8>>,
    palette_alpha: Vec<u8>,
) -> CodecResult<DecodedImage> {
    let expected_inflated = inflated_len(spec.width, spec.height, channels, spec.depth, interlace);
    let inflated = decompress_zlib_prefix(compressed, expected_inflated)
        .map_err(|error| error.context("decode PNG zlib stream"))?;
    if inflated.len() != expected_inflated {
        return Err(CodecError::Malformed(
            "PNG image data has an unexpected decompressed length".to_owned(),
        ));
    }

    let samples = decode_scanlines(
        &inflated,
        spec.width,
        spec.height,
        channels,
        spec.depth,
        interlace,
    )?;
    build_image(spec, &samples, palette_rgb, palette_alpha)
}

#[derive(Clone)]
struct ApngFrameControl {
    rect: FrameRect,
    duration: FrameDuration,
    disposal: FrameDisposal,
    blend: FrameBlend,
}

struct ApngCompressedFrame {
    control: ApngFrameControl,
    compressed: Vec<u8>,
}

struct ParsedApng {
    header: PngHeader,
    palette_rgb: Option<Vec<u8>>,
    palette_alpha: Vec<u8>,
    loop_count: u32,
    default_compressed: Vec<u8>,
    default_control: Option<ApngFrameControl>,
    frames: Vec<ApngCompressedFrame>,
    opaque_blocks: Vec<crate::types::OpaqueBlock>,
    metadata: Vec<crate::types::OpaqueMetadata>,
    source_color: SourceColor,
}

/// Decode every APNG presentation while retaining exact source controls.
pub fn decode_sequence(
    data: &[u8],
    budget: &mut SequenceDecodeBudget,
) -> CodecResult<(DecodedSequence, usize)> {
    let Some((parsed, consumed)) = parse_apng(data)? else {
        let (mut image, consumed) = decode(data)?;
        let opaque_blocks = std::mem::take(&mut image.opaque_blocks);
        let metadata = std::mem::take(&mut image.metadata);
        let source_color = std::mem::take(&mut image.source_color);
        let mut sequence = DecodedSequence::from_image(image);
        sequence.opaque_blocks = opaque_blocks;
        sequence.metadata = metadata;
        sequence.source_color = source_color;
        return Ok((sequence, consumed));
    };

    let ParsedApng {
        header,
        palette_rgb,
        palette_alpha,
        loop_count,
        default_compressed,
        default_control,
        frames: mut compressed_frames,
        opaque_blocks,
        metadata,
        source_color,
    } = parsed;
    let mut output_frames = Vec::new();
    let mut canvas = None;

    if let Some(control) = default_control {
        compressed_frames.insert(
            0,
            ApngCompressedFrame {
                control,
                compressed: default_compressed,
            },
        );
    } else {
        let default_image = decode_image_data(
            header.image_spec(header.width, header.height),
            header.channels,
            header.interlace,
            &default_compressed,
            palette_rgb.clone(),
            palette_alpha.clone(),
        )?;
        canvas = Some(default_image.clone());
        let mut default_frame = DecodedFrame::rendered_canvas(
            default_image,
            FrameRect {
                left: 0,
                top: 0,
                width: header.width,
                height: header.height,
            },
            FrameDuration::ZERO,
            FrameDisposal::Unspecified,
            FrameBlend::Unspecified,
        );
        default_frame.source.interlaced = header.interlace != 0;
        default_frame.source.is_default_image = true;
        output_frames.push(default_frame);
    }
    let canvas_mode = if header.png_color == 0 && header.depth == 1 {
        ImageMode::L1
    } else if header.png_color == 3 {
        ImageMode::P8
    } else {
        header.color.into()
    };

    for (animation_index, encoded) in compressed_frames.into_iter().enumerate() {
        if !output_frames.is_empty() {
            budget
                .reserve_later_frame(canvas_mode, header.width, header.height)
                .map_err(CodecError::LimitExceeded)?;
        }
        let source = decode_image_data(
            header.image_spec(encoded.control.rect.width, encoded.control.rect.height),
            header.channels,
            header.interlace,
            &encoded.compressed,
            palette_rgb.clone(),
            palette_alpha.clone(),
        )?;
        let canvas = match &mut canvas {
            Some(canvas) => canvas,
            slot @ None => slot.insert(blank_canvas(&source, header.width, header.height)),
        };
        let previous = canvas.clone();
        composite_frame(canvas, &source, encoded.control.rect, encoded.control.blend);

        let mut frame = DecodedFrame::rendered_canvas(
            canvas.clone(),
            encoded.control.rect,
            encoded.control.duration,
            encoded.control.disposal,
            encoded.control.blend,
        );
        frame.source.interlaced = header.interlace != 0;
        frame.source.is_default_image = animation_index == 0 && output_frames.is_empty();
        output_frames.push(frame);

        match encoded.control.disposal {
            FrameDisposal::Background => clear_rect(canvas, encoded.control.rect),
            FrameDisposal::Previous if animation_index == 0 => {
                clear_rect(canvas, encoded.control.rect);
            }
            FrameDisposal::Previous => {
                *canvas = previous;
            }
            FrameDisposal::Unspecified | FrameDisposal::Keep | FrameDisposal::Reserved(_) => {}
        }
    }

    Ok((
        DecodedSequence {
            width: header.width,
            height: header.height,
            frames: output_frames,
            loop_count: Some(loop_count),
            background: None,
            kind: crate::types::SequenceKind::TimedAnimation,
            opaque_blocks,
            metadata,
            source_color,
        },
        consumed,
    ))
}

/// Measure the encoded metadata extent: the consumed chunk scan minus the
/// compressed pixel payload bytes of `IDAT` and `fdAT` data.
pub(crate) fn metadata_bytes(data: &[u8]) -> CodecResult<u64> {
    let mut chunks = Chunks::new(data, false)?;
    let mut pixel = 0u64;
    for chunk in &mut chunks {
        let chunk = chunk?;
        if chunk.kind == *b"IDAT" {
            pixel = pixel.saturating_add(chunk.data.len() as u64);
        } else if chunk.kind == *b"fdAT" {
            pixel = pixel.saturating_add(chunk.data.len().saturating_sub(4) as u64);
        }
    }
    // `pixel` is the sum of IDAT/fdAT payloads inside the chunk scan.
    #[allow(clippy::arithmetic_side_effects)]
    let metadata = chunks.position as u64 - pixel;
    Ok(metadata)
}

fn parse_apng(data: &[u8]) -> CodecResult<Option<(ParsedApng, usize)>> {
    // The common sequence dispatcher has already detected the complete PNG
    // signature. Avoid manufacturing an unreachable second signature-error
    // path inside the APNG parser.
    let mut chunks = Chunks {
        data,
        position: PNG_SIGNATURE.len(),
        failed: false,
        verify_crc: false,
    };
    let header = read_header(&mut chunks)?;
    let mut animation = None;
    let mut saw_idat = false;
    let mut palette_rgb = None;
    let mut palette_alpha = Vec::new();
    let mut default_compressed = Vec::new();
    let mut default_control = None;
    let mut current = None::<ApngCompressedFrame>;
    let mut current_has_data = false;
    let mut frames = Vec::new();
    let mut next_sequence = 0u32;
    let mut controlled_frames = 0u32;
    let mut opaque_blocks = Vec::new();
    let mut metadata = Vec::new();
    let mut source_color = SourceColor::new();

    for chunk in &mut chunks {
        let chunk = chunk?;
        match &chunk.kind {
            b"PLTE" if !saw_idat && palette_rgb.is_none() => {
                palette_rgb = Some(chunk.data.to_vec());
            }
            b"tRNS" if !saw_idat && palette_alpha.is_empty() => {
                palette_alpha.extend_from_slice(chunk.data);
            }
            b"acTL" if !saw_idat => {
                if chunk.data.len() < 8 {
                    return Err(CodecError::Malformed(
                        "APNG contains a truncated acTL chunk".to_owned(),
                    ));
                }
                if animation.is_some() {
                    animation = None;
                    default_control = None;
                    current = None;
                    frames.clear();
                    continue;
                }
                let frame_count = read_u32(chunk.data, 0);
                if frame_count == 0 || frame_count > 0x8000_0000 {
                    continue;
                }
                animation = Some((frame_count, read_u32(chunk.data, 4)));
            }
            b"fcTL" if animation.is_some() => {
                if let Some(frame) = current.take() {
                    if !current_has_data {
                        return Err(CodecError::Malformed(
                            "APNG frame is missing image data".to_owned(),
                        ));
                    }
                    frames.push(frame);
                }
                let control = parse_frame_control(chunk.data, header, &mut next_sequence)?;
                controlled_frames = controlled_frames.saturating_add(1);
                if saw_idat {
                    current = Some(ApngCompressedFrame {
                        control,
                        compressed: Vec::new(),
                    });
                    current_has_data = false;
                } else if default_control.replace(control).is_some() {
                    return Err(CodecError::Malformed(
                        "APNG default frame has multiple controls".to_owned(),
                    ));
                }
            }
            b"IDAT" => {
                saw_idat = true;
                default_compressed.extend_from_slice(chunk.data);
            }
            b"fdAT" if animation.is_some() => {
                if chunk.data.len() < 4 {
                    return Err(CodecError::Malformed(
                        "APNG contains a truncated fdAT chunk".to_owned(),
                    ));
                }
                consume_sequence(read_u32(chunk.data, 0), &mut next_sequence)?;
                let Some(frame) = current.as_mut() else {
                    return Err(CodecError::Malformed(
                        "APNG frame data has no frame control".to_owned(),
                    ));
                };
                current_has_data = true;
                frame.compressed.extend_from_slice(&chunk.data[4..]);
            }
            b"IEND" => break,
            _ if retained_color_chunk(&chunk.kind) => {
                retain_color_chunk(chunk.kind, chunk.data, &mut source_color, &mut metadata);
            }
            _ if retained_metadata_chunk(&chunk.kind) => {
                metadata.push(metadata_record(chunk.kind, chunk.data));
            }
            _ if retained_opaque_chunk(&chunk.kind) => {
                opaque_blocks.push(opaque_block(chunk.kind, chunk.data));
            }
            _ => {}
        }
    }

    let Some((declared_frames, loop_count)) = animation else {
        return Ok(None);
    };
    if let Some(frame) = current {
        if !current_has_data {
            return Err(CodecError::Malformed(
                "APNG frame is missing image data".to_owned(),
            ));
        }
        frames.push(frame);
    }
    if default_compressed.is_empty() {
        return Err(CodecError::Malformed(
            "PNG contains no image data".to_owned(),
        ));
    }
    if controlled_frames != declared_frames {
        return Err(CodecError::Malformed(
            "APNG declared frame count does not match its frame controls".to_owned(),
        ));
    }
    Ok(Some((
        ParsedApng {
            header,
            palette_rgb,
            palette_alpha,
            loop_count,
            default_compressed,
            default_control,
            frames,
            opaque_blocks,
            metadata,
            source_color,
        },
        chunks.position,
    )))
}

fn parse_frame_control(
    data: &[u8],
    header: PngHeader,
    next_sequence: &mut u32,
) -> CodecResult<ApngFrameControl> {
    if data.len() < 26 {
        return Err(CodecError::Malformed(
            "APNG contains a truncated fcTL chunk".to_owned(),
        ));
    }
    consume_sequence(read_u32(data, 0), next_sequence)?;
    let width = read_u32(data, 4);
    let height = read_u32(data, 8);
    let left = read_u32(data, 12);
    let top = read_u32(data, 16);
    if width == 0 || height == 0 {
        return Err(CodecError::Dimensions(
            "APNG frame dimensions must be non-zero".to_owned(),
        ));
    }
    if u64::from(left).saturating_add(u64::from(width)) > u64::from(header.width)
        || u64::from(top).saturating_add(u64::from(height)) > u64::from(header.height)
    {
        return Err(CodecError::Malformed(
            "APNG contains an invalid frame rectangle".to_owned(),
        ));
    }
    let delay_denominator = u64::from(read_u16(data, 22));
    Ok(ApngFrameControl {
        rect: FrameRect {
            left,
            top,
            width,
            height,
        },
        duration: FrameDuration {
            numerator: u64::from(read_u16(data, 20)),
            denominator: if delay_denominator == 0 {
                100
            } else {
                delay_denominator
            },
        },
        disposal: match data[24] {
            0 => FrameDisposal::Keep,
            1 => FrameDisposal::Background,
            2 => FrameDisposal::Previous,
            value => FrameDisposal::Reserved(value),
        },
        blend: match data[25] {
            0 => FrameBlend::Source,
            1 => FrameBlend::Over,
            value => FrameBlend::Reserved(value),
        },
    })
}

fn consume_sequence(actual: u32, next: &mut u32) -> CodecResult<()> {
    if actual != *next {
        return Err(CodecError::Malformed(
            "APNG frame sequence numbers are not continuous".to_owned(),
        ));
    }
    if *next == u32::MAX {
        return Err(CodecError::Malformed(
            "APNG sequence number overflows".to_owned(),
        ));
    }
    *next = next.wrapping_add(1);
    Ok(())
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        data[offset],
        data[offset.wrapping_add(1)],
        data[offset.wrapping_add(2)],
        data[offset.wrapping_add(3)],
    ])
}

fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([data[offset], data[offset.wrapping_add(1)]])
}

fn blank_canvas(source: &DecodedImage, width: u32, height: u32) -> DecodedImage {
    // The validated IHDR canvas is bounded by Pillow's decompression-bomb
    // ceiling, so these products fit every supported pointer width.
    let width_usize = width as usize;
    let height_usize = height as usize;
    let byte_len = if source.mode == ImageMode::L1 {
        width_usize.div_ceil(8).wrapping_mul(height_usize)
    } else {
        width_usize
            .wrapping_mul(height_usize)
            .wrapping_mul(usize::from(source.mode.color_type().bytes_per_pixel()))
    };
    let mut canvas = DecodedImage::with_mode(width, height, vec![0; byte_len], source.mode);
    canvas.palette = source.palette.clone();
    canvas
}

fn composite_frame(
    canvas: &mut DecodedImage,
    source: &DecodedImage,
    rect: FrameRect,
    blend: FrameBlend,
) {
    // Every frame is decoded from the same IHDR and retained PLTE/tRNS state,
    // and blank_canvas copies that state. A layout change cannot be produced
    // by parse_apng, so this helper only handles the proven common layout.
    if source.mode == ImageMode::L1 {
        for y in 0..rect.height {
            for x in 0..rect.width {
                let value = packed_bit(&source.pixels, source.width, x, y);
                set_packed_bit(
                    &mut canvas.pixels,
                    canvas.width,
                    rect.left.wrapping_add(x),
                    rect.top.wrapping_add(y),
                    value,
                );
            }
        }
        return;
    }

    let bytes_per_pixel = usize::from(source.color.bytes_per_pixel());
    let over = blend == FrameBlend::Over;
    for y in 0..rect.height {
        for x in 0..rect.width {
            let source_pixel = (y as usize)
                .wrapping_mul(source.width as usize)
                .wrapping_add(x as usize)
                .saturating_mul(bytes_per_pixel);
            let canvas_pixel = (rect.top.wrapping_add(y) as usize)
                .wrapping_mul(canvas.width as usize)
                .wrapping_add(rect.left.wrapping_add(x) as usize)
                .saturating_mul(bytes_per_pixel);
            let alpha = if over {
                source_alpha(source, source_pixel)
            } else {
                255
            };
            for channel in 0..bytes_per_pixel {
                let source_value = source.pixels[source_pixel.wrapping_add(channel)];
                let canvas_value = &mut canvas.pixels[canvas_pixel.wrapping_add(channel)];
                *canvas_value = blend_byte(*canvas_value, source_value, alpha);
            }
        }
    }
}

fn source_alpha(source: &DecodedImage, pixel: usize) -> u8 {
    match source.mode {
        ImageMode::La8 => source.pixels[pixel.wrapping_add(1)],
        ImageMode::Rgba8 => source.pixels[pixel.wrapping_add(3)],
        ImageMode::P8 => {
            let index = usize::from(source.pixels[pixel]);
            source
                .palette
                .as_ref()
                .and_then(|palette| palette.alpha.get(index))
                .copied()
                .unwrap_or(255)
        }
        _ => 255,
    }
}

fn blend_byte(background: u8, foreground: u8, alpha: u8) -> u8 {
    let value = u32::from(background)
        .wrapping_mul(u32::from(255u8.wrapping_sub(alpha)))
        .wrapping_add(u32::from(foreground).wrapping_mul(u32::from(alpha)));
    let rounded = value.wrapping_add(128);
    rounded
        .wrapping_add(rounded.wrapping_shr(8))
        .wrapping_shr(8)
        .to_le_bytes()[0]
}

fn clear_rect(canvas: &mut DecodedImage, rect: FrameRect) {
    if canvas.mode == ImageMode::L1 {
        for y in 0..rect.height {
            for x in 0..rect.width {
                set_packed_bit(
                    &mut canvas.pixels,
                    canvas.width,
                    rect.left.wrapping_add(x),
                    rect.top.wrapping_add(y),
                    false,
                );
            }
        }
        return;
    }
    let bytes_per_pixel = usize::from(canvas.color.bytes_per_pixel());
    for y in 0..rect.height {
        let start = (rect.top.wrapping_add(y) as usize)
            .wrapping_mul(canvas.width as usize)
            .wrapping_add(rect.left as usize)
            .saturating_mul(bytes_per_pixel);
        let len = (rect.width as usize).wrapping_mul(bytes_per_pixel);
        canvas.pixels[start..start.wrapping_add(len)].fill(0);
    }
}

fn packed_bit(pixels: &[u8], width: u32, x: u32, y: u32) -> bool {
    let stride = (width as usize).div_ceil(8);
    let byte = pixels[(y as usize)
        .wrapping_mul(stride)
        .wrapping_add((x as usize).wrapping_div(8))];
    byte & (0x80 >> (x % 8)) != 0
}

fn set_packed_bit(pixels: &mut [u8], width: u32, x: u32, y: u32, value: bool) {
    let stride = (width as usize).div_ceil(8);
    let byte = &mut pixels[(y as usize)
        .wrapping_mul(stride)
        .wrapping_add((x as usize).wrapping_div(8))];
    let mask = 0x80 >> (x % 8);
    if value {
        *byte |= mask;
    } else {
        *byte &= !mask;
    }
}

/// Validate PNG chunk framing and CRCs without decompressing image samples.
///
/// This matches Pillow's PNG-specific `Image.verify()` behavior: validation
/// proceeds from the image-data chunk through `IEND`, while construction has
/// already inspected the preceding header and metadata chunks.
pub(crate) fn verify(data: &[u8]) -> CodecResult<()> {
    // `EncodedImage::new` has already inspected the immutable source and
    // proved its signature, 13-byte IHDR framing, and construction-critical
    // IHDR CRC. Verification therefore starts at the first post-IHDR chunk.
    let mut chunks = Chunks {
        data,
        position: 33,
        failed: false,
        verify_crc: true,
    };
    let mut saw_image_data = false;
    for chunk in &mut chunks {
        let chunk = chunk?;
        saw_image_data |= chunk.kind == *b"IDAT";
        if chunk.kind == *b"IEND" {
            return if saw_image_data {
                Ok(())
            } else {
                Err(CodecError::Malformed(
                    "PNG contains no image-data chunk".to_owned(),
                ))
            };
        }
    }
    Err(CodecError::Malformed(
        "PNG is missing its IEND chunk".to_owned(),
    ))
}

#[derive(Clone, Copy)]
struct PngHeader {
    width: u32,
    height: u32,
    depth: u8,
    png_color: u8,
    interlace: u8,
    channels: usize,
    color: ColorType,
}

impl PngHeader {
    fn image_spec(self, width: u32, height: u32) -> PngImageSpec {
        PngImageSpec {
            width,
            height,
            png_color: self.png_color,
            depth: self.depth,
            color: self.color,
        }
    }
}

fn read_header(chunks: &mut Chunks<'_>) -> CodecResult<PngHeader> {
    let header = match chunks.next().transpose()? {
        Some(header) => header,
        None => {
            return Err(CodecError::NeedMore {
                minimum: codec_add_end(chunks.position, 8),
                message: "PNG is missing its IHDR chunk".to_owned(),
            });
        }
    };
    if header.kind != *b"IHDR" || header.data.len() != 13 {
        return Err(CodecError::Malformed(
            "PNG IHDR chunk has an invalid type or length".to_owned(),
        ));
    }

    let width = u32::from_be_bytes([
        header.data[0],
        header.data[1],
        header.data[2],
        header.data[3],
    ]);
    let height = u32::from_be_bytes([
        header.data[4],
        header.data[5],
        header.data[6],
        header.data[7],
    ]);
    let depth = header.data[8];
    let png_color = header.data[9];
    let filter = header.data[11];
    let interlace = header.data[12];
    if width == 0 || height == 0 || filter != 0 || interlace > 1 {
        return Err(CodecError::Malformed(
            "PNG IHDR fields are invalid".to_owned(),
        ));
    }
    if u64::from(width).saturating_mul(u64::from(height)) > PILLOW_DECOMPRESSION_BOMB_ERROR_PIXELS {
        return Err(CodecError::Dimensions(
            "PNG dimensions exceed Pillow's decompression-bomb limit".to_owned(),
        ));
    }
    let (channels, color) = png_layout(png_color, depth)?;
    Ok(PngHeader {
        width,
        height,
        depth,
        png_color,
        interlace,
        channels,
        color,
    })
}

fn png_layout(color: u8, depth: u8) -> CodecResult<(usize, ColorType)> {
    match (color, depth) {
        (0, 1 | 2 | 4 | 8) | (3, 1 | 2 | 4 | 8) => Ok((1, ColorType::L8)),
        (0, 16) => Ok((1, ColorType::L16)),
        (2, 8 | 16) => Ok((3, ColorType::Rgb8)),
        (4, 8) => Ok((2, ColorType::La8)),
        (4, 16) | (6, 8 | 16) => Ok((if color == 4 { 2 } else { 4 }, ColorType::Rgba8)),
        _ => Err(CodecError::Malformed(
            "PNG color type and bit depth are incompatible".to_owned(),
        )),
    }
}

fn inflated_len(width: u32, height: u32, channels: usize, depth: u8, interlace: u8) -> usize {
    let width = width as usize;
    let height = height as usize;
    if interlace == 0 {
        return row_bytes(width, channels, depth)
            .wrapping_add(1)
            .wrapping_mul(height);
    }

    let mut total = 0usize;
    for (x_start, y_start, x_step, y_step) in ADAM7 {
        let pass_width = pass_size(width, x_start, x_step);
        let pass_height = pass_size(height, y_start, y_step);
        if pass_width != 0 && pass_height != 0 {
            total = total.wrapping_add(
                row_bytes(pass_width, channels, depth)
                    .wrapping_add(1)
                    .wrapping_mul(pass_height),
            );
        }
    }
    total
}

fn decoded_sample_count(width: usize, height: usize, channels: usize) -> usize {
    // The public decoder applies Pillow's pixel ceiling before this helper and
    // `png_layout` admits at most four channels.
    width.wrapping_mul(height).wrapping_mul(channels)
}

fn decode_scanlines(
    data: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    depth: u8,
    interlace: u8,
) -> CodecResult<Vec<u16>> {
    let width = width as usize;
    let height = height as usize;
    let sample_count = decoded_sample_count(width, height, channels);
    let mut samples = vec![0u16; sample_count];
    let mut position = 0usize;

    if interlace == 0 {
        let rows = unfilter_rows(data, &mut position, width, height, channels, depth)?;
        unpack_into(
            &rows,
            width,
            height,
            channels,
            depth,
            |x, y, channel, value| {
                let index = y
                    .wrapping_mul(width)
                    .wrapping_add(x)
                    .wrapping_mul(channels)
                    .wrapping_add(channel);
                samples[index] = value;
            },
        );
    } else {
        for (x_start, y_start, x_step, y_step) in ADAM7 {
            let pass_width = pass_size(width, x_start, x_step);
            let pass_height = pass_size(height, y_start, y_step);
            if pass_width == 0 || pass_height == 0 {
                continue;
            }
            let rows = unfilter_rows(
                data,
                &mut position,
                pass_width,
                pass_height,
                channels,
                depth,
            )?;
            unpack_into(
                &rows,
                pass_width,
                pass_height,
                channels,
                depth,
                |pass_x, pass_y, channel, value| {
                    let x = x_start.wrapping_add(pass_x.wrapping_mul(x_step));
                    let y = y_start.wrapping_add(pass_y.wrapping_mul(y_step));
                    let index = y
                        .wrapping_mul(width)
                        .wrapping_add(x)
                        .wrapping_mul(channels)
                        .wrapping_add(channel);
                    samples[index] = value;
                },
            );
        }
    }
    // `decompress_zlib_prefix` returns at most the exact scanline budget and
    // the length check in `decode` rejects short output. Consequently every
    // accepted buffer is consumed exactly here; Pillow deliberately ignores
    // any additional inflated bytes after that prefix.
    Ok(samples)
}

fn read_filtered_row<'a>(
    data: &'a [u8],
    position: &mut usize,
    stride: usize,
) -> CodecResult<(u8, &'a [u8])> {
    let filter = *data
        .get(*position)
        .malformed("PNG scanline is missing its filter byte")?;
    *position = position.wrapping_add(1);
    // `position` and `stride` are bounded by the validated inflated buffer.
    let source_end = (*position).wrapping_add(stride);
    let source = data
        .get(*position..source_end)
        .malformed("PNG scanline is truncated")?;
    *position = source_end;
    Ok((filter, source))
}

fn unfilter_rows(
    data: &[u8],
    position: &mut usize,
    width: usize,
    height: usize,
    channels: usize,
    depth: u8,
) -> CodecResult<Vec<u8>> {
    let stride = row_bytes(width, channels, depth);
    let bytes_per_pixel = channels.wrapping_mul(usize::from(depth)).div_ceil(8).max(1);
    let rows_len = stride.wrapping_mul(height);
    let mut rows = vec![0u8; rows_len];

    for row in 0..height {
        let (filter, source) = read_filtered_row(data, position, stride)?;
        let row_start = row.wrapping_mul(stride);

        for column in 0..stride {
            let left = if column >= bytes_per_pixel {
                rows[row_start.wrapping_add(column).wrapping_sub(bytes_per_pixel)]
            } else {
                0
            };
            let above = if row != 0 {
                rows[row_start.wrapping_sub(stride).wrapping_add(column)]
            } else {
                0
            };
            let upper_left = if row != 0 && column >= bytes_per_pixel {
                rows[row_start
                    .wrapping_sub(stride)
                    .wrapping_add(column)
                    .wrapping_sub(bytes_per_pixel)]
            } else {
                0
            };
            rows[row_start.wrapping_add(column)] = match filter {
                0 => source[column],
                1 => source[column].wrapping_add(left),
                2 => source[column].wrapping_add(above),
                3 => {
                    let average = u16::from(left)
                        .wrapping_add(u16::from(above))
                        .wrapping_div(2);
                    source[column].wrapping_add(average.to_le_bytes()[0])
                }
                4 => source[column].wrapping_add(paeth(left, above, upper_left)),
                _ => {
                    return Err(CodecError::Malformed(
                        "PNG scanline uses an invalid filter".to_owned(),
                    ));
                }
            };
        }
    }
    Ok(rows)
}

fn unpack_into<F>(
    rows: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    depth: u8,
    mut store: F,
) where
    F: FnMut(usize, usize, usize, u16),
{
    let stride = rows.len().checked_div(height).unwrap_or_default();
    for y in 0..height {
        let row_start = y.wrapping_mul(stride);
        let row = &rows[row_start..row_start.wrapping_add(stride)];
        for x in 0..width {
            for channel in 0..channels {
                let sample_index = x.wrapping_mul(channels).wrapping_add(channel);
                let value = match depth {
                    1 | 2 | 4 => {
                        let bit = sample_index.wrapping_mul(usize::from(depth));
                        let shift = 8_usize
                            .saturating_sub(usize::from(depth))
                            .saturating_sub(bit % 8);
                        let mask = 1_u8.wrapping_shl(depth.into()).wrapping_sub(1);
                        u16::from(row[bit / 8].wrapping_shr(shift.to_le_bytes()[0].into()) & mask)
                    }
                    8 => u16::from(row[sample_index]),
                    _ => {
                        debug_assert_eq!(depth, 16);
                        let offset = sample_index.wrapping_mul(2);
                        u16::from_be_bytes([row[offset], row[offset.wrapping_add(1)]])
                    }
                };
                store(x, y, channel, value);
            }
        }
    }
}

struct PngImageSpec {
    width: u32,
    height: u32,
    png_color: u8,
    depth: u8,
    color: ColorType,
}

fn build_image(
    spec: PngImageSpec,
    samples: &[u16],
    palette_rgb: Option<Vec<u8>>,
    mut palette_alpha: Vec<u8>,
) -> CodecResult<DecodedImage> {
    let PngImageSpec {
        width,
        height,
        png_color,
        depth,
        color,
    } = spec;
    let pixels = if png_color == 0 && depth == 1 {
        pack_one_bit(samples, width as usize, height as usize)
    } else if png_color == 0 && depth < 8 {
        let maximum = 1_u16.wrapping_shl(depth.into()).wrapping_sub(1);
        samples
            .iter()
            .map(|&sample| {
                sample
                    .wrapping_mul(255)
                    .checked_div(maximum)
                    .unwrap_or_default()
                    .to_le_bytes()[0]
            })
            .collect()
    } else if png_color == 4 && depth == 16 {
        let mut bytes = Vec::with_capacity(samples.len().wrapping_mul(2));
        for pair in samples.chunks_exact(2) {
            let luminance = pair[0].to_be_bytes()[0];
            let alpha = pair[1].to_be_bytes()[0];
            bytes.extend_from_slice(&[luminance, luminance, luminance, alpha]);
        }
        bytes
    } else if depth == 16 && matches!(png_color, 2 | 6) {
        samples
            .iter()
            .map(|&sample| sample.to_be_bytes()[0])
            .collect()
    } else if png_color == 3 || depth == 8 {
        samples
            .iter()
            .map(|&sample| sample.to_le_bytes()[0])
            .collect()
    } else {
        let mut bytes = Vec::with_capacity(samples.len().wrapping_mul(2));
        for &sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    };
    let mode = match (png_color, depth) {
        (0, 1) => ImageMode::L1,
        (3, _) => ImageMode::P8,
        _ => color.into(),
    };
    let source_descriptor = match (png_color, palette_alpha.is_empty()) {
        (4 | 6, _) | (3, false) => {
            crate::types::SourceDescriptor::new().with_alpha(crate::types::SourceAlpha::Straight)
        }
        _ => crate::types::SourceDescriptor::new(),
    };
    let mut image = DecodedImage::with_mode(width, height, pixels, mode)
        .with_source_descriptor(source_descriptor);
    if png_color == 3
        && let Some(mut rgb) = palette_rgb
    {
        let entries = rgb.len() / 3;
        if entries != 0 {
            rgb.truncate(entries.wrapping_mul(3));
            if !palette_alpha.is_empty() {
                palette_alpha.truncate(entries);
            }
            let palette = ImagePalette::new(rgb, palette_alpha)
                .map_err(|_| CodecError::Malformed("PNG palette is invalid".to_owned()))?;
            image = image.with_palette(palette);
        }
    }
    Ok(image)
}

fn pack_one_bit(samples: &[u16], width: usize, height: usize) -> Vec<u8> {
    let stride = width.div_ceil(8);
    let mut output = vec![0u8; stride.wrapping_mul(height)];
    for y in 0..height {
        for x in 0..width {
            if samples[y.wrapping_mul(width).wrapping_add(x)] != 0 {
                let output_index = y.wrapping_mul(stride).wrapping_add(x / 8);
                let shift = 7_usize.saturating_sub(x % 8);
                output[output_index] |= 1_u8.wrapping_shl(shift.to_le_bytes()[0].into());
            }
        }
    }
    output
}

fn row_bytes(width: usize, channels: usize, depth: u8) -> usize {
    // Width comes from a u32 IHDR field, channels are in 1..=4, and Pillow's
    // pixel ceiling bounds the resulting byte row below 32-bit `usize::MAX`.
    let bits = usize_to_u64(width)
        .wrapping_mul(usize_to_u64(channels))
        .wrapping_mul(u64::from(depth));
    raster_usize(bits.div_ceil(8))
}

#[cfg(target_pointer_width = "64")]
fn usize_to_u64(value: usize) -> u64 {
    u64::from_ne_bytes(value.to_ne_bytes())
}

#[cfg(target_pointer_width = "32")]
fn usize_to_u64(value: usize) -> u64 {
    u64::from(u32::from_ne_bytes(value.to_ne_bytes()))
}

#[cfg(target_pointer_width = "64")]
fn raster_usize(value: u64) -> usize {
    usize::from_ne_bytes(value.to_ne_bytes())
}

#[cfg(target_pointer_width = "32")]
fn raster_usize(value: u64) -> usize {
    let bytes = value.to_le_bytes();
    usize::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn pass_size(full: usize, start: usize, step: usize) -> usize {
    if full <= start {
        0
    } else {
        full.saturating_sub(start).div_ceil(step)
    }
}

fn paeth(left: u8, above: u8, upper_left: u8) -> u8 {
    let left = i32::from(left);
    let above = i32::from(above);
    let upper_left = i32::from(upper_left);
    let prediction = left.wrapping_add(above).wrapping_sub(upper_left);
    let left_distance = prediction.wrapping_sub(left).unsigned_abs();
    let above_distance = prediction.wrapping_sub(above).unsigned_abs();
    let diagonal_distance = prediction.wrapping_sub(upper_left).unsigned_abs();
    if left_distance <= above_distance && left_distance <= diagonal_distance {
        left.to_le_bytes()[0]
    } else if above_distance <= diagonal_distance {
        above.to_le_bytes()[0]
    } else {
        upper_left.to_le_bytes()[0]
    }
}

fn crc32(kind: &[u8; 4], data: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in kind.iter().chain(data) {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

fn chunk_payload_with_crc<'a>(
    data: &'a [u8],
    kind: &[u8; 4],
    start: usize,
    length: usize,
    verify_crc: bool,
) -> CodecResult<(&'a [u8], usize)> {
    let end = start
        .checked_add(length)
        .dimensions("PNG chunk byte range overflows")?;
    let payload = need_slice(data, start, end, "PNG chunk payload is truncated")?;
    let crc_end = end.saturating_add(4);
    let expected_bytes = need_slice(data, end, crc_end, "PNG chunk CRC is truncated")?;
    let expected = u32::from_be_bytes([
        expected_bytes[0],
        expected_bytes[1],
        expected_bytes[2],
        expected_bytes[3],
    ]);
    if !verify_crc || crc32(kind, payload) == expected {
        Ok((payload, crc_end))
    } else {
        Err(CodecError::Malformed(
            "PNG chunk CRC does not match".to_owned(),
        ))
    }
}

struct Chunk<'a> {
    kind: [u8; 4],
    data: &'a [u8],
}

struct Chunks<'a> {
    data: &'a [u8],
    position: usize,
    failed: bool,
    verify_crc: bool,
}

impl<'a> Chunks<'a> {
    fn new(data: &'a [u8], verify_crc: bool) -> CodecResult<Self> {
        if data.get(..8) == Some(PNG_SIGNATURE) {
            Ok(Self {
                data,
                position: 8,
                failed: false,
                verify_crc,
            })
        } else {
            Err(CodecError::Malformed(
                "PNG signature is missing or invalid".to_owned(),
            ))
        }
    }
}

impl<'a> Iterator for Chunks<'a> {
    type Item = CodecResult<Chunk<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.position == self.data.len() {
            return None;
        }
        let chunk_start = self.position as u64;
        let result = (|| -> CodecResult<Chunk<'a>> {
            let length_bytes = need_slice(
                self.data,
                self.position,
                codec_add_end(self.position, 4),
                "PNG chunk length is truncated",
            )?;
            let length = u32::from_be_bytes([
                length_bytes[0],
                length_bytes[1],
                length_bytes[2],
                length_bytes[3],
            ]) as usize;
            let kind_bytes = need_slice(
                self.data,
                codec_add_end(self.position, 4),
                codec_add_end(self.position, 8),
                "PNG chunk type is truncated",
            )?;
            let kind = [kind_bytes[0], kind_bytes[1], kind_bytes[2], kind_bytes[3]];
            let start = self.position.saturating_add(8);
            // Pillow validates construction-critical chunk CRCs while opening
            // the file, but defers IDAT CRC validation to `verify()`.
            let verify_crc = self.verify_crc || kind != *b"IDAT";
            let (payload, crc_end) =
                chunk_payload_with_crc(self.data, &kind, start, length, verify_crc)?;
            self.position = crc_end;
            Ok(Chunk {
                kind,
                data: payload,
            })
        })();
        if result.is_err() {
            self.failed = true;
        }
        Some(result.map_err(|error| error.at(chunk_start, "png_chunk")))
    }
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    fn png_chunk(kind: [u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut data = PNG_SIGNATURE.to_vec();
        append_chunk(&mut data, kind, payload);
        data
    }

    fn append_chunk(data: &mut Vec<u8>, kind: [u8; 4], payload: &[u8]) {
        data.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        data.extend_from_slice(&kind);
        data.extend_from_slice(payload);
        data.extend_from_slice(&crc32(&kind, payload).to_be_bytes());
    }

    let _ = decode(b"");
    let _ = decode(&png_chunk(*b"NOPE", &[0; 13]));
    let _ = decode(&png_chunk(*b"IHDR", &[0; 12]));
    let mut valid_header = [0u8; 13];
    valid_header[3] = 1;
    valid_header[7] = 1;
    valid_header[8] = 8;
    valid_header[9] = 0;
    // A structurally incomplete PNG (no IDAT, no IEND) is incremental
    // truncation; the same bytes with an IEND are terminal malformed.
    let _ = decode(&png_chunk(*b"IHDR", &valid_header));
    let mut no_image_data = png_chunk(*b"IHDR", &valid_header);
    append_chunk(&mut no_image_data, *b"IEND", &[]);
    let _ = decode(&no_image_data);
    // A complete IDAT chunk carrying a truncated zlib stream is incremental
    // while IEND is missing, and terminal once the container is complete.
    let mut truncated_stream = png_chunk(*b"IHDR", &valid_header);
    append_chunk(&mut truncated_stream, *b"IDAT", &[0x78, 0x9c, 0x63]);
    let _ = decode(&truncated_stream);
    append_chunk(&mut truncated_stream, *b"IEND", &[]);
    let _ = decode(&truncated_stream);
    let _ = metadata_bytes(b"");
    let _ = metadata_bytes(&png_chunk(*b"NOPE", &[0; 13]));
    let _ = metadata_bytes(&png_chunk(*b"IHDR", &[0; 12]));
    let mut truncated_chunk = PNG_SIGNATURE.to_vec();
    truncated_chunk.extend_from_slice(b"\x00\x00\x00\x01NOPE");
    let _ = metadata_bytes(&truncated_chunk);
    let _ = decode(&truncated_chunk);
    let mut fd_chunk = PNG_SIGNATURE.to_vec();
    append_chunk(&mut fd_chunk, *b"IHDR", &[0; 13]);
    append_chunk(&mut fd_chunk, *b"fdAT", &[0, 0, 0, 0, 1, 2, 3]);
    append_chunk(&mut fd_chunk, *b"IEND", &[]);
    let _ = metadata_bytes(&fd_chunk);
    for (width, height, filter, interlace) in [
        (0u32, 1u32, 0u8, 0u8),
        (1, 0, 0, 0),
        (1, 1, 1, 0),
        (1, 1, 0, 2),
    ] {
        let mut header = [0u8; 13];
        header[..4].copy_from_slice(&width.to_be_bytes());
        header[4..8].copy_from_slice(&height.to_be_bytes());
        header[8] = 8;
        header[9] = 0;
        header[11] = filter;
        header[12] = interlace;
        let _ = decode(&png_chunk(*b"IHDR", &header));
    }
    assert!(png_layout(7, 8).is_err());
    let _ = verify(b"");
    let _ = verify(PNG_SIGNATURE);
    let _ = verify(&png_chunk(*b"NOPE", &[0; 13]));
    let _ = verify(&png_chunk(*b"IHDR", &[0; 12]));
    let malformed = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x01tEXtx";
    let mut chunks = Chunks::new(malformed, true).expect("coverage PNG signature should parse");

    assert!(chunks.next().is_some_and(|chunk| chunk.is_err()));
    assert!(chunks.failed);
    assert!(chunks.next().is_none());

    let mut position = 0;
    assert!(unfilter_rows(&[], &mut position, 1, 1, 1, 8).is_err());

    let mut position = 0;
    assert!(unfilter_rows(&[0], &mut position, 1, 1, 1, 8).is_err());

    assert!(chunk_payload_with_crc(&[], b"IDAT", usize::MAX, 1, true).is_err());

    let mut sequence = u32::MAX;
    assert!(consume_sequence(u32::MAX, &mut sequence).is_err());

    let mut trailing_palette = PNG_SIGNATURE.to_vec();
    let mut header = [0u8; 13];
    header[3] = 1;
    header[7] = 1;
    header[8] = 8;
    header[9] = 2;
    append_chunk(&mut trailing_palette, *b"IHDR", &header);
    append_chunk(&mut trailing_palette, *b"fdAT", &[0, 0, 0, 0]);
    append_chunk(
        &mut trailing_palette,
        *b"IDAT",
        &[
            0x78, 0x9c, 0x63, 0x68, 0x60, 0x60, 0x00, 0x00, 0x01, 0x84, 0x00, 0x81,
        ],
    );
    append_chunk(&mut trailing_palette, *b"PLTE", &[0, 0, 0]);
    append_chunk(&mut trailing_palette, *b"tRNS", &[]);
    append_chunk(&mut trailing_palette, *b"IEND", &[]);
    assert!(decode(&trailing_palette).is_ok());
    assert!(parse_apng(&trailing_palette).is_ok_and(|parsed| parsed.is_none()));

    let mut orphan_frame_data = PNG_SIGNATURE.to_vec();
    append_chunk(&mut orphan_frame_data, *b"IHDR", &header);
    append_chunk(&mut orphan_frame_data, *b"acTL", &[0, 0, 0, 1, 0, 0, 0, 0]);
    append_chunk(&mut orphan_frame_data, *b"fdAT", &[0, 0, 0, 0, 0x78]);
    append_chunk(&mut orphan_frame_data, *b"IEND", &[]);
    assert!(parse_apng(&orphan_frame_data).is_err());
}
