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
    /// Report the AV1 display-plane copy allocation failure before reserving
    /// its output sample buffer.
    Av1DisplayPlaneCopyAllocation,
    /// Force AV1 temporal motion-field reservation to fail during frame completion.
    Av1TemporalMotionFieldReservation,
    /// Force AV1 projected temporal-field reservation to fail during frame setup.
    Av1ProjectedTemporalFieldReservation,
    /// Force AVIF sequence-frame reservation to fail before any frame is decoded.
    AvifSequenceFrameReservation,
    /// Force AV1 super-resolution position reservation to fail while resizing a frame.
    Av1SuperResolutionPositionReservation,
    /// Force AV1 super-resolution output-plane reservation to fail.
    Av1SuperResolutionPlaneReservation,
    /// Force JPEG multi-scan coefficient retention reservation to fail.
    JpegCoefficientBufferReservation,
    /// Force AV1 partial monochrome tile-state retention to fail.
    Av1MonochromeTileReservation,
    /// Force AV1 partial color tile-state retention to fail.
    Av1ColorTileReservation,
    /// Force AVIF color-grid cell retention to fail.
    Av1GridCellReservation,
    /// Force assembled AV1 monochrome loop-filter metadata reservation to fail.
    Av1AssembledMonochromeLoopFilterMetadataReservation,
    /// Force assembled AV1 monochrome CDEF active-map reservation to fail.
    Av1AssembledMonochromeCdefActiveMapReservation,
    /// Force AV1 per-tile luma cell storage reservation to fail.
    Av1TileCellReservation,
    /// Force AV1 per-tile chroma cell storage reservation to fail.
    Av1ChromaTileCellReservation,
    /// Force AV1 decoded block metadata reservation to fail.
    Av1TileBlockMetadataReservation,
    /// Force AV1 assembled loop-filter metadata reservation to fail.
    Av1AssembledLoopFilterMetadataReservation,
    /// Force AV1 partition-tree node reservation to fail.
    Av1PartitionNodeReservation,
    /// Force the AV1 SGR output-plane reservation to fail during restoration.
    Av1SgrRestorationOutputReservation,
    /// Force an AV1 SGR intermediate-buffer reservation to fail.
    Av1SgrIntermediateReservation,
    /// Force AV1 striped restoration scratch reservation to fail.
    Av1RestorationStripeScratchReservation,
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
