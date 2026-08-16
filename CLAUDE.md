# Repository Instructions

## Codec-Only Scope

- This repository detects, inspects, decodes, and encodes image formats. Do
  not add public or reusable general-purpose resizing, cropping, rotation,
  flipping, filtering, drawing, compositing, color adjustment, or mutable
  image-editor APIs.
- Codec-mandated transforms such as JPEG IDCT, PNG filtering, sample
  reconstruction, and animation disposal must remain private codec internals.
- Keep codec algorithm modules private. Every fallible codec operation,
  including private parser, inspector, decoder, encoder, sequence, compression,
  and native-wrapper helpers, must return `Result` and preserve a meaningful
  failure cause. `Option` is reserved for genuine absence such as optional
  metadata, Pillow-observable palette absence, or an infallible collection
  lookup. Public operations use the canonical structured `ImageResult` API; do
  not add duplicate `try_*` APIs.
- Indexed `P8` data may legitimately have no retained palette when Pillow
  accepts a malformed source without one. When a palette is present,
  `DecodedImage::validate` must revalidate its public RGB and alpha fields,
  entry count, mode compatibility, and every retained index.

## Correctness Authority

- The pinned Pillow oracle and manifest fixtures define observable codec
  behavior, including successful pixels/bytes and structured failures.
- Preserve exact output and error parity when refactoring codec arithmetic,
  parsing, encoding, feature dispatch, or lifecycle code.
- Detection must match the pinned Pillow plugin predicates. Near-miss signatures
  and every observable error-category change require a mutated full-file
  manifest fixture; prefix-only tests are not sufficient evidence.
- A private `Option` fast path is acceptable only when `None` selects a complete
  checked fallback or means a documented non-error state. It must never make
  malformed input indistinguishable from absence. Record every retained
  `Option` category in the accepted implementation review.
- Keep codec implementations in `image-slash-star`; downstream crates consume
  the canonical APIs instead of duplicating detection or codec logic.

## Dependencies And Features

- Keep the runtime dependency policy unchanged: `bytemuck` remains allowed;
  default codecs stay Rust-only; AVIF is an opt-in safe Rust implementation.
- Every image format remains independently feature-gated. ICO intentionally
  enables PNG and BMP.
- Test no-feature, each individual feature, default-feature, all-feature, and
  applicable native/WASM target-capability combinations.

## Tests And Coverage

- Use manifest-driven fixtures for inputs, exact outputs, and errors. Do not
  replace parity assertions with byte-size checks or synthetic magic prefixes.
- When public transfer-model fields need validation, use the closest complete
  Pillow source transformation in the manifest wherever Pillow exposes one.
  Never label a model-only defensive state as Pillow behavior.
- Use Coverage MCP for coverage runs and analysis. Always request line, branch,
  function, and region coverage and restore all four to 100% before accepting
  a slice.
- Run formatting and relevant manifest/feature/target tests after changes.

## Strict Clippy

Strict Clippy is mandatory. The acceptance command is:

```bash
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

- Fix every diagnostic; do not use a warning baseline to declare completion.
- Run equivalent strict commands for every supported native/WASM compilation
  target and isolated feature lane; `--all-targets` alone does not cross-compile.
- Do not add broad `allow` attributes to silence diagnostics. A narrow
  false-positive allowance requires an adjacent invariant explaining why the
  warned behavior is correct and unreachable for invalid input.
- Use checked arithmetic for untrusted dimensions and offsets, explicit
  wrapping only when required by the format, and proven-bounded conversions
  where exact algorithms require them.
- If a lint fix can change pixels, encoded bytes, overflow behavior, or error
  classification, add or update the corresponding Pillow-oracle fixture before
  accepting it.
- Integration tests and coverage hooks must pass the same strict gate; crate
  root allowances do not apply to them.
