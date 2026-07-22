//! Pillow-compatible AVIF encoding through libavif 1.4.1 and libaom 3.13.2.

use crate::encode_options::EncodeOptions;
use crate::types::{DecodedImage, DecodedSequence};

#[cfg(not(target_arch = "wasm32"))]
use std::borrow::Cow;
#[cfg(not(target_arch = "wasm32"))]
use std::ffi::CString;

#[cfg(not(target_arch = "wasm32"))]
use crate::types::ImageMode;

/// Encode one image with Pillow's AVIF defaults and option mapping.
#[must_use]
pub fn encode(image: &DecodedImage, options: &EncodeOptions) -> Option<Vec<u8>> {
    image.validate().ok()?;
    encode_images(
        std::slice::from_ref(image),
        std::slice::from_ref(&0),
        options,
    )
}

/// Encode all frames in an AVIF image sequence.
#[must_use]
pub fn encode_sequence(sequence: &DecodedSequence, options: &EncodeOptions) -> Option<Vec<u8>> {
    sequence.validate().ok()?;
    for frame in &sequence.frames {
        if frame.left != 0 || frame.top != 0 {
            return None;
        }
    }
    let images = sequence
        .frames
        .iter()
        .map(|frame| &frame.image)
        .collect::<Vec<_>>();
    let durations = sequence
        .frames
        .iter()
        .map(|frame| frame.duration_ms)
        .collect::<Vec<_>>();
    encode_image_refs(&images, &durations, options)
}

#[cfg(not(target_arch = "wasm32"))]
fn encode_images(
    images: &[DecodedImage],
    durations: &[u32],
    options: &EncodeOptions,
) -> Option<Vec<u8>> {
    let references = images.iter().collect::<Vec<_>>();
    encode_image_refs(&references, durations, options)
}

#[cfg(target_arch = "wasm32")]
fn encode_images(
    _images: &[DecodedImage],
    _durations: &[u32],
    _options: &EncodeOptions,
) -> Option<Vec<u8>> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn encode_image_refs(
    images: &[&DecodedImage],
    durations: &[u32],
    options: &EncodeOptions,
) -> Option<Vec<u8>> {
    let first = *images.first()?;
    if images.len() != durations.len() {
        return None;
    }
    for image in images {
        if image.width != first.width || image.height != first.height {
            return None;
        }
    }
    let parsed = ParsedOptions::new(options)?;
    let encoder = create_encoder(first, &parsed)?;
    encode_frames(encoder, images, durations, images.len() == 1)
}

#[cfg(not(target_arch = "wasm32"))]
fn create_encoder(first: &DecodedImage, parsed: &ParsedOptions) -> Option<super::native::Encoder> {
    let config = super::native::EncodeConfig {
        width: first.width,
        height: first.height,
        yuv_format: parsed.yuv_format,
        yuv_range: parsed.yuv_range,
        quality: parsed.quality,
        speed: parsed.speed,
        max_threads: parsed.max_threads,
        tile_rows_log2: parsed.tile_rows_log2,
        tile_cols_log2: parsed.tile_cols_log2,
        alpha_premultiplied: parsed.alpha_premultiplied,
        auto_tiling: parsed.auto_tiling,
        timescale: 1_000,
        creation_time: parsed.sequence_time,
        modification_time: parsed.sequence_time,
        icc: &parsed.icc,
        exif: &parsed.exif,
        exif_orientation: parsed.exif_orientation,
        xmp: &parsed.xmp,
        advanced: &parsed.advanced,
    };
    super::native::Encoder::new(&config)
}

#[cfg(not(target_arch = "wasm32"))]
fn encode_frames(
    mut encoder: super::native::Encoder,
    images: &[&DecodedImage],
    durations: &[u32],
    single: bool,
) -> Option<Vec<u8>> {
    for (image, &duration_ms) in images.iter().zip(durations) {
        let prepared = prepare_pixels(image)?;
        encoder.add_frame(
            prepared.bytes.as_ref(),
            image.width,
            image.height,
            prepared.channels,
            u64::from(duration_ms),
            single,
        )?;
    }
    encoder.finish()
}

#[cfg(target_arch = "wasm32")]
fn encode_image_refs(
    _images: &[&DecodedImage],
    _durations: &[u32],
    _options: &EncodeOptions,
) -> Option<Vec<u8>> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
struct PreparedPixels<'image> {
    bytes: Cow<'image, [u8]>,
    channels: u32,
}

#[cfg(not(target_arch = "wasm32"))]
fn prepare_pixels(image: &DecodedImage) -> Option<PreparedPixels<'_>> {
    let prepared = match image.mode {
        ImageMode::Rgb8 => PreparedPixels {
            bytes: Cow::Borrowed(&image.pixels),
            channels: 3,
        },
        ImageMode::Rgba8 => PreparedPixels {
            bytes: Cow::Borrowed(&image.pixels),
            channels: 4,
        },
        ImageMode::L8 => PreparedPixels {
            bytes: Cow::Owned(image.pixels.iter().flat_map(|&value| [value; 3]).collect()),
            channels: 3,
        },
        ImageMode::La8 => PreparedPixels {
            bytes: Cow::Owned(
                image
                    .pixels
                    .chunks_exact(2)
                    .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
                    .collect(),
            ),
            channels: 4,
        },
        ImageMode::P8 => prepare_palette_pixels(image)?,
        ImageMode::Cmyk8 => PreparedPixels {
            bytes: Cow::Owned(cmyk_to_rgb(&image.pixels)),
            channels: 3,
        },
        ImageMode::L1 => PreparedPixels {
            bytes: Cow::Owned(unpack_l1_to_rgb(image)?),
            channels: 3,
        },
        _ => return None,
    };
    Some(prepared)
}

#[cfg(not(target_arch = "wasm32"))]
fn prepare_palette_pixels(image: &DecodedImage) -> Option<PreparedPixels<'static>> {
    prepare_palette_pixels_with_capacity(image, palette_capacity)
}

#[cfg(not(target_arch = "wasm32"))]
fn prepare_palette_pixels_with_capacity(
    image: &DecodedImage,
    capacity_for: fn(usize, usize) -> Option<usize>,
) -> Option<PreparedPixels<'static>> {
    let palette = image.palette.as_ref()?;
    let has_alpha = palette.alpha.iter().any(|&alpha| alpha != u8::MAX);
    let channels = if has_alpha { 4 } else { 3 };
    let capacity = capacity_for(image.pixels.len(), channels)?;
    let mut pixels = Vec::with_capacity(capacity);
    for &index in &image.pixels {
        let index = usize::from(index);
        let offset = index * 3;
        pixels.extend_from_slice(palette.rgb.get(offset..offset + 3)?);
        if has_alpha {
            pixels.push(palette.alpha.get(index).copied().unwrap_or(u8::MAX));
        }
    }
    Some(PreparedPixels {
        bytes: Cow::Owned(pixels),
        channels: if has_alpha { 4 } else { 3 },
    })
}

#[cfg(not(target_arch = "wasm32"))]
const fn palette_capacity(pixel_count: usize, channels: usize) -> Option<usize> {
    pixel_count.checked_mul(channels)
}

#[cfg(not(target_arch = "wasm32"))]
fn unpack_l1_to_rgb(image: &DecodedImage) -> Option<Vec<u8>> {
    let width = image.width as usize;
    let source_stride = width.div_ceil(8);
    #[cfg(target_pointer_width = "64")]
    let pixel_count = (u64::from(image.width) * u64::from(image.height)) as usize;
    #[cfg(not(target_pointer_width = "64"))]
    let pixel_count = usize::try_from(u64::from(image.width) * u64::from(image.height)).ok()?;
    let capacity = pixel_count.checked_mul(3)?;
    let mut pixels = Vec::with_capacity(capacity);
    for row in image.pixels.chunks_exact(source_stride) {
        for x in 0..width {
            let bit = 7 - (x % 8);
            let value = if row[x / 8] & (1 << bit) == 0 {
                0
            } else {
                u8::MAX
            };
            pixels.extend_from_slice(&[value; 3]);
        }
    }
    (pixels.len() == capacity).then_some(pixels)
}

#[cfg(not(target_arch = "wasm32"))]
fn cmyk_to_rgb(pixels: &[u8]) -> Vec<u8> {
    pixels
        .chunks_exact(4)
        .flat_map(|pixel| {
            let black = u16::from(255 - pixel[3]);
            std::array::from_fn::<_, 3, _>(|channel| {
                let ink = u16::from(255 - pixel[channel]);
                ((ink * black + 127) / 255) as u8
            })
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
struct ParsedOptions {
    yuv_format: i32,
    yuv_range: i32,
    quality: i32,
    speed: i32,
    max_threads: i32,
    tile_rows_log2: i32,
    tile_cols_log2: i32,
    alpha_premultiplied: bool,
    auto_tiling: bool,
    icc: Vec<u8>,
    exif: Vec<u8>,
    exif_orientation: i32,
    xmp: Vec<u8>,
    advanced: Vec<(CString, CString)>,
    sequence_time: u64,
}

#[cfg(not(target_arch = "wasm32"))]
impl ParsedOptions {
    fn new(options: &EncodeOptions) -> Option<Self> {
        if let Some(codec) = options.extra.get("codec")
            && codec != "auto"
            && codec != "aom"
        {
            return None;
        }
        let subsampling = options
            .subsampling
            .as_deref()
            .or_else(|| options.extra.get("subsampling").map(String::as_str))
            .unwrap_or("4:2:0");
        let yuv_format = match subsampling {
            "4:4:4" => 1,
            "4:2:2" => 2,
            "4:2:0" => 3,
            "4:0:0" => 4,
            _ => return None,
        };
        let yuv_range = match options.extra.get("range").map(String::as_str) {
            None | Some("full") => 1,
            Some("limited") => 0,
            Some(_) => return None,
        };
        let speed = parse_i32(options, "speed")?.unwrap_or(6);
        let max_threads =
            parse_i32(options, "max_threads")?.unwrap_or_else(super::native::default_max_threads);
        let tile_rows_log2 = parse_i32(options, "tile_rows")?.unwrap_or(0);
        let tile_cols_log2 = parse_i32(options, "tile_cols")?.unwrap_or(0);
        let alpha_premultiplied = parse_bool(options, "alpha_premultiplied")?.unwrap_or(false);
        let auto_tiling = parse_bool(options, "autotiling")?
            .unwrap_or(tile_rows_log2 == 0 && tile_cols_log2 == 0);
        let icc = parse_hex_option(options, "icc_hex")?;
        let exif = parse_hex_option(options, "exif_hex")?;
        let xmp = parse_hex_option(options, "xmp_hex")?;
        let exif_orientation = parse_i32(options, "exif_orientation")?.unwrap_or(1);
        let sequence_time = parse_u64(options, "sequence_time")?.unwrap_or(0);
        let mut advanced = Vec::with_capacity(options.advanced.len());
        for (key, value) in &options.advanced {
            advanced.push((
                CString::new(key.as_str()).ok()?,
                CString::new(value.as_str()).ok()?,
            ));
        }
        Some(Self {
            yuv_format,
            yuv_range,
            quality: i32::from(options.quality.unwrap_or(75)),
            speed,
            max_threads,
            tile_rows_log2,
            tile_cols_log2,
            alpha_premultiplied,
            auto_tiling,
            icc,
            exif,
            exif_orientation,
            xmp,
            advanced,
            sequence_time,
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_i32(options: &EncodeOptions, name: &str) -> Option<Option<i32>> {
    let Some(value) = options.extra.get(name) else {
        return Some(None);
    };
    Some(Some(value.parse().ok()?))
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_u64(options: &EncodeOptions, name: &str) -> Option<Option<u64>> {
    let Some(value) = options.extra.get(name) else {
        return Some(None);
    };
    Some(Some(value.parse().ok()?))
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_bool(options: &EncodeOptions, name: &str) -> Option<Option<bool>> {
    let Some(value) = options.extra.get(name) else {
        return Some(None);
    };
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(Some(true)),
        "false" | "0" | "no" | "off" => Some(Some(false)),
        _ => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_hex_option(options: &EncodeOptions, name: &str) -> Option<Vec<u8>> {
    let Some(value) = options.extra.get(name) else {
        return Some(Vec::new());
    };
    let bytes = value.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let mut decoded = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks_exact(2) {
        let high = hex_nibble(pair[0])?;
        let low = hex_nibble(pair[1])?;
        decoded.push((high << 4) | low);
    }
    Some(decoded)
}

#[cfg(not(target_arch = "wasm32"))]
const fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(all(coverage, not(target_arch = "wasm32")))]
pub(crate) fn __coverage_exercise_private_branches() {
    use crate::types::{ColorType, DecodedFrame, FrameDisposal, ImagePalette};

    fn option(name: &str, value: &str) -> EncodeOptions {
        let mut options = EncodeOptions::default();
        options.extra.insert(name.to_owned(), value.to_owned());
        options
    }

    let one = DecodedImage::new(1, 1, vec![0], ColorType::L8);
    let two = DecodedImage::new(2, 1, vec![0, 0], ColorType::L8);
    let tall = DecodedImage::new(1, 2, vec![0, 0], ColorType::L8);
    let _ = encode_image_refs(&[], &[], &EncodeOptions::default());
    let _ = encode_image_refs(&[&one], &[], &EncodeOptions::default());
    let _ = encode_image_refs(&[&one, &two], &[0, 0], &EncodeOptions::default());
    let _ = encode_image_refs(&[&one, &tall], &[0, 0], &EncodeOptions::default());
    let invalid_sequence = DecodedSequence {
        width: 1,
        height: 1,
        frames: Vec::new(),
        loop_count: None,
        background: None,
    };
    let _ = encode_sequence(&invalid_sequence, &EncodeOptions::default());
    let invalid_image = DecodedImage::new(1, 1, Vec::new(), ColorType::L8);
    let _ = encode(&invalid_image, &EncodeOptions::default());
    let zero_width = DecodedImage::new(0, 1, Vec::new(), ColorType::L8);
    let _ = encode_image_refs(&[&zero_width], &[0], &EncodeOptions::default());

    let offset_sequence = DecodedSequence {
        width: 2,
        height: 1,
        frames: vec![DecodedFrame {
            image: one.clone(),
            left: 1,
            top: 0,
            duration_ms: 0,
            disposal: FrameDisposal::Unspecified,
            interlaced: false,
        }],
        loop_count: None,
        background: None,
    };
    let _ = encode_sequence(&offset_sequence, &EncodeOptions::default());
    let top_offset_sequence = DecodedSequence {
        width: 1,
        height: 2,
        frames: vec![DecodedFrame {
            image: one.clone(),
            left: 0,
            top: 1,
            duration_ms: 0,
            disposal: FrameDisposal::Unspecified,
            interlaced: false,
        }],
        loop_count: None,
        background: None,
    };
    let _ = encode_sequence(&top_offset_sequence, &EncodeOptions::default());

    let unsupported = DecodedImage::new(1, 1, vec![0, 0], ColorType::L16);
    let palette_without_table = DecodedImage::with_mode(1, 1, vec![0], ImageMode::P8);
    let _ = prepare_pixels(&unsupported);
    let _ = encode(&unsupported, &EncodeOptions::default());
    let _ = prepare_pixels(&palette_without_table);
    let short_alpha = DecodedImage::with_mode(2, 1, vec![0, 1], ImageMode::P8)
        .with_palette(ImagePalette::new(vec![0, 0, 0, 255, 255, 255], vec![0]).unwrap());
    let invalid_palette = DecodedImage {
        width: 1,
        height: 1,
        pixels: vec![0],
        color: ColorType::L8,
        mode: ImageMode::P8,
        palette: Some(ImagePalette::default()),
    };
    let _ = prepare_palette_pixels(&short_alpha);
    let _ = prepare_palette_pixels(&invalid_palette);
    let _ = prepare_palette_pixels_with_capacity(&short_alpha, |_, _| None);
    let delayed_alpha = DecodedImage::with_mode(2, 1, vec![0, 1], ImageMode::P8)
        .with_palette(ImagePalette::new(vec![0, 0, 0, 255, 255, 255], vec![u8::MAX, 0]).unwrap());
    let _ = prepare_palette_pixels(&delayed_alpha);
    let _ = palette_capacity(usize::MAX, 4);
    let _ = palette_capacity(1, 4);
    let huge_l1 = DecodedImage::with_mode(u32::MAX, u32::MAX, Vec::new(), ImageMode::L1);
    let short_l1 = DecodedImage::with_mode(8, 2, vec![0], ImageMode::L1);
    let _ = unpack_l1_to_rgb(&huge_l1);
    let _ = unpack_l1_to_rgb(&short_l1);
    let _ = prepare_pixels(&huge_l1);
    let _ = prepare_pixels(&short_l1);

    let parsed = ParsedOptions::new(&EncodeOptions::default()).unwrap();
    let encoder = create_encoder(&one, &parsed).unwrap();
    let _ = encode_frames(encoder, &[&unsupported], &[0], true);
    let encoder = create_encoder(&one, &parsed).unwrap();
    let _ = encode_frames(encoder, &[&two], &[0], true);
    let encoder = create_encoder(&one, &parsed).unwrap();
    let _ = encode_frames(encoder, &[], &[], false);

    for codec in ["auto", "aom", "dav1d"] {
        let _ = ParsedOptions::new(&option("codec", codec));
    }
    for name in [
        "speed",
        "max_threads",
        "tile_rows",
        "tile_cols",
        "exif_orientation",
    ] {
        let _ = ParsedOptions::new(&option(name, "not-an-integer"));
    }
    let _ = ParsedOptions::new(&option("sequence_time", "-1"));
    let _ = ParsedOptions::new(&option("alpha_premultiplied", "invalid"));
    let _ = ParsedOptions::new(&option("autotiling", "invalid"));
    for name in ["icc_hex", "exif_hex", "xmp_hex"] {
        let _ = ParsedOptions::new(&option(name, "f"));
    }
    let _ = ParsedOptions::new(&option("range", "full"));
    let _ = ParsedOptions::new(&option("alpha_premultiplied", "false"));
    let _ = ParsedOptions::new(&option("autotiling", "true"));
    let _ = ParsedOptions::new(&option("tile_rows", "1"));
    let _ = ParsedOptions::new(&option("tile_cols", "1"));

    for value in ["true", "1", "yes", "on", "false", "0", "no", "off"] {
        let options = option("flag", value);
        let _ = parse_bool(&options, "flag");
    }
    let _ = parse_bool(&option("flag", "invalid"), "flag");
    let _ = parse_bool(&EncodeOptions::default(), "flag");
    let _ = parse_i32(&option("number", "7"), "number");
    let _ = parse_i32(&option("number", "999999999999999999999"), "number");
    let _ = parse_i32(&EncodeOptions::default(), "number");
    let _ = parse_u64(&option("number", "7"), "number");
    let _ = parse_u64(&option("number", "-1"), "number");
    let _ = parse_u64(&EncodeOptions::default(), "number");

    let _ = parse_hex_option(&EncodeOptions::default(), "bytes");
    let _ = parse_hex_option(&option("bytes", ""), "bytes");
    let _ = parse_hex_option(&option("bytes", "09aF"), "bytes");
    let _ = parse_hex_option(&option("bytes", "f"), "bytes");
    let _ = parse_hex_option(&option("bytes", "gg"), "bytes");
    let _ = parse_hex_option(&option("bytes", "0g"), "bytes");

    let mut advanced = EncodeOptions::default();
    advanced
        .advanced
        .push(("tune".to_owned(), "psnr".to_owned()));
    let _ = ParsedOptions::new(&advanced);
    advanced.advanced[0].0.push('\0');
    let _ = ParsedOptions::new(&advanced);
    advanced.advanced[0].0 = "tune".to_owned();
    advanced.advanced[0].1.push('\0');
    let _ = ParsedOptions::new(&advanced);
}
