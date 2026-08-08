//! Cargo-feature and target-capability behavior driven by Pillow fixtures.

use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::fs;
use std::path::Path;

use bytemuck as _;
use image_slash_star::{
    Capability, CapabilityRestriction, CapabilityTarget, CapabilityUnavailableReason, ColorType,
    DecodedImage, DecodedSequence, DiagnosticKind, EncodeOptions, EncodedImage, ImageDiagnostic,
    ImageError, ImageErrorStage, ImageFormat, ImageMode, ImagePalette, SequenceKind, SourceColor,
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
    let checked_pixels = vec![0; 16 * 16 * 3];
    let checked_pixels_ptr = checked_pixels.as_ptr();
    let checked = DecodedImage::try_new(16, 16, checked_pixels, ColorType::Rgb8)?;
    assert_eq!(checked.pixels.as_ptr(), checked_pixels_ptr);
    assert_eq!(checked, encode_input);
    assert_eq!(
        DecodedImage::try_with_mode(16, 16, vec![0; 16 * 16 * 3], ImageMode::Rgb8)?,
        encode_input
    );
    let checked_indexed = DecodedImage::try_with_mode(1, 1, vec![0], ImageMode::P8)?
        .try_with_palette(ImagePalette {
            rgb: vec![0, 0, 0],
            alpha: Vec::new(),
        })?;
    assert_eq!(checked_indexed.mode, ImageMode::P8);
    assert!(matches!(
        DecodedImage::try_new(1, 1, vec![0], ColorType::Rgb8),
        Err(ImageError::Dimensions { .. })
    ));
    assert!(matches!(
        DecodedImage::try_new(1, 1, vec![0], ColorType::L8)?.try_with_palette(ImagePalette {
            rgb: vec![0, 0, 0],
            alpha: Vec::new(),
        }),
        Err(ImageError::Parameter { .. })
    ));
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

    let generic_avif = fs::read(
        root.join("tests/fixtures/input/images/avif")
            .join("generic_mif1.avif"),
    )?;
    assert_eq!(
        image_slash_star::detect_format(&generic_avif),
        Err(ImageError::UnknownFormat)
    );
    assert!(matches!(
        image_slash_star::decode_with_format(&generic_avif, ImageFormat::Avif),
        Err(ImageError::Malformed {
            format: ImageFormat::Avif,
            stage: Some(ImageErrorStage::StillDecode),
            ..
        })
    ));

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
                image_slash_star::decode_with_format(&bytes, format),
                Err(expected.clone())
            );
            assert_eq!(
                image_slash_star::decode_with_format_and_policy(
                    &bytes,
                    format,
                    &image_slash_star::DecodePolicy::default()
                ),
                Err(expected.clone())
            );
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
                image_slash_star::decode_with_format(&bytes, format),
                Err(expected_decode.clone())
            );
            assert_eq!(
                image_slash_star::decode_with_format_and_policy(
                    &bytes,
                    format,
                    &image_slash_star::DecodePolicy::default()
                ),
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
        assert!(matches!(
            image_slash_star::decode_with_format(source.bytes(), wrong_format),
            Err(ImageError::Parameter {
                format: Some(actual),
                stage: Some(ImageErrorStage::StillDecode),
                ..
            }) if actual == wrong_format
        ));
        let decoded = source.decode()?;
        assert_eq!(decoded.format, format);
        assert_eq!(decoded.content.mode, info.mode);
        assert_eq!(
            [decoded.content.width, decoded.content.height],
            expected_size
        );
        assert_eq!(
            image_slash_star::decode_with_format(source.bytes(), format)?,
            decoded.clone(),
            "{name} explicit-format decode"
        );
        assert_eq!(
            image_slash_star::decode_with_format_and_policy(
                source.bytes(),
                format,
                &image_slash_star::DecodePolicy::default()
            )?,
            decoded.clone(),
            "{name} explicit-format policy decode"
        );
        let strict_policy = image_slash_star::DecodePolicy::default()
            .with_max_encoded_bytes((source.bytes().len() as u64).saturating_sub(1));
        assert!(matches!(
            image_slash_star::decode_with_format_and_policy(source.bytes(), format, &strict_policy),
            Err(ImageError::LimitExceeded {
                format: None,
                operation: image_slash_star::CodecOperation::StillDecode,
                ..
            })
        ));
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

        // Keep a fixture-backed lossless WebP token path in this contract so
        // VP8L Huffman canonical-code and tree-insertion checkpoints are
        // exercised without adding a
        // Pillow-parity row or a synthetic coverage-only input.
        let data = fs::read(root.join("tests/fixtures/input/images/webp/lossless.webp"))?;
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
            "an uncancelled lossless WebP encode remains byte-identical"
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
                return Err(format!(
                    "cancelled lossless WebP encode returned {} bytes",
                    bytes.len()
                )
                .into());
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
    use image_slash_star::{
        AvifAuxiliaryRelationship, AvifColorProperties, AvifGridProperties,
        AvifItemColorProperties, AvifItemIccProfile, AvifItemRelationship, RawIccProfile,
        SourceAlpha,
    };

    // SourceAlpha is Rust source-provenance metadata, not a Pillow-observable
    // parity field. The AVIF case below uses the real committed fixture in
    // this feature-gated integration contract and adds no parity row.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let append_premultiplied_relationship =
        |input: &[u8]| -> Result<Vec<u8>, Box<dyn std::error::Error>> {
            let iref_type = input
                .windows(4)
                .position(|window| window == b"iref")
                .ok_or("AVIF alpha fixture has no iref box")?;
            let iref_start = iref_type
                .checked_sub(4)
                .ok_or("AVIF iref box has no size field")?;
            let iref_size =
                u32::from_be_bytes(input[iref_start..iref_start + 4].try_into()?) as usize;
            let iref_end = iref_start
                .checked_add(iref_size)
                .ok_or("AVIF iref box end overflowed")?;
            let child = avif_box(b"prem", &[0, 2, 0, 1, 0, 1]);
            let delta = u32::try_from(child.len())?;
            let mut output = Vec::with_capacity(
                input
                    .len()
                    .checked_add(child.len())
                    .ok_or("AVIF prem relationship output length overflowed")?,
            );
            output.extend_from_slice(&input[..iref_end]);
            output.extend_from_slice(&child);
            output.extend_from_slice(&input[iref_end..]);

            let iref_size = u32::from_be_bytes(output[iref_start..iref_start + 4].try_into()?)
                .checked_add(delta)
                .ok_or("AVIF iref size overflowed")?;
            output[iref_start..iref_start + 4].copy_from_slice(&iref_size.to_be_bytes());
            let meta_start = avif_box_offset(&output, b"meta")?;
            let meta_size = u32::from_be_bytes(output[meta_start..meta_start + 4].try_into()?)
                .checked_add(delta)
                .ok_or("AVIF meta size overflowed")?;
            output[meta_start..meta_start + 4].copy_from_slice(&meta_size.to_be_bytes());

            let iloc_type = output
                .windows(4)
                .position(|window| window == b"iloc")
                .ok_or("AVIF alpha fixture has no iloc box")?;
            let iloc = iloc_type
                .checked_sub(4)
                .ok_or("AVIF iloc box has no size field")?;
            if output[iloc + 12] != 0x44 || output[iloc + 13] != 0 {
                return Err("AVIF alpha fixture iloc layout changed".into());
            }
            let item_count = u16::from_be_bytes(output[iloc + 14..iloc + 16].try_into()?);
            let mut cursor = iloc + 16;
            for _ in 0..item_count {
                cursor = cursor
                    .checked_add(2 + 2)
                    .ok_or("AVIF iloc item offset overflowed")?;
                let extent_count = u16::from_be_bytes(output[cursor..cursor + 2].try_into()?);
                cursor += 2;
                for _ in 0..extent_count {
                    let offset_end = cursor.checked_add(4).ok_or("AVIF iloc offset overflowed")?;
                    let old_offset = u32::from_be_bytes(output[cursor..offset_end].try_into()?)
                        .checked_add(delta)
                        .ok_or("AVIF iloc extent offset overflowed")?;
                    output[cursor..offset_end].copy_from_slice(&old_offset.to_be_bytes());
                    cursor = cursor.checked_add(8).ok_or("AVIF iloc extent overflowed")?;
                }
            }
            Ok(output)
        };
    let append_item_color_property_association = |input: &[u8]| -> Result<
        Vec<u8>,
        Box<dyn std::error::Error>,
    > {
        let ipma_type = input
            .windows(4)
            .position(|window| window == b"ipma")
            .ok_or("AVIF alpha fixture has no ipma box")?;
        let ipma = ipma_type
            .checked_sub(4)
            .ok_or("AVIF ipma box has no size field")?;
        let ipma_size = u32::from_be_bytes(input[ipma..ipma + 4].try_into()?) as usize;
        let ipma_end = ipma
            .checked_add(ipma_size)
            .ok_or("AVIF ipma box end overflowed")?;
        if input.get(ipma + 8) != Some(&0) {
            return Err("AVIF alpha fixture ipma version changed".into());
        }
        let entry_count = u32::from_be_bytes(input[ipma + 12..ipma + 16].try_into()?);
        let mut cursor = ipma + 16;
        let mut insertion = None;
        let mut association_count_offset = None;
        for _ in 0..entry_count {
            let item_id_end = cursor
                .checked_add(2)
                .ok_or("AVIF ipma item ID offset overflowed")?;
            let item_id = u16::from_be_bytes(input[cursor..item_id_end].try_into()?);
            let count_offset = item_id_end;
            let association_count = *input
                .get(count_offset)
                .ok_or("AVIF ipma association count is missing")?;
            let association_start = count_offset
                .checked_add(1)
                .ok_or("AVIF ipma association start overflowed")?;
            let association_end = association_start
                .checked_add(usize::from(association_count))
                .ok_or("AVIF ipma association end overflowed")?;
            if item_id == 2 {
                insertion = Some(association_end);
                association_count_offset = Some(count_offset);
            }
            cursor = association_end;
        }
        if cursor != ipma_end {
            return Err("AVIF alpha fixture ipma layout changed".into());
        }
        let insertion = insertion.ok_or("AVIF alpha fixture has no auxiliary item")?;
        let association_count_offset = association_count_offset
            .ok_or("AVIF alpha fixture auxiliary association count is missing")?;
        let mut output = Vec::with_capacity(
            input
                .len()
                .checked_add(1)
                .ok_or("AVIF item color output length overflowed")?,
        );
        output.extend_from_slice(&input[..insertion]);
        output.push(4);
        output.extend_from_slice(&input[insertion..]);
        output[association_count_offset] = output[association_count_offset]
            .checked_add(1)
            .ok_or("AVIF ipma association count overflowed")?;

        for kind in [b"ipma", b"iprp", b"meta"] {
            let type_offset = output
                .windows(4)
                .position(|window| window == kind)
                .ok_or_else(|| format!("AVIF alpha fixture has no {kind:?} box"))?;
            let size_start = type_offset
                .checked_sub(4)
                .ok_or_else(|| format!("AVIF {kind:?} box has no size field"))?;
            let size = u32::from_be_bytes(output[size_start..size_start + 4].try_into()?)
                .checked_add(1)
                .ok_or("AVIF metadata box size overflowed")?;
            output[size_start..size_start + 4].copy_from_slice(&size.to_be_bytes());
        }

        let iloc_type = output
            .windows(4)
            .position(|window| window == b"iloc")
            .ok_or("AVIF alpha fixture has no iloc box")?;
        let iloc = iloc_type
            .checked_sub(4)
            .ok_or("AVIF iloc box has no size field")?;
        if output[iloc + 12] != 0x44 || output[iloc + 13] != 0 {
            return Err("AVIF alpha fixture iloc layout changed".into());
        }
        let item_count = u16::from_be_bytes(output[iloc + 14..iloc + 16].try_into()?);
        let mut iloc_cursor = iloc + 16;
        for _ in 0..item_count {
            iloc_cursor = iloc_cursor
                .checked_add(4)
                .ok_or("AVIF iloc item offset overflowed")?;
            let extent_count = u16::from_be_bytes(output[iloc_cursor..iloc_cursor + 2].try_into()?);
            iloc_cursor = iloc_cursor
                .checked_add(2)
                .ok_or("AVIF iloc extent count overflowed")?;
            for _ in 0..extent_count {
                let offset_end = iloc_cursor
                    .checked_add(4)
                    .ok_or("AVIF iloc offset overflowed")?;
                let old_offset = u32::from_be_bytes(output[iloc_cursor..offset_end].try_into()?)
                    .checked_add(1)
                    .ok_or("AVIF iloc extent offset overflowed")?;
                output[iloc_cursor..offset_end].copy_from_slice(&old_offset.to_be_bytes());
                iloc_cursor = iloc_cursor
                    .checked_add(8)
                    .ok_or("AVIF iloc extent overflowed")?;
            }
        }
        Ok(output)
    };
    let append_item_icc_profile_association =
        |input: &[u8]| -> Result<Vec<u8>, Box<dyn std::error::Error>> {
            let ipco_type = input
                .windows(4)
                .position(|window| window == b"ipco")
                .ok_or("AVIF alpha fixture has no ipco box")?;
            let ipco = ipco_type
                .checked_sub(4)
                .ok_or("AVIF ipco box has no size field")?;
            let ipco_size = u32::from_be_bytes(input[ipco..ipco + 4].try_into()?) as usize;
            let ipco_end = ipco
                .checked_add(ipco_size)
                .ok_or("AVIF ipco box end overflowed")?;
            let property = avif_box(b"colr", b"profitem-icc");
            let mut output = Vec::with_capacity(
                input
                    .len()
                    .checked_add(property.len())
                    .and_then(|length| length.checked_add(1))
                    .ok_or("AVIF item ICC output length overflowed")?,
            );
            output.extend_from_slice(&input[..ipco_end]);
            output.extend_from_slice(&property);
            output.extend_from_slice(&input[ipco_end..]);
            let new_ipco_end = ipco_end
                .checked_add(property.len())
                .ok_or("AVIF expanded ipco end overflowed")?;
            let mut property_cursor = ipco
                .checked_add(8)
                .ok_or("AVIF ipco property cursor overflowed")?;
            let mut property_count = 0usize;
            while property_cursor < new_ipco_end {
                let size_end = property_cursor
                    .checked_add(4)
                    .ok_or("AVIF property size offset overflowed")?;
                let property_size = usize::try_from(u32::from_be_bytes(
                    output[property_cursor..size_end].try_into()?,
                ))?;
                if property_size < 8 {
                    return Err("AVIF item ICC property has an invalid size".into());
                }
                property_cursor = property_cursor
                    .checked_add(property_size)
                    .ok_or("AVIF property cursor overflowed")?;
                if property_cursor > new_ipco_end {
                    return Err("AVIF item ICC property exceeds ipco".into());
                }
                property_count = property_count
                    .checked_add(1)
                    .ok_or("AVIF property count overflowed")?;
            }
            if property_cursor != new_ipco_end {
                return Err("AVIF ipco properties do not fill the box".into());
            }
            let property_index = u8::try_from(property_count)?;

            let ipma_type = output
                .windows(4)
                .position(|window| window == b"ipma")
                .ok_or("AVIF alpha fixture has no ipma box")?;
            let ipma = ipma_type
                .checked_sub(4)
                .ok_or("AVIF ipma box has no size field")?;
            let ipma_size =
                usize::try_from(u32::from_be_bytes(output[ipma..ipma + 4].try_into()?))?;
            let ipma_end = ipma
                .checked_add(ipma_size)
                .ok_or("AVIF ipma box end overflowed")?;
            if output.get(ipma + 8) != Some(&0) {
                return Err("AVIF alpha fixture ipma version changed".into());
            }
            let entry_count = u32::from_be_bytes(output[ipma + 12..ipma + 16].try_into()?);
            let mut cursor = ipma + 16;
            let mut insertion = None;
            let mut association_count_offset = None;
            for _ in 0..entry_count {
                let item_id_end = cursor
                    .checked_add(2)
                    .ok_or("AVIF ipma item ID offset overflowed")?;
                let item_id = u16::from_be_bytes(output[cursor..item_id_end].try_into()?);
                let count_offset = item_id_end;
                let association_count = *output
                    .get(count_offset)
                    .ok_or("AVIF ipma association count is missing")?;
                let association_start = count_offset
                    .checked_add(1)
                    .ok_or("AVIF ipma association start overflowed")?;
                let association_end = association_start
                    .checked_add(usize::from(association_count))
                    .ok_or("AVIF ipma association end overflowed")?;
                if item_id == 2 {
                    insertion = Some(association_end);
                    association_count_offset = Some(count_offset);
                }
                cursor = association_end;
            }
            if cursor != ipma_end {
                return Err("AVIF alpha fixture ipma layout changed".into());
            }
            let insertion = insertion.ok_or("AVIF alpha fixture has no auxiliary item")?;
            let association_count_offset = association_count_offset
                .ok_or("AVIF alpha fixture auxiliary association count is missing")?;
            output.insert(insertion, property_index);
            output[association_count_offset] = output[association_count_offset]
                .checked_add(1)
                .ok_or("AVIF ipma association count overflowed")?;

            let property_delta = u32::try_from(property.len())?;
            let total_delta = property_delta
                .checked_add(1)
                .ok_or("AVIF item ICC metadata delta overflowed")?;
            for kind in [b"ipco", b"ipma"] {
                let type_offset = output
                    .windows(4)
                    .position(|window| window == kind)
                    .ok_or_else(|| format!("AVIF alpha fixture has no {kind:?} box"))?;
                let size_start = type_offset
                    .checked_sub(4)
                    .ok_or_else(|| format!("AVIF {kind:?} box has no size field"))?;
                let delta = if kind == b"ipco" { property_delta } else { 1 };
                let size = u32::from_be_bytes(output[size_start..size_start + 4].try_into()?)
                    .checked_add(delta)
                    .ok_or("AVIF metadata box size overflowed")?;
                output[size_start..size_start + 4].copy_from_slice(&size.to_be_bytes());
            }
            for kind in [b"iprp", b"meta"] {
                let type_offset = output
                    .windows(4)
                    .position(|window| window == kind)
                    .ok_or_else(|| format!("AVIF alpha fixture has no {kind:?} box"))?;
                let size_start = type_offset
                    .checked_sub(4)
                    .ok_or_else(|| format!("AVIF {kind:?} box has no size field"))?;
                let size = u32::from_be_bytes(output[size_start..size_start + 4].try_into()?)
                    .checked_add(total_delta)
                    .ok_or("AVIF metadata box size overflowed")?;
                output[size_start..size_start + 4].copy_from_slice(&size.to_be_bytes());
            }

            let iloc_type = output
                .windows(4)
                .position(|window| window == b"iloc")
                .ok_or("AVIF alpha fixture has no iloc box")?;
            let iloc = iloc_type
                .checked_sub(4)
                .ok_or("AVIF iloc box has no size field")?;
            if output[iloc + 12] != 0x44 || output[iloc + 13] != 0 {
                return Err("AVIF alpha fixture iloc layout changed".into());
            }
            let item_count = u16::from_be_bytes(output[iloc + 14..iloc + 16].try_into()?);
            let mut iloc_cursor = iloc + 16;
            for _ in 0..item_count {
                iloc_cursor = iloc_cursor
                    .checked_add(4)
                    .ok_or("AVIF iloc item offset overflowed")?;
                let extent_count =
                    u16::from_be_bytes(output[iloc_cursor..iloc_cursor + 2].try_into()?);
                iloc_cursor = iloc_cursor
                    .checked_add(2)
                    .ok_or("AVIF iloc extent count overflowed")?;
                for _ in 0..extent_count {
                    let offset_end = iloc_cursor
                        .checked_add(4)
                        .ok_or("AVIF iloc offset overflowed")?;
                    let old_offset =
                        u32::from_be_bytes(output[iloc_cursor..offset_end].try_into()?)
                            .checked_add(total_delta)
                            .ok_or("AVIF iloc extent offset overflowed")?;
                    output[iloc_cursor..offset_end].copy_from_slice(&old_offset.to_be_bytes());
                    iloc_cursor = iloc_cursor
                        .checked_add(8)
                        .ok_or("AVIF iloc extent overflowed")?;
                }
            }
            Ok(output)
        };
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
    if cfg!(feature = "avif") {
        // The grid payload is source-provenance metadata outside the Pillow
        // parity schema. Inspection is portable, so retain this assertion in
        // the WASI and native feature-gate lanes alike.
        let bytes = fs::read(root.join("tests/fixtures/input/images/avif/grid.avif"))?;
        let expected_grid = AvifGridProperties::new(0, 0, 2, 1, 80, 80);
        let inspected = image_slash_star::inspect(&bytes)?;
        assert_eq!(
            inspected.source.avif_grid_properties(),
            Some(expected_grid),
            "grid inspect topology"
        );
        assert_eq!(expected_grid.version(), 0, "grid payload version");
        assert_eq!(expected_grid.flags(), 0, "grid payload flags");
        assert_eq!(expected_grid.rows(), 2, "grid row count");
        assert_eq!(expected_grid.columns(), 1, "grid column count");
        assert_eq!(expected_grid.output_width(), 80, "grid output width");
        assert_eq!(expected_grid.output_height(), 80, "grid output height");
    }

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
            let relationship = AvifAuxiliaryRelationship::new(2, 1);
            assert_eq!(
                relationship.auxiliary_item_id(),
                2,
                "{name} auxiliary item identity"
            );
            assert_eq!(
                relationship.target_item_id(),
                1,
                "{name} auxiliary target identity"
            );
            let expected_relationship = Some(relationship);
            let expected_relationships = [relationship];
            assert_eq!(
                info.source.avif_auxiliary_relationship(),
                expected_relationship,
                "{name} inspect auxiliary relationship"
            );
            assert_eq!(
                info.source.avif_auxiliary_relationships(),
                expected_relationships.as_slice(),
                "{name} inspect auxiliary relationships"
            );
            assert_eq!(
                decoded.content.source.avif_auxiliary_relationship(),
                expected_relationship,
                "{name} decode auxiliary relationship"
            );
            assert_eq!(
                decoded.content.source.avif_auxiliary_relationships(),
                expected_relationships.as_slice(),
                "{name} decode auxiliary relationships"
            );
            assert!(
                info.source.avif_item_relationships().is_empty(),
                "{name} alpha edges are not duplicated as generic relationships"
            );
            let sequence = image_slash_star::decode_sequence(&bytes)?;
            assert_eq!(
                sequence.content.frames[0].image.source.alpha(),
                expected,
                "{name} sequence"
            );
            assert_eq!(
                sequence.content.frames[0]
                    .image
                    .source
                    .avif_auxiliary_relationship(),
                expected_relationship,
                "{name} sequence auxiliary relationship"
            );
        } else if name == "avif opaque" {
            assert!(
                info.source.avif_grid_item_ids().is_empty(),
                "{name} inspect grid item IDs"
            );
            assert!(
                info.source.avif_auxiliary_relationships().is_empty(),
                "{name} inspect auxiliary relationships"
            );
            assert!(
                decoded
                    .content
                    .source
                    .avif_auxiliary_relationships()
                    .is_empty(),
                "{name} decode auxiliary relationships"
            );
            assert!(
                info.source.avif_item_relationships().is_empty(),
                "{name} generic relationships"
            );
        }
    }

    if !cfg!(target_arch = "wasm32") && cfg!(feature = "avif") {
        // The grid fixture has one primary grid item, two derived color items,
        // and one alpha auxiliary item for each derived color item. This is
        // source-provenance evidence outside the Pillow parity schema.
        let bytes = fs::read(root.join("tests/fixtures/input/images/avif/grid.avif"))?;
        let expected = [
            AvifAuxiliaryRelationship::new(5, 2),
            AvifAuxiliaryRelationship::new(6, 3),
        ];
        let expected_item_relationships = [
            AvifItemRelationship::new(*b"dimg", 1, 2),
            AvifItemRelationship::new(*b"dimg", 1, 3),
        ];
        let inspected = image_slash_star::inspect(&bytes)?;
        assert_eq!(
            inspected.source.alpha(),
            Some(SourceAlpha::Auxiliary),
            "grid inspect alpha"
        );
        assert_eq!(
            inspected.source.avif_auxiliary_relationship(),
            None,
            "grid has no direct primary-item alpha relationship"
        );
        assert_eq!(
            inspected.source.avif_auxiliary_relationships(),
            expected.as_slice(),
            "grid inspect auxiliary relationships"
        );
        assert_eq!(
            inspected.source.avif_grid_item_ids(),
            [2, 3].as_slice(),
            "grid inspect derived item IDs"
        );
        assert_eq!(
            inspected.source.avif_grid_properties(),
            Some(AvifGridProperties::new(0, 0, 2, 1, 80, 80)),
            "grid inspect topology"
        );
        assert_eq!(
            inspected.source.avif_item_relationships(),
            expected_item_relationships.as_slice(),
            "grid inspect generic relationships"
        );

        let decoded = image_slash_star::decode(&bytes)?;
        assert_eq!(
            decoded.content.source.avif_auxiliary_relationships(),
            expected.as_slice(),
            "grid decode auxiliary relationships"
        );
        assert_eq!(
            decoded.content.source.avif_grid_item_ids(),
            [2, 3].as_slice(),
            "grid decode derived item IDs"
        );
        assert_eq!(
            decoded.content.source.avif_grid_properties(),
            Some(AvifGridProperties::new(0, 0, 2, 1, 80, 80)),
            "grid decode topology"
        );
        assert_eq!(
            decoded.content.source.avif_item_relationships(),
            expected_item_relationships.as_slice(),
            "grid decode generic relationships"
        );
        let sequence = image_slash_star::decode_sequence(&bytes)?;
        assert_eq!(
            sequence.content.frames[0]
                .image
                .source
                .avif_auxiliary_relationships(),
            expected.as_slice(),
            "grid sequence auxiliary relationships"
        );
        assert_eq!(
            sequence.content.frames[0].image.source.avif_grid_item_ids(),
            [2, 3].as_slice(),
            "grid sequence derived item IDs"
        );
        assert_eq!(
            sequence.content.frames[0]
                .image
                .source
                .avif_grid_properties(),
            Some(AvifGridProperties::new(0, 0, 2, 1, 80, 80)),
            "grid sequence topology"
        );
        assert_eq!(
            sequence.content.frames[0]
                .image
                .source
                .avif_item_relationships(),
            expected_item_relationships.as_slice(),
            "grid sequence generic relationships"
        );

        // `prem` is an AVIF source relationship from an alpha item to the
        // color item it qualifies. It is not a Pillow-observable field, so
        // this mutation stays in the Rust source-provenance contract. The
        // existing alpha relationship remains present to prove the two facts
        // are retained independently, and decoded pixels remain unchanged.
        let alpha = fs::read(root.join("tests/fixtures/input/images/avif/alpha.avif"))?;
        let premultiplied = append_premultiplied_relationship(&alpha)?;
        let expected_premultiplied = [AvifItemRelationship::new(*b"prem", 2, 1)];
        let inspected = image_slash_star::inspect(&premultiplied)?;
        assert_eq!(
            inspected.source.alpha(),
            Some(SourceAlpha::Auxiliary),
            "prem inspect preserves separate alpha semantics"
        );
        assert_eq!(
            inspected.source.avif_premultiplied_relationships(),
            expected_premultiplied.as_slice(),
            "prem inspect relationship"
        );
        assert_eq!(
            inspected.source.avif_item_relationships(),
            expected_premultiplied.as_slice(),
            "prem inspect generic relationship"
        );
        let baseline_decoded = image_slash_star::decode(&alpha)?;
        let decoded = image_slash_star::decode(&premultiplied)?;
        assert_eq!(decoded.content.pixels, baseline_decoded.content.pixels);
        assert_eq!(
            decoded.content.source.avif_premultiplied_relationships(),
            expected_premultiplied.as_slice(),
            "prem decode relationship"
        );
        let sequence = image_slash_star::decode_sequence(&premultiplied)?;
        assert_eq!(
            sequence.content.frames[0]
                .image
                .source
                .avif_premultiplied_relationships(),
            expected_premultiplied.as_slice(),
            "prem sequence relationship"
        );

        // A non-primary item may declare CICP through the same typed `colr`/
        // `nclx` property vocabulary as the primary item. Pillow exposes no
        // item-level source-color result, so this mutation remains a Rust
        // provenance witness and deliberately adds no parity row. The
        // declaration must retain its item identity without changing the
        // primary color result or decoded samples.
        let item_color_bytes = append_item_color_property_association(&alpha)?;
        let expected_item_color = AvifColorProperties {
            color_primaries: 1,
            transfer_characteristics: 13,
            matrix_coefficients: 6,
            full_range: true,
        };
        let expected_item_colors = [AvifItemColorProperties::new(2, expected_item_color)];
        let item_color_inspected = image_slash_star::inspect(&item_color_bytes)?;
        assert_eq!(
            item_color_inspected.source.avif_item_color_properties(),
            expected_item_colors.as_slice(),
            "item color inspect declaration"
        );
        assert_eq!(
            item_color_inspected.source.avif_item_color_properties()[0].item_id(),
            2,
            "item color inspect identity"
        );
        assert_eq!(
            item_color_inspected.source.avif_item_color_properties()[0].color(),
            expected_item_color,
            "item color inspect CICP"
        );
        assert_eq!(
            item_color_inspected.source_color.avif_color(),
            Some(expected_item_color),
            "item color does not replace primary CICP"
        );
        let item_color_decoded = image_slash_star::decode(&item_color_bytes)?;
        assert_eq!(
            item_color_decoded.content.pixels, baseline_decoded.content.pixels,
            "item color preserves decoded pixels"
        );
        assert_eq!(
            item_color_decoded
                .content
                .source
                .avif_item_color_properties(),
            expected_item_colors.as_slice(),
            "item color decode declaration"
        );
        let item_color_sequence = image_slash_star::decode_sequence(&item_color_bytes)?;
        assert_eq!(
            item_color_sequence.content.frames[0]
                .image
                .source
                .avif_item_color_properties(),
            expected_item_colors.as_slice(),
            "item color sequence declaration"
        );

        // A non-primary item may carry an ICC `colr` property independently
        // from its CICP declaration. Pillow exposes neither item identity nor
        // item ICC results, so this remains source-provenance evidence without
        // a parity row; decoded samples and the primary color declaration stay
        // unchanged.
        let item_icc_bytes = append_item_icc_profile_association(&item_color_bytes)?;
        let expected_item_icc = [AvifItemIccProfile::new(
            2,
            RawIccProfile {
                keyword: b"prof".to_vec(),
                data: b"item-icc".to_vec(),
            },
        )];
        let item_icc_inspected = image_slash_star::inspect(&item_icc_bytes)?;
        assert_eq!(
            item_icc_inspected.source.avif_item_icc_profiles(),
            expected_item_icc.as_slice(),
            "item ICC inspect declaration"
        );
        let item_icc_decoded = image_slash_star::decode(&item_icc_bytes)?;
        assert_eq!(
            item_icc_decoded.content.source.avif_item_icc_profiles(),
            expected_item_icc.as_slice(),
            "item ICC decode declaration"
        );
        assert_eq!(
            item_icc_decoded.content.pixels,
            baseline_decoded.content.pixels
        );
        let item_icc_sequence = image_slash_star::decode_sequence(&item_icc_bytes)?;
        assert_eq!(
            item_icc_sequence.content.frames[0]
                .image
                .source
                .avif_item_icc_profiles(),
            expected_item_icc.as_slice(),
            "item ICC sequence declaration"
        );
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
    use image_slash_star::{EncodedImage, EncodedImageDecodeState, EncodedImageView};

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
        assert_eq!(
            source.decode_state(),
            EncodedImageDecodeState::NotAttempted,
            "{name} still cache starts empty"
        );
        assert_eq!(
            source.sequence_decode_state(),
            EncodedImageDecodeState::NotAttempted,
            "{name} sequence cache starts empty"
        );
        let count = source.info().frame_count.unwrap_or(1);
        let sequence = image_slash_star::decode_sequence(&data)?;
        let cached_sequence = source.decode_sequence()?;
        assert_eq!(cached_sequence, sequence, "{name} cached sequence");
        assert_eq!(
            source.sequence_decode_state(),
            EncodedImageDecodeState::Succeeded,
            "{name} sequence cache succeeds"
        );
        assert!(source.is_sequence_decoded(), "{name} sequence materialized");
        assert_eq!(
            source.decode_sequence()?,
            cached_sequence,
            "{name} sequence cache is reused"
        );
        assert_eq!(
            source.decode_sequence_with_policy(&image_slash_star::DecodePolicy::default())?,
            cached_sequence,
            "{name} default sequence policy uses the cache"
        );
        let clone = source.clone();
        assert_eq!(
            clone.sequence_decode_state(),
            EncodedImageDecodeState::Succeeded,
            "{name} clone observes sequence cache"
        );
        let still = source.decode()?;
        assert_eq!(
            source.decode_state(),
            EncodedImageDecodeState::Succeeded,
            "{name} still cache succeeds"
        );
        assert!(source.is_decoded(), "{name} still materialized");
        assert_eq!(
            still.content, sequence.content.frames[0].image,
            "{name} still first frame"
        );
        if count > 1 {
            let strict =
                image_slash_star::DecodePolicy::default().with_max_frames(count.saturating_sub(1));
            assert!(
                source.decode_sequence_with_policy(&strict).is_err(),
                "{name} strict sequence policy"
            );
            assert_eq!(
                source.sequence_decode_state(),
                EncodedImageDecodeState::Succeeded,
                "{name} policy failure does not poison cache"
            );
        }
        assert_eq!(
            u32::try_from(sequence.content.frames.len()).unwrap_or(u32::MAX),
            count,
            "{name} frame count"
        );
        let view = EncodedImageView::new(&data)?;
        for (index, frame) in sequence.content.frames.iter().enumerate() {
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
            // Pin the BMFF creation/modification time so repeated structural
            // delivery calls compare the same bytes instead of wall-clock
            // metadata. This is a test-input determinism control, not a
            // Pillow-parity field or a production timestamp policy.
            options.sequence_time = Some(1);
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
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
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

        // The committed 33x33 JPEG fixture reaches the forward-DCT and
        // quantization pass after the row-level conversion and sampling
        // checkpoints. Pillow has no caller work budget or equivalent result,
        // so this is a Rust-only interior contract with no parity row.
        let dct_data = fs::read(root.join("tests/fixtures/input/images/jpeg/33x33.jpg"))?;
        let dct_image = image_slash_star::decode(&dct_data)?.content;
        let dct_expected = image_slash_star::encode(&dct_image, ImageFormat::Jpeg, &options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &dct_image,
                ImageFormat::Jpeg,
                &options,
                &unlimited,
            )?,
            dct_expected,
            "an ample DCT budget preserves fixture-derived bytes"
        );
        let dct_policy = image_slash_star::EncodePolicy::new().with_max_work_units(70);
        let dct_error = match image_slash_star::encode_with_policy(
            &dct_image,
            ImageFormat::Jpeg,
            &options,
            &dct_policy,
        ) {
            Ok(_) => return Err("JPEG DCT work budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            dct_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 70,
                observed: 71,
            }
        ));
        let mut dct_sink = vec![0x5D];
        let dct_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &dct_image,
            ImageFormat::Jpeg,
            &options,
            &dct_policy,
            &mut dct_sink,
        ) {
            Ok(_) => return Err("JPEG DCT work budget unexpectedly wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            dct_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 70,
                observed: 71,
            }
        ));
        assert_eq!(dct_sink, vec![0x5D]);

        // The committed 257x129 JPEG fixture reaches the chroma downsample
        // inner loop after the row-level RGB conversion checkpoints. The
        // token path now charges every 1,024 output pixels, while the
        // no-token path remains monomorphized without polling overhead.
        // Pillow has no caller work budget or equivalent sink/result contract,
        // so this is Rust-only evidence with no parity row, parity fixture,
        // diagnostic origin, or coverage-only hook.
        let downsample_data = fs::read(root.join("tests/fixtures/input/images/jpeg/large.jpg"))?;
        let downsample_image = image_slash_star::decode(&downsample_data)?.content;
        let downsample_expected =
            image_slash_star::encode(&downsample_image, ImageFormat::Jpeg, &options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &downsample_image,
                ImageFormat::Jpeg,
                &options,
                &unlimited,
            )?,
            downsample_expected,
            "an ample downsample budget preserves fixture-derived bytes"
        );
        let downsample_policy = image_slash_star::EncodePolicy::new().with_max_work_units(228);
        let downsample_error = match image_slash_star::encode_with_policy(
            &downsample_image,
            ImageFormat::Jpeg,
            &options,
            &downsample_policy,
        ) {
            Ok(_) => return Err("JPEG downsample work budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            downsample_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 228,
                observed: 229,
            }
        ));
        let mut downsample_sink = vec![0x5E];
        let downsample_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &downsample_image,
            ImageFormat::Jpeg,
            &options,
            &downsample_policy,
            &mut downsample_sink,
        ) {
            Ok(_) => return Err("JPEG downsample work budget unexpectedly wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            downsample_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 228,
                observed: 229,
            }
        ));
        assert_eq!(downsample_sink, vec![0x5E]);

        // Optimized baseline JPEGs first gather Huffman symbol frequencies.
        // The committed large fixture reaches that coefficient scan after the
        // row/block/downsample checkpoints; charge one interval per 1,024 AC
        // coefficients so a wide optimized encode cannot hide the scan behind
        // one MCU-row poll. Pillow has no caller work budget or equivalent
        // sink/result contract, so this remains Rust-only evidence with no
        // parity row, parity fixture, diagnostic origin, or coverage-only hook.
        let mut optimized_jpeg_options = image_slash_star::JpegEncodeOptions::default();
        optimized_jpeg_options.optimize = Some(true);
        let optimized_options = EncodeOptions::from(optimized_jpeg_options);
        let optimized_expected =
            image_slash_star::encode(&downsample_image, ImageFormat::Jpeg, &optimized_options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &downsample_image,
                ImageFormat::Jpeg,
                &optimized_options,
                &unlimited,
            )?,
            optimized_expected,
            "an ample Huffman-frequency budget preserves fixture-derived bytes"
        );
        let frequency_policy = image_slash_star::EncodePolicy::new().with_max_work_units(1_220);
        let frequency_error = match image_slash_star::encode_with_policy(
            &downsample_image,
            ImageFormat::Jpeg,
            &optimized_options,
            &frequency_policy,
        ) {
            Ok(_) => return Err("JPEG Huffman-frequency budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            frequency_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 1_220,
                observed: 1_221,
            }
        ));
        let mut frequency_sink = vec![0x5F];
        let frequency_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &downsample_image,
            ImageFormat::Jpeg,
            &optimized_options,
            &frequency_policy,
            &mut frequency_sink,
        ) {
            Ok(_) => {
                return Err("JPEG Huffman-frequency budget unexpectedly wrote output".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            frequency_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 1_220,
                observed: 1_221,
            }
        ));
        assert_eq!(frequency_sink, vec![0x5F]);

        // Progressive JPEG scan-event generation walks block slots in its
        // DC/AC scan loops. The committed fixture reaches that interior path
        // after the earlier RGB, sampling, and entropy checkpoints; charge
        // one interval per 1,024 scan blocks so long scan generation cannot
        // hide behind row polls. Pillow has no caller work budget or
        // equivalent sink/result contract, so this remains Rust-only evidence
        // with no parity row, parity fixture, diagnostic origin, or
        // coverage-only hook.
        let mut progressive_jpeg_options = image_slash_star::JpegEncodeOptions::default();
        progressive_jpeg_options.progressive = Some(true);
        let progressive_options = EncodeOptions::from(progressive_jpeg_options);
        let progressive_expected =
            image_slash_star::encode(&downsample_image, ImageFormat::Jpeg, &progressive_options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &downsample_image,
                ImageFormat::Jpeg,
                &progressive_options,
                &unlimited,
            )?,
            progressive_expected,
            "an ample progressive-scan budget preserves fixture-derived bytes"
        );
        let progressive_policy = image_slash_star::EncodePolicy::new().with_max_work_units(1_364);
        let progressive_error = match image_slash_star::encode_with_policy(
            &downsample_image,
            ImageFormat::Jpeg,
            &progressive_options,
            &progressive_policy,
        ) {
            Ok(_) => return Err("JPEG progressive-scan work budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            progressive_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 1_364,
                observed: 1_365,
            }
        ));
        let mut progressive_sink = vec![0x60];
        let progressive_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &downsample_image,
            ImageFormat::Jpeg,
            &progressive_options,
            &progressive_policy,
            &mut progressive_sink,
        ) {
            Ok(_) => {
                return Err("JPEG progressive-scan work budget unexpectedly wrote output".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            progressive_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 1_364,
                observed: 1_365,
            }
        ));
        assert_eq!(progressive_sink, vec![0x60]);

        // Progressive scan encoding next counts the event vector while it
        // gathers per-scan Huffman frequencies. The block-slot checkpoint
        // above covers event generation; this separate interval prevents the
        // frequency pass from hiding a large event vector between polls.
        let progressive_event_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(1_378);
        let progressive_event_error = match image_slash_star::encode_with_policy(
            &downsample_image,
            ImageFormat::Jpeg,
            &progressive_options,
            &progressive_event_policy,
        ) {
            Ok(_) => return Err("JPEG progressive-event work budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            progressive_event_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 1_378,
                observed: 1_379,
            }
        ));
        let mut progressive_event_sink = vec![0x61];
        let progressive_event_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &downsample_image,
            ImageFormat::Jpeg,
            &progressive_options,
            &progressive_event_policy,
            &mut progressive_event_sink,
        ) {
            Ok(_) => {
                return Err("JPEG progressive-event work budget unexpectedly wrote output".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            progressive_event_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 1_378,
                observed: 1_379,
            }
        ));
        assert_eq!(progressive_event_sink, vec![0x61]);

        // Progressive AC scan generation also walks every coefficient in the
        // spectral band of each block. A constant 257x129 probe keeps this
        // coefficient-item boundary independently reachable from the event
        // frequency pass above while still traversing more than 1,024 AC
        // positions. Pillow has no caller work budget or equivalent sink/
        // result contract, so this remains Rust-only evidence.
        let progressive_coefficient_image =
            DecodedImage::new(257, 129, vec![0; 257 * 129 * 3], ColorType::Rgb8);
        let progressive_coefficient_expected = image_slash_star::encode(
            &progressive_coefficient_image,
            ImageFormat::Jpeg,
            &progressive_options,
        )?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &progressive_coefficient_image,
                ImageFormat::Jpeg,
                &progressive_options,
                &unlimited,
            )?,
            progressive_coefficient_expected,
            "an ample progressive-coefficient budget preserves byte identity"
        );
        let progressive_coefficient_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(1_378);
        let progressive_coefficient_error = match image_slash_star::encode_with_policy(
            &progressive_coefficient_image,
            ImageFormat::Jpeg,
            &progressive_options,
            &progressive_coefficient_policy,
        ) {
            Ok(_) => {
                return Err(
                    "JPEG progressive-coefficient work budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            progressive_coefficient_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 1_378,
                observed: 1_379,
            }
        ));
        let mut progressive_coefficient_sink = vec![0x62];
        let progressive_coefficient_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &progressive_coefficient_image,
            ImageFormat::Jpeg,
            &progressive_options,
            &progressive_coefficient_policy,
            &mut progressive_coefficient_sink,
        ) {
            Ok(_) => {
                return Err(
                    "JPEG progressive-coefficient work budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            progressive_coefficient_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 1_378,
                observed: 1_379,
            }
        ));
        assert_eq!(progressive_coefficient_sink, vec![0x62]);

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

        // Baseline JPEG entropy coding now charges an interior checkpoint
        // after each 1,024 MCUs. This low-entropy deterministic 512x512 RGB
        // probe has exactly 32x32 default 4:2:0 MCUs, so the boundary is
        // reachable without making the test spend time on unnecessary entropy
        // complexity. Pillow has no caller token,
        // work-budget result, or caller-owned sink, so this remains Rust-only
        // evidence with no parity row, fixture, diagnostic origin, new test
        // function, or coverage-only hook.
        let baseline_mcu_pixels = vec![128; 512 * 512 * 3];
        let baseline_mcu_image = DecodedImage::new(512, 512, baseline_mcu_pixels, ColorType::Rgb8);
        let baseline_mcu_expected =
            image_slash_star::encode(&baseline_mcu_image, ImageFormat::Jpeg, &options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &baseline_mcu_image,
                ImageFormat::Jpeg,
                &options,
                &unlimited,
            )?,
            baseline_mcu_expected,
            "an ample baseline-MCU budget preserves generated-probe bytes"
        );
        let baseline_mcu_policy = image_slash_star::EncodePolicy::new().with_max_work_units(7_720);
        let baseline_mcu_error = match image_slash_star::encode_with_policy(
            &baseline_mcu_image,
            ImageFormat::Jpeg,
            &options,
            &baseline_mcu_policy,
        ) {
            Ok(_) => return Err("JPEG baseline-MCU budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            baseline_mcu_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 7_720,
                observed: 7_721,
            }
        ));
        let mut baseline_mcu_sink = vec![0x63];
        let baseline_mcu_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &baseline_mcu_image,
            ImageFormat::Jpeg,
            &options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(7_720),
            &mut baseline_mcu_sink,
        ) {
            Ok(_) => return Err("JPEG baseline-MCU sink budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            baseline_mcu_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Jpeg),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 7_720,
                observed: 7_721,
            }
        ));
        assert_eq!(baseline_mcu_sink, vec![0x63]);
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

        // Lossy VP8 pads each plane to a 16x16 macroblock boundary before
        // analysis. The token-aware path now charges the shared Y/U/V
        // edge-replication pass after each 1,024 padded samples; Pillow has no
        // caller token, typed work-budget result, or sink-rollback contract,
        // so this is Rust-only evidence with no parity row or manifest entry.
        let padded_image = DecodedImage::new(17, 17, vec![128; 17 * 17 * 3], ColorType::Rgb8);
        let padded_expected = image_slash_star::encode(&padded_image, ImageFormat::WebP, &options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &padded_image,
                ImageFormat::WebP,
                &options,
                &unlimited,
            )?,
            padded_expected,
            "an ample padded-plane budget preserves byte identity"
        );
        let padded_policy = image_slash_star::EncodePolicy::new().with_max_work_units(2);
        let padded_error = match image_slash_star::encode_with_policy(
            &padded_image,
            ImageFormat::WebP,
            &options,
            &padded_policy,
        ) {
            Ok(_) => return Err("WebP padded-plane budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            padded_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 2,
                observed: 3,
            }
        ));
        let mut padded_sink = vec![0xA9];
        let padded_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &padded_image,
            ImageFormat::WebP,
            &options,
            &padded_policy,
            &mut padded_sink,
        ) {
            Ok(_) => return Err("WebP padded-plane sink budget wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            padded_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 2,
                observed: 3,
            }
        ));
        assert_eq!(padded_sink, vec![0xA9]);

        // Lossy VP8 coefficient-statistics collection scans every selected
        // macroblock before probability adaptation. The token-aware path now
        // polls after each 1,024 macroblocks; Pillow has no caller token,
        // typed work-budget result, or sink-rollback contract, so this remains
        // Rust-only evidence in this existing feature-gated test rather than
        // a parity row or manifest entry.
        let statistics_image =
            DecodedImage::new(512, 512, vec![128; 512 * 512 * 3], ColorType::Rgb8);
        let statistics_expected =
            image_slash_star::encode(&statistics_image, ImageFormat::WebP, &options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &statistics_image,
                ImageFormat::WebP,
                &options,
                &unlimited,
            )?,
            statistics_expected,
            "an ample VP8 coefficient-statistics budget preserves byte identity"
        );
        let statistics_policy = image_slash_star::EncodePolicy::new().with_max_work_units(712);
        let statistics_error = match image_slash_star::encode_with_policy(
            &statistics_image,
            ImageFormat::WebP,
            &options,
            &statistics_policy,
        ) {
            Ok(_) => return Err("VP8 coefficient-statistics budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            statistics_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 712,
                observed: 713,
            }
        ));
        let mut statistics_sink = vec![0xAA];
        let statistics_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &statistics_image,
            ImageFormat::WebP,
            &options,
            &statistics_policy,
            &mut statistics_sink,
        ) {
            Ok(_) => return Err("VP8 coefficient-statistics sink budget wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            statistics_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 712,
                observed: 713,
            }
        ));
        assert_eq!(statistics_sink, vec![0xAA]);

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
        // A constant 1x512 RGB image reaches the long backward-reference result
        // backfill. The token-aware path now polls every 256 backfilled entries
        // instead of allowing the outer 1,024-pixel checkpoint to be skipped;
        // this caller-work boundary is not representable by Pillow.
        let backfill_image = DecodedImage::new(1, 512, vec![128; 512 * 3], ColorType::Rgb8);
        let backfill_expected =
            image_slash_star::encode(&backfill_image, ImageFormat::WebP, &lossless_options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &backfill_image,
                ImageFormat::WebP,
                &lossless_options,
                &unlimited,
            )?,
            backfill_expected,
            "an ample backward-reference budget preserves byte identity"
        );
        let backfill_policy = image_slash_star::EncodePolicy::new().with_max_work_units(2_516);
        let backfill_error = match image_slash_star::encode_with_policy(
            &backfill_image,
            ImageFormat::WebP,
            &lossless_options,
            &backfill_policy,
        ) {
            Ok(_) => return Err("backward-reference backfill budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            backfill_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 2_516,
                observed: 2_517,
            }
        ));
        let mut backfill_sink = vec![0xC9];
        let backfill_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &backfill_image,
            ImageFormat::WebP,
            &lossless_options,
            &backfill_policy,
            &mut backfill_sink,
        ) {
            Ok(_) => {
                return Err(
                    "backward-reference backfill sink budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            backfill_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 2_516,
                observed: 2_517,
            }
        ));
        assert_eq!(
            backfill_sink,
            vec![
                0xC9, 0x52, 0x49, 0x46, 0x46, 0x24, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50,
            ],
            "the later sink checkpoint preserves the validated RIFF/WEBP prefix"
        );
        // Lossless VP8L RGB/RGBA materialization now polls after each 1,024
        // source pixels before the later stages begin. Pillow cannot exercise
        // this caller-work-budget boundary: it has no caller token, typed
        // work-budget result, or sink-rollback contract. This real public
        // encode therefore remains Rust-only evidence with no parity row,
        // manifest fixture, diagnostic origin, new test function, or
        // coverage-only hook.
        let conversion_policy = image_slash_star::EncodePolicy::new().with_max_work_units(2);
        let conversion_error = match image_slash_star::encode_with_policy(
            &lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &conversion_policy,
        ) {
            Ok(_) => return Err("VP8L pixel conversion budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            conversion_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 2,
                observed: 3,
            }
        ));
        let mut conversion_sink = vec![0xC4];
        let conversion_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &conversion_policy,
            &mut conversion_sink,
        ) {
            Ok(_) => return Err("VP8L pixel conversion sink budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            conversion_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 2,
                observed: 3,
            }
        ));
        assert_eq!(conversion_sink, vec![0xC4]);

        // Lossless VP8L RGBA cleanup now polls after each 1,024 scanned
        // pixels while replacing hidden RGB values in fully transparent
        // pixels. This deterministic 128x128 fixture reaches that first
        // interior checkpoint after the 16 conversion intervals and the
        // encoder's two fixed admission polls.
        // Pillow cannot exercise this caller-work-budget boundary either.
        // Pillow has no caller token, work-budget result, or sink-rollback
        // contract, so this remains Rust-only evidence with no parity row,
        // manifest fixture, diagnostic origin, new test function, or
        // coverage-only hook.
        let mut alpha_cleanup_pixels = Vec::with_capacity(128 * 128 * 4);
        for index in 0..128 * 128 {
            let value = u8::try_from(index % 256)?;
            alpha_cleanup_pixels.extend_from_slice(&[
                value,
                value.wrapping_mul(3),
                value.wrapping_add(7),
                0,
            ]);
        }
        let alpha_cleanup_image =
            DecodedImage::new(128, 128, alpha_cleanup_pixels, ColorType::Rgba8);
        let alpha_cleanup_expected =
            image_slash_star::encode(&alpha_cleanup_image, ImageFormat::WebP, &lossless_options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &alpha_cleanup_image,
                ImageFormat::WebP,
                &lossless_options,
                &image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX),
            )?,
            alpha_cleanup_expected,
            "an ample VP8L RGBA cleanup budget preserves byte identity"
        );
        let alpha_cleanup_policy = image_slash_star::EncodePolicy::new().with_max_work_units(18);
        let alpha_cleanup_error = match image_slash_star::encode_with_policy(
            &alpha_cleanup_image,
            ImageFormat::WebP,
            &lossless_options,
            &alpha_cleanup_policy,
        ) {
            Ok(_) => return Err("VP8L RGBA cleanup budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            alpha_cleanup_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 18,
                observed: 19,
            }
        ));
        let mut alpha_cleanup_sink = vec![0xB7];
        let alpha_cleanup_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &alpha_cleanup_image,
            ImageFormat::WebP,
            &lossless_options,
            &alpha_cleanup_policy,
            &mut alpha_cleanup_sink,
        ) {
            Ok(_) => return Err("VP8L RGBA cleanup sink budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            alpha_cleanup_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 18,
                observed: 19,
            }
        ));
        assert_eq!(alpha_cleanup_sink, vec![0xB7]);
        // This deterministic 128-entry palette fixture reaches lossless
        // VP8L palette mode and forces both forward and reverse RGB deltas.
        // The Rust-only work contract must therefore cover the inner nearest
        // candidate scan, not just the surrounding image-stream stages.
        let palette_work_fixture = (0_usize..128)
            .map(|index| {
                let value = u8::try_from((index * 37) & 0xff)?;
                Ok::<[u8; 3], std::num::TryFromIntError>([
                    value,
                    value.wrapping_mul(73).wrapping_add(17),
                    value.wrapping_mul(109).wrapping_add(83),
                ])
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut palette_work_pixels = Vec::with_capacity(128 * 4 * 3);
        let mut palette_work_state = 0x1234_5678_u32;
        for index in 0..128 * 4 {
            let palette_index = if index < 128 {
                index
            } else {
                palette_work_state = palette_work_state
                    .wrapping_mul(1_664_525)
                    .wrapping_add(1_013_904_223);
                usize::try_from((palette_work_state >> 25) & 0x7f)?
            };
            palette_work_pixels.extend_from_slice(&palette_work_fixture[palette_index]);
        }
        let palette_work_image = DecodedImage::new(128, 4, palette_work_pixels, ColorType::Rgb8);
        let palette_work_expected =
            image_slash_star::encode(&palette_work_image, ImageFormat::WebP, &lossless_options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &palette_work_image,
                ImageFormat::WebP,
                &lossless_options,
                &image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX),
            )?,
            palette_work_expected,
            "an ample lossless WebP palette budget preserves byte identity"
        );
        // Pillow cannot exercise this as parity: it has no caller-controlled
        // work budget, cancellation checkpoint, or sink-rollback contract.
        // This is consequently a Rust-only feature-gate result with no
        // Pillow parity row or fixture-manifest entry.
        // The token-aware Huffman-node ordering path keeps the stable output
        // order of the no-token sort while polling every 64 comparisons. This
        // existing 128-entry palette fixture reaches that first comparison
        // checkpoint before any structural sink delivery. Pillow has no
        // caller token or typed work-budget result, so this remains outside
        // the parity manifest and adds no coverage-only hook.
        let huffman_sort_policy = image_slash_star::EncodePolicy::new().with_max_work_units(2_412);
        let huffman_sort_error = match image_slash_star::encode_with_policy(
            &palette_work_image,
            ImageFormat::WebP,
            &lossless_options,
            &huffman_sort_policy,
        ) {
            Ok(_) => return Err("WebP Huffman-sort budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            huffman_sort_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 2_412,
                observed: 2_413,
            }
        ));
        let mut huffman_sort_sink = vec![0xC6];
        let huffman_sort_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &palette_work_image,
            ImageFormat::WebP,
            &lossless_options,
            &huffman_sort_policy,
            &mut huffman_sort_sink,
        ) {
            Ok(_) => return Err("bounded WebP Huffman sorting wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            huffman_sort_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 2_412,
                observed: 2_413,
            }
        ));
        assert_eq!(huffman_sort_sink, vec![0xC6]);

        let palette_work_policy = image_slash_star::EncodePolicy::new().with_max_work_units(3_000);
        let palette_work_error = match image_slash_star::encode_with_policy(
            &palette_work_image,
            ImageFormat::WebP,
            &lossless_options,
            &palette_work_policy,
        ) {
            Ok(_) => return Err("WebP palette work budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            palette_work_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 3_000,
                observed: 3_001,
            }
        ));
        let mut palette_work_sink = vec![0xA7];
        let palette_work_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &palette_work_image,
            ImageFormat::WebP,
            &lossless_options,
            &palette_work_policy,
            &mut palette_work_sink,
        ) {
            Ok(_) => return Err("bounded WebP palette work wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            palette_work_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 3_000,
                observed: 3_001,
            }
        ));
        assert_eq!(palette_work_sink, vec![0xA7]);

        // Lossless VP8L palette-index packing now charges after each 64
        // palette candidates examined by a repeated linear lookup. This
        // deterministic 128x128 probe keeps the 128-entry palette mode and
        // reaches the lookup boundary after the earlier palette ordering and
        // image-stream work. Pillow has no caller token, work-budget result,
        // or sink-rollback contract, so this remains Rust-only evidence with
        // no parity row, manifest fixture, diagnostic origin, or coverage-only
        // hook.
        let mut palette_lookup_pixels = Vec::with_capacity(128 * 128 * 3);
        let mut palette_lookup_state = 0x1234_5678_u32;
        for _ in 0..128 * 128 {
            palette_lookup_state = palette_lookup_state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            let palette_index = usize::try_from((palette_lookup_state >> 25) & 0x7f)?;
            palette_lookup_pixels.extend_from_slice(&palette_work_fixture[palette_index]);
        }
        let palette_lookup_image =
            DecodedImage::new(128, 128, palette_lookup_pixels, ColorType::Rgb8);
        let palette_lookup_expected =
            image_slash_star::encode(&palette_lookup_image, ImageFormat::WebP, &lossless_options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &palette_lookup_image,
                ImageFormat::WebP,
                &lossless_options,
                &image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX),
            )?,
            palette_lookup_expected,
            "an ample palette-lookup budget preserves byte identity"
        );
        let palette_lookup_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(9_820);
        let palette_lookup_error = match image_slash_star::encode_with_policy(
            &palette_lookup_image,
            ImageFormat::WebP,
            &lossless_options,
            &palette_lookup_policy,
        ) {
            Ok(_) => return Err("WebP palette lookup budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            palette_lookup_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 9_820,
                observed: 9_821,
            }
        ));
        let mut palette_lookup_sink = vec![0xA9];
        let palette_lookup_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &palette_lookup_image,
            ImageFormat::WebP,
            &lossless_options,
            &palette_lookup_policy,
            &mut palette_lookup_sink,
        ) {
            Ok(_) => return Err("bounded WebP palette lookup wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            palette_lookup_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 9_820,
                observed: 9_821,
            }
        ));
        assert_eq!(palette_lookup_sink, vec![0xA9]);

        let mut palette_packing_pixels = Vec::with_capacity(128 * 8 * 3);
        let mut palette_packing_state = 0x1234_5678_u32;
        for _ in 0..128 * 8 {
            palette_packing_state = palette_packing_state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            let palette_index = usize::try_from((palette_packing_state >> 25) & 0x7f)?;
            palette_packing_pixels.extend_from_slice(&palette_work_fixture[palette_index]);
        }
        let palette_packing_image =
            DecodedImage::new(128, 8, palette_packing_pixels, ColorType::Rgb8);
        // Lossless VP8L palette-mode index packing now polls after each
        // 1,024 source pixels. This is a caller-work-budget boundary, not
        // Pillow parity: the oracle has no caller token, typed work-budget
        // result, or caller-owned sink contract.
        let palette_packing_expected =
            image_slash_star::encode(&palette_packing_image, ImageFormat::WebP, &lossless_options)?;
        let palette_packing_unlimited =
            image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX);
        assert_eq!(
            image_slash_star::encode_with_policy(
                &palette_packing_image,
                ImageFormat::WebP,
                &lossless_options,
                &palette_packing_unlimited,
            )?,
            palette_packing_expected,
            "an ample VP8L palette-packing budget preserves byte identity"
        );
        // This 1,024-pixel palette probe also reaches the token-aware VP8L
        // cost-manager setup. Its pixel-sized cost/length tables now poll
        // while initializing, before any structural sink segment is allowed
        // to arrive; Pillow has no equivalent caller-work boundary.
        let palette_packing_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(5_205);
        let palette_packing_error = match image_slash_star::encode_with_policy(
            &palette_packing_image,
            ImageFormat::WebP,
            &lossless_options,
            &palette_packing_policy,
        ) {
            Ok(_) => return Err("VP8L palette-packing budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            palette_packing_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 5_205,
                observed: 5_206,
            }
        ));
        let mut palette_packing_sink = vec![0xC3];
        let palette_packing_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &palette_packing_image,
            ImageFormat::WebP,
            &lossless_options,
            &palette_packing_policy,
            &mut palette_packing_sink,
        ) {
            Ok(_) => return Err("bounded VP8L palette packing wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            palette_packing_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 5_205,
                observed: 5_206,
            }
        ));
        assert_eq!(
            palette_packing_sink,
            vec![0xC3],
            "the cost-manager setup boundary now rejects before sink delivery"
        );

        // A small monotone palette reaches the same public token-aware path
        // but has only forward deltas, proving its bounded early return with
        // a real lossless encode rather than a coverage-only call.
        let mut simple_palette_pixels = Vec::with_capacity(16 * 3);
        for value in 0_u8..16 {
            simple_palette_pixels.extend_from_slice(&[value, value, value]);
        }
        let simple_palette_image = DecodedImage::new(16, 1, simple_palette_pixels, ColorType::Rgb8);
        let simple_palette_expected =
            image_slash_star::encode(&simple_palette_image, ImageFormat::WebP, &lossless_options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &simple_palette_image,
                ImageFormat::WebP,
                &lossless_options,
                &image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX),
            )?,
            simple_palette_expected,
            "an ample simple-palette budget preserves byte identity"
        );

        // This short palette has both delta directions but stays below the
        // 18-entry rotation threshold, exercising that public branch without
        // widening the work-budget contract or adding a private test hook.
        let mixed_small_values = [
            0_u8, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214,
        ];
        let mut mixed_small_pixels = Vec::with_capacity(mixed_small_values.len() * 3);
        for value in mixed_small_values {
            mixed_small_pixels.extend_from_slice(&[value, value, value]);
        }
        let mixed_small_image = DecodedImage::new(16, 1, mixed_small_pixels, ColorType::Rgb8);
        let mixed_small_expected =
            image_slash_star::encode(&mixed_small_image, ImageFormat::WebP, &lossless_options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &mixed_small_image,
                ImageFormat::WebP,
                &lossless_options,
                &image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX),
            )?,
            mixed_small_expected,
            "an ample mixed-short-palette budget preserves byte identity"
        );

        // A transparent pixel makes the sorted palette begin with zero. The
        // remaining deterministic high-color fixture reaches palette mode and
        // proves the token-aware transparent-zero rotation through the public
        // encoder, not through a private coverage-only input.
        let mut reordered_palette_pixels = Vec::with_capacity(128 * 4 * 4);
        reordered_palette_pixels.extend_from_slice(&[0, 0, 0, 0]);
        let mut reordered_palette_state = 0x9abc_def0_u32;
        for _ in 1..128 * 4 {
            reordered_palette_state = reordered_palette_state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            let palette_index = usize::try_from((reordered_palette_state >> 25) & 0x7f)?;
            let [red, green, blue] = palette_work_fixture[palette_index];
            reordered_palette_pixels.extend_from_slice(&[red, green, blue, 255]);
        }
        let reordered_palette_image =
            DecodedImage::new(128, 4, reordered_palette_pixels, ColorType::Rgba8);
        let reordered_palette_expected = image_slash_star::encode(
            &reordered_palette_image,
            ImageFormat::WebP,
            &lossless_options,
        )?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &reordered_palette_image,
                ImageFormat::WebP,
                &lossless_options,
                &image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX),
            )?,
            reordered_palette_expected,
            "an ample transparent-palette budget preserves byte identity"
        );
        let alpha_palette_values = (0_u16..64)
            .chain(192_u16..256)
            .map(u8::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let mut alpha_palette_pixels = Vec::with_capacity(128 * 128 * 4);
        for index in 0..128 * 128 {
            let alpha = alpha_palette_values[index % alpha_palette_values.len()];
            alpha_palette_pixels.extend_from_slice(&[37, 83, 149, alpha]);
        }
        let alpha_palette_image =
            DecodedImage::new(128, 128, alpha_palette_pixels, ColorType::Rgba8);
        let mut alpha_options = EncodeOptions::for_format(ImageFormat::WebP);
        if let EncodeOptions::WebP(options) = &mut alpha_options {
            options.lossless = Some(false);
            options.quality = Some(75);
        }
        // Lossy WebP RGBA alpha-palette ordering now polls the nearest-delta
        // candidate scan after each 64 candidates. Pillow has no caller work
        // budget or sink contract, so this is Rust-only work-control evidence
        // and intentionally has no parity row or fixture-manifest entry.
        let alpha_unlimited = image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX);
        let alpha_expected =
            image_slash_star::encode(&alpha_palette_image, ImageFormat::WebP, &alpha_options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &alpha_palette_image,
                ImageFormat::WebP,
                &alpha_options,
                &alpha_unlimited,
            )?,
            alpha_expected,
            "an ample WebP alpha-palette budget preserves byte identity"
        );

        let alpha_policy = image_slash_star::EncodePolicy::new().with_max_work_units(40);
        let alpha_palette_error = match image_slash_star::encode_with_policy(
            &alpha_palette_image,
            ImageFormat::WebP,
            &alpha_options,
            &alpha_policy,
        ) {
            Ok(_) => return Err("WebP alpha-palette budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            alpha_palette_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 40,
                observed: 41,
            }
        ));

        let mut alpha_sink = vec![0xA8];
        let alpha_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &alpha_palette_image,
            ImageFormat::WebP,
            &alpha_options,
            &alpha_policy,
            &mut alpha_sink,
        ) {
            Ok(_) => return Err("bounded WebP alpha-palette budget wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            alpha_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 40,
                observed: 41,
            }
        ));
        assert_eq!(alpha_sink, vec![0xA8]);
        // Lossy WebP RGBA alpha-palette source collection now polls after
        // each 1,024 source pixels. This is a caller-work-budget boundary,
        // not Pillow parity: Pillow exposes neither a caller token nor a
        // typed work-budget or sink-rollback contract, so this real public
        // encode adds no parity row or fixture-manifest entry.
        let mut alpha_collection_pixels = Vec::with_capacity(1_024 * 4);
        for index in 0..1_024 {
            let alpha = alpha_palette_values[index % alpha_palette_values.len()];
            alpha_collection_pixels.extend_from_slice(&[37, 83, 149, alpha]);
        }
        let alpha_collection_image =
            DecodedImage::new(16, 64, alpha_collection_pixels, ColorType::Rgba8);
        let alpha_collection_policy = image_slash_star::EncodePolicy::new().with_max_work_units(5);
        let alpha_collection_error = match image_slash_star::encode_with_policy(
            &alpha_collection_image,
            ImageFormat::WebP,
            &alpha_options,
            &alpha_collection_policy,
        ) {
            Ok(_) => {
                return Err("WebP alpha-palette collection budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            alpha_collection_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 5,
                observed: 6,
            }
        ));
        let mut alpha_collection_sink = vec![0xC1];
        let alpha_collection_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &alpha_collection_image,
            ImageFormat::WebP,
            &alpha_options,
            &alpha_collection_policy,
            &mut alpha_collection_sink,
        ) {
            Ok(_) => return Err("bounded WebP alpha-palette collection wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            alpha_collection_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 5,
                observed: 6,
            }
        ));
        assert_eq!(alpha_collection_sink, vec![0xC1]);
        // Lossy WebP RGBA alpha-palette index packing now polls after each
        // 1,024 source pixels. This is another caller-work-budget boundary,
        // not Pillow parity: the oracle has no caller token, typed work-budget
        // result, or caller-owned sink contract.
        let alpha_packing_values = (0_u8..64).collect::<Vec<_>>();
        let mut alpha_packing_pixels = Vec::with_capacity(128 * 8 * 4);
        for index in 0..128 * 8 {
            let alpha = alpha_packing_values[index % alpha_packing_values.len()];
            alpha_packing_pixels.extend_from_slice(&[37, 83, 149, alpha]);
        }
        let alpha_packing_image = DecodedImage::new(128, 8, alpha_packing_pixels, ColorType::Rgba8);
        let alpha_packing_expected =
            image_slash_star::encode(&alpha_packing_image, ImageFormat::WebP, &alpha_options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &alpha_packing_image,
                ImageFormat::WebP,
                &alpha_options,
                &alpha_unlimited,
            )?,
            alpha_packing_expected,
            "an ample WebP alpha-palette packing budget preserves byte identity"
        );
        let alpha_packing_policy = image_slash_star::EncodePolicy::new().with_max_work_units(11);
        let alpha_packing_error = match image_slash_star::encode_with_policy(
            &alpha_packing_image,
            ImageFormat::WebP,
            &alpha_options,
            &alpha_packing_policy,
        ) {
            Ok(_) => return Err("WebP alpha-palette packing budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            alpha_packing_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 11,
                observed: 12,
            }
        ));
        let mut alpha_packing_sink = vec![0xC2];
        let alpha_packing_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &alpha_packing_image,
            ImageFormat::WebP,
            &alpha_options,
            &alpha_packing_policy,
            &mut alpha_packing_sink,
        ) {
            Ok(_) => return Err("bounded WebP alpha-palette packing wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            alpha_packing_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 11,
                observed: 12,
            }
        ));
        assert_eq!(alpha_packing_sink, vec![0xC2]);
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
        // Lossless VP8L palette construction now polls after each 1,024 source
        // pixels. The conversion stage has four earlier intervals on this
        // 64x64 fixture, so this boundary remains separate from the preceding
        // conversion assertion. Pillow exposes neither a caller token nor a
        // work-budget result or sink-rollback contract, so it adds no parity
        // row or fixture entry.
        let palette_scan_policy = image_slash_star::EncodePolicy::new().with_max_work_units(6);
        let palette_scan_error = match image_slash_star::encode_with_policy(
            &lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &palette_scan_policy,
        ) {
            Ok(_) => return Err("VP8L palette scan budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            palette_scan_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 6,
                observed: 7,
            }
        ));
        let mut palette_scan_sink = vec![0xBA];
        let palette_scan_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &palette_scan_policy,
            &mut palette_scan_sink,
        ) {
            Ok(_) => return Err("VP8L palette scan budget wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            palette_scan_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 6,
                observed: 7,
            }
        ));
        assert_eq!(palette_scan_sink, vec![0xBA]);
        // The non-palette VP8L preparation path now scans RGB-equal pixels
        // with a checkpoint after each 1,024 pixels. Varying alpha keeps this
        // deterministic grayscale probe above the 256-color palette limit,
        // so the real scan is exercised without a Pillow parity row or a
        // coverage-only input.
        let mut grayscale_lossless_pixels = Vec::with_capacity(128 * 128 * 4);
        for index in 0..128 * 128 {
            let value = u8::try_from(index % 256)?;
            let alpha = u8::try_from((index / 256) % 256)?;
            grayscale_lossless_pixels.extend_from_slice(&[value, value, value, alpha]);
        }
        let grayscale_lossless_image =
            DecodedImage::new(128, 128, grayscale_lossless_pixels, ColorType::Rgba8);
        let grayscale_lossless_expected = image_slash_star::encode(
            &grayscale_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
        )?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &grayscale_lossless_image,
                ImageFormat::WebP,
                &lossless_options,
                &lossless_unlimited,
            )?,
            grayscale_lossless_expected,
            "an ample VP8L grayscale budget preserves byte identity"
        );
        let grayscale_policy = image_slash_star::EncodePolicy::new().with_max_work_units(195);
        let grayscale_error = match image_slash_star::encode_with_policy(
            &grayscale_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &grayscale_policy,
        ) {
            Ok(_) => return Err("VP8L grayscale budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            grayscale_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 195,
                observed: 196,
            }
        ));
        let mut grayscale_sink = vec![0xB2];
        let grayscale_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &grayscale_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &grayscale_policy,
            &mut grayscale_sink,
        ) {
            Ok(_) => return Err("VP8L grayscale sink budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            grayscale_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 195,
                observed: 196,
            }
        ));
        assert_eq!(grayscale_sink, vec![0xB2]);
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

        let mut transform_pixels = Vec::with_capacity(1_024 * 3);
        for index in 0..1_024 {
            let value = u8::try_from(index % 256)?;
            transform_pixels.extend_from_slice(&[
                value,
                value.wrapping_add(u8::try_from(index / 4)?),
                value.wrapping_add(u8::try_from(index / 8)?),
            ]);
        }
        let transform_image = DecodedImage::new(1_024, 1, transform_pixels, ColorType::Rgb8);
        // The lossless VP8L fixed-predictor path now checkpoints its full
        // source snapshot copy and its interior transform after each 1,024
        // pixels. This wide one-row probe reaches the real transform boundary
        // after the earlier predictor setup and snapshot polls. Pillow has no
        // caller token or work-budget result, so this remains Rust-only
        // evidence with no parity row or coverage-only hook.
        let transform_policy = image_slash_star::EncodePolicy::new().with_max_work_units(3_675);
        let transform_error = match image_slash_star::encode_with_policy(
            &transform_image,
            ImageFormat::WebP,
            &lossless_options,
            &transform_policy,
        ) {
            Ok(_) => return Err("VP8L predictor transform budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            transform_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 3_675,
                observed: 3_676,
            }
        ));
        let mut transform_sink = vec![0xAA];
        let transform_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &transform_image,
            ImageFormat::WebP,
            &lossless_options,
            &transform_policy,
            &mut transform_sink,
        ) {
            Ok(_) => {
                return Err("VP8L predictor transform sink budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            transform_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 3_675,
                observed: 3_676,
            }
        ));
        assert_eq!(transform_sink, vec![0xAA]);

        let mut subtract_green_pixels = Vec::with_capacity(1_024 * 3);
        for index in 0..1_024 {
            let green = u8::try_from(index % 256)?;
            subtract_green_pixels.extend_from_slice(&[
                green.wrapping_add(17),
                green,
                green.wrapping_add(31),
            ]);
        }
        let subtract_green_image =
            DecodedImage::new(1_024, 1, subtract_green_pixels, ColorType::Rgb8);
        // The lossless VP8L subtract-green transform now charges an interior
        // checkpoint after each 1,024 applied pixels. This one-row probe
        // reaches that real transform boundary after the earlier setup polls.
        // Pillow has no caller token or work-budget result, so this remains
        // Rust-only evidence with no parity row or coverage-only hook.
        let subtract_green_policy = image_slash_star::EncodePolicy::new().with_max_work_units(59);
        let subtract_green_error = match image_slash_star::encode_with_policy(
            &subtract_green_image,
            ImageFormat::WebP,
            &lossless_options,
            &subtract_green_policy,
        ) {
            Ok(_) => return Err("VP8L subtract-green budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            subtract_green_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 59,
                observed: 60,
            }
        ));
        let mut subtract_green_sink = vec![0xAB];
        let subtract_green_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &subtract_green_image,
            ImageFormat::WebP,
            &lossless_options,
            &subtract_green_policy,
            &mut subtract_green_sink,
        ) {
            Ok(_) => return Err("VP8L subtract-green sink budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            subtract_green_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 59,
                observed: 60,
            }
        ));
        assert_eq!(subtract_green_sink, vec![0xAB]);

        let mut sampling_pixels = Vec::with_capacity(8_192 * 8 * 3);
        for index in 0..8_192 * 8 {
            let value = u32::try_from(index)?
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            let [red, green, blue, _] = value.to_le_bytes();
            sampling_pixels.extend_from_slice(&[red, green, blue]);
        }
        let sampling_image = DecodedImage::new(8_192, 8, sampling_pixels, ColorType::Rgb8);
        let sampling_policy = image_slash_star::EncodePolicy::new().with_max_work_units(129_602);
        // The lossless VP8L cross-color sampling pass now charges an interior
        // checkpoint after each 1,024 scanned or compacted tile-map samples.
        // This 8,192x8 probe creates a 1,024-entry tile map and reaches that
        // real reduction boundary after the earlier analysis and transform
        // work. Pillow has no caller token or work-budget result, so this
        // remains Rust-only evidence with no parity row or coverage-only hook.
        let sampling_error = match image_slash_star::encode_with_policy(
            &sampling_image,
            ImageFormat::WebP,
            &lossless_options,
            &sampling_policy,
        ) {
            Ok(_) => return Err("VP8L sampling budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            sampling_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 129_602,
                observed: 129_603,
            }
        ));
        let mut sampling_sink = vec![0xAC];
        let sampling_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &sampling_image,
            ImageFormat::WebP,
            &lossless_options,
            &sampling_policy,
            &mut sampling_sink,
        ) {
            Ok(_) => return Err("VP8L sampling sink budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            sampling_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 129_602,
                observed: 129_603,
            }
        ));
        assert_eq!(sampling_sink, vec![0xAC]);

        // The lossless VP8L entropy-mode analysis now charges after each 64
        // symbols while scanning its fixed-alphabet histogram costs. Pillow
        // has no caller token or work-budget result, so this remains Rust-only
        // evidence with no parity row or coverage-only hook.
        let entropy_analysis_policy = image_slash_star::EncodePolicy::new().with_max_work_units(23);
        let entropy_analysis_error = match image_slash_star::encode_with_policy(
            &lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &entropy_analysis_policy,
        ) {
            Ok(_) => return Err("VP8L entropy-analysis budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            entropy_analysis_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 23,
                observed: 24,
            }
        ));
        let mut entropy_analysis_sink = vec![0xAD];
        let entropy_analysis_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &entropy_analysis_policy,
            &mut entropy_analysis_sink,
        ) {
            Ok(_) => {
                return Err("VP8L entropy-analysis sink budget unexpectedly wrote output".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            entropy_analysis_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 23,
                observed: 24,
            }
        ));
        assert_eq!(entropy_analysis_sink, vec![0xAD]);

        // The VP8L histogram population scan remains separately bounded after
        // the entropy-analysis polls above. Keeping this boundary distinct
        // prevents the new interior checkpoint from silently replacing the
        // existing histogram evidence.
        let histogram_policy = image_slash_star::EncodePolicy::new().with_max_work_units(62);
        let histogram_error = match image_slash_star::encode_with_policy(
            &lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &histogram_policy,
        ) {
            Ok(_) => return Err("VP8L histogram budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            histogram_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 62,
                observed: 63,
            }
        ));
        let mut histogram_sink = vec![0xB8];
        let histogram_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &histogram_policy,
            &mut histogram_sink,
        ) {
            Ok(_) => return Err("VP8L histogram sink budget unexpectedly wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            histogram_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 62,
                observed: 63,
            }
        ));
        assert_eq!(histogram_sink, vec![0xB8]);

        // Combined histogram entropy-cost scans now charge after each 64
        // symbols as well. This reaches the first combined channel scan after
        // the earlier lossless setup polls. Pillow has no caller token or
        // work-budget result, so this remains Rust-only evidence with no
        // parity row or coverage-only hook.
        let combined_histogram_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(80);
        let combined_histogram_error = match image_slash_star::encode_with_policy(
            &lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &combined_histogram_policy,
        ) {
            Ok(_) => return Err("VP8L combined histogram budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            combined_histogram_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 80,
                observed: 81,
            }
        ));
        let mut combined_histogram_sink = vec![0xAE];
        let combined_histogram_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &combined_histogram_policy,
            &mut combined_histogram_sink,
        ) {
            Ok(_) => {
                return Err("VP8L combined histogram sink budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            combined_histogram_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 80,
                observed: 81,
            }
        ));
        assert_eq!(combined_histogram_sink, vec![0xAE]);

        // A materially larger budget reaches the long predictor/cross-color,
        // histogram/Huffman, backward-reference, and token-stream intervals
        // before rejecting. This remains Rust-only work-control evidence:
        // Pillow exposes no caller budget or equivalent result.
        let deep_lossless_bounded =
            image_slash_star::EncodePolicy::new().with_max_work_units(8_231);
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
                maximum: 8_231,
                observed,
            } if observed > 8_231
        ));

        // Histogram population merges now charge after each 64 symbols once
        // the earlier entropy-cost checkpoints have completed. This boundary
        // is Rust-only work-control evidence: Pillow has no caller token,
        // work-budget result, or caller-owned sink.
        let histogram_merge_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(8_258);
        let histogram_merge_error = match image_slash_star::encode_with_policy(
            &lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &histogram_merge_policy,
        ) {
            Ok(_) => return Err("VP8L histogram merge budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            histogram_merge_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 8_258,
                observed: 8_259,
            }
        ));
        let mut histogram_merge_sink = vec![0xAF];
        let histogram_merge_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &histogram_merge_policy,
            &mut histogram_merge_sink,
        ) {
            Ok(_) => {
                return Err("VP8L histogram merge sink budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            histogram_merge_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 8_258,
                observed: 8_259,
            }
        ));
        assert_eq!(histogram_merge_sink, vec![0xAF]);

        // Backward-reference candidate scoring also performs token and
        // fixed-alphabet Huffman cost scans. Charge those estimates after each
        // 1,024 tokens, before the later histogram stages. Pillow has no
        // caller token, work-budget result, or caller-owned sink, so this is
        // Rust-only work-control evidence with no parity row or coverage hook.
        let cost_estimate_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(14_092);
        let cost_estimate_error = match image_slash_star::encode_with_policy(
            &lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &cost_estimate_policy,
        ) {
            Ok(_) => return Err("VP8L cost-estimate budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            cost_estimate_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 14_092,
                observed: 14_093,
            }
        ));
        let mut cost_estimate_sink = vec![0xB0];
        let cost_estimate_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &cost_estimate_policy,
            &mut cost_estimate_sink,
        ) {
            Ok(_) => return Err("VP8L cost-estimate sink budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            cost_estimate_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 14_092,
                observed: 14_093,
            }
        ));
        assert_eq!(cost_estimate_sink, vec![0xB0]);

        // This patterned probe reaches the deeper VP8L writer intervals only
        // after the earlier lossless stages. The exact rejection at the
        // selected bitstream and output intervals below prove that real
        // bitstream work—not a parity fixture or a synthetic coverage hook—
        // owns these boundaries. Pillow has no caller work budget or sink.
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
        // Huffman-tree emission now checkpoints simple-tree symbol discovery
        // after each 64 code-length slots and the code-length-token frequency
        // scan after each 16 compressed token entries. A generated LCG probe
        // reaches these interior paths without adding a Pillow parity row,
        // fixture, or coverage-only input.
        let mut huffman_frequency_pixels = Vec::with_capacity(128 * 128 * 3);
        let mut huffman_frequency_state = 0xD1B5_4A32_u32;
        for _ in 0..128 * 128 {
            huffman_frequency_state = huffman_frequency_state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            huffman_frequency_pixels.extend_from_slice(&huffman_frequency_state.to_be_bytes()[..3]);
        }
        let huffman_frequency_image =
            DecodedImage::new(128, 128, huffman_frequency_pixels, ColorType::Rgb8);
        // Histogram clustering now checkpoints its min/max pre-pass after
        // each 64 tile histograms and its bin-assignment pre-pass at the same
        // interval. This boundary reaches the first bin-assignment poll after
        // the four min/max polls on the generated 128x128 probe. Pillow has
        // no caller token, work-budget result, or caller-owned sink, so this
        // is Rust-only work-control evidence with no parity row or fixture.
        let entropy_bin_prepass_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(5_325);
        let mut entropy_bin_prepass_sink = vec![0xB9];
        let entropy_bin_prepass_error = match image_slash_star::encode_to_sink_with_policy(
            &huffman_frequency_image,
            ImageFormat::WebP,
            &lossless_options,
            &entropy_bin_prepass_policy,
            &mut entropy_bin_prepass_sink,
        ) {
            Ok(_) => return Err("VP8L entropy-bin pre-pass budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            entropy_bin_prepass_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 5_325,
                observed: 5_326,
            }
        ));
        assert_eq!(entropy_bin_prepass_sink, vec![0xB9]);
        // The same generated LCG probe reaches the later code-length-token
        // frequency boundary without adding a Pillow parity row, fixture, or
        // coverage-only input.
        let huffman_frequency_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(44_001);
        let huffman_frequency_error = match image_slash_star::encode_with_policy(
            &huffman_frequency_image,
            ImageFormat::WebP,
            &lossless_options,
            &huffman_frequency_policy,
        ) {
            Ok(_) => return Err("VP8L Huffman frequency budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            huffman_frequency_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 44_001,
                observed: 44_002,
            }
        ));
        let mut huffman_frequency_sink = vec![0xB3];
        let huffman_frequency_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &huffman_frequency_image,
            ImageFormat::WebP,
            &lossless_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(44_000),
            &mut huffman_frequency_sink,
        ) {
            Ok(_) => {
                return Err("VP8L Huffman frequency sink budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            huffman_frequency_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 44_000,
                observed: 44_001,
            }
        ));
        assert_eq!(huffman_frequency_sink, vec![0xB3]);

        // Huffman code-length emission now polls at the same 16-entry
        // interval as the adjacent frequency and trailing-trim scans instead
        // of paying one work-budget poll per emitted token. The generated
        // probe remains a public, fixture-based Rust contract: an ample
        // budget must preserve the ordinary bytes, while a near-complete
        // budget must still reject before sink delivery is complete. Pillow
        // has no caller token, work budget, or sink-rollback equivalent, so
        // this is deliberately not a parity row or manifest fixture.
        let huffman_frequency_expected = image_slash_star::encode(
            &huffman_frequency_image,
            ImageFormat::WebP,
            &lossless_options,
        )?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &huffman_frequency_image,
                ImageFormat::WebP,
                &lossless_options,
                &image_slash_star::EncodePolicy::new().with_max_work_units(u64::MAX),
            )?,
            huffman_frequency_expected,
            "the coarser Huffman emission polling preserves fixture-derived bytes"
        );
        let huffman_emission_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(144_869);
        let huffman_emission_error = match image_slash_star::encode_with_policy(
            &huffman_frequency_image,
            ImageFormat::WebP,
            &lossless_options,
            &huffman_emission_policy,
        ) {
            Ok(_) => return Err("VP8L Huffman emission budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            huffman_emission_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 144_869,
                observed: 144_870,
            }
        ));
        let mut huffman_emission_sink = vec![0xB6];
        let huffman_emission_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &huffman_frequency_image,
            ImageFormat::WebP,
            &lossless_options,
            &huffman_emission_policy,
            &mut huffman_emission_sink,
        ) {
            Ok(_) => return Err("VP8L Huffman emission budget wrote complete output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            huffman_emission_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 144_869,
                observed: 144_870,
            }
        ));
        assert_eq!(
            huffman_emission_sink,
            vec![0xB6],
            "the new pre-output palette checkpoint leaves the sink sentinel only"
        );
        let mut cache_probe_pixels = Vec::with_capacity(512 * 512 * 3);
        for _ in 0..512 {
            for x in 0..512_u32 {
                let x_bytes = x.to_le_bytes();
                let mixed_bytes = x.wrapping_mul(37).to_le_bytes();
                cache_probe_pixels.extend_from_slice(&[x_bytes[0], x_bytes[1], mixed_bytes[0]]);
            }
        }
        let cache_probe_image = DecodedImage::new(512, 512, cache_probe_pixels, ColorType::Rgb8);
        // VP8L cache population inside a copy token now checkpoints after
        // each 256 pixels. The token-aware hash-chain path also checkpoints
        // repeated-run insertion after each 256 pixels; its no-token path
        // remains tight. This repeated-row probe reaches the cache boundary
        // without adding a Pillow parity row, fixture, or coverage-only input.
        let cache_probe_policy = image_slash_star::EncodePolicy::new().with_max_work_units(136_928);
        let cache_probe_error = match image_slash_star::encode_with_policy(
            &cache_probe_image,
            ImageFormat::WebP,
            &lossless_options,
            &cache_probe_policy,
        ) {
            Ok(_) => return Err("VP8L cache probe budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            cache_probe_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 136_928,
                observed: 136_929,
            }
        ));
        let mut cache_probe_sink = vec![0xB5];
        let cache_probe_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &cache_probe_image,
            ImageFormat::WebP,
            &lossless_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(136_928),
            &mut cache_probe_sink,
        ) {
            Ok(_) => return Err("VP8L cache sink probe budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            cache_probe_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 136_928,
                observed: 136_929,
            }
        ));
        assert_eq!(
            cache_probe_sink,
            vec![0xB5],
            "the new pre-output palette checkpoint leaves the sink sentinel only"
        );
        // A 16,384x16 RGBA probe keeps two real meta-histogram groups while
        // making the first 1,537 tile symbols equal across adjacent rows.
        // This reaches the interior sampling comparison after its first
        // 1,024 symbols. Pillow exposes neither a caller token nor a typed
        // work-budget or sink-rollback contract, so this is Rust-only
        // work-control evidence with no parity row or manifest fixture.
        let sampling_probe_width = 16_384;
        let sampling_probe_height = 16;
        let mut sampling_probe_pixels =
            Vec::with_capacity(sampling_probe_width * sampling_probe_height * 4);
        for y in 0..sampling_probe_height {
            let tile_row = y / 8;
            for x in 0..sampling_probe_width {
                let tile = x / 8;
                let within_tile = (y % 8) * 8 + (x % 8);
                let (red, green, blue) = if tile < 1_536 {
                    if x % 2 == 0 {
                        (17, 53, 91)
                    } else {
                        (193, 229, 251)
                    }
                } else {
                    let code = tile * 64 + within_tile;
                    let code = u32::try_from(code)?
                        .wrapping_mul(2_654_435_761)
                        .wrapping_add(0x2468_ace1);
                    let [red, green, blue, _] = code.to_le_bytes();
                    (red, green, blue)
                };
                let alpha = if tile_row == 0 { u8::MAX } else { 254 };
                sampling_probe_pixels.extend_from_slice(&[red, green, blue, alpha]);
            }
        }
        let sampling_probe_image = DecodedImage::new(
            u32::try_from(sampling_probe_width)?,
            u32::try_from(sampling_probe_height)?,
            sampling_probe_pixels,
            ColorType::Rgba8,
        );
        // The smaller lossless probe above already proves ordinary and
        // ample-budget byte identity; this wider fixture is reserved for the
        // interior sampling boundary and sink rollback.
        let sampling_probe_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(967_091);
        let sampling_probe_error = match image_slash_star::encode_with_policy(
            &sampling_probe_image,
            ImageFormat::WebP,
            &lossless_options,
            &sampling_probe_policy,
        ) {
            Ok(_) => return Err("VP8L histogram sampling row budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            sampling_probe_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 967_091,
                observed: 967_092,
            }
        ));
        let mut sampling_probe_sink = vec![0xB7];
        let sampling_probe_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &sampling_probe_image,
            ImageFormat::WebP,
            &lossless_options,
            &sampling_probe_policy,
            &mut sampling_probe_sink,
        ) {
            Ok(_) => return Err("VP8L histogram sampling row budget wrote output".into()),
            Err(error) => error,
        };
        assert!(matches!(
            sampling_probe_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 967_091,
                observed: 967_092,
            }
        ));
        assert_eq!(sampling_probe_sink, vec![0xB7]);
        // Lossless VP8L Huffman RLE preparation now charges after each 64
        // code-length symbols while optimizing and tokenizing the fixed
        // alphabets. This is Rust-only work-control evidence: Pillow has no
        // caller token, work budget, or caller-owned sink.
        let huffman_rle_policy = image_slash_star::EncodePolicy::new().with_max_work_units(828);
        let huffman_rle_error = match image_slash_star::encode_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &huffman_rle_policy,
        ) {
            Ok(_) => return Err("VP8L Huffman RLE budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            huffman_rle_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 828,
                observed: 829,
            }
        ));
        let mut huffman_rle_sink = vec![0xB1];
        let huffman_rle_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(827),
            &mut huffman_rle_sink,
        ) {
            Ok(_) => return Err("VP8L Huffman RLE sink budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            huffman_rle_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 827,
                observed: 828,
            }
        ));
        assert_eq!(huffman_rle_sink, vec![0xB1]);
        // The smaller lossless probe above already proves byte identity for
        // the ordinary and ample-budget VP8L paths. This larger patterned
        // fixture is reserved for the late logical/output boundaries so the
        // contract does not pay for two redundant full encodes before them.
        // VP8L token-aware bit writing now charges a checkpoint after each
        // 8 logical bits. This real patterned lossless probe reaches the
        // first new interval before the later writer work. Pillow has no
        // caller token or work-budget result, so this remains Rust-only
        // evidence with no parity row or coverage-only hook.
        let bitstream_8_checkpoint_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(200);
        let bitstream_8_checkpoint_error = match image_slash_star::encode_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &bitstream_8_checkpoint_policy,
        ) {
            Ok(_) => return Err("VP8L 8-bit bitstream budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_8_checkpoint_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 200,
                observed: 201,
            }
        ));
        let mut bitstream_8_checkpoint_sink = vec![0xC8];
        let bitstream_8_checkpoint_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(199),
            &mut bitstream_8_checkpoint_sink,
        ) {
            Ok(_) => {
                return Err("VP8L 8-bit bitstream sink budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_8_checkpoint_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 199,
                observed: 200,
            }
        ));
        assert_eq!(bitstream_8_checkpoint_sink, vec![0xC8]);

        // The 16-bit VP8L boundary remains independently enforced after the
        // new 8-bit poll.
        let bitstream_16_checkpoint_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(202);
        let bitstream_16_checkpoint_error = match image_slash_star::encode_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &bitstream_16_checkpoint_policy,
        ) {
            Ok(_) => return Err("VP8L 16-bit bitstream budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_16_checkpoint_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 202,
                observed: 203,
            }
        ));
        let mut bitstream_16_checkpoint_sink = vec![0xC7];
        let bitstream_16_checkpoint_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(201),
            &mut bitstream_16_checkpoint_sink,
        ) {
            Ok(_) => {
                return Err("VP8L 16-bit bitstream sink budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_16_checkpoint_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 201,
                observed: 202,
            }
        ));
        assert_eq!(bitstream_16_checkpoint_sink, vec![0xC7]);
        // The finer VP8L logical-bitstream interval rejects at the selected
        // 32-bit boundary after the earlier lossless stages. This remains
        // Rust-only work-control evidence: Pillow has no caller budget or
        // equivalent result.
        let bitstream_32_checkpoint_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(206);
        let bitstream_32_checkpoint_error = match image_slash_star::encode_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &bitstream_32_checkpoint_policy,
        ) {
            Ok(_) => return Err("VP8L 32-bit bitstream budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_32_checkpoint_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 206,
                observed: 207,
            }
        ));
        let mut bitstream_32_checkpoint_sink = vec![0xC6];
        let bitstream_32_checkpoint_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(205),
            &mut bitstream_32_checkpoint_sink,
        ) {
            Ok(_) => {
                return Err("VP8L 32-bit bitstream sink budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_32_checkpoint_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 205,
                observed: 206,
            }
        ));
        assert_eq!(bitstream_32_checkpoint_sink, vec![0xC6]);

        // The VP8L 64-bit logical-bitstream interval remains independently
        // enforced after the finer 32-bit boundary. Pillow has no caller
        // budget or equivalent result.
        let bitstream_64_checkpoint_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(32_567);
        let bitstream_64_checkpoint_error = match image_slash_star::encode_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &bitstream_64_checkpoint_policy,
        ) {
            Ok(_) => return Err("VP8L 64-bit bitstream budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_64_checkpoint_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 32_567,
                observed: 32_568,
            }
        ));
        let mut bitstream_64_checkpoint_sink = vec![0xC5];
        let bitstream_64_checkpoint_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(32_566),
            &mut bitstream_64_checkpoint_sink,
        ) {
            Ok(_) => {
                return Err("VP8L 64-bit bitstream sink budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_64_checkpoint_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 32_566,
                observed: 32_567,
            }
        ));
        assert_eq!(bitstream_64_checkpoint_sink, vec![0xC5]);

        // The existing VP8L 128-bit logical-bitstream interval remains
        // independently enforced after the finer 32-bit and 64-bit
        // boundaries. Pillow has
        // no caller budget or equivalent result.
        let finest_bitstream_checkpoint_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(54_502);
        let finest_bitstream_checkpoint_error = match image_slash_star::encode_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &finest_bitstream_checkpoint_policy,
        ) {
            Ok(_) => return Err("VP8L 128-bit bitstream budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            finest_bitstream_checkpoint_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 54_502,
                observed: 54_503,
            }
        ));
        let mut finest_bitstream_checkpoint_sink = vec![0xAB];
        let finest_bitstream_checkpoint_sink_error =
            match image_slash_star::encode_to_sink_with_policy(
                &output_lossless_image,
                ImageFormat::WebP,
                &lossless_options,
                &image_slash_star::EncodePolicy::new().with_max_work_units(54_501),
                &mut finest_bitstream_checkpoint_sink,
            ) {
                Ok(_) => {
                    return Err("VP8L 128-bit bitstream sink budget unexpectedly completed".into());
                }
                Err(error) => error,
            };
        assert!(matches!(
            finest_bitstream_checkpoint_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 54_501,
                observed: 54_502,
            }
        ));
        assert_eq!(finest_bitstream_checkpoint_sink, vec![0xAB]);

        // The existing VP8L 256-bit logical-bitstream interval remains
        // independently enforced after the finer 128-bit boundary.
        // Pillow has no caller budget or equivalent result.
        let fine_bitstream_checkpoint_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(54_555);
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
                maximum: 54_555,
                observed: 54_556,
            }
        ));
        let mut fine_bitstream_checkpoint_sink = vec![0xAA];
        let fine_bitstream_checkpoint_sink_error =
            match image_slash_star::encode_to_sink_with_policy(
                &output_lossless_image,
                ImageFormat::WebP,
                &lossless_options,
                &image_slash_star::EncodePolicy::new().with_max_work_units(54_554),
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
                maximum: 54_554,
                observed: 54_555,
            }
        ));
        assert_eq!(fine_bitstream_checkpoint_sink, vec![0xAA]);

        // The existing VP8L 512-bit logical-bitstream interval remains
        // independently enforced after the finer 128-bit and 256-bit
        // boundaries. Pillow has no caller budget or equivalent result.
        let bitstream_checkpoint_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(54_940);
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
                maximum: 54_940,
                observed: 54_941,
            }
        ));
        let mut bitstream_checkpoint_sink = vec![0xAA];
        let bitstream_checkpoint_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(54_939),
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
                maximum: 54_939,
                observed: 54_940,
            }
        ));
        assert_eq!(bitstream_checkpoint_sink, vec![0xAA]);

        // VP8L now charges the next logical bitstream interval after each
        // 1,024 written bits. This is Rust-only work-control evidence:
        // Pillow has no caller token or equivalent budget result.
        let bitstream_1024_checkpoint_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(56_194);
        let bitstream_1024_checkpoint_error = match image_slash_star::encode_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &bitstream_1024_checkpoint_policy,
        ) {
            Ok(_) => return Err("VP8L 1024-bit bitstream budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_1024_checkpoint_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 56_194,
                observed: 56_195,
            }
        ));
        let mut bitstream_1024_checkpoint_sink = vec![0xA9];
        let bitstream_1024_checkpoint_sink_error =
            match image_slash_star::encode_to_sink_with_policy(
                &output_lossless_image,
                ImageFormat::WebP,
                &lossless_options,
                &image_slash_star::EncodePolicy::new().with_max_work_units(56_193),
                &mut bitstream_1024_checkpoint_sink,
            ) {
                Ok(_) => {
                    return Err(
                        "VP8L 1024-bit bitstream sink budget unexpectedly wrote output".into(),
                    );
                }
                Err(error) => error,
            };
        assert!(matches!(
            bitstream_1024_checkpoint_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 56_193,
                observed: 56_194,
            }
        ));
        assert_eq!(bitstream_1024_checkpoint_sink, vec![0xA9]);

        // The next VP8L logical interval is independently enforced after
        // each 2,048 written bits. This is Rust-only work-control evidence:
        // Pillow has no caller token or equivalent budget result.
        let bitstream_2048_checkpoint_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(56_560);
        let bitstream_2048_checkpoint_error = match image_slash_star::encode_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &bitstream_2048_checkpoint_policy,
        ) {
            Ok(_) => return Err("VP8L 2048-bit bitstream budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_2048_checkpoint_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 56_560,
                observed: 56_561,
            }
        ));
        let mut bitstream_2048_checkpoint_sink = vec![0xA8];
        let bitstream_2048_checkpoint_sink_error =
            match image_slash_star::encode_to_sink_with_policy(
                &output_lossless_image,
                ImageFormat::WebP,
                &lossless_options,
                &image_slash_star::EncodePolicy::new().with_max_work_units(56_559),
                &mut bitstream_2048_checkpoint_sink,
            ) {
                Ok(_) => {
                    return Err(
                        "VP8L 2048-bit bitstream sink budget unexpectedly wrote output".into(),
                    );
                }
                Err(error) => error,
            };
        assert!(matches!(
            bitstream_2048_checkpoint_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 56_559,
                observed: 56_560,
            }
        ));
        assert_eq!(bitstream_2048_checkpoint_sink, vec![0xA8]);

        // The next VP8L logical interval is independently enforced after
        // each 4,096 written bits. This is Rust-only work-control evidence:
        // Pillow has no caller token or equivalent budget result.
        let bitstream_4096_checkpoint_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(57_074);
        let bitstream_4096_checkpoint_error = match image_slash_star::encode_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &bitstream_4096_checkpoint_policy,
        ) {
            Ok(_) => return Err("VP8L 4096-bit bitstream budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_4096_checkpoint_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 57_074,
                observed: 57_075,
            }
        ));
        let mut bitstream_4096_checkpoint_sink = vec![0xA7];
        let bitstream_4096_checkpoint_sink_error =
            match image_slash_star::encode_to_sink_with_policy(
                &output_lossless_image,
                ImageFormat::WebP,
                &lossless_options,
                &image_slash_star::EncodePolicy::new().with_max_work_units(57_073),
                &mut bitstream_4096_checkpoint_sink,
            ) {
                Ok(_) => {
                    return Err(
                        "VP8L 4096-bit bitstream sink budget unexpectedly wrote output".into(),
                    );
                }
                Err(error) => error,
            };
        assert!(matches!(
            bitstream_4096_checkpoint_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 57_073,
                observed: 57_074,
            }
        ));
        assert_eq!(bitstream_4096_checkpoint_sink, vec![0xA7]);

        // The next VP8L logical interval is independently enforced after
        // each 8,192 written bits. This is Rust-only work-control evidence:
        // Pillow has no caller token or equivalent budget result.
        let bitstream_8192_checkpoint_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(58_098);
        let bitstream_8192_checkpoint_error = match image_slash_star::encode_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &bitstream_8192_checkpoint_policy,
        ) {
            Ok(_) => return Err("VP8L 8192-bit bitstream budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_8192_checkpoint_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 58_098,
                observed: 58_099,
            }
        ));
        let mut bitstream_8192_checkpoint_sink = vec![0xA4];
        let bitstream_8192_checkpoint_sink_error =
            match image_slash_star::encode_to_sink_with_policy(
                &output_lossless_image,
                ImageFormat::WebP,
                &lossless_options,
                &image_slash_star::EncodePolicy::new().with_max_work_units(58_097),
                &mut bitstream_8192_checkpoint_sink,
            ) {
                Ok(_) => {
                    return Err(
                        "VP8L 8192-bit bitstream sink budget unexpectedly wrote output".into(),
                    );
                }
                Err(error) => error,
            };
        assert!(matches!(
            bitstream_8192_checkpoint_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 58_097,
                observed: 58_098,
            }
        ));
        assert_eq!(bitstream_8192_checkpoint_sink, vec![0xA4]);

        // A deterministic high-entropy probe reaches the next VP8L logical
        // interval without changing the smaller probe used by the earlier
        // boundaries. Pillow has no caller budget or equivalent result, so
        // this remains Rust-only work-control evidence.
        let mut bitstream_16384_pixels = Vec::with_capacity(128 * 128 * 3);
        let mut bitstream_16384_state = 0x1234_5678u32;
        for _ in 0..128 * 128 {
            bitstream_16384_state = bitstream_16384_state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            bitstream_16384_pixels.extend_from_slice(&[
                u8::try_from(bitstream_16384_state >> 24)?,
                u8::try_from((bitstream_16384_state >> 16) & 0xff)?,
                u8::try_from((bitstream_16384_state >> 8) & 0xff)?,
            ]);
        }
        let bitstream_16384_image =
            DecodedImage::new(128, 128, bitstream_16384_pixels, ColorType::Rgb8);
        let bitstream_16384_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(9_341);
        let bitstream_16384_error = match image_slash_star::encode_with_policy(
            &bitstream_16384_image,
            ImageFormat::WebP,
            &lossless_options,
            &bitstream_16384_policy,
        ) {
            Ok(_) => return Err("VP8L 16384-bit bitstream budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_16384_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 9_341,
                observed: 9_342,
            }
        ));
        let mut bitstream_16384_sink = vec![0xA3];
        let bitstream_16384_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &bitstream_16384_image,
            ImageFormat::WebP,
            &lossless_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(9_340),
            &mut bitstream_16384_sink,
        ) {
            Ok(_) => {
                return Err(
                    "VP8L 16384-bit bitstream sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_16384_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 9_340,
                observed: 9_341,
            }
        ));
        assert_eq!(bitstream_16384_sink, vec![0xA3]);

        // The deterministic probe also reaches the next VP8L logical
        // bitstream interval. This remains Rust-only work-control evidence:
        // Pillow has no caller budget or equivalent result.
        let bitstream_32768_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(9_342);
        let bitstream_32768_error = match image_slash_star::encode_with_policy(
            &bitstream_16384_image,
            ImageFormat::WebP,
            &lossless_options,
            &bitstream_32768_policy,
        ) {
            Ok(_) => return Err("VP8L 32768-bit bitstream budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_32768_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 9_342,
                observed: 9_343,
            }
        ));
        let mut bitstream_32768_sink = vec![0xA2];
        let bitstream_32768_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &bitstream_16384_image,
            ImageFormat::WebP,
            &lossless_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(9_341),
            &mut bitstream_32768_sink,
        ) {
            Ok(_) => {
                return Err(
                    "VP8L 32768-bit bitstream sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_32768_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 9_341,
                observed: 9_342,
            }
        ));
        assert_eq!(bitstream_32768_sink, vec![0xA2]);

        // The deterministic probe also reaches the next VP8L logical
        // bitstream interval. This remains Rust-only work-control evidence:
        // Pillow has no caller budget or equivalent result.
        let bitstream_65536_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(9_343);
        let bitstream_65536_error = match image_slash_star::encode_with_policy(
            &bitstream_16384_image,
            ImageFormat::WebP,
            &lossless_options,
            &bitstream_65536_policy,
        ) {
            Ok(_) => return Err("VP8L 65536-bit bitstream budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_65536_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 9_343,
                observed: 9_344,
            }
        ));
        let mut bitstream_65536_sink = vec![0xA1];
        let bitstream_65536_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &bitstream_16384_image,
            ImageFormat::WebP,
            &lossless_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(9_342),
            &mut bitstream_65536_sink,
        ) {
            Ok(_) => {
                return Err(
                    "VP8L 65536-bit bitstream sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_65536_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 9_342,
                observed: 9_343,
            }
        ));
        assert_eq!(bitstream_65536_sink, vec![0xA1]);

        let mut bitstream_131072_pixels = Vec::with_capacity(256 * 256 * 3);
        let mut bitstream_131072_state = 0x9E37_79B9u32;
        for _ in 0..256 * 256 {
            bitstream_131072_state = bitstream_131072_state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            bitstream_131072_pixels.extend_from_slice(&[
                u8::try_from(bitstream_131072_state >> 24)?,
                u8::try_from((bitstream_131072_state >> 16) & 0xff)?,
                u8::try_from((bitstream_131072_state >> 8) & 0xff)?,
            ]);
        }
        let bitstream_131072_image =
            DecodedImage::new(256, 256, bitstream_131072_pixels, ColorType::Rgb8);
        // The deterministic 256×256 probe reaches the next VP8L logical
        // bitstream interval. This remains Rust-only work-control evidence:
        // Pillow has no caller budget or equivalent result.
        let bitstream_131072_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(41_542);
        let bitstream_131072_error = match image_slash_star::encode_with_policy(
            &bitstream_131072_image,
            ImageFormat::WebP,
            &lossless_options,
            &bitstream_131072_policy,
        ) {
            Ok(_) => return Err("VP8L 131072-bit bitstream budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_131072_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 41_542,
                observed: 41_543,
            }
        ));
        let mut bitstream_131072_sink = vec![0xA0];
        let bitstream_131072_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &bitstream_131072_image,
            ImageFormat::WebP,
            &lossless_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(41_541),
            &mut bitstream_131072_sink,
        ) {
            Ok(_) => {
                return Err(
                    "VP8L 131072-bit bitstream sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_131072_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 41_541,
                observed: 41_542,
            }
        ));
        assert_eq!(bitstream_131072_sink, vec![0xA0]);

        let mut bitstream_262144_pixels = Vec::with_capacity(656 * 656 * 3);
        let mut bitstream_262144_state = 0xC001_C0DEu32;
        for _ in 0..656 * 656 {
            bitstream_262144_state = bitstream_262144_state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            bitstream_262144_pixels.extend_from_slice(&[
                u8::try_from(bitstream_262144_state >> 24)?,
                u8::try_from((bitstream_262144_state >> 16) & 0xff)?,
                u8::try_from((bitstream_262144_state >> 8) & 0xff)?,
            ]);
        }
        let bitstream_262144_image =
            DecodedImage::new(656, 656, bitstream_262144_pixels, ColorType::Rgb8);
        let bitstream_262144_error = match image_slash_star::encode_with_policy(
            &bitstream_262144_image,
            ImageFormat::WebP,
            &lossless_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(262_602),
        ) {
            Ok(_) => return Err("VP8L 262144-bit bitstream budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_262144_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 262_602,
                observed: 262_603,
            }
        ));
        let mut bitstream_262144_sink = vec![0x9F];
        let bitstream_262144_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &bitstream_262144_image,
            ImageFormat::WebP,
            &lossless_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(262_601),
            &mut bitstream_262144_sink,
        ) {
            Ok(_) => {
                return Err(
                    "VP8L 262144-bit bitstream sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_262144_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 262_601,
                observed: 262_602,
            }
        ));
        assert_eq!(bitstream_262144_sink, vec![0x9F]);

        // The next VP8L logical bitstream interval is independently enforced
        // after 524,288 written bits. This remains Rust-only work-control
        // evidence: Pillow has no caller token or equivalent budget result.
        let bitstream_524288_error = match image_slash_star::encode_with_policy(
            &bitstream_262144_image,
            ImageFormat::WebP,
            &lossless_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(328_138),
        ) {
            Ok(_) => return Err("VP8L 524288-bit bitstream budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_524288_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 328_138,
                observed: 328_139,
            }
        ));
        let mut bitstream_524288_sink = vec![0x9E];
        let bitstream_524288_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &bitstream_262144_image,
            ImageFormat::WebP,
            &lossless_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(328_137),
            &mut bitstream_524288_sink,
        ) {
            Ok(_) => {
                return Err(
                    "VP8L 524288-bit bitstream sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_524288_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 328_137,
                observed: 328_138,
            }
        ));
        assert_eq!(bitstream_524288_sink, vec![0x9E]);

        // The next VP8L logical bitstream interval is independently enforced
        // after 1,048,576 written bits. This remains Rust-only work-control
        // evidence: Pillow has no caller budget or equivalent result.
        let bitstream_1048576_error = match image_slash_star::encode_with_policy(
            &bitstream_262144_image,
            ImageFormat::WebP,
            &lossless_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(459_210),
        ) {
            Ok(_) => return Err("VP8L 1048576-bit bitstream budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_1048576_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 459_210,
                observed: 459_211,
            }
        ));
        let mut bitstream_1048576_sink = vec![0x9D];
        let bitstream_1048576_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &bitstream_262144_image,
            ImageFormat::WebP,
            &lossless_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(459_209),
            &mut bitstream_1048576_sink,
        ) {
            Ok(_) => {
                return Err(
                    "VP8L 1048576-bit bitstream sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            bitstream_1048576_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 459_209,
                observed: 459_210,
            }
        ));
        assert_eq!(bitstream_1048576_sink, vec![0x9D]);

        // A one-unit-lower budget rejects at the first 1,024-byte emitted
        // output interval. This is a separate Rust-only work-control
        // boundary; it is not Pillow byte/pixel parity evidence.
        let output_checkpoint_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(56_548);
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
                maximum: 56_548,
                observed: 56_549,
            }
        ));
        let mut output_checkpoint_sink = vec![0xAA];
        let output_checkpoint_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &output_lossless_image,
            ImageFormat::WebP,
            &lossless_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(56_547),
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
                maximum: 56_547,
                observed: 56_548,
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
        let partition_probe_pixels: Vec<u8> = (0..272 * 272 * 3)
            .map(|index: usize| u8::try_from(index.wrapping_mul(37) % 256).unwrap_or(0))
            .collect();
        let partition_probe = DecodedImage::new(272, 272, partition_probe_pixels, ColorType::Rgb8);
        // The late output/bit checkpoints need only compact patterned probes;
        // keeping their geometry small avoids repeating hundreds of thousands
        // of pixels for every whole-buffer and sink boundary.
        let deep_partition_probe_pixels: Vec<u8> = (0..64 * 96 * 3)
            .map(|index: usize| u8::try_from(index.wrapping_mul(37) % 256).unwrap_or(0))
            .collect();
        let deep_partition_probe =
            DecodedImage::new(64, 96, deep_partition_probe_pixels, ColorType::Rgb8);
        let wide_partition_probe_pixels: Vec<u8> = (0..64 * 64 * 3)
            .map(|index: usize| u8::try_from(index.wrapping_mul(37) % 256).unwrap_or(0))
            .collect();
        let wide_partition_probe =
            DecodedImage::new(64, 64, wide_partition_probe_pixels, ColorType::Rgb8);
        let partition_8192_probe_pixels: Vec<u8> = (0..512 * 512 * 3)
            .map(|index: usize| u8::try_from(index.wrapping_mul(37) % 256).unwrap_or(0))
            .collect();
        let partition_8192_probe =
            DecodedImage::new(512, 512, partition_8192_probe_pixels, ColorType::Rgb8);
        // The 32,768-bit first-partition boundary needs a larger deterministic
        // probe: this 1,024x960 image supplies 64x60 macroblocks while keeping
        // the work-control assertion independent from the Pillow oracle.
        let partition_32768_probe_pixels: Vec<u8> = (0..1024 * 960 * 3)
            .map(|index: usize| u8::try_from(index.wrapping_mul(37) % 256).unwrap_or(0))
            .collect();
        let partition_32768_probe =
            DecodedImage::new(1024, 960, partition_32768_probe_pixels, ColorType::Rgb8);
        // The 65,536-bit first-partition and coefficient boundaries use a
        // 1,024x1,024 patterned probe (64x64 macroblocks). Pillow has no
        // caller budget or equivalent result, so this remains Rust-only.
        let partition_65536_probe_pixels: Vec<u8> = (0..1024 * 1024 * 3)
            .map(|index: usize| u8::try_from(index.wrapping_mul(37) % 256).unwrap_or(0))
            .collect();
        let partition_65536_probe =
            DecodedImage::new(1024, 1024, partition_65536_probe_pixels, ColorType::Rgb8);
        // A compact high-entropy probe reaches both 131,072-bit VP8 paths at
        // quality 100 without repeating the 6 MiB 2,048x1,024 candidate.
        let mut partition_131072_probe_pixels = Vec::with_capacity(768 * 768 * 3);
        let mut partition_131072_state = 0xA5A5_5A5Au32;
        for _ in 0..768 * 768 {
            partition_131072_state = partition_131072_state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            partition_131072_probe_pixels.extend_from_slice(&[
                u8::try_from(partition_131072_state >> 24)?,
                u8::try_from((partition_131072_state >> 16) & 0xff)?,
                u8::try_from((partition_131072_state >> 8) & 0xff)?,
            ]);
        }
        let partition_131072_probe =
            DecodedImage::new(768, 768, partition_131072_probe_pixels, ColorType::Rgb8);
        let mut partition_131072_options = analysis_options.clone();
        if let EncodeOptions::WebP(options) = &mut partition_131072_options {
            options.quality = Some(100);
        }
        // A 1,024×1,024 high-entropy probe reaches the next 262,144-bit
        // interval in both VP8 paths at quality 100. It is kept separate from
        // the compact 131,072-bit probe so the boundary remains deterministic
        // without reintroducing the discarded 2,048×1,024 allocation.
        let mut partition_262144_probe_pixels = Vec::with_capacity(1024 * 1024 * 3);
        let mut partition_262144_state = 0xA5A5_5A5Au32;
        for _ in 0..1024 * 1024 {
            partition_262144_state = partition_262144_state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            partition_262144_probe_pixels.extend_from_slice(&[
                u8::try_from(partition_262144_state >> 24)?,
                u8::try_from((partition_262144_state >> 16) & 0xff)?,
                u8::try_from((partition_262144_state >> 8) & 0xff)?,
            ]);
        }
        let partition_262144_probe =
            DecodedImage::new(1024, 1024, partition_262144_probe_pixels, ColorType::Rgb8);
        let mut partition_262144_options = analysis_options.clone();
        if let EncodeOptions::WebP(options) = &mut partition_262144_options {
            options.quality = Some(100);
        }
        // A deterministic 832x832 checkerboard reaches both remaining
        // coefficient-only logical intervals at quality 100 while keeping the
        // boundary probe compact. Its strong alternating chroma/luma signal
        // generates the required coefficient work without a second
        // multi-megapixel high-entropy allocation.
        let mut coefficient_1048576_probe_pixels = Vec::with_capacity(832 * 832 * 3);
        for y in 0..832 {
            for x in 0..832 {
                let value = if (x + y) % 2 == 0 { 0 } else { 255 };
                coefficient_1048576_probe_pixels.extend_from_slice(&[value, 255 - value, value]);
            }
        }
        let coefficient_1048576_probe =
            DecodedImage::new(832, 832, coefficient_1048576_probe_pixels, ColorType::Rgb8);
        let mut coefficient_1048576_options = analysis_options.clone();
        if let EncodeOptions::WebP(options) = &mut coefficient_1048576_options {
            options.quality = Some(100);
        }
        // First-partition boolean coding now charges a checkpoint after each
        // 8 coded bits. The patterned probe reaches the first new logical
        // interval after the existing preparation work. Pillow has no caller
        // token or work-budget result, so this remains Rust-only evidence
        // with no parity row or coverage-only hook.
        let partition_bit_policy_8 = image_slash_star::EncodePolicy::new().with_max_work_units(102);
        let partition_bit_error_8 = match image_slash_star::encode_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &partition_bit_policy_8,
        ) {
            Ok(_) => {
                return Err("bounded WebP 8-bit partition budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_error_8,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 102,
                observed: 103,
            }
        ));
        let mut partition_bit_sink_8 = vec![0xC8];
        let partition_bit_sink_error_8 = match image_slash_star::encode_to_sink_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(101),
            &mut partition_bit_sink_8,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 8-bit partition sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_sink_error_8,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 101,
                observed: 102,
            }
        ));
        assert_eq!(partition_bit_sink_8, vec![0xC8]);

        // The 16-bit first-partition boundary remains independently enforced
        // after the new 8-bit poll.
        let partition_bit_policy_16 =
            image_slash_star::EncodePolicy::new().with_max_work_units(104);
        let partition_bit_error_16 = match image_slash_star::encode_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &partition_bit_policy_16,
        ) {
            Ok(_) => {
                return Err("bounded WebP 16-bit partition budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_error_16,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 104,
                observed: 105,
            }
        ));
        let mut partition_bit_sink_16 = vec![0xC7];
        let partition_bit_sink_error_16 = match image_slash_star::encode_to_sink_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(103),
            &mut partition_bit_sink_16,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 16-bit partition sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_sink_error_16,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 103,
                observed: 104,
            }
        ));
        assert_eq!(partition_bit_sink_16, vec![0xC7]);
        // First-partition boolean coding now charges the finer logical
        // checkpoint after each 32 coded bits. This patterned 272x272 probe
        // reaches that interval after the earlier partition stages. Pillow
        // has no caller token or work-budget result, so this remains Rust-only
        // evidence with no parity row or coverage-only hook.
        let partition_bit_policy_32 =
            image_slash_star::EncodePolicy::new().with_max_work_units(108);
        let partition_bit_error_32 = match image_slash_star::encode_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &partition_bit_policy_32,
        ) {
            Ok(_) => {
                return Err("bounded WebP 32-bit partition budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_error_32,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 108,
                observed: 109,
            }
        ));
        let mut partition_bit_sink_32 = vec![0xC6];
        let partition_bit_sink_error_32 = match image_slash_star::encode_to_sink_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(107),
            &mut partition_bit_sink_32,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 32-bit partition sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_sink_error_32,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 107,
                observed: 108,
            }
        ));
        assert_eq!(partition_bit_sink_32, vec![0xC6]);
        // First-partition boolean coding now charges a finer logical
        // checkpoint after each 64 coded bits. This patterned 272x272 probe
        // reaches that interval after the finer 32-bit interval. Pillow
        // has no caller token or work-budget result, so this remains Rust-only
        // evidence with no parity row or coverage-only hook.
        let partition_bit_policy_64 =
            image_slash_star::EncodePolicy::new().with_max_work_units(116);
        let partition_bit_error_64 = match image_slash_star::encode_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &partition_bit_policy_64,
        ) {
            Ok(_) => {
                return Err("bounded WebP 64-bit partition budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_error_64,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 116,
                observed: 117,
            }
        ));
        let mut partition_bit_sink_64 = vec![0xC4];
        let partition_bit_sink_error_64 = match image_slash_star::encode_to_sink_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(115),
            &mut partition_bit_sink_64,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 64-bit partition sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_sink_error_64,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 115,
                observed: 116,
            }
        ));
        assert_eq!(partition_bit_sink_64, vec![0xC4]);
        // First-partition boolean coding now charges a finer logical
        // checkpoint after each 128 coded bits. This patterned 272x272 probe
        // reaches that interval after the finer 32-bit and 64-bit intervals.
        // Pillow
        // has no caller token or work-budget result, so this remains Rust-only
        // evidence with no parity row or coverage-only hook.
        let partition_bit_policy_128 =
            image_slash_star::EncodePolicy::new().with_max_work_units(132);
        let partition_bit_error_128 = match image_slash_star::encode_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &partition_bit_policy_128,
        ) {
            Ok(_) => {
                return Err("bounded WebP 128-bit partition budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_error_128,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 132,
                observed: 133,
            }
        ));
        let mut partition_bit_sink_128 = vec![0xB8];
        let partition_bit_sink_error_128 = match image_slash_star::encode_to_sink_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(131),
            &mut partition_bit_sink_128,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 128-bit partition sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_sink_error_128,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 131,
                observed: 132,
            }
        ));
        assert_eq!(partition_bit_sink_128, vec![0xB8]);
        // First-partition boolean coding now charges a logical checkpoint
        // after each 256 coded bits. This patterned 272x272 probe reaches
        // that interval after the finer logical intervals. Pillow has no caller token
        // or work-budget result, so this remains Rust-only evidence with no
        // parity row or coverage-only hook; ordinary byte identity is covered
        // by the active parity matrix and the ample-budget probe above.
        let partition_bit_policy_256 =
            image_slash_star::EncodePolicy::new().with_max_work_units(164);
        let partition_bit_error_256 = match image_slash_star::encode_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &partition_bit_policy_256,
        ) {
            Ok(_) => {
                return Err("bounded WebP 256-bit partition budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_error_256,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 164,
                observed: 165,
            }
        ));
        let mut partition_bit_sink_256 = vec![0xB7];
        let partition_bit_sink_error_256 = match image_slash_star::encode_to_sink_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(163),
            &mut partition_bit_sink_256,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 256-bit partition sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_sink_error_256,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 163,
                observed: 164,
            }
        ));
        assert_eq!(partition_bit_sink_256, vec![0xB7]);

        // The 512-bit first-partition boundary remains separately enforced
        // after the finer logical intervals.
        let partition_bit_policy_512 =
            image_slash_star::EncodePolicy::new().with_max_work_units(354);
        let partition_bit_error_512 = match image_slash_star::encode_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &partition_bit_policy_512,
        ) {
            Ok(_) => {
                return Err("bounded WebP 512-bit partition budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_error_512,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 354,
                observed: 355,
            }
        ));
        let mut partition_bit_sink_512 = vec![0xB6];
        let partition_bit_sink_error_512 = match image_slash_star::encode_to_sink_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(353),
            &mut partition_bit_sink_512,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 512-bit partition sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_sink_error_512,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 353,
                observed: 354,
            }
        ));
        assert_eq!(partition_bit_sink_512, vec![0xB6]);

        // The next logical first-partition interval is independently enforced
        // after each 1,024 coded bits. Pillow has no caller token or
        // work-budget result, so this remains Rust-only evidence with no
        // parity row or coverage-only hook.
        let partition_bit_policy_1024 =
            image_slash_star::EncodePolicy::new().with_max_work_units(271);
        let partition_bit_error_1024 = match image_slash_star::encode_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &partition_bit_policy_1024,
        ) {
            Ok(_) => {
                return Err("bounded WebP 1024-bit partition budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_error_1024,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 271,
                observed: 272,
            }
        ));
        let mut partition_bit_sink_1024 = vec![0xB3];
        let partition_bit_sink_error_1024 = match image_slash_star::encode_to_sink_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(270),
            &mut partition_bit_sink_1024,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 1024-bit partition sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_sink_error_1024,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 270,
                observed: 271,
            }
        ));
        assert_eq!(partition_bit_sink_1024, vec![0xB3]);

        // The next logical first-partition interval is independently enforced
        // after each 2,048 coded bits. Pillow has no caller token or
        // work-budget result, so this remains Rust-only evidence with no
        // parity row or coverage-only hook.
        let partition_bit_policy_2048 =
            image_slash_star::EncodePolicy::new().with_max_work_units(527);
        let partition_bit_error_2048 = match image_slash_star::encode_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &partition_bit_policy_2048,
        ) {
            Ok(_) => {
                return Err("bounded WebP 2048-bit partition budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_error_2048,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 527,
                observed: 528,
            }
        ));
        let mut partition_bit_sink_2048 = vec![0xB2];
        let partition_bit_sink_error_2048 = match image_slash_star::encode_to_sink_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(526),
            &mut partition_bit_sink_2048,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 2048-bit partition sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_sink_error_2048,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 526,
                observed: 527,
            }
        ));
        assert_eq!(partition_bit_sink_2048, vec![0xB2]);

        // The next logical first-partition interval is independently enforced
        // after each 4,096 coded bits. Pillow has no caller token or
        // work-budget result, so this remains Rust-only evidence with no
        // parity row or coverage-only hook.
        let partition_bit_policy_4096 =
            image_slash_star::EncodePolicy::new().with_max_work_units(1_125);
        let partition_bit_error_4096 = match image_slash_star::encode_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &partition_bit_policy_4096,
        ) {
            Ok(_) => {
                return Err("bounded WebP 4096-bit partition budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_error_4096,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 1_125,
                observed: 1_126,
            }
        ));
        let mut partition_bit_sink_4096 = vec![0xB1];
        let partition_bit_sink_error_4096 = match image_slash_star::encode_to_sink_with_policy(
            &partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(1_124),
            &mut partition_bit_sink_4096,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 4096-bit partition sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_sink_error_4096,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 1_124,
                observed: 1_125,
            }
        ));
        assert_eq!(partition_bit_sink_4096, vec![0xB1]);

        // The next logical first-partition interval is independently enforced
        // after each 8,192 coded bits. This larger patterned probe is needed
        // to reach the interval; it remains Rust-only work-control evidence.
        let partition_bit_policy_8192 =
            image_slash_star::EncodePolicy::new().with_max_work_units(2_384);
        let partition_bit_error_8192 = match image_slash_star::encode_with_policy(
            &partition_8192_probe,
            ImageFormat::WebP,
            &analysis_options,
            &partition_bit_policy_8192,
        ) {
            Ok(_) => {
                return Err("bounded WebP 8192-bit partition budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_error_8192,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 2_384,
                observed: 2_385,
            }
        ));
        let mut partition_bit_sink_8192 = vec![0xB0];
        let partition_bit_sink_error_8192 = match image_slash_star::encode_to_sink_with_policy(
            &partition_8192_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(2_383),
            &mut partition_bit_sink_8192,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 8192-bit partition sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_sink_error_8192,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 2_383,
                observed: 2_384,
            }
        ));
        assert_eq!(partition_bit_sink_8192, vec![0xB0]);

        // The existing coarser first-partition boundary remains separately
        // enforced after each 16,384 coded bits, after the finer logical
        // checkpoints above. The compact 64x64 probe reaches this boundary
        // while keeping the smaller probe above fast for the interior polls.
        let partition_bit_bounded =
            image_slash_star::EncodePolicy::new().with_max_work_units(1_574);
        let partition_bit_error = match image_slash_star::encode_with_policy(
            &wide_partition_probe,
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
                maximum: 1_574,
                observed: 1_575,
            }
        ));
        let mut partition_bit_sink = vec![0xB4];
        let partition_bit_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(1_573),
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
                maximum: 1_573,
                observed: 1_574,
            }
        ));
        assert_eq!(partition_bit_sink, vec![0xB4]);

        // The next logical first-partition interval is independently enforced
        // after each 32,768 coded bits. Pillow has no caller token,
        // work-budget result, or caller-owned sink, so this remains Rust-only
        // evidence with no parity row or coverage-only hook.
        let partition_bit_policy_32768 =
            image_slash_star::EncodePolicy::new().with_max_work_units(9_427);
        let partition_bit_error_32768 = match image_slash_star::encode_with_policy(
            &partition_32768_probe,
            ImageFormat::WebP,
            &analysis_options,
            &partition_bit_policy_32768,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 32768-bit partition budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_error_32768,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 9_427,
                observed: 9_428,
            }
        ));
        let mut partition_bit_sink_32768 = vec![0xD1];
        let partition_bit_sink_error_32768 = match image_slash_star::encode_to_sink_with_policy(
            &partition_32768_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(9_426),
            &mut partition_bit_sink_32768,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 32768-bit partition sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_sink_error_32768,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 9_426,
                observed: 9_427,
            }
        ));
        assert_eq!(partition_bit_sink_32768, vec![0xD1]);

        // The coefficient partition has a separate 32,768-bit logical
        // boundary. The existing 512x512 patterned probe reaches it after
        // first-partition work completes, keeping this Rust-only contract
        // focused and avoiding a second large generated image.
        let coefficient_bit_policy_32768 =
            image_slash_star::EncodePolicy::new().with_max_work_units(11_187);
        let coefficient_bit_error_32768 = match image_slash_star::encode_with_policy(
            &partition_8192_probe,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_bit_policy_32768,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 32768-bit coefficient budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_error_32768,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 11_187,
                observed: 11_188,
            }
        ));
        let mut coefficient_bit_sink_32768 = vec![0xD2];
        let coefficient_bit_sink_error_32768 = match image_slash_star::encode_to_sink_with_policy(
            &partition_8192_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(11_186),
            &mut coefficient_bit_sink_32768,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 32768-bit coefficient sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_sink_error_32768,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 11_186,
                observed: 11_187,
            }
        ));
        assert_eq!(coefficient_bit_sink_32768, vec![0xD2]);

        // The next logical first-partition interval is enforced after each
        // 65,536 coded bits. This larger patterned probe reaches it after the
        // existing 32,768-bit checkpoint. Pillow has no caller token,
        // work-budget result, or caller-owned sink, so this remains Rust-only
        // evidence with no parity row or coverage-only hook.
        let partition_bit_policy_65536 =
            image_slash_star::EncodePolicy::new().with_max_work_units(19_010);
        let partition_bit_error_65536 = match image_slash_star::encode_with_policy(
            &partition_65536_probe,
            ImageFormat::WebP,
            &analysis_options,
            &partition_bit_policy_65536,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 65536-bit partition budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_error_65536,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 19_010,
                observed: 19_011,
            }
        ));
        let mut partition_bit_sink_65536 = vec![0xD3];
        let partition_bit_sink_error_65536 = match image_slash_star::encode_to_sink_with_policy(
            &partition_65536_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(19_009),
            &mut partition_bit_sink_65536,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 65536-bit partition sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_sink_error_65536,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 19_009,
                observed: 19_010,
            }
        ));
        assert_eq!(partition_bit_sink_65536, vec![0xD3]);

        // The coefficient partition has its own 65,536-bit logical interval.
        // The same probe reaches it only after first-partition work completes,
        // proving the inclusive boundary and sink no-write behavior without a
        // second parity fixture.
        let coefficient_bit_policy_65536 =
            image_slash_star::EncodePolicy::new().with_max_work_units(35_929);
        let coefficient_bit_error_65536 = match image_slash_star::encode_with_policy(
            &partition_65536_probe,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_bit_policy_65536,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 65536-bit coefficient budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_error_65536,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 35_929,
                observed: 35_930,
            }
        ));
        let mut coefficient_bit_sink_65536 = vec![0xD4];
        let coefficient_bit_sink_error_65536 = match image_slash_star::encode_to_sink_with_policy(
            &partition_65536_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(35_928),
            &mut coefficient_bit_sink_65536,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 65536-bit coefficient sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_sink_error_65536,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 35_928,
                observed: 35_929,
            }
        ));
        assert_eq!(coefficient_bit_sink_65536, vec![0xD4]);

        // The compact high-entropy probe reaches the next logical interval in
        // both VP8 partitions. Pillow has no caller token, work-budget result,
        // or caller-owned sink, so this remains Rust-only evidence with no
        // parity row or coverage-only hook.
        let partition_bit_policy_131072 =
            image_slash_star::EncodePolicy::new().with_max_work_units(33_524);
        let partition_bit_error_131072 = match image_slash_star::encode_with_policy(
            &partition_131072_probe,
            ImageFormat::WebP,
            &partition_131072_options,
            &partition_bit_policy_131072,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 131072-bit partition budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_error_131072,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 33_524,
                observed: 33_525,
            }
        ));
        let mut partition_bit_sink_131072 = vec![0xD5];
        let partition_bit_sink_error_131072 = match image_slash_star::encode_to_sink_with_policy(
            &partition_131072_probe,
            ImageFormat::WebP,
            &partition_131072_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(33_523),
            &mut partition_bit_sink_131072,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 131072-bit partition sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_sink_error_131072,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 33_523,
                observed: 33_524,
            }
        ));
        assert_eq!(partition_bit_sink_131072, vec![0xD5]);

        let coefficient_bit_policy_131072 =
            image_slash_star::EncodePolicy::new().with_max_work_units(75_692);
        let coefficient_bit_error_131072 = match image_slash_star::encode_with_policy(
            &partition_131072_probe,
            ImageFormat::WebP,
            &partition_131072_options,
            &coefficient_bit_policy_131072,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 131072-bit coefficient budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_error_131072,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 75_692,
                observed: 75_693,
            }
        ));
        let mut coefficient_bit_sink_131072 = vec![0xD6];
        let coefficient_bit_sink_error_131072 = match image_slash_star::encode_to_sink_with_policy(
            &partition_131072_probe,
            ImageFormat::WebP,
            &partition_131072_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(75_691),
            &mut coefficient_bit_sink_131072,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 131072-bit coefficient sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_sink_error_131072,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 75_691,
                observed: 75_692,
            }
        ));
        assert_eq!(coefficient_bit_sink_131072, vec![0xD6]);

        // The next logical interval is independently enforced after each
        // 262,144 coded bits in both VP8 partitions. The 1,024×1,024
        // high-entropy probe reaches whole-buffer and sink cumulative poll
        // counts of 66,880 and 66,879 at that checkpoint. Pillow has no
        // caller token, work-budget result, or caller-owned sink, so this
        // remains Rust-only evidence with no parity row or coverage-only hook.
        let partition_bit_policy_262144 =
            image_slash_star::EncodePolicy::new().with_max_work_units(66_879);
        let partition_bit_error_262144 = match image_slash_star::encode_with_policy(
            &partition_262144_probe,
            ImageFormat::WebP,
            &partition_262144_options,
            &partition_bit_policy_262144,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 262144-bit partition budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_error_262144,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 66_879,
                observed: 66_880,
            }
        ));
        let mut partition_bit_sink_262144 = vec![0xD7];
        let partition_bit_sink_error_262144 = match image_slash_star::encode_to_sink_with_policy(
            &partition_262144_probe,
            ImageFormat::WebP,
            &partition_262144_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(66_878),
            &mut partition_bit_sink_262144,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 262144-bit partition sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_bit_sink_error_262144,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 66_878,
                observed: 66_879,
            }
        ));
        assert_eq!(partition_bit_sink_262144, vec![0xD7]);

        let coefficient_bit_policy_262144 =
            image_slash_star::EncodePolicy::new().with_max_work_units(148_071);
        let coefficient_bit_error_262144 = match image_slash_star::encode_with_policy(
            &partition_262144_probe,
            ImageFormat::WebP,
            &partition_262144_options,
            &coefficient_bit_policy_262144,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 262144-bit coefficient budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_error_262144,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 148_071,
                observed: 148_072,
            }
        ));
        let mut coefficient_bit_sink_262144 = vec![0xD8];
        let coefficient_bit_sink_error_262144 = match image_slash_star::encode_to_sink_with_policy(
            &partition_262144_probe,
            ImageFormat::WebP,
            &partition_262144_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(148_070),
            &mut coefficient_bit_sink_262144,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 262144-bit coefficient sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_sink_error_262144,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 148_070,
                observed: 148_071,
            }
        ));
        assert_eq!(coefficient_bit_sink_262144, vec![0xD8]);

        // The next coefficient-only logical interval is independently
        // enforced after each 524,288 coded bit. The compact checkerboard's
        // cumulative whole-buffer poll count at that checkpoint is 187,406;
        // its sink path is one poll earlier because the final sink delivery
        // remains outside the staged encode. Pillow has no caller token,
        // work-budget result, or caller-owned sink, so this remains Rust-only
        // evidence with no parity row or coverage-only hook.
        let coefficient_bit_policy_524288 =
            image_slash_star::EncodePolicy::new().with_max_work_units(187_405);
        let coefficient_bit_error_524288 = match image_slash_star::encode_with_policy(
            &coefficient_1048576_probe,
            ImageFormat::WebP,
            &coefficient_1048576_options,
            &coefficient_bit_policy_524288,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP compact 524288-bit coefficient budget unexpectedly completed"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_error_524288,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 187_405,
                observed: 187_406,
            }
        ));
        let mut coefficient_bit_sink_524288 = vec![0xD9];
        let coefficient_bit_sink_error_524288 = match image_slash_star::encode_to_sink_with_policy(
            &coefficient_1048576_probe,
            ImageFormat::WebP,
            &coefficient_1048576_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(187_404),
            &mut coefficient_bit_sink_524288,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP compact 524288-bit coefficient sink budget unexpectedly wrote
                     output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_sink_error_524288,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 187_404,
                observed: 187_405,
            }
        ));
        assert_eq!(coefficient_bit_sink_524288, vec![0xD9]);

        // The next coefficient-only logical interval is independently
        // enforced after each 1,048,576 coded bit. The same compact
        // checkerboard's cumulative whole-buffer and sink poll counts are
        // 318,671 and 318,670. Pillow has no caller token, work-budget result,
        // or caller-owned sink, so this remains Rust-only evidence with no
        // parity row or coverage-only hook.
        let coefficient_bit_policy_1048576 =
            image_slash_star::EncodePolicy::new().with_max_work_units(318_670);
        let coefficient_bit_error_1048576 = match image_slash_star::encode_with_policy(
            &coefficient_1048576_probe,
            ImageFormat::WebP,
            &coefficient_1048576_options,
            &coefficient_bit_policy_1048576,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 1048576-bit coefficient budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_error_1048576,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 318_670,
                observed: 318_671,
            }
        ));
        let mut coefficient_bit_sink_1048576 = vec![0xDA];
        let coefficient_bit_sink_error_1048576 = match image_slash_star::encode_to_sink_with_policy(
            &coefficient_1048576_probe,
            ImageFormat::WebP,
            &coefficient_1048576_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(318_669),
            &mut coefficient_bit_sink_1048576,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 1048576-bit coefficient sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_sink_error_1048576,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 318_669,
                observed: 318_670,
            }
        ));
        assert_eq!(coefficient_bit_sink_1048576, vec![0xDA]);

        // First-partition output now charges an interior checkpoint after each
        // 1,024 emitted boolean-coder bytes. The deep patterned probe reaches
        // this boundary after the first-partition bit intervals.
        // Pillow has no caller token, work-budget result, or caller-owned
        // sink, so this remains Rust-only evidence with no parity row or
        // coverage-only hook.
        let output_policy = image_slash_star::EncodePolicy::new().with_max_work_units(1_115);
        let output_error = match image_slash_star::encode_with_policy(
            &deep_partition_probe,
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
                maximum: 1_115,
                observed: 1_116,
            }
        ));
        let output_sink_policy = image_slash_star::EncodePolicy::new().with_max_work_units(1_114);
        let mut output_sink = vec![0xB5];
        let output_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &deep_partition_probe,
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
                maximum: 1_114,
                observed: 1_115,
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

        // The basic VP8 probe above already proves ordinary/ample byte
        // identity. This 512x512 fixture is retained only for the real
        // analysis and macroblock checkpoint boundaries below. Its aligned
        // planes also exercise the direct-clone path that avoids needless
        // edge-replication work when no padding is required.
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

        // Segment assignment re-walks the analyzed macroblocks after the
        // analysis poll. It now charges its own 1,024-macroblock interval;
        // Pillow has no caller token or work-budget result, so this remains
        // Rust-only evidence with no parity row, fixture-manifest entry,
        // diagnostic origin, new test function, or coverage-only hook.
        let segment_assignment_bounded =
            image_slash_star::EncodePolicy::new().with_max_work_units(328);
        let segment_assignment_error = match image_slash_star::encode_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &segment_assignment_bounded,
        ) {
            Ok(_) => {
                return Err("bounded WebP segment-assignment budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            segment_assignment_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 328,
                observed: 329,
            }
        ));
        let mut segment_assignment_sink = vec![0xA7];
        let segment_assignment_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &segment_assignment_bounded,
            &mut segment_assignment_sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP segment-assignment sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            segment_assignment_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 328,
                observed: 329,
            }
        ));
        assert_eq!(segment_assignment_sink, vec![0xA7]);

        // The selection-only witness uses 128x128 (exactly 64 macroblocks),
        // so it reaches the new 64-macroblock batch without repeating the
        // larger analysis/segment-assignment probe. Each macroblock contains
        // sixteen 4x4 luma blocks, roughly 1,024 luma blocks per batch. The
        // preceding conversion work charges 16 units, so selection is
        // observed as 17. This remains Rust-only work-control evidence with
        // no parity row or coverage-only hook.
        let selection_image =
            DecodedImage::new(128, 128, vec![128; 128 * 128 * 3], ColorType::Rgb8);
        let selection_bounded = image_slash_star::EncodePolicy::new().with_max_work_units(16);
        let selection_error = match image_slash_star::encode_with_policy(
            &selection_image,
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
                maximum: 16,
                observed: 17,
            }
        ));
        let mut selection_sink = vec![0xAC];
        let selection_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &selection_image,
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
                maximum: 16,
                observed: 17,
            }
        ));
        assert_eq!(selection_sink, vec![0xAC]);

        // This committed 17x19 lossy WebP fixture is small enough to avoid
        // the outer 64-macroblock boundary. Its padded first macroblock still
        // reaches the first squared-error pixel in the initial distortion-only
        // intra4 candidate after the segment-clustering boundary above,
        // proving a later selection boundary without a generated parity row or
        // coverage-only hook. Pillow has no caller token, typed work-budget
        // result, or sink-rollback contract, so this remains Rust-only
        // feature-gate evidence.
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let selection_fixture_data =
            fs::read(root.join("tests/fixtures/input/images/webp/lossy_checker_17x19_q1_m0.webp"))?;
        let selection_fixture = image_slash_star::decode(&selection_fixture_data)?.content;
        let selection_fixture_expected =
            image_slash_star::encode(&selection_fixture, ImageFormat::WebP, &analysis_options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &selection_fixture,
                ImageFormat::WebP,
                &analysis_options,
                &unlimited,
            )?,
            selection_fixture_expected,
            "an ample fixture-derived selection budget preserves byte identity"
        );

        // Analysis histogram construction now charges after each 64 completed
        // 4x4 blocks. This fixture reaches the first histogram boundary at
        // the ninth admitted checkpoint, before segment clustering. Pillow
        // has no caller token, typed work-budget result, or sink-rollback
        // contract, so this is Rust-only feature-gate evidence with no new
        // parity row, fixture-manifest row, or coverage-only hook.
        let analysis_histogram_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(8);
        let analysis_histogram_error = match image_slash_star::encode_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &analysis_options,
            &analysis_histogram_policy,
        ) {
            Ok(_) => {
                return Err(
                    "fixture-derived analysis histogram budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            analysis_histogram_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 8,
                observed: 9,
            }
        ));
        let mut analysis_histogram_sink = vec![0xB6];
        let analysis_histogram_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &analysis_options,
            &analysis_histogram_policy,
            &mut analysis_histogram_sink,
        ) {
            Ok(_) => {
                return Err(
                    "fixture-derived analysis histogram sink budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            analysis_histogram_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 8,
                observed: 9,
            }
        ));
        assert_eq!(analysis_histogram_sink, vec![0xB6]);

        // Segment clustering scans the bounded alpha domain in 64-value
        // chunks. The first chunk is the tenth admitted checkpoint for this
        // committed fixture, after the histogram boundary and before any
        // intra4 candidate work begins. Pillow has no caller token, typed
        // work-budget result, or sink-rollback contract, so this is Rust-only
        // feature-gate evidence with no new parity row, fixture-manifest row,
        // diagnostic origin, or coverage-only hook.
        let segment_cluster_policy = image_slash_star::EncodePolicy::new().with_max_work_units(9);
        let segment_cluster_error = match image_slash_star::encode_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &analysis_options,
            &segment_cluster_policy,
        ) {
            Ok(_) => {
                return Err("fixture-derived segment-cluster budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            segment_cluster_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 9,
                observed: 10,
            }
        ));
        let mut segment_cluster_sink = vec![0xB5];
        let segment_cluster_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &analysis_options,
            &segment_cluster_policy,
            &mut segment_cluster_sink,
        ) {
            Ok(_) => {
                return Err(
                    "fixture-derived segment-cluster sink budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            segment_cluster_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 9,
                observed: 10,
            }
        ));
        assert_eq!(segment_cluster_sink, vec![0xB5]);

        let selection_fixture_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(12);
        let selection_fixture_error = match image_slash_star::encode_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &analysis_options,
            &selection_fixture_policy,
        ) {
            Ok(_) => return Err("fixture-derived selection budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            selection_fixture_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 12,
                observed: 13,
            }
        ));
        let mut selection_fixture_sink = vec![0xAE];
        let selection_fixture_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &analysis_options,
            &selection_fixture_policy,
            &mut selection_fixture_sink,
        ) {
            Ok(_) => {
                return Err(
                    "fixture-derived selection sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            selection_fixture_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 12,
                observed: 13,
            }
        ));
        assert_eq!(selection_fixture_sink, vec![0xAE]);

        // Method 0 deliberately spends its early budget in distortion-only
        // preselection. Reuse the same fixture with method 2 so the existing
        // witness reaches the full non-trellis intra4 candidate path and its
        // transform/quantization interiors. Pillow has no caller token, typed
        // work-budget result, or sink-rollback contract, so this remains
        // Rust-only feature-gate evidence without a new fixture, parity row,
        // or coverage-only hook.
        let mut interior_options = analysis_options.clone();
        if let EncodeOptions::WebP(options) = &mut interior_options {
            options.method = Some(2);
        }
        let interior_expected =
            image_slash_star::encode(&selection_fixture, ImageFormat::WebP, &interior_options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &selection_fixture,
                ImageFormat::WebP,
                &interior_options,
                &unlimited,
            )?,
            interior_expected,
            "an ample fixture-derived interior budget preserves byte identity"
        );

        // The first forward-transform row is the seventh admitted checkpoint:
        // five preparation/selection-boundary polls precede the first
        // candidate prediction poll. The transform itself then charges each
        // four-row and four-column subpass.
        let transform_policy = image_slash_star::EncodePolicy::new().with_max_work_units(6);
        let transform_error = match image_slash_star::encode_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &interior_options,
            &transform_policy,
        ) {
            Ok(_) => {
                return Err("fixture-derived transform budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            transform_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 6,
                observed: 7,
            }
        ));
        let mut transform_sink = vec![0xAD];
        let transform_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &interior_options,
            &transform_policy,
            &mut transform_sink,
        ) {
            Ok(_) => {
                return Err(
                    "fixture-derived transform sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            transform_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 6,
                observed: 7,
            }
        ));
        assert_eq!(transform_sink, vec![0xAD]);

        // After the eight forward-transform subpasses, the first non-trellis
        // quantization coefficient is the fifteenth admitted checkpoint.
        let quantization_policy = image_slash_star::EncodePolicy::new().with_max_work_units(14);
        let quantization_error = match image_slash_star::encode_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &interior_options,
            &quantization_policy,
        ) {
            Ok(_) => {
                return Err("fixture-derived quantization budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            quantization_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 14,
                observed: 15,
            }
        ));
        let mut quantization_sink = vec![0xAF];
        let quantization_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &interior_options,
            &quantization_policy,
            &mut quantization_sink,
        ) {
            Ok(_) => {
                return Err(
                    "fixture-derived quantization sink budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            quantization_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 14,
                observed: 15,
            }
        ));
        assert_eq!(quantization_sink, vec![0xAF]);

        // The inverse transform begins after all sixteen non-trellis
        // coefficient checkpoints. Its first column pass is therefore the
        // thirty-first admitted checkpoint.
        let inverse_transform_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(30);
        let inverse_transform_error = match image_slash_star::encode_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &interior_options,
            &inverse_transform_policy,
        ) {
            Ok(_) => {
                return Err(
                    "fixture-derived inverse-transform budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            inverse_transform_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 30,
                observed: 31,
            }
        ));
        let mut inverse_transform_sink = vec![0xB0];
        let inverse_transform_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &interior_options,
            &inverse_transform_policy,
            &mut inverse_transform_sink,
        ) {
            Ok(_) => {
                return Err(
                    "fixture-derived inverse-transform sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            inverse_transform_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 30,
                observed: 31,
            }
        ));
        assert_eq!(inverse_transform_sink, vec![0xB0]);

        // The first reconstructed-block squared-error pixel follows the
        // inverse-transform subpasses. Its per-pixel checkpoint is a
        // Rust-only work-control boundary: Pillow has no caller token,
        // typed work-budget result, or sink/rollback contract.
        let squared_error_policy = image_slash_star::EncodePolicy::new().with_max_work_units(38);
        let squared_error = match image_slash_star::encode_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &interior_options,
            &squared_error_policy,
        ) {
            Ok(_) => {
                return Err("fixture-derived squared-error budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            squared_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 38,
                observed: 39,
            }
        ));
        let mut squared_error_sink = vec![0xB2];
        let squared_error_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &interior_options,
            &squared_error_policy,
            &mut squared_error_sink,
        ) {
            Ok(_) => {
                return Err(
                    "fixture-derived squared-error sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            squared_error_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 38,
                observed: 39,
            }
        ));
        assert_eq!(squared_error_sink, vec![0xB2]);

        // The first row of the first weighted spectral-distortion transform
        // follows the sixteen squared-error pixels and the preserved outer
        // stage poll. This is the next Rust-only interior boundary.
        let spectral_policy = image_slash_star::EncodePolicy::new().with_max_work_units(55);
        let spectral_error = match image_slash_star::encode_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &interior_options,
            &spectral_policy,
        ) {
            Ok(_) => {
                return Err(
                    "fixture-derived spectral-distortion budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            spectral_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 55,
                observed: 56,
            }
        ));
        let mut spectral_sink = vec![0xB3];
        let spectral_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &interior_options,
            &spectral_policy,
            &mut spectral_sink,
        ) {
            Ok(_) => {
                return Err(
                    "fixture-derived spectral-distortion sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            spectral_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 55,
                observed: 56,
            }
        ));
        assert_eq!(spectral_sink, vec![0xB3]);

        // The first residual-cost coefficient follows both weighted transforms
        // and the preserved spectral-stage poll. Its coefficient-granular
        // boundary remains Rust-only because Pillow exposes no equivalent
        // caller-controlled work result or sink contract.
        let residual_cost_policy = image_slash_star::EncodePolicy::new().with_max_work_units(72);
        let residual_cost_error = match image_slash_star::encode_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &interior_options,
            &residual_cost_policy,
        ) {
            Ok(_) => {
                return Err("fixture-derived residual-cost budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            residual_cost_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 72,
                observed: 73,
            }
        ));
        let mut residual_cost_sink = vec![0xB4];
        let residual_cost_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &interior_options,
            &residual_cost_policy,
            &mut residual_cost_sink,
        ) {
            Ok(_) => {
                return Err(
                    "fixture-derived residual-cost sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            residual_cost_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 72,
                observed: 73,
            }
        ));
        assert_eq!(residual_cost_sink, vec![0xB4]);

        let mut trellis_options = interior_options.clone();
        if let EncodeOptions::WebP(options) = &mut trellis_options {
            options.method = Some(6);
        }
        let trellis_expected =
            image_slash_star::encode(&selection_fixture, ImageFormat::WebP, &trellis_options)?;
        assert_eq!(
            image_slash_star::encode_with_policy(
                &selection_fixture,
                ImageFormat::WebP,
                &trellis_options,
                &unlimited,
            )?,
            trellis_expected,
            "an ample fixture-derived trellis budget preserves byte identity"
        );

        // Method 6 performs a second intra4 selection with trellis
        // quantization. The first trellis block starts after the preceding
        // method-6 preparation and non-trellis selection work has admitted
        // 23,442 checkpoints; its first coefficient candidate is the next
        // controlled boundary. This is Rust-only caller-budget evidence:
        // Pillow has no caller token, typed work-budget result, sink, or
        // rollback contract, so it adds no parity row or coverage-only hook.
        let trellis_policy = image_slash_star::EncodePolicy::new().with_max_work_units(23_442);
        let trellis_error = match image_slash_star::encode_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &trellis_options,
            &trellis_policy,
        ) {
            Ok(_) => return Err("fixture-derived trellis budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            trellis_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 23_442,
                observed: 23_443,
            }
        ));
        let mut trellis_sink = vec![0xB1];
        let trellis_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &selection_fixture,
            ImageFormat::WebP,
            &trellis_options,
            &trellis_policy,
            &mut trellis_sink,
        ) {
            Ok(_) => {
                return Err("fixture-derived trellis sink budget unexpectedly wrote output".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            trellis_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 23_442,
                observed: 23_443,
            }
        ));
        assert_eq!(trellis_sink, vec![0xB1]);

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

        // The first-partition segment-probability prepass scans every selected
        // macroblock before it writes the fixed probability table. It now
        // charges after each 1,024 macroblocks. Pillow has no caller token or
        // work-budget result, so this remains Rust-only evidence with no
        // parity row, fixture-manifest entry, diagnostic origin, new test
        // function, or coverage-only hook.
        let partition_prepass_bounded =
            image_slash_star::EncodePolicy::new().with_max_work_units(332);
        let partition_prepass_error = match image_slash_star::encode_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &partition_prepass_bounded,
        ) {
            Ok(_) => {
                return Err("bounded WebP partition-prepass budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_prepass_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 332,
                observed: 333,
            }
        ));
        let mut partition_prepass_sink = vec![0xAE];
        let partition_prepass_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &partition_prepass_bounded,
            &mut partition_prepass_sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP partition-prepass sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            partition_prepass_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 332,
                observed: 333,
            }
        ));
        assert_eq!(partition_prepass_sink, vec![0xAE]);

        // VP8 filter-edge adjustment scans the selected macroblocks before
        // the segment-probability prepass. Method 3 selects this real
        // interior path, which now charges after each 1,024 macroblocks.
        // Pillow has no caller token or work-budget result, so this remains
        // Rust-only evidence with no parity row, fixture-manifest entry,
        // diagnostic origin, new test function, or coverage-only hook.
        let mut filter_edge_options = analysis_options.clone();
        if let EncodeOptions::WebP(options) = &mut filter_edge_options {
            options.method = Some(3);
        }
        let filter_edge_bounded = image_slash_star::EncodePolicy::new().with_max_work_units(719);
        let filter_edge_error = match image_slash_star::encode_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &filter_edge_options,
            &filter_edge_bounded,
        ) {
            Ok(_) => return Err("bounded WebP filter-edge budget unexpectedly completed".into()),
            Err(error) => error,
        };
        assert!(matches!(
            filter_edge_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 719,
                observed: 720,
            }
        ));
        let mut filter_edge_sink = vec![0xD3];
        let filter_edge_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &filter_edge_options,
            &filter_edge_bounded,
            &mut filter_edge_sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP filter-edge sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            filter_edge_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 719,
                observed: 720,
            }
        ));
        assert_eq!(filter_edge_sink, vec![0xD3]);

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
        // completed macroblocks. With the finer coefficient-bit checkpoint
        // below, this boundary is observed after the earlier residual work.
        // Pillow has no caller token or work-budget
        // result, so this remains Rust-only evidence with no parity row or
        // coverage-only hook.
        let coefficient_bounded = image_slash_star::EncodePolicy::new().with_max_work_units(626);
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
                maximum: 626,
                observed: 627,
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
                maximum: 626,
                observed: 627,
            }
        ));
        assert_eq!(coefficient_sink, vec![0xAE]);

        // Coefficient-token signaling is finer than block emission. On this
        // constant 512x512 probe, the 4,000-token charge lands after the
        // 62nd 64-block checkpoint, so it is observed as 554 after the
        // finer coefficient-bit polls. This remains
        // Rust-only work-control evidence with no parity row or coverage-only
        // hook.
        let coefficient_token_bounded =
            image_slash_star::EncodePolicy::new().with_max_work_units(553);
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
                maximum: 553,
                observed: 554,
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
                maximum: 553,
                observed: 554,
            }
        ));
        assert_eq!(coefficient_token_sink, vec![0xB2]);

        // Coefficient boolean coding now charges a checkpoint after each 8
        // coded bits. The compact patterned probe reaches the first new
        // coefficient interval without repeating the 512x512 analysis probe.
        // Pillow has no caller token or work-budget result, so this remains
        // Rust-only work-control evidence with no parity row or coverage-only
        // hook.
        let coefficient_bit_policy_8 =
            image_slash_star::EncodePolicy::new().with_max_work_units(568);
        let coefficient_bit_error_8 = match image_slash_star::encode_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_bit_policy_8,
        ) {
            Ok(_) => {
                return Err("bounded WebP 8-bit coefficient budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_error_8,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 568,
                observed: 569,
            }
        ));
        let mut coefficient_bit_sink_8 = vec![0xC7];
        let coefficient_bit_sink_error_8 = match image_slash_star::encode_to_sink_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(567),
            &mut coefficient_bit_sink_8,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 8-bit coefficient sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_sink_error_8,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 567,
                observed: 568,
            }
        ));
        assert_eq!(coefficient_bit_sink_8, vec![0xC7]);

        // The 16-bit coefficient boundary remains independently enforced
        // after the new 8-bit poll.
        let coefficient_bit_policy_16 =
            image_slash_star::EncodePolicy::new().with_max_work_units(570);
        let coefficient_bit_error_16 = match image_slash_star::encode_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_bit_policy_16,
        ) {
            Ok(_) => {
                return Err("bounded WebP 16-bit coefficient budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_error_16,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 570,
                observed: 571,
            }
        ));
        let mut coefficient_bit_sink_16 = vec![0xC6];
        let coefficient_bit_sink_error_16 = match image_slash_star::encode_to_sink_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(569),
            &mut coefficient_bit_sink_16,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 16-bit coefficient sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_sink_error_16,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 569,
                observed: 570,
            }
        ));
        assert_eq!(coefficient_bit_sink_16, vec![0xC6]);

        // Coefficient boolean coding now charges a finer logical checkpoint
        // after each 32 coded bits. Pillow has no caller token or work-budget
        // result, so this remains Rust-only work-control evidence with no
        // parity row or coverage-only hook.
        let coefficient_bit_policy_32 =
            image_slash_star::EncodePolicy::new().with_max_work_units(574);
        let coefficient_bit_error_32 = match image_slash_star::encode_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_bit_policy_32,
        ) {
            Ok(_) => {
                return Err("bounded WebP 32-bit coefficient budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_error_32,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 574,
                observed: 575,
            }
        ));
        let mut coefficient_bit_sink_32 = vec![0xC5];
        let coefficient_bit_sink_error_32 = match image_slash_star::encode_to_sink_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(573),
            &mut coefficient_bit_sink_32,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 32-bit coefficient sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_sink_error_32,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 573,
                observed: 574,
            }
        ));
        assert_eq!(coefficient_bit_sink_32, vec![0xC5]);

        // Coefficient boolean coding now charges a finer logical checkpoint
        // after each 64 coded bits. Pillow has no caller token or work-budget
        // result, so this remains Rust-only work-control evidence with no
        // parity row or coverage-only hook.
        let coefficient_bit_policy_64 =
            image_slash_star::EncodePolicy::new().with_max_work_units(582);
        let coefficient_bit_error_64 = match image_slash_star::encode_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_bit_policy_64,
        ) {
            Ok(_) => {
                return Err("bounded WebP 64-bit coefficient budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_error_64,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 582,
                observed: 583,
            }
        ));
        let mut coefficient_bit_sink_64 = vec![0xC4];
        let coefficient_bit_sink_error_64 = match image_slash_star::encode_to_sink_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(581),
            &mut coefficient_bit_sink_64,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 64-bit coefficient sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_sink_error_64,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 581,
                observed: 582,
            }
        ));
        assert_eq!(coefficient_bit_sink_64, vec![0xC4]);

        // The 128-bit logical coefficient checkpoint remains independently
        // enforced after the finer 64-bit boundary. The compact patterned
        // probe reaches the later logical interval after the earlier polls.
        let coefficient_bit_policy_128 =
            image_slash_star::EncodePolicy::new().with_max_work_units(598);
        let coefficient_bit_error_128 = match image_slash_star::encode_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_bit_policy_128,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 128-bit coefficient budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_error_128,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 598,
                observed: 599,
            }
        ));
        let mut coefficient_bit_sink_128 = vec![0xC3];
        let coefficient_bit_sink_error_128 = match image_slash_star::encode_to_sink_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(597),
            &mut coefficient_bit_sink_128,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 128-bit coefficient sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_sink_error_128,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 597,
                observed: 598,
            }
        ));
        assert_eq!(coefficient_bit_sink_128, vec![0xC3]);

        // The 256-bit logical coefficient checkpoint remains independently
        // enforced after the finer 128-bit boundary. The compact patterned
        // probe reaches the later logical interval after the earlier polls.
        let coefficient_bit_policy_256 =
            image_slash_star::EncodePolicy::new().with_max_work_units(629);
        let coefficient_bit_error_256 = match image_slash_star::encode_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_bit_policy_256,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 256-bit coefficient budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_error_256,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 629,
                observed: 630,
            }
        ));
        let mut coefficient_bit_sink_256 = vec![0xC2];
        let coefficient_bit_sink_error_256 = match image_slash_star::encode_to_sink_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(628),
            &mut coefficient_bit_sink_256,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 256-bit coefficient sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_sink_error_256,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 628,
                observed: 629,
            }
        ));
        assert_eq!(coefficient_bit_sink_256, vec![0xC2]);

        // The 512-bit logical coefficient checkpoint remains independently
        // enforced after the finer 256-bit boundary. The compact patterned
        // probe reaches the later logical interval without repeating the
        // larger analysis fixture.
        let coefficient_bit_policy_512 =
            image_slash_star::EncodePolicy::new().with_max_work_units(694);
        let coefficient_bit_error_512 = match image_slash_star::encode_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_bit_policy_512,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 512-bit coefficient budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_error_512,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 694,
                observed: 695,
            }
        ));
        let mut coefficient_bit_sink_512 = vec![0xC1];
        let coefficient_bit_sink_error_512 = match image_slash_star::encode_to_sink_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(693),
            &mut coefficient_bit_sink_512,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 512-bit coefficient sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_sink_error_512,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 693,
                observed: 694,
            }
        ));
        assert_eq!(coefficient_bit_sink_512, vec![0xC1]);

        // The next logical coefficient interval is independently enforced
        // after each 1,024 coded bits. Pillow has no caller token or
        // work-budget result, so this remains Rust-only evidence with no
        // parity row or coverage-only hook.
        let coefficient_bit_policy_1024 =
            image_slash_star::EncodePolicy::new().with_max_work_units(773);
        let coefficient_bit_error_1024 = match image_slash_star::encode_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_bit_policy_1024,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 1024-bit coefficient budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_error_1024,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 773,
                observed: 774,
            }
        ));
        let mut coefficient_bit_sink_1024 = vec![0xC0];
        let coefficient_bit_sink_error_1024 = match image_slash_star::encode_to_sink_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(772),
            &mut coefficient_bit_sink_1024,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 1024-bit coefficient sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_sink_error_1024,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 772,
                observed: 773,
            }
        ));
        assert_eq!(coefficient_bit_sink_1024, vec![0xC0]);

        // The next logical coefficient interval is independently enforced
        // after each 2,048 coded bits. Pillow has no caller token or
        // work-budget result, so this remains Rust-only evidence with no
        // parity row or coverage-only hook.
        let coefficient_bit_policy_2048 =
            image_slash_star::EncodePolicy::new().with_max_work_units(1_124);
        let coefficient_bit_error_2048 = match image_slash_star::encode_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_bit_policy_2048,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 2048-bit coefficient budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_error_2048,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 1_124,
                observed: 1_125,
            }
        ));
        let mut coefficient_bit_sink_2048 = vec![0xBF];
        let coefficient_bit_sink_error_2048 = match image_slash_star::encode_to_sink_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(1_123),
            &mut coefficient_bit_sink_2048,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 2048-bit coefficient sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_sink_error_2048,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 1_123,
                observed: 1_124,
            }
        ));
        assert_eq!(coefficient_bit_sink_2048, vec![0xBF]);

        // The next logical coefficient interval is independently enforced
        // after each 4,096 coded bits. Pillow has no caller token or
        // work-budget result, so this remains Rust-only evidence with no
        // parity row or coverage-only hook.
        let coefficient_bit_policy_4096 =
            image_slash_star::EncodePolicy::new().with_max_work_units(1_593);
        let coefficient_bit_error_4096 = match image_slash_star::encode_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_bit_policy_4096,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 4096-bit coefficient budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_error_4096,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 1_593,
                observed: 1_594,
            }
        ));
        let mut coefficient_bit_sink_4096 = vec![0xBE];
        let coefficient_bit_sink_error_4096 = match image_slash_star::encode_to_sink_with_policy(
            &wide_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(1_592),
            &mut coefficient_bit_sink_4096,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 4096-bit coefficient sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_sink_error_4096,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 1_592,
                observed: 1_593,
            }
        ));
        assert_eq!(coefficient_bit_sink_4096, vec![0xBE]);

        // The next logical coefficient interval is independently enforced
        // after each 8,192 coded bits. This is Rust-only work-control
        // evidence: Pillow has no caller token or equivalent budget result.
        let coefficient_bit_policy_8192 =
            image_slash_star::EncodePolicy::new().with_max_work_units(4_343);
        let coefficient_bit_error_8192 = match image_slash_star::encode_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_bit_policy_8192,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 8192-bit coefficient budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_error_8192,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 4_343,
                observed: 4_344,
            }
        ));
        let mut coefficient_bit_sink_8192 = vec![0xBD];
        let coefficient_bit_sink_error_8192 = match image_slash_star::encode_to_sink_with_policy(
            &analysis_image,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(4_342),
            &mut coefficient_bit_sink_8192,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP 8192-bit coefficient sink budget unexpectedly wrote output"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_bit_sink_error_8192,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 4_342,
                observed: 4_343,
            }
        ));
        assert_eq!(coefficient_bit_sink_8192, vec![0xBD]);

        // Coefficient-partition output charges an interior checkpoint after
        // each 1,024 emitted boolean-coder bytes. The deep patterned probe
        // reaches this boundary after the residual bit intervals. Pillow has
        // no caller token, work-budget result, or caller-owned sink, so this
        // remains Rust-only evidence with no parity row or coverage-only hook.
        let coefficient_output_policy =
            image_slash_star::EncodePolicy::new().with_max_work_units(2_184);
        let coefficient_output_error = match image_slash_star::encode_with_policy(
            &deep_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &coefficient_output_policy,
        ) {
            Ok(_) => {
                return Err("bounded WebP coefficient output budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_output_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 2_184,
                observed: 2_185,
            }
        ));
        let mut coefficient_output_sink = vec![0xC0];
        let coefficient_output_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &deep_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(2_183),
            &mut coefficient_output_sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded WebP coefficient output sink budget unexpectedly wrote output".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            coefficient_output_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::WebP),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 2_183,
                observed: 2_184,
            }
        ));
        assert_eq!(coefficient_output_sink, vec![0xC0]);

        // The coarser coefficient boolean checkpoint remains independently
        // enforced after each 16,384 coded bits. The same deep patterned
        // probe reaches this later interval after the finer logical polls.
        let coefficient_bit_bounded =
            image_slash_star::EncodePolicy::new().with_max_work_units(2_377);
        let coefficient_bit_error = match image_slash_star::encode_with_policy(
            &deep_partition_probe,
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
                maximum: 2_377,
                observed: 2_378,
            }
        ));
        let mut coefficient_bit_sink = vec![0xBF];
        let coefficient_bit_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &deep_partition_probe,
            ImageFormat::WebP,
            &analysis_options,
            &image_slash_star::EncodePolicy::new().with_max_work_units(2_376),
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
                maximum: 2_376,
                observed: 2_377,
            }
        ));
        assert_eq!(coefficient_bit_sink, vec![0xBF]);

        // The 64-block coefficient checkpoint remains in place after the
        // finer bit checkpoints. On this 512x512 probe, the 1,024th block
        // charge is observed as 468 after the earlier residual checkpoints.
        // This is
        // Rust-only work-control evidence
        // with no parity row or coverage-only hook.
        let coefficient_macroblock_bounded =
            image_slash_star::EncodePolicy::new().with_max_work_units(467);
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
                maximum: 467,
                observed: 468,
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
                maximum: 467,
                observed: 468,
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
        // Sequence work-control is a Rust-only result/sink contract. A tiny
        // two-frame caller-built sequence exercises the same SequenceEncode
        // admission and cancellation boundaries without paying to decode a
        // multi-frame fixture whose pixels are not observed here.
        let sequence_image = DecodedImage::new(1, 1, vec![0], ColorType::L8);
        let mut sequence = DecodedSequence::from_image(sequence_image);
        sequence.frames.push(sequence.frames[0].clone());
        sequence.kind = SequenceKind::TimedAnimation;
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
        let mut pixels = Vec::with_capacity(32 * 32);
        for index in 0..32 * 32 {
            pixels.push(u8::try_from(index % 256)?);
        }
        let image = DecodedImage::new(32, 32, pixels, ColorType::L8);
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
        let image = DecodedImage::new(1_024, 1, vec![128; 1_024 * 3], ColorType::Rgb8);
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
        let mut rgba_pixels = Vec::with_capacity(1_024 * 4);
        for _ in 0..1_024 {
            rgba_pixels.extend_from_slice(&[128, 64, 32, 255]);
        }
        let image = DecodedImage::new(1_024, 1, rgba_pixels, ColorType::Rgba8);
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
        let mut varied_rgba_pixels = Vec::with_capacity(1_024 * 4);
        for red in 0..8u8 {
            for green in 0..4u8 {
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
        let varied_image = DecodedImage::new(1_024, 1, varied_rgba_pixels, ColorType::Rgba8);
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
        let mut transparent_pixels = Vec::with_capacity(1_024 * 4);
        for index in 0..1_024u32 {
            transparent_pixels.extend_from_slice(&[
                u8::try_from(index & 0xff)?,
                u8::try_from((index >> 8) & 0xff)?,
                u8::try_from((index >> 16) & 0xff)?,
                0,
            ]);
        }
        let transparent_image = DecodedImage::new(1_024, 1, transparent_pixels, ColorType::Rgba8);
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
        let mut high_color_pixels = Vec::with_capacity(1_024 * 3);
        for index in 0..1_024u32 {
            high_color_pixels.extend_from_slice(&[
                u8::try_from(index & 0xff)?,
                u8::try_from((index >> 8) & 0xff)?,
                u8::try_from((index >> 16) & 0xff)?,
            ]);
        }
        let image = DecodedImage::new(1_024, 1, high_color_pixels, ColorType::Rgb8);
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

        // The high-color nearest-palette path now checkpoints its stable
        // candidate ordering and bounded candidate scan. Pillow has no caller
        // token or work-budget result, so this remains Rust-only work-control
        // evidence and adds no parity row, fixture, diagnostic origin, or
        // coverage-only hook.
        let nearest_policy = image_slash_star::EncodePolicy::new().with_max_work_units(2_048);
        let nearest_error = match image_slash_star::encode_with_policy(
            &image,
            ImageFormat::Gif,
            &options,
            &nearest_policy,
        ) {
            Ok(_) => {
                return Err("bounded GIF nearest-palette budget unexpectedly completed".into());
            }
            Err(error) => error,
        };
        assert!(matches!(
            nearest_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 2_048,
                observed: 2_049,
            }
        ));

        let nearest_sink_policy = image_slash_star::EncodePolicy::new().with_max_work_units(2_047);
        let mut nearest_sink = vec![0xC3];
        let nearest_sink_error = match image_slash_star::encode_to_sink_with_policy(
            &image,
            ImageFormat::Gif,
            &options,
            &nearest_sink_policy,
            &mut nearest_sink,
        ) {
            Ok(_) => {
                return Err(
                    "bounded GIF nearest-palette sink budget unexpectedly completed".into(),
                );
            }
            Err(error) => error,
        };
        assert!(matches!(
            nearest_sink_error,
            ImageError::LimitExceeded {
                format: Some(ImageFormat::Gif),
                operation: image_slash_star::CodecOperation::StillEncode,
                resource: image_slash_star::ResourceLimit::EncodeWorkUnits,
                maximum: 2_047,
                observed: 2_048,
            }
        ));
        assert_eq!(nearest_sink, vec![0xC3]);
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

    let decode_failure_bytes =
        fs::read(root.join("tests/fixtures/input/images/png/zlib_bad_adler.png"))?;
    let decode_failure_source = image_slash_star::EncodedImage::new(decode_failure_bytes)?;
    assert_eq!(
        decode_failure_source.decode_state(),
        image_slash_star::EncodedImageDecodeState::NotAttempted
    );
    assert!(decode_failure_source.decode().is_err());
    assert_eq!(
        decode_failure_source.decode_state(),
        image_slash_star::EncodedImageDecodeState::Failed
    );
    assert!(!decode_failure_source.is_decoded());
    assert!(decode_failure_source.decode().is_err());
    assert_eq!(
        decode_failure_source.sequence_decode_state(),
        image_slash_star::EncodedImageDecodeState::NotAttempted
    );
    assert!(decode_failure_source.decode_sequence().is_err());
    assert_eq!(
        decode_failure_source.sequence_decode_state(),
        image_slash_star::EncodedImageDecodeState::Failed
    );
    assert!(decode_failure_source.decode_sequence().is_err());

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
            "tests/fixtures/input/images/tiff/miniswhite_8bit.tiff",
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
            "tests/fixtures/input/images/avif/portable_probe_gray_128.avif",
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
