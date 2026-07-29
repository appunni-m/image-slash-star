//! AVIF codec.

pub mod decode;
pub mod encode;
pub mod inspect;

mod av1;
mod container;
mod samples;

#[cfg(not(target_arch = "wasm32"))]
mod native;

#[cfg(all(coverage, not(target_arch = "wasm32")))]
pub(crate) fn __coverage_exercise_private_branches() {
    av1::__coverage_exercise_private_branches();
    container::__coverage_exercise_private_branches();
    decode::__coverage_exercise_private_branches();
    encode::__coverage_exercise_private_branches();
    native::__coverage_exercise_private_branches();
    samples::__coverage_exercise_private_branches();
}

#[cfg(all(coverage, not(target_arch = "wasm32")))]
pub(crate) fn __coverage_entropy_reference_trace()
-> Result<Vec<crate::Av1EntropyTraceState>, &'static str> {
    av1::__coverage_entropy_reference_trace()
}

#[cfg(all(coverage, not(target_arch = "wasm32")))]
pub(crate) fn __coverage_reconstruction(data: &[u8]) -> Option<crate::Av1ReconstructionTrace> {
    av1::__coverage_reconstruction(data)
}

#[cfg(all(coverage, not(target_arch = "wasm32")))]
pub(crate) fn __coverage_sweep_first_leaf(data: &[u8]) {
    av1::__coverage_sweep_first_leaf(data);
}
