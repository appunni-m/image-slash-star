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

The Rust benchmark binary also accepts `encode-token` with the same arguments
as `encode` to time the public `encode_with_token` path on a deterministic
generated image. For example:

```sh
cargo run --manifest-path benchmarks/jpeg-production/rust/Cargo.toml --release -- \
  encode-token 512 512 gray 85 420 0 0 0 300
```

The command reports output length and hash before timing and includes the
operation-owned output lifetime in each sample.
