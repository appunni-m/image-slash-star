//! Assertions for coverage-only fixture setup and observations.
//!
//! A failed fixture prerequisite must fail the test, never skip the exercise
//! or substitute default input. These helpers preserve that contract while
//! attaching the caller's context to the failure.

#![allow(
    clippy::panic,
    reason = "coverage fixture prerequisites must panic so the integration test fails instead of skipping an exercise"
)]

use std::cell::Cell;

/// Require a successful fixture operation, reporting the original error.
#[track_caller]
pub(crate) fn require_ok<T, E: std::fmt::Debug>(value: Result<T, E>, context: &str) -> T {
    match value {
        Ok(value) => value,
        Err(error) => panic!("{context}: {error:?}"),
    }
}

/// Require a fixture value that the exercise promises is present.
#[track_caller]
pub(crate) fn require_some<T>(value: Option<T>, context: &str) -> T {
    match value {
        Some(value) => value,
        None => panic!("{context}: expected a present fixture value"),
    }
}

/// A deterministic failure point available only to coverage builds.
///
/// These points let public fault-contract cases reach target-side failure
/// outcomes without adding controls to normal library builds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverageFaultPoint {
    /// Force AV1 segment-map allocation to fail during segmented frame setup.
    Av1SegmentMapReservation,
    /// Mismatch AV1 frame and lossless-segment block depths during validation.
    Av1InterTranslationSampleDepthMismatch,
    /// Set AV1 motion-predictor width to zero before scratch geometry validation.
    Av1MotionPredictorZeroWidth,
    /// Sweep invalid AV1 reference-MV geometry through its public decode error.
    Av1ReferenceMvGeometryGuardSweep,
    /// Report the AV1 display-plane copy allocation failure before reserving
    /// its output sample buffer.
    Av1DisplayPlaneCopyAllocation,
    /// Force AV1 film-grain output-plane reservation to fail.
    Av1FilmGrainOutputReservation,
    /// Force AV1 film-grain point-array reservation to fail while parsing.
    Av1FilmGrainPointsReservation,
    /// Force AV1 temporal motion-field reservation to fail during frame completion.
    Av1TemporalMotionFieldReservation,
    /// Force AV1 projected temporal-field reservation to fail during frame setup.
    Av1ProjectedTemporalFieldReservation,
    /// Force AVIF sequence-frame reservation to fail before any frame is decoded.
    AvifSequenceFrameReservation,
    /// Shorten a valid AVIF 10-bit I444 alpha plane to test its length guard.
    Avif10BitI444AlphaPlaneLengthMismatch,
    /// Shorten a valid AVIF full-resolution alpha plane to test its generic length guard.
    AvifI444AlphaPlaneLengthMismatch,
    /// Shorten a valid AVIF 12-bit I420 alpha plane before color conversion.
    Avif12BitI420AlphaSampleCountMismatch,
    /// Pass a zero width to the AVIF 12-bit I420 alpha converter.
    Avif12BitI420AlphaZeroWidth,
    /// Shorten matching AVIF 12-bit I420 luma and alpha planes to a non-row length.
    Avif12BitI420AlphaNondivisibleLumaLength,
    /// Pass empty AVIF 12-bit I420 luma and alpha planes to the converter.
    Avif12BitI420AlphaZeroHeight,
    /// Shorten a valid AVIF 12-bit I420 U plane before color conversion.
    Avif12BitI420AlphaUPlaneLengthMismatch,
    /// Shorten a valid AVIF 12-bit I420 V plane before color conversion.
    Avif12BitI420AlphaVPlaneLengthMismatch,
    /// Shorten a reconstructed AVIF Y plane after canvas assembly.
    AvifPortableYPlaneLengthMismatch,
    /// Shorten a reconstructed AVIF U plane after canvas assembly.
    AvifPortableUPlaneLengthMismatch,
    /// Shorten a reconstructed AVIF V plane after canvas assembly.
    AvifPortableVPlaneLengthMismatch,
    /// Shorten a validated AVIF alpha plane before portable decoding.
    AvifPortableAlphaPlaneLengthMismatch,
    /// Force identity color conversion on subsampled AVIF planes.
    AvifSubsampledIdentityConversion,
    /// Shorten a valid AVIF monochrome alpha plane to test its dimension guard.
    AvifMonochromeAlphaPlaneLengthMismatch,
    /// Clear AVIF monochrome horizontal subsampling after validation.
    AvifMonochromePortableSubsamplingXFalse,
    /// Clear AVIF monochrome vertical subsampling after validation.
    AvifMonochromePortableSubsamplingYFalse,
    /// Populate the AVIF monochrome U plane after validation.
    AvifMonochromePortableUPlaneNonEmpty,
    /// Populate the AVIF monochrome V plane after validation.
    AvifMonochromePortableVPlaneNonEmpty,
    /// Shorten the AVIF monochrome luma plane after validation.
    AvifMonochromePortableYPlaneLengthMismatch,
    /// Shorten the AVIF monochrome alpha plane after validation.
    AvifMonochromePortableAlphaPlaneLengthMismatch,
    /// Inject an out-of-depth sample into AVIF monochrome luma after validation.
    AvifMonochromePortableYSampleOutOfDepth,
    /// Inject an out-of-depth sample into AVIF monochrome alpha after validation.
    AvifMonochromePortableAlphaSampleOutOfDepth,
    /// Shorten decoded AVIF monochrome pixels before the output-length guard.
    AvifMonochromePortableOutputLengthMismatch,
    /// Shorten a valid generic AVIF I444 U plane to test its length guard.
    AvifGenericI444UPlaneLengthMismatch,
    /// Shorten a valid generic AVIF I444 V plane to test its length guard.
    AvifGenericI444VPlaneLengthMismatch,
    /// Inject an out-of-range sample before generic AVIF I444 validation.
    AvifGenericI444InvalidSample,
    /// Shorten a valid AVIF 10-bit I444 U plane to test its length guard.
    Avif10BitI444UPlaneLengthMismatch,
    /// Shorten a valid AVIF 10-bit I444 V plane to test its length guard.
    Avif10BitI444VPlaneLengthMismatch,
    /// Inject an out-of-range AVIF 10-bit I444 sample before validation.
    Avif10BitI444InvalidSample,
    /// Force the AVIF 10-bit I444 alpha output reservation to fail.
    Avif10BitI444AlphaOutputReservation,
    /// Force generic AVIF I444 RGB output reservation to fail.
    AvifI444RgbOutputReservation,
    /// Force subsampled AVIF RGB output reservation to fail.
    AvifSubsampledRgbOutputReservation,
    /// Force AVIF item-information table reservation to fail during parsing.
    AvifItemTableReservation,
    /// Force AVIF item-location table reservation to fail during parsing.
    AvifItemLocationTableReservation,
    /// Force AVIF item-extent span reservation to fail during parsing.
    AvifItemExtentSpanReservation,
    /// Force AV1 super-resolution position reservation to fail while resizing a frame.
    Av1SuperResolutionPositionReservation,
    /// Corrupt the decoded leaf width before AV1 super-resolution geometry validation.
    Av1SuperResolutionHeaderGeometryMismatch,
    /// Force AV1 super-resolution output-plane reservation to fail.
    Av1SuperResolutionPlaneReservation,
    /// Force AV1 super-resolution chroma U output-plane reservation to fail.
    Av1SuperResolutionChromaUPlaneReservation,
    /// Force AV1 super-resolution chroma V output-plane reservation to fail.
    Av1SuperResolutionChromaVPlaneReservation,
    /// End a JPEG entropy segment immediately after an FF marker prefix.
    JpegFastReaderMarkerPrefixEof,
    /// Reject the JPEG baseline fast path when scan metadata reports multiple scans.
    JpegFastReaderScanCountMismatch,
    /// Reject the JPEG baseline fast path when scan entropy starts disagree.
    JpegFastReaderEntropyStartMismatch,
    /// Reject the JPEG baseline fast path when scan entropy extends past EOI.
    JpegFastReaderEntropyEndAfterEoi,
    /// Reject the JPEG baseline fast path when the indexed EOI marker differs.
    JpegFastReaderEoiMarkerMismatch,
    /// Reject the JPEG baseline fast path when trailing marker parsing misses EOI.
    JpegFastReaderTrailingMarkerPositionMismatch,
    /// Force JPEG multi-scan coefficient retention reservation to fail.
    JpegCoefficientBufferReservation,
    /// Force reservation of default JPEG Huffman table slots to fail.
    JpegHuffmanTableSlotReservation,
    /// Force the JPEG 1x2 upsampler to reject empty component geometry.
    JpegH1v2UpsampleEmptyGeometry,
    /// Force the JPEG 1x2 upsampler to reject a zero component width.
    JpegH1v2UpsampleZeroWidth,
    /// Force the JPEG 1x2 upsampler to reject a short source plane.
    JpegH1v2UpsampleSourceLengthMismatch,
    /// Force reservation of the JPEG 1x2 upsampled component plane to fail.
    JpegH1v2UpsamplePlaneReservation,
    /// Force the row-wise JPEG upsampler to reject invalid component geometry.
    JpegFancyUpsampleRowInvalidGeometry,
    /// Force the row-wise JPEG upsampler to reject a zero component height.
    JpegFancyUpsampleRowZeroHeight,
    /// Force the row-wise JPEG upsampler to reject a zero horizontal ratio.
    JpegFancyUpsampleRowZeroHorizontalRatio,
    /// Force the row-wise JPEG upsampler to reject a zero vertical ratio.
    JpegFancyUpsampleRowZeroVerticalRatio,
    /// Force the row-wise JPEG upsampler to reject a component stride shorter than its width.
    JpegFancyUpsampleRowShortStride,
    /// Force the row-wise JPEG upsampler to reject a component sample buffer that is too short.
    JpegFancyUpsampleRowSourceLengthMismatch,
    /// Force checked multiplication of JPEG row stride and row index to overflow.
    JpegFancyUpsampleRowLastRowStartOverflow,
    /// Force checked addition of JPEG row start and width to overflow.
    JpegFancyUpsampleRowRequiredSourceLengthOverflow,
    /// Force checked multiplication of JPEG height and vertical ratio to overflow.
    JpegFancyUpsampleRowDestinationHeightOverflow,
    /// Force Adobe YCCK decoded-pixel output reservation to fail.
    JpegYcckPixelsReservation,
    /// Force reservation of a BMP palette to fail before reading its entries.
    BmpPaletteReservation,
    /// Force the BMP 24-bit decoded-pixel output reservation to fail.
    BmpDecodedPixelsReservation,
    /// Present a height beyond BMP's signed dimension limit to the encoder guard.
    BmpEncodeHeightBeyondSignedLimit,
    /// Force PNG decoded-sample reservation to fail before scanline unpacking.
    PngDecodedSampleBufferReservation,
    /// Inject a PNG chunk-iteration failure and exercise the iterator's failed state.
    PngChunkIteratorFailure,
    /// Model APNG sequence-counter exhaustion during coverage builds.
    ApngSequenceCounterExhaustion,
    /// Model an oversized WebP hash-chain index during coverage builds.
    WebpHashChainIndexOverflow,
    /// Shorten a valid WebP RIFF output before sink header validation.
    WebpRiffHeaderTooShort,
    /// Corrupt the RIFF signature before WebP sink header validation.
    WebpRiffSignatureMismatch,
    /// Corrupt the WEBP form type before WebP sink header validation.
    WebpRiffFormTypeMismatch,
    /// Corrupt the declared RIFF size before WebP sink header validation.
    WebpRiffDeclaredSizeMismatch,
    /// Force TIFF strip assembly output reservation to fail before decoding.
    TiffStripAssemblyReservation,
    /// Force TIFF RGB16 conversion output reservation to fail.
    TiffRgb16OutputReservation,
    /// Force TIFF LZW decoding to observe a zero expected output length.
    TiffLzwZeroExpectedOutput,
    /// Force TIFF header validation to reject an otherwise registered magic value.
    TiffUnregisteredMagic,
    /// Report a TIFF subsampled YCbCr clump-buffer length mismatch.
    TiffYcbcrClumpLengthMismatch,
    /// Force specialized TIFF YCbCr RGB output reservation to fail.
    TiffYcbcrRgbOutputReservation,
    /// Force contiguous TIFF YCbCr tile assembly reservation to fail.
    TiffYcbcrTileAssemblyReservation,
    /// Force a contiguous TIFF YCbCr tile prefix to exceed its output budget.
    TiffYcbcrTilePrefixLimit,
    /// Report a TIFF full-resolution YCbCr sample-buffer length mismatch.
    TiffYcbcrFullResolutionLengthMismatch,
    /// Report zero subsampling after TIFF YCbCr layout validation.
    TiffYcbcrHorizontalSubsamplingZero,
    /// Report zero vertical subsampling after TIFF YCbCr layout validation.
    TiffYcbcrVerticalSubsamplingZero,
    /// Reach TIFF YCbCr rational bounds validation during inspection.
    TiffYcbcrRationalIndexOutOfRange,
    /// Reach TIFF YCbCr floating-point bounds validation during inspection.
    TiffYcbcrFloatingIndexOutOfRange,
    /// Model an inconsistent TIFF numeric-entry byte width during inspection.
    TiffDirectoryNumericValueWidthInvalid,
    /// Model a numeric-entry byte length that is not divisible by its count.
    TiffDirectoryNumericValueWidthRemainder,
    /// Model a trailing byte in a TIFF numeric-entry value during inspection.
    TiffDirectoryNumericValueRemainderNonEmpty,
    /// Report zero row bytes while compacting TIFF ignored samples.
    TiffIgnoredSampleRowBytesZero,
    /// Shorten TIFF ignored-sample pixels to test the assembly-length guard.
    TiffIgnoredSampleAssemblyLengthMismatch,
    /// Force AV1 partial monochrome tile-state retention to fail.
    Av1MonochromeTileReservation,
    /// Force AV1 complete monochrome tile buffering to fail.
    Av1MonochromeGroupBufferReservation,
    /// Force AV1 partial color tile-state retention to fail.
    Av1ColorTileReservation,
    /// Force reservation of nonempty AV1 tile-group temporal-MV samples to fail.
    Av1TileGroupTemporalSampleReservation,
    /// Force partial-group temporal-MV retention to fail when storage must grow.
    Av1RetainedTemporalSampleReservation,
    /// Force AVIF color-grid cell retention to fail.
    Av1GridCellReservation,
    /// Force AVIF monochrome-grid cell retention to fail.
    Av1MonochromeGridCellReservation,
    /// Force assembled AV1 monochrome loop-filter metadata reservation to fail.
    Av1AssembledMonochromeLoopFilterMetadataReservation,
    /// Force assembled AV1 monochrome CDEF region-map reservation to fail.
    Av1AssembledMonochromeCdefRegionMapReservation,
    /// Force assembled AV1 monochrome CDEF active-map reservation to fail.
    Av1AssembledMonochromeCdefActiveMapReservation,
    /// Set AV1 CDEF sample depth outside its supported range before filtering.
    Av1CdefInvalidSampleDepth,
    /// Set AV1 CDEF direction sample depth outside its supported range.
    Av1CdefDirectionInvalidSampleDepth,
    /// Run AV1 CDEF private input guards with malformed geometry in a coverage build.
    Av1CdefInputGuardSweep,
    /// Shorten the color AV1 CDEF active map to exercise its public extent error.
    Av1CdefActiveMapExtentMismatch,
    /// Set one active color CDEF region index beyond its Y and UV strength counts.
    Av1CdefActiveStrengthIndexOutOfRange,
    /// Set the color AV1 CDEF luma strength count outside the syntax range.
    Av1CdefInvalidYStrengthCount,
    /// Set the color AV1 CDEF chroma strength count outside the syntax range.
    Av1CdefInvalidUvStrengthCount,
    /// Skip one primary and secondary CDEF tap in a coverage-only guard sweep.
    Av1CdefMissingTapSamples,
    /// Force AV1 per-tile luma cell storage reservation to fail.
    Av1TileCellReservation,
    /// Force AV1 per-tile chroma cell storage reservation to fail.
    Av1ChromaTileCellReservation,
    /// Force AV1 decoded block metadata reservation to fail.
    Av1TileBlockMetadataReservation,
    /// Force AV1 assembled loop-filter metadata reservation to fail.
    Av1AssembledLoopFilterMetadataReservation,
    /// Force assembled AV1 color CDEF region-map reservation to fail.
    Av1AssembledCdefRegionMapReservation,
    /// Force per-tile AV1 CDEF region-map reservation to fail.
    Av1TileCdefRegionMapReservation,
    /// Force per-tile AV1 CDEF active-map reservation to fail.
    Av1TileCdefActiveMapReservation,
    /// Force assembled AV1 color CDEF active-map reservation to fail.
    Av1AssembledCdefActiveMapReservation,
    /// Force AV1 partition-tree node reservation to fail.
    Av1PartitionNodeReservation,
    /// Force AV1 above partition-context reservation to fail.
    Av1PartitionContextAboveReservation,
    /// Force the AV1 SGR source-plane reservation to fail before restoration.
    Av1SgrSourceReservation,
    /// Force the AV1 SGR output-plane reservation to fail during restoration.
    Av1SgrRestorationOutputReservation,
    /// Force an AV1 SGR intermediate-buffer reservation to fail.
    Av1SgrIntermediateReservation,
    /// Force AV1 striped restoration scratch reservation to fail.
    Av1RestorationStripeScratchReservation,
    /// Force AV1 striped restoration output reservation to fail.
    Av1RestorationStripeOutputReservation,
    /// Route an all-lossless I444 frame through first-partition fallback validation.
    Av1LosslessI444FirstPartitionFallback,
    /// Shorten AV1 difference-compound reference 0 before its length guard.
    Av1DifferenceMaskFirstPlaneShort,
    /// Shorten AV1 difference-compound reference 1 before its length guard.
    Av1DifferenceMaskSecondPlaneShort,
    /// Shorten AV1 difference-compound output before its length guard.
    Av1DifferenceMaskOutputPlaneShort,
    /// Shorten AV1 difference-compound scratch before its length guard.
    Av1DifferenceMaskScratchShort,
    /// Set the AV1 wedge-fill width to zero before its geometry guard.
    Av1WedgeFillEmptyWidth,
    /// Set the AV1 wedge-fill height to zero before its geometry guard.
    Av1WedgeFillEmptyHeight,
    /// Shorten AV1 wedge-fill scratch before its length guard.
    Av1WedgeFillScratchShort,
    /// Set the AV1 wedge-reduction width to zero before its geometry guard.
    Av1WedgeReduceEmptyWidth,
    /// Set the AV1 wedge-reduction height to zero before its geometry guard.
    Av1WedgeReduceEmptyHeight,
    /// Shorten AV1 wedge-reduction scratch before its length guard.
    Av1WedgeReduceScratchShort,
    /// Set the AV1 inter-intra mask mode outside its normative range.
    Av1InterIntraMaskInvalidMode,
    /// Set the AV1 inter-intra mask width to zero before its geometry guard.
    Av1InterIntraMaskEmptyWidth,
    /// Set the AV1 inter-intra mask height to zero before its geometry guard.
    Av1InterIntraMaskEmptyHeight,
    /// Shorten AV1 inter-intra mask scratch before its length guard.
    Av1InterIntraMaskScratchShort,
    /// Set the AV1 inter-intra mask width outside normative block sizes.
    Av1InterIntraMaskNonNormativeWidth,
    /// Set the AV1 inter-intra mask height outside normative block sizes.
    Av1InterIntraMaskNonNormativeHeight,
    /// Force the unfiltered AV1 copy helper's source-end addition to overflow.
    Av1UnfilteredCopySourceXEndOverflow,
}

thread_local! {
    /// The next fault armed by the current thread, consumed at most once.
    static ACTIVE_FAULT_POINT: Cell<Option<CoverageFaultPoint>> = const { Cell::new(None) };
}

/// Run an operation with one fault point armed on the current thread.
///
/// The matching code consumes the point once. The prior thread-local value is
/// restored when the operation finishes or unwinds, so parallel tests cannot
/// interfere and an unreached point cannot leak into a later operation.
pub(crate) fn with_fault_point<T>(point: CoverageFaultPoint, operation: impl FnOnce() -> T) -> T {
    struct RestoreFaultPoint(Option<CoverageFaultPoint>);

    impl Drop for RestoreFaultPoint {
        fn drop(&mut self) {
            ACTIVE_FAULT_POINT.with(|active| active.set(self.0));
        }
    }

    let previous = ACTIVE_FAULT_POINT.with(|active| active.replace(Some(point)));
    let _restore = RestoreFaultPoint(previous);
    operation()
}

/// Consume a matching fault point from the current thread, if present.
pub(crate) fn take_fault_point(point: CoverageFaultPoint) -> bool {
    ACTIVE_FAULT_POINT.with(|active| {
        if active.get() == Some(point) {
            active.set(None);
            true
        } else {
            false
        }
    })
}
