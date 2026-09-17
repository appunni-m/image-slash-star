//! Complete native containers are expectations; only semantic inputs reach Rust.
//! Behavioral execution is deferred until implementation is complete.

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

fn read_blob(root: &Path, record: &Value) -> Vec<u8> {
    let relative: String = field(record, "path");
    assert!(
        !relative.is_empty()
            && Path::new(&relative)
                .components()
                .all(|p| matches!(p, Component::Normal(_)))
    );
    let bytes = require_ok(fs::read(root.join(relative)), "native artifact");
    assert_eq!(bytes.len(), field::<usize>(record, "bytes"));
    assert_eq!(
        sha256::digest_hex(&bytes),
        field::<String>(record, "sha256")
    );
    bytes
}

fn bundle() -> (PathBuf, Vec<Value>) {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = repo.join("tests/fixtures/outputs/avif_mux");
    let index: Value = require_ok(
        json::from_str(&require_ok(
            fs::read_to_string(root.join("index.json")),
            "mux index",
        )),
        "mux JSON",
    );
    assert_eq!(
        field::<String>(&index, "schema"),
        "image-slash-star/avif-still-mux-oracle@1"
    );
    assert_eq!(
        field::<String>(&index, "native_codecs"),
        "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2"
    );
    assert_eq!(
        field::<String>(&field::<Value>(&index, "oracle"), "version"),
        "12.2.0"
    );
    assert_eq!(
        field::<String>(&field::<Value>(&index, "source"), "commit"),
        "6543b22b5bc706c53f038a16fe515f921556d9b3"
    );
    let artifacts: Vec<Value> = field(&index, "artifacts");
    let mut paths = std::collections::HashSet::new();
    for record in artifacts {
        assert!(paths.insert(field::<String>(&record, "path")));
        read_blob(&root, &record);
    }
    let cases: Vec<Value> = field(&index, "cases");
    assert_eq!(cases.len(), 28);
    assert_eq!(
        cases
            .iter()
            .filter(|case| field::<bool>(case, "registered_matrix_row"))
            .count(),
        16
    );
    let mut names = std::collections::HashSet::new();
    for case in &cases {
        assert!(names.insert(field::<String>(case, "row_id")));
        assert_eq!(field::<u32>(case, "native_observations"), 2);
        let source: Value = field(case, "source");
        let bytes = require_ok(
            fs::read(repo.join(field::<String>(&source, "path"))),
            "native source",
        );
        assert_eq!(
            sha256::digest_hex(&bytes),
            field::<String>(&source, "sha256")
        );
        let observations: Value = field(case, "observations");
        let native = read_blob(&root, &field::<Value>(&observations, "encoded"));
        for span in field::<Vec<Value>>(case, "input_provenance") {
            let path: String = field(&span, "input_path");
            assert!(paths.contains(&path));
            let input = require_ok(fs::read(root.join(path)), "extracted input");
            let start: usize = field(&span, "native_offset");
            let end = require_some(start.checked_add(input.len()), "span end");
            assert_eq!(
                input.as_slice(),
                require_some(native.get(start..end), "native source span")
            );
            assert_eq!(input.len(), field::<usize>(&span, "bytes"));
            assert_eq!(sha256::digest_hex(&input), field::<String>(&span, "sha256"));
        }
    }
    (root, cases)
}

struct Inputs {
    semantic: Value,
    color: Vec<u8>,
    color_configuration: [u8; 4],
    alpha: Option<(Vec<u8>, [u8; 4])>,
    icc: Vec<u8>,
    exif: Vec<u8>,
    xmp: Vec<u8>,
}

fn configuration(value: &Value) -> [u8; 4] {
    field::<[u32; 4]>(value, "configuration").map(|v| require_ok(u8::try_from(v), "config byte"))
}

impl Inputs {
    fn read(root: &Path, case: &Value) -> Self {
        let semantic: Value = field(case, "mux_inputs");
        let color: Value = field(&semantic, "color");
        let alpha = field::<Option<Value>>(&semantic, "alpha").map(|value| {
            (
                read_blob(root, &field::<Value>(&value, "payload")),
                configuration(&value),
            )
        });
        let metadata = |name| {
            field::<Option<Value>>(&semantic, name)
                .map_or_else(Vec::new, |record| read_blob(root, &record))
        };
        Self {
            color: read_blob(root, &field::<Value>(&color, "payload")),
            color_configuration: configuration(&color),
            alpha,
            icc: metadata("icc"),
            exif: metadata("exif"),
            xmp: metadata("xmp"),
            semantic,
        }
    }

    fn view(&self) -> img::__coverage_avif_mux::Input<'_> {
        let transform = |name| {
            field::<Option<u32>>(&self.semantic, name)
                .map(|v| require_ok(u8::try_from(v), "transform byte"))
        };
        img::__coverage_avif_mux::Input {
            dimensions: field(&self.semantic, "dimensions"),
            color: (&self.color, self.color_configuration),
            alpha: self
                .alpha
                .as_ref()
                .map(|(bytes, config)| (bytes.as_slice(), *config)),
            cicp: field(&self.semantic, "cicp"),
            full_range: field(&self.semantic, "full_range"),
            premultiplied: field(&self.semantic, "premultiplied"),
            icc: &self.icc,
            exif: &self.exif,
            xmp: &self.xmp,
            rotation: transform("rotation"),
            mirror: transform("mirror"),
        }
    }
}

#[test]
fn still_mux_matches_complete_independent_native_files() {
    let (root, cases) = bundle();
    for case in cases {
        let owned = Inputs::read(&root, &case);
        let input = owned.view();
        let observations: Value = field(&case, "observations");
        let expected = read_blob(&root, &field::<Value>(&observations, "encoded"));
        let exact_length = require_ok(u64::try_from(expected.len()), "native length");
        let policy = img::EncodePolicy::new().with_max_output_bytes(exact_length);
        for _ in 0..2 {
            assert_eq!(
                require_ok(
                    img::__coverage_avif_mux::write(&input, policy, None),
                    "native still mux"
                ),
                expected,
                "{}",
                field::<String>(&case, "row_id")
            );
        }
        let error = match img::__coverage_avif_mux::write(
            &input,
            policy.with_max_output_bytes(exact_length.saturating_sub(1)),
            None,
        ) {
            Ok(_) => panic!("output policy accepted too many bytes"),
            Err(error) => error,
        };
        assert!(
            matches!(error, img::ImageError::LimitExceeded { resource: img::ResourceLimit::EncodedOutputBytes, maximum, observed, .. } if maximum == exact_length.saturating_sub(1) && observed == exact_length)
        );
    }
}

#[test]
fn mux_model_validation_and_interruption_are_transactional() {
    let (root, cases) = bundle();
    let case = require_some(
        cases
            .iter()
            .find(|c| field::<String>(c, "row_id") == "mux_alpha_metadata"),
        "combined native case",
    );
    let owned = Inputs::read(&root, case);
    let policy = img::EncodePolicy::new();
    let token = img::CancellationToken::new();
    token.cancel();
    assert!(matches!(
        img::__coverage_avif_mux::write(&owned.view(), policy, Some(&token)),
        Err(img::ImageError::Cancelled { .. })
    ));
    for variation in 0..12 {
        let mut input = owned.view();
        match variation {
            0 => input.dimensions[0] = 0,
            1 => input.color.0 = &[],
            2 => input.color.1[0] = 0,
            3 => input.color.1[3] = 1,
            4 => input.rotation = Some(4),
            5 => input.mirror = Some(2),
            6 => input.alpha = Some(input.color),
            7 => input.exif = b"\0\0\0\x10II*\0",
            8 => input.exif = b"\xff\xff\xff\xffII*\0",
            9 => input.exif = b"\xff\xff\xff\xfcII*\0",
            10 => input.color.1[2] |= 128,
            _ => input.color.1[1] = 24,
        }
        assert!(matches!(
            img::__coverage_avif_mux::write(&input, policy, None),
            Err(img::ImageError::Parameter { .. } | img::ImageError::Dimensions { .. })
        ));
    }
    for value in [0, u64::from(u32::MAX)] {
        assert_eq!(
            u64::from(require_ok(
                img::__coverage_avif_mux::checked_length(value),
                "representable mux length"
            )),
            value
        );
    }
    for value in [u64::from(u32::MAX).saturating_add(1), u64::MAX] {
        assert!(matches!(
            img::__coverage_avif_mux::checked_length(value),
            Err(img::ImageError::Unsupported { .. })
        ));
    }
    // These are Rust-only work-budget states, not native codec observations.
    // Sweep every checkpoint through sizing and emission, then prove retries
    // still produce the original complete output with a sufficient budget.
    let expected = require_ok(
        img::__coverage_avif_mux::write(&owned.view(), policy, None),
        "unlimited mux",
    );
    let mut finished = false;
    for maximum in 0..2_000 {
        match img::__coverage_avif_mux::write(
            &owned.view(),
            policy.with_max_work_units(maximum),
            None,
        ) {
            Ok(bytes) => {
                assert_eq!(bytes, expected);
                finished = true;
                break;
            }
            Err(img::ImageError::LimitExceeded {
                resource: img::ResourceLimit::EncodeWorkUnits,
                maximum: actual,
                observed,
                ..
            }) => {
                assert_eq!(actual, maximum);
                assert_eq!(observed, maximum.saturating_add(1));
            }
            Err(error) => panic!("unexpected work-budget error: {error}"),
        }
    }
    assert!(finished, "work-budget sweep did not reach completion");
}
