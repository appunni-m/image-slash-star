# JPEG safe-vectorization boundary

This document records what was learned from the separate
`/Users/lazytrot/work/vectorization` experiment, what safe batch boundaries
were integrated into the production JPEG codec, and which architecture-
specific intrinsic files were deliberately not copied.

## What was evaluated

The vectorization candidate was a dirty experiment branch (`25198d5`) rather
than a small, reviewable patch. Its JPEG source contained unsafe operations in
these groups:

| Candidate area | Unsafe surface | Decision |
| --- | --- | --- |
| `decode/idct.rs` | AArch64 NEON IDCT, dequantization, transposed workspace, and direct output stores | Do not import; retain the audited fixed-point implementation and use safe batch boundaries around it. |
| `decode/neon.rs`, `decode/x86.rs`, `decode/wasm.rs` | Architecture intrinsics, raw loads/stores, narrowing, and target-feature entry points | Do not import; these are target-specific unsafe implementations. |
| `encode/neon.rs`, `encode/x86.rs`, `encode/wasm.rs` | RGB conversion, FDCT/quantization, downsampling, pointer loads, and SIMD stores | Do not import; the candidate evidence includes rejected unsafe bounds-elision probes. |
| `decode/decode.rs` | Uninitialized output allocation and raw-slice reconstruction | Do not import; safe `Vec`/slice ownership stays in the production path. |
| Candidate `lib.rs` | `no_mangle` diagnostic probe exports | Do not import; these are evidence harness hooks, not codec behavior. |

The audit counted 168 occurrences of the `unsafe` token across the eight
candidate JPEG implementation files. That count includes `unsafe` attributes,
function declarations, blocks, and safety comments; it is an inventory count,
not a claim that every occurrence is an independent unsafe operation.

## Safe path that landed

`src/codecs/jpeg/kernels.rs` is the production boundary. It provides:

- an exact scalar RGB→YCbCr pixel reference;
- four-lane safe SIMD arithmetic through the `wide` abstraction;
- eight-pixel batching with a scalar tail; and
- the matching YCbCr→RGB batch path with safe slice bounds.

The public codec control flow still owns image dimensions, MCU padding,
malformed-input recovery, cancellation, and tails. The scalar pixel functions
remain the semantic reference. `wide` supplies safe Rust calls; no
`std::arch` intrinsic, raw pointer, `unsafe` block, or unsafe allowance exists
in the integrated JPEG source. The same rule applies to
`src/bin/jpeg-runtime.rs`: the benchmark driver uses checked conversions and
does not weaken the workspace safety policy.

The decoder uses the YCbCr→RGB batch path by default. The encoder's RGB→YCbCr
batch is available behind the opt-in `jpeg-wide-color` feature, but its
complete-path measurements were not faster than the existing scalar encoder,
so it is not promoted into the default path.

The release AArch64 artifact was inspected after integration: the opt-in RGB
kernel contains `mul.4s`, `mla.4s`, and `sshr.4s`, while the default reverse
kernel contains four-lane add and clamp operations. This is static artifact
evidence for the two color kernels only; it is not a claim that the entire JPEG
evidence for the two color kernels only; it is not a claim that the entire JPEG
codec is vectorized or that SIMD lowers to the same instructions on every
target.

## Runtime probes and production comparison

The checked-in `src/bin/jpeg-runtime.rs` runs the public JPEG encode/decode
APIs in release mode. Its workload helper has a unit test; only the thin CLI
environment-reading wrapper is excluded from coverage. It remains a small
development probe for size scaling, not the release comparison: it does not
run a TurboJPEG control and does not cover the complete option matrix.

The release comparison is the checked-in
[`benchmarks/jpeg-production`](../benchmarks/jpeg-production/README.md)
harness. It calls the public Rust API and the public TurboJPEG 3 API in fresh
single-threaded operations, alternates implementation order, uses five rounds,
and reports the median of per-round medians. It includes RGB, grayscale, and
CMYK; 4:2:0, 4:2:2, and 4:4:4; quality extremes; optimized and progressive
output; restart markers; and odd dimensions. It records raw JSONL, linked
library identity, compiler flags, hardware, load averages, Git state, output
hashes, and the complete summary. The result is valid only for that exact host,
revision, build, API boundary, and matrix.

The earlier narrow probe produced these medians:

| RGB image | Encode | Decode |
| --- | ---: | ---: |
| 8×8 | 3.375 µs | 5.666 µs |
| 32×32 | 18.625 µs | 17.417 µs |
| 128×128 | 218.458 µs | 176.000 µs |
| 256×256 | 1.228 ms | 691.209 µs |

The 50 µs expectation is therefore met for the small 8×8 and 32×32 workload;
larger images have proportionally more pixels. Enabling `jpeg-wide-color`
made the complete encoder workload slower in the same release probe (for
example, 299.084 µs versus 218.458 µs at 128×128), so that candidate remains
available for experiments without changing the default production path. The
production matrix is the source of truth for current TurboJPEG comparisons.

No result from a selected image size, kernel-only timer, altered boundary,
buffer-reuse path, LTO/PGO build, or disabled TurboJPEG SIMD path is a release
claim.

## Verification contract

Every batch kernel has a focused test comparing its full batches and tail to
the scalar pixel reference. JPEG fixture parity and the complete managed
coverage run remain separate gates. A timing result is reported only for the
exact benchmark workload and image dimensions; it is not used as proof of
SIMD execution or as a substitute for parity.

To audit the production safety boundary again:

```text
rg -n "\\bunsafe\\b" src/codecs/jpeg src/bin/jpeg-runtime.rs
```

The expected result is empty. The only intentional unsafe Rust in the project
is the separately documented native AVIF bridge at
`src/codecs/avif/native.rs`; third-party dependency internals are outside the
repository's Rust source and are not counted as project unsafe code.
