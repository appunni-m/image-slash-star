//! The pillow-rs-image type system.
//!
//! This module provides the core image types matching the `image` crate's API,
//! allowing `pillow-rs` to swap `use image::*` for `use pillow_rs_image::*`.

pub mod buffer;
pub mod color;
pub mod dynamic;
pub mod error;
pub mod traits;

// Re-exports matching the `image` crate's top-level API.
pub use self::buffer::{
    ConvertBuffer,
    // Iterators
    EnumeratePixels,
    EnumeratePixelsMut,
    EnumerateRows,
    EnumerateRowsMut,
    GrayAlphaImage,
    GrayImage,
    ImageBuffer,
    Pixels,
    PixelsMut,
    Rgb32FImage,
    RgbImage,
    Rgba32FImage,
    RgbaImage,
    Rows,
    RowsMut,
};
pub use self::color::{
    ColorType, ExtendedColorType, FromColor, FromPrimitive, Luma, LumaA, Rgb, Rgba,
};
pub use self::dynamic::DynamicImage;
pub use self::error::{ImageError, ImageResult, Rect};
pub use self::traits::{
    EncodableLayout, Enlargeable, GenericImage, GenericImageView, Pixel, Primitive,
};

// ---------------------------------------------------------------------------
// ImageFormat — supported encoding/decoding formats
// ---------------------------------------------------------------------------

/// Supported image formats for encoding and decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    /// JPEG
    Jpeg,
    /// PNG
    Png,
    /// GIF
    Gif,
    /// BMP
    Bmp,
    /// WebP
    WebP,
    /// TIFF
    Tiff,
    /// ICO
    Ico,
    /// AVIF
    Avif,
}

impl ImageFormat {
    /// Attempt to detect the image format from a file path extension.
    pub fn from_path<P: AsRef<std::path::Path>>(path: P) -> Result<ImageFormat, ImageError> {
        let ext = path
            .as_ref()
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "jpg" | "jpeg" => Ok(ImageFormat::Jpeg),
            "png" => Ok(ImageFormat::Png),
            "gif" => Ok(ImageFormat::Gif),
            "bmp" => Ok(ImageFormat::Bmp),
            "webp" => Ok(ImageFormat::WebP),
            "tiff" | "tif" => Ok(ImageFormat::Tiff),
            "ico" => Ok(ImageFormat::Ico),
            "avif" => Ok(ImageFormat::Avif),
            _ => Err(ImageError::Unsupported(format!(
                "unknown extension: {}",
                ext
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// DecodedImage — raw decoded pixel buffer
// ---------------------------------------------------------------------------

/// The observable sample layout of decoded bytes.
///
/// `ColorType` alone cannot distinguish grayscale samples from palette indices,
/// or byte-per-pixel luminance from Pillow's packed `1` mode. Codecs must retain
/// this distinction so a later encode operation receives the same information
/// that Pillow keeps on its image object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageMode {
    /// One-bit samples, packed most-significant bit first with rows byte-aligned.
    L1,
    /// One byte per palette index.
    P8,
    /// One byte per luminance sample.
    L8,
    /// Eight-bit luminance and alpha samples.
    La8,
    /// Eight-bit RGB samples.
    Rgb8,
    /// Eight-bit RGBA samples.
    Rgba8,
    /// Eight-bit cyan, magenta, yellow, and black samples.
    Cmyk8,
    /// Little-endian sixteen-bit luminance samples.
    L16,
    /// Little-endian sixteen-bit luminance and alpha samples.
    La16,
    /// Little-endian sixteen-bit RGB samples.
    Rgb16,
    /// Little-endian sixteen-bit RGBA samples.
    Rgba16,
    /// Native-endian 32-bit floating-point RGB samples.
    Rgb32F,
    /// Native-endian 32-bit floating-point RGBA samples.
    Rgba32F,
    /// Pillow-observable 32-bit floating-point luminance samples.
    F32,
    /// Pillow-observable 32-bit integer luminance samples.
    I32,
}

impl From<ColorType> for ImageMode {
    fn from(color: ColorType) -> Self {
        match color {
            ColorType::L8 => Self::L8,
            ColorType::La8 => Self::La8,
            ColorType::Rgb8 => Self::Rgb8,
            ColorType::Rgba8 => Self::Rgba8,
            ColorType::Cmyk8 => Self::Cmyk8,
            ColorType::L16 => Self::L16,
            ColorType::La16 => Self::La16,
            ColorType::Rgb16 => Self::Rgb16,
            ColorType::Rgba16 => Self::Rgba16,
            ColorType::Rgb32F => Self::Rgb32F,
            ColorType::Rgba32F => Self::Rgba32F,
            ColorType::L32F => Self::F32,
            ColorType::L32I => Self::I32,
        }
    }
}

impl ImageMode {
    /// Return the unpacked channel representation used by generic operations.
    #[must_use]
    pub const fn color_type(self) -> ColorType {
        match self {
            Self::L1 | Self::P8 | Self::L8 => ColorType::L8,
            Self::La8 => ColorType::La8,
            Self::Rgb8 => ColorType::Rgb8,
            Self::Rgba8 => ColorType::Rgba8,
            Self::Cmyk8 => ColorType::Cmyk8,
            Self::L16 => ColorType::L16,
            Self::La16 => ColorType::La16,
            Self::Rgb16 => ColorType::Rgb16,
            Self::Rgba16 => ColorType::Rgba16,
            Self::Rgb32F => ColorType::Rgb32F,
            Self::Rgba32F => ColorType::Rgba32F,
            Self::F32 => ColorType::L32F,
            Self::I32 => ColorType::L32I,
        }
    }

    fn expected_bytes(self, width: u32, height: u32) -> Option<usize> {
        let width = usize::try_from(width).ok()?;
        let height = usize::try_from(height).ok()?;
        if self == Self::L1 {
            return width.div_ceil(8).checked_mul(height);
        }
        width
            .checked_mul(height)?
            .checked_mul(usize::from(self.color_type().bytes_per_pixel()))
    }
}

/// RGB palette and optional per-entry alpha values for indexed images.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImagePalette {
    /// Consecutive RGB triplets indexed by the decoded `P8` samples.
    pub rgb: Vec<u8>,
    /// Optional alpha value for each palette entry.
    pub alpha: Vec<u8>,
}

impl ImagePalette {
    /// Construct a palette when its table lengths are structurally valid.
    pub fn new(rgb: Vec<u8>, alpha: Vec<u8>) -> ImageResult<Self> {
        let entries = rgb.len() / 3;
        if rgb.is_empty() || !rgb.len().is_multiple_of(3) || entries > 256 || alpha.len() > entries
        {
            return Err(ImageError::Parameter("invalid indexed palette".to_owned()));
        }
        Ok(Self { rgb, alpha })
    }

    /// Number of RGB entries in this palette.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rgb.len() / 3
    }

    /// Whether this palette contains no RGB entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rgb.is_empty()
    }
}

/// Raw decoded pixel buffer produced by decoders and consumed by encoders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Flat pixel data. Layout depends on `mode`.
    pub pixels: Vec<u8>,
    /// Generic unpacked color representation.
    pub color: ColorType,
    /// Exact observable byte/sample mode.
    pub mode: ImageMode,
    /// Palette retained for `P8` images.
    pub palette: Option<ImagePalette>,
}

impl DecodedImage {
    /// Create a new decoded image.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>, color: ColorType) -> Self {
        Self {
            width,
            height,
            pixels,
            color,
            mode: color.into(),
            palette: None,
        }
    }

    /// Create an image with an exact packed or indexed mode.
    pub fn with_mode(width: u32, height: u32, pixels: Vec<u8>, mode: ImageMode) -> Self {
        Self {
            width,
            height,
            pixels,
            color: mode.color_type(),
            mode,
            palette: None,
        }
    }

    /// Attach an indexed palette while preserving the decoded sample bytes.
    #[must_use]
    pub fn with_palette(mut self, palette: ImagePalette) -> Self {
        self.palette = Some(palette);
        self
    }

    /// Verify dimensions, byte layout, mode, and palette invariants.
    pub fn validate(&self) -> ImageResult<()> {
        let expected = self
            .mode
            .expected_bytes(self.width, self.height)
            .ok_or(ImageError::Dimensions)?;
        if self.width == 0 || self.height == 0 || self.pixels.len() != expected {
            return Err(ImageError::Dimensions);
        }
        if self.color != self.mode.color_type() {
            return Err(ImageError::Parameter(
                "decoded color type does not match its byte mode".to_owned(),
            ));
        }
        match &self.palette {
            Some(palette) if self.mode == ImageMode::P8 => {
                if palette.is_empty()
                    || self
                        .pixels
                        .iter()
                        .any(|&index| usize::from(index) >= palette.len())
                {
                    return Err(ImageError::Parameter(
                        "palette index is outside the retained palette".to_owned(),
                    ));
                }
            }
            Some(_) => {
                return Err(ImageError::Parameter(
                    "only indexed images may carry a palette".to_owned(),
                ));
            }
            None => {}
        }
        Ok(())
    }

    /// Return raw pixel bytes for comparison against PIL reference.
    pub fn as_bytes(&self) -> &[u8] {
        &self.pixels
    }
}

/// Disposal operation applied before displaying the next animation frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameDisposal {
    /// The format does not specify a disposal operation.
    Unspecified,
    /// Leave the rendered frame in place.
    Keep,
    /// Restore the frame rectangle to the background.
    Background,
    /// Restore the canvas to its state before this frame.
    Previous,
    /// Preserve a GIF reserved disposal value exactly as decoded.
    Reserved(u8),
}

/// One decoded animation frame and its presentation metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFrame {
    /// Frame image samples.
    pub image: DecodedImage,
    /// Horizontal offset on the animation canvas.
    pub left: u32,
    /// Vertical offset on the animation canvas.
    pub top: u32,
    /// Presentation duration in milliseconds.
    pub duration_ms: u32,
    /// Disposal operation after presentation.
    pub disposal: FrameDisposal,
    /// Whether the frame samples were stored in GIF interlace order.
    pub interlaced: bool,
}

/// A still image or animation with all frames retained for re-encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedSequence {
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
    /// Frames in presentation order.
    pub frames: Vec<DecodedFrame>,
    /// Format loop count; zero means infinite when present.
    pub loop_count: Option<u32>,
}

impl DecodedSequence {
    /// Wrap one decoded image as a still sequence.
    #[must_use]
    pub fn from_image(image: DecodedImage) -> Self {
        let width = image.width;
        let height = image.height;
        Self {
            width,
            height,
            frames: vec![DecodedFrame {
                image,
                left: 0,
                top: 0,
                duration_ms: 0,
                disposal: FrameDisposal::Unspecified,
                interlaced: false,
            }],
            loop_count: None,
        }
    }

    /// Verify canvas, frame bounds, and each frame's sample layout.
    pub fn validate(&self) -> ImageResult<()> {
        if self.width == 0 || self.height == 0 || self.frames.is_empty() {
            return Err(ImageError::Dimensions);
        }
        for frame in &self.frames {
            frame.image.validate()?;
            let right = frame
                .left
                .checked_add(frame.image.width)
                .ok_or(ImageError::Dimensions)?;
            let bottom = frame
                .top
                .checked_add(frame.image.height)
                .ok_or(ImageError::Dimensions)?;
            if right > self.width || bottom > self.height {
                return Err(ImageError::Dimensions);
            }
        }
        Ok(())
    }

    /// Return the first frame used by still-image APIs.
    #[must_use]
    pub fn first(&self) -> Option<&DecodedImage> {
        self.frames.first().map(|frame| &frame.image)
    }
}
