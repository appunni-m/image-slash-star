# Contributing

Thank you for helping make `image-slash-star` accurate, portable, and easy to
audit.

By submitting a contribution, you agree that it may be distributed under the
repository's licensing terms. Follow [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)
in project spaces.

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
- Read the current boundaries in
  [docs/architecture.md](docs/architecture.md) and planned work in
  [docs/roadmap-new.md](docs/roadmap-new.md). The older
  [roadmap audit](docs/roadmap.md) is historical context, not the work queue.

## Set up the repository

The exact Rust toolchain, formatter, Clippy component, and WASM target are
declared in `rust-toolchain.toml`.

```bash
git clone https://github.com/appunni-m/image-slash-star.git
cd image-slash-star
cargo check --locked
```

Default codecs need no native library. Native all-feature work requires the
fixed stack in [docs/avif.md](docs/avif.md#native-setup).

## Verification

For documentation or API-comment changes, run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps --locked
cargo test --doc --all-features --locked
python3 scripts/verify_third_party_licenses.py
```

Codec, feature, or error behavior additionally requires:

```bash
cargo test --locked --all-features --test coverage_matrix_tests
scripts/test_feature_matrix.sh
```

JPEG performance changes additionally require the fixed, same-machine
production comparison in
[`benchmarks/jpeg-production/README.md`](benchmarks/jpeg-production/README.md).
Report the complete matrix, not a selected image size or a kernel-only timing.

Repository agents run coverage only through Coverage MCP. Every accepted codec
slice must retain 100% line, branch, function, and region coverage.

The integration suite is manifest-driven. Add or update a complete row in
`manifest.yaml`, generate the exact Pillow reference, and compare actual bytes
rather than adding isolated unit tests, prefix-only probes, or file-size
assertions. Use the pinned macOS arm64 oracle described in
[docs/testing.md](docs/testing.md) when regenerating fixtures.

Update current documentation in the same change when a public API, feature,
target, option, error, package, or scope contract moves. Do not create
per-sweep progress logs under `docs/`.

## Pull requests

Keep each pull request focused. Explain the Pillow behavior being matched,
identify the first divergent pipeline stage when fixing parity, and include
the exact verification commands and results. Identify copied or translated
source and its license, or state that the change is original.

Do not include sensitive or malicious fixtures in a public pull request.
Follow [SECURITY.md](SECURITY.md) for private vulnerability reports.
