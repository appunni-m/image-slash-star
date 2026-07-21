//! GIF codec.

pub mod decode;
pub mod encode;

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    encode::__coverage_exercise_private_branches();
}
