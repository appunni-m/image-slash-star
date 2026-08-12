# JPEG safe-vectorization boundary

This document records what was imported from the separate
`/Users/lazytrot/work/vectorization` experiment and why the architecture-
specific intrinsic files were not copied into the production codec.

## What was evaluated

The vectorization candidate was a dirty experiment branch (`25198d5`) rather
than a small, reviewable patch. Its JPEG source contained unsafe operations in
these groups:

| Candidate area | Unsafe surface | Decision |
| --- | --- | --- |
| `decode/idct.rs` | AArch64 NEON IDCT, dequantization, transposed workspace, and direct output stores | Do not import; retain the current scalar reference and use the safe batch boundary. |
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
in the integrated JPEG source.

The decoder uses the YCbCr→RGB batch path by default. The encoder's RGB→YCbCr
batch is available behind the opt-in `jpeg-wide-color` feature, but its
complete-path measurements were not faster than the existing scalar encoder,
so it is not promoted into the default path.

The release AArch64 artifact was inspected after integration: the opt-in RGB
kernel contains `mul.4s`, `mla.4s`, and `sshr.4s`, while the default reverse
kernel contains four-lane add and clamp operations. This is static artifact
evidence for the two color kernels only; it is not a claim that the entire JPEG
codec is vectorized.

## Runtime receipt

The checked-in `src/bin/jpeg-runtime.rs` runs the public JPEG encode/decode
APIs in release mode. On the native AArch64 macOS target, with 1,000 rounds,
the default path produced these medians:

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
available for experiments without changing the default production path.

## Verification contract

Every batch kernel has a focused test comparing its full batches and tail to
the scalar pixel reference. JPEG fixture parity and the complete managed
coverage run remain separate gates. A timing result is reported only for the
exact benchmark workload and image dimensions; it is not used as proof of
SIMD execution.

To audit the production safety boundary again:

```text
rg -n "\\bunsafe\\b" src/codecs/jpeg src/bin/jpeg-runtime.rs
```

The expected result is empty. Third-party dependency internals are outside the
repository's Rust source and are not counted as project unsafe code.
