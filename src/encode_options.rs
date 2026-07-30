//! Portable options accepted by the private format-specific encoders.
//!
//! Each encoder consumes the fields documented below and ignores unrelated
//! fields. Encoding always receives its target format separately through
//! [`crate::encode`] or [`crate::encode_sequence`].

use std::collections::HashMap;

/// Shared encoder settings plus codec-specific compatibility values.
///
/// The typed fields cover settings shared by one or more formats. [`Self::extra`]
/// retains the remaining Pillow-oracle compatibility surface without making
/// those string keys part of another codec's behavior.
#[derive(Debug, Clone, Default)]
pub struct EncodeOptions {
    /// Requested quality in the `0..=100` range for JPEG, lossy WebP, or AVIF.
    pub quality: Option<u8>,
    /// PNG DEFLATE level in the `0..=9` range; zero disables compression.
    pub compression: Option<u8>,
    /// Whether JPEG should use progressive scans.
    pub progressive: Option<bool>,
    /// Whether JPEG should optimize Huffman tables or PNG should use its
    /// maximum-compression optimization path.
    pub optimize: Option<bool>,
    /// Chroma subsampling.
    ///
    /// JPEG accepts `"444"`, `"422"`, or `"420"`. Native AVIF accepts
    /// `"4:4:4"`, `"4:2:2"`, `"4:2:0"`, or `"4:0:0"`.
    pub subsampling: Option<String>,
    /// Whether WebP should use lossless rather than lossy encoding.
    pub lossless: Option<bool>,
    /// WebP encoder effort in the `0..=6` range.
    pub method: Option<u8>,
    /// Whether GIF frame data should be interlaced.
    ///
    /// PNG Adam7 output is not currently implemented.
    pub interlace: Option<bool>,
    /// Ordered codec-specific AVIF encoder key/value pairs.
    ///
    /// The order and duplicate keys are retained because Pillow accepts both
    /// mappings and sequences of pairs, and native libavif applies them in
    /// order. Portable AVIF encoding is not currently implemented.
    pub advanced: Vec<(String, String)>,
    /// String-valued compatibility options consumed by individual encoders.
    ///
    /// Supported keys are:
    ///
    /// - JPEG: `restart_interval`, `exif_hex`;
    /// - PNG: `compression`, `gamma`, `srgb`, `physical`, `text_chunks`, `time`;
    /// - GIF: `animated`, `disposal`, `color_table`, `transparency`, `loop`;
    /// - TIFF: `compression`, `predictor`;
    /// - WebP: `icc_hex`, `exif_hex`, `xmp_hex`;
    /// - ICO: `entry_type`, `sizes`;
    /// - native AVIF: `codec`, `subsampling`, `range`, `speed`,
    ///   `max_threads`, `tile_rows`, `tile_cols`, `alpha_premultiplied`,
    ///   `autotiling`, `icc_hex`, `exif_hex`, `exif_orientation`, `xmp_hex`,
    ///   and `sequence_time`.
    ///
    /// Unknown keys are ignored. Invalid values for a consumed key return
    /// [`crate::ImageError::Parameter`].
    pub extra: HashMap<String, String>,
}

impl EncodeOptions {
    /// Returns an option set with every override unset.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }
}
