# Development and command reference

Use GNU Make from the repository root. Rust 1.96.1 and the matching formatter,
Clippy, and WASM targets are pinned in `rust-toolchain.toml`. Documentation
uses Python 3.12.10 in CI and hash-locked MkDocs dependencies.

## Common commands

| Task | Command | Effect |
| --- | --- | --- |
| Help | `make help` | Lists contributor commands without running Cargo metadata |
| Build | `make build` | Compiles the locked default-feature crate |
| Format / fix | `make fmt` / `make fmt-fix` | Checks / rewrites Rust formatting |
| Contracts | `make verify` | Legal, fixture, capability, roadmap, coverage-origin, and release-tool checks |
| Lint | `make lint` | Strict library and JPEG benchmark Clippy |
| Tests | `make test` | Rustdoc, all-feature tests, and feature/target lanes |
| Package example | `make example` | Executes the self-contained public PNG example |
| Coverage | `make coverage` | Measures the full source denominator and enforces alpha floors |
| Complete coverage | `make coverage-complete` | Requires all four metrics to reach 100% |
| Package inspection | `make package-verify` | Builds, extracts, and tests the distributable archive |
| Documentation setup | `make docs-setup` | Creates an isolated environment and installs the hashed lock |
| Site build / preview | `make docs-build` / `make docs-serve` | Builds checked HTML / serves localhost:8000 |
| JPEG oracle setup | `make bench-setup` | Downloads, verifies, and builds pinned TurboJPEG under target |
| JPEG comparison | `make bench` | Runs all 20 configurations for encode and decode, five rounds each |
| Full local CI | `make ci` | Runs validation, tests, coverage, and packaging |

Setup and initial toolchain/dependency builds may need network access.
Documentation builds do not execute benchmarks. Stop the local preview with
Ctrl-C. Benchmark output directories must be new or empty; use
`BENCH_OUTPUT=target/benchmarks/jpeg/run-2` for a repeat.

## Parity inputs and outputs

The maintained manifest, complete encoded inputs, and pinned Pillow oracle
define observable behavior. Generate references using the documented canonical
macOS ARM64 environment. Compare formats, modes, metadata, frames, pixels,
palettes, deterministic output bytes, and structured error behavior where the
contract requires them.

Do not replace exact comparisons with prefix checks, dimensions, or hashes
chosen to match the Rust output. Add the corresponding public input when
changing codec behavior. Preserve planned and not-applicable outcomes.

Some oracle outputs are checked in because clean CI source checkouts consume
them. They are reproducible through maintained generators, but regeneration
requires the exact source assets and pinned oracle. In particular, AV1's index
and five sidecars form one input set. Do not delete them just because they are
generated. [AVIF provenance](../tests/fixtures/input/images/avif/README.md)
describes how to regenerate them.

## Coverage and focused work

Use the repository's registered Coverage MCP flow for incremental source
claims, with the exact selected fixture IDs and source revision. Report all
four aggregate totals. Selected-case reports do not establish full coverage,
and missing observations are not automatically regressions.

The alpha floors are 59% lines, 46% branches, 52% functions, and 58% regions.
No source exclusions are added to reach those floors. Complete coverage retains
100%; every executed test must pass. The [evidence guide](EVIDENCE.md) preserves
the distinction between old claim-ledger measurements and the release report.

## Submit a change

Run the narrow failing lane first, then the relevant full contract. Explain
the first observed divergence and how the pinned oracle behaves. Keep durable
implementation nuance beside the relevant source, with public guidance here
when it changes contributor procedure. Follow [Contributing](../CONTRIBUTING.md).

Run `make test-feature-matrix` to repeat the complete native, WASI, and WASM feature lanes independently.
