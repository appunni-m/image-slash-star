//! Minimal first-use program retained in the release package.
//!
//! This is deliberately dependency-free and uses only the public PNG
//! detection, inspection, and decode APIs. `scripts/verify_package_consumer.py`
//! runs the same contract from a separate temporary consumer against the
//! packaged archive.

#![allow(
    unused_crate_dependencies,
    reason = "the smoke program exercises the published public API, not the library's internal dependency"
)]

use image_slash_star::{ImageFormat, ImageMode, ImageResult, decode, detect_format, inspect};

const ONE_BY_ONE_RGB_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x68, 0x60, 0x60, 0x00,
    0x00, 0x01, 0x84, 0x00, 0x81, 0xf9, 0xfe, 0x65, 0x88, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
    0x44, 0xae, 0x42, 0x60, 0x82,
];

fn main() -> ImageResult<()> {
    assert_eq!(detect_format(ONE_BY_ONE_RGB_PNG)?, ImageFormat::Png);

    let info = inspect(ONE_BY_ONE_RGB_PNG)?;
    assert_eq!(
        (info.width, info.height, info.mode),
        (1, 1, ImageMode::Rgb8)
    );

    let decoded = decode(ONE_BY_ONE_RGB_PNG)?;
    assert_eq!(decoded.format, ImageFormat::Png);
    assert_eq!(decoded.content.pixels, [128, 0, 0]);
    Ok(())
}
