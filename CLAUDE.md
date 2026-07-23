# Repository Instructions

## Correctness Authority

- The pinned Pillow oracle and manifest fixtures define observable codec
  behavior, including successful pixels/bytes and structured failures.
- Preserve exact output and error parity when refactoring codec arithmetic,
  parsing, encoding, feature dispatch, or lifecycle code.
- Keep codec implementations in `image-slash-star`; downstream crates consume
  the canonical APIs instead of duplicating detection or codec logic.

## Dependencies And Features

- Keep the runtime dependency policy unchanged: `bytemuck` remains allowed;
  default codecs stay Rust-only; AVIF uses the pinned optional native stack.
- Every image format remains independently feature-gated. ICO intentionally
  enables PNG and BMP.
- Test no-feature, each individual feature, default-feature, all-feature, and
  applicable native/WASM target-capability combinations.

## Tests And Coverage

- Use manifest-driven fixtures for inputs, exact outputs, and errors. Do not
  replace parity assertions with byte-size checks or synthetic magic prefixes.
- Use Coverage MCP for coverage runs and analysis. Always request line, branch,
  and region coverage and restore all three to 100% before accepting a slice.
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
