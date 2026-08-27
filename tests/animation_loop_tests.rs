//! Common animation-loop semantics and target-conversion contracts.

use image_slash_star::{
    AnimationLoop, ColorType, DecodedFrame, DecodedImage, DecodedSequence, EncodeOptions,
    FrameBlend, FrameDisposal, FrameDuration, FrameRect, ImageFormat, SequenceKind, SourceColor,
};

use bytemuck as _;
#[cfg(feature = "jpeg")]
use wide as _;

fn two_frame_sequence(loop_count: AnimationLoop) -> DecodedSequence {
    let image = DecodedImage::new(1, 1, vec![0], ColorType::L8);
    let frame = || {
        DecodedFrame::rendered_canvas(
            image.clone(),
            FrameRect {
                left: 0,
                top: 0,
                width: 1,
                height: 1,
            },
            FrameDuration::ZERO,
            FrameDisposal::Unspecified,
            FrameBlend::Unspecified,
        )
    };
    DecodedSequence {
        width: 1,
        height: 1,
        frames: vec![frame(), frame()],
        loop_count,
        background: None,
        kind: SequenceKind::TimedAnimation,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    }
}

#[test]
fn loop_values_keep_all_four_states() {
    assert!(AnimationLoop::Unspecified.is_unspecified());
    assert!(AnimationLoop::Infinite.is_infinite());
    assert!(AnimationLoop::Unknown.is_unknown());
    assert_eq!(AnimationLoop::finite(7).finite_total_plays(), Some(7));
    assert_eq!(
        AnimationLoop::Finite { total_plays: 0 }.finite_total_plays(),
        Some(0)
    );
    assert_eq!(AnimationLoop::Unspecified.finite_total_plays(), None);
}

#[cfg(feature = "gif")]
#[test]
fn gif_maps_common_loop_states_without_silent_zero_reinterpretation() {
    let options = EncodeOptions::for_format(ImageFormat::Gif);

    let unspecified = image_slash_star::encode_sequence(
        &two_frame_sequence(AnimationLoop::Unspecified),
        ImageFormat::Gif,
        &options,
    )
    .unwrap_or_else(|error| panic!("GIF unspecified loop failed: {error}"));
    assert!(
        !unspecified
            .windows(b"NETSCAPE2.0".len())
            .any(|window| window == b"NETSCAPE2.0")
    );

    let infinite = image_slash_star::encode_sequence(
        &two_frame_sequence(AnimationLoop::Infinite),
        ImageFormat::Gif,
        &options,
    )
    .unwrap_or_else(|error| panic!("GIF infinite loop failed: {error}"));
    assert!(
        infinite
            .windows(b"NETSCAPE2.0".len())
            .any(|window| window == b"NETSCAPE2.0")
    );
    assert!(
        infinite
            .windows(3)
            .any(|window| window == [0x00, 0x00, 0x00])
    );

    let finite = image_slash_star::encode_sequence(
        &two_frame_sequence(AnimationLoop::Finite { total_plays: 3 }),
        ImageFormat::Gif,
        &options,
    )
    .unwrap_or_else(|error| panic!("GIF finite loop failed: {error}"));
    assert!(finite.windows(3).any(|window| window == [0x02, 0x00, 0x00]));

    let zero = image_slash_star::encode_sequence(
        &two_frame_sequence(AnimationLoop::Finite { total_plays: 0 }),
        ImageFormat::Gif,
        &options,
    );
    assert!(
        zero.is_err(),
        "GIF must not reinterpret finite zero as infinite"
    );

    let unknown = image_slash_star::encode_sequence(
        &two_frame_sequence(AnimationLoop::Unknown),
        ImageFormat::Gif,
        &options,
    );
    assert!(unknown.is_err(), "GIF must reject unknown loop semantics");
}

#[cfg(feature = "tiff")]
#[test]
fn tiff_rejects_timed_loop_states_instead_of_dropping_them() {
    let options = EncodeOptions::for_format(ImageFormat::Tiff);
    for loop_count in [
        AnimationLoop::Finite { total_plays: 1 },
        AnimationLoop::Infinite,
        AnimationLoop::Unknown,
    ] {
        let result = image_slash_star::encode_sequence(
            &two_frame_sequence(loop_count),
            ImageFormat::Tiff,
            &options,
        );
        assert!(result.is_err(), "TIFF must reject {loop_count:?}");
    }
}

#[cfg(feature = "webp")]
#[test]
fn webp_checks_loop_width_and_reserved_semantics() {
    let options = EncodeOptions::for_format(ImageFormat::WebP);
    let finite = image_slash_star::encode_sequence(
        &two_frame_sequence(AnimationLoop::Finite { total_plays: 3 }),
        ImageFormat::WebP,
        &options,
    );
    assert!(finite.is_ok(), "WebP should encode a 16-bit finite count");

    for loop_count in [
        AnimationLoop::Finite { total_plays: 0 },
        AnimationLoop::Finite {
            total_plays: u32::from(u16::MAX) + 2,
        },
        AnimationLoop::Unknown,
    ] {
        let result = image_slash_star::encode_sequence(
            &two_frame_sequence(loop_count),
            ImageFormat::WebP,
            &options,
        );
        assert!(result.is_err(), "WebP must reject {loop_count:?}");
    }
}
