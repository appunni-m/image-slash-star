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
    let frames = coalesce_identical_frames(sequence, requested_frames)?;
    write_gif(sequence, &frames, opts, settings)
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
    let mut restore_canvas = canvas.clone();
    let mut previous_frame = None::<&crate::types::DecodedFrame>;
    let mut previous_render = None::<Vec<u8>>;
    let mut output = Vec::<crate::types::DecodedFrame>::new();

    for frame in sequence.frames.iter().take(requested_frames) {
        if let Some(previous) = previous_frame {
            match previous.disposal {
                FrameDisposal::Unspecified | FrameDisposal::Keep => {}
                FrameDisposal::Background => clear_frame_rect(&mut canvas, width, previous)?,
                FrameDisposal::Previous => canvas.copy_from_slice(&restore_canvas),
            }
        }

        let before_frame = canvas.clone();
        composite_indexed_frame(&mut canvas, width, frame)?;
        let identical = previous_render.as_deref() == Some(canvas.as_slice())
            && output
                .last()
                .is_some_and(|previous| previous.disposal == frame.disposal);
        if identical {
            let previous = output.last_mut()?;
            previous.duration_ms = previous.duration_ms.checked_add(frame.duration_ms)?;
        } else {
            let mut output_frame = frame.clone();
            if !output.is_empty() {
                let rgb = canvas
                    .chunks_exact(4)
                    .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
                    .collect();
                output_frame.image =
                    DecodedImage::new(sequence.width, sequence.height, rgb, ColorType::Rgb8);
                output_frame.left = 0;
                output_frame.top = 0;
            }
            output.push(output_frame);
            previous_render = Some(canvas.clone());
        }
        restore_canvas = before_frame;
        previous_frame = Some(frame);
    }
    Some(output)
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

fn composite_indexed_frame(
    canvas: &mut [u8],
    canvas_width: usize,
    frame: &crate::types::DecodedFrame,
) -> Option<()> {
    let image = &frame.image;
    if image.mode != ImageMode::P8 {
        return None;
    }
    let palette = image.palette.as_ref()?;
    let left = usize::try_from(frame.left).ok()?;
    let top = usize::try_from(frame.top).ok()?;
    let width = usize::try_from(image.width).ok()?;
    let height = usize::try_from(image.height).ok()?;
    for y in 0..height {
        for x in 0..width {
            let source = y.checked_mul(width)?.checked_add(x)?;
            let index = usize::from(*image.pixels.get(source)?);
            let alpha = palette.alpha.get(index).copied().unwrap_or(255);
            if alpha == 0 {
                continue;
            }
            let palette_offset = index.checked_mul(3)?;
            let rgb = palette.rgb.get(palette_offset..palette_offset + 3)?;
            let destination = (top
                .checked_add(y)?
                .checked_mul(canvas_width)?
                .checked_add(left)?
                .checked_add(x)?)
            .checked_mul(4)?;
            canvas
                .get_mut(destination..destination + 4)?
                .copy_from_slice(&[rgb[0], rgb[1], rgb[2], alpha]);
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
            if img.pixels.chunks_exact(4).all(|pixel| pixel[3] >= 128) {
                let rgb = img
                    .pixels
                    .chunks_exact(4)
                    .flat_map(|pixel| pixel[..3].iter().copied())
                    .collect::<Vec<_>>();
                let (palette, indices) = quantize_rgb(&rgb)?;
                (palette, indices, None)
            } else {
                let (palette, indices, transparent_idx) = quantize_rgba(&img.pixels);
                (palette, indices, transparent_idx)
            }
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
    // Pillow's GIF writer normalizes even a one-color image to a four-entry
    // table while retaining the GIF-mandated minimum LZW code width of two.
    let color_count = (palette.len() / 3).max(4).next_power_of_two();
    let table_bits = usize::BITS - color_count.leading_zeros() - 1;
    let size_field = u8::try_from(table_bits.checked_sub(1)?).ok()?;
    // Pillow's P-mode GIF encoder uses an eight-bit LZW root alphabet even
    // when the emitted color table contains fewer entries.
    let minimum_code_size = 8;
    Some((color_count, size_field, minimum_code_size))
}

fn write_gif(
    sequence: &DecodedSequence,
    frames: &[crate::types::DecodedFrame],
    opts: &EncodeOptions,
    settings: GifSettings,
) -> Option<Vec<u8>> {
    let width = u16::try_from(sequence.width).ok()?;
    let height = u16::try_from(sequence.height).ok()?;
    let first = prepare_image(&frames.first()?.image)?;
    let (global_count, global_size, _) = table_parameters(&first.palette)?;
    let global_table = !settings.local_color_table;

    let needs_89a = frames.len() > 1
        || settings.loop_count.is_some()
        || option_bool(opts, "transparency") == Some(true)
        || frames.iter().any(|frame| {
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

    let mut previous_rgb = None::<Vec<u8>>;
    for frame in frames {
        let mut prepared = prepare_image(&frame.image)?;
        let frame_rgb = image_rgb(&frame.image)?;
        if let Some(previous) = previous_rgb.as_deref()
            && prepared.transparent.is_none()
            && prepared.palette.len() / 3 < 256
            && previous.len() == frame_rgb.len()
        {
            let transparent = u8::try_from(prepared.palette.len() / 3).ok()?;
            for (index, (before, after)) in previous
                .chunks_exact(3)
                .zip(frame_rgb.chunks_exact(3))
                .enumerate()
            {
                if before == after {
                    prepared.indices[index] = transparent;
                }
            }
            prepared.transparent = Some(transparent);
        }
        previous_rgb = Some(frame_rgb);
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

fn image_rgb(image: &DecodedImage) -> Option<Vec<u8>> {
    match image.mode {
        ImageMode::Rgb8 if image.color == ColorType::Rgb8 => Some(image.pixels.clone()),
        ImageMode::Rgba8 if image.color == ColorType::Rgba8 => Some(
            image
                .pixels
                .chunks_exact(4)
                .flat_map(|pixel| pixel[..3].iter().copied())
                .collect(),
        ),
        ImageMode::P8 if image.color == ColorType::L8 => {
            let palette = image.palette.as_ref()?;
            let mut rgb = Vec::with_capacity(image.pixels.len().checked_mul(3)?);
            for &index in &image.pixels {
                let offset = usize::from(index).checked_mul(3)?;
                rgb.extend_from_slice(palette.rgb.get(offset..offset + 3)?);
            }
            Some(rgb)
        }
        ImageMode::L8 if image.color == ColorType::L8 => {
            let mut rgb = Vec::with_capacity(image.pixels.len().checked_mul(3)?);
            for &value in &image.pixels {
                rgb.extend_from_slice(&[value, value, value]);
            }
            Some(rgb)
        }
        _ => None,
    }
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
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut indices = Vec::with_capacity(pixels.len() / 3);
    for chunk in pixels.chunks_exact(3) {
        let color = [chunk[0], chunk[1], chunk[2]];
        let index = match find_color(&palette, &color) {
            Some(index) => index,
            None if palette.len() < 256 => {
                palette.push(color);
                palette.len() - 1
            }
            None => find_nearest(&palette, &color),
        };
        indices.push(u8::try_from(index).ok()?);
    }
    Some((palette.into_iter().flatten().collect(), indices))
}

#[derive(Clone)]
struct MedianBox {
    axes: [Vec<usize>; 3],
    pixel_count: u32,
    children: Option<(usize, usize)>,
}

fn pillow_median_cut_order(colors: &[[u8; 3]], counts: &[u32]) -> Option<Vec<usize>> {
    if colors.is_empty() || colors.len() != counts.len() || colors.len() > 256 {
        return None;
    }

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

    for _ in 1..colors.len() {
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

    fn visit(index: usize, boxes: &[MedianBox], output: &mut Vec<usize>) {
        if let Some((left, right)) = boxes[index].children {
            visit(left, boxes, output);
            visit(right, boxes, output);
        } else if let Some(&color) = boxes[index].axes[0].first() {
            output.push(color);
        }
    }
    let mut order = Vec::with_capacity(colors.len());
    visit(0, &boxes, &mut order);
    (order.len() == colors.len()).then_some(order)
}

fn pillow_hash_iteration_order(colors: &[[u8; 3]]) -> Vec<usize> {
    fn hash(color: [u8; 3]) -> u32 {
        u32::from(color[0]).wrapping_mul(463)
            ^ u32::from(color[1]).wrapping_shl(8).wrapping_mul(10_069)
            ^ u32::from(color[2]).wrapping_shl(16).wrapping_mul(64_997)
    }

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
    iteration.sort_by_key(|&index| (hash(colors[index]) % length, hash(colors[index])));
    let mut rank = vec![0usize; colors.len()];
    for (position, index) in iteration.into_iter().enumerate() {
        rank[index] = position;
    }
    rank
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
    if split == 0 || split == sorted.len() {
        return None;
    }
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
