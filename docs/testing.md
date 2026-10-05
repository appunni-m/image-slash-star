# Development and command reference

Use GNU Make from the repository root. Rust 1.96.1 and the matching formatter,
Clippy, and WASM targets are pinned in `rust-toolchain.toml`. `make lint` also
uses the pinned `nightly-2026-07-16` Clippy toolchain to check coverage-only
code paths; install that nightly with its `clippy` component before running the
full lint command. Documentation uses Python 3.12.10 in CI and hash-locked
MkDocs dependencies.

On hosts without the x86-64 Linux target installed, including ARM hosts, prepare
the target before running `make lint-x86-simd`:

```sh
rustup target add x86_64-unknown-linux-gnu --toolchain 1.96.1
```

## Common commands

| Task | Command | Effect |
| --- | --- | --- |
| Help | `make help` | Lists contributor commands without running Cargo metadata |
| Build | `make build` | Compiles the locked default-feature crate |
| Format / fix | `make fmt` / `make fmt-fix` | Checks / rewrites Rust formatting |
| Contracts | `make verify` | Legal, fixture, capability, roadmap, coverage-origin, and release-tool checks |
| Lint | `make lint` | Strict debug/release, coverage-hook, and JPEG benchmark Clippy |
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

### Optional native coverage exporter

The pinned Rust LLVM coverage reader can select a hash-zero unused mapping
before a real mapping with the same function name. Historical exports of the
same profile and object set depended on object order. The optional native patch
recognizes complete unused mappings, prefers real mappings even with zero
executions, and retains unmatched unused source regions. It changes mapping
selection in LLVM itself; it does not rewrite report counters or omit sources.

The supported setup recipe is native macOS ARM64 with
`nightly-2026-07-16` (Rust commit
`d0babd8b6b05ef9bb65d42f928cef4129d64cf65`, LLVM 22.1.8). Independent source
review and native builds passed on that host. A fresh seven-executable campaign
retained all 110 repository sources and produced identical JSON and LCOV for 18
order labels (17 distinct object orders). All 64 restored positive functions
matched their exact merged-profile tuples; 397 unused-only functions stayed at
zero. A genuine raw-profile subset also preserved real zero-count branch
mappings with two and seven branches. Positive foreign-hash unmatched-unused
retention has source-policy proof only: no natural runtime control occurred.
The descriptor records conditional admission for this pinned native platform.
A separately bound maintained v3 recipe built from the prepared source, passed
real no-op/concurrent reuse, and matched the audited native exports in all 17
distinct object orders. The integrated canonical coverage command also passed
all executed tests and the alpha floors. Construction and command checks are
recorded separately in [the evidence guide](EVIDENCE.md); the descriptor records
the producer pin and its admission scope. The
first-source-fetch path remains source-reviewed and unexercised. This is an
opt-in contributor tool; another platform and an upstream fixed release have
not been verified.

Install Git, CMake, Ninja and the pinned Rust toolchain with
`llvm-tools-preview` through the normal contributor setup, then run:

```bash
make coverage-exporter-setup
make coverage-with-exporter
make coverage-with-exporter COVERAGE_EXPORTER_TARGET=coverage-complete
```

The first setup fetches the exact
[Rust LLVM source commit](https://github.com/rust-lang/llvm-project/tree/52ed14fcd56afc30f9cccd8ca8ce237c2eef7e04),
checks the source and patch hashes, and builds only `llvm-cov` with two jobs.
LLVM is governed by its [upstream license](https://github.com/rust-lang/llvm-project/blob/52ed14fcd56afc30f9cccd8ca8ce237c2eef7e04/llvm/LICENSE.TXT).
Rust runtime dependencies and installed toolchain files remain unchanged.
`COVERAGE_EXPORTER_SOURCE=/absolute/path/to/pinned/llvm-project` reuses a
prepared clean checkout or the exact reviewed patched checkout without another
download. `COVERAGE_EXPORTER_PREFIX=/absolute/path/to/exporter` selects an
isolated build directory; its default is under the removable `target/` tree.
Setup verifies the descriptor, source, native executable, build configuration
and coupled profile tool on every reuse. Changed or stale bindings fail.

The explicit coverage target uses the documented
[`LLVM_COV` and `LLVM_PROFDATA` overrides](https://github.com/taiki-e/cargo-llvm-cov/blob/v0.8.7/README.md#environment-variables)
only for its recursive Make invocation. Conflicting caller overrides, custom
LLVM flags and unsupported toolchain or cross-target settings are rejected.
The setup also rejects custom native build flags or a CMake toolchain file
instead of changing caller settings. The optional alias accepts report names
with spaces or apostrophes. It rejects dollar signs, backticks, double quotes,
backslashes and CR/LF before running a coverage command: the unchanged inner
Make recipes cannot represent those characters literally.
Normal `make coverage` keeps the normal Cargo command, alpha floors, complete
source denominator and test-failure policy. The strict command still requires
100% for all four metrics. Run setup separately before using the explicit
target; a missing or invalid receipt never triggers a silent download.

## Submit a change

Run the narrow failing lane first, then the relevant full contract. Explain
the first observed divergence and how the pinned oracle behaves. Keep durable
implementation nuance beside the relevant source, with public guidance here
when it changes contributor procedure. Follow [Contributing](../CONTRIBUTING.md).

Run `make test-feature-matrix` to repeat the complete native, WASI, and WASM feature lanes independently.
