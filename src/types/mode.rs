//! Exact raster identities carried from encoded headers to raster clients.

#[cfg(any(
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "bmp",
    feature = "tiff",
    feature = "webp",
    feature = "ico",
    feature = "avif"
))]
use super::{ImageFormat, ImageMode, SourceAlpha, SourceByteOrder};

/// Exact identity of a decoded raster, independent of the carrier used for
/// its sample bytes.
///
/// Header inspection selects this mode without decoding image pixels. Several
/// identities can use the same number of bytes per pixel while retaining
/// different pixel semantics, such as RGB, CMYK, YCbCr, HSV, and RGBX.
/// Some internal identities describe rasters created by downstream
/// processing and are not currently emitted by a codec header inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Mode {
    /// Bilevel `1` mode. The codec transfer buffer may store these samples packed.
    One,
    /// 8-bit luminance (`L`).
    ImageLuma8,
    /// 8-bit luminance with straight alpha (`LA`).
    ImageLumaA8,
    /// 8-bit luminance with premultiplied alpha (`La`).
    ImageLaPremultiplied8,
    /// 8-bit palette indices (`P`).
    ImagePalette8,
    /// Palette indices with per-pixel alpha (`PA`).
    ImagePaletteAlpha8,
    /// 8-bit RGB.
    ImageRgb8,
    /// 8-bit RGBA with straight alpha.
    ImageRgba8,
    /// 8-bit RGBA with premultiplied alpha (`RGBa`).
    ImageRgbPremultiplied8,
    /// 8-bit RGB with an unused fourth byte (`RGBX`).
    ImageRgbx8,
    /// 8-bit CMYK.
    ImageCmyk8,
    /// 8-bit YCbCr.
    ImageYCbCr8,
    /// 8-bit CIE Lab.
    ImageLab8,
    /// 8-bit HSV.
    ImageHsv8,
    /// Signed 32-bit integer (`I`) samples.
    ImageI32,
    /// 32-bit float (`F`) samples.
    ImageF32,
    /// Native-endian unsigned 16-bit (`I;16`) samples.
    ImageI16,
    /// Little-endian unsigned 16-bit (`I;16L`) samples.
    ImageI16L,
    /// Big-endian unsigned 16-bit (`I;16B`) samples.
    ImageI16B,
    /// Native-order unsigned 16-bit (`I;16N`) samples.
    ImageI16N,
    /// Internal 16-bit luminance-alpha samples.
    ImageLumaA16,
    /// Internal 16-bit RGB samples.
    ImageRgb16,
    /// Internal 16-bit RGBA samples.
    ImageRgba16,
    /// Internal 32-bit float RGB samples.
    ImageRgb32F,
    /// Internal 32-bit float RGBA samples.
    ImageRgba32F,
}

impl Mode {
    /// Every public and internal raster identity.
    pub const ALL: [Self; 25] = [
        Self::One,
        Self::ImageLuma8,
        Self::ImageLumaA8,
        Self::ImageLaPremultiplied8,
        Self::ImagePalette8,
        Self::ImagePaletteAlpha8,
        Self::ImageRgb8,
        Self::ImageRgba8,
        Self::ImageRgbPremultiplied8,
        Self::ImageRgbx8,
        Self::ImageCmyk8,
        Self::ImageYCbCr8,
        Self::ImageLab8,
        Self::ImageHsv8,
        Self::ImageI32,
        Self::ImageF32,
        Self::ImageI16,
        Self::ImageI16L,
        Self::ImageI16B,
        Self::ImageI16N,
        Self::ImageLumaA16,
        Self::ImageRgb16,
        Self::ImageRgba16,
        Self::ImageRgb32F,
        Self::ImageRgba32F,
    ];

    /// Return the default raster identity implied by a decoded byte mode.
    ///
    /// Codec paths with source-specific semantics can override this identity
    /// without changing the transfer buffer's byte layout.
    pub(crate) const fn from_image_mode(mode: super::ImageMode) -> Self {
        match mode {
            super::ImageMode::L1 => Self::One,
            super::ImageMode::P8 => Self::ImagePalette8,
            super::ImageMode::L8 => Self::ImageLuma8,
            super::ImageMode::La8 => Self::ImageLumaA8,
            super::ImageMode::L16 => Self::ImageI16,
            super::ImageMode::La16 => Self::ImageLumaA16,
            super::ImageMode::Rgb8 => Self::ImageRgb8,
            super::ImageMode::Rgba8 => Self::ImageRgba8,
            super::ImageMode::Rgb16 => Self::ImageRgb16,
            super::ImageMode::Rgba16 => Self::ImageRgba16,
            super::ImageMode::Rgb32F => Self::ImageRgb32F,
            super::ImageMode::Rgba32F => Self::ImageRgba32F,
            super::ImageMode::F32 => Self::ImageF32,
            super::ImageMode::I32 => Self::ImageI32,
            super::ImageMode::Cmyk8 => Self::ImageCmyk8,
        }
    }

    /// Compatibility alias for Pillow's `L` mode.
    pub const L: Self = Self::ImageLuma8;
    /// Compatibility alias for Pillow's `LA` mode.
    pub const LA: Self = Self::ImageLumaA8;
    /// Compatibility alias for Pillow's premultiplied `La` mode.
    #[allow(
        non_upper_case_globals,
        reason = "Preserve the established Rust alias spelling for Pillow's La mode."
    )]
    pub const LaPremultiplied: Self = Self::ImageLaPremultiplied8;
    /// Compatibility alias for Pillow's `P` mode.
    pub const P: Self = Self::ImagePalette8;
    /// Compatibility alias for Pillow's bilevel `1` mode.
    #[allow(
        non_upper_case_globals,
        reason = "Preserve the established Rust alias spelling for Pillow's 1 mode."
    )]
    pub const Mode1: Self = Self::One;
    /// Compatibility alias for Pillow's `RGB` mode.
    pub const RGB: Self = Self::ImageRgb8;
    /// Compatibility alias for Pillow's `RGBA` mode.
    pub const RGBA: Self = Self::ImageRgba8;
    /// Compatibility alias for Pillow's premultiplied `RGBa` mode.
    #[allow(
        non_upper_case_globals,
        reason = "Preserve the established Rust alias spelling for Pillow's RGBa mode."
    )]
    pub const RgbPremultiplied: Self = Self::ImageRgbPremultiplied8;
    /// Compatibility alias for Pillow's padded `RGBX` mode.
    #[allow(
        non_upper_case_globals,
        reason = "Preserve the established Rust alias spelling for Pillow's RGBX mode."
    )]
    pub const Rgbx: Self = Self::ImageRgbx8;
    /// Compatibility alias for Pillow's `CMYK` mode.
    pub const CMYK: Self = Self::ImageCmyk8;
    /// Compatibility alias for Pillow's `YCbCr` mode.
    #[allow(
        non_upper_case_globals,
        reason = "Preserve the established Rust alias spelling for Pillow's YCbCr mode."
    )]
    pub const YCbCr: Self = Self::ImageYCbCr8;
    /// Compatibility alias for Pillow's `HSV` mode.
    pub const HSV: Self = Self::ImageHsv8;
    /// Compatibility alias for Pillow's `I` mode.
    pub const I: Self = Self::ImageI32;
    /// Compatibility alias for Pillow's `F` mode.
    pub const F: Self = Self::ImageF32;
    /// Compatibility alias for Pillow's native-endian `I;16` mode.
    pub const I16: Self = Self::ImageI16;

    /// Parses a Pillow mode spelling at an external API boundary.
    ///
    /// ```
    /// use image_slash_star::Mode;
    /// assert_eq!(Mode::from_pillow_name("RGB"), Some(Mode::ImageRgb8));
    /// ```
    #[must_use]
    pub fn from_pillow_name(name: &str) -> Option<Self> {
        match name {
            "1" => Some(Self::One),
            "L" => Some(Self::ImageLuma8),
            "LA" => Some(Self::ImageLumaA8),
            "La" => Some(Self::ImageLaPremultiplied8),
            "P" => Some(Self::ImagePalette8),
            "PA" => Some(Self::ImagePaletteAlpha8),
            "RGB" => Some(Self::ImageRgb8),
            "RGBA" => Some(Self::ImageRgba8),
            "RGBa" => Some(Self::ImageRgbPremultiplied8),
            "RGBX" => Some(Self::ImageRgbx8),
            "CMYK" => Some(Self::ImageCmyk8),
            "YCbCr" => Some(Self::ImageYCbCr8),
            "LAB" => Some(Self::ImageLab8),
            "HSV" => Some(Self::ImageHsv8),
            "I" => Some(Self::ImageI32),
            "F" => Some(Self::ImageF32),
            "I;16" => Some(Self::ImageI16),
            "I;16L" => Some(Self::ImageI16L),
            "I;16B" => Some(Self::ImageI16B),
            "I;16N" => Some(Self::ImageI16N),
            _ => None,
        }
    }

    /// Returns the Pillow spelling for modes with a Pillow representation.
    #[must_use]
    pub const fn pillow_name(self) -> Option<&'static str> {
        match self {
            Self::One => Some("1"),
            Self::ImageLuma8 => Some("L"),
            Self::ImageLumaA8 => Some("LA"),
            Self::ImageLaPremultiplied8 => Some("La"),
            Self::ImagePalette8 => Some("P"),
            Self::ImagePaletteAlpha8 => Some("PA"),
            Self::ImageRgb8 => Some("RGB"),
            Self::ImageRgba8 => Some("RGBA"),
            Self::ImageRgbPremultiplied8 => Some("RGBa"),
            Self::ImageRgbx8 => Some("RGBX"),
            Self::ImageCmyk8 => Some("CMYK"),
            Self::ImageYCbCr8 => Some("YCbCr"),
            Self::ImageLab8 => Some("LAB"),
            Self::ImageHsv8 => Some("HSV"),
            Self::ImageI32 => Some("I"),
            Self::ImageF32 => Some("F"),
            Self::ImageI16 => Some("I;16"),
            Self::ImageI16L => Some("I;16L"),
            Self::ImageI16B => Some("I;16B"),
            Self::ImageI16N => Some("I;16N"),
            Self::ImageLumaA16
            | Self::ImageRgb16
            | Self::ImageRgba16
            | Self::ImageRgb32F
            | Self::ImageRgba32F => None,
        }
    }

    /// Returns the number of sample bands represented by this identity.
    #[must_use]
    pub const fn band_count(self) -> usize {
        match self {
            Self::One
            | Self::ImageLuma8
            | Self::ImagePalette8
            | Self::ImageI32
            | Self::ImageF32
            | Self::ImageI16
            | Self::ImageI16L
            | Self::ImageI16B
            | Self::ImageI16N => 1,
            Self::ImageLumaA8
            | Self::ImageLaPremultiplied8
            | Self::ImagePaletteAlpha8
            | Self::ImageLumaA16 => 2,
            Self::ImageRgb8
            | Self::ImageYCbCr8
            | Self::ImageLab8
            | Self::ImageHsv8
            | Self::ImageRgb16
            | Self::ImageRgb32F => 3,
            Self::ImageRgba8
            | Self::ImageRgbPremultiplied8
            | Self::ImageRgbx8
            | Self::ImageCmyk8
            | Self::ImageRgba16
            | Self::ImageRgba32F => 4,
        }
    }

    /// Returns Pillow's public band count, or zero for internal-only modes.
    #[must_use]
    pub const fn pillow_band_count(self) -> usize {
        match self.pillow_name() {
            Some(_) => self.band_count(),
            None => 0,
        }
    }

    /// Returns the index of an uppercase `A` band, if present.
    ///
    /// This describes the band name, not alpha semantics: the `A` band in Lab
    /// is a color axis, while the `A` band in LA/RGBA is alpha.
    #[must_use]
    pub const fn uppercase_a_band_index(self) -> Option<usize> {
        match self {
            Self::ImageLumaA8 | Self::ImagePaletteAlpha8 | Self::ImageLab8 | Self::ImageLumaA16 => {
                Some(1)
            }
            Self::ImageRgba8 | Self::ImageRgba16 | Self::ImageRgba32F => Some(3),
            Self::One
            | Self::ImageLuma8
            | Self::ImageLaPremultiplied8
            | Self::ImagePalette8
            | Self::ImageRgb8
            | Self::ImageRgbPremultiplied8
            | Self::ImageRgbx8
            | Self::ImageCmyk8
            | Self::ImageYCbCr8
            | Self::ImageHsv8
            | Self::ImageI32
            | Self::ImageF32
            | Self::ImageI16
            | Self::ImageI16L
            | Self::ImageI16B
            | Self::ImageI16N
            | Self::ImageRgb16
            | Self::ImageRgb32F => None,
        }
    }

    /// Whether this identity includes alpha, including premultiplied alpha.
    #[must_use]
    pub const fn has_alpha(self) -> bool {
        matches!(
            self,
            Self::ImageLumaA8
                | Self::ImageLaPremultiplied8
                | Self::ImagePaletteAlpha8
                | Self::ImageRgba8
                | Self::ImageRgbPremultiplied8
                | Self::ImageLumaA16
                | Self::ImageRgba16
                | Self::ImageRgba32F
        )
    }

    /// Select the raster identity represented by an inspected codec header.
    #[cfg(any(
        feature = "jpeg",
        feature = "png",
        feature = "gif",
        feature = "bmp",
        feature = "tiff",
        feature = "webp",
        feature = "ico",
        feature = "avif"
    ))]
    pub(crate) const fn from_header(
        mode: ImageMode,
        format: ImageFormat,
        alpha: Option<SourceAlpha>,
        byte_order: Option<SourceByteOrder>,
    ) -> Self {
        match mode {
            ImageMode::La8 if matches!(alpha, Some(SourceAlpha::Premultiplied)) => {
                Self::ImageLaPremultiplied8
            }
            ImageMode::L16
                if matches!(
                    (format, byte_order),
                    (ImageFormat::Tiff, Some(SourceByteOrder::Big))
                ) =>
            {
                Self::ImageI16B
            }
            _ => Self::from_image_mode(mode),
        }
    }
}
