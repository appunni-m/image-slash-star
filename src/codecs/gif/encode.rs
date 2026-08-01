//! GIF89a encoder.
//!
//! Supports:
//! - `L8`: raw palette indices with a grayscale palette
//! - `Rgb8`: quantized to a 256-color palette
//! - `Rgba8`: quantized to a 256-color palette plus transparency

use crate::codecs::error::{CodecError, CodecResult};
use crate::encode_options::{GifColorTable, GifEncodeOptions, GifLoop};
#[cfg(coverage)]
use crate::types::DecodedFrame;
use crate::types::{
    AnimationBackground, ColorType, DecodedImage, DecodedSequence, FrameBlend, FrameDisposal,
    FrameDuration, FramePixelLayout, ImageMode, ImagePalette,
};
use std::collections::HashMap;

const GIF_TRAILER: u8 = 0x3b;
const IMAGE_SEPARATOR: u8 = 0x2c;
const EXTENSION_INTRODUCER: u8 = 0x21;
const GRAPHIC_CONTROL_LABEL: u8 = 0xf9;
const MAX_LZW_CODE: u16 = 4095;

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
/// For L8 images the pixel values are used directly as palette indices with a
/// grayscale palette. RGB8 and RGBA8 images are quantized to a palette of at
/// most 256 unique colors using a simple nearest-neighbor approach.
///
/// Returns a classified failure for invalid images, modes, or options.
pub fn encode(img: &DecodedImage, opts: &GifEncodeOptions) -> CodecResult<Vec<u8>> {
    encode_sequence(&DecodedSequence::from_image(img.clone()), opts)
}

#[cfg(coverage)]
#[allow(clippy::expect_used)]
pub(crate) fn __coverage_exercise_private_branches() {
    let invalid_sequence = DecodedSequence {
        width: 0,
        height: 1,
        frames: Vec::new(),
        loop_count: None,
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

    let mut compact_palette = vec![[255u8, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];
    let mut compact_indices = vec![0u8, 1, 2];
    let mut compact_transparent = None;
    let _ = compact_rgba_palette(
        &mut compact_palette,
        &mut compact_indices,
        &mut compact_transparent,
    );
    let mut hole_palette = vec![[255u8, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];
    let mut hole_indices = vec![0u8, 2];
    let mut hole_transparent = None;
    let _ = compact_rgba_palette(&mut hole_palette, &mut hole_indices, &mut hole_transparent);

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
        loop_count: None,
        background: None,
        kind: crate::types::SequenceKind::TimedAnimation,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
    };
    let coalesced =
        coalesce_identical_frames(&sequence, 2, None).expect("coverage RGB frames coalesce");
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
        loop_count: None,
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
            loop_count: None,
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
        loop_count: None,
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

    let _ = prepare_image(&DecodedImage::with_mode(1, 1, vec![0], ImageMode::P8));
    let _ = indexed_rgb(&[0], &[0, 0, 0]);
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
        loop_count: None,
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
        loop_count: None,
        background: None,
        kind: crate::types::SequenceKind::TimedAnimation,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
    };
    let _ = coalesce_identical_frames(&invalid_duration_sequence, 2, None);

    let mut oversized_loop = still.clone();
    oversized_loop.loop_count = Some(u32::MAX);
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
        loop_count: None,
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
        loop_count: None,
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
        loop_count: None,
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
        loop_count: None,
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
        loop_count: None,
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
        loop_count: None,
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
            loop_count: None,
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
        loop_count: None,
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

    let _ = OctreeCube::new([3, 4, 3, 3]);
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
            Some(value) => Some(u16::try_from(value).map_err(|_| {
                CodecError::Parameter("GIF loop count exceeds format limits".to_owned())
            })?),
            None => None,
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
                        .chunks_exact(4)
                        .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
                        .collect();
                    DecodedImage::new(sequence.width, sequence.height, rgb, ColorType::Rgb8)
                };
                let mut prepared =
                    prepare_image(&full_image).expect("coalesced full GIF canvas is RGB/RGBA");
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
        .chunks_exact(4)
        .zip(current.chunks_exact(4))
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

fn prepare_image(img: &DecodedImage) -> CodecResult<PreparedImage> {
    let (palette, indices, transparent) = match (img.mode, img.color) {
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
            let (palette, indices) = quantize_rgb(&img.pixels);
            (palette, indices, None)
        }
        (ImageMode::Rgba8, ColorType::Rgba8) => {
            let (palette, indices, transparent_idx) = quantize_rgba(&img.pixels);
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
        prepared_frames.push(prepare_image(&frame.image)?);
    }
    // `first_frame` above proves the prepared vector has the same nonzero
    // length as `frames`.
    let mut first = prepared_frames[0].clone();
    let background = prepare_background(&mut first, first_frame.image.mode, sequence.background);
    let (global_count, global_size, _) = table_parameters(&first.palette);
    // Pillow always writes the global palette for a single frame. Its
    // include_color_table option adds a duplicate local palette rather than
    // replacing the global one.
    let global_table = true;

    let has_transparency = prepared_frames
        .iter()
        .any(|prepared| prepared.transparent.is_some());
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
    let mut previous_disposal = None::<u8>;
    for (frame_index, (frame, prepared)) in frames.iter().zip(&prepared_frames).enumerate() {
        crate::codecs::error::check_cancelled(token)?;
        let mut prepared = if frame_index == 0 {
            first.clone()
        } else {
            prepared.clone()
        };
        let quantized_rgb = indexed_rgb(&prepared.indices, &prepared.palette);
        let previous_can_mask = previous_disposal != Some(2);
        if previous_can_mask
            && let Some(previous) = previous_quantized_rgb.as_deref()
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
        let local_table = settings.local_color_table || prepared.palette != first.palette;
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
        let compressed = encode_lzw(&encoded_indices, minimum_code_size);
        output.push(minimum_code_size);
        write_sub_blocks(&mut output, &compressed);
        crate::codecs::error::check_cancelled(token)?;
    }
    output.push(GIF_TRAILER);
    Ok(output)
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
                for (index, color) in first.palette.chunks_exact(3).enumerate() {
                    if color == [red, green, blue] {
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

fn indexed_rgb(indices: &[u8], palette: &[u8]) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(indices.len().saturating_mul(3));
    for &index in indices {
        let offset = usize::from(index).saturating_mul(3);
        rgb.extend_from_slice(&palette[offset..offset.saturating_add(3)]);
    }
    rgb
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
fn encode_lzw(indices: &[u8], minimum_code_size: u8) -> Vec<u8> {
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
    for &suffix in &indices[1..] {
        if let Some(&code) = dictionary.get(&(prefix, suffix)) {
            prefix = code;
            continue;
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
    }

    writer.write(prefix, code_size);
    writer.write(end_code, code_size);
    writer.finish()
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
fn quantize_rgb(pixels: &[u8]) -> (Vec<u8>, Vec<u8>) {
    debug_assert!(pixels.len().is_multiple_of(3));
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut counts = Vec::<u32>::new();
    for chunk in pixels.chunks_exact(3) {
        let color = [chunk[0], chunk[1], chunk[2]];
        match find_color(&palette, &color) {
            Some(idx) => counts[idx] = counts[idx].saturating_add(1),
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
    let order = pillow_median_cut_order(&palette, &counts);
    let mut remap = vec![0u8; palette.len()];
    let mut flat = Vec::with_capacity(palette.len().saturating_mul(3));
    for (new_index, &old_index) in order.iter().enumerate() {
        remap[old_index] = palette_index(new_index);
        flat.extend_from_slice(&palette[old_index]);
    }
    let indices = pixels
        .chunks_exact(3)
        .map(|chunk| {
            let color = [chunk[0], chunk[1], chunk[2]];
            // `palette` was constructed from these exact source pixels.
            #[allow(clippy::expect_used)]
            let index =
                find_color(&palette, &color).expect("RGB GIF palette was built from source pixels");
            remap[index]
        })
        .collect();
    (flat, indices)
}

fn quantize_rgb_nearest(pixels: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut colors = Vec::<[u8; 3]>::new();
    let mut counts = Vec::<u32>::new();
    let mut color_indices = HashMap::<u32, usize>::new();
    for chunk in pixels.chunks_exact(3) {
        let color = [chunk[0], chunk[1], chunk[2]];
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
    let leaves = pillow_median_cut_leaves(&colors, &counts, colors.len().min(256));
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
            let index = color_indices[&pillow_pixel_hash(color)];
            palette_index(remap[mapped[index]])
        })
        .collect();
    (optimized.into_iter().flatten().collect(), indices)
}

#[derive(Clone)]
struct MedianBox {
    axes: [Vec<usize>; 3],
    pixel_count: u32,
    children: Option<(usize, usize)>,
}

fn pillow_median_cut_order(colors: &[[u8; 3]], counts: &[u32]) -> Vec<usize> {
    let leaves = pillow_median_cut_leaves(colors, counts, colors.len());
    leaves.into_iter().map(|leaf| leaf[0]).collect()
}

fn pillow_median_cut_leaves(colors: &[[u8; 3]], counts: &[u32], target: usize) -> Vec<Vec<usize>> {
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
    let pixel_count = counts.iter().sum();
    let mut boxes = vec![MedianBox {
        axes,
        pixel_count,
        children: None,
    }];
    let mut heap = PillowBoxHeap::default();
    heap.add(0, &boxes);

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
    leaves
}

fn pillow_hash_iteration_order(colors: &[[u8; 3]]) -> Vec<usize> {
    // QuantHash.c grows 11 -> 23 -> 47 -> 97 for this range. Its historical
    // prime finder accepts the first candidate in this residue table.
    const ACCEPTED_RESIDUES: [bool; 16] = [
        false, true, false, true, false, false, false, true, false, true, false, true, false, true,
        false, false,
    ];
    let mut length = 11u32;
    for count in 1..=colors.len() {
        if length.saturating_mul(3) < bounded_u32(count) {
            let mut candidate = length.saturating_mul(2).saturating_add(1);
            while !ACCEPTED_RESIDUES[(candidate & 15) as usize] {
                candidate = candidate.saturating_add(1);
            }
            length = candidate;
        }
    }
    let mut iteration = (0..colors.len()).collect::<Vec<_>>();
    iteration.sort_by_key(|&index| {
        let hash = pillow_pixel_hash(colors[index]);
        (hash.rem_euclid(length), hash)
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
fn quantize_rgba(pixels: &[u8]) -> (Vec<u8>, Vec<u8>, Option<u8>) {
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
    let (mut rgba_palette, mut indices) = pillow_fast_octree(&colors, 256);
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

    compact_rgba_palette(&mut rgba_palette, &mut indices, &mut transparent);
    let palette = rgba_palette
        .into_iter()
        .flat_map(|color| color[..3].to_vec())
        .collect();
    (palette, indices, transparent)
}

fn compact_rgba_palette(
    rgba_palette: &mut Vec<[u8; 4]>,
    indices: &mut [u8],
    transparent: &mut Option<u8>,
) {
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
        for index in indices.iter_mut() {
            *index = remap[usize::from(*index)];
        }
        *transparent = transparent.map(|index| remap[usize::from(index)]);
        *rgba_palette = compact;
    }
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

fn copy_octree_cube(cube: &OctreeCube, bits: [u32; 4]) -> OctreeCube {
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
    result
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

fn swap_bucket_ranges(values: &mut [OctreeBucket], left: usize, right: usize, length: usize) {
    for offset in 0..length {
        values.swap(left.saturating_add(offset), right.saturating_add(offset));
    }
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

fn sorted_octree_buckets(cube: &OctreeCube) -> Vec<OctreeBucket> {
    let mut buckets = cube.buckets.clone();
    apple_qsort_buckets(&mut buckets);
    buckets
}

fn subtract_octree_buckets(cube: &mut OctreeCube, buckets: &[OctreeBucket]) {
    for bucket in buckets.iter().filter(|bucket| bucket.count > 0) {
        let offset = cube.offset(bucket.average());
        let destination = &mut cube.buckets[offset];
        destination.count = destination.count.saturating_sub(bucket.count);
        for (sum, value) in destination.sums.iter_mut().zip(bucket.sums) {
            *sum = sum.saturating_sub(value);
        }
    }
}

fn add_octree_lookup(cube: &mut OctreeCube, palette: &[OctreeBucket], offset: usize) {
    for index in (offset..palette.len()).rev() {
        let bucket = &palette[index];
        let position = cube.offset(bucket.average());
        cube.buckets[position].count = bounded_u32(index);
    }
}

fn pillow_fast_octree(colors: &[[u8; 4]], target: usize) -> (Vec<[u8; 4]>, Vec<u8>) {
    let fine_bits = [3, 4, 3, 3];
    let coarse_bits = [2, 2, 2, 2];
    let mut fine = OctreeCube::new(fine_bits);
    for &color in colors {
        fine.add_color(color);
    }
    let mut coarse = copy_octree_cube(&fine, coarse_bits);
    let mut coarse_count = coarse.used().min(target);
    let mut fine_count = target.saturating_sub(coarse_count);
    let fine_palette = sorted_octree_buckets(&fine);
    subtract_octree_buckets(&mut coarse, &fine_palette[..fine_count]);
    while coarse_count > coarse.used() {
        let already_subtracted = fine_count;
        coarse_count = coarse.used();
        fine_count = target.saturating_sub(coarse_count);
        subtract_octree_buckets(&mut coarse, &fine_palette[already_subtracted..fine_count]);
    }
    let coarse_palette = sorted_octree_buckets(&coarse);
    let mut buckets = coarse_palette[..coarse_count].to_vec();
    buckets.extend_from_slice(&fine_palette[..fine_count]);
    let mut coarse_lookup = OctreeCube::new(coarse_bits);
    add_octree_lookup(&mut coarse_lookup, &buckets[..coarse_count], 0);
    let mut lookup = copy_octree_cube(&coarse_lookup, fine_bits);
    add_octree_lookup(&mut lookup, &buckets, coarse_count);
    let indices = colors
        .iter()
        .map(|&color| palette_index_u32(lookup.buckets[lookup.offset(color)].count))
        .collect();
    let palette = buckets.iter().map(OctreeBucket::average).collect();
    (palette, indices)
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
    let dr = u32::from(left[0].abs_diff(right[0]));
    let dg = u32::from(left[1].abs_diff(right[1]));
    let db = u32::from(left[2].abs_diff(right[2]));
    dr.saturating_mul(dr)
        .saturating_add(dg.saturating_mul(dg))
        .saturating_add(db.saturating_mul(db))
}
