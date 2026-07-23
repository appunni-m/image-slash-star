//! Immutable encoded-image sources with persistent lazy decoding.

use std::sync::{Arc, OnceLock};

use crate::{Decoded, DecodedImage, ImageFormat, ImageInfo, ImageResult};

#[derive(Debug)]
struct EncodedImageInner {
    bytes: Arc<[u8]>,
    info: ImageInfo,
    decoded: OnceLock<ImageResult<Decoded<DecodedImage>>>,
}

/// An immutable encoded-image snapshot with a shared lazy decode cache.
///
/// Construction performs signature detection and header inspection but does
/// not decompress pixels. Clones share both the encoded bytes and the
/// once-initialized decode result. Deterministic decode failures are cached as
/// well as successful results.
#[derive(Debug, Clone)]
pub struct EncodedImage {
    inner: Arc<EncodedImageInner>,
}

impl EncodedImage {
    /// Creates a stable encoded snapshot and inspects its header.
    ///
    /// # Errors
    ///
    /// Returns a structured error when the signature is unknown, the detected
    /// codec feature is disabled, or the encoded header is malformed.
    pub fn new(bytes: impl Into<Arc<[u8]>>) -> ImageResult<Self> {
        let bytes = bytes.into();
        let info = crate::inspect(&bytes)?;
        Ok(Self {
            inner: Arc::new(EncodedImageInner {
                bytes,
                info,
                decoded: OnceLock::new(),
            }),
        })
    }

    /// Returns the immutable encoded byte snapshot.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.inner.bytes
    }

    /// Returns metadata inspected from the encoded header.
    #[must_use]
    pub fn info(&self) -> &ImageInfo {
        &self.inner.info
    }

    /// Returns the detected source container format.
    #[must_use]
    pub fn format(&self) -> ImageFormat {
        self.inner.info.format
    }

    /// Returns whether ordinary decoding has completed successfully.
    ///
    /// A cached failure is not considered materialized.
    #[must_use]
    pub fn is_decoded(&self) -> bool {
        matches!(self.inner.decoded.get(), Some(Ok(_)))
    }

    /// Decodes pixels once and returns the shared cached result.
    ///
    /// Every clone of this source observes the same initialized value. Both a
    /// successful decode and a deterministic decode failure are retained.
    ///
    /// # Errors
    ///
    /// Returns the structured decoder failure for malformed, unsupported, or
    /// feature-disabled input.
    pub fn decode(&self) -> ImageResult<&Decoded<DecodedImage>> {
        self.inner
            .decoded
            .get_or_init(|| crate::decode(&self.inner.bytes))
            .as_ref()
            .map_err(Clone::clone)
    }

    /// Fully validates the snapshot without populating the ordinary cache.
    ///
    /// This deliberately performs an independent decode so callers can verify
    /// input without changing observable lazy-load state.
    ///
    /// # Errors
    ///
    /// Returns the same structured decoder errors as [`Self::decode`].
    pub fn verify(&self) -> ImageResult<()> {
        crate::decode(&self.inner.bytes).map(|_| ())
    }
}
