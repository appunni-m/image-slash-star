# Contributing

Small reproductions, documentation corrections, portability fixes, and
fixture-backed codec work are welcome. Follow the [code of conduct](CODE_OF_CONDUCT.md)
and report vulnerabilities through [security](SECURITY.md).

Start with [maturity](docs/MATURITY.md), [architecture](docs/architecture.md),
and the [command reference](docs/testing.md). Discuss large API or dependency
changes in an issue before implementing them.

## Prepare and verify

```sh
make help
make build
make verify
make fmt
make lint
```

For documentation-only changes, also run `make docs-setup`,
`make docs-test docs-lint`, and `make docs-build`. For codec or feature
changes, run `make test` and the matching source-bound coverage flow.
JPEG performance changes require the complete [benchmark matrix](docs/BENCHMARKING.md).

## Implementation rules

Keep runtime code safe Rust and codec-only. Do not add public image editing,
native fallback libraries, or unsafe exceptions. Use the existing
`bytemuck`/`wide` feature policy and explain any proposed new dependency.
Gate each format and keep algorithms under its private codec module.

Preserve structured failures and the earliest meaningful error cause. A mode,
palette, frame, policy, or error-path change needs a corresponding complete
public manifest input. Diagnose the first C/Pillow-versus-Rust divergence before
changing arithmetic or output handling.

Never remove a failing input, change expected output, or weaken a threshold to
obtain a pass. Generated outputs remain reproducible evidence and some are
required by clean CI checkouts. Follow the fixture provenance and generator
instructions before changing them.

## Pull requests

Explain the observable problem, resulting behavior, exact case IDs, commands
run, results, and remaining limits. Record subtle reference behavior beside the
implementation. Retain authorship and license notices for translated code.

Keep public guides current in the same change. Superseded session diaries
belong in Git history. Package releases use the separate
[maintainer process](RELEASING.md).

## Workflow validation

Run `make workflows-check` before changing GitHub Actions. This validates all
workflow YAML, expressions, action inputs, and job dependencies with actionlint
1.7.12; its archive is checksum-verified and cached under `target/`. The first
run downloads the tool. Shell and Python lint remain separate checks. CI runs
this gate on every commit. Benchmark harness/workflow changes on main also run
the benchmark immediately, in addition to the weekly and manual triggers.
