//! AVIF codec.

pub mod decode;
pub mod encode;
pub mod inspect;

#[cfg(not(target_arch = "wasm32"))]
mod native;

#[cfg(all(coverage, not(target_arch = "wasm32")))]
pub(crate) fn __coverage_exercise_private_branches() {
    decode::__coverage_exercise_private_branches();
    encode::__coverage_exercise_private_branches();
    native::__coverage_exercise_private_branches();
}
