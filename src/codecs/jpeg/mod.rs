// Modified Rust port copyright (c) 2026 Appunni M.
// Derived from libjpeg-turbo/IJG sources; see third_party/libjpeg-turbo/.

//! JPEG codec.

pub mod decode;
pub mod encode;

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    decode::__coverage_exercise_private_branches();
    encode::__coverage_exercise_private_branches();
}
