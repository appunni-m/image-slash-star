//! GIF87a/GIF89a still-image and animation decoder.
//!
//! Frames retain palette indices, palette tables, timing, offsets, disposal,
//! and loop metadata required for deterministic re-encoding.

use crate::SequenceDecodeBudget;
use crate::codecs::{CodecError, CodecResult, OptionCodecExt};
use crate::types::{
    AnimationBackground, DecodedFrame, DecodedImage, DecodedSequence, FrameBlend, FrameDisposal,
    FrameDuration, ImageMode, ImagePalette,
};

const IMAGE_SEPARATOR: u8 = 0x2c;
const EXTENSION_INTRODUCER: u8 = 0x21;
const TRAILER: u8 = 0x3b;
const MAX_LZW_CODE: usize = 4096;
const PILLOW_DECOMPRESSION_BOMB_ERROR_PIXELS: u64 = 178_956_970;

/// Decode the first image frame in a GIF87a or GIF89a stream.
///
pub fn decode(data: &[u8]) -> CodecResult<(DecodedImage, usize)> {
    let (mut sequence, consumed) = decode_sequence(
        data,
        &mut SequenceDecodeBudget::default_for(crate::ImageFormat::Gif),
    )?;
    // `decode_sequence` rejects a GIF without an image descriptor before
    // constructing its return value, so the first frame is a local invariant.
    Ok((sequence.frames.remove(0).image, consumed))
}

/// Decode every image descriptor and its presentation metadata.
pub fn decode_sequence(
    data: &[u8],
    budget: &mut SequenceDecodeBudget,
) -> CodecResult<(DecodedSequence, usize)> {
    let mut input = Input::new(data);
    let signature = input.read_bytes(6)?;
    if signature != b"GIF87a" && signature != b"GIF89a" {
        return Err(CodecError::Malformed("invalid GIF signature".to_owned()));
    }

    let logical_width = input.read_u16()?;
    let logical_height = input.read_u16()?;
    let packed = input.read_u8()?;
    let background_index = input.read_u8()?;
    input.skip(1)?; // Pixel aspect ratio.

    let global_palette = if packed & 0x80 != 0 {
        Some(input.read_bytes(color_table_len(packed))?.to_vec())
    } else {
        None
    };
    let mut graphic_control = GraphicControl::default();
    let mut frames = Vec::new();
    let mut loop_count = None;
    let mut recovering_from_bad_gce = false;

    loop {
        match input.read_u8()? {
            EXTENSION_INTRODUCER => {
                let label = input.read_u8()?;
                if label == 0xf9 {
                    (graphic_control, recovering_from_bad_gce) = read_graphic_control(&mut input)?;
                } else if label == 0xff {
                    let identifier_len = usize::from(input.read_u8()?);
                    let identifier = input.read_bytes(identifier_len)?;
                    let payload = input.read_sub_blocks()?;
                    // Pillow 12.2.0 recognizes only NETSCAPE2.0 here. It
                    // exposes ANIMEXTS1.0 as an opaque application extension.
                    let is_loop_extension = identifier == b"NETSCAPE2.0";
                    if is_loop_extension && payload.len() >= 3 && payload[0] == 1 {
                        let bytes = [payload[1], payload[2]];
                        loop_count = Some(u32::from(u16::from_le_bytes(bytes)));
                    }
                } else {
                    input.skip_sub_blocks()?;
                }
            }
            IMAGE_SEPARATOR => {
                let (image, left, top, interlaced) = decode_image(
                    &mut input,
                    global_palette.as_deref(),
                    graphic_control.transparent_index,
                    if frames.is_empty() {
                        None
                    } else {
                        Some(&mut *budget)
                    },
                )?;
                frames.push(DecodedFrame::source_rectangle(
                    image,
                    u32::from(left),
                    u32::from(top),
                    FrameDuration {
                        numerator: u64::from(graphic_control.delay_cs),
                        denominator: 100,
                    },
                    graphic_control.disposal,
                    FrameBlend::Unspecified,
                    interlaced,
                ));
                graphic_control = GraphicControl::default();
                recovering_from_bad_gce = false;
            }
            TRAILER => break,
            _ if recovering_from_bad_gce => {}
            _ => {
                return Err(CodecError::Malformed("unknown GIF block marker".to_owned()));
            }
        }
    }

    let first_frame = frames.first().malformed("GIF contains no image frame")?;
    let mut fallback_width = first_frame
        .source
        .rect
        .left
        .saturating_add(first_frame.image.width);
    let mut fallback_height = first_frame
        .source
        .rect
        .top
        .saturating_add(first_frame.image.height);
    for frame in &frames[1..] {
        fallback_width =
            fallback_width.max(frame.source.rect.left.saturating_add(frame.image.width));
        fallback_height =
            fallback_height.max(frame.source.rect.top.saturating_add(frame.image.height));
    }
    let logical_width = u32::from(logical_width);
    let logical_height = u32::from(logical_height);
    let consumed = input.position();
    let sequence = DecodedSequence {
        width: logical_width.max(fallback_width),
        height: logical_height.max(fallback_height),
        frames,
        loop_count,
        background: Some(AnimationBackground::PaletteIndex(background_index)),
    };
    Ok((sequence, consumed))
}

#[derive(Clone, Copy)]
struct GraphicControl {
    delay_cs: u16,
    transparent_index: Option<u8>,
    disposal: FrameDisposal,
}

impl Default for GraphicControl {
    fn default() -> Self {
        Self {
            delay_cs: 0,
            transparent_index: None,
            disposal: FrameDisposal::Unspecified,
        }
    }
}

fn read_graphic_control(input: &mut Input<'_>) -> CodecResult<(GraphicControl, bool)> {
    let _declared_size = input.read_u8()?;
    let packed = input.read_u8()?;
    let delay_cs = input.read_u16()?;
    let index = input.read_u8()?;
    let terminator = input.read_u8()?;
    let recovering = terminator != 0;
    if recovering {
        // Pillow treats the byte as another sub-block length, consumes that
        // payload and its terminating sub-block, then scans for a recognized
        // marker. Retaining that behavior is observable for malformed files:
        // the resynchronized descriptor can trigger the decompression limit.
        input.skip(usize::from(terminator))?;
        input.skip_sub_blocks()?;
    }
    let disposal = match (packed >> 2) & 7 {
        0 => FrameDisposal::Unspecified,
        1 => FrameDisposal::Keep,
        2 => FrameDisposal::Background,
        3 => FrameDisposal::Previous,
        value => FrameDisposal::Reserved(value),
    };
    Ok((
        GraphicControl {
            delay_cs,
            transparent_index: (packed & 1 != 0).then_some(index),
            disposal,
        },
        recovering,
    ))
}

fn decode_image(
    input: &mut Input<'_>,
    global_palette: Option<&[u8]>,
    transparent_index: Option<u8>,
    budget: Option<&mut SequenceDecodeBudget>,
) -> CodecResult<(DecodedImage, u16, u16, bool)> {
    let left = input.read_u16()?;
    let top = input.read_u16()?;
    let width = input.read_u16()?;
    let height = input.read_u16()?;
    if width == 0 || height == 0 {
        return Err(CodecError::Dimensions(
            "GIF frame dimensions must be nonzero".to_owned(),
        ));
    }
    if u64::from(width).saturating_mul(u64::from(height)) > PILLOW_DECOMPRESSION_BOMB_ERROR_PIXELS {
        return Err(CodecError::Dimensions(
            "GIF frame expands beyond Pillow's decompression-bomb limit".to_owned(),
        ));
    }

    let packed = input.read_u8()?;
    let interlaced = packed & 0x40 != 0;
    let local_palette = if packed & 0x80 != 0 {
        Some(input.read_bytes(color_table_len(packed))?)
    } else {
        None
    };
    let palette_rgb = local_palette.or(global_palette);
    if let Some(budget) = budget {
        let mode = if palette_rgb.is_some() {
            ImageMode::P8
        } else {
            ImageMode::L8
        };
        budget
            .reserve_later_frame(mode, u32::from(width), u32::from(height))
            .map_err(CodecError::LimitExceeded)?;
    }

    let minimum_code_size = input.read_u8()?;
    let compressed = input.read_sub_blocks()?;
    let pixel_count = usize::from(width).saturating_mul(usize::from(height));
    let mut indices = decode_lzw(&compressed, minimum_code_size, pixel_count)?;

    if interlaced {
        indices = deinterlace(&indices, usize::from(width), usize::from(height));
    }

    let image = if let Some(palette_rgb) = palette_rgb {
        let entries = palette_rgb.len().div_euclid(3);
        let required_entries = indices
            .iter()
            .copied()
            .map(usize::from)
            .max()
            .map_or(0, |index| index.saturating_add(1));
        let padded_entries = entries.max(required_entries);
        let mut rgb = palette_rgb.to_vec();
        rgb.resize(padded_entries.saturating_mul(3), 0);
        let mut alpha = Vec::new();
        if let Some(index) = transparent_index
            && usize::from(index) < padded_entries
        {
            alpha = vec![255; padded_entries];
            alpha[usize::from(index)] = 0;
        }
        let palette = ImagePalette { rgb, alpha };
        DecodedImage::with_mode(u32::from(width), u32::from(height), indices, ImageMode::P8)
            .with_palette(palette)
    } else {
        DecodedImage::with_mode(u32::from(width), u32::from(height), indices, ImageMode::L8)
    };
    Ok((image, left, top, interlaced))
}

fn color_table_len(packed: u8) -> usize {
    (1usize << (packed & 0x07).saturating_add(1)).saturating_mul(3)
}

/// Decode GIF's variable-width, least-significant-bit-first LZW stream.
///
/// The fixed-size prefix/suffix tables mirror the 12-bit dictionary described
/// by GIF89a Appendix F without allocating per-code strings.
fn decode_lzw(data: &[u8], minimum_code_size: u8, expected_len: usize) -> CodecResult<Vec<u8>> {
    if !(2..=8).contains(&minimum_code_size) {
        return Err(CodecError::Malformed(
            "invalid GIF LZW minimum code size".to_owned(),
        ));
    }

    let clear_code = 1u16 << minimum_code_size;
    let end_code = clear_code.saturating_add(1);
    let first_free_code = end_code.saturating_add(1);
    let mut code_size = minimum_code_size.saturating_add(1);
    let mut next_code = first_free_code;
    let mut previous_code = None;
    let mut prefixes = [0u16; MAX_LZW_CODE];
    let mut suffixes = [0u8; MAX_LZW_CODE];
    let mut stack = [0u8; MAX_LZW_CODE];
    let mut bits = BitReader::new(data);
    let mut output = Vec::with_capacity(expected_len);

    for value in 0..clear_code {
        suffixes[usize::from(value)] = value.to_le_bytes()[0];
    }

    loop {
        let code = bits.read(code_size)?;
        if code == clear_code {
            code_size = minimum_code_size.saturating_add(1);
            next_code = first_free_code;
            previous_code = None;
            continue;
        }
        if code == end_code {
            return if output.len() == expected_len {
                Ok(output)
            } else {
                Err(CodecError::Malformed(
                    "GIF LZW stream ended before filling the frame".to_owned(),
                ))
            };
        }

        let Some(previous) = previous_code else {
            if code >= clear_code || output.len() >= expected_len {
                return Err(CodecError::Malformed(
                    "invalid first GIF LZW code".to_owned(),
                ));
            }
            output.push(code.to_le_bytes()[0]);
            if output.len() == expected_len {
                return Ok(output);
            }
            previous_code = Some(code);
            continue;
        };

        let first = if code < next_code {
            append_code(
                code,
                clear_code,
                &prefixes,
                &suffixes,
                &mut stack,
                &mut output,
                expected_len,
            )
        } else if code == next_code {
            let first = append_code(
                previous,
                clear_code,
                &prefixes,
                &suffixes,
                &mut stack,
                &mut output,
                expected_len,
            );
            if output.len() < expected_len {
                output.push(first);
            }
            first
        } else {
            return Err(CodecError::Malformed(
                "GIF LZW code references a future dictionary entry".to_owned(),
            ));
        };

        if output.len() == expected_len {
            return Ok(output);
        }

        if usize::from(next_code) < MAX_LZW_CODE {
            prefixes[usize::from(next_code)] = previous;
            suffixes[usize::from(next_code)] = first;
            next_code = next_code.saturating_add(1);

            if code_size < 12 && next_code == (1u16 << code_size) {
                code_size = code_size.saturating_add(1);
            }
        }

        previous_code = Some(code);
    }
}

#[allow(clippy::too_many_arguments)]
fn append_code(
    mut code: u16,
    clear_code: u16,
    prefixes: &[u16; MAX_LZW_CODE],
    suffixes: &[u8; MAX_LZW_CODE],
    stack: &mut [u8; MAX_LZW_CODE],
    output: &mut Vec<u8>,
    expected_len: usize,
) -> u8 {
    let mut len = 0usize;
    while code >= clear_code {
        stack[len] = suffixes[usize::from(code)];
        len = len.saturating_add(1);
        code = prefixes[usize::from(code)];
    }

    let first = code.to_le_bytes()[0];
    debug_assert!(len < MAX_LZW_CODE);
    stack[len] = first;
    len = len.saturating_add(1);

    let remaining = expected_len.saturating_sub(output.len());
    output.extend(stack[..len].iter().rev().take(remaining));
    first
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let mut budget = SequenceDecodeBudget::default_for(crate::ImageFormat::Gif);
    assert!(decode_sequence(b"", &mut budget).is_err());
    assert!(decode_sequence(b"not gif", &mut budget).is_err());
    assert!(decode_sequence(b"GIF89a", &mut budget).is_err());
    assert!(decode_sequence(b"GIF89a\x01\0\x01\0\0\0\0\x7f", &mut budget).is_err());
    // Two 1x1 palette-less frames prove the later-frame L8 budget branch.
    let no_palette_two_frame =
        b"GIF89a\x01\0\x01\0\0\0\0\x2c\0\0\0\0\x01\0\x01\0\0\x02\x03\x44\x01\0\0\x2c\0\0\0\0\x01\0\x01\0\0\x02\x03\x44\x01\0\0\x3b";
    assert!(decode_sequence(no_palette_two_frame, &mut budget).is_ok());
    assert!(decode_lzw(&[0], 2, 0).is_err());
    assert_eq!(decode_lzw(&[0x2c], 2, 0), Ok(Vec::new()));
}

fn deinterlace(indices: &[u8], width: usize, height: usize) -> Vec<u8> {
    debug_assert_eq!(indices.len(), width.saturating_mul(height));

    let mut output = vec![0; indices.len()];
    let mut source_row = 0usize;
    for (start, step) in [(0usize, 8usize), (4, 8), (2, 4), (1, 2)] {
        for destination_row in (start..height).step_by(step) {
            let source_start = source_row.saturating_mul(width);
            let destination_start = destination_row.saturating_mul(width);
            output[destination_start..destination_start.saturating_add(width)]
                .copy_from_slice(&indices[source_start..source_start.saturating_add(width)]);
            source_row = source_row.saturating_add(1);
        }
    }
    debug_assert_eq!(source_row, height);
    output
}

struct Input<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> Input<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    fn read_u8(&mut self) -> CodecResult<u8> {
        let value = *self
            .data
            .get(self.position)
            .malformed("truncated GIF byte field")?;
        self.position = self.position.saturating_add(1);
        Ok(value)
    }

    fn position(&self) -> usize {
        self.position
    }

    fn read_u16(&mut self) -> CodecResult<u16> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_bytes(&mut self, len: usize) -> CodecResult<&'a [u8]> {
        if len > self.data.len().saturating_sub(self.position) {
            return Err(CodecError::Malformed("truncated GIF field".to_owned()));
        }
        let end = self.position.saturating_add(len);
        let bytes = &self.data[self.position..end];
        self.position = end;
        Ok(bytes)
    }

    fn skip(&mut self, len: usize) -> CodecResult<()> {
        self.read_bytes(len).map(|_| ())
    }

    fn read_sub_blocks(&mut self) -> CodecResult<Vec<u8>> {
        let mut output = Vec::new();
        loop {
            let len = usize::from(self.read_u8()?);
            if len == 0 {
                return Ok(output);
            }
            output.extend_from_slice(self.read_bytes(len)?);
        }
    }

    fn skip_sub_blocks(&mut self) -> CodecResult<()> {
        loop {
            let len = usize::from(self.read_u8()?);
            if len == 0 {
                return Ok(());
            }
            self.skip(len)?;
        }
    }
}

struct BitReader<'a> {
    data: &'a [u8],
    bit_position: usize,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            bit_position: 0,
        }
    }

    fn read(&mut self, width: u8) -> CodecResult<u16> {
        let end = self.bit_position.saturating_add(usize::from(width));
        if end > self.data.len().saturating_mul(8) {
            return Err(CodecError::Malformed(
                "truncated GIF LZW bitstream".to_owned(),
            ));
        }

        let mut value = 0u16;
        for shift in 0..width {
            let byte = self.data[self.bit_position.div_euclid(8)];
            let bit = (byte >> self.bit_position.rem_euclid(8)) & 1;
            value |= u16::from(bit) << shift;
            self.bit_position = self.bit_position.saturating_add(1);
        }
        Ok(value)
    }
}
