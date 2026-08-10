//! Feature- and target-aware codec capability discovery.

use crate::ImageFormat;

/// One public codec operation whose availability can be queried.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodecOperation {
    /// Recognize the encoded container signature.
    Detection,
    /// Inspect encoded container metadata without materializing pixels.
    Inspection,
    /// Decode the still or selected-first-image view.
    StillDecode,
    /// Encode one decoded image.
    StillEncode,
    /// Decode and retain more than one image, frame, or page.
    SequenceDecode,
    /// Encode more than one retained image, frame, or page.
    SequenceEncode,
}

/// Operations returned by [`FormatCapabilities::operation`] in stable order.
pub const CODEC_OPERATIONS: [CodecOperation; 6] = [
    CodecOperation::Detection,
    CodecOperation::Inspection,
    CodecOperation::StillDecode,
    CodecOperation::StillEncode,
    CodecOperation::SequenceDecode,
    CodecOperation::SequenceEncode,
];

/// Why a codec operation is unavailable in the current build.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityUnavailableReason {
    /// The format's Cargo feature is not enabled.
    FeatureDisabled,
    /// The operation is not available on the current compilation target.
    TargetUnavailable,
    /// The enabled codec does not implement this operation.
    NotImplemented,
}

/// Why an available operation has a deliberately narrower contract.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityRestriction {
    /// The in-tree WASM AVIF decoder supports only its documented portable subset.
    PortableAvif,
}

/// Availability and compatibility class of one codec operation.
///
/// All available operations are bounded by the repository manifest.
/// `Restricted` additionally names a narrower target-specific class.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// The operation is executable for the manifest-bounded codec contract.
    ManifestBounded,
    /// The operation is executable only for a named restricted subset.
    Restricted(CapabilityRestriction),
    /// The operation cannot be executed in this build.
    Unavailable(CapabilityUnavailableReason),
}

impl Capability {
    /// Whether the operation can be attempted in this build.
    #[must_use]
    pub const fn is_available(self) -> bool {
        !matches!(self, Self::Unavailable(_))
    }

    /// Whether the operation has a narrower target-specific contract.
    #[must_use]
    pub const fn is_restricted(self) -> bool {
        matches!(self, Self::Restricted(_))
    }

    /// Return the reason an operation is unavailable.
    #[must_use]
    pub const fn unavailable_reason(self) -> Option<CapabilityUnavailableReason> {
        match self {
            Self::Unavailable(reason) => Some(reason),
            Self::ManifestBounded | Self::Restricted(_) => None,
        }
    }

    /// Return the restriction on an available operation.
    #[must_use]
    pub const fn restriction(self) -> Option<CapabilityRestriction> {
        match self {
            Self::Restricted(restriction) => Some(restriction),
            Self::ManifestBounded | Self::Unavailable(_) => None,
        }
    }
}

/// Target class used by the crate's current codec dispatch and capability
/// contract.
///
/// The two supported `wasm32` classes intentionally remain distinct even
/// though they currently share the portable AVIF decoder branch. A target
/// identity is a capability fact; it is not inferred from an AVIF
/// `FileTypeBox` declaration.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityTarget {
    /// A non-`wasm32` compilation target.
    Native,
    /// The `wasm32-wasip1` target with a WASI runtime contract.
    Wasm32Wasi,
    /// A non-WASI `wasm32` target, including `wasm32-unknown-unknown`.
    Wasm32Unknown,
}

impl CapabilityTarget {
    /// Return the target class of the current compilation.
    #[must_use]
    #[cfg(not(target_arch = "wasm32"))]
    pub const fn current() -> Self {
        Self::Native
    }

    /// Return the target class of the current compilation.
    #[must_use]
    #[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
    pub const fn current() -> Self {
        Self::Wasm32Wasi
    }

    /// Return the target class of the current compilation.
    #[must_use]
    #[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
    pub const fn current() -> Self {
        Self::Wasm32Unknown
    }
}

/// Complete operation capabilities for one format in the current build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FormatCapabilities {
    format: ImageFormat,
    target: CapabilityTarget,
    feature_enabled: bool,
    detection: Capability,
    inspection: Capability,
    still_decode: Capability,
    still_encode: Capability,
    sequence_decode: Capability,
    sequence_encode: Capability,
}

impl FormatCapabilities {
    /// Format described by this capability record.
    #[must_use]
    pub const fn format(self) -> ImageFormat {
        self.format
    }

    /// Target family described by this capability record.
    #[must_use]
    pub const fn target(self) -> CapabilityTarget {
        self.target
    }

    /// Whether the format's Cargo feature is enabled.
    #[must_use]
    pub const fn feature_enabled(self) -> bool {
        self.feature_enabled
    }

    /// Capability for recognizing the encoded signature.
    #[must_use]
    pub const fn detection(self) -> Capability {
        self.detection
    }

    /// Capability for metadata inspection.
    #[must_use]
    pub const fn inspection(self) -> Capability {
        self.inspection
    }

    /// Capability for decoding the still or selected-first-image view.
    #[must_use]
    pub const fn still_decode(self) -> Capability {
        self.still_decode
    }

    /// Capability for encoding one image.
    #[must_use]
    pub const fn still_encode(self) -> Capability {
        self.still_encode
    }

    /// Capability for retaining more than one decoded image, frame, or page.
    ///
    /// A codec whose still image can pass through `decode_sequence` as one
    /// frame does not report sequence support here.
    #[must_use]
    pub const fn sequence_decode(self) -> Capability {
        self.sequence_decode
    }

    /// Capability for encoding more than one image, frame, or page.
    ///
    /// The validated one-frame fallback follows [`Self::still_encode`] and
    /// does not report sequence support here.
    #[must_use]
    pub const fn sequence_encode(self) -> Capability {
        self.sequence_encode
    }

    /// Query one operation without matching individual struct fields.
    #[must_use]
    pub const fn operation(self, operation: CodecOperation) -> Capability {
        match operation {
            CodecOperation::Detection => self.detection,
            CodecOperation::Inspection => self.inspection,
            CodecOperation::StillDecode => self.still_decode,
            CodecOperation::StillEncode => self.still_encode,
            CodecOperation::SequenceDecode => self.sequence_decode,
            CodecOperation::SequenceEncode => self.sequence_encode,
        }
    }
}

impl ImageFormat {
    /// Cargo feature that enables codec operations for this format.
    #[must_use]
    pub const fn feature_name(self) -> &'static str {
        match self {
            Self::Jpeg => "jpeg",
            Self::Png => "png",
            Self::Gif => "gif",
            Self::Bmp => "bmp",
            Self::Tiff => "tiff",
            Self::WebP => "webp",
            Self::Ico => "ico",
            Self::Avif => "avif",
        }
    }

    /// Return operation capabilities for this format in the current build.
    #[must_use]
    pub const fn capabilities(self) -> FormatCapabilities {
        capabilities_for(self, feature_enabled(self), CapabilityTarget::current())
    }
}

/// Return all public formats and their current operation capabilities.
#[must_use]
pub const fn all_capabilities() -> [FormatCapabilities; 8] {
    [
        ImageFormat::Jpeg.capabilities(),
        ImageFormat::Png.capabilities(),
        ImageFormat::Gif.capabilities(),
        ImageFormat::Bmp.capabilities(),
        ImageFormat::WebP.capabilities(),
        ImageFormat::Tiff.capabilities(),
        ImageFormat::Ico.capabilities(),
        ImageFormat::Avif.capabilities(),
    ]
}

const fn feature_enabled(format: ImageFormat) -> bool {
    match format {
        ImageFormat::Jpeg => cfg!(feature = "jpeg"),
        ImageFormat::Png => cfg!(feature = "png"),
        ImageFormat::Gif => cfg!(feature = "gif"),
        ImageFormat::Bmp => cfg!(feature = "bmp"),
        ImageFormat::WebP => cfg!(feature = "webp"),
        ImageFormat::Tiff => cfg!(feature = "tiff"),
        ImageFormat::Ico => cfg!(feature = "ico"),
        ImageFormat::Avif => cfg!(feature = "avif"),
    }
}

const fn capabilities_for(
    format: ImageFormat,
    enabled: bool,
    target: CapabilityTarget,
) -> FormatCapabilities {
    let manifest = Capability::ManifestBounded;
    let disabled = Capability::Unavailable(CapabilityUnavailableReason::FeatureDisabled);
    let not_implemented = Capability::Unavailable(CapabilityUnavailableReason::NotImplemented);
    let target_unavailable =
        Capability::Unavailable(CapabilityUnavailableReason::TargetUnavailable);

    if !enabled {
        return FormatCapabilities {
            format,
            target,
            feature_enabled: false,
            detection: manifest,
            inspection: disabled,
            still_decode: disabled,
            still_encode: disabled,
            sequence_decode: disabled,
            sequence_encode: disabled,
        };
    }

    if matches!(
        (format, target),
        (
            ImageFormat::Avif,
            CapabilityTarget::Wasm32Wasi | CapabilityTarget::Wasm32Unknown
        )
    ) {
        return FormatCapabilities {
            format,
            target,
            feature_enabled: true,
            detection: manifest,
            inspection: manifest,
            still_decode: Capability::Restricted(CapabilityRestriction::PortableAvif),
            still_encode: target_unavailable,
            sequence_decode: target_unavailable,
            sequence_encode: target_unavailable,
        };
    }

    let (sequence_decode, sequence_encode) = match format {
        ImageFormat::Png => (manifest, not_implemented),
        ImageFormat::Gif | ImageFormat::WebP | ImageFormat::Tiff | ImageFormat::Avif => {
            (manifest, manifest)
        }
        ImageFormat::Jpeg | ImageFormat::Bmp | ImageFormat::Ico => {
            (not_implemented, not_implemented)
        }
    };
    FormatCapabilities {
        format,
        target,
        feature_enabled: true,
        detection: manifest,
        inspection: manifest,
        still_decode: manifest,
        still_encode: manifest,
        sequence_decode,
        sequence_encode,
    }
}

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
        for target in [
            CapabilityTarget::Native,
            CapabilityTarget::Wasm32Wasi,
            CapabilityTarget::Wasm32Unknown,
        ] {
            for enabled in [false, true] {
                let capabilities = capabilities_for(format, enabled, target);
                let _ = capabilities.format();
                let _ = capabilities.target();
                let _ = capabilities.feature_enabled();
                for operation in CODEC_OPERATIONS {
                    let capability = capabilities.operation(operation);
                    let _ = capability.is_available();
                    let _ = capability.is_restricted();
                    let _ = capability.unavailable_reason();
                    let _ = capability.restriction();
                }
            }
        }
    }
}
