//! Immutable encoded-image sources with persistent lazy decoding.

use std::sync::{Arc, OnceLock};

use crate::{
    CodecOperation, DecodePolicy, Decoded, DecodedImage, DecodedSequence, ImageError,
    ImageErrorStage, ImageFormat, ImageInfo, ImageResult, TransferLayout, VerificationScope,
};

/// State of one persistent lazy decode cache on an [`EncodedImage`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EncodedImageDecodeState {
    /// No decode result has been attempted or retained.
    NotAttempted,
    /// A decoded result was retained successfully.
    Succeeded,
    /// A deterministic decode failure was retained.
    Failed,
}

#[derive(Debug)]
struct EncodedImageInner {
    bytes: Arc<[u8]>,
    info: ImageInfo,
    decoded: OnceLock<ImageResult<Decoded<DecodedImage>>>,
    sequence_decoded: OnceLock<ImageResult<Decoded<DecodedSequence>>>,
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

/// A borrowed, immutable encoded-image view.
///
/// [`EncodedImageView`] borrows a byte slice and performs the same detection,
/// inspection, verification, and decoding as [`EncodedImage`] without copying
/// the bytes into an owned snapshot. It has no cache: every decode reparses
/// the borrowed bytes, so it is best for short-lived uses where ownership and
/// shared lazy caching are not needed. The owned [`EncodedImage`] remains the
/// primary API.
#[derive(Debug, Clone)]
pub struct EncodedImageView<'a> {
    bytes: &'a [u8],
    format: ImageFormat,
    info: ImageInfo,
}

impl<'a> EncodedImageView<'a> {
    /// Borrow encoded bytes and inspect the header.
    ///
    /// # Errors
    ///
    /// Returns a structured error when the signature is unknown, the detected
    /// codec feature is disabled, or the encoded header is malformed.
    pub fn new(bytes: &'a [u8]) -> ImageResult<Self> {
        Self::new_with_policy(bytes, &DecodePolicy::default())
    }

    /// Borrow encoded bytes and inspect the header under an explicit policy.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::new`], plus
    /// [`crate::ImageError::LimitExceeded`] for configured resource limits.
    pub fn new_with_policy(bytes: &'a [u8], policy: &DecodePolicy) -> ImageResult<Self> {
        let info = crate::inspect_with_policy(bytes, policy)?;
        Ok(Self {
            bytes,
            format: info.format,
            info,
        })
    }

    /// Header metadata retained at construction.
    #[must_use]
    pub const fn info(&self) -> &ImageInfo {
        &self.info
    }

    /// Detected encoded format.
    #[must_use]
    pub const fn format(&self) -> ImageFormat {
        self.format
    }

    /// Exact transfer-byte length for the inspected image.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::Dimensions`] when the byte length overflows
    /// `usize`.
    pub fn decoded_bytes(&self) -> ImageResult<usize> {
        self.info.decoded_bytes()
    }

    /// Exact transfer layout for the inspected image.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::Dimensions`] when the byte length overflows
    /// `usize`.
    pub fn transfer_layout(&self) -> ImageResult<TransferLayout> {
        self.info.transfer_layout()
    }

    /// Decode the still/first-image view.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`crate::decode`].
    pub fn decode(&self) -> ImageResult<Decoded<DecodedImage>> {
        self.decode_with_policy(&DecodePolicy::default())
    }

    /// Decode the still view under an explicit policy.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`crate::decode_with_policy`].
    pub fn decode_with_policy(&self, policy: &DecodePolicy) -> ImageResult<Decoded<DecodedImage>> {
        policy.check_encoded_input(self.bytes, CodecOperation::StillDecode)?;
        crate::decode_selected_with_policy(self.bytes, self.format, policy)
    }

    /// Decode every retained frame/page.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`crate::decode_sequence`].
    pub fn decode_sequence(&self) -> ImageResult<Decoded<crate::DecodedSequence>> {
        self.decode_sequence_with_policy(&DecodePolicy::default())
    }

    /// Decode every retained frame/page under an explicit policy.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`crate::decode_sequence_with_policy`].
    pub fn decode_sequence_with_policy(
        &self,
        policy: &DecodePolicy,
    ) -> ImageResult<Decoded<crate::DecodedSequence>> {
        policy.check_encoded_input(self.bytes, CodecOperation::SequenceDecode)?;
        crate::decode_sequence_selected_with_policy(self.bytes, self.format, policy)
    }

    /// Decode exactly one retained frame or page by index.
    ///
    /// TIFF decodes only the selected page's IFD; other sequence formats
    /// currently decode the full sequence and return the indexed frame.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::Parameter`] when the index is out of range, and
    /// the format's structured decode error for the selected frame otherwise.
    pub fn decode_frame(&self, index: u32) -> ImageResult<crate::DecodedFrame> {
        crate::codecs::decode_frame_format(self.bytes, self.format, index)
    }

    /// Verify the borrowed bytes under the format's default scope.
    ///
    /// # Errors
    ///
    /// Returns a structured error when the format's verification contract
    /// rejects the bytes.
    pub fn verify(&self) -> ImageResult<()> {
        crate::codecs::verify_format(self.bytes, self.format)
    }

    /// Verify with an explicit caller-requested strength.
    ///
    /// The requested scope must be provided by the source format; requesting
    /// a stronger scope fails with a format-qualified
    /// [`ImageError::Unsupported`] instead of silently reporting weaker
    /// evidence.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::verify`], or
    /// [`ImageError::Unsupported`] when the requested scope is stronger than
    /// the format provides.
    pub fn verify_with_scope(&self, requested: VerificationScope) -> ImageResult<()> {
        let provided = self.verification_scope();
        if !provided.provides(requested) {
            return Err(ImageError::Unsupported {
                format: Some(self.format),
                message: format!(
                    "{requested:?} verification is not provided; {} provides {provided:?}",
                    self.format.as_str(),
                ),
                stage: Some(ImageErrorStage::Verification),
                reason: Some(crate::UnsupportedReason::NotImplemented),
                offset: None,
                identity: None,
            });
        }
        self.verify()
    }

    /// How much validation [`Self::verify`] performs for this format.
    #[must_use]
    pub fn verification_scope(&self) -> VerificationScope {
        self.format.verification_scope()
    }
}

impl EncodedImage {
    /// Creates a stable encoded snapshot and inspects its header.
    ///
    /// # Errors
    ///
    /// Returns a structured error when the signature is unknown, the detected
    /// codec feature is disabled, or the encoded header is malformed.
    pub fn new(bytes: impl Into<Arc<[u8]>>) -> ImageResult<Self> {
        Self::new_with_policy(bytes, &DecodePolicy::default())
    }

    /// Creates and inspects a stable encoded snapshot under an explicit policy.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ImageError::LimitExceeded`] before detection when the
    /// encoded snapshot is too large, or after inspection when its primary
    /// canvas, decoded transfer-byte length, or inspected frame count exceeds
    /// a configured maximum. Otherwise returns the same errors as
    /// [`Self::new`].
    pub fn new_with_policy(
        bytes: impl Into<Arc<[u8]>>,
        policy: &DecodePolicy,
    ) -> ImageResult<Self> {
        let bytes = bytes.into();
        let info = crate::inspect_with_policy(&bytes, policy)?;
        Ok(Self {
            inner: Arc::new(EncodedImageInner {
                bytes,
                info,
                decoded: OnceLock::new(),
                sequence_decoded: OnceLock::new(),
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

    /// Returns the state of the ordinary still-decode cache.
    #[must_use]
    pub fn decode_state(&self) -> EncodedImageDecodeState {
        cache_state(&self.inner.decoded)
    }

    /// Returns the state of the retained sequence-decode cache.
    #[must_use]
    pub fn sequence_decode_state(&self) -> EncodedImageDecodeState {
        cache_state(&self.inner.sequence_decoded)
    }

    /// Returns whether ordinary still decoding has completed successfully.
    ///
    /// A cached failure is not considered materialized. Use [`Self::decode_state`]
    /// when callers must distinguish a failed attempt from no attempt.
    #[must_use]
    pub fn is_decoded(&self) -> bool {
        self.decode_state() == EncodedImageDecodeState::Succeeded
    }

    /// Returns whether sequence decoding has completed successfully.
    ///
    /// A cached failure is not considered materialized. Use
    /// [`Self::sequence_decode_state`] when callers must distinguish a failed
    /// attempt from no attempt.
    #[must_use]
    pub fn is_sequence_decoded(&self) -> bool {
        self.sequence_decode_state() == EncodedImageDecodeState::Succeeded
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
        self.decode_with_policy(&DecodePolicy::default())
    }

    /// Lazily decode pixels under an explicit caller-controlled policy.
    ///
    /// A policy rejection happens before consulting or initializing the
    /// shared cache. A later call with a sufficient policy can therefore
    /// materialize normally, while a cached success cannot bypass a stricter
    /// encoded-input maximum.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ImageError::LimitExceeded`] before cache access when
    /// the encoded snapshot, retained primary canvas, or primary decoded
    /// transfer-byte length, or a zero frame-count maximum exceeds the
    /// materialized still frame. Otherwise returns the same errors as
    /// [`Self::decode`].
    pub fn decode_with_policy(&self, policy: &DecodePolicy) -> ImageResult<&Decoded<DecodedImage>> {
        policy.check_encoded_input(&self.inner.bytes, CodecOperation::StillDecode)?;
        policy.check_metadata_bytes(
            &self.inner.bytes,
            self.format(),
            CodecOperation::StillDecode,
        )?;
        policy.check_image_info(&self.inner.info, CodecOperation::StillDecode)?;
        self.inner
            .decoded
            .get_or_init(|| {
                crate::decode_selected_with_policy(
                    &self.inner.bytes,
                    self.format(),
                    &DecodePolicy::default(),
                )
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    /// Decode and retain every frame or page in a shared lazy sequence cache.
    ///
    /// The returned sequence is cloned from the retained result so this method
    /// preserves an owned return value while avoiding repeated codec work.
    /// Clones of this source share the same cache. Deterministic failures from
    /// the unlimited compatibility operation are cached as well.
    ///
    /// # Errors
    ///
    /// Returns the structured sequence decoder failure for malformed,
    /// unsupported, or feature-disabled input.
    pub fn decode_sequence(&self) -> ImageResult<Decoded<DecodedSequence>> {
        self.inner
            .sequence_decoded
            .get_or_init(|| {
                crate::decode_sequence_selected_with_policy(
                    &self.inner.bytes,
                    self.format(),
                    &DecodePolicy::default(),
                )
            })
            .clone()
    }

    /// Decode every frame or page under an explicit policy.
    ///
    /// The unlimited compatibility policy uses the shared sequence cache.
    /// A policy with resource limits runs the policy-aware root operation so
    /// policy-dependent failures are not retained as if they were inherent to
    /// the encoded source.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`crate::decode_sequence_with_policy`].
    pub fn decode_sequence_with_policy(
        &self,
        policy: &DecodePolicy,
    ) -> ImageResult<Decoded<DecodedSequence>> {
        if *policy == DecodePolicy::default() {
            return self.decode_sequence();
        }
        policy.check_encoded_input(&self.inner.bytes, CodecOperation::SequenceDecode)?;
        crate::decode_sequence_selected_with_policy(&self.inner.bytes, self.format(), policy)
    }

    /// Decode exactly one retained frame or page by index.
    ///
    /// TIFF decodes only the selected page's IFD; other sequence formats
    /// currently decode the full sequence and return the indexed frame, so
    /// the returned frame always matches [`crate::decode_sequence`] ordering
    /// and per-frame content exactly.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::Parameter`] when the index is out of range, and
    /// the format's structured decode error for the selected frame otherwise.
    pub fn decode_frame(&self, index: u32) -> ImageResult<crate::DecodedFrame> {
        crate::codecs::decode_frame_format(&self.inner.bytes, self.format(), index)
    }

    /// Applies the format-specific Pillow verification contract to the snapshot.
    ///
    /// [`Self::verification_scope`] distinguishes a format-specific structural
    /// scan from Pillow's header-only default. A successful header-only result
    /// does not prove that later pixel decompression will succeed.
    ///
    /// Verification executes independently from ordinary materialization, so
    /// it does not populate or change the shared decode cache.
    ///
    /// # Errors
    ///
    /// Returns a structured error when the pinned Pillow oracle's
    /// format-specific verification contract rejects the snapshot.
    pub fn verify(&self) -> ImageResult<()> {
        crate::codecs::verify_format(&self.inner.bytes, self.format())
    }

    /// Verify with an explicit caller-requested strength.
    ///
    /// The requested scope must be provided by the source format's
    /// [`Self::verification_scope`]; requesting a stronger scope fails with a
    /// format-qualified [`ImageError::Unsupported`] instead of silently
    /// reporting weaker evidence. Every format provides weaker or equal
    /// scopes, and no format currently provides
    /// [`VerificationScope::FullPixels`].
    ///
    /// Verification executes independently from ordinary materialization, so
    /// it does not populate or change the shared decode cache.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::verify`], or
    /// [`ImageError::Unsupported`] when the requested scope is stronger than
    /// the format provides.
    pub fn verify_with_scope(&self, requested: VerificationScope) -> ImageResult<()> {
        let provided = self.verification_scope();
        if !provided.provides(requested) {
            return Err(ImageError::Unsupported {
                format: Some(self.format()),
                message: format!(
                    "{requested:?} verification is not provided; {} provides {provided:?}",
                    self.format().as_str(),
                ),
                stage: Some(ImageErrorStage::Verification),
                reason: Some(crate::UnsupportedReason::NotImplemented),
                offset: None,
                identity: None,
            });
        }
        self.verify()
    }

    /// Returns how much validation [`Self::verify`] performs for this format.
    #[must_use]
    pub fn verification_scope(&self) -> VerificationScope {
        self.format().verification_scope()
    }
}

fn cache_state<T>(cache: &OnceLock<ImageResult<T>>) -> EncodedImageDecodeState {
    match cache.get() {
        None => EncodedImageDecodeState::NotAttempted,
        Some(Ok(_)) => EncodedImageDecodeState::Succeeded,
        Some(Err(_)) => EncodedImageDecodeState::Failed,
    }
}
