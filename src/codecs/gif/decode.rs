//! GIF87a/GIF89a still-image and animation decoder.
//!
//! Frames retain palette indices, palette tables, timing, offsets, disposal,
//! and loop metadata required for deterministic re-encoding.

use crate::types::{
    DecodedFrame, DecodedImage, DecodedSequence, FrameDisposal, ImageMode, ImagePalette,
};

const IMAGE_SEPARATOR: u8 = 0x2c;
const EXTENSION_INTRODUCER: u8 = 0x21;
const TRAILER: u8 = 0x3b;
const MAX_LZW_CODE: usize = 4096;

/// Decode the first image frame in a GIF87a or GIF89a stream.
///
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    decode_sequence(data)?
        .frames
        .into_iter()
        .next()
        .map(|frame| frame.image)
}

/// Decode every image descriptor and its presentation metadata.
pub fn decode_sequence(data: &[u8]) -> Option<DecodedSequence> {
    let mut input = Input::new(data);
    let signature = input.read_bytes(6)?;
    if signature != b"GIF87a" && signature != b"GIF89a" {
        return None;
    }

    let logical_width = input.read_u16()?;
    let logical_height = input.read_u16()?;
    let packed = input.read_u8()?;
    input.skip(2)?; // Background color index and pixel aspect ratio.

    let global_palette = if packed & 0x80 != 0 {
        Some(input.read_bytes(color_table_len(packed)?)?.to_vec())
    } else {
        None
    };
    let mut graphic_control = GraphicControl::default();
    let mut frames = Vec::new();
    let mut loop_count = None;

    loop {
        match input.read_u8()? {
            EXTENSION_INTRODUCER => {
                let label = input.read_u8()?;
                if label == 0xf9 {
                    graphic_control = read_graphic_control(&mut input)?;
                } else if label == 0xff {
                    let identifier_len = usize::from(input.read_u8()?);
                    let identifier = input.read_bytes(identifier_len)?;
                    let payload = input.read_sub_blocks()?;
                    let is_loop_extension = matches!(identifier, b"NETSCAPE2.0" | b"ANIMEXTS1.0");
                    if is_loop_extension && payload.first() == Some(&1) {
                        let bytes: [u8; 2] = payload.get(1..3)?.try_into().ok()?;
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
                )?;
                frames.push(DecodedFrame {
                    image,
                    left: u32::from(left),
                    top: u32::from(top),
                    duration_ms: u32::from(graphic_control.delay_cs).checked_mul(10)?,
                    disposal: graphic_control.disposal,
                    interlaced,
                });
                graphic_control = GraphicControl::default();
            }
            TRAILER => break,
            _ => return None,
        }
    }

    let fallback_width = frames
        .iter()
        .filter_map(|frame| frame.left.checked_add(frame.image.width))
        .max()?;
    let fallback_height = frames
        .iter()
        .filter_map(|frame| frame.top.checked_add(frame.image.height))
        .max()?;
    let sequence = DecodedSequence {
        width: if logical_width == 0 {
            fallback_width
        } else {
            u32::from(logical_width)
        },
        height: if logical_height == 0 {
            fallback_height
        } else {
            u32::from(logical_height)
        },
        frames,
        loop_count,
    };
    sequence.validate().ok()?;
    Some(sequence)
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

fn read_graphic_control(input: &mut Input<'_>) -> Option<GraphicControl> {
    let _declared_size = input.read_u8()?;
    let packed = input.read_u8()?;
    let delay_cs = input.read_u16()?;
    let index = input.read_u8()?;
    if input.read_u8()? != 0 {
        return None;
    }
    let disposal = match (packed >> 2) & 7 {
        0 => FrameDisposal::Unspecified,
        1 => FrameDisposal::Keep,
        2 => FrameDisposal::Background,
        3 => FrameDisposal::Previous,
        value => FrameDisposal::Reserved(value),
    };
    Some(GraphicControl {
        delay_cs,
        transparent_index: (packed & 1 != 0).then_some(index),
        disposal,
    })
}

fn decode_image(
    input: &mut Input<'_>,
    global_palette: Option<&[u8]>,
    transparent_index: Option<u8>,
) -> Option<(DecodedImage, u16, u16, bool)> {
    let left = input.read_u16()?;
    let top = input.read_u16()?;
    let width = input.read_u16()?;
    let height = input.read_u16()?;
    if width == 0 || height == 0 {
        return None;
    }

    let packed = input.read_u8()?;
    let interlaced = packed & 0x40 != 0;
    let local_palette = if packed & 0x80 != 0 {
        Some(input.read_bytes(color_table_len(packed)?)?)
    } else {
        None
    };
    let palette_rgb = local_palette.or(global_palette);

    let minimum_code_size = input.read_u8()?;
    let compressed = input.read_sub_blocks()?;
    let pixel_count = usize::from(width).checked_mul(usize::from(height))?;
    let mut indices = decode_lzw(&compressed, minimum_code_size, pixel_count)?;

    if interlaced {
        indices = deinterlace(&indices, usize::from(width), usize::from(height))?;
    }

    let image = if let Some(palette_rgb) = palette_rgb {
        let entries = palette_rgb.len() / 3;
        let mut alpha = Vec::new();
        if let Some(index) = transparent_index {
            if usize::from(index) < entries {
                alpha = vec![255; entries];
                alpha[usize::from(index)] = 0;
            }
        }
        let palette = ImagePalette::new(palette_rgb.to_vec(), alpha).ok()?;
        DecodedImage::with_mode(u32::from(width), u32::from(height), indices, ImageMode::P8)
            .with_palette(palette)
    } else {
        DecodedImage::with_mode(u32::from(width), u32::from(height), indices, ImageMode::L8)
    };
    Some((image, left, top, interlaced))
}

fn color_table_len(packed: u8) -> Option<usize> {
    let entries = 1usize.checked_shl(u32::from((packed & 0x07) + 1))?;
    entries.checked_mul(3)
}

/// Decode GIF's variable-width, least-significant-bit-first LZW stream.
///
/// The fixed-size prefix/suffix tables mirror the 12-bit dictionary described
/// by GIF89a Appendix F without allocating per-code strings.
fn decode_lzw(data: &[u8], minimum_code_size: u8, expected_len: usize) -> Option<Vec<u8>> {
    if !(2..=8).contains(&minimum_code_size) {
        return None;
    }

    let clear_code = 1u16.checked_shl(u32::from(minimum_code_size))?;
    let end_code = clear_code.checked_add(1)?;
    let first_free_code = end_code.checked_add(1)?;
    let mut code_size = minimum_code_size.checked_add(1)?;
    let mut next_code = first_free_code;
    let mut previous_code = None;
    let mut prefixes = [0u16; MAX_LZW_CODE];
    let mut suffixes = [0u8; MAX_LZW_CODE];
    let mut stack = [0u8; MAX_LZW_CODE];
    let mut bits = BitReader::new(data);
    let mut output = Vec::with_capacity(expected_len);

    for value in 0..clear_code {
        suffixes[usize::from(value)] = value as u8;
    }

    while let Some(code) = bits.read(code_size) {
        if code == clear_code {
            code_size = minimum_code_size.checked_add(1)?;
            next_code = first_free_code;
            previous_code = None;
            continue;
        }
        if code == end_code {
            return (output.len() == expected_len).then_some(output);
        }

        let Some(previous) = previous_code else {
            if code >= clear_code || output.len() >= expected_len {
                return None;
            }
            output.push(code as u8);
            if output.len() == expected_len {
                return Some(output);
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
            return None;
        };

        if output.len() == expected_len {
            return Some(output);
        }

        if usize::from(next_code) < MAX_LZW_CODE {
            prefixes[usize::from(next_code)] = previous;
            suffixes[usize::from(next_code)] = first;
            next_code = next_code.checked_add(1)?;

            if code_size < 12 && next_code == (1u16 << code_size) {
                code_size += 1;
            }
        }

        previous_code = Some(code);
    }

    None
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
        debug_assert!(usize::from(code) < MAX_LZW_CODE && len < MAX_LZW_CODE);
        stack[len] = suffixes[usize::from(code)];
        len += 1;
        code = prefixes[usize::from(code)];
    }

    let first = code as u8;
    debug_assert!(len < MAX_LZW_CODE);
    stack[len] = first;
    len += 1;

    let remaining = expected_len - output.len();
    output.extend(stack[..len].iter().rev().take(remaining));
    first
}

fn deinterlace(indices: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
    debug_assert_eq!(indices.len(), width.checked_mul(height)?);

    let mut output = vec![0; indices.len()];
    let mut source_row = 0usize;
    for (start, step) in [(0usize, 8usize), (4, 8), (2, 4), (1, 2)] {
        for destination_row in (start..height).step_by(step) {
            let source_start = source_row.checked_mul(width)?;
            let destination_start = destination_row.checked_mul(width)?;
            output
                .get_mut(destination_start..destination_start + width)?
                .copy_from_slice(indices.get(source_start..source_start + width)?);
            source_row += 1;
        }
    }
    (source_row == height).then_some(output)
}

struct Input<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> Input<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    fn read_u8(&mut self) -> Option<u8> {
        let value = *self.data.get(self.position)?;
        self.position = self.position.checked_add(1)?;
        Some(value)
    }

    fn read_u16(&mut self) -> Option<u16> {
        let bytes: [u8; 2] = self.read_bytes(2)?.try_into().ok()?;
        Some(u16::from_le_bytes(bytes))
    }

    fn read_bytes(&mut self, len: usize) -> Option<&'a [u8]> {
        let end = self.position.checked_add(len)?;
        let bytes = self.data.get(self.position..end)?;
        self.position = end;
        Some(bytes)
    }

    fn skip(&mut self, len: usize) -> Option<()> {
        self.read_bytes(len).map(|_| ())
    }

    fn read_sub_blocks(&mut self) -> Option<Vec<u8>> {
        let mut output = Vec::new();
        loop {
            let len = usize::from(self.read_u8()?);
            if len == 0 {
                return Some(output);
            }
            output.extend_from_slice(self.read_bytes(len)?);
        }
    }

    fn skip_sub_blocks(&mut self) -> Option<()> {
        loop {
            let len = usize::from(self.read_u8()?);
            if len == 0 {
                return Some(());
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

    fn read(&mut self, width: u8) -> Option<u16> {
        let end = self.bit_position.checked_add(usize::from(width))?;
        if end > self.data.len().checked_mul(8)? {
            return None;
        }

        let mut value = 0u16;
        for shift in 0..width {
            let byte = *self.data.get(self.bit_position / 8)?;
            let bit = (byte >> (self.bit_position % 8)) & 1;
            value |= u16::from(bit) << shift;
            self.bit_position += 1;
        }
        Some(value)
    }
}
