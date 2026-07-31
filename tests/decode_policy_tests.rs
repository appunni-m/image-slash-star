//! Manifest-driven caller-controlled decode-policy boundaries.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use bytemuck as _;
use image_slash_star as img;

#[allow(dead_code)]
mod support;

use support::json::{self, FromJson, Object, Value};

struct Manifest {
    format_version: u32,
    assertion_origin: String,
    source_case_id: String,
    asset_path: String,
    encoded_bytes: u64,
    unknown_asset_path: String,
    unknown_encoded_bytes: u64,
    malformed_asset_path: String,
    layout_overflow_asset_path: String,
    layout_overflow_dimension_offset: u64,
    decoded_path: String,
    primary_decoded_bytes: u64,
    expected_format: String,
    expected_mode: String,
    expected_size: [u32; 2],
    cases: Vec<Case>,
}

struct Case {
    id: String,
    operation: String,
    resource: String,
    maximum: u64,
    status: String,
}

impl FromJson for Manifest {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            format_version: object.take("format_version")?,
            assertion_origin: object.take("assertion_origin")?,
            source_case_id: object.take("source_case_id")?,
            asset_path: object.take("asset_path")?,
            encoded_bytes: object.take("encoded_bytes")?,
            unknown_asset_path: object.take("unknown_asset_path")?,
            unknown_encoded_bytes: object.take("unknown_encoded_bytes")?,
            malformed_asset_path: object.take("malformed_asset_path")?,
            layout_overflow_asset_path: object.take("layout_overflow_asset_path")?,
            layout_overflow_dimension_offset: object.take("layout_overflow_dimension_offset")?,
            decoded_path: object.take("decoded_path")?,
            primary_decoded_bytes: object.take("primary_decoded_bytes")?,
            expected_format: object.take("expected_format")?,
            expected_mode: object.take("expected_mode")?,
            expected_size: object.take("expected_size")?,
            cases: object.take("cases")?,
        })
    }
}

impl FromJson for Case {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            id: object.take("id")?,
            operation: object.take("operation")?,
            resource: object.take("resource")?,
            maximum: object.take("maximum")?,
            status: object.take("status")?,
        })
    }
}

fn assert_info(info: &img::ImageInfo, expected_size: [u32; 2]) {
    assert_eq!(info.format, img::ImageFormat::Png);
    assert_eq!([info.width, info.height], expected_size);
    assert_eq!(info.mode, img::ImageMode::Rgb8);
    assert_eq!(info.bit_depth, 8);
    assert!(!info.is_animated);
    assert_eq!(info.frame_count, Some(1));
}

fn assert_image(
    decoded: &img::Decoded<img::DecodedImage>,
    expected_size: [u32; 2],
    expected_pixels: &[u8],
) {
    assert_eq!(decoded.format, img::ImageFormat::Png);
    assert_eq!(
        [decoded.content.width, decoded.content.height],
        expected_size
    );
    assert_eq!(decoded.content.mode, img::ImageMode::Rgb8);
    assert_eq!(decoded.content.pixels, expected_pixels);
}

fn assert_sequence(
    decoded: &img::Decoded<img::DecodedSequence>,
    expected_size: [u32; 2],
    expected_pixels: &[u8],
) {
    assert_eq!(decoded.format, img::ImageFormat::Png);
    assert_eq!(
        [decoded.content.width, decoded.content.height],
        expected_size
    );
    assert_eq!(decoded.content.frames.len(), 1);
    assert_eq!(decoded.content.frames[0].image.mode, img::ImageMode::Rgb8);
    assert_eq!(decoded.content.frames[0].image.pixels, expected_pixels);
    assert_eq!(
        decoded.content.first().map(|frame| &frame.image),
        decoded.content.first_image()
    );
}

fn assert_limit_error(
    error: img::ImageError,
    operation: img::CodecOperation,
    resource: img::ResourceLimit,
    maximum: u64,
    observed: u64,
    format: Option<img::ImageFormat>,
) {
    assert_eq!(error.kind(), img::ImageErrorKind::LimitExceeded);
    assert_eq!(error.format(), format);
    assert_eq!(
        error,
        img::ImageError::LimitExceeded {
            format,
            operation,
            resource,
            maximum,
            observed,
        }
    );
    assert_eq!(error.message(), None);
    assert_eq!(
        error.to_string(),
        format!(
            "{}{operation:?} exceeded {resource:?} limit: observed {observed}, maximum {maximum}",
            format.map_or(String::new(), |format| format!("{format:?} "))
        )
    );
}

fn assert_malformed(error: img::ImageError) {
    assert_eq!(error.kind(), img::ImageErrorKind::Malformed);
    assert_eq!(error.format(), Some(img::ImageFormat::Png));
    assert!(error.message().is_some_and(|message| !message.is_empty()));
}

fn assert_dimensions(error: img::ImageError, format: img::ImageFormat) {
    assert_eq!(error.kind(), img::ImageErrorKind::Dimensions);
    assert_eq!(error.format(), Some(format));
    assert!(error.message().is_some_and(|message| !message.is_empty()));
}

#[test]
fn encoded_input_limit_manifest_matches_the_public_contract()
-> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(feature = "png") {
        return Ok(());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: Manifest = json::from_str(&fs::read_to_string(
        root.join("tests/fixtures/decode_policy_manifest.json"),
    )?)?;
    assert_eq!(manifest.format_version, 1);
    assert_eq!(manifest.assertion_origin, "defensive_model");
    assert_eq!(manifest.source_case_id, "size_1x1");
    assert_eq!(manifest.expected_format, "png");
    assert_eq!(manifest.expected_mode, "Rgb8");

    let unlimited_limits = img::DecodeLimits::new();
    assert_eq!(unlimited_limits.max_encoded_bytes(), None);
    let explicit_limits = unlimited_limits.with_max_encoded_bytes(manifest.encoded_bytes);
    assert_eq!(
        explicit_limits.max_encoded_bytes(),
        Some(manifest.encoded_bytes)
    );
    let explicit_limits = explicit_limits
        .with_max_width(manifest.expected_size[0])
        .with_max_height(manifest.expected_size[1])
        .with_max_pixels(
            u64::from(manifest.expected_size[0])
                .saturating_mul(u64::from(manifest.expected_size[1])),
        )
        .with_max_primary_decoded_bytes(manifest.primary_decoded_bytes);
    assert_eq!(explicit_limits.max_width(), Some(manifest.expected_size[0]));
    assert_eq!(
        explicit_limits.max_height(),
        Some(manifest.expected_size[1])
    );
    assert_eq!(explicit_limits.max_pixels(), Some(1));
    assert_eq!(
        explicit_limits.max_primary_decoded_bytes(),
        Some(manifest.primary_decoded_bytes)
    );
    let unlimited_policy = img::DecodePolicy::new();
    assert_eq!(unlimited_policy.limits(), unlimited_limits);
    assert_eq!(
        img::DecodePolicy::with_limits(explicit_limits).limits(),
        explicit_limits
    );

    let bytes = fs::read(root.join(&manifest.asset_path))?;
    let unknown_bytes = fs::read(root.join(&manifest.unknown_asset_path))?;
    let malformed_bytes = fs::read(root.join(&manifest.malformed_asset_path))?;
    let mut layout_overflow_bytes = fs::read(root.join(&manifest.layout_overflow_asset_path))?;
    let dimension_offset = usize::try_from(manifest.layout_overflow_dimension_offset)?;
    layout_overflow_bytes[dimension_offset..dimension_offset + 8].fill(u8::MAX);
    let expected_pixels = fs::read(root.join(&manifest.decoded_path))?;
    assert_eq!(bytes.len() as u64, manifest.encoded_bytes);
    assert_eq!(unknown_bytes.len() as u64, manifest.unknown_encoded_bytes);
    assert_eq!(expected_pixels.len(), 3);
    assert_eq!(expected_pixels.len() as u64, manifest.primary_decoded_bytes);

    let mut ids = HashSet::new();
    for case in manifest.cases {
        assert!(ids.insert(case.id.clone()), "duplicate case {}", case.id);
        assert!(matches!(
            case.status.as_str(),
            "ok" | "error" | "malformed" | "dimensions"
        ));
        let (policy, expected_resource, observed, expected_format) = match case.resource.as_str() {
            "encoded_bytes" => (
                img::DecodePolicy::default().with_max_encoded_bytes(case.maximum),
                img::ResourceLimit::EncodedBytes,
                if case.operation == "decode_unknown" {
                    manifest.unknown_encoded_bytes
                } else {
                    manifest.encoded_bytes
                },
                None,
            ),
            "width" => (
                img::DecodePolicy::default().with_max_width(u32::try_from(case.maximum)?),
                img::ResourceLimit::Width,
                u64::from(manifest.expected_size[0]),
                Some(img::ImageFormat::Png),
            ),
            "height" => (
                img::DecodePolicy::default().with_max_height(u32::try_from(case.maximum)?),
                img::ResourceLimit::Height,
                u64::from(manifest.expected_size[1]),
                Some(img::ImageFormat::Png),
            ),
            "pixels" => (
                img::DecodePolicy::default().with_max_pixels(case.maximum),
                img::ResourceLimit::Pixels,
                u64::from(manifest.expected_size[0])
                    .saturating_mul(u64::from(manifest.expected_size[1])),
                Some(img::ImageFormat::Png),
            ),
            "primary_decoded_bytes" => (
                img::DecodePolicy::default().with_max_primary_decoded_bytes(case.maximum),
                img::ResourceLimit::PrimaryDecodedBytes,
                manifest.primary_decoded_bytes,
                Some(img::ImageFormat::Png),
            ),
            resource => panic!("unknown resource `{resource}`"),
        };
        let expected_operation = match case.operation.as_str() {
            "inspect" | "inspect_layout_overflow" | "source_new" => img::CodecOperation::Inspection,
            "decode" | "decode_unknown" | "decode_malformed" | "source_decode" => {
                img::CodecOperation::StillDecode
            }
            "decode_sequence" | "decode_sequence_malformed" => img::CodecOperation::SequenceDecode,
            operation => panic!("unknown operation `{operation}`"),
        };

        match case.operation.as_str() {
            "inspect" => match img::inspect_with_policy(&bytes, &policy) {
                Ok(info) if case.status == "ok" => {
                    assert_info(&info, manifest.expected_size);
                }
                Err(error) if case.status == "error" => assert_limit_error(
                    error,
                    expected_operation,
                    expected_resource,
                    case.maximum,
                    observed,
                    expected_format,
                ),
                Ok(_) => panic!("{} unexpectedly succeeded", case.id),
                Err(error) => panic!("{} unexpectedly failed: {error}", case.id),
            },
            "inspect_layout_overflow" => {
                match img::inspect_with_policy(&layout_overflow_bytes, &policy) {
                    Err(error) if case.status == "dimensions" => {
                        assert_dimensions(error, img::ImageFormat::Avif);
                    }
                    Ok(_) => panic!("{} unexpectedly succeeded", case.id),
                    Err(error) => panic!("{} unexpectedly failed: {error}", case.id),
                }
            }
            "decode" => match img::decode_with_policy(&bytes, &policy) {
                Ok(decoded) if case.status == "ok" => {
                    assert_image(&decoded, manifest.expected_size, &expected_pixels);
                }
                Err(error) if case.status == "error" => assert_limit_error(
                    error,
                    expected_operation,
                    expected_resource,
                    case.maximum,
                    observed,
                    expected_format,
                ),
                Ok(_) => panic!("{} unexpectedly succeeded", case.id),
                Err(error) => panic!("{} unexpectedly failed: {error}", case.id),
            },
            "decode_unknown" => match img::decode_with_policy(&unknown_bytes, &policy) {
                Err(error) if case.status == "error" => assert_limit_error(
                    error,
                    expected_operation,
                    expected_resource,
                    case.maximum,
                    observed,
                    expected_format,
                ),
                Ok(_) => panic!("{} unexpectedly succeeded", case.id),
                Err(error) => panic!("{} unexpectedly failed: {error}", case.id),
            },
            "decode_malformed" => match img::decode_with_policy(&malformed_bytes, &policy) {
                Err(error) if case.status == "malformed" => assert_malformed(error),
                Ok(_) => panic!("{} unexpectedly succeeded", case.id),
                Err(error) => panic!("{} unexpectedly failed: {error}", case.id),
            },
            "decode_sequence" => match img::decode_sequence_with_policy(&bytes, &policy) {
                Ok(decoded) if case.status == "ok" => {
                    assert_sequence(&decoded, manifest.expected_size, &expected_pixels);
                }
                Err(error) if case.status == "error" => assert_limit_error(
                    error,
                    expected_operation,
                    expected_resource,
                    case.maximum,
                    observed,
                    expected_format,
                ),
                Ok(_) => panic!("{} unexpectedly succeeded", case.id),
                Err(error) => panic!("{} unexpectedly failed: {error}", case.id),
            },
            "decode_sequence_malformed" => {
                match img::decode_sequence_with_policy(&malformed_bytes, &policy) {
                    Err(error) if case.status == "malformed" => assert_malformed(error),
                    Ok(_) => panic!("{} unexpectedly succeeded", case.id),
                    Err(error) => panic!("{} unexpectedly failed: {error}", case.id),
                }
            }
            "source_new" => match img::EncodedImage::new_with_policy(bytes.clone(), &policy) {
                Ok(source) if case.status == "ok" => {
                    assert_info(source.info(), manifest.expected_size);
                    assert!(!source.is_decoded());
                }
                Err(error) if case.status == "error" => assert_limit_error(
                    error,
                    expected_operation,
                    expected_resource,
                    case.maximum,
                    observed,
                    expected_format,
                ),
                Ok(_) => panic!("{} unexpectedly succeeded", case.id),
                Err(error) => panic!("{} unexpectedly failed: {error}", case.id),
            },
            "source_decode" => {
                let source = img::EncodedImage::new(bytes.clone())?;
                match source.decode_with_policy(&policy) {
                    Ok(decoded) if case.status == "ok" => {
                        assert_image(decoded, manifest.expected_size, &expected_pixels);
                        assert!(source.is_decoded());
                    }
                    Err(error) if case.status == "error" => {
                        assert_limit_error(
                            error,
                            expected_operation,
                            expected_resource,
                            case.maximum,
                            observed,
                            expected_format,
                        );
                        assert!(!source.is_decoded());
                        assert_image(source.decode()?, manifest.expected_size, &expected_pixels);
                        assert!(source.is_decoded());
                    }
                    Ok(_) => panic!("{} unexpectedly succeeded", case.id),
                    Err(error) => panic!("{} unexpectedly failed: {error}", case.id),
                }
            }
            operation => panic!("unknown operation `{operation}`"),
        }
    }
    assert_eq!(ids.len(), 79);
    Ok(())
}
