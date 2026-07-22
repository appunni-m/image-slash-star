# Coverage sweep active tracker

This file tracks the current retained coverage state and the next bulk fixes.
It is intentionally shorter than `docs/coverage-region-sweep.md`: this is the
active checklist for getting the remaining LLVM source regions and branch to
100%.

## Current retained baseline

- Coverage MCP run: `5bdbffc9-86fe-44a4-bb1d-cfaadd9e16e1`
- Coverage MCP snapshot: `0060828f-ffce-4589-b59a-99a11a8e23c4`
- Command: `all-features-llvm-cov-json-nightly-branch`
- Result: `5 passed / 0 failed`
- Lines: `26811 / 26811` (100%)
- Functions: `1620 / 1620` (100%)
- Branches: `3477 / 3478` (1 missing)
- Regions: `42501 / 42575` (74 missing)

## Remaining gap map

| File | Missing regions | Missing branches | Current reason |
| --- | ---: | ---: | --- |
| `src/codecs/compression/zlib_ng.rs` | 0 | 0 | Complete after converting proven generated-token/matcher invariants to direct operations or invariant `expect(...)` calls. |
| `src/codecs/webp/native/decoder.rs` | 35 | 0 | Missing-`ALPH` parity is fixed; remaining debt is exact `Seek`/`Read` propagation in VP8X/ANIM/ANMF plus normalized animation/alpha regions. |
| `src/codecs/webp/native/lossless.rs` | 29 | 1 | VP8L truncation fixtures reduced public EOF/read propagation debt; the retained branch gap maps to coverage-only generic `BitReader::read_bits` monomorphs, not a Pillow image path. |
| `src/codecs/tiff/decode.rs` | 0 | 0 | Complete after coverage-only malformed header overflow fixtures. |
| `src/codecs/webp/native/vp8.rs` | 9 | 0 | VP8 parser/loop-filter region debt, mostly malformed payload read propagation and macroblock/residual decode propagation. |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 | Top-level `WebPEncoder::encode()` source-region artifact remains after direct writer-failure hook coverage. |

## Batch 54 plan

Goal: run a fixture-first WebP sweep against the retained Batch 52 baseline,
starting with public Pillow-observable states before adding any new private
coverage hooks.

Reverse mapping from retained snapshot
`61ceca33-93de-4d3e-94e8-2a0400689782`:

- `decoder.rs` remaining regions are all line-covered. Fixture-reachable
  targets are:
  - lossy VP8X with `alpha=true` but no top-level `ALPH` chunk;
  - VP8L with the alpha bit set but only the five-byte VP8L signature/header.
- Pillow accepts the missing-`ALPH` lossy VP8X file and returns RGBA output.
  That is a real parity gap, not an expected-error fixture. The Rust decoder
  should fill RGB from VP8 and default alpha to `255` when `ALPH` is absent.
- Pillow rejects the VP8L alpha-header-only file with an oracle error. That is
  an expected-error fixture and should exercise the alpha VP8L `read_image`
  path before failing in the VP8L bitstream.
- Decoder seek/read failures inside `VP8X`, `ANIM`, and nested `ANMF` parsing
  remain hook-only with `Cursor`-backed fixtures: byte fixtures cannot make
  `Seek` fail, and public construction scans enough nested ANMF headers that
  some `read_frame` header errors cannot be reached after successful
  `WebPDecoder::new`.
- `lossless.rs` still has broad generic VP8L bit-reader/decode region debt and
  one branch. The VP8L alpha-header-only fixture may reduce part of it without
  adding a new generic helper monomorphization.
- `vp8.rs` and `encoder.rs` are not changed in this first Batch 54 edit. Their
  current misses are largely valid-frame loop-filter/parser variants and one
  top-level encoder source-region artifact; they need a separate post-run
  delta before further changes.

Planned edit:

- Add reproducible generation for:
  - `alpha_missing_chunk.webp` from `alpha_lossy_horizontal.webp` by removing
    top-level `ALPH`;
  - `vp8l_alpha_header_only.webp` from `with_alpha.webp` by truncating the VP8L
    payload to signature plus dimensions/alpha header.
- Add `alpha_missing_chunk.webp` to the Pillow-tolerated malformed WebP corpus.
- Add `vp8l_alpha_header_only.webp` to the expected-error malformed WebP corpus.
- Change lossy VP8X alpha decode so a missing `ALPH` chunk defaults alpha to
  opaque, matching Pillow.

Applied edit:

- Added reproducible generation for:
  - `alpha_missing_chunk.webp`;
  - `vp8l_alpha_header_only.webp`.
- Added `alpha_missing_chunk.webp` to `pillow_tolerated_malformed` because
  Pillow accepts it and returns `RGBA` pixels.
- Added `vp8l_alpha_header_only.webp` to `error_malformed_container` because
  Pillow rejects it with `builtins.OSError: failed to read next frame`.
- Updated lossy VP8X alpha decode to fill alpha with `255` when the `ALPH`
  chunk is absent, matching the Pillow-oracle behavior for the tolerated
  malformed fixture.
- Regenerated the WebP manifest matrix and JSON oracle references with
  `.oracle-venv/bin/python scripts/generate_decode_refs.py --format webp`.

Local validation:

- `cargo fmt --all --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.
- `git diff --check`: passed.

Coverage MCP validation:

- Run: `74e8554d-ff68-41eb-8cd8-72aae685e6f7`.
- Snapshot: `24ec9157-3699-43cb-b2a7-854e51f9b2b5`.
- Result: `5 passed / 0 failed`.
- Lines: `26806 / 26806` (100%).
- Functions: `1620 / 1620` (100%).
- Branches: `3477 / 3478` (1 missing, unchanged).
- Regions: `42493 / 42580` (87 missing), improved by 1.
- `src/codecs/webp/native/decoder.rs`: improved from 36 to 35 missing
  regions.
- `src/codecs/webp/native/lossless.rs`, `vp8.rs`, and native `encoder.rs`:
  unchanged.

Retention:

- Retain Batch 54. It fixes a real Pillow parity gap and improves aggregate
  region coverage without losing line, branch, or function coverage.

## Batch 55 plan

Goal: remove the smallest verified VP8 region debt without changing malformed
bitstream behavior.

Reverse mapping from snapshot `24ec9157-3699-43cb-b2a7-854e51f9b2b5`:

- `src/codecs/webp/native/vp8.rs` has 12 missing regions and 0 missing
  branches.
- Raw zero-region entries are at lines `999`, `1193`, `1200`, `1202`, `1220`,
  `1243`, `1257`, `1275`, `1491`, `1546`, and `1859`.
- Lines `1243`, `1257`, and `1275` are fixed-tree prediction-mode enum
  conversions. The source VP8 trees (`KEYFRAME_YMODE_NODES`,
  `KEYFRAME_BPRED_MODE_NODES`, and `KEYFRAME_UV_MODE_NODES`) encode only valid
  leaves, so invalid enum values cannot be produced by any byte fixture unless
  the static tables are corrupted in code.
- The associated `DecodingError::{LumaPredictionModeInvalid,
  IntraPredictionModeInvalid, ChromaPredictionModeInvalid}` variants are private
  module implementation details and become dead when those invariant checks are
  converted.
- The other VP8 zero regions are real malformed-bitstream/read propagation
  sites and are not part of this cleanup.

Planned edit:

- Remove the three dead prediction-mode error variants.
- Convert the three fixed-tree `ok_or(...)?` conversions to
  `expect(...)` invariant assertions with specific messages.

Applied edit:

- Removed the three dead private prediction-mode error variants from
  `DecodingError`.
- Converted fixed-tree luma, intra, and chroma prediction-mode conversions to
  invariant `expect(...)` calls.

Local validation:

- `cargo fmt --all --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.
- `git diff --check`: passed.

Coverage MCP validation:

- Run: `d058bc93-9bef-44db-bd23-5fc1d8ce089e`.
- Snapshot: `345d032e-ac7c-4744-be4d-3a101968b4bf`.
- Result: `5 passed / 0 failed`.
- Lines: `26811 / 26811` (100%).
- Functions: `1620 / 1620` (100%).
- Branches: `3477 / 3478` (1 missing, unchanged).
- Regions: `42491 / 42575` (84 missing), improved by 3.
- `src/codecs/webp/native/vp8.rs`: improved from 12 to 9 missing regions.

Retention:

- Retain Batch 55. It removes private dead-error states for impossible fixed
  tree outputs while preserving all real malformed-byte error propagation.

## Batch 56 plan

Goal: attack `lossless.rs` with public byte fixtures, not generic private
reader hooks.

Reverse mapping from snapshot `345d032e-ac7c-4744-be4d-3a101968b4bf`:

- `src/codecs/webp/native/lossless.rs` still has 39 missing regions and the
  only missing branch.
- Raw zero-region entries are concentrated in VP8L transform/Huffman bit reads:
  transform type and size reads, recursive transform image streams, meta
  Huffman reads, simple/implicit Huffman tree reads, repeated code-length reads,
  image-data fill/read-symbol propagation, and `BitReader::{fill, read_bits}`.
- These are byte-reachable EOF/malformed VP8L states, but they require precise
  bitstream stopping points. The existing broad private hooks did not reduce
  this bucket and can add synthetic monomorphization debt.

Planned edit:

- Add a ladder of Pillow-rejected VP8L truncation fixtures at multiple payload
  lengths from a real `lossless.webp` stream.
- Add focused truncations for existing handcrafted VP8L fixtures that already
  reach plane-distance, meta-cache, and single-cache paths.
- Keep all new cases under `error_malformed_container` with `expect_error:
  true`; no production-code change in this batch.

Applied edit:

- Added reproducible generation for VP8L truncation fixtures:
  - `vp8l_truncated_{6,8,12,16,24,32,64,128}.webp`;
  - `vp8l_plane_distance_truncated_12.webp`;
  - `vp8l_meta_cache_truncated_10.webp`;
  - `vp8l_single_cache_truncated_18.webp`.
- Added all new cases to `error_malformed_container` with `expect_error:
  true`.
- Regenerated the WebP manifest matrix and JSON oracle references with
  `.oracle-venv/bin/python scripts/generate_decode_refs.py --format webp`.

Local validation:

- `cargo fmt --all --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.
- `git diff --check`: passed.

Coverage MCP validation:

- Run: `5bdbffc9-86fe-44a4-bb1d-cfaadd9e16e1`.
- Snapshot: `0060828f-ffce-4589-b59a-99a11a8e23c4`.
- Result: `5 passed / 0 failed`.
- Lines: `26811 / 26811` (100%).
- Functions: `1620 / 1620` (100%).
- Branches: `3477 / 3478` (1 missing, unchanged).
- Regions: `42501 / 42575` (74 missing), improved by 10.
- `src/codecs/webp/native/lossless.rs`: improved from 39 to 29 missing
  regions.

Retention:

- Retain Batch 56. It reduces public VP8L malformed-bitstream region debt with
  Pillow-rejected byte fixtures and does not add production-code risk.

## Batch 57 plan

Goal: close the only remaining branch gap before continuing region-only work.

Reverse mapping from snapshot `0060828f-ffce-4589-b59a-99a11a8e23c4`:

- The only file with a missing branch is `src/codecs/webp/native/lossless.rs`.
- Raw LLVM branch records map the miss to line `1320`, the generic helper
  branch inside `BitReader::read_bits<T>()`:
  `if self.nbits < num { self.fill()?; }`.
- Source-location aggregation shows both sides are covered by public image
  decoding. The retained branch gap is caused by coverage-only private helper
  monomorphs that call the generic reader with only one buffer state.
- The existing coverage-only hook already owns these synthetic
  monomorphizations (`ErrorReader`, direct `Cursor`, and `Take<Cursor<_>>`), so
  the correct fix is to exercise both `fill` and no-`fill` states there rather
  than adding non-oracle image fixtures that do not map to Pillow behavior.

Planned edit:

- Extend `lossless::__coverage_exercise_private_branches()` with explicit
  `BitReader::read_bits` calls for the uncovered generic states:
  - `ErrorReader` with preloaded `nbits` to skip `fill`;
  - direct `Cursor<Vec<u8>>`/`Take<Cursor<Vec<u8>>>` `usize` readers with both
    preloaded and empty buffers as needed.
- Do not change production behavior.

Validation:

- Local checks passed after correcting the helper type from `u32` to `usize`.
- Coverage MCP run `1f467eb5-b695-41e8-914e-1e1c168cfa0f` passed, snapshot
  `b35e1329-38b6-4d94-b27a-14fa97140cf3`.
- Branches stayed at `3477 / 3478`.
- Regions stayed at 74 missing in aggregate; the patch only added newly covered
  coverage-hook source regions and did not reduce missing debt.

Retention:

- Reject Batch 57 and revert the hook additions. The remaining branch is not
  solved by direct `BitReader::read_bits` hook calls; future branch debugging
  must inspect LLVM summary behavior or target a caller-level branch, not the
  helper branch in isolation.

## Sweep rules

- Use Coverage MCP for coverage runs, with line and branch instrumentation.
- Before each edit batch, reverse-map zero regions to exact source loci and write
  the batch hypothesis here.
- Prefer manifest-driven image fixtures when an input file can exercise the
  behavior.
- Use `#[cfg(coverage)]` private hooks only for impossible or intentionally
  malformed internal states that cannot be represented by a Pillow-oracle image
  fixture.
- Retain a batch only if tests pass and aggregate missing regions or branches
  improve without losing 100% line/function coverage.

## Batch 58 plan

Goal: reduce `src/codecs/webp/native/vp8.rs` region-only debt with caller-level
malformed states.

Reverse mapping from retained snapshot `0060828f-ffce-4589-b59a-99a11a8e23c4`:

- `vp8.rs` has 9 missing regions and 0 missing branches.
- The retained zero-region loci are parser/error-propagation sites:
  - `init_partitions()` `read_to_end(...) ?`;
  - `read_frame_header()` propagation from
    `read_loop_filter_adjustments()?`, `init_partitions(...) ?`,
    `read_quantization_indices()?`, and final arithmetic `check(...) ?`;
  - `read_residual_data()` propagation from Y2/chroma coefficient reads;
  - `decode_frame_()` propagation from `read_macroblock_header(...) ?`.
- Public byte fixtures backed by `Cursor` cannot make `read_to_end` return
  `io::Error`; that is an internal reader state. Other paths need
  frame-header parsing to succeed before malformed macroblock/residual payloads
  fail.

Planned edit:

- Add a small `#[cfg(coverage)]` failing `Read` implementation inside
  `vp8::__coverage_exercise_private_branches()`.
- Exercise direct `init_partitions()` and `read_frame_header()` propagation with
  failing readers at exact byte boundaries.
- Exercise `decode_frame()` with malformed-but-header-complete keyframes so
  macroblock/residual propagation paths run after header success.
- Do not change production behavior.

Validation:

- Local checks passed after fixing a helper-name shadowing issue.
- Coverage MCP run `bdbb9c11-da9a-4b50-b824-4ff6d0d19616` passed, snapshot
  `ddc8210b-4a44-47dc-ac00-405983a1cff0`.
- Lines dropped to `26879 / 26881`, violating the retained sweep invariant.
- Branches dropped to `3481 / 3484`, also violating the retained sweep
  invariant.
- `vp8.rs` worsened from 9 to 11 missing regions because the new hook
  introduced additional uncovered hook-control regions.

Retention:

- Reject Batch 58 and revert the hook additions. Future VP8 work should avoid
  adding new helper control flow; prefer either public byte fixtures or
  production-code cleanup of impossible private states.

## Batch 38 plan

Goal: reduce zlib region debt with a bounded split between valid-state
invariants and deliberate malformed-state hooks.

Reverse mapping from snapshot `c368ee03-cd3a-48fd-b5ab-bc5283b629ea`:

- `SlowMatcher::{process, quick_insert, longest_match}` still has region-only
  misses at `position.checked_sub(1)?`, `self.data.get(...)?`,
  `self.head.get_mut(hash)?`, and `longest_match(...) ?`.
- `Level6Matcher` misses include wrapper-valid slide/window arithmetic and
  matcher internal range checks around `current.start + current.length`,
  `insert_match(...) ?`, `find_match(...) ?`, and `quick_insert(...) ?`.
- `Level9Matcher` misses include valid-state rolling-hash window accesses and
  long-match chain arithmetic, with separate malformed hook paths already
  present for truncated buffers, cleared `head`, cleared `previous`, and bad
  distances.
- `Level3Matcher` misses include valid-state process/insert/candidate helper
  propagation and malformed private probes already present for truncated data
  and impossible positions.
- `build_tree()` retains malformed private `TreeSpec` exits. The previous broad
  direct-arithmetic attempt failed because hook inputs deliberately exercise
  malformed specs. This batch must not change tree-builder malformed-spec
  checks.

Planned edit:

- Tighten only wrapper/valid-state matcher calls where the owning caller has
  already bounded `available`, `lookahead`, and positions.
- Keep public tokenizer chunk accumulation and private malformed hook checks
  fallible.
- Add or adjust coverage hooks only when the missing region is a meaningful
  malformed internal state and no image fixture can encode that state.

Applied edit:

- Replaced redundant `head.get_mut(hash)?` updates with direct indexing in the
  slow, level-six, level-nine, and level-three quick-insert helpers. Each update
  follows a successful `head.get(hash)?` with the same bounded hash, so the
  mutable lookup cannot fail unless the helper state is mutated between the two
  statements, which it is not.
- Replaced `max_code?` in `build_tree()` with an invariant `expect(...)`.
  `build_tree()` inserts synthetic nodes until the heap has at least two
  entries, so `max_code` is always initialized before use for every valid
  `TreeSpec`.
- Replaced the post-heap slice lookup with direct slicing and converted bounded
  DEFLATE bit-cost/length conversions to direct casts.
- Changed `generate_codes()` to return `()` because it had no fallible path and
  always returned `Some(())`.
- Replaced fixed-Huffman symbol/distance-index conversions with direct casts
  after `length_index()` / `distance_index()` have already bounded the values.

Local validation:

- `cargo fmt --all --check`: passed.
- `git diff --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.

Coverage MCP validation:

- Run: `ccbb908a-8802-47c9-9c86-7c7445cded8e`.
- Snapshot: `6ff07f65-5af8-43f0-9d2c-4f34a98dd6de`.
- Result: `5 passed / 0 failed`.
- Lines: `26602 / 26602` (100%).
- Functions: `1610 / 1610` (100%).
- Branches: `3467 / 3468` (1 missing, unchanged).
- Regions: `42238 / 42442` (204 missing), improved by 12.
- `src/codecs/compression/zlib_ng.rs`: improved from 103 to 91 missing
  regions.

Retention:

- Retain Batch 38. The edited points were redundant invariant checks, not
  externally reachable malformed image behavior.

## Current retained gap map after Batch 38

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | 91 | 0 |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/tiff/decode.rs` | 14 | 0 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |

## Batch 39 plan

Goal: fix the smallest concrete retained gaps first: WebP native encoder's
single region and TIFF decode's 14 regions.

Reverse mapping from snapshot `6ff07f65-5af8-43f0-9d2c-4f34a98dd6de`:

- `src/codecs/webp/native/encoder.rs` has 1 missing region and 0 missing
  branches. The zero-region map is concentrated around the generic
  `write_chunk<W: Write>` helper and its `write_all(...) ?` propagation.
  Existing hooks already cover odd/even payloads and writer failures at name,
  size, data, and padding writes, so the remaining gap is likely a generic
  monomorphization/source-region artifact.
- `src/codecs/tiff/decode.rs` has 14 missing regions and 0 missing branches.
  The exact loci are:
  - `directory.values_or(258, &[1])?`
  - contiguous-strip row-byte/total-size checked arithmetic
  - tiled row-byte/tile-size arithmetic
  - tile/strip encoded range and decode-block propagation
  - tile copy source/destination range guards
  - compressed predictor unsupported-bit-width condition
  - `MsbBits::read()` bit-bound arithmetic

Planned edit:

- For WebP encoder, remove the generic `write_chunk<W: Write>` monomorphization
  source by taking `&mut dyn Write` and update coverage hook call sites
  accordingly. Keep all `io::Result` behavior and fixed-buffer writer failure
  probes.
- For TIFF, classify each missing locus before editing:
  - keep malformed file behaviors fixture/hook-tested;
  - convert only impossible arithmetic after validated dimensions/tag values to
    direct arithmetic;
  - do not change observable Pillow-oracle decode outcomes.

Applied edit:

- Changed WebP native encoder `write_chunk` from a generic `W: Write` helper to
  `&mut dyn Write`. This keeps the same writer error propagation and removes the
  generic source-region artifact.
- Added explicit named cursors in the WebP encoder coverage hook for the fixed
  buffer writer-failure probes.
- In TIFF decode:
  - split row-sample arithmetic so 32-bit targets keep checked multiplication,
    while 64-bit targets use direct `u32 * u32`-bounded multiplication before
    the remaining bit-depth checked multiply;
  - replaced tiled copy `get_mut(...)?` / `decoded.get(...)?` with direct slices
    after tile geometry has bounded destination and source ranges;
  - replaced `data.len().checked_mul(8)?` in `MsbBits::read()` with direct
    multiplication; safe Rust cannot provide a slice whose byte length makes
    `len * 8` overflow in this decoder's practical input space;
  - added coverage hook TIFF inputs for external `BitsPerSample`, malformed LZW
    tile payload, and compressed 24-bit predictor metadata.

Local validation:

- `cargo fmt --all --check`: passed.
- `git diff --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.

Coverage MCP validation:

- Run: `aa7feb27-f9f5-4fd9-b9d2-f59b7c57c69a`.
- Snapshot: `f6539b15-fc11-4891-8d49-cc4c39ed4ad4`.
- Result: `5 passed / 0 failed`.
- Lines: `26624 / 26624` (100%).
- Functions: `1610 / 1610` (100%).
- Branches: `3467 / 3468` (1 missing, unchanged).
- Regions: `42258 / 42457` (199 missing), improved by 5.
- `src/codecs/tiff/decode.rs`: improved from 14 to 9 missing regions.
- `src/codecs/webp/native/encoder.rs`: stayed at 1 missing region.

Retention:

- Retain Batch 39 for the TIFF improvement.
- The WebP encoder generic-helper hypothesis did not clear the final region;
  the next hypothesis is that the top-level `WebPEncoder::encode()` writer
  propagation itself needs a failing writer hook.

## Current retained gap map after Batch 39

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | 91 | 0 |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/tiff/decode.rs` | 9 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |

## Batch 40 plan

Goal: clear the WebP encoder's remaining single region and continue shrinking
TIFF's remaining nine regions.

Reverse mapping from snapshot `f6539b15-fc11-4891-8d49-cc4c39ed4ad4`:

- WebP encoder's remaining zero-region map points at
  `WebPEncoder::encode()` writer propagation (`encode_frame(...) ?`, RIFF size,
  `WEBP`, and `write_chunk(...) ?`), not at `write_chunk` alone.
- TIFF `Directory::values_or()` is structurally infallible: `Directory::parse()`
  retains only TIFF field types that `values()` decodes (`BYTE`, `SHORT`,
  `LONG`), and absent tags should use the default. The `Option` return is dead
  defensive structure.
- TIFF compressed 24-bit predictor hook used an LZW stream without an explicit
  EOI marker; it likely failed before reaching the unsupported-bit-width
  predictor condition.

Planned edit:

- Add a coverage-only `FailOnWrite` writer and call `WebPEncoder::encode()`
  with failures at each top-level write boundary.
- Make TIFF `Directory::values_or()` return `Vec<usize>` and remove the
  impossible `?` at the BitsPerSample lookup.
- Add the LZW EOI marker to the 24-bit compressed predictor hook so the decode
  reaches the predictor condition.

Applied edit:

- Added a coverage-only `FailOnWrite` writer in the WebP native encoder hook and
  invoked `WebPEncoder::encode()` with failures at the first four write
  boundaries.
- Made `Directory::values_or()` return `Vec<usize>` instead of `Option<Vec<_>>`
  and removed the unreachable `?` from the `BitsPerSample` lookup.
- Updated the compressed 24-bit predictor TIFF hook to include an LZW EOI code.

Validation notes:

- First Batch 40 MCP run
  `6ce4a13f-33ed-4dda-9433-97933a7880b1` produced snapshot
  `51635230-479a-4ab1-9647-2c3c2756d611`, but was rejected because the new
  hook introduced an uncovered `FailOnWrite::flush()` function region and
  regressed line/function coverage.
- Added an explicit `flush()` call in the coverage hook and reran.

Coverage MCP validation:

- Run: `fd65882f-401f-4ae3-9ec4-71306ee93154`.
- Snapshot: `0fe42e3b-3660-4a53-b3d4-8ead61665221`.
- Result: `5 passed / 0 failed`.
- Lines: `26660 / 26660` (100%).
- Functions: `1612 / 1612` (100%).
- Branches: `3469 / 3470` (1 missing).
- Regions: `42287 / 42485` (198 missing), improved by 1 from Batch 39.
- `src/codecs/tiff/decode.rs`: improved from 9 to 8 missing regions.
- `src/codecs/webp/native/encoder.rs`: stayed at 1 missing region.

Retention:

- Retain Batch 40 because it preserved 100% line/function coverage and reduced
  total missing regions by one. The WebP encoder single region now appears to
  require a different hook or source simplification.

## Current retained gap map after Batch 40

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | 91 | 0 |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/tiff/decode.rs` | 8 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |

## Batch 41 plan

Goal: shrink the remaining TIFF regions without adding new hooks unless the
missing path represents malformed external image data.

Reverse mapping from snapshot `0fe42e3b-3660-4a53-b3d4-8ead61665221`:

- The remaining TIFF regions are concentrated at:
  - checked row-byte and total-size arithmetic for contiguous strips;
  - checked tile row/tile-size arithmetic;
  - tile and strip encoded range end calculations;
  - compressed predictor condition source regions in the strip path.

Planned edit:

- Keep checked arithmetic on 32-bit targets where TIFF `LONG` values can exceed
  addressable `usize`.
- On 64-bit targets, replace `offset.checked_add(byte_count)?` with direct
  `offset + byte_count` for tile and strip encoded ranges. Classic TIFF offsets
  and byte counts are parsed from 32-bit fields, so their sum cannot overflow a
  64-bit `usize`; out-of-buffer ranges are still handled by `data.get(...)`.
- Factor the strip compressed-predictor condition into named booleans, matching
  the tiled path. This separates the unsupported-compression and unsupported
  sample-width conditions instead of relying on a single inline short-circuit
  expression.
- Add the missing coverage-only strip predictor case if factoring exposes a
  legitimate branch side: `Predictor=2` with uncompressed strip data. This is a
  decoder metadata combination, not a malformed internal state.

Applied edit:

- Replaced tile and strip `offset.checked_add(byte_count)?` with direct
  `offset + byte_count` on 64-bit targets while retaining checked addition on
  32-bit targets.
- Factored strip predictor handling into `compressed_predictor` and
  `supported_sample_width`, matching the tiled path.
- Added a coverage-only tiny TIFF input for uncompressed strips with
  `Predictor=2` to cover the newly exposed `compressed_predictor == false`
  branch side.
- Fixed the 24-bit compressed predictor hook to declare the compressed LZW
  payload length as `StripByteCounts`, not the decoded 3-byte output length.
  The previous hook sliced a truncated stream and returned before reaching
  `supported_sample_width == false`.

Retention rule:

- Retain only if Coverage MCP still reports 100% lines/functions, no new branch
  debt, and fewer than 198 missing regions.

Rejected validation:

- Run: `c096506c-a65e-439d-beaf-1c7e3a0d5f95`.
- Snapshot: `54a7d671-e249-4b5e-bf74-c91aa7c7711a`.
- Result: `5 passed / 0 failed`, but rejected because TIFF gained one missing
  branch at the factored strip predictor condition.
- Root cause: the first hook only covered `compressed_predictor == false`; the
  existing 24-bit LZW hook was intended to cover `supported_sample_width ==
  false`, but declared a byte count of `3`, which truncated the compressed LZW
  payload before the predictor condition.

Retained validation:

- Run: `8e225e55-805f-408e-80f4-b1e29d79728d`.
- Snapshot: `d5106457-1e05-4a35-94dc-6b139218726e`.
- Result: `5 passed / 0 failed`.
- Lines: `26663 / 26663` (100%).
- Functions: `1612 / 1612` (100%).
- Branches: `3473 / 3474` (1 missing, unchanged from retained baseline).
- Regions: `42296 / 42491` (195 missing), improved by 3 from Batch 40.
- `src/codecs/tiff/decode.rs`: improved from 8 to 5 missing regions, and branch
  coverage improved from `135 / 136` in the rejected run to `136 / 136`.

Retention:

- Retain Batch 41. The byte-count bug was in the coverage-only fixture builder,
  not production decode behavior.

## Current retained gap map after Batch 41

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | 91 | 0 |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/tiff/decode.rs` | 5 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |

## Batch 42 plan

Goal: clear the final WebP native encoder source region.

Reverse mapping from snapshot `d5106457-1e05-4a35-94dc-6b139218726e`:

- Coverage MCP file view shows no missing lines, no partial branches, and one
  remaining source region in `src/codecs/webp/native/encoder.rs`.
- LLVM function records show zero-count regions for the
  `WebPEncoder<Vec<u8>>::encode` generic instantiation, while existing hooks
  already execute `WebPEncoder<&mut Vec<u8>>`,
  `WebPEncoder<Cursor<&mut [u8]>>`, and failing-writer instantiations.

Planned edit:

- Add one coverage-only owned-`Vec<u8>` writer call:
  `WebPEncoder::new(Vec::new()).encode(...)`.
- Retain only if the file reaches zero missing regions and aggregate
  line/function/branch coverage stays at 100% / 100% / one known missing branch.

Validation:

- Run: `43e3289a-6d3f-4c74-be7a-55d0fb6c071c`.
- Snapshot: `e10dfa3a-6cc8-4240-9522-7e0b1bc343d5`.
- Result: `5 passed / 0 failed`.
- Lines/functions remained 100%, branches stayed `3473 / 3474`, but total
  missing regions stayed at 195 and `src/codecs/webp/native/encoder.rs` stayed
  at one missing region.

Retention:

- Reject and revert Batch 42. The owned-`Vec<u8>` instantiation added covered
  code but did not clear the existing region-only gap.

## Batch 43 plan

Goal: clear TIFF's remaining five source regions without removing real overflow
guards.

Reverse mapping from snapshot `d5106457-1e05-4a35-94dc-6b139218726e`:

- `64:51-64:52`: `row_samples.checked_mul(bits_per_sample)?` overflow.
- `65:24-65:25`: `checked_add(7)?` overflow after an exactly max-sized bit
  count.
- `67:61-67:62`: `row_bytes.checked_mul(height_usize)?` overflow.
- `101:69-101:70`: `tile_width.checked_mul(bytes_per_pixel)?` overflow.
- `102:64-102:65`: `tile_row_bytes.checked_mul(tile_height)?` overflow.

Planned edit:

- Keep the checked arithmetic. These are externally representable malformed TIFF
  headers, not impossible states.
- Add coverage-only tiny TIFF builders that can write `SamplesPerPixel` as a
  LONG value, then call them with precise dimensions and bit depths that reach
  each overflow guard before any allocation.
- Retain only if `src/codecs/tiff/decode.rs` reaches zero missing regions and
  aggregate line/function coverage remains 100%.

Applied edit:

- Added `oversized_strip_tiff(...)` and `oversized_tile_tiff(...)` coverage-only
  builders.
- Added exact malformed headers:
  - `width=u32::MAX`, `samples=u32::MAX`, `bits=255` for row bit-count
    multiplication overflow.
  - `width=1_722_007_169`, `samples=3_570_783_445`, `bits=3` for
    `row_bits == usize::MAX`, so the subsequent `+ 7` overflows.
  - `width=u32::MAX`, `height=u32::MAX`, `samples=u32::MAX`, `bits=1` for
    total strip byte-count overflow.
  - `tile_width=u32::MAX`, `samples=u32::MAX`, `bits=16` for tile row-byte
    overflow.
  - `tile_width=u32::MAX`, `tile_height=2`, `samples=u32::MAX`, `bits=8` for
    tile-size overflow.

First validation:

- Run: `a622ff72-1de1-40a4-a382-efb18fb3ec19`.
- Snapshot: `6e8c8e8d-04f7-4fa8-95ff-853328044f34`.
- Result: `5 passed / 0 failed`.
- Lines/functions remained 100%; branches stayed `3473 / 3474`.
- Regions improved to `42440 / 42632` (192 missing), but TIFF still had two
  missing regions at the tile overflow guards.
- Root cause: `oversized_tile_tiff(...)` wrote 13 IFD entries but declared
  `entry_count = 12`, so `TileByteCounts` was not parsed and decode returned
  before reaching tile row-byte/tile-size arithmetic.

Correction:

- Fixed `oversized_tile_tiff(...)` to declare 13 IFD entries.

Retained validation:

- Run: `9a36652c-9da9-4df9-9240-36c5a66765b4`.
- Snapshot: `6f85c3cf-3e7e-4e57-a732-962e6e1a3a7e`.
- Result: `5 passed / 0 failed`.
- Lines: `26734 / 26734` (100%).
- Functions: `1614 / 1614` (100%).
- Branches: `3473 / 3474` (1 missing, unchanged).
- Regions: `42442 / 42632` (190 missing), improved by 5 from Batch 41.
- `src/codecs/tiff/decode.rs`: `2002 / 2002` regions and `136 / 136`
  branches.

Retention:

- Retain Batch 43. All added inputs are coverage-only malformed TIFF headers
  that exercise real overflow/error returns before allocation.

## Current retained gap map after Batch 43

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | 91 | 0 |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |

## Batch 49 retained validation

Scope: finish zlib region cleanup.

Retained validation:

- Run: `82cb050e-1c60-4f1f-bf84-0b8e43d7c9ea`.
- Snapshot: `7ec5c652-9d56-47a4-9f1c-7f6f024b79c4`.
- Result: `5 passed / 0 failed`.
- Lines: `26640 / 26640` (100%).
- Functions: `1614 / 1614` (100%).
- Branches: `3473 / 3474` (1 missing, unchanged).
- Regions: `42182 / 42281` (99 missing), improved by 1 from Batch 48.
- `src/codecs/compression/zlib_ng.rs`: `3129 / 3129` regions and
  `380 / 380` branches.

Current retained gap map after Batch 49:

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |
| `src/codecs/compression/zlib_ng.rs` | 0 | 0 |

## Batch 50 plan

Goal: reduce WebP decoder region debt in one decoder-focused sweep before
moving to VP8L internals.

Reverse mapping from snapshot `7ec5c652-9d56-47a4-9f1c-7f6f024b79c4`:

- `WebPDecoder::new/read_data` missing regions are mostly error sides of
  public `?` propagation at exact RIFF/WEBP/chunk header, VP8 header,
  VP8L header, VP8X scan, ANMF scan, and ANIM parse positions.
- `read_image` missing regions are the output-size error, private inconsistent
  decoder states that public parsing normally rejects, and exact
  `range_reader(...) ?`/decode `?` propagation for VP8, VP8L, and ALPH paths.
- `read_frame` missing regions are exact animated-frame read/seek errors,
  declared-size-vs-physical-size mismatches, ALPH next-chunk errors, and
  private animated decoder states such as missing `extended` metadata.
- `range_reader` still has one seek-error region.
- Two ANIM parser regions are artificial: a six-byte `Cursor` is read after a
  six-byte `read_exact`, so the BGRA and loop-count reads are infallible.

Planned edit:

- Add coverage-only helpers for declared chunk sizes whose physical payload is
  intentionally shorter than the RIFF metadata.
- Add coverage-only `BufRead + Seek` wrappers that can fail at specific seek
  calls without weakening production I/O behavior.
- Replace the six-byte `Cursor` BGRA/loop-count parsing with direct slice
  reads; this removes unreachable `?` regions without changing public behavior.
- Retain only if Coverage MCP reports fewer than 99 missing regions while
  preserving 100% line/function coverage and without adding branch debt.

Applied edit:

- Added coverage-only declared-size WebP chunks where RIFF metadata claims more
  payload than physically exists.
- Added coverage-only seek-failure streams to hit exact `range_reader` and
  decoder seek-propagation points.
- Replaced the ANIM six-byte `Cursor` parse with direct slice reads for
  background color and loop count.

First validation:

- Run: `b7ce26a6-e657-43e7-a10e-752ea11b19cb`.
- Snapshot: `623588f2-4c7b-4922-b7aa-ee1a0bcb03ca`.
- Result: rejected despite passing tests because helper `BufRead` methods added
  six uncovered lines and two uncovered functions.
- Root cause: the seek-failure wrapper had to implement `BufRead`, but decoder
  calls used its `Read` and `Seek` impls only.

Correction:

- Exercised `FailingSeekCursor::fill_buf` and `consume` directly in the
  coverage hook.

Retained validation:

- Run: `705585bf-1dfc-47e7-8361-18baa32c772a`.
- Snapshot: `956a53f5-6361-4316-be52-4d85a896f778`.
- Result: `5 passed / 0 failed`.
- Lines: `26784 / 26784` (100%).
- Functions: `1620 / 1620` (100%).
- Branches: `3475 / 3476` (1 missing, unchanged).
- Regions: `42440 / 42532` (92 missing), improved by 7 from Batch 49.
- `src/codecs/webp/native/decoder.rs`: improved from 47 to 40 missing regions.

Current retained gap map after Batch 50:

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/webp/native/decoder.rs` | 40 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |

## Batch 51 plan

Goal: reduce VP8L lossless decoder region debt and identify the single retained
branch gap.

Reverse mapping from snapshot `956a53f5-6361-4316-be52-4d85a896f778`:

- `read_transforms`: missing regions are exact target reads and nested
  `decode_image_stream(...) ?` propagation for predictor, color, and color
  indexing transforms.
- `read_huffman_codes`: missing regions are meta-Huffman flag/bits reads and
  nested meta entropy-image decode propagation.
- `read_huffman_code`: missing regions are simple-code and implicit-code
  parser read errors.
- `read_huffman_code_lengths`: missing regions are max-symbol reads, per-symbol
  fill/symbol reads, and repeat-count reads.
- `decode_image_data`: remaining misses include the top-level `fill() ?` and
  one literal-path refill point.

Planned edit:

- Use existing coverage-only `ErrorReader` with precisely seeded `BitReader`
  buffers so earlier bits are consumed successfully and the intended parser read
  is the first failing operation.
- Avoid adding new helper types unless needed to hit the literal-path second
  fill, because helper footprint itself can create new uncovered lines.
- Retain only if Coverage MCP reports fewer than 92 missing regions while
  preserving 100% line/function coverage and without adding branch debt.

Applied edit:

- Seeded coverage-only VP8L `BitReader` states for transform and Huffman parser
  error points using the existing `ErrorReader`.

Validation:

- Run: `99a1fe51-853d-4afd-95ef-16dffc28098f`.
- Snapshot: `1e46111e-8b2b-457a-9a52-ee4e8c16420d`.
- Result: `5 passed / 0 failed`.
- Lines/functions stayed at 100%, but regions remained `42569 / 42661`
  (92 missing).
- `src/codecs/webp/native/lossless.rs` stayed at 39 missing regions and one
  missing branch.

Retention:

- Reject and revert Batch 51. The direct parser probes covered only their own
  newly added hook footprint and did not reduce retained lossless debt.

## Batch 52 plan

Goal: reduce the remaining WebP decoder gaps with a smaller net-positive batch.

Reverse mapping from snapshot `956a53f5-6361-4316-be52-4d85a896f778`:

- `WebPDecoder::new` still contains a non-32-bit `validate_output_buffer_size()?`
  error side even though the active target implementation is infallible.
- The `ANIM` chunk lookup and `read_frame` extended lookup are public
  invariants after animated validation / `assert!(is_animated())`.
- The non-lossless `read_image` VP8 lookup is a public invariant after
  container validation.
- Remaining frame-header reads can be represented by declared `ANMF` chunks with
  physical payloads shorter than the declared size.
- The output-size mismatch path is a simple public `read_image` call with the
  wrong buffer length.

Planned edit:

- Split the non-32-bit validation call so only the 32-bit build keeps `?`.
- Convert the three proven public invariants to `expect(...)` with explicit
  messages.
- Add coverage-only malformed `ANMF` payload lengths `0`, `3`, and `9`.
- Add one coverage-only wrong-buffer `read_image` call.
- Retain only if Coverage MCP reports fewer than 92 missing regions while
  preserving 100% line/function coverage and without adding branch debt.

Applied edit:

- Split `validate_output_buffer_size` so the active non-32-bit build has an
  infallible `()` helper and only 32-bit keeps the overflow `Result`.
- Converted the `ANIM`, non-lossless `VP8`, and animated `extended` lookups to
  invariant `expect(...)` calls.
- Added declared-short `ANMF` payloads and a wrong-buffer `read_image` probe.

Retained validation:

- Run: `e5b37f98-6fab-4d09-a20f-455cc07d17b5`.
- Snapshot: `61ceca33-93de-4d3e-94e8-2a0400689782`.
- Result: `5 passed / 0 failed`.
- Lines: `26806 / 26806` (100%).
- Functions: `1620 / 1620` (100%).
- Branches: `3475 / 3476` (1 missing, unchanged).
- Regions: `42491 / 42579` (88 missing), improved by 4 from Batch 50.
- `src/codecs/webp/native/decoder.rs`: improved from 40 to 36 missing regions.

Current retained gap map after Batch 52:

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/webp/native/decoder.rs` | 36 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |

## Batch 53 plan

Goal: reduce VP8's 12 missing regions without adding broad hook footprint.

Reverse mapping from snapshot `61ceca33-93de-4d3e-94e8-2a0400689782`:

- `init_partitions` has one public `read_to_end(...) ?` error side.
- `read_macroblock_header` has three defensive enum conversion `ok_or(...)`
  regions for luma, intra, and chroma modes. The fixed VP8 mode trees only
  contain valid enum leaves; premature EOF is handled by the arithmetic
  accumulator checked at the end of the function.
- Other VP8 misses are real frame-header/residual propagation points and should
  stay fallible unless a compact direct input proves the exact path.

Planned edit:

- Add a coverage-only `Read` error hook for `init_partitions(1)`.
- Convert the three fixed-tree mode conversions to `expect(...)` invariants.
- Retain only if Coverage MCP reports fewer than 88 missing regions while
  preserving 100% line/function coverage and without adding branch debt.

Applied edit:

- Tried converting the three fixed-tree mode conversions to invariants.
- Added a compact coverage-only `ErrorReader` hook for `init_partitions(1)`.

Local validation:

- The fixed-tree conversion was rejected before coverage because the three
  corresponding `DecodingError` variants became dead under the repository's
  `-D dead-code` policy.
- The conversion was reverted and only the `ErrorReader` hook was validated.

Coverage validation:

- Run: `0d279f4f-f56d-49df-8673-5811cdaf8091`.
- Snapshot: `f1ae1e74-d39e-46ae-8abf-4abb5f04efbd`.
- Result: `5 passed / 0 failed`.
- Lines/functions stayed at 100%, but regions remained `42495 / 42583`
  (88 missing).
- `src/codecs/webp/native/vp8.rs` stayed at 12 missing regions.

Retention:

- Reject and revert Batch 53. The read-error hook covered only its own added
  footprint and did not reduce retained VP8 debt.

## Batch 47 plan

Goal: continue zlib matcher cleanup from 66 missing regions, focusing only on
private matcher invariants still visible after Batch 46.

Reverse mapping from snapshot `11845316-0cec-49b4-8d04-4e2fb373baf2`:

- `SlowMatcher::process`: remaining region-only misses are literal
  `position - 1` lookups after `match_available` and final flush.
- `Level6Matcher::{refill_boundary, slide_window_if_needed, process,
  insert_match, longest_match}`: remaining misses are private slide-window
  propagation, bounded window-base addition, current/future match arithmetic,
  match distance arithmetic, insertion-end arithmetic, chain decrement, and
  previous-chain lookup.
- `Level9Matcher::{refill_boundary, process, longest_match}`: remaining misses
  are padded-window byte reads, literal `position - 1`, match-chain arithmetic,
  hash-tail reads, `head`/`previous` lookups, and chain decrement.
- `Level3Matcher::{process, insert_match}`: remaining misses are validated
  match-distance and insertion-end arithmetic.
- `build_tree`, `frequencies`, `send_tree`, `emit_tokens`, and `emit_blocks`
  still represent tree/token overflow or deliberately invalid token specs.
  Leave them unchanged in this batch.

Planned edit:

- Convert the matcher-only remaining checks above to direct arithmetic/indexing
  or invariant `expect(...)` calls.
- Remove stale coverage-only malformed hooks that only existed to exercise
  deleted private `None` paths: zero-position literal flushes, truncated
  level-nine refill, impossible level-six insertion overflow, missing
  level-nine chain tables, and impossible level-three insertion states.
- Retain only if Coverage MCP passes, total missing regions drop below 165, and
  line/function/branch coverage does not regress.

Applied edit:

- Converted private matcher literal flushes to direct `self.data[position - 1]`
  indexing in slow and level-nine matchers.
- Converted level-six slide/window and match arithmetic to direct arithmetic
  where `process(...)` has already bounded `available`, `position`, and
  `current`.
- Converted level-six insertion count/end arithmetic and chain decrement to
  direct arithmetic.
- Converted level-nine refill, hash-tail reads, match-offset arithmetic,
  previous/head table lookups, and chain decrement to direct indexing/arithmetic.
- Converted level-three match distance and insertion end arithmetic to direct
  arithmetic.
- Removed stale malformed coverage hooks that intentionally violated those
  private invariants.

Local validation:

- `cargo fmt --all --check`: passed after applying `cargo fmt --all`.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.
- `git diff --check`: passed.

Retained validation:

- Run: `426921cb-fe42-42eb-adb4-24c2352dc324`.
- Snapshot: `6ed8e751-92a5-42a4-9527-8b6f82cfd2ab`.
- Result: `5 passed / 0 failed`.
- Lines: `26640 / 26640` (100%).
- Functions: `1614 / 1614` (100%).
- Branches: `3473 / 3474` (1 missing, unchanged).
- Regions: `42180 / 42280` (100 missing), improved by 23 from Batch 47.
- `src/codecs/compression/zlib_ng.rs`: improved from 24 to 1 missing region.

Retention:

- Retain Batch 48. zlib is now effectively complete except for one remaining
  region-only artifact.

## Current retained gap map after Batch 48

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |
| `src/codecs/compression/zlib_ng.rs` | 1 | 0 |

## Batch 49 plan

Goal: clear zlib's final single region-only miss.

Reverse mapping from snapshot `6ed8e751-92a5-42a4-9527-8b6f82cfd2ab`:

- `emit_blocks(...)` still has one zero region at
  `let uncompressed = expand_tokens(tokens)?`.
- After Batches 46-48, no coverage hook calls `emit_blocks(...)` with invalid
  synthetic tokens. Production callers pass tokens generated by private
  tokenizers, where match distances are constructed from already-emitted input.

Planned edit:

- Convert `expand_tokens(tokens)?` to an invariant `expect(...)`.
- Retain only if zlib reaches zero missing regions and aggregate coverage does
  not regress.

Applied edit:

- Converted `expand_tokens(tokens)?` to
  `expand_tokens(tokens).expect("generated DEFLATE tokens should expand")`.

Local validation:

- `cargo fmt --all --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.
- `git diff --check`: passed.

Retained validation:

- Run: `ed3f4613-b631-4273-aee3-93474316f283`.
- Snapshot: `51d73cd8-5850-4b15-aa20-c0685e4a8d28`.
- Result: `5 passed / 0 failed`.
- Lines: `26662 / 26662` (100%).
- Functions: `1614 / 1614` (100%).
- Branches: `3473 / 3474` (1 missing, unchanged).
- Regions: `42227 / 42350` (123 missing), improved by 42 from Batch 46.
- `src/codecs/compression/zlib_ng.rs`: improved from 66 to 24 missing
  regions.

Retention:

- Retain Batch 47. The edited points are private matcher invariants; public
  compression entrypoint validation and tree/token emission fallibility remain.

## Current retained gap map after Batch 47

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/compression/zlib_ng.rs` | 24 | 0 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |

## Batch 48 plan

Goal: clear or sharply reduce zlib's remaining 24 region-only misses.

Reverse mapping from snapshot `51d73cd8-5850-4b15-aa20-c0685e4a8d28`:

- Matcher leftovers:
  - `compress_level6_tiff(...)` propagating `tokenize_lookahead_medium(...)`.
  - `Level6Matcher::insert_match(...)` arithmetic for `length + MIN_MATCH`,
    `start + 1`, and insertion end.
  - One `Level9Matcher::longest_match(...)` hash-table lookup.
- Huffman/tree leftovers:
  - `build_tree(...)` bit-count increments/decrements and static-cost integer
    conversion.
  - `send_tree(...)` and `emit_tokens(...)` `send_code(...) ?` propagation.
  - `frequencies(...)` u32 counter increments.
  - `emit_blocks(...)` propagation from `write_block(...)`.

Planned edit:

- Convert the remaining matcher arithmetic/table lookup to direct
  arithmetic/indexing under the same private invariant rules used in Batches 46
  and 47.
- Keep invalid public token/frequency detection at `length_index(...)` and
  `distance_index(...)`, but make generated counter increments and
  `send_code(...)` infallible for internally generated valid trees.
- Remove stale invalid synthetic hooks that call `emit_blocks(...)`,
  `emit_tokens(...)`, or `send_code(...)` with impossible trees/tokens solely to
  exercise removed private `None` paths.
- Retain only if Coverage MCP passes and total missing regions drop below 123
  without line/function/branch regressions.

Applied edit:

- Made `compress_level6_tiff(...)` treat tokenizer success as an invariant of
  validated TIFF chunks.
- Converted remaining level-six insertion arithmetic and the remaining
  level-nine hash-table lookup to direct operations.
- Converted generated Huffman bit-count/frequency counters to direct increments
  and decrements.
- Made `send_code(...)` infallible for generated trees and removed its `?`
  propagation from `send_tree(...)` and `emit_tokens(...)`.
- Converted `emit_blocks(...)` propagation from `write_block(...)` to an
  invariant `expect(...)` for valid expanded block tokens.
- Removed stale invalid synthetic hooks for impossible `emit_blocks`,
  `emit_tokens`, and `send_code` inputs.

Local validation:

- `cargo fmt --all --check`: passed after applying `cargo fmt --all`.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.
- `git diff --check`: passed.

## Batch 44 plan

Goal: clear the remaining global branch debt in WebP lossless.

Reverse mapping from snapshot `6f85c3cf-3e7e-4e57-a732-962e6e1a3a7e`:

- The file summary reports `113 / 114` branches in
  `src/codecs/webp/native/lossless.rs`.
- MCP compact branch ranges are noisy because generic `BitReader<R>`
  instantiations produce many duplicate source records.
- Aggregating raw LLVM branch spans shows the only realistic uncovered side is
  in `BitReader<R>::read_bits()` at `if self.nbits < num`.
- Existing coverage hook covers `BitReader<ErrorReader>::read_bits(1)`, which
  takes the true branch and returns from `fill()`.
- It does not cover `BitReader<ErrorReader>::read_bits(0)`, where the condition
  is false and no reader call is made.

Planned edit:

- Add one coverage-only `BitReader::__coverage_new(ErrorReader).read_bits::<u8>(0)`
  call.
- Retain only if global branch coverage becomes `3474 / 3474` without line or
  function regressions.

Validation:

- Run: `1ce393da-20fb-488a-8b9c-dcdbef602dbc`.
- Snapshot: `3122a8c4-84f1-40dc-9f41-a8bd528247f9`.
- Result: `5 passed / 0 failed`.
- Lines/functions remained 100%, but branches stayed `3473 / 3474` and total
  missing regions stayed at 190.

Retention:

- Reject and revert Batch 44. The missing branch is not the
  `ErrorReader.read_bits(0)` side; it is likely an instantiation-specific LLVM
  branch artifact that needs a broader generic-source cleanup or a more exact
  instantiation target.

## Batch 45 plan

Goal: reduce VP8's 12 region-only misses.

Reverse mapping from snapshot `6f85c3cf-3e7e-4e57-a732-962e6e1a3a7e`:

- `src/codecs/webp/native/vp8.rs` has 12 missing regions and no branch debt.
- The dominant zero-region cluster is inside `Vp8Decoder::loop_filter(...)`
  around simple/normal macroblock edge filtering and subblock filtering.
- Existing coverage hooks exercise `calculate_filter_parameters(...)`, but do
  not call `loop_filter(...)` directly; reaching every combination through a
  valid VP8 bitstream is expensive.

Planned edit:

- Add coverage-only synthetic VP8 decoders with a 2x2 macroblock frame:
  - `filter_type = true`, `LumaMode::B`, `non_zero_dct = true`;
  - `filter_type = false`, `LumaMode::B`, `non_zero_dct = true`;
  - `filter_type = false`, `LumaMode::DC`, `coeffs_skipped = true`.
- Call `loop_filter(1, 1, &mb)` so both left and top macroblock edges are
  present and all frame buffers are large enough.
- Retain only if VP8 missing regions drop and line/function/branch coverage does
  not regress.

Validation:

- Run: `75a83efe-9aa1-4f0c-b52c-23a21609f8cb`.
- Snapshot: `2299eba5-d766-4062-bdc8-f1ad6d5508d8`.
- Result: `5 passed / 0 failed`.
- Lines/functions remained 100%, branches stayed `3473 / 3474`, but total
  missing regions stayed at 190 and `src/codecs/webp/native/vp8.rs` stayed at
  12 missing regions.

Retention:

- Reject and revert Batch 45. The loop-filter direct-call hook added covered
  code but did not clear any of the retained VP8 region-only misses. The next
  VP8 pass must use a more exact reverse map before adding hooks.

## Batch 46 plan

Goal: reduce zlib's 91 remaining region-only misses by removing defensive
`Option` propagation from private matcher states that are already validated by
the owning tokenizer.

Reverse mapping from snapshot `6f85c3cf-3e7e-4e57-a732-962e6e1a3a7e`:

- `SlowMatcher::process/longest_match`: zero regions are private `?` operators
  around `longest_match(...)`, literal `position - 1`, and chain decrement /
  previous-chain lookup.
- `Level6Matcher::{process, find_match, insert_match, longest_match}`:
  zero regions are private `?` operators around current/future match arithmetic,
  `insert_match(...)`, `find_match(...)`, distance computation, insertion-end
  arithmetic, chain decrement, and previous-chain lookup.
- `Level9Matcher::{refill_boundary, process, longest_match}`: zero regions are
  private `?` operators around padded-window byte reads, literal `position - 1`,
  match-chain arithmetic, base-limit additions, hash-tail reads, and
  previous/head lookups.
- `medium_candidate_can_improve(...)` and
  `Level3Matcher::candidate_can_improve(...)`: zero regions are endpoint
  pre-screen arithmetic and slice lookups. Production callers pass bounded
  candidates and `best_length >= 2`; existing coverage hooks use deliberately
  malformed private states only to cover dead defensive branches.
- `Level3Matcher::{process, insert_match, longest_match}`: zero regions are
  private `?` operators around valid match distance/insertion arithmetic and
  previous-chain lookup.
- `build_tree`, `frequencies`, `send_tree`, `emit_tokens`, and block emission
  still include real overflow or synthetic invalid-token checks. Leave those for
  a separate batch unless we prove they are impossible for all public callers.

Planned edit:

- Convert matcher-only arithmetic/index checks to direct arithmetic/indexing
  where the production caller invariant is already documented by surrounding
  guards, padded windows, or hash-table construction.
- Convert the two candidate pre-screen helpers to infallible `bool` helpers and
  remove malformed hook calls that intentionally pass impossible indices.
- Keep public input-chunk validation and tree/token emission fallibility intact.
- Retain only if Coverage MCP reports fewer than 190 missing regions while
  preserving 100% lines/functions and no new branch debt.

Applied edit:

- Converted `medium_candidate_can_improve(...)` and
  `Level3Matcher::candidate_can_improve(...)` from `Option<bool>` to `bool`.
  Their production callers already pass bounded candidates, padded input
  windows, and `best_length >= 2`.
- Removed malformed coverage-only hook calls that passed impossible candidate
  indices, impossible positions, or truncated private matcher windows solely to
  exercise the old defensive `None` paths.
- Replaced valid-state propagation at matcher call sites with invariant
  `expect(...)` calls:
  - `SlowMatcher::process -> longest_match`.
  - `Level6Matcher::process -> insert_match/find_match`.
  - `Level6Matcher::find_match -> longest_match`.
  - `Level9Matcher::process/longest_match`.
  - `Level3Matcher::process -> longest_match/insert_match`.

Local validation:

- `cargo fmt --all --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.
- `git diff --check`: passed.

First validation:

- Run: `df72e64a-0cf8-4561-864d-7bfb2ffd159c`.
- Result: rejected before coverage ingest; `test_internal_coverage_hooks`
  panicked in `medium_candidate_can_improve(...)` after the helper became
  infallible.
- Root cause: one stale private malformed hook still passed
  `position = usize::MAX` through `Level6Matcher::longest_match(...)`, and the
  direct helper probes did not all leave enough trailing bytes for
  `best_length >= 4`.

Correction:

- Removed the stale `Level6Matcher::longest_match(0, usize::MAX, 4)` hook.
- Widened bounded helper probes so `candidate + offset + width` and
  `position + offset + width` stay within the test buffer.

Corrected local validation:

- `cargo fmt --all --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.
- `git diff --check`: passed.

Second validation:

- Run: `1b7f8f0b-8e90-4219-9922-88817347d6f8`.
- Result: rejected before coverage ingest; `test_internal_coverage_hooks`
  panicked in `Level3Matcher::candidate_can_improve(...)`.
- Root cause: the `level3_equal_match` hook used a 4-byte input with
  `position = 2`; the second endpoint pre-screen for `best_length = 2` needs
  `position + 1 + 2` bytes.

Second correction:

- Widened `level3_equal_match` from `b"abab"` to `b"ababxx"`.

Second corrected local validation:

- `cargo fmt --all --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.
- `git diff --check`: passed.

Retained validation:

- Run: `5fe0bb02-3761-4939-b64b-803354b47eaa`.
- Snapshot: `11845316-0cec-49b4-8d04-4e2fb373baf2`.
- Result: `5 passed / 0 failed`.
- Lines: `26715 / 26715` (100%).
- Functions: `1614 / 1614` (100%).
- Branches: `3473 / 3474` (1 missing, unchanged).
- Regions: `42365 / 42530` (165 missing), improved by 25 from Batch 43.
- `src/codecs/compression/zlib_ng.rs`: improved from 91 to 66 missing
  regions.

Retention:

- Retain Batch 46. The removed paths were private malformed matcher states;
  public input validation and tree/token overflow checks remain fallible.

## Current retained gap map after Batch 46

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | 66 | 0 |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |
