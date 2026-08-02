//! Unit tests for the pure target-tool resolution used by the AVIF build script.

#[path = "../build_tool.rs"]
mod build_tool;

use bytemuck as _;
use image_slash_star as _;

#[test]
fn target_tool_names_match_cargo_target_environment_conventions() {
    assert_eq!(
        build_tool::target_tool_env_names("CC", "aarch64-apple-darwin"),
        [
            "CC_aarch64_apple_darwin".to_owned(),
            "TARGET_CC".to_owned(),
            "CC".to_owned()
        ]
    );
    assert_eq!(
        build_tool::target_tool_env_names("AR", "wasm32-wasip1"),
        [
            "AR_wasm32_wasip1".to_owned(),
            "TARGET_AR".to_owned(),
            "AR".to_owned()
        ]
    );
}

#[test]
fn target_tool_resolution_prefers_specific_then_target_then_host() {
    let specific = build_tool::target_tool_from_lookup(
        "CC",
        "aarch64-apple-darwin",
        "cc",
        |name| match name {
            "CC_aarch64_apple_darwin" => Some("specific-cc".to_owned()),
            "TARGET_CC" => Some("target-cc".to_owned()),
            "CC" => Some("host-cc".to_owned()),
            _ => None,
        },
    );
    assert_eq!(specific, "specific-cc");

    let target =
        build_tool::target_tool_from_lookup(
            "AR",
            "aarch64-apple-darwin",
            "ar",
            |name| match name {
                "TARGET_AR" => Some("target-ar".to_owned()),
                "AR" => Some("host-ar".to_owned()),
                _ => None,
            },
        );
    assert_eq!(target, "target-ar");

    let fallback = build_tool::target_tool_from_lookup("CC", "unknown-target", "cc", |_| None);
    assert_eq!(fallback, "cc");
}
