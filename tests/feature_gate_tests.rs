//! Cargo-feature and target-capability behavior driven by Pillow fixtures.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use bytemuck as _;
use image_slash_star::{
    Capability, CapabilityRestriction, CapabilityTarget, CapabilityUnavailableReason, ColorType,
    DecodedImage, DecodedSequence, EncodeOptions, EncodedImage, ImageError, ImageErrorStage,
    ImageFormat, ImageMode, SequenceKind, SourceColor,
};

mod support;

use support::json::{self, FromJson, Object, Value};

struct CoverageMatrix {
    formats: HashMap<String, FormatRows>,
}

struct FormatRows {
    decode: Vec<DecodeRow>,
}

struct DecodeRow {
    status: String,
    asset: Option<String>,
    expect_error: bool,
    oracle_detects_format: bool,
    ref_mode: Option<String>,
    ref_size: Option<[u32; 2]>,
    verify_status: Option<String>,
}

struct EncodeOptionErrorManifest {
    format_version: u32,
    assertion_origin: String,
    cases: Vec<EncodeOptionErrorRow>,
}

struct EncodeOptionAcceptanceManifest {
    format_version: u32,
    assertion_origin: String,
    cases: Vec<EncodeOptionAcceptanceRow>,
}

struct EncodeOptionErrorRow {
    id: String,
    format: String,
    pairs: Vec<[String; 2]>,
    message_contains: String,
}

struct EncodeOptionAcceptanceRow {
    id: String,
    format: String,
    pairs: Vec<[String; 2]>,
}

impl FromJson for CoverageMatrix {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            formats: object.take("formats")?,
        })
    }
}

impl FromJson for FormatRows {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            decode: object.take("decode")?,
        })
    }
}

impl FromJson for DecodeRow {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            status: object.take("status")?,
            asset: object.take("asset")?,
            expect_error: object.take_or_default("expect_error")?,
            oracle_detects_format: object.take("oracle_detects_format")?,
            ref_mode: object.take("ref_mode")?,
            ref_size: object.take("ref_size")?,
            verify_status: object.take("verify_status")?,
        })
    }
}

impl FromJson for EncodeOptionErrorManifest {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            format_version: object.take("format_version")?,
            assertion_origin: object.take("assertion_origin")?,
            cases: object.take("cases")?,
        })
    }
}

impl FromJson for EncodeOptionAcceptanceManifest {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            format_version: object.take("format_version")?,
            assertion_origin: object.take("assertion_origin")?,
            cases: object.take("cases")?,
        })
    }
}

impl FromJson for EncodeOptionErrorRow {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            id: object.take("id")?,
            format: object.take("format")?,
            pairs: object.take("pairs")?,
            message_contains: object.take("message_contains")?,
        })
    }
}

impl FromJson for EncodeOptionAcceptanceRow {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            id: object.take("id")?,
            format: object.take("format")?,
            pairs: object.take("pairs")?,
        })
    }
}

fn format(name: &str) -> (ImageFormat, &'static str, bool) {
    match name {
        "jpeg" => (ImageFormat::Jpeg, "jpeg", cfg!(feature = "jpeg")),
        "png" => (ImageFormat::Png, "png", cfg!(feature = "png")),
        "gif" => (ImageFormat::Gif, "gif", cfg!(feature = "gif")),
        "bmp" => (ImageFormat::Bmp, "bmp", cfg!(feature = "bmp")),
        "tiff" => (ImageFormat::Tiff, "tiff", cfg!(feature = "tiff")),
        "webp" => (ImageFormat::WebP, "webp", cfg!(feature = "webp")),
        "ico" => (ImageFormat::Ico, "ico", cfg!(feature = "ico")),
        "avif" => (ImageFormat::Avif, "avif", cfg!(feature = "avif")),
        other => panic!("unknown manifest format {other}"),
    }
}

fn mode(name: &str) -> ImageMode {
    match name {
        "1" | "L1" => ImageMode::L1,
        "P" | "P8" => ImageMode::P8,
        "L" | "L8" => ImageMode::L8,
        "LA" | "La8" => ImageMode::La8,
        "RGB" | "Rgb8" => ImageMode::Rgb8,
        "RGBA" | "Rgba8" => ImageMode::Rgba8,
        "CMYK" | "Cmyk8" => ImageMode::Cmyk8,
        "I;16" | "L16" => ImageMode::L16,
        "La16" => ImageMode::La16,
        "Rgb16" => ImageMode::Rgb16,
        "Rgba16" => ImageMode::Rgba16,
        "Rgb32F" => ImageMode::Rgb32F,
        "Rgba32F" => ImageMode::Rgba32F,
        "F" | "F32" => ImageMode::F32,
        "I" | "I32" => ImageMode::I32,
        other => panic!("unknown manifest image mode {other}"),
    }
}

fn unavailable(reason: CapabilityUnavailableReason) -> Capability {
    Capability::Unavailable(reason)
}

fn assert_capability_contract(format: ImageFormat, feature: &str, enabled: bool) {
    let capabilities = format.capabilities();
    assert_eq!(capabilities.format(), format);
    assert_eq!(format.feature_name(), feature);
    assert_eq!(capabilities.target(), CapabilityTarget::current());
    assert_eq!(capabilities.feature_enabled(), enabled);
    assert_eq!(capabilities.detection(), Capability::ManifestBounded);

    if !enabled {
        let disabled = unavailable(CapabilityUnavailableReason::FeatureDisabled);
        assert_eq!(capabilities.inspection(), disabled);
        assert_eq!(capabilities.still_decode(), disabled);
        assert_eq!(capabilities.still_encode(), disabled);
        assert_eq!(capabilities.sequence_decode(), disabled);
        assert_eq!(capabilities.sequence_encode(), disabled);
        return;
    }

    assert_eq!(capabilities.inspection(), Capability::ManifestBounded);
    if cfg!(target_arch = "wasm32") && format == ImageFormat::Avif {
        let target_unavailable = unavailable(CapabilityUnavailableReason::TargetUnavailable);
        assert_eq!(
            capabilities.still_decode(),
            Capability::Restricted(CapabilityRestriction::PortableAvif)
        );
        assert_eq!(capabilities.still_encode(), target_unavailable);
        assert_eq!(capabilities.sequence_decode(), target_unavailable);
        assert_eq!(capabilities.sequence_encode(), target_unavailable);
        return;
    }

    assert_eq!(capabilities.still_decode(), Capability::ManifestBounded);
    assert_eq!(capabilities.still_encode(), Capability::ManifestBounded);
    let not_implemented = unavailable(CapabilityUnavailableReason::NotImplemented);
    match format {
        ImageFormat::Png => {
            assert_eq!(capabilities.sequence_decode(), Capability::ManifestBounded);
            assert_eq!(capabilities.sequence_encode(), not_implemented);
        }
        ImageFormat::Gif | ImageFormat::WebP | ImageFormat::Tiff | ImageFormat::Avif => {
            assert_eq!(capabilities.sequence_decode(), Capability::ManifestBounded);
            assert_eq!(capabilities.sequence_encode(), Capability::ManifestBounded);
        }
        ImageFormat::Jpeg | ImageFormat::Bmp | ImageFormat::Ico => {
            assert_eq!(capabilities.sequence_decode(), not_implemented);
            assert_eq!(capabilities.sequence_encode(), not_implemented);
        }
        _ => panic!("capability fixture does not cover a newly added image format"),
    }
}

#[test]
fn extension_aliases_and_mime_queries_match_the_public_contract() {
    let cases: &[(&str, ImageFormat, &str, &str, &[&str])] = &[
        (
            "jpeg",
            ImageFormat::Jpeg,
            "image/jpeg",
            "jpg",
            &["jpg", "jpeg", "jfif", "jpe"],
        ),
        (
            "png",
            ImageFormat::Png,
            "image/png",
            "png",
            &["png", "apng"],
        ),
        ("gif", ImageFormat::Gif, "image/gif", "gif", &["gif"]),
        ("bmp", ImageFormat::Bmp, "image/bmp", "bmp", &["bmp"]),
        ("webp", ImageFormat::WebP, "image/webp", "webp", &["webp"]),
        (
            "tiff",
            ImageFormat::Tiff,
            "image/tiff",
            "tiff",
            &["tiff", "tif"],
        ),
        (
            "ico",
            ImageFormat::Ico,
            "image/x-icon",
            "ico",
            &["ico", "cur"],
        ),
        (
            "avif",
            ImageFormat::Avif,
            "image/avif",
            "avif",
            &["avif", "avifs"],
        ),
    ];
    for &(name, format, mime, canonical, extensions) in cases {
        assert_eq!(ImageFormat::from_name(name), Ok(format));
        assert_eq!(format.mime_type(), mime);
        assert_eq!(format.canonical_extension(), canonical);
        assert_eq!(format.extensions(), extensions);
        assert_eq!(ImageFormat::from_name(canonical), Ok(format));
        assert_eq!(ImageFormat::from_name(format.as_str()), Ok(format));
        for extension in extensions {
            assert_eq!(ImageFormat::from_name(extension), Ok(format));
            assert_eq!(
                ImageFormat::from_path(format!("fixture.{extension}")),
                Ok(format)
            );
            assert_eq!(
                ImageFormat::from_path(format!("some/dir/FIXTURE.{extension}")),
                Ok(format)
            );
        }
    }
    assert_eq!(
        ImageFormat::from_name("dib"),
        Err(ImageError::UnknownFormat)
    );
    assert_eq!(
        ImageFormat::from_path("fixture.dib"),
        Err(ImageError::Unsupported {
            format: None,
            message: "unknown extension: dib".to_owned(),
            stage: None,
            offset: None,
            identity: None,
        })
    );
}

#[test]
fn manifest_inputs_obey_the_exact_feature_and_target_contract()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let option_acceptance_text =
        fs::read_to_string(root.join("tests/fixtures/encode_option_acceptance_manifest.json"))?;
    let option_acceptance: EncodeOptionAcceptanceManifest =
        json::from_str(&option_acceptance_text)?;
    assert_eq!(option_acceptance.format_version, 1);
    assert_eq!(option_acceptance.assertion_origin, "compatibility_contract");
    let mut option_acceptance_ids = HashSet::new();
    for row in option_acceptance.cases {
        assert!(
            option_acceptance_ids.insert(row.id.clone()),
            "duplicate {}",
            row.id
        );
        let (format, _, _) = format(&row.format);
        let pairs = row
            .pairs
            .into_iter()
            .map(|[key, value]| (key, value))
            .collect::<Vec<_>>();
        let options = EncodeOptions::try_from_legacy_pairs(format, &pairs)
            .unwrap_or_else(|error| panic!("{} returned {error}", row.id));
        assert_eq!(options.format(), format, "{}", row.id);
    }

    let option_error_text =
        fs::read_to_string(root.join("tests/fixtures/encode_option_error_manifest.json"))?;
    let option_errors: EncodeOptionErrorManifest = json::from_str(&option_error_text)?;
    assert_eq!(option_errors.format_version, 1);
    assert_eq!(option_errors.assertion_origin, "defensive_model");
    let mut option_error_ids = HashSet::new();
    for row in option_errors.cases {
        assert!(
            option_error_ids.insert(row.id.clone()),
            "duplicate {}",
            row.id
        );
        let (format, _, _) = format(&row.format);
        let pairs = row
            .pairs
            .into_iter()
            .map(|[key, value]| (key, value))
            .collect::<Vec<_>>();
        let Err(error) = EncodeOptions::try_from_legacy_pairs(format, &pairs) else {
            panic!("{} did not produce its declared error", row.id);
        };
        assert_eq!(
            error.kind(),
            image_slash_star::ImageErrorKind::Parameter,
            "{}",
            row.id
        );
        assert_eq!(error.format(), Some(format), "{}", row.id);
        assert!(
            error
                .message()
                .is_some_and(|message| message.contains(&row.message_contains)),
            "{} returned the wrong diagnostic: {error}",
            row.id
        );
    }

    let manifest_text = fs::read_to_string(root.join("tests/fixtures/coverage_matrix.json"))?;
    let manifest_value: Value = json::from_str(&manifest_text)?;
    let Some(root_object) = manifest_value.as_object() else {
        panic!("coverage matrix root must be an object");
    };
    let Some(format_values) = root_object.get("formats").and_then(Value::as_object) else {
        panic!("coverage matrix formats must be an object");
    };
    let Some(decode_values) = format_values
        .values()
        .find_map(|format_value| format_value.as_object()?.get("decode")?.as_array())
    else {
        panic!("coverage matrix must contain a decode array");
    };
    assert!(decode_values.iter().any(|row| {
        row.as_object()
            .and_then(|row| row.get("status"))
            .and_then(Value::as_str)
            .is_some()
    }));
    assert!(format_values.values().any(|format_value| {
        format_value
            .as_object()
            .and_then(|format_value| format_value.get("decode"))
            .and_then(Value::as_array)
            .is_some_and(|rows| {
                rows.iter().any(|row| {
                    row.as_object()
                        .and_then(|row| row.get("expect_error"))
                        .and_then(Value::as_bool)
                        .is_some()
                })
            })
    }));
    assert!(
        root_object
            .get("summary")
            .and_then(Value::as_object)
            .and_then(|summary| summary.get("total_rows"))
            .and_then(Value::as_u64)
            .is_some()
    );
    let manifest = CoverageMatrix::from_json(manifest_value)?;
    let encode_input = DecodedImage::new(16, 16, vec![0; 16 * 16 * 3], ColorType::Rgb8);
    let encode_sequence = DecodedSequence::from_image(encode_input.clone());
    let all_capabilities = image_slash_star::all_capabilities();
    let capability_formats = all_capabilities
        .into_iter()
        .map(|capabilities| {
            assert_eq!(
                capabilities,
                capabilities.format().capabilities(),
                "capability table and direct query differ"
            );
            capabilities.format()
        })
        .collect::<HashSet<_>>();
    assert_eq!(capability_formats.len(), 8);

    for (name, rows) in manifest.formats {
        let (format, feature, enabled) = format(&name);
        let options = EncodeOptions::for_format(format);
        assert_eq!(options.format(), format);
        assert_capability_contract(format, feature, enabled);
        let Some(row) = rows
            .decode
            .iter()
            .find(|row| row.status != "planned" && !row.expect_error && row.asset.is_some())
        else {
            panic!("format {name} has no successful fixture row");
        };
        let Some(asset) = row.asset.as_deref() else {
            panic!("selected {name} row has no fixture asset");
        };
        assert!(
            row.oracle_detects_format,
            "selected successful {name} fixture must satisfy Pillow detection"
        );
        let bytes = fs::read(
            root.join("tests/fixtures/input/images")
                .join(&name)
                .join(asset),
        )?;

        assert_eq!(image_slash_star::detect_format(&bytes), Ok(format));

        if !enabled {
            let expected = ImageError::FeatureDisabled { format, feature };
            assert_eq!(image_slash_star::inspect(&bytes), Err(expected.clone()));
            assert_eq!(image_slash_star::decode(&bytes), Err(expected.clone()));
            assert_eq!(
                image_slash_star::decode_sequence(&bytes),
                Err(expected.clone())
            );
            assert!(matches!(EncodedImage::new(bytes), Err(error) if error == expected));
            assert_eq!(
                image_slash_star::encode(&encode_input, format, &options),
                Err(expected.clone())
            );
            assert_eq!(
                image_slash_star::encode_sequence(&encode_sequence, format, &options),
                Err(expected)
            );
            continue;
        }

        if cfg!(target_arch = "wasm32") && format == ImageFormat::Avif {
            let expected_decode = ImageError::Unsupported {
                format: Some(format),
                message: "AVIF input is outside the portable WASM decode subset".to_owned(),
                stage: Some(ImageErrorStage::StillDecode),
                offset: None,
                identity: None,
            };
            let expected_sequence_decode = ImageError::Unsupported {
                format: Some(format),
                message: "decode sequence: AVIF sequence decoding requires the native AVIF stack"
                    .to_owned(),
                stage: Some(ImageErrorStage::SequenceDecode),
                offset: None,
                identity: None,
            };
            let expected_encode = ImageError::Unsupported {
                format: Some(format),
                message: "encode: AVIF encoding requires the native extra module".to_owned(),
                stage: Some(ImageErrorStage::StillEncode),
                offset: None,
                identity: None,
            };
            let expected_sequence_encode = ImageError::Unsupported {
                format: Some(format),
                message: "encode sequence: AVIF encoding requires the native extra module"
                    .to_owned(),
                stage: Some(ImageErrorStage::SequenceEncode),
                offset: None,
                identity: None,
            };
            let info = image_slash_star::inspect(&bytes)?;
            let Some(expected_size) = row.ref_size else {
                panic!("successful {name} row has no expected size");
            };
            let Some(expected_mode) = row.ref_mode.as_deref() else {
                panic!("successful {name} row has no expected mode");
            };
            assert_eq!(info.format, format);
            assert_eq!([info.width, info.height], expected_size);
            assert_eq!(info.mode, mode(expected_mode));

            let source = EncodedImage::new(bytes.clone())?;
            assert_eq!(source.format(), format);
            assert_eq!(source.info(), &info);
            assert_eq!(source.verify(), Ok(()));
            assert!(!source.is_decoded());
            assert_eq!(
                image_slash_star::decode(&bytes),
                Err(expected_decode.clone())
            );
            assert_eq!(
                image_slash_star::decode_sequence(&bytes),
                Err(expected_sequence_decode)
            );
            assert_eq!(source.decode(), Err(expected_decode.clone()));
            assert!(!source.is_decoded());
            assert_eq!(
                image_slash_star::encode(&encode_input, format, &options),
                Err(expected_encode)
            );
            assert_eq!(
                image_slash_star::encode_sequence(&encode_sequence, format, &options),
                Err(expected_sequence_encode)
            );
            continue;
        }

        let wrong_format = if format == ImageFormat::Jpeg {
            ImageFormat::Png
        } else {
            ImageFormat::Jpeg
        };
        let wrong_options = EncodeOptions::for_format(wrong_format);
        assert!(matches!(
            image_slash_star::encode(&encode_input, format, &wrong_options),
            Err(ImageError::Parameter {
                format: Some(actual),
                ..
            }) if actual == format
        ));
        assert!(matches!(
            image_slash_star::encode_sequence(&encode_sequence, format, &wrong_options),
            Err(ImageError::Parameter {
                format: Some(actual),
                ..
            }) if actual == format
        ));

        let info = image_slash_star::inspect(&bytes)?;
        let Some(expected_size) = row.ref_size else {
            panic!("successful {name} row has no expected size");
        };
        let Some(expected_mode) = row.ref_mode.as_deref() else {
            panic!("successful {name} row has no expected mode");
        };
        assert_eq!(info.format, format);
        assert_eq!([info.width, info.height], expected_size);
        assert_eq!(info.mode, mode(expected_mode));

        let source = EncodedImage::new(bytes)?;
        assert_eq!(source.format(), format);
        assert_eq!(source.info(), &info);
        assert_eq!(
            source.verify().is_ok(),
            row.verify_status.as_deref() == Some("ok")
        );
        let decoded = source.decode()?;
        assert_eq!(decoded.format, format);
        assert_eq!(decoded.content.mode, info.mode);
        assert_eq!(
            [decoded.content.width, decoded.content.height],
            expected_size
        );
        assert_eq!(
            image_slash_star::decode_sequence(source.bytes())?.format,
            format
        );

        let encoded = image_slash_star::encode(&encode_input, format, &options)?;
        assert_eq!(image_slash_star::detect_format(&encoded), Ok(format));
        let encoded_sequence =
            image_slash_star::encode_sequence(&encode_sequence, format, &options)?;
        assert_eq!(
            image_slash_star::detect_format(&encoded_sequence),
            Ok(format)
        );
    }
    Ok(())
}

#[test]
fn sequence_kind_matches_the_container_contract() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut cases: Vec<(&str, bool, &str, SequenceKind)> = vec![
        (
            "gif animation",
            cfg!(feature = "gif"),
            "tests/fixtures/input/images/gif/animated_3frame.gif",
            SequenceKind::TimedAnimation,
        ),
        (
            "apng animation",
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/apng_l_over.png",
            SequenceKind::TimedAnimation,
        ),
        (
            "webp animation",
            cfg!(feature = "webp"),
            "tests/fixtures/input/images/webp/animated_sequence_rgba_keyframes.webp",
            SequenceKind::TimedAnimation,
        ),
        (
            "tiff pages",
            cfg!(feature = "tiff"),
            "tests/fixtures/input/images/tiff/multipage.tiff",
            SequenceKind::UntimedPages,
        ),
        (
            "jpeg still fallback",
            cfg!(feature = "jpeg"),
            "tests/fixtures/input/images/jpeg/1x1.jpg",
            SequenceKind::SingleFrame,
        ),
        (
            "png still fallback",
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/1x1.png",
            SequenceKind::SingleFrame,
        ),
        (
            "webp still fallback",
            cfg!(feature = "webp"),
            "tests/fixtures/input/images/webp/16x16.webp",
            SequenceKind::SingleFrame,
        ),
        (
            "bmp still fallback",
            cfg!(feature = "bmp"),
            "tests/fixtures/input/images/bmp/1x1.bmp",
            SequenceKind::SingleFrame,
        ),
        (
            "ico still fallback",
            cfg!(feature = "ico"),
            "tests/fixtures/input/images/ico/16x16.ico",
            SequenceKind::SingleFrame,
        ),
    ];
    if !cfg!(target_arch = "wasm32") && cfg!(feature = "avif") {
        cases.push((
            "avif animation",
            true,
            "tests/fixtures/input/images/avif/animated.avif",
            SequenceKind::TimedAnimation,
        ));
    }

    for &(name, enabled, path, expected) in &cases {
        if !enabled {
            continue;
        }
        let bytes = fs::read(root.join(path))?;
        let sequence = image_slash_star::decode_sequence(&bytes)?;
        assert_eq!(sequence.content.kind, expected, "{name}");
        if expected == SequenceKind::UntimedPages {
            for frame in &sequence.content.frames {
                assert_eq!(
                    frame.source.duration,
                    image_slash_star::FrameDuration::ZERO,
                    "{name} pages must never carry timed durations"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn source_alpha_matches_the_container_contract() -> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::SourceAlpha;

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut cases: Vec<(&str, bool, &str, Option<SourceAlpha>)> = vec![
        (
            "gif binary mask",
            cfg!(feature = "gif"),
            "tests/fixtures/input/images/gif/transparent.gif",
            Some(SourceAlpha::BinaryMask),
        ),
        (
            "gif opaque",
            cfg!(feature = "gif"),
            "tests/fixtures/input/images/gif/1x1.gif",
            None,
        ),
        (
            "png straight rgba",
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/alpha_checker.png",
            Some(SourceAlpha::Straight),
        ),
        (
            "png straight gray-alpha",
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/gray_alpha.png",
            Some(SourceAlpha::Straight),
        ),
        (
            "png straight palette tRNS",
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/apng_palette_over.png",
            Some(SourceAlpha::Straight),
        ),
        (
            "png opaque",
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/1x1.png",
            None,
        ),
        (
            "webp straight alpha",
            cfg!(feature = "webp"),
            "tests/fixtures/input/images/webp/alpha_uncompressed.webp",
            Some(SourceAlpha::Straight),
        ),
        (
            "webp opaque",
            cfg!(feature = "webp"),
            "tests/fixtures/input/images/webp/16x16.webp",
            None,
        ),
        (
            "tiff straight alpha",
            cfg!(feature = "tiff"),
            "tests/fixtures/input/images/tiff/gray_alpha.tiff",
            Some(SourceAlpha::Straight),
        ),
        (
            "tiff opaque",
            cfg!(feature = "tiff"),
            "tests/fixtures/input/images/tiff/gray.tiff",
            None,
        ),
        (
            "jpeg no alpha",
            cfg!(feature = "jpeg"),
            "tests/fixtures/input/images/jpeg/1x1.jpg",
            None,
        ),
        (
            "bmp no alpha",
            cfg!(feature = "bmp"),
            "tests/fixtures/input/images/bmp/1x1.bmp",
            None,
        ),
        (
            "ico no alpha",
            cfg!(feature = "ico"),
            "tests/fixtures/input/images/ico/16x16.ico",
            None,
        ),
    ];
    if !cfg!(target_arch = "wasm32") && cfg!(feature = "avif") {
        cases.push((
            "avif straight alpha",
            true,
            "tests/fixtures/input/images/avif/alpha.avif",
            Some(SourceAlpha::Straight),
        ));
        cases.push((
            "avif opaque",
            true,
            "tests/fixtures/input/images/avif/baseline.avif",
            None,
        ));
    }

    for &(name, enabled, path, expected) in &cases {
        if !enabled {
            continue;
        }
        let bytes = fs::read(root.join(path))?;
        let info = image_slash_star::inspect(&bytes)?;
        assert_eq!(info.source.alpha(), expected, "{name} inspect");
        let decoded = image_slash_star::decode(&bytes)?;
        assert_eq!(decoded.content.source.alpha(), expected, "{name} decode");
    }
    Ok(())
}

fn png_crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "test payloads are tiny fixed literals that always fit u32"
)]
fn png_chunk(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut chunk = Vec::with_capacity(12usize.wrapping_add(payload.len()));
    chunk.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    chunk.extend_from_slice(kind);
    chunk.extend_from_slice(payload);
    let crc = png_crc32(&chunk[4..]);
    chunk.extend_from_slice(&crc.to_be_bytes());
    chunk
}

#[allow(
    clippy::arithmetic_side_effects,
    reason = "offsets are bounds-checked against the in-memory fixture slice"
)]
fn png_chunk_offset(data: &[u8], kind: &[u8; 4]) -> Result<usize, Box<dyn std::error::Error>> {
    let mut position = 8usize;
    while position.wrapping_add(8) <= data.len() {
        let length = u32::from_be_bytes([
            data[position],
            data[position + 1],
            data[position + 2],
            data[position + 3],
        ]) as usize;
        if &data[position.wrapping_add(4)..position.wrapping_add(8)] == kind {
            return Ok(position);
        }
        position = position.wrapping_add(12).wrapping_add(length);
    }
    Err(format!("PNG chunk {kind:?} not found").into())
}

fn contains_chunk_type(data: &[u8], kind: &[u8; 4]) -> bool {
    data.windows(4).any(|window| window == kind)
}

#[test]
fn opaque_blocks_match_the_container_contract() -> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::OpaqueBlock;

    if !cfg!(feature = "png") {
        return Ok(());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let base = fs::read(root.join("tests/fixtures/input/images/png/1x1.png"))?;

    // Insert unknown ancillary chunks around the image data: a safe-to-copy
    // private chunk before IDAT, its duplicate, an unsafe-to-copy variant, an
    // unknown chunk after IDAT, and a critical chunk that must never be
    // retained as opaque.
    let safe = png_chunk(b"prVt", b"safe-payload");
    let duplicate = png_chunk(b"prVt", b"duplicate-payload");
    let unsafe_chunk = png_chunk(b"prVT", b"unsafe-payload");
    let after_idat = png_chunk(b"teSt", b"after-idat-payload");
    let critical = png_chunk(b"ABCD", b"critical-payload");
    let idat_offset = png_chunk_offset(&base, b"IDAT")?;
    let iend_offset = png_chunk_offset(&base, b"IEND")?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&base[..idat_offset]);
    bytes.extend_from_slice(&safe);
    bytes.extend_from_slice(&duplicate);
    bytes.extend_from_slice(&unsafe_chunk);
    bytes.extend_from_slice(&critical);
    bytes.extend_from_slice(&base[idat_offset..iend_offset]);
    bytes.extend_from_slice(&after_idat);
    bytes.extend_from_slice(&base[iend_offset..]);

    let expected = vec![
        OpaqueBlock {
            kind: b"prVt".to_vec(),
            data: b"safe-payload".to_vec(),
            safe_to_copy: true,
        },
        OpaqueBlock {
            kind: b"prVt".to_vec(),
            data: b"duplicate-payload".to_vec(),
            safe_to_copy: true,
        },
        OpaqueBlock {
            kind: b"prVT".to_vec(),
            data: b"unsafe-payload".to_vec(),
            safe_to_copy: false,
        },
        OpaqueBlock {
            kind: b"teSt".to_vec(),
            data: b"after-idat-payload".to_vec(),
            safe_to_copy: true,
        },
    ];

    let decoded = image_slash_star::decode(&bytes)?;
    assert_eq!(decoded.content.opaque_blocks, expected, "still decode");
    let sequence = image_slash_star::decode_sequence(&bytes)?;
    assert_eq!(
        sequence.content.opaque_blocks, expected,
        "still fallback sequence decode"
    );

    // Default encoding must not replay retained opaque blocks.
    let options = image_slash_star::EncodeOptions::for_format(ImageFormat::Png);
    let encoded = image_slash_star::encode(&decoded.content, ImageFormat::Png, &options)?;
    for kind in [b"prVt", b"prVT", b"teSt", b"ABCD"] {
        assert!(
            !contains_chunk_type(&encoded, kind),
            "encoded PNG must not replay retained chunk {kind:?}"
        );
    }

    // The unmodified fixture retains no opaque blocks.
    let plain = image_slash_star::decode(&base)?;
    assert!(plain.content.opaque_blocks.is_empty());
    let plain_sequence = image_slash_star::decode_sequence(&base)?;
    assert!(plain_sequence.content.opaque_blocks.is_empty());

    // Retained opaque blocks count toward the metadata policy extent, so a
    // caller-set limit rejects before retention can bypass resource bounds.
    let strict_policy = image_slash_star::DecodePolicy::new().with_max_metadata_bytes(1);
    assert!(matches!(
        image_slash_star::decode_with_policy(&bytes, &strict_policy),
        Err(image_slash_star::ImageError::LimitExceeded { .. })
    ));

    // APNG sequence decode retains the same ordered container-level blocks.
    let apng_base = fs::read(root.join("tests/fixtures/input/images/png/apng_l_over.png"))?;
    let apng_idat = png_chunk_offset(&apng_base, b"IDAT")?;
    let apng_iend = png_chunk_offset(&apng_base, b"IEND")?;
    let mut apng = Vec::new();
    apng.extend_from_slice(&apng_base[..apng_idat]);
    apng.extend_from_slice(&safe);
    apng.extend_from_slice(&apng_base[apng_idat..apng_iend]);
    apng.extend_from_slice(&after_idat);
    apng.extend_from_slice(&apng_base[apng_iend..]);
    let apng_sequence = image_slash_star::decode_sequence(&apng)?;
    assert_eq!(
        apng_sequence.content.opaque_blocks,
        vec![expected[0].clone(), expected[3].clone()],
        "APNG sequence decode"
    );
    Ok(())
}

#[test]
fn metadata_matches_the_container_contract() -> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::{OpaqueBlock, OpaqueMetadata, RawIccProfile};

    if !cfg!(feature = "png") {
        return Ok(());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let base = fs::read(root.join("tests/fixtures/input/images/png/1x1.png"))?;

    // Known metadata chunks are retained as raw, unparsed metadata records
    // (compressed payloads are never inflated), while unknown ancillary
    // chunks stay in the opaque-block list.
    let text = png_chunk(b"tEXt", b"Comment\0hello world");
    let ztext = png_chunk(b"zTXt", b"Author\0\0raw-compressed-bytes");
    let iccp = png_chunk(b"iCCP", b"profile\0\0raw-profile-bytes");
    let exif = png_chunk(b"eXIf", b"raw-exif-bytes");
    let unknown = png_chunk(b"prVt", b"unknown-payload");
    let idat_offset = png_chunk_offset(&base, b"IDAT")?;
    let iend_offset = png_chunk_offset(&base, b"IEND")?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&base[..idat_offset]);
    bytes.extend_from_slice(&text);
    bytes.extend_from_slice(&unknown);
    bytes.extend_from_slice(&ztext);
    bytes.extend_from_slice(&base[idat_offset..iend_offset]);
    bytes.extend_from_slice(&iccp);
    bytes.extend_from_slice(&exif);
    bytes.extend_from_slice(&base[iend_offset..]);

    let expected_metadata = vec![
        OpaqueMetadata {
            kind: b"tEXt".to_vec(),
            data: b"Comment\0hello world".to_vec(),
        },
        OpaqueMetadata {
            kind: b"zTXt".to_vec(),
            data: b"Author\0\0raw-compressed-bytes".to_vec(),
        },
        OpaqueMetadata {
            kind: b"eXIf".to_vec(),
            data: b"raw-exif-bytes".to_vec(),
        },
    ];
    let expected_blocks = vec![OpaqueBlock {
        kind: b"prVt".to_vec(),
        data: b"unknown-payload".to_vec(),
        safe_to_copy: true,
    }];

    let decoded = image_slash_star::decode(&bytes)?;
    assert_eq!(
        decoded.content.metadata, expected_metadata,
        "still metadata"
    );
    assert_eq!(
        decoded.content.opaque_blocks, expected_blocks,
        "still blocks"
    );
    assert_eq!(
        decoded.content.source_color.icc_profile(),
        Some(&RawIccProfile {
            keyword: b"profile".to_vec(),
            data: b"\0raw-profile-bytes".to_vec(),
        }),
        "iCCP classifies as source color metadata"
    );
    let sequence = image_slash_star::decode_sequence(&bytes)?;
    assert_eq!(
        sequence.content.metadata, expected_metadata,
        "fallback sequence metadata"
    );
    assert_eq!(
        sequence.content.opaque_blocks, expected_blocks,
        "fallback sequence blocks"
    );

    // Default encoding must not replay metadata or unknown blocks.
    let options = image_slash_star::EncodeOptions::for_format(ImageFormat::Png);
    let encoded = image_slash_star::encode(&decoded.content, ImageFormat::Png, &options)?;
    for kind in [b"tEXt", b"zTXt", b"iCCP", b"eXIf", b"prVt"] {
        assert!(
            !contains_chunk_type(&encoded, kind),
            "encoded PNG must not replay metadata chunk {kind:?}"
        );
    }

    // Unmodified fixtures carry no metadata records, and the metadata extent
    // policy still bounds retention.
    let plain = image_slash_star::decode(&base)?;
    assert!(plain.content.metadata.is_empty());
    assert!(plain.content.opaque_blocks.is_empty());
    let strict_policy = image_slash_star::DecodePolicy::new().with_max_metadata_bytes(1);
    assert!(matches!(
        image_slash_star::decode_with_policy(&bytes, &strict_policy),
        Err(image_slash_star::ImageError::LimitExceeded { .. })
    ));

    // APNG sequence decode classifies the same chunks into the same lists.
    let apng_base = fs::read(root.join("tests/fixtures/input/images/png/apng_l_over.png"))?;
    let apng_idat = png_chunk_offset(&apng_base, b"IDAT")?;
    let apng_iend = png_chunk_offset(&apng_base, b"IEND")?;
    let mut apng = Vec::new();
    apng.extend_from_slice(&apng_base[..apng_idat]);
    apng.extend_from_slice(&text);
    apng.extend_from_slice(&unknown);
    apng.extend_from_slice(&apng_base[apng_idat..apng_iend]);
    apng.extend_from_slice(&exif);
    apng.extend_from_slice(&apng_base[apng_iend..]);
    let apng_sequence = image_slash_star::decode_sequence(&apng)?;
    assert_eq!(
        apng_sequence.content.metadata,
        vec![expected_metadata[0].clone(), expected_metadata[2].clone()],
        "APNG metadata"
    );
    assert_eq!(
        apng_sequence.content.opaque_blocks, expected_blocks,
        "APNG blocks"
    );
    Ok(())
}

#[test]
fn source_color_matches_the_container_contract() -> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::{
        OpaqueBlock, OpaqueMetadata, RawIccProfile, SourceChromaticities, SourceColor, SrgbIntent,
    };

    if !cfg!(feature = "png") {
        return Ok(());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let base = fs::read(root.join("tests/fixtures/input/images/png/1x1.png"))?;

    let mut chroma = Vec::new();
    for value in [31270u32, 32900, 64000, 33000, 30000, 60000, 15000, 6000] {
        chroma.extend_from_slice(&value.to_be_bytes());
    }
    let srgb = png_chunk(b"sRGB", &[0]);
    let duplicate_srgb = png_chunk(b"sRGB", &[1]);
    let malformed_srgb = png_chunk(b"sRGB", &[0, 1]);
    let gamma = png_chunk(b"gAMA", &[0, 0, 0xB1, 0x8F]);
    let duplicate_gamma = png_chunk(b"gAMA", &[0, 0, 0xB1, 0x8F]);
    let malformed_gamma = png_chunk(b"gAMA", &[0, 1]);
    let chroma_chunk = png_chunk(b"cHRM", &chroma);
    let duplicate_chroma = png_chunk(b"cHRM", &chroma);
    let malformed_chroma = png_chunk(b"cHRM", &[0, 0, 0]);
    let iccp = png_chunk(b"iCCP", b"profile\0\0raw-profile-bytes");
    let iccp_no_nul = png_chunk(b"iCCP", b"nonul");
    let iccp_nul_first = png_chunk(b"iCCP", b"\0raw");
    let iccp_no_profile = png_chunk(b"iCCP", b"a\0");
    let duplicate_iccp = png_chunk(b"iCCP", b"other\0\0raw");
    let text = png_chunk(b"tEXt", b"Comment\0hello");
    let unknown = png_chunk(b"prVt", b"unknown-payload");
    let idat_offset = png_chunk_offset(&base, b"IDAT")?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&base[..idat_offset]);
    for chunk in [
        &srgb,
        &text,
        &gamma,
        &chroma_chunk,
        &iccp_no_nul,
        &iccp_nul_first,
        &iccp_no_profile,
        &iccp,
        &duplicate_srgb,
        &malformed_srgb,
        &duplicate_gamma,
        &malformed_gamma,
        &duplicate_chroma,
        &malformed_chroma,
        &duplicate_iccp,
        &unknown,
    ] {
        bytes.extend_from_slice(chunk);
    }
    bytes.extend_from_slice(&base[idat_offset..]);

    let expected_color = SourceColor::new()
        .with_srgb(SrgbIntent::Perceptual)
        .with_gamma(45_455)
        .with_chromaticities(SourceChromaticities {
            white_x: 31_270,
            white_y: 32_900,
            red_x: 64_000,
            red_y: 33_000,
            green_x: 30_000,
            green_y: 60_000,
            blue_x: 15_000,
            blue_y: 6_000,
        })
        .with_icc_profile(RawIccProfile {
            keyword: b"profile".to_vec(),
            data: b"\0raw-profile-bytes".to_vec(),
        });
    let expected_metadata = vec![
        OpaqueMetadata {
            kind: b"tEXt".to_vec(),
            data: b"Comment\0hello".to_vec(),
        },
        OpaqueMetadata {
            kind: b"iCCP".to_vec(),
            data: b"nonul".to_vec(),
        },
        OpaqueMetadata {
            kind: b"iCCP".to_vec(),
            data: b"\0raw".to_vec(),
        },
        OpaqueMetadata {
            kind: b"iCCP".to_vec(),
            data: b"a\0".to_vec(),
        },
        OpaqueMetadata {
            kind: b"sRGB".to_vec(),
            data: vec![1],
        },
        OpaqueMetadata {
            kind: b"sRGB".to_vec(),
            data: vec![0, 1],
        },
        OpaqueMetadata {
            kind: b"gAMA".to_vec(),
            data: vec![0, 0, 0xB1, 0x8F],
        },
        OpaqueMetadata {
            kind: b"gAMA".to_vec(),
            data: vec![0, 1],
        },
        OpaqueMetadata {
            kind: b"cHRM".to_vec(),
            data: chroma.clone(),
        },
        OpaqueMetadata {
            kind: b"cHRM".to_vec(),
            data: vec![0, 0, 0],
        },
        OpaqueMetadata {
            kind: b"iCCP".to_vec(),
            data: b"other\0\0raw".to_vec(),
        },
    ];
    let expected_blocks = vec![OpaqueBlock {
        kind: b"prVt".to_vec(),
        data: b"unknown-payload".to_vec(),
        safe_to_copy: true,
    }];

    let decoded = image_slash_star::decode(&bytes)?;
    assert_eq!(decoded.content.source_color, expected_color, "still color");
    assert_eq!(
        decoded.content.metadata, expected_metadata,
        "still metadata"
    );
    assert_eq!(
        decoded.content.opaque_blocks, expected_blocks,
        "still blocks"
    );
    let sequence = image_slash_star::decode_sequence(&bytes)?;
    assert_eq!(
        sequence.content.source_color, expected_color,
        "fallback sequence color"
    );
    assert_eq!(
        sequence.content.metadata, expected_metadata,
        "fallback sequence metadata"
    );

    // Default encoding never replays color metadata.
    let options = image_slash_star::EncodeOptions::for_format(ImageFormat::Png);
    let encoded = image_slash_star::encode(&decoded.content, ImageFormat::Png, &options)?;
    for kind in [b"sRGB", b"gAMA", b"cHRM", b"iCCP"] {
        assert!(
            !contains_chunk_type(&encoded, kind),
            "encoded PNG must not replay color chunk {kind:?}"
        );
    }

    // Unmodified fixtures retain no source color facts.
    let plain = image_slash_star::decode(&base)?;
    assert!(plain.content.source_color.is_empty());

    // Every sRGB intent value parses; invalid values fall back to metadata.
    for (value, expected) in [
        (1u8, Some(SrgbIntent::RelativeColorimetric)),
        (2, Some(SrgbIntent::Saturation)),
        (3, Some(SrgbIntent::AbsoluteColorimetric)),
        (9, None),
    ] {
        let mut variant = Vec::new();
        variant.extend_from_slice(&base[..idat_offset]);
        variant.extend_from_slice(&png_chunk(b"sRGB", &[value]));
        variant.extend_from_slice(&base[idat_offset..]);
        let variant_decoded = image_slash_star::decode(&variant)?;
        assert_eq!(
            variant_decoded.content.source_color.srgb(),
            expected,
            "sRGB value {value}"
        );
        if expected.is_none() {
            assert_eq!(
                variant_decoded.content.metadata,
                vec![OpaqueMetadata {
                    kind: b"sRGB".to_vec(),
                    data: vec![value],
                }],
                "invalid sRGB value {value} must fall back to metadata"
            );
        }
    }

    // APNG sequence decode retains the same container-level color metadata.
    let apng_base = fs::read(root.join("tests/fixtures/input/images/png/apng_l_over.png"))?;
    let apng_idat = png_chunk_offset(&apng_base, b"IDAT")?;
    let mut apng = Vec::new();
    apng.extend_from_slice(&apng_base[..apng_idat]);
    apng.extend_from_slice(&srgb);
    apng.extend_from_slice(&gamma);
    apng.extend_from_slice(&apng_base[apng_idat..]);
    let apng_sequence = image_slash_star::decode_sequence(&apng)?;
    assert_eq!(
        apng_sequence.content.source_color.srgb(),
        Some(SrgbIntent::Perceptual),
        "APNG srgb"
    );
    assert_eq!(
        apng_sequence.content.source_color.gamma(),
        Some(45_455),
        "APNG gamma"
    );
    Ok(())
}

#[test]
fn verification_scope_requests_fail_when_the_codec_cannot_provide_them()
-> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::VerificationScope;

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cases: &[(&str, bool, &str, VerificationScope)] = &[
        (
            "jpeg",
            cfg!(feature = "jpeg"),
            "tests/fixtures/input/images/jpeg/1x1.jpg",
            VerificationScope::Structure,
        ),
        (
            "png",
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/1x1.png",
            VerificationScope::Structure,
        ),
        (
            "gif",
            cfg!(feature = "gif"),
            "tests/fixtures/input/images/gif/1x1.gif",
            VerificationScope::HeaderOnly,
        ),
        (
            "bmp",
            cfg!(feature = "bmp"),
            "tests/fixtures/input/images/bmp/1x1.bmp",
            VerificationScope::HeaderOnly,
        ),
        (
            "tiff",
            cfg!(feature = "tiff"),
            "tests/fixtures/input/images/tiff/1bit.tiff",
            VerificationScope::HeaderOnly,
        ),
        (
            "webp",
            cfg!(feature = "webp"),
            "tests/fixtures/input/images/webp/16x16.webp",
            VerificationScope::Structure,
        ),
        (
            "ico",
            cfg!(feature = "ico"),
            "tests/fixtures/input/images/ico/16x16.ico",
            VerificationScope::HeaderOnly,
        ),
        (
            "avif",
            cfg!(feature = "avif"),
            "tests/fixtures/input/images/avif/baseline.avif",
            VerificationScope::HeaderOnly,
        ),
    ];

    for &(name, enabled, path, provided) in cases {
        if !enabled {
            continue;
        }
        let bytes = fs::read(root.join(path))?;
        let source = EncodedImage::new(bytes)?;
        assert_eq!(source.format().verification_scope(), provided, "{name}");
        assert_eq!(source.verification_scope(), provided, "{name}");
        assert!(provided.provides(provided), "{name}");
        source.verify()?;

        // Weaker or equal requests are satisfied by the same verification pass.
        source.verify_with_scope(VerificationScope::HeaderOnly)?;
        source.verify_with_scope(provided)?;

        // A stronger request fails with a format-qualified Unsupported
        // instead of silently reporting weaker evidence.
        let stronger = if provided == VerificationScope::HeaderOnly {
            VerificationScope::Structure
        } else {
            VerificationScope::FullPixels
        };
        assert!(!provided.provides(stronger), "{name}");
        let error = match source.verify_with_scope(stronger) {
            Err(error) => error,
            Ok(()) => panic!("{name} unexpectedly provided {stronger:?}"),
        };
        assert_eq!(
            error.kind(),
            image_slash_star::ImageErrorKind::Unsupported,
            "{name}"
        );
        assert_eq!(error.format(), Some(source.format()), "{name}");
        assert!(
            error.message().is_some_and(|message| !message.is_empty()),
            "{name}"
        );

        // Full pixel verification is not provided by any codec.
        assert!(!provided.provides(VerificationScope::FullPixels), "{name}");
        let error = match source.verify_with_scope(VerificationScope::FullPixels) {
            Err(error) => error,
            Ok(()) => panic!("{name} unexpectedly provided FullPixels"),
        };
        assert_eq!(
            error.kind(),
            image_slash_star::ImageErrorKind::Unsupported,
            "{name}"
        );
        assert_eq!(error.format(), Some(source.format()), "{name}");
    }

    assert!(VerificationScope::Structure.provides(VerificationScope::HeaderOnly));
    assert!(!VerificationScope::Structure.provides(VerificationScope::FullPixels));
    assert!(VerificationScope::FullPixels.provides(VerificationScope::FullPixels));
    Ok(())
}

#[test]
fn error_stages_name_the_public_operation() -> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::{
        ColorType, DecodedFrame, DecodedImage, DecodedSequence, EncodeOptions, FrameBlend,
        FrameDisposal, FrameDuration, FrameRect, ImageErrorKind, ImageErrorStage, ImageFormat,
    };

    if !cfg!(feature = "png") || !cfg!(feature = "jpeg") {
        return Ok(());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let malformed = fs::read(root.join("tests/fixtures/input/images/png/truncated.png"))?;
    let verify_only = fs::read(root.join("tests/fixtures/input/images/png/bad_idat_crc.png"))?;

    let inspect_error = match image_slash_star::inspect(&malformed) {
        Err(error) => error,
        Ok(_) => panic!("truncated PNG must fail inspection"),
    };
    assert_eq!(inspect_error.kind(), ImageErrorKind::Malformed);
    assert_eq!(inspect_error.stage(), Some(ImageErrorStage::Inspection));
    assert_eq!(inspect_error.identity(), Some("png_chunk"));
    assert!(inspect_error.offset().is_some());

    let decode_error = match image_slash_star::decode(&malformed) {
        Err(error) => error,
        Ok(_) => panic!("truncated PNG must fail still decode"),
    };
    assert_eq!(decode_error.kind(), ImageErrorKind::Malformed);
    assert_eq!(decode_error.stage(), Some(ImageErrorStage::StillDecode));
    assert_eq!(decode_error.identity(), Some("png_chunk"));
    assert!(decode_error.offset().is_some());

    let sequence_error = match image_slash_star::decode_sequence(&malformed) {
        Err(error) => error,
        Ok(_) => panic!("truncated PNG must fail sequence decode"),
    };
    assert_eq!(sequence_error.kind(), ImageErrorKind::Malformed);
    assert_eq!(
        sequence_error.stage(),
        Some(ImageErrorStage::SequenceDecode)
    );
    assert_eq!(sequence_error.identity(), Some("png_chunk"));
    assert!(sequence_error.offset().is_some());

    let source_error = match image_slash_star::EncodedImage::new(malformed) {
        Err(error) => error,
        Ok(_) => panic!("truncated PNG must fail source construction"),
    };
    assert_eq!(source_error.kind(), ImageErrorKind::Malformed);
    assert_eq!(source_error.stage(), Some(ImageErrorStage::Inspection));
    assert_eq!(source_error.identity(), Some("png_chunk"));
    assert!(source_error.offset().is_some());

    let source = image_slash_star::EncodedImage::new(verify_only)?;
    let verify_error = match source.verify() {
        Err(error) => error,
        Ok(_) => panic!("bad IDAT CRC must fail verification"),
    };
    assert_eq!(verify_error.kind(), ImageErrorKind::Malformed);
    assert_eq!(verify_error.stage(), Some(ImageErrorStage::Verification));
    assert_eq!(verify_error.identity(), Some("png_chunk"));
    assert!(verify_error.offset().is_some());

    let cmyk = DecodedImage::new(1, 1, vec![0; 4], ColorType::Cmyk8);
    let encode_error = match image_slash_star::encode_default(&cmyk, ImageFormat::Png) {
        Err(error) => error,
        Ok(_) => panic!("PNG must reject CMYK input"),
    };
    assert_eq!(encode_error.kind(), ImageErrorKind::Unsupported);
    assert_eq!(encode_error.stage(), Some(ImageErrorStage::StillEncode));
    assert_eq!(encode_error.identity(), None);
    assert_eq!(encode_error.offset(), None);

    let frame = DecodedFrame::rendered_canvas(
        DecodedImage::new(1, 1, vec![0], ColorType::L8),
        FrameRect {
            left: 0,
            top: 0,
            width: 1,
            height: 1,
        },
        FrameDuration::ZERO,
        FrameDisposal::Unspecified,
        FrameBlend::Unspecified,
    );
    let sequence = DecodedSequence {
        width: 1,
        height: 1,
        frames: vec![frame.clone(), frame],
        loop_count: None,
        background: None,
        kind: image_slash_star::SequenceKind::TimedAnimation,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    };
    let sequence_error = match image_slash_star::encode_sequence(
        &sequence,
        ImageFormat::Jpeg,
        &EncodeOptions::for_format(ImageFormat::Jpeg),
    ) {
        Err(error) => error,
        Ok(_) => panic!("JPEG must reject multi-frame sequences"),
    };
    assert_eq!(sequence_error.kind(), ImageErrorKind::Unsupported);
    assert_eq!(
        sequence_error.stage(),
        Some(ImageErrorStage::SequenceEncode)
    );
    assert_eq!(sequence_error.identity(), None);
    assert_eq!(sequence_error.offset(), None);

    for (name, enabled, path, identity_prefix) in [
        (
            "gif",
            cfg!(feature = "gif"),
            "tests/fixtures/input/images/gif/truncated_image_descriptor.gif",
            "gif_",
        ),
        (
            "jpeg",
            cfg!(feature = "jpeg"),
            "tests/fixtures/input/images/jpeg/truncated.jpg",
            "jpeg_",
        ),
        (
            "tiff",
            cfg!(feature = "tiff"),
            "tests/fixtures/input/images/tiff/truncated_ifd_entry.tiff",
            "tiff_",
        ),
    ] {
        if !enabled {
            continue;
        }
        let bytes = fs::read(root.join(path))?;
        let error = match image_slash_star::decode(&bytes) {
            Err(error) => error,
            Ok(_) => panic!("{name} truncated fixture must fail decode"),
        };
        assert_eq!(error.kind(), ImageErrorKind::Malformed);
        assert_eq!(error.stage(), Some(ImageErrorStage::StillDecode));
        assert!(
            error
                .identity()
                .is_some_and(|identity| identity.starts_with(identity_prefix)),
            "{name} identity"
        );
        assert!(error.offset().is_some(), "{name} offset");
    }

    if cfg!(feature = "avif") {
        let baseline = fs::read(root.join("tests/fixtures/input/images/avif/baseline.avif"))?;
        let truncated_avif = &baseline[..100];
        let error = match image_slash_star::decode(truncated_avif) {
            Err(error) => error,
            Ok(_) => panic!("truncated AVIF must fail decode"),
        };
        assert_eq!(error.kind(), ImageErrorKind::Malformed);
        assert_eq!(error.stage(), Some(ImageErrorStage::StillDecode));
        assert_eq!(error.identity(), Some("avif_box"));
        assert!(error.offset().is_some());
    }
    Ok(())
}
