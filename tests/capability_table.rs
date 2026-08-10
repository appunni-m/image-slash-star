//! Emit the runtime capability table for the current feature/target lane.
//!
//! `scripts/generate_capability_tables.py` executes this probe in every
//! native and WASI lane, captures the single `CAPABILITY_TABLE_JSON` line, and
//! assembles the committed `tests/fixtures/capability_tables.json` fixture.
//! Running the probe is itself a test so every lane proves the emitted table
//! is well-formed at runtime. The feature matrix can set
//! `CAPABILITY_TABLE_OUTPUT` to persist that row without launching a second
//! test process after the full lane suite.

use image_slash_star::{
    CODEC_OPERATIONS, Capability, CapabilityRestriction, CapabilityTarget,
    CapabilityUnavailableReason, CodecOperation, ImageFormat,
};

use bytemuck as _;

const FORMAT_FEATURES: [(&str, ImageFormat); 8] = [
    ("jpeg", ImageFormat::Jpeg),
    ("png", ImageFormat::Png),
    ("gif", ImageFormat::Gif),
    ("bmp", ImageFormat::Bmp),
    ("tiff", ImageFormat::Tiff),
    ("webp", ImageFormat::WebP),
    ("ico", ImageFormat::Ico),
    ("avif", ImageFormat::Avif),
];

fn feature_enabled(feature: &str) -> bool {
    match feature {
        "jpeg" => cfg!(feature = "jpeg"),
        "png" => cfg!(feature = "png"),
        "gif" => cfg!(feature = "gif"),
        "bmp" => cfg!(feature = "bmp"),
        "tiff" => cfg!(feature = "tiff"),
        "webp" => cfg!(feature = "webp"),
        "ico" => cfg!(feature = "ico"),
        "avif" => cfg!(feature = "avif"),
        other => panic!("unknown feature {other}"),
    }
}

fn enabled_features() -> Vec<&'static str> {
    FORMAT_FEATURES
        .iter()
        .map(|(feature, _)| *feature)
        .filter(|feature| feature_enabled(feature))
        .collect()
}

fn lane(features: &[&'static str]) -> &'static str {
    match features {
        [] => "none",
        [feature] => feature,
        [..] if features.len() == 3
            && features.contains(&"bmp")
            && features.contains(&"png")
            && features.contains(&"ico") =>
        {
            "ico"
        }
        [..] if features.len() == 7 && !features.contains(&"avif") => "default",
        [..] if features.len() == 8 => "all",
        _ => "mixed",
    }
}

fn capability_name(capability: Capability) -> &'static str {
    match capability {
        Capability::ManifestBounded => "manifest_bounded",
        Capability::Restricted(CapabilityRestriction::PortableAvif) => "restricted:portable_avif",
        Capability::Unavailable(CapabilityUnavailableReason::FeatureDisabled) => {
            "unavailable:feature_disabled"
        }
        Capability::Unavailable(CapabilityUnavailableReason::TargetUnavailable) => {
            "unavailable:target_unavailable"
        }
        Capability::Unavailable(CapabilityUnavailableReason::NotImplemented) => {
            "unavailable:not_implemented"
        }
        _ => "unknown",
    }
}

fn target_name(target: CapabilityTarget) -> &'static str {
    match target {
        CapabilityTarget::Native => "native",
        CapabilityTarget::Wasm32Wasi => "wasm32-wasip1",
        CapabilityTarget::Wasm32Unknown => "wasm32-unknown-unknown",
        _ => "unknown",
    }
}

fn operation_name(operation: CodecOperation) -> &'static str {
    match operation {
        CodecOperation::Detection => "detection",
        CodecOperation::Inspection => "inspection",
        CodecOperation::StillDecode => "still_decode",
        CodecOperation::StillEncode => "still_encode",
        CodecOperation::SequenceDecode => "sequence_decode",
        CodecOperation::SequenceEncode => "sequence_encode",
        _ => "unknown",
    }
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", u32::from(character)));
            }
            character => escaped.push(character),
        }
    }
    escaped
}

#[test]
fn emit_runtime_capability_table() {
    let triple = std::env::var("CAPABILITY_TRIPLE")
        .unwrap_or_else(|_| format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS));
    assert!(
        triple
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character)),
        "CAPABILITY_TRIPLE contains unsupported characters: {triple}"
    );

    let features = enabled_features();
    let features_json = features
        .iter()
        .map(|feature| format!("\"{}\"", json_escape(feature)))
        .collect::<Vec<_>>()
        .join(",");
    let mut formats = Vec::new();
    for &(feature, format) in &FORMAT_FEATURES {
        let capabilities = format.capabilities();
        assert_eq!(capabilities.format(), format);
        assert_eq!(capabilities.feature_enabled(), feature_enabled(feature));
        assert_eq!(
            capabilities.target(),
            CapabilityTarget::current(),
            "{feature}"
        );
        let operations = CODEC_OPERATIONS
            .iter()
            .map(|operation| {
                format!(
                    "\"{}\":\"{}\"",
                    operation_name(*operation),
                    capability_name(capabilities.operation(*operation))
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        formats.push(format!(
            "{{\"format\":\"{}\",\"feature_enabled\":{},\"operations\":{{{}}}}}",
            json_escape(feature),
            capabilities.feature_enabled(),
            operations
        ));
    }
    let target = capabilities_target_name();
    let lane = lane(&features);
    let row = format!(
        "CAPABILITY_TABLE_JSON {{\"triple\":\"{}\",\"target\":\"{}\",\"lane\":\"{}\",\"features\":[{}],\"formats\":[{}]}}",
        json_escape(&triple),
        target,
        lane,
        features_json,
        formats.join(",")
    );
    if let Some(path) = std::env::var_os("CAPABILITY_TABLE_OUTPUT") {
        std::fs::write(&path, format!("{row}\n"))
            .unwrap_or_else(|error| panic!("cannot write capability table: {error}"));
    }
    println!("{row}");
}

fn capabilities_target_name() -> &'static str {
    target_name(CapabilityTarget::current())
}
