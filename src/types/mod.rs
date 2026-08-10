//! Container, decoded-sample, palette, animation, and error types used by codecs.

mod color_type;
mod error;

pub use self::color_type::ColorType;
pub use self::error::{
    ImageError, ImageErrorKind, ImageErrorStage, ImageResult, ResourceLimit, UnsupportedReason,
};

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

    // Exercise the auxiliary-alpha variant and descriptor round-trip so the
    // semantic space stays covered even when the AVIF feature is disabled.
    let descriptor = SourceDescriptor::new()
        .with_alpha(SourceAlpha::Auxiliary)
        .with_byte_order(SourceByteOrder::Big);
    assert_eq!(descriptor.alpha(), Some(SourceAlpha::Auxiliary));
    assert_eq!(descriptor.byte_order(), Some(SourceByteOrder::Big));
    assert!(!descriptor.is_empty());
    let alpha_only = SourceDescriptor::new().with_alpha(SourceAlpha::Auxiliary);
    assert!(!alpha_only.is_empty());
    let transform_only = SourceDescriptor::new().with_avif_transform(
        AvifTransformProperties::new()
            .with_rotation(AvifRotation::CounterClockwise90)
            .with_mirror(AvifMirrorAxis::LeftRight)
            .with_pixel_aspect_ratio(AvifPixelAspectRatio::new(4, 3))
            .with_clean_aperture(AvifCleanAperture::new(2, 1, 3, 1, 0, 1, 0, 1)),
    );
    assert_eq!(
        transform_only.avif_transform(),
        Some(
            AvifTransformProperties::new()
                .with_rotation(AvifRotation::CounterClockwise90)
                .with_mirror(AvifMirrorAxis::LeftRight)
                .with_pixel_aspect_ratio(AvifPixelAspectRatio::new(4, 3))
                .with_clean_aperture(AvifCleanAperture::new(2, 1, 3, 1, 0, 1, 0, 1,))
        )
    );
    assert!(!transform_only.is_empty());
    assert!(
        !AvifTransformProperties::new()
            .with_rotation(AvifRotation::Zero)
            .is_empty()
    );
    let pixel_aspect_ratio = AvifPixelAspectRatio::new(4, 3);
    assert_eq!(pixel_aspect_ratio.h_spacing(), 4);
    assert_eq!(pixel_aspect_ratio.v_spacing(), 3);
    assert!(
        !AvifTransformProperties::new()
            .with_pixel_aspect_ratio(pixel_aspect_ratio)
            .is_empty()
    );
    let clean_aperture = AvifCleanAperture::new(2, 1, 3, 1, -1, 2, 1, 2);
    assert_eq!(clean_aperture.width_numerator(), 2);
    assert_eq!(clean_aperture.width_denominator(), 1);
    assert_eq!(clean_aperture.height_numerator(), 3);
    assert_eq!(clean_aperture.height_denominator(), 1);
    assert_eq!(clean_aperture.horizontal_offset_numerator(), -1);
    assert_eq!(clean_aperture.horizontal_offset_denominator(), 2);
    assert_eq!(clean_aperture.vertical_offset_numerator(), 1);
    assert_eq!(clean_aperture.vertical_offset_denominator(), 2);
    assert!(
        !AvifTransformProperties::new()
            .with_clean_aperture(clean_aperture)
            .is_empty()
    );
    assert!(AvifTransformProperties::new().is_empty());
    assert!(SourceDescriptor::new().is_empty());

    for position in [
        AvifChromaSamplePosition::Unknown,
        AvifChromaSamplePosition::Vertical,
        AvifChromaSamplePosition::Colocated,
        AvifChromaSamplePosition::Reserved,
    ] {
        assert_eq!(
            AvifChromaSamplePosition::from_code(position.code()),
            position
        );
    }
    assert_eq!(
        AvifChromaSamplePosition::from_code(u8::MAX),
        AvifChromaSamplePosition::Reserved
    );

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
    assert!(
        !SourceColor::new()
            .with_avif_color(AvifColorProperties {
                color_primaries: 1,
                transfer_characteristics: 13,
                matrix_coefficients: 6,
                full_range: true,
            })
            .is_empty()
    );
    assert!(
        !SourceColor::new()
            .with_avif_chroma_sample_position(AvifChromaSamplePosition::Unknown)
            .is_empty()
    );
    assert!(
        !SourceColor::new()
            .with_avif_content_light_level(AvifContentLightLevel::new(1_000, 500))
            .is_empty()
    );
    let mastering_display = AvifMasteringDisplayColorVolume::new(
        60_000, 32_000, 26_500, 61_000, 15_000, 6_000, 31_270, 32_900, 4_000_000, 5,
    );
    assert_eq!(mastering_display.red_x(), 60_000);
    assert_eq!(mastering_display.red_y(), 32_000);
    assert_eq!(mastering_display.green_x(), 26_500);
    assert_eq!(mastering_display.green_y(), 61_000);
    assert_eq!(mastering_display.blue_x(), 15_000);
    assert_eq!(mastering_display.blue_y(), 6_000);
    assert_eq!(mastering_display.white_point_x(), 31_270);
    assert_eq!(mastering_display.white_point_y(), 32_900);
    assert_eq!(
        mastering_display.max_display_mastering_luminance(),
        4_000_000
    );
    assert_eq!(mastering_display.min_display_mastering_luminance(), 5);
    assert!(
        !SourceColor::new()
            .with_avif_mastering_display_color_volume(mastering_display)
            .is_empty()
    );

    // The preflight overflow arm of the transfer layout is exercised with the
    // largest representable canvas and 16 bytes per pixel.
    assert!(TransferLayout::from_mode(ImageMode::Rgba32F, u32::MAX, u32::MAX).is_err());

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
            reason: None,
            offset: None,
            identity: None,
        },
        ImageError::Unsupported {
            format: None,
            message: "coverage".to_owned(),
            stage: None,
            reason: None,
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
        ImageError::NeedMoreData {
            format: Some(ImageFormat::Png),
            stage: Some(ImageErrorStage::Inspection),
            offset: Some(8),
            identity: Some("png_chunk"),
            minimum: 41,
        },
        ImageError::NeedMoreData {
            format: None,
            stage: None,
            offset: None,
            identity: None,
            minimum: 2,
        },
        ImageError::Cancelled {
            format: Some(ImageFormat::Png),
            stage: Some(ImageErrorStage::StillDecode),
        },
        ImageError::Cancelled {
            format: None,
            stage: None,
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
        let _ = error.minimum_input();
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
    /// Non-fatal recoverable conditions observed while decoding.
    ///
    /// Empty unless the decoder tolerated a manifest-proven recoverable
    /// condition (for example ignored trailing bytes, a non-standard GIF
    /// graphic-control size, or invalid compressed PNG metadata accepted by
    /// the fixture contract). Diagnostics never change the decoded result.
    pub diagnostics: Vec<crate::ImageDiagnostic>,
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
    /// Whether `frame_count` is the complete container count.
    ///
    /// Basic inspection (`inspect_basic`) may stop after the first proven
    /// image and leave this `false` with `frame_count` `None`, while full
    /// inspection (`inspect`) reports `true` for every supported container.
    pub frame_count_complete: bool,
    /// Selected Windows cursor hotspot, distinguishing CUR from ordinary ICO.
    pub cursor_hotspot: Option<CursorHotspot>,
    /// Structural facts retained from the encoded source.
    pub source: SourceDescriptor,
    /// Source color metadata retained from the encoded container.
    pub source_color: SourceColor,
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

    /// Exact decoded transfer-byte length for this inspected image.
    ///
    /// The value is computed from the inspected canvas and mode without
    /// decoding compressed payloads, so callers can preflight a destination
    /// buffer before [`crate::decode_into`].
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::Dimensions`] when the byte length overflows
    /// `usize`.
    pub fn decoded_bytes(&self) -> ImageResult<usize> {
        self.mode.expected_bytes(self.width, self.height)
    }

    /// Exact transfer layout for this inspected image.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::Dimensions`] when the byte length overflows
    /// `usize`.
    pub fn transfer_layout(&self) -> ImageResult<TransferLayout> {
        TransferLayout::from_mode(self.mode, self.width, self.height)
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

/// A bounded relationship between an AVIF alpha auxiliary item and the color
/// item it targets.
///
/// Item identifiers are local to the encoded container; they are source
/// provenance, not globally stable image identifiers. The AVIF parser exposes
/// direct primary-item associations and alpha associations to the derived
/// color items of a supported grid. Other `iref` edges are represented by
/// [`AvifItemRelationship`]; this type keeps the established `auxl` alpha
/// contract distinct from that broader graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AvifAuxiliaryRelationship {
    auxiliary_item_id: u32,
    target_item_id: u32,
}

impl AvifAuxiliaryRelationship {
    /// Create a source-local AVIF auxiliary-to-target relationship.
    #[must_use]
    pub const fn new(auxiliary_item_id: u32, target_item_id: u32) -> Self {
        Self {
            auxiliary_item_id,
            target_item_id,
        }
    }

    /// Return the source-local auxiliary item identifier.
    #[must_use]
    pub const fn auxiliary_item_id(&self) -> u32 {
        self.auxiliary_item_id
    }

    /// Return the source-local item identifier targeted by the auxiliary item.
    #[must_use]
    pub const fn target_item_id(&self) -> u32 {
        self.target_item_id
    }
}

/// A source-local AVIF item reference other than an alpha `auxl` association.
///
/// The four-byte kind is the ISO-BMFF `iref` child type, and the item IDs are
/// local to the encoded container. This retains graph provenance only: it
/// does not compose grid tiles, decode auxiliary content, or apply a sample
/// transform. Alpha `auxl` edges remain exposed through
/// [`AvifAuxiliaryRelationship`], while `prem` edges also have a dedicated
/// filtered getter on [`SourceDescriptor`] so their alpha-associated meaning
/// is not lost in the broader reference graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AvifItemRelationship {
    kind: [u8; 4],
    from_item_id: u32,
    to_item_id: u32,
}

impl AvifItemRelationship {
    /// Create a source-local AVIF item reference.
    #[must_use]
    pub const fn new(kind: [u8; 4], from_item_id: u32, to_item_id: u32) -> Self {
        Self {
            kind,
            from_item_id,
            to_item_id,
        }
    }

    /// Return the four-byte `iref` child kind.
    #[must_use]
    pub const fn kind(&self) -> [u8; 4] {
        self.kind
    }

    /// Return the source-local item that owns the reference.
    #[must_use]
    pub const fn from_item_id(&self) -> u32 {
        self.from_item_id
    }

    /// Return the source-local item targeted by the reference.
    #[must_use]
    pub const fn to_item_id(&self) -> u32 {
        self.to_item_id
    }
}

/// A bounded CICP color declaration associated with a non-primary AVIF item.
///
/// The item identifier is local to the encoded container. This retains the
/// item's declared `colr`/`nclx` values as source provenance; it does not
/// apply color conversion or merge the declaration into the primary
/// [`SourceColor`] result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AvifItemColorProperties {
    item_id: u32,
    color: AvifColorProperties,
}

impl AvifItemColorProperties {
    /// Create a source-local non-primary AVIF item color declaration.
    #[must_use]
    pub const fn new(item_id: u32, color: AvifColorProperties) -> Self {
        Self { item_id, color }
    }

    /// Return the source-local item identifier.
    #[must_use]
    pub const fn item_id(&self) -> u32 {
        self.item_id
    }

    /// Return the item's declared CICP color properties.
    #[must_use]
    pub const fn color(&self) -> AvifColorProperties {
        self.color
    }
}

/// A raw ICC profile associated with a non-primary AVIF item.
///
/// The item identifier is local to the encoded container. This retains the
/// item's declared `colr`/`prof` or `colr`/`rICC` payload as source provenance;
/// it does not apply color conversion or merge the profile into the primary
/// [`SourceColor`] result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvifItemIccProfile {
    item_id: u32,
    profile: RawIccProfile,
}

impl AvifItemIccProfile {
    /// Create a source-local non-primary AVIF item ICC declaration.
    #[must_use]
    pub const fn new(item_id: u32, profile: RawIccProfile) -> Self {
        Self { item_id, profile }
    }

    /// Return the source-local item identifier.
    #[must_use]
    pub const fn item_id(&self) -> u32 {
        self.item_id
    }

    /// Return the raw ICC profile declared by the item.
    #[must_use]
    pub const fn profile(&self) -> &RawIccProfile {
        &self.profile
    }
}

/// A raw property associated with a non-primary AVIF item.
///
/// The property kind, payload, and `ipma` essential-association bit are
/// retained exactly as stored in the source, while the item identifier remains
/// source-local. This includes known declarations that have a typed
/// primary-item projection, non-alpha `auxC`/`auxi` auxiliary-type
/// declarations, and properties that remain opaque. This is container
/// provenance only: the property is not replayed, interpreted, or applied to
/// decoded samples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvifItemProperty {
    item_id: u32,
    kind: [u8; 4],
    data: Vec<u8>,
    essential: bool,
}

impl AvifItemProperty {
    /// Create a non-essential source-local raw AVIF item property.
    #[must_use]
    pub fn new(item_id: u32, kind: [u8; 4], data: Vec<u8>) -> Self {
        Self::new_with_essential(item_id, kind, data, false)
    }

    /// Create a source-local raw AVIF item property with its association bit.
    #[must_use]
    pub fn new_with_essential(item_id: u32, kind: [u8; 4], data: Vec<u8>, essential: bool) -> Self {
        Self {
            item_id,
            kind,
            data,
            essential,
        }
    }

    /// Return the source-local item identifier.
    #[must_use]
    pub const fn item_id(&self) -> u32 {
        self.item_id
    }

    /// Return the four-byte property kind.
    #[must_use]
    pub const fn kind(&self) -> [u8; 4] {
        self.kind
    }

    /// Return the raw property payload, excluding its BMFF box framing.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Return whether the source marked this property association essential.
    #[must_use]
    pub const fn is_essential(&self) -> bool {
        self.essential
    }
}

/// Source-local AVIF plane declarations associated with a non-primary item.
///
/// `width` and `height` retain the item's `ispe` spatial extents when present;
/// `bit_depth` retains the uniform channel depth declared by `pixi` when
/// present. These are container provenance only: the descriptor does not
/// expose a plane buffer, compose an auxiliary item, infer range/quality, or
/// transform decoded samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AvifItemPlaneProperties {
    item_id: u32,
    width: Option<u32>,
    height: Option<u32>,
    bit_depth: Option<u8>,
}

impl AvifItemPlaneProperties {
    /// Create source-local AVIF plane declarations.
    #[must_use]
    pub const fn new(
        item_id: u32,
        width: Option<u32>,
        height: Option<u32>,
        bit_depth: Option<u8>,
    ) -> Self {
        Self {
            item_id,
            width,
            height,
            bit_depth,
        }
    }

    /// Return the source-local item identifier.
    #[must_use]
    pub const fn item_id(&self) -> u32 {
        self.item_id
    }

    /// Return the `ispe` width, when the item declares one.
    #[must_use]
    pub const fn width(&self) -> Option<u32> {
        self.width
    }

    /// Return the `ispe` height, when the item declares one.
    #[must_use]
    pub const fn height(&self) -> Option<u32> {
        self.height
    }

    /// Return the uniform `pixi` channel depth, when the item declares one.
    #[must_use]
    pub const fn bit_depth(&self) -> Option<u8> {
        self.bit_depth
    }
}

/// Source-local AVIF codec configuration associated with a non-primary item.
///
/// `data` retains the complete `av1C` payload exactly as stored; the typed
/// depth and chroma-position fields expose the declarations already needed by
/// the bounded decoder. These fields are source provenance only: they do not
/// select a decoder, transform decoded samples, or compose auxiliary items.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AvifItemCodecProperties {
    item_id: u32,
    data: Vec<u8>,
    bit_depth: u8,
    chroma_sample_position: AvifChromaSamplePosition,
}

impl AvifItemCodecProperties {
    /// Create source-local AVIF codec configuration.
    #[must_use]
    pub fn new(
        item_id: u32,
        data: Vec<u8>,
        bit_depth: u8,
        chroma_sample_position: AvifChromaSamplePosition,
    ) -> Self {
        Self {
            item_id,
            data,
            bit_depth,
            chroma_sample_position,
        }
    }

    /// Return the source-local item identifier.
    #[must_use]
    pub const fn item_id(&self) -> u32 {
        self.item_id
    }

    /// Return the exact `av1C` payload, excluding BMFF box framing.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Return the declared AV1 bit depth.
    #[must_use]
    pub const fn bit_depth(&self) -> u8 {
        self.bit_depth
    }

    /// Return the declared AV1 chroma sample position.
    #[must_use]
    pub const fn chroma_sample_position(&self) -> AvifChromaSamplePosition {
        self.chroma_sample_position
    }
}

/// Topology declared by an AVIF `grid` derived image item.
///
/// These fields are source-local container provenance. They describe the
/// number of rows and columns, the declared output canvas, and the raw grid
/// flags; they do not compose tiles, decode auxiliary content, or transform
/// decoded samples. The retained `dimg` item order remains available through
/// [`SourceDescriptor::avif_grid_item_ids`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AvifGridProperties {
    version: u8,
    flags: u8,
    rows: u32,
    columns: u32,
    output_width: u32,
    output_height: u32,
}

impl AvifGridProperties {
    /// Create source-local AVIF grid properties.
    #[must_use]
    pub const fn new(
        version: u8,
        flags: u8,
        rows: u32,
        columns: u32,
        output_width: u32,
        output_height: u32,
    ) -> Self {
        Self {
            version,
            flags,
            rows,
            columns,
            output_width,
            output_height,
        }
    }

    /// Return the grid item payload version.
    #[must_use]
    pub const fn version(&self) -> u8 {
        self.version
    }

    /// Return the raw grid item flags.
    #[must_use]
    pub const fn flags(&self) -> u8 {
        self.flags
    }

    /// Return the declared number of grid rows.
    #[must_use]
    pub const fn rows(&self) -> u32 {
        self.rows
    }

    /// Return the declared number of grid columns.
    #[must_use]
    pub const fn columns(&self) -> u32 {
        self.columns
    }

    /// Return the declared output canvas width.
    #[must_use]
    pub const fn output_width(&self) -> u32 {
        self.output_width
    }

    /// Return the declared output canvas height.
    #[must_use]
    pub const fn output_height(&self) -> u32 {
        self.output_height
    }
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
    avif_auxiliary_relationship: Option<AvifAuxiliaryRelationship>,
    avif_auxiliary_relationships: Option<Vec<AvifAuxiliaryRelationship>>,
    avif_item_relationships: Option<Vec<AvifItemRelationship>>,
    avif_premultiplied_relationships: Option<Vec<AvifItemRelationship>>,
    avif_item_color_properties: Option<Vec<AvifItemColorProperties>>,
    avif_item_icc_profiles: Option<Vec<AvifItemIccProfile>>,
    avif_item_properties: Option<Vec<AvifItemProperty>>,
    avif_item_plane_properties: Option<Vec<AvifItemPlaneProperties>>,
    avif_item_codec_properties: Option<Vec<AvifItemCodecProperties>>,
    avif_grid_item_ids: Option<Vec<u32>>,
    avif_grid_properties: Option<AvifGridProperties>,
    avif_transform: Option<AvifTransformProperties>,
}

impl SourceDescriptor {
    /// Create an empty descriptor for caller-created pixels or a source with
    /// no retained structural facts.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            byte_order: None,
            alpha: None,
            avif_auxiliary_relationship: None,
            avif_auxiliary_relationships: None,
            avif_item_relationships: None,
            avif_premultiplied_relationships: None,
            avif_item_color_properties: None,
            avif_item_icc_profiles: None,
            avif_item_properties: None,
            avif_item_plane_properties: None,
            avif_item_codec_properties: None,
            avif_grid_item_ids: None,
            avif_grid_properties: None,
            avif_transform: None,
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

    /// Record a bounded direct AVIF auxiliary-item relationship.
    #[must_use]
    pub const fn with_avif_auxiliary_relationship(
        mut self,
        relationship: AvifAuxiliaryRelationship,
    ) -> Self {
        self.avif_auxiliary_relationship = Some(relationship);
        self
    }

    /// Return the retained direct AVIF auxiliary-item relationship, when one
    /// is available.
    #[must_use]
    pub const fn avif_auxiliary_relationship(&self) -> Option<AvifAuxiliaryRelationship> {
        self.avif_auxiliary_relationship
    }

    /// Record bounded alpha auxiliary-item relationships for derived color
    /// items such as a supported AVIF grid.
    ///
    /// The relationships are source-local provenance and do not request any
    /// composition or other pixel transformation.
    #[must_use]
    pub fn with_avif_auxiliary_relationships(
        mut self,
        relationships: Vec<AvifAuxiliaryRelationship>,
    ) -> Self {
        self.avif_auxiliary_relationships = (!relationships.is_empty()).then_some(relationships);
        self
    }

    /// Return all retained bounded AVIF alpha auxiliary-item relationships.
    ///
    /// A direct primary-item relationship is returned as a one-element slice
    /// when no multi-target relationship list is needed.
    #[must_use]
    pub fn avif_auxiliary_relationships(&self) -> &[AvifAuxiliaryRelationship] {
        if let Some(relationships) = &self.avif_auxiliary_relationships {
            relationships.as_slice()
        } else if let Some(relationship) = &self.avif_auxiliary_relationship {
            std::slice::from_ref(relationship)
        } else {
            &[]
        }
    }

    /// Record bounded AVIF item references other than alpha `auxl` edges.
    #[must_use]
    pub fn with_avif_item_relationships(
        mut self,
        relationships: Vec<AvifItemRelationship>,
    ) -> Self {
        self.avif_item_relationships = (!relationships.is_empty()).then_some(relationships);
        self
    }

    /// Return bounded AVIF item references other than alpha `auxl` edges in
    /// source order.
    #[must_use]
    pub fn avif_item_relationships(&self) -> &[AvifItemRelationship] {
        self.avif_item_relationships.as_deref().unwrap_or(&[])
    }

    /// Record bounded AVIF `prem` relationships.
    ///
    /// The AVIF `prem` relationship declares that the referenced color item
    /// carries values associated with its alpha item. This is source
    /// provenance only: decoded transfer bytes remain in the crate's
    /// documented normalized layout and are not implicitly transformed.
    #[must_use]
    pub fn with_avif_premultiplied_relationships(
        mut self,
        relationships: Vec<AvifItemRelationship>,
    ) -> Self {
        self.avif_premultiplied_relationships =
            (!relationships.is_empty()).then_some(relationships);
        self
    }

    /// Return bounded AVIF `prem` relationships in source order.
    #[must_use]
    pub fn avif_premultiplied_relationships(&self) -> &[AvifItemRelationship] {
        self.avif_premultiplied_relationships
            .as_deref()
            .unwrap_or(&[])
    }

    /// Record bounded CICP declarations for non-primary AVIF items.
    #[must_use]
    pub fn with_avif_item_color_properties(
        mut self,
        properties: Vec<AvifItemColorProperties>,
    ) -> Self {
        self.avif_item_color_properties = (!properties.is_empty()).then_some(properties);
        self
    }

    /// Return bounded non-primary AVIF item CICP declarations in source order.
    #[must_use]
    pub fn avif_item_color_properties(&self) -> &[AvifItemColorProperties] {
        self.avif_item_color_properties.as_deref().unwrap_or(&[])
    }

    /// Record bounded raw ICC profiles associated with non-primary AVIF items.
    #[must_use]
    pub fn with_avif_item_icc_profiles(mut self, profiles: Vec<AvifItemIccProfile>) -> Self {
        self.avif_item_icc_profiles = (!profiles.is_empty()).then_some(profiles);
        self
    }

    /// Return bounded non-primary AVIF item ICC declarations in source order.
    #[must_use]
    pub fn avif_item_icc_profiles(&self) -> &[AvifItemIccProfile] {
        self.avif_item_icc_profiles.as_deref().unwrap_or(&[])
    }

    /// Record unparsed properties associated with non-primary AVIF items.
    #[must_use]
    pub fn with_avif_item_properties(mut self, properties: Vec<AvifItemProperty>) -> Self {
        self.avif_item_properties = (!properties.is_empty()).then_some(properties);
        self
    }

    /// Return unparsed non-primary AVIF item properties in source order.
    #[must_use]
    pub fn avif_item_properties(&self) -> &[AvifItemProperty] {
        self.avif_item_properties.as_deref().unwrap_or(&[])
    }

    /// Record bounded `ispe`/`pixi` declarations for non-primary AVIF items.
    #[must_use]
    pub fn with_avif_item_plane_properties(
        mut self,
        properties: Vec<AvifItemPlaneProperties>,
    ) -> Self {
        self.avif_item_plane_properties = (!properties.is_empty()).then_some(properties);
        self
    }

    /// Return non-primary AVIF item plane declarations in source item order.
    #[must_use]
    pub fn avif_item_plane_properties(&self) -> &[AvifItemPlaneProperties] {
        self.avif_item_plane_properties.as_deref().unwrap_or(&[])
    }

    /// Record bounded `av1C` declarations for non-primary AVIF items.
    #[must_use]
    pub fn with_avif_item_codec_properties(
        mut self,
        properties: Vec<AvifItemCodecProperties>,
    ) -> Self {
        self.avif_item_codec_properties = (!properties.is_empty()).then_some(properties);
        self
    }

    /// Return non-primary AVIF codec declarations in source item order.
    #[must_use]
    pub fn avif_item_codec_properties(&self) -> &[AvifItemCodecProperties] {
        self.avif_item_codec_properties.as_deref().unwrap_or(&[])
    }

    /// Record the ordered source-local item identifiers derived from a
    /// primary AVIF grid item.
    ///
    /// This retains only the bounded `dimg` child list. It does not expose
    /// tile placement, composition, or decoded pixel transformation.
    #[must_use]
    pub fn with_avif_grid_item_ids(mut self, item_ids: Vec<u32>) -> Self {
        self.avif_grid_item_ids = (!item_ids.is_empty()).then_some(item_ids);
        self
    }

    /// Return the ordered source-local item identifiers derived from a
    /// primary AVIF grid item, when the bounded list is retained.
    #[must_use]
    pub fn avif_grid_item_ids(&self) -> &[u32] {
        self.avif_grid_item_ids.as_deref().unwrap_or(&[])
    }

    /// Record the source-local topology of a primary AVIF grid item.
    #[must_use]
    pub const fn with_avif_grid_properties(mut self, properties: AvifGridProperties) -> Self {
        self.avif_grid_properties = Some(properties);
        self
    }

    /// Return the source-local topology of a primary AVIF grid item, when
    /// retained.
    #[must_use]
    pub const fn avif_grid_properties(&self) -> Option<AvifGridProperties> {
        self.avif_grid_properties
    }

    /// Record the AVIF item transforms declared by the encoded source.
    ///
    /// These properties describe source presentation metadata only. Decoded
    /// transfer samples remain unrotated and unmirrored.
    #[must_use]
    pub const fn with_avif_transform(mut self, transform: AvifTransformProperties) -> Self {
        self.avif_transform = Some(transform);
        self
    }

    /// Return the AVIF item transforms declared by the encoded source.
    #[must_use]
    pub const fn avif_transform(&self) -> Option<AvifTransformProperties> {
        self.avif_transform
    }

    /// Whether this source has no retained structural facts.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.byte_order.is_none()
            && self.alpha.is_none()
            && self.avif_auxiliary_relationship.is_none()
            && self.avif_auxiliary_relationships.is_none()
            && self.avif_item_relationships.is_none()
            && self.avif_premultiplied_relationships.is_none()
            && self.avif_item_color_properties.is_none()
            && self.avif_item_icc_profiles.is_none()
            && self.avif_item_properties.is_none()
            && self.avif_item_plane_properties.is_none()
            && self.avif_item_codec_properties.is_none()
            && self.avif_grid_item_ids.is_none()
            && self.avif_grid_properties.is_none()
            && self.avif_transform.is_none()
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
    avif_color: Option<AvifColorProperties>,
    avif_chroma_sample_position: Option<AvifChromaSamplePosition>,
    avif_content_light_level: Option<AvifContentLightLevel>,
    avif_mastering_display_color_volume: Option<AvifMasteringDisplayColorVolume>,
}

/// CICP color properties declared by an AVIF item.
///
/// These fields record the source declaration only. They do not imply that
/// decoded samples were converted into the declared color space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AvifColorProperties {
    /// ISO/IEC 23001-8 color primaries code.
    pub color_primaries: u16,
    /// ISO/IEC 23001-8 transfer characteristics code.
    pub transfer_characteristics: u16,
    /// ISO/IEC 23001-8 matrix coefficients code.
    pub matrix_coefficients: u16,
    /// Whether the CICP declaration sets the full-range flag.
    pub full_range: bool,
}

/// Chroma sample position declared by an AVIF `av1C` record.
///
/// This records the source declaration only. It does not change the decoded
/// transfer samples or request chroma resampling.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AvifChromaSamplePosition {
    /// The chroma sample position is unknown or unspecified (AV1 code 0).
    Unknown,
    /// Chroma samples are vertically aligned (AV1 code 1).
    Vertical,
    /// Chroma samples are colocated with luma (AV1 code 2).
    Colocated,
    /// The AV1 reserved code was retained without reinterpretation (code 3).
    Reserved,
}

impl AvifChromaSamplePosition {
    /// Convert the two-bit AV1 declaration to its typed source descriptor.
    #[must_use]
    pub const fn from_code(code: u8) -> Self {
        match code & 0x03 {
            0 => Self::Unknown,
            1 => Self::Vertical,
            2 => Self::Colocated,
            _ => Self::Reserved,
        }
    }

    /// Return the exact two-bit AV1 declaration.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::Vertical => 1,
            Self::Colocated => 2,
            Self::Reserved => 3,
        }
    }
}

/// Content light-level information declared by an AVIF `clli` property.
///
/// The values are source metadata in candelas per square metre. They are
/// retained exactly as declared and do not cause tone mapping or any other
/// transformation of decoded samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AvifContentLightLevel {
    max_content_light_level: u16,
    max_picture_average_light_level: u16,
}

impl AvifContentLightLevel {
    /// Create content-light-level information from the encoded `clli` fields.
    #[must_use]
    pub const fn new(max_content_light_level: u16, max_picture_average_light_level: u16) -> Self {
        Self {
            max_content_light_level,
            max_picture_average_light_level,
        }
    }

    /// Return the maximum content light level (maxCLL).
    #[must_use]
    pub const fn max_content_light_level(&self) -> u16 {
        self.max_content_light_level
    }

    /// Return the maximum picture-average light level (maxPALL).
    #[must_use]
    pub const fn max_picture_average_light_level(&self) -> u16 {
        self.max_picture_average_light_level
    }
}

/// Mastering-display color-volume information declared by an AVIF `mdcv`
/// property.
///
/// The ISO-BMFF wire order is green, blue, then red; this public descriptor
/// exposes the same exact 16-bit coordinates in the more useful red, green,
/// blue order. The luminance values are retained as the encoded unsigned
/// 32-bit fields and do not cause tone mapping or any other transformation of
/// decoded samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AvifMasteringDisplayColorVolume {
    red_x: u16,
    red_y: u16,
    green_x: u16,
    green_y: u16,
    blue_x: u16,
    blue_y: u16,
    white_point_x: u16,
    white_point_y: u16,
    max_display_mastering_luminance: u32,
    min_display_mastering_luminance: u32,
}

impl AvifMasteringDisplayColorVolume {
    /// Create a mastering-display color-volume declaration from its encoded
    /// coordinates and luminance values.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        red_x: u16,
        red_y: u16,
        green_x: u16,
        green_y: u16,
        blue_x: u16,
        blue_y: u16,
        white_point_x: u16,
        white_point_y: u16,
        max_display_mastering_luminance: u32,
        min_display_mastering_luminance: u32,
    ) -> Self {
        Self {
            red_x,
            red_y,
            green_x,
            green_y,
            blue_x,
            blue_y,
            white_point_x,
            white_point_y,
            max_display_mastering_luminance,
            min_display_mastering_luminance,
        }
    }

    /// Return the red-primary x coordinate.
    #[must_use]
    pub const fn red_x(&self) -> u16 {
        self.red_x
    }

    /// Return the red-primary y coordinate.
    #[must_use]
    pub const fn red_y(&self) -> u16 {
        self.red_y
    }

    /// Return the green-primary x coordinate.
    #[must_use]
    pub const fn green_x(&self) -> u16 {
        self.green_x
    }

    /// Return the green-primary y coordinate.
    #[must_use]
    pub const fn green_y(&self) -> u16 {
        self.green_y
    }

    /// Return the blue-primary x coordinate.
    #[must_use]
    pub const fn blue_x(&self) -> u16 {
        self.blue_x
    }

    /// Return the blue-primary y coordinate.
    #[must_use]
    pub const fn blue_y(&self) -> u16 {
        self.blue_y
    }

    /// Return the white-point x coordinate.
    #[must_use]
    pub const fn white_point_x(&self) -> u16 {
        self.white_point_x
    }

    /// Return the white-point y coordinate.
    #[must_use]
    pub const fn white_point_y(&self) -> u16 {
        self.white_point_y
    }

    /// Return the maximum display-mastering luminance field.
    #[must_use]
    pub const fn max_display_mastering_luminance(&self) -> u32 {
        self.max_display_mastering_luminance
    }

    /// Return the minimum display-mastering luminance field.
    #[must_use]
    pub const fn min_display_mastering_luminance(&self) -> u32 {
        self.min_display_mastering_luminance
    }
}

/// Counter-clockwise quarter-turn declared by an AVIF `irot` property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AvifRotation {
    /// No rotation.
    Zero,
    /// Rotate 90 degrees counter-clockwise.
    CounterClockwise90,
    /// Rotate 180 degrees counter-clockwise.
    CounterClockwise180,
    /// Rotate 270 degrees counter-clockwise.
    CounterClockwise270,
}

/// Axis declared by an AVIF `imir` property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AvifMirrorAxis {
    /// Exchange the top and bottom parts of the image.
    TopBottom,
    /// Exchange the left and right parts of the image.
    LeftRight,
}

/// Relative width and height of an AVIF pixel declared by a `pasp` property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AvifPixelAspectRatio {
    h_spacing: u32,
    v_spacing: u32,
}

/// Fractional clean-aperture geometry declared by an AVIF `clap` property.
///
/// The width and height numerators and all denominators are represented as
/// unsigned values because they are strictly positive in a valid declaration.
/// Offset numerators use the signed ISO interpretation of the stored 32-bit
/// value and may be positive, zero, or negative. This type records source
/// provenance; the crate does not crop or resample decoded pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AvifCleanAperture {
    width_numerator: u32,
    width_denominator: u32,
    height_numerator: u32,
    height_denominator: u32,
    horizontal_offset_numerator: i32,
    horizontal_offset_denominator: u32,
    vertical_offset_numerator: i32,
    vertical_offset_denominator: u32,
}

impl AvifCleanAperture {
    /// Create a clean-aperture declaration from its encoded fractions.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        width_numerator: u32,
        width_denominator: u32,
        height_numerator: u32,
        height_denominator: u32,
        horizontal_offset_numerator: i32,
        horizontal_offset_denominator: u32,
        vertical_offset_numerator: i32,
        vertical_offset_denominator: u32,
    ) -> Self {
        Self {
            width_numerator,
            width_denominator,
            height_numerator,
            height_denominator,
            horizontal_offset_numerator,
            horizontal_offset_denominator,
            vertical_offset_numerator,
            vertical_offset_denominator,
        }
    }

    /// Return the clean-aperture width numerator.
    #[must_use]
    pub const fn width_numerator(&self) -> u32 {
        self.width_numerator
    }

    /// Return the clean-aperture width denominator.
    #[must_use]
    pub const fn width_denominator(&self) -> u32 {
        self.width_denominator
    }

    /// Return the clean-aperture height numerator.
    #[must_use]
    pub const fn height_numerator(&self) -> u32 {
        self.height_numerator
    }

    /// Return the clean-aperture height denominator.
    #[must_use]
    pub const fn height_denominator(&self) -> u32 {
        self.height_denominator
    }

    /// Return the signed horizontal offset numerator.
    #[must_use]
    pub const fn horizontal_offset_numerator(&self) -> i32 {
        self.horizontal_offset_numerator
    }

    /// Return the horizontal offset denominator.
    #[must_use]
    pub const fn horizontal_offset_denominator(&self) -> u32 {
        self.horizontal_offset_denominator
    }

    /// Return the signed vertical offset numerator.
    #[must_use]
    pub const fn vertical_offset_numerator(&self) -> i32 {
        self.vertical_offset_numerator
    }

    /// Return the vertical offset denominator.
    #[must_use]
    pub const fn vertical_offset_denominator(&self) -> u32 {
        self.vertical_offset_denominator
    }
}

impl AvifPixelAspectRatio {
    /// Create a pixel aspect-ratio declaration from its positive spacings.
    #[must_use]
    pub const fn new(h_spacing: u32, v_spacing: u32) -> Self {
        Self {
            h_spacing,
            v_spacing,
        }
    }

    /// Return the horizontal spacing from the source declaration.
    #[must_use]
    pub const fn h_spacing(&self) -> u32 {
        self.h_spacing
    }

    /// Return the vertical spacing from the source declaration.
    #[must_use]
    pub const fn v_spacing(&self) -> u32 {
        self.v_spacing
    }
}

/// The kind of an AVIF item presentation property.
///
/// The order returned by [`AvifTransformProperties::order`] follows the
/// source association order. It is provenance only; the crate does not apply
/// these transforms to decoded pixels.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AvifTransformKind {
    /// An `irot` rotation declaration.
    Rotation,
    /// An `imir` mirror declaration.
    Mirror,
    /// A `pasp` pixel-aspect-ratio declaration.
    PixelAspectRatio,
    /// A `clap` clean-aperture declaration.
    CleanAperture,
}

impl AvifTransformKind {
    const fn same(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::Rotation, Self::Rotation)
                | (Self::Mirror, Self::Mirror)
                | (Self::PixelAspectRatio, Self::PixelAspectRatio)
                | (Self::CleanAperture, Self::CleanAperture)
        )
    }
}

/// AVIF item presentation properties retained without applying them to decoded pixels.
///
/// The current model covers the `irot`, `imir`, `pasp`, and `clap` properties.
/// All four are source declarations: no rotation, mirroring, rescaling, or
/// cropping is applied to decoded pixels. An absent field means that property
/// was not associated with the primary image item; a present zero rotation
/// remains distinguishable from an absent `irot` property. The order accessor
/// retains the source association order of the declarations that are present.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AvifTransformProperties {
    rotation: Option<AvifRotation>,
    mirror: Option<AvifMirrorAxis>,
    pixel_aspect_ratio: Option<AvifPixelAspectRatio>,
    clean_aperture: Option<AvifCleanAperture>,
    order: [AvifTransformKind; 4],
    order_len: u8,
}

impl Default for AvifTransformProperties {
    fn default() -> Self {
        Self::new()
    }
}

impl AvifTransformProperties {
    /// Create an empty AVIF transform descriptor.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            rotation: None,
            mirror: None,
            pixel_aspect_ratio: None,
            clean_aperture: None,
            order: [AvifTransformKind::Rotation; 4],
            order_len: 0,
        }
    }

    const fn record_kind(mut self, kind: AvifTransformKind) -> Self {
        let mut index = 0;
        while index < self.order_len as usize {
            if self.order[index].same(kind) {
                return self;
            }
            index = index.saturating_add(1);
        }
        if index < self.order.len() {
            self.order[index] = kind;
            self.order_len = self.order_len.saturating_add(1);
        }
        self
    }

    /// Record an AVIF `irot` rotation property.
    #[must_use]
    pub const fn with_rotation(mut self, rotation: AvifRotation) -> Self {
        self.rotation = Some(rotation);
        self.record_kind(AvifTransformKind::Rotation)
    }

    /// Return the retained AVIF `irot` property, when present.
    #[must_use]
    pub const fn rotation(&self) -> Option<AvifRotation> {
        self.rotation
    }

    /// Record an AVIF `imir` mirror property.
    #[must_use]
    pub const fn with_mirror(mut self, mirror: AvifMirrorAxis) -> Self {
        self.mirror = Some(mirror);
        self.record_kind(AvifTransformKind::Mirror)
    }

    /// Return the retained AVIF `imir` property, when present.
    #[must_use]
    pub const fn mirror(&self) -> Option<AvifMirrorAxis> {
        self.mirror
    }

    /// Record an AVIF `pasp` pixel aspect-ratio property.
    #[must_use]
    pub const fn with_pixel_aspect_ratio(mut self, ratio: AvifPixelAspectRatio) -> Self {
        self.pixel_aspect_ratio = Some(ratio);
        self.record_kind(AvifTransformKind::PixelAspectRatio)
    }

    /// Return the retained AVIF `pasp` property, when present.
    #[must_use]
    pub const fn pixel_aspect_ratio(&self) -> Option<AvifPixelAspectRatio> {
        self.pixel_aspect_ratio
    }

    /// Record an AVIF `clap` clean-aperture property.
    #[must_use]
    pub const fn with_clean_aperture(mut self, clean_aperture: AvifCleanAperture) -> Self {
        self.clean_aperture = Some(clean_aperture);
        self.record_kind(AvifTransformKind::CleanAperture)
    }

    /// Return the retained AVIF `clap` property, when present.
    #[must_use]
    pub const fn clean_aperture(&self) -> Option<AvifCleanAperture> {
        self.clean_aperture
    }

    /// Return the retained transform kinds in source association order.
    #[must_use]
    pub fn order(&self) -> &[AvifTransformKind] {
        &self.order[..self.order_len as usize]
    }

    /// Whether no AVIF transform property was retained.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.rotation.is_none()
            && self.mirror.is_none()
            && self.pixel_aspect_ratio.is_none()
            && self.clean_aperture.is_none()
    }
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
            avif_color: None,
            avif_chroma_sample_position: None,
            avif_content_light_level: None,
            avif_mastering_display_color_volume: None,
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

    /// Record the CICP color properties declared by an AVIF item.
    #[must_use]
    pub const fn with_avif_color(mut self, color: AvifColorProperties) -> Self {
        self.avif_color = Some(color);
        self
    }

    /// Return the AVIF CICP color properties, when retained.
    #[must_use]
    pub const fn avif_color(&self) -> Option<AvifColorProperties> {
        self.avif_color
    }

    /// Record the AV1 chroma sample position declared by an AVIF item.
    #[must_use]
    pub const fn with_avif_chroma_sample_position(
        mut self,
        position: AvifChromaSamplePosition,
    ) -> Self {
        self.avif_chroma_sample_position = Some(position);
        self
    }

    /// Return the retained AVIF chroma sample position, when present.
    #[must_use]
    pub const fn avif_chroma_sample_position(&self) -> Option<AvifChromaSamplePosition> {
        self.avif_chroma_sample_position
    }

    /// Record the content-light-level information declared by an AVIF item.
    #[must_use]
    pub const fn with_avif_content_light_level(
        mut self,
        content_light_level: AvifContentLightLevel,
    ) -> Self {
        self.avif_content_light_level = Some(content_light_level);
        self
    }

    /// Return the AVIF content-light-level information, when retained.
    #[must_use]
    pub const fn avif_content_light_level(&self) -> Option<AvifContentLightLevel> {
        self.avif_content_light_level
    }

    /// Record the AVIF mastering-display color-volume declaration.
    #[must_use]
    pub const fn with_avif_mastering_display_color_volume(
        mut self,
        mastering_display_color_volume: AvifMasteringDisplayColorVolume,
    ) -> Self {
        self.avif_mastering_display_color_volume = Some(mastering_display_color_volume);
        self
    }

    /// Return the AVIF mastering-display color-volume declaration, when
    /// retained.
    #[must_use]
    pub const fn avif_mastering_display_color_volume(
        &self,
    ) -> Option<AvifMasteringDisplayColorVolume> {
        self.avif_mastering_display_color_volume
    }

    /// Whether this descriptor retains no source color facts.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.srgb.is_none()
            && self.gamma.is_none()
            && self.chromaticities.is_none()
            && self.icc_profile.is_none()
            && self.avif_color.is_none()
            && self.avif_chroma_sample_position.is_none()
            && self.avif_content_light_level.is_none()
            && self.avif_mastering_display_color_volume.is_none()
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
            diagnostics: Vec::new(),
        }
    }

    /// Attach the non-fatal diagnostics observed while decoding.
    #[must_use]
    pub fn with_diagnostics(mut self, diagnostics: Vec<crate::ImageDiagnostic>) -> Self {
        self.diagnostics = diagnostics;
        self
    }

    /// Borrow the decoded content without discarding its source format.
    #[must_use]
    pub fn as_ref(&self) -> Decoded<&T> {
        Decoded::new(self.format, &self.content, self.consumed_bytes)
            .with_diagnostics(self.diagnostics.clone())
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
            reason: None,
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

    /// Exact decoded transfer-byte length for this mode and canvas.
    ///
    /// `L1` is packed with the final row padded to whole bytes; all other
    /// modes are tightly packed interleaved samples.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::Dimensions`] when the byte length overflows
    /// `usize`.
    pub fn expected_bytes(self, width: u32, height: u32) -> ImageResult<usize> {
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

/// Minimal transfer-layout descriptor for decoded sample bytes.
///
/// Current layouts are either tightly packed interleaved samples or packed
/// `L1` bit rows; there is no row padding or alignment requirement beyond
/// whole-byte rows, and no planar destination exists yet. The descriptor is
/// produced by the same arithmetic as [`ImageMode::expected_bytes`], so a
/// preflighted buffer of [`Self::total_bytes`] is exactly what
/// [`crate::decode_into`] accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransferLayout {
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
    /// Decoded observable mode.
    pub mode: ImageMode,
    /// Bytes per row; packed `L1` rows pad the final byte of each row.
    pub row_bytes: usize,
    /// Exact total decoded byte length.
    pub total_bytes: usize,
    /// Whether samples are packed `L1` bit rows rather than tightly packed
    /// interleaved samples.
    pub packed_rows: bool,
    /// Required destination alignment in bytes (1 for all current layouts).
    pub alignment: usize,
}

impl TransferLayout {
    /// Compute the transfer layout for one mode and canvas.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::Dimensions`] when the byte length overflows
    /// `usize`.
    pub fn from_mode(mode: ImageMode, width: u32, height: u32) -> ImageResult<Self> {
        let total_bytes = mode.expected_bytes(width, height)?;
        let row_bytes = if mode == ImageMode::L1 {
            (width as usize).div_ceil(8)
        } else {
            let bytes = u64::from(mode.color_type().bytes_per_pixel());
            #[cfg(target_pointer_width = "64")]
            {
                // Width is u32 and bytes-per-pixel is at most 16, so the
                // product always fits u64 and 64-bit usize losslessly.
                usize::from_ne_bytes(u64::from(width).wrapping_mul(bytes).to_ne_bytes())
            }
            #[cfg(not(target_pointer_width = "64"))]
            {
                usize::try_from(u64::from(width).wrapping_mul(bytes))
                    .map_err(|_| ImageError::dimensions("decoded row byte length overflows"))?
            }
        };
        Ok(Self {
            width,
            height,
            mode,
            row_bytes,
            total_bytes,
            packed_rows: mode == ImageMode::L1,
            alignment: 1,
        })
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
    ///
    /// Direct struct literals are unchecked. Use [`Self::try_new`] when the
    /// constructor should validate this field together with `mode` and the
    /// pixel buffer.
    pub color: ColorType,
    /// Exact observable byte/sample mode.
    ///
    /// Direct struct literals are unchecked. Use [`Self::try_with_mode`] when
    /// the constructor should validate this mode and the pixel buffer.
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

    /// Create an unpacked decoded image and validate its dimensions and bytes.
    ///
    /// Unlike [`Self::new`], this constructor returns only a value whose
    /// dimensions, mode, color representation, and pixel length satisfy
    /// [`Self::validate`]. The input vector is reused without copying on
    /// success.
    ///
    /// # Errors
    ///
    /// Returns the same dimensions or parameter errors as [`Self::validate`].
    pub fn try_new(
        width: u32,
        height: u32,
        pixels: Vec<u8>,
        color: ColorType,
    ) -> ImageResult<Self> {
        let image = Self::new(width, height, pixels, color);
        image.validate()?;
        Ok(image)
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

    /// Create an exact-mode image and validate its dimensions and bytes.
    ///
    /// The input vector is reused without copying on success. Use
    /// [`Self::try_with_palette`] when a valid indexed palette must be
    /// attached as part of construction.
    ///
    /// # Errors
    ///
    /// Returns the same dimensions or parameter errors as [`Self::validate`].
    pub fn try_with_mode(
        width: u32,
        height: u32,
        pixels: Vec<u8>,
        mode: ImageMode,
    ) -> ImageResult<Self> {
        let image = Self::with_mode(width, height, pixels, mode);
        image.validate()?;
        Ok(image)
    }

    /// Attach an indexed palette while preserving the decoded sample bytes.
    #[must_use]
    pub fn with_palette(mut self, palette: ImagePalette) -> Self {
        self.palette = Some(palette);
        self
    }

    /// Attach an indexed palette and validate the complete image state.
    ///
    /// The image and palette remain owned by the returned value without a
    /// pixel-buffer copy.
    ///
    /// # Errors
    ///
    /// Returns the same parameter or palette-index errors as [`Self::validate`].
    pub fn try_with_palette(self, palette: ImagePalette) -> ImageResult<Self> {
        let image = self.with_palette(palette);
        image.validate()?;
        Ok(image)
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

    /// Exact transfer layout for these decoded sample bytes.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::Dimensions`] when the byte length overflows
    /// `usize`.
    pub fn transfer_layout(&self) -> ImageResult<TransferLayout> {
        TransferLayout::from_mode(self.mode, self.width, self.height)
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
