//! Portable options accepted by the format-specific encoders.
//!
//! Each encoder consumes the fields it documents and ignores unrelated fields.

use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
/// Format-neutral encoder settings plus ordered codec-specific values.
pub struct EncodeOptions {
    /// Quality 0-100 (JPEG, WebP lossy, and AVIF).
    pub quality: Option<u8>,
    /// Compression level 0-9 (PNG), 0=none, 9=max
    pub compression: Option<u8>,
    /// Progressive encoding (JPEG, PNG)
    pub progressive: Option<bool>,
    /// Optimize Huffman tables (JPEG)
    pub optimize: Option<bool>,
    /// Chroma subsampling: "444", "422", "420" (JPEG)
    pub subsampling: Option<String>,
    /// Lossless mode (WebP)
    pub lossless: Option<bool>,
    /// Encoder effort method 0-6 (WebP)
    pub method: Option<u8>,
    /// Interlaced (PNG Adam7, GIF)
    pub interlace: Option<bool>,
    /// Ordered codec-specific AVIF encoder key/value pairs.
    ///
    /// The order and duplicate keys are retained because Pillow accepts both
    /// mappings and sequences of pairs, and libavif applies them in order.
    pub advanced: Vec<(String, String)>,
    /// Catch-all for future params
    pub extra: HashMap<String, String>,
}

impl EncodeOptions {
    /// Returns an option set with every override unset.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }
}
