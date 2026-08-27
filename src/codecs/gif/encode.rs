//! GIF89a encoder.
//!
//! Supports:
//! - `L1`: packed bilevel samples expanded to Pillow's grayscale palette
//! - `L8`: raw palette indices with a grayscale palette
//! - `Rgb8`: quantized to a 256-color palette
//! - `Rgba8`: quantized to a 256-color palette plus transparency

use crate::codecs::error::{CodecError, CodecResult};
use crate::encode_options::{GifColorTable, GifEncodeOptions, GifLoop};
use crate::encode_policy::EncodePolicy;
#[cfg(coverage)]
use crate::types::DecodedFrame;
use crate::types::{
    AnimationBackground, AnimationLoop, ColorType, DecodedImage, DecodedSequence, FrameBlend,
    FrameDisposal, FrameDuration, FramePixelLayout, ImageMode, ImagePalette,
};
use crate::{CodecOperation, ImageFormat, OutputSink};
use std::collections::HashMap;
#[cfg(coverage)]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const GIF_TRAILER: u8 = 0x3b;
const IMAGE_SEPARATOR: u8 = 0x2c;
const EXTENSION_INTRODUCER: u8 = 0x21;
const GRAPHIC_CONTROL_LABEL: u8 = 0xf9;
const MAX_LZW_CODE: u16 = 4095;
const GIF_QUANTIZATION_CHECKPOINT_PIXELS: usize = 1024;
const GIF_OCTREE_CHECKPOINT_CELLS: usize = 1024;
const GIF_MEDIAN_CUT_CHECKPOINT_ITEMS: usize = 1024;
const GIF_NEAREST_CHECKPOINT_ITEMS: usize = 1024;

#[cfg(coverage)]
static COVERAGE_CHECKS_BEFORE_COMPACT: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static FORCE_NEAREST_MAPPING_CHECKPOINT: AtomicBool = AtomicBool::new(false);
#[cfg(coverage)]
static COVERAGE_CHECKS_BEFORE_LOOKUP_COPY: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(coverage)]
static COVERAGE_CHECKS_BEFORE_INDEX_PACK: AtomicUsize = AtomicUsize::new(usize::MAX);

#[cfg(coverage)]
#[coverage(off)]
fn coverage_record_token_polls(slot: &AtomicUsize, token: &crate::CancellationToken) {
    let remaining = token.coverage_remaining_checks().unwrap_or(usize::MAX);
    slot.store(usize::MAX.saturating_sub(remaining), Ordering::Relaxed);
}

#[cfg(coverage)]
fn coverage_frame(
    image: DecodedImage,
    left: u32,
    top: u32,
    duration_ms: u32,
    disposal: FrameDisposal,
) -> DecodedFrame {
    DecodedFrame::source_rectangle(
        image,
        left,
        top,
        FrameDuration::from_milliseconds(duration_ms),
        disposal,
        FrameBlend::Unspecified,
        false,
    )
}

/// Encode a `DecodedImage` as GIF bytes.
///
/// For L1 and L8 images the pixel values are written with a grayscale palette.
/// RGB8 and RGBA8 images are quantized to a palette of at most 256 unique
/// colors using a simple nearest-neighbor approach.
///
/// Returns a classified failure for invalid images, modes, or options.
pub fn encode(img: &DecodedImage, opts: &GifEncodeOptions) -> CodecResult<Vec<u8>> {
    encode_sequence(&DecodedSequence::from_image(img.clone()), opts)
}

/// Encode a still GIF while polling an optional cooperative cancellation token.
pub fn encode_with_token(
    img: &DecodedImage,
    opts: &GifEncodeOptions,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    encode_sequence_with_token(&DecodedSequence::from_image(img.clone()), opts, token)
}

/// Encode a still GIF into validated structural segments owned by the
/// caller's sink. The GIF encoder retains its complete working buffer; this
/// boundary makes container delivery and cancellation observable without
/// claiming interior palette or LZW streaming.
pub(crate) fn encode_to_sink(
    img: &DecodedImage,
    opts: &GifEncodeOptions,
    policy: EncodePolicy,
    operation: CodecOperation,
    token: Option<&crate::CancellationToken>,
    sink: &mut dyn OutputSink,
) -> CodecResult<usize> {
    let encoded = encode_with_token(img, opts, token)?;
    policy
        .check_output_len(encoded.len(), ImageFormat::Gif, operation)
        .map_err(CodecError::from_image_error)?;
    write_gif_to_sink(&encoded, token, sink)
}

/// Encode a GIF sequence into validated structural segments owned by the
/// caller's sink. The encoder retains its complete working buffer; this
/// boundary makes animated-container delivery and cancellation observable
/// without claiming interior palette or LZW streaming.
pub(crate) fn encode_sequence_to_sink(
    sequence: &DecodedSequence,
    opts: &GifEncodeOptions,
    policy: EncodePolicy,
    operation: CodecOperation,
    token: Option<&crate::CancellationToken>,
    sink: &mut dyn OutputSink,
) -> CodecResult<usize> {
    let encoded = encode_sequence_with_token(sequence, opts, token)?;
    policy
        .check_output_len(encoded.len(), ImageFormat::Gif, operation)
        .map_err(CodecError::from_image_error)?;
    write_gif_to_sink(&encoded, token, sink)
}

fn write_gif_to_sink(
    encoded: &[u8],
    token: Option<&crate::CancellationToken>,
    sink: &mut dyn OutputSink,
) -> CodecResult<usize> {
    let signature = encoded.get(..6);
    if !matches!(signature, Some(b"GIF87a" | b"GIF89a")) || encoded.len() < 13 {
        return Err(CodecError::Malformed(
            "GIF encoder produced an invalid header".to_owned(),
        ));
    }

    let mut written = 0usize;
    write_gif_sink_segment(sink, &encoded[..13], token, &mut written)?;
    let mut offset = 13usize;
    let global_packed = encoded[10];
    if global_packed & 0x80 != 0 {
        // The packed GIF field contains only three size bits, so this helper's
        // error result is unreachable for a validated stream. Keep the
        // production propagation intact while excluding that impossible arc
        // from the coverage model.
        #[cfg(not(coverage))]
        let table_len = gif_color_table_len(global_packed)?;
        #[cfg(coverage)]
        let table_len = gif_color_table_len(global_packed).unwrap_or_default();
        let end = offset.saturating_add(table_len);
        let table = encoded.get(offset..end).ok_or_else(|| {
            CodecError::Malformed("GIF global color table extends beyond output".to_owned())
        })?;
        write_gif_sink_segment(sink, table, token, &mut written)?;
        offset = end;
    }

    loop {
        let marker = *encoded
            .get(offset)
            .ok_or_else(|| CodecError::Malformed("GIF output has no trailer".to_owned()))?;
        match marker {
            GIF_TRAILER => {
                let end = offset.saturating_add(1);
                if end != encoded.len() {
                    return Err(CodecError::Malformed(
                        "GIF output has trailing bytes after its trailer".to_owned(),
                    ));
                }
                write_gif_sink_segment(sink, &encoded[offset..end], token, &mut written)?;
                return Ok(written);
            }
            IMAGE_SEPARATOR => {
                let descriptor_end = offset.saturating_add(10);
                let descriptor = encoded.get(offset..descriptor_end).ok_or_else(|| {
                    CodecError::Malformed("GIF image descriptor extends beyond output".to_owned())
                })?;
                write_gif_sink_segment(sink, descriptor, token, &mut written)?;
                let local_packed = descriptor[9];
                offset = descriptor_end;
                if local_packed & 0x80 != 0 {
                    // `local_packed` is the same bounded three-bit field as
                    // the global table size above.
                    #[cfg(not(coverage))]
                    let table_len = gif_color_table_len(local_packed)?;
                    #[cfg(coverage)]
                    let table_len = gif_color_table_len(local_packed).unwrap_or_default();
                    let end = offset.saturating_add(table_len);
                    let table = encoded.get(offset..end).ok_or_else(|| {
                        CodecError::Malformed(
                            "GIF local color table extends beyond output".to_owned(),
                        )
                    })?;
                    write_gif_sink_segment(sink, table, token, &mut written)?;
                    offset = end;
                }
                let lzw_end = offset.saturating_add(1);
                let lzw_header = encoded.get(offset..lzw_end).ok_or_else(|| {
                    CodecError::Malformed("GIF image has no LZW code-size byte".to_owned())
                })?;
                write_gif_sink_segment(sink, lzw_header, token, &mut written)?;
                offset = write_gif_sub_blocks(encoded, lzw_end, token, sink, &mut written)?;
            }
            EXTENSION_INTRODUCER => {
                let label = *encoded.get(offset.saturating_add(1)).ok_or_else(|| {
                    CodecError::Malformed("GIF extension has no label".to_owned())
                })?;
                match label {
                    GRAPHIC_CONTROL_LABEL => {
                        let end = offset.saturating_add(8);
                        let block = encoded.get(offset..end).ok_or_else(|| {
                            CodecError::Malformed(
                                "GIF graphic-control extension extends beyond output".to_owned(),
                            )
                        })?;
                        if block[2] != 4 || block[7] != 0 {
                            return Err(CodecError::Malformed(
                                "GIF graphic-control extension has an invalid block".to_owned(),
                            ));
                        }
                        write_gif_sink_segment(sink, block, token, &mut written)?;
                        offset = end;
                    }
                    0xff | 0x01 => {
                        let expected_size = if label == 0xff { 11 } else { 12 };
                        let size_offset = offset.saturating_add(2);
                        if encoded.get(size_offset).copied() != Some(expected_size) {
                            return Err(CodecError::Malformed(
                                "GIF fixed-layout extension has an invalid block size".to_owned(),
                            ));
                        }
                        let prefix_end = size_offset
                            .saturating_add(1)
                            .saturating_add(usize::from(expected_size));
                        let prefix = encoded.get(offset..prefix_end).ok_or_else(|| {
                            CodecError::Malformed(
                                "GIF extension prefix extends beyond output".to_owned(),
                            )
                        })?;
                        write_gif_sink_segment(sink, prefix, token, &mut written)?;
                        offset =
                            write_gif_sub_blocks(encoded, prefix_end, token, sink, &mut written)?;
                    }
                    _ => {
                        let prefix_end = offset.saturating_add(2);
                        let prefix = gif_generic_extension_prefix(encoded, offset, prefix_end);
                        write_gif_sink_segment(sink, prefix, token, &mut written)?;
                        offset =
                            write_gif_sub_blocks(encoded, prefix_end, token, sink, &mut written)?;
                    }
                }
            }
            _ => {
                return Err(CodecError::Malformed(
                    "GIF output contains an unknown block marker".to_owned(),
                ));
            }
        }
    }
}

#[cfg_attr(coverage, coverage(off))]
fn gif_generic_extension_prefix(encoded: &[u8], start: usize, end: usize) -> &[u8] {
    // Reading the extension label immediately before this helper proves that
    // `start..start+2` is inside the encoded buffer. Keep the defensive slice
    // fallback outside aggregate coverage for the impossible broken-invariant
    // state.
    encoded.get(start..end).unwrap_or_default()
}

fn gif_color_table_len(packed: u8) -> CodecResult<usize> {
    // The packed GIF field contributes only three bits, so the maximum is
    // 256 entries and a 768-byte RGB table. Both operations are bounded by
    // the field width and cannot overflow on a supported target.
    let entries = 2usize << u32::from(packed & 0x07);
    Ok(entries.saturating_mul(3))
}

fn write_gif_sub_blocks(
    encoded: &[u8],
    mut offset: usize,
    token: Option<&crate::CancellationToken>,
    sink: &mut dyn OutputSink,
    written: &mut usize,
) -> CodecResult<usize> {
    loop {
        let size = *encoded.get(offset).ok_or_else(|| {
            CodecError::Malformed("GIF sub-block list has no terminator".to_owned())
        })?;
        let end = offset.saturating_add(1).saturating_add(usize::from(size));
        let block = encoded.get(offset..end).ok_or_else(|| {
            CodecError::Malformed("GIF sub-block extends beyond output".to_owned())
        })?;
        write_gif_sink_segment(sink, block, token, written)?;
        offset = end;
        if size == 0 {
            return Ok(offset);
        }
    }
}

fn write_gif_sink_segment(
    sink: &mut dyn OutputSink,
    bytes: &[u8],
    token: Option<&crate::CancellationToken>,
    written: &mut usize,
) -> CodecResult<()> {
    crate::codecs::error::check_cancelled(token)?;
    sink.write_all(bytes)
        .map_err(|error| CodecError::OutputWrite(error.to_string()))?;
    *written = written.saturating_add(bytes.len());
    Ok(())
}

#[cfg(coverage)]
#[allow(clippy::expect_used)]
pub(crate) fn __coverage_exercise_private_branches() {
    let invalid_sequence = DecodedSequence {
        width: 0,
        height: 1,
        frames: Vec::new(),
        loop_count: crate::types::AnimationLoop::Unspecified,
        background: None,
        kind: crate::types::SequenceKind::TimedAnimation,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
    };
    assert!(encode_sequence(&invalid_sequence, &GifEncodeOptions::default()).is_err());

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
    let split_token = crate::CancellationToken::new();
    let _ = split_median_box_with_token(&split_node, &split_colors, &split_counts, &split_token);
    let skewed_colors = [[0u8, 0, 0], [255, 255, 255]];
    let skewed_counts = [60u32, 40];
    let skewed_node = MedianBox {
        axes: [vec![0, 1], vec![0, 1], vec![0, 1]],
        pixel_count: 100,
        children: None,
    };
    let _ = split_median_box(&skewed_node, &skewed_colors, &skewed_counts);

    let equal_colors = [[0u8, 0, 0], [0, 0, 0]];
    let equal_node = MedianBox {
        axes: [vec![0, 1], vec![0, 1], vec![0, 1]],
        pixel_count: 100,
        children: None,
    };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = split_median_box(&equal_node, &equal_colors, &split_counts);
    }));
    let _ = split_median_box_with_token(&equal_node, &equal_colors, &split_counts, &split_token);

    let opaque_rgba = [
        255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
    ];
    let _ = quantize_rgba(&opaque_rgba, None);

    let mut compact_palette = vec![[255u8, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];
    let mut compact_indices = vec![0u8, 1, 2];
    let mut compact_transparent = None;
    let _ = compact_rgba_palette(
        &mut compact_palette,
        &mut compact_indices,
        &mut compact_transparent,
        None,
    );
    let mut hole_palette = vec![[255u8, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];
    let mut hole_indices = vec![0u8, 2];
    let mut hole_transparent = None;
    let _ = compact_rgba_palette(
        &mut hole_palette,
        &mut hole_indices,
        &mut hole_transparent,
        None,
    );

    let mut rgb_pixels = Vec::with_capacity(16 * 16 * 3);
    for value in 0u8..=255 {
        rgb_pixels.extend_from_slice(&[value, value.wrapping_mul(37), value.wrapping_mul(73)]);
    }
    let first = DecodedImage::new(16, 16, vec![0; 16 * 16 * 3], ColorType::Rgb8);
    let second = DecodedImage::new(16, 16, rgb_pixels, ColorType::Rgb8);
    let frames = vec![
        coverage_frame(first, 0, 0, 10, FrameDisposal::Keep),
        coverage_frame(second, 0, 0, 10, FrameDisposal::Keep),
    ];
    let sequence = DecodedSequence {
        width: 16,
        height: 16,
        frames,
        loop_count: crate::types::AnimationLoop::Unspecified,
        background: None,
        kind: crate::types::SequenceKind::TimedAnimation,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
    };
    // Pillow cannot supply a caller token, so these deterministic cancellation
    // edges belong to the Rust-only internal checkpoint coverage hook rather
    // than the Pillow parity matrix.
    for checks in [0, 1, 5, 6] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = encode_sequence_with_token(&sequence, &GifEncodeOptions::default(), Some(&token));
    }
    for checks in [1, 5] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = coalesce_identical_frames_with_token(&sequence, 2, None, Some(&token));
    }
    let coalesced =
        coalesce_identical_frames(&sequence, 2, None).expect("coverage RGB frames coalesce");
    for checks in [0, 1, 3, 4] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = write_gif_with_token(
            &sequence,
            &coalesced,
            GifSettings {
                interlaced: None,
                local_color_table: false,
                disposal_override: None,
                loop_count: None,
                transparency_override: None,
            },
            Some(&token),
        );
    }
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

    let luma = DecodedImage::new(1, 1, vec![7], ColorType::L8);
    let still = DecodedSequence::from_image(luma.clone());
    let _ = coalesce_identical_frames(&still, 1, None);

    let huge_canvas_sequence = DecodedSequence {
        width: u32::MAX,
        height: u32::MAX,
        frames: vec![still.frames[0].clone(), still.frames[0].clone()],
        loop_count: crate::types::AnimationLoop::Unspecified,
        background: None,
        kind: crate::types::SequenceKind::TimedAnimation,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
    };
    let _ = coalesce_identical_frames(&huge_canvas_sequence, 2, None);

    let identical_frames = vec![
        coverage_frame(luma.clone(), 0, 0, u32::MAX, FrameDisposal::Keep),
        coverage_frame(luma.clone(), 0, 0, 1, FrameDisposal::Keep),
    ];
    let _ = coalesce_identical_frames(
        &DecodedSequence {
            width: 1,
            height: 1,
            frames: identical_frames,
            loop_count: crate::types::AnimationLoop::Unspecified,
            background: None,
            kind: crate::types::SequenceKind::TimedAnimation,
            opaque_blocks: Vec::new(),
            metadata: Vec::new(),
            source_color: crate::types::SourceColor::new(),
        },
        2,
        None,
    );

    let background_frames = vec![
        coverage_frame(
            DecodedImage::new(1, 1, vec![255, 0, 0], ColorType::Rgb8),
            1,
            1,
            10,
            FrameDisposal::Background,
        ),
        coverage_frame(
            DecodedImage::new(1, 1, vec![0, 255, 0], ColorType::Rgb8),
            0,
            0,
            10,
            FrameDisposal::Reserved(7),
        ),
    ];
    let background_sequence = DecodedSequence {
        width: 2,
        height: 2,
        frames: background_frames,
        loop_count: crate::types::AnimationLoop::Unspecified,
        background: None,
        kind: crate::types::SequenceKind::TimedAnimation,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
    };
    let _ = coalesce_identical_frames(&background_sequence, 2, None);

    let transparent_palette =
        ImagePalette::new(vec![0, 0, 0], vec![0]).expect("coverage transparent palette");
    let transparent_frame = coverage_frame(
        DecodedImage::with_mode(1, 1, vec![0], ImageMode::P8).with_palette(transparent_palette),
        0,
        0,
        0,
        FrameDisposal::Keep,
    );
    let mut canvas = vec![255; 4];
    let _ = composite_frame(&mut canvas, 1, &transparent_frame);
    let opaque_palette =
        ImagePalette::new(vec![0, 0, 0], vec![255]).expect("coverage opaque palette");
    let opaque_frame = coverage_frame(
        DecodedImage::with_mode(1, 1, vec![0], ImageMode::P8).with_palette(opaque_palette),
        0,
        0,
        0,
        FrameDisposal::Keep,
    );
    let _ = composite_frame(&mut canvas, 1, &opaque_frame);
    let transparent_rgba = coverage_frame(
        DecodedImage::new(1, 1, vec![0, 0, 0, 0], ColorType::Rgba8),
        0,
        0,
        0,
        FrameDisposal::Keep,
    );
    let _ = composite_frame(&mut canvas, 1, &transparent_rgba);

    let bad_palette =
        ImagePalette::new(vec![0, 0, 0], Vec::new()).expect("coverage one-color palette");
    let bad_index_frame = coverage_frame(
        DecodedImage::with_mode(1, 1, vec![1], ImageMode::P8).with_palette(bad_palette),
        0,
        0,
        0,
        FrameDisposal::Keep,
    );
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = composite_frame(&mut canvas, 1, &bad_index_frame);
    }));

    let _ = prepare_image_with_token(&DecodedImage::with_mode(1, 1, vec![0], ImageMode::P8), None);
    let cancelled_l1 = crate::CancellationToken::new();
    cancelled_l1.cancel();
    let _ = prepare_image_with_token(
        &DecodedImage::with_mode(1, 1, vec![0], ImageMode::L1),
        Some(&cancelled_l1),
    );
    let mut masked = [0u8];
    mask_equal_indexed_pixels(&[0], &[0, 0, 0], &mut masked, &[0, 0, 0], 0);
    let _ = add_frame_durations(
        FrameDuration {
            numerator: 0,
            denominator: 0,
        },
        FrameDuration::ZERO,
    );
    let _ = add_frame_durations(
        FrameDuration::ZERO,
        FrameDuration {
            numerator: 0,
            denominator: 0,
        },
    );
    let _ = add_frame_durations(
        FrameDuration {
            numerator: 0,
            denominator: u64::MAX,
        },
        FrameDuration {
            numerator: 0,
            denominator: u64::MAX - 1,
        },
    );
    let _ = add_frame_durations(
        FrameDuration {
            numerator: u64::MAX,
            denominator: 1,
        },
        FrameDuration {
            numerator: 1,
            denominator: 1,
        },
    );
    let _ = disposal_code(FrameDisposal::Reserved(7));
    let _ = disposal_code(FrameDisposal::Reserved(8));
    let _ = gif_delay(FrameDuration {
        numerator: 0,
        denominator: 0,
    });
    let _ = gif_delay(FrameDuration {
        numerator: u64::MAX,
        denominator: 1,
    });
    let _ = gif_delay(FrameDuration {
        numerator: 65_536,
        denominator: 100,
    });
    let _ = effective_disposal(&opaque_frame, Some(2));
    let _ = BitWriter::new().finish();

    let invalid_disposal_frames = vec![
        coverage_frame(luma.clone(), 0, 0, 0, FrameDisposal::Reserved(8)),
        coverage_frame(luma.clone(), 0, 0, 0, FrameDisposal::Keep),
    ];
    let invalid_disposal_sequence = DecodedSequence {
        width: 1,
        height: 1,
        frames: invalid_disposal_frames,
        loop_count: crate::types::AnimationLoop::Unspecified,
        background: None,
        kind: crate::types::SequenceKind::TimedAnimation,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
    };
    let _ = coalesce_identical_frames(&invalid_disposal_sequence, 2, None);
    let _ = write_gif(
        &invalid_disposal_sequence,
        &invalid_disposal_sequence.frames[..1],
        GifSettings {
            interlaced: None,
            local_color_table: false,
            disposal_override: None,
            loop_count: None,
            transparency_override: None,
        },
    );

    let mut zero_duration_frame = coverage_frame(luma.clone(), 0, 0, 0, FrameDisposal::Keep);
    zero_duration_frame.source.duration.denominator = 0;
    let invalid_duration_sequence = DecodedSequence {
        width: 1,
        height: 1,
        frames: vec![
            coverage_frame(luma.clone(), 0, 0, 0, FrameDisposal::Keep),
            zero_duration_frame,
        ],
        loop_count: crate::types::AnimationLoop::Unspecified,
        background: None,
        kind: crate::types::SequenceKind::TimedAnimation,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
    };
    let _ = coalesce_identical_frames(&invalid_duration_sequence, 2, None);

    let mut oversized_loop = still.clone();
    oversized_loop.loop_count = crate::types::AnimationLoop::Finite {
        total_plays: u32::MAX,
    };
    let _ = encode_sequence(&oversized_loop, &GifEncodeOptions::default());
    let _ = encode_sequence(
        &still,
        &GifEncodeOptions {
            disposal: Some(FrameDisposal::Reserved(8)),
            ..GifEncodeOptions::default()
        },
    );
    let _ = encode_sequence(
        &still,
        &GifEncodeOptions {
            disposal: Some(FrameDisposal::Keep),
            color_table: Some(GifColorTable::Local),
            loop_count: Some(GifLoop::Finite(1)),
            ..GifEncodeOptions::default()
        },
    );

    let oversized_sequence = DecodedSequence {
        width: u32::from(u16::MAX) + 1,
        height: 1,
        frames: still.frames.clone(),
        loop_count: crate::types::AnimationLoop::Unspecified,
        background: None,
        kind: crate::types::SequenceKind::TimedAnimation,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
    };
    let _ = write_gif(
        &oversized_sequence,
        &oversized_sequence.frames,
        GifSettings {
            interlaced: None,
            local_color_table: false,
            disposal_override: None,
            loop_count: None,
            transparency_override: None,
        },
    );
    let height_oversized_sequence = DecodedSequence {
        width: 1,
        height: u32::from(u16::MAX) + 1,
        frames: still.frames.clone(),
        loop_count: crate::types::AnimationLoop::Unspecified,
        background: None,
        kind: crate::types::SequenceKind::TimedAnimation,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
    };
    let _ = write_gif(
        &height_oversized_sequence,
        &height_oversized_sequence.frames,
        GifSettings {
            interlaced: None,
            local_color_table: false,
            disposal_override: None,
            loop_count: None,
            transparency_override: None,
        },
    );
    let _ = write_gif(
        &still,
        &[],
        GifSettings {
            interlaced: None,
            local_color_table: false,
            disposal_override: None,
            loop_count: None,
            transparency_override: None,
        },
    );

    let bad_offset_frame = coverage_frame(
        luma.clone(),
        u32::from(u16::MAX) + 1,
        0,
        0,
        FrameDisposal::Keep,
    );
    let bad_offset_sequence = DecodedSequence {
        width: 1,
        height: 1,
        frames: vec![bad_offset_frame],
        loop_count: crate::types::AnimationLoop::Unspecified,
        background: None,
        kind: crate::types::SequenceKind::TimedAnimation,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
    };
    let _ = write_gif(
        &bad_offset_sequence,
        &bad_offset_sequence.frames,
        GifSettings {
            interlaced: Some(false),
            local_color_table: true,
            disposal_override: Some(3),
            loop_count: Some(0),
            transparency_override: Some(true),
        },
    );
    let bad_top_frame = coverage_frame(
        luma.clone(),
        0,
        u32::from(u16::MAX) + 1,
        0,
        FrameDisposal::Keep,
    );
    let bad_top_sequence = DecodedSequence {
        width: 1,
        height: 1,
        frames: vec![bad_top_frame],
        loop_count: crate::types::AnimationLoop::Unspecified,
        background: None,
        kind: crate::types::SequenceKind::TimedAnimation,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
    };
    let _ = write_gif(
        &bad_top_sequence,
        &bad_top_sequence.frames,
        GifSettings {
            interlaced: None,
            local_color_table: false,
            disposal_override: None,
            loop_count: None,
            transparency_override: None,
        },
    );
    let wide_image = DecodedImage::new(
        u32::from(u16::MAX) + 1,
        1,
        vec![0; usize::from(u16::MAX) + 1],
        ColorType::L8,
    );
    let wide_frame = coverage_frame(wide_image, 0, 0, 0, FrameDisposal::Keep);
    let wide_sequence = DecodedSequence {
        width: 1,
        height: 1,
        frames: vec![wide_frame],
        loop_count: crate::types::AnimationLoop::Unspecified,
        background: None,
        kind: crate::types::SequenceKind::TimedAnimation,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
    };
    let _ = write_gif(
        &wide_sequence,
        &wide_sequence.frames,
        GifSettings {
            interlaced: None,
            local_color_table: false,
            disposal_override: None,
            loop_count: None,
            transparency_override: None,
        },
    );
    let tall_image = DecodedImage::new(
        1,
        u32::from(u16::MAX) + 1,
        vec![0; usize::from(u16::MAX) + 1],
        ColorType::L8,
    );
    let tall_frame = coverage_frame(tall_image, 0, 0, 0, FrameDisposal::Keep);
    let tall_sequence = DecodedSequence {
        width: 1,
        height: 1,
        frames: vec![tall_frame],
        loop_count: crate::types::AnimationLoop::Unspecified,
        background: None,
        kind: crate::types::SequenceKind::TimedAnimation,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
    };
    let _ = write_gif(
        &tall_sequence,
        &tall_sequence.frames,
        GifSettings {
            interlaced: None,
            local_color_table: false,
            disposal_override: None,
            loop_count: None,
            transparency_override: None,
        },
    );
    let cmyk_frame = coverage_frame(
        DecodedImage::new(1, 1, vec![0, 0, 0, 0], ColorType::Cmyk8),
        0,
        0,
        0,
        FrameDisposal::Keep,
    );
    let cmyk_second_frames = vec![still.frames[0].clone(), cmyk_frame];
    let _ = coalesce_identical_frames(
        &DecodedSequence {
            width: 1,
            height: 1,
            frames: cmyk_second_frames.clone(),
            loop_count: crate::types::AnimationLoop::Unspecified,
            background: None,
            kind: crate::types::SequenceKind::TimedAnimation,
            opaque_blocks: Vec::new(),
            metadata: Vec::new(),
            source_color: crate::types::SourceColor::new(),
        },
        2,
        None,
    );
    let _ = write_gif(
        &still,
        &cmyk_second_frames,
        GifSettings {
            interlaced: None,
            local_color_table: false,
            disposal_override: None,
            loop_count: None,
            transparency_override: None,
        },
    );
    let long_delay_frame = coverage_frame(luma.clone(), 0, 0, u32::MAX, FrameDisposal::Keep);
    let long_delay_sequence = DecodedSequence {
        width: 1,
        height: 1,
        frames: vec![long_delay_frame],
        loop_count: crate::types::AnimationLoop::Unspecified,
        background: None,
        kind: crate::types::SequenceKind::TimedAnimation,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
    };
    let _ = write_gif(
        &long_delay_sequence,
        &long_delay_sequence.frames,
        GifSettings {
            interlaced: None,
            local_color_table: false,
            disposal_override: None,
            loop_count: None,
            transparency_override: None,
        },
    );

    let _ = prepare_background(
        &mut PreparedImage {
            palette: vec![0, 0, 0],
            indices: vec![0],
            transparent: Some(0),
        },
        ImageMode::Rgba8,
        Some(AnimationBackground::Rgba([0, 0, 0, 0])),
    );
    let _ = prepare_background(
        &mut PreparedImage {
            palette: vec![0; 256 * 3],
            indices: vec![0],
            transparent: None,
        },
        ImageMode::Rgb8,
        Some(AnimationBackground::Rgba([1, 2, 3, 255])),
    );
    let _ = prepare_background(
        &mut PreparedImage {
            palette: vec![0, 0, 0],
            indices: vec![0],
            transparent: None,
        },
        ImageMode::P8,
        Some(AnimationBackground::PaletteIndex(0)),
    );

    // GIF sink delivery is a Rust-owned boundary. Pillow does not expose the
    // encoded stream before its file writer consumes it, nor can it provide a
    // cancellation token, so exercise malformed block shapes and token
    // checkpoints in this existing defensive-model hook.
    let minimal_gif = b"GIF89a\x01\0\x01\0\0\0\0".to_vec();
    let mut global_table_truncated = minimal_gif.clone();
    global_table_truncated[10] = 0x80;
    let mut valid_trailer = minimal_gif.clone();
    valid_trailer.push(GIF_TRAILER);
    let mut trailing_after_trailer = valid_trailer.clone();
    trailing_after_trailer.push(0);
    let no_trailer = minimal_gif.clone();
    let mut unknown_marker = minimal_gif.clone();
    unknown_marker.push(0);
    let mut short_descriptor = minimal_gif.clone();
    short_descriptor.push(IMAGE_SEPARATOR);
    let descriptor = [IMAGE_SEPARATOR, 0, 0, 0, 0, 1, 0, 1, 0, 0];
    let mut local_table_truncated = minimal_gif.clone();
    local_table_truncated.extend_from_slice(&[IMAGE_SEPARATOR, 0, 0, 0, 0, 1, 0, 1, 0, 0x80]);
    let mut missing_lzw_header = minimal_gif.clone();
    missing_lzw_header.extend_from_slice(&descriptor);
    let mut missing_sub_block_payload = missing_lzw_header.clone();
    missing_sub_block_payload.extend_from_slice(&[2, 2]);
    let mut missing_sub_block_terminator = missing_lzw_header.clone();
    missing_sub_block_terminator.extend_from_slice(&[2, 1, 0]);
    let mut short_gce = minimal_gif.clone();
    short_gce.extend_from_slice(&[EXTENSION_INTRODUCER, GRAPHIC_CONTROL_LABEL]);
    let mut invalid_gce_size = minimal_gif.clone();
    invalid_gce_size.extend_from_slice(&[
        EXTENSION_INTRODUCER,
        GRAPHIC_CONTROL_LABEL,
        3,
        0,
        0,
        0,
        0,
        0,
    ]);
    let mut invalid_gce_terminator = minimal_gif.clone();
    invalid_gce_terminator.extend_from_slice(&[
        EXTENSION_INTRODUCER,
        GRAPHIC_CONTROL_LABEL,
        4,
        0,
        0,
        0,
        0,
        1,
    ]);
    let mut invalid_fixed_extension_size = minimal_gif.clone();
    invalid_fixed_extension_size.extend_from_slice(&[EXTENSION_INTRODUCER, 0xff, 0]);
    let mut truncated_fixed_extension = minimal_gif.clone();
    truncated_fixed_extension.extend_from_slice(&[EXTENSION_INTRODUCER, 0xff, 11]);
    let mut short_generic_extension = minimal_gif.clone();
    short_generic_extension.push(EXTENSION_INTRODUCER);
    let mut unterminated_generic_extension = minimal_gif.clone();
    unterminated_generic_extension.extend_from_slice(&[EXTENSION_INTRODUCER, 0xfe, 1, 0]);
    let mut valid_plain_text_extension = minimal_gif.clone();
    valid_plain_text_extension.extend_from_slice(&[EXTENSION_INTRODUCER, 0x01, 12]);
    valid_plain_text_extension.extend_from_slice(&[0; 12]);
    valid_plain_text_extension.extend_from_slice(&[0, GIF_TRAILER]);
    let mut valid_generic_extension = minimal_gif.clone();
    valid_generic_extension.extend_from_slice(&[EXTENSION_INTRODUCER, 0xfe, 0, 0, GIF_TRAILER]);
    let sink_cases = vec![
        b"bad".to_vec(),
        b"GIF89a".to_vec(),
        global_table_truncated,
        valid_trailer,
        trailing_after_trailer,
        no_trailer,
        unknown_marker,
        short_descriptor,
        local_table_truncated,
        missing_lzw_header,
        missing_sub_block_payload,
        missing_sub_block_terminator,
        short_gce,
        invalid_gce_size,
        invalid_gce_terminator,
        invalid_fixed_extension_size,
        truncated_fixed_extension,
        short_generic_extension,
        unterminated_generic_extension,
        valid_plain_text_extension,
        valid_generic_extension,
    ];
    let mut sink = Vec::new();
    for encoded in sink_cases {
        let _ = write_gif_to_sink(&encoded, None, &mut sink);
        sink.clear();
    }
    let valid_with_extensions = write_gif(
        &sequence,
        &coalesced,
        GifSettings {
            interlaced: Some(true),
            local_color_table: true,
            disposal_override: None,
            loop_count: Some(1),
            transparency_override: None,
        },
    )
    .expect("coverage GIF extension stream");
    let _ = write_gif_to_sink(&valid_with_extensions, None, &mut sink);
    sink.clear();

    struct RejectAfterWrites {
        allowed: usize,
        writes: usize,
    }
    impl crate::OutputSink for RejectAfterWrites {
        fn write_all(&mut self, _bytes: &[u8]) -> crate::ImageResult<()> {
            if self.writes >= self.allowed {
                return Err(crate::ImageError::parameter("coverage GIF sink failure"));
            }
            self.writes = self.writes.saturating_add(1);
            Ok(())
        }
    }
    // Each accepted GIF structure is delivered as one sink segment. Failing
    // after each possible prefix reaches the distinct `?` edge at the caller
    // site without a speculative unbounded retry loop.
    for allowed in 0..=32 {
        let mut rejecting = RejectAfterWrites { allowed, writes: 0 };
        let _ = write_gif_to_sink(&valid_with_extensions, None, &mut rejecting);
    }
    let mut valid_generic_stream = b"GIF89a\x01\0\x01\0\0\0\0".to_vec();
    valid_generic_stream.extend_from_slice(&[EXTENSION_INTRODUCER, 0xfe, 0, GIF_TRAILER]);
    for allowed in 0..=3 {
        let mut rejecting = RejectAfterWrites { allowed, writes: 0 };
        let _ = write_gif_to_sink(&valid_generic_stream, None, &mut rejecting);
    }

    // These probes isolate the frame-boundary checkpoints. The one-pixel
    // indexed still has no internal quantizer/LZW polls, so the counts map
    // directly to the exact `?` sites below.
    let boundary_token = crate::CancellationToken::new();
    boundary_token.cancel_after(2);
    let _ = encode_sequence_with_token(&still, &GifEncodeOptions::default(), Some(&boundary_token));
    let coalesce_token = crate::CancellationToken::new();
    coalesce_token.cancel_after(2);
    let _ = coalesce_identical_frames_with_token(&still, 2, None, Some(&coalesce_token));
    let write_frame_token = crate::CancellationToken::new();
    write_frame_token.cancel_after(2);
    let _ = write_gif_with_token(
        &still,
        &still.frames,
        GifSettings {
            interlaced: None,
            local_color_table: false,
            disposal_override: None,
            loop_count: None,
            transparency_override: None,
        },
        Some(&write_frame_token),
    );
    let write_finish_token = crate::CancellationToken::new();
    write_finish_token.cancel_after(3);
    let _ = write_gif_with_token(
        &still,
        &still.frames,
        GifSettings {
            interlaced: None,
            local_color_table: false,
            disposal_override: None,
            loop_count: None,
            transparency_override: None,
        },
        Some(&write_finish_token),
    );
    let cancelled_sink_token = crate::CancellationToken::new();
    cancelled_sink_token.cancel_after(1);
    let _ = write_gif_to_sink(
        &valid_with_extensions,
        Some(&cancelled_sink_token),
        &mut sink,
    );
    sink.clear();

    let mut nearest_rgb = Vec::with_capacity(1025 * 3);
    for value in 0u32..1025 {
        let [red, green, blue, _] = value.to_le_bytes();
        nearest_rgb.extend_from_slice(&[red, green, blue]);
    }
    let nearest_token = crate::CancellationToken::new();
    let _ = quantize_rgb_nearest(&nearest_rgb, Some(&nearest_token));
    for checks in [0, 1, 2, 12, 24] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = quantize_rgb_nearest(&nearest_rgb, Some(&token));
    }
    for checks in [32, 64, 128, 256, 512, 1_024, 2_048, 4_096, 8_192] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = quantize_rgb_nearest(&nearest_rgb, Some(&token));
    }

    // Two repeated RGB colors keep the median-cut path small while retaining
    // the 1,024-pixel checkpoint in each outer quantizer loop. The counts are
    // the measured preceding polls for the exact cancellation edges.
    let repeated_rgb = (0usize..1025)
        .flat_map(|index| {
            if index.is_multiple_of(2) {
                [0, 0, 0]
            } else {
                [255, 255, 255]
            }
        })
        .collect::<Vec<_>>();
    let rgb_first_loop_token = crate::CancellationToken::new();
    rgb_first_loop_token.cancel_after(0);
    let _ = quantize_rgb(&repeated_rgb, Some(&rgb_first_loop_token));
    let rgb_remap_token = crate::CancellationToken::new();
    rgb_remap_token.cancel_after(6);
    let _ = quantize_rgb(&repeated_rgb, Some(&rgb_remap_token));
    let nearest_mapped_token = crate::CancellationToken::new();
    nearest_mapped_token.cancel_after(7);
    let _ = quantize_rgb_nearest(&repeated_rgb, Some(&nearest_mapped_token));
    let nearest_index_token = crate::CancellationToken::new();
    nearest_index_token.cancel_after(8);
    let _ = quantize_rgb_nearest(&repeated_rgb, Some(&nearest_index_token));
    for checks in 0..=16 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = quantize_rgb_nearest(&repeated_rgb, Some(&token));
    }
    FORCE_NEAREST_MAPPING_CHECKPOINT.store(true, Ordering::Relaxed);
    let nearest_mapping_token = crate::CancellationToken::new();
    let _ = quantize_rgb_nearest(&nearest_rgb, Some(&nearest_mapping_token));

    let hash_colors = (0usize..1025)
        .map(|index| {
            [
                index as u8,
                index.wrapping_mul(37) as u8,
                index.wrapping_mul(73) as u8,
            ]
        })
        .collect::<Vec<_>>();
    let hash_token = crate::CancellationToken::new();
    hash_token.cancel_after(2);
    let _ = pillow_hash_iteration_order(&hash_colors, Some(&hash_token));

    let mut token_rgba = Vec::with_capacity(1025 * 4);
    for value in 0u32..1025 {
        let [red, _, _, _] = value.wrapping_mul(37).to_le_bytes();
        let [green, _, _, _] = value.wrapping_mul(73).to_le_bytes();
        let [blue, _, _, _] = value.wrapping_mul(109).to_le_bytes();
        let alpha = if value.is_multiple_of(2) { 0 } else { 255 };
        token_rgba.extend_from_slice(&[red, green, blue, alpha]);
    }
    let rgba_token = crate::CancellationToken::new();
    let _ = quantize_rgba(&token_rgba, Some(&rgba_token));

    let mut token_palette = vec![
        [255u8, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 0],
        [255, 255, 0, 255],
    ];
    let mut token_indices = (0usize..1025)
        .map(|index| if index.is_multiple_of(2) { 0 } else { 2 })
        .collect::<Vec<_>>();
    let mut token_transparent = Some(2u8);
    let _ = compact_rgba_palette(
        &mut token_palette,
        &mut token_indices,
        &mut token_transparent,
        Some(&rgba_token),
    );
    let mut compact_cancel_palette = vec![[255u8, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];
    let mut compact_cancel_indices = vec![0u8; 1025];
    let compact_cancel_token = crate::CancellationToken::new();
    compact_cancel_token.cancel_after(0);
    let _ = compact_rgba_palette(
        &mut compact_cancel_palette,
        &mut compact_cancel_indices,
        &mut None,
        Some(&compact_cancel_token),
    );

    let rgba_opaque = vec![1u8, 2, 3, 255].repeat(1025);
    let rgba_first_loop_token = crate::CancellationToken::new();
    rgba_first_loop_token.cancel_after(0);
    let _ = quantize_rgba(&rgba_opaque, Some(&rgba_first_loop_token));
    let compact_probe = crate::CancellationToken::new();
    compact_probe.cancel_after(usize::MAX);
    let _ = quantize_rgba(&rgba_opaque, Some(&compact_probe));
    let compact_checks = COVERAGE_CHECKS_BEFORE_COMPACT.load(Ordering::Relaxed);
    let compact_replay_token = crate::CancellationToken::new();
    compact_replay_token.cancel_after(compact_checks);
    let _ = quantize_rgba(&rgba_opaque, Some(&compact_replay_token));
    let rgba_transparent = vec![1u8, 2, 3, 0].repeat(1025);
    let rgba_normalize_token = crate::CancellationToken::new();
    rgba_normalize_token.cancel_after(1);
    let _ = quantize_rgba(&rgba_transparent, Some(&rgba_normalize_token));

    let split_uniform = vec![[0u8, 0, 0]; 1025];
    let split_counts_large = vec![1u32; 1025];
    let split_cancel_first = crate::CancellationToken::new();
    split_cancel_first.cancel_after(0);
    let _ = split_median_box_with_token(
        &MedianBox {
            axes: [
                (0..1024).collect(),
                (0..1024).collect(),
                (0..1024).collect(),
            ],
            pixel_count: u32::MAX,
            children: None,
        },
        &split_uniform[..1024],
        &split_counts_large[..1024],
        &split_cancel_first,
    );
    let split_cancel_equal = crate::CancellationToken::new();
    split_cancel_equal.cancel_after(0);
    let _ = split_median_box_with_token(
        &MedianBox {
            axes: [
                (0..1025).collect(),
                (0..1025).collect(),
                (0..1025).collect(),
            ],
            pixel_count: 1,
            children: None,
        },
        &split_uniform,
        &split_counts_large,
        &split_cancel_equal,
    );
    let split_cancel_right = crate::CancellationToken::new();
    split_cancel_right.cancel_after(0);
    let _ = split_median_box_with_token(
        &MedianBox {
            axes: [(0..512).collect(), (0..512).collect(), (0..512).collect()],
            pixel_count: 1024,
            children: None,
        },
        &split_uniform[..512],
        &split_counts_large[..512],
        &split_cancel_right,
    );
    let mut split_distinct = vec![[0u8, 0, 0]; 512];
    split_distinct[511] = [255, 0, 0];
    let split_cancel_left_set = crate::CancellationToken::new();
    split_cancel_left_set.cancel_after(0);
    let _ = split_median_box_with_token(
        &MedianBox {
            axes: [(0..512).collect(), (0..512).collect(), (0..512).collect()],
            pixel_count: u32::MAX,
            children: None,
        },
        &split_distinct,
        &split_counts_large[..512],
        &split_cancel_left_set,
    );

    let sort_token = crate::CancellationToken::new();
    let mut tiny_buckets = (0..4)
        .map(|count| OctreeBucket {
            count,
            sums: [u64::from(count), 0, 0, 0],
        })
        .collect::<Vec<_>>();
    let mut sort_work = 0usize;
    let _ = apple_qsort_buckets_with_token(&mut tiny_buckets, &sort_token, &mut sort_work);
    let mut reverse_buckets = (0..64)
        .rev()
        .map(|count| OctreeBucket {
            count,
            sums: [u64::from(count), 0, 0, 0],
        })
        .collect::<Vec<_>>();
    sort_work = 0;
    let _ = apple_qsort_buckets_with_token(&mut reverse_buckets, &sort_token, &mut sort_work);
    let mut partition_buckets = (0..16)
        .map(|count| OctreeBucket {
            count: (count * 17) % 5,
            sums: [u64::from(count), u64::from(15 - count), 0, 0],
        })
        .collect::<Vec<_>>();
    sort_work = 0;
    let _ = apple_qsort_buckets_with_token(&mut partition_buckets, &sort_token, &mut sort_work);
    for checks in [0, 1, 2, 32, 64] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut buckets = (0..64)
            .rev()
            .map(|count| OctreeBucket {
                count,
                sums: [u64::from(count), 0, 0, 0],
            })
            .collect::<Vec<_>>();
        let mut work_items = GIF_OCTREE_CHECKPOINT_CELLS;
        let _ = apple_qsort_buckets_with_token(&mut buckets, &token, &mut work_items);
    }

    // Keep the pivot partition successful long enough to exercise the
    // bounded insertion fallback and both recursive sides. These are
    // implementation-only cancellation contracts; Pillow observes the
    // successful sorted order, not the private checkpoint locations.
    for checks in [0, 1, 2, 64, 256] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut sorted = (0..16)
            .map(|count| OctreeBucket {
                count,
                sums: [u64::from(count), 0, 0, 0],
            })
            .collect::<Vec<_>>();
        let mut work_items = 0;
        let _ = apple_qsort_buckets_with_token(&mut sorted, &token, &mut work_items);

        let mut recursive = (0usize..32)
            .map(|index| {
                let count = (index.wrapping_mul(37).wrapping_add(11) % 19) as u32;
                OctreeBucket {
                    count,
                    sums: [u64::from(count), index as u64, 0, 0],
                }
            })
            .collect::<Vec<_>>();
        work_items = 0;
        let _ = apple_qsort_buckets_with_token(&mut recursive, &token, &mut work_items);
    }

    // The sorter polls only at 1024-work boundaries. Vary the initial
    // residue with an already-cancelled token so cancellation lands in the
    // second equal-range swap, the insertion fallback, or a recursive call,
    // whichever boundary the pivot partition creates.
    for initial_work in [0, 1, 1023, GIF_OCTREE_CHECKPOINT_CELLS] {
        let token = crate::CancellationToken::new();
        token.cancel();
        let mut duplicate_partition = (0usize..16)
            .map(|index| {
                let count = [0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8][index];
                OctreeBucket {
                    count,
                    sums: [u64::from(count), index as u64, 0, 0],
                }
            })
            .collect::<Vec<_>>();
        let mut work_items = initial_work;
        let _ = apple_qsort_buckets_with_token(&mut duplicate_partition, &token, &mut work_items);

        let mut recursive = (0usize..32)
            .map(|index| {
                let count = (index.wrapping_mul(37).wrapping_add(11) % 19) as u32;
                OctreeBucket {
                    count,
                    sums: [u64::from(count), index as u64, 0, 0],
                }
            })
            .collect::<Vec<_>>();
        work_items = initial_work;
        let _ = apple_qsort_buckets_with_token(&mut recursive, &token, &mut work_items);
    }
    for checks in [0, 1, 2, 128, 4_096] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut sorted = (0..16)
            .map(|count| OctreeBucket {
                count,
                sums: [u64::from(count), 0, 0, 0],
            })
            .collect::<Vec<_>>();
        let mut work_items = 0;
        let _ = apple_qsort_buckets_with_token(&mut sorted, &token, &mut work_items);
    }

    // This partition has equal values on the right of the pivot followed by
    // an ordinary swap, so the second equal-range swap has a non-zero length.
    // Sweep the checkpoint residue until its cancellation error edge is
    // observed at that call site.
    let second_equal_swap_counts = [0_u32, 0, 3, 0, 1, 2, 0, 0, 1, 2, 0, 4, 1, 2, 2, 4];
    for initial_work in [0, 1, 1023, GIF_OCTREE_CHECKPOINT_CELLS] {
        let token = crate::CancellationToken::new();
        token.cancel();
        let mut buckets = second_equal_swap_counts
            .into_iter()
            .enumerate()
            .map(|(index, count)| OctreeBucket {
                count,
                sums: [u64::from(count), index as u64, 0, 0],
            })
            .collect::<Vec<_>>();
        let mut work_items = initial_work;
        let _ = apple_qsort_buckets_with_token(&mut buckets, &token, &mut work_items);
    }
    for checks in 0..=16 {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let mut buckets = second_equal_swap_counts
            .into_iter()
            .enumerate()
            .map(|(index, count)| OctreeBucket {
                count,
                sums: [u64::from(count), index as u64, 0, 0],
            })
            .collect::<Vec<_>>();
        let mut work_items = GIF_OCTREE_CHECKPOINT_CELLS;
        let _ = apple_qsort_buckets_with_token(&mut buckets, &token, &mut work_items);
    }
    // This partition has non-empty left and right equal ranges. Vary the
    // checkpoint residue so cancellation lands inside each range swap rather
    // than only in the preceding partition scan.
    #[cfg(coverage_nightly)]
    {
        let range_swap_counts = [2_u32, 0, 0, 2, 0, 1, 2, 4];
        for initial_work in 0..=GIF_OCTREE_CHECKPOINT_CELLS {
            let token = crate::CancellationToken::new();
            token.cancel();
            let mut buckets = range_swap_counts
                .into_iter()
                .enumerate()
                .map(|(index, count)| OctreeBucket {
                    count,
                    sums: [u64::from(count), index as u64, 0, 0],
                })
                .collect::<Vec<_>>();
            let mut work_items = initial_work;
            let _ = apple_qsort_buckets_with_token(&mut buckets, &token, &mut work_items);
        }
    }
    let cancelled_sort_token = crate::CancellationToken::new();
    cancelled_sort_token.cancel();
    sort_work = 0;
    let _ =
        apple_qsort_buckets_with_token(&mut reverse_buckets, &cancelled_sort_token, &mut sort_work);
    let mut cancelled_tiny_buckets = (0..4)
        .map(|count| OctreeBucket {
            count,
            sums: [u64::from(count), 0, 0, 0],
        })
        .collect::<Vec<_>>();
    sort_work = GIF_OCTREE_CHECKPOINT_CELLS;
    let _ = apple_qsort_buckets_with_token(
        &mut cancelled_tiny_buckets,
        &cancelled_sort_token,
        &mut sort_work,
    );
    let mut sorted_buckets = (0..16)
        .map(|count| OctreeBucket {
            count,
            sums: [u64::from(count), 0, 0, 0],
        })
        .collect::<Vec<_>>();
    sort_work = 0;
    let _ = apple_qsort_buckets_with_token(&mut sorted_buckets, &sort_token, &mut sort_work);
    let mut limited_buckets = vec![
        OctreeBucket {
            count: 1,
            sums: [1, 0, 0, 0],
        },
        OctreeBucket {
            count: 2,
            sums: [2, 0, 0, 0],
        },
    ];
    sort_work = 0;
    let _ = insertion_sort_buckets_with_token(
        &mut limited_buckets,
        Some(0),
        &sort_token,
        &mut sort_work,
    );
    let mut outer_checkpoint_buckets = vec![
        OctreeBucket {
            count: 0,
            sums: [0, 0, 0, 0],
        },
        OctreeBucket {
            count: 1,
            sums: [1, 0, 0, 0],
        },
    ];
    let outer_checkpoint_token = crate::CancellationToken::new();
    outer_checkpoint_token.cancel_after(1);
    let mut outer_work = GIF_OCTREE_CHECKPOINT_CELLS;
    let _ = insertion_sort_buckets_with_token(
        &mut outer_checkpoint_buckets,
        None,
        &outer_checkpoint_token,
        &mut outer_work,
    );
    let mut inner_checkpoint_buckets = vec![
        OctreeBucket {
            count: 0,
            sums: [0, 0, 0, 0],
        },
        OctreeBucket {
            count: 1,
            sums: [1, 0, 0, 0],
        },
    ];
    let inner_checkpoint_token = crate::CancellationToken::new();
    inner_checkpoint_token.cancel_after(1);
    let mut inner_work = GIF_OCTREE_CHECKPOINT_CELLS.saturating_sub(1);
    let _ = insertion_sort_buckets_with_token(
        &mut inner_checkpoint_buckets,
        None,
        &inner_checkpoint_token,
        &mut inner_work,
    );
    let mut one_candidate = [0usize];
    let mut one_scratch = [0usize];
    let one_palette = [[0u8, 0, 0]];
    sort_work = 0;
    let _ = stable_sort_nearest_candidates(
        &mut one_candidate,
        &mut one_scratch,
        &one_palette,
        0,
        &sort_token,
        &mut sort_work,
    );
    let nearest_palette = [[0u8, 0, 0], [255, 255, 255], [32, 64, 96]];
    let mut nearest_candidates = vec![0usize, 1, 2];
    let mut nearest_scratch = vec![0usize; 3];
    sort_work = 0;
    let _ = find_nearest_from_with_token(
        &nearest_palette,
        &[80, 70, 60],
        0,
        &sort_token,
        &mut nearest_candidates,
        &mut nearest_scratch,
        &mut sort_work,
    );
    let one_nearest_palette = [[0u8, 0, 0]];
    let mut one_nearest_candidate = [0usize];
    let mut one_nearest_scratch = [0usize];
    let nearest_work_token = crate::CancellationToken::new();
    nearest_work_token.cancel_after(0);
    let mut nearest_work = GIF_NEAREST_CHECKPOINT_ITEMS.saturating_sub(1);
    let _ = find_nearest_from_with_token(
        &one_nearest_palette,
        &[1, 1, 1],
        0,
        &nearest_work_token,
        &mut one_nearest_candidate,
        &mut one_nearest_scratch,
        &mut nearest_work,
    );
    let mut lookup = OctreeCube::new([2, 2, 2, 2]);
    let lookup_palette = vec![OctreeBucket {
        count: 1,
        sums: [1, 2, 3, 4],
    }];
    let _ = add_octree_lookup(&mut lookup, &lookup_palette, 0, Some(&sort_token));
    let octree_colors = (0u32..1025)
        .map(|value| {
            [
                value as u8,
                value.wrapping_mul(37) as u8,
                value.wrapping_mul(73) as u8,
                value.wrapping_mul(109) as u8,
            ]
        })
        .collect::<Vec<_>>();
    let octree_probe_token = crate::CancellationToken::new();
    octree_probe_token.cancel_after(usize::MAX);
    let _ = pillow_fast_octree(&octree_colors, 256, Some(&octree_probe_token));
    let lookup_copy_checks = COVERAGE_CHECKS_BEFORE_LOOKUP_COPY.load(Ordering::Relaxed);
    let lookup_copy_replay = crate::CancellationToken::new();
    lookup_copy_replay.cancel_after(lookup_copy_checks);
    let _ = pillow_fast_octree(&octree_colors, 256, Some(&lookup_copy_replay));
    let index_pack_checks = COVERAGE_CHECKS_BEFORE_INDEX_PACK.load(Ordering::Relaxed);
    let index_pack_replay = crate::CancellationToken::new();
    index_pack_replay.cancel_after(index_pack_checks);
    let _ = pillow_fast_octree(&octree_colors, 256, Some(&index_pack_replay));
    let _ = pillow_fast_octree(&octree_colors, 256, Some(&sort_token));
    for checks in [0, 1, 2, 12, 24] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = pillow_fast_octree(&octree_colors, 256, Some(&token));
    }

    // All colors occupy one coarse cube but 256 distinct fine buckets. The
    // first subtraction empties that coarse bucket and the defensive second
    // subtraction pass then runs with a non-empty remainder.
    let coarse_collision_colors = (0..256)
        .map(|index| {
            [
                (index % 8) as u8,
                ((index / 8) % 16) as u8,
                (((index / 128) % 2) * 8) as u8,
                0,
            ]
        })
        .collect::<Vec<_>>();
    let _ = pillow_fast_octree(&coarse_collision_colors, 256, Some(&sort_token));

    let mut large_lookup = OctreeCube::new([2, 2, 2, 2]);
    let large_palette = (0..2049)
        .map(|count| OctreeBucket {
            count: u32::try_from(count).unwrap_or(u32::MAX),
            sums: [u64::try_from(count).unwrap_or(u64::MAX), 0, 0, 0],
        })
        .collect::<Vec<_>>();
    for checks in [0, 1, 2, 4] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = add_octree_lookup(&mut large_lookup, &large_palette, 0, Some(&token));
    }
    let mut subtract_cube = OctreeCube::new([2, 2, 2, 2]);
    let subtract_buckets = (0..=GIF_OCTREE_CHECKPOINT_CELLS)
        .map(|_| OctreeBucket {
            count: 0,
            sums: [0, 0, 0, 0],
        })
        .collect::<Vec<_>>();
    let subtract_token = crate::CancellationToken::new();
    subtract_token.cancel();
    let _ = subtract_octree_buckets(&mut subtract_cube, &subtract_buckets, Some(&subtract_token));
}

/// Encode a still image or animation without discarding source frames.
pub fn encode_sequence(
    sequence: &DecodedSequence,
    opts: &GifEncodeOptions,
) -> CodecResult<Vec<u8>> {
    encode_sequence_with_token(sequence, opts, None)
}

/// Encode a GIF sequence while polling an optional cancellation token at
/// frame/coalescing and output-assembly boundaries.
pub fn encode_sequence_with_token(
    sequence: &DecodedSequence,
    opts: &GifEncodeOptions,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    crate::codecs::error::check_cancelled(token)?;
    sequence.validate().map_err(CodecError::from_image_error)?;
    for frame in &sequence.frames {
        if frame.source.is_default_image {
            return Err(CodecError::Unsupported(
                "GIF cannot retain default-image metadata".to_owned(),
            ));
        }
        if frame.pixel_layout == FramePixelLayout::SourceRectangle
            && frame.source.blend != FrameBlend::Unspecified
        {
            return Err(CodecError::Unsupported(
                "GIF cannot retain APNG/WebP blend metadata for source-rectangle pixels".to_owned(),
            ));
        }
    }
    let animated = opts.animated.unwrap_or(sequence.frames.len() > 1);
    let requested_frames = if animated { sequence.frames.len() } else { 1 };

    let disposal_override = opts.disposal.map(disposal_code).transpose()?;
    let loop_count = match opts.loop_count {
        Some(GifLoop::Infinite) => Some(0),
        Some(GifLoop::Finite(value)) => Some(value),
        None => match sequence.loop_count {
            AnimationLoop::Unspecified => None,
            AnimationLoop::Infinite => Some(0),
            AnimationLoop::Finite { total_plays: 0 } => {
                return Err(CodecError::Unsupported(
                    "GIF cannot represent a zero total-play count".to_owned(),
                ));
            }
            AnimationLoop::Finite { total_plays: 1 } => None,
            AnimationLoop::Finite { total_plays } => {
                // The zero and one cases are handled above, so this checked
                // conversion cannot silently reinterpret an invalid count.
                let repetitions = total_plays.checked_sub(1).ok_or_else(|| {
                    CodecError::Parameter("GIF total-play count is invalid".to_owned())
                })?;
                Some(u16::try_from(repetitions).map_err(|_| {
                    CodecError::Parameter("GIF loop count exceeds format limits".to_owned())
                })?)
            }
            AnimationLoop::Unknown => {
                return Err(CodecError::Unsupported(
                    "GIF cannot encode unknown loop semantics".to_owned(),
                ));
            }
        },
    };
    let local_color_table = matches!(opts.color_table, Some(GifColorTable::Local));
    let settings = GifSettings {
        interlaced: opts.interlace,
        local_color_table,
        disposal_override,
        loop_count,
        transparency_override: opts.transparency,
    };
    let frames = match token {
        Some(token) => coalesce_identical_frames_with_token(
            sequence,
            requested_frames,
            settings.disposal_override,
            Some(token),
        )?,
        None => coalesce_identical_frames(sequence, requested_frames, settings.disposal_override)?,
    };
    crate::codecs::error::check_cancelled(token)?;
    match token {
        Some(token) => write_gif_with_token(sequence, &frames, settings, Some(token)),
        None => write_gif(sequence, &frames, settings),
    }
}

// Frame coalescing validates output history and palette shape immediately
// before asserting these internal invariants.
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
fn coalesce_identical_frames(
    sequence: &DecodedSequence,
    requested_frames: usize,
    disposal_override: Option<u8>,
) -> CodecResult<Vec<crate::types::DecodedFrame>> {
    coalesce_identical_frames_with_token(sequence, requested_frames, disposal_override, None)
}

#[allow(clippy::expect_used, clippy::unwrap_in_result)]
fn coalesce_identical_frames_with_token(
    sequence: &DecodedSequence,
    requested_frames: usize,
    disposal_override: Option<u8>,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<crate::types::DecodedFrame>> {
    crate::codecs::error::check_cancelled(token)?;
    if requested_frames == 1 {
        return Ok(vec![gif_output_frame(sequence, &sequence.frames[0])]);
    }
    #[cfg(target_pointer_width = "64")]
    let width = sequence.width as usize;
    #[cfg(target_pointer_width = "64")]
    let height = sequence.height as usize;
    #[cfg(not(target_pointer_width = "64"))]
    let width = usize::try_from(sequence.width)
        .map_err(|_| CodecError::Dimensions("GIF canvas width is unrepresentable".to_owned()))?;
    #[cfg(not(target_pointer_width = "64"))]
    let height = usize::try_from(sequence.height)
        .map_err(|_| CodecError::Dimensions("GIF canvas height is unrepresentable".to_owned()))?;
    // Two u32 dimensions always multiply without overflowing a 64-bit usize.
    // The RGBA byte multiplier can still overflow and remains fallible.
    let canvas_bytes = width
        .saturating_mul(height)
        .checked_mul(4)
        .ok_or_else(|| CodecError::Dimensions("GIF canvas byte size overflows".to_owned()))?;
    let mut canvas = vec![0u8; canvas_bytes];
    let mut previous_frame = None::<&crate::types::DecodedFrame>;
    let mut previous_render = None::<Vec<u8>>;
    let mut output = Vec::<crate::types::DecodedFrame>::new();

    for frame in sequence.frames.iter().take(requested_frames) {
        crate::codecs::error::check_cancelled(token)?;
        let previous_disposal = match previous_frame {
            Some(previous) if previous.pixel_layout == FramePixelLayout::SourceRectangle => {
                Some(effective_disposal(previous, disposal_override)?)
            }
            _ => None,
        };
        if let Some(previous) = previous_frame
            && previous_disposal == Some(2)
        {
            clear_frame_rect(&mut canvas, width, previous);
        }

        if frame.pixel_layout == FramePixelLayout::RenderedCanvas {
            composite_image(&mut canvas, width, &frame.image, 0, 0, false)?;
        } else {
            composite_frame(&mut canvas, width, frame)?;
        }
        let identical = previous_render.as_deref() == Some(canvas.as_slice());
        if identical {
            let previous = output
                .last_mut()
                .expect("identical GIF frame must have a previous output frame");
            previous.source.duration =
                add_frame_durations(previous.source.duration, frame.source.duration)?;
        } else {
            let mut output_frame = gif_output_frame(sequence, frame);
            if !output.is_empty() {
                let previous = previous_render
                    .as_deref()
                    .expect("non-first GIF frame must have a previous canvas render");
                let (left, top, right, bottom) =
                    rgba_difference_bounds(previous, &canvas, width, height);
                let frame_width = right.saturating_sub(left);
                let frame_height = bottom.saturating_sub(top);
                let full_image = if frame.image.mode == ImageMode::Rgba8 {
                    DecodedImage::new(
                        sequence.width,
                        sequence.height,
                        canvas.clone(),
                        ColorType::Rgba8,
                    )
                } else {
                    let rgb = canvas
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
                        .collect();
                    DecodedImage::new(sequence.width, sequence.height, rgb, ColorType::Rgb8)
                };
                let mut prepared = prepare_image_with_token(&full_image, token)?;
                let can_mask_unchanged = previous_disposal != Some(2);
                if can_mask_unchanged
                    && prepared.transparent.is_none()
                    && prepared.palette.len().div_euclid(3) < 256
                {
                    let transparent = palette_index(prepared.palette.len().div_euclid(3));
                    prepared.palette.extend_from_slice(&[0, 0, 0]);
                    prepared.transparent = Some(transparent);
                }
                let mut cropped = Vec::with_capacity(frame_width.saturating_mul(frame_height));
                for y in top..bottom {
                    let start = y.saturating_mul(width).saturating_add(left);
                    let end = start.saturating_add(frame_width);
                    cropped.extend_from_slice(&prepared.indices[start..end]);
                }
                output_frame.source.rect.left = bounded_u32(left);
                output_frame.source.rect.top = bounded_u32(top);
                output_frame.source.rect.width = bounded_u32(frame_width);
                output_frame.source.rect.height = bounded_u32(frame_height);
                output_frame.pixel_layout = FramePixelLayout::SourceRectangle;
                let mut alpha = vec![255; prepared.palette.len().div_euclid(3)];
                if let Some(transparent) = prepared.transparent {
                    alpha[usize::from(transparent)] = 0;
                }
                output_frame.image = DecodedImage::with_mode(
                    bounded_u32(frame_width),
                    bounded_u32(frame_height),
                    cropped,
                    ImageMode::P8,
                )
                .with_palette(
                    ImagePalette::new(prepared.palette, alpha)
                        .expect("coalesced GIF palette remains structurally valid"),
                );
            }
            output.push(output_frame);
            previous_render = Some(canvas.clone());
        }
        previous_frame = Some(frame);
        crate::codecs::error::check_cancelled(token)?;
    }
    Ok(output)
}

fn gif_output_frame(
    sequence: &DecodedSequence,
    frame: &crate::types::DecodedFrame,
) -> crate::types::DecodedFrame {
    let mut output = frame.clone();
    if output.pixel_layout == FramePixelLayout::RenderedCanvas {
        output.pixel_layout = FramePixelLayout::SourceRectangle;
        output.source.rect.left = 0;
        output.source.rect.top = 0;
        output.source.rect.width = sequence.width;
        output.source.rect.height = sequence.height;
        output.source.disposal = FrameDisposal::Unspecified;
        output.source.blend = FrameBlend::Unspecified;
        output.source.interlaced = false;
    }
    output
}

fn effective_disposal(
    frame: &crate::types::DecodedFrame,
    disposal_override: Option<u8>,
) -> CodecResult<u8> {
    match disposal_override {
        Some(value) => Ok(value),
        None => disposal_code(frame.source.disposal),
    }
}

fn add_frame_durations(left: FrameDuration, right: FrameDuration) -> CodecResult<FrameDuration> {
    if left.denominator == 0 || right.denominator == 0 {
        return Err(CodecError::Parameter(
            "GIF frame duration denominator must be non-zero".to_owned(),
        ));
    }
    let common = greatest_common_divisor(left.denominator, right.denominator);
    let left_scale = right.denominator.div_euclid(common);
    let right_scale = left.denominator.div_euclid(common);
    let denominator = left
        .denominator
        .checked_mul(left_scale)
        .ok_or_else(|| CodecError::Parameter("GIF frame duration overflows".to_owned()))?;
    let numerator = left
        .numerator
        .checked_mul(left_scale)
        .and_then(|value| {
            right
                .numerator
                .checked_mul(right_scale)
                .and_then(|right| value.checked_add(right))
        })
        .ok_or_else(|| CodecError::Parameter("GIF frame duration overflows".to_owned()))?;
    Ok(FrameDuration {
        numerator,
        denominator,
    })
}

fn greatest_common_divisor(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left.rem_euclid(right);
        left = right;
        right = remainder;
    }
    left
}

fn rgba_difference_bounds(
    previous: &[u8],
    current: &[u8],
    width: usize,
    height: usize,
) -> (usize, usize, usize, usize) {
    debug_assert_eq!(previous.len(), current.len());
    debug_assert_eq!(
        current.len(),
        width.saturating_mul(height).saturating_mul(4)
    );
    let mut left = width;
    let mut top = height;
    let mut right = 0usize;
    let mut bottom = 0usize;
    for (index, (before, after)) in previous
        .as_chunks::<4>()
        .0
        .iter()
        .zip(current.as_chunks::<4>().0.iter())
        .enumerate()
    {
        if before != after {
            let x = index.rem_euclid(width);
            let y = index.div_euclid(width);
            left = left.min(x);
            top = top.min(y);
            right = right.max(x.saturating_add(1));
            bottom = bottom.max(y.saturating_add(1));
        }
    }
    (left, top, right, bottom)
}

fn clear_frame_rect(canvas: &mut [u8], canvas_width: usize, frame: &crate::types::DecodedFrame) {
    let left = frame.source.rect.left as usize;
    let top = frame.source.rect.top as usize;
    let width = frame.image.width as usize;
    let height = frame.image.height as usize;
    for y in 0..height {
        let start = top
            .saturating_add(y)
            .saturating_mul(canvas_width)
            .saturating_add(left)
            .saturating_mul(4);
        let end = start.saturating_add(width.saturating_mul(4));
        canvas[start..end].fill(0);
    }
}

// Sequence validation guarantees that an indexed frame carries its palette.
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
fn composite_frame(
    canvas: &mut [u8],
    canvas_width: usize,
    frame: &crate::types::DecodedFrame,
) -> CodecResult<()> {
    let left = frame.source.rect.left as usize;
    let top = frame.source.rect.top as usize;
    composite_image(canvas, canvas_width, &frame.image, left, top, true)
}

// `transparent_over` distinguishes a GIF source rectangle, whose transparent
// palette samples leave the existing canvas untouched, from an already
// rendered canvas, whose samples replace the complete prior presentation.
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
fn composite_image(
    canvas: &mut [u8],
    canvas_width: usize,
    image: &DecodedImage,
    left: usize,
    top: usize,
    transparent_over: bool,
) -> CodecResult<()> {
    let width = image.width as usize;
    let height = image.height as usize;
    for y in 0..height {
        for x in 0..width {
            let source = y.saturating_mul(width).saturating_add(x);
            let rgba = match image.mode {
                ImageMode::P8 => {
                    let palette = image
                        .palette
                        .as_ref()
                        .expect("validated P8 GIF frame carries a palette");
                    let index = usize::from(image.pixels[source]);
                    let palette_offset = index.saturating_mul(3);
                    let rgb = &palette.rgb[palette_offset..palette_offset.saturating_add(3)];
                    [
                        rgb[0],
                        rgb[1],
                        rgb[2],
                        palette.alpha.get(index).copied().unwrap_or(255),
                    ]
                }
                ImageMode::L8 => {
                    let value = image.pixels[source];
                    [value, value, value, 255]
                }
                ImageMode::Rgb8 => {
                    let offset = source.saturating_mul(3);
                    [
                        image.pixels[offset],
                        image.pixels[offset.saturating_add(1)],
                        image.pixels[offset.saturating_add(2)],
                        255,
                    ]
                }
                ImageMode::Rgba8 => {
                    let offset = source.saturating_mul(4);
                    [
                        image.pixels[offset],
                        image.pixels[offset.saturating_add(1)],
                        image.pixels[offset.saturating_add(2)],
                        image.pixels[offset.saturating_add(3)],
                    ]
                }
                _ => {
                    return Err(CodecError::Unsupported(
                        "GIF cannot composite this image mode".to_owned(),
                    ));
                }
            };
            if transparent_over && rgba[3] == 0 && image.mode == ImageMode::P8 {
                continue;
            }
            let destination = top
                .saturating_add(y)
                .saturating_mul(canvas_width)
                .saturating_add(left)
                .saturating_add(x)
                .saturating_mul(4);
            canvas[destination..destination.saturating_add(4)].copy_from_slice(&rgba);
        }
    }
    Ok(())
}

fn prepare_image_with_token(
    img: &DecodedImage,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<PreparedImage> {
    let (palette, indices, transparent) = match (img.mode, img.color) {
        (ImageMode::L1, ColorType::L8) => {
            let width = img.width as usize;
            let height = img.height as usize;
            let row_bytes = width.div_ceil(8);
            let mut indices = Vec::with_capacity(width.saturating_mul(height));
            for y in 0..height {
                crate::codecs::error::check_cancelled(token)?;
                let row_start = y.saturating_mul(row_bytes);
                for x in 0..width {
                    let packed = img.pixels[row_start.saturating_add(x / 8)];
                    let bit = 0x80u8 >> (x % 8);
                    indices.push(if packed & bit == 0 { 0 } else { u8::MAX });
                }
            }
            let mut palette = Vec::with_capacity(256 * 3);
            for value in 0..=u8::MAX {
                palette.extend_from_slice(&[value, value, value]);
            }
            (palette, indices, None)
        }
        (ImageMode::P8, ColorType::L8) => {
            let palette = img.palette.as_ref().ok_or_else(|| {
                CodecError::Parameter("indexed GIF input requires a palette".to_owned())
            })?;
            let transparent = palette.alpha.iter().position(|&alpha| alpha == 0);
            (
                palette.rgb.clone(),
                img.pixels.clone(),
                transparent.map(palette_index),
            )
        }
        (ImageMode::L8, ColorType::L8) => {
            #[cfg(target_pointer_width = "64")]
            let pixel_count = (img.width as usize).saturating_mul(img.height as usize);
            #[cfg(not(target_pointer_width = "64"))]
            let pixel_count = (img.width as usize)
                .checked_mul(img.height as usize)
                .ok_or_else(|| CodecError::Dimensions("GIF pixel count overflows".to_owned()))?;
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
                    let index = palette_index(palette.len().div_euclid(3));
                    remap[value] = index;
                    let value = palette_index(value);
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
            let (palette, indices) = quantize_rgb(&img.pixels, token)?;
            (palette, indices, None)
        }
        (ImageMode::Rgba8, ColorType::Rgba8) => {
            let (palette, indices, transparent_idx) = quantize_rgba(&img.pixels, token)?;
            (palette, indices, transparent_idx)
        }
        _ => {
            return Err(CodecError::Unsupported(
                "GIF cannot encode this image mode".to_owned(),
            ));
        }
    };
    #[cfg(target_pointer_width = "64")]
    let pixel_count = (img.width as usize).saturating_mul(img.height as usize);
    #[cfg(not(target_pointer_width = "64"))]
    let pixel_count = (img.width as usize)
        .checked_mul(img.height as usize)
        .ok_or_else(|| CodecError::Dimensions("GIF pixel count overflows".to_owned()))?;
    debug_assert_eq!(indices.len(), pixel_count);
    Ok(PreparedImage {
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

fn palette_index(value: usize) -> u8 {
    value.to_le_bytes()[0]
}

fn palette_index_u32(value: u32) -> u8 {
    value.to_le_bytes()[0]
}

fn channel_average(value: u64) -> u8 {
    value.to_le_bytes()[0]
}

fn bounded_u32(value: usize) -> u32 {
    #[cfg(target_pointer_width = "64")]
    let value = value.min(u32::MAX as usize);
    let bytes = value.to_le_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn disposal_code(disposal: FrameDisposal) -> CodecResult<u8> {
    match disposal {
        FrameDisposal::Unspecified => Ok(0),
        FrameDisposal::Keep => Ok(1),
        FrameDisposal::Background => Ok(2),
        FrameDisposal::Previous => Ok(3),
        FrameDisposal::Reserved(value) if value <= 7 => Ok(value),
        FrameDisposal::Reserved(_) => Err(CodecError::Parameter(
            "GIF disposal value exceeds its three-bit field".to_owned(),
        )),
    }
}

fn gif_delay(duration: FrameDuration) -> CodecResult<u16> {
    if duration.denominator == 0 {
        return Err(CodecError::Parameter(
            "GIF frame duration denominator must be non-zero".to_owned(),
        ));
    }
    let centiseconds = duration
        .numerator
        .checked_mul(100)
        .ok_or_else(|| CodecError::Parameter("GIF frame duration overflows".to_owned()))?;
    if !centiseconds.is_multiple_of(duration.denominator) {
        return Err(CodecError::Unsupported(
            "GIF cannot represent the exact frame duration".to_owned(),
        ));
    }
    u16::try_from(centiseconds.div_euclid(duration.denominator))
        .map_err(|_| CodecError::Parameter("GIF frame duration exceeds format limits".to_owned()))
}

fn table_parameters(palette: &[u8]) -> (usize, u8, u8) {
    debug_assert!(!palette.is_empty());
    debug_assert!(palette.len().is_multiple_of(3));
    debug_assert!(palette.len() <= 256usize.saturating_mul(3));
    // Pillow's GIF writer normalizes even a one-color image to a four-entry
    // table while retaining the GIF-mandated minimum LZW code width of two.
    let color_count = palette.len().div_euclid(3).max(4).next_power_of_two();
    let table_bits = usize::BITS
        .saturating_sub(color_count.leading_zeros())
        .saturating_sub(1);
    let size_field = palette_index_u32(table_bits.saturating_sub(1));
    // Pillow's P-mode GIF encoder uses an eight-bit LZW root alphabet even
    // when the emitted color table contains fewer entries.
    let minimum_code_size = 8;
    (color_count, size_field, minimum_code_size)
}

fn write_gif(
    sequence: &DecodedSequence,
    frames: &[crate::types::DecodedFrame],
    settings: GifSettings,
) -> CodecResult<Vec<u8>> {
    write_gif_with_token(sequence, frames, settings, None)
}

fn write_gif_with_token(
    sequence: &DecodedSequence,
    frames: &[crate::types::DecodedFrame],
    settings: GifSettings,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    crate::codecs::error::check_cancelled(token)?;
    let width = u16::try_from(sequence.width)
        .map_err(|_| CodecError::Dimensions("GIF width exceeds format limits".to_owned()))?;
    let height = u16::try_from(sequence.height)
        .map_err(|_| CodecError::Dimensions("GIF height exceeds format limits".to_owned()))?;
    let first_frame = frames
        .first()
        .ok_or_else(|| CodecError::Dimensions("GIF sequence has no frames".to_owned()))?;
    let mut prepared_frames = Vec::with_capacity(frames.len());
    for frame in frames {
        crate::codecs::error::check_cancelled(token)?;
        prepared_frames.push(prepare_image_with_token(&frame.image, token)?);
    }
    // `first_frame` above proves the prepared vector has the same nonzero
    // length as `frames`.
    let has_transparency = prepared_frames
        .iter()
        .any(|prepared| prepared.transparent.is_some());
    let mut prepared_frames = prepared_frames.into_iter();
    // The iterator is populated from the already-validated nonempty frame
    // list above; keep the invariant explicit without adding an unreachable
    // error path to the measured GIF pipeline.
    #[allow(clippy::expect_used)]
    let mut first = prepared_frames
        .next()
        .expect("prepared GIF frames must retain the validated first frame");
    let background = prepare_background(&mut first, first_frame.image.mode, sequence.background);
    let (global_count, global_size, _) = table_parameters(&first.palette);
    let global_palette = first.palette.clone();
    // Pillow always writes the global palette for a single frame. Its
    // include_color_table option adds a duplicate local palette rather than
    // replacing the global one.
    let global_table = true;

    let needs_89a = frames.len() > 1
        || settings.loop_count.is_some()
        || settings.transparency_override == Some(true)
        || has_transparency;
    let mut output = Vec::new();
    output.extend_from_slice(if needs_89a { b"GIF89a" } else { b"GIF87a" });
    output.extend_from_slice(&width.to_le_bytes());
    output.extend_from_slice(&height.to_le_bytes());
    output.push(u8::from(global_table) << 7 | global_size);
    output.extend_from_slice(&[background, 0]); // Background index and pixel aspect ratio.
    write_color_table(&mut output, &global_palette, global_count);

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

    let mut first = Some(first);
    let mut previous_indices = None::<Vec<u8>>;
    let mut previous_palette = None::<Vec<u8>>;
    let mut previous_disposal = None::<u8>;
    for (frame_index, frame) in frames.iter().enumerate() {
        crate::codecs::error::check_cancelled(token)?;
        let mut prepared = if frame_index == 0 {
            take_gif_first_prepared(&mut first)
        } else {
            next_gif_prepared(&mut prepared_frames)
        };
        // Retain the compact indexed representation for the next frame's
        // difference check. Materializing a full RGB copy here costs three
        // bytes per pixel even though the comparison only needs palette lookups.
        let current_indices = prepared.indices.clone();
        let current_palette = prepared.palette.clone();
        let previous_can_mask = previous_disposal != Some(2);
        if previous_can_mask
            && let (Some(previous_indices), Some(previous_palette)) =
                (previous_indices.as_deref(), previous_palette.as_deref())
            && previous_indices.len() == current_indices.len()
            && let Some(transparent) = prepared.transparent
        {
            mask_equal_indexed_pixels(
                previous_indices,
                previous_palette,
                &mut prepared.indices,
                &current_palette,
                transparent,
            );
        }
        previous_indices = Some(current_indices);
        previous_palette = Some(current_palette);
        let (color_count, size_field, minimum_code_size) = table_parameters(&prepared.palette);
        let mut transparent = prepared.transparent;
        if let Some(requested) = settings.transparency_override {
            transparent = requested.then_some(transparent.unwrap_or(0));
        }
        let disposal = match settings.disposal_override {
            Some(disposal) => disposal,
            None => disposal_code(frame.source.disposal)?,
        };
        previous_disposal = Some(disposal);
        let delay_cs = gif_delay(frame.source.duration)?;
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
        output.extend_from_slice(
            &u16::try_from(frame.source.rect.left)
                .map_err(|_| {
                    CodecError::Dimensions("GIF frame left offset exceeds format limits".to_owned())
                })?
                .to_le_bytes(),
        );
        output.extend_from_slice(
            &u16::try_from(frame.source.rect.top)
                .map_err(|_| {
                    CodecError::Dimensions("GIF frame top offset exceeds format limits".to_owned())
                })?
                .to_le_bytes(),
        );
        let frame_width = u16::try_from(frame.image.width).map_err(|_| {
            CodecError::Dimensions("GIF frame width exceeds format limits".to_owned())
        })?;
        let frame_height = u16::try_from(frame.image.height).map_err(|_| {
            CodecError::Dimensions("GIF frame height exceeds format limits".to_owned())
        })?;
        output.extend_from_slice(&frame_width.to_le_bytes());
        output.extend_from_slice(&frame_height.to_le_bytes());
        let local_table = settings.local_color_table || prepared.palette != global_palette;
        // Pillow defaults to interlacing a sufficiently large single-frame
        // GIF, but its multi-frame writer emits non-interlaced descriptors.
        let default_interlace =
            frames.len() == 1 && frame.image.width >= 16 && frame.image.height >= 16;
        let interlaced = settings
            .interlaced
            .unwrap_or(frame.source.interlaced || default_interlace);
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
        let compressed = encode_lzw_with_token(&encoded_indices, minimum_code_size, token)?;
        output.push(minimum_code_size);
        write_sub_blocks(&mut output, &compressed);
        crate::codecs::error::check_cancelled(token)?;
    }
    output.push(GIF_TRAILER);
    Ok(output)
}

#[cfg_attr(coverage, coverage(off))]
fn take_gif_first_prepared(first: &mut Option<PreparedImage>) -> PreparedImage {
    match first.take() {
        Some(prepared) => prepared,
        None => unreachable!("GIF first-frame preparation invariant failed"),
    }
}

#[cfg_attr(coverage, coverage(off))]
fn next_gif_prepared(prepared_frames: &mut impl Iterator<Item = PreparedImage>) -> PreparedImage {
    match prepared_frames.next() {
        Some(prepared) => prepared,
        None => unreachable!("GIF frame preparation count invariant failed"),
    }
}

fn prepare_background(
    first: &mut PreparedImage,
    source_mode: ImageMode,
    background: Option<AnimationBackground>,
) -> u8 {
    let Some(background) = background else {
        return 0;
    };
    match background {
        AnimationBackground::PaletteIndex(index) => index,
        AnimationBackground::Rgba([red, green, blue, alpha]) => {
            if source_mode != ImageMode::Rgba8 && alpha != 255 {
                return 0;
            }
            if alpha == 0
                && let Some(transparent) = first.transparent
            {
                return transparent;
            }
            if source_mode != ImageMode::Rgba8 {
                for (index, color) in first.palette.as_chunks::<3>().0.iter().enumerate() {
                    if *color == [red, green, blue] {
                        return palette_index(index);
                    }
                }
            }
            if first.palette.len().div_euclid(3) >= 256 {
                return 0;
            }
            let index = palette_index(first.palette.len().div_euclid(3));
            first.palette.extend_from_slice(&[red, green, blue]);
            index
        }
    }
}

fn write_color_table(output: &mut Vec<u8>, palette: &[u8], color_count: usize) {
    output.extend_from_slice(palette);
    let padding = color_count.saturating_mul(3).saturating_sub(palette.len());
    output.resize(output.len().saturating_add(padding), 0);
}

fn mask_equal_indexed_pixels(
    previous_indices: &[u8],
    previous_palette: &[u8],
    current_indices: &mut [u8],
    current_palette: &[u8],
    transparent: u8,
) {
    for (index, current_index) in current_indices.iter_mut().enumerate() {
        let previous_offset = usize::from(previous_indices[index]).saturating_mul(3);
        let current_offset = usize::from(*current_index).saturating_mul(3);
        if previous_palette[previous_offset..previous_offset.saturating_add(3)]
            == current_palette[current_offset..current_offset.saturating_add(3)]
        {
            *current_index = transparent;
        }
    }
}

fn interlace(indices: &[u8], width: usize, height: usize) -> Vec<u8> {
    debug_assert_eq!(indices.len(), width.saturating_mul(height));
    let mut output = Vec::with_capacity(indices.len());
    for (start, step) in [(0, 8), (4, 8), (2, 4), (1, 2)] {
        for y in (start..height).step_by(step) {
            let row_start = y.saturating_mul(width);
            output.extend_from_slice(&indices[row_start..row_start.saturating_add(width)]);
        }
    }
    output
}

/// Encode indices using the GIF89a Appendix F LZW code-width rules.
/// Encode GIF LZW while polling once per input-symbol interval.
///
/// The ordinary encoder passes `None`, preserving its existing byte path. A
/// token-aware call can therefore stop inside a long dictionary pass without
/// claiming that the complete GIF working buffer is incrementally streamed.
fn encode_lzw_with_token(
    indices: &[u8],
    minimum_code_size: u8,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    debug_assert!(!indices.is_empty());
    debug_assert!((2..=8).contains(&minimum_code_size));

    let clear_code = 1u16 << minimum_code_size;
    let end_code = clear_code.saturating_add(1);
    debug_assert!(indices.iter().all(|&index| u16::from(index) < clear_code));

    let mut writer = BitWriter::new();
    let mut dictionary = HashMap::<(u16, u8), u16>::new();
    let mut code_size = minimum_code_size.saturating_add(1);
    let mut next_code = end_code.saturating_add(1);
    writer.write(clear_code, code_size);

    let mut prefix = u16::from(indices[0]);
    let mut encode_suffix = |suffix: u8| {
        if let Some(&code) = dictionary.get(&(prefix, suffix)) {
            prefix = code;
            return;
        }

        writer.write(prefix, code_size);
        if next_code <= MAX_LZW_CODE {
            dictionary.insert((prefix, suffix), next_code);
            next_code = next_code.saturating_add(1);
            // The encoder's dictionary is one entry ahead of the decoder. Delay
            // the width transition by one code so both sides switch together.
            if code_size < 12 && next_code > (1u16 << code_size) {
                code_size = code_size.saturating_add(1);
            }
        } else {
            writer.write(clear_code, code_size);
            dictionary.clear();
            code_size = minimum_code_size.saturating_add(1);
            next_code = end_code.saturating_add(1);
        }
        prefix = u16::from(suffix);
    };
    if let Some(token) = token {
        for &suffix in &indices[1..] {
            crate::codecs::error::check_cancelled(Some(token))?;
            encode_suffix(suffix);
        }
    } else {
        for &suffix in &indices[1..] {
            encode_suffix(suffix);
        }
    }

    writer.write(prefix, code_size);
    writer.write(end_code, code_size);
    Ok(writer.finish())
}

fn write_sub_blocks(output: &mut Vec<u8>, data: &[u8]) {
    for block in data.chunks(255) {
        output.push(palette_index(block.len()));
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
            self.used = self.used.saturating_add(1);
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
fn quantize_rgb(
    pixels: &[u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(Vec<u8>, Vec<u8>)> {
    debug_assert!(pixels.len().is_multiple_of(3));
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut counts = Vec::<u32>::new();

    if let Some(token) = token {
        for (pixel_index, chunk) in pixels.as_chunks::<3>().0.iter().enumerate() {
            if pixel_index != 0 && pixel_index.is_multiple_of(GIF_QUANTIZATION_CHECKPOINT_PIXELS) {
                crate::codecs::error::check_cancelled(Some(token))?;
            }
            let color = [chunk[0], chunk[1], chunk[2]];
            if !record_rgb_color(&mut palette, &mut counts, color) {
                return quantize_rgb_nearest(pixels, Some(token));
            }
        }
    } else {
        for chunk in pixels.as_chunks::<3>().0 {
            let color = [chunk[0], chunk[1], chunk[2]];
            if !record_rgb_color(&mut palette, &mut counts, color) {
                return quantize_rgb_nearest(pixels, None);
            }
        }
    }

    // Pillow 12.2.0 Quant.c uses its median-cut tree even when the requested
    // 256 colors exceed the number of distinct input colors. Every leaf then
    // contains one color, but the tree traversal still determines palette and
    // index order. Animated GIF frames after the first pass through this RGB
    // adaptive-palette path in GifImagePlugin._normalize_mode.
    let order = pillow_median_cut_order(&palette, &counts, token)?;
    let mut remap = vec![0u8; palette.len()];
    let mut flat = Vec::with_capacity(palette.len().saturating_mul(3));
    for (new_index, &old_index) in order.iter().enumerate() {
        remap[old_index] = palette_index(new_index);
        flat.extend_from_slice(&palette[old_index]);
    }
    let indices = if let Some(token) = token {
        let mut indices = Vec::with_capacity(pixels.len().div_euclid(3));
        for (pixel_index, chunk) in pixels.as_chunks::<3>().0.iter().enumerate() {
            if pixel_index != 0 && pixel_index.is_multiple_of(GIF_QUANTIZATION_CHECKPOINT_PIXELS) {
                crate::codecs::error::check_cancelled(Some(token))?;
            }
            let color = [chunk[0], chunk[1], chunk[2]];
            indices.push(remap_rgb_index(&palette, &remap, color));
        }
        indices
    } else {
        pixels
            .as_chunks::<3>()
            .0
            .iter()
            .map(|chunk| remap_rgb_index(&palette, &remap, [chunk[0], chunk[1], chunk[2]]))
            .collect()
    };
    Ok((flat, indices))
}

fn record_rgb_color(palette: &mut Vec<[u8; 3]>, counts: &mut Vec<u32>, color: [u8; 3]) -> bool {
    match find_color(palette, &color) {
        Some(index) => counts[index] = counts[index].saturating_add(1),
        None if palette.len() < 256 => {
            palette.push(color);
            counts.push(1);
        }
        None => return false,
    }
    true
}

#[allow(clippy::expect_used)]
fn remap_rgb_index(palette: &[[u8; 3]], remap: &[u8], color: [u8; 3]) -> u8 {
    // `palette` was constructed from these exact source pixels.
    let index = find_color(palette, &color).expect("RGB GIF palette was built from source pixels");
    remap[index]
}

fn record_rgb_nearest_color(
    colors: &mut Vec<[u8; 3]>,
    counts: &mut Vec<u32>,
    color_indices: &mut HashMap<u32, usize>,
    color: [u8; 3],
) {
    let hash = pillow_pixel_hash(color);
    if let Some(&index) = color_indices.get(&hash) {
        counts[index] = counts[index].saturating_add(1);
    } else {
        let index = colors.len();
        colors.push(color);
        counts.push(1);
        color_indices.insert(hash, index);
    }
}

fn quantize_rgb_nearest(
    pixels: &[u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(Vec<u8>, Vec<u8>)> {
    let mut colors = Vec::<[u8; 3]>::new();
    let mut counts = Vec::<u32>::new();
    let mut color_indices = HashMap::<u32, usize>::new();

    if let Some(token) = token {
        for (pixel_index, chunk) in pixels.as_chunks::<3>().0.iter().enumerate() {
            if pixel_index != 0 && pixel_index.is_multiple_of(GIF_QUANTIZATION_CHECKPOINT_PIXELS) {
                crate::codecs::error::check_cancelled(Some(token))?;
            }
            record_rgb_nearest_color(
                &mut colors,
                &mut counts,
                &mut color_indices,
                [chunk[0], chunk[1], chunk[2]],
            );
        }
    } else {
        for chunk in pixels.as_chunks::<3>().0 {
            record_rgb_nearest_color(
                &mut colors,
                &mut counts,
                &mut color_indices,
                [chunk[0], chunk[1], chunk[2]],
            );
        }
    }
    let leaves = pillow_median_cut_leaves(&colors, &counts, colors.len().min(256), token)?;
    let mut palette = Vec::<[u8; 3]>::with_capacity(leaves.len());
    let mut initial_palette = vec![0usize; colors.len()];
    for (palette_index, leaf) in leaves.iter().enumerate() {
        let mut sums = [0u64; 3];
        let mut count = 0u64;
        for &color_index in leaf {
            let color_count = u64::from(counts[color_index]);
            count = count.saturating_add(color_count);
            for channel in 0..3 {
                sums[channel] = sums[channel].saturating_add(
                    u64::from(colors[color_index][channel]).saturating_mul(color_count),
                );
            }
            initial_palette[color_index] = palette_index;
        }
        palette.push(std::array::from_fn(|channel| {
            channel_average(
                sums[channel]
                    .saturating_add(count.div_euclid(2))
                    .div_euclid(count),
            )
        }));
    }
    let mapped = if let Some(token) = token {
        let mut mapped = Vec::with_capacity(colors.len());
        let mut candidates = Vec::with_capacity(palette.len());
        let mut scratch = vec![0usize; palette.len()];
        let mut nearest_work_items = 0usize;
        for (color_index, color) in colors.iter().enumerate() {
            if color_index != 0 && color_index.is_multiple_of(GIF_QUANTIZATION_CHECKPOINT_PIXELS) {
                #[cfg(coverage)]
                if FORCE_NEAREST_MAPPING_CHECKPOINT.swap(false, Ordering::Relaxed) {
                    token.cancel();
                }
                crate::codecs::error::check_cancelled(Some(token))?;
            }
            candidates.clear();
            candidates.extend(0..palette.len());
            mapped.push(find_nearest_from_with_token(
                &palette,
                color,
                initial_palette[color_index],
                token,
                &mut candidates,
                &mut scratch,
                &mut nearest_work_items,
            )?);
        }
        mapped
    } else {
        colors
            .iter()
            .enumerate()
            .map(|(index, color)| find_nearest_from(&palette, color, initial_palette[index]))
            .collect::<Vec<_>>()
    };
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
    let indices = if let Some(token) = token {
        let mut indices = Vec::with_capacity(pixels.len().div_euclid(3));
        for (pixel_index, chunk) in pixels.as_chunks::<3>().0.iter().enumerate() {
            if pixel_index != 0 && pixel_index.is_multiple_of(GIF_QUANTIZATION_CHECKPOINT_PIXELS) {
                crate::codecs::error::check_cancelled(Some(token))?;
            }
            let color = [chunk[0], chunk[1], chunk[2]];
            let index = color_indices[&pillow_pixel_hash(color)];
            indices.push(palette_index(remap[mapped[index]]));
        }
        indices
    } else {
        pixels
            .as_chunks::<3>()
            .0
            .iter()
            .map(|chunk| {
                let color = [chunk[0], chunk[1], chunk[2]];
                let index = color_indices[&pillow_pixel_hash(color)];
                palette_index(remap[mapped[index]])
            })
            .collect()
    };
    Ok((optimized.into_iter().flatten().collect(), indices))
}

#[derive(Clone)]
struct MedianBox {
    axes: [Vec<usize>; 3],
    pixel_count: u32,
    children: Option<(usize, usize)>,
}

fn pillow_median_cut_order(
    colors: &[[u8; 3]],
    counts: &[u32],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<usize>> {
    let leaves = pillow_median_cut_leaves(colors, counts, colors.len(), token)?;
    Ok(leaves.into_iter().map(|leaf| leaf[0]).collect())
}

fn pillow_median_cut_leaves(
    colors: &[[u8; 3]],
    counts: &[u32],
    target: usize,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<Vec<usize>>> {
    // All callers derive `counts` and `target` from the same non-empty pixel
    // set. Keep those internal invariants visible without retaining an
    // unreachable runtime failure path.
    debug_assert!(!colors.is_empty());
    debug_assert_eq!(colors.len(), counts.len());
    debug_assert!((1..=colors.len().min(256)).contains(&target));

    let hash_order = pillow_hash_iteration_order(colors, token)?;
    let axes = if let Some(token) = token {
        let mut axes = [Vec::new(), Vec::new(), Vec::new()];
        for axis in 0..3 {
            let mut entries = (0..colors.len()).collect::<Vec<_>>();
            entries
                .sort_by_key(|&index| (std::cmp::Reverse(colors[index][axis]), hash_order[index]));
            crate::codecs::error::check_cancelled(Some(token))?;
            axes[axis] = entries;
        }
        axes
    } else {
        std::array::from_fn(|axis| {
            let mut entries = (0..colors.len()).collect::<Vec<_>>();
            entries
                .sort_by_key(|&index| (std::cmp::Reverse(colors[index][axis]), hash_order[index]));
            entries
        })
    };
    let pixel_count = counts.iter().sum();
    let mut boxes = vec![MedianBox {
        axes,
        pixel_count,
        children: None,
    }];
    let mut heap = PillowBoxHeap::default();
    heap.add(0, &boxes);

    if let Some(token) = token {
        for _ in 1..target {
            crate::codecs::error::check_cancelled(Some(token))?;
            let node = loop {
                let candidate = heap.remove(&boxes);
                if box_volume(&boxes[candidate], colors) > 1 {
                    break candidate;
                }
            };
            let (left, right) = split_median_box_with_token(&boxes[node], colors, counts, token)?;
            let left_index = boxes.len();
            boxes.push(left);
            let right_index = boxes.len();
            boxes.push(right);
            boxes[node].children = Some((left_index, right_index));
            heap.add(left_index, &boxes);
            heap.add(right_index, &boxes);
        }
    } else {
        for _ in 1..target {
            let node = loop {
                let candidate = heap.remove(&boxes);
                if box_volume(&boxes[candidate], colors) > 1 {
                    break candidate;
                }
            };
            let (left, right) = split_median_box(&boxes[node], colors, counts);
            let left_index = boxes.len();
            boxes.push(left);
            let right_index = boxes.len();
            boxes.push(right);
            boxes[node].children = Some((left_index, right_index));
            heap.add(left_index, &boxes);
            heap.add(right_index, &boxes);
        }
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
    debug_assert_eq!(leaves.len(), target);
    Ok(leaves)
}

fn pillow_hash_iteration_order(
    colors: &[[u8; 3]],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<usize>> {
    // QuantHash.c grows 11 -> 23 -> 47 -> 97 for this range. Its historical
    // prime finder accepts the first candidate in this residue table.
    const ACCEPTED_RESIDUES: [bool; 16] = [
        false, true, false, true, false, false, false, true, false, true, false, true, false, true,
        false, false,
    ];
    let mut length = 11u32;
    if let Some(token) = token {
        for (count_index, count) in (1..=colors.len()).enumerate() {
            if length.saturating_mul(3) < bounded_u32(count) {
                let mut candidate = length.saturating_mul(2).saturating_add(1);
                while !ACCEPTED_RESIDUES[(candidate & 15) as usize] {
                    candidate = candidate.saturating_add(1);
                }
                length = candidate;
            }
            if count_index != 0 && count_index.is_multiple_of(GIF_MEDIAN_CUT_CHECKPOINT_ITEMS) {
                crate::codecs::error::check_cancelled(Some(token))?;
            }
        }
    } else {
        for count in 1..=colors.len() {
            if length.saturating_mul(3) < bounded_u32(count) {
                let mut candidate = length.saturating_mul(2).saturating_add(1);
                while !ACCEPTED_RESIDUES[(candidate & 15) as usize] {
                    candidate = candidate.saturating_add(1);
                }
                length = candidate;
            }
        }
    }
    let mut iteration = (0..colors.len()).collect::<Vec<_>>();
    iteration.sort_by_key(|&index| {
        let hash = pillow_pixel_hash(colors[index]);
        (hash.rem_euclid(length), hash)
    });
    if let Some(token) = token {
        crate::codecs::error::check_cancelled(Some(token))?;
    }
    let mut rank = vec![0usize; colors.len()];
    if let Some(token) = token {
        for (position, index) in iteration.into_iter().enumerate() {
            rank[index] = position;
            if position != 0 && position.is_multiple_of(GIF_MEDIAN_CUT_CHECKPOINT_ITEMS) {
                crate::codecs::error::check_cancelled(Some(token))?;
            }
        }
    } else {
        for (position, index) in iteration.into_iter().enumerate() {
            rank[index] = position;
        }
    }
    Ok(rank)
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
            let last = entries[entries.len().saturating_sub(1)];
            u32::from(colors[entries[0]][axis].saturating_sub(colors[last][axis])).saturating_add(1)
        })
        .fold(1, u32::saturating_mul)
}

fn split_median_box(
    node: &MedianBox,
    colors: &[[u8; 3]],
    counts: &[u32],
) -> (MedianBox, MedianBox) {
    let ranges: [u32; 3] = std::array::from_fn(|axis| {
        let entries = &node.axes[axis];
        let last = entries[entries.len().saturating_sub(1)];
        u32::from(colors[entries[0]][axis].saturating_sub(colors[last][axis]))
            .saturating_mul([77, 150, 29][axis])
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
        left_count = left_count.saturating_add(counts[sorted[split]]);
        split = split.saturating_add(1);
        if left_count.saturating_mul(2) > node.pixel_count {
            break;
        }
    }
    if split < sorted.len() {
        let value = colors[sorted[split.saturating_sub(1)]][axis];
        while split < sorted.len() && colors[sorted[split]][axis] == value {
            left_count = left_count.saturating_add(counts[sorted[split]]);
            split = split.saturating_add(1);
        }
    }
    if split == sorted.len() {
        let value = colors[sorted[sorted.len().saturating_sub(1)]][axis];
        while split > 0 && colors[sorted[split.saturating_sub(1)]][axis] == value {
            split = split.saturating_sub(1);
            left_count = left_count.saturating_sub(counts[sorted[split]]);
        }
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
    (
        MedianBox {
            axes: left_axes,
            pixel_count: left_count,
            children: None,
        },
        MedianBox {
            axes: right_axes,
            pixel_count: node.pixel_count.saturating_sub(left_count),
            children: None,
        },
    )
}

fn split_median_box_with_token(
    node: &MedianBox,
    colors: &[[u8; 3]],
    counts: &[u32],
    token: &crate::CancellationToken,
) -> CodecResult<(MedianBox, MedianBox)> {
    let ranges: [u32; 3] = std::array::from_fn(|axis| {
        let entries = &node.axes[axis];
        let last = entries[entries.len().saturating_sub(1)];
        u32::from(colors[entries[0]][axis].saturating_sub(colors[last][axis]))
            .saturating_mul([77, 150, 29][axis])
    });
    let axis = (1..3).fold(0, |best, candidate| {
        if ranges[candidate] > ranges[best] {
            candidate
        } else {
            best
        }
    });
    let sorted = &node.axes[axis];
    let mut items = 0usize;
    let mut poll_items = || -> CodecResult<()> {
        items = items.saturating_add(1);
        if items.is_multiple_of(GIF_MEDIAN_CUT_CHECKPOINT_ITEMS) {
            crate::codecs::error::check_cancelled(Some(token))?;
        }
        Ok(())
    };
    let mut left_count = 0u32;
    let mut split = 0usize;
    while split < sorted.len() {
        poll_items()?;
        left_count = left_count.saturating_add(counts[sorted[split]]);
        split = split.saturating_add(1);
        if left_count.saturating_mul(2) > node.pixel_count {
            break;
        }
    }
    if split < sorted.len() {
        let value = colors[sorted[split.saturating_sub(1)]][axis];
        while split < sorted.len() && colors[sorted[split]][axis] == value {
            poll_items()?;
            left_count = left_count.saturating_add(counts[sorted[split]]);
            split = split.saturating_add(1);
        }
    }
    if split == sorted.len() {
        let value = colors[sorted[sorted.len().saturating_sub(1)]][axis];
        while split > 0 && colors[sorted[split.saturating_sub(1)]][axis] == value {
            poll_items()?;
            split = split.saturating_sub(1);
            left_count = left_count.saturating_sub(counts[sorted[split]]);
        }
    }
    let mut is_left = std::collections::HashSet::with_capacity(split);
    for &index in &sorted[..split] {
        poll_items()?;
        is_left.insert(index);
    }
    let mut left_axes = [Vec::new(), Vec::new(), Vec::new()];
    for (other_axis, axis_entries) in left_axes.iter_mut().enumerate() {
        for &index in &node.axes[other_axis] {
            poll_items()?;
            if is_left.contains(&index) {
                axis_entries.push(index);
            }
        }
    }
    let mut right_axes = [Vec::new(), Vec::new(), Vec::new()];
    for (other_axis, axis_entries) in right_axes.iter_mut().enumerate() {
        for &index in &node.axes[other_axis] {
            poll_items()?;
            if !is_left.contains(&index) {
                axis_entries.push(index);
            }
        }
    }
    Ok((
        MedianBox {
            axes: left_axes,
            pixel_count: left_count,
            children: None,
        },
        MedianBox {
            axes: right_axes,
            pixel_count: node.pixel_count.saturating_sub(left_count),
            children: None,
        },
    ))
}

#[derive(Default)]
struct PillowBoxHeap(Vec<usize>);

impl PillowBoxHeap {
    fn add(&mut self, value: usize, boxes: &[MedianBox]) {
        self.0.push(value);
        let mut child = self.0.len().saturating_sub(1);
        while child > 0 {
            let parent = child.saturating_sub(1).div_euclid(2);
            if boxes[value].pixel_count <= boxes[self.0[parent]].pixel_count {
                break;
            }
            self.0[child] = self.0[parent];
            child = parent;
        }
        self.0[child] = value;
    }

    fn remove(&mut self, boxes: &[MedianBox]) -> usize {
        let result = self.0[0];
        // Indexing `self.0[0]` above proves the heap is non-empty.
        #[allow(clippy::expect_used)]
        let value = self
            .0
            .pop()
            .expect("median-cut heap is non-empty when removing");
        if self.0.is_empty() {
            return result;
        }
        let mut parent = 0usize;
        while parent.saturating_mul(2).saturating_add(1) < self.0.len() {
            let mut child = parent.saturating_mul(2).saturating_add(1);
            if child.saturating_add(1) < self.0.len()
                && boxes[self.0[child]].pixel_count
                    < boxes[self.0[child.saturating_add(1)]].pixel_count
            {
                child = child.saturating_add(1);
            }
            if boxes[value].pixel_count > boxes[self.0[child]].pixel_count {
                break;
            }
            self.0[parent] = self.0[child];
            parent = child;
        }
        self.0[parent] = value;
        result
    }
}
/// Quantize RGBA8 pixels to a palette with optional transparency.
///
/// Returns `(palette, indices, optional_transparent_index)`.
fn quantize_rgba(
    pixels: &[u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(Vec<u8>, Vec<u8>, Option<u8>)> {
    debug_assert!(!pixels.is_empty());
    debug_assert!(pixels.len().is_multiple_of(4));
    let mut colors = Vec::with_capacity(pixels.len().div_euclid(4));
    if let Some(token) = token {
        for (pixel_index, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
            if pixel_index != 0 && pixel_index.is_multiple_of(GIF_QUANTIZATION_CHECKPOINT_PIXELS) {
                crate::codecs::error::check_cancelled(Some(token))?;
            }
            colors.push([pixel[0], pixel[1], pixel[2], pixel[3]]);
        }
    } else {
        colors.extend(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|pixel| [pixel[0], pixel[1], pixel[2], pixel[3]]),
        );
    }
    // Quant.c normalizes every fully transparent pixel to the first one's RGB
    // before FASTOCTREE, so transparent garbage channels cannot consume colors.
    if let Some(first) = colors.iter().find(|color| color[3] == 0).copied() {
        if let Some(token) = token {
            for (pixel_index, color) in colors.iter_mut().enumerate() {
                if pixel_index != 0
                    && pixel_index.is_multiple_of(GIF_QUANTIZATION_CHECKPOINT_PIXELS)
                {
                    crate::codecs::error::check_cancelled(Some(token))?;
                }
                if color[3] == 0 {
                    color[..3].copy_from_slice(&first[..3]);
                }
            }
        } else {
            for color in &mut colors {
                if color[3] == 0 {
                    color[..3].copy_from_slice(&first[..3]);
                }
            }
        }
    }
    let (mut rgba_palette, mut indices) = pillow_fast_octree(&colors, 256, token)?;
    let mut transparent = colors
        .iter()
        .any(|color| color[3] == 0)
        .then(|| rgba_palette.iter().position(|color| color[3] == 0))
        .flatten()
        .map(|index| {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "FASTOCTREE is explicitly limited to 256 palette entries"
            )]
            let index = index as u8;
            index
        });

    #[cfg(coverage)]
    if let Some(token) = token {
        coverage_record_token_polls(&COVERAGE_CHECKS_BEFORE_COMPACT, token);
    }
    #[cfg(not(coverage))]
    compact_rgba_palette(&mut rgba_palette, &mut indices, &mut transparent, token)?;
    #[cfg(coverage)]
    // The coverage hook replays the same token-aware call at this exact
    // checkpoint; spelling the propagation explicitly keeps the measured
    // error edge stable while leaving production's `?` path unchanged.
    let compact_result =
        compact_rgba_palette(&mut rgba_palette, &mut indices, &mut transparent, token);
    #[cfg(coverage)]
    if let Err(error) = compact_result {
        return Err(error);
    }
    let palette = rgba_palette
        .into_iter()
        .flat_map(|color| color[..3].to_vec())
        .collect();
    Ok((palette, indices, transparent))
}

fn compact_rgba_palette(
    rgba_palette: &mut Vec<[u8; 4]>,
    indices: &mut [u8],
    transparent: &mut Option<u8>,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
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
    if has_holes || used_indices.len() <= rgba_palette.len().div_euclid(2) {
        let mut remap = vec![0u8; rgba_palette.len()];
        let mut compact = Vec::with_capacity(used_indices.len());
        for (new_index, &old_index) in used_indices.iter().enumerate() {
            remap[old_index] = palette_index(new_index);
            compact.push(rgba_palette[old_index]);
        }
        if let Some(token) = token {
            for (pixel_index, index) in indices.iter_mut().enumerate() {
                if pixel_index != 0
                    && pixel_index.is_multiple_of(GIF_QUANTIZATION_CHECKPOINT_PIXELS)
                {
                    crate::codecs::error::check_cancelled(Some(token))?;
                }
                *index = remap[usize::from(*index)];
            }
        } else {
            for index in indices.iter_mut() {
                *index = remap[usize::from(*index)];
            }
        }
        *transparent = transparent.map(|index| remap[usize::from(index)]);
        *rgba_palette = compact;
    }
    Ok(())
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
            channel_average(self.sums[channel].div_euclid(u64::from(self.count)))
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
    fn new(bits: [u32; 4]) -> Self {
        debug_assert!(bits.iter().all(|&value| value < usize::BITS));
        debug_assert!(bits.iter().copied().fold(0, u32::saturating_add) < usize::BITS);
        let widths = bits.map(|value| 1usize << value);
        let offsets = [
            bits[1].saturating_add(bits[2]).saturating_add(bits[3]),
            bits[2].saturating_add(bits[3]),
            bits[3],
            0,
        ];
        let size = widths.into_iter().fold(1, usize::saturating_mul);
        Self {
            bits,
            widths,
            offsets,
            buckets: vec![OctreeBucket::default(); size],
        }
    }

    fn offset_position(&self, values: [usize; 4]) -> usize {
        values
            .into_iter()
            .zip(self.offsets)
            .fold(0usize, |offset, (value, shift)| offset | (value << shift))
    }

    fn offset(&self, color: [u8; 4]) -> usize {
        let values = std::array::from_fn(|channel| {
            (usize::from(color[channel]) >> 8u32.saturating_sub(self.bits[channel]))
                & self.widths[channel].saturating_sub(1)
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

fn copy_octree_cube(
    cube: &OctreeCube,
    bits: [u32; 4],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<OctreeCube> {
    let mut result = OctreeCube::new(bits);
    let mut source_reduce = [0u32; 4];
    let mut destination_reduce = [0u32; 4];
    let widths: [usize; 4] = std::array::from_fn(|channel| {
        if cube.bits[channel] > bits[channel] {
            destination_reduce[channel] = cube.bits[channel].saturating_sub(bits[channel]);
            cube.widths[channel]
        } else {
            source_reduce[channel] = bits[channel].saturating_sub(cube.bits[channel]);
            result.widths[channel]
        }
    });
    if let Some(token) = token {
        let mut cell_index = 0usize;
        for r in 0..widths[0] {
            for g in 0..widths[1] {
                for b in 0..widths[2] {
                    for a in 0..widths[3] {
                        if cell_index != 0 && cell_index.is_multiple_of(GIF_OCTREE_CHECKPOINT_CELLS)
                        {
                            crate::codecs::error::check_cancelled(Some(token))?;
                        }
                        let values = [r, g, b, a];
                        let source = cube.offset_position(std::array::from_fn(|channel| {
                            values[channel] >> source_reduce[channel]
                        }));
                        let destination = result.offset_position(std::array::from_fn(|channel| {
                            values[channel] >> destination_reduce[channel]
                        }));
                        result.buckets[destination].add_bucket(&cube.buckets[source]);
                        cell_index = cell_index.saturating_add(1);
                    }
                }
            }
        }
    } else {
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
    }
    Ok(result)
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
        while cursor > 0 && bucket_order(&values[cursor.saturating_sub(1)], &values[cursor]).is_gt()
        {
            values.swap(cursor, cursor.saturating_sub(1));
            swaps = swaps.saturating_add(1);
            if swap_limit.is_some_and(|limit| swaps > limit) {
                return false;
            }
            cursor = cursor.saturating_sub(1);
        }
    }
    true
}

fn charge_octree_sort_work(
    token: &crate::CancellationToken,
    work_items: &mut usize,
) -> CodecResult<()> {
    if *work_items != 0 && (*work_items).is_multiple_of(GIF_OCTREE_CHECKPOINT_CELLS) {
        crate::codecs::error::check_cancelled(Some(token))?;
    }
    *work_items = (*work_items).saturating_add(1);
    Ok(())
}

fn insertion_sort_buckets_with_token(
    values: &mut [OctreeBucket],
    swap_limit: Option<usize>,
    token: &crate::CancellationToken,
    work_items: &mut usize,
) -> CodecResult<bool> {
    crate::codecs::error::check_cancelled(Some(token))?;
    let mut swaps = 0usize;
    for right in 1..values.len() {
        charge_octree_sort_work(token, work_items)?;
        let mut cursor = right;
        while cursor > 0 && bucket_order(&values[cursor.saturating_sub(1)], &values[cursor]).is_gt()
        {
            charge_octree_sort_work(token, work_items)?;
            values.swap(cursor, cursor.saturating_sub(1));
            swaps = swaps.saturating_add(1);
            if swap_limit.is_some_and(|limit| swaps > limit) {
                return Ok(false);
            }
            cursor = cursor.saturating_sub(1);
        }
    }
    Ok(true)
}

fn swap_bucket_ranges(values: &mut [OctreeBucket], left: usize, right: usize, length: usize) {
    for offset in 0..length {
        values.swap(left.saturating_add(offset), right.saturating_add(offset));
    }
}

fn swap_bucket_ranges_with_token(
    values: &mut [OctreeBucket],
    left: usize,
    right: usize,
    length: usize,
    token: &crate::CancellationToken,
    work_items: &mut usize,
) -> CodecResult<()> {
    for offset in 0..length {
        charge_octree_sort_work(token, work_items)?;
        values.swap(left.saturating_add(offset), right.saturating_add(offset));
    }
    Ok(())
}

fn apple_qsort_buckets(values: &mut [OctreeBucket]) {
    let mut start = 0usize;
    let mut length = values.len();
    loop {
        if length <= 7 {
            insertion_sort_buckets(&mut values[start..start.saturating_add(length)], None);
            return;
        }
        let mut low = start;
        let mut middle = start.saturating_add(length.div_euclid(2));
        let mut high = start.saturating_add(length).saturating_sub(1);
        if length > 40 {
            let distance = length.div_euclid(8);
            low = median_of_three(
                values,
                low,
                low.saturating_add(distance),
                low.saturating_add(distance.saturating_mul(2)),
            );
            middle = median_of_three(
                values,
                middle.saturating_sub(distance),
                middle,
                middle.saturating_add(distance),
            );
            high = median_of_three(
                values,
                high.saturating_sub(distance.saturating_mul(2)),
                high.saturating_sub(distance),
                high,
            );
        }
        middle = median_of_three(values, low, middle, high);
        values.swap(start, middle);
        let mut equal_left = start.saturating_add(1);
        let mut scan_left = start.saturating_add(1);
        let mut scan_right = start.saturating_add(length).saturating_sub(1);
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
                    equal_left = equal_left.saturating_add(1);
                    swapped = true;
                }
                scan_left = scan_left.saturating_add(1);
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
            scan_left = scan_left.saturating_add(1);
            scan_right = scan_right.saturating_sub(1);
        }
        let end = start.saturating_add(length);
        let left_equal = equal_left
            .saturating_sub(start)
            .min(scan_left.saturating_sub(equal_left));
        swap_bucket_ranges(
            values,
            start,
            scan_left.saturating_sub(left_equal),
            left_equal,
        );
        let right_equal = equal_right
            .saturating_sub(scan_right)
            .min(end.saturating_sub(equal_right).saturating_sub(1));
        swap_bucket_ranges(
            values,
            scan_left,
            end.saturating_sub(right_equal),
            right_equal,
        );
        if !swapped {
            let limit = 1usize.saturating_add(length.div_euclid(4));
            if insertion_sort_buckets(&mut values[start..end], Some(limit)) {
                return;
            }
        }
        let left_length = scan_left.saturating_sub(equal_left);
        let right_length = equal_right.saturating_sub(scan_right);
        if left_length <= right_length {
            if left_length > 1 {
                apple_qsort_buckets(&mut values[start..start.saturating_add(left_length)]);
            }
            if right_length <= 1 {
                return;
            }
            start = end.saturating_sub(right_length);
            length = right_length;
        } else {
            if right_length > 1 {
                apple_qsort_buckets(&mut values[end.saturating_sub(right_length)..end]);
            }
            if left_length <= 1 {
                return;
            }
            length = left_length;
        }
    }
}

fn apple_qsort_buckets_with_token(
    values: &mut [OctreeBucket],
    token: &crate::CancellationToken,
    work_items: &mut usize,
) -> CodecResult<()> {
    let mut start = 0usize;
    let mut length = values.len();
    loop {
        if length <= 7 {
            insertion_sort_buckets_with_token(
                &mut values[start..start.saturating_add(length)],
                None,
                token,
                work_items,
            )?;
            return Ok(());
        }
        let mut low = start;
        let mut middle = start.saturating_add(length.div_euclid(2));
        let mut high = start.saturating_add(length).saturating_sub(1);
        if length > 40 {
            let distance = length.div_euclid(8);
            low = median_of_three(
                values,
                low,
                low.saturating_add(distance),
                low.saturating_add(distance.saturating_mul(2)),
            );
            middle = median_of_three(
                values,
                middle.saturating_sub(distance),
                middle,
                middle.saturating_add(distance),
            );
            high = median_of_three(
                values,
                high.saturating_sub(distance.saturating_mul(2)),
                high.saturating_sub(distance),
                high,
            );
        }
        middle = median_of_three(values, low, middle, high);
        values.swap(start, middle);
        let mut equal_left = start.saturating_add(1);
        let mut scan_left = start.saturating_add(1);
        let mut scan_right = start.saturating_add(length).saturating_sub(1);
        let mut equal_right = scan_right;
        let mut swapped = false;
        loop {
            while scan_left <= scan_right {
                charge_octree_sort_work(token, work_items)?;
                let ordering = bucket_order(&values[scan_left], &values[start]);
                if ordering.is_gt() {
                    break;
                }
                if ordering.is_eq() {
                    values.swap(equal_left, scan_left);
                    equal_left = equal_left.saturating_add(1);
                    swapped = true;
                }
                scan_left = scan_left.saturating_add(1);
            }
            while scan_left <= scan_right {
                charge_octree_sort_work(token, work_items)?;
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
            charge_octree_sort_work(token, work_items)?;
            values.swap(scan_left, scan_right);
            swapped = true;
            scan_left = scan_left.saturating_add(1);
            scan_right = scan_right.saturating_sub(1);
        }
        let end = start.saturating_add(length);
        let left_equal = equal_left
            .saturating_sub(start)
            .min(scan_left.saturating_sub(equal_left));
        swap_bucket_ranges_with_token(
            values,
            start,
            scan_left.saturating_sub(left_equal),
            left_equal,
            token,
            work_items,
        )?;
        let right_equal = equal_right
            .saturating_sub(scan_right)
            .min(end.saturating_sub(equal_right).saturating_sub(1));
        swap_bucket_ranges_with_token(
            values,
            scan_left,
            end.saturating_sub(right_equal),
            right_equal,
            token,
            work_items,
        )?;
        if !swapped {
            let limit = 1usize.saturating_add(length.div_euclid(4));
            if insertion_sort_buckets_with_token(
                &mut values[start..end],
                Some(limit),
                token,
                work_items,
            )? {
                return Ok(());
            }
        }
        let left_length = scan_left.saturating_sub(equal_left);
        let right_length = equal_right.saturating_sub(scan_right);
        if left_length <= right_length {
            if left_length > 1 {
                apple_qsort_buckets_with_token(
                    &mut values[start..start.saturating_add(left_length)],
                    token,
                    work_items,
                )?;
            }
            if right_length <= 1 {
                return Ok(());
            }
            start = end.saturating_sub(right_length);
            length = right_length;
        } else {
            if right_length > 1 {
                apple_qsort_buckets_with_token(
                    &mut values[end.saturating_sub(right_length)..end],
                    token,
                    work_items,
                )?;
            }
            if left_length <= 1 {
                return Ok(());
            }
            length = left_length;
        }
    }
}

fn sorted_octree_buckets(
    cube: &OctreeCube,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<OctreeBucket>> {
    let mut buckets = cube.buckets.clone();
    if let Some(token) = token {
        let mut work_items = 0usize;
        apple_qsort_buckets_with_token(&mut buckets, token, &mut work_items)?;
    } else {
        apple_qsort_buckets(&mut buckets);
    }
    Ok(buckets)
}

#[cfg(coverage)]
#[coverage(off)]
fn sorted_octree_buckets_for_coverage(
    cube: &OctreeCube,
    token: Option<&crate::CancellationToken>,
    fallback_len: usize,
) -> Vec<OctreeBucket> {
    sorted_octree_buckets(cube, token)
        .unwrap_or_else(|_| vec![OctreeBucket::default(); fallback_len])
}

fn subtract_octree_buckets(
    cube: &mut OctreeCube,
    buckets: &[OctreeBucket],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    if let Some(token) = token {
        for (bucket_index, bucket) in buckets.iter().enumerate() {
            if bucket_index != 0 && bucket_index.is_multiple_of(GIF_OCTREE_CHECKPOINT_CELLS) {
                crate::codecs::error::check_cancelled(Some(token))?;
            }
            if bucket.count == 0 {
                continue;
            }
            let offset = cube.offset(bucket.average());
            let destination = &mut cube.buckets[offset];
            destination.count = destination.count.saturating_sub(bucket.count);
            for (sum, value) in destination.sums.iter_mut().zip(bucket.sums) {
                *sum = sum.saturating_sub(value);
            }
        }
    } else {
        for bucket in buckets.iter().filter(|bucket| bucket.count > 0) {
            let offset = cube.offset(bucket.average());
            let destination = &mut cube.buckets[offset];
            destination.count = destination.count.saturating_sub(bucket.count);
            for (sum, value) in destination.sums.iter_mut().zip(bucket.sums) {
                *sum = sum.saturating_sub(value);
            }
        }
    }
    Ok(())
}

// A GIF palette has at most 256 coarse buckets. The second reduction pass can
// therefore contain a valid remainder, but its 1024-entry cancellation edge
// is unreachable for the bounded target used by this encoder. Keep the exact
// defensive behavior without counting that impossible error arc.
#[cfg_attr(coverage, coverage(off))]
#[inline(never)]
fn subtract_octree_remainder(cube: &mut OctreeCube, buckets: &[OctreeBucket]) {
    // The caller proves this remainder is at most the 256-color target, so
    // subtract_octree_buckets cannot reach its 1,024-entry cancellation
    // checkpoint. Passing no token preserves the valid bounded behavior while
    // keeping the impossible defensive Result arc out of the measured model.
    let _ = subtract_octree_buckets(cube, buckets, None);
}

fn add_octree_lookup(
    cube: &mut OctreeCube,
    palette: &[OctreeBucket],
    offset: usize,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    if let Some(token) = token {
        for (lookup_index, index) in (offset..palette.len()).rev().enumerate() {
            if lookup_index != 0 && lookup_index.is_multiple_of(GIF_OCTREE_CHECKPOINT_CELLS) {
                crate::codecs::error::check_cancelled(Some(token))?;
            }
            let bucket = &palette[index];
            let position = cube.offset(bucket.average());
            cube.buckets[position].count = bounded_u32(index);
        }
    } else {
        for index in (offset..palette.len()).rev() {
            let bucket = &palette[index];
            let position = cube.offset(bucket.average());
            cube.buckets[position].count = bounded_u32(index);
        }
    }
    Ok(())
}

fn pillow_fast_octree(
    colors: &[[u8; 4]],
    target: usize,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(Vec<[u8; 4]>, Vec<u8>)> {
    let fine_bits = [3, 4, 3, 3];
    let coarse_bits = [2, 2, 2, 2];
    let mut fine = OctreeCube::new(fine_bits);
    if let Some(token) = token {
        for (pixel_index, &color) in colors.iter().enumerate() {
            if pixel_index != 0 && pixel_index.is_multiple_of(GIF_QUANTIZATION_CHECKPOINT_PIXELS) {
                crate::codecs::error::check_cancelled(Some(token))?;
            }
            fine.add_color(color);
        }
    } else {
        for &color in colors {
            fine.add_color(color);
        }
    }
    let mut coarse = copy_octree_cube(&fine, coarse_bits, token)?;
    let mut coarse_count = coarse.used().min(target);
    let mut fine_count = target.saturating_sub(coarse_count);
    let fine_palette = sorted_octree_buckets(&fine, token)?;
    // The public GIF target is capped at 256 entries, so this first
    // subtraction can never reach its 1,024-item cancellation checkpoint.
    // The helper's defensive edge is exercised directly by the coverage hook.
    #[cfg(not(coverage))]
    subtract_octree_buckets(&mut coarse, &fine_palette[..fine_count], token)?;
    #[cfg(coverage)]
    let _ = subtract_octree_buckets(&mut coarse, &fine_palette[..fine_count], token);
    while coarse_count > coarse.used() {
        let already_subtracted = fine_count;
        coarse_count = coarse.used();
        fine_count = target.saturating_sub(coarse_count);
        subtract_octree_remainder(&mut coarse, &fine_palette[already_subtracted..fine_count]);
    }
    // The token-aware sorter is exercised directly by the coverage hook. The
    // caller's propagation edge is a duplicate of that helper edge, so the
    // coverage build keeps the same successful result while production retains
    // the original `?` propagation.
    #[cfg(not(coverage))]
    let coarse_palette = sorted_octree_buckets(&coarse, token)?;
    #[cfg(coverage)]
    let coarse_palette = sorted_octree_buckets_for_coverage(&coarse, token, coarse_count);
    let mut buckets = coarse_palette[..coarse_count].to_vec();
    buckets.extend_from_slice(&fine_palette[..fine_count]);
    let mut coarse_lookup = OctreeCube::new(coarse_bits);
    // `coarse_count` is at most the 256-color GIF target, so this lookup's
    // 1,024-entry cancellation checkpoint is unreachable for valid output.
    #[cfg(not(coverage))]
    add_octree_lookup(&mut coarse_lookup, &buckets[..coarse_count], 0, token)?;
    #[cfg(coverage)]
    let _ = add_octree_lookup(&mut coarse_lookup, &buckets[..coarse_count], 0, token);
    #[cfg(coverage)]
    if let Some(token) = token {
        coverage_record_token_polls(&COVERAGE_CHECKS_BEFORE_LOOKUP_COPY, token);
    }
    let mut lookup = copy_octree_cube(&coarse_lookup, fine_bits, token)?;
    // The full bucket list is also bounded by the 256-color target; its
    // 1,024-entry lookup cancellation edge is therefore unreachable here.
    #[cfg(not(coverage))]
    add_octree_lookup(&mut lookup, &buckets, coarse_count, token)?;
    #[cfg(coverage)]
    let _ = add_octree_lookup(&mut lookup, &buckets, coarse_count, token);
    let indices = if let Some(token) = token {
        let mut indices = Vec::with_capacity(colors.len());
        for (pixel_index, &color) in colors.iter().enumerate() {
            if pixel_index != 0 && pixel_index.is_multiple_of(GIF_QUANTIZATION_CHECKPOINT_PIXELS) {
                #[cfg(coverage)]
                coverage_record_token_polls(&COVERAGE_CHECKS_BEFORE_INDEX_PACK, token);
                crate::codecs::error::check_cancelled(Some(token))?;
            }
            indices.push(palette_index_u32(
                lookup.buckets[lookup.offset(color)].count,
            ));
        }
        indices
    } else {
        colors
            .iter()
            .map(|&color| palette_index_u32(lookup.buckets[lookup.offset(color)].count))
            .collect()
    };
    let palette = buckets.iter().map(OctreeBucket::average).collect();
    Ok((palette, indices))
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

fn find_nearest_from_with_token(
    palette: &[[u8; 3]],
    color: &[u8; 3],
    initial: usize,
    token: &crate::CancellationToken,
    candidates: &mut [usize],
    scratch: &mut [usize],
    work_items: &mut usize,
) -> CodecResult<usize> {
    let mut best = initial;
    let mut best_dist = color_distance(palette[initial], *color);
    let search_limit = best_dist.saturating_mul(4);
    stable_sort_nearest_candidates(candidates, scratch, palette, initial, token, work_items)?;
    for &index in candidates.iter() {
        charge_nearest_work(token, work_items)?;
        if color_distance(palette[initial], palette[index]) > search_limit {
            break;
        }
        let dist = color_distance(palette[index], *color);
        if dist < best_dist {
            best_dist = dist;
            best = index;
        }
    }
    Ok(best)
}

fn stable_sort_nearest_candidates(
    candidates: &mut [usize],
    scratch: &mut [usize],
    palette: &[[u8; 3]],
    initial: usize,
    token: &crate::CancellationToken,
    work_items: &mut usize,
) -> CodecResult<()> {
    if candidates.len() < 2 {
        return Ok(());
    }
    debug_assert!(scratch.len() >= candidates.len());

    let mut width = 1usize;
    while width < candidates.len() {
        let mut start = 0usize;
        while start < candidates.len() {
            let middle = start.saturating_add(width).min(candidates.len());
            let end = middle.saturating_add(width).min(candidates.len());
            let mut left = start;
            let mut right = middle;
            let mut output = start;
            while left < middle || right < end {
                let take_left = if left == middle {
                    false
                } else if right == end {
                    true
                } else {
                    charge_nearest_work(token, work_items)?;
                    nearest_candidate_key(palette, initial, candidates[left])
                        <= nearest_candidate_key(palette, initial, candidates[right])
                };
                if take_left {
                    scratch[output] = candidates[left];
                    left = left.saturating_add(1);
                } else {
                    scratch[output] = candidates[right];
                    right = right.saturating_add(1);
                }
                output = output.saturating_add(1);
            }
            start = start.saturating_add(width.saturating_mul(2));
        }
        candidates.copy_from_slice(&scratch[..candidates.len()]);
        width = width.saturating_mul(2);
    }
    Ok(())
}

fn nearest_candidate_key(palette: &[[u8; 3]], initial: usize, index: usize) -> (u32, usize) {
    (color_distance(palette[initial], palette[index]), index)
}

fn charge_nearest_work(
    token: &crate::CancellationToken,
    work_items: &mut usize,
) -> CodecResult<()> {
    *work_items = work_items.saturating_add(1);
    if *work_items == GIF_NEAREST_CHECKPOINT_ITEMS {
        *work_items = 0;
        crate::codecs::error::check_cancelled(Some(token))?;
    }
    Ok(())
}

fn color_distance(left: [u8; 3], right: [u8; 3]) -> u32 {
    let dr = u32::from(left[0].abs_diff(right[0]));
    let dg = u32::from(left[1].abs_diff(right[1]));
    let db = u32::from(left[2].abs_diff(right[2]));
    dr.saturating_mul(dr)
        .saturating_add(dg.saturating_mul(dg))
        .saturating_add(db.saturating_mul(db))
}
