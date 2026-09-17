//! AVIF codec.

pub mod decode;
pub mod encode;
pub mod inspect;

mod av1;
mod container;
mod samples;

#[derive(Clone, Copy)]
enum PortableYuvMatrix {
    Bt601,
    Bt2020NonConstant,
    Bt601Limited,
}

/// Select only independently witnessed color declarations. `None` means
/// absence of a supported conversion, not failure while decoding samples.
fn portable_yuv_matrix(
    cicp: [u32; 3],
    bit_depth: u32,
    full_range: bool,
    subsampling: (bool, bool),
    alpha: bool,
) -> Option<PortableYuvMatrix> {
    match (cicp, bit_depth, full_range, subsampling, alpha) {
        ([1, 13, 6], 8 | 10 | 12, true, (false, false) | (true, false) | (true, true), _) => {
            Some(PortableYuvMatrix::Bt601)
        }
        ([9, 16, 9], 10, true, (false, false), false) => Some(PortableYuvMatrix::Bt2020NonConstant),
        // 10bit.avif actually declares 12-bit limited-range I422 with a
        // full-range monochrome auxiliary plane. MC=2 resolves to I601 in
        // pinned libavif; other unspecified declarations need their own proof.
        ([2, 2, 2], 12, false, (true, false), true) => Some(PortableYuvMatrix::Bt601Limited),
        _ => None,
    }
}

#[cfg(coverage)]
pub(crate) use av1::{color_conversion_trace, select_short_references, temporal_candidate_trace};

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
