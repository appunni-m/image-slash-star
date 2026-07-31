//! Container, decoded-sample, palette, animation, and error types used by codecs.

mod color_type;
mod error;

pub use self::color_type::ColorType;
pub use self::error::{ImageError, ImageErrorKind, ImageErrorStage, ImageResult, ResourceLimit};

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let formats = [
        ImageFormat::Jpeg,
        ImageFormat::Png,
        ImageFormat::Gif,
        ImageFormat::Bmp,
        ImageFormat::WebP,
        ImageFormat::Tiff,
        ImageFormat::Ico,
        ImageFormat::Avif,
    ];
    for format in formats {
        let _ = format.as_str();
        let _ = format.to_string();
        let _ = format.mime_type();
        let _ = format.canonical_extension();
        let _ = format.extensions();
        let _ = format.verification_scope();
    }
    // No format currently provides FullPixels; exercise its provides branch
    // directly so the strength ordering stays fully covered.
    for requested in [
        VerificationScope::HeaderOnly,
        VerificationScope::Structure,
        VerificationScope::FullPixels,
    ] {
        assert!(VerificationScope::FullPixels.provides(requested));
    }
    for name in [
        "jpeg", "jpg", "jfif", "jpe", "png", "apng", "gif", "bmp", "webp", "tiff", "tif", "ico",
        "cur", "avif", "avifs",
    ] {
        let _ = ImageFormat::from_name(name);
        let _ = name.parse::<ImageFormat>();
    }
    let _ = ImageFormat::from_name("unknown");
    let _ = ImageFormat::from_path("fixture.PNG");
    let _ = ImageFormat::from_path("fixture.unknown");
    let _ = ImageFormat::from_path("fixture");

    // No decoder currently emits auxiliary alpha; exercise the reserved
    // variant and descriptor round-trip so the semantic space stays covered.
    let descriptor = SourceDescriptor::new()
        .with_alpha(SourceAlpha::Auxiliary)
        .with_byte_order(SourceByteOrder::Big);
    assert_eq!(descriptor.alpha(), Some(SourceAlpha::Auxiliary));
    assert_eq!(descriptor.byte_order(), Some(SourceByteOrder::Big));
    assert!(!descriptor.is_empty());
    let alpha_only = SourceDescriptor::new().with_alpha(SourceAlpha::Auxiliary);
    assert!(!alpha_only.is_empty());
    assert!(SourceDescriptor::new().is_empty());

    // Exercise every short-circuit path of the source-color emptiness check
    // with descriptors that set exactly one field.
    assert!(
        !SourceColor::new()
            .with_srgb(SrgbIntent::Perceptual)
            .is_empty()
    );
    assert!(!SourceColor::new().with_gamma(45_455).is_empty());
    assert!(
        !SourceColor::new()
            .with_chromaticities(SourceChromaticities {
                white_x: 1,
                white_y: 1,
                red_x: 1,
                red_y: 1,
                green_x: 1,
                green_y: 1,
                blue_x: 1,
                blue_y: 1,
            })
            .is_empty()
    );
    assert!(
        !SourceColor::new()
            .with_icc_profile(RawIccProfile {
                keyword: Vec::new(),
                data: Vec::new(),
            })
            .is_empty()
    );

    let colors = [
        ColorType::L8,
        ColorType::La8,
        ColorType::Rgb8,
        ColorType::Rgba8,
        ColorType::Cmyk8,
        ColorType::L16,
        ColorType::La16,
        ColorType::Rgb16,
        ColorType::Rgba16,
        ColorType::Rgb32F,
        ColorType::Rgba32F,
        ColorType::L32F,
        ColorType::L32I,
    ];
    for color in colors {
        let _ = color.bytes_per_pixel();
        let _ = color.channel_count();
        let _ = color.bits_per_pixel();
        let _ = color.has_alpha();
        let _ = color.has_color();
        let mode = ImageMode::from(color);
        let _ = mode.color_type();
    }

    for (rgb, alpha) in [
        (Vec::new(), Vec::new()),
        (vec![0], Vec::new()),
        (vec![0; 257 * 3], Vec::new()),
        (vec![0, 0, 0], vec![0, 0]),
    ] {
        let _ = ImagePalette::new(rgb, alpha);
    }
    let palette =
        ImagePalette::new(vec![0, 0, 0], Vec::new()).expect("coverage palette should be valid");
    let _ = palette.len();
    let _ = palette.is_empty();

    let errors = [
        ImageError::UnknownFormat,
        ImageError::FeatureDisabled {
            format: ImageFormat::Png,
            feature: "png",
        },
        ImageError::Malformed {
            format: ImageFormat::Png,
            message: "coverage".to_owned(),
            stage: Some(ImageErrorStage::StillDecode),
            offset: Some(8),
            identity: Some("png_chunk"),
        },
        ImageError::Unsupported {
            format: Some(ImageFormat::Png),
            message: "coverage".to_owned(),
            stage: Some(ImageErrorStage::StillEncode),
            offset: None,
            identity: None,
        },
        ImageError::Unsupported {
            format: None,
            message: "coverage".to_owned(),
            stage: None,
            offset: None,
            identity: None,
        },
        ImageError::dimensions("coverage"),
        ImageError::parameter("coverage"),
        ImageError::Dimensions {
            format: Some(ImageFormat::Png),
            message: "coverage".to_owned(),
            stage: Some(ImageErrorStage::Inspection),
            offset: Some(8),
            identity: Some("tiff_ifd"),
        },
        ImageError::Parameter {
            format: Some(ImageFormat::Png),
            message: "coverage".to_owned(),
            stage: Some(ImageErrorStage::SequenceDecode),
            offset: None,
            identity: None,
        },
    ];
    for error in errors {
        let _ = error.kind();
        let _ = error.format();
        let _ = error.message();
        let _ = error.to_string();
        let _ = error.stage();
        let _ = error.offset();
        let _ = error.identity();
        let _ = error.with_format(ImageFormat::Png);
    }

    let image = DecodedImage::new(1, 1, vec![0], ColorType::L8);
    let _ = image.validate();
    let _ = DecodedImage::new(0, 1, Vec::new(), ColorType::L8).validate();
    let _ = DecodedImage::new(1, 0, Vec::new(), ColorType::L8).validate();
    let _ = DecodedImage::with_mode(1, 1, vec![0], ImageMode::P8)
        .with_palette(ImagePalette {
            rgb: Vec::new(),
            alpha: Vec::new(),
        })
        .validate();
    let _ = DecodedImage::with_mode(1, 1, vec![1], ImageMode::P8)
        .with_palette(ImagePalette {
            rgb: vec![0, 0, 0],
            alpha: Vec::new(),
        })
        .validate();
    let _ = DecodedImage {
        width: 1,
        height: 1,
        pixels: vec![0],
        color: ColorType::Rgb8,
        mode: ImageMode::L8,
        palette: None,
        cursor_hotspot: None,
        source: SourceDescriptor::new(),
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    }
    .validate();
    let _ = DecodedImage::new(1, 1, vec![0], ColorType::L8)
        .with_palette(ImagePalette {
            rgb: vec![0, 0, 0],
            alpha: Vec::new(),
        })
        .validate();
    let _ = DecodedImage::new(u32::MAX, u32::MAX, Vec::new(), ColorType::Rgb8).validate();
    let _ = DecodedSequence {
        width: 1,
        height: 1,
        frames: Vec::new(),
        loop_count: None,
        background: None,
        kind: SequenceKind::SingleFrame,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    }
    .validate();
    let _ = DecodedSequence {
        width: 0,
        height: 1,
        frames: Vec::new(),
        loop_count: None,
        background: None,
        kind: SequenceKind::SingleFrame,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    }
    .validate();
    let _ = DecodedSequence {
        width: 1,
        height: 0,
        frames: Vec::new(),
        loop_count: None,
        background: None,
        kind: SequenceKind::SingleFrame,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    }
    .validate();
    let frame = DecodedFrame::source_rectangle(
        image,
        0,
        1,
        FrameDuration::ZERO,
        FrameDisposal::Keep,
        FrameBlend::Unspecified,
        false,
    );
    let _ = DecodedSequence {
        width: 1,
        height: 1,
        frames: vec![frame],
        loop_count: None,
        background: None,
        kind: SequenceKind::SingleFrame,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    }
    .validate();
    let right_outside_frame = DecodedFrame::source_rectangle(
        DecodedImage::new(1, 1, vec![0], ColorType::L8),
        1,
        0,
        FrameDuration::ZERO,
        FrameDisposal::Keep,
        FrameBlend::Unspecified,
        false,
    );
    let _ = DecodedSequence {
        width: 1,
        height: 1,
        frames: vec![right_outside_frame],
        loop_count: None,
        background: None,
        kind: SequenceKind::SingleFrame,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    }
    .validate();
    let invalid_frame = DecodedFrame::source_rectangle(
        DecodedImage::new(0, 1, Vec::new(), ColorType::L8),
        0,
        0,
        FrameDuration::ZERO,
        FrameDisposal::Keep,
        FrameBlend::Unspecified,
        false,
    );
    let _ = DecodedSequence {
        width: 1,
        height: 1,
        frames: vec![invalid_frame],
        loop_count: None,
        background: None,
        kind: SequenceKind::SingleFrame,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    }
    .validate();
    let right_overflow_frame = DecodedFrame::source_rectangle(
        DecodedImage::new(1, 1, vec![0], ColorType::L8),
        u32::MAX,
        0,
        FrameDuration::ZERO,
        FrameDisposal::Keep,
        FrameBlend::Unspecified,
        false,
    );
    let _ = DecodedSequence {
        width: u32::MAX,
        height: 1,
        frames: vec![right_overflow_frame],
        loop_count: None,
        background: None,
        kind: SequenceKind::SingleFrame,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    }
    .validate();
    let bottom_overflow_frame = DecodedFrame::source_rectangle(
        DecodedImage::new(1, 1, vec![0], ColorType::L8),
        0,
        u32::MAX,
        FrameDuration::ZERO,
        FrameDisposal::Keep,
        FrameBlend::Unspecified,
        false,
    );
    let _ = DecodedSequence {
        width: 1,
        height: u32::MAX,
        frames: vec![bottom_overflow_frame],
        loop_count: None,
        background: None,
        kind: SequenceKind::SingleFrame,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    }
    .validate();

    let base = DecodedImage::new(1, 1, vec![0], ColorType::L8);
    let mut zero_denominator = DecodedFrame::source_rectangle(
        base.clone(),
        0,
        0,
        FrameDuration::ZERO,
        FrameDisposal::Keep,
        FrameBlend::Unspecified,
        false,
    );
    zero_denominator.source.duration.denominator = 0;
    let _ = DecodedSequence {
        width: 1,
        height: 1,
        frames: vec![zero_denominator],
        loop_count: None,
        background: None,
        kind: SequenceKind::SingleFrame,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    }
    .validate();

    let mut zero_rect = DecodedFrame::source_rectangle(
        base.clone(),
        0,
        0,
        FrameDuration::ZERO,
        FrameDisposal::Keep,
        FrameBlend::Unspecified,
        false,
    );
    zero_rect.source.rect.width = 0;
    let _ = DecodedSequence {
        width: 1,
        height: 1,
        frames: vec![zero_rect],
        loop_count: None,
        background: None,
        kind: SequenceKind::SingleFrame,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    }
    .validate();
    let mut zero_rect_height = DecodedFrame::source_rectangle(
        base.clone(),
        0,
        0,
        FrameDuration::ZERO,
        FrameDisposal::Keep,
        FrameBlend::Unspecified,
        false,
    );
    zero_rect_height.source.rect.height = 0;
    let _ = DecodedSequence {
        width: 1,
        height: 1,
        frames: vec![zero_rect_height],
        loop_count: None,
        background: None,
        kind: SequenceKind::SingleFrame,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    }
    .validate();

    let mut mismatched_source = DecodedFrame::source_rectangle(
        base.clone(),
        0,
        0,
        FrameDuration::ZERO,
        FrameDisposal::Keep,
        FrameBlend::Unspecified,
        false,
    );
    mismatched_source.source.rect.width = 2;
    let _ = DecodedSequence {
        width: 2,
        height: 1,
        frames: vec![mismatched_source],
        loop_count: None,
        background: None,
        kind: SequenceKind::SingleFrame,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    }
    .validate();
    let mut mismatched_source_height = DecodedFrame::source_rectangle(
        base.clone(),
        0,
        0,
        FrameDuration::ZERO,
        FrameDisposal::Keep,
        FrameBlend::Unspecified,
        false,
    );
    mismatched_source_height.source.rect.height = 2;
    let _ = DecodedSequence {
        width: 1,
        height: 2,
        frames: vec![mismatched_source_height],
        loop_count: None,
        background: None,
        kind: SequenceKind::SingleFrame,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    }
    .validate();

    let rendered = DecodedFrame::rendered_canvas(
        base.clone(),
        FrameRect {
            left: 0,
            top: 0,
            width: 1,
            height: 1,
        },
        FrameDuration::ZERO,
        FrameDisposal::Keep,
        FrameBlend::Over,
    );
    let _ = DecodedSequence {
        width: 2,
        height: 1,
        frames: vec![rendered],
        loop_count: None,
        background: None,
        kind: SequenceKind::SingleFrame,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    }
    .validate();
    let rendered_height = DecodedFrame::rendered_canvas(
        base.clone(),
        FrameRect {
            left: 0,
            top: 0,
            width: 1,
            height: 1,
        },
        FrameDuration::ZERO,
        FrameDisposal::Keep,
        FrameBlend::Over,
    );
    let _ = DecodedSequence {
        width: 1,
        height: 2,
        frames: vec![rendered_height],
        loop_count: None,
        background: None,
        kind: SequenceKind::SingleFrame,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
    }
    .validate();

    let still = DecodedSequence::from_image(base);
    let _ = still.first();
    let _ = still.first_image();
    for duration in [
        FrameDuration {
            numerator: 1,
            denominator: 0,
        },
        FrameDuration {
            numerator: u64::MAX,
            denominator: 1,
        },
        FrameDuration {
            numerator: 1,
            denominator: 3,
        },
        FrameDuration {
            numerator: 2,
            denominator: 3,
        },
        FrameDuration {
            numerator: 1,
            denominator: 2_000,
        },
        FrameDuration {
            numerator: 3,
            denominator: 2_000,
        },
    ] {
        let _ = duration.milliseconds_rounded();
    }
}

// ---------------------------------------------------------------------------
// ImageFormat — supported encoding/decoding formats
// ---------------------------------------------------------------------------

/// Supported image formats for encoding and decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
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

/// Amount of validation performed by [`crate::EncodedImage::verify`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum VerificationScope {
    /// Construction-time header inspection is the complete Pillow contract.
    HeaderOnly,
    /// Verification additionally scans format-specific encoded structure.
    Structure,
    /// Verification requires decompressing and comparing every retained
    /// pixel. No codec currently provides this scope.
    FullPixels,
}

impl VerificationScope {
    /// Whether this provided scope satisfies a caller's requested scope.
    ///
    /// Scope strength is ordered `HeaderOnly` < `Structure` < `FullPixels`.
    /// A codec that provides a stronger scope also satisfies every weaker
    /// request; a request stronger than the provided scope must fail rather
    /// than silently report weaker evidence as sufficient.
    #[must_use]
    pub const fn provides(self, requested: Self) -> bool {
        match self {
            Self::HeaderOnly => matches!(requested, Self::HeaderOnly),
            Self::Structure => matches!(requested, Self::HeaderOnly | Self::Structure),
            Self::FullPixels => true,
        }
    }
}

/// A decoded value paired with the encoded container format detected from its input.
///
/// The envelope deliberately keeps source format separate from [`DecodedImage`] and
/// [`DecodedSequence`]. Pixel buffers and sequences created by callers have no
/// intrinsic encoded format until an encoder is selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded<T> {
    /// Encoded container format selected by signature detection.
    pub format: ImageFormat,
    /// Decoded still image or retained image sequence.
    pub content: T,
    /// Encoded bytes of the container-defined extent, or `None` when the
    /// container does not define an unambiguous total extent.
    ///
    /// Decoders ignore well-formed trailing bytes after this extent and never
    /// let them change the decoded result. `None` means the caller should
    /// treat the complete input as the source; it does not mean trailing
    /// bytes were rejected.
    pub consumed_bytes: Option<usize>,
}

/// Header metadata obtained without materializing compressed pixel payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageInfo {
    /// Detected encoded container format.
    pub format: ImageFormat,
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
    /// Observable decoded pixel/sample mode.
    pub mode: ImageMode,
    /// Encoded bits per channel or palette index.
    pub bit_depth: u8,
    /// Palette retained from a separately stored color table.
    pub palette: Option<ImagePalette>,
    /// Whether the container declares more than one presentation frame.
    pub is_animated: bool,
    /// Exact frame count when a cheap container scan provides it.
    pub frame_count: Option<u32>,
    /// Selected Windows cursor hotspot, distinguishing CUR from ordinary ICO.
    pub cursor_hotspot: Option<CursorHotspot>,
    /// Structural facts retained from the encoded source.
    pub source: SourceDescriptor,
}

impl ImageInfo {
    /// Whether the encoded image uses palette indices as its sample mode.
    #[must_use]
    pub const fn is_indexed(&self) -> bool {
        matches!(self.mode, ImageMode::P8)
    }

    /// Whether inspection retained an explicit palette table.
    ///
    /// This is independent of [`Self::is_indexed`]: malformed-but-tolerated or
    /// implicit indexed containers can expose `P8` samples without a table.
    #[must_use]
    pub const fn has_palette_table(&self) -> bool {
        self.palette.is_some()
    }
}

/// Pixel coordinate selected as the activation point of a Windows cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CursorHotspot {
    /// Horizontal coordinate from the cursor image's left edge.
    pub x: u16,
    /// Vertical coordinate from the cursor image's top edge.
    pub y: u16,
}

/// Byte order declared by an encoded source container.
///
/// This does not by itself describe the byte order of
/// [`DecodedImage::pixels`]. A codec may normalize its transfer layout. TIFF
/// `I32` and `F32` deliberately retain source-order bytes, while TIFF `L16`
/// is normalized to little-endian transfer bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceByteOrder {
    /// Least-significant byte first.
    Little,
    /// Most-significant byte first.
    Big,
}

/// Source alpha association declared by an encoded container.
///
/// This records what the container says about its own alpha, not the transfer
/// layout of [`DecodedImage::pixels`]. Decoded transfer bytes remain the
/// documented Pillow-observable normalized layout (unassociated samples),
/// except where a codec explicitly retains source-order bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SourceAlpha {
    /// Straight (unassociated) alpha: samples are not premultiplied.
    Straight,
    /// Premultiplied (associated) alpha.
    Premultiplied,
    /// A binary transparency mask selects fully transparent samples.
    BinaryMask,
    /// Alpha carried by a separate auxiliary channel or image.
    Auxiliary,
}

/// Extensible structural facts retained from an encoded source.
///
/// The descriptor is separate from opaque ICC, EXIF, XMP, text, or
/// format-specific payloads. Its private representation permits adding
/// independently proved source facts without making callers rebuild public
/// struct literals.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceDescriptor {
    byte_order: Option<SourceByteOrder>,
    alpha: Option<SourceAlpha>,
}

impl SourceDescriptor {
    /// Create an empty descriptor for caller-created pixels or a source with
    /// no retained structural facts.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            byte_order: None,
            alpha: None,
        }
    }

    /// Record the byte order declared by the encoded source.
    #[must_use]
    pub const fn with_byte_order(mut self, byte_order: SourceByteOrder) -> Self {
        self.byte_order = Some(byte_order);
        self
    }

    /// Return the byte order declared by the encoded source, when retained.
    #[must_use]
    pub const fn byte_order(&self) -> Option<SourceByteOrder> {
        self.byte_order
    }

    /// Record the alpha association declared by the encoded source.
    #[must_use]
    pub const fn with_alpha(mut self, alpha: SourceAlpha) -> Self {
        self.alpha = Some(alpha);
        self
    }

    /// Return the alpha association declared by the encoded source, when
    /// retained. `None` means the container declares no alpha semantics.
    #[must_use]
    pub const fn alpha(&self) -> Option<SourceAlpha> {
        self.alpha
    }

    /// Whether this source has no retained structural facts.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.byte_order.is_none() && self.alpha.is_none()
    }
}

/// One opaque container block retained from an encoded source in original
/// order.
///
/// Blocks are payload-only: length, type framing, checksums, and offsets are
/// not retained. A codec may retain only blocks its own container defines;
/// blocks from one format are never replayed by another. Default encoding
/// never writes retained blocks implicitly; a future explicit replay API must
/// define collisions with encoder-generated blocks first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueBlock {
    /// Format-specific block type label (PNG: the four-byte chunk type).
    pub kind: Vec<u8>,
    /// Raw encoded payload bytes.
    pub data: Vec<u8>,
    /// Whether the container marks this block safe to copy on re-encode.
    pub safe_to_copy: bool,
}

/// Known metadata retained from an encoded source without semantic parsing.
///
/// Payloads are kept exactly as stored: compressed metadata (for example PNG
/// `zTXt`, `iTXt`, or `iCCP`) is not inflated. Default encoding never replays
/// metadata implicitly; an explicit replay API must define collisions with
/// encoder-generated metadata first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueMetadata {
    /// Format-specific metadata kind (PNG: the four-byte chunk type).
    pub kind: Vec<u8>,
    /// Raw encoded payload bytes exactly as stored.
    pub data: Vec<u8>,
}

/// sRGB rendering intent declared by a PNG `sRGB` chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SrgbIntent {
    /// Perceptual intent (PNG value 0).
    Perceptual,
    /// Relative colorimetric intent (PNG value 1).
    RelativeColorimetric,
    /// Saturation intent (PNG value 2).
    Saturation,
    /// Absolute colorimetric intent (PNG value 3).
    AbsoluteColorimetric,
}

/// Exact PNG `cHRM` chromaticity values, each scaled by 100,000.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceChromaticities {
    /// White point x.
    pub white_x: u32,
    /// White point y.
    pub white_y: u32,
    /// Red primary x.
    pub red_x: u32,
    /// Red primary y.
    pub red_y: u32,
    /// Green primary x.
    pub green_x: u32,
    /// Green primary y.
    pub green_y: u32,
    /// Blue primary x.
    pub blue_x: u32,
    /// Blue primary y.
    pub blue_y: u32,
}

/// Raw ICC profile retained from an encoded container without inflation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawIccProfile {
    /// Profile keyword from the container record.
    pub keyword: Vec<u8>,
    /// Raw profile bytes exactly as stored after the keyword terminator
    /// (compressed payloads are not inflated).
    pub data: Vec<u8>,
}

/// Source color metadata retained from an encoded container.
///
/// Retaining these fields records what the source declares; it never implies
/// that color conversion was applied to decoded samples.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceColor {
    srgb: Option<SrgbIntent>,
    gamma: Option<u32>,
    chromaticities: Option<SourceChromaticities>,
    icc_profile: Option<RawIccProfile>,
}

impl SourceColor {
    /// Create an empty source color descriptor.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            srgb: None,
            gamma: None,
            chromaticities: None,
            icc_profile: None,
        }
    }

    /// Record the sRGB rendering intent declared by the source.
    #[must_use]
    pub const fn with_srgb(mut self, srgb: SrgbIntent) -> Self {
        self.srgb = Some(srgb);
        self
    }

    /// Return the sRGB rendering intent declared by the source, when retained.
    #[must_use]
    pub const fn srgb(&self) -> Option<SrgbIntent> {
        self.srgb
    }

    /// Record the exact PNG gamma value (scaled by 100,000).
    #[must_use]
    pub const fn with_gamma(mut self, gamma: u32) -> Self {
        self.gamma = Some(gamma);
        self
    }

    /// Return the exact PNG gamma value, when retained.
    #[must_use]
    pub const fn gamma(&self) -> Option<u32> {
        self.gamma
    }

    /// Record the exact PNG chromaticity values.
    #[must_use]
    pub const fn with_chromaticities(mut self, chromaticities: SourceChromaticities) -> Self {
        self.chromaticities = Some(chromaticities);
        self
    }

    /// Return the exact PNG chromaticity values, when retained.
    #[must_use]
    pub const fn chromaticities(&self) -> Option<SourceChromaticities> {
        self.chromaticities
    }

    /// Record the raw ICC profile retained from the source.
    #[must_use]
    pub fn with_icc_profile(mut self, icc_profile: RawIccProfile) -> Self {
        self.icc_profile = Some(icc_profile);
        self
    }

    /// Return the raw ICC profile, when retained.
    #[must_use]
    pub const fn icc_profile(&self) -> Option<&RawIccProfile> {
        self.icc_profile.as_ref()
    }

    /// Whether this descriptor retains no source color facts.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.srgb.is_none()
            && self.gamma.is_none()
            && self.chromaticities.is_none()
            && self.icc_profile.is_none()
    }
}

impl<T> Decoded<T> {
    /// Pair decoded content with its detected encoded format.
    #[must_use]
    pub const fn new(format: ImageFormat, content: T, consumed_bytes: Option<usize>) -> Self {
        Self {
            format,
            content,
            consumed_bytes,
        }
    }

    /// Borrow the decoded content without discarding its source format.
    #[must_use]
    pub const fn as_ref(&self) -> Decoded<&T> {
        Decoded::new(self.format, &self.content, self.consumed_bytes)
    }

    /// Consume the envelope and return only its decoded content.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.content
    }
}

impl ImageFormat {
    /// Returns Pillow's canonical uppercase name for this encoded format.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "JPEG",
            ImageFormat::Png => "PNG",
            ImageFormat::Gif => "GIF",
            ImageFormat::Bmp => "BMP",
            ImageFormat::WebP => "WEBP",
            ImageFormat::Tiff => "TIFF",
            ImageFormat::Ico => "ICO",
            ImageFormat::Avif => "AVIF",
        }
    }

    /// Returns the validation scope of [`crate::EncodedImage::verify`].
    ///
    /// Pillow 12.2.0 implements an additional structural verification pass for
    /// PNG. This crate also retains its independently proved JPEG and WebP
    /// structural verifiers. Pillow's default verifier for the other formats
    /// performs no work beyond a successful open, which corresponds to the
    /// header inspection already performed by [`crate::EncodedImage::new`].
    #[must_use]
    pub const fn verification_scope(self) -> VerificationScope {
        match self {
            Self::Jpeg | Self::Png | Self::WebP => VerificationScope::Structure,
            Self::Gif | Self::Bmp | Self::Tiff | Self::Ico | Self::Avif => {
                VerificationScope::HeaderOnly
            }
        }
    }

    /// Return one stable MIME media type for this format.
    #[must_use]
    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Gif => "image/gif",
            Self::Bmp => "image/bmp",
            Self::WebP => "image/webp",
            Self::Tiff => "image/tiff",
            Self::Ico => "image/x-icon",
            Self::Avif => "image/avif",
        }
    }

    /// Return the canonical file extension without a leading dot.
    #[must_use]
    pub const fn canonical_extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Gif => "gif",
            Self::Bmp => "bmp",
            Self::WebP => "webp",
            Self::Tiff => "tiff",
            Self::Ico => "ico",
            Self::Avif => "avif",
        }
    }

    /// Return every accepted file extension in canonical-first order.
    #[must_use]
    pub const fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Jpeg => &["jpg", "jpeg", "jfif", "jpe"],
            Self::Png => &["png", "apng"],
            Self::Gif => &["gif"],
            Self::Bmp => &["bmp"],
            Self::WebP => &["webp"],
            Self::Tiff => &["tiff", "tif"],
            Self::Ico => &["ico", "cur"],
            Self::Avif => &["avif", "avifs"],
        }
    }

    /// Parses a case-insensitive image format name or common extension alias.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::UnknownFormat`] when `name` does not identify a
    /// supported image container.
    pub fn from_name(name: &str) -> Result<Self, ImageError> {
        if name.eq_ignore_ascii_case("jpeg")
            || name.eq_ignore_ascii_case("jpg")
            || name.eq_ignore_ascii_case("jfif")
            || name.eq_ignore_ascii_case("jpe")
        {
            Ok(ImageFormat::Jpeg)
        } else if name.eq_ignore_ascii_case("png") || name.eq_ignore_ascii_case("apng") {
            Ok(ImageFormat::Png)
        } else if name.eq_ignore_ascii_case("gif") {
            Ok(ImageFormat::Gif)
        } else if name.eq_ignore_ascii_case("bmp") {
            Ok(ImageFormat::Bmp)
        } else if name.eq_ignore_ascii_case("webp") {
            Ok(ImageFormat::WebP)
        } else if name.eq_ignore_ascii_case("tiff") || name.eq_ignore_ascii_case("tif") {
            Ok(ImageFormat::Tiff)
        } else if name.eq_ignore_ascii_case("ico") || name.eq_ignore_ascii_case("cur") {
            Ok(ImageFormat::Ico)
        } else if name.eq_ignore_ascii_case("avif") || name.eq_ignore_ascii_case("avifs") {
            Ok(ImageFormat::Avif)
        } else {
            Err(ImageError::UnknownFormat)
        }
    }

    /// Detects the image format from the final file-name extension.
    ///
    /// This function performs no filesystem access: it does not open,
    /// canonicalize, resolve, or validate `path`. Parent-directory components
    /// therefore have no special meaning here. A caller that subsequently
    /// reads an untrusted path must enforce its own allowed-root and symlink
    /// policy at that I/O boundary.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::Unsupported`] when the path has no recognized
    /// Unicode extension.
    pub fn from_path<P: AsRef<std::path::Path>>(path: P) -> Result<ImageFormat, ImageError> {
        let ext = path
            .as_ref()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        Self::from_name(ext).map_err(|_| ImageError::Unsupported {
            format: None,
            message: format!("unknown extension: {}", ext.to_ascii_lowercase()),
            stage: None,
            offset: None,
            identity: None,
        })
    }
}

impl std::fmt::Display for ImageFormat {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for ImageFormat {
    type Err = ImageError;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::from_name(name)
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
#[non_exhaustive]
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
    /// Exact Pillow-observable 32-bit floating-point luminance bytes.
    ///
    /// This mode does not promise a portable scalar byte order. In particular,
    /// Pillow retains the file byte order for TIFF `F` images.
    F32,
    /// Exact Pillow-observable 32-bit integer luminance bytes.
    ///
    /// This mode does not promise a portable scalar byte order. In particular,
    /// Pillow retains the file byte order for TIFF `I` images.
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
    /// Return the unpacked channel representation used by codecs.
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

    pub(crate) fn expected_bytes(self, width: u32, height: u32) -> ImageResult<usize> {
        let width = width as usize;
        let height = height as usize;
        if self == Self::L1 {
            return width
                .div_ceil(8)
                .checked_mul(height)
                .ok_or(ImageError::dimensions(
                    "packed bilevel byte length overflows",
                ));
        }
        width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(usize::from(self.color_type().bytes_per_pixel())))
            .ok_or(ImageError::dimensions(
                "decoded pixel byte length overflows",
            ))
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
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::Parameter`] when RGB bytes are empty or not
    /// complete triplets, more than 256 entries are supplied, or the alpha
    /// table is longer than the RGB table.
    pub fn new(rgb: Vec<u8>, alpha: Vec<u8>) -> ImageResult<Self> {
        let palette = Self { rgb, alpha };
        palette.validate()?;
        Ok(palette)
    }

    fn validate(&self) -> ImageResult<()> {
        let entries = self.rgb.len() / 3;
        if self.rgb.is_empty()
            || !self.rgb.len().is_multiple_of(3)
            || entries > 256
            || self.alpha.len() > entries
        {
            return Err(ImageError::parameter("invalid indexed palette"));
        }
        Ok(())
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
    /// Selected Windows cursor hotspot, or `None` for ordinary images/icons.
    pub cursor_hotspot: Option<CursorHotspot>,
    /// Structural facts retained from the encoded source.
    pub source: SourceDescriptor,
    /// Opaque container blocks retained in original order.
    pub opaque_blocks: Vec<OpaqueBlock>,
    /// Known metadata retained in original order without semantic parsing.
    pub metadata: Vec<OpaqueMetadata>,
    /// Source color metadata retained from the encoded container.
    pub source_color: SourceColor,
}

impl DecodedImage {
    /// Create an unpacked decoded image.
    ///
    /// This constructor records the buffer without validating its dimensions
    /// or length. Call [`Self::validate`] before handing caller-built samples
    /// to code that relies on those invariants; encoders validate inputs.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>, color: ColorType) -> Self {
        Self {
            width,
            height,
            pixels,
            color,
            mode: color.into(),
            palette: None,
            cursor_hotspot: None,
            source: SourceDescriptor::new(),
            opaque_blocks: Vec::new(),
            metadata: Vec::new(),
            source_color: SourceColor::new(),
        }
    }

    /// Create an image with an exact packed or indexed mode.
    ///
    /// This constructor does not attach an optional [`ImageMode::P8`] palette
    /// and does not validate the byte length. Use [`Self::with_palette`] when a
    /// source palette is available, then call [`Self::validate`].
    pub fn with_mode(width: u32, height: u32, pixels: Vec<u8>, mode: ImageMode) -> Self {
        Self {
            width,
            height,
            pixels,
            color: mode.color_type(),
            mode,
            palette: None,
            cursor_hotspot: None,
            source: SourceDescriptor::new(),
            opaque_blocks: Vec::new(),
            metadata: Vec::new(),
            source_color: SourceColor::new(),
        }
    }

    /// Attach an indexed palette while preserving the decoded sample bytes.
    #[must_use]
    pub fn with_palette(mut self, palette: ImagePalette) -> Self {
        self.palette = Some(palette);
        self
    }

    /// Attach a Windows cursor hotspot while preserving decoded sample bytes.
    #[must_use]
    pub fn with_cursor_hotspot(mut self, hotspot: CursorHotspot) -> Self {
        self.cursor_hotspot = Some(hotspot);
        self
    }

    /// Attach structural source facts without changing decoded sample bytes.
    #[must_use]
    pub fn with_source_descriptor(mut self, source: SourceDescriptor) -> Self {
        self.source = source;
        self
    }

    /// Attach opaque container blocks without changing decoded sample bytes.
    #[must_use]
    pub fn with_opaque_blocks(mut self, opaque_blocks: Vec<OpaqueBlock>) -> Self {
        self.opaque_blocks = opaque_blocks;
        self
    }

    /// Attach known metadata without changing decoded sample bytes.
    #[must_use]
    pub fn with_metadata(mut self, metadata: Vec<OpaqueMetadata>) -> Self {
        self.metadata = metadata;
        self
    }

    /// Attach source color metadata without changing decoded sample bytes.
    #[must_use]
    pub fn with_source_color(mut self, color: SourceColor) -> Self {
        self.source_color = color;
        self
    }

    /// Verify dimensions, byte layout, mode, and palette invariants.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::Dimensions`] for zero, overflowing, or
    /// byte-length-mismatched dimensions. Returns [`ImageError::Parameter`]
    /// for inconsistent modes, color layouts, palettes, or palette indices.
    pub fn validate(&self) -> ImageResult<()> {
        let expected = self.mode.expected_bytes(self.width, self.height)?;
        if self.width == 0 || self.height == 0 || self.pixels.len() != expected {
            return Err(ImageError::dimensions(
                "decoded dimensions and pixel byte length do not agree",
            ));
        }
        if self.color != self.mode.color_type() {
            return Err(ImageError::parameter(
                "decoded color type does not match its byte mode",
            ));
        }
        match &self.palette {
            Some(palette) if self.mode == ImageMode::P8 => {
                palette.validate()?;
                if self
                    .pixels
                    .iter()
                    .any(|&index| usize::from(index) >= palette.len())
                {
                    return Err(ImageError::parameter(
                        "palette index is outside the retained palette",
                    ));
                }
            }
            Some(_) => {
                return Err(ImageError::parameter(
                    "only indexed images may carry a palette",
                ));
            }
            None => {}
        }
        Ok(())
    }

    /// Return the decoded sample bytes without changing their mode.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.pixels
    }
}

/// Disposal operation applied before displaying the next animation frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
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

/// Alpha-composition rule used when a source frame enters its canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FrameBlend {
    /// The source format does not define a blend operation.
    Unspecified,
    /// Replace every sample in the source rectangle.
    Source,
    /// Alpha-composite the source rectangle over the existing canvas.
    Over,
    /// Preserve a format-reserved value for diagnostics and future support.
    Reserved(u8),
}

/// Exact source-frame duration as a fraction of one second.
///
/// Decoders retain the source numerator and effective denominator without
/// rounding to milliseconds. A zero denominator is invalid in the common
/// model; formats such as APNG that encode zero as a special default normalize
/// it to that effective denominator while parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameDuration {
    /// Source or caller-supplied numerator.
    pub numerator: u64,
    /// Effective non-zero denominator.
    pub denominator: u64,
}

impl FrameDuration {
    /// A zero-length duration represented without format-specific scaling.
    pub const ZERO: Self = Self {
        numerator: 0,
        denominator: 1,
    };

    /// Construct an exact millisecond duration.
    #[must_use]
    pub const fn from_milliseconds(milliseconds: u32) -> Self {
        Self {
            numerator: milliseconds as u64,
            denominator: 1_000,
        }
    }

    /// Return the nearest whole millisecond using round-half-to-even.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::Parameter`] for a zero denominator and
    /// [`ImageError::Dimensions`] when the rounded value does not fit `u32`.
    pub fn milliseconds_rounded(self) -> ImageResult<u32> {
        if self.denominator == 0 {
            return Err(ImageError::parameter(
                "frame duration denominator must be non-zero",
            ));
        }
        let numerator = u128::from(self.numerator).saturating_mul(1_000);
        let denominator = u128::from(self.denominator);
        let quotient = numerator.div_euclid(denominator);
        let remainder = numerator.rem_euclid(denominator);
        let doubled_remainder = remainder.saturating_mul(2);
        let rounded = quotient.saturating_add(u128::from(
            doubled_remainder > denominator
                || (doubled_remainder == denominator && !quotient.is_multiple_of(2)),
        ));
        u32::try_from(rounded)
            .map_err(|_| ImageError::dimensions("frame duration exceeds u32 milliseconds"))
    }
}

/// Rectangle occupied by one encoded source frame on its animation canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameRect {
    /// Horizontal offset from the canvas origin.
    pub left: u32,
    /// Vertical offset from the canvas origin.
    pub top: u32,
    /// Source-frame width.
    pub width: u32,
    /// Source-frame height.
    pub height: u32,
}

/// Relationship between [`DecodedFrame::image`] and its animation canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FramePixelLayout {
    /// `image` contains only the uncomposited source rectangle.
    SourceRectangle,
    /// `image` contains the complete composited canvas at display time.
    RenderedCanvas,
}

/// Source-frame metadata retained independently from decoded pixel layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameSource {
    /// Encoded frame rectangle on the animation canvas.
    pub rect: FrameRect,
    /// Exact presentation duration.
    pub duration: FrameDuration,
    /// Disposal operation after presentation.
    pub disposal: FrameDisposal,
    /// Composition operation used before presentation.
    pub blend: FrameBlend,
    /// Whether frame samples used GIF interlace storage order.
    pub interlaced: bool,
    /// Whether this frame is also the container's default still image.
    pub is_default_image: bool,
}

/// One decoded animation frame with explicit pixel and source semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFrame {
    /// Frame samples in the representation identified by `pixel_layout`.
    pub image: DecodedImage,
    /// Whether `image` is a source rectangle or a rendered canvas.
    pub pixel_layout: FramePixelLayout,
    /// Metadata retained from the encoded source frame.
    pub source: FrameSource,
}

impl DecodedFrame {
    /// Construct an uncomposited source-rectangle frame.
    #[must_use]
    pub fn source_rectangle(
        image: DecodedImage,
        left: u32,
        top: u32,
        duration: FrameDuration,
        disposal: FrameDisposal,
        blend: FrameBlend,
        interlaced: bool,
    ) -> Self {
        let rect = FrameRect {
            left,
            top,
            width: image.width,
            height: image.height,
        };
        Self {
            image,
            pixel_layout: FramePixelLayout::SourceRectangle,
            source: FrameSource {
                rect,
                duration,
                disposal,
                blend,
                interlaced,
                is_default_image: false,
            },
        }
    }

    /// Construct a rendered-canvas frame while retaining its source rectangle.
    #[must_use]
    pub fn rendered_canvas(
        image: DecodedImage,
        rect: FrameRect,
        duration: FrameDuration,
        disposal: FrameDisposal,
        blend: FrameBlend,
    ) -> Self {
        Self {
            image,
            pixel_layout: FramePixelLayout::RenderedCanvas,
            source: FrameSource {
                rect,
                duration,
                disposal,
                blend,
                interlaced: false,
                is_default_image: false,
            },
        }
    }
}

/// Background metadata retained from an animated image container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AnimationBackground {
    /// A GIF global color-table index.
    PaletteIndex(u8),
    /// A WebP animation canvas color in red, green, blue, alpha order.
    Rgba([u8; 4]),
}

/// Container meaning of a retained multi-image sequence.
///
/// The kind is part of the decoded contract: TIFF pages are untimed pages,
/// never timed animation, even when a page carries a zero or non-zero frame
/// duration in the common model. Caller-built sequences choose the variant
/// that describes their own contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SequenceKind {
    /// Frames form a timed animation (GIF, APNG, animated WebP, animated AVIF).
    TimedAnimation,
    /// Frames are untimed pages of one container (TIFF).
    UntimedPages,
    /// One retained frame from a source that defines no multi-image meaning
    /// (still decode fallback or a caller-built still sequence).
    SingleFrame,
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
    /// Container background metadata, when the source format defines it.
    pub background: Option<AnimationBackground>,
    /// Container meaning of this sequence.
    pub kind: SequenceKind,
    /// Opaque container blocks retained in original order.
    pub opaque_blocks: Vec<OpaqueBlock>,
    /// Known metadata retained in original order without semantic parsing.
    pub metadata: Vec<OpaqueMetadata>,
    /// Source color metadata retained from the encoded container.
    pub source_color: SourceColor,
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
            frames: vec![DecodedFrame::rendered_canvas(
                image,
                FrameRect {
                    left: 0,
                    top: 0,
                    width,
                    height,
                },
                FrameDuration::ZERO,
                FrameDisposal::Unspecified,
                FrameBlend::Unspecified,
            )],
            loop_count: None,
            background: None,
            kind: SequenceKind::SingleFrame,
            opaque_blocks: Vec::new(),
            metadata: Vec::new(),
            source_color: SourceColor::new(),
        }
    }

    /// Verify canvas, frame bounds, and each frame's sample layout.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::Dimensions`] for an empty or zero-sized sequence,
    /// an overflowing frame rectangle, or a frame outside the canvas. Also
    /// returns any validation error from a retained frame image.
    pub fn validate(&self) -> ImageResult<()> {
        if self.width == 0 || self.height == 0 || self.frames.is_empty() {
            return Err(ImageError::dimensions(
                "sequence canvas must be non-zero and contain a frame",
            ));
        }
        for frame in &self.frames {
            frame.image.validate()?;
            if frame.source.duration.denominator == 0 {
                return Err(ImageError::parameter(
                    "frame duration denominator must be non-zero",
                ));
            }
            if frame.source.rect.width == 0 || frame.source.rect.height == 0 {
                return Err(ImageError::dimensions(
                    "source frame rectangle must be non-zero",
                ));
            }
            match frame.pixel_layout {
                FramePixelLayout::SourceRectangle
                    if frame.image.width != frame.source.rect.width
                        || frame.image.height != frame.source.rect.height =>
                {
                    return Err(ImageError::dimensions(
                        "source-rectangle pixels do not match the source rectangle",
                    ));
                }
                FramePixelLayout::RenderedCanvas
                    if frame.image.width != self.width || frame.image.height != self.height =>
                {
                    return Err(ImageError::dimensions(
                        "rendered-frame pixels do not match the sequence canvas",
                    ));
                }
                _ => {}
            }
            let right = frame
                .source
                .rect
                .left
                .checked_add(frame.source.rect.width)
                .ok_or_else(|| ImageError::dimensions("frame right edge overflows"))?;
            let bottom = frame
                .source
                .rect
                .top
                .checked_add(frame.source.rect.height)
                .ok_or_else(|| ImageError::dimensions("frame bottom edge overflows"))?;
            if right > self.width || bottom > self.height {
                return Err(ImageError::dimensions(
                    "frame rectangle extends outside the sequence canvas",
                ));
            }
        }
        Ok(())
    }

    /// Return the first complete frame, or `None` before an empty caller-built
    /// sequence has been validated.
    #[must_use]
    pub fn first(&self) -> Option<&DecodedFrame> {
        self.frames.first()
    }

    /// Return only the first frame's image.
    ///
    /// This convenience intentionally discards source rectangle, timing,
    /// disposal, blend, interlace, and default-image metadata. Use
    /// [`Self::first`] when that state matters.
    #[must_use]
    pub fn first_image(&self) -> Option<&DecodedImage> {
        self.frames.first().map(|frame| &frame.image)
    }
}
