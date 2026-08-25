//! Pure-Rust WebP encoder: internal VP8L lossless and VP8 lossy pipelines.

use crate::codecs::{CodecError, CodecResult};
use crate::encode_options::WebPEncodeOptions;
use crate::encode_policy::EncodePolicy;
use crate::types::{
    AnimationBackground, DecodedImage, DecodedSequence, FrameBlend, FrameDisposal,
    FramePixelLayout, ImageMode,
};
use crate::{CodecOperation, ImageFormat, OutputSink};
use std::borrow::Cow;
#[cfg(coverage)]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub mod vp8;

const PREPARE_CHECKPOINT_PIXELS: usize = 1_024;
const OUTPUT_COPY_CHECKPOINT_BYTES: usize = 1_024;

#[cfg(coverage)]
static FORCE_RIFF_SIZE_ERROR: AtomicBool = AtomicBool::new(false);
#[cfg(coverage)]
static FORCE_PAYLOAD_LEN_ERROR: AtomicBool = AtomicBool::new(false);
#[cfg(coverage)]
static FORCE_CHUNK_PAYLOAD_CALL: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static FORCE_SINK_OUTPUT_END_ERROR: AtomicBool = AtomicBool::new(false);
#[cfg(coverage)]
static FORCE_EXISTING_END_ERROR: AtomicBool = AtomicBool::new(false);
#[cfg(coverage)]
static FORCE_METADATA_START_ERROR: AtomicBool = AtomicBool::new(false);
#[cfg(coverage)]
static FORCE_ALPHA_SCAN_ERROR: AtomicBool = AtomicBool::new(false);
#[cfg(coverage)]
static FORCE_RGB_EXTRACTION_ERROR: AtomicBool = AtomicBool::new(false);
#[cfg(coverage)]
static FORCE_WRITE_CHUNK_IN_PLACE_CALL: AtomicUsize = AtomicUsize::new(usize::MAX);

#[cfg(coverage)]
#[coverage(off)]
fn coverage_should_fail_chunk_payload_call() -> bool {
    let remaining = FORCE_CHUNK_PAYLOAD_CALL.load(Ordering::Relaxed);
    if remaining == usize::MAX {
        return false;
    }
    if remaining == 0 {
        FORCE_CHUNK_PAYLOAD_CALL.store(usize::MAX, Ordering::Relaxed);
        true
    } else {
        FORCE_CHUNK_PAYLOAD_CALL.store(remaining.saturating_sub(1), Ordering::Relaxed);
        false
    }
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_should_fail_write_chunk_in_place() -> bool {
    let remaining = FORCE_WRITE_CHUNK_IN_PLACE_CALL.load(Ordering::Relaxed);
    if remaining == usize::MAX {
        return false;
    }
    if remaining == 0 {
        FORCE_WRITE_CHUNK_IN_PLACE_CALL.store(usize::MAX, Ordering::Relaxed);
        true
    } else {
        FORCE_WRITE_CHUNK_IN_PLACE_CALL.store(remaining.saturating_sub(1), Ordering::Relaxed);
        false
    }
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_should_fail_alpha_scan() -> bool {
    FORCE_ALPHA_SCAN_ERROR.swap(false, Ordering::Relaxed)
}

fn extend_with_output_checkpoint(
    output: &mut Vec<u8>,
    source: &[u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    let Some(token) = token else {
        output.extend_from_slice(source);
        return Ok(());
    };
    for chunk in source.chunks(OUTPUT_COPY_CHECKPOINT_BYTES) {
        output.extend_from_slice(chunk);
        if chunk.len() == OUTPUT_COPY_CHECKPOINT_BYTES {
            crate::codecs::error::check_cancelled(Some(token))?;
        }
    }
    Ok(())
}

fn checkpoint_after_prepare_pixel(
    pixels_until_checkpoint: &mut usize,
    token: &crate::CancellationToken,
) -> CodecResult<()> {
    *pixels_until_checkpoint = pixels_until_checkpoint.saturating_sub(1);
    if *pixels_until_checkpoint == 0 {
        crate::codecs::error::check_cancelled(Some(token))?;
        *pixels_until_checkpoint = PREPARE_CHECKPOINT_PIXELS;
    }
    Ok(())
}

/// Encode a DecodedImage to WebP format.
///
/// Lossless uses the internal VP8L encoder, which polls WebP source-mode
/// preparation, RGBA alpha/RGB extraction, RGB/RGBA source-pixel materialization,
/// image-palette construction, and palette-mode index packing after each 1,024
/// source pixels, plus transparent-pixel hidden-RGB cleanup after each 1,024
/// scanned pixels. Predictor mode application polls its pre-transform source
/// snapshot copy after each 1,024 source pixels and copies each wide source row
/// in completed 1,024-pixel chunks when a caller supplies a cancellation token;
/// the no-token path retains its original bulk row copy. The token-aware
/// lossless backward-reference cost manager
/// also initializes its pixel-sized cost/length tables in 1,024-entry intervals;
/// its capacity reservations retain the existing no-recoverable-OOM policy.
/// Token-aware lossless VP8L hash-chain candidate selection polls after each
/// 64 completed candidate trials across the pass; the no-token candidate loop
/// remains a separate tight path.
/// Token-aware palette-mode VP8L box-chain selection polls after each 64
/// completed low-distance candidate offsets across the pass; the no-token box
/// chain retains its original tight path.
/// Long backward-reference result backfills also poll after each 256 entries.
/// Token-aware lossless VP8L assembly copies the complete RIFF frame payload
/// after each 1,024 bytes; the no-token path retains one bulk copy.
/// Token-aware VP8L candidate-trial selection also copies the winning suffix
/// after each 1,024 bytes; the no-token path retains one bulk suffix copy.
/// Token-aware entropy-mode analysis polls its pixel histogram after each
/// completed 1,024-pixel chunk on rows wider than 1,024 pixels; narrower rows
/// remain bounded by their existing row-start polls, and the no-token pass is
/// a direct loop without token scheduling.
/// The no-token path retains its tight source materialization maps.
/// Lossy: uses our own pure-Rust VP8 intra-frame encoder. Token-aware lossy
/// VP8 encoding polls required padded Y/U/V edge-replication items after each
/// 1,024 items, analysis and segment-assignment macroblocks after each 1,024
/// items, intra4 mode selection after each completed luma 4×4 block and its
/// outer 64-macroblock batch for intra16/chroma work, filter-edge adjustment,
/// coefficient-statistics
/// collection, and the
/// first-partition segment-probability prepass after each 1,024 selected
/// macroblocks, transparent-area cleanup after each 1,024 scanned or flattened
/// pixels, and alpha-palette source collection and index packing after each
/// 1,024 source pixels, plus lossy VP8/ALPH RIFF payload copies, compressed/raw
/// alpha-stream buffer copies, lossless VP8L candidate-trial suffix copies,
/// and RIFF frame payload copies after each 1,024 output bytes, when a caller
/// supplies a cancellation token.
/// Aligned
/// planes are cloned directly because no edge replication is needed. The
/// no-token helpers retain their original tight paths; token-aware selection
/// checks after candidate-trial stages, each forward- and inverse-transform
/// row/column subpass, each non-trellis quantization coefficient, method-6
/// trellis-quantization coefficient candidates and path-reconstruction nodes,
/// each squared-error pixel, each spectral-distortion weighted-transform
/// row/column pass, each residual-cost coefficient, and each candidate. Each
/// other individual stage remains one uninterruptible unit.
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
    let mut lossless_encoder = super::native::WebPEncoder::new();
    let (encoded, alpha) = encode_pixels(img, opts, &mut lossless_encoder, token)?;
    crate::codecs::error::check_cancelled(token)?;
    attach_metadata(encoded, img.width, img.height, alpha, opts, token)
}

/// Encode a still WebP into validated RIFF segments owned by the caller's
/// sink. The codec still retains its complete working buffer; this boundary
/// makes container delivery and cancellation observable without claiming
/// interior VP8/VP8L streaming.
pub(crate) fn encode_to_sink(
    img: &DecodedImage,
    opts: &WebPEncodeOptions,
    policy: EncodePolicy,
    operation: CodecOperation,
    token: Option<&crate::CancellationToken>,
    sink: &mut dyn OutputSink,
) -> CodecResult<usize> {
    let encoded = encode_with_token(img, opts, token)?;
    policy
        .check_output_len(encoded.len(), ImageFormat::WebP, operation)
        .map_err(CodecError::from_image_error)?;
    write_riff_to_sink(&encoded, token, sink)
}

/// Encode a WebP animation into validated RIFF segments owned by the
/// caller's sink. The codec retains its complete animation working buffer;
/// this boundary makes container delivery and cancellation observable without
/// claiming interior VP8/VP8L streaming.
pub(crate) fn encode_sequence_to_sink(
    sequence: &DecodedSequence,
    opts: &WebPEncodeOptions,
    policy: EncodePolicy,
    operation: CodecOperation,
    token: Option<&crate::CancellationToken>,
    sink: &mut dyn OutputSink,
) -> CodecResult<usize> {
    let encoded = encode_sequence_with_token(sequence, opts, token)?;
    policy
        .check_output_len(encoded.len(), ImageFormat::WebP, operation)
        .map_err(CodecError::from_image_error)?;
    write_riff_to_sink(&encoded, token, sink)
}

fn write_riff_to_sink(
    encoded: &[u8],
    token: Option<&crate::CancellationToken>,
    sink: &mut dyn OutputSink,
) -> CodecResult<usize> {
    if encoded.len() < 12 {
        return Err(CodecError::Malformed(
            "WebP encoder produced an invalid RIFF header".to_owned(),
        ));
    }
    if encoded.get(..4) != Some(b"RIFF") {
        return Err(CodecError::Malformed(
            "WebP encoder produced an invalid RIFF header".to_owned(),
        ));
    }
    if encoded.get(8..12) != Some(b"WEBP") {
        return Err(CodecError::Malformed(
            "WebP encoder produced an invalid RIFF header".to_owned(),
        ));
    }
    let declared_size = u32::from_le_bytes([encoded[4], encoded[5], encoded[6], encoded[7]]);
    let expected_size = webp_riff_size_from_len(encoded.len())?;
    if declared_size != expected_size {
        return Err(CodecError::Malformed(
            "WebP encoder produced an inconsistent RIFF size".to_owned(),
        ));
    }

    let mut written = 0usize;
    write_sink_segment(sink, &encoded[..12], token, &mut written)?;
    let mut offset = 12usize;
    while offset < encoded.len() {
        let header_end = webp_chunk_payload_len(offset, 8)?;
        let header = encoded.get(offset..header_end).ok_or_else(|| {
            CodecError::Malformed("WebP chunk header extends beyond the RIFF output".to_owned())
        })?;
        let payload_len = webp_payload_len_from_u32(u32::from_le_bytes([
            header[4], header[5], header[6], header[7],
        ]))?;
        let payload_end = webp_chunk_payload_len(header_end, payload_len)?;
        let chunk_end =
            webp_chunk_payload_len(payload_end, usize::from(u8::from(payload_len % 2 != 0)))?;
        let payload = encoded.get(header_end..chunk_end).ok_or_else(|| {
            CodecError::Malformed("WebP chunk payload extends beyond the RIFF output".to_owned())
        })?;
        write_sink_segment(sink, header, token, &mut written)?;
        if !payload.is_empty() {
            write_sink_segment(sink, payload, token, &mut written)?;
        }
        offset = chunk_end;
    }
    Ok(written)
}

fn write_sink_segment(
    sink: &mut dyn OutputSink,
    bytes: &[u8],
    token: Option<&crate::CancellationToken>,
    written: &mut usize,
) -> CodecResult<()> {
    crate::codecs::error::check_cancelled(token)?;
    sink.write_all(bytes)
        .map_err(|error| CodecError::OutputWrite(error.to_string()))?;
    *written = webp_sink_output_end(*written, bytes.len())?;
    Ok(())
}

fn encode_pixels(
    img: &DecodedImage,
    opts: &WebPEncodeOptions,
    lossless_encoder: &mut super::native::WebPEncoder,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(Vec<u8>, bool)> {
    crate::codecs::error::check_cancelled(token)?;
    let prepared = prepare_pixels(img, token)?;
    crate::codecs::error::check_cancelled(token)?;
    let encoded = if opts.lossless == Some(true) {
        encode_lossless(&prepared, img.width, img.height, lossless_encoder, token)
    } else {
        encode_lossy(
            &prepared,
            img.width,
            img.height,
            opts,
            lossless_encoder,
            token,
        )
    }?;
    crate::codecs::error::check_cancelled(token)?;
    let alpha = prepared.has_nonopaque_alpha_with_token(token)?;
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
/// token at frame boundaries and each 1,024-byte container-assembly copy
/// interval.
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

    if token.is_none() {
        return encode_sequence_without_token(sequence, opts, loop_count, background);
    }

    let mut encoded_frames = Vec::with_capacity(sequence.frames.len());
    let mut lossless_encoder = super::native::WebPEncoder::new();
    let mut has_alpha = false;
    for frame in &sequence.frames {
        crate::codecs::error::check_cancelled(token)?;
        validate_keyframe(sequence, frame)?;
        let duration = duration_milliseconds(frame.source.duration)?;
        let (encoded, alpha) = encode_pixels(&frame.image, opts, &mut lossless_encoder, token)?;
        has_alpha |= alpha;
        encoded_frames.push((duration, encoded));
        crate::codecs::error::check_cancelled(token)?;
    }

    let mut output = Vec::new();
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&[0; 4]);
    output.extend_from_slice(b"WEBP");

    let mut vp8x = vec![0x02 | u8::from(has_alpha).wrapping_shl(4), 0, 0, 0];
    vp8x.extend_from_slice(&sequence.width.wrapping_sub(1).to_le_bytes()[..3]);
    vp8x.extend_from_slice(&sequence.height.wrapping_sub(1).to_le_bytes()[..3]);
    write_chunk(&mut output, b"VP8X", &vp8x, token)?;

    let mut animation = vec![background[2], background[1], background[0], background[3]];
    animation.extend_from_slice(&loop_count.to_le_bytes());
    write_chunk(&mut output, b"ANIM", &animation, token)?;

    for (duration, encoded) in encoded_frames {
        crate::codecs::error::check_cancelled(token)?;
        let chunks = if encoded.get(12..16) == Some(b"VP8X") {
            &encoded[30..]
        } else {
            &encoded[12..]
        };
        let mut frame_header = [0u8; 16];
        frame_header[6..9].copy_from_slice(&sequence.width.wrapping_sub(1).to_le_bytes()[..3]);
        frame_header[9..12].copy_from_slice(&sequence.height.wrapping_sub(1).to_le_bytes()[..3]);
        frame_header[12..15].copy_from_slice(&duration.to_le_bytes()[..3]);
        frame_header[15] = 0x02;
        write_chunk_with_prefix(&mut output, b"ANMF", &frame_header, chunks, token)?;
    }

    crate::codecs::error::check_cancelled(token)?;
    let output_len = output.len();
    finish_riff(output, output_len)
}

fn encode_sequence_without_token(
    sequence: &DecodedSequence,
    opts: &WebPEncodeOptions,
    loop_count: u16,
    background: [u8; 4],
) -> CodecResult<Vec<u8>> {
    // The no-token path can transfer each completed frame into the final RIFF
    // buffer immediately. The VP8X alpha flag is patched after the last frame,
    // so no completed frame buffer needs to remain live just to assemble the
    // container. This is an ownership optimization, not a streaming boundary:
    // the complete encoded animation remains owned by the returned Vec.
    let mut output = Vec::new();
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&[0; 4]);
    output.extend_from_slice(b"WEBP");

    let mut vp8x = vec![0x02, 0, 0, 0];
    vp8x.extend_from_slice(&sequence.width.wrapping_sub(1).to_le_bytes()[..3]);
    vp8x.extend_from_slice(&sequence.height.wrapping_sub(1).to_le_bytes()[..3]);
    write_chunk(&mut output, b"VP8X", &vp8x, None)?;

    let mut animation = vec![background[2], background[1], background[0], background[3]];
    animation.extend_from_slice(&loop_count.to_le_bytes());
    write_chunk(&mut output, b"ANIM", &animation, None)?;

    let mut lossless_encoder = super::native::WebPEncoder::new();
    let mut has_alpha = false;
    for frame in &sequence.frames {
        validate_keyframe(sequence, frame)?;
        let duration = duration_milliseconds(frame.source.duration)?;
        let (encoded, alpha) = encode_pixels(&frame.image, opts, &mut lossless_encoder, None)?;
        has_alpha |= alpha;

        let chunks = if encoded.get(12..16) == Some(b"VP8X") {
            &encoded[30..]
        } else {
            &encoded[12..]
        };
        let mut frame_header = [0u8; 16];
        frame_header[6..9].copy_from_slice(&sequence.width.wrapping_sub(1).to_le_bytes()[..3]);
        frame_header[9..12].copy_from_slice(&sequence.height.wrapping_sub(1).to_le_bytes()[..3]);
        frame_header[12..15].copy_from_slice(&duration.to_le_bytes()[..3]);
        frame_header[15] = 0x02;
        write_chunk_with_prefix(&mut output, b"ANMF", &frame_header, chunks, None)?;
    }

    // RIFF/VP8X/size are fixed at the start of the container, and the VP8X
    // payload's first byte is the animation/alpha flag byte.
    output[20] = 0x02 | u8::from(has_alpha).wrapping_shl(4);
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
    fn has_nonopaque_alpha_with_token(
        &self,
        token: Option<&crate::CancellationToken>,
    ) -> CodecResult<bool> {
        #[cfg(coverage)]
        if coverage_should_fail_alpha_scan() {
            return Err(CodecError::Cancelled);
        }
        if self.color != super::native::ColorType::Rgba8 {
            return Ok(false);
        }
        if let Some(token) = token {
            let mut pixels_until_checkpoint = PREPARE_CHECKPOINT_PIXELS;
            for pixel in self.bytes.as_chunks::<4>().0 {
                let has_alpha = pixel[3] != u8::MAX;
                checkpoint_after_prepare_pixel(&mut pixels_until_checkpoint, token)?;
                if has_alpha {
                    return Ok(true);
                }
            }
            Ok(false)
        } else {
            Ok(self
                .bytes
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] != u8::MAX))
        }
    }

    fn rgb_without_alpha_with_token(
        &self,
        token: Option<&crate::CancellationToken>,
    ) -> CodecResult<Vec<u8>> {
        #[cfg(coverage)]
        if FORCE_RGB_EXTRACTION_ERROR.swap(false, Ordering::Relaxed) {
            return Err(CodecError::Cancelled);
        }
        if let Some(token) = token {
            let mut rgb = Vec::with_capacity(self.bytes.len().saturating_div(4).saturating_mul(3));
            let mut pixels_until_checkpoint = PREPARE_CHECKPOINT_PIXELS;
            for pixel in self.bytes.as_chunks::<4>().0 {
                rgb.extend_from_slice(&pixel[..3]);
                checkpoint_after_prepare_pixel(&mut pixels_until_checkpoint, token)?;
            }
            Ok(rgb)
        } else {
            Ok(self
                .bytes
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|pixel| pixel[..3].iter().copied())
                .collect())
        }
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

fn prepare_pixels<'a>(
    img: &'a DecodedImage,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<PreparedPixels<'a>> {
    match img.mode {
        ImageMode::L1 => Ok(PreparedPixels {
            bytes: Cow::Owned(expand_bilevel_to_rgb(img, token)?),
            color: super::native::ColorType::Rgb8,
        }),
        ImageMode::P8 => expand_indexed(img, token),
        ImageMode::L8 => {
            let bytes = if let Some(token) = token {
                let mut rgb = Vec::with_capacity(img.pixels.len().saturating_mul(3));
                let mut pixels_until_checkpoint = PREPARE_CHECKPOINT_PIXELS;
                for &value in &img.pixels {
                    rgb.extend_from_slice(&[value; 3]);
                    checkpoint_after_prepare_pixel(&mut pixels_until_checkpoint, token)?;
                }
                rgb
            } else {
                img.pixels.iter().flat_map(|&value| [value; 3]).collect()
            };
            Ok(PreparedPixels {
                bytes: Cow::Owned(bytes),
                color: super::native::ColorType::Rgb8,
            })
        }
        ImageMode::L16 => expand_l16_to_rgb(img, token),
        ImageMode::La8 => expand_luminance_alpha(img, token),
        ImageMode::Rgb8 => Ok(PreparedPixels {
            bytes: Cow::Borrowed(&img.pixels),
            color: super::native::ColorType::Rgb8,
        }),
        ImageMode::Rgba8 => Ok(PreparedPixels {
            bytes: Cow::Borrowed(&img.pixels),
            color: super::native::ColorType::Rgba8,
        }),
        ImageMode::Cmyk8 => Ok(PreparedPixels {
            bytes: Cow::Owned(cmyk_to_rgb(&img.pixels, token)?),
            color: super::native::ColorType::Rgb8,
        }),
        _ => Err(CodecError::Unsupported(
            "WebP encoder does not support this image mode".to_owned(),
        )),
    }
}

fn expand_bilevel_to_rgb(
    img: &DecodedImage,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    let width = img.width as usize;
    let row_bytes = width.div_ceil(8);
    if let Some(token) = token {
        let mut rgb =
            Vec::with_capacity(width.saturating_mul(img.height as usize).saturating_mul(3));
        let mut pixels_until_checkpoint = PREPARE_CHECKPOINT_PIXELS;
        for row in img.pixels.chunks_exact(row_bytes) {
            for x in 0..width {
                let bit = (row[x / 8] >> 7usize.wrapping_sub(x % 8)) & 1;
                rgb.extend_from_slice(&[0u8.wrapping_sub(bit); 3]);
                checkpoint_after_prepare_pixel(&mut pixels_until_checkpoint, token)?;
            }
        }
        return Ok(rgb);
    }
    let mut rgb = Vec::with_capacity(width.saturating_mul(img.height as usize).saturating_mul(3));
    for row in img.pixels.chunks_exact(row_bytes) {
        for x in 0..width {
            let bit = (row[x / 8] >> 7usize.wrapping_sub(x % 8)) & 1;
            rgb.extend_from_slice(&[0u8.wrapping_sub(bit); 3]);
        }
    }
    Ok(rgb)
}

fn expand_indexed(
    img: &DecodedImage,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<PreparedPixels<'static>> {
    if let Some(token) = token {
        let mut rgba = Vec::with_capacity(img.pixels.len().saturating_mul(4));
        let mut has_alpha = false;
        let mut pixels_until_checkpoint = PREPARE_CHECKPOINT_PIXELS;
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
            checkpoint_after_prepare_pixel(&mut pixels_until_checkpoint, token)?;
        }
        if has_alpha {
            return Ok(PreparedPixels {
                bytes: Cow::Owned(rgba),
                color: super::native::ColorType::Rgba8,
            });
        }
        let mut rgb = Vec::with_capacity(rgba.len().saturating_div(4).saturating_mul(3));
        pixels_until_checkpoint = PREPARE_CHECKPOINT_PIXELS;
        for pixel in rgba.as_chunks::<4>().0 {
            rgb.extend_from_slice(&pixel[..3]);
            checkpoint_after_prepare_pixel(&mut pixels_until_checkpoint, token)?;
        }
        return Ok(PreparedPixels {
            bytes: Cow::Owned(rgb),
            color: super::native::ColorType::Rgb8,
        });
    }

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
        Ok(PreparedPixels {
            bytes: Cow::Owned(rgba),
            color: super::native::ColorType::Rgba8,
        })
    } else {
        Ok(PreparedPixels {
            bytes: Cow::Owned(
                rgba.as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|pixel| pixel[..3].iter().copied())
                    .collect(),
            ),
            color: super::native::ColorType::Rgb8,
        })
    }
}

fn expand_luminance_alpha(
    img: &DecodedImage,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<PreparedPixels<'static>> {
    if let Some(token) = token {
        let mut has_alpha = false;
        let mut pixels_until_checkpoint = PREPARE_CHECKPOINT_PIXELS;
        for pixel in img.pixels.as_chunks::<2>().0 {
            has_alpha |= pixel[1] != u8::MAX;
            checkpoint_after_prepare_pixel(&mut pixels_until_checkpoint, token)?;
        }
        if has_alpha {
            let mut rgba = Vec::with_capacity(img.pixels.len().saturating_div(2).saturating_mul(4));
            pixels_until_checkpoint = PREPARE_CHECKPOINT_PIXELS;
            for pixel in img.pixels.as_chunks::<2>().0 {
                rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
                checkpoint_after_prepare_pixel(&mut pixels_until_checkpoint, token)?;
            }
            return Ok(PreparedPixels {
                bytes: Cow::Owned(rgba),
                color: super::native::ColorType::Rgba8,
            });
        }
        let mut rgb = Vec::with_capacity(img.pixels.len().saturating_div(2).saturating_mul(3));
        pixels_until_checkpoint = PREPARE_CHECKPOINT_PIXELS;
        for pixel in img.pixels.as_chunks::<2>().0 {
            rgb.extend_from_slice(&[pixel[0]; 3]);
            checkpoint_after_prepare_pixel(&mut pixels_until_checkpoint, token)?;
        }
        return Ok(PreparedPixels {
            bytes: Cow::Owned(rgb),
            color: super::native::ColorType::Rgb8,
        });
    }

    let has_alpha = img
        .pixels
        .as_chunks::<2>()
        .0
        .iter()
        .any(|pixel| pixel[1] != u8::MAX);
    if has_alpha {
        Ok(PreparedPixels {
            bytes: Cow::Owned(
                img.pixels
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
                    .collect(),
            ),
            color: super::native::ColorType::Rgba8,
        })
    } else {
        Ok(PreparedPixels {
            bytes: Cow::Owned(
                img.pixels
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .flat_map(|pixel| [pixel[0]; 3])
                    .collect(),
            ),
            color: super::native::ColorType::Rgb8,
        })
    }
}

fn write_chunk(
    output: &mut Vec<u8>,
    name: &[u8; 4],
    payload: &[u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    write_chunk_with_prefix(output, name, &[], payload, token)
}

fn write_chunk_with_prefix(
    output: &mut Vec<u8>,
    name: &[u8; 4],
    prefix: &[u8],
    payload: &[u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    // Metadata and animation payloads are caller-sized. Keep the ordinary
    // no-token path as one bulk copy, while token-aware assembly polls every
    // complete 1,024-byte output interval.
    let payload_len = webp_chunk_payload_len(prefix.len(), payload.len())?;
    output.extend_from_slice(name);
    output.extend_from_slice(&low_u32(payload_len).to_le_bytes());
    output.extend_from_slice(prefix);
    extend_with_output_checkpoint(output, payload, token)?;
    if !payload_len.is_multiple_of(2) {
        output.push(0);
    }
    Ok(())
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
    if token.is_none() {
        return attach_metadata_reusing_output(encoded, width, height, alpha, opts);
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
    write_chunk(&mut output, b"VP8X", &vp8x, token)?;
    crate::codecs::error::check_cancelled(token)?;
    if let Some(payload) = icc {
        write_chunk(&mut output, b"ICCP", payload, token)?;
        crate::codecs::error::check_cancelled(token)?;
    }
    extend_with_output_checkpoint(&mut output, encoded_chunks, token)?;
    if let Some(payload) = exif {
        let payload = payload.strip_prefix(b"Exif\0\0").unwrap_or(payload);
        write_chunk(&mut output, b"EXIF", payload, token)?;
        crate::codecs::error::check_cancelled(token)?;
    }
    if let Some(payload) = xmp {
        write_chunk(&mut output, b"XMP ", payload, token)?;
        crate::codecs::error::check_cancelled(token)?;
    }
    crate::codecs::error::check_cancelled(token)?;
    finish_riff_with_options(output, opts)
}

// Metadata is inserted after the VP8X header, so an ordinary still encode can
// retain the completed codec buffer and shift only the existing RIFF chunks.
// Keep the token-aware path above separate: its output copies are observable
// through the established cancellation/work-budget checkpoints.
fn attach_metadata_reusing_output(
    mut encoded: Vec<u8>,
    width: u32,
    height: u32,
    alpha: bool,
    opts: &WebPEncodeOptions,
) -> CodecResult<Vec<u8>> {
    let icc = opts.icc.as_deref();
    let exif = opts
        .exif
        .as_deref()
        .map(|payload| payload.strip_prefix(b"Exif\0\0").unwrap_or(payload));
    let xmp = opts.xmp.as_deref();
    let existing_start = if encoded.get(12..16) == Some(b"VP8X") && encoded.len() >= 30 {
        30
    } else {
        12
    };
    let existing_len = encoded.len().checked_sub(existing_start).ok_or_else(|| {
        CodecError::Malformed("WebP encoded chunks are missing their RIFF header".to_owned())
    })?;
    let existing_end = webp_existing_end(existing_start, existing_len)?;

    let icc_len = icc.map_or(Ok(0), chunk_storage_len)?;
    let exif_len = exif.map_or(Ok(0), chunk_storage_len)?;
    let xmp_len = xmp.map_or(Ok(0), chunk_storage_len)?;
    let existing_chunks_start = webp_metadata_start(icc_len)?;
    let output_len = webp_chunk_payload_len(existing_chunks_start, existing_len)
        .and_then(|length| webp_chunk_payload_len(length, exif_len))
        .and_then(|length| webp_chunk_payload_len(length, xmp_len))?;
    encoded.reserve(output_len.saturating_sub(encoded.len()));
    encoded.resize(output_len, 0);
    encoded.copy_within(existing_start..existing_end, existing_chunks_start);

    encoded[0..4].copy_from_slice(b"RIFF");
    encoded[8..12].copy_from_slice(b"WEBP");
    let mut flags = u8::from(alpha).wrapping_shl(4);
    if icc.is_some() {
        flags |= 1u8.wrapping_shl(5);
    }
    if opts.exif.is_some() {
        flags |= 1u8.wrapping_shl(3);
    }
    if xmp.is_some() {
        flags |= 1u8.wrapping_shl(2);
    }
    let mut vp8x = [0u8; 10];
    vp8x[0] = flags;
    vp8x[4..7].copy_from_slice(&width.saturating_sub(1).to_le_bytes()[..3]);
    vp8x[7..10].copy_from_slice(&height.saturating_sub(1).to_le_bytes()[..3]);
    let mut offset = 12usize;
    write_chunk_in_place(&mut encoded, &mut offset, b"VP8X", &vp8x)?;
    if let Some(payload) = icc {
        write_chunk_in_place(&mut encoded, &mut offset, b"ICCP", payload)?;
    }
    offset = webp_chunk_payload_len(existing_chunks_start, existing_len)?;
    if let Some(payload) = exif {
        write_chunk_in_place(&mut encoded, &mut offset, b"EXIF", payload)?;
    }
    if let Some(payload) = xmp {
        write_chunk_in_place(&mut encoded, &mut offset, b"XMP ", payload)?;
    }
    debug_assert_eq!(offset, output_len);

    finish_riff_with_options(encoded, opts)
}

fn finish_riff_with_options(output: Vec<u8>, _opts: &WebPEncodeOptions) -> CodecResult<Vec<u8>> {
    #[cfg(coverage)]
    let output_len = if _opts.force_riff_size_overflow() {
        usize::MAX
    } else {
        output.len()
    };
    #[cfg(not(coverage))]
    let output_len = output.len();
    finish_riff(output, output_len)
}

fn chunk_storage_len(payload: &[u8]) -> CodecResult<usize> {
    let payload_end = webp_chunk_payload_len(8, payload.len())?;
    webp_chunk_payload_len(payload_end, usize::from(!payload.len().is_multiple_of(2)))
}

fn write_chunk_in_place(
    output: &mut [u8],
    offset: &mut usize,
    name: &[u8; 4],
    payload: &[u8],
) -> CodecResult<()> {
    #[cfg(coverage)]
    if coverage_should_fail_write_chunk_in_place() {
        return Err(CodecError::Malformed(
            "coverage-forced WebP in-place chunk failure".to_owned(),
        ));
    }
    let chunk_len = chunk_storage_len(payload)?;
    let end = webp_chunk_payload_len(*offset, chunk_len)?;
    let chunk = output.get_mut(*offset..end).ok_or_else(|| {
        CodecError::Malformed("WebP output buffer is too short for its metadata".to_owned())
    })?;
    chunk[..4].copy_from_slice(name);
    chunk[4..8].copy_from_slice(&low_u32(payload.len()).to_le_bytes());
    let payload_end = webp_chunk_payload_len(8, payload.len())?;
    chunk[8..payload_end].copy_from_slice(payload);
    if !payload.len().is_multiple_of(2) {
        chunk[payload_end] = 0;
    }
    *offset = end;
    Ok(())
}

/// Lossless VP8L encoding via the internal `WebPEncoder`.
fn encode_lossless(
    pixels: &PreparedPixels<'_>,
    width: u32,
    height: u32,
    encoder: &mut super::native::WebPEncoder,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    encoder
        .encode_with_token(&pixels.bytes, width, height, pixels.color, token)
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
    encoder: &mut super::native::WebPEncoder,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    let quality = opts.quality.unwrap_or(80);
    let method = opts.method.unwrap_or(4);
    let encoded = match pixels.color {
        super::native::ColorType::Rgb8 => {
            vp8::encoder::encode_vp8_lossy(&pixels.bytes, width, height, quality, method, token)?
        }
        super::native::ColorType::Rgba8 => {
            let has_alpha = pixels.has_nonopaque_alpha_with_token(token)?;
            if has_alpha {
                crate::codecs::error::check_cancelled(token)?;
                let alpha_chunk = encoder
                    .encode_alpha_from_rgba_with_token(&pixels.bytes, width, height, token)
                    .map_err(encode_error)?;
                crate::codecs::error::check_cancelled(token)?;
                vp8::encoder::encode_vp8_lossy_rgba(
                    &pixels.bytes,
                    width,
                    height,
                    quality,
                    method,
                    &alpha_chunk,
                    token,
                )?
            } else {
                let rgb = pixels.rgb_without_alpha_with_token(token)?;
                crate::codecs::error::check_cancelled(token)?;
                vp8::encoder::encode_vp8_lossy(&rgb, width, height, quality, method, token)?
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
        super::native::EncodingError::Cancelled => CodecError::Cancelled,
        super::native::EncodingError::WorkBudgetExceeded { maximum, observed } => {
            CodecError::WorkBudgetExceeded {
                maximum,
                observed,
                resource: crate::ResourceLimit::EncodeWorkUnits,
            }
        }
    }
}

fn cmyk_to_rgb(pixels: &[u8], token: Option<&crate::CancellationToken>) -> CodecResult<Vec<u8>> {
    if let Some(token) = token {
        let mut rgb = Vec::with_capacity(pixels.len().saturating_div(4).saturating_mul(3));
        let mut pixels_until_checkpoint = PREPARE_CHECKPOINT_PIXELS;
        for pixel in pixels.as_chunks::<4>().0 {
            let black = u16::from(255u8.saturating_sub(pixel[3]));
            for &channel in pixel.iter().take(3) {
                let ink = u16::from(255u8.saturating_sub(channel));
                rgb.push(
                    ink.saturating_mul(black)
                        .saturating_add(127)
                        .div_euclid(255)
                        .to_le_bytes()[0],
                );
            }
            checkpoint_after_prepare_pixel(&mut pixels_until_checkpoint, token)?;
        }
        return Ok(rgb);
    }
    Ok(pixels
        .as_chunks::<4>()
        .0
        .iter()
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
        .collect())
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

#[cfg_attr(all(coverage, target_pointer_width = "64"), coverage(off))]
fn webp_riff_size_from_len(encoded_len: usize) -> CodecResult<u32> {
    #[cfg(coverage)]
    if FORCE_RIFF_SIZE_ERROR.swap(false, Ordering::Relaxed) {
        return Err(CodecError::Dimensions(
            "coverage-forced WebP RIFF size failure".to_owned(),
        ));
    }
    u32::try_from(encoded_len.saturating_sub(8)).map_err(|_| {
        CodecError::Dimensions("WebP RIFF output exceeds its 32-bit size field".to_owned())
    })
}

fn expand_l16_to_rgb(
    img: &DecodedImage,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<PreparedPixels<'static>> {
    // Pillow's I;16 -> RGB conversion clamps each unsigned sample to 255.
    // PNG decoding stores L16 samples as little-endian bytes, so a zero high
    // byte keeps the low byte and every larger sample becomes 255.
    let pixel_count = img.pixels.len().saturating_div(2);
    let mut rgb = Vec::with_capacity(pixel_count.saturating_mul(3));
    if let Some(token) = token {
        let mut pixels_until_checkpoint = PREPARE_CHECKPOINT_PIXELS;
        for pixel in img.pixels.as_chunks::<2>().0 {
            let value = if pixel[1] == 0 { pixel[0] } else { u8::MAX };
            rgb.extend_from_slice(&[value; 3]);
            checkpoint_after_prepare_pixel(&mut pixels_until_checkpoint, token)?;
        }
    } else {
        for pixel in img.pixels.as_chunks::<2>().0 {
            let value = if pixel[1] == 0 { pixel[0] } else { u8::MAX };
            rgb.extend_from_slice(&[value; 3]);
        }
    }
    Ok(PreparedPixels {
        bytes: Cow::Owned(rgb),
        color: super::native::ColorType::Rgb8,
    })
}

#[cfg_attr(all(coverage, target_pointer_width = "64"), coverage(off))]
fn webp_payload_len_from_u32(value: u32) -> CodecResult<usize> {
    #[cfg(coverage)]
    if FORCE_PAYLOAD_LEN_ERROR.swap(false, Ordering::Relaxed) {
        return Err(CodecError::Dimensions(
            "coverage-forced WebP chunk size failure".to_owned(),
        ));
    }
    usize::try_from(value)
        .map_err(|_| CodecError::Dimensions("WebP chunk size does not fit usize".to_owned()))
}

#[cfg_attr(coverage, coverage(off))]
fn webp_chunk_payload_len(prefix_len: usize, payload_len: usize) -> CodecResult<usize> {
    #[cfg(coverage)]
    if coverage_should_fail_chunk_payload_call() {
        return Err(CodecError::Dimensions(
            "coverage-forced WebP chunk payload failure".to_owned(),
        ));
    }
    prefix_len.checked_add(payload_len).ok_or_else(|| {
        CodecError::Dimensions("WebP chunk payload exceeds addressable size".to_owned())
    })
}

#[cfg_attr(coverage, coverage(off))]
fn webp_sink_output_end(written: usize, bytes: usize) -> CodecResult<usize> {
    #[cfg(coverage)]
    if FORCE_SINK_OUTPUT_END_ERROR.swap(false, Ordering::Relaxed) {
        return Err(CodecError::Dimensions(
            "coverage-forced WebP sink output failure".to_owned(),
        ));
    }
    written
        .checked_add(bytes)
        .ok_or_else(|| CodecError::Dimensions("WebP sink output length overflows".to_owned()))
}

#[cfg_attr(coverage, coverage(off))]
fn webp_existing_end(start: usize, length: usize) -> CodecResult<usize> {
    #[cfg(coverage)]
    if FORCE_EXISTING_END_ERROR.swap(false, Ordering::Relaxed) {
        return Err(CodecError::Dimensions(
            "coverage-forced WebP existing chunk end failure".to_owned(),
        ));
    }
    start.checked_add(length).ok_or_else(|| {
        CodecError::Dimensions("WebP encoded chunks exceed addressable size".to_owned())
    })
}

#[cfg_attr(coverage, coverage(off))]
fn webp_metadata_start(icc_len: usize) -> CodecResult<usize> {
    #[cfg(coverage)]
    if FORCE_METADATA_START_ERROR.swap(false, Ordering::Relaxed) {
        return Err(CodecError::Dimensions(
            "coverage-forced WebP metadata start failure".to_owned(),
        ));
    }
    30usize
        .checked_add(icc_len)
        .ok_or_else(|| CodecError::Dimensions("WebP metadata exceeds addressable size".to_owned()))
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    vp8::__coverage_exercise_private_branches();

    // These are defensive container and source-normalization states. They are
    // real Rust contracts, but Pillow cannot manufacture malformed RIFF
    // output, caller tokens, or an owned output sink for them.
    let mut sink = Vec::new();
    for malformed in [
        Vec::new(),
        b"not a RIFF".to_vec(),
        b"NOPE\0\0\0\0WEBP".to_vec(),
        b"RIFF\0\0\0\0WEBP".to_vec(),
        b"RIFF\0\0\0\0NOPE".to_vec(),
        b"RIFF\x0c\0\0\0WEBPVP8 ".to_vec(),
        b"RIFF\x08\0\0\0WEBPVP8 ".to_vec(),
        b"RIFF\x0c\0\0\0WEBPVP8 \0\0\0\x01".to_vec(),
        b"RIFF\x0c\0\0\0WEBPVP8 \0\0\0\x04\0\0\0\0".to_vec(),
    ] {
        let _ = write_riff_to_sink(&malformed, None, &mut sink);
        sink.clear();
    }
    let mut zero_chunk = b"RIFF\x0c\0\0\0WEBPVP8 \0\0\0\0".to_vec();
    let _ = write_riff_to_sink(&zero_chunk, None, &mut sink);
    sink.clear();
    zero_chunk[4..8].copy_from_slice(&20u32.to_le_bytes());
    let _ = write_riff_to_sink(&zero_chunk, None, &mut sink);
    sink.clear();
    let mut extended = b"RIFF\x14\0\0\0WEBP".to_vec();
    extended.extend_from_slice(&1u32.to_be_bytes());
    extended.extend_from_slice(b"VP8 ");
    extended.extend_from_slice(&16u64.to_be_bytes());
    let _ = write_riff_to_sink(&extended, None, &mut sink);
    sink.clear();
    let mut extended_short = b"RIFF\x0c\0\0\0WEBP".to_vec();
    extended_short.extend_from_slice(&1u32.to_be_bytes());
    extended_short.extend_from_slice(b"VP8 ");
    let _ = write_riff_to_sink(&extended_short, None, &mut sink);
    sink.clear();
    let mut extended_overflow = b"RIFF\x14\0\0\0WEBP".to_vec();
    extended_overflow.extend_from_slice(&1u32.to_be_bytes());
    extended_overflow.extend_from_slice(b"VP8 ");
    extended_overflow.extend_from_slice(&u64::MAX.to_be_bytes());
    let _ = write_riff_to_sink(&extended_overflow, None, &mut sink);
    let mut too_short = [0u8; 12];
    let mut offset = too_short.len();
    let _ = write_chunk_in_place(&mut too_short, &mut offset, b"TEST", &[1]);
    let mut copied = Vec::new();
    let _ = extend_with_output_checkpoint(&mut copied, &[0; OUTPUT_COPY_CHECKPOINT_BYTES], None);
    let cancelled_copy = crate::CancellationToken::new();
    cancelled_copy.cancel();
    let _ = extend_with_output_checkpoint(
        &mut copied,
        &[0; OUTPUT_COPY_CHECKPOINT_BYTES],
        Some(&cancelled_copy),
    );

    let opaque_rgba = PreparedPixels {
        bytes: Cow::Owned(vec![0, 0, 0, u8::MAX]),
        color: super::native::ColorType::Rgba8,
    };
    let transparent_rgba = PreparedPixels {
        bytes: Cow::Owned(vec![0, 0, 0, 0]),
        color: super::native::ColorType::Rgba8,
    };
    let token = crate::CancellationToken::new();
    let _ = opaque_rgba.has_nonopaque_alpha_with_token(Some(&token));
    let _ = transparent_rgba.has_nonopaque_alpha_with_token(Some(&token));
    let _ = opaque_rgba.rgb_without_alpha_with_token(Some(&token));

    let bilevel = DecodedImage::with_mode(8, 1, vec![0b1010_1010], ImageMode::L1);
    let indexed_opaque = DecodedImage::with_mode(2, 1, vec![0, 1], ImageMode::P8).with_palette(
        crate::types::ImagePalette::new(vec![255, 0, 0, 0, 255, 0], vec![u8::MAX, u8::MAX])
            .expect("coverage palette should be valid"),
    );
    let indexed_alpha = indexed_opaque.clone().with_palette(
        crate::types::ImagePalette::new(vec![255, 0, 0, 0, 255, 0], vec![u8::MAX, 0])
            .expect("coverage alpha palette should be valid"),
    );
    let la_opaque = DecodedImage::new(1, 1, vec![7, u8::MAX], crate::types::ColorType::La8);
    let la_alpha = DecodedImage::new(1, 1, vec![7, 0], crate::types::ColorType::La8);
    let cmyk = DecodedImage::new(1, 1, vec![1, 2, 3, 4], crate::types::ColorType::Cmyk8);
    for image in [
        &bilevel,
        &indexed_opaque,
        &indexed_alpha,
        &la_opaque,
        &la_alpha,
        &cmyk,
    ] {
        let token = crate::CancellationToken::new();
        let _ = prepare_pixels(image, Some(&token));
    }
    let _ = cmyk_to_rgb(&cmyk.pixels, Some(&crate::CancellationToken::new()));
    let _ = encode_error(super::native::EncodingError::Cancelled);

    // Large mode-specific inputs reach the 1,024-pixel preparation polls.
    // Small parity images intentionally stay below those Rust-only
    // cancellation boundaries, so exercise each branch with a bounded
    // cancellation point here.
    let large_l1 = DecodedImage::with_mode(1_024, 1, vec![0; 128], ImageMode::L1);
    let large_l8 = DecodedImage::new(1_024, 1, vec![0; 1_024], crate::types::ColorType::L8);
    let large_l16 = DecodedImage::with_mode(
        1_024,
        1,
        (0..1_024)
            .flat_map(|pixel| [pixel as u8, u8::from(pixel % 2 != 0)])
            .collect(),
        ImageMode::L16,
    );
    let large_cmyk =
        DecodedImage::new(1_024, 1, vec![0; 1_024 * 4], crate::types::ColorType::Cmyk8);
    let large_la_alpha = DecodedImage::new(
        1_024,
        1,
        (0..1_024).flat_map(|_| [7, 0]).collect(),
        crate::types::ColorType::La8,
    );
    let large_la_opaque = DecodedImage::new(
        1_024,
        1,
        (0..1_024).flat_map(|_| [7, u8::MAX]).collect(),
        crate::types::ColorType::La8,
    );
    let large_palette =
        crate::types::ImagePalette::new(vec![255, 0, 0, 0, 255, 0], vec![u8::MAX, u8::MAX])
            .expect("coverage palette should be valid");
    let large_indexed = DecodedImage::with_mode(1_024, 1, vec![0; 1_024], ImageMode::P8)
        .with_palette(large_palette);
    for image in [
        &large_l1,
        &large_l8,
        &large_l16,
        &large_cmyk,
        &large_la_alpha,
        &large_la_opaque,
    ] {
        let token = crate::CancellationToken::new();
        token.cancel_after(0);
        let _ = prepare_pixels(image, Some(&token));
    }
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let _ = prepare_pixels(&large_indexed, Some(&token));
    let token = crate::CancellationToken::new();
    token.cancel_after(1);
    let _ = prepare_pixels(&large_indexed, Some(&token));
    let token = crate::CancellationToken::new();
    token.cancel_after(1);
    let _ = prepare_pixels(&large_la_alpha, Some(&token));
    let token = crate::CancellationToken::new();
    token.cancel_after(1);
    let _ = prepare_pixels(&large_la_opaque, Some(&token));
    let large_prepared_alpha = PreparedPixels {
        bytes: Cow::Owned((0..1_024).flat_map(|_| [0, 0, 0, u8::MAX]).collect()),
        color: super::native::ColorType::Rgba8,
    };
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let _ = large_prepared_alpha.has_nonopaque_alpha_with_token(Some(&token));
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let _ = large_prepared_alpha.rgb_without_alpha_with_token(Some(&token));

    let mut metadata = WebPEncodeOptions::default();
    metadata.icc = Some(vec![0]);
    metadata.exif = Some(b"Exif\0\0metadata".to_vec());
    metadata.xmp = Some(vec![1, 2, 3]);
    let rgba = DecodedImage::new(1, 1, vec![1, 2, 3, 0], crate::types::ColorType::Rgba8);
    let metadata_token = crate::CancellationToken::new();
    let _ = encode_with_token(&rgba, &metadata, Some(&metadata_token));
    let _ = attach_metadata(
        b"RIFF\0\0\0\0WEBPVP8 \0\0\0\0".to_vec(),
        1,
        1,
        false,
        &metadata,
        None,
    );
    let _ = attach_metadata(
        b"RIFF\0\0\0\0WEBPVP8 \0\0\0\0".to_vec(),
        1,
        1,
        false,
        &metadata,
        Some(&metadata_token),
    );
    let mut existing_vp8x = b"RIFF\0\0\0\0WEBPVP8X".to_vec();
    existing_vp8x.extend_from_slice(&18u32.to_le_bytes());
    existing_vp8x.extend_from_slice(&[0; 10]);
    let _ = attach_metadata(existing_vp8x, 1, 1, false, &metadata, None);
    let _ = attach_metadata(
        b"RIFF\0\0\0\0WEBPVP8X".to_vec(),
        1,
        1,
        false,
        &metadata,
        None,
    );
    let _ = attach_metadata(b"short".to_vec(), 1, 1, false, &metadata, None);

    // Exercise each independently optional token-aware metadata chunk and
    // the short existing-VP8X defensive shape. These are caller-owned Rust
    // metadata controls, not synthetic Pillow parity rows.
    let metadata_input = b"RIFF\0\0\0\0WEBPVP8 \0\0\0\0".to_vec();
    let metadata_cases = [
        {
            let mut options = WebPEncodeOptions::default();
            options.icc = Some(vec![1]);
            options
        },
        {
            let mut options = WebPEncodeOptions::default();
            options.exif = Some(b"Exif\0\0x".to_vec());
            options
        },
        {
            let mut options = WebPEncodeOptions::default();
            options.xmp = Some(vec![2]);
            options
        },
    ];
    for options in &metadata_cases {
        let token = crate::CancellationToken::new();
        let _ = attach_metadata(metadata_input.clone(), 1, 1, true, options, Some(&token));
    }

    let mut animation = DecodedSequence::from_image(rgba.clone());
    animation.frames.push(animation.frames[0].clone());
    animation.kind = crate::types::SequenceKind::TimedAnimation;
    let _ = encode_sequence_with_token(&animation, &WebPEncodeOptions::default(), Some(&token));
    let animation_token = crate::CancellationToken::new();
    let _ = encode_sequence_with_token(
        &animation,
        &WebPEncodeOptions::default(),
        Some(&animation_token),
    );
    let animation_probe = crate::CancellationToken::new();
    animation_probe.cancel_after(usize::MAX);
    let _ = encode_sequence_with_token(
        &animation,
        &WebPEncodeOptions::default(),
        Some(&animation_probe),
    );
    let animation_checks = usize::MAX.saturating_sub(
        animation_probe
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=animation_checks {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = encode_sequence_with_token(&animation, &WebPEncodeOptions::default(), Some(&token));
    }
    let _ = encode_sequence(&animation, &WebPEncodeOptions::default());

    let mut invalid_keyframe = DecodedSequence::from_image(rgba.clone());
    invalid_keyframe.frames[0].source.interlaced = true;
    let _ = encode_sequence_with_token(
        &invalid_keyframe,
        &WebPEncodeOptions::default(),
        Some(&crate::CancellationToken::new()),
    );
    let mut invalid_sequence_sink = Vec::new();
    let _ = encode_sequence_to_sink(
        &invalid_keyframe,
        &WebPEncodeOptions::default(),
        EncodePolicy::default(),
        CodecOperation::SequenceEncode,
        None,
        &mut invalid_sequence_sink,
    );
    let mut invalid_duration = DecodedSequence::from_image(rgba.clone());
    invalid_duration.frames[0].source.duration = crate::types::FrameDuration {
        numerator: 1,
        denominator: 3,
    };
    let _ = encode_sequence_with_token(
        &invalid_duration,
        &WebPEncodeOptions::default(),
        Some(&crate::CancellationToken::new()),
    );

    let mut opts = WebPEncodeOptions::default();
    opts.icc = Some(vec![0]);
    opts.set_force_riff_size_overflow();
    let _ = attach_metadata(b"RIFF\0\0\0\0WEBP".to_vec(), 1, 1, false, &opts, None);

    let simple_rgb = DecodedImage::new(1, 1, vec![0, 0, 0], crate::types::ColorType::Rgb8);
    let valid_encoded = encode(&simple_rgb, &WebPEncodeOptions::default())
        .expect("coverage WebP input must encode");
    let valid_metadata_token = crate::CancellationToken::new();
    let _ = attach_metadata(
        valid_encoded.clone(),
        1,
        1,
        false,
        &metadata,
        Some(&valid_metadata_token),
    );
    let _ = attach_metadata_reusing_output(valid_encoded.clone(), 1, 1, false, &metadata);
    for call in [0, 2, 3] {
        FORCE_CHUNK_PAYLOAD_CALL.store(call, Ordering::Relaxed);
        let token = crate::CancellationToken::new();
        let _ = attach_metadata(valid_encoded.clone(), 1, 1, false, &metadata, Some(&token));
    }
    FORCE_WRITE_CHUNK_IN_PLACE_CALL.store(usize::MAX, Ordering::Relaxed);
    FORCE_CHUNK_PAYLOAD_CALL.store(17, Ordering::Relaxed);
    let _ = attach_metadata_reusing_output(valid_encoded.clone(), 1, 1, false, &metadata);
    FORCE_RIFF_SIZE_ERROR.store(true, Ordering::Relaxed);
    let _ = write_riff_to_sink(&valid_encoded, None, &mut sink);
    for call in 0..=2 {
        FORCE_CHUNK_PAYLOAD_CALL.store(call, Ordering::Relaxed);
        let _ = write_riff_to_sink(&valid_encoded, None, &mut sink);
        sink.clear();
    }
    FORCE_PAYLOAD_LEN_ERROR.store(true, Ordering::Relaxed);
    let _ = write_riff_to_sink(&valid_encoded, None, &mut sink);
    FORCE_SINK_OUTPUT_END_ERROR.store(true, Ordering::Relaxed);
    let _ = write_riff_to_sink(&valid_encoded, None, &mut sink);
    sink.clear();

    for call in 0..=2 {
        FORCE_CHUNK_PAYLOAD_CALL.store(call, Ordering::Relaxed);
        let token = crate::CancellationToken::new();
        let _ = encode_sequence_with_token(&animation, &WebPEncodeOptions::default(), Some(&token));
    }
    for call in 0..=2 {
        FORCE_CHUNK_PAYLOAD_CALL.store(call, Ordering::Relaxed);
        let _ = encode_sequence(&animation, &WebPEncodeOptions::default());
    }

    FORCE_EXISTING_END_ERROR.store(true, Ordering::Relaxed);
    let _ = attach_metadata_reusing_output(valid_encoded.clone(), 1, 1, false, &metadata);
    FORCE_METADATA_START_ERROR.store(true, Ordering::Relaxed);
    let _ = attach_metadata_reusing_output(valid_encoded.clone(), 1, 1, false, &metadata);
    for call in [0, 2, 4, 6] {
        FORCE_CHUNK_PAYLOAD_CALL.store(call, Ordering::Relaxed);
        let _ = attach_metadata_reusing_output(valid_encoded.clone(), 1, 1, false, &metadata);
    }
    FORCE_CHUNK_PAYLOAD_CALL.store(0, Ordering::Relaxed);
    let _ = chunk_storage_len(&[0]);
    let mut in_place_buffer = vec![0; 64];
    for call in [0, 2, 3] {
        let mut in_place_offset = 12;
        FORCE_CHUNK_PAYLOAD_CALL.store(call, Ordering::Relaxed);
        let _ = write_chunk_in_place(&mut in_place_buffer, &mut in_place_offset, b"TEST", &[1, 2]);
    }
    for call in 0..=3 {
        FORCE_WRITE_CHUNK_IN_PLACE_CALL.store(call, Ordering::Relaxed);
        let _ = attach_metadata_reusing_output(valid_encoded.clone(), 1, 1, false, &metadata);
    }

    let mut lossless_alpha_options = WebPEncodeOptions::default();
    lossless_alpha_options.lossless = Some(true);
    let alpha_pipeline_token = crate::CancellationToken::new();
    FORCE_ALPHA_SCAN_ERROR.store(true, Ordering::Relaxed);
    let mut alpha_pipeline_encoder = super::native::WebPEncoder::new();
    let _ = encode_pixels(
        &rgba,
        &lossless_alpha_options,
        &mut alpha_pipeline_encoder,
        Some(&alpha_pipeline_token),
    );

    let mut alpha_lossless_encoder = super::native::WebPEncoder::new();
    let alpha_lossy_token = crate::CancellationToken::new();
    alpha_lossy_token.cancel_after(0);
    let _ = encode_lossy(
        &large_prepared_alpha,
        1_024,
        1,
        &WebPEncodeOptions::default(),
        &mut alpha_lossless_encoder,
        Some(&alpha_lossy_token),
    );
    FORCE_RGB_EXTRACTION_ERROR.store(true, Ordering::Relaxed);
    let mut forced_rgb_encoder = super::native::WebPEncoder::new();
    let _ = encode_lossy(
        &opaque_rgba,
        1,
        1,
        &WebPEncodeOptions::default(),
        &mut forced_rgb_encoder,
        Some(&crate::CancellationToken::new()),
    );
    let opaque_rgb_token = crate::CancellationToken::new();
    opaque_rgb_token.cancel_after(2_047);
    let mut opaque_rgb_encoder = super::native::WebPEncoder::new();
    let _ = encode_lossy(
        &large_prepared_alpha,
        1_024,
        1,
        &WebPEncodeOptions::default(),
        &mut opaque_rgb_encoder,
        Some(&opaque_rgb_token),
    );
    let opaque_lossy_probe = crate::CancellationToken::new();
    opaque_lossy_probe.cancel_after(usize::MAX);
    let mut opaque_lossless_encoder = super::native::WebPEncoder::new();
    let _ = encode_lossy(
        &opaque_rgba,
        1,
        1,
        &WebPEncodeOptions::default(),
        &mut opaque_lossless_encoder,
        Some(&opaque_lossy_probe),
    );
    let opaque_lossy_checks = usize::MAX.saturating_sub(
        opaque_lossy_probe
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=opaque_lossy_checks {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut encoder = super::native::WebPEncoder::new();
        let _ = encode_lossy(
            &opaque_rgba,
            1,
            1,
            &WebPEncodeOptions::default(),
            &mut encoder,
            Some(&token),
        );
    }
    let encode_pixels_probe = crate::CancellationToken::new();
    encode_pixels_probe.cancel_after(usize::MAX);
    let mut encode_pixels_encoder = super::native::WebPEncoder::new();
    let _ = encode_pixels(
        &rgba,
        &WebPEncodeOptions::default(),
        &mut encode_pixels_encoder,
        Some(&encode_pixels_probe),
    );
    let encode_pixels_checks = usize::MAX.saturating_sub(
        encode_pixels_probe
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=encode_pixels_checks {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut encoder = super::native::WebPEncoder::new();
        let _ = encode_pixels(
            &rgba,
            &WebPEncodeOptions::default(),
            &mut encoder,
            Some(&token),
        );
    }
    let write_probe = crate::CancellationToken::new();
    write_probe.cancel_after(usize::MAX);
    let mut write_probe_sink = Vec::new();
    let _ = write_riff_to_sink(&valid_encoded, Some(&write_probe), &mut write_probe_sink);
    let write_checks = usize::MAX.saturating_sub(
        write_probe
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=write_checks {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut sink = Vec::new();
        let _ = write_riff_to_sink(&valid_encoded, Some(&token), &mut sink);
    }
    struct FailingSink;
    impl OutputSink for FailingSink {
        fn write_all(&mut self, _bytes: &[u8]) -> crate::ImageResult<()> {
            Err(crate::ImageError::parameter("coverage sink failure"))
        }
    }
    let mut failing_sink = FailingSink;
    let _ = write_riff_to_sink(&valid_encoded, None, &mut failing_sink);

    let mut metadata_sweep_opts = WebPEncodeOptions::default();
    metadata_sweep_opts.icc = Some(vec![1; 1_025]);
    metadata_sweep_opts.exif = Some(b"Exif\0\0coverage".to_vec());
    metadata_sweep_opts.xmp = Some(vec![2, 3, 4]);
    let metadata_probe = crate::CancellationToken::new();
    metadata_probe.cancel_after(usize::MAX);
    let _ = attach_metadata(
        valid_encoded.clone(),
        1,
        1,
        false,
        &metadata_sweep_opts,
        Some(&metadata_probe),
    );
    let metadata_checks = usize::MAX.saturating_sub(
        metadata_probe
            .coverage_remaining_checks()
            .unwrap_or(usize::MAX),
    );
    for checks in 0..=metadata_checks {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = attach_metadata(
            valid_encoded.clone(),
            1,
            1,
            false,
            &metadata_sweep_opts,
            Some(&token),
        );
    }

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
    for checks in 0..=16 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = encode_sequence_with_token(&sequence, &WebPEncodeOptions::default(), Some(&token));
    }

    // Sweep the still checkpoints, including metadata assembly, without
    // turning caller-controlled cancellation into a Pillow parity row.
    let still = DecodedImage::new(1, 1, vec![0, 0, 0], crate::types::ColorType::Rgb8);
    let default_opts = WebPEncodeOptions::default();
    for checks in 0..=8 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = encode_with_token(&still, &default_opts, Some(&token));
    }

    let mut metadata_opts = WebPEncodeOptions::default();
    metadata_opts.icc = Some(vec![0]);
    metadata_opts.exif = Some(b"Exif\0\0metadata".to_vec());
    metadata_opts.xmp = Some(b"xmp".to_vec());
    for checks in 0..=14 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = encode_with_token(&still, &metadata_opts, Some(&token));
    }

    #[cfg(coverage_nightly)]
    {
        // Measure each complete public pipeline, then cancel at every poll
        // boundary. The fixed prefix above is cheap for ordinary coverage;
        // this nightly-only sweep reaches the later metadata and animation
        // assembly `?` edges without guessing their checkpoint counts.
        let sweep_still = |image: &DecodedImage, options: &WebPEncodeOptions| {
            let probe = crate::CancellationToken::new();
            probe.cancel_after(usize::MAX);
            let _ = encode_with_token(image, options, Some(&probe));
            let checks =
                usize::MAX.saturating_sub(probe.coverage_remaining_checks().unwrap_or(usize::MAX));
            for checks in 0..=checks {
                let token = crate::CancellationToken::new();
                token.cancel_after(checks);
                let _ = encode_with_token(image, options, Some(&token));
            }
        };
        sweep_still(&still, &default_opts);
        sweep_still(&rgba, &default_opts);
        sweep_still(&still, &metadata_opts);

        let mut sweep_sequence = DecodedSequence::from_image(still.clone());
        sweep_sequence.frames.push(sweep_sequence.frames[0].clone());
        sweep_sequence.kind = crate::types::SequenceKind::TimedAnimation;
        let probe = crate::CancellationToken::new();
        probe.cancel_after(usize::MAX);
        let _ = encode_sequence_with_token(&sweep_sequence, &default_opts, Some(&probe));
        let checks =
            usize::MAX.saturating_sub(probe.coverage_remaining_checks().unwrap_or(usize::MAX));
        for checks in 0..=checks {
            let token = crate::CancellationToken::new();
            token.cancel_after(checks);
            let _ = encode_sequence_with_token(&sweep_sequence, &default_opts, Some(&token));
        }
    }
}
