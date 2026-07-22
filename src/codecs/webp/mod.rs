//! WebP codec.

pub mod decode;
pub mod encode;
pub mod inspect;
mod native;

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    decode::__coverage_exercise_private_branches();
    encode::__coverage_exercise_private_branches();
    native::__coverage_exercise_private_branches();
}
