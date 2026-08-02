//! Cargo-feature and target-capability behavior driven by Pillow fixtures.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use bytemuck as _;
use image_slash_star::{
    Capability, CapabilityRestriction, CapabilityTarget, CapabilityUnavailableReason, ColorType,
    DecodedImage, DecodedSequence, DiagnosticKind, EncodeOptions, EncodedImage, ImageDiagnostic,
    ImageError, ImageErrorStage, ImageFormat, ImageMode, SequenceKind, SourceColor,
    UnsupportedReason,
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

struct DiagnosticManifest {
    format_version: u32,
    assertion_origin: String,
    pillow_version: String,
    cases: Vec<DiagnosticCase>,
}

struct DiagnosticCase {
    id: String,
    feature: String,
    format: String,
    asset_path: String,
    mutation: String,
    chunk_kind: String,
    chunk_payload: Vec<u64>,
    operation: String,
    diagnostic_kind: String,
    stage: String,
    offset: u64,
    identity: String,
    pillow_outcome: String,
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

impl FromJson for DiagnosticManifest {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            format_version: object.take("format_version")?,
            assertion_origin: object.take("assertion_origin")?,
            pillow_version: object.take("pillow_version")?,
            cases: object.take("cases")?,
        })
    }
}

impl FromJson for DiagnosticCase {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            id: object.take("id")?,
            feature: object.take("feature")?,
            format: object.take("format")?,
            asset_path: object.take("asset_path")?,
            mutation: object.take("mutation")?,
            chunk_kind: object.take("chunk_kind")?,
            chunk_payload: object.take("chunk_payload")?,
            operation: object.take("operation")?,
            diagnostic_kind: object.take("diagnostic_kind")?,
            stage: object.take("stage")?,
            offset: object.take("offset")?,
            identity: object.take("identity")?,
            pillow_outcome: object.take("pillow_outcome")?,
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

struct IncrementalInputManifest {
    format_version: u32,
    assertion_origin: String,
    detection_cases: Vec<DetectionCase>,
    inspection_fixtures: Vec<InspectionFixture>,
}

struct DetectionCase {
    id: String,
    input_hex: String,
    expect: String,
    minimum: Option<u64>,
    format: Option<String>,
    legacy_parity: Option<bool>,
}

struct InspectionFixture {
    id: String,
    format: String,
    asset_path: String,
    signature_prefix: u64,
    signature_minimum: u64,
    need_more_prefix: u64,
    need_more_minimum: u64,
    basic_prefix: Option<u64>,
    basic_frame_count_complete: Option<bool>,
    decode_need_more_prefix: Option<u64>,
    decode_need_more_minimum: Option<u64>,
}

impl FromJson for IncrementalInputManifest {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            format_version: object.take("format_version")?,
            assertion_origin: object.take("assertion_origin")?,
            detection_cases: object.take("detection_cases")?,
            inspection_fixtures: object.take("inspection_fixtures")?,
        })
    }
}

impl FromJson for DetectionCase {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            id: object.take("id")?,
            input_hex: object.take("input_hex")?,
            expect: object.take("expect")?,
            minimum: object.take("minimum")?,
            format: object.take("format")?,
            legacy_parity: object.take("legacy_parity")?,
        })
    }
}

impl FromJson for InspectionFixture {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            id: object.take("id")?,
            format: object.take("format")?,
            asset_path: object.take("asset_path")?,
            signature_prefix: object.take("signature_prefix")?,
            signature_minimum: object.take("signature_minimum")?,
            need_more_prefix: object.take("need_more_prefix")?,
            need_more_minimum: object.take("need_more_minimum")?,
            basic_prefix: object.take("basic_prefix")?,
            basic_frame_count_complete: object.take("basic_frame_count_complete")?,
            decode_need_more_prefix: object.take("decode_need_more_prefix")?,
            decode_need_more_minimum: object.take("decode_need_more_minimum")?,
        })
    }
}

fn hex_bytes(hex: &str) -> Vec<u8> {
    assert!(
        hex.len().is_multiple_of(2),
        "manifest hex must have an even length"
    );
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair)
                .unwrap_or_else(|error| panic!("invalid hex encoding: {error}"));
            u8::from_str_radix(pair, 16)
                .unwrap_or_else(|error| panic!("invalid hex digit {pair:?}: {error}"))
        })
        .collect()
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
            reason: None,
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
                reason: None,
                offset: None,
                identity: None,
            };
            let expected_sequence_decode = ImageError::Unsupported {
                format: Some(format),
                message: "decode sequence: AVIF sequence decoding requires the native AVIF stack"
                    .to_owned(),
                stage: Some(ImageErrorStage::SequenceDecode),
                reason: Some(UnsupportedReason::TargetUnavailable),
                offset: None,
                identity: None,
            };
            let expected_encode = ImageError::Unsupported {
                format: Some(format),
                message: "encode: AVIF encoding requires the native extra module".to_owned(),
                stage: Some(ImageErrorStage::StillEncode),
                reason: Some(UnsupportedReason::TargetUnavailable),
                offset: None,
                identity: None,
            };
            let expected_sequence_encode = ImageError::Unsupported {
                format: Some(format),
                message: "encode sequence: AVIF encoding requires the native extra module"
                    .to_owned(),
                stage: Some(ImageErrorStage::SequenceEncode),
                reason: Some(UnsupportedReason::TargetUnavailable),
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
fn cancellation_token_stops_decode_without_partial_state() -> Result<(), Box<dyn std::error::Error>>
{
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cases: &[(ImageFormat, bool, &str)] = &[
        (
            ImageFormat::Png,
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/1x1.png",
        ),
        (
            ImageFormat::Gif,
            cfg!(feature = "gif"),
            "tests/fixtures/input/images/gif/1x1.gif",
        ),
        (
            ImageFormat::Bmp,
            cfg!(feature = "bmp"),
            "tests/fixtures/input/images/bmp/1x1.bmp",
        ),
        (
            ImageFormat::Tiff,
            cfg!(feature = "tiff"),
            "tests/fixtures/input/images/tiff/8bit.tiff",
        ),
        (
            ImageFormat::Jpeg,
            cfg!(feature = "jpeg"),
            "tests/fixtures/input/images/jpeg/1x1.jpg",
        ),
        (
            ImageFormat::WebP,
            cfg!(feature = "webp"),
            "tests/fixtures/input/images/webp/16x16.webp",
        ),
        (
            ImageFormat::Ico,
            cfg!(feature = "ico"),
            "tests/fixtures/input/images/ico/16x16.ico",
        ),
        (
            ImageFormat::Avif,
            cfg!(feature = "avif"),
            "tests/fixtures/input/images/avif/baseline.avif",
        ),
    ];
    for (format, enabled, path) in cases {
        if !enabled {
            continue;
        }
        let bytes = fs::read(root.join(path))?;
        // Targets without a full decode path for this format (for example
        // portable WASM AVIF) keep their Unsupported classification.
        if image_slash_star::decode(&bytes).is_err() {
            continue;
        }
        // A never-cancelled token leaves the result byte-identical to legacy.
        let token = image_slash_star::CancellationToken::new();
        let decoded = image_slash_star::decode_with_token(&bytes, &token)?;
        let legacy = image_slash_star::decode(&bytes)?;
        assert_eq!(decoded.format, legacy.format, "{path} format");
        assert_eq!(
            decoded.content.pixels, legacy.content.pixels,
            "{path} pixels"
        );

        // A pre-cancelled token stops at the first checkpoint without
        // publishing partial state.
        let cancelled = image_slash_star::CancellationToken::new();
        cancelled.cancel();
        let error = match image_slash_star::decode_with_token(&bytes, &cancelled) {
            Ok(info) => panic!("a cancelled token must stop {path}: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Cancelled);
        assert_eq!(error.format(), Some(*format));
        assert_eq!(error.stage(), Some(ImageErrorStage::StillDecode));
        assert_eq!(error.minimum_input(), None);

        // Clones observe the same cancellation state.
        let shared = image_slash_star::CancellationToken::new();
        let clone = shared.clone();
        shared.cancel();
        assert!(clone.is_cancelled());

        // Truncated input still reports the non-terminal status.
        let error = match image_slash_star::decode_with_token(&bytes[..5], &token) {
            Ok(info) => panic!("a partial signature must need more data: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::NeedMoreData);

        // Policy limits still apply before codec work.
        let limited = image_slash_star::DecodePolicy::default().with_max_encoded_bytes(10);
        let error = match image_slash_star::decode_with_token_and_policy(&bytes, &limited, &token) {
            Ok(info) => panic!("an encoded-byte limit must reject the input: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(
            error.kind(),
            image_slash_star::ImageErrorKind::LimitExceeded
        );
    }

    // Sequence decode cancellation carries the sequence stage for formats
    // with a real sequence path, and never returns a partial frame list.
    if cfg!(feature = "gif") {
        let bytes = fs::read(root.join("tests/fixtures/input/images/gif/animated_3frame.gif"))?;
        let token = image_slash_star::CancellationToken::new();
        let sequence = image_slash_star::decode_sequence_with_token(&bytes, &token)?;
        let legacy = image_slash_star::decode_sequence(&bytes)?;
        assert_eq!(sequence.content.frames.len(), legacy.content.frames.len());
        let cancelled = image_slash_star::CancellationToken::new();
        cancelled.cancel();
        let error = match image_slash_star::decode_sequence_with_token(&bytes, &cancelled) {
            Ok(info) => panic!("a cancelled token must stop the sequence: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Cancelled);
        assert_eq!(error.format(), Some(ImageFormat::Gif));
        assert_eq!(error.stage(), Some(ImageErrorStage::SequenceDecode));
        // A fresh token decodes the same input completely, proving the
        // cancelled attempt never corrupted reusable state.
        let fresh = image_slash_star::CancellationToken::new();
        let retry = image_slash_star::decode_sequence_with_token(&bytes, &fresh)?;
        assert_eq!(retry.content.frames.len(), legacy.content.frames.len());
    }

    // Still formats use the sequence fallback under the token API too.
    if cfg!(feature = "jpeg") {
        let bytes = fs::read(root.join("tests/fixtures/input/images/jpeg/1x1.jpg"))?;
        let token = image_slash_star::CancellationToken::new();
        let sequence = image_slash_star::decode_sequence_with_token(&bytes, &token)?;
        assert_eq!(sequence.content.frames.len(), 1);
        let cancelled = image_slash_star::CancellationToken::new();
        cancelled.cancel();
        let error = match image_slash_star::decode_sequence_with_token(&bytes, &cancelled) {
            Ok(info) => panic!("a cancelled token must stop the fallback sequence: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Cancelled);
    }

    // Token policy variants still apply limits and inspection preflight.
    if cfg!(feature = "png") {
        let bytes = fs::read(root.join("tests/fixtures/input/images/png/1x1.png"))?;
        let token = image_slash_star::CancellationToken::new();
        let limited = image_slash_star::DecodePolicy::default().with_max_encoded_bytes(10);
        let error =
            match image_slash_star::decode_sequence_with_token_and_policy(&bytes, &limited, &token)
            {
                Ok(info) => panic!("an encoded-byte limit must reject the input: {info:?}"),
                Err(error) => error,
            };
        assert_eq!(
            error.kind(),
            image_slash_star::ImageErrorKind::LimitExceeded
        );

        let width = image_slash_star::DecodePolicy::default().with_max_width(1);
        let decoded = image_slash_star::decode_with_token_and_policy(&bytes, &width, &token)?;
        assert_eq!(decoded.content.width, 1);
        let sequence =
            image_slash_star::decode_sequence_with_token_and_policy(&bytes, &width, &token)?;
        assert_eq!(sequence.content.frames.len(), 1);

        // Metadata and violating-width limits reject through the token
        // policy variants on both still and sequence paths.
        let metadata = image_slash_star::DecodePolicy::default().with_max_metadata_bytes(10);
        let error = match image_slash_star::decode_with_token_and_policy(&bytes, &metadata, &token)
        {
            Ok(info) => panic!("decode token policy must reject the metadata limit: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(
            error.kind(),
            image_slash_star::ImageErrorKind::LimitExceeded
        );
        let error = match image_slash_star::decode_sequence_with_token_and_policy(
            &bytes, &metadata, &token,
        ) {
            Ok(info) => panic!("sequence token policy must reject the metadata limit: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(
            error.kind(),
            image_slash_star::ImageErrorKind::LimitExceeded
        );

        let rejecting = image_slash_star::DecodePolicy::default().with_max_width(0);
        let error = match image_slash_star::decode_with_token_and_policy(&bytes, &rejecting, &token)
        {
            Ok(info) => panic!("decode token policy must reject an exceeding width: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(
            error.kind(),
            image_slash_star::ImageErrorKind::LimitExceeded
        );
        let error = match image_slash_star::decode_sequence_with_token_and_policy(
            &bytes, &rejecting, &token,
        ) {
            Ok(info) => {
                panic!("sequence token policy must reject an exceeding width: {info:?}")
            }
            Err(error) => error,
        };
        assert_eq!(
            error.kind(),
            image_slash_star::ImageErrorKind::LimitExceeded
        );

        // Inspection preflight truncation propagates as NeedMoreData.
        let error =
            match image_slash_star::decode_with_token_and_policy(&bytes[..40], &width, &token) {
                Ok(info) => panic!("a truncated header must need more data: {info:?}"),
                Err(error) => error,
            };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::NeedMoreData);
        let error = match image_slash_star::decode_sequence_with_token_and_policy(
            &bytes[..40],
            &width,
            &token,
        ) {
            Ok(info) => panic!("a truncated header must need more data: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::NeedMoreData);

        // Detection-level truncation propagates through the sequence policy
        // variant before any codec work.
        let error = match image_slash_star::decode_sequence_with_token_and_policy(
            &bytes[..5],
            &image_slash_star::DecodePolicy::default(),
            &token,
        ) {
            Ok(info) => panic!("a partial signature must need more data: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::NeedMoreData);
        assert_eq!(error.format(), None);

        // The cumulative sequence-byte budget fails inside the budget charge.
        let budgeted = image_slash_star::DecodePolicy::default().with_max_sequence_decoded_bytes(1);
        let error = match image_slash_star::decode_sequence_with_token_and_policy(
            &bytes, &budgeted, &token,
        ) {
            Ok(info) => panic!("the sequence budget must reject the primary frame: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(
            error.kind(),
            image_slash_star::ImageErrorKind::LimitExceeded
        );
    }
    Ok(())
}

#[test]
fn encode_cancellation_is_a_non_parity_contract() -> Result<(), Box<dyn std::error::Error>> {
    // Pillow has no caller-controlled cancellation token or OutputSink. These
    // assertions cover the Rust operation boundary and must not become parity
    // rows. Whole-buffer still codecs can only observe the token at dispatch;
    // sequence codecs additionally poll at frame and finalization boundaries.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    if cfg!(feature = "png") {
        let data = fs::read(root.join("tests/fixtures/input/images/png/1x1.png"))?;
        let decoded = image_slash_star::decode(&data)?;
        let options = EncodeOptions::for_format(ImageFormat::Png);
        let expected = image_slash_star::encode(&decoded.content, ImageFormat::Png, &options)?;
        let token = image_slash_star::CancellationToken::new();
        assert_eq!(
            image_slash_star::encode_with_token_and_policy(
                &decoded.content,
                ImageFormat::Png,
                &options,
                &image_slash_star::EncodePolicy::default(),
                &token,
            )?,
            expected,
            "an uncancelled still encode remains byte-identical"
        );

        let cancelled = image_slash_star::CancellationToken::new();
        cancelled.cancel();
        let error = match image_slash_star::encode_with_token(
            &decoded.content,
            ImageFormat::Png,
            &options,
            &cancelled,
        ) {
            Ok(bytes) => {
                return Err(format!("cancelled PNG encode returned {} bytes", bytes.len()).into());
            }
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Cancelled);
        assert_eq!(error.format(), Some(ImageFormat::Png));
        assert_eq!(error.stage(), Some(ImageErrorStage::StillEncode));

        let mut sink = vec![0xCC];
        assert!(matches!(
            image_slash_star::encode_to_sink_with_token(
                &decoded.content,
                ImageFormat::Png,
                &options,
                &cancelled,
                &mut sink,
            ),
            Err(ImageError::Cancelled { .. })
        ));
        assert_eq!(sink, vec![0xCC], "cancellation must precede sink writes");

        let token = image_slash_star::CancellationToken::new();
        let mut sink = vec![0xCC];
        let written = image_slash_star::encode_to_sink_with_token(
            &decoded.content,
            ImageFormat::Png,
            &options,
            &token,
            &mut sink,
        )?;
        assert_eq!(written, expected.len());
        assert_eq!(&sink[1..], expected.as_slice());
    }

    if cfg!(feature = "gif") {
        let data = fs::read(root.join("tests/fixtures/input/images/gif/animated_3frame.gif"))?;
        let sequence = image_slash_star::decode_sequence(&data)?.into_inner();
        let options = EncodeOptions::for_format(ImageFormat::Gif);
        let expected = image_slash_star::encode_sequence(&sequence, ImageFormat::Gif, &options)?;
        let token = image_slash_star::CancellationToken::new();
        assert_eq!(
            image_slash_star::encode_sequence_with_token(
                &sequence,
                ImageFormat::Gif,
                &options,
                &token,
            )?,
            expected,
            "an uncancelled sequence encode remains byte-identical"
        );

        let cancelled = image_slash_star::CancellationToken::new();
        cancelled.cancel();
        let error = match image_slash_star::encode_sequence_with_token_and_policy(
            &sequence,
            ImageFormat::Gif,
            &options,
            &image_slash_star::EncodePolicy::default(),
            &cancelled,
        ) {
            Ok(bytes) => {
                return Err(format!(
                    "cancelled GIF sequence encode returned {} bytes",
                    bytes.len()
                )
                .into());
            }
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Cancelled);
        assert_eq!(error.format(), Some(ImageFormat::Gif));
        assert_eq!(error.stage(), Some(ImageErrorStage::SequenceEncode));

        let mut sink = vec![0xDD];
        assert!(matches!(
            image_slash_star::encode_sequence_to_sink_with_token(
                &sequence,
                ImageFormat::Gif,
                &options,
                &cancelled,
                &mut sink,
            ),
            Err(ImageError::Cancelled { .. })
        ));
        assert_eq!(
            sink,
            vec![0xDD],
            "sequence cancellation must precede sink writes"
        );

        let token = image_slash_star::CancellationToken::new();
        let mut sink = vec![0xDD];
        let written = image_slash_star::encode_sequence_to_sink_with_token(
            &sequence,
            ImageFormat::Gif,
            &options,
            &token,
            &mut sink,
        )?;
        assert_eq!(written, expected.len());
        assert_eq!(&sink[1..], expected.as_slice());
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
    // (valid compressed payloads are validated but never exposed inflated),
    // while unknown ancillary chunks stay in the opaque-block list.
    let text = png_chunk(b"tEXt", b"Comment\0hello world");
    let ztext_payload =
        b"Author\0\0\x78\x9c\x2b\x4a\x2c\xd7\x4d\xce\xcf\x2d\x28\x4a\x2d\x2e\x4e\x4d\xd1\x4d\xaa\x2c\x49\x2d\x06\x00\x53\x72\x08\x01";
    let ztext = png_chunk(b"zTXt", ztext_payload);
    let malformed_ztext_payload = b"\0\0raw";
    let malformed_ztext = png_chunk(b"zTXt", malformed_ztext_payload);
    let uncompressed_itxt_payload = b"Comment\0\0\0\0\0text";
    let uncompressed_itxt = png_chunk(b"iTXt", uncompressed_itxt_payload);
    let malformed_itxt_payload = b"\0\x01\0\0\0not-zlib";
    let malformed_itxt = png_chunk(b"iTXt", malformed_itxt_payload);
    let iccp_payload =
        b"profile\0\0\x78\x9c\x2b\x4a\x2c\xd7\x2d\x28\xca\x4f\xcb\xcc\x49\xd5\x4d\xaa\x2c\x49\x2d\x06\x00\x3c\x34\x06\xbd";
    let iccp = png_chunk(b"iCCP", iccp_payload);
    let exif = png_chunk(b"eXIf", b"raw-exif-bytes");
    let unknown = png_chunk(b"prVt", b"unknown-payload");
    let idat_offset = png_chunk_offset(&base, b"IDAT")?;
    let iend_offset = png_chunk_offset(&base, b"IEND")?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&base[..idat_offset]);
    bytes.extend_from_slice(&text);
    bytes.extend_from_slice(&malformed_ztext);
    bytes.extend_from_slice(&uncompressed_itxt);
    bytes.extend_from_slice(&malformed_itxt);
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
            data: malformed_ztext_payload.to_vec(),
        },
        OpaqueMetadata {
            kind: b"iTXt".to_vec(),
            data: uncompressed_itxt_payload.to_vec(),
        },
        OpaqueMetadata {
            kind: b"iTXt".to_vec(),
            data: malformed_itxt_payload.to_vec(),
        },
        OpaqueMetadata {
            kind: b"zTXt".to_vec(),
            data: ztext_payload.to_vec(),
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
            data: iccp_payload[8..].to_vec(),
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
        vec![expected_metadata[0].clone(), expected_metadata[5].clone()],
        "APNG metadata"
    );
    assert_eq!(
        apng_sequence.content.opaque_blocks, expected_blocks,
        "APNG blocks"
    );
    Ok(())
}

// This is intentionally separate from the generated Pillow parity matrix.
// Pillow exposes no structured warning/recovery field for these successful
// decodes, so the expected kind, stage, offset, and identity are Rust
// defensive-model policy rather than oracle output. Keep this as a normal
// fixture-backed behavior contract, not a coverage-only diagnostic hook.
#[test]
fn diagnostic_manifest_matches_the_non_parity_contract() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: DiagnosticManifest = json::from_str(&fs::read_to_string(
        root.join("tests/fixtures/diagnostic_manifest.json"),
    )?)?;
    assert_eq!(manifest.format_version, 1);
    assert_eq!(manifest.assertion_origin, "defensive_model");
    assert_eq!(manifest.pillow_version, "12.2.0");

    for case in manifest.cases {
        let enabled = match case.feature.as_str() {
            "gif" => cfg!(feature = "gif"),
            "png" => cfg!(feature = "png"),
            other => panic!("{}: unknown feature `{other}`", case.id),
        };
        if !enabled {
            continue;
        }
        assert_eq!(case.pillow_outcome, "ok", "{}", case.id);
        let expected_format = match case.format.as_str() {
            "gif" => ImageFormat::Gif,
            "png" => ImageFormat::Png,
            other => panic!("{}: unknown format `{other}`", case.id),
        };
        let expected_kind = match case.diagnostic_kind.as_str() {
            "recovered_structure" => DiagnosticKind::RecoveredStructure,
            "invalid_metadata_ignored" => DiagnosticKind::InvalidMetadataIgnored,
            other => panic!("{}: unknown diagnostic kind `{other}`", case.id),
        };
        let expected_stage = match case.stage.as_str() {
            "still_decode" => ImageErrorStage::StillDecode,
            "sequence_decode" => ImageErrorStage::SequenceDecode,
            other => panic!("{}: unknown diagnostic stage `{other}`", case.id),
        };
        let expected_identity = match case.identity.as_str() {
            "gif_graphic_control" => "gif_graphic_control",
            "png_zTXt" => "png_zTXt",
            "png_iCCP" => "png_iCCP",
            "png_iTXt" => "png_iTXt",
            "png_IDAT_crc" => "png_IDAT_crc",
            "png_reserved_bit" => "png_reserved_bit",
            "png_ancillary_after_idat" => "png_ancillary_after_idat",
            other => panic!("{}: unknown diagnostic identity `{other}`", case.id),
        };
        let base = fs::read(root.join(&case.asset_path))?;
        let bytes = match case.mutation.as_str() {
            "none" => base.clone(),
            "png_before_idat" => {
                let kind: [u8; 4] = case
                    .chunk_kind
                    .as_bytes()
                    .try_into()
                    .map_err(|_| format!("{}: chunk kind is not four bytes", case.id))?;
                let payload = case
                    .chunk_payload
                    .iter()
                    .copied()
                    .map(u8::try_from)
                    .collect::<Result<Vec<_>, _>>()?;
                let idat_offset = png_chunk_offset(&base, b"IDAT")?;
                let mut mutated = Vec::with_capacity(base.len() + payload.len() + 12);
                mutated.extend_from_slice(&base[..idat_offset]);
                mutated.extend_from_slice(&png_chunk(&kind, &payload));
                mutated.extend_from_slice(&base[idat_offset..]);
                mutated
            }
            "png_after_idat" => {
                let kind: [u8; 4] = case
                    .chunk_kind
                    .as_bytes()
                    .try_into()
                    .map_err(|_| format!("{}: chunk kind is not four bytes", case.id))?;
                let payload = case
                    .chunk_payload
                    .iter()
                    .copied()
                    .map(u8::try_from)
                    .collect::<Result<Vec<_>, _>>()?;
                let iend_offset = png_chunk_offset(&base, b"IEND")?;
                let mut mutated = Vec::with_capacity(base.len() + payload.len() + 12);
                mutated.extend_from_slice(&base[..iend_offset]);
                mutated.extend_from_slice(&png_chunk(&kind, &payload));
                mutated.extend_from_slice(&base[iend_offset..]);
                mutated
            }
            other => panic!("{}: unknown mutation `{other}`", case.id),
        };

        let expected = ImageDiagnostic {
            kind: expected_kind,
            format: expected_format,
            stage: Some(expected_stage),
            offset: Some(case.offset),
            identity: Some(expected_identity),
        };
        match case.operation.as_str() {
            "decode" => {
                let base_decoded = image_slash_star::decode(&base)?;
                let decoded = image_slash_star::decode(&bytes)?;
                assert_eq!(decoded.format, expected_format, "{} format", case.id);
                assert_eq!(
                    decoded.content.pixels, base_decoded.content.pixels,
                    "{} pixels",
                    case.id
                );
                assert_eq!(
                    decoded.diagnostics,
                    vec![expected],
                    "{} diagnostic",
                    case.id
                );
                if expected_format == ImageFormat::Png {
                    assert!(decoded.content.metadata.is_empty(), "{} metadata", case.id);
                    assert!(decoded.content.source_color.is_empty(), "{} color", case.id);
                }
            }
            "decode_sequence" => {
                let base_sequence = image_slash_star::decode_sequence(&base)?;
                let sequence = image_slash_star::decode_sequence(&bytes)?;
                assert_eq!(sequence.format, expected_format, "{} format", case.id);
                assert_eq!(
                    sequence.content.frames, base_sequence.content.frames,
                    "{} frames",
                    case.id
                );
                assert_eq!(
                    sequence.diagnostics,
                    vec![expected],
                    "{} diagnostic",
                    case.id
                );
                if expected_format == ImageFormat::Png {
                    assert!(sequence.content.metadata.is_empty(), "{} metadata", case.id);
                    assert!(
                        sequence.content.source_color.is_empty(),
                        "{} color",
                        case.id
                    );
                }
            }
            other => panic!("{}: unknown operation `{other}`", case.id),
        }
    }
    Ok(())
}

#[test]
fn png_unsupported_compressed_metadata_methods_remain_fatal()
-> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(feature = "png") {
        return Ok(());
    }
    // Pillow rejects these full-file mutations. This test guards only that
    // observable fatal boundary: the Rust-only diagnostic field itself is not
    // added to the Pillow parity matrix, because Pillow has no such field.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let base = fs::read(root.join("tests/fixtures/input/images/png/1x1.png"))?;
    let idat_offset = png_chunk_offset(&base, b"IDAT")?;
    for (kind, payload) in [
        (b"zTXt", b"Comment\0\x01not-zlib".as_slice()),
        (b"iCCP", b"profile\0\x01not-zlib".as_slice()),
    ] {
        let chunk = png_chunk(kind, payload);
        let mut bytes = Vec::with_capacity(base.len() + chunk.len());
        bytes.extend_from_slice(&base[..idat_offset]);
        bytes.extend_from_slice(&chunk);
        bytes.extend_from_slice(&base[idat_offset..]);

        let error = match image_slash_star::decode(&bytes) {
            Ok(decoded) => {
                return Err(format!(
                    "PNG {:?} unsupported compression method was recovered with {:?}",
                    kind, decoded.diagnostics
                )
                .into());
            }
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Malformed);
        assert_eq!(error.format(), Some(ImageFormat::Png));
        assert_eq!(error.stage(), Some(ImageErrorStage::StillDecode));
        assert_eq!(error.offset(), Some(idat_offset as u64));
        assert_eq!(error.identity(), Some("png_chunk"));

        let sequence_error = match image_slash_star::decode_sequence(&bytes) {
            Ok(sequence) => {
                return Err(format!(
                    "PNG {:?} unsupported compression method was recovered in sequence with {:?}",
                    kind, sequence.diagnostics
                )
                .into());
            }
            Err(error) => error,
        };
        assert_eq!(
            sequence_error.kind(),
            image_slash_star::ImageErrorKind::Malformed
        );
        assert_eq!(sequence_error.format(), Some(ImageFormat::Png));
        assert_eq!(
            sequence_error.stage(),
            Some(ImageErrorStage::SequenceDecode)
        );
        assert_eq!(sequence_error.offset(), Some(idat_offset as u64));
        assert_eq!(sequence_error.identity(), Some("png_chunk"));
    }
    Ok(())
}

#[test]
fn png_compressed_metadata_shape_contract_preserves_raw_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::OpaqueMetadata;

    if !cfg!(feature = "png") {
        return Ok(());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let base = fs::read(root.join("tests/fixtures/input/images/png/1x1.png"))?;
    let cases: [(&[u8; 4], &[u8]); 8] = [
        (b"zTXt", b"no-nul"),
        (b"zTXt", b"Comment\0"),
        (b"iCCP", b"profile\0"),
        (b"iTXt", b"no-nul"),
        (b"iTXt", b"Comment\0"),
        (b"iTXt", b"Comment\0\x01"),
        (b"iTXt", b"Comment\0\x01\0"),
        (b"iTXt", b"Comment\0\x01\0lang\0"),
    ];
    let idat_offset = png_chunk_offset(&base, b"IDAT")?;
    let mut bytes = base[..idat_offset].to_vec();
    let mut expected = Vec::new();
    for (kind, payload) in cases {
        bytes.extend_from_slice(&png_chunk(kind, payload));
        expected.push(OpaqueMetadata {
            kind: kind.to_vec(),
            data: payload.to_vec(),
        });
    }
    bytes.extend_from_slice(&base[idat_offset..]);

    let decoded = image_slash_star::decode(&bytes)?;
    assert_eq!(decoded.content.metadata, expected);
    assert!(decoded.content.source_color.is_empty());
    assert!(decoded.diagnostics.is_empty());
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
    let iccp_payload =
        b"profile\0\0\x78\x9c\x2b\x4a\x2c\xd7\x2d\x28\xca\x4f\xcb\xcc\x49\xd5\x4d\xaa\x2c\x49\x2d\x06\x00\x3c\x34\x06\xbd";
    let iccp = png_chunk(b"iCCP", iccp_payload);
    let iccp_no_nul = png_chunk(b"iCCP", b"nonul");
    let iccp_nul_first = png_chunk(b"iCCP", b"\0raw");
    let iccp_no_profile = png_chunk(b"iCCP", b"a\0");
    let duplicate_iccp = png_chunk(
        b"iCCP",
        b"other\0\0\x78\x9c\x2b\x4a\x2c\xd7\x2d\x28\xca\x4f\xcb\xcc\x49\xd5\x4d\xaa\x2c\x49\x2d\x06\x00\x3c\x34\x06\xbd",
    );
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
            data: iccp_payload[8..].to_vec(),
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
            data: b"other\0\0\x78\x9c\x2b\x4a\x2c\xd7\x2d\x28\xca\x4f\xcb\xcc\x49\xd5\x4d\xaa\x2c\x49\x2d\x06\x00\x3c\x34\x06\xbd".to_vec(),
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

#[allow(
    clippy::cast_possible_truncation,
    reason = "GIF extension payloads in this test are tiny fixed literals"
)]
fn gif_extension(label: u8, payload: &[u8]) -> Vec<u8> {
    let mut extension = vec![0x21, label];
    for chunk in payload.chunks(255) {
        extension.push(chunk.len() as u8);
        extension.extend_from_slice(chunk);
    }
    extension.push(0);
    extension
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "GIF application identifiers in this test are tiny fixed literals"
)]
fn gif_app_extension(identifier: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut extension = vec![0x21, 0xff, identifier.len() as u8];
    extension.extend_from_slice(identifier);
    for chunk in payload.chunks(255) {
        extension.push(chunk.len() as u8);
        extension.extend_from_slice(chunk);
    }
    extension.push(0);
    extension
}

fn gif_image_separator_offset(data: &[u8]) -> Result<usize, Box<dyn std::error::Error>> {
    let packed = data[10];
    let palette_bytes = if packed & 0x80 != 0 {
        3usize.wrapping_mul(2usize.wrapping_shl((packed & 7).into()))
    } else {
        0
    };
    let start = 13usize.wrapping_add(palette_bytes);
    data[start..]
        .iter()
        .position(|&byte| byte == 0x2c)
        .map(|offset| start.wrapping_add(offset))
        .ok_or_else(|| "GIF image separator not found".into())
}

#[test]
fn gif_metadata_matches_the_container_contract() -> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::{OpaqueBlock, OpaqueMetadata};

    if !cfg!(feature = "gif") {
        return Ok(());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let base = fs::read(root.join("tests/fixtures/input/images/gif/1x1.gif"))?;

    let comment = gif_extension(0xfe, b"hello");
    let mut plain_text = vec![0x21, 0x01, 12];
    plain_text.extend_from_slice(&[0, 0, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0]);
    plain_text.extend_from_slice(&[3, b'p', b'l', b'a']);
    plain_text.push(0);
    let app = gif_app_extension(b"ANIMEXTS1.0", &[1, 2, 3]);
    let loop_extension = gif_app_extension(b"NETSCAPE2.0", &[1, 0xE8, 0x03]);
    let unknown_label = gif_extension(0xcc, &[0xDE, 0xAD]);
    let comment_after = gif_extension(0xfe, b"after");

    let separator = gif_image_separator_offset(&base)?;
    let trailer = base
        .iter()
        .rposition(|&byte| byte == 0x3b)
        .ok_or_else(|| "GIF trailer not found".to_string())?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&base[..separator]);
    for extension in [&comment, &plain_text, &app, &loop_extension, &unknown_label] {
        bytes.extend_from_slice(extension);
    }
    bytes.extend_from_slice(&base[separator..trailer]);
    bytes.extend_from_slice(&comment_after);
    bytes.extend_from_slice(&base[trailer..]);

    let expected_metadata = vec![
        OpaqueMetadata {
            kind: vec![0xfe],
            data: b"\x05hello\x00".to_vec(),
        },
        OpaqueMetadata {
            kind: vec![0x01],
            data: b"\x0c\x00\x00\x01\x01\x01\x01\x00\x00\x00\x00\x00\x00\x03pla\x00".to_vec(),
        },
        OpaqueMetadata {
            kind: vec![0xff],
            data: b"\x0bANIMEXTS1.0\x03\x01\x02\x03\x00".to_vec(),
        },
        OpaqueMetadata {
            kind: vec![0xfe],
            data: b"\x05after\x00".to_vec(),
        },
    ];
    let expected_blocks = vec![OpaqueBlock {
        kind: vec![0xcc],
        data: b"\x02\xde\xad\x00".to_vec(),
        safe_to_copy: true,
    }];

    let sequence = image_slash_star::decode_sequence(&bytes)?;
    assert_eq!(
        sequence.content.metadata, expected_metadata,
        "sequence metadata"
    );
    assert_eq!(
        sequence.content.opaque_blocks, expected_blocks,
        "sequence blocks"
    );
    assert_eq!(sequence.content.loop_count, Some(1000), "loop extension");
    let decoded = image_slash_star::decode(&bytes)?;
    assert_eq!(
        decoded.content.metadata, expected_metadata,
        "still metadata"
    );
    assert_eq!(
        decoded.content.opaque_blocks, expected_blocks,
        "still blocks"
    );

    // Default encoding never replays retained GIF extensions.
    let options = image_slash_star::EncodeOptions::for_format(ImageFormat::Gif);
    let encoded = image_slash_star::encode(&decoded.content, ImageFormat::Gif, &options)?;
    for needle in [
        &b"hello"[..],
        &b"ANIMEXTS1.0"[..],
        &b"pla"[..],
        &b"after"[..],
    ] {
        assert!(
            !encoded.windows(needle.len()).any(|window| window == needle),
            "encoded GIF must not replay retained extension {needle:?}"
        );
    }
    assert!(
        !encoded.windows(2).any(|window| window == [0x21, 0xcc]),
        "encoded GIF must not replay unknown-label extensions"
    );

    // The unmodified fixture retains no extensions.
    let plain = image_slash_star::decode_sequence(&base)?;
    assert!(plain.content.metadata.is_empty());
    assert!(plain.content.opaque_blocks.is_empty());

    // A truncated unknown-label extension exercises the error-propagation
    // path of the raw retention reader.
    let mut truncated = Vec::new();
    truncated.extend_from_slice(&base[..trailer]);
    truncated.extend_from_slice(&[0x21, 0xcc, 5, 1, 2]);
    truncated.extend_from_slice(&base[trailer..]);
    assert!(
        image_slash_star::decode_sequence(&truncated).is_err(),
        "truncated unknown-label extension must fail"
    );
    Ok(())
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "JPEG test segments are tiny fixed literals that always fit u16"
)]
fn jpeg_segment(marker: u8, payload: &[u8]) -> Vec<u8> {
    let mut segment = vec![0xff, marker];
    segment.extend_from_slice(&((payload.len() as u16).wrapping_add(2)).to_be_bytes());
    segment.extend_from_slice(payload);
    segment
}

fn jpeg_marker_offset(data: &[u8], marker: u16) -> Result<usize, Box<dyn std::error::Error>> {
    let needle = marker.to_be_bytes();
    data[2..]
        .windows(2)
        .position(|window| window == needle)
        .map(|offset| offset.wrapping_add(2))
        .ok_or_else(|| format!("JPEG marker {marker:#x} not found").into())
}

#[test]
fn jpeg_metadata_matches_the_container_contract() -> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::OpaqueMetadata;

    if !cfg!(feature = "jpeg") {
        return Ok(());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let base = fs::read(root.join("tests/fixtures/input/images/jpeg/1x1.jpg"))?;

    let app0_offset = jpeg_marker_offset(&base, 0xffe0)?;
    let app0_length = u16::from_be_bytes([base[app0_offset + 2], base[app0_offset + 3]]) as usize;
    let app0_payload = base[app0_offset + 4..app0_offset + app0_length + 2].to_vec();

    let mut adobe = b"Adobe".to_vec();
    adobe.extend_from_slice(&[0u8; 11]);
    let inserted = [
        jpeg_segment(0xe1, b"first-app1"),
        jpeg_segment(0xe2, b"icc-frag-1"),
        jpeg_segment(0xfe, b"hello comment"),
        jpeg_segment(0xe2, b"icc-frag-2"),
        jpeg_segment(0xee, &adobe),
    ];
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&base[..2]);
    for segment in &inserted {
        bytes.extend_from_slice(segment);
    }
    bytes.extend_from_slice(&base[2..]);

    let expected_metadata = vec![
        OpaqueMetadata {
            kind: vec![0xe1],
            data: b"first-app1".to_vec(),
        },
        OpaqueMetadata {
            kind: vec![0xe2],
            data: b"icc-frag-1".to_vec(),
        },
        OpaqueMetadata {
            kind: vec![0xfe],
            data: b"hello comment".to_vec(),
        },
        OpaqueMetadata {
            kind: vec![0xe2],
            data: b"icc-frag-2".to_vec(),
        },
        OpaqueMetadata {
            kind: vec![0xee],
            data: adobe.clone(),
        },
        OpaqueMetadata {
            kind: vec![0xe0],
            data: app0_payload.clone(),
        },
    ];

    let decoded = image_slash_star::decode(&bytes)?;
    assert_eq!(
        decoded.content.metadata, expected_metadata,
        "still metadata"
    );
    let sequence = image_slash_star::decode_sequence(&bytes)?;
    assert_eq!(
        sequence.content.metadata, expected_metadata,
        "sequence metadata"
    );

    // The unmodified fixture retains its JFIF APP0 record only.
    let plain = image_slash_star::decode(&base)?;
    assert_eq!(
        plain.content.metadata,
        vec![OpaqueMetadata {
            kind: vec![0xe0],
            data: app0_payload,
        }],
        "unmodified fixture metadata"
    );

    // Default encoding never replays retained JPEG markers.
    let options = image_slash_star::EncodeOptions::for_format(ImageFormat::Jpeg);
    let encoded = image_slash_star::encode(&decoded.content, ImageFormat::Jpeg, &options)?;
    for needle in [
        &b"first-app1"[..],
        &b"icc-frag-1"[..],
        &b"icc-frag-2"[..],
        &b"hello comment"[..],
    ] {
        assert!(
            !encoded.windows(needle.len()).any(|window| window == needle),
            "encoded JPEG must not replay retained marker {needle:?}"
        );
    }
    Ok(())
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "WebP test chunks are tiny fixed literals that always fit u32"
)]
fn webp_chunk(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut chunk = Vec::with_capacity(8usize.wrapping_add(payload.len()));
    chunk.extend_from_slice(kind);
    chunk.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    chunk.extend_from_slice(payload);
    if payload.len() & 1 != 0 {
        chunk.push(0);
    }
    chunk
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "WebP test buffers are tiny fixed literals that always fit u32"
)]
fn patch_riff_size(bytes: &mut [u8]) {
    let size = (bytes.len() as u32).wrapping_sub(8);
    bytes[4..8].copy_from_slice(&size.to_le_bytes());
}

#[test]
fn webp_metadata_matches_the_container_contract() -> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::{OpaqueBlock, OpaqueMetadata};

    if !cfg!(feature = "webp") {
        return Ok(());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let base = fs::read(root.join("tests/fixtures/input/images/webp/16x16.webp"))?;

    // Build an extended WebP from the fixture's VP8 chunk plus a VP8X header
    // and metadata/unknown chunks in stream order.
    let vp8_chunk = base[12..].to_vec();
    let mut vp8x = vec![0u8; 10];
    vp8x[4..7].copy_from_slice(&15u32.to_le_bytes()[..3]);
    vp8x[7..10].copy_from_slice(&15u32.to_le_bytes()[..3]);
    let iccp = webp_chunk(b"ICCP", b"webp-icc");
    let exif = webp_chunk(b"EXIF", b"webp-exif");
    let xmp = webp_chunk(b"XMP ", b"webp-xmp");
    let unknown = webp_chunk(b"ABCD", b"weird");
    let duplicate_iccp = webp_chunk(b"ICCP", b"second-icc");
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(b"WEBP");
    bytes.extend_from_slice(&webp_chunk(b"VP8X", &vp8x));
    for chunk in [&iccp, &exif, &xmp, &unknown, &duplicate_iccp] {
        bytes.extend_from_slice(chunk);
    }
    bytes.extend_from_slice(&vp8_chunk);
    patch_riff_size(&mut bytes);

    let expected_metadata = vec![
        OpaqueMetadata {
            kind: b"ICCP".to_vec(),
            data: b"webp-icc".to_vec(),
        },
        OpaqueMetadata {
            kind: b"EXIF".to_vec(),
            data: b"webp-exif".to_vec(),
        },
        OpaqueMetadata {
            kind: b"XMP ".to_vec(),
            data: b"webp-xmp".to_vec(),
        },
        OpaqueMetadata {
            kind: b"ICCP".to_vec(),
            data: b"second-icc".to_vec(),
        },
    ];
    let expected_blocks = vec![OpaqueBlock {
        kind: b"ABCD".to_vec(),
        data: b"weird".to_vec(),
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
    let sequence = image_slash_star::decode_sequence(&bytes)?;
    assert_eq!(
        sequence.content.metadata, expected_metadata,
        "sequence metadata"
    );
    assert_eq!(
        sequence.content.opaque_blocks, expected_blocks,
        "sequence blocks"
    );

    // A truncated metadata chunk is not retained, and decode still succeeds.
    let mut truncated = bytes.clone();
    let mut bad = b"ICCP".to_vec();
    bad.extend_from_slice(&100u32.to_le_bytes());
    bad.extend_from_slice(b"abc");
    truncated.extend_from_slice(&bad);
    patch_riff_size(&mut truncated);
    let truncated_decoded = image_slash_star::decode(&truncated)?;
    assert_eq!(
        truncated_decoded.content.metadata, expected_metadata,
        "truncated metadata chunk must be skipped"
    );

    // The unmodified fixture retains no chunks, and default encoding never
    // replays retained metadata.
    let plain = image_slash_star::decode(&base)?;
    assert!(plain.content.metadata.is_empty());
    assert!(plain.content.opaque_blocks.is_empty());
    let options = image_slash_star::EncodeOptions::for_format(ImageFormat::WebP);
    let encoded = image_slash_star::encode(&decoded.content, ImageFormat::WebP, &options)?;
    for needle in [
        &b"webp-icc"[..],
        &b"webp-exif"[..],
        &b"webp-xmp"[..],
        &b"weird"[..],
    ] {
        assert!(
            !encoded.windows(needle.len()).any(|window| window == needle),
            "encoded WebP must not replay retained chunk {needle:?}"
        );
    }

    // Animated sequence decode retains the same container-level records.
    let animated = fs::read(
        root.join("tests/fixtures/input/images/webp/animated_sequence_rgba_keyframes.webp"),
    )?;
    let mut animated_bytes = Vec::new();
    animated_bytes.extend_from_slice(&animated[..30]);
    animated_bytes.extend_from_slice(&webp_chunk(b"ICCP", b"anim-icc"));
    animated_bytes.extend_from_slice(&animated[30..]);
    patch_riff_size(&mut animated_bytes);
    let animated_sequence = image_slash_star::decode_sequence(&animated_bytes)?;
    assert_eq!(
        animated_sequence.content.metadata,
        vec![OpaqueMetadata {
            kind: b"ICCP".to_vec(),
            data: b"anim-icc".to_vec(),
        }],
        "animated metadata"
    );
    Ok(())
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::type_complexity,
    reason = "test TIFF buffers are tiny fixed literals that always fit u32/u16"
)]
#[test]
fn tiff_tags_match_the_container_contract() -> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::{OpaqueBlock, OpaqueMetadata};

    if !cfg!(feature = "tiff") {
        return Ok(());
    }

    fn entry(tag: u16, field_type: u16, count: u32, value: [u8; 4]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(12);
        bytes.extend_from_slice(&tag.to_le_bytes());
        bytes.extend_from_slice(&field_type.to_le_bytes());
        bytes.extend_from_slice(&count.to_le_bytes());
        bytes.extend_from_slice(&value);
        bytes
    }

    // (tag, field_type, count, inline value, out-of-line payload)
    let plan: Vec<(u16, u16, u32, [u8; 4], Vec<u8>)> = vec![
        (256, 3, 1, [1, 0, 0, 0], Vec::new()),
        (257, 3, 1, [1, 0, 0, 0], Vec::new()),
        (258, 3, 1, [8, 0, 0, 0], Vec::new()),
        (259, 3, 1, [1, 0, 0, 0], Vec::new()),
        (262, 3, 1, [1, 0, 0, 0], Vec::new()),
        (270, 2, 6, [0; 4], b"hello\0".to_vec()),
        (273, 4, 1, [0; 4], Vec::new()),
        (277, 3, 1, [1, 0, 0, 0], Vec::new()),
        (278, 3, 1, [1, 0, 0, 0], Vec::new()),
        (279, 4, 1, [0; 4], Vec::new()),
        (306, 2, 20, [0; 4], b"2026:08:01 00:00:00\0".to_vec()),
        (34_675, 7, 3, [1, 2, 3, 0], Vec::new()),
        (65_000, 3, 2, [2, 1, 0, 0], Vec::new()),
        (65_001, 1, 5, [0; 4], vec![9, 8, 7, 6, 5]),
        (65_000, 3, 1, [9, 0, 0, 0], Vec::new()),
        (65_003, 5, 1, [0; 4], vec![1, 2, 3, 4, 5, 6, 7, 8]),
    ];
    let ifd_count = plan.len() as u16;
    let mut bytes = b"II".to_vec();
    bytes.extend_from_slice(&42u16.to_le_bytes());
    bytes.extend_from_slice(&8u32.to_le_bytes());
    bytes.extend_from_slice(&ifd_count.to_le_bytes());
    let entries_start = 8usize.wrapping_add(2);
    for item in &plan {
        let tag = item.0;
        let field_type = item.1;
        let count = item.2;
        let value = if item.4.is_empty() { item.3 } else { [0; 4] };
        bytes.extend_from_slice(&entry(tag, field_type, count, value));
    }
    bytes.extend_from_slice(&[0; 4]);
    for (index, item) in plan.iter().enumerate() {
        if item.4.is_empty() {
            continue;
        }
        let offset = bytes.len() as u32;
        bytes.extend_from_slice(&item.4);
        if item.4.len() & 1 != 0 {
            bytes.push(0);
        }
        let entry_offset = entries_start.wrapping_add(index.wrapping_mul(12));
        bytes[entry_offset.wrapping_add(8)..entry_offset.wrapping_add(12)]
            .copy_from_slice(&offset.to_le_bytes());
    }
    let strip_offset = bytes.len() as u32;
    bytes.push(128);
    for (index, item) in plan.iter().enumerate() {
        if item.0 == 273 {
            let entry_offset = entries_start.wrapping_add(index.wrapping_mul(12));
            bytes[entry_offset.wrapping_add(8)..entry_offset.wrapping_add(12)]
                .copy_from_slice(&strip_offset.to_le_bytes());
        }
        if item.0 == 279 {
            let entry_offset = entries_start.wrapping_add(index.wrapping_mul(12));
            bytes[entry_offset.wrapping_add(8)..entry_offset.wrapping_add(12)]
                .copy_from_slice(&1u32.to_le_bytes());
        }
    }

    let expected_blocks = vec![
        OpaqueBlock {
            kind: 65_000u16.to_le_bytes().to_vec(),
            data: vec![2, 1, 0, 0],
            safe_to_copy: true,
        },
        OpaqueBlock {
            kind: 65_001u16.to_le_bytes().to_vec(),
            data: vec![9, 8, 7, 6, 5],
            safe_to_copy: true,
        },
        OpaqueBlock {
            kind: 65_000u16.to_le_bytes().to_vec(),
            data: vec![9, 0],
            safe_to_copy: true,
        },
        OpaqueBlock {
            kind: 65_003u16.to_le_bytes().to_vec(),
            data: vec![1, 2, 3, 4, 5, 6, 7, 8],
            safe_to_copy: true,
        },
    ];
    let expected_metadata = vec![
        OpaqueMetadata {
            kind: 270u16.to_le_bytes().to_vec(),
            data: b"hello\0".to_vec(),
        },
        OpaqueMetadata {
            kind: 306u16.to_le_bytes().to_vec(),
            data: b"2026:08:01 00:00:00\0".to_vec(),
        },
        OpaqueMetadata {
            kind: 34_675u16.to_le_bytes().to_vec(),
            data: vec![1, 2, 3],
        },
    ];

    let decoded = image_slash_star::decode(&bytes)?;
    assert_eq!(
        decoded.content.opaque_blocks, expected_blocks,
        "still blocks"
    );
    assert_eq!(
        decoded.content.metadata, expected_metadata,
        "still metadata"
    );
    let sequence = image_slash_star::decode_sequence(&bytes)?;
    assert_eq!(
        sequence.content.frames[0].image.opaque_blocks, expected_blocks,
        "page blocks"
    );
    assert_eq!(
        sequence.content.frames[0].image.metadata, expected_metadata,
        "page metadata"
    );

    // The unmodified fixture retains no records, and encoding never replays.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plain = fs::read(root.join("tests/fixtures/input/images/tiff/gray.tiff"))?;
    let plain_decoded = image_slash_star::decode(&plain)?;
    assert!(plain_decoded.content.opaque_blocks.is_empty());
    assert!(plain_decoded.content.metadata.is_empty());
    let options = image_slash_star::EncodeOptions::for_format(ImageFormat::Tiff);
    let encoded = image_slash_star::encode(&decoded.content, ImageFormat::Tiff, &options)?;
    for needle in [&b"hello"[..], &b"2026:08:01"[..]] {
        assert!(
            !encoded.windows(needle.len()).any(|window| window == needle),
            "encoded TIFF must not replay retained tag {needle:?}"
        );
    }
    Ok(())
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "AVIF test boxes are tiny fixed literals that always fit u32"
)]
fn avif_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut box_bytes = Vec::with_capacity(payload.len().wrapping_add(8));
    box_bytes.extend_from_slice(&((payload.len() as u32).wrapping_add(8)).to_be_bytes());
    box_bytes.extend_from_slice(kind);
    box_bytes.extend_from_slice(payload);
    box_bytes
}

#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "offsets are bounds-checked against the in-memory fixture slice"
)]
fn avif_box_offset(data: &[u8], kind: &[u8; 4]) -> Result<usize, Box<dyn std::error::Error>> {
    let mut position = 0usize;
    while position.wrapping_add(8) <= data.len() {
        let size = u32::from_be_bytes([
            data[position],
            data[position + 1],
            data[position + 2],
            data[position + 3],
        ]) as usize;
        if &data[position.wrapping_add(4)..position.wrapping_add(8)] == kind {
            return Ok(position);
        }
        position = if size == 1 {
            let extended = u64::from_be_bytes([
                data[position + 8],
                data[position + 9],
                data[position + 10],
                data[position + 11],
                data[position + 12],
                data[position + 13],
                data[position + 14],
                data[position + 15],
            ]);
            position.wrapping_add(16).wrapping_add(extended as usize)
        } else if size == 0 {
            return Err("AVIF box extends to end of input".into());
        } else {
            position.wrapping_add(size)
        };
    }
    Err(format!("AVIF box {kind:?} not found").into())
}

#[test]
fn avif_boxes_match_the_container_contract() -> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::OpaqueBlock;

    if cfg!(target_arch = "wasm32") || !cfg!(feature = "avif") {
        return Ok(());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let base = fs::read(root.join("tests/fixtures/input/images/avif/baseline.avif"))?;
    let mdat = avif_box_offset(&base, b"mdat")?;

    // Unknown and free/skip boxes appended after the pixel payload are
    // retained raw while decode behavior is unchanged (item extents point
    // into the untouched mdat).
    let unknown = avif_box(b"ABCD", b"unknown-payload");
    let free = avif_box(b"free", b"padding");
    let skip = avif_box(b"skip", b"more-padding");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&base[..mdat]);
    bytes.extend_from_slice(&base[mdat..]);
    bytes.extend_from_slice(&unknown);
    bytes.extend_from_slice(&free);
    bytes.extend_from_slice(&skip);

    let expected = vec![
        OpaqueBlock {
            kind: b"ABCD".to_vec(),
            data: unknown.clone(),
            safe_to_copy: true,
        },
        OpaqueBlock {
            kind: b"free".to_vec(),
            data: free.clone(),
            safe_to_copy: true,
        },
        OpaqueBlock {
            kind: b"skip".to_vec(),
            data: skip.clone(),
            safe_to_copy: true,
        },
    ];
    let decoded = image_slash_star::decode(&bytes)?;
    assert_eq!(decoded.content.opaque_blocks, expected, "still boxes");
    let sequence = image_slash_star::decode_sequence(&bytes)?;
    assert_eq!(sequence.content.opaque_blocks, expected, "sequence boxes");

    // The unmodified fixture retains nothing, and encoding never replays.
    let plain = image_slash_star::decode(&base)?;
    assert!(plain.content.opaque_blocks.is_empty());
    let options = image_slash_star::EncodeOptions::for_format(ImageFormat::Avif);
    let encoded = image_slash_star::encode(&decoded.content, ImageFormat::Avif, &options)?;
    for needle in [
        &b"unknown-payload"[..],
        &b"padding"[..],
        &b"more-padding"[..],
    ] {
        assert!(
            !encoded.windows(needle.len()).any(|window| window == needle),
            "encoded AVIF must not replay retained box {needle:?}"
        );
    }

    // A truncated trailing box is ignored without retention.
    let mut truncated = bytes.clone();
    truncated.extend_from_slice(&[0, 0, 0, 16, b'A', b'B', b'C', b'D', 1, 2, 3]);
    let truncated_decoded = image_slash_star::decode(&truncated)?;
    assert_eq!(
        truncated_decoded.content.opaque_blocks, expected,
        "truncated trailing box must be skipped"
    );
    Ok(())
}

#[test]
fn avif_primary_cicp_color_matches_the_container_contract() -> Result<(), Box<dyn std::error::Error>>
{
    use image_slash_star::{AvifChromaSamplePosition, AvifColorProperties, SourceColor};

    if cfg!(target_arch = "wasm32") || !cfg!(feature = "avif") {
        return Ok(());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bytes = fs::read(root.join("tests/fixtures/input/images/avif/baseline.avif"))?;
    let expected = SourceColor::new()
        .with_avif_color(AvifColorProperties {
            color_primaries: 1,
            transfer_characteristics: 13,
            matrix_coefficients: 6,
            full_range: true,
        })
        .with_avif_chroma_sample_position(AvifChromaSamplePosition::Unknown);
    assert_eq!(
        expected.avif_color(),
        Some(AvifColorProperties {
            color_primaries: 1,
            transfer_characteristics: 13,
            matrix_coefficients: 6,
            full_range: true,
        })
    );
    assert_eq!(
        expected.avif_chroma_sample_position(),
        Some(AvifChromaSamplePosition::Unknown)
    );

    // This is defensive/specification evidence for an item property. Pillow
    // parity rows still assert pixels and mode, but do not expose this
    // structured CICP declaration as an oracle result.
    let inspected = image_slash_star::inspect(&bytes)?;
    assert_eq!(inspected.source_color, expected, "AVIF inspect");
    let decoded = image_slash_star::decode(&bytes)?;
    assert_eq!(decoded.content.source_color, expected, "AVIF still decode");
    let sequence = image_slash_star::decode_sequence(&bytes)?;
    assert_eq!(
        sequence.content.source_color, expected,
        "AVIF sequence fallback"
    );
    assert_eq!(decoded.content.source_color, inspected.source_color);

    // Reserved nclx flag bits are a structural error, not a Pillow parity
    // outcome. The bounded parser must reject them before pixel decoding.
    let nclx = bytes
        .windows(4)
        .position(|window| window == b"nclx")
        .ok_or("baseline AVIF has no nclx property")?;
    let mut invalid = bytes.clone();
    invalid[nclx + 10] |= 1;
    let error = match image_slash_star::inspect(&invalid) {
        Ok(_) => return Err("reserved nclx bits must fail".into()),
        Err(error) => error,
    };
    assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Malformed);
    assert_eq!(error.identity(), Some("avif_box"));

    // A declared nclx payload with extra bytes is a malformed property in
    // both the inspection parser and the sample extractor.
    let payload_end = nclx
        .checked_add(11)
        .ok_or("baseline AVIF nclx payload offset overflowed")?;
    let mut extra = Vec::with_capacity(bytes.len() + 1);
    extra.extend_from_slice(&bytes[..payload_end]);
    extra.push(0);
    extra.extend_from_slice(&bytes[payload_end..]);
    let box_start = |kind: &[u8]| -> Result<usize, Box<dyn std::error::Error>> {
        let type_offset = bytes
            .windows(4)
            .position(|window| window == kind)
            .ok_or_else(|| format!("baseline AVIF has no {kind:?} box"))?;
        type_offset
            .checked_sub(4)
            .ok_or_else(|| format!("baseline AVIF {kind:?} box has no size field").into())
    };
    for kind in [&b"colr"[..], &b"ipco"[..], &b"iprp"[..], &b"meta"[..]] {
        let start = box_start(kind)?;
        let size = u32::from_be_bytes(bytes[start..start + 4].try_into()?) + 1;
        extra[start..start + 4].copy_from_slice(&size.to_be_bytes());
    }
    let error = match image_slash_star::inspect(&extra) {
        Ok(_) => return Err("extra nclx payload must fail inspection".into()),
        Err(error) => error,
    };
    assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Malformed);
    let error = match image_slash_star::decode(&extra) {
        Ok(_) => return Err("extra nclx payload must fail decode".into()),
        Err(error) => error,
    };
    assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Malformed);

    // Each truncated CICP field is a structural error. This is defensive
    // specification evidence; Pillow does not expose item-property parsing
    // as a parity oracle.
    for retained_payload in [0_usize, 4, 6, 8, 10] {
        let removed = 11_usize - retained_payload;
        let payload_end = nclx
            .checked_add(retained_payload)
            .ok_or("truncated nclx payload offset overflowed")?;
        let original_payload_end = nclx
            .checked_add(11)
            .ok_or("baseline AVIF nclx payload offset overflowed")?;
        let mut truncated = Vec::with_capacity(bytes.len() - removed);
        truncated.extend_from_slice(&bytes[..payload_end]);
        truncated.extend_from_slice(&bytes[original_payload_end..]);
        for kind in [&b"colr"[..], &b"ipco"[..], &b"iprp"[..], &b"meta"[..]] {
            let start = box_start(kind)?;
            let old_size = u32::from_be_bytes(bytes[start..start + 4].try_into()?);
            let new_size = old_size
                .checked_sub(u32::try_from(removed)?)
                .ok_or("AVIF box size underflowed while truncating nclx")?;
            truncated[start..start + 4].copy_from_slice(&new_size.to_be_bytes());
        }
        let error = match image_slash_star::inspect(&truncated) {
            Ok(_) => return Err("truncated nclx payload must fail inspection".into()),
            Err(error) => error,
        };
        assert_eq!(error.identity(), Some("avif_box"));
    }
    Ok(())
}

#[test]
fn avif_item_properties_match_the_non_parity_contract() -> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::{
        AvifChromaSamplePosition, AvifCleanAperture, AvifColorProperties, AvifContentLightLevel,
        AvifMasteringDisplayColorVolume, AvifMirrorAxis, AvifPixelAspectRatio, AvifRotation,
        AvifTransformProperties, OpaqueMetadata, RawIccProfile, SourceColor, SourceDescriptor,
    };

    // These helpers construct malformed/duplicate item-property witnesses
    // for the parser contract. Pillow parity does not expose structured AVIF
    // item properties, so none of these cases belongs in coverage_matrix.json.
    fn box_start(data: &[u8], kind: &[u8; 4]) -> Result<usize, Box<dyn std::error::Error>> {
        let type_offset = data
            .windows(4)
            .position(|window| window == kind)
            .ok_or_else(|| format!("AVIF fixture has no {kind:?} box"))?;
        type_offset
            .checked_sub(4)
            .ok_or_else(|| format!("AVIF {kind:?} box has no size field").into())
    }

    fn grow_box_size(
        data: &mut [u8],
        kind: &[u8; 4],
        amount: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let start = box_start(data, kind)?;
        let size = u32::from_be_bytes(data[start..start + 4].try_into()?)
            .checked_add(amount)
            .ok_or("AVIF box size overflowed")?;
        data[start..start + 4].copy_from_slice(&size.to_be_bytes());
        Ok(())
    }

    fn shrink_box_size(
        data: &mut [u8],
        kind: &[u8; 4],
        amount: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let start = box_start(data, kind)?;
        let size = u32::from_be_bytes(data[start..start + 4].try_into()?)
            .checked_sub(amount)
            .ok_or("AVIF box size underflowed")?;
        data[start..start + 4].copy_from_slice(&size.to_be_bytes());
        Ok(())
    }

    fn append_associated_property(
        input: &[u8],
        kind: &[u8; 4],
        payload: &[u8],
        property_index: u8,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let ipco = box_start(input, b"ipco")?;
        let ipco_size = usize::try_from(u32::from_be_bytes(input[ipco..ipco + 4].try_into()?))?;
        let property = avif_box(kind, payload);
        let ipco_end = ipco.checked_add(ipco_size).ok_or("ipco end overflowed")?;
        let mut output = Vec::with_capacity(input.len() + property.len() + 1);
        output.extend_from_slice(&input[..ipco_end]);
        output.extend_from_slice(&property);
        output.extend_from_slice(&input[ipco_end..]);

        let ipma = box_start(&output, b"ipma")?;
        let ipma_size = usize::try_from(u32::from_be_bytes(output[ipma..ipma + 4].try_into()?))?;
        let association_count = ipma
            .checked_add(18)
            .ok_or("ipma association count offset overflowed")?;
        let count = output
            .get_mut(association_count)
            .ok_or("ipma association count is missing")?;
        *count = count
            .checked_add(1)
            .ok_or("ipma association count overflowed")?;
        let ipma_end = ipma.checked_add(ipma_size).ok_or("ipma end overflowed")?;
        output.insert(ipma_end, property_index);

        let property_size = u32::try_from(property.len())?;
        let iprp_delta = property_size
            .checked_add(1)
            .ok_or("iprp delta overflowed")?;
        grow_box_size(&mut output, b"ipco", property_size)?;
        grow_box_size(&mut output, b"ipma", 1)?;
        grow_box_size(&mut output, b"iprp", iprp_delta)?;
        grow_box_size(&mut output, b"meta", iprp_delta)?;

        // This committed fixture uses iloc version 0 with one 32-bit file
        // extent and places mdat immediately after meta. The inserted
        // property plus its ipma association shifts that extent by the same
        // amount as the metadata growth.
        let iloc = box_start(&output, b"iloc")?;
        let extent_offset = iloc.checked_add(22).ok_or("iloc offset overflowed")?;
        let extent_end = extent_offset
            .checked_add(4)
            .ok_or("iloc extent offset end overflowed")?;
        let old_offset = u32::from_be_bytes(output[extent_offset..extent_end].try_into()?);
        let new_offset = old_offset
            .checked_add(iprp_delta)
            .ok_or("iloc extent offset overflowed")?;
        output[extent_offset..extent_end].copy_from_slice(&new_offset.to_be_bytes());
        Ok(output)
    }

    fn assert_malformed(data: &[u8], label: &str) -> Result<(), Box<dyn std::error::Error>> {
        let inspected = match image_slash_star::inspect(data) {
            Ok(_) => return Err(format!("{label}: malformed AVIF was inspected").into()),
            Err(error) => error,
        };
        assert_eq!(
            inspected.kind(),
            image_slash_star::ImageErrorKind::Malformed,
            "{label}: inspect"
        );
        let decoded = match image_slash_star::decode(data) {
            Ok(_) => return Err(format!("{label}: malformed AVIF was decoded").into()),
            Err(error) => error,
        };
        assert_eq!(
            decoded.kind(),
            image_slash_star::ImageErrorKind::Malformed,
            "{label}: decode"
        );
        let sequence = match image_slash_star::decode_sequence(data) {
            Ok(_) => return Err(format!("{label}: malformed AVIF became a sequence").into()),
            Err(error) => error,
        };
        assert_eq!(
            sequence.kind(),
            image_slash_star::ImageErrorKind::Malformed,
            "{label}: sequence"
        );
        Ok(())
    }

    if cfg!(target_arch = "wasm32") || !cfg!(feature = "avif") {
        return Ok(());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bytes =
        fs::read(root.join("tests/fixtures/outputs/encoded/Encode.avif_enc_orientation.bin"))?;
    let irot = bytes
        .windows(4)
        .position(|window| window == b"irot")
        .ok_or("orientation AVIF has no irot property")?;
    let baseline = image_slash_star::decode(&bytes)?;
    let expected_pixels = baseline.content.pixels;

    // ICC in AVIF is a `colr` item property, not a Pillow-observable decoded
    // field. Use the committed Pillow-generated metadata output as the source
    // witness, but keep the structured assertion in this defensive/specification
    // contract rather than adding a parity-matrix row.
    let metadata =
        fs::read(root.join("tests/fixtures/outputs/encoded/Encode.avif_enc_metadata.bin"))?;
    let expected_icc = SourceColor::new()
        .with_avif_color(AvifColorProperties {
            color_primaries: 2,
            transfer_characteristics: 2,
            matrix_coefficients: 6,
            full_range: true,
        })
        .with_avif_chroma_sample_position(AvifChromaSamplePosition::Unknown)
        .with_icc_profile(RawIccProfile {
            keyword: b"prof".to_vec(),
            data: b"pillow-rs-icc".to_vec(),
        });
    let metadata_inspected = image_slash_star::inspect(&metadata)?;
    assert_eq!(
        metadata_inspected.source_color, expected_icc,
        "AVIF ICC inspect"
    );
    let metadata_decoded = image_slash_star::decode(&metadata)?;
    assert_eq!(
        metadata_decoded.content.source_color, expected_icc,
        "AVIF ICC decode"
    );
    let metadata_sequence = image_slash_star::decode_sequence(&metadata)?;
    assert_eq!(
        metadata_sequence.content.source_color, expected_icc,
        "AVIF ICC sequence fallback"
    );
    let expected_metadata = vec![
        OpaqueMetadata {
            kind: b"Exif".to_vec(),
            data: b"\0\0\0\x06Exif\0\0II*\0\x08\0\0\0\0\0\0\0".to_vec(),
        },
        OpaqueMetadata {
            kind: b"XMP ".to_vec(),
            data: b"<x:xmpmeta/>".to_vec(),
        },
    ];
    assert_eq!(metadata_decoded.content.metadata, expected_metadata);
    assert_eq!(metadata_sequence.content.metadata, expected_metadata);

    let profile_box = box_start(&metadata, b"prof")?;
    let mut ricc = metadata;
    ricc[profile_box + 4..profile_box + 8].copy_from_slice(b"rICC");
    let expected_ricc = SourceColor::new()
        .with_avif_color(AvifColorProperties {
            color_primaries: 2,
            transfer_characteristics: 2,
            matrix_coefficients: 6,
            full_range: true,
        })
        .with_avif_chroma_sample_position(AvifChromaSamplePosition::Unknown)
        .with_icc_profile(RawIccProfile {
            keyword: b"rICC".to_vec(),
            data: b"pillow-rs-icc".to_vec(),
        });
    let ricc_inspected = image_slash_star::inspect(&ricc)?;
    assert_eq!(ricc_inspected.source_color, expected_ricc, "rICC inspect");
    let ricc_decoded = image_slash_star::decode(&ricc)?;
    assert_eq!(
        ricc_decoded.content.source_color, expected_ricc,
        "rICC decode"
    );
    assert_eq!(ricc_decoded.content.pixels, metadata_decoded.content.pixels);
    let ricc_sequence = image_slash_star::decode_sequence(&ricc)?;
    assert_eq!(
        ricc_sequence.content.source_color, expected_ricc,
        "rICC sequence fallback"
    );

    let empty_icc = append_associated_property(&bytes, b"colr", b"prof", 6)?;
    assert_malformed(&empty_icc, "empty AVIF ICC profile")?;

    // CLLI is an item property, not a Pillow-observable result. Keep this
    // witness in the defensive/specification contract rather than adding a
    // synthetic Pillow parity row or a coverage-only assertion.
    let clli_payload = [0x01, 0xf4, 0x00, 0x64];
    let clli = append_associated_property(&bytes, b"clli", &clli_payload, 6)?;
    let expected_clli = SourceColor::new()
        .with_avif_color(AvifColorProperties {
            color_primaries: 1,
            transfer_characteristics: 13,
            matrix_coefficients: 6,
            full_range: true,
        })
        .with_avif_chroma_sample_position(AvifChromaSamplePosition::Unknown)
        .with_avif_content_light_level(AvifContentLightLevel::new(500, 100));
    assert_eq!(
        expected_clli.avif_content_light_level(),
        Some(AvifContentLightLevel::new(500, 100))
    );
    let levels = expected_clli
        .avif_content_light_level()
        .ok_or("expected CLLI metadata")?;
    assert_eq!(levels.max_content_light_level(), 500);
    assert_eq!(levels.max_picture_average_light_level(), 100);
    let clli_inspected = image_slash_star::inspect(&clli)?;
    assert_eq!(clli_inspected.source_color, expected_clli, "clli inspect");
    let clli_decoded = image_slash_star::decode(&clli)?;
    assert_eq!(
        clli_decoded.content.source_color, expected_clli,
        "clli decode"
    );
    assert_eq!(clli_decoded.content.pixels, expected_pixels);
    let clli_sequence = image_slash_star::decode_sequence(&clli)?;
    assert_eq!(
        clli_sequence.content.source_color, expected_clli,
        "clli sequence"
    );

    // `mdcv` is a fixed-width HDR mastering-display declaration, not a
    // Pillow-observable result. Keep its exact field contract in this
    // defensive/specification test rather than adding a synthetic parity row.
    // The ISO-BMFF wire order is green, blue, red; the public descriptor is
    // normalized to red, green, blue accessors.
    let mdcv_payload = [
        0x11, 0x11, 0x22, 0x22, // green x/y
        0x33, 0x33, 0x44, 0x44, // blue x/y
        0x55, 0x55, 0x66, 0x66, // red x/y
        0x77, 0x77, 0x88, 0x88, // white point x/y
        0x00, 0x0f, 0x42, 0x40, // maximum luminance
        0x00, 0x00, 0x00, 0x32, // minimum luminance
    ];
    let mdcv = append_associated_property(&bytes, b"mdcv", &mdcv_payload, 6)?;
    let expected_mdcv_value = AvifMasteringDisplayColorVolume::new(
        0x5555, 0x6666, 0x1111, 0x2222, 0x3333, 0x4444, 0x7777, 0x8888, 1_000_000, 50,
    );
    let expected_mdcv = SourceColor::new()
        .with_avif_color(AvifColorProperties {
            color_primaries: 1,
            transfer_characteristics: 13,
            matrix_coefficients: 6,
            full_range: true,
        })
        .with_avif_chroma_sample_position(AvifChromaSamplePosition::Unknown)
        .with_avif_mastering_display_color_volume(expected_mdcv_value);
    assert_eq!(
        expected_mdcv.avif_mastering_display_color_volume(),
        Some(expected_mdcv_value)
    );
    assert!(!expected_mdcv.is_empty());
    let mdcv_inspected = image_slash_star::inspect(&mdcv)?;
    assert_eq!(mdcv_inspected.source_color, expected_mdcv, "mdcv inspect");
    let mdcv_decoded = image_slash_star::decode(&mdcv)?;
    assert_eq!(
        mdcv_decoded.content.source_color, expected_mdcv,
        "mdcv decode"
    );
    assert_eq!(mdcv_decoded.content.pixels, expected_pixels);
    let mdcv_sequence = image_slash_star::decode_sequence(&mdcv)?;
    assert_eq!(
        mdcv_sequence.content.source_color, expected_mdcv,
        "mdcv sequence"
    );

    let mdcv_box = box_start(&mdcv, b"mdcv")?;
    // Exercise each fixed-width reader boundary in both bounded parsers with
    // real malformed-property witnesses, rather than adding a coverage-only
    // hook. The final 23-byte case is retained as the one-byte truncation
    // contract; the earlier boundaries reach each preceding field failure.
    for retained in [0, 2, 4, 6, 8, 10, 12, 14, 16, 20, 23] {
        let removed = 24 - retained;
        let mut truncated_mdcv = Vec::with_capacity(mdcv.len() - removed);
        truncated_mdcv.extend_from_slice(&mdcv[..mdcv_box + 8 + retained]);
        truncated_mdcv.extend_from_slice(&mdcv[mdcv_box + 8 + 24..]);
        for kind in [b"mdcv", b"ipco", b"iprp", b"meta"] {
            shrink_box_size(&mut truncated_mdcv, kind, u32::try_from(removed)?)?;
        }
        assert_malformed(
            &truncated_mdcv,
            &format!("truncated mdcv payload at {retained} bytes"),
        )?;
    }

    let mut extra_mdcv = Vec::with_capacity(mdcv.len() + 1);
    extra_mdcv.extend_from_slice(&mdcv[..mdcv_box + 8 + 24]);
    extra_mdcv.push(0);
    extra_mdcv.extend_from_slice(&mdcv[mdcv_box + 8 + 24..]);
    for kind in [b"mdcv", b"ipco", b"iprp", b"meta"] {
        grow_box_size(&mut extra_mdcv, kind, 1)?;
    }
    assert_malformed(&extra_mdcv, "extra mdcv payload")?;

    let duplicate_mdcv = append_associated_property(&mdcv, b"mdcv", &mdcv_payload, 7)?;
    assert_malformed(&duplicate_mdcv, "duplicate mdcv association")?;

    let clli_box = box_start(&clli, b"clli")?;
    let mut empty_clli = Vec::with_capacity(clli.len() - 4);
    empty_clli.extend_from_slice(&clli[..clli_box + 8]);
    empty_clli.extend_from_slice(&clli[clli_box + 12..]);
    for kind in [b"clli", b"ipco", b"iprp", b"meta"] {
        shrink_box_size(&mut empty_clli, kind, 4)?;
    }
    assert_malformed(&empty_clli, "empty clli payload")?;

    let mut extra_clli = Vec::with_capacity(clli.len() + 1);
    extra_clli.extend_from_slice(&clli[..clli_box + 12]);
    extra_clli.push(0);
    extra_clli.extend_from_slice(&clli[clli_box + 12..]);
    for kind in [b"clli", b"ipco", b"iprp", b"meta"] {
        grow_box_size(&mut extra_clli, kind, 1)?;
    }
    assert_malformed(&extra_clli, "extra clli payload")?;

    for (value, rotation) in [
        (0, AvifRotation::Zero),
        (1, AvifRotation::CounterClockwise90),
        (2, AvifRotation::CounterClockwise180),
        (3, AvifRotation::CounterClockwise270),
    ] {
        let mut variant = bytes.clone();
        variant[irot + 4] = value;
        let expected = SourceDescriptor::new()
            .with_avif_transform(AvifTransformProperties::new().with_rotation(rotation));
        assert_eq!(
            expected.avif_transform().and_then(|value| value.rotation()),
            Some(rotation)
        );
        assert_eq!(
            expected.avif_transform().and_then(|value| value.mirror()),
            None
        );
        assert!(!expected.is_empty());
        let inspected = image_slash_star::inspect(&variant)?;
        assert_eq!(inspected.source, expected, "irot inspect value {value}");
        let decoded = image_slash_star::decode(&variant)?;
        assert_eq!(
            decoded.content.source, expected,
            "irot decode value {value}"
        );
        assert_eq!(decoded.content.pixels, expected_pixels);
        let sequence = image_slash_star::decode_sequence(&variant)?;
        assert_eq!(
            sequence.content.frames[0].image.source, expected,
            "irot sequence value {value}"
        );
        assert_eq!(sequence.content.frames[0].image.pixels, expected_pixels);
    }

    let mut mirrored = bytes.clone();
    mirrored[irot..irot + 4].copy_from_slice(b"imir");
    for (value, mirror) in [
        (0, AvifMirrorAxis::TopBottom),
        (1, AvifMirrorAxis::LeftRight),
    ] {
        mirrored[irot + 4] = value;
        let expected_mirror = SourceDescriptor::new()
            .with_avif_transform(AvifTransformProperties::new().with_mirror(mirror));
        assert_eq!(
            expected_mirror
                .avif_transform()
                .and_then(|transform| transform.mirror()),
            Some(mirror)
        );
        assert_eq!(
            expected_mirror
                .avif_transform()
                .and_then(|transform| transform.rotation()),
            None
        );
        let inspected = image_slash_star::inspect(&mirrored)?;
        assert_eq!(
            inspected.source, expected_mirror,
            "imir inspect value {value}"
        );
        let mirrored_decoded = image_slash_star::decode(&mirrored)?;
        assert_eq!(mirrored_decoded.content.source, expected_mirror);
        assert_eq!(mirrored_decoded.content.pixels, expected_pixels);
        let mirrored_sequence = image_slash_star::decode_sequence(&mirrored)?;
        assert_eq!(
            mirrored_sequence.content.frames[0].image.source, expected_mirror,
            "imir sequence value {value}"
        );
    }

    let pasp_payload = [0, 0, 0, 4, 0, 0, 0, 3];
    let pasp = append_associated_property(&bytes, b"pasp", &pasp_payload, 6)?;
    let expected_pasp = SourceDescriptor::new().with_avif_transform(
        AvifTransformProperties::new()
            .with_rotation(AvifRotation::CounterClockwise270)
            .with_pixel_aspect_ratio(AvifPixelAspectRatio::new(4, 3)),
    );
    assert_eq!(
        expected_pasp
            .avif_transform()
            .and_then(|transform| transform.pixel_aspect_ratio()),
        Some(AvifPixelAspectRatio::new(4, 3))
    );
    let pasp_inspected = image_slash_star::inspect(&pasp)?;
    assert_eq!(pasp_inspected.source, expected_pasp, "pasp inspect");
    let pasp_decoded = image_slash_star::decode(&pasp)?;
    assert_eq!(pasp_decoded.content.source, expected_pasp, "pasp decode");
    assert_eq!(pasp_decoded.content.pixels, expected_pixels);
    let pasp_sequence = image_slash_star::decode_sequence(&pasp)?;
    assert_eq!(
        pasp_sequence.content.frames[0].image.source, expected_pasp,
        "pasp sequence"
    );
    assert_eq!(
        pasp_sequence.content.frames[0].image.pixels,
        expected_pixels
    );

    let mut clap_payload = Vec::with_capacity(32);
    clap_payload.extend_from_slice(&baseline.content.width.to_be_bytes());
    clap_payload.extend_from_slice(&1_u32.to_be_bytes());
    clap_payload.extend_from_slice(&baseline.content.height.to_be_bytes());
    clap_payload.extend_from_slice(&1_u32.to_be_bytes());
    clap_payload.extend_from_slice(&0_i32.to_be_bytes());
    clap_payload.extend_from_slice(&1_u32.to_be_bytes());
    clap_payload.extend_from_slice(&0_i32.to_be_bytes());
    clap_payload.extend_from_slice(&1_u32.to_be_bytes());
    let clap = append_associated_property(&bytes, b"clap", &clap_payload, 0x86)?;
    let expected_clap = SourceDescriptor::new().with_avif_transform(
        AvifTransformProperties::new()
            .with_rotation(AvifRotation::CounterClockwise270)
            .with_clean_aperture(AvifCleanAperture::new(
                baseline.content.width,
                1,
                baseline.content.height,
                1,
                0,
                1,
                0,
                1,
            )),
    );
    assert_eq!(
        expected_clap
            .avif_transform()
            .and_then(|transform| transform.clean_aperture()),
        Some(AvifCleanAperture::new(
            baseline.content.width,
            1,
            baseline.content.height,
            1,
            0,
            1,
            0,
            1,
        ))
    );
    let clap_inspected = image_slash_star::inspect(&clap)?;
    assert_eq!(clap_inspected.source, expected_clap, "clap inspect");
    let clap_decoded = image_slash_star::decode(&clap)?;
    assert_eq!(clap_decoded.content.source, expected_clap, "clap decode");
    assert_eq!(clap_decoded.content.pixels, expected_pixels);
    let clap_sequence = image_slash_star::decode_sequence(&clap)?;
    assert_eq!(
        clap_sequence.content.frames[0].image.source, expected_clap,
        "clap sequence"
    );
    assert_eq!(
        clap_sequence.content.frames[0].image.pixels,
        expected_pixels
    );

    let signed_clap_payload = [
        0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 3, 0, 0, 0, 1, 0xff, 0xff, 0xff, 0xff, 0, 0, 0, 2, 0, 0,
        0, 1, 0, 0, 0, 2,
    ];
    let signed_clap = append_associated_property(&bytes, b"clap", &signed_clap_payload, 0x86)?;
    let expected_signed_clap = SourceDescriptor::new().with_avif_transform(
        AvifTransformProperties::new()
            .with_rotation(AvifRotation::CounterClockwise270)
            .with_clean_aperture(AvifCleanAperture::new(2, 1, 3, 1, -1, 2, 1, 2)),
    );
    assert_eq!(
        image_slash_star::inspect(&signed_clap)?.source,
        expected_signed_clap,
        "signed clap inspect"
    );

    let pasp_box = box_start(&pasp, b"pasp")?;
    let mut empty_pasp = Vec::with_capacity(pasp.len() - 1);
    empty_pasp.extend_from_slice(&pasp[..pasp_box + 8]);
    empty_pasp.extend_from_slice(&pasp[pasp_box + 9..]);
    for kind in [b"pasp", b"ipco", b"iprp", b"meta"] {
        shrink_box_size(&mut empty_pasp, kind, 1)?;
    }
    assert_malformed(&empty_pasp, "empty pasp payload")?;

    let mut extra_pasp = Vec::with_capacity(pasp.len() + 1);
    extra_pasp.extend_from_slice(&pasp[..pasp_box + 16]);
    extra_pasp.push(0);
    extra_pasp.extend_from_slice(&pasp[pasp_box + 16..]);
    for kind in [b"pasp", b"ipco", b"iprp", b"meta"] {
        grow_box_size(&mut extra_pasp, kind, 1)?;
    }
    assert_malformed(&extra_pasp, "extra pasp payload")?;

    let duplicate_pasp = append_associated_property(&pasp, b"pasp", &pasp_payload, 7)?;
    assert_malformed(&duplicate_pasp, "duplicate pasp association")?;

    let mut invalid_pasp = pasp.clone();
    invalid_pasp[pasp_box + 8..pasp_box + 12].fill(0);
    assert_malformed(&invalid_pasp, "zero pasp spacing")?;

    let mut invalid_v_pasp = pasp;
    invalid_v_pasp[pasp_box + 12..pasp_box + 16].fill(0);
    assert_malformed(&invalid_v_pasp, "zero pasp vertical spacing")?;

    let clap_box = box_start(&clap, b"clap")?;
    let mut empty_clap = Vec::with_capacity(clap.len() - 1);
    empty_clap.extend_from_slice(&clap[..clap_box + 8]);
    empty_clap.extend_from_slice(&clap[clap_box + 9..]);
    for kind in [b"clap", b"ipco", b"iprp", b"meta"] {
        shrink_box_size(&mut empty_clap, kind, 1)?;
    }
    assert_malformed(&empty_clap, "truncated clap payload")?;

    let mut extra_clap = Vec::with_capacity(clap.len() + 1);
    extra_clap.extend_from_slice(&clap[..clap_box + 40]);
    extra_clap.push(0);
    extra_clap.extend_from_slice(&clap[clap_box + 40..]);
    for kind in [b"clap", b"ipco", b"iprp", b"meta"] {
        grow_box_size(&mut extra_clap, kind, 1)?;
    }
    assert_malformed(&extra_clap, "extra clap payload")?;

    let duplicate_clap = append_associated_property(&clap, b"clap", &clap_payload, 0x87)?;
    assert_malformed(&duplicate_clap, "duplicate clap association")?;

    for (offset, label) in [
        (8, "zero clap width numerator"),
        (12, "zero clap width denominator"),
        (16, "zero clap height numerator"),
        (20, "zero clap height denominator"),
        (28, "zero clap horizontal offset denominator"),
        (36, "zero clap vertical offset denominator"),
    ] {
        let mut invalid_clap = clap.clone();
        invalid_clap[clap_box + offset..clap_box + offset + 4].fill(0);
        assert_malformed(&invalid_clap, label)?;
    }

    // Empty and overlong payloads are malformed even when the first byte is a
    // legal value. The size changes keep each witness at the same container
    // boundary, so the parser reaches the property-specific check.
    let mut empty_rotation = Vec::with_capacity(bytes.len() - 1);
    empty_rotation.extend_from_slice(&bytes[..irot + 4]);
    empty_rotation.extend_from_slice(&bytes[irot + 5..]);
    for kind in [b"irot", b"ipco", b"iprp", b"meta"] {
        shrink_box_size(&mut empty_rotation, kind, 1)?;
    }
    assert_malformed(&empty_rotation, "empty irot payload")?;

    let mut empty_mirror = Vec::with_capacity(mirrored.len() - 1);
    empty_mirror.extend_from_slice(&mirrored[..irot + 4]);
    empty_mirror.extend_from_slice(&mirrored[irot + 5..]);
    for kind in [b"imir", b"ipco", b"iprp", b"meta"] {
        shrink_box_size(&mut empty_mirror, kind, 1)?;
    }
    assert_malformed(&empty_mirror, "empty imir payload")?;

    let mut extra_rotation = Vec::with_capacity(bytes.len() + 1);
    extra_rotation.extend_from_slice(&bytes[..irot + 5]);
    extra_rotation.push(0);
    extra_rotation.extend_from_slice(&bytes[irot + 5..]);
    for kind in [b"irot", b"ipco", b"iprp", b"meta"] {
        grow_box_size(&mut extra_rotation, kind, 1)?;
    }
    assert_malformed(&extra_rotation, "extra irot payload")?;

    let mut extra_mirror = Vec::with_capacity(mirrored.len() + 1);
    extra_mirror.extend_from_slice(&mirrored[..irot + 5]);
    extra_mirror.push(0);
    extra_mirror.extend_from_slice(&mirrored[irot + 5..]);
    for kind in [b"imir", b"ipco", b"iprp", b"meta"] {
        grow_box_size(&mut extra_mirror, kind, 1)?;
    }
    assert_malformed(&extra_mirror, "extra imir payload")?;

    let duplicate_rotation = append_associated_property(&bytes, b"irot", &[1], 6)?;
    assert_malformed(&duplicate_rotation, "duplicate irot association")?;

    let duplicate_mirror = append_associated_property(&mirrored, b"imir", &[1], 6)?;
    assert_malformed(&duplicate_mirror, "duplicate imir association")?;

    let mut invalid_rotation = bytes;
    invalid_rotation[irot + 4] = 4;
    let error = match image_slash_star::inspect(&invalid_rotation) {
        Ok(_) => return Err("invalid irot was accepted".into()),
        Err(error) => error,
    };
    assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Malformed);
    assert_eq!(error.identity(), Some("avif_box"));
    let error = match image_slash_star::decode(&invalid_rotation) {
        Ok(_) => return Err("invalid irot decode was accepted".into()),
        Err(error) => error,
    };
    assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Malformed);

    let mut invalid_mirror = mirrored;
    invalid_mirror[irot + 4] = 2;
    let error = match image_slash_star::inspect(&invalid_mirror) {
        Ok(_) => return Err("invalid imir was accepted".into()),
        Err(error) => error,
    };
    assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Malformed);
    let error = match image_slash_star::decode(&invalid_mirror) {
        Ok(_) => return Err("invalid imir decode was accepted".into()),
        Err(error) => error,
    };
    assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Malformed);
    Ok(())
}

#[test]
fn destination_buffers_match_the_output_size_contract() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut cases: Vec<(&str, bool, &str, ImageMode)> = vec![
        (
            "png rgb",
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/1x1.png",
            ImageMode::Rgb8,
        ),
        (
            "png rgba",
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/alpha_checker.png",
            ImageMode::Rgba8,
        ),
        (
            "png bilevel",
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/1bit.png",
            ImageMode::L1,
        ),
        (
            "gif indexed",
            cfg!(feature = "gif"),
            "tests/fixtures/input/images/gif/1x1.gif",
            ImageMode::P8,
        ),
        (
            "tiff gray-alpha",
            cfg!(feature = "tiff"),
            "tests/fixtures/input/images/tiff/gray_alpha.tiff",
            ImageMode::La8,
        ),
        (
            "webp rgb",
            cfg!(feature = "webp"),
            "tests/fixtures/input/images/webp/16x16.webp",
            ImageMode::Rgb8,
        ),
        (
            "jpeg rgb",
            cfg!(feature = "jpeg"),
            "tests/fixtures/input/images/jpeg/1x1.jpg",
            ImageMode::Rgb8,
        ),
        (
            "bmp rgb",
            cfg!(feature = "bmp"),
            "tests/fixtures/input/images/bmp/1x1.bmp",
            ImageMode::Rgb8,
        ),
        (
            "ico rgb",
            cfg!(feature = "ico"),
            "tests/fixtures/input/images/ico/16x16.ico",
            ImageMode::Rgb8,
        ),
    ];
    if !cfg!(target_arch = "wasm32") && cfg!(feature = "avif") {
        cases.push((
            "avif rgb",
            true,
            "tests/fixtures/input/images/avif/baseline.avif",
            ImageMode::Rgb8,
        ));
    }

    for &(name, enabled, path, expected_mode) in &cases {
        if !enabled {
            continue;
        }
        let data = fs::read(root.join(path))?;
        let info = image_slash_star::inspect(&data)?;
        assert_eq!(info.mode, expected_mode, "{name} mode");
        let expected = info.decoded_bytes()?;
        let decoded = image_slash_star::decode(&data)?;
        assert_eq!(decoded.content.pixels.len(), expected, "{name} length");

        let mut exact = vec![0xAA; expected];
        let into_decoded = image_slash_star::decode_into(&data, &mut exact)?;
        assert_eq!(exact, decoded.content.pixels, "{name} exact destination");
        assert_eq!(
            into_decoded.content, decoded.content,
            "{name} returned image"
        );

        let mut short = vec![0xAA; expected.saturating_sub(1)];
        assert!(
            matches!(
                image_slash_star::decode_into(&data, &mut short),
                Err(ImageError::Parameter { .. })
            ),
            "{name} short destination must be rejected"
        );
        assert!(
            short.iter().all(|&byte| byte == 0xAA),
            "{name} short destination must remain untouched"
        );

        let mut oversized = vec![0xAA; expected.saturating_add(1)];
        assert!(
            matches!(
                image_slash_star::decode_into(&data, &mut oversized),
                Err(ImageError::Parameter { .. })
            ),
            "{name} oversized destination must be rejected"
        );
        assert!(
            oversized.iter().all(|&byte| byte == 0xAA),
            "{name} oversized destination must remain untouched"
        );
    }

    // Policy limits still apply before the destination length check.
    if cfg!(feature = "png") {
        let data = fs::read(root.join("tests/fixtures/input/images/png/1x1.png"))?;
        let mut buffer = vec![0xAA; 3];
        let strict = image_slash_star::DecodePolicy::new().with_max_encoded_bytes(1);
        assert!(matches!(
            image_slash_star::decode_into_with_policy(&data, &strict, &mut buffer),
            Err(ImageError::LimitExceeded { .. })
        ));
        assert!(buffer.iter().all(|&byte| byte == 0xAA));
    }
    Ok(())
}

#[test]
fn transfer_layout_matches_the_output_contract() -> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::TransferLayout;

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cases: &[(&str, bool, &str)] = &[
        (
            "png bilevel",
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/1bit.png",
        ),
        (
            "png rgba",
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/alpha_checker.png",
        ),
        (
            "gif indexed",
            cfg!(feature = "gif"),
            "tests/fixtures/input/images/gif/1x1.gif",
        ),
        (
            "tiff gray-alpha",
            cfg!(feature = "tiff"),
            "tests/fixtures/input/images/tiff/gray_alpha.tiff",
        ),
        (
            "jpeg rgb",
            cfg!(feature = "jpeg"),
            "tests/fixtures/input/images/jpeg/1x1.jpg",
        ),
    ];
    for &(name, enabled, path) in cases {
        if !enabled {
            continue;
        }
        let data = fs::read(root.join(path))?;
        let info = image_slash_star::inspect(&data)?;
        let layout = info.transfer_layout()?;
        assert_eq!(
            layout,
            TransferLayout::from_mode(info.mode, info.width, info.height)?,
            "{name} from_mode"
        );
        assert_eq!(layout.width, info.width, "{name} width");
        assert_eq!(layout.height, info.height, "{name} height");
        assert_eq!(layout.mode, info.mode, "{name} mode");
        assert_eq!(layout.total_bytes, info.decoded_bytes()?, "{name} total");
        assert_eq!(
            layout.packed_rows,
            info.mode == ImageMode::L1,
            "{name} packed rows"
        );
        assert_eq!(layout.alignment, 1, "{name} alignment");
        let expected_row_bytes = if info.mode == ImageMode::L1 {
            (info.width as usize).div_ceil(8)
        } else {
            layout.total_bytes / (info.height as usize)
        };
        assert_eq!(layout.row_bytes, expected_row_bytes, "{name} row bytes");

        let decoded = image_slash_star::decode(&data)?;
        assert_eq!(decoded.content.transfer_layout()?, layout, "{name} decoded");
        let mut buffer = vec![0xAA; layout.total_bytes];
        let _ = image_slash_star::decode_into(&data, &mut buffer)?;
        assert_eq!(buffer.len(), layout.total_bytes, "{name} destination");
    }
    Ok(())
}

#[allow(
    clippy::type_complexity,
    reason = "the fixture expectation tuple is compact and local"
)]
#[test]
fn basic_inspection_reports_completeness() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    // (name, enabled, path, basic_complete, basic_count, basic_animated,
    //  full_complete, full_count_min)
    let cases: &[(&str, bool, &str, bool, Option<u32>, bool, bool, u32)] = &[
        (
            "gif animated",
            cfg!(feature = "gif"),
            "tests/fixtures/input/images/gif/animated_3frame.gif",
            false,
            None,
            false,
            true,
            3,
        ),
        (
            "gif still",
            cfg!(feature = "gif"),
            "tests/fixtures/input/images/gif/1x1.gif",
            true,
            Some(1),
            false,
            true,
            1,
        ),
        (
            "tiff multipage",
            cfg!(feature = "tiff"),
            "tests/fixtures/input/images/tiff/multipage.tiff",
            false,
            None,
            false,
            true,
            2,
        ),
        (
            "tiff single",
            cfg!(feature = "tiff"),
            "tests/fixtures/input/images/tiff/1bit.tiff",
            true,
            Some(1),
            false,
            true,
            1,
        ),
        (
            "webp animated",
            cfg!(feature = "webp"),
            "tests/fixtures/input/images/webp/animated_sequence_rgba_keyframes.webp",
            false,
            None,
            true,
            true,
            2,
        ),
        (
            "webp still",
            cfg!(feature = "webp"),
            "tests/fixtures/input/images/webp/16x16.webp",
            true,
            Some(1),
            false,
            true,
            1,
        ),
        (
            "webp extended still",
            cfg!(feature = "webp"),
            "tests/fixtures/input/images/webp/alpha_uncompressed.webp",
            true,
            Some(1),
            false,
            true,
            1,
        ),
        (
            "png apng",
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/apng_l_over.png",
            true,
            Some(2),
            true,
            true,
            2,
        ),
        (
            "png still",
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/1x1.png",
            true,
            Some(1),
            false,
            true,
            1,
        ),
        (
            "jpeg still",
            cfg!(feature = "jpeg"),
            "tests/fixtures/input/images/jpeg/1x1.jpg",
            true,
            Some(1),
            false,
            true,
            1,
        ),
        (
            "bmp still",
            cfg!(feature = "bmp"),
            "tests/fixtures/input/images/bmp/1x1.bmp",
            true,
            Some(1),
            false,
            true,
            1,
        ),
        (
            "ico still",
            cfg!(feature = "ico"),
            "tests/fixtures/input/images/ico/16x16.ico",
            true,
            Some(1),
            false,
            true,
            1,
        ),
    ];
    for &(
        name,
        enabled,
        path,
        basic_complete,
        basic_count,
        basic_animated,
        full_complete,
        full_min,
    ) in cases
    {
        if !enabled {
            continue;
        }
        let data = fs::read(root.join(path))?;
        let full = image_slash_star::inspect(&data)?;
        let basic = image_slash_star::inspect_basic(&data)?;
        assert_eq!(basic.format, full.format, "{name} format");
        assert_eq!(basic.width, full.width, "{name} width");
        assert_eq!(basic.height, full.height, "{name} height");
        assert_eq!(basic.mode, full.mode, "{name} mode");
        assert_eq!(basic.bit_depth, full.bit_depth, "{name} bit depth");
        assert_eq!(basic.palette, full.palette, "{name} palette");
        assert_eq!(
            basic.frame_count_complete, basic_complete,
            "{name} basic completeness"
        );
        assert_eq!(basic.frame_count, basic_count, "{name} basic count");
        assert_eq!(basic.is_animated, basic_animated, "{name} basic animated");
        assert_eq!(
            full.frame_count_complete, full_complete,
            "{name} full completeness"
        );
        assert!(
            full.frame_count.is_some_and(|count| count >= full_min),
            "{name} full count"
        );
    }

    if !cfg!(target_arch = "wasm32") && cfg!(feature = "avif") {
        let data = fs::read(root.join("tests/fixtures/input/images/avif/baseline.avif"))?;
        let full = image_slash_star::inspect(&data)?;
        let basic = image_slash_star::inspect_basic(&data)?;
        assert_eq!(basic, full, "avif basic matches full");
    }
    assert!(
        image_slash_star::inspect_basic(b"not an image").is_err(),
        "unknown signature must fail basic inspection"
    );
    assert!(
        image_slash_star::inspect_basic(b"GIF89a").is_err(),
        "truncated header must fail basic inspection"
    );
    Ok(())
}

#[test]
fn borrowed_view_matches_the_owned_snapshot_contract() -> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::{DecodePolicy, EncodedImageView, VerificationScope};

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cases: &[(&str, bool, &str)] = &[
        (
            "png",
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/1x1.png",
        ),
        (
            "gif animated",
            cfg!(feature = "gif"),
            "tests/fixtures/input/images/gif/animated_3frame.gif",
        ),
        (
            "webp",
            cfg!(feature = "webp"),
            "tests/fixtures/input/images/webp/16x16.webp",
        ),
        (
            "tiff multipage",
            cfg!(feature = "tiff"),
            "tests/fixtures/input/images/tiff/multipage.tiff",
        ),
    ];
    for &(name, enabled, path) in cases {
        if !enabled {
            continue;
        }
        let data = fs::read(root.join(path))?;
        let view = EncodedImageView::new(&data)?;
        let expected = image_slash_star::inspect(&data)?;
        assert_eq!(view.format(), expected.format, "{name} format");
        assert_eq!(view.info(), &expected, "{name} info");
        assert_eq!(
            view.decoded_bytes()?,
            expected.decoded_bytes()?,
            "{name} decoded bytes"
        );
        assert_eq!(
            view.transfer_layout()?,
            expected.transfer_layout()?,
            "{name} transfer layout"
        );
        assert_eq!(
            view.decode()?,
            image_slash_star::decode(&data)?,
            "{name} decode"
        );
        assert_eq!(
            view.decode_sequence()?,
            image_slash_star::decode_sequence(&data)?,
            "{name} sequence"
        );
        let default = DecodePolicy::default();
        assert_eq!(
            view.decode_with_policy(&default)?,
            image_slash_star::decode(&data)?,
            "{name} policy decode"
        );
        assert_eq!(
            view.decode_sequence_with_policy(&default)?,
            image_slash_star::decode_sequence(&data)?,
            "{name} policy sequence"
        );
        assert!(view.verify().is_ok(), "{name} verify");
        let provided = view.verification_scope();
        assert!(
            view.verify_with_scope(provided).is_ok(),
            "{name} provided scope"
        );
        let stronger = if provided == VerificationScope::HeaderOnly {
            VerificationScope::Structure
        } else {
            VerificationScope::FullPixels
        };
        assert!(
            view.verify_with_scope(stronger).is_err(),
            "{name} stronger scope"
        );
        let strict = DecodePolicy::new().with_max_encoded_bytes(1);
        assert!(
            EncodedImageView::new_with_policy(&data, &strict).is_err(),
            "{name} strict construction"
        );
        assert!(
            view.decode_with_policy(&strict).is_err(),
            "{name} strict decode"
        );
    }
    Ok(())
}

#[test]
fn source_bound_frame_decode_matches_sequence_ordering() -> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::{EncodedImage, EncodedImageView};

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cases: &[(&str, bool, &str)] = &[
        (
            "gif animated",
            cfg!(feature = "gif"),
            "tests/fixtures/input/images/gif/animated_3frame.gif",
        ),
        (
            "tiff multipage",
            cfg!(feature = "tiff"),
            "tests/fixtures/input/images/tiff/multipage.tiff",
        ),
        (
            "png apng",
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/apng_l_over.png",
        ),
        (
            "webp animated",
            cfg!(feature = "webp"),
            "tests/fixtures/input/images/webp/animated_sequence_rgba_keyframes.webp",
        ),
    ];
    for &(name, enabled, path) in cases {
        if !enabled {
            continue;
        }
        let data = fs::read(root.join(path))?;
        let source = EncodedImage::new(data.clone())?;
        let count = source.info().frame_count.unwrap_or(1);
        let sequence = image_slash_star::decode_sequence(&data)?.into_inner();
        assert_eq!(
            u32::try_from(sequence.frames.len()).unwrap_or(u32::MAX),
            count,
            "{name} frame count"
        );
        let view = EncodedImageView::new(&data)?;
        for (index, frame) in sequence.frames.iter().enumerate() {
            let index_u32 = u32::try_from(index).unwrap_or(u32::MAX);
            assert_eq!(
                source.decode_frame(index_u32)?,
                *frame,
                "{name} owned frame {index}"
            );
            assert_eq!(
                view.decode_frame(index_u32)?,
                *frame,
                "{name} view frame {index}"
            );
        }
        assert!(
            matches!(
                source.decode_frame(count),
                Err(ImageError::Parameter { .. })
            ),
            "{name} out-of-range frame must fail"
        );
    }

    if cfg!(feature = "png") {
        let data = fs::read(root.join("tests/fixtures/input/images/png/1x1.png"))?;
        let source = EncodedImage::new(data)?;
        let still = source.decode()?;
        assert_eq!(
            source.decode_frame(0)?.image.pixels,
            still.content.pixels,
            "still frame zero"
        );
        assert!(
            matches!(source.decode_frame(1), Err(ImageError::Parameter { .. })),
            "still format has only one frame"
        );
    }
    Ok(())
}

#[test]
fn output_sinks_receive_the_exact_encoded_bytes() -> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::{EncodeOptions, EncodePolicy, ImageFormat, OutputSink};

    struct FailingSink;
    impl OutputSink for FailingSink {
        fn write_all(&mut self, _bytes: &[u8]) -> image_slash_star::ImageResult<()> {
            Err(image_slash_star::ImageError::Unsupported {
                format: None,
                message: "sink rejected the write".to_owned(),
                stage: None,
                reason: None,
                offset: None,
                identity: None,
            })
        }
    }

    struct FailingAfterWrites {
        fail_at: usize,
        writes: usize,
    }

    impl OutputSink for FailingAfterWrites {
        fn write_all(&mut self, _bytes: &[u8]) -> image_slash_star::ImageResult<()> {
            self.writes += 1;
            if self.writes >= self.fail_at {
                return Err(image_slash_star::ImageError::Unsupported {
                    format: None,
                    message: "sink rejected a later write".to_owned(),
                    stage: None,
                    reason: None,
                    offset: None,
                    identity: None,
                });
            }
            Ok(())
        }
    }

    struct RecordingSink {
        bytes: Vec<u8>,
        writes: usize,
    }

    impl OutputSink for RecordingSink {
        fn write_all(&mut self, bytes: &[u8]) -> image_slash_star::ImageResult<()> {
            self.writes += 1;
            self.bytes.extend_from_slice(bytes);
            Ok(())
        }
    }

    struct CancellingSink {
        bytes: Vec<u8>,
        token: image_slash_star::CancellationToken,
        writes: usize,
    }

    impl OutputSink for CancellingSink {
        fn write_all(&mut self, bytes: &[u8]) -> image_slash_star::ImageResult<()> {
            self.writes += 1;
            self.bytes.extend_from_slice(bytes);
            if self.writes == 1 {
                self.token.cancel();
            }
            Ok(())
        }
    }

    // Exercise both standard-library sink impls directly, because the generic
    // encode functions only ever select the `&mut Vec<u8>` implementation.
    let mut direct = Vec::new();
    OutputSink::write_all(&mut direct, b"abc")?;
    assert_eq!(direct, b"abc");
    let mut direct_ref: &mut Vec<u8> = &mut Vec::new();
    OutputSink::write_all(&mut direct_ref, b"def")?;
    assert_eq!(*direct_ref, b"def");

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    if cfg!(feature = "png") {
        let data = fs::read(root.join("tests/fixtures/input/images/png/1x1.png"))?;
        let decoded = image_slash_star::decode(&data)?;
        let options = EncodeOptions::for_format(ImageFormat::Png);
        let expected = image_slash_star::encode(&decoded.content, ImageFormat::Png, &options)?;

        let mut owned = Vec::new();
        assert_eq!(
            image_slash_star::encode_to_sink(
                &decoded.content,
                ImageFormat::Png,
                &options,
                &mut owned
            )?,
            expected.len(),
            "owned sink length"
        );
        assert_eq!(owned, expected, "owned sink bytes");

        let mut borrowed = Vec::new();
        let mut borrowed_ref: &mut Vec<u8> = &mut borrowed;
        assert_eq!(
            image_slash_star::encode_to_sink(
                &decoded.content,
                ImageFormat::Png,
                &options,
                &mut borrowed_ref
            )?,
            expected.len(),
            "borrowed sink length"
        );
        assert_eq!(borrowed, expected, "borrowed sink bytes");

        // The PNG still writer emits the validated container in structural
        // pieces. This is a Rust-only output-delivery contract: Pillow has no
        // caller-owned sink and the parity matrix remains unchanged.
        let mut structural = RecordingSink {
            bytes: Vec::new(),
            writes: 0,
        };
        assert_eq!(
            image_slash_star::encode_to_sink(
                &decoded.content,
                ImageFormat::Png,
                &options,
                &mut structural,
            )?,
            expected.len()
        );
        assert!(
            structural.writes > 1,
            "PNG output must cross write boundaries"
        );
        assert_eq!(structural.bytes, expected);
        assert_eq!(&structural.bytes[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&structural.bytes[8..12], &13u32.to_be_bytes());
        assert_eq!(&structural.bytes[12..16], b"IHDR");

        // A sink can cancel between structural writes. The already-delivered
        // prefix is intentionally observable; cleanup/rollback remains a
        // future destination contract rather than an unproved guarantee.
        let token = image_slash_star::CancellationToken::new();
        let mut cancelling = CancellingSink {
            bytes: Vec::new(),
            token: token.clone(),
            writes: 0,
        };
        let error = match image_slash_star::encode_to_sink_with_token(
            &decoded.content,
            ImageFormat::Png,
            &options,
            &token,
            &mut cancelling,
        ) {
            Ok(length) => {
                return Err(format!(
                    "sink-triggered cancellation unexpectedly wrote {length} bytes"
                )
                .into());
            }
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Cancelled);
        assert_eq!(error.format(), Some(ImageFormat::Png));
        assert_eq!(error.stage(), Some(ImageErrorStage::StillEncode));
        assert_eq!(cancelling.writes, 1);
        assert_eq!(cancelling.bytes, b"\x89PNG\r\n\x1a\n");

        let sequence = image_slash_star::DecodedSequence::from_image(decoded.content.clone());
        let sequence_expected =
            image_slash_star::encode_sequence(&sequence, ImageFormat::Png, &options)?;
        let mut sequence_sink = RecordingSink {
            bytes: Vec::new(),
            writes: 0,
        };
        assert_eq!(
            image_slash_star::encode_sequence_to_sink(
                &sequence,
                ImageFormat::Png,
                &options,
                &mut sequence_sink,
            )?,
            sequence_expected.len()
        );
        assert_eq!(sequence_sink.bytes, sequence_expected);
        assert!(sequence_sink.writes > 1);

        let sequence_token = image_slash_star::CancellationToken::new();
        let mut token_sequence_sink = RecordingSink {
            bytes: Vec::new(),
            writes: 0,
        };
        assert_eq!(
            image_slash_star::encode_sequence_to_sink_with_token(
                &sequence,
                ImageFormat::Png,
                &options,
                &sequence_token,
                &mut token_sequence_sink,
            )?,
            sequence_expected.len()
        );
        assert_eq!(token_sequence_sink.bytes, sequence_expected);
        assert!(token_sequence_sink.writes > 1);

        let mismatch_options = EncodeOptions::for_format(ImageFormat::Gif);
        let mut mismatch_sink = RecordingSink {
            bytes: Vec::new(),
            writes: 0,
        };
        let mismatch_error = match image_slash_star::encode_to_sink(
            &decoded.content,
            ImageFormat::Png,
            &mismatch_options,
            &mut mismatch_sink,
        ) {
            Ok(length) => {
                return Err(
                    format!("PNG accepted mismatched options and wrote {length} bytes").into(),
                );
            }
            Err(error) => error,
        };
        assert_eq!(
            mismatch_error.kind(),
            image_slash_star::ImageErrorKind::Parameter
        );
        assert_eq!(mismatch_error.stage(), Some(ImageErrorStage::StillEncode));
        assert_eq!(mismatch_sink.writes, 0);
        assert!(mismatch_sink.bytes.is_empty());

        let mut sequence_mismatch_sink = RecordingSink {
            bytes: Vec::new(),
            writes: 0,
        };
        let sequence_mismatch_error = match image_slash_star::encode_sequence_to_sink(
            &sequence,
            ImageFormat::Png,
            &mismatch_options,
            &mut sequence_mismatch_sink,
        ) {
            Ok(length) => {
                return Err(format!(
                    "PNG sequence accepted mismatched options and wrote {length} bytes"
                )
                .into());
            }
            Err(error) => error,
        };
        assert_eq!(
            sequence_mismatch_error.kind(),
            image_slash_star::ImageErrorKind::Parameter
        );
        assert_eq!(
            sequence_mismatch_error.stage(),
            Some(ImageErrorStage::SequenceEncode)
        );
        assert_eq!(sequence_mismatch_sink.writes, 0);

        let mut multiple = sequence.clone();
        multiple.frames.push(multiple.frames[0].clone());
        multiple.kind = image_slash_star::SequenceKind::TimedAnimation;
        let mut multiple_sink = RecordingSink {
            bytes: Vec::new(),
            writes: 0,
        };
        let multiple_error = match image_slash_star::encode_sequence_to_sink(
            &multiple,
            ImageFormat::Png,
            &options,
            &mut multiple_sink,
        ) {
            Ok(length) => {
                return Err(format!(
                    "PNG accepted multiple retained frames and wrote {length} bytes"
                )
                .into());
            }
            Err(error) => error,
        };
        assert_eq!(
            multiple_error.kind(),
            image_slash_star::ImageErrorKind::Unsupported
        );
        assert_eq!(
            multiple_error.unsupported_reason(),
            Some(UnsupportedReason::NotImplemented)
        );
        assert_eq!(
            multiple_error.stage(),
            Some(ImageErrorStage::SequenceEncode)
        );
        assert_eq!(multiple_sink.writes, 0);

        let mut retained_metadata = sequence.clone();
        retained_metadata.loop_count = Some(1);
        let mut metadata_sink = RecordingSink {
            bytes: Vec::new(),
            writes: 0,
        };
        let metadata_error = match image_slash_star::encode_sequence_to_sink(
            &retained_metadata,
            ImageFormat::Png,
            &options,
            &mut metadata_sink,
        ) {
            Ok(length) => {
                return Err(format!(
                    "PNG accepted retained sequence metadata and wrote {length} bytes"
                )
                .into());
            }
            Err(error) => error,
        };
        assert_eq!(
            metadata_error.kind(),
            image_slash_star::ImageErrorKind::Unsupported
        );
        assert_eq!(metadata_error.unsupported_reason(), None);
        assert_eq!(
            metadata_error.stage(),
            Some(ImageErrorStage::SequenceEncode)
        );
        assert_eq!(metadata_sink.writes, 0);

        let invalid_sequence = image_slash_star::DecodedSequence {
            width: 0,
            height: 1,
            frames: Vec::new(),
            loop_count: None,
            background: None,
            kind: image_slash_star::SequenceKind::SingleFrame,
            opaque_blocks: Vec::new(),
            metadata: Vec::new(),
            source_color: SourceColor::new(),
        };
        let mut invalid_sequence_sink = RecordingSink {
            bytes: Vec::new(),
            writes: 0,
        };
        let invalid_sequence_error = match image_slash_star::encode_sequence_to_sink(
            &invalid_sequence,
            ImageFormat::Png,
            &options,
            &mut invalid_sequence_sink,
        ) {
            Ok(length) => {
                return Err(
                    format!("PNG accepted an invalid sequence and wrote {length} bytes").into(),
                );
            }
            Err(error) => error,
        };
        assert_eq!(
            invalid_sequence_error.kind(),
            image_slash_star::ImageErrorKind::Dimensions
        );
        assert_eq!(invalid_sequence_error.stage(), None);
        assert_eq!(invalid_sequence_sink.writes, 0);

        let mut sequence_failing = FailingSink;
        let sequence_failing_error = match image_slash_star::encode_sequence_to_sink(
            &sequence,
            ImageFormat::Png,
            &options,
            &mut sequence_failing,
        ) {
            Ok(length) => {
                return Err(
                    format!("failing PNG sequence sink unexpectedly wrote {length} bytes").into(),
                );
            }
            Err(error) => error,
        };
        assert_eq!(
            sequence_failing_error.kind(),
            image_slash_star::ImageErrorKind::OutputWrite
        );
        assert_eq!(
            sequence_failing_error.stage(),
            Some(ImageErrorStage::SequenceEncode)
        );

        let mut token_sequence_failing = FailingSink;
        let token_sequence_failing_error =
            match image_slash_star::encode_sequence_to_sink_with_token(
                &sequence,
                ImageFormat::Png,
                &options,
                &sequence_token,
                &mut token_sequence_failing,
            ) {
                Ok(length) => {
                    return Err(format!(
                        "failing token PNG sequence sink unexpectedly wrote {length} bytes"
                    )
                    .into());
                }
                Err(error) => error,
            };
        assert_eq!(
            token_sequence_failing_error.kind(),
            image_slash_star::ImageErrorKind::OutputWrite
        );
        assert_eq!(
            token_sequence_failing_error.stage(),
            Some(ImageErrorStage::SequenceEncode)
        );

        let mut failing = FailingSink;
        let error = match image_slash_star::encode_to_sink(
            &decoded.content,
            ImageFormat::Png,
            &options,
            &mut failing,
        ) {
            Ok(length) => {
                return Err(format!("failing sink unexpectedly wrote {length} bytes").into());
            }
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::OutputWrite);
        assert_eq!(error.format(), Some(ImageFormat::Png));
        assert_eq!(error.stage(), Some(ImageErrorStage::StillEncode));
        // Output-destination failures are Rust-only contract errors, not
        // Pillow-parity rows, and therefore never carry an UnsupportedReason.
        assert_eq!(error.unsupported_reason(), None);
        assert_eq!(error.offset(), None);
        assert_eq!(error.identity(), None);
        assert_eq!(error.minimum_input(), None);
        assert_eq!(
            error.message(),
            Some("unsupported: sink rejected the write")
        );
        assert_eq!(
            error.to_string(),
            "failed to write Png output: unsupported: sink rejected the write"
        );
        let unselected = ImageError::OutputWrite {
            format: None,
            message: "unselected destination".to_owned(),
            stage: None,
        };
        assert_eq!(
            unselected.to_string(),
            "failed to write encoded output: unselected destination"
        );

        // Invalid still input exercises the encoder error path before the sink.
        let invalid = image_slash_star::DecodedImage::new(
            1,
            1,
            Vec::new(),
            image_slash_star::ColorType::Rgb8,
        );
        let mut unused_sink = Vec::new();
        assert!(
            image_slash_star::encode_to_sink(
                &invalid,
                ImageFormat::Png,
                &options,
                &mut unused_sink
            )
            .is_err(),
            "invalid still input must fail before the sink"
        );

        if cfg!(feature = "bmp") {
            // BMP still encoding now emits its validated header, palette, and
            // rows structurally. This is a Rust-only output-delivery
            // contract: Pillow has no caller-owned sink, so the parity matrix
            // remains unchanged.
            let bmp_options = EncodeOptions::for_format(ImageFormat::Bmp);
            let expected_bmp =
                image_slash_star::encode(&decoded.content, ImageFormat::Bmp, &bmp_options)?;
            let mut bmp_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            assert_eq!(
                image_slash_star::encode_to_sink(
                    &decoded.content,
                    ImageFormat::Bmp,
                    &bmp_options,
                    &mut bmp_sink,
                )?,
                expected_bmp.len()
            );
            assert_eq!(bmp_sink.bytes, expected_bmp);
            assert!(bmp_sink.writes > 1);

            let bmp_token = image_slash_star::CancellationToken::new();
            let mut bmp_token_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            assert_eq!(
                image_slash_star::encode_to_sink_with_token(
                    &decoded.content,
                    ImageFormat::Bmp,
                    &bmp_options,
                    &bmp_token,
                    &mut bmp_token_sink,
                )?,
                expected_bmp.len()
            );
            assert_eq!(bmp_token_sink.bytes, expected_bmp);
            assert!(bmp_token_sink.writes > 1);

            let mut failing_bmp = FailingSink;
            let failing_bmp_error = match image_slash_star::encode_to_sink(
                &decoded.content,
                ImageFormat::Bmp,
                &bmp_options,
                &mut failing_bmp,
            ) {
                Ok(length) => {
                    return Err(
                        format!("failing BMP sink unexpectedly wrote {length} bytes").into(),
                    );
                }
                Err(error) => error,
            };
            assert_eq!(
                failing_bmp_error.kind(),
                image_slash_star::ImageErrorKind::OutputWrite
            );
            assert_eq!(failing_bmp_error.format(), Some(ImageFormat::Bmp));
            assert_eq!(
                failing_bmp_error.stage(),
                Some(ImageErrorStage::StillEncode)
            );

            let mismatch_options = EncodeOptions::for_format(ImageFormat::Gif);
            let mut mismatch_bmp_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let mismatch_bmp_error = match image_slash_star::encode_to_sink(
                &decoded.content,
                ImageFormat::Bmp,
                &mismatch_options,
                &mut mismatch_bmp_sink,
            ) {
                Ok(length) => {
                    return Err(format!(
                        "BMP accepted mismatched options and wrote {length} bytes"
                    )
                    .into());
                }
                Err(error) => error,
            };
            assert_eq!(
                mismatch_bmp_error.kind(),
                image_slash_star::ImageErrorKind::Parameter
            );
            assert_eq!(mismatch_bmp_sink.writes, 0);

            // Later-segment destination failures exercise the real
            // OutputWrite contract for each BMP payload shape. These are
            // Rust-only sink cases; Pillow parity has no sink equivalent.
            let assert_bmp_later_failure = |image: &image_slash_star::DecodedImage,
                                            fail_at: usize|
             -> Result<(), Box<dyn std::error::Error>> {
                let mut sink = FailingAfterWrites { fail_at, writes: 0 };
                let error = match image_slash_star::encode_to_sink(
                    image,
                    ImageFormat::Bmp,
                    &bmp_options,
                    &mut sink,
                ) {
                    Ok(length) => {
                        return Err(format!(
                                "BMP later-write failure unexpectedly wrote {length} bytes in {} writes",
                                sink.writes
                            )
                        .into());
                    }
                    Err(error) => error,
                };
                assert_eq!(error.kind(), image_slash_star::ImageErrorKind::OutputWrite);
                assert_eq!(error.format(), Some(ImageFormat::Bmp));
                assert_eq!(error.stage(), Some(ImageErrorStage::StillEncode));
                Ok(())
            };
            let l1 = image_slash_star::DecodedImage::with_mode(8, 1, vec![0], ImageMode::L1);
            assert_bmp_later_failure(&l1, 2)?;
            assert_bmp_later_failure(&l1, 3)?;
            let l8 = image_slash_star::DecodedImage::new(1, 1, vec![0], ColorType::L8);
            assert_bmp_later_failure(&l8, 2)?;
            assert_bmp_later_failure(&l8, 3)?;
            let palette_less_indexed =
                image_slash_star::DecodedImage::with_mode(1, 1, vec![0], ImageMode::P8);
            assert_bmp_later_failure(&palette_less_indexed, 3)?;
            let rgba =
                image_slash_star::DecodedImage::new(1, 1, vec![0, 0, 0, 255], ColorType::Rgba8);
            assert_bmp_later_failure(&rgba, 2)?;

            let cancelling_bmp_token = image_slash_star::CancellationToken::new();
            let mut cancelling_bmp = CancellingSink {
                bytes: Vec::new(),
                token: cancelling_bmp_token.clone(),
                writes: 0,
            };
            let cancelling_bmp_error = match image_slash_star::encode_to_sink_with_token(
                &decoded.content,
                ImageFormat::Bmp,
                &bmp_options,
                &cancelling_bmp_token,
                &mut cancelling_bmp,
            ) {
                Ok(length) => {
                    return Err(format!(
                        "BMP sink-triggered cancellation unexpectedly wrote {length} bytes"
                    )
                    .into());
                }
                Err(error) => error,
            };
            assert_eq!(
                cancelling_bmp_error.kind(),
                image_slash_star::ImageErrorKind::Cancelled
            );
            assert_eq!(cancelling_bmp_error.format(), Some(ImageFormat::Bmp));
            assert_eq!(
                cancelling_bmp_error.stage(),
                Some(ImageErrorStage::StillEncode)
            );
            assert_eq!(cancelling_bmp.writes, 1);
            assert_eq!(cancelling_bmp.bytes, &expected_bmp[..54]);

            // A pre-cancelled BMP still stops before its first structural
            // write. This is a Rust-only interruption contract; Pillow has
            // no equivalent token or caller-owned sink.
            let pre_cancelled_bmp_token = image_slash_star::CancellationToken::new();
            pre_cancelled_bmp_token.cancel();
            let mut pre_cancelled_bmp = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let pre_cancelled_bmp_error = match image_slash_star::encode_to_sink_with_token(
                &decoded.content,
                ImageFormat::Bmp,
                &bmp_options,
                &pre_cancelled_bmp_token,
                &mut pre_cancelled_bmp,
            ) {
                Ok(length) => {
                    return Err(
                        format!("pre-cancelled BMP unexpectedly wrote {length} bytes").into(),
                    );
                }
                Err(error) => error,
            };
            assert_eq!(
                pre_cancelled_bmp_error.kind(),
                image_slash_star::ImageErrorKind::Cancelled
            );
            assert_eq!(pre_cancelled_bmp_error.format(), Some(ImageFormat::Bmp));
            assert_eq!(
                pre_cancelled_bmp_error.stage(),
                Some(ImageErrorStage::StillEncode)
            );
            assert_eq!(pre_cancelled_bmp.writes, 0);
            assert!(pre_cancelled_bmp.bytes.is_empty());

            let too_small = EncodePolicy::default()
                .with_max_output_bytes(u64::try_from(expected_bmp.len() - 1)?);
            let mut limited_bmp = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let limited_bmp_error = match image_slash_star::encode_to_sink_with_policy(
                &decoded.content,
                ImageFormat::Bmp,
                &bmp_options,
                &too_small,
                &mut limited_bmp,
            ) {
                Ok(length) => {
                    return Err(
                        format!("BMP output policy unexpectedly admitted {length} bytes").into(),
                    );
                }
                Err(error) => error,
            };
            assert_eq!(
                limited_bmp_error.kind(),
                image_slash_star::ImageErrorKind::LimitExceeded
            );
            assert_eq!(limited_bmp.writes, 0);

            let bmp_sequence =
                image_slash_star::DecodedSequence::from_image(decoded.content.clone());
            let mut bmp_sequence_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            assert_eq!(
                image_slash_star::encode_sequence_to_sink_with_token(
                    &bmp_sequence,
                    ImageFormat::Bmp,
                    &bmp_options,
                    &bmp_token,
                    &mut bmp_sequence_sink,
                )?,
                expected_bmp.len()
            );
            assert_eq!(bmp_sequence_sink.bytes, expected_bmp);
            // Sequence BMP still uses the existing whole-buffer fallback;
            // structural sequence writing is a separate roadmap item.
            assert_eq!(bmp_sequence_sink.writes, 1);

            let mut invalid_bmp_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            assert!(
                image_slash_star::encode_to_sink(
                    &invalid,
                    ImageFormat::Bmp,
                    &bmp_options,
                    &mut invalid_bmp_sink,
                )
                .is_err(),
                "invalid BMP still input must fail before the sink"
            );
            assert_eq!(invalid_bmp_sink.writes, 0);

            let invalid_bmp_token = image_slash_star::CancellationToken::new();
            let mut invalid_bmp_token_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            assert!(
                image_slash_star::encode_to_sink_with_token(
                    &invalid,
                    ImageFormat::Bmp,
                    &bmp_options,
                    &invalid_bmp_token,
                    &mut invalid_bmp_token_sink,
                )
                .is_err(),
                "invalid token BMP still input must fail before the sink"
            );
            assert_eq!(invalid_bmp_token_sink.writes, 0);
        }

        if cfg!(feature = "jpeg") {
            // Exercise the generic whole-buffer sink fallback so the
            // structural-writer dispatch remains explicit and complete.
            let jpeg_image =
                image_slash_star::DecodedImage::new(1, 1, vec![0, 0, 0], ColorType::Rgb8);
            let jpeg_options = EncodeOptions::for_format(ImageFormat::Jpeg);
            let jpeg_expected =
                image_slash_star::encode(&jpeg_image, ImageFormat::Jpeg, &jpeg_options)?;
            let mut jpeg_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            assert_eq!(
                image_slash_star::encode_to_sink(
                    &jpeg_image,
                    ImageFormat::Jpeg,
                    &jpeg_options,
                    &mut jpeg_sink,
                )?,
                jpeg_expected.len()
            );
            assert_eq!(jpeg_sink.bytes, jpeg_expected);
            assert_eq!(jpeg_sink.writes, 1);

            let jpeg_token = image_slash_star::CancellationToken::new();
            let mut jpeg_token_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            assert_eq!(
                image_slash_star::encode_to_sink_with_token(
                    &jpeg_image,
                    ImageFormat::Jpeg,
                    &jpeg_options,
                    &jpeg_token,
                    &mut jpeg_token_sink,
                )?,
                jpeg_expected.len()
            );
            assert_eq!(jpeg_token_sink.bytes, jpeg_expected);
            assert_eq!(jpeg_token_sink.writes, 1);

            // The generic whole-buffer fallback must also preserve invalid
            // still-input errors before touching its sink. These are
            // Rust-owned API/error contracts, not Pillow parity rows.
            let invalid_jpeg =
                image_slash_star::DecodedImage::new(1, 1, Vec::new(), ColorType::Rgb8);
            let mut invalid_jpeg_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let invalid_jpeg_error = match image_slash_star::encode_to_sink(
                &invalid_jpeg,
                ImageFormat::Jpeg,
                &jpeg_options,
                &mut invalid_jpeg_sink,
            ) {
                Ok(length) => {
                    return Err(
                        format!("invalid JPEG fallback unexpectedly wrote {length} bytes").into(),
                    );
                }
                Err(error) => error,
            };
            assert_eq!(
                invalid_jpeg_error.kind(),
                image_slash_star::ImageErrorKind::Dimensions
            );
            assert_eq!(invalid_jpeg_sink.writes, 0);
            assert!(invalid_jpeg_sink.bytes.is_empty());

            let invalid_jpeg_token = image_slash_star::CancellationToken::new();
            let mut invalid_jpeg_token_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let invalid_jpeg_token_error = match image_slash_star::encode_to_sink_with_token(
                &invalid_jpeg,
                ImageFormat::Jpeg,
                &jpeg_options,
                &invalid_jpeg_token,
                &mut invalid_jpeg_token_sink,
            ) {
                Ok(length) => {
                    return Err(format!(
                        "invalid token JPEG fallback unexpectedly wrote {length} bytes"
                    )
                    .into());
                }
                Err(error) => error,
            };
            assert_eq!(
                invalid_jpeg_token_error.kind(),
                image_slash_star::ImageErrorKind::Dimensions
            );
            assert_eq!(invalid_jpeg_token_sink.writes, 0);
            assert!(invalid_jpeg_token_sink.bytes.is_empty());
        }
    }

    if cfg!(feature = "gif") {
        let data = fs::read(root.join("tests/fixtures/input/images/gif/animated_3frame.gif"))?;
        let sequence = image_slash_star::decode_sequence(&data)?.into_inner();
        let options = EncodeOptions::for_format(ImageFormat::Gif);
        let expected = image_slash_star::encode_sequence(&sequence, ImageFormat::Gif, &options)?;
        let mut sink = Vec::new();
        assert_eq!(
            image_slash_star::encode_sequence_to_sink(
                &sequence,
                ImageFormat::Gif,
                &options,
                &mut sink
            )?,
            expected.len(),
            "sequence sink length"
        );
        assert_eq!(sink, expected, "sequence sink bytes");

        // Invalid sequence and failing sequence sink error paths.
        let empty = image_slash_star::DecodedSequence {
            width: 1,
            height: 1,
            frames: Vec::new(),
            loop_count: None,
            background: None,
            kind: image_slash_star::SequenceKind::SingleFrame,
            opaque_blocks: Vec::new(),
            metadata: Vec::new(),
            source_color: image_slash_star::SourceColor::new(),
        };
        let mut unused_sink = Vec::new();
        assert!(
            image_slash_star::encode_sequence_to_sink(
                &empty,
                ImageFormat::Gif,
                &options,
                &mut unused_sink
            )
            .is_err(),
            "invalid sequence must fail before the sink"
        );
        let mut failing = FailingSink;
        let error = match image_slash_star::encode_sequence_to_sink(
            &sequence,
            ImageFormat::Gif,
            &options,
            &mut failing,
        ) {
            Ok(length) => {
                return Err(
                    format!("failing sequence sink unexpectedly wrote {length} bytes").into(),
                );
            }
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::OutputWrite);
        assert_eq!(error.format(), Some(ImageFormat::Gif));
        assert_eq!(error.stage(), Some(ImageErrorStage::SequenceEncode));
        assert_eq!(error.offset(), None);
        assert_eq!(error.identity(), None);
        assert_eq!(
            error.message(),
            Some("unsupported: sink rejected the write")
        );
    }

    // Every remaining enabled whole-buffer codec uses the same generic
    // destination boundary. This is a Rust-only OutputWrite contract:
    // Pillow has no caller-owned sink, so these cases must not become parity
    // rows or alter the oracle-based coverage count.
    let sink_image = image_slash_star::DecodedImage::new(1, 1, vec![0, 0, 0], ColorType::Rgb8);
    for (format, enabled) in [
        (ImageFormat::Jpeg, cfg!(feature = "jpeg")),
        (ImageFormat::Gif, cfg!(feature = "gif")),
        (ImageFormat::Tiff, cfg!(feature = "tiff")),
        (ImageFormat::WebP, cfg!(feature = "webp")),
        (ImageFormat::Ico, cfg!(feature = "ico")),
        (ImageFormat::Avif, cfg!(feature = "avif")),
    ] {
        if !enabled || (format == ImageFormat::Avif && cfg!(target_arch = "wasm32")) {
            continue;
        }
        let options = EncodeOptions::for_format(format);
        let mut failing = FailingSink;
        let error =
            match image_slash_star::encode_to_sink(&sink_image, format, &options, &mut failing) {
                Ok(length) => {
                    return Err(format!(
                        "failing {format} whole-buffer sink unexpectedly wrote {length} bytes"
                    )
                    .into());
                }
                Err(error) => error,
            };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::OutputWrite);
        assert_eq!(error.format(), Some(format));
        assert_eq!(error.stage(), Some(ImageErrorStage::StillEncode));
        assert_eq!(error.unsupported_reason(), None);
        assert_eq!(error.offset(), None);
        assert_eq!(error.identity(), None);
    }
    Ok(())
}

#[test]
fn encoded_output_policy_is_a_non_parity_result_contract() -> Result<(), Box<dyn std::error::Error>>
{
    // Pillow has no caller-controlled maximum-output policy. These assertions
    // classify the Rust result/sink boundary and must not become parity rows.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    if cfg!(feature = "png") {
        let data = fs::read(root.join("tests/fixtures/input/images/png/1x1.png"))?;
        let decoded = image_slash_star::decode(&data)?;
        let options = EncodeOptions::for_format(ImageFormat::Png);
        let expected = image_slash_star::encode(&decoded.content, ImageFormat::Png, &options)?;
        let exact = u64::try_from(expected.len())?;
        let admitted = image_slash_star::EncodePolicy::new().with_max_output_bytes(exact);
        assert_eq!(admitted.max_output_bytes(), Some(exact));
        assert_eq!(
            image_slash_star::encode_with_policy(
                &decoded.content,
                ImageFormat::Png,
                &options,
                &admitted,
            )?,
            expected,
            "exact output limit admits the complete result"
        );

        let below = image_slash_star::EncodePolicy::new().with_max_output_bytes(exact - 1);
        let error = match image_slash_star::encode_with_policy(
            &decoded.content,
            ImageFormat::Png,
            &options,
            &below,
        ) {
            Ok(_) => return Err("output policy unexpectedly admitted an oversized result".into()),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Png),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
                maximum,
                observed,
            } if maximum == exact - 1 && observed == exact
        ));

        let mut sink = vec![0xAA];
        assert!(matches!(
            image_slash_star::encode_to_sink_with_policy(
                &decoded.content,
                ImageFormat::Png,
                &options,
                &below,
                &mut sink,
            ),
            Err(ImageError::LimitExceeded {
                resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
                ..
            })
        ));
        assert_eq!(sink, vec![0xAA], "policy failure must precede sink writes");

        let token = image_slash_star::CancellationToken::new();
        assert!(matches!(
            image_slash_star::encode_with_token_and_policy(
                &decoded.content,
                ImageFormat::Png,
                &options,
                &below,
                &token,
            ),
            Err(ImageError::LimitExceeded {
                resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
                ..
            })
        ));
    }

    if cfg!(feature = "gif") {
        let data = fs::read(root.join("tests/fixtures/input/images/gif/animated_3frame.gif"))?;
        let sequence = image_slash_star::decode_sequence(&data)?.into_inner();
        let options = EncodeOptions::for_format(ImageFormat::Gif);
        let expected = image_slash_star::encode_sequence(&sequence, ImageFormat::Gif, &options)?;
        let exact = u64::try_from(expected.len())?;
        let below = image_slash_star::EncodePolicy::new().with_max_output_bytes(exact - 1);
        let error = match image_slash_star::encode_sequence_with_policy(
            &sequence,
            ImageFormat::Gif,
            &options,
            &below,
        ) {
            Ok(_) => {
                return Err(
                    "sequence output policy unexpectedly admitted an oversized result".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::SequenceEncode,
                resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
                maximum,
                observed,
            } if maximum == exact - 1 && observed == exact
        ));

        let mut sink = vec![0xBB];
        assert!(matches!(
            image_slash_star::encode_sequence_to_sink_with_policy(
                &sequence,
                ImageFormat::Gif,
                &options,
                &below,
                &mut sink,
            ),
            Err(ImageError::LimitExceeded {
                resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
                ..
            })
        ));
        assert_eq!(
            sink,
            vec![0xBB],
            "sequence policy failure must precede sink writes"
        );

        let token = image_slash_star::CancellationToken::new();
        assert!(matches!(
            image_slash_star::encode_sequence_with_token_and_policy(
                &sequence,
                ImageFormat::Gif,
                &options,
                &below,
                &token,
            ),
            Err(ImageError::LimitExceeded {
                resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
                ..
            })
        ));
    }
    Ok(())
}

#[test]
fn unsupported_reasons_are_non_parity_capability_contracts()
-> Result<(), Box<dyn std::error::Error>> {
    // These reason fields classify the Rust API's capability boundary. Pillow
    // exposes neither this field nor a portable equivalent, so this test is
    // intentionally outside the generated parity matrix.
    let input_class = ImageError::Unsupported {
        format: Some(ImageFormat::Png),
        message: "retained sequence metadata".to_owned(),
        stage: Some(ImageErrorStage::SequenceEncode),
        reason: None,
        offset: None,
        identity: None,
    };
    assert_eq!(input_class.unsupported_reason(), None);

    for reason in [
        UnsupportedReason::TargetUnavailable,
        UnsupportedReason::NotImplemented,
    ] {
        let error = ImageError::Unsupported {
            format: Some(ImageFormat::Avif),
            message: "capability boundary".to_owned(),
            stage: Some(ImageErrorStage::StillEncode),
            reason: Some(reason),
            offset: None,
            identity: None,
        };
        assert_eq!(error.unsupported_reason(), Some(reason));
    }

    if cfg!(feature = "jpeg") {
        let image = DecodedImage::new(1, 1, vec![0, 0, 0], ColorType::Rgb8);
        let mut sequence = DecodedSequence::from_image(image);
        sequence.frames.push(sequence.frames[0].clone());
        sequence.kind = SequenceKind::TimedAnimation;
        let error = match image_slash_star::encode_sequence(
            &sequence,
            ImageFormat::Jpeg,
            &EncodeOptions::for_format(ImageFormat::Jpeg),
        ) {
            Ok(_) => return Err("JPEG unexpectedly encoded multiple frames".into()),
            Err(error) => error,
        };
        assert_eq!(
            error.unsupported_reason(),
            Some(UnsupportedReason::NotImplemented)
        );
    }
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

    if cfg!(feature = "bmp") {
        // BMP parse-site context is a Rust error-detail contract. Pillow has
        // no portable offset/identity result to compare, so this witness is
        // intentionally outside the generated parity matrix.
        let truncated_bmp = b"BM";
        let inspect_error = match image_slash_star::inspect_basic(truncated_bmp) {
            Err(error) => error,
            Ok(info) => panic!("truncated BMP must fail basic inspection: {info:?}"),
        };
        assert_eq!(inspect_error.kind(), ImageErrorKind::Malformed);
        assert_eq!(inspect_error.stage(), Some(ImageErrorStage::Inspection));
        assert_eq!(inspect_error.identity(), Some("bmp_field"));
        assert_eq!(inspect_error.offset(), Some(10));

        let mut unsupported_header = vec![0u8; 18];
        unsupported_header[..2].copy_from_slice(b"BM");
        unsupported_header[14..18].copy_from_slice(&20u32.to_le_bytes());
        let header_error = match image_slash_star::inspect_basic(&unsupported_header) {
            Err(error) => error,
            Ok(info) => panic!("unsupported BMP DIB header must fail: {info:?}"),
        };
        assert_eq!(header_error.kind(), ImageErrorKind::Unsupported);
        assert_eq!(header_error.stage(), Some(ImageErrorStage::Inspection));
        assert_eq!(header_error.identity(), Some("bmp_dib_header"));
        assert_eq!(header_error.offset(), Some(14));

        let mut unsupported_info_depth = vec![0u8; 50];
        unsupported_info_depth[..2].copy_from_slice(b"BM");
        unsupported_info_depth[14..18].copy_from_slice(&40u32.to_le_bytes());
        unsupported_info_depth[18..22].copy_from_slice(&1i32.to_le_bytes());
        unsupported_info_depth[22..26].copy_from_slice(&1i32.to_le_bytes());
        unsupported_info_depth[28..30].copy_from_slice(&3u16.to_le_bytes());
        let info_depth_error = match image_slash_star::inspect_basic(&unsupported_info_depth) {
            Err(error) => error,
            Ok(info) => panic!("unsupported BMP INFO depth must fail: {info:?}"),
        };
        assert_eq!(info_depth_error.kind(), ImageErrorKind::Unsupported);
        assert_eq!(info_depth_error.identity(), Some("bmp_dib_header"));
        assert_eq!(info_depth_error.offset(), Some(28));

        let mut unsupported_core_depth = vec![0u8; 26];
        unsupported_core_depth[..2].copy_from_slice(b"BM");
        unsupported_core_depth[14..18].copy_from_slice(&12u32.to_le_bytes());
        unsupported_core_depth[18..20].copy_from_slice(&1u16.to_le_bytes());
        unsupported_core_depth[20..22].copy_from_slice(&1u16.to_le_bytes());
        unsupported_core_depth[24..26].copy_from_slice(&3u16.to_le_bytes());
        let core_depth_error = match image_slash_star::inspect_basic(&unsupported_core_depth) {
            Err(error) => error,
            Ok(info) => panic!("unsupported BMP core depth must fail: {info:?}"),
        };
        assert_eq!(core_depth_error.kind(), ImageErrorKind::Unsupported);
        assert_eq!(core_depth_error.identity(), Some("bmp_dib_header"));
        assert_eq!(core_depth_error.offset(), Some(24));

        let decode_error = match image_slash_star::decode(truncated_bmp) {
            Err(error) => error,
            Ok(info) => panic!("truncated BMP must fail still decode: {info:?}"),
        };
        assert_eq!(decode_error.kind(), ImageErrorKind::Malformed);
        assert_eq!(decode_error.stage(), Some(ImageErrorStage::StillDecode));
        assert_eq!(decode_error.identity(), Some("bmp_field"));
        assert_eq!(decode_error.offset(), Some(2));

        let prefix_error = match image_slash_star::decode_prefix(truncated_bmp) {
            Err(error) => error,
            Ok(info) => panic!("truncated BMP prefix must need more data: {info:?}"),
        };
        assert_eq!(prefix_error.kind(), ImageErrorKind::NeedMoreData);
        assert_eq!(prefix_error.stage(), Some(ImageErrorStage::StillDecode));
        assert_eq!(prefix_error.identity(), Some("bmp_field"));
        assert_eq!(prefix_error.offset(), Some(2));
        assert_eq!(prefix_error.minimum_input(), Some(6));
    }

    if cfg!(feature = "ico") {
        // ICO parse-site context is a Rust error-detail contract. Pillow's
        // decode result does not expose stable byte offsets or container
        // identities, so these witnesses intentionally stay outside the
        // generated parity matrix.
        let truncated_ico = b"\0\0\x01\0";
        let inspect_error = match image_slash_star::inspect_basic(truncated_ico) {
            Err(error) => error,
            Ok(info) => panic!("truncated ICO header must fail inspection: {info:?}"),
        };
        assert_eq!(inspect_error.kind(), ImageErrorKind::Malformed);
        assert_eq!(inspect_error.stage(), Some(ImageErrorStage::Inspection));
        assert_eq!(inspect_error.identity(), Some("ico_header"));
        assert_eq!(inspect_error.offset(), Some(0));

        let inspect_prefix_error = match image_slash_star::inspect_basic_prefix(truncated_ico) {
            Err(error) => error,
            Ok(info) => panic!("truncated ICO header must need more data: {info:?}"),
        };
        assert_eq!(inspect_prefix_error.kind(), ImageErrorKind::NeedMoreData);
        assert_eq!(
            inspect_prefix_error.stage(),
            Some(ImageErrorStage::Inspection)
        );
        assert_eq!(inspect_prefix_error.identity(), Some("ico_header"));
        assert_eq!(inspect_prefix_error.offset(), Some(0));
        assert_eq!(inspect_prefix_error.minimum_input(), Some(6));

        let decode_error = match image_slash_star::decode(truncated_ico) {
            Err(error) => error,
            Ok(info) => panic!("truncated ICO header must fail still decode: {info:?}"),
        };
        assert_eq!(decode_error.kind(), ImageErrorKind::Malformed);
        assert_eq!(decode_error.stage(), Some(ImageErrorStage::StillDecode));
        assert_eq!(decode_error.identity(), Some("ico_header"));
        assert_eq!(decode_error.offset(), Some(0));

        let decode_prefix_error = match image_slash_star::decode_prefix(truncated_ico) {
            Err(error) => error,
            Ok(info) => panic!("truncated ICO header must need more data: {info:?}"),
        };
        assert_eq!(decode_prefix_error.kind(), ImageErrorKind::NeedMoreData);
        assert_eq!(
            decode_prefix_error.stage(),
            Some(ImageErrorStage::StillDecode)
        );
        assert_eq!(decode_prefix_error.identity(), Some("ico_header"));
        assert_eq!(decode_prefix_error.offset(), Some(0));
        assert_eq!(decode_prefix_error.minimum_input(), Some(6));

        let truncated_directory = b"\0\0\x01\0\x01\0";
        let directory_error = match image_slash_star::inspect_basic(truncated_directory) {
            Err(error) => error,
            Ok(info) => panic!("truncated ICO directory must fail inspection: {info:?}"),
        };
        assert_eq!(directory_error.kind(), ImageErrorKind::Malformed);
        assert_eq!(directory_error.identity(), Some("ico_directory"));
        assert_eq!(directory_error.offset(), Some(6));

        let directory_prefix_error =
            match image_slash_star::inspect_basic_prefix(truncated_directory) {
                Err(error) => error,
                Ok(info) => panic!("truncated ICO directory must need more data: {info:?}"),
            };
        assert_eq!(directory_prefix_error.kind(), ImageErrorKind::NeedMoreData);
        assert_eq!(directory_prefix_error.identity(), Some("ico_directory"));
        assert_eq!(directory_prefix_error.offset(), Some(6));
        assert_eq!(directory_prefix_error.minimum_input(), Some(22));

        let mut missing_payload = truncated_directory.to_vec();
        missing_payload.extend_from_slice(&[
            0, 0, 0, 0, // width, height, palette count, reserved
            0, 0, // planes
            0, 0, // bit depth
            1, 0, 0, 0, // payload length
            22, 0, 0, 0, // payload offset
        ]);
        let payload_error = match image_slash_star::inspect_basic(&missing_payload) {
            Err(error) => error,
            Ok(info) => panic!("missing ICO payload must fail inspection: {info:?}"),
        };
        assert_eq!(payload_error.kind(), ImageErrorKind::Malformed);
        assert_eq!(payload_error.identity(), Some("ico_entry"));
        assert_eq!(payload_error.offset(), Some(22));

        let mut unsupported_dib = truncated_directory.to_vec();
        unsupported_dib.extend_from_slice(&[
            0, 0, 0, 0, // width, height, palette count, reserved
            0, 0, // planes
            3, 0, // bit depth
            40, 0, 0, 0, // payload length
            22, 0, 0, 0, // payload offset
        ]);
        let mut dib = vec![0u8; 40];
        dib[..4].copy_from_slice(&40u32.to_le_bytes());
        dib[4..8].copy_from_slice(&1u32.to_le_bytes());
        dib[8..12].copy_from_slice(&2u32.to_le_bytes());
        dib[14..16].copy_from_slice(&3u16.to_le_bytes());
        unsupported_dib.extend_from_slice(&dib);
        let dib_inspect_error = match image_slash_star::inspect_basic(&unsupported_dib) {
            Err(error) => error,
            Ok(info) => panic!("unsupported ICO DIB depth must fail inspection: {info:?}"),
        };
        assert_eq!(dib_inspect_error.kind(), ImageErrorKind::Unsupported);
        assert_eq!(dib_inspect_error.identity(), Some("ico_dib"));
        assert_eq!(dib_inspect_error.offset(), Some(22));

        let dib_decode_error = match image_slash_star::decode(&unsupported_dib) {
            Err(error) => error,
            Ok(info) => panic!("unsupported ICO DIB depth must fail decode: {info:?}"),
        };
        assert_eq!(dib_decode_error.kind(), ImageErrorKind::Unsupported);
        assert_eq!(dib_decode_error.stage(), Some(ImageErrorStage::StillDecode));
        assert_eq!(dib_decode_error.identity(), Some("ico_dib"));
        assert_eq!(dib_decode_error.offset(), Some(22));
    }

    if cfg!(feature = "webp") {
        // WebP inspection parse-site context is a Rust defensive contract.
        // Pillow parity does not expose byte offsets or stable container
        // identities, so these assertions are not generated parity rows.
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let empty_payload =
            fs::read(root.join("tests/fixtures/input/images/webp/vp8_empty_payload.webp"))?;
        let chunk_error = match image_slash_star::inspect_basic(&empty_payload) {
            Err(error) => error,
            Ok(info) => panic!("empty WebP VP8 payload must fail inspection: {info:?}"),
        };
        assert_eq!(chunk_error.kind(), ImageErrorKind::Malformed);
        assert_eq!(chunk_error.stage(), Some(ImageErrorStage::Inspection));
        assert_eq!(chunk_error.identity(), Some("webp_chunk"));
        assert_eq!(chunk_error.offset(), Some(12));

        let extended = fs::read(
            root.join("tests/fixtures/input/images/webp/extended_missing_image_chunk.webp"),
        )?;
        let prefix_error = match image_slash_star::inspect_basic_prefix(&extended) {
            Err(error) => error,
            Ok(info) => {
                panic!("extended WebP without an image chunk must need more data: {info:?}")
            }
        };
        assert_eq!(prefix_error.kind(), ImageErrorKind::NeedMoreData);
        assert_eq!(prefix_error.stage(), Some(ImageErrorStage::Inspection));
        assert_eq!(prefix_error.identity(), Some("webp_chunk"));
        assert!(prefix_error.offset().is_some());
        assert!(prefix_error.minimum_input().is_some());
    }

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

#[test]
fn incremental_detection_reports_exact_minimums_and_terminal_results()
-> Result<(), Box<dyn std::error::Error>> {
    // The committed defensive-model manifest pins every detection edge case:
    // exact minimums, terminal unknowns, and legacy complete-slice parity.
    use image_slash_star::{ImageError, ImageErrorKind, ImageErrorStage};

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: IncrementalInputManifest = json::from_str(&fs::read_to_string(
        root.join("tests/fixtures/incremental_input_manifest.json"),
    )?)?;
    assert_eq!(manifest.format_version, 1);
    assert_eq!(manifest.assertion_origin, "defensive_model");
    let mut ids = HashSet::new();
    for case in &manifest.detection_cases {
        assert!(
            ids.insert(case.id.clone()),
            "duplicate detection case {}",
            case.id
        );
        let input = hex_bytes(&case.input_hex);
        let actual = image_slash_star::detect_prefix(&input);
        match case.expect.as_str() {
            "identified" => {
                let (expected_format, _, _) = format(
                    case.format
                        .as_deref()
                        .unwrap_or_else(|| panic!("identified case {} needs a format", case.id)),
                );
                assert_eq!(actual, Ok(expected_format), "detection case {}", case.id);
                assert_eq!(
                    image_slash_star::detect_format(&input),
                    Ok(expected_format),
                    "legacy detection case {}",
                    case.id
                );
            }
            "need_more" => {
                let error = match actual {
                    Ok(found) => panic!(
                        "detection case {} must need more data, got {found:?}",
                        case.id
                    ),
                    Err(error) => error,
                };
                assert_eq!(
                    error.kind(),
                    ImageErrorKind::NeedMoreData,
                    "case {}",
                    case.id
                );
                assert_eq!(error.minimum_input(), case.minimum, "case {}", case.id);
                assert_eq!(error.format(), None, "case {}", case.id);
                assert_eq!(error.stage(), None, "case {}", case.id);
                assert_eq!(
                    image_slash_star::detect_format(&input),
                    Err(ImageError::UnknownFormat),
                    "legacy detection must stay terminal for {}",
                    case.id
                );
            }
            "unknown" => {
                assert_eq!(
                    actual,
                    Err(ImageError::UnknownFormat),
                    "detection case {}",
                    case.id
                );
                if case.legacy_parity.unwrap_or(true) {
                    assert_eq!(
                        image_slash_star::detect_format(&input),
                        Err(ImageError::UnknownFormat),
                        "legacy detection case {}",
                        case.id
                    );
                }
            }
            expect => panic!("unknown manifest expectation {expect:?} for {}", case.id),
        }
    }

    for fixture in &manifest.inspection_fixtures {
        let (expected_format, _, enabled) = format(&fixture.format);
        if !enabled {
            continue;
        }
        let bytes = fs::read(root.join(&fixture.asset_path))?;
        let full = image_slash_star::inspect_basic_prefix(&bytes)
            .unwrap_or_else(|error| panic!("fixture {} must inspect fully: {error}", fixture.id));
        assert_eq!(
            full,
            image_slash_star::inspect_basic(&bytes)?,
            "fixture {} full result",
            fixture.id
        );

        let signature_prefix = usize::try_from(fixture.signature_prefix)?;
        let error = match image_slash_star::inspect_basic_prefix(&bytes[..signature_prefix]) {
            Ok(info) => panic!(
                "fixture {} signature prefix must need more data: {info:?}",
                fixture.id
            ),
            Err(error) => error,
        };
        assert_eq!(
            error.kind(),
            ImageErrorKind::NeedMoreData,
            "fixture {}",
            fixture.id
        );
        assert_eq!(
            error.minimum_input(),
            Some(fixture.signature_minimum),
            "fixture {}",
            fixture.id
        );
        assert_eq!(error.format(), None, "fixture {}", fixture.id);
        assert_eq!(error.stage(), None, "fixture {}", fixture.id);

        let need_more_prefix = usize::try_from(fixture.need_more_prefix)?;
        let error = match image_slash_star::inspect_basic_prefix(&bytes[..need_more_prefix]) {
            Ok(info) => panic!(
                "fixture {} need-more prefix must need more data: {info:?}",
                fixture.id
            ),
            Err(error) => error,
        };
        assert_eq!(
            error.kind(),
            ImageErrorKind::NeedMoreData,
            "fixture {}",
            fixture.id
        );
        assert_eq!(
            error.minimum_input(),
            Some(fixture.need_more_minimum),
            "fixture {}",
            fixture.id
        );
        assert_eq!(
            error.format(),
            Some(expected_format),
            "fixture {}",
            fixture.id
        );
        assert_eq!(
            error.stage(),
            Some(ImageErrorStage::Inspection),
            "fixture {}",
            fixture.id
        );
        let legacy = match image_slash_star::inspect_basic(&bytes[..need_more_prefix]) {
            Ok(info) => panic!(
                "legacy must reject the need-more prefix for {}: {info:?}",
                fixture.id
            ),
            Err(error) => error,
        };
        assert_eq!(
            legacy.kind(),
            ImageErrorKind::Malformed,
            "fixture {}",
            fixture.id
        );

        if let Some(basic_prefix) = fixture.basic_prefix {
            let info =
                image_slash_star::inspect_basic_prefix(&bytes[..usize::try_from(basic_prefix)?])?;
            assert_eq!(
                info.frame_count_complete,
                fixture.basic_frame_count_complete.unwrap_or(true),
                "fixture {} basic completeness",
                fixture.id
            );
        }

        if let (Some(decode_prefix), Some(decode_minimum)) = (
            fixture.decode_need_more_prefix,
            fixture.decode_need_more_minimum,
        ) {
            if image_slash_star::decode(&bytes).is_err() {
                continue;
            }
            let error =
                match image_slash_star::decode_prefix(&bytes[..usize::try_from(decode_prefix)?]) {
                    Ok(info) => panic!(
                        "fixture {} decode need-more prefix must need more data: {info:?}",
                        fixture.id
                    ),
                    Err(error) => error,
                };
            assert_eq!(
                error.kind(),
                ImageErrorKind::NeedMoreData,
                "fixture {}",
                fixture.id
            );
            assert_eq!(
                error.minimum_input(),
                Some(decode_minimum),
                "fixture {}",
                fixture.id
            );
            assert_eq!(
                error.format(),
                Some(expected_format),
                "fixture {}",
                fixture.id
            );
            assert_eq!(
                error.stage(),
                Some(ImageErrorStage::StillDecode),
                "fixture {}",
                fixture.id
            );
            let legacy = match image_slash_star::decode(&bytes[..usize::try_from(decode_prefix)?]) {
                Ok(info) => panic!(
                    "legacy must reject the decode need-more prefix for {}: {info:?}",
                    fixture.id
                ),
                Err(error) => error,
            };
            assert_eq!(
                legacy.kind(),
                ImageErrorKind::Malformed,
                "fixture {}",
                fixture.id
            );
        }
    }
    Ok(())
}

#[test]
fn incremental_detection_retries_to_completion_on_real_input()
-> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::ImageError;

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cases: &[(&str, ImageFormat)] = &[
        ("tests/fixtures/input/images/png/1x1.png", ImageFormat::Png),
        (
            "tests/fixtures/input/images/jpeg/1x1.jpg",
            ImageFormat::Jpeg,
        ),
        ("tests/fixtures/input/images/gif/1x1.gif", ImageFormat::Gif),
        (
            "tests/fixtures/input/images/webp/16x16.webp",
            ImageFormat::WebP,
        ),
        (
            "tests/fixtures/input/images/avif/baseline.avif",
            ImageFormat::Avif,
        ),
    ];
    for (path, expected) in cases {
        let bytes = fs::read(root.join(path))?;
        let mut prefix: Vec<u8> = Vec::new();
        let mut attempts = 0;
        loop {
            attempts += 1;
            assert!(
                attempts <= bytes.len() + 1,
                "detection must complete after at most the full input for {path}"
            );
            match image_slash_star::detect_prefix(&prefix) {
                Ok(format) => {
                    assert_eq!(&format, expected);
                    break;
                }
                Err(ImageError::NeedMoreData { minimum, .. }) => {
                    let minimum = usize::try_from(minimum).unwrap_or(usize::MAX);
                    assert!(
                        minimum > prefix.len(),
                        "minimum must exceed the current prefix for {path}"
                    );
                    let next_len = minimum.min(bytes.len());
                    prefix.clear();
                    prefix.extend_from_slice(&bytes[..next_len]);
                }
                Err(error) => {
                    panic!("valid {path} must never terminate detection: {error:?}");
                }
            }
        }
    }
    Ok(())
}

#[test]
fn incremental_basic_inspection_tracks_truncation_progress_per_format()
-> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::ImageError;

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cases: &[(ImageFormat, bool, &str)] = &[
        (
            ImageFormat::Png,
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/1x1.png",
        ),
        (
            ImageFormat::Gif,
            cfg!(feature = "gif"),
            "tests/fixtures/input/images/gif/1x1.gif",
        ),
        (
            ImageFormat::Bmp,
            cfg!(feature = "bmp"),
            "tests/fixtures/input/images/bmp/1x1.bmp",
        ),
        (
            ImageFormat::Tiff,
            cfg!(feature = "tiff"),
            "tests/fixtures/input/images/tiff/8bit.tiff",
        ),
        (
            ImageFormat::Jpeg,
            cfg!(feature = "jpeg"),
            "tests/fixtures/input/images/jpeg/1x1.jpg",
        ),
        (
            ImageFormat::WebP,
            cfg!(feature = "webp"),
            "tests/fixtures/input/images/webp/16x16.webp",
        ),
        (
            ImageFormat::Ico,
            cfg!(feature = "ico"),
            "tests/fixtures/input/images/ico/16x16.ico",
        ),
        (
            ImageFormat::Avif,
            cfg!(feature = "avif"),
            "tests/fixtures/input/images/avif/baseline.avif",
        ),
    ];
    for (format, enabled, path) in cases {
        if !enabled {
            continue;
        }
        let bytes = fs::read(root.join(path))?;
        let full = image_slash_star::inspect_basic_prefix(&bytes)?;
        assert_eq!(full, image_slash_star::inspect_basic(&bytes)?);
        assert_eq!(full.format, *format);

        let mut saw_need_more = false;
        let mut saw_ok = false;
        for end in 0..=bytes.len() {
            let prefix = &bytes[..end];
            match image_slash_star::inspect_basic_prefix(prefix) {
                Ok(info) => {
                    saw_ok = true;
                    let legacy = image_slash_star::inspect_basic(prefix).unwrap_or_else(|error| {
                        panic!("legacy inspect_basic must agree on {path} prefix {end}: {error}")
                    });
                    assert_eq!(info, legacy, "{path} prefix {end}");
                }
                Err(ImageError::NeedMoreData {
                    format, minimum, ..
                }) => {
                    saw_need_more = true;
                    let minimum = usize::try_from(minimum).unwrap_or(usize::MAX);
                    let legacy = match image_slash_star::inspect_basic(prefix) {
                        Ok(info) => panic!(
                            "legacy must reject truncation for {path} prefix {end}: {info:?}"
                        ),
                        Err(error) => error,
                    };
                    assert_eq!(
                        legacy.kind(),
                        if format.is_some() {
                            image_slash_star::ImageErrorKind::Malformed
                        } else {
                            image_slash_star::ImageErrorKind::UnknownFormat
                        },
                        "legacy classification must match for {path} prefix {end}"
                    );
                    assert!(
                        minimum > prefix.len(),
                        "minimum must exceed the prefix for {path} at {end}"
                    );
                    assert!(
                        minimum <= bytes.len(),
                        "minimum for a valid fixture must not exceed the file for {path} at {end}"
                    );
                    if minimum > prefix.len() {
                        let next = image_slash_star::inspect_basic_prefix(&bytes[..minimum]);
                        let advanced = match next {
                            Ok(_) => true,
                            Err(ImageError::NeedMoreData {
                                minimum: next_minimum,
                                ..
                            }) => usize::try_from(next_minimum).unwrap_or(usize::MAX) > minimum,
                            Err(_) => true,
                        };
                        assert!(
                            advanced,
                            "retrying with minimum must progress for {path} at {end}"
                        );
                    }
                }
                Err(other) => {
                    let legacy = match image_slash_star::inspect_basic(prefix) {
                        Ok(info) => panic!("legacy must reject {path} prefix {end}: {info:?}"),
                        Err(error) => error,
                    };
                    assert_eq!(
                        legacy.kind(),
                        other.kind(),
                        "terminal kinds must match for {path} prefix {end}"
                    );
                }
            }
        }
        assert!(
            saw_need_more,
            "{path} must exercise at least one need-more boundary"
        );
        assert!(saw_ok, "{path} must succeed on the complete input");
    }
    Ok(())
}

#[test]
fn incremental_decode_tracks_truncation_progress_per_format()
-> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::ImageError;

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cases: &[(ImageFormat, bool, &str)] = &[
        (
            ImageFormat::Png,
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/1x1.png",
        ),
        (
            ImageFormat::Gif,
            cfg!(feature = "gif"),
            "tests/fixtures/input/images/gif/1x1.gif",
        ),
        (
            ImageFormat::Bmp,
            cfg!(feature = "bmp"),
            "tests/fixtures/input/images/bmp/1x1.bmp",
        ),
        (
            ImageFormat::Tiff,
            cfg!(feature = "tiff"),
            "tests/fixtures/input/images/tiff/8bit.tiff",
        ),
        (
            ImageFormat::Jpeg,
            cfg!(feature = "jpeg"),
            "tests/fixtures/input/images/jpeg/1x1.jpg",
        ),
        (
            ImageFormat::WebP,
            cfg!(feature = "webp"),
            "tests/fixtures/input/images/webp/16x16.webp",
        ),
        (
            ImageFormat::Ico,
            cfg!(feature = "ico"),
            "tests/fixtures/input/images/ico/16x16.ico",
        ),
        (
            ImageFormat::Avif,
            cfg!(feature = "avif"),
            "tests/fixtures/input/images/avif/baseline.avif",
        ),
    ];
    for (_, enabled, path) in cases {
        if !enabled {
            continue;
        }
        let bytes = fs::read(root.join(path))?;
        // Targets without a full decode path for this format (for example
        // portable WASM AVIF) keep their Unsupported classification; the
        // incremental decode contract is exercised where decode is available.
        if image_slash_star::decode(&bytes).is_err() {
            continue;
        }
        let full = image_slash_star::decode_prefix(&bytes)?;
        let legacy_full = image_slash_star::decode(&bytes)?;
        assert_eq!(full.format, legacy_full.format, "{path} format");
        assert_eq!(
            full.content.pixels, legacy_full.content.pixels,
            "{path} pixels"
        );
        assert_eq!(
            full.consumed_bytes, legacy_full.consumed_bytes,
            "{path} consumed"
        );

        let mut saw_need_more = false;
        let mut saw_ok = false;
        for end in 0..=bytes.len() {
            let prefix = &bytes[..end];
            match image_slash_star::decode_prefix(prefix) {
                Ok(decoded) => {
                    saw_ok = true;
                    let legacy = image_slash_star::decode(prefix).unwrap_or_else(|error| {
                        panic!("legacy decode must agree on {path} prefix {end}: {error}")
                    });
                    assert_eq!(decoded.format, legacy.format, "{path} prefix {end} format");
                    assert_eq!(
                        decoded.content.pixels, legacy.content.pixels,
                        "{path} prefix {end} pixels"
                    );
                }
                Err(ImageError::NeedMoreData {
                    format, minimum, ..
                }) => {
                    saw_need_more = true;
                    let minimum = usize::try_from(minimum).unwrap_or(usize::MAX);
                    let legacy = match image_slash_star::decode(prefix) {
                        Ok(info) => panic!("legacy must reject {path} prefix {end}: {info:?}"),
                        Err(error) => error,
                    };
                    assert_eq!(
                        legacy.kind(),
                        if format.is_some() {
                            image_slash_star::ImageErrorKind::Malformed
                        } else {
                            image_slash_star::ImageErrorKind::UnknownFormat
                        },
                        "legacy classification must match for {path} prefix {end}"
                    );
                    assert!(
                        minimum > prefix.len(),
                        "minimum must exceed the prefix for {path} at {end}"
                    );
                    assert!(
                        minimum <= bytes.len(),
                        "minimum for a valid fixture must not exceed the file for {path} at {end}"
                    );
                    if minimum > prefix.len() {
                        let next = image_slash_star::decode_prefix(&bytes[..minimum]);
                        let advanced = match next {
                            Ok(_) => true,
                            Err(ImageError::NeedMoreData {
                                minimum: next_minimum,
                                ..
                            }) => usize::try_from(next_minimum).unwrap_or(usize::MAX) > minimum,
                            Err(_) => true,
                        };
                        assert!(
                            advanced,
                            "retrying with minimum must progress for {path} at {end}"
                        );
                    }
                }
                Err(other) => {
                    let legacy = match image_slash_star::decode(prefix) {
                        Ok(info) => panic!("legacy must reject {path} prefix {end}: {info:?}"),
                        Err(error) => error,
                    };
                    assert_eq!(
                        legacy.kind(),
                        other.kind(),
                        "terminal kinds must match for {path} prefix {end}"
                    );
                }
            }
        }
        assert!(
            saw_need_more,
            "{path} must exercise at least one need-more boundary"
        );
        assert!(saw_ok, "{path} must succeed on the complete input");
    }
    Ok(())
}

#[test]
fn tiff_compressed_payload_failures_retain_parse_context() -> Result<(), Box<dyn std::error::Error>>
{
    if !cfg!(feature = "tiff") {
        return Ok(());
    }
    // The fixture's StripOffsets tag names byte 122 as the malformed Deflate
    // payload. This is a Rust-owned parse-site contract, so the offset and
    // identity are not added to the Pillow parity matrix.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bytes = fs::read(root.join("tests/fixtures/input/images/tiff/deflate_bad_adler.tiff"))?;

    let still_error = match image_slash_star::decode(&bytes) {
        Ok(decoded) => panic!("malformed TIFF Deflate must fail still decode: {decoded:?}"),
        Err(error) => error,
    };
    assert_eq!(
        still_error.kind(),
        image_slash_star::ImageErrorKind::Malformed
    );
    assert_eq!(still_error.format(), Some(ImageFormat::Tiff));
    assert_eq!(still_error.stage(), Some(ImageErrorStage::StillDecode));
    assert_eq!(still_error.offset(), Some(122));
    assert_eq!(still_error.identity(), Some("tiff_strip"));

    let sequence_error = match image_slash_star::decode_sequence(&bytes) {
        Ok(sequence) => panic!("malformed TIFF Deflate must fail sequence decode: {sequence:?}"),
        Err(error) => error,
    };
    assert_eq!(
        sequence_error.kind(),
        image_slash_star::ImageErrorKind::Malformed
    );
    assert_eq!(sequence_error.format(), Some(ImageFormat::Tiff));
    assert_eq!(
        sequence_error.stage(),
        Some(ImageErrorStage::SequenceDecode)
    );
    assert_eq!(sequence_error.offset(), Some(122));
    assert_eq!(sequence_error.identity(), Some("tiff_strip"));
    Ok(())
}

#[test]
fn tiff_capability_and_destination_failures_are_structured()
-> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(feature = "tiff") {
        return Ok(());
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let unknown_compression =
        fs::read(root.join("tests/fixtures/input/images/tiff/unknown_compression.tiff"))?;
    // The unknown-compression input already has a Pillow error row. This
    // assertion adds only the Rust parse-site contract; it is not a new
    // Pillow-parity matrix row.
    let compression_error = match image_slash_star::decode(&unknown_compression) {
        Ok(decoded) => panic!("unknown TIFF compression unexpectedly decoded: {decoded:?}"),
        Err(error) => error,
    };
    assert_eq!(
        compression_error.kind(),
        image_slash_star::ImageErrorKind::Malformed
    );
    assert_eq!(compression_error.format(), Some(ImageFormat::Tiff));
    assert_eq!(
        compression_error.stage(),
        Some(ImageErrorStage::StillDecode)
    );
    assert_eq!(compression_error.offset(), Some(140));
    assert_eq!(compression_error.identity(), Some("tiff_strip"));

    // TIFF's current encoder contract rejects a valid public mode it cannot
    // represent. This is a Rust capability boundary, not a Pillow parity
    // row, because the parity matrix has no caller-owned DecodedImage mode.
    let unsupported_mode = DecodedImage::new(1, 1, vec![0; 4], ColorType::La16);
    let encode_error = match image_slash_star::encode(
        &unsupported_mode,
        ImageFormat::Tiff,
        &EncodeOptions::for_format(ImageFormat::Tiff),
    ) {
        Ok(bytes) => panic!(
            "TIFF unexpectedly encoded unsupported La16 mode ({} bytes)",
            bytes.len()
        ),
        Err(error) => error,
    };
    assert_eq!(
        encode_error.kind(),
        image_slash_star::ImageErrorKind::Unsupported
    );
    assert_eq!(encode_error.format(), Some(ImageFormat::Tiff));
    assert_eq!(encode_error.stage(), Some(ImageErrorStage::StillEncode));
    assert_eq!(encode_error.unsupported_reason(), None);

    struct RejectingTiffSink;
    impl image_slash_star::OutputSink for RejectingTiffSink {
        fn write_all(&mut self, _bytes: &[u8]) -> image_slash_star::ImageResult<()> {
            Err(ImageError::Unsupported {
                format: None,
                message: "TIFF destination rejected".to_owned(),
                stage: None,
                reason: None,
                offset: None,
                identity: None,
            })
        }
    }

    let decoded = image_slash_star::decode(&fs::read(
        root.join("tests/fixtures/input/images/tiff/8bit.tiff"),
    )?)?;
    let mut still_sink = RejectingTiffSink;
    let sink_error = match image_slash_star::encode_to_sink(
        &decoded.content,
        ImageFormat::Tiff,
        &EncodeOptions::for_format(ImageFormat::Tiff),
        &mut still_sink,
    ) {
        Ok(length) => panic!("rejecting TIFF sink unexpectedly accepted {length} bytes"),
        Err(error) => error,
    };
    assert_eq!(
        sink_error.kind(),
        image_slash_star::ImageErrorKind::OutputWrite
    );
    assert_eq!(sink_error.format(), Some(ImageFormat::Tiff));
    assert_eq!(sink_error.stage(), Some(ImageErrorStage::StillEncode));
    assert_eq!(sink_error.unsupported_reason(), None);

    let sequence = DecodedSequence::from_image(decoded.content.clone());
    let mut sequence_sink = RejectingTiffSink;
    let sequence_error = match image_slash_star::encode_sequence_to_sink(
        &sequence,
        ImageFormat::Tiff,
        &EncodeOptions::for_format(ImageFormat::Tiff),
        &mut sequence_sink,
    ) {
        Ok(length) => panic!("rejecting TIFF sequence sink unexpectedly accepted {length} bytes"),
        Err(error) => error,
    };
    assert_eq!(
        sequence_error.kind(),
        image_slash_star::ImageErrorKind::OutputWrite
    );
    assert_eq!(sequence_error.format(), Some(ImageFormat::Tiff));
    assert_eq!(
        sequence_error.stage(),
        Some(ImageErrorStage::SequenceEncode)
    );
    assert_eq!(sequence_error.unsupported_reason(), None);
    Ok(())
}

#[test]
fn incremental_decode_attaches_structured_context() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    if cfg!(feature = "png") {
        let bytes = fs::read(root.join("tests/fixtures/input/images/png/1x1.png"))?;
        // A partial signature is a detection-level status with no format yet.
        let error = match image_slash_star::decode_prefix(&bytes[..5]) {
            Ok(info) => panic!("a partial signature must need more data: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::NeedMoreData);
        assert_eq!(error.format(), None);
        assert_eq!(error.stage(), None);
        assert_eq!(error.minimum_input(), Some(8));

        // A codec-level truncation carries format and still-decode stage.
        let error = match image_slash_star::decode_prefix(&bytes[..40]) {
            Ok(info) => panic!("a truncated header must need more data: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::NeedMoreData);
        assert_eq!(error.format(), Some(ImageFormat::Png));
        assert_eq!(error.stage(), Some(ImageErrorStage::StillDecode));
        assert!(error.minimum_input().is_some());

        let error = match image_slash_star::decode_sequence_prefix(&bytes[..40]) {
            Ok(info) => panic!("a truncated header must need more data: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::NeedMoreData);
        assert_eq!(error.format(), Some(ImageFormat::Png));
        assert_eq!(error.stage(), Some(ImageErrorStage::SequenceDecode));
        assert!(error.minimum_input().is_some());
    }
    Ok(())
}

#[test]
fn incremental_decode_policy_variants_apply_limits() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    if cfg!(feature = "png") {
        let bytes = fs::read(root.join("tests/fixtures/input/images/png/1x1.png"))?;
        // Encoded-byte and metadata limits fail before codec parsing on both
        // prefix variants.
        for maximum in [10u64, 20] {
            let policy = image_slash_star::DecodePolicy::default().with_max_encoded_bytes(maximum);
            let error = match image_slash_star::decode_prefix_with_policy(&bytes, &policy) {
                Ok(info) => panic!("encoded-byte limit must reject decode_prefix: {info:?}"),
                Err(error) => error,
            };
            assert_eq!(
                error.kind(),
                image_slash_star::ImageErrorKind::LimitExceeded
            );
            let error = match image_slash_star::decode_sequence_prefix_with_policy(&bytes, &policy)
            {
                Ok(info) => {
                    panic!("encoded-byte limit must reject decode_sequence_prefix: {info:?}")
                }
                Err(error) => error,
            };
            assert_eq!(
                error.kind(),
                image_slash_star::ImageErrorKind::LimitExceeded
            );
        }
        let policy = image_slash_star::DecodePolicy::default().with_max_metadata_bytes(10);
        let error = match image_slash_star::decode_prefix_with_policy(&bytes, &policy) {
            Ok(info) => panic!("metadata limit must reject decode_prefix: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(
            error.kind(),
            image_slash_star::ImageErrorKind::LimitExceeded
        );
        let error = match image_slash_star::decode_sequence_prefix_with_policy(&bytes, &policy) {
            Ok(info) => panic!("metadata limit must reject decode_sequence_prefix: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(
            error.kind(),
            image_slash_star::ImageErrorKind::LimitExceeded
        );

        // A width limit forces inspection preflight inside the prefix path.
        let policy = image_slash_star::DecodePolicy::default().with_max_width(1);
        let decoded = image_slash_star::decode_prefix_with_policy(&bytes, &policy)?;
        assert_eq!(decoded.content.width, 1);
        let sequence = image_slash_star::decode_sequence_prefix_with_policy(&bytes, &policy)?;
        assert_eq!(sequence.content.frames.len(), 1);

        // Detection-level truncation propagates through the policy variant.
        let error = match image_slash_star::decode_sequence_prefix_with_policy(
            &bytes[..5],
            &image_slash_star::DecodePolicy::default(),
        ) {
            Ok(info) => panic!("a partial signature must need more data: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::NeedMoreData);
        assert_eq!(error.format(), None);

        // Inspection-level truncation propagates when the policy requires
        // image information before still or sequence materialization.
        let error = match image_slash_star::decode_prefix_with_policy(&bytes[..40], &policy) {
            Ok(info) => panic!("a truncated header must need more data: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::NeedMoreData);
        let error =
            match image_slash_star::decode_sequence_prefix_with_policy(&bytes[..40], &policy) {
                Ok(info) => panic!("a truncated header must need more data: {info:?}"),
                Err(error) => error,
            };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::NeedMoreData);

        // A limit the inspected image violates fails the preflight check.
        let rejecting = image_slash_star::DecodePolicy::default().with_max_width(0);
        let error = match image_slash_star::decode_prefix_with_policy(&bytes, &rejecting) {
            Ok(info) => panic!("decode_prefix must reject an exceeding width: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(
            error.kind(),
            image_slash_star::ImageErrorKind::LimitExceeded
        );
        let error = match image_slash_star::decode_sequence_prefix_with_policy(&bytes, &rejecting) {
            Ok(info) => panic!("decode_sequence_prefix must reject an exceeding width: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(
            error.kind(),
            image_slash_star::ImageErrorKind::LimitExceeded
        );

        // The cumulative sequence-byte budget fails inside the budget charge
        // after the image-info preflight passes.
        let budgeted = image_slash_star::DecodePolicy::default().with_max_sequence_decoded_bytes(1);
        let error = match image_slash_star::decode_sequence_prefix_with_policy(&bytes, &budgeted) {
            Ok(info) => panic!("the sequence budget must reject the primary frame: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(
            error.kind(),
            image_slash_star::ImageErrorKind::LimitExceeded
        );

        // Complete-input sequence decode exercises the success mapping of the
        // sequence prefix API and agrees with the legacy sequence result.
        let prefix_sequence = image_slash_star::decode_sequence_prefix(&bytes)?;
        let legacy_sequence = image_slash_star::decode_sequence(&bytes)?;
        assert_eq!(
            prefix_sequence.content.frames.len(),
            legacy_sequence.content.frames.len()
        );
        assert_eq!(
            prefix_sequence.content.frames[0].image.pixels,
            legacy_sequence.content.frames[0].image.pixels
        );
    }

    // Still formats use the sequence fallback, which must expose the same
    // non-terminal status as still decode.
    if cfg!(feature = "jpeg") {
        let bytes = fs::read(root.join("tests/fixtures/input/images/jpeg/1x1.jpg"))?;
        let sequence = image_slash_star::decode_sequence_prefix(&bytes)?;
        assert_eq!(sequence.content.frames.len(), 1);
        let error = match image_slash_star::decode_sequence_prefix(&bytes[..20]) {
            Ok(info) => panic!("a truncated JPEG must need more data: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::NeedMoreData);
        assert_eq!(error.format(), Some(ImageFormat::Jpeg));
        // The still-format sequence fallback keeps the still-decode stage,
        // exactly like the legacy sequence fallback.
        assert_eq!(error.stage(), Some(ImageErrorStage::StillDecode));
    }
    Ok(())
}

#[test]
fn incremental_inspection_attaches_structured_context() -> Result<(), Box<dyn std::error::Error>> {
    use image_slash_star::ImageError;

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    if cfg!(feature = "png") {
        let bytes = fs::read(root.join("tests/fixtures/input/images/png/1x1.png"))?;
        // A partial signature is a detection-level status with no format yet.
        let error = match image_slash_star::inspect_basic_prefix(&bytes[..5]) {
            Ok(info) => panic!("a partial signature must need more data: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::NeedMoreData);
        assert_eq!(error.format(), None);
        assert_eq!(error.stage(), None);
        assert_eq!(error.minimum_input(), Some(8));

        // A terminal malformed header carries format and stage context and
        // never advertises a retry minimum.
        let mut bad_filter = bytes.clone();
        bad_filter[27] = 1;
        let error = match image_slash_star::inspect_basic_prefix(&bad_filter) {
            Ok(info) => panic!("bad filter method must fail: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Malformed);
        assert_eq!(error.format(), Some(ImageFormat::Png));
        assert_eq!(error.stage(), Some(ImageErrorStage::Inspection));
        assert_eq!(error.minimum_input(), None);
        let legacy = match image_slash_star::inspect_basic(&bad_filter) {
            Ok(info) => panic!("legacy must fail on the bad filter method: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(legacy.kind(), error.kind());

        // A codec-level truncation carries the format and inspection stage.
        let error = match image_slash_star::inspect_basic_prefix(&bytes[..40]) {
            Ok(info) => panic!("a truncated header must need more data: {info:?}"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::NeedMoreData);
        assert_eq!(error.format(), Some(ImageFormat::Png));
        assert_eq!(error.stage(), Some(ImageErrorStage::Inspection));
        assert!(error.minimum_input().is_some());
        let _: ImageError = error;
    }
    Ok(())
}

#[test]
fn incremental_inspection_reports_feature_state_like_legacy()
-> Result<(), Box<dyn std::error::Error>> {
    let cases: &[(ImageFormat, bool, &[u8])] = &[
        (ImageFormat::Jpeg, cfg!(feature = "jpeg"), b"\xff\xd8\xff"),
        (
            ImageFormat::Png,
            cfg!(feature = "png"),
            b"\x89PNG\r\n\x1a\n",
        ),
        (ImageFormat::Gif, cfg!(feature = "gif"), b"GIF89a"),
        (ImageFormat::Bmp, cfg!(feature = "bmp"), b"BM"),
        (ImageFormat::Tiff, cfg!(feature = "tiff"), b"II\x2a\0"),
        (
            ImageFormat::WebP,
            cfg!(feature = "webp"),
            b"RIFF\0\0\0\0WEBPVP8 ",
        ),
        (ImageFormat::Ico, cfg!(feature = "ico"), b"\0\0\x01\0"),
        (
            ImageFormat::Avif,
            cfg!(feature = "avif"),
            b"\0\0\0\x08ftypavif",
        ),
    ];
    for (format, enabled, signature) in cases {
        if *enabled {
            continue;
        }
        let incremental = match image_slash_star::inspect_basic_prefix(signature) {
            Ok(info) => panic!("feature-disabled inspection must fail for {format:?}: {info:?}"),
            Err(error) => error,
        };
        let legacy = match image_slash_star::inspect_basic(signature) {
            Ok(info) => {
                panic!("legacy feature-disabled inspection must fail for {format:?}: {info:?}")
            }
            Err(error) => error,
        };
        assert_eq!(incremental, legacy, "feature-disabled {format:?}");
        assert_eq!(
            incremental.kind(),
            image_slash_star::ImageErrorKind::FeatureDisabled
        );
        assert_eq!(incremental.minimum_input(), None);
    }
    Ok(())
}
