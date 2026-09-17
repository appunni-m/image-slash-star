//! Assertions for coverage-only fixture setup and observations.
//!
//! A failed fixture prerequisite must fail the test, never skip the exercise
//! or substitute default input. These helpers preserve that contract while
//! attaching the caller's context to the failure.

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

/// Convert the byte length of an AV1 fixture to its exact bit length.
#[cfg(feature = "avif")]
#[track_caller]
pub(crate) fn bit_len(bytes: usize) -> usize {
    require_some(bytes.checked_mul(8), "fixture bit length must fit usize")
}
