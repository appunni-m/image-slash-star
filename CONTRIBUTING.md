# Contributing

Thank you for helping make `image/*` accurate, portable, and easy to audit.

## Before opening a change

- Discuss large API or parity changes in an issue first.
- Keep default runtime code safe Rust and free of native-library dependencies.
  AVIF changes must stay inside its opt-in bridge, preserve exact version
  gates, and document every unsafe invariant.
- Preserve `bytemuck` as the only runtime utility dependency unless a proposal
  explains why a new dependency is necessary.
- Keep format-specific code under `src/codecs/<format>/` and gate it with the
  matching Cargo feature.
- Record copied or translated code in `NOTICE.md`, retain its license text, and
  identify the exact upstream version in the source comments.

## Verification

Install the pinned toolchain from `rust-toolchain.toml`, then run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo check --all-targets --no-default-features
```

Native all-feature checks additionally require libavif 1.4.1 built with dav1d
1.5.3 and libaom 3.13.2. Follow the AVIF setup in the README, then run the same
Clippy command with `--all-features` and run
`cargo test --all-features --test coverage_matrix_tests`.

The integration suite is manifest-driven. Add or update a row in
`manifest.yaml`, generate the exact Pillow reference, and compare actual bytes
rather than adding isolated unit tests or assertions about file size alone.
Use the pinned macOS arm64 Pillow oracle described in the README when
regenerating fixtures.

## Pull requests

Keep each pull request focused. Explain the Pillow behavior being matched,
identify the first divergent pipeline stage when fixing parity, and include
the verification commands and results. All contributions are accepted under
the repository's licensing terms.
