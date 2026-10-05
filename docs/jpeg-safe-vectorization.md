# JPEG implementation and vectorization

JPEG runtime code remains safe Rust. The production kernel boundary is
`src/codecs/jpeg/kernels.rs`, with exact scalar reference behavior and safe
`wide` arithmetic. The historical `jpeg-wide-color` feature is retained as a
compatibility alias for `jpeg`; the JPEG feature uses safe SIMD kernels on the
encoder and decoder paths where they are admitted.

The direct baseline and progressive reconstruction paths use the portable
`wide` kernels on AArch64 and x86_64. The progressive path retains its checked
scalar fallback for inputs outside the parser-proven coefficient range. The
x86_64 build uses its SSE2 baseline; the benchmark lane also builds
an AVX2 variant. Production dispatch does not detect AVX2 at runtime, so a
binary compiled with `+avx2` requires an AVX2-capable deployment CPU. Use the
paired same-host measurements before claiming that either x86 build is faster.

The baseline 4:2:0 encoder keeps four FDCT coefficients in packed lanes through
quantization. Safe `u32x4::widening_mul` computes their reciprocal products
together; the possible one-step quotient correction stays exact as a 0-or-1
subtraction. Keep this path byte-identical across the public Pillow JPEG encode
matrix.

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
