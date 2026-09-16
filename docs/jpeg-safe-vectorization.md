# JPEG implementation and vectorization

JPEG runtime code remains safe Rust. The production kernel boundary is
`src/codecs/jpeg/kernels.rs`, with exact scalar reference behavior and safe
`wide` arithmetic where enabled. `jpeg-wide-color` is an opt-in color-conversion
candidate; its existence is not a general performance claim.

Evaluate a change using the complete [public API benchmark](BENCHMARKING.md),
including odd dimensions, quality, chroma sampling, progressive/optimized modes,
and grayscale/CMYK. A faster inner loop can lose end-to-end after allocation,
conversion, or routing costs. Keep context/output lifetime inside both measured
implementations' boundaries.

Do not import architecture-specific unsafe intrinsics, unchecked pointer loads,
uninitialized output buffers, or fixture-specific fast paths to claim parity.
Preserve exact fixed-point rounding, saturation, and byte output with maintained
Pillow cases. Document the earliest reference divergence beside a subtle fix.

Historical rejected candidates and their local measurement diaries remain in
Git history. Current evidence comes from the maintained same-host benchmark,
source-bound reports, and the full fixture matrix.
