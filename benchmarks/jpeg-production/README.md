# JPEG production comparison

The maintained entry point is `make bench` from the repository root, after
`make bench-setup` builds the pinned TurboJPEG oracle. Both implementations
run complete public codec operations on the same inputs and host.

Read the [benchmark protocol](../../docs/BENCHMARKING.md) for timing boundaries,
repeat policy, correctness limits, the CMYK convention difference, and output
artifacts. Results are published on the
[benchmark site](https://appunni-m.github.io/image-slash-star/benchmarks/).

The runner in this directory remains the source of the fixed 20-configuration
matrix. Results belong under ignored `target/`, not in source control.
