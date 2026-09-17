//! Deferred public sequence parity, backed by complete native file observations.

use super::{FromJson, Value, img, json, require_ok, require_some, sha256};
use std::fs;
use std::path::{Path, PathBuf};

fn field<T: FromJson>(value: &Value, name: &str) -> T {
    let object = require_some(value.as_object(), "oracle object");
    require_ok(
        T::from_json(require_some(object.get(name), "oracle field").clone()),
        name,
    )
}

fn read_index(bundle: &Path, schema: &str) -> (Value, Vec<u8>) {
    let index: Value = require_ok(
        json::from_str(&require_ok(
            fs::read_to_string(bundle.join("index.json")),
            "oracle index",
        )),
        "oracle JSON",
    );
    assert_eq!(field::<String>(&index, "schema"), schema);
    let fixture: Value = field(&index, "fixture");
    let data = require_ok(
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(field::<String>(&fixture, "path"))),
        "complete encoded fixture",
    );
    assert_eq!(data.len(), field::<usize>(&fixture, "bytes"));
    assert_eq!(
        sha256::digest_hex(&data),
        field::<String>(&fixture, "sha256")
    );
    let artifacts: Vec<Value> = field(&index, "artifacts");
    let mut seen = std::collections::HashSet::new();
    for artifact in artifacts {
        let relative: String = field(&artifact, "path");
        assert!(seen.insert(relative.clone()), "duplicate oracle artifact");
        assert!(
            Path::new(&relative)
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)))
        );
        let bytes = require_ok(fs::read(bundle.join(relative)), "native artifact");
        assert_eq!(bytes.len(), field::<usize>(&artifact, "bytes"));
        assert_eq!(
            sha256::digest_hex(&bytes),
            field::<String>(&artifact, "sha256")
        );
    }
    (index, data)
}

struct NativeSequence {
    data: Vec<u8>,
    width: u32,
    height: u32,
    mode: img::ImageMode,
    pixels: Vec<Vec<u8>>,
    durations: Vec<img::FrameDuration>,
    milliseconds: Vec<u32>,
    native_loop: img::AnimationLoop,
}

fn bundle(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/outputs")
        .join(relative)
}

fn animated() -> NativeSequence {
    let root = bundle("av1_sequence/animated");
    let (index, data) = read_index(&root, "image-slash-star/av1-sequence-oracle@2");
    let oracle: Value = field(&index, "oracle");
    assert_eq!(field::<String>(&oracle, "pillow"), "12.2.0");
    assert_eq!(field::<String>(&oracle, "libavif"), "1.4.1");
    assert!(field::<bool>(&oracle, "pillow_repeat_equal"));
    let provenance: Value = field(&index, "source_provenance");
    assert_eq!(
        field::<String>(&provenance, "dav1d_commit"),
        "b546257f770768b2c88258c533da38b91a06f737"
    );
    let roles: Value = field(&index, "roles");
    let color: Value = field(&roles, "color");
    let counts: Value = field(&color, "counts");
    assert_eq!(field::<u32>(&counts, "decoded_frame_count"), 6);
    assert_eq!(field::<u32>(&counts, "hidden_decoded_count"), 2);
    assert_eq!(field::<u32>(&counts, "show_existing_count"), 1);
    let noninterference: Value = field(&color, "noninterference");
    assert!(field::<bool>(&noninterference, "byte_equal"));
    let pillow: Value = field(&index, "pillow");
    assert_eq!(field::<Option<u32>>(&pillow, "loop_count"), None);
    let native_loop: Value = field(&index, "native_loop");
    assert_eq!(field::<i32>(&native_loop, "repetition_count"), 0);
    assert_eq!(
        field::<String>(&native_loop, "origin"),
        "libavif.avifDecoder.repetitionCount"
    );
    let frames: Vec<Value> = field(&pillow, "frames");
    assert_eq!(frames.len(), 5);
    let mut pixels = Vec::new();
    let mut durations = Vec::new();
    let mut milliseconds = Vec::new();
    for (ordinal, frame) in frames.iter().enumerate() {
        assert_eq!(field::<usize>(frame, "index"), ordinal);
        assert_eq!(field::<String>(frame, "mode"), "RGB");
        assert_eq!(field::<Vec<u32>>(frame, "size"), [150, 150]);
        let raw = require_ok(
            fs::read(root.join(field::<String>(frame, "raw_path"))),
            "Pillow RGB",
        );
        assert_eq!(raw.len(), field::<usize>(frame, "raw_bytes"));
        assert_eq!(
            sha256::digest_hex(&raw),
            field::<String>(frame, "raw_sha256")
        );
        pixels.push(raw);
        durations.push(img::FrameDuration {
            numerator: field(frame, "native_duration"),
            denominator: field(frame, "native_timescale"),
        });
        milliseconds.push(field(frame, "duration"));
    }
    NativeSequence {
        data,
        width: 150,
        height: 150,
        mode: img::ImageMode::Rgb8,
        pixels,
        durations,
        milliseconds,
        native_loop: img::AnimationLoop::Finite { total_plays: 1 },
    }
}

fn highdepth() -> NativeSequence {
    let root = bundle("avif_sequence_color/high_bitdepth");
    let (index, data) = read_index(&root, "image-slash-star/avif-sequence-color-oracle@1");
    let frames: Vec<Value> = field(&index, "frames");
    assert_eq!(frames.len(), 5);
    let mut pixels = Vec::new();
    let mut durations = Vec::new();
    let mut milliseconds = Vec::new();
    for (ordinal, frame) in frames.iter().enumerate() {
        assert_eq!(field::<usize>(frame, "index"), ordinal);
        assert_eq!(field::<Option<u32>>(frame, "pillow_loop"), None);
        assert_eq!(field::<i32>(frame, "repetition_count"), -1);
        pixels.push(require_ok(
            fs::read(root.join(format!("frame_{ordinal}.rgba"))),
            "native RGBA",
        ));
        durations.push(img::FrameDuration {
            numerator: field(frame, "duration"),
            denominator: field(frame, "timescale"),
        });
        milliseconds.push(field(frame, "pillow_duration_ms"));
    }
    NativeSequence {
        data,
        width: 64,
        height: 64,
        mode: img::ImageMode::Rgba8,
        pixels,
        durations,
        milliseconds,
        native_loop: img::AnimationLoop::Infinite,
    }
}

fn assert_frames(actual: &img::DecodedSequence, expected: &NativeSequence) {
    assert_eq!(
        (actual.width, actual.height),
        (expected.width, expected.height)
    );
    assert_eq!(actual.kind, img::SequenceKind::TimedAnimation);
    // Loop metadata is a native source observation. Pillow omits its loop
    // key for both files; that omission does not describe their edit lists.
    assert_eq!(actual.loop_count, expected.native_loop);
    assert_eq!(actual.frames.len(), expected.pixels.len());
    for (index, frame) in actual.frames.iter().enumerate() {
        assert_eq!(
            frame.image.pixels, expected.pixels[index],
            "display {index}"
        );
        assert_eq!(frame.image.mode, expected.mode);
        assert_eq!(
            (frame.image.width, frame.image.height),
            (expected.width, expected.height)
        );
        assert_eq!(frame.source.duration, expected.durations[index]);
        assert_eq!(
            require_ok(
                frame.source.duration.milliseconds_rounded(),
                "rounded frame duration"
            ),
            expected.milliseconds[index]
        );
        assert_eq!(
            frame.source.rect,
            img::FrameRect {
                left: 0,
                top: 0,
                width: expected.width,
                height: expected.height
            }
        );
        assert_eq!(frame.pixel_layout, img::FramePixelLayout::RenderedCanvas);
        assert_eq!(frame.source.disposal, img::FrameDisposal::Unspecified);
        assert_eq!(frame.source.blend, img::FrameBlend::Unspecified);
        // Match the independent AVIF movie-source normalization in
        // generate_decode_refs.py: no extra default-only display is inserted.
        assert!(!frame.source.is_default_image);
        assert!(!frame.source.interlaced);
    }
}

#[test]
fn public_avif_sequences_match_every_native_display_and_exact_timing() {
    if super::matrix_selection_is_filtered() {
        return;
    }
    for expected in [animated(), highdepth()] {
        let actual = require_ok(
            img::decode_sequence(&expected.data),
            "public sequence parity",
        );
        assert_frames(&actual.content, &expected);
        let first = require_ok(
            img::decode(&expected.data),
            "independent first/default image",
        );
        assert_eq!(first.content.pixels, expected.pixels[0]);
        assert_eq!(first.content.mode, expected.mode);
        assert_eq!(
            (first.content.width, first.content.height),
            (expected.width, expected.height)
        );
    }
}

#[test]
fn avif_sequence_transfer_budget_is_reserved_once_and_before_reconstruction() {
    if super::matrix_selection_is_filtered() {
        return;
    }
    for expected in [animated(), highdepth()] {
        let frame_bytes = require_ok(u64::try_from(expected.pixels[0].len()), "frame bytes");
        let count = require_ok(u64::try_from(expected.pixels.len()), "frame count");
        let total = require_some(frame_bytes.checked_mul(count), "sequence byte total");
        let maximum = require_some(total.checked_sub(1), "below sequence limit");
        let error = match img::decode_sequence_with_policy(
            &expected.data,
            &img::DecodePolicy::new().with_max_sequence_decoded_bytes(maximum),
        ) {
            Ok(_) => panic!("sequence exceeded its transfer limit"),
            Err(error) => error,
        };
        assert!(matches!(error, img::ImageError::LimitExceeded {
            format: Some(img::ImageFormat::Avif), operation: img::CodecOperation::SequenceDecode,
            resource: img::ResourceLimit::SequenceDecodedBytes, maximum: actual_max, observed,
        } if actual_max == maximum && observed == total));
        let policy = img::DecodePolicy::new()
            .with_max_sequence_decoded_bytes(total)
            .with_max_frame_decoded_bytes(frame_bytes);
        let exact = require_ok(
            img::decode_sequence_with_policy(&expected.data, &policy),
            "exact sequence limits",
        );
        assert_frames(&exact.content, &expected);
        let token = img::CancellationToken::new();
        token.cancel();
        let cancelled = img::decode_sequence_with_token(&expected.data, &token);
        assert!(matches!(cancelled, Err(img::ImageError::Cancelled { .. })));
    }
}
