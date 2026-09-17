//! Full-file native color evidence; AV1 and sequence execution stay separate.

use super::{FromJson, Value, img, json, require_ok, require_some, sha256};
use std::fs;
use std::path::Path;

fn field<T: FromJson>(value: &Value, name: &str) -> T {
    let object = require_some(value.as_object(), "oracle record must be an object");
    require_ok(
        T::from_json(require_some(object.get(name), "oracle field").clone()),
        name,
    )
}

fn exact_keys(value: &Value, keys: &[&str]) {
    let object = require_some(value.as_object(), "oracle record must be an object");
    assert_eq!(object.len(), keys.len(), "oracle schema has unknown fields");
    assert!(keys.iter().all(|key| object.contains_key(*key)));
}

fn load_frames() -> Vec<(img::Av1ReconstructionTrace, Vec<u16>, Vec<u8>)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bundle = root.join("tests/fixtures/outputs/avif_sequence_color/high_bitdepth");
    let index: Value = require_ok(
        json::from_str(&require_ok(
            fs::read_to_string(bundle.join("index.json")),
            "oracle index",
        )),
        "oracle JSON",
    );
    exact_keys(
        &index,
        &[
            "schema",
            "fixture",
            "declaration",
            "frames",
            "tracks",
            "sources",
            "oracle",
            "layout",
            "checks",
            "artifacts",
            "limitations",
        ],
    );
    assert_eq!(
        field::<String>(&index, "schema"),
        "image-slash-star/avif-sequence-color-oracle@1"
    );
    let fixture: Value = field(&index, "fixture");
    exact_keys(&fixture, &["path", "bytes", "sha256"]);
    assert_eq!(
        field::<String>(&fixture, "path"),
        "tests/fixtures/input/images/avif/10bit.avif"
    );
    let data = require_ok(
        fs::read(root.join(field::<String>(&fixture, "path"))),
        "full AVIF input",
    );
    assert_eq!(data.len(), field::<usize>(&fixture, "bytes"));
    assert_eq!(
        sha256::digest_hex(&data),
        field::<String>(&fixture, "sha256")
    );
    let sources: Value = field(&index, "sources");
    exact_keys(&sources, &["dav1d", "libavif", "libyuv"]);
    for (name, commit) in [
        ("dav1d", "b546257f770768b2c88258c533da38b91a06f737"),
        ("libavif", "6543b22b5bc706c53f038a16fe515f921556d9b3"),
        ("libyuv", "6067afde563c3946eebd94f146b3824ab7a97a9c"),
    ] {
        let source: Value = field(&sources, name);
        assert_eq!(field::<String>(&source, "commit"), commit);
    }
    let artifacts: Vec<Value> = field(&index, "artifacts");
    let mut expected_names = vec!["observer.cc".to_owned()];
    for frame in 0..5 {
        expected_names.push(format!("frame_{frame}.yuva"));
        expected_names.push(format!("frame_{frame}.rgba"));
    }
    let mut actual_names = Vec::new();
    for artifact in artifacts {
        exact_keys(&artifact, &["path", "bytes", "sha256"]);
        let name: String = field(&artifact, "path");
        assert!(expected_names.contains(&name));
        let bytes = require_ok(fs::read(bundle.join(&name)), "oracle artifact");
        assert_eq!(bytes.len(), field::<usize>(&artifact, "bytes"));
        assert_eq!(
            sha256::digest_hex(&bytes),
            field::<String>(&artifact, "sha256")
        );
        actual_names.push(name);
    }
    actual_names.sort();
    expected_names.sort();
    assert_eq!(actual_names, expected_names);
    let declaration: Value = field(&index, "declaration");
    exact_keys(
        &declaration,
        &[
            "width",
            "height",
            "bit_depth",
            "monochrome",
            "color_primaries",
            "transfer_characteristics",
            "matrix_coefficients",
            "color_range",
            "subsampling_x",
            "subsampling_y",
            "alpha",
        ],
    );
    assert!(field::<bool>(&declaration, "alpha"));
    let frames: Vec<Value> = field(&index, "frames");
    assert_eq!(frames.len(), 5);
    frames
        .iter()
        .enumerate()
        .map(|(ordinal, frame)| {
            exact_keys(
                frame,
                &[
                    "index",
                    "timescale",
                    "pts",
                    "duration",
                    "repetition_count",
                    "pillow_duration_ms",
                    "pillow_loop",
                    "sample_ranges",
                ],
            );
            assert_eq!(field::<usize>(frame, "index"), ordinal);
            let raw = require_ok(
                fs::read(bundle.join(format!("frame_{ordinal}.yuva"))),
                "native YUVA",
            );
            assert_eq!(raw.len(), 24_576);
            let (words, remainder) = raw.as_chunks::<2>();
            assert!(remainder.is_empty());
            let samples: Vec<u16> = words.iter().map(|word| u16::from_le_bytes(*word)).collect();
            let input = img::Av1ReconstructionTrace {
                width: field(&declaration, "width"),
                height: field(&declaration, "height"),
                bit_depth: field(&declaration, "bit_depth"),
                monochrome: field(&declaration, "monochrome"),
                color_primaries: field(&declaration, "color_primaries"),
                transfer_characteristics: field(&declaration, "transfer_characteristics"),
                matrix_coefficients: field(&declaration, "matrix_coefficients"),
                color_range: field(&declaration, "color_range"),
                subsampling_x: field(&declaration, "subsampling_x"),
                subsampling_y: field(&declaration, "subsampling_y"),
                planes: [
                    samples[..4096].to_vec(),
                    samples[4096..6144].to_vec(),
                    samples[6144..8192].to_vec(),
                ],
                entropy_operations: Vec::new(),
            };
            assert_eq!((input.width, input.height, input.bit_depth), (64, 64, 12));
            assert_eq!(
                (
                    input.color_primaries,
                    input.transfer_characteristics,
                    input.matrix_coefficients
                ),
                (2, 2, 2)
            );
            assert!(
                !input.monochrome
                    && !input.color_range
                    && input.subsampling_x
                    && !input.subsampling_y
            );
            let rgba = require_ok(
                fs::read(bundle.join(format!("frame_{ordinal}.rgba"))),
                "Pillow RGBA",
            );
            assert_eq!(rgba.len(), 16_384);
            (input, samples[8192..].to_vec(), rgba)
        })
        .collect()
}

#[test]
fn highdepth_limited_422_alpha_matches_all_five_native_frames() {
    if super::matrix_selection_is_filtered() {
        return;
    }
    let mut straight_alpha_witnesses = 0_usize;
    for (frame, (input, alpha, expected)) in load_frames().into_iter().enumerate() {
        straight_alpha_witnesses = straight_alpha_witnesses.saturating_add(
            expected
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|pixel| {
                    pixel[3] != 0
                        && pixel[3] != 255
                        && pixel[..3].iter().any(|channel| *channel > pixel[3])
                })
                .count(),
        );
        let actual = require_ok(
            img::__coverage_av1_color_conversion(input.clone(), Some(alpha.clone())),
            "12-bit limited-range color conversion",
        );
        assert_eq!(
            (actual.width, actual.height, actual.color),
            (64, 64, img::ColorType::Rgba8)
        );
        assert_eq!(actual.pixels, expected, "frame {frame}");
        // I422 interpolation is row-local. Preserve full real rows and their
        // original left/right endpoints while varying the canvas height.
        for (start_row, rows) in [(0_usize, 1_usize), (1, 3), (7, 8), (31, 17), (63, 1)] {
            let end_row = require_some(start_row.checked_add(rows), "last row");
            let mut slice = input.clone();
            slice.height = require_ok(u32::try_from(rows), "slice height");
            for (plane_index, plane) in slice.planes.iter_mut().enumerate() {
                let stride = if plane_index == 0 { 64 } else { 32 };
                let start = require_some(start_row.checked_mul(stride), "plane start");
                let end = require_some(end_row.checked_mul(stride), "plane end");
                *plane = input.planes[plane_index][start..end].to_vec();
            }
            let start = require_some(start_row.checked_mul(64), "alpha start");
            let end = require_some(end_row.checked_mul(64), "alpha end");
            let actual = require_ok(
                img::__coverage_av1_color_conversion(slice, Some(alpha[start..end].to_vec())),
                "row-local conversion",
            );
            let byte_start = require_some(start.checked_mul(4), "RGBA start");
            let byte_end = require_some(end.checked_mul(4), "RGBA end");
            assert_eq!(actual.pixels, expected[byte_start..byte_end]);
        }
    }
    assert!(
        straight_alpha_witnesses > 0,
        "source must distinguish straight from premultiplied alpha"
    );
}

#[test]
fn highdepth_limited_color_retains_exact_declaration_and_sample_bounds() {
    if super::matrix_selection_is_filtered() {
        return;
    }
    let (input, alpha, _) = require_some(load_frames().into_iter().next(), "first native frame");
    let reject = |candidate, alpha| {
        assert!(matches!(
            img::__coverage_av1_color_conversion(candidate, alpha),
            Err(img::ImageError::Unsupported { .. })
        ))
    };
    reject(input.clone(), None);
    for depth in [8, 10, 14] {
        let mut candidate = input.clone();
        candidate.bit_depth = depth;
        let shift = 12_u32.saturating_sub(depth);
        for plane in &mut candidate.planes {
            for sample in plane {
                *sample = sample.wrapping_shr(shift);
            }
        }
        let candidate_alpha = alpha
            .iter()
            .map(|sample| sample.wrapping_shr(shift))
            .collect();
        reject(candidate, Some(candidate_alpha));
    }
    let mut candidate = input.clone();
    candidate.color_range = true;
    reject(candidate, Some(alpha.clone()));
    for (x, y) in [(false, false), (true, true), (false, true)] {
        let mut candidate = input.clone();
        candidate.subsampling_x = x;
        candidate.subsampling_y = y;
        // Keep nominal plane extents valid so sample validation cannot hide
        // an accidentally broadened color declaration gate.
        let chroma_length = match (x, y) {
            (false, false) => 4096,
            (true, true) => 1024,
            (false, true) | (true, false) => 2048,
        };
        candidate.planes[1].resize(chroma_length, input.planes[1][0]);
        candidate.planes[2].resize(chroma_length, input.planes[2][0]);
        reject(candidate, Some(alpha.clone()));
    }
    let mut candidate = input.clone();
    candidate.matrix_coefficients = 6;
    reject(candidate, Some(alpha.clone()));
    for index in 0..3 {
        let mut candidate = input.clone();
        candidate.planes[index][0] = 4096;
        reject(candidate, Some(alpha.clone()));
        let mut candidate = input.clone();
        candidate.planes[index].pop();
        reject(candidate, Some(alpha.clone()));
    }
    let mut invalid_alpha = alpha.clone();
    invalid_alpha[0] = 4096;
    reject(input.clone(), Some(invalid_alpha));
    let mut invalid_alpha = alpha;
    invalid_alpha.pop();
    reject(input, Some(invalid_alpha));
}
