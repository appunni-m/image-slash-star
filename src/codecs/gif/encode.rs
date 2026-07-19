//! GIF89a encoder.
//!
//! Supports:
//! - `L8`: raw palette indices with a grayscale palette
//! - `Rgb8`: quantized to a 256-color palette
//! - `Rgba8`: quantized to a 256-color palette plus transparency
use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage, DecodedSequence, FrameDisposal, ImageMode};
use std::collections::HashMap;

const GIF_TRAILER: u8 = 0x3b;
const IMAGE_SEPARATOR: u8 = 0x2c;
const EXTENSION_INTRODUCER: u8 = 0x21;
const GRAPHIC_CONTROL_LABEL: u8 = 0xf9;
const MAX_LZW_CODE: u16 = 4095;
/// Encode a `DecodedImage` as GIF bytes.
///
/// For L8 images the pixel values are used directly as palette indices with a
/// grayscale palette. RGB8 and RGBA8 images are quantized to a palette of at
/// most 256 unique colors using a simple nearest-neighbor approach.
///
/// Returns `None` for unsupported color types or images with no pixels.
pub fn encode(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    encode_sequence(&DecodedSequence::from_image(img.clone()), opts)
}

/// Encode a still image or animation without discarding source frames.
pub fn encode_sequence(sequence: &DecodedSequence, opts: &EncodeOptions) -> Option<Vec<u8>> {
    sequence.validate().ok()?;
    let animated = option_bool(opts, "animated").unwrap_or(sequence.frames.len() > 1);
    let requested_frames = if animated {
        option_u16(opts, "frames")
            .map(usize::from)
            .unwrap_or(sequence.frames.len())
    } else {
        1
    };
    if requested_frames == 0 || requested_frames > sequence.frames.len() {
        return None;
    }

    let disposal_override = opts.extra.get("disposal").map(String::as_str);
    let disposal_override = match disposal_override {
        Some(value) => Some(parse_disposal(value)?),
        None => None,
    };
    let loop_count = match parse_loop_count(opts)? {
        Some(value) => Some(value),
        None => sequence
            .loop_count
            .and_then(|value| u16::try_from(value).ok()),
    };
    let settings = GifSettings {
        interlaced: opts.interlace,
        local_color_table: opts
            .extra
            .get("color_table")
            .is_some_and(|value| value == "local"),
        disposal_override,
        loop_count,
    };
    write_gif(sequence, requested_frames, opts, settings)
}

fn prepare_image(img: &DecodedImage) -> Option<PreparedImage> {
    let (palette, indices, transparent) = match (img.mode, img.color) {
        (ImageMode::P8, ColorType::L8) => {
            let palette = img.palette.as_ref()?;
            let transparent = palette.alpha.iter().position(|&alpha| alpha == 0);
            (
                palette.rgb.clone(),
                img.pixels.clone(),
                transparent.and_then(|index| u8::try_from(index).ok()),
            )
        }
        (ImageMode::L8, ColorType::L8) => {
            let pixel_count = usize::try_from(img.width)
                .ok()?
                .checked_mul(usize::try_from(img.height).ok()?)?;
            if img.pixels.len() != pixel_count {
                return None;
            }
            let mut palette = Vec::with_capacity(256 * 3);
            for i in 0u16..256 {
                let v = i as u8;
                palette.extend_from_slice(&[v, v, v]);
            }
            (palette, img.pixels.clone(), None)
        }
        (ImageMode::Rgb8, ColorType::Rgb8) => {
            let (palette, indices) = quantize_rgb(&img.pixels)?;
            (palette, indices, None)
        }
        (ImageMode::Rgba8, ColorType::Rgba8) => {
            let (palette, indices, transparent_idx) = quantize_rgba(&img.pixels);
            (palette, indices, transparent_idx)
        }
        _ => return None,
    };
    let pixel_count = usize::try_from(img.width)
        .ok()?
        .checked_mul(usize::try_from(img.height).ok()?)?;
    if indices.len() != pixel_count {
        return None;
    }
    Some(PreparedImage {
        palette,
        indices,
        transparent,
    })
}

#[derive(Clone, Copy)]
struct GifSettings {
    interlaced: Option<bool>,
    local_color_table: bool,
    disposal_override: Option<u8>,
    loop_count: Option<u16>,
}

struct PreparedImage {
    palette: Vec<u8>,
    indices: Vec<u8>,
    transparent: Option<u8>,
}

fn option_bool(opts: &EncodeOptions, key: &str) -> Option<bool> {
    match opts.extra.get(key)?.as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

fn option_u16(opts: &EncodeOptions, key: &str) -> Option<u16> {
    opts.extra.get(key)?.parse().ok()
}

fn parse_disposal(value: &str) -> Option<u8> {
    match value {
        "none" | "0" => Some(0),
        "keep" | "1" => Some(1),
        "background" | "2" => Some(2),
        "previous" | "3" => Some(3),
        _ => None,
    }
}

fn parse_loop_count(opts: &EncodeOptions) -> Option<Option<u16>> {
    let Some(value) = opts.extra.get("loop") else {
        return Some(None);
    };
    match value.as_str() {
        "true" | "infinite" => Some(Some(0)),
        "false" => Some(None),
        number => number.parse().ok().map(Some),
    }
}

fn frame_disposal(disposal: FrameDisposal) -> u8 {
    match disposal {
        FrameDisposal::Unspecified => 0,
        FrameDisposal::Keep => 1,
        FrameDisposal::Background => 2,
        FrameDisposal::Previous => 3,
    }
}

fn table_parameters(palette: &[u8]) -> Option<(usize, u8, u8)> {
    if palette.is_empty() || !palette.len().is_multiple_of(3) || palette.len() > 256 * 3 {
        return None;
    }
    let color_count = (palette.len() / 3).max(2).next_power_of_two();
    let table_bits = usize::BITS - color_count.leading_zeros() - 1;
    let size_field = u8::try_from(table_bits.checked_sub(1)?).ok()?;
    let minimum_code_size = u8::try_from(table_bits.max(2)).ok()?;
    Some((color_count, size_field, minimum_code_size))
}

fn write_gif(
    sequence: &DecodedSequence,
    frame_count: usize,
    opts: &EncodeOptions,
    settings: GifSettings,
) -> Option<Vec<u8>> {
    let width = u16::try_from(sequence.width).ok()?;
    let height = u16::try_from(sequence.height).ok()?;
    let first = prepare_image(&sequence.frames.first()?.image)?;
    let (global_count, global_size, _) = table_parameters(&first.palette)?;
    let global_table = !settings.local_color_table;

    let needs_89a = frame_count > 1
        || settings.loop_count.is_some()
        || sequence.frames.iter().take(frame_count).any(|frame| {
            prepare_image(&frame.image).is_some_and(|image| image.transparent.is_some())
        });
    let mut output = Vec::new();
    output.extend_from_slice(if needs_89a { b"GIF89a" } else { b"GIF87a" });
    output.extend_from_slice(&width.to_le_bytes());
    output.extend_from_slice(&height.to_le_bytes());
    output.push(u8::from(global_table) << 7 | global_size);
    output.extend_from_slice(&[0, 0]); // Background index and pixel aspect ratio.
    if global_table {
        write_color_table(&mut output, &first.palette, global_count)?;
    }

    if frame_count > 1
        && let Some(loop_count) = settings.loop_count
    {
        output.extend_from_slice(&[
            EXTENSION_INTRODUCER,
            0xff,
            0x0b,
            b'N',
            b'E',
            b'T',
            b'S',
            b'C',
            b'A',
            b'P',
            b'E',
            b'2',
            b'.',
            b'0',
            0x03,
            0x01,
        ]);
        output.extend_from_slice(&loop_count.to_le_bytes());
        output.push(0);
    }

    for frame in sequence.frames.iter().take(frame_count) {
        let prepared = prepare_image(&frame.image)?;
        let (color_count, size_field, minimum_code_size) = table_parameters(&prepared.palette)?;
        let mut transparent = prepared.transparent;
        if let Some(requested) = option_bool(opts, "transparency") {
            transparent = requested.then_some(transparent.unwrap_or(0));
        }
        let disposal = settings
            .disposal_override
            .unwrap_or_else(|| frame_disposal(frame.disposal));
        let delay_cs = u16::try_from(frame.duration_ms / 10).ok()?;
        if transparent.is_some() || disposal != 0 || delay_cs != 0 {
            output.extend_from_slice(&[
                EXTENSION_INTRODUCER,
                GRAPHIC_CONTROL_LABEL,
                0x04,
                disposal << 2 | u8::from(transparent.is_some()),
            ]);
            output.extend_from_slice(&delay_cs.to_le_bytes());
            output.extend_from_slice(&[transparent.unwrap_or(0), 0]);
        }

        output.push(IMAGE_SEPARATOR);
        output.extend_from_slice(&u16::try_from(frame.left).ok()?.to_le_bytes());
        output.extend_from_slice(&u16::try_from(frame.top).ok()?.to_le_bytes());
        let frame_width = u16::try_from(frame.image.width).ok()?;
        let frame_height = u16::try_from(frame.image.height).ok()?;
        output.extend_from_slice(&frame_width.to_le_bytes());
        output.extend_from_slice(&frame_height.to_le_bytes());
        let local_table = !global_table || prepared.palette != first.palette;
        let interlaced = settings.interlaced.unwrap_or(frame.interlaced);
        output.push(u8::from(local_table) << 7 | u8::from(interlaced) << 6 | size_field);
        if local_table {
            write_color_table(&mut output, &prepared.palette, color_count)?;
        }
        let encoded_indices = if interlaced {
            interlace(
                &prepared.indices,
                usize::from(frame_width),
                usize::from(frame_height),
            )?
        } else {
            prepared.indices
        };
        let compressed = encode_lzw(&encoded_indices, minimum_code_size)?;
        output.push(minimum_code_size);
        write_sub_blocks(&mut output, &compressed);
    }
    output.push(GIF_TRAILER);
    Some(output)
}

fn write_color_table(output: &mut Vec<u8>, palette: &[u8], color_count: usize) -> Option<()> {
    output.extend_from_slice(palette);
    output.resize(
        output
            .len()
            .checked_add((color_count * 3).checked_sub(palette.len())?)?,
        0,
    );
    Some(())
}

fn interlace(indices: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
    if indices.len() != width.checked_mul(height)? {
        return None;
    }
    let mut output = Vec::with_capacity(indices.len());
    for (start, step) in [(0, 8), (4, 8), (2, 4), (1, 2)] {
        for y in (start..height).step_by(step) {
            let row_start = y.checked_mul(width)?;
            output.extend_from_slice(indices.get(row_start..row_start.checked_add(width)?)?);
        }
    }
    Some(output)
}

/// Encode indices using the GIF89a Appendix F LZW code-width rules.
fn encode_lzw(indices: &[u8], minimum_code_size: u8) -> Option<Vec<u8>> {
    if indices.is_empty() || !(2..=8).contains(&minimum_code_size) {
        return None;
    }

    let clear_code = 1u16.checked_shl(u32::from(minimum_code_size))?;
    let end_code = clear_code.checked_add(1)?;
    if indices.iter().any(|&index| u16::from(index) >= clear_code) {
        return None;
    }

    let mut writer = BitWriter::new();
    let mut dictionary = HashMap::<(u16, u8), u16>::new();
    let mut code_size = minimum_code_size.checked_add(1)?;
    let mut next_code = end_code.checked_add(1)?;
    writer.write(clear_code, code_size);

    let mut prefix = u16::from(indices[0]);
    for &suffix in &indices[1..] {
        if let Some(&code) = dictionary.get(&(prefix, suffix)) {
            prefix = code;
            continue;
        }

        writer.write(prefix, code_size);
        if next_code <= MAX_LZW_CODE {
            dictionary.insert((prefix, suffix), next_code);
            next_code = next_code.checked_add(1)?;
            // The encoder's dictionary is one entry ahead of the decoder. Delay
            // the width transition by one code so both sides switch together.
            if code_size < 12 && next_code > (1u16 << code_size) {
                code_size += 1;
            }
        } else {
            writer.write(clear_code, code_size);
            dictionary.clear();
            code_size = minimum_code_size.checked_add(1)?;
            next_code = end_code.checked_add(1)?;
        }
        prefix = u16::from(suffix);
    }

    writer.write(prefix, code_size);
    writer.write(end_code, code_size);
    Some(writer.finish())
}

fn write_sub_blocks(output: &mut Vec<u8>, data: &[u8]) {
    for block in data.chunks(255) {
        output.push(block.len() as u8);
        output.extend_from_slice(block);
    }
    output.push(0);
}

struct BitWriter {
    bytes: Vec<u8>,
    current: u8,
    used: u8,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            current: 0,
            used: 0,
        }
    }

    fn write(&mut self, code: u16, width: u8) {
        for shift in 0..width {
            let bit = ((code >> shift) & 1) as u8;
            self.current |= bit << self.used;
            self.used += 1;
            if self.used == 8 {
                self.bytes.push(self.current);
                self.current = 0;
                self.used = 0;
            }
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.used != 0 {
            self.bytes.push(self.current);
        }
        self.bytes
    }
}
/// Quantize RGB8 pixels to a palette (max 256 colors).
///
/// Returns `(palette, indices)` where palette is a flat vec of RGB triplets
/// and indices are the per-pixel palette index values.
fn quantize_rgb(pixels: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    if !pixels.len().is_multiple_of(3) {
        return None;
    }
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut indices = Vec::with_capacity(pixels.len() / 3);
    for chunk in pixels.chunks_exact(3) {
        let color = [chunk[0], chunk[1], chunk[2]];
        match find_color(&palette, &color) {
            Some(idx) => indices.push(idx as u8),
            None => {
                if palette.len() < 256 {
                    let idx = palette.len() as u8;
                    palette.push(color);
                    indices.push(idx);
                } else {
                    // Palette full: find nearest neighbor
                    let nearest = find_nearest(&palette, &color);
                    indices.push(nearest as u8);
                }
            }
        }
    }
    // Flatten palette to RGB triplets
    let mut flat = Vec::with_capacity(palette.len() * 3);
    for c in &palette {
        flat.push(c[0]);
        flat.push(c[1]);
        flat.push(c[2]);
    }
    Some((flat, indices))
}
/// Quantize RGBA8 pixels to a palette with optional transparency.
///
/// Returns `(palette, indices, optional_transparent_index)`.
fn quantize_rgba(pixels: &[u8]) -> (Vec<u8>, Vec<u8>, Option<u8>) {
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let has_transparency = pixels.chunks_exact(4).any(|pixel| pixel[3] < 128);
    let transparent_idx = has_transparency.then_some(0);
    let pixel_count = pixels.len() / 4;
    let mut indices = Vec::with_capacity(pixel_count);
    let mut transparent_color = None;
    for chunk in pixels.chunks_exact(4) {
        let alpha = chunk[3];
        let rgb = [chunk[0], chunk[1], chunk[2]];
        if alpha < 128 {
            transparent_color.get_or_insert(rgb);
            indices.push(0);
        } else {
            let palette_offset = usize::from(has_transparency);
            match find_color(&palette, &rgb) {
                Some(idx) => indices.push((idx + palette_offset) as u8),
                None => {
                    if palette.len() < 256 - palette_offset {
                        let idx = palette.len() + palette_offset;
                        palette.push(rgb);
                        indices.push(idx as u8);
                    } else {
                        let nearest = find_nearest(&palette, &rgb) + palette_offset;
                        indices.push(nearest as u8);
                    }
                }
            }
        }
    }
    // Build flat palette. If we have transparent pixels, index 0 is the
    // transparent entry (use the first transparent color found).
    let mut flat = Vec::with_capacity(palette.len() * 3);
    if has_transparency {
        flat.extend_from_slice(&transparent_color.unwrap_or([0, 0, 0]));
    }
    for c in &palette {
        flat.push(c[0]);
        flat.push(c[1]);
        flat.push(c[2]);
    }
    (flat, indices, transparent_idx)
}
/// Find a color in the palette. Returns its index if found.
fn find_color(palette: &[[u8; 3]], color: &[u8; 3]) -> Option<usize> {
    palette.iter().position(|c| c == color)
}
/// Find the nearest color in the palette by Euclidean distance.
fn find_nearest(palette: &[[u8; 3]], color: &[u8; 3]) -> usize {
    let mut best = 0;
    let mut best_dist = u32::MAX;
    for (i, entry) in palette.iter().enumerate() {
        let dr = entry[0] as i32 - color[0] as i32;
        let dg = entry[1] as i32 - color[1] as i32;
        let db = entry[2] as i32 - color[2] as i32;
        let dist = (dr * dr + dg * dg + db * db) as u32;
        if dist < best_dist {
            best_dist = dist;
            best = i;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::encode;
    use crate::decode::gif::decode;
    use crate::encode_options::EncodeOptions;
    use crate::types::{ColorType, DecodedImage};

    #[test]
    fn native_lzw_roundtrip_crosses_dictionary_width_boundaries() {
        let width = 128;
        let height = 64;
        let pixels = (0..width * height)
            .map(|index| ((index * 37 + index / 11) & 0xff) as u8)
            .collect::<Vec<_>>();
        let image = DecodedImage::new(width, height, pixels.clone(), ColorType::L8);

        let encoded = encode(&image, &EncodeOptions::default()).expect("GIF should encode");
        let decoded = decode(&encoded).expect("native encoder output should decode");

        assert_eq!(decoded.width, width);
        assert_eq!(decoded.height, height);
        assert_eq!(decoded.pixels, pixels);
    }

    #[test]
    fn native_lzw_roundtrip_handles_single_pixel() {
        let image = DecodedImage::new(1, 1, vec![173], ColorType::L8);

        let encoded = encode(&image, &EncodeOptions::default()).expect("GIF should encode");
        let decoded = decode(&encoded).expect("native encoder output should decode");

        assert_eq!(decoded.pixels, vec![173]);
    }
}
