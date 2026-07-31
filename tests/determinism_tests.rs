//! Cross-target determinism for exact encoded bytes and decoded pixels.
//!
//! Every case computes a SHA-256 over encoder output or decoded pixels and
//! compares it with the committed golden hashes in
//! `tests/fixtures/determinism.json`. The suite runs natively and on
//! `wasm32-wasip1` in the feature-matrix command, so a matching hash proves
//! byte-identical output across those targets for the same toolchain source.
//!
//! Set `DETERMINISM_PRINT=1` to regenerate the golden file from the native
//! host (the assertion is skipped in that mode).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[path = "support/sha256.rs"]
mod sha256;

use image_slash_star::{ColorType, DecodedImage, EncodeOptions, ImageFormat, ImageMode};

use bytemuck as _;

fn checkerboard(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(
        (width as usize)
            .wrapping_mul(height as usize)
            .wrapping_mul(3),
    );
    for y in 0..height {
        for x in 0..width {
            let value = if x.wrapping_add(y).is_multiple_of(2) {
                255
            } else {
                0
            };
            pixels.extend_from_slice(&[value, value, value]);
        }
    }
    pixels
}

fn indexed_pixels(width: u32, height: u32) -> Vec<u8> {
    let count = (width as usize).wrapping_mul(height as usize);
    (0..count)
        .map(|index| u8::try_from(index % 4).unwrap_or_else(|error| panic!("case failed: {error}")))
        .collect()
}

fn luma_pixels(width: u32, height: u32) -> Vec<u8> {
    let count = (width as usize).wrapping_mul(height as usize);
    (0..count)
        .map(|index| if index.is_multiple_of(2) { 255 } else { 0 })
        .collect()
}

fn encode_case(name: &str) -> (String, Vec<u8>) {
    let width = 16;
    let height = 16;
    let rgb = checkerboard(width, height);
    match name {
        "png_rgb" => (
            name.to_owned(),
            image_slash_star::encode(
                &DecodedImage::new(width, height, rgb, ColorType::Rgb8),
                ImageFormat::Png,
                &EncodeOptions::for_format(ImageFormat::Png),
            )
            .unwrap_or_else(|error| panic!("case failed: {error}")),
        ),
        "png_rgba" => {
            let mut rgba = Vec::new();
            for pixel in rgb.chunks_exact(3) {
                rgba.extend_from_slice(pixel);
                rgba.push(255);
            }
            (
                name.to_owned(),
                image_slash_star::encode(
                    &DecodedImage::new(width, height, rgba, ColorType::Rgba8),
                    ImageFormat::Png,
                    &EncodeOptions::for_format(ImageFormat::Png),
                )
                .unwrap_or_else(|error| panic!("case failed: {error}")),
            )
        }
        "png_l8" => (
            name.to_owned(),
            image_slash_star::encode(
                &DecodedImage::new(width, height, luma_pixels(width, height), ColorType::L8),
                ImageFormat::Png,
                &EncodeOptions::for_format(ImageFormat::Png),
            )
            .unwrap_or_else(|error| panic!("case failed: {error}")),
        ),
        "png_la8" => {
            let mut la = Vec::new();
            for value in luma_pixels(width, height) {
                la.push(value);
                la.push(255);
            }
            (
                name.to_owned(),
                image_slash_star::encode(
                    &DecodedImage::new(width, height, la, ColorType::La8),
                    ImageFormat::Png,
                    &EncodeOptions::for_format(ImageFormat::Png),
                )
                .unwrap_or_else(|error| panic!("case failed: {error}")),
            )
        }
        "png_p8" => {
            let mut image = DecodedImage::with_mode(
                width,
                height,
                indexed_pixels(width, height),
                ImageMode::P8,
            );
            image = image.with_palette(
                image_slash_star::ImagePalette::new(
                    vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
                    Vec::new(),
                )
                .unwrap_or_else(|error| panic!("case failed: {error}")),
            );
            (
                name.to_owned(),
                image_slash_star::encode(
                    &image,
                    ImageFormat::Png,
                    &EncodeOptions::for_format(ImageFormat::Png),
                )
                .unwrap_or_else(|error| panic!("case failed: {error}")),
            )
        }
        "gif_rgb" => (
            name.to_owned(),
            image_slash_star::encode(
                &DecodedImage::new(width, height, rgb, ColorType::Rgb8),
                ImageFormat::Gif,
                &EncodeOptions::for_format(ImageFormat::Gif),
            )
            .unwrap_or_else(|error| panic!("case failed: {error}")),
        ),
        "webp_rgb" => (
            name.to_owned(),
            image_slash_star::encode(
                &DecodedImage::new(width, height, rgb, ColorType::Rgb8),
                ImageFormat::WebP,
                &EncodeOptions::for_format(ImageFormat::WebP),
            )
            .unwrap_or_else(|error| panic!("case failed: {error}")),
        ),
        "tiff_rgb" => (
            name.to_owned(),
            image_slash_star::encode(
                &DecodedImage::new(width, height, rgb, ColorType::Rgb8),
                ImageFormat::Tiff,
                &EncodeOptions::for_format(ImageFormat::Tiff),
            )
            .unwrap_or_else(|error| panic!("case failed: {error}")),
        ),
        "jpeg_rgb" => (
            name.to_owned(),
            image_slash_star::encode(
                &DecodedImage::new(width, height, rgb, ColorType::Rgb8),
                ImageFormat::Jpeg,
                &EncodeOptions::for_format(ImageFormat::Jpeg),
            )
            .unwrap_or_else(|error| panic!("case failed: {error}")),
        ),
        "bmp_rgb" => (
            name.to_owned(),
            image_slash_star::encode(
                &DecodedImage::new(width, height, rgb, ColorType::Rgb8),
                ImageFormat::Bmp,
                &EncodeOptions::for_format(ImageFormat::Bmp),
            )
            .unwrap_or_else(|error| panic!("case failed: {error}")),
        ),
        "ico_rgba" => {
            let mut rgba = Vec::new();
            for pixel in rgb.chunks_exact(3) {
                rgba.extend_from_slice(pixel);
                rgba.push(255);
            }
            (
                name.to_owned(),
                image_slash_star::encode(
                    &DecodedImage::new(width, height, rgba, ColorType::Rgba8),
                    ImageFormat::Ico,
                    &EncodeOptions::for_format(ImageFormat::Ico),
                )
                .unwrap_or_else(|error| panic!("case failed: {error}")),
            )
        }
        other => panic!("unknown encode case {other}"),
    }
}

fn decode_case(name: &str) -> (String, Vec<u8>) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = match name {
        "decode_png_rgba" => "tests/fixtures/input/images/png/alpha_checker.png",
        "decode_webp_rgb" => "tests/fixtures/input/images/webp/16x16.webp",
        "decode_tiff_la" => "tests/fixtures/input/images/tiff/gray_alpha.tiff",
        "decode_gif_p8" => "tests/fixtures/input/images/gif/1x1.gif",
        other => panic!("unknown decode case {other}"),
    };
    let bytes = fs::read(root.join(path)).unwrap_or_else(|error| panic!("case failed: {error}"));
    let decoded =
        image_slash_star::decode(&bytes).unwrap_or_else(|error| panic!("case failed: {error}"));
    (name.to_owned(), decoded.content.pixels)
}

fn run_cases() -> BTreeMap<String, String> {
    let mut cases = BTreeMap::new();
    for name in [
        "png_rgb",
        "png_rgba",
        "png_l8",
        "png_la8",
        "png_p8",
        "gif_rgb",
        "webp_rgb",
        "tiff_rgb",
        "jpeg_rgb",
        "bmp_rgb",
        "ico_rgba",
        "decode_png_rgba",
        "decode_webp_rgb",
        "decode_tiff_la",
        "decode_gif_p8",
    ] {
        let (name, bytes) = if name.starts_with("decode_") {
            decode_case(name)
        } else {
            encode_case(name)
        };
        cases.insert(name, sha256::digest_hex(&bytes));
    }
    cases
}

#[test]
fn deterministic_outputs_match_the_committed_golden_hashes()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cases = run_cases();
    if std::env::var_os("DETERMINISM_PRINT").is_some() {
        for (name, digest) in &cases {
            println!("{name} {digest}");
        }
        return Ok(());
    }
    let text = fs::read_to_string(root.join("tests/fixtures/determinism.json"))?;
    let golden = parse_golden(&text)?;
    assert_eq!(
        golden, cases,
        "deterministic outputs must match golden hashes"
    );
    Ok(())
}

fn parse_golden(text: &str) -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    let value: support_json::Value = support_json::from_str(text)?;
    let object = value
        .as_object()
        .ok_or_else(|| "golden file must be a JSON object".to_string())?;
    let cases = object
        .get("cases")
        .and_then(support_json::Value::as_object)
        .ok_or_else(|| "golden file must contain a cases object".to_string())?;
    let mut parsed = BTreeMap::new();
    for (key, value) in cases {
        parsed.insert(
            key.clone(),
            value
                .as_str()
                .ok_or_else(|| format!("digest for {key} must be a string"))?
                .to_owned(),
        );
    }
    Ok(parsed)
}

#[path = "support/json.rs"]
#[allow(dead_code)]
mod support_json;
