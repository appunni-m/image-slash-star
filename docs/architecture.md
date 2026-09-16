# Architecture

The public API detects, inspects, decodes, and encodes image bytes. Codec-
mandated reconstruction, color conversion, filtering, and animation disposal
are private codec internals. There is no public image-editor API.

| Location | Responsibility |
| --- | --- |
| `src/lib.rs` | Public dispatch and operation entry points |
| `src/types/` | Structured images, modes, metadata, and errors |
| `src/decode_policy.rs`, `src/encode_policy.rs` | Caller-selected resource and work limits |
| `src/codecs/<format>/` | Private format parsing, reconstruction, and encoding |
| `manifest.yaml`, `tests/fixtures/` | Maintained input/oracle contract and generated comparison matrix |
| `scripts/` | Reproducible generators, validators, release and documentation tooling |
| `benchmarks/jpeg-production/` | Same-host public JPEG API comparison |
| `docs/` | Public user and contributor guides |

## Runtime boundary

Production code is safe Rust. `bytemuck` supports data utilities, and the
JPEG/AVIF features enable `wide` for safe SIMD abstractions. Native codec
projects are offline oracles and attribution sources, never runtime shortcuts.

Format features are independent except explicit dependencies such as ICO's
BMP/PNG payload support. Rust implementations remain the same source across
native and WASM targets; successful compilation alone does not establish
target behavior.

## Data and errors

Preserve exact mode, dimensions, pixels, palettes, frames, metadata, and
structured failures where the public contract compares them. Byte layout and
codec-required transforms must not silently change through refactoring.

Use checked dimensions and meaningful `ImageResult` failures for fallible
codec operations. Keep optional absence distinct from malformed or unsupported
data. Error message prose is diagnostic, while typed recovery remains public.

## Evidence ownership

Generators and version-matched Pillow/codec oracles own reference creation.
Tests compare complete inputs and observations. Do not modify expected output,
hashes, selectors, or thresholds to obtain a pass. Keep undefined or unsupported
states explicit.

The [evidence guide](EVIDENCE.md) is the canonical public rendering of the
claim ledger. [Testing](testing.md) describes the maintained validation flow.
Internal session diaries have been consolidated into these guides and source
comments; historical detail remains in Git history.
