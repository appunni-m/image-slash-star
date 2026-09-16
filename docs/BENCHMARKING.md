# Benchmark methodology

This is the contributor guide for collecting and interpreting measurements.
For comparisons, start with [benchmark results](https://appunni-m.github.io/image-slash-star/benchmarks/).

The [benchmark site](https://appunni-m.github.io/image-slash-star/benchmarks/)
compares public JPEG operations with TurboJPEG on the same host. It measures
JPEG encode/decode, not other codecs, application I/O, or image editing.

## Reproduce the full comparison

```sh
make bench-setup
make bench
make docs-benchmark
make docs-build
```

The setup target downloads the official libjpeg-turbo **3.2.0** source archive,
checks its SHA-256, and builds a release oracle under `target/`. It does not
add a native runtime dependency to the Rust crate. CI uses the same target.
Select a different existing installation with `TURBOJPEG_PREFIX`; its identity
must be recorded and comparable before making a regression claim.

Use a new `BENCH_OUTPUT` for each repeat. The runner requires an odd number of
rounds, at least three; the maintained default is five. CI records the complete
20-configuration encode/decode matrix.

## Timing boundary

Each timed encode starts from the same deterministic RGB, grayscale, or CMYK
bytes and includes operation-owned output destruction. Each decode starts from
the same complete JPEG byte slice and includes decoded-output destruction.

Rust calls its public codec API. C creates and configures a TurboJPEG 3 handle,
calls the public operation, frees output, and destroys the handle. Input
generation, sample storage, build work, and one-time fixture I/O are outside
both timed operations.

Both use accurate integer DCT behavior. Normal TurboJPEG SIMD stays enabled;
the comparison does not disable it to improve the Rust ratio. Rust uses ordinary
locked release builds, and the thin C harness uses `-O3`. There is no hidden
LTO, PGO, or native-CPU tuning in the default comparison.

## Samples and correctness

Five rounds alternate implementation order. The summary is the median of the
five round medians. Raw records retain each round's median, P95, minimum, result
length/hash, order, and host load. A percentile of these medians would not equal
the pooled per-operation percentile; the public table leaves unavailable
spread unmeasured instead of inventing it.

The configurations vary dimensions, quality, chroma sampling, progressive and
optimized encoding, restart markers, grayscale, CMYK, and odd edges.
FNV output-hash and byte-length comparisons are diagnostic equality checks,
not substitutes for the full Pillow parity suite.

CMYK has a specific convention difference: Pillow/image-slash-star expose
conventional CMYK, while direct TurboJPEG decode exposes JPEG's stored Adobe
sample convention. Unequal CMYK hashes must remain visible with that explanation.
Do not label every timing row pixel-parity-proven.

## Interpret results

Lower latency means less time only for the same operation and configuration.
The original `rust_over_turbo` ratio below 1 means Rust took less time; above
1 means TurboJPEG took less time. Do not reverse the ratio or average unrelated
operations into a project-wide claim.

Control host load, power, compiler/build flags, linked library, inputs, and
repeat policy. Hosted-runner observations are not a calibrated performance lab.
Keep all rows and raw samples, repeat noisy experiments, and use separate
profiling runs to investigate causes.

## Public data and CI

The Benchmark workflow retains `metadata.json`, `summary.csv`, and
`raw.jsonl`. Its validated public snapshot feeds this repository's Pages
workflow. The original report hashes, source revision, environment, measurement
policy, and every result remain identifiable. Local paths and hostnames are
omitted from the public presentation.

Fixture-harness timing from `benchmark_fixture_workloads.py` measures harness
execution cost and artifact sizes. It is not a codec throughput comparison and
is not mixed into these tables.

References: [libjpeg-turbo build guidance](https://github.com/libjpeg-turbo/libjpeg-turbo/blob/3.2.0/BUILDING.md),
[Rust Performance Book](https://nnethercote.github.io/perf-book/benchmarking.html),
and [Criterion analysis](https://bheisler.github.io/criterion.rs/book/analysis.html).
