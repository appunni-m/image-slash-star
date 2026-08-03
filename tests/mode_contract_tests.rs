//! Public decoded-mode boundaries consumed by downstream operation layers.

use bytemuck as _;
use image_slash_star::ImageMode;

#[cfg(feature = "tiff")]
use image_slash_star::{decode, inspect};

const SIXTEEN_BIT_MODES: [ImageMode; 4] = [
    ImageMode::L16,
    ImageMode::La16,
    ImageMode::Rgb16,
    ImageMode::Rgba16,
];

const NON_SIXTEEN_BIT_MODES: [ImageMode; 11] = [
    ImageMode::L1,
    ImageMode::P8,
    ImageMode::L8,
    ImageMode::La8,
    ImageMode::Rgb8,
    ImageMode::Rgba8,
    ImageMode::Cmyk8,
    ImageMode::Rgb32F,
    ImageMode::Rgba32F,
    ImageMode::F32,
    ImageMode::I32,
];

#[test]
fn image_modes_classify_sixteen_bit_samples() {
    for mode in SIXTEEN_BIT_MODES {
        assert!(mode.is_16_bit(), "{mode:?} should be sixteen-bit");
    }
    for mode in NON_SIXTEEN_BIT_MODES {
        assert!(!mode.is_16_bit(), "{mode:?} should not be sixteen-bit");
    }
}

#[cfg(feature = "tiff")]
#[test]
fn valid_sixteen_bit_tiff_remains_decodable() {
    let bytes = include_bytes!("fixtures/input/images/tiff/16bit.tiff");
    let info = inspect(bytes)
        .unwrap_or_else(|error| panic!("valid sixteen-bit TIFF must inspect: {error}"));
    assert_eq!(info.mode, ImageMode::L16);
    assert!(info.mode.is_16_bit());

    let decoded =
        decode(bytes).unwrap_or_else(|error| panic!("valid sixteen-bit TIFF must decode: {error}"));

    assert_eq!(decoded.content.mode, info.mode);
    assert!(decoded.content.mode.is_16_bit());
    assert_eq!(
        decoded.content.pixels.len(),
        decoded
            .content
            .mode
            .expected_bytes(decoded.content.width, decoded.content.height)
            .unwrap_or_else(|error| panic!("mode byte length must be computable: {error}"))
    );
    decoded
        .content
        .validate()
        .unwrap_or_else(|error| panic!("valid sixteen-bit TIFF must validate: {error}"));
}
