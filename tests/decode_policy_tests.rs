//! Manifest-driven caller-controlled decode-policy boundaries.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use bytemuck as _;
use image_slash_star as img;

#[path = "support/sha256.rs"]
mod sha256;
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

struct SequenceManifest {
    format_version: u32,
    assertion_origin: String,
    source_case_id: String,
    asset_path: String,
    encoded_bytes: u64,
    asset_sha256: String,
    frame_count: u32,
    decoded_path: String,
    decoded_bytes: u64,
    decoded_sha256: String,
    unknown_asset_path: String,
    unknown_encoded_bytes: u64,
    expected_format: String,
    expected_mode: String,
    expected_size: [u32; 2],
    cases: Vec<SequenceCase>,
}

struct SequenceCase {
    id: String,
    operation: String,
    resource: String,
    maximum: u64,
    status: String,
    also_max_pixels: u64,
    also_max_primary_decoded_bytes: u64,
    also_max_encoded_bytes: u64,
    also_max_frame_decoded_bytes: u64,
    also_max_sequence_decoded_bytes: u64,
    expected_resource: String,
    expected_maximum: u64,
    observed: u64,
    expected_format: String,
}

impl FromJson for SequenceManifest {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            format_version: object.take("format_version")?,
            assertion_origin: object.take("assertion_origin")?,
            source_case_id: object.take("source_case_id")?,
            asset_path: object.take("asset_path")?,
            encoded_bytes: object.take("encoded_bytes")?,
            asset_sha256: object.take("asset_sha256")?,
            frame_count: object.take("frame_count")?,
            decoded_path: object.take("decoded_path")?,
            decoded_bytes: object.take("decoded_bytes")?,
            decoded_sha256: object.take("decoded_sha256")?,
            unknown_asset_path: object.take("unknown_asset_path")?,
            unknown_encoded_bytes: object.take("unknown_encoded_bytes")?,
            expected_format: object.take("expected_format")?,
            expected_mode: object.take("expected_mode")?,
            expected_size: object.take("expected_size")?,
            cases: object.take("cases")?,
        })
    }
}

impl FromJson for SequenceCase {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            id: object.take("id")?,
            operation: object.take("operation")?,
            resource: object.take("resource")?,
            maximum: object.take("maximum")?,
            status: object.take("status")?,
            also_max_pixels: object.take_or_default("also_max_pixels")?,
            also_max_primary_decoded_bytes: object
                .take_or_default("also_max_primary_decoded_bytes")?,
            also_max_encoded_bytes: object.take_or_default("also_max_encoded_bytes")?,
            also_max_frame_decoded_bytes: object.take_or_default("also_max_frame_decoded_bytes")?,
            also_max_sequence_decoded_bytes: object
                .take_or_default("also_max_sequence_decoded_bytes")?,
            expected_resource: object.take_or_default("expected_resource")?,
            expected_maximum: object.take_or_default("expected_maximum")?,
            observed: object.take_or_default("observed")?,
            expected_format: object.take_or_default("expected_format")?,
        })
    }
}

struct TrailingManifest {
    format_version: u32,
    assertion_origin: String,
    pillow_version: String,
    pillow_outcome: String,
    trailing_payloads: Vec<TrailingPayload>,
    formats: Vec<TrailingFormat>,
}

struct TrailingPayload {
    name: String,
    payload: Vec<u64>,
}

struct TrailingFormat {
    name: String,
    feature: String,
    asset_path: String,
    expected_format: String,
    consumed_bytes: Option<u64>,
    consumed_origin: String,
    pillow_outcome: String,
}

struct MetadataManifest {
    format_version: u32,
    assertion_origin: String,
    formats: Vec<MetadataFormat>,
}

struct MetadataFormat {
    name: String,
    feature: String,
    asset_path: String,
    asset_sha256: String,
    expected_format: String,
    metadata_bytes: u64,
    metadata_origin: String,
}

impl FromJson for TrailingManifest {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            format_version: object.take("format_version")?,
            assertion_origin: object.take("assertion_origin")?,
            pillow_version: object.take("pillow_version")?,
            pillow_outcome: object.take("pillow_outcome")?,
            trailing_payloads: object.take("trailing_payloads")?,
            formats: object.take("formats")?,
        })
    }
}

impl FromJson for TrailingPayload {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            name: object.take("name")?,
            payload: object.take("payload")?,
        })
    }
}

impl FromJson for TrailingFormat {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            name: object.take("name")?,
            feature: object.take("feature")?,
            asset_path: object.take("asset_path")?,
            expected_format: object.take("expected_format")?,
            consumed_bytes: object.take("consumed_bytes")?,
            consumed_origin: object.take("consumed_origin")?,
            pillow_outcome: object.take("pillow_outcome")?,
        })
    }
}

impl FromJson for MetadataManifest {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            format_version: object.take("format_version")?,
            assertion_origin: object.take("assertion_origin")?,
            formats: object.take("formats")?,
        })
    }
}

impl FromJson for MetadataFormat {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            name: object.take("name")?,
            feature: object.take("feature")?,
            asset_path: object.take("asset_path")?,
            asset_sha256: object.take("asset_sha256")?,
            expected_format: object.take("expected_format")?,
            metadata_bytes: object.take("metadata_bytes")?,
            metadata_origin: object.take("metadata_origin")?,
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

fn assert_gif_info(info: &img::ImageInfo) {
    assert_eq!(info.format, img::ImageFormat::Gif);
    assert_eq!([info.width, info.height], [128, 128]);
    assert_eq!(info.mode, img::ImageMode::P8);
    assert_eq!(info.bit_depth, 8);
    assert!(info.is_animated);
    assert_eq!(info.frame_count, Some(3));
    assert!(info.has_palette_table());
}

fn assert_gif_image(decoded: &img::Decoded<img::DecodedImage>, expected_pixels: &[u8]) {
    assert_eq!(decoded.format, img::ImageFormat::Gif);
    assert_eq!([decoded.content.width, decoded.content.height], [128, 128]);
    assert_eq!(decoded.content.mode, img::ImageMode::P8);
    assert_eq!(decoded.content.pixels, expected_pixels);
}

fn assert_gif_sequence(decoded: &img::Decoded<img::DecodedSequence>) {
    use img::{AnimationBackground, FrameBlend, FrameDisposal, FramePixelLayout};

    assert_eq!(decoded.format, img::ImageFormat::Gif);
    let sequence = &decoded.content;
    assert_eq!([sequence.width, sequence.height], [128, 128]);
    assert_eq!(sequence.loop_count, Some(0));
    assert_eq!(
        sequence.background,
        Some(AnimationBackground::PaletteIndex(0))
    );
    assert_eq!(sequence.frames.len(), 3);
    let expected_durations: &[(u64, u64)] = &[(2, 100), (8, 100), (16, 100)];
    for (frame, &(numerator, denominator)) in sequence.frames.iter().zip(expected_durations) {
        assert_eq!(frame.pixel_layout, FramePixelLayout::SourceRectangle);
        assert_eq!(
            frame.source.rect,
            img::FrameRect {
                left: 0,
                top: 0,
                width: 128,
                height: 128,
            }
        );
        assert_eq!(frame.source.duration.numerator, numerator);
        assert_eq!(frame.source.duration.denominator, denominator);
        assert_eq!(frame.source.disposal, FrameDisposal::Unspecified);
        assert_eq!(frame.source.blend, FrameBlend::Unspecified);
        assert!(!frame.source.interlaced);
        assert!(!frame.source.is_default_image);
    }
    assert_eq!(sequence.first(), Some(&sequence.frames[0]));
    assert_eq!(sequence.first_image(), Some(&sequence.frames[0].image));
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
    assert_eq!(ids.len(), 87);
    Ok(())
}

#[test]
fn sequence_frame_limit_manifest_matches_the_public_contract()
-> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(feature = "gif") {
        return Ok(());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: SequenceManifest = json::from_str(&fs::read_to_string(
        root.join("tests/fixtures/sequence_policy_manifest.json"),
    )?)?;
    assert_eq!(manifest.format_version, 1);
    assert_eq!(manifest.assertion_origin, "defensive_model");
    assert_eq!(manifest.source_case_id, "animated_3frame");
    assert_eq!(manifest.expected_format, "gif");
    assert_eq!(manifest.expected_mode, "P8");
    assert_eq!(manifest.expected_size, [128, 128]);

    let bytes = fs::read(root.join(&manifest.asset_path))?;
    let unknown_bytes = fs::read(root.join(&manifest.unknown_asset_path))?;
    let expected_pixels = fs::read(root.join(&manifest.decoded_path))?;
    assert_eq!(bytes.len() as u64, manifest.encoded_bytes);
    assert_eq!(unknown_bytes.len() as u64, manifest.unknown_encoded_bytes);
    assert_eq!(expected_pixels.len() as u64, manifest.decoded_bytes);
    assert_eq!(sha256::digest_hex(&bytes), manifest.asset_sha256);
    assert_eq!(
        sha256::digest_hex(&expected_pixels),
        manifest.decoded_sha256
    );

    let mut ids = HashSet::new();
    for case in manifest.cases {
        assert!(ids.insert(case.id.clone()), "duplicate case {}", case.id);
        assert!(matches!(case.status.as_str(), "ok" | "error" | "unknown"));

        let mut policy = match case.resource.as_str() {
            "frames" => img::DecodePolicy::default().with_max_frames(u32::try_from(case.maximum)?),
            "frame_decoded_bytes" => {
                img::DecodePolicy::default().with_max_frame_decoded_bytes(case.maximum)
            }
            "sequence_decoded_bytes" => {
                img::DecodePolicy::default().with_max_sequence_decoded_bytes(case.maximum)
            }
            resource => panic!("unknown resource `{resource}`"),
        };
        if case.also_max_pixels != 0 {
            policy = policy.with_max_pixels(case.also_max_pixels);
        }
        if case.also_max_primary_decoded_bytes != 0 {
            policy = policy.with_max_primary_decoded_bytes(case.also_max_primary_decoded_bytes);
        }
        if case.also_max_encoded_bytes != 0 {
            policy = policy.with_max_encoded_bytes(case.also_max_encoded_bytes);
        }
        if case.also_max_frame_decoded_bytes != 0 {
            policy = policy.with_max_frame_decoded_bytes(case.also_max_frame_decoded_bytes);
        }
        if case.also_max_sequence_decoded_bytes != 0 {
            policy = policy.with_max_sequence_decoded_bytes(case.also_max_sequence_decoded_bytes);
        }
        match case.resource.as_str() {
            "frames" => assert_eq!(
                policy.limits().max_frames(),
                Some(u32::try_from(case.maximum)?)
            ),
            "frame_decoded_bytes" => assert_eq!(
                policy.limits().max_frame_decoded_bytes(),
                Some(case.maximum)
            ),
            "sequence_decoded_bytes" => assert_eq!(
                policy.limits().max_sequence_decoded_bytes(),
                Some(case.maximum)
            ),
            resource => panic!("unknown resource `{resource}`"),
        }
        assert_eq!(img::DecodeLimits::new().max_frames(), None);
        assert_eq!(img::DecodeLimits::new().max_frame_decoded_bytes(), None);
        assert_eq!(img::DecodeLimits::new().max_sequence_decoded_bytes(), None);

        let default_observed = match case.operation.as_str() {
            "decode" | "source_decode" => 1,
            _ => u64::from(manifest.frame_count),
        };
        let (expected_resource, expected_maximum, observed) = if case.expected_resource.is_empty() {
            let observed = if case.observed != 0 {
                case.observed
            } else {
                default_observed
            };
            (case.resource.as_str(), case.maximum, observed)
        } else {
            (
                case.expected_resource.as_str(),
                case.expected_maximum,
                case.observed,
            )
        };
        let expected_resource = match expected_resource {
            "frames" => img::ResourceLimit::Frames,
            "encoded_bytes" => img::ResourceLimit::EncodedBytes,
            "pixels" => img::ResourceLimit::Pixels,
            "primary_decoded_bytes" => img::ResourceLimit::PrimaryDecodedBytes,
            "frame_decoded_bytes" => img::ResourceLimit::FrameDecodedBytes,
            "sequence_decoded_bytes" => img::ResourceLimit::SequenceDecodedBytes,
            resource => panic!("unknown resource `{resource}`"),
        };
        let expected_operation = match case.operation.as_str() {
            "inspect" | "source_new" => img::CodecOperation::Inspection,
            "decode" | "source_decode" | "decode_unknown" => img::CodecOperation::StillDecode,
            "decode_sequence" => img::CodecOperation::SequenceDecode,
            operation => panic!("unknown operation `{operation}`"),
        };
        let expected_format = match case.expected_format.as_str() {
            "" => Some(img::ImageFormat::Gif),
            "none" => None,
            format => panic!("unknown expected format `{format}`"),
        };

        match case.operation.as_str() {
            "inspect" => match img::inspect_with_policy(&bytes, &policy) {
                Ok(info) if case.status == "ok" => assert_gif_info(&info),
                Err(error) if case.status == "error" => assert_limit_error(
                    error,
                    expected_operation,
                    expected_resource,
                    expected_maximum,
                    observed,
                    expected_format,
                ),
                Ok(_) => panic!("{} unexpectedly succeeded", case.id),
                Err(error) => panic!("{} unexpectedly failed: {error}", case.id),
            },
            "decode" => match img::decode_with_policy(&bytes, &policy) {
                Ok(decoded) if case.status == "ok" => {
                    assert_gif_image(&decoded, &expected_pixels);
                }
                Err(error) if case.status == "error" => assert_limit_error(
                    error,
                    expected_operation,
                    expected_resource,
                    expected_maximum,
                    observed,
                    expected_format,
                ),
                Ok(_) => panic!("{} unexpectedly succeeded", case.id),
                Err(error) => panic!("{} unexpectedly failed: {error}", case.id),
            },
            "decode_unknown" => match img::decode_with_policy(&unknown_bytes, &policy) {
                Err(error) if case.status == "unknown" => {
                    assert_eq!(error.kind(), img::ImageErrorKind::UnknownFormat);
                    assert_eq!(error.format(), None);
                    assert_eq!(error.message(), None);
                }
                Ok(_) => panic!("{} unexpectedly succeeded", case.id),
                Err(error) => panic!("{} unexpectedly failed: {error}", case.id),
            },
            "decode_sequence" => match img::decode_sequence_with_policy(&bytes, &policy) {
                Ok(decoded) if case.status == "ok" => assert_gif_sequence(&decoded),
                Err(error) if case.status == "error" => assert_limit_error(
                    error,
                    expected_operation,
                    expected_resource,
                    expected_maximum,
                    observed,
                    expected_format,
                ),
                Ok(_) => panic!("{} unexpectedly succeeded", case.id),
                Err(error) => panic!("{} unexpectedly failed: {error}", case.id),
            },
            "source_new" => match img::EncodedImage::new_with_policy(bytes.clone(), &policy) {
                Ok(source) if case.status == "ok" => {
                    assert_gif_info(source.info());
                    assert!(!source.is_decoded());
                }
                Err(error) if case.status == "error" => assert_limit_error(
                    error,
                    expected_operation,
                    expected_resource,
                    expected_maximum,
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
                        assert_gif_image(decoded, &expected_pixels);
                        assert!(source.is_decoded());
                    }
                    Err(error) if case.status == "error" => {
                        assert_limit_error(
                            error,
                            expected_operation,
                            expected_resource,
                            expected_maximum,
                            observed,
                            expected_format,
                        );
                        assert!(!source.is_decoded());
                        assert_gif_image(source.decode()?, &expected_pixels);
                        assert!(source.is_decoded());
                    }
                    Ok(_) => panic!("{} unexpectedly succeeded", case.id),
                    Err(error) => panic!("{} unexpectedly failed: {error}", case.id),
                }
            }
            operation => panic!("unknown operation `{operation}`"),
        }
    }
    assert_eq!(ids.len(), 35);
    Ok(())
}

#[test]
fn sequence_byte_limits_reject_before_every_sequence_codec_materializes_later_frames()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cases: &[(&str, bool, &str)] = &[
        (
            "gif",
            cfg!(feature = "gif"),
            "tests/fixtures/input/images/gif/animated_3frame.gif",
        ),
        (
            "gif_no_palette",
            cfg!(feature = "gif"),
            "tests/fixtures/input/images/gif/animated_no_palette.gif",
        ),
        (
            "png",
            cfg!(feature = "png"),
            "tests/fixtures/input/images/png/apng_l_over.png",
        ),
        (
            "webp",
            cfg!(feature = "webp"),
            "tests/fixtures/input/images/webp/animated_sequence_rgba_keyframes.webp",
        ),
        (
            "tiff",
            cfg!(feature = "tiff"),
            "tests/fixtures/input/images/tiff/multipage.tiff",
        ),
        (
            "avif",
            cfg!(feature = "avif"),
            "tests/fixtures/input/images/avif/animated_error_resilient.avif",
        ),
    ];

    for &(name, enabled, path) in cases {
        if !enabled {
            continue;
        }
        let format = match name {
            "gif" => img::ImageFormat::Gif,
            "gif_no_palette" => img::ImageFormat::Gif,
            "png" => img::ImageFormat::Png,
            "webp" => img::ImageFormat::WebP,
            "tiff" => img::ImageFormat::Tiff,
            "avif" => img::ImageFormat::Avif,
            other => panic!("unknown codec `{other}`"),
        };
        let bytes = fs::read(root.join(path))?;
        let unlimited = img::decode_sequence(&bytes)?;
        let frames = &unlimited.content.frames;
        assert!(frames.len() >= 2, "{name} needs at least two frames");

        let mut total = 0u64;
        let mut max_later = 0u64;
        let mut primary = 0u64;
        for (index, frame) in frames.iter().enumerate() {
            let bytes = u64::try_from(frame.image.pixels.len())?;
            total = total.saturating_add(bytes);
            if index == 0 {
                primary = bytes;
            } else {
                max_later = max_later.max(bytes);
            }
        }
        assert!(max_later != 0, "{name} later frame has no retained bytes");
        for frame in frames.iter().skip(1) {
            assert_eq!(
                u64::try_from(frame.image.pixels.len())?,
                max_later,
                "{name} later frames must be uniform for this boundary fixture"
            );
        }

        // Every later frame may not exceed the per-frame maximum.
        let error = match img::decode_sequence_with_policy(
            &bytes,
            &img::DecodePolicy::new().with_max_frame_decoded_bytes(max_later.saturating_sub(1)),
        ) {
            Err(error) => error,
            Ok(_) => panic!("{name} accepted a later frame above the byte maximum"),
        };
        assert_limit_error(
            error,
            img::CodecOperation::SequenceDecode,
            img::ResourceLimit::FrameDecodedBytes,
            max_later.saturating_sub(1),
            max_later,
            Some(format),
        );
        let error = match img::decode_sequence_with_policy(
            &bytes,
            &img::DecodePolicy::new().with_max_frame_decoded_bytes(0),
        ) {
            Err(error) => error,
            Ok(_) => panic!("{name} accepted a later frame with a zero byte maximum"),
        };
        assert_limit_error(
            error,
            img::CodecOperation::SequenceDecode,
            img::ResourceLimit::FrameDecodedBytes,
            0,
            max_later,
            Some(format),
        );
        let at_limit = img::decode_sequence_with_policy(
            &bytes,
            &img::DecodePolicy::new().with_max_frame_decoded_bytes(max_later),
        )?;
        assert_eq!(at_limit.content.frames, *frames, "{name}");

        // The cumulative retained sequence may not exceed the total maximum.
        let error = match img::decode_sequence_with_policy(
            &bytes,
            &img::DecodePolicy::new().with_max_sequence_decoded_bytes(total.saturating_sub(1)),
        ) {
            Err(error) => error,
            Ok(_) => panic!("{name} accepted a sequence above the cumulative maximum"),
        };
        assert_limit_error(
            error,
            img::CodecOperation::SequenceDecode,
            img::ResourceLimit::SequenceDecodedBytes,
            total.saturating_sub(1),
            total,
            Some(format),
        );
        let error = match img::decode_sequence_with_policy(
            &bytes,
            &img::DecodePolicy::new().with_max_sequence_decoded_bytes(0),
        ) {
            Err(error) => error,
            Ok(_) => panic!("{name} accepted a sequence with a zero cumulative maximum"),
        };
        assert_limit_error(
            error,
            img::CodecOperation::SequenceDecode,
            img::ResourceLimit::SequenceDecodedBytes,
            0,
            primary,
            Some(format),
        );
        let at_total = img::decode_sequence_with_policy(
            &bytes,
            &img::DecodePolicy::new().with_max_sequence_decoded_bytes(total),
        )?;
        assert_eq!(at_total.content.frames, *frames, "{name}");
    }
    Ok(())
}

#[test]
fn trailing_input_policy_manifest_matches_the_public_contract()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: TrailingManifest = json::from_str(&fs::read_to_string(
        root.join("tests/fixtures/trailing_input_manifest.json"),
    )?)?;
    assert_eq!(manifest.format_version, 1);
    assert_eq!(manifest.assertion_origin, "defensive_model");
    assert_eq!(manifest.pillow_version, "12.2.0");
    assert_eq!(manifest.pillow_outcome, "ok");
    for payload in &manifest.trailing_payloads {
        assert!(!payload.name.is_empty());
        assert!(!payload.payload.is_empty());
    }

    for format in manifest.formats {
        let enabled = match format.feature.as_str() {
            "jpeg" => cfg!(feature = "jpeg"),
            "png" => cfg!(feature = "png"),
            "gif" => cfg!(feature = "gif"),
            "bmp" => cfg!(feature = "bmp"),
            "tiff" => cfg!(feature = "tiff"),
            "webp" => cfg!(feature = "webp"),
            "ico" => cfg!(feature = "ico"),
            "avif" => cfg!(feature = "avif"),
            other => panic!("{}: unknown feature `{other}`", format.name),
        };
        if !enabled {
            continue;
        }
        assert_eq!(format.consumed_origin, "defensive_model");
        assert_eq!(format.pillow_outcome, "ok");
        let expected_format = match format.expected_format.as_str() {
            "jpeg" => img::ImageFormat::Jpeg,
            "png" => img::ImageFormat::Png,
            "gif" => img::ImageFormat::Gif,
            "bmp" => img::ImageFormat::Bmp,
            "tiff" => img::ImageFormat::Tiff,
            "webp" => img::ImageFormat::WebP,
            "ico" => img::ImageFormat::Ico,
            "avif" => img::ImageFormat::Avif,
            other => panic!("{}: unknown expected format `{other}`", format.name),
        };
        let expected_consumed = format.consumed_bytes.map(usize::try_from).transpose()?;

        let bytes = fs::read(root.join(&format.asset_path))?;
        let base_image = img::decode(&bytes)?;
        assert_eq!(base_image.format, expected_format, "{}", format.name);
        assert_eq!(
            base_image.consumed_bytes, expected_consumed,
            "{} still consumed",
            format.name
        );
        let base_sequence = img::decode_sequence(&bytes)?;
        assert_eq!(
            base_sequence.consumed_bytes, expected_consumed,
            "{} sequence consumed",
            format.name
        );
        let base_info = img::inspect(&bytes)?;
        assert_eq!(base_info.format, expected_format, "{}", format.name);

        for payload in &manifest.trailing_payloads {
            let mut trailing = bytes.clone();
            for byte in &payload.payload {
                trailing.push(u8::try_from(*byte)?);
            }
            let image = img::decode(&trailing)?;
            assert_eq!(
                image.content.pixels, base_image.content.pixels,
                "{}/{} still pixels",
                format.name, payload.name
            );
            assert_eq!(
                image.consumed_bytes, expected_consumed,
                "{}/{} still consumed",
                format.name, payload.name
            );
            let sequence = img::decode_sequence(&trailing)?;
            assert_eq!(
                sequence.content.frames, base_sequence.content.frames,
                "{}/{} sequence frames",
                format.name, payload.name
            );
            assert_eq!(
                sequence.consumed_bytes, expected_consumed,
                "{}/{} sequence consumed",
                format.name, payload.name
            );
            assert_eq!(
                img::inspect(&trailing)?,
                base_info,
                "{}/{} inspection",
                format.name,
                payload.name
            );
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_metadata_case(
    operation: &str,
    bytes: &[u8],
    policy: &img::DecodePolicy,
    base_image: &img::Decoded<img::DecodedImage>,
    base_sequence: &img::Decoded<img::DecodedSequence>,
    base_info: &img::ImageInfo,
    expected_format: img::ImageFormat,
    expected_operation: img::CodecOperation,
    expected_maximum: u64,
    expected_observed: u64,
    expected_status: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match operation {
        "inspect" => match img::inspect_with_policy(bytes, policy) {
            Ok(info) if expected_status == "ok" => assert_eq!(&info, base_info),
            Err(error) if expected_status == "error" => assert_limit_error(
                error,
                expected_operation,
                img::ResourceLimit::MetadataBytes,
                expected_maximum,
                expected_observed,
                Some(expected_format),
            ),
            Ok(_) => panic!("inspect unexpectedly succeeded"),
            Err(error) => panic!("inspect unexpectedly failed: {error}"),
        },
        "decode" => match img::decode_with_policy(bytes, policy) {
            Ok(decoded) if expected_status == "ok" => {
                assert_eq!(decoded.content.pixels, base_image.content.pixels);
                assert_eq!(decoded.consumed_bytes, base_image.consumed_bytes);
            }
            Err(error) if expected_status == "error" => assert_limit_error(
                error,
                expected_operation,
                img::ResourceLimit::MetadataBytes,
                expected_maximum,
                expected_observed,
                Some(expected_format),
            ),
            Ok(_) => panic!("decode unexpectedly succeeded"),
            Err(error) => panic!("decode unexpectedly failed: {error}"),
        },
        "decode_sequence" => match img::decode_sequence_with_policy(bytes, policy) {
            Ok(decoded) if expected_status == "ok" => {
                assert_eq!(decoded.content.frames, base_sequence.content.frames);
                assert_eq!(decoded.consumed_bytes, base_sequence.consumed_bytes);
            }
            Err(error) if expected_status == "error" => assert_limit_error(
                error,
                expected_operation,
                img::ResourceLimit::MetadataBytes,
                expected_maximum,
                expected_observed,
                Some(expected_format),
            ),
            Ok(_) => panic!("decode_sequence unexpectedly succeeded"),
            Err(error) => panic!("decode_sequence unexpectedly failed: {error}"),
        },
        "source_new" => match img::EncodedImage::new_with_policy(bytes.to_vec(), policy) {
            Ok(source) if expected_status == "ok" => {
                assert_eq!(source.info(), base_info);
            }
            Err(error) if expected_status == "error" => assert_limit_error(
                error,
                expected_operation,
                img::ResourceLimit::MetadataBytes,
                expected_maximum,
                expected_observed,
                Some(expected_format),
            ),
            Ok(_) => panic!("source_new unexpectedly succeeded"),
            Err(error) => panic!("source_new unexpectedly failed: {error}"),
        },
        "source_decode" => {
            let source = img::EncodedImage::new(bytes.to_vec())?;
            match source.decode_with_policy(policy) {
                Ok(decoded) if expected_status == "ok" => {
                    assert_eq!(decoded.content.pixels, base_image.content.pixels);
                }
                Err(error) if expected_status == "error" => {
                    assert_limit_error(
                        error,
                        expected_operation,
                        img::ResourceLimit::MetadataBytes,
                        expected_maximum,
                        expected_observed,
                        Some(expected_format),
                    );
                    assert!(!source.is_decoded());
                }
                Ok(_) => panic!("source_decode unexpectedly succeeded"),
                Err(error) => panic!("source_decode unexpectedly failed: {error}"),
            }
        }
        operation => panic!("unknown operation `{operation}`"),
    }
    Ok(())
}

#[test]
fn metadata_policy_manifest_matches_the_public_contract() -> Result<(), Box<dyn std::error::Error>>
{
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: MetadataManifest = json::from_str(&fs::read_to_string(
        root.join("tests/fixtures/metadata_policy_manifest.json"),
    )?)?;
    assert_eq!(manifest.format_version, 1);
    assert_eq!(manifest.assertion_origin, "defensive_model");
    assert_eq!(img::DecodeLimits::new().max_metadata_bytes(), None);

    let unknown_bytes = fs::read(root.join("tests/fixtures/input/images/png/not_a_png.png"))?;

    for format in manifest.formats {
        let enabled = match format.feature.as_str() {
            "jpeg" => cfg!(feature = "jpeg"),
            "png" => cfg!(feature = "png"),
            "gif" => cfg!(feature = "gif"),
            "bmp" => cfg!(feature = "bmp"),
            "tiff" => cfg!(feature = "tiff"),
            "webp" => cfg!(feature = "webp"),
            "ico" => cfg!(feature = "ico"),
            "avif" => cfg!(feature = "avif"),
            other => panic!("{}: unknown feature `{other}`", format.name),
        };
        if !enabled {
            continue;
        }
        assert_eq!(format.metadata_origin, "independent_measurement");
        let expected_format = match format.expected_format.as_str() {
            "jpeg" => img::ImageFormat::Jpeg,
            "png" => img::ImageFormat::Png,
            "gif" => img::ImageFormat::Gif,
            "bmp" => img::ImageFormat::Bmp,
            "tiff" => img::ImageFormat::Tiff,
            "webp" => img::ImageFormat::WebP,
            "ico" => img::ImageFormat::Ico,
            "avif" => img::ImageFormat::Avif,
            other => panic!("{}: unknown expected format `{other}`", format.name),
        };
        let bytes = fs::read(root.join(&format.asset_path))?;
        assert_eq!(sha256::digest_hex(&bytes), format.asset_sha256);

        let base_image = img::decode(&bytes)?;
        let base_sequence = img::decode_sequence(&bytes)?;
        let base_info = img::inspect(&bytes)?;
        let metadata = format.metadata_bytes;

        for (operation, expected_operation) in [
            ("inspect", img::CodecOperation::Inspection),
            ("decode", img::CodecOperation::StillDecode),
            ("decode_sequence", img::CodecOperation::SequenceDecode),
            ("source_new", img::CodecOperation::Inspection),
            ("source_decode", img::CodecOperation::StillDecode),
        ] {
            let below =
                img::DecodePolicy::new().with_max_metadata_bytes(metadata.saturating_sub(1));
            run_metadata_case(
                operation,
                &bytes,
                &below,
                &base_image,
                &base_sequence,
                &base_info,
                expected_format,
                expected_operation,
                metadata.saturating_sub(1),
                metadata,
                "error",
            )?;
            let zero = img::DecodePolicy::new().with_max_metadata_bytes(0);
            run_metadata_case(
                operation,
                &bytes,
                &zero,
                &base_image,
                &base_sequence,
                &base_info,
                expected_format,
                expected_operation,
                0,
                metadata,
                "error",
            )?;
            let at = img::DecodePolicy::new().with_max_metadata_bytes(metadata);
            run_metadata_case(
                operation,
                &bytes,
                &at,
                &base_image,
                &base_sequence,
                &base_info,
                expected_format,
                expected_operation,
                metadata,
                metadata,
                "ok",
            )?;
            let above =
                img::DecodePolicy::new().with_max_metadata_bytes(metadata.saturating_add(1));
            run_metadata_case(
                operation,
                &bytes,
                &above,
                &base_image,
                &base_sequence,
                &base_info,
                expected_format,
                expected_operation,
                metadata.saturating_add(1),
                metadata,
                "ok",
            )?;
            let near_max = img::DecodePolicy::new().with_max_metadata_bytes(u64::MAX);
            run_metadata_case(
                operation,
                &bytes,
                &near_max,
                &base_image,
                &base_sequence,
                &base_info,
                expected_format,
                expected_operation,
                u64::MAX,
                metadata,
                "ok",
            )?;
            assert_eq!(
                img::DecodePolicy::new()
                    .with_max_metadata_bytes(metadata)
                    .limits()
                    .max_metadata_bytes(),
                Some(metadata)
            );
        }

        // Precedence: encoded bytes are checked before the metadata scan.
        let encoded_first = img::DecodePolicy::new()
            .with_max_metadata_bytes(0)
            .with_max_encoded_bytes(bytes.len().saturating_sub(1) as u64);
        let error = match img::decode_with_policy(&bytes, &encoded_first) {
            Err(error) => error,
            Ok(_) => panic!("encoded-bytes precedence row unexpectedly succeeded"),
        };
        assert_limit_error(
            error,
            img::CodecOperation::StillDecode,
            img::ResourceLimit::EncodedBytes,
            bytes.len().saturating_sub(1) as u64,
            bytes.len() as u64,
            None,
        );

        // Precedence: the metadata scan runs before primary-canvas checks.
        let metadata_first = img::DecodePolicy::new()
            .with_max_metadata_bytes(0)
            .with_max_pixels(1);
        let error = match img::decode_sequence_with_policy(&bytes, &metadata_first) {
            Err(error) => error,
            Ok(_) => panic!("metadata-precedence row unexpectedly succeeded"),
        };
        assert_limit_error(
            error,
            img::CodecOperation::SequenceDecode,
            img::ResourceLimit::MetadataBytes,
            0,
            metadata,
            Some(expected_format),
        );

        // Lazy still decode also checks metadata before cache access.
        let lazy_source = img::EncodedImage::new(bytes.clone())?;
        let error = match lazy_source.decode_with_policy(&encoded_first) {
            Err(error) => error,
            Ok(_) => panic!("lazy encoded-bytes precedence row unexpectedly succeeded"),
        };
        assert_limit_error(
            error,
            img::CodecOperation::StillDecode,
            img::ResourceLimit::EncodedBytes,
            bytes.len().saturating_sub(1) as u64,
            bytes.len() as u64,
            None,
        );

        // Detection precedes the metadata scan on unknown signatures.
        let unknown_policy = img::DecodePolicy::new().with_max_metadata_bytes(0);
        match img::decode_with_policy(&unknown_bytes, &unknown_policy) {
            Err(error) => {
                assert_eq!(error.kind(), img::ImageErrorKind::UnknownFormat);
                assert_eq!(error.format(), None);
                assert_eq!(error.message(), None);
            }
            Ok(_) => panic!("unknown signature unexpectedly succeeded"),
        }
    }

    // A malformed-but-detected input makes the metadata scan itself fail and
    // propagates the codec error through the policy preflight.
    let truncated = fs::read(root.join("tests/fixtures/input/images/png/truncated.png"))?;
    let malformed_policy = img::DecodePolicy::new().with_max_metadata_bytes(0);
    match img::inspect_with_policy(&truncated, &malformed_policy) {
        Err(error) => {
            assert_eq!(error.kind(), img::ImageErrorKind::Malformed);
            assert_eq!(error.format(), Some(img::ImageFormat::Png));
            assert!(error.message().is_some_and(|message| !message.is_empty()));
        }
        Ok(_) => panic!("truncated PNG must fail the metadata scan"),
    }
    Ok(())
}
