//! Cargo-feature and target-capability behavior driven by Pillow fixtures.

use std::collections::{HashMap, HashSet, hash_map::Entry};
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

// The feature matrix already builds this test target in every native and
// WASI lane. Keeping the capability probe here lets the runtime-table check
// reuse those compiled artifacts instead of compiling a second integration
// test target after the matrix completes.
#[path = "capability_table.rs"]
mod capability_table;

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
    diagnostic_identities: Vec<String>,
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
            diagnostic_identities: object.take_or_default("diagnostic_identities")?,
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
    // rows. JPEG additionally polls internal row/block/scan checkpoints;
    // deterministic checkpoint firing is covered by its codec-local
    // `#[cfg(coverage)]` drill rather than by a timing-sensitive public test.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    if cfg!(feature = "jpeg") {
        let data = fs::read(root.join("tests/fixtures/input/images/jpeg/1x1.jpg"))?;
        let decoded = image_slash_star::decode(&data)?;
        let options = EncodeOptions::for_format(ImageFormat::Jpeg);
        let expected = image_slash_star::encode(&decoded.content, ImageFormat::Jpeg, &options)?;
        let token = image_slash_star::CancellationToken::new();
        assert_eq!(
            image_slash_star::encode_with_token(
                &decoded.content,
                ImageFormat::Jpeg,
                &options,
                &token,
            )?,
            expected,
            "an uncancelled JPEG encode remains byte-identical"
        );

        let cancelled = image_slash_star::CancellationToken::new();
        cancelled.cancel();
        let error = match image_slash_star::encode_with_token(
            &decoded.content,
            ImageFormat::Jpeg,
            &options,
            &cancelled,
        ) {
            Ok(bytes) => {
                return Err(format!("cancelled JPEG encode returned {} bytes", bytes.len()).into());
            }
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Cancelled);
        assert_eq!(error.format(), Some(ImageFormat::Jpeg));
        assert_eq!(error.stage(), Some(ImageErrorStage::StillEncode));
    }

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

    if cfg!(feature = "bmp") {
        let data = fs::read(root.join("tests/fixtures/input/images/bmp/1x1.bmp"))?;
        let decoded = image_slash_star::decode(&data)?;
        let options = EncodeOptions::for_format(ImageFormat::Bmp);
        let expected = image_slash_star::encode(&decoded.content, ImageFormat::Bmp, &options)?;
        let token = image_slash_star::CancellationToken::new();
        assert_eq!(
            image_slash_star::encode_with_token_and_policy(
                &decoded.content,
                ImageFormat::Bmp,
                &options,
                &image_slash_star::EncodePolicy::default(),
                &token,
            )?,
            expected,
            "an uncancelled BMP still encode remains byte-identical"
        );

        let cancelled = image_slash_star::CancellationToken::new();
        cancelled.cancel();
        let error = match image_slash_star::encode_with_token(
            &decoded.content,
            ImageFormat::Bmp,
            &options,
            &cancelled,
        ) {
            Ok(bytes) => {
                return Err(format!("cancelled BMP encode returned {} bytes", bytes.len()).into());
            }
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Cancelled);
        assert_eq!(error.format(), Some(ImageFormat::Bmp));
        assert_eq!(error.stage(), Some(ImageErrorStage::StillEncode));
    }

    if cfg!(feature = "ico") {
        let data = fs::read(root.join("tests/fixtures/input/images/ico/16x16.ico"))?;
        let decoded = image_slash_star::decode(&data)?;
        let options = EncodeOptions::for_format(ImageFormat::Ico);
        let expected = image_slash_star::encode(&decoded.content, ImageFormat::Ico, &options)?;
        let token = image_slash_star::CancellationToken::new();
        assert_eq!(
            image_slash_star::encode_with_token(
                &decoded.content,
                ImageFormat::Ico,
                &options,
                &token,
            )?,
            expected,
            "an uncancelled ICO still encode remains byte-identical"
        );

        let cancelled = image_slash_star::CancellationToken::new();
        cancelled.cancel();
        let error = match image_slash_star::encode_with_token(
            &decoded.content,
            ImageFormat::Ico,
            &options,
            &cancelled,
        ) {
            Ok(bytes) => {
                return Err(format!("cancelled ICO encode returned {} bytes", bytes.len()).into());
            }
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Cancelled);
        assert_eq!(error.format(), Some(ImageFormat::Ico));
        assert_eq!(error.stage(), Some(ImageErrorStage::StillEncode));
    }

    if cfg!(feature = "tiff") {
        let data = fs::read(root.join("tests/fixtures/input/images/tiff/8bit.tiff"))?;
        let decoded = image_slash_star::decode(&data)?;
        let options = EncodeOptions::for_format(ImageFormat::Tiff);
        let expected = image_slash_star::encode(&decoded.content, ImageFormat::Tiff, &options)?;
        let token = image_slash_star::CancellationToken::new();
        assert_eq!(
            image_slash_star::encode_with_token(
                &decoded.content,
                ImageFormat::Tiff,
                &options,
                &token,
            )?,
            expected,
            "an uncancelled TIFF encode remains byte-identical"
        );

        let cancelled = image_slash_star::CancellationToken::new();
        cancelled.cancel();
        let error = match image_slash_star::encode_with_token(
            &decoded.content,
            ImageFormat::Tiff,
            &options,
            &cancelled,
        ) {
            Ok(bytes) => {
                return Err(format!("cancelled TIFF encode returned {} bytes", bytes.len()).into());
            }
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Cancelled);
        assert_eq!(error.format(), Some(ImageFormat::Tiff));
        assert_eq!(error.stage(), Some(ImageErrorStage::StillEncode));
    }

    if cfg!(feature = "gif") {
        let data = fs::read(root.join("tests/fixtures/input/images/gif/1x1.gif"))?;
        let decoded = image_slash_star::decode(&data)?;
        let options = EncodeOptions::for_format(ImageFormat::Gif);
        let expected = image_slash_star::encode(&decoded.content, ImageFormat::Gif, &options)?;
        let token = image_slash_star::CancellationToken::new();
        assert_eq!(
            image_slash_star::encode_with_token_and_policy(
                &decoded.content,
                ImageFormat::Gif,
                &options,
                &image_slash_star::EncodePolicy::default(),
                &token,
            )?,
            expected,
            "an uncancelled GIF still encode remains byte-identical"
        );

        let cancelled = image_slash_star::CancellationToken::new();
        cancelled.cancel();
        let error = match image_slash_star::encode_with_token(
            &decoded.content,
            ImageFormat::Gif,
            &options,
            &cancelled,
        ) {
            Ok(bytes) => {
                return Err(
                    format!("cancelled GIF still encode returned {} bytes", bytes.len()).into(),
                );
            }
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Cancelled);
        assert_eq!(error.format(), Some(ImageFormat::Gif));
        assert_eq!(error.stage(), Some(ImageErrorStage::StillEncode));

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

    if cfg!(feature = "webp") {
        let data = fs::read(root.join("tests/fixtures/input/images/webp/lossy.webp"))?;
        let decoded = image_slash_star::decode(&data)?;
        let options = EncodeOptions::for_format(ImageFormat::WebP);
        let expected = image_slash_star::encode(&decoded.content, ImageFormat::WebP, &options)?;
        let token = image_slash_star::CancellationToken::new();
        assert_eq!(
            image_slash_star::encode_with_token(
                &decoded.content,
                ImageFormat::WebP,
                &options,
                &token,
            )?,
            expected,
            "an uncancelled WebP still encode remains byte-identical"
        );

        let cancelled = image_slash_star::CancellationToken::new();
        cancelled.cancel();
        let error = match image_slash_star::encode_with_token(
            &decoded.content,
            ImageFormat::WebP,
            &options,
            &cancelled,
        ) {
            Ok(bytes) => {
                return Err(
                    format!("cancelled WebP still encode returned {} bytes", bytes.len()).into(),
                );
            }
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Cancelled);
        assert_eq!(error.format(), Some(ImageFormat::WebP));
        assert_eq!(error.stage(), Some(ImageErrorStage::StillEncode));
    }

    if cfg!(feature = "avif") && !cfg!(target_arch = "wasm32") {
        let data = fs::read(root.join("tests/fixtures/input/images/avif/baseline.avif"))?;
        let decoded = image_slash_star::decode(&data)?;
        let options = EncodeOptions::for_format(ImageFormat::Avif);
        let expected = image_slash_star::encode(&decoded.content, ImageFormat::Avif, &options)?;
        let token = image_slash_star::CancellationToken::new();
        assert_eq!(
            image_slash_star::encode_with_token(
                &decoded.content,
                ImageFormat::Avif,
                &options,
                &token,
            )?,
            expected,
            "an uncancelled AVIF still encode remains byte-identical"
        );

        let cancelled = image_slash_star::CancellationToken::new();
        cancelled.cancel();
        let error = match image_slash_star::encode_with_token(
            &decoded.content,
            ImageFormat::Avif,
            &options,
            &cancelled,
        ) {
            Ok(bytes) => {
                return Err(
                    format!("cancelled AVIF still encode returned {} bytes", bytes.len()).into(),
                );
            }
            Err(error) => error,
        };
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Cancelled);
        assert_eq!(error.format(), Some(ImageFormat::Avif));
        assert_eq!(error.stage(), Some(ImageErrorStage::StillEncode));
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

    // SourceAlpha is Rust source-provenance metadata, not a Pillow-observable
    // parity field. The AVIF case below uses the real committed fixture in
    // this feature-gated integration contract and adds no parity row.
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
            "avif auxiliary alpha",
            true,
            "tests/fixtures/input/images/avif/alpha.avif",
            Some(SourceAlpha::Auxiliary),
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
        if name == "avif auxiliary alpha" {
            let sequence = image_slash_star::decode_sequence(&bytes)?;
            assert_eq!(
                sequence.content.frames[0].image.source.alpha(),
                expected,
                "{name} sequence"
            );
        }
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

#[allow(
    clippy::arithmetic_side_effects,
    reason = "offsets are bounds-checked against the in-memory fixture slice"
)]
fn png_chunk_offset_after(
    data: &[u8],
    kind: &[u8; 4],
    after: usize,
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut position = after;
    while position.wrapping_add(8) <= data.len() {
        let length = u32::from_be_bytes([
            data[position],
            data[position + 1],
            data[position + 2],
            data[position + 3],
        ]) as usize;
        if position > after && &data[position.wrapping_add(4)..position.wrapping_add(8)] == kind {
            return Ok(position);
        }
        position = position.wrapping_add(12).wrapping_add(length);
    }
    Err(format!("PNG chunk {kind:?} after offset {after} not found").into())
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
// defensive-model policy rather than oracle output. Cases with `mutation ==
// "none"` may overlap an existing parity asset, but that parity row owns only
// Pillow's outer result; runtime mutations are not parity inputs. Keep this as
// a normal fixture-backed behavior contract, not a coverage-only diagnostic
// hook.
#[test]
fn diagnostic_manifest_matches_the_non_parity_contract() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: DiagnosticManifest = json::from_str(&fs::read_to_string(
        root.join("tests/fixtures/diagnostic_manifest.json"),
    )?)?;
    assert_eq!(manifest.format_version, 1);
    assert_eq!(manifest.assertion_origin, "defensive_model");
    assert_eq!(manifest.pillow_version, "12.2.0");

    // Multiple rows intentionally derive mutations from the same committed
    // baseline. Keep the fixture bytes and unmodified decode result hot so the
    // contract spends its time checking the mutation rather than reparsing
    // identical inputs for every row.
    let mut base_cache = HashMap::new();
    let mut still_baselines = HashMap::new();
    let mut sequence_baselines = HashMap::new();
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
        if let Entry::Vacant(entry) = base_cache.entry(case.asset_path.clone()) {
            entry.insert(fs::read(root.join(&case.asset_path))?);
        }
        let base = base_cache
            .get(&case.asset_path)
            .ok_or_else(|| format!("{}: diagnostic baseline cache entry", case.id))?;
        let bytes = match case.mutation.as_str() {
            "none" => base.to_vec(),
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
                let idat_offset = png_chunk_offset(base, b"IDAT")?;
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
                let iend_offset = png_chunk_offset(base, b"IEND")?;
                let mut mutated = Vec::with_capacity(base.len() + payload.len() + 12);
                mutated.extend_from_slice(&base[..iend_offset]);
                mutated.extend_from_slice(&png_chunk(&kind, &payload));
                mutated.extend_from_slice(&base[iend_offset..]);
                mutated
            }
            "png_bad_crc" => {
                let kind: [u8; 4] = case
                    .chunk_kind
                    .as_bytes()
                    .try_into()
                    .map_err(|_| format!("{}: chunk kind is not four bytes", case.id))?;
                let chunk_offset = png_chunk_offset(base, &kind)?;
                let length = u32::from_be_bytes([
                    base[chunk_offset],
                    base[chunk_offset + 1],
                    base[chunk_offset + 2],
                    base[chunk_offset + 3],
                ]) as usize;
                let crc_offset = chunk_offset
                    .checked_add(8)
                    .and_then(|offset| offset.checked_add(length))
                    .ok_or_else(|| format!("{}: CRC offset overflow", case.id))?;
                let mut mutated = base.to_vec();
                *mutated
                    .get_mut(crc_offset)
                    .ok_or_else(|| format!("{}: CRC is truncated", case.id))? ^= 1;
                mutated
            }
            "png_bad_crc_after_idat" => {
                let kind: [u8; 4] = case
                    .chunk_kind
                    .as_bytes()
                    .try_into()
                    .map_err(|_| format!("{}: chunk kind is not four bytes", case.id))?;
                let idat_offset = png_chunk_offset(base, b"IDAT")?;
                let chunk_offset = png_chunk_offset_after(base, &kind, idat_offset)?;
                let length = u32::from_be_bytes([
                    base[chunk_offset],
                    base[chunk_offset + 1],
                    base[chunk_offset + 2],
                    base[chunk_offset + 3],
                ]) as usize;
                let crc_offset = chunk_offset
                    .checked_add(8)
                    .and_then(|offset| offset.checked_add(length))
                    .ok_or_else(|| format!("{}: CRC offset overflow", case.id))?;
                let mut mutated = base.to_vec();
                *mutated
                    .get_mut(crc_offset)
                    .ok_or_else(|| format!("{}: CRC is truncated", case.id))? ^= 1;
                mutated
            }
            "png_after_idat_bad_crc" => {
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
                let iend_offset = png_chunk_offset(base, b"IEND")?;
                let mut chunk = png_chunk(&kind, &payload);
                *chunk
                    .last_mut()
                    .ok_or_else(|| format!("{}: generated chunk has no CRC", case.id))? ^= 1;
                let mut mutated = Vec::with_capacity(base.len() + chunk.len());
                mutated.extend_from_slice(&base[..iend_offset]);
                mutated.extend_from_slice(&chunk);
                mutated.extend_from_slice(&base[iend_offset..]);
                mutated
            }
            other => panic!("{}: unknown mutation `{other}`", case.id),
        };

        let diagnostic_identities = if case.diagnostic_identities.is_empty() {
            vec![case.identity.clone()]
        } else {
            case.diagnostic_identities.clone()
        };
        let expected = diagnostic_identities
            .iter()
            .map(|identity| ImageDiagnostic {
                kind: expected_kind,
                format: expected_format,
                stage: Some(expected_stage),
                offset: Some(case.offset),
                identity: Some(match identity.as_str() {
                    "gif_graphic_control" => "gif_graphic_control",
                    "png_zTXt" => "png_zTXt",
                    "png_iCCP" => "png_iCCP",
                    "png_iTXt" => "png_iTXt",
                    "png_IDAT_crc" => "png_IDAT_crc",
                    "png_IEND_crc" => "png_IEND_crc",
                    "png_acTL_crc" => "png_acTL_crc",
                    "png_fcTL_crc" => "png_fcTL_crc",
                    "png_fdAT_crc" => "png_fdAT_crc",
                    "png_post_idat_crc" => "png_post_idat_crc",
                    "png_reserved_bit" => "png_reserved_bit",
                    "png_ancillary_after_idat" => "png_ancillary_after_idat",
                    "png_missing_iend" => "png_missing_iend",
                    "png_duplicate_plte" => "png_duplicate_plte",
                    "png_duplicate_trns" => "png_duplicate_trns",
                    "png_trns_overlong" => "png_trns_overlong",
                    "png_missing_plte" => "png_missing_plte",
                    "png_empty_plte" => "png_empty_plte",
                    "png_partial_plte" => "png_partial_plte",
                    "png_trns_without_plte" => "png_trns_without_plte",
                    "png_apng_zero_frames" => "png_apng_zero_frames",
                    "png_apng_frame_count_out_of_range" => "png_apng_frame_count_out_of_range",
                    "png_duplicate_actl" => "png_duplicate_actl",
                    "png_actl_after_idat" => "png_actl_after_idat",
                    "png_actl_overlong" => "png_actl_overlong",
                    "png_oversized_scanline" => "png_oversized_scanline",
                    other => panic!("{}: unknown diagnostic identity `{other}`", case.id),
                }),
            })
            .collect::<Vec<_>>();
        match case.operation.as_str() {
            "decode" => {
                let base_decoded = match still_baselines.entry(case.asset_path.clone()) {
                    Entry::Occupied(entry) => entry.into_mut(),
                    Entry::Vacant(entry) => entry.insert(image_slash_star::decode(base)?),
                };
                let decoded = image_slash_star::decode(&bytes)?;
                assert_eq!(decoded.format, expected_format, "{} format", case.id);
                assert_eq!(
                    decoded.content.pixels, base_decoded.content.pixels,
                    "{} pixels",
                    case.id
                );
                assert_eq!(decoded.diagnostics, expected, "{} diagnostic", case.id);
                if expected_format == ImageFormat::Png {
                    assert!(decoded.content.metadata.is_empty(), "{} metadata", case.id);
                    assert!(decoded.content.source_color.is_empty(), "{} color", case.id);
                }
            }
            "decode_sequence" => {
                let base_sequence = match sequence_baselines.entry(case.asset_path.clone()) {
                    Entry::Occupied(entry) => entry.into_mut(),
                    Entry::Vacant(entry) => entry.insert(image_slash_star::decode_sequence(base)?),
                };
                let sequence = image_slash_star::decode_sequence(&bytes)?;
                assert_eq!(sequence.format, expected_format, "{} format", case.id);
                assert_eq!(
                    sequence.content.frames, base_sequence.content.frames,
                    "{} frames",
                    case.id
                );
                assert_eq!(sequence.diagnostics, expected, "{} diagnostic", case.id);
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
fn png_iend_crc_recovery_keeps_verification_strict() -> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(feature = "png") {
        return Ok(());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let base = fs::read(root.join("tests/fixtures/input/images/png/1x1.png"))?;
    let iend_offset = png_chunk_offset(&base, b"IEND")?;
    let length = u32::from_be_bytes([
        base[iend_offset],
        base[iend_offset + 1],
        base[iend_offset + 2],
        base[iend_offset + 3],
    ]) as usize;
    let crc_offset = iend_offset + 8 + length;
    let mut bytes = base;
    bytes[crc_offset] ^= 1;

    let decoded = image_slash_star::decode(&bytes)?;
    assert_eq!(
        decoded.diagnostics,
        vec![ImageDiagnostic {
            kind: DiagnosticKind::RecoveredStructure,
            format: ImageFormat::Png,
            stage: Some(ImageErrorStage::StillDecode),
            offset: Some(iend_offset as u64),
            identity: Some("png_IEND_crc"),
        }]
    );
    let sequence = image_slash_star::decode_sequence(&bytes)?;
    assert_eq!(
        sequence.diagnostics,
        vec![ImageDiagnostic {
            kind: DiagnosticKind::RecoveredStructure,
            format: ImageFormat::Png,
            stage: Some(ImageErrorStage::SequenceDecode),
            offset: Some(iend_offset as u64),
            identity: Some("png_IEND_crc"),
        }]
    );

    let source = EncodedImage::new(bytes)?;
    let error = source.verify().err().ok_or_else(|| {
        std::io::Error::other("Rust structural verification must keep rejecting the bad IEND CRC")
    })?;
    assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Malformed);
    assert_eq!(error.stage(), Some(ImageErrorStage::Verification));
    assert_eq!(error.offset(), Some(iend_offset as u64));
    assert_eq!(error.identity(), Some("png_chunk"));
    Ok(())
}

#[test]
fn png_post_idat_crc_recovery_keeps_verification_strict() -> Result<(), Box<dyn std::error::Error>>
{
    if !cfg!(feature = "png") {
        return Ok(());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let base = fs::read(root.join("tests/fixtures/input/images/png/apng_animated.png"))?;
    let idat_offset = png_chunk_offset(&base, b"IDAT")?;
    for (kind, identity) in [(b"fcTL", "png_fcTL_crc"), (b"fdAT", "png_fdAT_crc")] {
        let chunk_offset = png_chunk_offset_after(&base, kind, idat_offset)?;
        let length = u32::from_be_bytes([
            base[chunk_offset],
            base[chunk_offset + 1],
            base[chunk_offset + 2],
            base[chunk_offset + 3],
        ]) as usize;
        let crc_offset = chunk_offset + 8 + length;
        let mut bytes = base.clone();
        bytes[crc_offset] ^= 1;

        let decoded = image_slash_star::decode(&bytes)?;
        assert_eq!(
            decoded.diagnostics,
            vec![ImageDiagnostic {
                kind: DiagnosticKind::RecoveredStructure,
                format: ImageFormat::Png,
                stage: Some(ImageErrorStage::StillDecode),
                offset: Some(chunk_offset as u64),
                identity: Some(identity),
            }]
        );
        let source = EncodedImage::new(bytes)?;
        let error = source.verify().err().ok_or_else(|| {
            std::io::Error::other("Rust structural verification must reject a bad post-IDAT CRC")
        })?;
        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::Malformed);
        assert_eq!(error.stage(), Some(ImageErrorStage::Verification));
        assert_eq!(error.offset(), Some(chunk_offset as u64));
        assert_eq!(error.identity(), Some("png_chunk"));
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

    struct FlushSink {
        bytes: Vec<u8>,
        flushes: usize,
        fail: bool,
    }

    impl OutputSink for FlushSink {
        fn write_all(&mut self, bytes: &[u8]) -> image_slash_star::ImageResult<()> {
            self.bytes.extend_from_slice(bytes);
            Ok(())
        }

        fn flush(&mut self) -> image_slash_star::ImageResult<()> {
            self.flushes += 1;
            if self.fail {
                return Err(image_slash_star::ImageError::Unsupported {
                    format: None,
                    message: "sink rejected finalization".to_owned(),
                    stage: None,
                    reason: None,
                    offset: None,
                    identity: None,
                });
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

        let mut finalized = FlushSink {
            bytes: Vec::new(),
            flushes: 0,
            fail: false,
        };
        assert_eq!(
            image_slash_star::encode_to_sink(
                &decoded.content,
                ImageFormat::Png,
                &options,
                &mut finalized,
            )?,
            expected.len(),
            "finalized sink length"
        );
        assert_eq!(finalized.bytes, expected, "finalized sink bytes");
        assert_eq!(finalized.flushes, 1, "finalized sink flush count");

        let mut rejected_finalization = FlushSink {
            bytes: Vec::new(),
            flushes: 0,
            fail: true,
        };
        let finalization_error = match image_slash_star::encode_to_sink(
            &decoded.content,
            ImageFormat::Png,
            &options,
            &mut rejected_finalization,
        ) {
            Ok(length) => {
                return Err(
                    format!("flush-rejecting sink unexpectedly accepted {length} bytes").into(),
                );
            }
            Err(error) => error,
        };
        assert_eq!(
            finalization_error.kind(),
            image_slash_star::ImageErrorKind::OutputWrite
        );
        assert_eq!(
            finalization_error.format(),
            Some(ImageFormat::Png),
            "flush error format"
        );
        assert_eq!(
            finalization_error.stage(),
            Some(ImageErrorStage::StillEncode),
            "flush error stage"
        );
        assert_eq!(rejected_finalization.flushes, 1);
        assert_eq!(rejected_finalization.bytes, expected);

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
            assert!(
                bmp_sequence_sink.writes > 1,
                "BMP sequence output must cross structural write boundaries"
            );

            let mismatched_bmp_options = EncodeOptions::for_format(ImageFormat::Png);
            let mut mismatched_bmp_sequence_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let mismatched_bmp_sequence_error = match image_slash_star::encode_sequence_to_sink(
                &bmp_sequence,
                ImageFormat::Bmp,
                &mismatched_bmp_options,
                &mut mismatched_bmp_sequence_sink,
            ) {
                Ok(length) => {
                    return Err(format!(
                        "BMP sequence accepted mismatched options and wrote {length} bytes"
                    )
                    .into());
                }
                Err(error) => error,
            };
            assert_eq!(
                mismatched_bmp_sequence_error.kind(),
                image_slash_star::ImageErrorKind::Parameter
            );
            assert_eq!(
                mismatched_bmp_sequence_error.format(),
                Some(ImageFormat::Bmp)
            );
            assert_eq!(
                mismatched_bmp_sequence_error.stage(),
                Some(ImageErrorStage::SequenceEncode)
            );
            assert_eq!(mismatched_bmp_sequence_sink.writes, 0);
            assert!(mismatched_bmp_sequence_sink.bytes.is_empty());

            let mut multiple_bmp_sequence = bmp_sequence.clone();
            multiple_bmp_sequence
                .frames
                .push(multiple_bmp_sequence.frames[0].clone());
            multiple_bmp_sequence.kind = image_slash_star::SequenceKind::TimedAnimation;
            let mut multiple_bmp_sequence_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let multiple_bmp_sequence_error = match image_slash_star::encode_sequence_to_sink(
                &multiple_bmp_sequence,
                ImageFormat::Bmp,
                &bmp_options,
                &mut multiple_bmp_sequence_sink,
            ) {
                Ok(length) => {
                    return Err(format!(
                        "BMP sequence accepted multiple frames and wrote {length} bytes"
                    )
                    .into());
                }
                Err(error) => error,
            };
            assert_eq!(
                multiple_bmp_sequence_error.kind(),
                image_slash_star::ImageErrorKind::Unsupported
            );
            assert_eq!(
                multiple_bmp_sequence_error.unsupported_reason(),
                Some(UnsupportedReason::NotImplemented)
            );
            assert_eq!(multiple_bmp_sequence_error.format(), Some(ImageFormat::Bmp));
            assert_eq!(
                multiple_bmp_sequence_error.stage(),
                Some(ImageErrorStage::SequenceEncode)
            );
            assert_eq!(multiple_bmp_sequence_sink.writes, 0);
            assert!(multiple_bmp_sequence_sink.bytes.is_empty());

            let mut limited_bmp_sequence = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let limited_bmp_sequence_error =
                match image_slash_star::encode_sequence_to_sink_with_policy(
                    &bmp_sequence,
                    ImageFormat::Bmp,
                    &bmp_options,
                    &too_small,
                    &mut limited_bmp_sequence,
                ) {
                    Ok(length) => {
                        return Err(format!(
                            "BMP sequence output policy unexpectedly admitted {length} bytes"
                        )
                        .into());
                    }
                    Err(error) => error,
                };
            assert_eq!(
                limited_bmp_sequence_error.kind(),
                image_slash_star::ImageErrorKind::LimitExceeded
            );
            assert!(matches!(
                limited_bmp_sequence_error,
                image_slash_star::ImageError::LimitExceeded {
                    format: Some(ImageFormat::Bmp),
                    operation: image_slash_star::CodecOperation::SequenceEncode,
                    resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
                    ..
                }
            ));
            assert_eq!(limited_bmp_sequence.writes, 0);
            assert!(limited_bmp_sequence.bytes.is_empty());

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

        if cfg!(feature = "ico") {
            // ICO still and one-frame sequence delivery now split the fixed
            // directory header from the embedded PNG/DIB payload. This is a
            // Rust-only destination contract: Pillow has no caller-owned
            // sink, so the parity matrix remains unchanged.
            let ico_options = EncodeOptions::for_format(ImageFormat::Ico);
            let expected_ico =
                image_slash_star::encode(&decoded.content, ImageFormat::Ico, &ico_options)?;
            let mut ico_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            assert_eq!(
                image_slash_star::encode_to_sink(
                    &decoded.content,
                    ImageFormat::Ico,
                    &ico_options,
                    &mut ico_sink,
                )?,
                expected_ico.len()
            );
            assert_eq!(ico_sink.bytes, expected_ico);
            assert_eq!(ico_sink.writes, 2);
            assert_eq!(&ico_sink.bytes[..6], &[0, 0, 1, 0, 1, 0]);

            let ico_token = image_slash_star::CancellationToken::new();
            let mut token_ico_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            assert_eq!(
                image_slash_star::encode_to_sink_with_token(
                    &decoded.content,
                    ImageFormat::Ico,
                    &ico_options,
                    &ico_token,
                    &mut token_ico_sink,
                )?,
                expected_ico.len()
            );
            assert_eq!(token_ico_sink.bytes, expected_ico);
            assert_eq!(token_ico_sink.writes, 2);

            let pre_cancelled_ico_token = image_slash_star::CancellationToken::new();
            pre_cancelled_ico_token.cancel();
            let mut pre_cancelled_ico = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let pre_cancelled_ico_error = match image_slash_star::encode_to_sink_with_token(
                &decoded.content,
                ImageFormat::Ico,
                &ico_options,
                &pre_cancelled_ico_token,
                &mut pre_cancelled_ico,
            ) {
                Ok(length) => {
                    return Err(
                        format!("pre-cancelled ICO unexpectedly wrote {length} bytes").into(),
                    );
                }
                Err(error) => error,
            };
            assert_eq!(
                pre_cancelled_ico_error.kind(),
                image_slash_star::ImageErrorKind::Cancelled
            );
            assert_eq!(pre_cancelled_ico_error.format(), Some(ImageFormat::Ico));
            assert_eq!(
                pre_cancelled_ico_error.stage(),
                Some(ImageErrorStage::StillEncode)
            );
            assert_eq!(pre_cancelled_ico.writes, 0);
            assert!(pre_cancelled_ico.bytes.is_empty());

            let mut first_failing_ico = FailingSink;
            let first_failing_ico_error = match image_slash_star::encode_to_sink(
                &decoded.content,
                ImageFormat::Ico,
                &ico_options,
                &mut first_failing_ico,
            ) {
                Ok(length) => {
                    return Err(
                        format!("first-write ICO sink unexpectedly wrote {length} bytes").into(),
                    );
                }
                Err(error) => error,
            };
            assert_eq!(
                first_failing_ico_error.kind(),
                image_slash_star::ImageErrorKind::OutputWrite
            );
            assert_eq!(first_failing_ico_error.format(), Some(ImageFormat::Ico));
            assert_eq!(
                first_failing_ico_error.stage(),
                Some(ImageErrorStage::StillEncode)
            );

            let mut failing_ico = FailingAfterWrites {
                fail_at: 2,
                writes: 0,
            };
            let failing_ico_error = match image_slash_star::encode_to_sink(
                &decoded.content,
                ImageFormat::Ico,
                &ico_options,
                &mut failing_ico,
            ) {
                Ok(length) => {
                    return Err(
                        format!("failing ICO sink unexpectedly wrote {length} bytes").into(),
                    );
                }
                Err(error) => error,
            };
            assert_eq!(
                failing_ico_error.kind(),
                image_slash_star::ImageErrorKind::OutputWrite
            );
            assert_eq!(failing_ico_error.format(), Some(ImageFormat::Ico));
            assert_eq!(
                failing_ico_error.stage(),
                Some(ImageErrorStage::StillEncode)
            );
            assert_eq!(failing_ico.writes, 2);

            let cancelling_ico_token = image_slash_star::CancellationToken::new();
            let mut cancelling_ico = CancellingSink {
                bytes: Vec::new(),
                token: cancelling_ico_token.clone(),
                writes: 0,
            };
            let cancelling_ico_error = match image_slash_star::encode_to_sink_with_token(
                &decoded.content,
                ImageFormat::Ico,
                &ico_options,
                &cancelling_ico_token,
                &mut cancelling_ico,
            ) {
                Ok(length) => {
                    return Err(
                        format!("cancelled ICO sink unexpectedly wrote {length} bytes").into(),
                    );
                }
                Err(error) => error,
            };
            assert_eq!(
                cancelling_ico_error.kind(),
                image_slash_star::ImageErrorKind::Cancelled
            );
            assert_eq!(cancelling_ico_error.format(), Some(ImageFormat::Ico));
            assert_eq!(
                cancelling_ico_error.stage(),
                Some(ImageErrorStage::StillEncode)
            );
            assert_eq!(cancelling_ico.writes, 1);
            assert_eq!(cancelling_ico.bytes, expected_ico[..22].to_vec());

            let too_small_ico = EncodePolicy::default()
                .with_max_output_bytes(u64::try_from(expected_ico.len() - 1)?);
            let mut limited_ico = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let limited_ico_error = match image_slash_star::encode_to_sink_with_policy(
                &decoded.content,
                ImageFormat::Ico,
                &ico_options,
                &too_small_ico,
                &mut limited_ico,
            ) {
                Ok(length) => {
                    return Err(
                        format!("ICO output policy unexpectedly admitted {length} bytes").into(),
                    );
                }
                Err(error) => error,
            };
            assert!(matches!(
                limited_ico_error,
                ImageError::LimitExceeded {
                    format: Some(ImageFormat::Ico),
                    operation: image_slash_star::CodecOperation::StillEncode,
                    resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
                    ..
                }
            ));
            assert_eq!(limited_ico.writes, 0);
            assert!(limited_ico.bytes.is_empty());

            let sequence = DecodedSequence::from_image(decoded.content.clone());
            let mut sequence_ico_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            assert_eq!(
                image_slash_star::encode_sequence_to_sink(
                    &sequence,
                    ImageFormat::Ico,
                    &ico_options,
                    &mut sequence_ico_sink,
                )?,
                expected_ico.len()
            );
            assert_eq!(sequence_ico_sink.bytes, expected_ico);
            assert_eq!(sequence_ico_sink.writes, 2);

            let mismatch_sequence_ico_options = EncodeOptions::for_format(ImageFormat::Gif);
            let mut mismatch_sequence_ico_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let mismatch_sequence_ico_error = match image_slash_star::encode_sequence_to_sink(
                &sequence,
                ImageFormat::Ico,
                &mismatch_sequence_ico_options,
                &mut mismatch_sequence_ico_sink,
            ) {
                Ok(length) => {
                    return Err(format!(
                        "ICO sequence accepted mismatched options and wrote {length} bytes"
                    )
                    .into());
                }
                Err(error) => error,
            };
            assert_eq!(
                mismatch_sequence_ico_error.kind(),
                image_slash_star::ImageErrorKind::Parameter
            );
            assert_eq!(
                mismatch_sequence_ico_error.stage(),
                Some(ImageErrorStage::SequenceEncode)
            );
            assert_eq!(mismatch_sequence_ico_sink.writes, 0);
            assert!(mismatch_sequence_ico_sink.bytes.is_empty());

            let mut multiple_ico_sequence = sequence.clone();
            multiple_ico_sequence
                .frames
                .push(multiple_ico_sequence.frames[0].clone());
            multiple_ico_sequence.kind = image_slash_star::SequenceKind::TimedAnimation;
            let mut multiple_ico_sequence_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let multiple_ico_sequence_error = match image_slash_star::encode_sequence_to_sink(
                &multiple_ico_sequence,
                ImageFormat::Ico,
                &ico_options,
                &mut multiple_ico_sequence_sink,
            ) {
                Ok(length) => {
                    return Err(format!(
                        "ICO sequence accepted multiple frames and wrote {length} bytes"
                    )
                    .into());
                }
                Err(error) => error,
            };
            assert_eq!(
                multiple_ico_sequence_error.kind(),
                image_slash_star::ImageErrorKind::Unsupported
            );
            assert_eq!(
                multiple_ico_sequence_error.unsupported_reason(),
                Some(UnsupportedReason::NotImplemented)
            );
            assert_eq!(multiple_ico_sequence_error.format(), Some(ImageFormat::Ico));
            assert_eq!(
                multiple_ico_sequence_error.stage(),
                Some(ImageErrorStage::SequenceEncode)
            );
            assert_eq!(multiple_ico_sequence_sink.writes, 0);
            assert!(multiple_ico_sequence_sink.bytes.is_empty());

            let mismatch_options = EncodeOptions::for_format(ImageFormat::Gif);
            let mut mismatch_ico_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let mismatch_ico_error = match image_slash_star::encode_to_sink(
                &decoded.content,
                ImageFormat::Ico,
                &mismatch_options,
                &mut mismatch_ico_sink,
            ) {
                Ok(length) => {
                    return Err(format!(
                        "ICO accepted mismatched options and wrote {length} bytes"
                    )
                    .into());
                }
                Err(error) => error,
            };
            assert_eq!(
                mismatch_ico_error.kind(),
                image_slash_star::ImageErrorKind::Parameter
            );
            assert_eq!(
                mismatch_ico_error.stage(),
                Some(ImageErrorStage::StillEncode)
            );
            assert_eq!(mismatch_ico_sink.writes, 0);

            let mismatched_size_ico_options = {
                let mut options = EncodeOptions::for_format(ImageFormat::Ico);
                if let EncodeOptions::Ico(options) = &mut options {
                    options.sizes = vec![image_slash_star::IcoSize {
                        width: 2,
                        height: 2,
                    }];
                }
                options
            };
            let mut mismatched_size_ico_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let mismatched_size_ico_error = match image_slash_star::encode_to_sink(
                &decoded.content,
                ImageFormat::Ico,
                &mismatched_size_ico_options,
                &mut mismatched_size_ico_sink,
            ) {
                Ok(length) => {
                    return Err(format!(
                        "ICO accepted mismatched source size and wrote {length} bytes"
                    )
                    .into());
                }
                Err(error) => error,
            };
            assert_eq!(
                mismatched_size_ico_error.kind(),
                image_slash_star::ImageErrorKind::Parameter
            );
            assert_eq!(mismatched_size_ico_error.format(), Some(ImageFormat::Ico));
            assert_eq!(
                mismatched_size_ico_error.stage(),
                Some(ImageErrorStage::StillEncode)
            );
            assert_eq!(mismatched_size_ico_sink.writes, 0);

            let mut ico_bmp_options = EncodeOptions::for_format(ImageFormat::Ico);
            if let EncodeOptions::Ico(options) = &mut ico_bmp_options {
                options.entry_type = image_slash_star::IcoEntryType::Bmp;
            }
            let expected_ico_bmp =
                image_slash_star::encode(&decoded.content, ImageFormat::Ico, &ico_bmp_options)?;
            let mut ico_bmp_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            assert_eq!(
                image_slash_star::encode_to_sink(
                    &decoded.content,
                    ImageFormat::Ico,
                    &ico_bmp_options,
                    &mut ico_bmp_sink,
                )?,
                expected_ico_bmp.len()
            );
            assert_eq!(ico_bmp_sink.bytes, expected_ico_bmp);
            assert_eq!(ico_bmp_sink.writes, 2);

            let invalid_ico_image =
                image_slash_star::DecodedImage::new(1, 1, Vec::new(), ColorType::Rgb8);
            let mut invalid_ico_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let invalid_ico_error = match image_slash_star::encode_to_sink(
                &invalid_ico_image,
                ImageFormat::Ico,
                &ico_options,
                &mut invalid_ico_sink,
            ) {
                Ok(length) => {
                    return Err(
                        format!("invalid ICO input unexpectedly wrote {length} bytes").into(),
                    );
                }
                Err(error) => error,
            };
            assert_eq!(
                invalid_ico_error.kind(),
                image_slash_star::ImageErrorKind::Dimensions
            );
            assert_eq!(invalid_ico_error.format(), Some(ImageFormat::Ico));
            assert_eq!(invalid_ico_sink.writes, 0);

            let oversized_ico_image =
                image_slash_star::DecodedImage::new(257, 1, vec![0; 257 * 3], ColorType::Rgb8);
            let mut oversized_ico_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let oversized_ico_error = match image_slash_star::encode_to_sink(
                &oversized_ico_image,
                ImageFormat::Ico,
                &ico_options,
                &mut oversized_ico_sink,
            ) {
                Ok(length) => {
                    return Err(
                        format!("oversized ICO input unexpectedly wrote {length} bytes").into(),
                    );
                }
                Err(error) => error,
            };
            assert_eq!(
                oversized_ico_error.kind(),
                image_slash_star::ImageErrorKind::Dimensions
            );
            assert_eq!(oversized_ico_error.format(), Some(ImageFormat::Ico));
            assert_eq!(oversized_ico_sink.writes, 0);

            let unsupported_ico_image =
                image_slash_star::DecodedImage::new(1, 1, vec![0, 0, 0, 0], ColorType::Cmyk8);
            let mut unsupported_ico_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let unsupported_ico_error = match image_slash_star::encode_to_sink(
                &unsupported_ico_image,
                ImageFormat::Ico,
                &ico_options,
                &mut unsupported_ico_sink,
            ) {
                Ok(length) => {
                    return Err(
                        format!("unsupported ICO mode unexpectedly wrote {length} bytes").into(),
                    );
                }
                Err(error) => error,
            };
            assert_eq!(
                unsupported_ico_error.kind(),
                image_slash_star::ImageErrorKind::Unsupported
            );
            assert_eq!(unsupported_ico_error.format(), Some(ImageFormat::Ico));
            assert_eq!(unsupported_ico_sink.writes, 0);
            assert!(unsupported_ico_sink.bytes.is_empty());

            let mut unsupported_ico_bmp_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let unsupported_ico_bmp_error = match image_slash_star::encode_to_sink(
                &unsupported_ico_image,
                ImageFormat::Ico,
                &ico_bmp_options,
                &mut unsupported_ico_bmp_sink,
            ) {
                Ok(length) => {
                    return Err(format!(
                        "unsupported BMP-backed ICO mode unexpectedly wrote {length} bytes"
                    )
                    .into());
                }
                Err(error) => error,
            };
            assert_eq!(
                unsupported_ico_bmp_error.kind(),
                image_slash_star::ImageErrorKind::Unsupported
            );
            assert_eq!(unsupported_ico_bmp_error.format(), Some(ImageFormat::Ico));
            assert_eq!(unsupported_ico_bmp_sink.writes, 0);

            let unsupported_ico_sequence =
                DecodedSequence::from_image(unsupported_ico_image.clone());
            let mut unsupported_ico_sequence_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let unsupported_ico_sequence_error = match image_slash_star::encode_sequence_to_sink(
                &unsupported_ico_sequence,
                ImageFormat::Ico,
                &ico_options,
                &mut unsupported_ico_sequence_sink,
            ) {
                Ok(length) => {
                    return Err(format!(
                        "unsupported ICO sequence mode unexpectedly wrote {length} bytes"
                    )
                    .into());
                }
                Err(error) => error,
            };
            assert_eq!(
                unsupported_ico_sequence_error.kind(),
                image_slash_star::ImageErrorKind::Unsupported
            );
            assert_eq!(
                unsupported_ico_sequence_error.format(),
                Some(ImageFormat::Ico)
            );
            assert_eq!(
                unsupported_ico_sequence_error.stage(),
                Some(ImageErrorStage::SequenceEncode)
            );
            assert_eq!(unsupported_ico_sequence_sink.writes, 0);
            assert!(unsupported_ico_sequence_sink.bytes.is_empty());
        }

        if cfg!(feature = "webp") {
            // WebP still delivery now splits the validated RIFF header and
            // chunks while retaining the complete codec working buffer. This
            // is a Rust-only sink contract: Pillow has no caller-owned sink.
            let webp_options = EncodeOptions::for_format(ImageFormat::WebP);
            let expected_webp =
                image_slash_star::encode(&decoded.content, ImageFormat::WebP, &webp_options)?;
            let mut webp_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            assert_eq!(
                image_slash_star::encode_to_sink(
                    &decoded.content,
                    ImageFormat::WebP,
                    &webp_options,
                    &mut webp_sink,
                )?,
                expected_webp.len()
            );
            assert_eq!(webp_sink.bytes, expected_webp);
            assert!(webp_sink.writes > 1);
            assert_eq!(&webp_sink.bytes[..4], b"RIFF");
            assert_eq!(&webp_sink.bytes[8..12], b"WEBP");

            let webp_token = image_slash_star::CancellationToken::new();
            let mut cancelling_webp = CancellingSink {
                bytes: Vec::new(),
                token: webp_token.clone(),
                writes: 0,
            };
            let cancelling_webp_error = match image_slash_star::encode_to_sink_with_token(
                &decoded.content,
                ImageFormat::WebP,
                &webp_options,
                &webp_token,
                &mut cancelling_webp,
            ) {
                Ok(length) => {
                    return Err(format!(
                        "WebP sink-triggered cancellation unexpectedly wrote {length} bytes"
                    )
                    .into());
                }
                Err(error) => error,
            };
            assert_eq!(
                cancelling_webp_error.kind(),
                image_slash_star::ImageErrorKind::Cancelled
            );
            assert_eq!(cancelling_webp_error.format(), Some(ImageFormat::WebP));
            assert_eq!(
                cancelling_webp_error.stage(),
                Some(ImageErrorStage::StillEncode)
            );
            assert_eq!(cancelling_webp.writes, 1);
            assert_eq!(cancelling_webp.bytes, expected_webp[..12].to_vec());

            let too_small_webp = EncodePolicy::default()
                .with_max_output_bytes(u64::try_from(expected_webp.len() - 1)?);
            let mut limited_webp = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let limited_webp_error = match image_slash_star::encode_to_sink_with_policy(
                &decoded.content,
                ImageFormat::WebP,
                &webp_options,
                &too_small_webp,
                &mut limited_webp,
            ) {
                Ok(length) => {
                    return Err(
                        format!("WebP output policy unexpectedly admitted {length} bytes").into(),
                    );
                }
                Err(error) => error,
            };
            assert!(matches!(
                limited_webp_error,
                ImageError::LimitExceeded {
                    format: Some(ImageFormat::WebP),
                    operation: image_slash_star::CodecOperation::StillEncode,
                    resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
                    ..
                }
            ));
            assert_eq!(limited_webp.writes, 0);

            let mut failing_webp = FailingAfterWrites {
                fail_at: 2,
                writes: 0,
            };
            let failing_webp_error = match image_slash_star::encode_to_sink(
                &decoded.content,
                ImageFormat::WebP,
                &webp_options,
                &mut failing_webp,
            ) {
                Ok(length) => {
                    return Err(format!("WebP sink unexpectedly accepted {length} bytes").into());
                }
                Err(error) => error,
            };
            assert_eq!(
                failing_webp_error.kind(),
                image_slash_star::ImageErrorKind::OutputWrite
            );
            assert_eq!(failing_webp_error.format(), Some(ImageFormat::WebP));
            assert_eq!(
                failing_webp_error.stage(),
                Some(ImageErrorStage::StillEncode)
            );
            assert_eq!(failing_webp.writes, 2);

            // A one-frame WebP sequence uses the same structural RIFF
            // delivery while retaining SequenceEncode error context. This is
            // a Rust-only sink contract; Pillow has no caller-owned sink.
            let webp_sequence =
                image_slash_star::DecodedSequence::from_image(decoded.content.clone());
            let webp_sequence_expected = image_slash_star::encode_sequence(
                &webp_sequence,
                ImageFormat::WebP,
                &webp_options,
            )?;
            let mut webp_sequence_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            assert_eq!(
                image_slash_star::encode_sequence_to_sink(
                    &webp_sequence,
                    ImageFormat::WebP,
                    &webp_options,
                    &mut webp_sequence_sink,
                )?,
                webp_sequence_expected.len()
            );
            assert_eq!(webp_sequence_sink.bytes, webp_sequence_expected);
            assert!(webp_sequence_sink.writes > 1);

            let webp_sequence_token = image_slash_star::CancellationToken::new();
            let mut cancelling_webp_sequence = CancellingSink {
                bytes: Vec::new(),
                token: webp_sequence_token.clone(),
                writes: 0,
            };
            let cancelling_webp_sequence_error =
                match image_slash_star::encode_sequence_to_sink_with_token(
                    &webp_sequence,
                    ImageFormat::WebP,
                    &webp_options,
                    &webp_sequence_token,
                    &mut cancelling_webp_sequence,
                ) {
                    Ok(length) => {
                        return Err(format!(
                        "WebP sequence sink-triggered cancellation unexpectedly wrote {length} bytes"
                    )
                    .into());
                    }
                    Err(error) => error,
                };
            assert_eq!(
                cancelling_webp_sequence_error.kind(),
                image_slash_star::ImageErrorKind::Cancelled
            );
            assert_eq!(
                cancelling_webp_sequence_error.format(),
                Some(ImageFormat::WebP)
            );
            assert_eq!(
                cancelling_webp_sequence_error.stage(),
                Some(ImageErrorStage::SequenceEncode)
            );
            assert_eq!(cancelling_webp_sequence.writes, 1);
            assert_eq!(
                cancelling_webp_sequence.bytes,
                webp_sequence_expected[..12].to_vec()
            );

            let too_small_webp_sequence = EncodePolicy::default()
                .with_max_output_bytes(u64::try_from(webp_sequence_expected.len() - 1)?);
            let mut limited_webp_sequence = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let limited_webp_sequence_error =
                match image_slash_star::encode_sequence_to_sink_with_policy(
                    &webp_sequence,
                    ImageFormat::WebP,
                    &webp_options,
                    &too_small_webp_sequence,
                    &mut limited_webp_sequence,
                ) {
                    Ok(length) => {
                        return Err(format!(
                            "WebP sequence output policy unexpectedly admitted {length} bytes"
                        )
                        .into());
                    }
                    Err(error) => error,
                };
            assert!(matches!(
                limited_webp_sequence_error,
                ImageError::LimitExceeded {
                    format: Some(ImageFormat::WebP),
                    operation: image_slash_star::CodecOperation::SequenceEncode,
                    resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
                    ..
                }
            ));
            assert_eq!(limited_webp_sequence.writes, 0);

            let mut failing_webp_sequence = FailingAfterWrites {
                fail_at: 2,
                writes: 0,
            };
            let failing_webp_sequence_error = match image_slash_star::encode_sequence_to_sink(
                &webp_sequence,
                ImageFormat::WebP,
                &webp_options,
                &mut failing_webp_sequence,
            ) {
                Ok(length) => {
                    return Err(
                        format!("WebP sequence sink unexpectedly accepted {length} bytes").into(),
                    );
                }
                Err(error) => error,
            };
            assert_eq!(
                failing_webp_sequence_error.kind(),
                image_slash_star::ImageErrorKind::OutputWrite
            );
            assert_eq!(
                failing_webp_sequence_error.format(),
                Some(ImageFormat::WebP)
            );
            assert_eq!(
                failing_webp_sequence_error.stage(),
                Some(ImageErrorStage::SequenceEncode)
            );
            assert_eq!(failing_webp_sequence.writes, 2);

            // Multi-frame WebP now uses the same validated RIFF structural
            // delivery boundary. This remains a Rust-only sink contract:
            // Pillow has no caller-owned sink and the parity matrix is
            // unchanged.
            let mut multiple_webp_sequence = webp_sequence.clone();
            multiple_webp_sequence
                .frames
                .push(multiple_webp_sequence.frames[0].clone());
            multiple_webp_sequence.kind = image_slash_star::SequenceKind::TimedAnimation;
            let multiple_webp_expected = image_slash_star::encode_sequence(
                &multiple_webp_sequence,
                ImageFormat::WebP,
                &webp_options,
            )?;
            let mut multiple_webp_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            assert_eq!(
                image_slash_star::encode_sequence_to_sink(
                    &multiple_webp_sequence,
                    ImageFormat::WebP,
                    &webp_options,
                    &mut multiple_webp_sink,
                )?,
                multiple_webp_expected.len()
            );
            assert_eq!(multiple_webp_sink.bytes, multiple_webp_expected);
            assert!(multiple_webp_sink.writes > 1);
            assert_eq!(&multiple_webp_sink.bytes[..4], b"RIFF");
            assert_eq!(&multiple_webp_sink.bytes[8..12], b"WEBP");
            assert!(
                multiple_webp_sink
                    .bytes
                    .windows(4)
                    .any(|chunk| chunk == b"ANMF"),
                "multi-frame WebP must retain animation chunks"
            );

            let multiple_webp_token = image_slash_star::CancellationToken::new();
            let mut cancelling_multiple_webp = CancellingSink {
                bytes: Vec::new(),
                token: multiple_webp_token.clone(),
                writes: 0,
            };
            let multiple_webp_cancellation_error =
                match image_slash_star::encode_sequence_to_sink_with_token(
                    &multiple_webp_sequence,
                    ImageFormat::WebP,
                    &webp_options,
                    &multiple_webp_token,
                    &mut cancelling_multiple_webp,
                ) {
                    Ok(length) => {
                        return Err(format!(
                            "multi-frame WebP sink cancellation unexpectedly wrote {length} bytes"
                        )
                        .into());
                    }
                    Err(error) => error,
                };
            assert_eq!(
                multiple_webp_cancellation_error.kind(),
                image_slash_star::ImageErrorKind::Cancelled
            );
            assert_eq!(
                multiple_webp_cancellation_error.format(),
                Some(ImageFormat::WebP)
            );
            assert_eq!(
                multiple_webp_cancellation_error.stage(),
                Some(ImageErrorStage::SequenceEncode)
            );
            assert_eq!(cancelling_multiple_webp.writes, 1);
            assert_eq!(
                cancelling_multiple_webp.bytes,
                multiple_webp_expected[..12].to_vec()
            );

            let too_small_multiple_webp = EncodePolicy::default()
                .with_max_output_bytes(u64::try_from(multiple_webp_expected.len() - 1)?);
            let mut limited_multiple_webp = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let multiple_webp_limit_error =
                match image_slash_star::encode_sequence_to_sink_with_policy(
                    &multiple_webp_sequence,
                    ImageFormat::WebP,
                    &webp_options,
                    &too_small_multiple_webp,
                    &mut limited_multiple_webp,
                ) {
                    Ok(length) => {
                        return Err(format!(
                            "multi-frame WebP output policy unexpectedly admitted {length} bytes"
                        )
                        .into());
                    }
                    Err(error) => error,
                };
            assert!(matches!(
                multiple_webp_limit_error,
                ImageError::LimitExceeded {
                    format: Some(ImageFormat::WebP),
                    operation: image_slash_star::CodecOperation::SequenceEncode,
                    resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
                    ..
                }
            ));
            assert_eq!(limited_multiple_webp.writes, 0);
            assert!(limited_multiple_webp.bytes.is_empty());

            let mut failing_multiple_webp = FailingAfterWrites {
                fail_at: 2,
                writes: 0,
            };
            let multiple_webp_write_error = match image_slash_star::encode_sequence_to_sink(
                &multiple_webp_sequence,
                ImageFormat::WebP,
                &webp_options,
                &mut failing_multiple_webp,
            ) {
                Ok(length) => {
                    return Err(format!(
                        "multi-frame WebP sink unexpectedly accepted {length} bytes"
                    )
                    .into());
                }
                Err(error) => error,
            };
            assert_eq!(
                multiple_webp_write_error.kind(),
                image_slash_star::ImageErrorKind::OutputWrite
            );
            assert_eq!(multiple_webp_write_error.format(), Some(ImageFormat::WebP));
            assert_eq!(
                multiple_webp_write_error.stage(),
                Some(ImageErrorStage::SequenceEncode)
            );
            assert_eq!(failing_multiple_webp.writes, 2);
        }

        if cfg!(feature = "jpeg") {
            // JPEG marker and entropy-scan delivery is a Rust-only structural
            // sink contract. Pillow has no caller-owned sink, so parity stays
            // unchanged while the complete encoder buffer remains retained.
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
            assert!(
                jpeg_sink.writes > 1,
                "JPEG output must cross marker boundaries"
            );

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
            assert!(
                jpeg_token_sink.writes > 1,
                "JPEG token path must cross marker boundaries"
            );

            // A one-frame JPEG sequence uses the same validated marker/scan
            // writer as still output. This is a Rust-only destination
            // contract: Pillow has no caller-owned sink, so the parity
            // matrix remains unchanged.
            let jpeg_sequence = DecodedSequence::from_image(jpeg_image.clone());
            let jpeg_sequence_expected = image_slash_star::encode_sequence(
                &jpeg_sequence,
                ImageFormat::Jpeg,
                &jpeg_options,
            )?;
            let mut jpeg_sequence_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            assert_eq!(
                image_slash_star::encode_sequence_to_sink(
                    &jpeg_sequence,
                    ImageFormat::Jpeg,
                    &jpeg_options,
                    &mut jpeg_sequence_sink,
                )?,
                jpeg_sequence_expected.len()
            );
            assert_eq!(jpeg_sequence_sink.bytes, jpeg_sequence_expected);
            assert!(
                jpeg_sequence_sink.writes > 1,
                "one-frame JPEG sequence must cross marker boundaries"
            );

            let jpeg_sequence_token = image_slash_star::CancellationToken::new();
            let mut jpeg_sequence_token_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            assert_eq!(
                image_slash_star::encode_sequence_to_sink_with_token(
                    &jpeg_sequence,
                    ImageFormat::Jpeg,
                    &jpeg_options,
                    &jpeg_sequence_token,
                    &mut jpeg_sequence_token_sink,
                )?,
                jpeg_sequence_expected.len()
            );
            assert_eq!(
                jpeg_sequence_token_sink.bytes, jpeg_sequence_expected,
                "one-frame JPEG sequence token bytes"
            );
            assert!(jpeg_sequence_token_sink.writes > 1);

            let jpeg_sequence_cancel_token = image_slash_star::CancellationToken::new();
            let mut cancelling_jpeg_sequence = CancellingSink {
                bytes: Vec::new(),
                token: jpeg_sequence_cancel_token.clone(),
                writes: 0,
            };
            let jpeg_sequence_cancel_error =
                match image_slash_star::encode_sequence_to_sink_with_token(
                    &jpeg_sequence,
                    ImageFormat::Jpeg,
                    &jpeg_options,
                    &jpeg_sequence_cancel_token,
                    &mut cancelling_jpeg_sequence,
                ) {
                    Ok(length) => {
                        return Err(format!(
                            "one-frame JPEG sequence cancellation unexpectedly wrote {length} bytes"
                        )
                        .into());
                    }
                    Err(error) => error,
                };
            assert_eq!(
                jpeg_sequence_cancel_error.kind(),
                image_slash_star::ImageErrorKind::Cancelled
            );
            assert_eq!(jpeg_sequence_cancel_error.format(), Some(ImageFormat::Jpeg));
            assert_eq!(
                jpeg_sequence_cancel_error.stage(),
                Some(ImageErrorStage::SequenceEncode)
            );
            assert_eq!(cancelling_jpeg_sequence.writes, 1);
            assert_eq!(cancelling_jpeg_sequence.bytes, b"\xff\xd8");

            let too_small_jpeg_sequence = EncodePolicy::default()
                .with_max_output_bytes(u64::try_from(jpeg_sequence_expected.len() - 1)?);
            let mut limited_jpeg_sequence = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let jpeg_sequence_limit_error =
                match image_slash_star::encode_sequence_to_sink_with_policy(
                    &jpeg_sequence,
                    ImageFormat::Jpeg,
                    &jpeg_options,
                    &too_small_jpeg_sequence,
                    &mut limited_jpeg_sequence,
                ) {
                    Ok(length) => {
                        return Err(format!(
                            "JPEG sequence output policy unexpectedly admitted {length} bytes"
                        )
                        .into());
                    }
                    Err(error) => error,
                };
            assert!(matches!(
                jpeg_sequence_limit_error,
                ImageError::LimitExceeded {
                    format: Some(ImageFormat::Jpeg),
                    operation: image_slash_star::CodecOperation::SequenceEncode,
                    resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
                    ..
                }
            ));
            assert_eq!(limited_jpeg_sequence.writes, 0);
            assert!(limited_jpeg_sequence.bytes.is_empty());

            let mut multiple_jpeg_sequence = jpeg_sequence.clone();
            multiple_jpeg_sequence
                .frames
                .push(multiple_jpeg_sequence.frames[0].clone());
            multiple_jpeg_sequence.kind = SequenceKind::TimedAnimation;
            let mut multiple_jpeg_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let multiple_jpeg_error = match image_slash_star::encode_sequence_to_sink(
                &multiple_jpeg_sequence,
                ImageFormat::Jpeg,
                &jpeg_options,
                &mut multiple_jpeg_sink,
            ) {
                Ok(length) => {
                    return Err(format!(
                        "multi-frame JPEG sequence unexpectedly accepted {length} bytes"
                    )
                    .into());
                }
                Err(error) => error,
            };
            assert_eq!(
                multiple_jpeg_error.kind(),
                image_slash_star::ImageErrorKind::Unsupported
            );
            assert_eq!(
                multiple_jpeg_error.unsupported_reason(),
                Some(UnsupportedReason::NotImplemented)
            );
            assert_eq!(
                multiple_jpeg_error.stage(),
                Some(ImageErrorStage::SequenceEncode)
            );
            assert_eq!(multiple_jpeg_sink.writes, 0);
            assert!(multiple_jpeg_sink.bytes.is_empty());

            let progressive_options = match EncodeOptions::for_format(ImageFormat::Jpeg) {
                EncodeOptions::Jpeg(mut options) => {
                    options.progressive = Some(true);
                    EncodeOptions::Jpeg(options)
                }
                _ => unreachable!("JPEG options must name JPEG"),
            };
            let progressive_expected =
                image_slash_star::encode(&jpeg_image, ImageFormat::Jpeg, &progressive_options)?;
            let mut progressive_sink = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            assert_eq!(
                image_slash_star::encode_to_sink(
                    &jpeg_image,
                    ImageFormat::Jpeg,
                    &progressive_options,
                    &mut progressive_sink,
                )?,
                progressive_expected.len()
            );
            assert_eq!(progressive_sink.bytes, progressive_expected);
            assert!(
                progressive_sink.writes > 1,
                "progressive JPEG must cross scan boundaries"
            );
            assert!(
                progressive_expected
                    .windows(2)
                    .any(|marker| marker == b"\xff\xc2"),
                "progressive JPEG must contain SOF2"
            );

            let jpeg_cancel_token = image_slash_star::CancellationToken::new();
            let mut cancelling_jpeg = CancellingSink {
                bytes: Vec::new(),
                token: jpeg_cancel_token.clone(),
                writes: 0,
            };
            let jpeg_cancel_error = match image_slash_star::encode_to_sink_with_token(
                &jpeg_image,
                ImageFormat::Jpeg,
                &jpeg_options,
                &jpeg_cancel_token,
                &mut cancelling_jpeg,
            ) {
                Ok(length) => {
                    return Err(format!(
                        "JPEG sink cancellation unexpectedly wrote {length} bytes"
                    )
                    .into());
                }
                Err(error) => error,
            };
            assert_eq!(
                jpeg_cancel_error.kind(),
                image_slash_star::ImageErrorKind::Cancelled
            );
            assert_eq!(jpeg_cancel_error.format(), Some(ImageFormat::Jpeg));
            assert_eq!(
                jpeg_cancel_error.stage(),
                Some(ImageErrorStage::StillEncode)
            );
            assert_eq!(cancelling_jpeg.writes, 1);
            assert_eq!(cancelling_jpeg.bytes, b"\xff\xd8");

            let too_small_jpeg = EncodePolicy::default()
                .with_max_output_bytes(u64::try_from(jpeg_expected.len() - 1)?);
            let mut limited_jpeg = RecordingSink {
                bytes: Vec::new(),
                writes: 0,
            };
            let jpeg_limit_error = match image_slash_star::encode_to_sink_with_policy(
                &jpeg_image,
                ImageFormat::Jpeg,
                &jpeg_options,
                &too_small_jpeg,
                &mut limited_jpeg,
            ) {
                Ok(length) => {
                    return Err(
                        format!("JPEG output policy unexpectedly admitted {length} bytes").into(),
                    );
                }
                Err(error) => error,
            };
            assert!(matches!(
                jpeg_limit_error,
                ImageError::LimitExceeded {
                    format: Some(ImageFormat::Jpeg),
                    operation: image_slash_star::CodecOperation::StillEncode,
                    resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
                    ..
                }
            ));
            assert_eq!(limited_jpeg.writes, 0);
            assert!(limited_jpeg.bytes.is_empty());

            let mut failing_jpeg = FailingAfterWrites {
                fail_at: 2,
                writes: 0,
            };
            let jpeg_write_error = match image_slash_star::encode_to_sink(
                &jpeg_image,
                ImageFormat::Jpeg,
                &jpeg_options,
                &mut failing_jpeg,
            ) {
                Ok(length) => {
                    return Err(format!("JPEG sink unexpectedly accepted {length} bytes").into());
                }
                Err(error) => error,
            };
            assert_eq!(
                jpeg_write_error.kind(),
                image_slash_star::ImageErrorKind::OutputWrite
            );
            assert_eq!(jpeg_write_error.format(), Some(ImageFormat::Jpeg));
            assert_eq!(jpeg_write_error.stage(), Some(ImageErrorStage::StillEncode));
            assert_eq!(failing_jpeg.writes, 2);

            // The structural writer must preserve invalid still-input errors
            // before touching its sink. These are Rust-owned API/error
            // contracts, not Pillow parity rows.
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
                    return Err(format!(
                        "invalid JPEG structural sink unexpectedly wrote {length} bytes"
                    )
                    .into());
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
                        "invalid token JPEG structural sink unexpectedly wrote {length} bytes"
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
        // GIF still delivery now splits the validated header, palette,
        // extensions, image blocks, and trailer while retaining the complete
        // encoder working buffer. This is a Rust-only sink contract: Pillow
        // has no caller-owned sink, so the parity matrix remains unchanged.
        let gif_image = image_slash_star::DecodedImage::new(1, 1, vec![0, 0, 0], ColorType::Rgb8);
        let gif_options = EncodeOptions::for_format(ImageFormat::Gif);
        let gif_expected = image_slash_star::encode(&gif_image, ImageFormat::Gif, &gif_options)?;
        let mut gif_sink = RecordingSink {
            bytes: Vec::new(),
            writes: 0,
        };
        assert_eq!(
            image_slash_star::encode_to_sink(
                &gif_image,
                ImageFormat::Gif,
                &gif_options,
                &mut gif_sink,
            )?,
            gif_expected.len()
        );
        assert_eq!(gif_sink.bytes, gif_expected);
        assert!(
            gif_sink.writes > 1,
            "GIF output must cross block boundaries"
        );
        assert!(gif_sink.bytes.starts_with(b"GIF87a") || gif_sink.bytes.starts_with(b"GIF89a"));

        let gif_token = image_slash_star::CancellationToken::new();
        let mut cancelling_gif = CancellingSink {
            bytes: Vec::new(),
            token: gif_token.clone(),
            writes: 0,
        };
        let cancelling_gif_error = match image_slash_star::encode_to_sink_with_token(
            &gif_image,
            ImageFormat::Gif,
            &gif_options,
            &gif_token,
            &mut cancelling_gif,
        ) {
            Ok(length) => {
                return Err(
                    format!("GIF sink cancellation unexpectedly wrote {length} bytes").into(),
                );
            }
            Err(error) => error,
        };
        assert_eq!(
            cancelling_gif_error.kind(),
            image_slash_star::ImageErrorKind::Cancelled
        );
        assert_eq!(cancelling_gif_error.format(), Some(ImageFormat::Gif));
        assert_eq!(
            cancelling_gif_error.stage(),
            Some(ImageErrorStage::StillEncode)
        );
        assert_eq!(cancelling_gif.writes, 1);
        assert_eq!(cancelling_gif.bytes, gif_expected[..13].to_vec());

        let too_small_gif =
            EncodePolicy::default().with_max_output_bytes(u64::try_from(gif_expected.len() - 1)?);
        let mut limited_gif = RecordingSink {
            bytes: Vec::new(),
            writes: 0,
        };
        let limited_gif_error = match image_slash_star::encode_to_sink_with_policy(
            &gif_image,
            ImageFormat::Gif,
            &gif_options,
            &too_small_gif,
            &mut limited_gif,
        ) {
            Ok(length) => {
                return Err(
                    format!("GIF output policy unexpectedly admitted {length} bytes").into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            limited_gif_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
                ..
            }
        ));
        assert_eq!(limited_gif.writes, 0);
        assert!(limited_gif.bytes.is_empty());

        let mut failing_gif = FailingAfterWrites {
            fail_at: 2,
            writes: 0,
        };
        let failing_gif_error = match image_slash_star::encode_to_sink(
            &gif_image,
            ImageFormat::Gif,
            &gif_options,
            &mut failing_gif,
        ) {
            Ok(length) => {
                return Err(format!("GIF sink unexpectedly accepted {length} bytes").into());
            }
            Err(error) => error,
        };
        assert_eq!(
            failing_gif_error.kind(),
            image_slash_star::ImageErrorKind::OutputWrite
        );
        assert_eq!(failing_gif_error.format(), Some(ImageFormat::Gif));
        assert_eq!(
            failing_gif_error.stage(),
            Some(ImageErrorStage::StillEncode)
        );
        assert_eq!(failing_gif.writes, 2);

        let data = fs::read(root.join("tests/fixtures/input/images/gif/animated_3frame.gif"))?;
        let sequence = image_slash_star::decode_sequence(&data)?.into_inner();
        let options = EncodeOptions::for_format(ImageFormat::Gif);
        let expected = image_slash_star::encode_sequence(&sequence, ImageFormat::Gif, &options)?;
        let mut sink = RecordingSink {
            bytes: Vec::new(),
            writes: 0,
        };
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
        assert_eq!(sink.bytes, expected, "sequence sink bytes");
        assert!(sink.writes > 1, "GIF sequence must cross block boundaries");

        let sequence_token = image_slash_star::CancellationToken::new();
        let mut cancelling_sequence = CancellingSink {
            bytes: Vec::new(),
            token: sequence_token.clone(),
            writes: 0,
        };
        let cancellation_error = match image_slash_star::encode_sequence_to_sink_with_token(
            &sequence,
            ImageFormat::Gif,
            &options,
            &sequence_token,
            &mut cancelling_sequence,
        ) {
            Ok(length) => {
                return Err(format!(
                    "GIF sequence sink cancellation unexpectedly wrote {length} bytes"
                )
                .into());
            }
            Err(error) => error,
        };
        assert_eq!(
            cancellation_error.kind(),
            image_slash_star::ImageErrorKind::Cancelled
        );
        assert_eq!(cancellation_error.format(), Some(ImageFormat::Gif));
        assert_eq!(
            cancellation_error.stage(),
            Some(ImageErrorStage::SequenceEncode)
        );
        assert_eq!(cancelling_sequence.writes, 1);
        assert_eq!(cancelling_sequence.bytes, expected[..13].to_vec());

        let too_small_sequence =
            EncodePolicy::default().with_max_output_bytes(u64::try_from(expected.len() - 1)?);
        let mut limited_sequence = RecordingSink {
            bytes: Vec::new(),
            writes: 0,
        };
        let limited_error = match image_slash_star::encode_sequence_to_sink_with_policy(
            &sequence,
            ImageFormat::Gif,
            &options,
            &too_small_sequence,
            &mut limited_sequence,
        ) {
            Ok(length) => {
                return Err(format!(
                    "GIF sequence output policy unexpectedly admitted {length} bytes"
                )
                .into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            limited_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::SequenceEncode,
                resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
                ..
            }
        ));
        assert_eq!(limited_sequence.writes, 0);
        assert!(limited_sequence.bytes.is_empty());

        let mut failing_later = FailingAfterWrites {
            fail_at: 2,
            writes: 0,
        };
        let later_write_error = match image_slash_star::encode_sequence_to_sink(
            &sequence,
            ImageFormat::Gif,
            &options,
            &mut failing_later,
        ) {
            Ok(length) => {
                return Err(
                    format!("GIF sequence sink unexpectedly accepted {length} bytes").into(),
                );
            }
            Err(error) => error,
        };
        assert_eq!(
            later_write_error.kind(),
            image_slash_star::ImageErrorKind::OutputWrite
        );
        assert_eq!(later_write_error.format(), Some(ImageFormat::Gif));
        assert_eq!(
            later_write_error.stage(),
            Some(ImageErrorStage::SequenceEncode)
        );
        assert_eq!(failing_later.writes, 2);

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

    if cfg!(feature = "avif") && !cfg!(target_arch = "wasm32") {
        // Native AVIF top-level ISO-BMFF delivery is a Rust-only structural
        // sink contract. Pillow has no caller-owned sink, so this adds no
        // parity row or fixture and retains the complete native output buffer.
        let avif_image = image_slash_star::DecodedImage::new(1, 1, vec![0, 0, 0], ColorType::Rgb8);
        let mut avif_options = EncodeOptions::for_format(ImageFormat::Avif);
        if let EncodeOptions::Avif(options) = &mut avif_options {
            // This native-only structural test invokes the encoder repeatedly
            // to exercise sink, cancellation, policy, and write-failure paths.
            // One worker keeps those comparisons deterministic and avoids
            // spawning a large codec thread pool for a two-frame fixture.
            options.max_threads = Some(1);
        }
        let avif_expected =
            image_slash_star::encode(&avif_image, ImageFormat::Avif, &avif_options)?;
        assert!(
            avif_expected.len() >= 8,
            "AVIF output must contain one box header"
        );
        let mut avif_sink = RecordingSink {
            bytes: Vec::new(),
            writes: 0,
        };
        assert_eq!(
            image_slash_star::encode_to_sink(
                &avif_image,
                ImageFormat::Avif,
                &avif_options,
                &mut avif_sink,
            )?,
            avif_expected.len()
        );
        assert_eq!(avif_sink.bytes, avif_expected);
        assert!(
            avif_sink.writes > 1,
            "AVIF output must cross box boundaries"
        );

        let avif_token = image_slash_star::CancellationToken::new();
        let mut cancelling_avif = CancellingSink {
            bytes: Vec::new(),
            token: avif_token.clone(),
            writes: 0,
        };
        let avif_cancel_error = match image_slash_star::encode_to_sink_with_token(
            &avif_image,
            ImageFormat::Avif,
            &avif_options,
            &avif_token,
            &mut cancelling_avif,
        ) {
            Ok(length) => {
                return Err(
                    format!("AVIF sink cancellation unexpectedly wrote {length} bytes").into(),
                );
            }
            Err(error) => error,
        };
        assert_eq!(
            avif_cancel_error.kind(),
            image_slash_star::ImageErrorKind::Cancelled
        );
        assert_eq!(avif_cancel_error.format(), Some(ImageFormat::Avif));
        assert_eq!(
            avif_cancel_error.stage(),
            Some(ImageErrorStage::StillEncode)
        );
        assert_eq!(cancelling_avif.writes, 1);
        assert_eq!(cancelling_avif.bytes, avif_expected[..8].to_vec());

        let too_small_avif =
            EncodePolicy::default().with_max_output_bytes(u64::try_from(avif_expected.len() - 1)?);
        let mut limited_avif = RecordingSink {
            bytes: Vec::new(),
            writes: 0,
        };
        let avif_limit_error = match image_slash_star::encode_to_sink_with_policy(
            &avif_image,
            ImageFormat::Avif,
            &avif_options,
            &too_small_avif,
            &mut limited_avif,
        ) {
            Ok(length) => {
                return Err(
                    format!("AVIF output policy unexpectedly admitted {length} bytes").into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            avif_limit_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Avif),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
                ..
            }
        ));
        assert_eq!(limited_avif.writes, 0);
        assert!(limited_avif.bytes.is_empty());

        let mut failing_avif = FailingAfterWrites {
            fail_at: 2,
            writes: 0,
        };
        let avif_write_error = match image_slash_star::encode_to_sink(
            &avif_image,
            ImageFormat::Avif,
            &avif_options,
            &mut failing_avif,
        ) {
            Ok(length) => {
                return Err(format!("AVIF sink unexpectedly accepted {length} bytes").into());
            }
            Err(error) => error,
        };
        assert_eq!(
            avif_write_error.kind(),
            image_slash_star::ImageErrorKind::OutputWrite
        );
        assert_eq!(avif_write_error.format(), Some(ImageFormat::Avif));
        assert_eq!(avif_write_error.stage(), Some(ImageErrorStage::StillEncode));
        assert_eq!(failing_avif.writes, 2);

        // Native AVIF sequence top-level ISO-BMFF delivery is the same
        // Rust-only structural sink contract at SequenceEncode stage. The
        // Pillow parity matrix has no caller-owned sink, so this adds no row.
        let animated_avif = fs::read(root.join("tests/fixtures/input/images/avif/animated.avif"))?;
        let avif_sequence = image_slash_star::decode_sequence(&animated_avif)?.into_inner();
        assert!(avif_sequence.frames.len() > 1);
        let avif_sequence_expected =
            image_slash_star::encode_sequence(&avif_sequence, ImageFormat::Avif, &avif_options)?;
        assert!(avif_sequence_expected.len() >= 8);
        let mut avif_sequence_sink = RecordingSink {
            bytes: Vec::new(),
            writes: 0,
        };
        assert_eq!(
            image_slash_star::encode_sequence_to_sink(
                &avif_sequence,
                ImageFormat::Avif,
                &avif_options,
                &mut avif_sequence_sink,
            )?,
            avif_sequence_expected.len()
        );
        assert_eq!(avif_sequence_sink.bytes, avif_sequence_expected);
        assert!(
            avif_sequence_sink.writes > 1,
            "AVIF sequence output must cross box boundaries"
        );

        let avif_sequence_token = image_slash_star::CancellationToken::new();
        let mut cancelling_avif_sequence = CancellingSink {
            bytes: Vec::new(),
            token: avif_sequence_token.clone(),
            writes: 0,
        };
        let avif_sequence_cancel_error = match image_slash_star::encode_sequence_to_sink_with_token(
            &avif_sequence,
            ImageFormat::Avif,
            &avif_options,
            &avif_sequence_token,
            &mut cancelling_avif_sequence,
        ) {
            Ok(length) => {
                return Err(format!(
                    "AVIF sequence sink cancellation unexpectedly wrote {length} bytes"
                )
                .into());
            }
            Err(error) => error,
        };
        assert_eq!(
            avif_sequence_cancel_error.kind(),
            image_slash_star::ImageErrorKind::Cancelled
        );
        assert_eq!(avif_sequence_cancel_error.format(), Some(ImageFormat::Avif));
        assert_eq!(
            avif_sequence_cancel_error.stage(),
            Some(ImageErrorStage::SequenceEncode)
        );
        assert_eq!(cancelling_avif_sequence.writes, 1);
        assert_eq!(
            cancelling_avif_sequence.bytes,
            avif_sequence_expected[..8].to_vec()
        );

        let too_small_avif_sequence = EncodePolicy::default()
            .with_max_output_bytes(u64::try_from(avif_sequence_expected.len() - 1)?);
        let mut limited_avif_sequence = RecordingSink {
            bytes: Vec::new(),
            writes: 0,
        };
        let avif_sequence_limit_error = match image_slash_star::encode_sequence_to_sink_with_policy(
            &avif_sequence,
            ImageFormat::Avif,
            &avif_options,
            &too_small_avif_sequence,
            &mut limited_avif_sequence,
        ) {
            Ok(length) => {
                return Err(format!(
                    "AVIF sequence output policy unexpectedly admitted {length} bytes"
                )
                .into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            avif_sequence_limit_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Avif),
                operation: image_slash_star::CodecOperation::SequenceEncode,
                resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
                ..
            }
        ));
        assert_eq!(limited_avif_sequence.writes, 0);
        assert!(limited_avif_sequence.bytes.is_empty());

        let mut failing_avif_sequence = FailingAfterWrites {
            fail_at: 2,
            writes: 0,
        };
        let avif_sequence_write_error = match image_slash_star::encode_sequence_to_sink(
            &avif_sequence,
            ImageFormat::Avif,
            &avif_options,
            &mut failing_avif_sequence,
        ) {
            Ok(length) => {
                return Err(
                    format!("AVIF sequence sink unexpectedly accepted {length} bytes").into(),
                );
            }
            Err(error) => error,
        };
        assert_eq!(
            avif_sequence_write_error.kind(),
            image_slash_star::ImageErrorKind::OutputWrite
        );
        assert_eq!(avif_sequence_write_error.format(), Some(ImageFormat::Avif));
        assert_eq!(
            avif_sequence_write_error.stage(),
            Some(ImageErrorStage::SequenceEncode)
        );
        assert_eq!(failing_avif_sequence.writes, 2);
    }
    Ok(())
}

#[test]
fn partial_structural_sink_write_preserves_prefix_without_flush()
-> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(feature = "png") {
        return Ok(());
    }

    // Pillow has no caller-owned OutputSink. This real destination contract is
    // Rust-only evidence and must not become a synthetic parity or coverage
    // row.

    struct PartialWriteSink {
        bytes: Vec<u8>,
        writes: usize,
        flushes: usize,
    }

    impl image_slash_star::OutputSink for PartialWriteSink {
        fn write_all(&mut self, bytes: &[u8]) -> image_slash_star::ImageResult<()> {
            self.writes += 1;
            if self.writes == 2 {
                let accepted = 5.min(bytes.len());
                self.bytes.extend_from_slice(&bytes[..accepted]);
                return Err(image_slash_star::ImageError::Unsupported {
                    format: None,
                    message: "sink accepted a short prefix".to_owned(),
                    stage: None,
                    reason: None,
                    offset: None,
                    identity: None,
                });
            }
            self.bytes.extend_from_slice(bytes);
            Ok(())
        }

        fn flush(&mut self) -> image_slash_star::ImageResult<()> {
            self.flushes += 1;
            Ok(())
        }
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = fs::read(root.join("tests/fixtures/input/images/png/1x1.png"))?;
    let decoded = image_slash_star::decode(&data)?;
    let options = image_slash_star::EncodeOptions::for_format(ImageFormat::Png);
    let expected = image_slash_star::encode(&decoded.content, ImageFormat::Png, &options)?;
    let mut sink = PartialWriteSink {
        bytes: Vec::new(),
        writes: 0,
        flushes: 0,
    };

    let error = match image_slash_star::encode_to_sink(
        &decoded.content,
        ImageFormat::Png,
        &options,
        &mut sink,
    ) {
        Ok(length) => {
            return Err(format!("short-writing sink unexpectedly accepted {length} bytes").into());
        }
        Err(error) => error,
    };

    assert_eq!(error.kind(), image_slash_star::ImageErrorKind::OutputWrite);
    assert_eq!(error.format(), Some(ImageFormat::Png));
    assert_eq!(error.stage(), Some(ImageErrorStage::StillEncode));
    assert_eq!(
        sink.writes, 2,
        "the error must occur at a structural boundary"
    );
    assert_eq!(sink.flushes, 0, "failed delivery must not be finalized");
    assert_eq!(
        sink.bytes.len(),
        13,
        "8-byte signature plus a short 5-byte prefix"
    );
    assert_eq!(sink.bytes, expected[..sink.bytes.len()]);
    assert_eq!(&sink.bytes[..8], b"\x89PNG\r\n\x1a\n");
    Ok(())
}

#[test]
fn partial_structural_sink_write_preserves_prefix_across_available_encoders()
-> Result<(), Box<dyn std::error::Error>> {
    // Pillow has no caller-owned OutputSink. This loops over the real still
    // and supported multi-frame sequence writers available in each
    // feature/target lane, so it is Rust-only contract evidence rather than a
    // synthetic parity or coverage row.
    struct PartialWriteSink {
        bytes: Vec<u8>,
        writes: usize,
        flushes: usize,
        failed_segment_len: usize,
        accepted_prefix_len: usize,
    }

    impl image_slash_star::OutputSink for PartialWriteSink {
        fn write_all(&mut self, bytes: &[u8]) -> image_slash_star::ImageResult<()> {
            self.writes += 1;
            if self.writes == 2 {
                self.failed_segment_len = bytes.len();
                let accepted = (bytes.len() / 2).max(1);
                self.accepted_prefix_len = accepted;
                self.bytes.extend_from_slice(&bytes[..accepted]);
                return Err(image_slash_star::ImageError::Unsupported {
                    format: None,
                    message: "sink accepted a partial structural prefix".to_owned(),
                    stage: None,
                    reason: None,
                    offset: None,
                    identity: None,
                });
            }
            self.bytes.extend_from_slice(bytes);
            Ok(())
        }

        fn flush(&mut self) -> image_slash_star::ImageResult<()> {
            self.flushes += 1;
            Ok(())
        }
    }

    let image = image_slash_star::DecodedImage::new(1, 1, vec![0, 0, 0], ColorType::Rgb8);
    let formats = [
        ImageFormat::Jpeg,
        ImageFormat::Png,
        ImageFormat::Gif,
        ImageFormat::Bmp,
        ImageFormat::Tiff,
        ImageFormat::WebP,
        ImageFormat::Ico,
        ImageFormat::Avif,
    ];
    for format in formats {
        if !format.capabilities().still_encode().is_available() {
            continue;
        }

        let mut options = EncodeOptions::for_format(format);
        if format == ImageFormat::Avif
            && let EncodeOptions::Avif(avif_options) = &mut options
        {
            // Native AVIF comparisons are intentionally single-worker so
            // concurrent contract tests cannot perturb byte identity.
            avif_options.max_threads = Some(1);
        }
        let expected = image_slash_star::encode(&image, format, &options)?;
        let mut sink = PartialWriteSink {
            bytes: Vec::new(),
            writes: 0,
            flushes: 0,
            failed_segment_len: 0,
            accepted_prefix_len: 0,
        };
        let error = match image_slash_star::encode_to_sink(&image, format, &options, &mut sink) {
            Ok(length) => {
                return Err(format!(
                    "{format:?} accepted a partial sink write and returned {length} bytes"
                )
                .into());
            }
            Err(error) => error,
        };

        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::OutputWrite);
        assert_eq!(error.format(), Some(format));
        assert_eq!(error.stage(), Some(ImageErrorStage::StillEncode));
        assert_eq!(sink.writes, 2, "{format:?} must reject at a later segment");
        assert_eq!(
            sink.flushes, 0,
            "{format:?} must not finalize failed delivery"
        );
        assert!(sink.failed_segment_len > sink.accepted_prefix_len);
        assert!(!sink.bytes.is_empty(), "{format:?} must deliver a prefix");
        assert!(sink.bytes.len() < expected.len());
        assert_eq!(sink.bytes, expected[..sink.bytes.len()]);
    }

    let sequence_formats = [
        ImageFormat::Gif,
        ImageFormat::Tiff,
        ImageFormat::WebP,
        ImageFormat::Avif,
    ];
    for format in sequence_formats {
        if !format.capabilities().sequence_encode().is_available() {
            continue;
        }

        let mut sequence = image_slash_star::DecodedSequence::from_image(image.clone());
        sequence.frames.push(sequence.frames[0].clone());
        sequence.kind = if format == ImageFormat::Tiff {
            image_slash_star::SequenceKind::UntimedPages
        } else {
            image_slash_star::SequenceKind::TimedAnimation
        };
        let mut options = EncodeOptions::for_format(format);
        if format == ImageFormat::Avif
            && let EncodeOptions::Avif(avif_options) = &mut options
        {
            // Keep sequence byte identity deterministic beside the native
            // AVIF sequence tests in the same feature-gate process.
            avif_options.max_threads = Some(1);
        }
        let expected = image_slash_star::encode_sequence(&sequence, format, &options)?;
        let mut sink = PartialWriteSink {
            bytes: Vec::new(),
            writes: 0,
            flushes: 0,
            failed_segment_len: 0,
            accepted_prefix_len: 0,
        };
        let error =
            match image_slash_star::encode_sequence_to_sink(&sequence, format, &options, &mut sink)
            {
                Ok(length) => {
                    return Err(format!(
                    "{format:?} sequence accepted a partial sink write and returned {length} bytes"
                )
                .into());
                }
                Err(error) => error,
            };

        assert_eq!(error.kind(), image_slash_star::ImageErrorKind::OutputWrite);
        assert_eq!(error.format(), Some(format));
        assert_eq!(error.stage(), Some(ImageErrorStage::SequenceEncode));
        assert_eq!(
            sink.writes, 2,
            "{format:?} sequence must reject at a later segment"
        );
        assert_eq!(
            sink.flushes, 0,
            "{format:?} sequence must not finalize failed delivery"
        );
        assert!(sink.failed_segment_len > sink.accepted_prefix_len);
        assert!(
            !sink.bytes.is_empty(),
            "{format:?} sequence must deliver a prefix"
        );
        assert!(sink.bytes.len() < expected.len());
        assert_eq!(sink.bytes, expected[..sink.bytes.len()]);
    }

    // Feature-matrix lanes with no encoder are compile/capability lanes; the
    // all-feature and default lanes exercise the full available writer set.
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
fn encode_work_budget_is_a_non_parity_result_contract() -> Result<(), Box<dyn std::error::Error>> {
    // Pillow has no caller-controlled checkpoint budget or equivalent result.
    // This is a Rust-only work-control contract, not a generated parity row.
    if cfg!(feature = "png") {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let data = fs::read(root.join("tests/fixtures/input/images/png/1x1.png"))?;
        let decoded = image_slash_star::decode(&data)?;
        let options = EncodeOptions::for_format(ImageFormat::Png);
        let unlimited = image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX);
        assert_eq!(unlimited.max_work_units(), Some(u64::MAX));
        let expected = image_slash_star::encode(&decoded.content, ImageFormat::Png, &options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &decoded.content,
                ImageFormat::Png,
                &options,
                &unlimited,
            )?,
            expected,
            "an ample checkpoint budget preserves the ordinary result"
        );
        let mut stored_options = image_slash_star::PngEncodeOptions::default();
        stored_options.compression = Some(image_slash_star::PngCompression::None);
        let stored_options = EncodeOptions::from(stored_options);
        let stored_expected =
            image_slash_star::encode(&decoded.content, ImageFormat::Png, &stored_options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &decoded.content,
                ImageFormat::Png,
                &stored_options,
                &unlimited,
            )?,
            stored_expected,
            "an ample budget preserves PNG stored-block bytes"
        );
        let stored_interior_image =
            DecodedImage::new(11_000, 1, vec![0; 11_000 * 3], ColorType::Rgb8);
        let stored_interior_expected =
            image_slash_star::encode(&stored_interior_image, ImageFormat::Png, &stored_options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &stored_interior_image,
                ImageFormat::Png,
                &stored_options,
                &unlimited,
            )?,
            stored_interior_expected,
            "an ample budget preserves PNG non-final stored blocks"
        );
        let stored_copy_policy = image_slash_star::EncodePolicy::new().with_max_work_units(164);
        let stored_copy_error = match image_slash_star::encode_with_policy(
            &stored_interior_image,
            ImageFormat::Png,
            &stored_options,
            &stored_copy_policy,
        ) {
            Ok(_) => return Err("PNG stored-block copy budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            stored_copy_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Png),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 164,
                observed: 165,
            }
        ));
        let mut stored_copy_sink = vec![0xAA];
        let stored_copy_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &stored_interior_image,
            ImageFormat::Png,
            &stored_options,
            &stored_copy_policy,
            &mut stored_copy_sink,
        ) {
            Ok(_) => return Err("PNG stored-block copy budget wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            stored_copy_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Png),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 164,
                observed: 165,
            }
        ));
        assert_eq!(
            stored_copy_sink,
            vec![0xAA],
            "PNG stored-block copy rejection happens before sink delivery"
        );
        let mut maximum_options = image_slash_star::PngEncodeOptions::default();
        maximum_options.compression = Some(image_slash_star::PngCompression::Maximum);
        let maximum_options = EncodeOptions::from(maximum_options);
        let maximum_expected =
            image_slash_star::encode(&decoded.content, ImageFormat::Png, &maximum_options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &decoded.content,
                ImageFormat::Png,
                &maximum_options,
                &unlimited,
            )?,
            maximum_expected,
            "an ample budget preserves PNG non-default level bytes"
        );
        let caller_token = image_slash_star::CancellationToken::new();
        assert_eq!(
            image_slash_star::encode_with_token_and_policy(
                &decoded.content,
                ImageFormat::Png,
                &options,
                &unlimited,
                &caller_token,
            )?,
            expected,
            "a budget layered over a caller token preserves the ordinary result"
        );

        let zero = image_slash_star::EncodePolicy::new().with_max_work_units(0);
        let error = match image_slash_star::encode_with_policy(
            &decoded.content,
            ImageFormat::Png,
            &options,
            &zero,
        ) {
            Ok(_) => return Err("zero work budget unexpectedly encoded PNG".into()),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Png),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 0,
                observed: 1,
            }
        ));

        let mut sink = vec![0xA5];
        let sink_error = match image_slash_star::encode_to_sink_with_policy(
            &decoded.content,
            ImageFormat::Png,
            &options,
            &zero,
            &mut sink,
        ) {
            Ok(_) => return Err("zero work budget unexpectedly wrote PNG".into()),
            Err(error) => error,
        };
        assert!(matches!(
            sink_error,
            ImageError::LimitExceeded {
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                operation: image_slash_star::CodecOperation::StillEncode,
                ..
            }
        ));
        assert_eq!(sink, vec![0xA5]);

        let cancelled = image_slash_star::CancellationToken::new();
        cancelled.cancel();
        assert!(matches!(
            image_slash_star::encode_with_token_and_policy(
                &decoded.content,
                ImageFormat::Png,
                &options,
                &zero,
                &cancelled,
            ),
            Err(ImageError::Cancelled { .. })
        ));
        let mut cancelled_sink = vec![0xA6];
        assert!(matches!(
            image_slash_star::encode_to_sink_with_token_and_policy(
                &decoded.content,
                ImageFormat::Png,
                &options,
                &zero,
                &cancelled,
                &mut cancelled_sink,
            ),
            Err(ImageError::Cancelled { .. })
        ));
        assert_eq!(
            cancelled_sink,
            vec![0xA6],
            "caller cancellation precedes work-budget delivery"
        );

        // A long adaptive-filter row now charges a deterministic interior
        // checkpoint after each 1,024 filtered bytes. Pillow has no caller
        // token or work-budget result, so this remains Rust-only evidence.
        let interior_image = DecodedImage::new(1_024, 1, vec![0; 1_024 * 3], ColorType::Rgb8);
        let interior_policy = image_slash_star::EncodePolicy::new().with_max_work_units(3);
        let interior_error = match image_slash_star::encode_with_policy(
            &interior_image,
            ImageFormat::Png,
            &options,
            &interior_policy,
        ) {
            Ok(_) => return Err("PNG interior work budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            interior_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Png),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 3,
                observed: 4,
            }
        ));
        let mut interior_sink = vec![0xA7];
        let interior_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &interior_image,
            ImageFormat::Png,
            &options,
            &interior_policy,
            &mut interior_sink,
        ) {
            Ok(_) => return Err("PNG interior budget unexpectedly wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            interior_sink_error,
            ImageError::LimitExceeded {
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 3,
                observed: 4,
                ..
            }
        ));
        assert_eq!(interior_sink, vec![0xA7]);

        // The default PNG level-six DEFLATE path now remains interruptible
        // after filtering. Its first matcher checkpoint is deterministic for
        // this probe, while Pillow has no caller budget or equivalent result.
        let deflate_policy = image_slash_star::EncodePolicy::new().with_max_work_units(20);
        let deflate_error = match image_slash_star::encode_with_policy(
            &interior_image,
            ImageFormat::Png,
            &options,
            &deflate_policy,
        ) {
            Ok(_) => return Err("PNG DEFLATE work budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            deflate_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Png),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 20,
                observed: 21,
            }
        ));
        let mut deflate_sink = vec![0xA8];
        let deflate_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &interior_image,
            ImageFormat::Png,
            &options,
            &deflate_policy,
            &mut deflate_sink,
        ) {
            Ok(_) => return Err("PNG DEFLATE budget unexpectedly wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            deflate_sink_error,
            ImageError::LimitExceeded {
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 20,
                observed: 21,
                ..
            }
        ));
        assert_eq!(deflate_sink, vec![0xA8]);

        // The remaining zlib-ng levels now use the same token-aware matcher,
        // emission, and checksum controller. Pillow has no caller budget or
        // equivalent result, so these are Rust-only contract probes.
        for level in [1, 2, 3, 4, 5, 7, 8, 9] {
            let mut level_options = image_slash_star::PngEncodeOptions::default();
            level_options.compression = Some(image_slash_star::PngCompression::Level(level));
            let level_options = EncodeOptions::from(level_options);
            let level_expected =
                image_slash_star::encode(&interior_image, ImageFormat::Png, &level_options)?;
            assert_eq!(
                image_slash_star::encode_with_policy(
                    &interior_image,
                    ImageFormat::Png,
                    &level_options,
                    &unlimited,
                )?,
                level_expected,
                "an ample budget preserves PNG level-{level} bytes"
            );

            let level_error = match image_slash_star::encode_with_policy(
                &interior_image,
                ImageFormat::Png,
                &level_options,
                &deflate_policy,
            ) {
                Ok(_) => {
                    return Err(format!(
                        "PNG level-{level} Deflate work budget unexpectedly completed"
                    )
                    .into());
                }
                Err(error) => error,
            };
            assert!(matches!(
                level_error,
                ImageError::LimitExceeded {
                    format: Some(ImageFormat::Png),
                    operation: image_slash_star::CodecOperation::StillEncode,
                    resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                    maximum: 20,
                    observed,
                } if observed > 20
            ));

            let mut level_sink = vec![0xA9];
            let level_sink_error = match image_slash_star::encode_to_sink_with_policy(
                &interior_image,
                ImageFormat::Png,
                &level_options,
                &deflate_policy,
                &mut level_sink,
            ) {
                Ok(_) => {
                    return Err(format!(
                        "PNG level-{level} Deflate budget unexpectedly wrote output"
                    )
                    .into());
                }
                Err(error) => error,
            };
            assert!(matches!(
                level_sink_error,
                ImageError::LimitExceeded {
                    format: Some(ImageFormat::Png),
                    operation: image_slash_star::CodecOperation::StillEncode,
                    resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                    maximum: 20,
                    observed,
                } if observed > 20
            ));
            assert_eq!(level_sink, vec![0xA9]);
        }
    }

    if cfg!(feature = "jpeg") {
        // Pillow has no caller-controlled checkpoint budget or equivalent
        // result. This is a Rust-only work-control contract, including the
        // pre-write policy boundary for the generic whole-buffer sink path.
        let image = DecodedImage::new(17, 17, vec![128; 17 * 17 * 3], ColorType::Rgb8);
        let options = EncodeOptions::for_format(ImageFormat::Jpeg);
        let unlimited = image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX);
        let expected = image_slash_star::encode(&image, ImageFormat::Jpeg, &options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(&image, ImageFormat::Jpeg, &options, &unlimited,)?,
            expected,
            "an ample JPEG checkpoint budget preserves byte identity"
        );

        let bounded = image_slash_star::EncodePolicy::new().with_max_work_units(1);
        let error = match image_slash_star::encode_with_policy(
            &image,
            ImageFormat::Jpeg,
            &options,
            &bounded,
        ) {
            Ok(_) => return Err("bounded JPEG work budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 1,
                observed,
            } if observed > 1
        ));

        let zero = image_slash_star::EncodePolicy::new().with_max_work_units(0);
        let mut sink = vec![0x5A];
        let sink_error = match image_slash_star::encode_to_sink_with_policy(
            &image,
            ImageFormat::Jpeg,
            &options,
            &zero,
            &mut sink,
        ) {
            Ok(_) => return Err("zero JPEG work budget unexpectedly wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 0,
                observed: 1,
            }
        ));
        assert_eq!(
            sink,
            vec![0x5A],
            "budget rejection must precede sink writes"
        );

        // A wide single-row image must not hide an unbounded RGB→YCbCr inner
        // loop behind its one row checkpoint. Pillow has no caller-controlled
        // work budget, so this remains a Rust-only result/sink contract.
        let conversion_image = DecodedImage::new(2_048, 1, vec![128; 2_048 * 3], ColorType::Rgb8);
        let conversion_policy = image_slash_star::EncodePolicy::new().with_max_work_units(3);
        let conversion_error = match image_slash_star::encode_with_policy(
            &conversion_image,
            ImageFormat::Jpeg,
            &options,
            &conversion_policy,
        ) {
            Ok(_) => {
                return Err("JPEG RGB conversion budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            conversion_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 3,
                observed: 4,
            }
        ));
        let mut conversion_sink = vec![0x5C];
        let conversion_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &conversion_image,
            ImageFormat::Jpeg,
            &options,
            &conversion_policy,
            &mut conversion_sink,
        ) {
            Ok(_) => {
                return Err("JPEG RGB conversion budget unexpectedly wrote output".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            conversion_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 3,
                observed: 4,
            }
        ));
        assert_eq!(
            conversion_sink,
            vec![0x5C],
            "conversion budget rejection must precede sink writes"
        );

        let mut entropy_pixels = Vec::with_capacity(64 * 64 * 3);
        for index in 0..64 * 64 {
            let x = u8::try_from(index % 64)?;
            let y = u8::try_from(index / 64)?;
            entropy_pixels.extend_from_slice(&[
                x.wrapping_mul(17) ^ y.wrapping_mul(31),
                x.wrapping_mul(43).wrapping_add(y.wrapping_mul(7)),
                x.wrapping_mul(11) ^ y.wrapping_mul(19),
            ]);
        }
        let entropy_image = DecodedImage::new(64, 64, entropy_pixels, ColorType::Rgb8);
        let entropy_expected =
            image_slash_star::encode(&entropy_image, ImageFormat::Jpeg, &options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &entropy_image,
                ImageFormat::Jpeg,
                &options,
                &unlimited,
            )?,
            entropy_expected,
            "an ample JPEG entropy budget preserves byte identity"
        );
        let entropy_policy = image_slash_star::EncodePolicy::new().with_max_work_units(150);
        let entropy_error = match image_slash_star::encode_with_policy(
            &entropy_image,
            ImageFormat::Jpeg,
            &options,
            &entropy_policy,
        ) {
            Ok(_) => return Err("JPEG entropy work budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            entropy_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 150,
                observed: 151,
            }
        ));
        let mut entropy_sink = vec![0x5B];
        let entropy_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &entropy_image,
            ImageFormat::Jpeg,
            &options,
            &entropy_policy,
            &mut entropy_sink,
        ) {
            Ok(_) => return Err("JPEG entropy work budget unexpectedly wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            entropy_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 150,
                observed: 151,
            }
        ));
        assert_eq!(entropy_sink, vec![0x5B]);
    }

    if cfg!(feature = "webp") {
        // The lossy VP8 encoder now charges checkpoints between its major
        // analysis, mode-selection, probability, and bitstream stages. Pillow
        // has no caller-controlled checkpoint budget or equivalent result, so
        // this is Rust-only work-control evidence and must not become a parity
        // row.
        let image = DecodedImage::new(64, 64, vec![128; 64 * 64 * 3], ColorType::Rgb8);
        let options = EncodeOptions::for_format(ImageFormat::WebP);
        let unlimited = image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX);
        let expected = image_slash_star::encode(&image, ImageFormat::WebP, &options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(&image, ImageFormat::WebP, &options, &unlimited,)?,
            expected,
            "an ample WebP VP8 budget preserves byte identity"
        );

        let mut alpha_pixels = Vec::with_capacity(64 * 64 * 4);
        for index in 0..64 * 64 {
            alpha_pixels.extend_from_slice(&[
                128,
                128,
                128,
                if index % 2 == 0 { u8::MAX } else { 127 },
            ]);
        }
        let alpha_image = DecodedImage::new(64, 64, alpha_pixels, ColorType::Rgba8);
        let alpha_expected = image_slash_star::encode(&alpha_image, ImageFormat::WebP, &options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &alpha_image,
                ImageFormat::WebP,
                &options,
                &unlimited,
            )?,
            alpha_expected,
            "an ample WebP VP8 alpha budget preserves byte identity"
        );

        let bounded = image_slash_star::EncodePolicy::new().with_max_work_units(8);
        let error = match image_slash_star::encode_with_policy(
            &image,
            ImageFormat::WebP,
            &options,
            &bounded,
        ) {
            Ok(_) => return Err("bounded WebP VP8 work budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 8,
                observed,
            } if observed > 8
        ));

        let mut sink = vec![0xA8];
        let sink_error = match image_slash_star::encode_to_sink_with_policy(
            &image,
            ImageFormat::WebP,
            &options,
            &bounded,
            &mut sink,
        ) {
            Ok(_) => return Err("bounded WebP VP8 budget unexpectedly wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 8,
                observed,
            } if observed > 8
        ));
        assert_eq!(sink, vec![0xA8]);

        // Lossless VP8L now charges checkpoints around predictor, cross-color,
        // entropy, transform, histogram/Huffman, and bitstream stages plus
        // bounded backward-reference and token-stream intervals. Pillow has no caller-controlled checkpoint
        // budget or equivalent result, so this remains ordinary Rust-only
        // work-control evidence and adds no parity row.
        let mut lossless_pixels = Vec::with_capacity(64 * 64 * 3);
        for index in 0..64 * 64 {
            let x = u8::try_from(index % 64)?;
            let y = u8::try_from(index / 64)?;
            lossless_pixels.extend_from_slice(&[
                x.wrapping_mul(3) ^ y.wrapping_mul(5),
                x.wrapping_add(y.wrapping_mul(7)),
                x.wrapping_mul(11).wrapping_add(y),
            ]);
        }
        let lossless_image = DecodedImage::new(64, 64, lossless_pixels, ColorType::Rgb8);
        let mut lossless_options = EncodeOptions::for_format(ImageFormat::WebP);
        if let EncodeOptions::WebP(options) = &mut lossless_options {
            options.lossless = Some(true);
        }
        let lossless_unlimited =
            image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX);
        let lossless_expected =
            image_slash_star::encode(&lossless_image, ImageFormat::WebP, &lossless_options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &lossless_image,
                ImageFormat::WebP,
                &lossless_options,
                &lossless_unlimited,
            )?,
            lossless_expected,
            "an ample WebP VP8L budget preserves byte identity"
        );
        let lossless_bounded = image_slash_star::EncodePolicy::new().with_max_work_units(8);
        let lossless_error = match image_slash_star::encode_with_policy(
            &lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &lossless_bounded,
        ) {
            Ok(_) => return Err("bounded WebP VP8L budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            lossless_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 8,
                observed,
            } if observed > 8
        ));
        let mut lossless_sink = vec![0xA9];
        let lossless_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &lossless_bounded,
            &mut lossless_sink,
        ) {
            Ok(_) => return Err("bounded WebP VP8L budget unexpectedly wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            lossless_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 8,
                observed,
            } if observed > 8
        ));
        assert_eq!(lossless_sink, vec![0xA9]);

        // A materially larger budget reaches the long predictor/cross-color,
        // histogram/Huffman, backward-reference, and token-stream intervals
        // before rejecting. This remains Rust-only work-control evidence:
        // Pillow exposes no caller budget or equivalent result.
        let deep_lossless_bounded =
            image_slash_star::EncodePolicy::new().with_max_work_units(8_192);
        let deep_lossless_error = match image_slash_star::encode_with_policy(
            &lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &deep_lossless_bounded,
        ) {
            Ok(_) => return Err("deep WebP VP8L budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            deep_lossless_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 8_192,
                observed,
            } if observed > 8_192
        ));

        // This patterned probe reaches the deeper VP8L writer intervals only
        // after the earlier lossless stages. The exact rejection at the
        // logical 4,096-bit interval proves that bitstream work—not a parity
        // fixture or a synthetic coverage hook—owns this boundary. Pillow has
        // no caller work budget or sink.
        let mut output_lossless_pixels = Vec::with_capacity(128 * 128 * 3);
        for index in 0..128 * 128 {
            let x = u8::try_from(index % 128)?;
            let y = u8::try_from(index / 128)?;
            output_lossless_pixels.extend_from_slice(&[
                x.wrapping_mul(3) ^ y.wrapping_mul(5),
                x.wrapping_add(y.wrapping_mul(7)),
                x.wrapping_mul(11).wrapping_add(y),
            ]);
        }
        let output_lossless_image =
            DecodedImage::new(128, 128, output_lossless_pixels, ColorType::Rgb8);
        let output_lossless_expected =
            image_slash_star::encode(&output_lossless_image, ImageFormat::WebP, &lossless_options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &output_lossless_image,
                ImageFormat::WebP,
                &lossless_options,
                &lossless_unlimited,
            )?,
            output_lossless_expected,
            "an ample VP8L output budget preserves byte identity"
        );
        // The finer VP8L logical-bitstream interval rejects at the first
        // 256-bit boundary. This remains Rust-only work-control evidence:
        // Pillow has no caller budget or equivalent result.
        let finest_bitstream_checkpoint_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(54_820);
        let finest_bitstream_checkpoint_error = match image_slash_star::encode_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &finest_bitstream_checkpoint_policy,
        ) {
            Ok(_) => return Err("VP8L finest bitstream budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            finest_bitstream_checkpoint_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 54_820,
                observed: 54_821,
            }
        ));
        let mut finest_bitstream_checkpoint_sink = vec![0xAB];
        let finest_bitstream_checkpoint_sink_error =
            match image_slash_star::encode_to_sink_with_policy(
                &output_lossless_image,
                ImageFormat::WebP,
                &lossless_options,
                &finest_bitstream_checkpoint_policy,
                &mut finest_bitstream_checkpoint_sink,
            ) {
                Ok(_) => {
                    return Err("VP8L finest bitstream sink budget unexpectedly completed".into());
                }
                Err(error) => error,
            };
        assert!(matches!(
            finest_bitstream_checkpoint_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 54_820,
                observed: 54_821,
            }
        ));
        assert_eq!(finest_bitstream_checkpoint_sink, vec![0xAB]);

        // The existing VP8L 512-bit logical-bitstream interval remains
        // independently enforced after the finer 256-bit boundary.
        // Pillow has no caller budget or equivalent result.
        let fine_bitstream_checkpoint_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(54_823);
        let fine_bitstream_checkpoint_error = match image_slash_star::encode_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &fine_bitstream_checkpoint_policy,
        ) {
            Ok(_) => return Err("VP8L fine bitstream budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            fine_bitstream_checkpoint_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 54_823,
                observed: 54_824,
            }
        ));
        let mut fine_bitstream_checkpoint_sink = vec![0xAA];
        let fine_bitstream_checkpoint_sink_error =
            match image_slash_star::encode_to_sink_with_policy(
                &output_lossless_image,
                ImageFormat::WebP,
                &lossless_options,
                &fine_bitstream_checkpoint_policy,
                &mut fine_bitstream_checkpoint_sink,
            ) {
                Ok(_) => {
                    return Err("VP8L fine bitstream sink budget unexpectedly completed".into());
                }
                Err(error) => error,
            };
        assert!(matches!(
            fine_bitstream_checkpoint_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 54_823,
                observed: 54_824,
            }
        ));
        assert_eq!(fine_bitstream_checkpoint_sink, vec![0xAA]);

        let bitstream_checkpoint_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(56_000);
        let bitstream_checkpoint_error = match image_slash_star::encode_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &bitstream_checkpoint_policy,
        ) {
            Ok(_) => return Err("VP8L bitstream checkpoint budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_checkpoint_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 56_000,
                observed: 56_001,
            }
        ));
        let mut bitstream_checkpoint_sink = vec![0xAA];
        let bitstream_checkpoint_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &bitstream_checkpoint_policy,
            &mut bitstream_checkpoint_sink,
        ) {
            Ok(_) => {
                return Err("VP8L bitstream checkpoint sink budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_checkpoint_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 56_000,
                observed: 56_001,
            }
        ));
        assert_eq!(bitstream_checkpoint_sink, vec![0xAA]);

        // A one-unit-lower budget rejects at the first 1,024-byte emitted
        // output interval. This is a separate Rust-only work-control
        // boundary; it is not Pillow byte/pixel parity evidence.
        let output_checkpoint_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(55_999);
        let output_checkpoint_error = match image_slash_star::encode_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &output_checkpoint_policy,
        ) {
            Ok(_) => return Err("VP8L output checkpoint budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            output_checkpoint_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 55_999,
                observed: 56_000,
            }
        ));
        let mut output_checkpoint_sink = vec![0xAA];
        let output_checkpoint_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &output_checkpoint_policy,
            &mut output_checkpoint_sink,
        ) {
            Ok(_) => return Err("VP8L output checkpoint sink budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            output_checkpoint_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 55_999,
                observed: 56_000,
            }
        ));
        assert_eq!(output_checkpoint_sink, vec![0xAA]);

        // Lossy VP8 RGB-to-YUV conversion now charges an interior checkpoint
        // after each 1,024 conversion items. Pillow has no caller token or
        // work-budget result, so this remains Rust-only evidence and adds no
        // parity row. The ordinary path and an ample budget remain identical.
        let yuv_image = DecodedImage::new(2_048, 1, vec![128; 2_048 * 3], ColorType::Rgb8);
        let yuv_expected = image_slash_star::encode(&yuv_image, ImageFormat::WebP, &options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &yuv_image,
                ImageFormat::WebP,
                &options,
                &unlimited,
            )?,
            yuv_expected,
            "an ample WebP YUV-conversion budget preserves byte identity"
        );
        let yuv_bounded = image_slash_star::EncodePolicy::new().with_max_work_units(3);
        let yuv_error = match image_slash_star::encode_with_policy(
            &yuv_image,
            ImageFormat::WebP,
            &options,
            &yuv_bounded,
        ) {
            Ok(_) => return Err("bounded WebP YUV conversion unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            yuv_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 3,
                observed: 4,
            }
        ));
        let mut yuv_sink = vec![0xAA];
        let yuv_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &yuv_image,
            ImageFormat::WebP,
            &options,
            &yuv_bounded,
            &mut yuv_sink,
        ) {
            Ok(_) => return Err("bounded WebP YUV sink budget unexpectedly wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            yuv_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 3,
                observed: 4,
            }
        ));
        assert_eq!(yuv_sink, vec![0xAA]);

        // Lossy VP8 analysis now charges an interior checkpoint after each
        // 1,024 analyzed macroblocks. Pillow has no caller token or
        // work-budget result, so this remains Rust-only evidence and adds no
        // parity row. Reusing the analysis result for frame selection keeps
        // this checkpoint from adding duplicate ordinary work.
        let analysis_image = DecodedImage::new(512, 512, vec![128; 512 * 512 * 3], ColorType::Rgb8);
        let mut analysis_options = EncodeOptions::for_format(ImageFormat::WebP);
        if let EncodeOptions::WebP(options) = &mut analysis_options {
            options.method = Some(0);
        }
        let analysis_expected =
            image_slash_star::encode(&analysis_image, ImageFormat::WebP, &analysis_options)?;
        let partition_probe_pixels: Vec<u8> = (0..896 * 512 * 3)
            .map(|index: usize| u8::try_from(index.wrapping_mul(37) % 256).unwrap_or(0))
            .collect();
        let partition_probe = DecodedImage::new(896, 512, partition_probe_pixels, ColorType::Rgb8);
        // First-partition boolean coding now charges a finer logical
        // checkpoint after each 256 coded bits. This patterned 896x512 probe
        // reaches that interval before the existing 512-bit interval. Pillow
        // has no caller token or work-budget result, so this remains Rust-only
        // evidence with no parity row or coverage-only hook.
        let finest_partition_bit_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(334);
        let finest_partition_bit_error = match image_slash_star::encode_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &finest_partition_bit_policy,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP finest partition-bit budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            finest_partition_bit_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 334,
                observed: 335,
            }
        ));
        let mut finest_partition_bit_sink = vec![0xB7];
        let finest_partition_bit_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &finest_partition_bit_policy,
            &mut finest_partition_bit_sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP finest partition-bit sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            finest_partition_bit_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 334,
                observed: 335,
            }
        ));
        assert_eq!(finest_partition_bit_sink, vec![0xB7]);
        // First-partition boolean coding now charges a logical checkpoint
        // after each 512 coded bits. This patterned 896x512 probe reaches
        // that interval before residual emission. Pillow has no caller token
        // or work-budget result, so this remains Rust-only evidence with no
        // parity row or coverage-only hook; ordinary byte identity is covered
        // by the active parity matrix and the ample-budget probe above.
        let finer_partition_bit_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(593);
        let finer_partition_bit_error = match image_slash_star::encode_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &finer_partition_bit_policy,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP finer partition-bit budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            finer_partition_bit_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 593,
                observed: 594,
            }
        ));
        let mut finer_partition_bit_sink = vec![0xB5];
        let finer_partition_bit_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &finer_partition_bit_policy,
            &mut finer_partition_bit_sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP finer partition-bit sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            finer_partition_bit_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 593,
                observed: 594,
            }
        ));
        assert_eq!(finer_partition_bit_sink, vec![0xB5]);

        let fine_partition_bit_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(580);
        let fine_partition_bit_error = match image_slash_star::encode_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &fine_partition_bit_policy,
        ) {
            Ok(_) => {
                return Err("bounded WebP fine partition-bit budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            fine_partition_bit_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 580,
                observed: 581,
            }
        ));
        let mut fine_partition_bit_sink = vec![0xB6];
        let fine_partition_bit_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &fine_partition_bit_policy,
            &mut fine_partition_bit_sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP fine partition-bit sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            fine_partition_bit_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 580,
                observed: 581,
            }
        ));
        assert_eq!(fine_partition_bit_sink, vec![0xB6]);

        // The existing coarser first-partition boundary remains separately
        // enforced after each 16,384 coded bits, after the finer logical
        // checkpoints above. This is the same Rust-only contract.
        let partition_bit_bounded = image_slash_star::EncodePolicy::new().with_max_work_units(613);
        let partition_bit_error = match image_slash_star::encode_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &partition_bit_bounded,
        ) {
            Ok(_) => return Err("bounded WebP partition-bit budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 613,
                observed: 614,
            }
        ));
        let mut partition_bit_sink = vec![0xB4];
        let partition_bit_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &partition_bit_bounded,
            &mut partition_bit_sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP partition-bit sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 613,
                observed: 614,
            }
        ));
        assert_eq!(partition_bit_sink, vec![0xB4]);
        // Coefficient-partition output now charges an interior checkpoint
        // after each 1,024 emitted boolean-coder bytes. This reaches the first
        // coefficient-output interval after the first-partition bit interval.
        // Pillow has no caller token, work-budget result, or caller-owned
        // sink, so this remains Rust-only evidence with no parity row or
        // coverage-only hook.
        let output_policy = image_slash_star::EncodePolicy::new().with_max_work_units(589);
        let output_error = match image_slash_star::encode_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &output_policy,
        ) {
            Ok(_) => return Err("bounded WebP output-byte budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            output_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 589,
                observed: 590,
            }
        ));
        let output_sink_policy = image_slash_star::EncodePolicy::new().with_max_work_units(588);
        let mut output_sink = vec![0xB5];
        let output_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &output_sink_policy,
            &mut output_sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP output-byte sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            output_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 588,
                observed: 589,
            }
        ));
        assert_eq!(output_sink, vec![0xB5]);

        // A later budget reaches the coefficient partition after the first
        // partition has completed. This keeps the residual writer's empty
        // block and block-wrapper paths covered even though the finer first
        // partition checkpoint now interrupts earlier probes. Pillow has no
        // caller token or work-budget result, so this remains Rust-only
        // evidence with no parity row or coverage-only hook.
        let residual_policy = image_slash_star::EncodePolicy::new().with_max_work_units(700);
        let residual_error = match image_slash_star::encode_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &residual_policy,
        ) {
            Ok(_) => return Err("bounded WebP residual budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            residual_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 700,
                observed: 701,
            }
        ));

        assert_eq!(
            image_slash_star::encode_with_policy(
                &analysis_image,
                ImageFormat::WebP,
                &analysis_options,
                &unlimited,
            )?,
            analysis_expected,
            "an ample WebP analysis budget preserves byte identity"
        );
        let analysis_bounded = image_slash_star::EncodePolicy::new().with_max_work_units(326);
        let analysis_error = match image_slash_star::encode_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &analysis_bounded,
        ) {
            Ok(_) => return Err("bounded WebP analysis budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            analysis_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 326,
                observed: 327,
            }
        ));
        let mut analysis_sink = vec![0xAB];
        let analysis_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &analysis_bounded,
            &mut analysis_sink,
        ) {
            Ok(_) => {
                return Err("bounded WebP analysis sink budget unexpectedly wrote output".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            analysis_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 326,
                observed: 327,
            }
        ));
        assert_eq!(analysis_sink, vec![0xAB]);

        // The following selection pass now charges its own 1,024-macroblock
        // interior checkpoint. The preceding analysis checkpoint is allowed
        // through, so the next charge is observed as 330. This remains
        // Rust-only work-control evidence with no parity row or coverage-only
        // hook.
        let selection_bounded = image_slash_star::EncodePolicy::new().with_max_work_units(329);
        let selection_error = match image_slash_star::encode_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &selection_bounded,
        ) {
            Ok(_) => return Err("bounded WebP selection budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            selection_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 329,
                observed: 330,
            }
        ));
        let mut selection_sink = vec![0xAC];
        let selection_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &selection_bounded,
            &mut selection_sink,
        ) {
            Ok(_) => {
                return Err("bounded WebP selection sink budget unexpectedly wrote output".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            selection_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 329,
                observed: 330,
            }
        ));
        assert_eq!(selection_sink, vec![0xAC]);

        // Coefficient-probability adaptation has 1,056 fixed probability
        // nodes and charges at 1,024 nodes. Pillow has no caller token or
        // work-budget result, so this remains Rust-only evidence with no
        // parity row or coverage-only hook.
        let probability_bounded = image_slash_star::EncodePolicy::new().with_max_work_units(331);
        let probability_error = match image_slash_star::encode_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &probability_bounded,
        ) {
            Ok(_) => return Err("bounded WebP probability budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            probability_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 331,
                observed: 332,
            }
        ));
        let mut probability_sink = vec![0xAD];
        let probability_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &probability_bounded,
            &mut probability_sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP probability sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            probability_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 331,
                observed: 332,
            }
        ));
        assert_eq!(probability_sink, vec![0xAD]);

        // The first VP8 partition charges its fixed coefficient-probability
        // signaling table after 1,024 nodes. Pillow has no caller token or
        // work-budget result, so this remains Rust-only evidence with no
        // parity row or coverage-only hook.
        let partition_probability_bounded =
            image_slash_star::EncodePolicy::new().with_max_work_units(333);
        let partition_probability_error = match image_slash_star::encode_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &partition_probability_bounded,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP partition-probability budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_probability_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 333,
                observed: 334,
            }
        ));
        let mut partition_probability_sink = vec![0xAF];
        let partition_probability_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &partition_probability_bounded,
            &mut partition_probability_sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP partition-probability sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_probability_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 333,
                observed: 334,
            }
        ));
        assert_eq!(partition_probability_sink, vec![0xAF]);

        // Mode signaling charges after each batch of 256 macroblocks. This
        // remains Rust-only work-control evidence with no parity row or
        // coverage-only hook.
        let partition_mode_bounded = image_slash_star::EncodePolicy::new().with_max_work_units(334);
        let partition_mode_error = match image_slash_star::encode_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &partition_mode_bounded,
        ) {
            Ok(_) => return Err("bounded WebP partition-mode budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            partition_mode_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 334,
                observed: 335,
            }
        ));
        let mut partition_mode_sink = vec![0xB0];
        let partition_mode_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &partition_mode_bounded,
            &mut partition_mode_sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP partition-mode sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_mode_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 334,
                observed: 335,
            }
        ));
        assert_eq!(partition_mode_sink, vec![0xB0]);

        // Coefficient bitstream emission charges after each batch of 256
        // completed macroblocks. Pillow has no caller token or work-budget
        // result, so this remains Rust-only evidence with no parity row or
        // coverage-only hook.
        let coefficient_bounded = image_slash_star::EncodePolicy::new().with_max_work_units(339);
        let coefficient_error = match image_slash_star::encode_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_bounded,
        ) {
            Ok(_) => return Err("bounded WebP coefficient budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 339,
                observed: 340,
            }
        ));
        let mut coefficient_sink = vec![0xAE];
        let coefficient_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_bounded,
            &mut coefficient_sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP coefficient sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 339,
                observed: 340,
            }
        ));
        assert_eq!(coefficient_sink, vec![0xAE]);

        // Coefficient-token signaling is finer than block emission. On this
        // constant 512x512 probe, the 4,000-token charge lands after the
        // 62nd 64-block checkpoint, so it is observed as 401. This remains
        // Rust-only work-control evidence with no parity row or coverage-only
        // hook.
        let coefficient_token_bounded =
            image_slash_star::EncodePolicy::new().with_max_work_units(400);
        let coefficient_token_error = match image_slash_star::encode_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_token_bounded,
        ) {
            Ok(_) => {
                return Err("bounded WebP coefficient-token budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_token_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 400,
                observed: 401,
            }
        ));
        let mut coefficient_token_sink = vec![0xB2];
        let coefficient_token_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_token_bounded,
            &mut coefficient_token_sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP coefficient-token sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_token_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 400,
                observed: 401,
            }
        ));
        assert_eq!(coefficient_token_sink, vec![0xB2]);

        // Coefficient boolean coding now charges a finer logical checkpoint
        // after each 256 coded bits. Pillow has no caller token or work-budget
        // result, so this remains Rust-only work-control evidence with no
        // parity row or coverage-only hook.
        let coefficient_finest_bit_bounded =
            image_slash_star::EncodePolicy::new().with_max_work_units(820);
        let coefficient_finest_bit_error = match image_slash_star::encode_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_finest_bit_bounded,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP finest coefficient-bit budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_finest_bit_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 820,
                observed: 821,
            }
        ));
        let mut coefficient_finest_bit_sink = vec![0xB5];
        let coefficient_finest_bit_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_finest_bit_bounded,
            &mut coefficient_finest_bit_sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP finest coefficient-bit sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_finest_bit_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 820,
                observed: 821,
            }
        ));
        assert_eq!(coefficient_finest_bit_sink, vec![0xB5]);

        // The 512-bit logical coefficient checkpoint remains independently
        // enforced after the finer 256-bit boundary. This 512x512 probe
        // reaches the later logical interval after the earlier checkpoints.
        let coefficient_fine_bit_bounded =
            image_slash_star::EncodePolicy::new().with_max_work_units(821);
        let coefficient_fine_bit_error = match image_slash_star::encode_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_fine_bit_bounded,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP fine coefficient-bit budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_fine_bit_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 821,
                observed: 822,
            }
        ));
        let mut coefficient_fine_bit_sink = vec![0xB3];
        let coefficient_fine_bit_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_fine_bit_bounded,
            &mut coefficient_fine_bit_sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP fine coefficient-bit sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_fine_bit_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 821,
                observed: 822,
            }
        ));
        assert_eq!(coefficient_fine_bit_sink, vec![0xB3]);

        // The existing coarser coefficient boolean checkpoint remains
        // independently enforced after each 16,384 coded bits.
        let coefficient_bit_bounded =
            image_slash_star::EncodePolicy::new().with_max_work_units(647);
        let coefficient_bit_error = match image_slash_star::encode_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_bit_bounded,
        ) {
            Ok(_) => {
                return Err("bounded WebP coefficient-bit budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 647,
                observed: 648,
            }
        ));
        let mut coefficient_bit_sink = vec![0xB3];
        let coefficient_bit_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_bit_bounded,
            &mut coefficient_bit_sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP coefficient-bit sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 647,
                observed: 648,
            }
        ));
        assert_eq!(coefficient_bit_sink, vec![0xB3]);

        // The coarser coefficient macroblock checkpoint remains in place
        // after the finer block checkpoint. On this 512x512 probe, the
        // macroblock charge is observed as 467 after the earlier residual
        // checkpoints. This is
        // Rust-only work-control evidence
        // with no parity row or coverage-only hook.
        let coefficient_macroblock_bounded =
            image_slash_star::EncodePolicy::new().with_max_work_units(466);
        let coefficient_macroblock_error = match image_slash_star::encode_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_macroblock_bounded,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP coefficient macroblock budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_macroblock_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 466,
                observed: 467,
            }
        ));
        let mut coefficient_macroblock_sink = vec![0xB1];
        let coefficient_macroblock_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_macroblock_bounded,
            &mut coefficient_macroblock_sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP coefficient macroblock sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_macroblock_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 466,
                observed: 467,
            }
        ));
        assert_eq!(coefficient_macroblock_sink, vec![0xB1]);

        // Lossy VP8 RGBA transparent-area cleanup now charges a checkpoint
        // after each 1,024 scanned or flattened pixels. Pillow has no caller
        // token or work-budget result, so this remains Rust-only work-control
        // evidence and adds no parity row. The 128x128 all-transparent probe
        // reaches the cleanup interval after the preceding alpha/conversion
        // checkpoints; both return and direct-sink rejection leave output
        // unpublished.
        let transparent_cleanup_image = DecodedImage::new(
            128,
            128,
            [128, 128, 128, 0].repeat(128 * 128),
            ColorType::Rgba8,
        );
        let transparent_cleanup_expected =
            image_slash_star::encode(&transparent_cleanup_image, ImageFormat::WebP, &options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &transparent_cleanup_image,
                ImageFormat::WebP,
                &options,
                &unlimited,
            )?,
            transparent_cleanup_expected,
            "an ample WebP transparent-area budget preserves byte identity"
        );
        let transparent_cleanup_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(400);
        let transparent_cleanup_error = match image_slash_star::encode_with_policy(
            &transparent_cleanup_image,
            ImageFormat::WebP,
            &options,
            &transparent_cleanup_policy,
        ) {
            Ok(_) => return Err("bounded WebP transparent cleanup unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            transparent_cleanup_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 400,
                observed: 401,
            }
        ));
        let mut transparent_cleanup_sink = vec![0xB4];
        let transparent_cleanup_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &transparent_cleanup_image,
            ImageFormat::WebP,
            &options,
            &transparent_cleanup_policy,
            &mut transparent_cleanup_sink,
        ) {
            Ok(_) => {
                return Err("bounded WebP transparent cleanup sink unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            transparent_cleanup_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 400,
                observed: 401,
            }
        ));
        assert_eq!(transparent_cleanup_sink, vec![0xB4]);
    }

    if cfg!(feature = "gif") {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let data = fs::read(root.join("tests/fixtures/input/images/gif/animated_3frame.gif"))?;
        let sequence = image_slash_star::decode_sequence(&data)?.into_inner();
        let options = EncodeOptions::for_format(ImageFormat::Gif);
        let zero = image_slash_star::EncodePolicy::new().with_max_work_units(0);
        let error = match image_slash_star::encode_sequence_with_policy(
            &sequence,
            ImageFormat::Gif,
            &options,
            &zero,
        ) {
            Ok(_) => return Err("zero work budget unexpectedly encoded GIF".into()),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::SequenceEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 0,
                observed: 1,
            }
        ));

        let cancelled = image_slash_star::CancellationToken::new();
        cancelled.cancel();
        assert!(matches!(
            image_slash_star::encode_sequence_with_token_and_policy(
                &sequence,
                ImageFormat::Gif,
                &options,
                &zero,
                &cancelled,
            ),
            Err(ImageError::Cancelled { .. })
        ));
        let mut cancelled_sink = vec![0xBC];
        assert!(matches!(
            image_slash_star::encode_sequence_to_sink_with_token_and_policy(
                &sequence,
                ImageFormat::Gif,
                &options,
                &zero,
                &cancelled,
                &mut cancelled_sink,
            ),
            Err(ImageError::Cancelled { .. })
        ));
        assert_eq!(
            cancelled_sink,
            vec![0xBC],
            "caller cancellation precedes sequence work-budget delivery"
        );

        // GIF LZW now charges an input-symbol checkpoint inside its
        // dictionary pass. Pillow has no caller token or work-budget result,
        // so this remains Rust-only work-control evidence and adds no parity
        // row. The ordinary path and an ample budget must remain byte-identical.
        let mut pixels = Vec::with_capacity(64 * 64);
        for index in 0..64 * 64 {
            pixels.push(u8::try_from(index % 256)?);
        }
        let image = DecodedImage::new(64, 64, pixels, ColorType::L8);
        let expected = image_slash_star::encode(&image, ImageFormat::Gif, &options)?;
        let unlimited = image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX);
        assert_eq!(
            image_slash_star::encode_with_policy(&image, ImageFormat::Gif, &options, &unlimited)?,
            expected,
            "an ample GIF LZW budget preserves byte identity"
        );

        let bounded = image_slash_star::EncodePolicy::new().with_max_work_units(7);
        let error = match image_slash_star::encode_with_policy(
            &image,
            ImageFormat::Gif,
            &options,
            &bounded,
        ) {
            Ok(_) => return Err("bounded GIF LZW work budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 7,
                observed: 8,
            }
        ));

        // The structural sink calls the GIF writer directly, so its same
        // input-symbol interval is reached after one fewer dispatcher poll.
        let sink_policy = image_slash_star::EncodePolicy::new().with_max_work_units(6);
        let mut sink = vec![0xBD];
        let sink_error = match image_slash_star::encode_to_sink_with_policy(
            &image,
            ImageFormat::Gif,
            &options,
            &sink_policy,
            &mut sink,
        ) {
            Ok(_) => return Err("bounded GIF LZW sink budget unexpectedly wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 6,
                observed: 7,
            }
        ));
        assert_eq!(sink, vec![0xBD]);

        // GIF RGB palette preparation now charges a checkpoint after each
        // 1,024-pixel quantization interval. Pillow has no caller token or
        // work-budget result, so this remains Rust-only work-control evidence
        // and adds no parity row. The ordinary path and an ample budget must
        // remain byte-identical.
        let image = DecodedImage::new(2_048, 1, vec![128; 2_048 * 3], ColorType::Rgb8);
        let expected = image_slash_star::encode(&image, ImageFormat::Gif, &options)?;
        let unlimited = image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX);
        assert_eq!(
            image_slash_star::encode_with_policy(&image, ImageFormat::Gif, &options, &unlimited)?,
            expected,
            "an ample GIF RGB quantization budget preserves byte identity"
        );

        let bounded = image_slash_star::EncodePolicy::new().with_max_work_units(6);
        let error = match image_slash_star::encode_with_policy(
            &image,
            ImageFormat::Gif,
            &options,
            &bounded,
        ) {
            Ok(_) => {
                return Err("bounded GIF RGB quantization budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 6,
                observed: 7,
            }
        ));

        // The structural sink calls the GIF writer directly, so the same
        // quantization interval is reached after one fewer dispatcher poll.
        let sink_policy = image_slash_star::EncodePolicy::new().with_max_work_units(5);
        let mut sink = vec![0xBF];
        let sink_error = match image_slash_star::encode_to_sink_with_policy(
            &image,
            ImageFormat::Gif,
            &options,
            &sink_policy,
            &mut sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded GIF RGB quantization sink budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 5,
                observed: 6,
            }
        ));
        assert_eq!(sink, vec![0xBF]);

        // GIF RGBA FASTOCTREE palette preparation now charges checkpoints in
        // pixel accumulation and final index emission. Pillow has no caller
        // token or work-budget result, so this remains Rust-only work-control
        // evidence and adds no parity row. The ordinary path and an ample
        // budget must remain byte-identical.
        let mut rgba_pixels = Vec::with_capacity(2_048 * 4);
        for _ in 0..2_048 {
            rgba_pixels.extend_from_slice(&[128, 64, 32, 255]);
        }
        let image = DecodedImage::new(2_048, 1, rgba_pixels, ColorType::Rgba8);
        let expected = image_slash_star::encode(&image, ImageFormat::Gif, &options)?;
        let unlimited = image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX);
        assert_eq!(
            image_slash_star::encode_with_policy(&image, ImageFormat::Gif, &options, &unlimited)?,
            expected,
            "an ample GIF RGBA quantization budget preserves byte identity"
        );

        let bounded = image_slash_star::EncodePolicy::new().with_max_work_units(6);
        let error = match image_slash_star::encode_with_policy(
            &image,
            ImageFormat::Gif,
            &options,
            &bounded,
        ) {
            Ok(_) => {
                return Err("bounded GIF RGBA quantization budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 6,
                observed: 7,
            }
        ));

        // The structural sink calls the GIF writer directly, so the same
        // quantization interval is reached after one fewer dispatcher poll.
        let sink_policy = image_slash_star::EncodePolicy::new().with_max_work_units(5);
        let mut sink = vec![0xC0];
        let sink_error = match image_slash_star::encode_to_sink_with_policy(
            &image,
            ImageFormat::Gif,
            &options,
            &sink_policy,
            &mut sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded GIF RGBA quantization sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 5,
                observed: 6,
            }
        ));
        assert_eq!(sink, vec![0xC0]);

        // GIF RGBA FASTOCTREE cube-copy work now charges a checkpoint after
        // each 1,024 fixed-cell interval. This is still Rust-only evidence:
        // Pillow has no caller token or work-budget result, and no parity row
        // or synthetic coverage-only input is added.
        let image = DecodedImage::new(1, 1, vec![128, 64, 32, 255], ColorType::Rgba8);
        let expected = image_slash_star::encode(&image, ImageFormat::Gif, &options)?;
        let unlimited = image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX);
        assert_eq!(
            image_slash_star::encode_with_policy(&image, ImageFormat::Gif, &options, &unlimited)?,
            expected,
            "an ample GIF RGBA octree budget preserves byte identity"
        );

        let bounded = image_slash_star::EncodePolicy::new().with_max_work_units(6);
        let error = match image_slash_star::encode_with_policy(
            &image,
            ImageFormat::Gif,
            &options,
            &bounded,
        ) {
            Ok(_) => return Err("bounded GIF RGBA octree budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 6,
                observed: 7,
            }
        ));

        let sink_policy = image_slash_star::EncodePolicy::new().with_max_work_units(5);
        let mut sink = vec![0xC1];
        let sink_error = match image_slash_star::encode_to_sink_with_policy(
            &image,
            ImageFormat::Gif,
            &options,
            &sink_policy,
            &mut sink,
        ) {
            Ok(_) => {
                return Err("bounded GIF RGBA octree sink budget unexpectedly wrote output".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 5,
                observed: 6,
            }
        ));
        assert_eq!(sink, vec![0xC1]);

        // The token-aware Apple-compatible bucket sort now charges every
        // 1,024 sorting operations. The already-tested ample token path keeps
        // bytes identical; these bounds prove rejection inside the sorter,
        // without adding a Pillow parity row or a coverage-only input.
        let bounded = image_slash_star::EncodePolicy::new().with_max_work_units(8);
        let error = match image_slash_star::encode_with_policy(
            &image,
            ImageFormat::Gif,
            &options,
            &bounded,
        ) {
            Ok(_) => return Err("bounded GIF bucket-sort budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 8,
                observed: 9,
            }
        ));

        let mut sink = vec![0xC3];
        let sink_error = match image_slash_star::encode_to_sink_with_policy(
            &image,
            ImageFormat::Gif,
            &options,
            &bounded,
            &mut sink,
        ) {
            Ok(_) => {
                return Err("bounded GIF bucket-sort sink budget unexpectedly wrote output".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 8,
                observed: 9,
            }
        ));
        assert_eq!(sink, vec![0xC3]);

        // A diverse RGBA palette exercises the sorter’s nontrivial partitions
        // and recursive small ranges while preserving the ordinary bytes under
        // an ample token budget. This is implementation work-control evidence,
        // not a parity or coverage-only fixture.
        let mut varied_rgba_pixels = Vec::with_capacity(2_048 * 4);
        for red in 0..8u8 {
            for green in 0..8u8 {
                for blue in 0..32u8 {
                    varied_rgba_pixels.extend_from_slice(&[
                        red.saturating_mul(32),
                        green.saturating_mul(32),
                        blue.saturating_mul(8),
                        255,
                    ]);
                }
            }
        }
        let varied_image = DecodedImage::new(2_048, 1, varied_rgba_pixels, ColorType::Rgba8);
        let varied_expected = image_slash_star::encode(&varied_image, ImageFormat::Gif, &options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &varied_image,
                ImageFormat::Gif,
                &options,
                &unlimited,
            )?,
            varied_expected,
            "an ample GIF varied-RGBA bucket-sort budget preserves byte identity"
        );

        // Transparent-pixel RGB normalization is part of Pillow-compatible
        // FASTOCTREE preparation. This real RGBA work-control probe exercises
        // the token-aware normalization interval without adding a parity row.
        let mut transparent_pixels = Vec::with_capacity(2_048 * 4);
        for index in 0..2_048u32 {
            transparent_pixels.extend_from_slice(&[
                u8::try_from(index & 0xff)?,
                u8::try_from((index >> 8) & 0xff)?,
                u8::try_from((index >> 16) & 0xff)?,
                0,
            ]);
        }
        let transparent_image = DecodedImage::new(2_048, 1, transparent_pixels, ColorType::Rgba8);
        let transparent_expected =
            image_slash_star::encode(&transparent_image, ImageFormat::Gif, &options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &transparent_image,
                ImageFormat::Gif,
                &options,
                &unlimited,
            )?,
            transparent_expected,
            "an ample GIF transparent-normalization budget preserves byte identity"
        );

        let bounded = image_slash_star::EncodePolicy::new().with_max_work_units(2);
        let error = match image_slash_star::encode_with_policy(
            &transparent_image,
            ImageFormat::Gif,
            &options,
            &bounded,
        ) {
            Ok(_) => {
                return Err(
                    "bounded GIF transparent-normalization budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 2,
                observed: 3,
            }
        ));

        let sink_policy = image_slash_star::EncodePolicy::new().with_max_work_units(1);
        let mut sink = vec![0xC4];
        let sink_error = match image_slash_star::encode_to_sink_with_policy(
            &transparent_image,
            ImageFormat::Gif,
            &options,
            &sink_policy,
            &mut sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded GIF transparent-normalization sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 1,
                observed: 2,
            }
        ));
        assert_eq!(sink, vec![0xC4]);

        // GIF high-color RGB median-cut preparation now charges its hash/order,
        // axis, split, and partition scans. This is a real Rust-only
        // work-control contract: Pillow has no caller token or work-budget
        // result, and no parity row or synthetic coverage-only input is added.
        let mut high_color_pixels = Vec::with_capacity(2_048 * 3);
        for index in 0..2_048u32 {
            high_color_pixels.extend_from_slice(&[
                u8::try_from(index & 0xff)?,
                u8::try_from((index >> 8) & 0xff)?,
                u8::try_from((index >> 16) & 0xff)?,
            ]);
        }
        let image = DecodedImage::new(2_048, 1, high_color_pixels, ColorType::Rgb8);
        let expected = image_slash_star::encode(&image, ImageFormat::Gif, &options)?;
        let unlimited = image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX);
        assert_eq!(
            image_slash_star::encode_with_policy(&image, ImageFormat::Gif, &options, &unlimited)?,
            expected,
            "an ample GIF high-color median-cut budget preserves byte identity"
        );

        let bounded = image_slash_star::EncodePolicy::new().with_max_work_units(6);
        let error = match image_slash_star::encode_with_policy(
            &image,
            ImageFormat::Gif,
            &options,
            &bounded,
        ) {
            Ok(_) => {
                return Err(
                    "bounded GIF high-color median-cut budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 6,
                observed: 7,
            }
        ));

        let sink_policy = image_slash_star::EncodePolicy::new().with_max_work_units(5);
        let mut sink = vec![0xC2];
        let sink_error = match image_slash_star::encode_to_sink_with_policy(
            &image,
            ImageFormat::Gif,
            &options,
            &sink_policy,
            &mut sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded GIF high-color median-cut sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 5,
                observed: 6,
            }
        ));
        assert_eq!(sink, vec![0xC2]);
    }

    if cfg!(feature = "bmp") {
        // BMP true-color/indexed row conversion now charges a checkpoint
        // inside a wide row. Pillow has no caller token or work-budget result,
        // so this remains Rust-only work-control evidence and adds no parity
        // row. The ordinary path and an ample budget must remain byte-identical.
        let image = DecodedImage::new(2_048, 1, vec![128; 2_048 * 3], ColorType::Rgb8);
        let options = EncodeOptions::for_format(ImageFormat::Bmp);
        let expected = image_slash_star::encode(&image, ImageFormat::Bmp, &options)?;
        let unlimited = image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX);
        assert_eq!(
            image_slash_star::encode_with_policy(&image, ImageFormat::Bmp, &options, &unlimited)?,
            expected,
            "an ample BMP row budget preserves byte identity"
        );

        let bounded = image_slash_star::EncodePolicy::new().with_max_work_units(4);
        let error = match image_slash_star::encode_with_policy(
            &image,
            ImageFormat::Bmp,
            &options,
            &bounded,
        ) {
            Ok(_) => return Err("bounded BMP row work budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Bmp),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 4,
                observed: 5,
            }
        ));

        // The structural sink calls the BMP writer directly, so its same
        // row-conversion interval is reached after one fewer dispatcher poll.
        let sink_policy = image_slash_star::EncodePolicy::new().with_max_work_units(3);
        let mut sink = vec![0xBE];
        let sink_error = match image_slash_star::encode_to_sink_with_policy(
            &image,
            ImageFormat::Bmp,
            &options,
            &sink_policy,
            &mut sink,
        ) {
            Ok(_) => return Err("bounded BMP row sink budget unexpectedly wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Bmp),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 3,
                observed: 4,
            }
        ));
        let mut expected_prefix = vec![0xBE];
        expected_prefix.extend_from_slice(&expected[..54]);
        assert_eq!(
            sink, expected_prefix,
            "interior BMP work rejection preserves the delivered header prefix"
        );
    }

    if cfg!(feature = "tiff") {
        use image_slash_star::TiffCompression;

        // TIFF Deflate polls at input-row boundaries and inside the level-six
        // matcher. This is a Rust work-control contract: Pillow has no
        // caller-owned checkpoint budget or equivalent result.
        let mut pixels = Vec::with_capacity(256 * 256 * 3);
        for row in 0..256u16 {
            for column in 0..256u16 {
                pixels.extend_from_slice(&[
                    row.wrapping_add(column).to_le_bytes()[0],
                    row.wrapping_mul(3).wrapping_add(column).to_le_bytes()[0],
                    row.wrapping_add(column.wrapping_mul(5)).to_le_bytes()[0],
                ]);
            }
        }
        let image = DecodedImage::new(256, 256, pixels, ColorType::Rgb8);
        let mut options = EncodeOptions::for_format(ImageFormat::Tiff);
        if let EncodeOptions::Tiff(options) = &mut options {
            options.compression = Some(TiffCompression::Deflate);
        }
        let unlimited = image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX);
        let expected = image_slash_star::encode(&image, ImageFormat::Tiff, &options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(&image, ImageFormat::Tiff, &options, &unlimited,)?,
            expected,
            "an ample TIFF Deflate budget preserves byte identity"
        );

        let bounded = image_slash_star::EncodePolicy::new().with_max_work_units(32);
        let error = match image_slash_star::encode_with_policy(
            &image,
            ImageFormat::Tiff,
            &options,
            &bounded,
        ) {
            Ok(_) => return Err("row-bounded TIFF Deflate unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Tiff),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 32,
                ..
            }
        ));

        // These budgets reach the final row's post-processing checkpoint,
        // final-row completion, post-tokenization, and post-output
        // checkpoints respectively. They prove that each late rejection
        // still publishes no returned bytes, rather than merely exercising
        // the first row-boundary failure.
        for maximum in [517, 518, 519, 520] {
            let bounded = image_slash_star::EncodePolicy::new().with_max_work_units(maximum);
            let error = match image_slash_star::encode_with_policy(
                &image,
                ImageFormat::Tiff,
                &options,
                &bounded,
            ) {
                Ok(_) => {
                    return Err(
                        format!("TIFF Deflate budget {maximum} unexpectedly completed").into(),
                    );
                }
                Err(error) => error,
            };
            assert!(matches!(
                error,
                ImageError::LimitExceeded {
                    format: Some(ImageFormat::Tiff),
                    operation: image_slash_star::CodecOperation::StillEncode,
                    resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                    maximum: observed_maximum,
                    ..
                } if observed_maximum == maximum
            ));
        }

        // A single wide row leaves no row boundary inside the compression
        // pass. The smaller budget above proves matcher-internal checkpoints;
        // this materially larger budget reaches the Deflate expansion,
        // Huffman, and bitstream checkpoints before rejecting. Pillow has no
        // caller budget or equivalent result, so this remains Rust-only
        // work-control evidence.
        let mut interior_pixels = Vec::with_capacity(4_096 * 3);
        for position in 0..4_096usize {
            let value =
                u8::try_from((position.wrapping_mul(37) ^ position.wrapping_shr(3)) & 0xff)?;
            interior_pixels.extend_from_slice(&[
                value,
                value.rotate_left(1),
                value.wrapping_add(17),
            ]);
        }
        let interior_image = DecodedImage::new(4_096, 1, interior_pixels, ColorType::Rgb8);
        let emission_policy = image_slash_star::EncodePolicy::new().with_max_work_units(36_000);
        let emission_error = match image_slash_star::encode_with_policy(
            &interior_image,
            ImageFormat::Tiff,
            &options,
            &emission_policy,
        ) {
            Ok(_) => return Err("emission TIFF Deflate budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            emission_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Tiff),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 36_000,
                observed,
            } if observed > 36_000
        ));
        let mut emission_sink = vec![0xAA];
        let emission_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &interior_image,
            ImageFormat::Tiff,
            &options,
            &emission_policy,
            &mut emission_sink,
        ) {
            Ok(_) => return Err("emission TIFF Deflate sink budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            emission_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Tiff),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 36_000,
                observed,
            } if observed > 36_000
        ));
        assert_eq!(emission_sink, vec![0xAA]);
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

        // WebP bitstream context is a Rust defensive error contract. These
        // fields describe the encoded payload parse site, which Pillow does
        // not expose and therefore must not become parity-matrix columns.
        for (label, fixture) in [
            ("VP8", "vp8_tail_truncated.webp"),
            ("VP8L", "vp8l_truncated_6.webp"),
        ] {
            let bytes = fs::read(root.join("tests/fixtures/input/images/webp").join(fixture))?;
            let decode_error = match image_slash_star::decode(&bytes) {
                Err(error) => error,
                Ok(image) => panic!("truncated {label} bitstream must fail decode: {image:?}"),
            };
            assert_eq!(decode_error.kind(), ImageErrorKind::Malformed, "{label}");
            assert_eq!(
                decode_error.stage(),
                Some(ImageErrorStage::StillDecode),
                "{label}"
            );
            assert_eq!(decode_error.identity(), Some("webp_bitstream"), "{label}");
            assert_eq!(decode_error.offset(), Some(20), "{label}");
        }
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

    use image_slash_star::TiffCompression;
    struct RecordingTiffSink {
        bytes: Vec<u8>,
        writes: usize,
    }
    impl image_slash_star::OutputSink for RecordingTiffSink {
        fn write_all(&mut self, bytes: &[u8]) -> image_slash_star::ImageResult<()> {
            self.writes += 1;
            self.bytes.extend_from_slice(bytes);
            Ok(())
        }
    }

    let decoded = image_slash_star::decode(&fs::read(
        root.join("tests/fixtures/input/images/tiff/8bit.tiff"),
    )?)?;
    // TIFF still delivery is a Rust-only structural sink contract. Pillow has
    // no caller-owned destination, so these assertions do not add parity rows.
    for compression in [TiffCompression::Raw, TiffCompression::Deflate] {
        let mut options = EncodeOptions::for_format(ImageFormat::Tiff);
        if let EncodeOptions::Tiff(options) = &mut options {
            options.compression = Some(compression);
        }
        let expected = image_slash_star::encode(&decoded.content, ImageFormat::Tiff, &options)?;
        let mut structural = RecordingTiffSink {
            bytes: Vec::new(),
            writes: 0,
        };
        assert_eq!(
            image_slash_star::encode_to_sink(
                &decoded.content,
                ImageFormat::Tiff,
                &options,
                &mut structural,
            )?,
            expected.len()
        );
        assert_eq!(structural.bytes, expected);
        assert!(
            structural.writes > 1,
            "TIFF output must cross structural write boundaries"
        );

        let limited = image_slash_star::EncodePolicy::new()
            .with_max_output_bytes(expected.len().saturating_sub(1) as u64);
        let mut policy_sink = RecordingTiffSink {
            bytes: Vec::new(),
            writes: 0,
        };
        let policy_error = match image_slash_star::encode_to_sink_with_policy(
            &decoded.content,
            ImageFormat::Tiff,
            &options,
            &limited,
            &mut policy_sink,
        ) {
            Ok(length) => panic!("limited TIFF sink unexpectedly accepted {length} bytes"),
            Err(error) => error,
        };
        assert_eq!(
            policy_error.kind(),
            image_slash_star::ImageErrorKind::LimitExceeded
        );
        assert!(matches!(
            policy_error,
            ImageError::LimitExceeded {
                resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
                ..
            }
        ));
        assert_eq!(policy_sink.writes, 0);
        assert!(policy_sink.bytes.is_empty());
    }

    let mismatch_options = EncodeOptions::for_format(ImageFormat::Png);
    let mut mismatch_sink = RecordingTiffSink {
        bytes: Vec::new(),
        writes: 0,
    };
    let mismatch_error = match image_slash_star::encode_to_sink(
        &decoded.content,
        ImageFormat::Tiff,
        &mismatch_options,
        &mut mismatch_sink,
    ) {
        Ok(length) => panic!("mismatched TIFF sink unexpectedly accepted {length} bytes"),
        Err(error) => error,
    };
    assert_eq!(
        mismatch_error.kind(),
        image_slash_star::ImageErrorKind::Parameter
    );
    assert_eq!(mismatch_error.stage(), Some(ImageErrorStage::StillEncode));
    assert_eq!(mismatch_sink.writes, 0);

    let tiff_sequence = DecodedSequence::from_image(decoded.content.clone());
    let sequence_options = EncodeOptions::for_format(ImageFormat::Tiff);
    let expected_sequence =
        image_slash_star::encode_sequence(&tiff_sequence, ImageFormat::Tiff, &sequence_options)?;
    let mut sequence_sink = RecordingTiffSink {
        bytes: Vec::new(),
        writes: 0,
    };
    assert_eq!(
        image_slash_star::encode_sequence_to_sink(
            &tiff_sequence,
            ImageFormat::Tiff,
            &sequence_options,
            &mut sequence_sink,
        )?,
        expected_sequence.len()
    );
    assert_eq!(sequence_sink.bytes, expected_sequence);
    assert!(sequence_sink.writes > 1);

    let limited_sequence = image_slash_star::EncodePolicy::new()
        .with_max_output_bytes(expected_sequence.len().saturating_sub(1) as u64);
    let mut limited_sequence_sink = RecordingTiffSink {
        bytes: Vec::new(),
        writes: 0,
    };
    let limited_sequence_error = match image_slash_star::encode_sequence_to_sink_with_policy(
        &tiff_sequence,
        ImageFormat::Tiff,
        &sequence_options,
        &limited_sequence,
        &mut limited_sequence_sink,
    ) {
        Ok(length) => {
            panic!("limited TIFF sequence sink unexpectedly accepted {length} bytes")
        }
        Err(error) => error,
    };
    assert!(matches!(
        limited_sequence_error,
        ImageError::LimitExceeded {
            format: Some(ImageFormat::Tiff),
            operation: image_slash_star::CodecOperation::SequenceEncode,
            resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
            ..
        }
    ));
    assert_eq!(limited_sequence_sink.writes, 0);
    assert!(limited_sequence_sink.bytes.is_empty());

    let mut sequence_mismatch_sink = RecordingTiffSink {
        bytes: Vec::new(),
        writes: 0,
    };
    let sequence_mismatch_error = match image_slash_star::encode_sequence_to_sink(
        &tiff_sequence,
        ImageFormat::Tiff,
        &mismatch_options,
        &mut sequence_mismatch_sink,
    ) {
        Ok(length) => panic!("mismatched TIFF sequence sink unexpectedly accepted {length} bytes"),
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

    struct CancellingTiffSink {
        bytes: Vec<u8>,
        token: image_slash_star::CancellationToken,
        writes: usize,
    }
    impl image_slash_star::OutputSink for CancellingTiffSink {
        fn write_all(&mut self, bytes: &[u8]) -> image_slash_star::ImageResult<()> {
            self.writes += 1;
            self.bytes.extend_from_slice(bytes);
            if self.writes == 1 {
                self.token.cancel();
            }
            Ok(())
        }
    }

    let raw_options = EncodeOptions::for_format(ImageFormat::Tiff);
    let expected_raw = image_slash_star::encode(&decoded.content, ImageFormat::Tiff, &raw_options)?;
    let token = image_slash_star::CancellationToken::new();
    let mut cancelling = CancellingTiffSink {
        bytes: Vec::new(),
        token: token.clone(),
        writes: 0,
    };
    let cancellation_error = match image_slash_star::encode_to_sink_with_token(
        &decoded.content,
        ImageFormat::Tiff,
        &raw_options,
        &token,
        &mut cancelling,
    ) {
        Ok(length) => panic!("cancelling TIFF sink unexpectedly accepted {length} bytes"),
        Err(error) => error,
    };
    assert_eq!(
        cancellation_error.kind(),
        image_slash_star::ImageErrorKind::Cancelled
    );
    assert_eq!(cancelling.writes, 1);
    assert_eq!(cancelling.bytes, &expected_raw[..8]);

    let sequence_token = image_slash_star::CancellationToken::new();
    let mut token_sequence_sink = RecordingTiffSink {
        bytes: Vec::new(),
        writes: 0,
    };
    assert_eq!(
        image_slash_star::encode_sequence_to_sink_with_token(
            &tiff_sequence,
            ImageFormat::Tiff,
            &sequence_options,
            &sequence_token,
            &mut token_sequence_sink,
        )?,
        expected_sequence.len()
    );
    assert_eq!(token_sequence_sink.bytes, expected_sequence);
    assert!(token_sequence_sink.writes > 1);

    let sequence_cancellation_token = image_slash_star::CancellationToken::new();
    let mut sequence_cancelling = CancellingTiffSink {
        bytes: Vec::new(),
        token: sequence_cancellation_token.clone(),
        writes: 0,
    };
    let sequence_cancellation_error = match image_slash_star::encode_sequence_to_sink_with_token(
        &tiff_sequence,
        ImageFormat::Tiff,
        &sequence_options,
        &sequence_cancellation_token,
        &mut sequence_cancelling,
    ) {
        Ok(length) => panic!("cancelling TIFF sequence sink unexpectedly accepted {length} bytes"),
        Err(error) => error,
    };
    assert_eq!(
        sequence_cancellation_error.kind(),
        image_slash_star::ImageErrorKind::Cancelled
    );
    assert_eq!(
        sequence_cancellation_error.stage(),
        Some(ImageErrorStage::SequenceEncode)
    );
    assert_eq!(sequence_cancelling.writes, 1);
    assert_eq!(sequence_cancelling.bytes, &expected_sequence[..8]);

    let multipage_sequence = image_slash_star::decode_sequence(&fs::read(
        root.join("tests/fixtures/input/images/tiff/multipage.tiff"),
    )?)?
    .into_inner();
    assert!(
        multipage_sequence.frames.len() > 1,
        "multipage TIFF fixture must retain multiple pages"
    );
    let mut multipage_options = EncodeOptions::for_format(ImageFormat::Tiff);
    if let EncodeOptions::Tiff(options) = &mut multipage_options {
        options.compression = Some(TiffCompression::Deflate);
    }
    let expected_multipage = image_slash_star::encode_sequence(
        &multipage_sequence,
        ImageFormat::Tiff,
        &multipage_options,
    )?;
    let mut multipage_sink = RecordingTiffSink {
        bytes: Vec::new(),
        writes: 0,
    };
    assert_eq!(
        image_slash_star::encode_sequence_to_sink(
            &multipage_sequence,
            ImageFormat::Tiff,
            &multipage_options,
            &mut multipage_sink,
        )?,
        expected_multipage.len()
    );
    assert_eq!(multipage_sink.bytes, expected_multipage);
    assert!(
        multipage_sink.writes >= multipage_sequence.frames.len() * 3,
        "multi-page TIFF delivery must retain per-page structural boundaries"
    );

    let limited_multipage = image_slash_star::EncodePolicy::new()
        .with_max_output_bytes(expected_multipage.len().saturating_sub(1) as u64);
    let mut limited_multipage_sink = RecordingTiffSink {
        bytes: Vec::new(),
        writes: 0,
    };
    let limited_multipage_error = match image_slash_star::encode_sequence_to_sink_with_policy(
        &multipage_sequence,
        ImageFormat::Tiff,
        &multipage_options,
        &limited_multipage,
        &mut limited_multipage_sink,
    ) {
        Ok(length) => panic!("limited multi-page TIFF sink unexpectedly accepted {length} bytes"),
        Err(error) => error,
    };
    assert!(matches!(
        limited_multipage_error,
        ImageError::LimitExceeded {
            format: Some(ImageFormat::Tiff),
            operation: image_slash_star::CodecOperation::SequenceEncode,
            resource: image_slash_star::ResourceLimit::EncodedOutputBytes,
            ..
        }
    ));
    assert_eq!(limited_multipage_sink.writes, 0);
    assert!(limited_multipage_sink.bytes.is_empty());

    let multipage_token = image_slash_star::CancellationToken::new();
    let mut cancelling_multipage = CancellingTiffSink {
        bytes: Vec::new(),
        token: multipage_token.clone(),
        writes: 0,
    };
    let multipage_cancellation_error = match image_slash_star::encode_sequence_to_sink_with_token(
        &multipage_sequence,
        ImageFormat::Tiff,
        &multipage_options,
        &multipage_token,
        &mut cancelling_multipage,
    ) {
        Ok(length) => {
            panic!("cancelling multi-page TIFF sink unexpectedly accepted {length} bytes")
        }
        Err(error) => error,
    };
    assert_eq!(
        multipage_cancellation_error.kind(),
        image_slash_star::ImageErrorKind::Cancelled
    );
    assert_eq!(
        multipage_cancellation_error.stage(),
        Some(ImageErrorStage::SequenceEncode)
    );
    assert_eq!(cancelling_multipage.writes, 1);
    assert_eq!(cancelling_multipage.bytes, &expected_multipage[..8]);

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
