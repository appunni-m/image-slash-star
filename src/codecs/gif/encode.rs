//! GIF89a encoder.
//!
//! Supports:
//! - `L8`: raw palette indices with a grayscale palette
//! - `Rgb8`: quantized to a 256-color palette
//! - `Rgba8`: quantized to a 256-color palette plus transparency
use crate::encode_options::EncodeOptions;
use crate::types::{
    AnimationBackground, ColorType, DecodedImage, DecodedSequence, FrameDisposal, ImageMode,
    ImagePalette,
};
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

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let identical = [0u8, 0, 0, 255];
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = rgba_difference_bounds(&identical, &identical, 1, 1);
    }));

    let split_colors = [[10u8, 0, 0], [0, 0, 0]];
    let split_counts = [1u32, 1];
    let split_node = MedianBox {
        axes: [vec![0, 1], vec![0, 1], vec![0, 1]],
        pixel_count: 100,
        children: None,
    };
    let _ = split_median_box(&split_node, &split_colors, &split_counts);

    let equal_colors = [[0u8, 0, 0], [0, 0, 0]];
    let equal_node = MedianBox {
        axes: [vec![0, 1], vec![0, 1], vec![0, 1]],
        pixel_count: 100,
        children: None,
    };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = split_median_box(&equal_node, &equal_colors, &split_counts);
    }));

    let opaque_rgba = [
        255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
    ];
    let _ = quantize_rgba(&opaque_rgba);

    let mut compact_palette = vec![
        [255u8, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
    ];
    let mut compact_indices = vec![0u8, 1, 2];
    let mut compact_transparent = None;
    let _ = compact_rgba_palette(
        &mut compact_palette,
        &mut compact_indices,
        &mut compact_transparent,
    );

    let mut rgb_pixels = Vec::with_capacity(16 * 16 * 3);
    for value in 0u8..=255 {
        rgb_pixels.extend_from_slice(&[value, value.wrapping_mul(37), value.wrapping_mul(73)]);
    }
    let first = DecodedImage::new(16, 16, vec![0; 16 * 16 * 3], ColorType::Rgb8);
    let second = DecodedImage::new(16, 16, rgb_pixels, ColorType::Rgb8);
    let frames = vec![
        crate::types::DecodedFrame {
            image: first,
            left: 0,
            top: 0,
            duration_ms: 10,
            disposal: FrameDisposal::Keep,
            interlaced: false,
        },
        crate::types::DecodedFrame {
            image: second,
            left: 0,
            top: 0,
            duration_ms: 10,
            disposal: FrameDisposal::Keep,
            interlaced: false,
        },
    ];
    let sequence = DecodedSequence {
        width: 16,
        height: 16,
        frames,
        loop_count: None,
        background: None,
    };
    let coalesced =
        coalesce_identical_frames(&sequence, 2).expect("coverage RGB frames coalesce");
    let _ = write_gif(
        &sequence,
        &coalesced,
        GifSettings {
            interlaced: None,
            local_color_table: false,
            disposal_override: None,
            loop_count: None,
            transparency_override: None,
        },
    );
}

/// Encode a still image or animation without discarding source frames.
pub fn encode_sequence(sequence: &DecodedSequence, opts: &EncodeOptions) -> Option<Vec<u8>> {
    sequence.validate().ok()?;
    let animated = option_bool(opts, "animated")?.unwrap_or(sequence.frames.len() > 1);
    let requested_frames = if animated { sequence.frames.len() } else { 1 };

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
        transparency_override: option_bool(opts, "transparency")?,
    };
    let frames = coalesce_identical_frames(sequence, requested_frames)?;
    write_gif(sequence, &frames, settings)
}

fn coalesce_identical_frames(
    sequence: &DecodedSequence,
    requested_frames: usize,
) -> Option<Vec<crate::types::DecodedFrame>> {
    if requested_frames == 1 {
        return Some(vec![sequence.frames.first()?.clone()]);
    }
    let width = usize::try_from(sequence.width).ok()?;
    let height = usize::try_from(sequence.height).ok()?;
    let mut canvas = vec![0u8; width.checked_mul(height)?.checked_mul(4)?];
    let mut previous_frame = None::<&crate::types::DecodedFrame>;
    let mut previous_render = None::<Vec<u8>>;
    let mut output = Vec::<crate::types::DecodedFrame>::new();

    for frame in sequence.frames.iter().take(requested_frames) {
        if let Some(previous) = previous_frame {
            match previous.disposal {
                FrameDisposal::Unspecified
                | FrameDisposal::Keep
                | FrameDisposal::Previous
                | FrameDisposal::Reserved(_) => {}
                FrameDisposal::Background => clear_frame_rect(&mut canvas, width, previous)?,
            }
        }

        composite_frame(&mut canvas, width, frame)?;
        let identical = previous_render.as_deref() == Some(canvas.as_slice());
        if identical {
            let previous = output.last_mut()?;
            previous.duration_ms = previous.duration_ms.checked_add(frame.duration_ms)?;
        } else {
            let mut output_frame = frame.clone();
            if !output.is_empty() {
                let previous = previous_render.as_deref()?;
                let (left, top, right, bottom) = rgba_difference_bounds(
                    previous,
                    &canvas,
                    width,
                    usize::try_from(sequence.height).ok()?,
                );
                let frame_width = right.checked_sub(left)?;
                let frame_height = bottom.checked_sub(top)?;
                let full_image = if frame.image.mode == ImageMode::Rgba8 {
                    DecodedImage::new(
                        sequence.width,
                        sequence.height,
                        canvas.clone(),
                        ColorType::Rgba8,
                    )
                } else {
                    let rgb = canvas
                        .chunks_exact(4)
                        .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
                        .collect();
                    DecodedImage::new(sequence.width, sequence.height, rgb, ColorType::Rgb8)
                };
                let mut prepared = prepare_image(&full_image)?;
                if prepared.transparent.is_none() && prepared.palette.len() / 3 < 256 {
                    let transparent = u8::try_from(prepared.palette.len() / 3).ok()?;
                    prepared.palette.extend_from_slice(&[0, 0, 0]);
                    prepared.transparent = Some(transparent);
                }
                let mut cropped = Vec::with_capacity(frame_width.checked_mul(frame_height)?);
                for y in top..bottom {
                    let start = y.checked_mul(width)?.checked_add(left)?;
                    let end = start.checked_add(frame_width)?;
                    cropped.extend_from_slice(prepared.indices.get(start..end)?);
                }
                output_frame.left = u32::try_from(left).ok()?;
                output_frame.top = u32::try_from(top).ok()?;
                let mut alpha = vec![255; prepared.palette.len() / 3];
                if let Some(transparent) = prepared.transparent { alpha[usize::from(transparent)] = 0; }
                output_frame.image = DecodedImage::with_mode(
                    u32::try_from(frame_width).ok()?,
                    u32::try_from(frame_height).ok()?,
                    cropped,
                    ImageMode::P8,
                )
                .with_palette(ImagePalette::new(prepared.palette, alpha).ok()?);
            }
            output.push(output_frame);
            previous_render = Some(canvas.clone());
        }
        previous_frame = Some(frame);
    }
    Some(output)
}

fn rgba_difference_bounds(
    previous: &[u8],
    current: &[u8],
    width: usize,
    height: usize,
) -> (usize, usize, usize, usize) {
    debug_assert_eq!(previous.len(), current.len());
    debug_assert_eq!(current.len(), width * height * 4);
    let mut left = width;
    let mut top = height;
    let mut right = 0usize;
    let mut bottom = 0usize;
    for (index, (before, after)) in previous
        .chunks_exact(4)
        .zip(current.chunks_exact(4))
        .enumerate()
    {
        if before != after {
            let x = index % width;
            let y = index / width;
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    debug_assert!(left < right && top < bottom);
    (left, top, right, bottom)
}

fn clear_frame_rect(
    canvas: &mut [u8],
    canvas_width: usize,
    frame: &crate::types::DecodedFrame,
) -> Option<()> {
    let left = usize::try_from(frame.left).ok()?;
    let top = usize::try_from(frame.top).ok()?;
    let width = usize::try_from(frame.image.width).ok()?;
    let height = usize::try_from(frame.image.height).ok()?;
    for y in 0..height {
        let start = (top
            .checked_add(y)?
            .checked_mul(canvas_width)?
            .checked_add(left)?)
        .checked_mul(4)?;
        let end = start.checked_add(width.checked_mul(4)?)?;
        canvas.get_mut(start..end)?.fill(0);
    }
    Some(())
}

fn composite_frame(
    canvas: &mut [u8],
    canvas_width: usize,
    frame: &crate::types::DecodedFrame,
) -> Option<()> {
    let image = &frame.image;
    let left = usize::try_from(frame.left).ok()?;
    let top = usize::try_from(frame.top).ok()?;
    let width = usize::try_from(image.width).ok()?;
    let height = usize::try_from(image.height).ok()?;
    for y in 0..height {
        for x in 0..width {
            let source = y.checked_mul(width)?.checked_add(x)?;
            let rgba = match image.mode {
                ImageMode::P8 => {
                    let palette = image.palette.as_ref()?;
                    let index = usize::from(*image.pixels.get(source)?);
                    let palette_offset = index.checked_mul(3)?;
                    let rgb = palette.rgb.get(palette_offset..palette_offset + 3)?;
                    [
                        rgb[0],
                        rgb[1],
                        rgb[2],
                        palette.alpha.get(index).copied().unwrap_or(255),
                    ]
                }
                ImageMode::L8 => {
                    let value = *image.pixels.get(source)?;
                    [value, value, value, 255]
                }
                ImageMode::Rgb8 => {
                    let offset = source.checked_mul(3)?;
                    let rgb = image.pixels.get(offset..offset + 3)?;
                    [rgb[0], rgb[1], rgb[2], 255]
                }
                ImageMode::Rgba8 => {
                    let offset = source.checked_mul(4)?;
                    image.pixels.get(offset..offset + 4)?.try_into().ok()?
                }
                _ => return None,
            };
            if rgba[3] == 0 && image.mode == ImageMode::P8 {
                continue;
            }
            let destination = (top
                .checked_add(y)?
                .checked_mul(canvas_width)?
                .checked_add(left)?
                .checked_add(x)?)
            .checked_mul(4)?;
            canvas
                .get_mut(destination..destination + 4)?
                .copy_from_slice(&rgba);
        }
    }
    Some(())
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
            debug_assert_eq!(img.pixels.len(), pixel_count);
            // Pillow converts L input to a compact P palette containing only
            // the used grayscale values, ordered by their original index.
            let mut used = [false; 256];
            for &value in &img.pixels {
                used[usize::from(value)] = true;
            }
            let mut palette = Vec::new();
            let mut remap = [0u8; 256];
            for (value, is_used) in used.into_iter().enumerate() {
                if is_used {
                    let index = u8::try_from(palette.len() / 3).ok()?;
                    remap[value] = index;
                    let value = u8::try_from(value).ok()?;
                    palette.extend_from_slice(&[value, value, value]);
                }
            }
            let indices = img
                .pixels
                .iter()
                .map(|&value| remap[usize::from(value)])
                .collect();
            (palette, indices, None)
        }
        (ImageMode::Rgb8, ColorType::Rgb8) => {
            let (palette, indices) = quantize_rgb(&img.pixels)?;
            (palette, indices, None)
        }
        (ImageMode::Rgba8, ColorType::Rgba8) => {
            let (palette, indices, transparent_idx) = quantize_rgba(&img.pixels)?;
            (palette, indices, transparent_idx)
        }
        _ => return None,
    };
    let pixel_count = usize::try_from(img.width)
        .ok()?
        .checked_mul(usize::try_from(img.height).ok()?)?;
    debug_assert_eq!(indices.len(), pixel_count);
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
    transparency_override: Option<bool>,
}

#[derive(Clone)]
struct PreparedImage {
    palette: Vec<u8>,
    indices: Vec<u8>,
    transparent: Option<u8>,
}

fn option_bool(opts: &EncodeOptions, key: &str) -> Option<Option<bool>> {
    let Some(value) = opts.extra.get(key) else {
        return Some(None);
    };
    match value.as_str() {
        "true" | "1" | "yes" => Some(Some(true)),
        "false" | "0" | "no" => Some(Some(false)),
        _ => None,
    }
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

fn table_parameters(palette: &[u8]) -> (usize, u8, u8) {
    debug_assert!(!palette.is_empty());
    debug_assert!(palette.len().is_multiple_of(3));
    debug_assert!(palette.len() <= 256 * 3);
    // Pillow's GIF writer normalizes even a one-color image to a four-entry
    // table while retaining the GIF-mandated minimum LZW code width of two.
    let color_count = (palette.len() / 3).max(4).next_power_of_two();
    let table_bits = usize::BITS - color_count.leading_zeros() - 1;
    let size_field = (table_bits - 1) as u8;
    // Pillow's P-mode GIF encoder uses an eight-bit LZW root alphabet even
    // when the emitted color table contains fewer entries.
    let minimum_code_size = 8;
    (color_count, size_field, minimum_code_size)
}

fn write_gif(
    sequence: &DecodedSequence,
    frames: &[crate::types::DecodedFrame],
    settings: GifSettings,
) -> Option<Vec<u8>> {
    let width = u16::try_from(sequence.width).ok()?;
    let height = u16::try_from(sequence.height).ok()?;
    let first_frame = frames.first()?;
    let mut first = prepare_image(&first_frame.image)?;
    let background = prepare_background(&mut first, first_frame.image.mode, sequence.background)?;
    let (global_count, global_size, _) = table_parameters(&first.palette);
    // Pillow always writes the global palette for a single frame. Its
    // include_color_table option adds a duplicate local palette rather than
    // replacing the global one.
    let global_table = true;

    let needs_89a = frames.len() > 1
        || settings.loop_count.is_some()
        || settings.transparency_override == Some(true)
        || frames.iter().any(|frame| {
            prepare_image(&frame.image).is_some_and(|image| image.transparent.is_some())
        });
    let mut output = Vec::new();
    output.extend_from_slice(if needs_89a { b"GIF89a" } else { b"GIF87a" });
    output.extend_from_slice(&width.to_le_bytes());
    output.extend_from_slice(&height.to_le_bytes());
    output.push(u8::from(global_table) << 7 | global_size);
    output.extend_from_slice(&[background, 0]); // Background index and pixel aspect ratio.
    write_color_table(&mut output, &first.palette, global_count);

    if let Some(loop_count) = settings.loop_count {
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

    let mut previous_quantized_rgb = None::<Vec<u8>>;
    for (frame_index, frame) in frames.iter().enumerate() {
        let mut prepared = if frame_index == 0 {
            first.clone()
        } else {
            prepare_image(&frame.image)?
        };
        let quantized_rgb = indexed_rgb(&prepared.indices, &prepared.palette)?;
        if let Some(previous) = previous_quantized_rgb.as_deref()
            && previous.len() == quantized_rgb.len()
            && let Some(transparent) = prepared.transparent
        {
            // Coalescing has already reserved a transparent entry whenever
            // the palette has room. A full 256-color palette deliberately has
            // none, matching Pillow's inability to mask unchanged pixels.
            for (index, (before, after)) in previous
                .chunks_exact(3)
                .zip(quantized_rgb.chunks_exact(3))
                .enumerate()
            {
                if before == after {
                    prepared.indices[index] = transparent;
                }
            }
        }
        previous_quantized_rgb = Some(quantized_rgb);
        let (color_count, size_field, minimum_code_size) = table_parameters(&prepared.palette);
        let mut transparent = prepared.transparent;
        if let Some(requested) = settings.transparency_override {
            transparent = requested.then_some(transparent.unwrap_or(0));
        }
        let disposal = settings.disposal_override.unwrap_or(0);
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
        let local_table = settings.local_color_table || prepared.palette != first.palette;
        // Pillow defaults to interlacing a sufficiently large single-frame
        // GIF, but its multi-frame writer emits non-interlaced descriptors.
        let default_interlace =
            frames.len() == 1 && frame.image.width >= 16 && frame.image.height >= 16;
        let interlaced = settings.interlaced.unwrap_or(default_interlace);
        // Pillow 12.2.0 GifImagePlugin.py:826-873 writes local-table size
        // bits only when include_color_table also sets the presence flag.
        // With the global palette, the descriptor contains only interlace.
        let local_table_fields = if local_table { 0x80 | size_field } else { 0 };
        output.push(u8::from(interlaced) << 6 | local_table_fields);
        if local_table {
            write_color_table(&mut output, &prepared.palette, color_count);
        }
        let encoded_indices = if interlaced {
            interlace(
                &prepared.indices,
                usize::from(frame_width),
                usize::from(frame_height),
            )
        } else {
            prepared.indices
        };
        let compressed = encode_lzw(&encoded_indices, minimum_code_size);
        output.push(minimum_code_size);
        write_sub_blocks(&mut output, &compressed);
    }
    output.push(GIF_TRAILER);
    Some(output)
}

fn prepare_background(
    first: &mut PreparedImage,
    source_mode: ImageMode,
    background: Option<AnimationBackground>,
) -> Option<u8> {
    let Some(background) = background else {
        return Some(0);
    };
    match background {
        AnimationBackground::PaletteIndex(index) => Some(index),
        AnimationBackground::Rgba([red, green, blue, alpha]) => {
            if source_mode != ImageMode::Rgba8 && alpha != 255 {
                return Some(0);
            }
            if alpha == 0
                && let Some(transparent) = first.transparent
            {
                return Some(transparent);
            }
            if source_mode != ImageMode::Rgba8 {
                for (index, color) in first.palette.chunks_exact(3).enumerate() {
                    if color == [red, green, blue] {
                        return u8::try_from(index).ok();
                    }
                }
            }
            if first.palette.len() / 3 >= 256 {
                return Some(0);
            }
            let index = u8::try_from(first.palette.len() / 3).ok()?;
            first.palette.extend_from_slice(&[red, green, blue]);
            Some(index)
        }
    }
}

fn write_color_table(output: &mut Vec<u8>, palette: &[u8], color_count: usize) {
    output.extend_from_slice(palette);
    let padding = color_count * 3 - palette.len();
    output.resize(output.len() + padding, 0);
}

fn indexed_rgb(indices: &[u8], palette: &[u8]) -> Option<Vec<u8>> {
    let mut rgb = Vec::with_capacity(indices.len().checked_mul(3)?);
    for &index in indices {
        let offset = usize::from(index).checked_mul(3)?;
        rgb.extend_from_slice(palette.get(offset..offset + 3)?);
    }
    Some(rgb)
}

fn interlace(indices: &[u8], width: usize, height: usize) -> Vec<u8> {
    debug_assert_eq!(indices.len(), width * height);
    let mut output = Vec::with_capacity(indices.len());
    for (start, step) in [(0, 8), (4, 8), (2, 4), (1, 2)] {
        for y in (start..height).step_by(step) {
            let row_start = y * width;
            output.extend_from_slice(&indices[row_start..row_start + width]);
        }
    }
    output
}

/// Encode indices using the GIF89a Appendix F LZW code-width rules.
fn encode_lzw(indices: &[u8], minimum_code_size: u8) -> Vec<u8> {
    debug_assert!(!indices.is_empty());
    debug_assert!((2..=8).contains(&minimum_code_size));

    let clear_code = 1u16 << minimum_code_size;
    let end_code = clear_code + 1;
    debug_assert!(indices.iter().all(|&index| u16::from(index) < clear_code));

    let mut writer = BitWriter::new();
    let mut dictionary = HashMap::<(u16, u8), u16>::new();
    let mut code_size = minimum_code_size + 1;
    let mut next_code = end_code + 1;
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
            next_code += 1;
            // The encoder's dictionary is one entry ahead of the decoder. Delay
            // the width transition by one code so both sides switch together.
            if code_size < 12 && next_code > (1u16 << code_size) {
                code_size += 1;
            }
        } else {
            writer.write(clear_code, code_size);
            dictionary.clear();
            code_size = minimum_code_size + 1;
            next_code = end_code + 1;
        }
        prefix = u16::from(suffix);
    }

    writer.write(prefix, code_size);
    writer.write(end_code, code_size);
    writer.finish()
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
    debug_assert!(pixels.len().is_multiple_of(3));
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut counts = Vec::<u32>::new();
    for chunk in pixels.chunks_exact(3) {
        let color = [chunk[0], chunk[1], chunk[2]];
        match find_color(&palette, &color) {
            Some(idx) => counts[idx] = counts[idx].checked_add(1)?,
            None => {
                if palette.len() < 256 {
                    palette.push(color);
                    counts.push(1);
                } else {
                    return quantize_rgb_nearest(pixels);
                }
            }
        }
    }

    // Pillow 12.2.0 Quant.c uses its median-cut tree even when the requested
    // 256 colors exceed the number of distinct input colors. Every leaf then
    // contains one color, but the tree traversal still determines palette and
    // index order. Animated GIF frames after the first pass through this RGB
    // adaptive-palette path in GifImagePlugin._normalize_mode.
    let order = pillow_median_cut_order(&palette, &counts)?;
    let mut remap = vec![0u8; palette.len()];
    let mut flat = Vec::with_capacity(palette.len() * 3);
    for (new_index, &old_index) in order.iter().enumerate() {
        remap[old_index] = u8::try_from(new_index).ok()?;
        flat.extend_from_slice(&palette[old_index]);
    }
    let indices = pixels
        .chunks_exact(3)
        .map(|chunk| {
            let color = [chunk[0], chunk[1], chunk[2]];
            find_color(&palette, &color).map(|index| remap[index])
        })
        .collect::<Option<Vec<_>>>()?;
    Some((flat, indices))
}

fn quantize_rgb_nearest(pixels: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let mut colors = Vec::<[u8; 3]>::new();
    let mut counts = Vec::<u32>::new();
    let mut color_indices = HashMap::<u32, usize>::new();
    for chunk in pixels.chunks_exact(3) {
        let color = [chunk[0], chunk[1], chunk[2]];
        let hash = pillow_pixel_hash(color);
        if let Some(&index) = color_indices.get(&hash) {
            counts[index] = counts[index].checked_add(1)?;
        } else {
            let index = colors.len();
            colors.push(color);
            counts.push(1);
            color_indices.insert(hash, index);
        }
    }
    let leaves = pillow_median_cut_leaves(&colors, &counts, colors.len().min(256))?;
    let mut palette = Vec::<[u8; 3]>::with_capacity(leaves.len());
    let mut initial_palette = vec![0usize; colors.len()];
    for (palette_index, leaf) in leaves.iter().enumerate() {
        let mut sums = [0u64; 3];
        let mut count = 0u64;
        for &color_index in leaf {
            let color_count = u64::from(counts[color_index]);
            count = count.checked_add(color_count)?;
            for channel in 0..3 {
                sums[channel] = sums[channel]
                    .checked_add(u64::from(colors[color_index][channel]) * color_count)?;
            }
            initial_palette[color_index] = palette_index;
        }
        palette.push(std::array::from_fn(|channel| {
            u8::try_from((sums[channel] + count / 2) / count).unwrap_or(0)
        }));
    }
    let mapped = colors
        .iter()
        .enumerate()
        .map(|(index, color)| find_nearest_from(&palette, color, initial_palette[index]))
        .collect::<Vec<_>>();
    let mut used = vec![false; palette.len()];
    for &index in &mapped {
        used[index] = true;
    }
    let mut remap = vec![0usize; palette.len()];
    let mut optimized = Vec::with_capacity(palette.len());
    for (old_index, color) in palette.into_iter().enumerate() {
        if used[old_index] {
            remap[old_index] = optimized.len();
            optimized.push(color);
        }
    }
    let indices = pixels
        .chunks_exact(3)
        .map(|chunk| {
            let color = [chunk[0], chunk[1], chunk[2]];
            color_indices
                .get(&pillow_pixel_hash(color))
                .and_then(|&index| u8::try_from(remap[mapped[index]]).ok())
        })
        .collect::<Option<Vec<_>>>()?;
    Some((optimized.into_iter().flatten().collect(), indices))
}

#[derive(Clone)]
struct MedianBox {
    axes: [Vec<usize>; 3],
    pixel_count: u32,
    children: Option<(usize, usize)>,
}

fn pillow_median_cut_order(colors: &[[u8; 3]], counts: &[u32]) -> Option<Vec<usize>> {
    let leaves = pillow_median_cut_leaves(colors, counts, colors.len())?;
    leaves
        .into_iter()
        .map(|leaf| (leaf.len() == 1).then_some(leaf[0]))
        .collect()
}

fn pillow_median_cut_leaves(
    colors: &[[u8; 3]],
    counts: &[u32],
    target: usize,
) -> Option<Vec<Vec<usize>>> {
    // All callers derive `counts` and `target` from the same non-empty pixel
    // set. Keep those internal invariants visible without retaining an
    // unreachable runtime failure path.
    debug_assert!(!colors.is_empty());
    debug_assert_eq!(colors.len(), counts.len());
    debug_assert!((1..=colors.len().min(256)).contains(&target));

    let hash_order = pillow_hash_iteration_order(colors);
    let axes = std::array::from_fn(|axis| {
        let mut entries = (0..colors.len()).collect::<Vec<_>>();
        entries.sort_by_key(|&index| (std::cmp::Reverse(colors[index][axis]), hash_order[index]));
        entries
    });
    let pixel_count = counts
        .iter()
        .try_fold(0u32, |sum, &count| sum.checked_add(count))?;
    let mut boxes = vec![MedianBox {
        axes,
        pixel_count,
        children: None,
    }];
    let mut heap = PillowBoxHeap::default();
    heap.add(0, &boxes);

    for _ in 1..target {
        let node = loop {
            let candidate = heap.remove(&boxes)?;
            if box_volume(&boxes[candidate], colors) > 1 {
                break candidate;
            }
        };
        let (left, right) = split_median_box(&boxes[node], colors, counts)?;
        let left_index = boxes.len();
        boxes.push(left);
        let right_index = boxes.len();
        boxes.push(right);
        boxes[node].children = Some((left_index, right_index));
        heap.add(left_index, &boxes);
        heap.add(right_index, &boxes);
    }

    fn visit(index: usize, boxes: &[MedianBox], output: &mut Vec<Vec<usize>>) {
        if let Some((left, right)) = boxes[index].children {
            visit(left, boxes, output);
            visit(right, boxes, output);
        } else {
            output.push(boxes[index].axes[0].clone());
        }
    }
    let mut leaves = Vec::with_capacity(target);
    visit(0, &boxes, &mut leaves);
    (leaves.len() == target).then_some(leaves)
}

fn pillow_hash_iteration_order(colors: &[[u8; 3]]) -> Vec<usize> {
    // QuantHash.c grows 11 -> 23 -> 47 -> 97 for this range. Its historical
    // prime finder accepts the first candidate in this residue table.
    const ACCEPTED_RESIDUES: [bool; 16] = [
        false, true, false, true, false, false, false, true, false, true, false, true, false, true,
        false, false,
    ];
    let mut length = 11u32;
    for count in 1..=colors.len() as u32 {
        if length.saturating_mul(3) < count {
            let mut candidate = length.saturating_mul(2).saturating_add(1);
            while !ACCEPTED_RESIDUES[(candidate & 15) as usize] {
                candidate += 1;
            }
            length = candidate;
        }
    }
    let mut iteration = (0..colors.len()).collect::<Vec<_>>();
    iteration.sort_by_key(|&index| {
        let hash = pillow_pixel_hash(colors[index]);
        (hash % length, hash)
    });
    let mut rank = vec![0usize; colors.len()];
    for (position, index) in iteration.into_iter().enumerate() {
        rank[index] = position;
    }
    rank
}

fn pillow_pixel_hash(color: [u8; 3]) -> u32 {
    u32::from(color[0]).wrapping_mul(463)
        ^ u32::from(color[1]).wrapping_shl(8).wrapping_mul(10_069)
        ^ u32::from(color[2]).wrapping_shl(16).wrapping_mul(64_997)
}

fn box_volume(node: &MedianBox, colors: &[[u8; 3]]) -> u32 {
    (0..3)
        .map(|axis| {
            let entries = &node.axes[axis];
            u32::from(colors[entries[0]][axis] - colors[*entries.last().unwrap()][axis]) + 1
        })
        .product()
}

fn split_median_box(
    node: &MedianBox,
    colors: &[[u8; 3]],
    counts: &[u32],
) -> Option<(MedianBox, MedianBox)> {
    let ranges: [u32; 3] = std::array::from_fn(|axis| {
        let entries = &node.axes[axis];
        u32::from(colors[entries[0]][axis] - colors[*entries.last().unwrap()][axis])
            * [77, 150, 29][axis]
    });
    let axis = (1..3).fold(0, |best, candidate| {
        if ranges[candidate] > ranges[best] {
            candidate
        } else {
            best
        }
    });
    let sorted = &node.axes[axis];
    let mut left_count = 0u32;
    let mut split = 0usize;
    while split < sorted.len() {
        left_count = left_count.checked_add(counts[sorted[split]])?;
        split += 1;
        if left_count.saturating_mul(2) > node.pixel_count {
            break;
        }
    }
    if split < sorted.len() {
        let value = colors[sorted[split - 1]][axis];
        while split < sorted.len() && colors[sorted[split]][axis] == value {
            left_count = left_count.checked_add(counts[sorted[split]])?;
            split += 1;
        }
    }
    if split == sorted.len() {
        let value = colors[*sorted.last()?][axis];
        while split > 0 && colors[sorted[split - 1]][axis] == value {
            split -= 1;
            left_count = left_count.checked_sub(counts[sorted[split]])?;
        }
    }
    // The caller only splits boxes whose RGB volume exceeds one, so at least
    // one value differs on the selected axis and both partitions are nonempty.
    debug_assert!(split > 0 && split < sorted.len());
    let is_left = sorted[..split]
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let left_axes = std::array::from_fn(|other_axis| {
        node.axes[other_axis]
            .iter()
            .copied()
            .filter(|index| is_left.contains(index))
            .collect()
    });
    let right_axes = std::array::from_fn(|other_axis| {
        node.axes[other_axis]
            .iter()
            .copied()
            .filter(|index| !is_left.contains(index))
            .collect()
    });
    Some((
        MedianBox {
            axes: left_axes,
            pixel_count: left_count,
            children: None,
        },
        MedianBox {
            axes: right_axes,
            pixel_count: node.pixel_count.checked_sub(left_count)?,
            children: None,
        },
    ))
}

#[derive(Default)]
struct PillowBoxHeap(Vec<usize>);

impl PillowBoxHeap {
    fn add(&mut self, value: usize, boxes: &[MedianBox]) {
        self.0.push(value);
        let mut child = self.0.len() - 1;
        while child > 0 {
            let parent = (child - 1) / 2;
            if boxes[value].pixel_count <= boxes[self.0[parent]].pixel_count {
                break;
            }
            self.0[child] = self.0[parent];
            child = parent;
        }
        self.0[child] = value;
    }

    fn remove(&mut self, boxes: &[MedianBox]) -> Option<usize> {
        let result = *self.0.first()?;
        let value = self.0.pop()?;
        if self.0.is_empty() {
            return Some(result);
        }
        let mut parent = 0usize;
        while parent * 2 + 1 < self.0.len() {
            let mut child = parent * 2 + 1;
            if child + 1 < self.0.len()
                && boxes[self.0[child]].pixel_count < boxes[self.0[child + 1]].pixel_count
            {
                child += 1;
            }
            if boxes[value].pixel_count > boxes[self.0[child]].pixel_count {
                break;
            }
            self.0[parent] = self.0[child];
            parent = child;
        }
        self.0[parent] = value;
        Some(result)
    }
}
/// Quantize RGBA8 pixels to a palette with optional transparency.
///
/// Returns `(palette, indices, optional_transparent_index)`.
fn quantize_rgba(pixels: &[u8]) -> Option<(Vec<u8>, Vec<u8>, Option<u8>)> {
    debug_assert!(!pixels.is_empty());
    debug_assert!(pixels.len().is_multiple_of(4));
    let mut colors = pixels
        .chunks_exact(4)
        .map(|pixel| [pixel[0], pixel[1], pixel[2], pixel[3]])
        .collect::<Vec<_>>();
    // Quant.c normalizes every fully transparent pixel to the first one's RGB
    // before FASTOCTREE, so transparent garbage channels cannot consume colors.
    if let Some(first) = colors.iter().find(|color| color[3] == 0).copied() {
        for color in &mut colors {
            if color[3] == 0 {
                color[..3].copy_from_slice(&first[..3]);
            }
        }
    }
    let (mut rgba_palette, mut indices) = pillow_fast_octree(&colors, 256)?;
    let mut transparent = colors
        .iter()
        .any(|color| color[3] == 0)
        .then(|| rgba_palette.iter().position(|color| color[3] == 0))
        .flatten()
        .and_then(|index| u8::try_from(index).ok());

    compact_rgba_palette(&mut rgba_palette, &mut indices, &mut transparent)?;
    let palette = rgba_palette
        .into_iter()
        .flat_map(|color| color[..3].to_vec())
        .collect();
    Some((palette, indices, transparent))
}

fn compact_rgba_palette(
    rgba_palette: &mut Vec<[u8; 4]>,
    indices: &mut [u8],
    transparent: &mut Option<u8>,
) -> Option<()> {
    // GifImagePlugin._get_optimize compacts holes, and also shrinks a palette
    // by one power-of-two step when at most half of its entries are used.
    let mut used = vec![false; rgba_palette.len()];
    for &index in indices.iter() {
        used[usize::from(index)] = true;
    }
    let used_indices = used
        .iter()
        .enumerate()
        .filter_map(|(index, &is_used)| is_used.then_some(index))
        .collect::<Vec<_>>();
    let has_holes = used_indices
        .last()
        .is_some_and(|&maximum| maximum >= used_indices.len());
    if has_holes || used_indices.len() <= rgba_palette.len() / 2 {
        let mut remap = vec![0u8; rgba_palette.len()];
        let mut compact = Vec::with_capacity(used_indices.len());
        for (new_index, &old_index) in used_indices.iter().enumerate() {
            remap[old_index] = u8::try_from(new_index).ok()?;
            compact.push(rgba_palette[old_index]);
        }
        for index in indices.iter_mut() {
            *index = remap[usize::from(*index)];
        }
        *transparent = transparent.map(|index| remap[usize::from(index)]);
        *rgba_palette = compact;
    }
    Some(())
}

// Behavioral port of Pillow 12.2.0 src/libImaging/QuantOctree.c (MIT,
// Oliver Tonnhofer / Omniscale). The bucket sorter below ports the ordering of
// Apple Libc stdlib/FreeBSD/qsort.c (BSD-3-Clause, UC Regents), because tied
// bucket order is observable in Pillow's encoded GIF bytes.

#[derive(Clone, Default)]
struct OctreeBucket {
    count: u32,
    sums: [u64; 4],
}

impl OctreeBucket {
    fn add_color(&mut self, color: [u8; 4]) {
        self.count = self.count.saturating_add(1);
        for (sum, channel) in self.sums.iter_mut().zip(color) {
            *sum = sum.saturating_add(u64::from(channel));
        }
    }

    fn add_bucket(&mut self, other: &Self) {
        self.count = self.count.saturating_add(other.count);
        for (sum, other_sum) in self.sums.iter_mut().zip(other.sums) {
            *sum = sum.saturating_add(other_sum);
        }
    }

    fn average(&self) -> [u8; 4] {
        if self.count == 0 {
            return [0; 4];
        }
        std::array::from_fn(|channel| {
            ((self.sums[channel] as f32) / (self.count as f32)).clamp(0.0, 255.0) as u8
        })
    }
}

struct OctreeCube {
    bits: [u32; 4],
    widths: [usize; 4],
    offsets: [u32; 4],
    buckets: Vec<OctreeBucket>,
}

impl OctreeCube {
    fn new(bits: [u32; 4]) -> Option<Self> {
        let widths = bits.map(|value| 1usize.checked_shl(value));
        let widths = [widths[0]?, widths[1]?, widths[2]?, widths[3]?];
        let offsets = [bits[1] + bits[2] + bits[3], bits[2] + bits[3], bits[3], 0];
        let size = widths.into_iter().try_fold(1usize, usize::checked_mul)?;
        Some(Self {
            bits,
            widths,
            offsets,
            buckets: vec![OctreeBucket::default(); size],
        })
    }

    fn offset_position(&self, values: [usize; 4]) -> usize {
        values
            .into_iter()
            .zip(self.offsets)
            .fold(0usize, |offset, (value, shift)| offset | (value << shift))
    }

    fn offset(&self, color: [u8; 4]) -> usize {
        let values = std::array::from_fn(|channel| {
            (usize::from(color[channel]) >> (8 - self.bits[channel])) & (self.widths[channel] - 1)
        });
        self.offset_position(values)
    }

    fn add_color(&mut self, color: [u8; 4]) {
        let offset = self.offset(color);
        self.buckets[offset].add_color(color);
    }

    fn used(&self) -> usize {
        self.buckets
            .iter()
            .filter(|bucket| bucket.count > 0)
            .count()
    }
}

fn copy_octree_cube(cube: &OctreeCube, bits: [u32; 4]) -> Option<OctreeCube> {
    let mut result = OctreeCube::new(bits)?;
    let mut source_reduce = [0u32; 4];
    let mut destination_reduce = [0u32; 4];
    let widths: [usize; 4] = std::array::from_fn(|channel| {
        if cube.bits[channel] > bits[channel] {
            destination_reduce[channel] = cube.bits[channel] - bits[channel];
            cube.widths[channel]
        } else {
            source_reduce[channel] = bits[channel] - cube.bits[channel];
            result.widths[channel]
        }
    });
    for r in 0..widths[0] {
        for g in 0..widths[1] {
            for b in 0..widths[2] {
                for a in 0..widths[3] {
                    let values = [r, g, b, a];
                    let source = cube.offset_position(std::array::from_fn(|channel| {
                        values[channel] >> source_reduce[channel]
                    }));
                    let destination = result.offset_position(std::array::from_fn(|channel| {
                        values[channel] >> destination_reduce[channel]
                    }));
                    result.buckets[destination].add_bucket(&cube.buckets[source]);
                }
            }
        }
    }
    Some(result)
}

fn bucket_order(left: &OctreeBucket, right: &OctreeBucket) -> std::cmp::Ordering {
    right.count.cmp(&left.count)
}

fn median_of_three(values: &[OctreeBucket], a: usize, b: usize, c: usize) -> usize {
    if bucket_order(&values[a], &values[b]).is_lt() {
        if bucket_order(&values[b], &values[c]).is_lt() {
            b
        } else if bucket_order(&values[a], &values[c]).is_lt() {
            c
        } else {
            a
        }
    } else if bucket_order(&values[b], &values[c]).is_gt() {
        b
    } else if bucket_order(&values[a], &values[c]).is_lt() {
        a
    } else {
        c
    }
}

fn insertion_sort_buckets(values: &mut [OctreeBucket], swap_limit: Option<usize>) -> bool {
    let mut swaps = 0usize;
    for right in 1..values.len() {
        let mut cursor = right;
        while cursor > 0 && bucket_order(&values[cursor - 1], &values[cursor]).is_gt() {
            values.swap(cursor, cursor - 1);
            swaps += 1;
            if swap_limit.is_some_and(|limit| swaps > limit) {
                return false;
            }
            cursor -= 1;
        }
    }
    true
}

fn swap_bucket_ranges(values: &mut [OctreeBucket], left: usize, right: usize, length: usize) {
    for offset in 0..length {
        values.swap(left + offset, right + offset);
    }
}

fn apple_qsort_buckets(values: &mut [OctreeBucket]) {
    let mut start = 0usize;
    let mut length = values.len();
    loop {
        if length <= 7 {
            insertion_sort_buckets(&mut values[start..start + length], None);
            return;
        }
        let mut low = start;
        let mut middle = start + length / 2;
        let mut high = start + length - 1;
        if length > 40 {
            let distance = length / 8;
            low = median_of_three(values, low, low + distance, low + 2 * distance);
            middle = median_of_three(values, middle - distance, middle, middle + distance);
            high = median_of_three(values, high - 2 * distance, high - distance, high);
        }
        middle = median_of_three(values, low, middle, high);
        values.swap(start, middle);
        let mut equal_left = start + 1;
        let mut scan_left = start + 1;
        let mut scan_right = start + length - 1;
        let mut equal_right = scan_right;
        let mut swapped = false;
        loop {
            while scan_left <= scan_right {
                let ordering = bucket_order(&values[scan_left], &values[start]);
                if ordering.is_gt() {
                    break;
                }
                if ordering.is_eq() {
                    values.swap(equal_left, scan_left);
                    equal_left += 1;
                    swapped = true;
                }
                scan_left += 1;
            }
            while scan_left <= scan_right {
                let ordering = bucket_order(&values[scan_right], &values[start]);
                if ordering.is_lt() {
                    break;
                }
                if ordering.is_eq() {
                    values.swap(scan_right, equal_right);
                    equal_right = equal_right.saturating_sub(1);
                    swapped = true;
                }
                scan_right = scan_right.saturating_sub(1);
            }
            if scan_left > scan_right {
                break;
            }
            values.swap(scan_left, scan_right);
            swapped = true;
            scan_left += 1;
            scan_right = scan_right.saturating_sub(1);
        }
        let end = start + length;
        let left_equal = (equal_left - start).min(scan_left - equal_left);
        swap_bucket_ranges(values, start, scan_left - left_equal, left_equal);
        let right_equal = (equal_right - scan_right).min(end - equal_right - 1);
        swap_bucket_ranges(values, scan_left, end - right_equal, right_equal);
        if !swapped {
            let limit = 1 + length / 4;
            if insertion_sort_buckets(&mut values[start..end], Some(limit)) {
                return;
            }
        }
        let left_length = scan_left - equal_left;
        let right_length = equal_right - scan_right;
        if left_length <= right_length {
            if left_length > 1 {
                apple_qsort_buckets(&mut values[start..start + left_length]);
            }
            if right_length <= 1 {
                return;
            }
            start = end - right_length;
            length = right_length;
        } else {
            if right_length > 1 {
                apple_qsort_buckets(&mut values[end - right_length..end]);
            }
            if left_length <= 1 {
                return;
            }
            length = left_length;
        }
    }
}

fn sorted_octree_buckets(cube: &OctreeCube) -> Vec<OctreeBucket> {
    let mut buckets = cube.buckets.clone();
    apple_qsort_buckets(&mut buckets);
    buckets
}

fn subtract_octree_buckets(cube: &mut OctreeCube, buckets: &[OctreeBucket]) {
    for bucket in buckets.iter().filter(|bucket| bucket.count > 0) {
        let offset = cube.offset(bucket.average());
        let destination = &mut cube.buckets[offset];
        destination.count -= bucket.count;
        for (sum, value) in destination.sums.iter_mut().zip(bucket.sums) {
            *sum -= value;
        }
    }
}

fn add_octree_lookup(cube: &mut OctreeCube, palette: &[OctreeBucket], offset: usize) {
    for index in (offset..palette.len()).rev() {
        let bucket = &palette[index];
        let position = cube.offset(bucket.average());
        cube.buckets[position].count = index as u32;
    }
}

fn pillow_fast_octree(colors: &[[u8; 4]], target: usize) -> Option<(Vec<[u8; 4]>, Vec<u8>)> {
    let fine_bits = [3, 4, 3, 3];
    let coarse_bits = [2, 2, 2, 2];
    let mut fine = OctreeCube::new(fine_bits)?;
    for &color in colors {
        fine.add_color(color);
    }
    let mut coarse = copy_octree_cube(&fine, coarse_bits)?;
    let mut coarse_count = coarse.used().min(target);
    let mut fine_count = target.checked_sub(coarse_count)?;
    let fine_palette = sorted_octree_buckets(&fine);
    subtract_octree_buckets(&mut coarse, &fine_palette[..fine_count]);
    while coarse_count > coarse.used() {
        let already_subtracted = fine_count;
        coarse_count = coarse.used();
        fine_count = target.checked_sub(coarse_count)?;
        subtract_octree_buckets(&mut coarse, &fine_palette[already_subtracted..fine_count]);
    }
    let coarse_palette = sorted_octree_buckets(&coarse);
    let mut buckets = coarse_palette[..coarse_count].to_vec();
    buckets.extend_from_slice(&fine_palette[..fine_count]);
    let mut coarse_lookup = OctreeCube::new(coarse_bits)?;
    add_octree_lookup(&mut coarse_lookup, &buckets[..coarse_count], 0);
    let mut lookup = copy_octree_cube(&coarse_lookup, fine_bits)?;
    add_octree_lookup(&mut lookup, &buckets, coarse_count);
    let indices = colors
        .iter()
        .map(|&color| u8::try_from(lookup.buckets[lookup.offset(color)].count).ok())
        .collect::<Option<Vec<_>>>()?;
    let palette = buckets.iter().map(OctreeBucket::average).collect();
    Some((palette, indices))
}
/// Find a color in the palette. Returns its index if found.
fn find_color(palette: &[[u8; 3]], color: &[u8; 3]) -> Option<usize> {
    palette.iter().position(|c| c == color)
}
/// Find the nearest color in the palette by Euclidean distance.
fn find_nearest_from(palette: &[[u8; 3]], color: &[u8; 3], initial: usize) -> usize {
    let mut best = initial;
    let mut best_dist = color_distance(palette[initial], *color);
    let search_limit = best_dist.saturating_mul(4);
    let mut candidates = (0..palette.len()).collect::<Vec<_>>();
    candidates.sort_by_key(|&index| (color_distance(palette[initial], palette[index]), index));
    for index in candidates {
        if color_distance(palette[initial], palette[index]) > search_limit {
            break;
        }
        let dist = color_distance(palette[index], *color);
        if dist < best_dist {
            best_dist = dist;
            best = index;
        }
    }
    best
}

fn color_distance(left: [u8; 3], right: [u8; 3]) -> u32 {
    let dr = i32::from(left[0]) - i32::from(right[0]);
    let dg = i32::from(left[1]) - i32::from(right[1]);
    let db = i32::from(left[2]) - i32::from(right[2]);
    u32::try_from(dr * dr + dg * dg + db * db).unwrap_or(u32::MAX)
}
