//! Decoded sample layout metadata shared by the codecs.

/// The unpacked color representation of a decoded pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ColorType {
    /// Eight-bit luminance.
    L8,
    /// Eight-bit luminance and alpha.
    La8,
    /// Eight-bit red, green, and blue.
    Rgb8,
    /// Eight-bit red, green, blue, and alpha.
    Rgba8,
    /// Eight-bit cyan, magenta, yellow, and black.
    Cmyk8,
    /// Sixteen-bit luminance.
    L16,
    /// Sixteen-bit luminance and alpha.
    La16,
    /// Sixteen-bit red, green, and blue.
    Rgb16,
    /// Sixteen-bit red, green, blue, and alpha.
    Rgba16,
    /// Thirty-two-bit floating-point red, green, and blue.
    Rgb32F,
    /// Thirty-two-bit floating-point red, green, blue, and alpha.
    Rgba32F,
    /// Thirty-two-bit floating-point luminance.
    L32F,
    /// Thirty-two-bit integer luminance.
    L32I,
}

impl ColorType {
    /// Number of bytes in one unpacked pixel.
    #[must_use]
    pub const fn bytes_per_pixel(self) -> u8 {
        match self {
            Self::L8 => 1,
            Self::L16 | Self::La8 => 2,
            Self::Rgb8 => 3,
            Self::Rgba8 | Self::Cmyk8 | Self::La16 | Self::L32F | Self::L32I => 4,
            Self::Rgb16 => 6,
            Self::Rgba16 => 8,
            Self::Rgb32F => 12,
            Self::Rgba32F => 16,
        }
    }

    /// Number of channels in one unpacked pixel.
    #[must_use]
    pub const fn channel_count(self) -> u8 {
        match self {
            Self::L8 | Self::L16 | Self::L32F | Self::L32I => 1,
            Self::La8 | Self::La16 => 2,
            Self::Rgb8 | Self::Rgb16 | Self::Rgb32F => 3,
            Self::Rgba8 | Self::Cmyk8 | Self::Rgba16 | Self::Rgba32F => 4,
        }
    }

    /// Number of bits in one unpacked pixel.
    #[must_use]
    pub fn bits_per_pixel(self) -> u16 {
        u16::from(self.bytes_per_pixel()).saturating_mul(8)
    }

    /// Whether this representation contains alpha.
    #[must_use]
    pub const fn has_alpha(self) -> bool {
        matches!(
            self,
            Self::La8 | Self::Rgba8 | Self::La16 | Self::Rgba16 | Self::Rgba32F
        )
    }

    /// Whether this representation contains color rather than only luminance.
    #[must_use]
    pub const fn has_color(self) -> bool {
        matches!(
            self,
            Self::Rgb8
                | Self::Rgba8
                | Self::Cmyk8
                | Self::Rgb16
                | Self::Rgba16
                | Self::Rgb32F
                | Self::Rgba32F
        )
    }
}
