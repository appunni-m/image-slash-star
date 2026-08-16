//! AVIF codec.

pub mod decode;
pub mod encode;
pub mod inspect;

mod av1;
mod container;
mod samples;

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    av1::__coverage_exercise_private_branches();
    container::__coverage_exercise_private_branches();
    decode::__coverage_exercise_private_branches();
    encode::__coverage_exercise_private_branches();
    samples::__coverage_exercise_private_branches();
}

#[cfg(coverage)]
pub(crate) fn __coverage_entropy_reference_trace()
-> crate::codecs::CodecResult<Vec<crate::Av1EntropyTraceState>> {
    av1::__coverage_entropy_reference_trace()
}

#[cfg(coverage)]
pub(crate) fn __coverage_reconstruction(
    data: &[u8],
) -> crate::codecs::CodecResult<Option<crate::Av1ReconstructionTrace>> {
    av1::__coverage_reconstruction(data)
}

#[cfg(coverage)]
pub(crate) fn __coverage_sweep_first_leaf(data: &[u8]) {
    av1::__coverage_sweep_first_leaf(data);
}
