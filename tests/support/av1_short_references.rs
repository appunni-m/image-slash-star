//! Native selection traces and full-file sequence witnesses; execution deferred.

use super::{FromJson, Value, img, json, require_ok, require_some, sha256};
use std::fs;
use std::path::{Component, Path, PathBuf};

fn field<T: FromJson>(value: &Value, name: &str) -> T {
    let object = require_some(value.as_object(), "native record");
    require_ok(
        T::from_json(require_some(object.get(name), name).clone()),
        name,
    )
}

fn exact_keys(value: &Value, keys: &[&str]) {
    let object = require_some(value.as_object(), "native record");
    assert_eq!(object.len(), keys.len(), "unrecognized oracle field");
    assert!(keys.iter().all(|key| object.contains_key(*key)));
}

fn read_bundle() -> (PathBuf, Vec<Value>) {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = repository.join("tests/fixtures/outputs/av1_short_references");
    let index: Value = require_ok(
        json::from_str(&require_ok(
            fs::read_to_string(root.join("index.json")),
            "native index",
        )),
        "native JSON",
    );
    exact_keys(
        &index,
        &[
            "artifacts",
            "builds",
            "cases",
            "oracle",
            "schema",
            "source",
            "source_boundary",
            "source_fixture",
            "target_execution",
        ],
    );
    assert_eq!(
        field::<String>(&index, "schema"),
        "image-slash-star/av1-short-references-oracle@1"
    );
    let source: Value = field(&index, "source");
    assert_eq!(
        field::<String>(&source, "commit"),
        "b546257f770768b2c88258c533da38b91a06f737"
    );
    let fixture: Value = field(&index, "source_fixture");
    let original = require_ok(
        fs::read(repository.join(field::<String>(&fixture, "path"))),
        "full source",
    );
    assert_eq!(original.len(), field::<usize>(&fixture, "bytes"));
    assert_eq!(
        sha256::digest_hex(&original),
        field::<String>(&fixture, "sha256")
    );
    let oracle: Value = field(&index, "oracle");
    assert_eq!(field::<String>(&oracle, "pillow"), "12.2.0");
    assert_eq!(field::<String>(&oracle, "libavif"), "1.4.1");
    assert_eq!(
        field::<String>(&oracle, "codecs"),
        "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2"
    );
    let artifacts: Vec<Value> = field(&index, "artifacts");
    assert_eq!(artifacts.len(), 49);
    let mut seen = std::collections::HashSet::new();
    for artifact in artifacts {
        exact_keys(&artifact, &["path", "bytes", "sha256"]);
        let path: String = field(&artifact, "path");
        assert!(
            !path.is_empty()
                && Path::new(&path)
                    .components()
                    .all(|part| matches!(part, Component::Normal(_)))
        );
        assert!(seen.insert(path.clone()));
        let bytes = require_ok(fs::read(root.join(path)), "native artifact");
        assert_eq!(bytes.len(), field::<usize>(&artifact, "bytes"));
        assert_eq!(
            sha256::digest_hex(&bytes),
            field::<String>(&artifact, "sha256")
        );
    }
    let cases: Vec<Value> = field(&index, "cases");
    assert_eq!(cases.len(), 6);
    let mut names = std::collections::HashSet::new();
    for case in &cases {
        exact_keys(
            case,
            &[
                "name",
                "input_path",
                "input_bytes",
                "input_sha256",
                "native",
                "pillow",
                "native_yuv_bytes",
                "native_yuv_sha256",
                "native_noninterference",
                "native_repeat_equal",
                "pillow_repeat_equal",
                "syntax_matches_native",
            ],
        );
        let name: String = field(case, "name");
        assert!(matches!(
            name.as_str(),
            "past_distinct"
                | "equal_distinct"
                | "wrapped_future_distinct"
                | "past_duplicate"
                | "equal_duplicate"
                | "wrapped_future_duplicate"
        ));
        assert!(names.insert(name.clone()));
        for flag in [
            "native_noninterference",
            "native_repeat_equal",
            "pillow_repeat_equal",
            "syntax_matches_native",
        ] {
            assert!(field::<bool>(case, flag));
        }
        let native: Value = field(case, "native");
        exact_keys(
            &native,
            &[
                "kind",
                "event",
                "order_hint_bits",
                "order_hint",
                "last",
                "golden",
                "reference_hints",
                "distances",
                "selected",
            ],
        );
        let native_file: Value = require_ok(
            json::from_str(&require_ok(
                fs::read_to_string(root.join(&name).join("native.json")),
                "native event",
            )),
            "event JSON",
        );
        assert_eq!(native, native_file);
        let input_path: String = field(case, "input_path");
        assert_eq!(input_path, format!("{name}/input.avif"));
        assert!(seen.contains(&input_path));
        let bytes = require_ok(fs::read(root.join(input_path)), "complete variant");
        assert_eq!(bytes.len(), field::<usize>(case, "input_bytes"));
        assert_eq!(
            sha256::digest_hex(&bytes),
            field::<String>(case, "input_sha256")
        );
    }
    (root, cases)
}

#[test]
fn complete_short_reference_sequences_match_native_frames() {
    if super::matrix_selection_is_filtered() {
        return;
    }
    let (root, cases) = read_bundle();
    for case in cases {
        let name: String = field(&case, "name");
        let input = require_ok(
            fs::read(root.join(field::<String>(&case, "input_path"))),
            "complete AVIF",
        );
        let actual = require_ok(img::decode_sequence(&input), &name);
        let pillow: Value = field(&case, "pillow");
        let frames: Vec<Value> = field(&pillow, "frames");
        assert_eq!(actual.content.frames.len(), frames.len());
        for (frame, expected) in actual.content.frames.iter().zip(&frames) {
            let pixels = require_ok(
                fs::read(root.join(&name).join(field::<String>(expected, "raw_path"))),
                "native RGB",
            );
            assert_eq!(
                sha256::digest_hex(&pixels),
                field::<String>(expected, "raw_sha256")
            );
            assert_eq!(frame.image.pixels, pixels, "{name}");
            assert_eq!(frame.image.mode, img::ImageMode::Rgb8);
            assert_eq!((frame.image.width, frame.image.height), (16, 16));
            assert_eq!(
                frame.source.duration,
                img::FrameDuration {
                    numerator: field(expected, "native_duration"),
                    denominator: field(expected, "native_timescale"),
                }
            );
        }
        let first = require_ok(img::decode(&input), "independent first image");
        assert_eq!(first.content.pixels, actual.content.frames[0].image.pixels);
    }
}

#[cfg(coverage)]
#[test]
fn selected_short_reference_indices_match_native_decoder_state() {
    if super::matrix_selection_is_filtered() {
        return;
    }
    let (_, cases) = read_bundle();
    for case in cases {
        let native: Value = field(&case, "native");
        let width = field(&native, "order_hint_bits");
        let hint = field(&native, "order_hint");
        let hints: [u32; 8] = field(&native, "reference_hints");
        let last = field(&native, "last");
        let golden = field(&native, "golden");
        let expected: [usize; 7] = field(&native, "selected");
        assert_eq!(
            require_ok(
                img::__coverage_av1_short_references(width, hint, &hints, last, golden),
                "native selection"
            ),
            expected
        );
        // These altered scalar arguments are impossible internal model states,
        // not additional malformed-file observations attributed to Pillow.
        for (bad_width, bad_hint, bad_last, bad_golden) in [
            (0, hint, last, golden),
            (9, hint, last, golden),
            (width, 128, last, golden),
            (width, hint, 8, golden),
            (width, hint, last, 8),
        ] {
            let error = match img::__coverage_av1_short_references(
                bad_width, bad_hint, &hints, bad_last, bad_golden,
            ) {
                Ok(_) => panic!("invalid model boundary was accepted"),
                Err(error) => error,
            };
            assert_eq!(error.kind(), img::ImageErrorKind::Malformed);
        }
        let mut bad_hints = hints;
        bad_hints[7] = 128;
        assert!(
            img::__coverage_av1_short_references(width, hint, &bad_hints, last, golden).is_err()
        );
    }
}
