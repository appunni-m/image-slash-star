//! Independent full-file color evidence, separate from AV1 reconstruction.

use super::{FromJson, Value, img, json, require_ok, require_some, sha256};
use std::fs;
use std::path::Path;

fn field<T: FromJson>(value: &Value, name: &str) -> T {
    let object = require_some(value.as_object(), "oracle record must be an object");
    require_ok(
        T::from_json(require_some(object.get(name), "oracle field must exist").clone()),
        name,
    )
}

fn exact_keys(value: &Value, keys: &[&str]) {
    let object = require_some(value.as_object(), "oracle record must be an object");
    assert_eq!(object.len(), keys.len(), "oracle schema has unknown fields");
    assert!(keys.iter().all(|key| object.contains_key(*key)));
}

fn read_oracle() -> (img::Av1ReconstructionTrace, Vec<u8>) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bundle = root.join("tests/fixtures/outputs/avif_hdr_color/hdr");
    let index: Value = require_ok(
        json::from_str(&require_ok(
            fs::read_to_string(bundle.join("index.json")),
            "oracle index",
        )),
        "oracle index JSON",
    );
    exact_keys(
        &index,
        &[
            "schema",
            "fixture",
            "declaration",
            "sources",
            "oracle",
            "layout",
            "sample",
            "checks",
            "artifacts",
            "limitations",
        ],
    );
    assert_eq!(
        field::<String>(&index, "schema"),
        "image-slash-star/avif-hdr-color-oracle@1"
    );
    let fixture: Value = field(&index, "fixture");
    exact_keys(&fixture, &["path", "bytes", "sha256"]);
    assert_eq!(
        field::<String>(&fixture, "path"),
        "tests/fixtures/input/images/avif/hdr.avif"
    );
    let bytes = require_ok(
        fs::read(root.join(field::<String>(&fixture, "path"))),
        "full AVIF input",
    );
    assert_eq!(bytes.len(), field::<usize>(&fixture, "bytes"));
    assert_eq!(
        sha256::digest_hex(&bytes),
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
        exact_keys(&source, &["commit", "tree_sha256", "files"]);
        assert_eq!(field::<String>(&source, "commit"), commit);
    }
    let artifacts: Vec<Value> = field(&index, "artifacts");
    let mut names = Vec::new();
    for artifact in artifacts {
        exact_keys(&artifact, &["path", "bytes", "sha256"]);
        let name: String = field(&artifact, "path");
        assert!(matches!(
            name.as_str(),
            "display.yuv" | "display.rgb" | "observer.cc"
        ));
        let bytes = require_ok(fs::read(bundle.join(&name)), "oracle artifact");
        assert_eq!(bytes.len(), field::<usize>(&artifact, "bytes"));
        assert_eq!(
            sha256::digest_hex(&bytes),
            field::<String>(&artifact, "sha256")
        );
        names.push(name);
    }
    names.sort();
    assert_eq!(names, ["display.rgb", "display.yuv", "observer.cc"]);
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
    assert!(!field::<bool>(&declaration, "alpha"));
    let yuv = require_ok(fs::read(bundle.join("display.yuv")), "native YUV");
    assert_eq!(yuv.len(), 240_000);
    let (words, remainder) = yuv.as_chunks::<2>();
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
            samples[..40_000].to_vec(),
            samples[40_000..80_000].to_vec(),
            samples[80_000..].to_vec(),
        ],
        entropy_operations: Vec::new(),
    };
    assert_eq!((input.width, input.height, input.bit_depth), (200, 200, 10));
    assert_eq!(
        (
            input.color_primaries,
            input.transfer_characteristics,
            input.matrix_coefficients
        ),
        (9, 16, 9)
    );
    assert!(input.color_range && !input.monochrome && !input.subsampling_x && !input.subsampling_y);
    let rgb = require_ok(fs::read(bundle.join("display.rgb")), "Pillow RGB");
    assert_eq!(rgb.len(), 120_000);
    (input, rgb)
}

#[test]
fn hdr_color_matches_full_file_native_planes_and_pillow_rgb() {
    if super::matrix_selection_is_filtered() {
        return;
    }
    let (input, expected) = read_oracle();
    let actual = require_ok(
        img::__coverage_av1_color_conversion(input.clone(), None),
        "HDR color conversion",
    );
    assert_eq!(
        (actual.width, actual.height, actual.color),
        (200, 200, img::ColorType::Rgb8)
    );
    assert_eq!(actual.pixels, expected);
    // Real native samples and corresponding Pillow bytes exercise vector tails.
    // These slices prove the color kernel, not newly encoded AVIF dimensions.
    for start in [0_usize, 201, 12_345, 39_969] {
        for count in [1_usize, 2, 3, 7, 8, 9, 15, 16, 17, 23, 24, 31] {
            let end = require_some(start.checked_add(count), "sample end");
            let mut slice = input.clone();
            slice.width = require_ok(u32::try_from(count), "slice width");
            slice.height = 1;
            slice.planes = input
                .planes
                .each_ref()
                .map(|plane| plane[start..end].to_vec());
            let actual = require_ok(
                img::__coverage_av1_color_conversion(slice, None),
                "HDR color tail",
            );
            let byte_start = require_some(start.checked_mul(3), "RGB start");
            let byte_end = require_some(end.checked_mul(3), "RGB end");
            assert_eq!(
                actual.pixels,
                expected[byte_start..byte_end],
                "start={start}, count={count}"
            );
        }
    }
}

#[test]
fn hdr_color_rejects_unwitnessed_declarations_and_invalid_internal_planes() {
    if super::matrix_selection_is_filtered() {
        return;
    }
    let (input, _) = read_oracle();
    let reject = |candidate, alpha| {
        assert!(matches!(
            img::__coverage_av1_color_conversion(candidate, alpha),
            Err(img::ImageError::Unsupported { .. })
        ));
    };
    // Defensive internal states; these are not Pillow malformed-file claims.
    for depth in [8, 12, 14] {
        let mut candidate = input.clone();
        candidate.bit_depth = depth;
        if depth == 8 {
            for plane in &mut candidate.planes {
                for sample in plane {
                    *sample = sample.wrapping_shr(2);
                }
            }
        }
        reject(candidate, None);
    }
    for (x, y) in [(true, false), (true, true), (false, true)] {
        let mut candidate = input.clone();
        candidate.subsampling_x = x;
        candidate.subsampling_y = y;
        let chroma_count = match (x, y) {
            (true, true) => 10_000,
            _ => 20_000,
        };
        candidate.planes[1].truncate(chroma_count);
        candidate.planes[2].truncate(chroma_count);
        reject(candidate, None);
    }
    reject(input.clone(), Some(vec![1023; 40_000]));
    let mut candidate = input.clone();
    candidate.matrix_coefficients = 10;
    reject(candidate, None);
    let mut candidate = input.clone();
    candidate.color_range = false;
    reject(candidate, None);
    let mut candidate = input.clone();
    candidate.color_primaries = 1;
    reject(candidate, None);
    let mut candidate = input.clone();
    candidate.transfer_characteristics = 13;
    reject(candidate, None);
    for plane in 0..3 {
        let mut candidate = input.clone();
        candidate.planes[plane][0] = 1024;
        reject(candidate, None);
        let mut candidate = input.clone();
        candidate.planes[plane].pop();
        reject(candidate, None);
    }
}
