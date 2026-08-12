# JPEG production comparison

This benchmark compares the public `image-slash-star` JPEG API with the public
TurboJPEG 3 API on the same machine. It is deliberately an end-to-end release
comparison, not a kernel microbenchmark.

## Fair boundary

Each timed encode starts with the same deterministic RGB, grayscale, or CMYK
bytes and ends after the operation-owned JPEG output has been destroyed. Each
timed decode starts with the same JPEG byte slice and ends after the decoded
output has been destroyed.

- Rust calls public `encode` or `decode` once per sample.
- C calls `tj3Init`, configures the handle, calls `tj3Compress8` or
  `tj3Decompress8`, frees the output, and destroys the handle once per sample.
- Inputs, benchmark sample arrays, and one-time fixture file I/O stay outside
  the timed operation in both implementations.
- Both use accurate integer DCT behavior. TurboJPEG's `FASTDCT` is disabled;
  its normal release SIMD, including its Huffman SIMD encoder, remains enabled.
- The benchmark is single-threaded. It does not reuse codec contexts or output
  buffers, call TurboJPEG from the Rust implementation, alter image boundaries,
  enable LTO/PGO/`target-cpu=native`, or remove inconvenient configurations.
- Five rounds run in alternating implementation order. The checked summary is
  the median of the five per-round medians; `raw.jsonl` preserves every round.

The matrix varies dimensions, quality, 4:2:0/4:2:2/4:4:4 sampling, baseline,
optimized, progressive, restart-marker, grayscale, CMYK, and odd-edge cases.
It is representative, not a claim to enumerate every legal JPEG parameter.

CMYK needs one semantic caveat: Pillow/image-slash-star exposes conventional
CMYK values, while a direct TurboJPEG CMYK decode exposes JPEG's stored Adobe
sample convention. Therefore equal CMYK output hashes are not required. Both
decoders still consume exactly the same checked input bytes.

## Run

On the macOS Arm host used for the checked receipt, Homebrew installs headers
and `libturbojpeg` under `/opt/homebrew`:

```sh
python3 benchmarks/jpeg-production/run_matrix.py \
  --rounds 5 \
  --output benchmarks/jpeg-production/results/local
```

Use `--turbojpeg-prefix` (or `TURBOJPEG_PREFIX`) for another release install.
The runner records the linked library, compiler versions, OS/hardware metadata,
Git revision/status, commands, load averages, raw reports, hashes, and summary.
It uses ordinary `cargo build --release --locked` for Rust and `cc -O3` only
for the thin C timing harness.

Interpret `rust_over_turbo` as follows: below `1.0` means image-slash-star was
faster; `1.0` is equal; above `1.0` means TurboJPEG was faster. Results apply
only to the recorded host, revisions, workload, and API boundary.
