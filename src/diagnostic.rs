//! Dependency-free non-fatal diagnostics returned beside decode success.
//!
//! Decoders tolerate a small, manifest-proven set of recoverable conditions
//! without failing: well-formed trailing bytes after the container extent, a
//! non-standard-but-accepted GIF graphic-control size, a PNG stream accepted
//! without its `IEND` terminator, duplicate PNG palette/transparency chunks,
//! tolerated PNG palette-shape and APNG declaration damage, an oversized PNG
//! scanline stream whose first raster remains usable, invalid compressed PNG
//! ancillary metadata whose pixels remain usable, and Pillow-deferred PNG CRC
//! failures in image-data, terminator, APNG, and post-`IDAT` ancillary chunks.
//! These conditions are reported as stable [`ImageDiagnostic`] records on the
//! [`crate::Decoded`] envelope. Fields are contractual; there is deliberately
//! no free-form prose.

use crate::types::{ImageErrorStage, ImageFormat};

/// Stable category of a non-fatal diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DiagnosticKind {
    /// The decoder recovered from a malformed container structure that the
    /// accepted fixture/oracle decision tolerates, and continued with the
    /// usable result.
    RecoveredStructure,
    /// Invalid compressed ancillary metadata was ignored while pixel decode
    /// continued with the usable result.
    InvalidMetadataIgnored,
    /// Well-formed bytes after the container-defined extent were ignored;
    /// `offset` names the first ignored byte.
    TrailingDataIgnored,
}

/// A non-fatal diagnostic returned beside a successful decode.
///
/// The fields are the stable recovery surface: callers may match
/// [`DiagnosticKind`], the format, the operation stage, the byte offset, and
/// the container-structure identity. Diagnostics never change the decoded
/// result and are empty when nothing recoverable occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageDiagnostic {
    /// Stable category of the recoverable condition.
    pub kind: DiagnosticKind,
    /// Detected encoded container format.
    pub format: ImageFormat,
    /// Public operation that produced the diagnostic, when known.
    pub stage: Option<ImageErrorStage>,
    /// Encoded-input byte offset of the condition, when the parser can name
    /// it.
    pub offset: Option<u64>,
    /// Stable container-structure identity, when the parser can name it.
    pub identity: Option<&'static str>,
}
