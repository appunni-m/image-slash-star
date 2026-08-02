//! Pure-Rust WebP encoder: internal VP8L lossless and VP8 lossy pipelines.

use crate::codecs::{CodecError, CodecResult};
use crate::encode_options::WebPEncodeOptions;
use crate::types::{
    AnimationBackground, DecodedImage, DecodedSequence, FrameBlend, FrameDisposal,
    FramePixelLayout, ImageMode,
};
use std::borrow::Cow;

pub mod vp8;

/// Encode a DecodedImage to WebP format.
///
/// Lossless uses the internal VP8L encoder.
/// Lossy: uses our own pure-Rust VP8 intra-frame encoder.
pub fn encode(img: &DecodedImage, opts: &WebPEncodeOptions) -> CodecResult<Vec<u8>> {
    encode_with_token(img, opts, None)
}

/// Encode a still WebP while polling an optional cooperative cancellation token.
pub fn encode_with_token(
    img: &DecodedImage,
    opts: &WebPEncodeOptions,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    crate::codecs::error::check_cancelled(token)?;
    validate_options(opts)?;
    let (encoded, alpha) = encode_pixels(img, opts, token)?;
    crate::codecs::error::check_cancelled(token)?;
    attach_metadata(encoded, img.width, img.height, alpha, opts, token)
}

fn encode_pixels(
    img: &DecodedImage,
    opts: &WebPEncodeOptions,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(Vec<u8>, bool)> {
    crate::codecs::error::check_cancelled(token)?;
    let prepared = prepare_pixels(img)?;
    crate::codecs::error::check_cancelled(token)?;
    let encoded = if opts.lossless == Some(true) {
        encode_lossless(&prepared, img.width, img.height)
    } else {
        encode_lossy(&prepared, img.width, img.height, opts)
    }?;
    crate::codecs::error::check_cancelled(token)?;
    let alpha = prepared.has_nonopaque_alpha();
    crate::codecs::error::check_cancelled(token)?;
    Ok((encoded, alpha))
}

/// Encode two or more rendered canvases as full-canvas WebP keyframes.
pub fn encode_sequence(
    sequence: &DecodedSequence,
    opts: &WebPEncodeOptions,
) -> CodecResult<Vec<u8>> {
    encode_sequence_with_token(sequence, opts, None)
}

/// Encode WebP animation keyframes while polling an optional cancellation
/// token at frame and container-assembly boundaries.
pub fn encode_sequence_with_token(
    sequence: &DecodedSequence,
    opts: &WebPEncodeOptions,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    crate::codecs::error::check_cancelled(token)?;
    validate_options(opts)?;
    validate_sequence_options(opts)?;
    let loop_count = match sequence.loop_count {
        Some(value) => u16::try_from(value).map_err(|_| {
            CodecError::Parameter("WebP animation loop count exceeds 16 bits".to_owned())
        })?,
        None => 0,
    };
    let background = match sequence.background {
        Some(AnimationBackground::Rgba(rgba)) => rgba,
        Some(AnimationBackground::PaletteIndex(_)) => {
            return Err(CodecError::Unsupported(
                "WebP animation cannot represent a palette-index background".to_owned(),
            ));
        }
        None => [0; 4],
    };

    let mut encoded_frames = Vec::with_capacity(sequence.frames.len());
    let mut has_alpha = false;
    for frame in &sequence.frames {
        crate::codecs::error::check_cancelled(token)?;
        validate_keyframe(sequence, frame)?;
        let duration = duration_milliseconds(frame.source.duration)?;
        let (encoded, alpha) = encode_pixels(&frame.image, opts, token)?;
        has_alpha |= alpha;
        let chunks = if encoded.get(12..16) == Some(b"VP8X") {
            &encoded[30..]
        } else {
            &encoded[12..]
        };
        encoded_frames.push((duration, chunks.to_vec()));
        crate::codecs::error::check_cancelled(token)?;
    }

    let mut output = Vec::new();
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&[0; 4]);
    output.extend_from_slice(b"WEBP");

    let mut vp8x = vec![0x02 | u8::from(has_alpha).wrapping_shl(4), 0, 0, 0];
    vp8x.extend_from_slice(&sequence.width.wrapping_sub(1).to_le_bytes()[..3]);
    vp8x.extend_from_slice(&sequence.height.wrapping_sub(1).to_le_bytes()[..3]);
    write_chunk(&mut output, b"VP8X", &vp8x);

    let mut animation = vec![background[2], background[1], background[0], background[3]];
    animation.extend_from_slice(&loop_count.to_le_bytes());
    write_chunk(&mut output, b"ANIM", &animation);

    for (duration, chunks) in encoded_frames {
        crate::codecs::error::check_cancelled(token)?;
        let mut payload = vec![0; 6];
        payload.extend_from_slice(&sequence.width.wrapping_sub(1).to_le_bytes()[..3]);
        payload.extend_from_slice(&sequence.height.wrapping_sub(1).to_le_bytes()[..3]);
        payload.extend_from_slice(&duration.to_le_bytes()[..3]);
        payload.push(0x02);
        payload.extend_from_slice(&chunks);
        write_chunk(&mut output, b"ANMF", &payload);
    }

    crate::codecs::error::check_cancelled(token)?;
    let output_len = output.len();
    finish_riff(output, output_len)
}

fn validate_sequence_options(opts: &WebPEncodeOptions) -> CodecResult<()> {
    if opts.icc.is_some() || opts.exif.is_some() || opts.xmp.is_some() {
        return Err(CodecError::Unsupported(
            "WebP sequence metadata output is not implemented".to_owned(),
        ));
    }
    match opts.legacy_kmax() {
        Some(value) if value != 1 => {
            return Err(CodecError::Parameter(
                "WebP sequence encoder currently requires kmax=1".to_owned(),
            ));
        }
        Some(_) | None => {}
    }
    if opts.has_unsupported_legacy_sequence_option() {
        return Err(CodecError::Unsupported(
            "WebP animation optimization options are not implemented".to_owned(),
        ));
    }
    Ok(())
}

fn validate_keyframe(
    sequence: &DecodedSequence,
    frame: &crate::types::DecodedFrame,
) -> CodecResult<()> {
    let rect = frame.source.rect;
    if frame.pixel_layout == FramePixelLayout::SourceRectangle
        && [rect.left, rect.top, rect.width, rect.height] != [0, 0, sequence.width, sequence.height]
    {
        return Err(CodecError::Unsupported(
            "WebP keyframe encoder requires full-canvas frame rectangles".to_owned(),
        ));
    }
    if frame.source.interlaced {
        return Err(CodecError::Unsupported(
            "WebP cannot represent retained interlace or default-image state".to_owned(),
        ));
    }
    if frame.source.is_default_image {
        return Err(CodecError::Unsupported(
            "WebP cannot represent retained interlace or default-image state".to_owned(),
        ));
    }
    if matches!(frame.source.disposal, FrameDisposal::Reserved(_)) {
        return Err(CodecError::Unsupported(
            "WebP keyframe encoder cannot replay reserved presentation controls".to_owned(),
        ));
    }
    if matches!(frame.source.blend, FrameBlend::Reserved(_)) {
        return Err(CodecError::Unsupported(
            "WebP keyframe encoder cannot replay reserved presentation controls".to_owned(),
        ));
    }
    Ok(())
}

fn duration_milliseconds(duration: crate::types::FrameDuration) -> CodecResult<u32> {
    let scaled = u128::from(duration.numerator).saturating_mul(1000);
    let denominator = u128::from(duration.denominator);
    if !scaled.is_multiple_of(denominator) {
        return Err(CodecError::Parameter(
            "WebP frame duration must be an exact number of milliseconds".to_owned(),
        ));
    }
    // The public sequence validator rejects a zero denominator before codec
    // dispatch, so division is safe here.
    let milliseconds = scaled.div_euclid(denominator);
    if milliseconds > 0x00ff_ffff {
        return Err(CodecError::Parameter(
            "WebP frame duration exceeds 24 bits".to_owned(),
        ));
    }
    let bytes = milliseconds.to_le_bytes();
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

struct PreparedPixels<'a> {
    bytes: Cow<'a, [u8]>,
    color: super::native::ColorType,
}

impl PreparedPixels<'_> {
    fn has_nonopaque_alpha(&self) -> bool {
        self.color == super::native::ColorType::Rgba8
            && self.bytes.chunks_exact(4).any(|pixel| pixel[3] != u8::MAX)
    }
}

fn validate_options(opts: &WebPEncodeOptions) -> CodecResult<()> {
    if opts.quality.is_some_and(|quality| quality > 100) {
        return Err(CodecError::Parameter(
            "WebP quality must be between 0 and 100".to_owned(),
        ));
    }
    if opts.method.is_some_and(|method| method > 6) {
        return Err(CodecError::Parameter(
            "WebP method must be between 0 and 6".to_owned(),
        ));
    }
    Ok(())
}

fn prepare_pixels(img: &DecodedImage) -> CodecResult<PreparedPixels<'_>> {
    let prepared = match img.mode {
        ImageMode::L1 => PreparedPixels {
            bytes: Cow::Owned(expand_bilevel_to_rgb(img)),
            color: super::native::ColorType::Rgb8,
        },
        ImageMode::P8 => expand_indexed(img),
        ImageMode::L8 => PreparedPixels {
            bytes: Cow::Owned(img.pixels.iter().flat_map(|&value| [value; 3]).collect()),
            color: super::native::ColorType::Rgb8,
        },
        ImageMode::La8 => expand_luminance_alpha(img),
        ImageMode::Rgb8 => PreparedPixels {
            bytes: Cow::Borrowed(&img.pixels),
            color: super::native::ColorType::Rgb8,
        },
        ImageMode::Rgba8 => PreparedPixels {
            bytes: Cow::Borrowed(&img.pixels),
            color: super::native::ColorType::Rgba8,
        },
        ImageMode::Cmyk8 => PreparedPixels {
            bytes: Cow::Owned(cmyk_to_rgb(&img.pixels)),
            color: super::native::ColorType::Rgb8,
        },
        _ => {
            return Err(CodecError::Unsupported(
                "WebP encoder does not support this image mode".to_owned(),
            ));
        }
    };
    Ok(prepared)
}

fn expand_bilevel_to_rgb(img: &DecodedImage) -> Vec<u8> {
    let width = img.width as usize;
    let row_bytes = width.div_ceil(8);
    let mut rgb = Vec::with_capacity(width.saturating_mul(img.height as usize).saturating_mul(3));
    for row in img.pixels.chunks_exact(row_bytes) {
        for x in 0..width {
            let bit = (row[x / 8] >> 7usize.wrapping_sub(x % 8)) & 1;
            rgb.extend_from_slice(&[0u8.wrapping_sub(bit); 3]);
        }
    }
    rgb
}

fn expand_indexed(img: &DecodedImage) -> PreparedPixels<'static> {
    let mut rgba = Vec::with_capacity(img.pixels.len().saturating_mul(4));
    let mut has_alpha = false;
    for &index in &img.pixels {
        let index = usize::from(index);
        let rgb_start = index.saturating_mul(3);
        let color = img
            .palette
            .as_ref()
            .and_then(|palette| palette.rgb.get(rgb_start..rgb_start.saturating_add(3)))
            .unwrap_or(&[0, 0, 0]);
        let alpha = img
            .palette
            .as_ref()
            .and_then(|palette| palette.alpha.get(index))
            .copied()
            .unwrap_or(u8::MAX);
        has_alpha |= alpha != u8::MAX;
        rgba.extend_from_slice(color);
        rgba.push(alpha);
    }
    if has_alpha {
        PreparedPixels {
            bytes: Cow::Owned(rgba),
            color: super::native::ColorType::Rgba8,
        }
    } else {
        PreparedPixels {
            bytes: Cow::Owned(
                rgba.chunks_exact(4)
                    .flat_map(|pixel| pixel[..3].iter().copied())
                    .collect(),
            ),
            color: super::native::ColorType::Rgb8,
        }
    }
}

fn expand_luminance_alpha(img: &DecodedImage) -> PreparedPixels<'static> {
    let has_alpha = img.pixels.chunks_exact(2).any(|pixel| pixel[1] != u8::MAX);
    if has_alpha {
        PreparedPixels {
            bytes: Cow::Owned(
                img.pixels
                    .chunks_exact(2)
                    .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
                    .collect(),
            ),
            color: super::native::ColorType::Rgba8,
        }
    } else {
        PreparedPixels {
            bytes: Cow::Owned(
                img.pixels
                    .chunks_exact(2)
                    .flat_map(|pixel| [pixel[0]; 3])
                    .collect(),
            ),
            color: super::native::ColorType::Rgb8,
        }
    }
}

fn write_chunk(output: &mut Vec<u8>, name: &[u8; 4], payload: &[u8]) {
    output.extend_from_slice(name);
    output.extend_from_slice(&low_u32(payload.len()).to_le_bytes());
    output.extend_from_slice(payload);
    if !payload.len().is_multiple_of(2) {
        output.push(0);
    }
}

fn finish_riff(mut output: Vec<u8>, output_len: usize) -> CodecResult<Vec<u8>> {
    let riff_size = u32::try_from(output_len.saturating_sub(8)).map_err(|error| {
        CodecError::Dimensions(format!("WebP RIFF output exceeds format limits: {error}"))
    })?;
    output[4..8].copy_from_slice(&riff_size.to_le_bytes());
    Ok(output)
}

fn attach_metadata(
    encoded: Vec<u8>,
    width: u32,
    height: u32,
    alpha: bool,
    opts: &WebPEncodeOptions,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    crate::codecs::error::check_cancelled(token)?;
    let icc = opts.icc.as_deref();
    let exif = opts.exif.as_deref();
    let xmp = opts.xmp.as_deref();
    if icc.is_none() && exif.is_none() && xmp.is_none() {
        crate::codecs::error::check_cancelled(token)?;
        return Ok(encoded);
    }
    // Both internal encoders return a complete RIFF header. Lossy alpha output
    // additionally begins with the fixed 18-byte VP8X chunk constructed by
    // `encode_vp8_lossy_rgba`; replace that chunk instead of reparsing bytes
    // emitted by the same implementation.
    let encoded_chunks = &encoded[12..];
    let encoded_chunks = if encoded_chunks.starts_with(b"VP8X") {
        debug_assert!(encoded_chunks.len() >= 18);
        &encoded_chunks[18..]
    } else {
        encoded_chunks
    };
    let mut flags = u8::from(alpha).wrapping_shl(4);
    if icc.is_some() {
        flags |= 1u8.wrapping_shl(5);
    }
    if exif.is_some() {
        flags |= 1u8.wrapping_shl(3);
    }
    if xmp.is_some() {
        flags |= 1u8.wrapping_shl(2);
    }
    let mut output = Vec::new();
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&[0; 4]);
    output.extend_from_slice(b"WEBP");
    crate::codecs::error::check_cancelled(token)?;
    let mut vp8x = vec![flags, 0, 0, 0];
    vp8x.extend_from_slice(&width.saturating_sub(1).to_le_bytes()[..3]);
    vp8x.extend_from_slice(&height.saturating_sub(1).to_le_bytes()[..3]);
    write_chunk(&mut output, b"VP8X", &vp8x);
    crate::codecs::error::check_cancelled(token)?;
    if let Some(payload) = icc {
        write_chunk(&mut output, b"ICCP", payload);
        crate::codecs::error::check_cancelled(token)?;
    }
    output.extend_from_slice(encoded_chunks);
    if let Some(payload) = exif {
        let payload = payload.strip_prefix(b"Exif\0\0").unwrap_or(payload);
        write_chunk(&mut output, b"EXIF", payload);
        crate::codecs::error::check_cancelled(token)?;
    }
    if let Some(payload) = xmp {
        write_chunk(&mut output, b"XMP ", payload);
        crate::codecs::error::check_cancelled(token)?;
    }
    #[cfg(coverage)]
    let output_len = if opts.force_riff_size_overflow() {
        usize::MAX
    } else {
        output.len()
    };
    #[cfg(not(coverage))]
    let output_len = output.len();
    crate::codecs::error::check_cancelled(token)?;
    finish_riff(output, output_len)
}

/// Lossless VP8L encoding via the internal `WebPEncoder`.
fn encode_lossless(pixels: &PreparedPixels<'_>, width: u32, height: u32) -> CodecResult<Vec<u8>> {
    super::native::WebPEncoder::new()
        .encode(&pixels.bytes, width, height, pixels.color)
        .map_err(encode_error)
}

/// Lossy VP8 encoding — own pure-Rust implementation.
///
/// Encodes VP8 keyframe bitstream in RIFF/WEBP container.
fn encode_lossy(
    pixels: &PreparedPixels<'_>,
    width: u32,
    height: u32,
    opts: &WebPEncodeOptions,
) -> CodecResult<Vec<u8>> {
    let quality = opts.quality.unwrap_or(80);
    let method = opts.method.unwrap_or(4);
    let encoded = match pixels.color {
        super::native::ColorType::Rgb8 => {
            vp8::encoder::encode_vp8_lossy(&pixels.bytes, width, height, quality, method)
        }
        super::native::ColorType::Rgba8 => {
            let has_alpha = pixels.has_nonopaque_alpha();
            if has_alpha {
                let alpha = pixels
                    .bytes
                    .chunks_exact(4)
                    .map(|pixel| pixel[3])
                    .collect::<Vec<_>>();
                let alpha_chunk = super::native::encode_alpha(&alpha, width, height);
                vp8::encoder::encode_vp8_lossy_rgba(
                    &pixels.bytes,
                    width,
                    height,
                    quality,
                    method,
                    &alpha_chunk,
                )
            } else {
                let rgb = pixels
                    .bytes
                    .chunks_exact(4)
                    .flat_map(|pixel| pixel[..3].iter().copied())
                    .collect::<Vec<_>>();
                vp8::encoder::encode_vp8_lossy(&rgb, width, height, quality, method)
            }
        }
    };
    Ok(encoded)
}

fn encode_error(error: super::native::EncodingError) -> CodecError {
    match error {
        super::native::EncodingError::InvalidDimensions => {
            CodecError::Dimensions("WebP lossless dimensions are invalid".to_owned())
        }
    }
}

fn cmyk_to_rgb(pixels: &[u8]) -> Vec<u8> {
    pixels
        .chunks_exact(4)
        .flat_map(|pixel| {
            let black = u16::from(255u8.saturating_sub(pixel[3]));
            std::array::from_fn::<_, 3, _>(|channel| {
                let ink = u16::from(255u8.saturating_sub(pixel[channel]));
                ink.saturating_mul(black)
                    .saturating_add(127)
                    .div_euclid(255)
                    .to_le_bytes()[0]
            })
        })
        .collect()
}

fn low_u32(value: usize) -> u32 {
    #[cfg(target_pointer_width = "64")]
    {
        let [a, b, c, d, ..] = value.to_le_bytes();
        u32::from_le_bytes([a, b, c, d])
    }
    #[cfg(target_pointer_width = "32")]
    {
        u32::from_le_bytes(value.to_le_bytes())
    }
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    vp8::__coverage_exercise_private_branches();

    let mut opts = WebPEncodeOptions::default();
    opts.icc = Some(vec![0]);
    opts.set_force_riff_size_overflow();
    let _ = attach_metadata(b"RIFF\0\0\0\0WEBP".to_vec(), 1, 1, false, &opts, None);

    let zero_width = DecodedImage::new(0, 1, Vec::new(), crate::types::ColorType::Rgb8);
    let mut opts = WebPEncodeOptions::default();
    opts.lossless = Some(true);
    let _ = encode(&zero_width, &opts);

    let unsupported = DecodedImage::new(1, 1, vec![0; 8], crate::types::ColorType::Rgb32F);
    let _ = encode(&unsupported, &opts);
    let _ = encode(&unsupported, &WebPEncodeOptions::default());

    // Pillow has no caller-controlled token. Keep these checkpoint exercises
    // in the Rust-only private coverage hook rather than the parity matrix.
    let mut sequence = DecodedSequence::from_image(DecodedImage::new(
        1,
        1,
        vec![0, 0, 0],
        crate::types::ColorType::Rgb8,
    ));
    sequence.frames.push(sequence.frames[0].clone());
    sequence.kind = crate::types::SequenceKind::TimedAnimation;
    for checks in [0, 1, 2, 5, 7] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = encode_sequence_with_token(&sequence, &WebPEncodeOptions::default(), Some(&token));
    }
}
