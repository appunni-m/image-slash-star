//! Reduced-feature behavior driven by the Pillow coverage manifest.

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(unused_crate_dependencies)]

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use image_slash_star::{EncodedImage, ImageError, ImageFormat};
use serde::Deserialize;

#[derive(Deserialize)]
struct CoverageMatrix {
    formats: HashMap<String, FormatRows>,
}

#[derive(Deserialize)]
struct FormatRows {
    decode: Vec<DecodeRow>,
}

#[derive(Deserialize)]
struct DecodeRow {
    status: String,
    asset: Option<String>,
    #[serde(default)]
    expect_error: bool,
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

#[test]
fn manifest_inputs_report_the_exact_disabled_codec_feature() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: CoverageMatrix = serde_json::from_slice(
        &fs::read(root.join("tests/fixtures/coverage_matrix.json"))
            .expect("coverage manifest must be readable"),
    )
    .expect("coverage manifest must be valid JSON");

    for (name, rows) in manifest.formats {
        let (format, feature, enabled) = format(&name);
        if enabled {
            continue;
        }
        let row = rows
            .decode
            .iter()
            .find(|row| row.status != "planned" && !row.expect_error && row.asset.is_some())
            .expect("every format must have a successful fixture row");
        let bytes = fs::read(
            root.join("tests/fixtures/input/images")
                .join(&name)
                .join(row.asset.as_deref().expect("selected row has an asset")),
        )
        .expect("encoded fixture must be readable");
        let expected = ImageError::FeatureDisabled { format, feature };

        assert_eq!(image_slash_star::inspect(&bytes), Err(expected.clone()));
        assert_eq!(image_slash_star::decode(&bytes), Err(expected.clone()));
        assert!(matches!(EncodedImage::new(bytes), Err(error) if error == expected));
    }
}
