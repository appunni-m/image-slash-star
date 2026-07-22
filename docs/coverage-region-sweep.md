# Coverage region sweep tracker

This file tracks the bulk pass toward 100% line, branch, function, and region
coverage. It supersedes one-off notes for new work: every sweep batch should
update this file before implementation and again after the Coverage MCP run.

## Rules for this sweep

- Coverage test command: `all-features-llvm-cov-json-nightly-branch`.
- Run coverage only through Coverage MCP.
- Validate each retained batch with:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Retain only when missing lines/functions do not regress and at least one of
  missing branches or missing regions improves.
- Prefer fixture/oracle parity cases for public states. Use coverage hooks only
  for private helper states, defensive overflow/error states, or impossible
  public inputs.
- Avoid broad generic coverage probes. If a synthetic reader/writer only exists
  to hit one generic error arm, extract the policy into a small non-generic
  helper and probe that directly.

## Current MCP baseline

- Source state: dirty worktree on pushed `main` commit `85f4694`.
- Included retained dirty change: WebP decoder chunk-scan error helper
  extraction from interrupted Attempt 107.
- Coverage MCP run: `78dac182-671a-41be-9000-62cab52ba392`.
- Coverage MCP snapshot: `c42f49db-28c6-4dd2-ba36-f7ead7b3611b`.
- Result: `5 passed / 0 failed`.
- Lines: `26559 / 26562` (3 missing).
- Branches: `3462 / 3466` (4 missing).
- Functions: `1611 / 1611` (0 missing).
- Regions: `42357 / 42779` (422 missing).

## File gap map

| File | Missing regions | Missing branches | Missing lines | Current read |
| --- | ---: | ---: | ---: | --- |
| `src/codecs/compression/zlib_ng.rs` | 192 | 0 | 0 | Real DEFLATE/zlib algorithm states; largest region bucket. Branches/lines/functions are already closed, so focus on compact helper probes or code-shape extraction only when it removes multiple regions. |
| `src/codecs/tiff/decode.rs` | 63 | 0 | 0 | Real TIFF parser/decoder region gaps with full branch coverage. Likely malformed tag/value/compression variants and defensive bounds. |
| `src/codecs/jpeg/encode/mod.rs` | 54 | 0 | 0 | JPEG encoder progressive/baseline event-region gaps; likely coefficient patterns and scan-script edge cases. |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 | 0 | Decoder branch is closed after helper extraction; remaining regions are mostly complex container/animation arms and normalized compiler regions. |
| `src/codecs/webp/native/lossless.rs` | 42 | 1 | 0 | One real branch remains plus private VP8L/Huffman/transform region gaps. Several gaps are generic-instantiation artifacts from coverage-only readers. |
| `src/codecs/webp/native/vp8.rs` | 20 | 3 | 3 | Highest-priority correctness coverage: all remaining line gaps and 3 of 4 remaining branches are here. |
| `src/types/buffer.rs` | 2 | 0 | 0 | Small iterator/debug/blend region artifacts; should be cheap after WebP branch work. |
| `src/codecs/webp/native/encoder.rs` | 2 | 0 | 0 | Small writer/encode generic-region artifacts. |

## Root-cause buckets

### Bucket A: generic-instantiation artifacts

Evidence: the interrupted decoder work showed a coverage-only `OtherErrorAt`
reader forced a new `WebPDecoder<OtherErrorAt>` monomorphization and created
many uncovered regions while only exercising one intended error arm. Removing
that generic probe and extracting `allow_vp8x_chunk_scan_error()` closed the
decoder branch miss.

Targets:

- `src/codecs/webp/native/lossless.rs`: coverage-only readers such as
  `OneThenErrorReader` instantiate large `LosslessDecoder<R>` methods.
- `src/codecs/webp/native/vp8.rs`: coverage-only reader/error types appear in
  `init_partitions` and related generic decoder methods.
- `src/codecs/webp/native/encoder.rs`: generic `write_chunk<W>`/encoder writer
  probes may create writer-specific region debt.

Fix pattern:

1. Identify the generic probe and intended branch/error policy.
2. Extract a narrow non-generic helper only if production behavior stays
   byte-for-byte equivalent.
3. Replace full-decoder/full-encoder synthetic probes with direct helper probes.
4. Run MCP to confirm the region/branch improvement.

### Bucket B: WebP VP8 line/branch gaps

Evidence: MCP snapshot `c42f49db-28c6-4dd2-ba36-f7ead7b3611b` reports the only
line gaps in the project are in `src/codecs/webp/native/vp8.rs`
(`1429 / 1432` lines), and 3 missing branches remain there.

Known raw one-sided branch loci from the MCP-produced LLVM JSON:

- `vp8.rs:969`, `981`, `1033`, `1056`, `1067`, `1104`, `1125`, `1129`,
  `1173`, `1177`, `1183`, `1192`, `1204`, `1215`, `1468`, `1576`,
  `1810`, `1821`.

Not all one-sided raw records are aggregate misses; many are covered in another
monomorphization. Reverse-map by function instantiation before writing tests.

### Bucket C: high-region codecs with closed branches

Targets:

- `zlib_ng.rs`: `longest_match`, `build_tree`, `write_block`, matcher
  processing, and send-tree helpers.
- `tiff/decode.rs`: `decode`, `Directory::parse`, `decode_packbits`,
  `unpack_indices`, `MsbBits::read`.
- `jpeg/encode/mod.rs`: `baseline_frequencies`, `encode`,
  progressive event helpers.

Fix pattern:

1. Use source context and existing hooks to classify each region as public
   fixture state, private invariant state, or compiler artifact.
2. Add grouped probes only where one probe closes multiple regions.
3. Avoid expanding total branch/line debt with artificial monomorphizations.

## Batch 1 plan

Goal: close the retained decoder cleanup and attack the remaining WebP branch
debt in one batch.

Planned edits:

1. Keep the WebP decoder helper extraction from Attempt 107 and record its
   measured outcome here.
2. Reverse-map `vp8.rs` missing lines/branches to exact functions and decide
   whether they are fixture-reachable or coverage-hook-only.
3. Reverse-map the single `lossless.rs` missing branch to the responsible
   function instantiation; remove any coverage-only generic artifact if that is
   the cause.
4. Validate locally and run the approved MCP command.

Implemented before measurement:

- Kept `decoder.rs` helper extraction:
  `allow_vp8x_chunk_scan_error(error) -> Result<(), DecodingError>`, replacing
  the previous full `WebPDecoder<OtherErrorAt>` synthetic-reader probe.
- Removed `vp8.rs` `CoverageReadError` full-decoder `init_partitions(1)` probe.
  It only exercised a synthetic `read_to_end` error but created a mostly
  uncovered `Vp8Decoder<CoverageReadError>` monomorphization.
- Removed `lossless.rs` full
  `LosslessDecoder<OneThenErrorReader>::decode_image_data()` probe. Direct
  `BitReader<OneThenErrorReader>::fill()` coverage remains, so the low-level
  I/O error state is still exercised without monomorphizing the full image-data
  decoder for a synthetic reader.

Batch 1 retention gate:

- Missing branches must drop below 4, or missing regions must drop below 422.
- Missing lines must stay at or below 3.
- Missing functions must stay at 0.

Batch 1 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `37fa47b6-30e1-4308-8d7f-d10df2ebe45c`.
- Coverage MCP snapshot: `fc0da0e9-ed87-4041-8772-32ae92e7779d`.
- Result: `5 passed / 0 failed`.
- Lines: `26524 / 26527` (3 missing), unchanged missing count.
- Branches: `3462 / 3466` (4 missing), unchanged missing count.
- Functions: `1610 / 1610` (0 missing), unchanged missing count.
- Regions: `42335 / 42757` (422 missing), unchanged missing count.

Retention decision:

- The decoder helper extraction remains retained from snapshot
  `c42f49db-28c6-4dd2-ba36-f7ead7b3611b` because it removed the decoder branch
  miss.
- The `vp8.rs` and `lossless.rs` probe removals are provisional. They reduced
  synthetic total code but did not reduce missing counts, so they should only
  stay if a later batch in this sweep improves missing branch/region counts.

## Batch 2 plan

Goal: reverse-map the four remaining branch misses and choose fixes that either
exercise real states or remove coverage-only generic debt.

Planned investigation:

1. Inspect raw LLVM function branch records for `vp8.rs` and `lossless.rs` at
   snapshot `fc0da0e9-ed87-4041-8772-32ae92e7779d`.
2. Group one-sided branches by function monomorphization, not just by source
   line, so that test changes target the aggregate miss instead of a branch
   already covered by another instantiation.
3. Inspect existing coverage hooks for generic reader/writer probes that create
   uncovered full-decoder methods.
4. Apply only high-confidence grouped fixes, then rerun the MCP command.

Implemented before measurement:

- `vp8.rs`: added a small coverage-only `take_decoder(bytes)` helper that
  builds `Vp8Decoder<Take<Cursor<&[u8]>>>`, matching the reader shape used by
  WebP slice decoding.
- `vp8.rs`: duplicated the missing private-state probes into that production
  reader shape:
  - `calculate_filter_parameters()` now covers B and non-B luma modes plus the
    sharpness branch in the `Take<Cursor<&[u8]>>` instantiation.
  - `read_quantization_indices()` now covers segment delta and absolute
    quantizer paths in the same instantiation.
  - `read_loop_filter_adjustments()` now covers both enabled and disabled
    adjustment flags in the same instantiation.
  - `read_frame_header()` now receives the same malformed/partial/keyframe/
    interframe cases already used for `Cursor<Vec<u8>>`, but through
    `Take<Cursor<&[u8]>>`.
- `lossless.rs`: changed coverage-only `LosslessDecoder` probes from
  `Cursor<[u8; N]>` inputs to `Cursor<Vec<u8>>` inputs. This preserves the byte
  stream while avoiding narrow decoder monomorphizations such as
  `LosslessDecoder<Cursor<[u8; 1]>>::decode_image_data()`, which owned the
  42-region lossless debt in the raw function records.

Batch 2 correction:

- The first VP8 implementation used `Cursor::take()`, which created a new
  `Vp8Decoder<Take<Cursor<&[u8]>>>` monomorphization. That run improved regions
  by only 1 but regressed missing lines and branches.
- Corrected VP8 to use a coverage-only `with_take_decoder!` macro around
  `cursor.by_ref().take(...)`, matching the existing private probe reader shape
  instead of adding a new one.

Batch 2 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `65f036f6-f5af-43d9-880a-bd87c8a4a036`.
- Coverage MCP snapshot: `bef49a4b-b056-4ad6-a8b0-fd4c5651f622`.
- Result: `5 passed / 0 failed`.
- Lines: `26575 / 26576` (1 missing), improved from 3 missing.
- Branches: `3464 / 3466` (2 missing), improved from 4 missing.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42419 / 42832` (413 missing), improved from 422 missing.

Retained outcome:

- `vp8.rs`: missing regions improved from 20 to 13; missing branches improved
  from 3 to 1; missing lines improved from 3 to 1.
- `lossless.rs`: missing regions improved from 42 to 40; missing branch remains
  1.

## Batch 3 plan

Goal: close the remaining two aggregate branch misses before doing high-region
closed-branch files.

Current branch/line targets at snapshot
`bef49a4b-b056-4ad6-a8b0-fd4c5651f622`:

- `src/codecs/webp/native/vp8.rs`: 1 missing line, 1 missing branch, 13 missing
  regions.
- `src/codecs/webp/native/lossless.rs`: 1 missing branch, 40 missing regions.

Planned investigation:

1. Reverse-map the remaining VP8 branch/line through raw LLVM function records.
2. Reverse-map the remaining lossless branch through raw LLVM function records.
3. Prefer adding one focused private probe per codec only if it avoids new
   generic monomorphizations.

Implemented before measurement:

- `vp8.rs`: added a full-enough interframe byte sequence to the existing
  `with_take_decoder!` frame-header cases. The remaining VP8 branch candidate
  is `read_frame_header()` line 1204, `if !self.frame.keyframe`; previous short
  interframe inputs failed before reaching that unsupported-feature return.
- `lossless.rs`: added
  `LosslessDecoder::<Cursor<Vec<u8>>>::get_copy_distance(&mut reader, 1)` next
  to the existing prefix-code 4 probe. The remaining lossless branch candidate
  is the `prefix_code < 4` fast path at line 655 for the existing
  `Cursor<Vec<u8>>` instantiation.

Batch 3 follow-up before measurement:

- Raw records after the first Batch 3 MCP run showed VP8 line/branch coverage
  was closed; the only remaining aggregate branch was still in `lossless.rs`.
- The likely lossless branch is `plane_code_to_distance()` line 675,
  `if dist < 1`. Existing probes used positive-distance cases only.
- Added `LosslessDecoder::<Cursor<Vec<u8>>>::plane_code_to_distance(1, 4)`,
  where distance map entry 4 is `(-1, 1)` and `xsize == 1`, producing
  `dist == 0` and taking the clamp-to-1 branch.

Batch 3 second follow-up before measurement:

- The distance-map probe improved one region but did not close the final branch.
- Latest raw records show the branch miss is per-instantiation; the smallest
  remaining candidates are `BitReader::consume()` error paths for existing
  `Cursor<[u8; 5]>`, `Cursor<[u8; 8]>`, and
  `Take<&mut Cursor<&[u8]>>` instantiations.
- Added direct `consume(1)` calls with `nbits == 0` for those existing
  instantiations.
- Also added a generic helper that calls `plane_code_to_distance(1, 4)` for the
  existing take-reader `LosslessDecoder<R>` type without spelling a new concrete
  type.

Batch 3 second follow-up measurement:

- Coverage MCP run: `a51ab64a-7275-4466-9f5c-4169427d01bb`.
- Coverage MCP snapshot: `09457357-2e91-4796-9f63-4c19e2fc4365`.
- Result: `5 passed / 0 failed`.
- Lines: `26598 / 26598` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1611 / 1611` (0 missing), unchanged.
- Regions: `42467 / 42879` (412 missing), unchanged missing count.

Rejection:

- This follow-up did not reduce missing branches or regions, so the extra
  `consume()` and take-reader generic helper probes were removed.
- Retain only the earlier `plane_code_to_distance(1, 4)` addition, which
  reduced missing regions by 1.

Correction:

- Rechecking the missing-count rule showed
  `get_copy_distance(&mut reader, 1)` and
  `plane_code_to_distance(1, 4)` increased covered and total regions together,
  but did not reduce missing regions.
- Both no-op probes were removed.
- The retained lossless improvement in this batch is the later
  `read_color_cache()` valid-code-bits probe.

Batch 3 third follow-up before measurement:

- Re-examined one-sided lossless branches after removing the rejected probes.
- Better candidate: `read_color_cache()` line 640,
  `if !(1..=11).contains(&code_bits)`.
- Existing probes covered:
  - absent color cache (`read_bits(1) == 0`),
  - invalid color-cache bits.
- Added a direct `LosslessDecoder<Cursor<Vec<u8>>>` bit-buffer state with
  `buffer == 0b00011` and `nbits == 5`, which reads the present flag as `1`
  and `code_bits == 1`, covering the valid code-bits branch without creating a
  new reader type.

Batch 3 third follow-up measurement:

- Coverage MCP run: `c6c554c9-55c5-4e20-aba3-841c60de7aab`.
- Coverage MCP snapshot: `59fb9d6c-06a6-4f88-9449-8395a9ca16eb`.
- Result: `5 passed / 0 failed`.
- Lines: `26593 / 26593` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42437 / 42848` (411 missing), improved from 412 missing.

Retained outcome:

- Line coverage is 100%.
- VP8 line and branch coverage are 100%.
- One aggregate branch remains in `lossless.rs`.
- Total missing regions are down to 411 before removing the no-op lossless
  probes; rerun required after cleanup to confirm the same retained missing
  count.

Batch 3 cleaned-state measurement:

- Removed the no-op `get_copy_distance(1)` and `plane_code_to_distance(1, 4)`
  probes.
- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `64c05140-af39-469b-bad1-c6bb4ca3c331`.
- Coverage MCP snapshot: `7e711cbc-b0a5-4009-8834-e2ac30597abb`.
- Result: `5 passed / 0 failed`.
- Lines: `26591 / 26591` (0 missing).
- Branches: `3465 / 3466` (1 missing).
- Functions: `1610 / 1610` (0 missing).
- Regions: `42435 / 42846` (411 missing).

## Batch 4 plan

Goal: after closing all line gaps and all VP8 branch gaps, reduce remaining
region debt starting with the smallest files to avoid losing time in the large
DEFLATE bucket.

Current retained gaps at snapshot `7e711cbc-b0a5-4009-8834-e2ac30597abb`:

- `src/codecs/webp/native/encoder.rs`: 2 regions.
- `src/types/buffer.rs`: 2 regions.
- `src/codecs/webp/native/vp8.rs`: 12 regions.
- `src/codecs/webp/native/lossless.rs`: 39 regions and 1 aggregate branch.
- `src/codecs/webp/native/decoder.rs`: 47 regions.
- `src/codecs/jpeg/encode/mod.rs`: 54 regions.
- `src/codecs/tiff/decode.rs`: 63 regions.
- `src/codecs/compression/zlib_ng.rs`: 192 regions.

Planned order:

1. Close or classify the two 2-region files.
2. Try one focused VP8/lossless cleanup pass.
3. Move to decoder/JPEG/TIFF before the large zlib bucket.

Implemented before measurement:

- `src/codecs/webp/native/encoder.rs`: added a successful odd-length
  fixed-buffer `write_chunk(Cursor<&mut [u8]>, b"ODD!", &[1, 2, 3])` case.
  MCP mapped the encoder gap to `write_chunk()` line 1129, the odd-padding
  branch. Existing fixed-buffer probes covered even success and odd padding
  failure; this adds the missing odd-padding success state without a new writer
  type.
- `tests/coverage_matrix_tests.rs`: added generic `exercise_buffer<P>` edge
  calls for the existing manifest-driven buffer matrix instantiations:
  one-pixel `next_back()`/`next()` exhaustion for pixel and row iterators, and
  panicking `get_pixel()` / `get_pixel_mut()` calls for both `x >= width` and
  `y >= height`.

Hypothesis:

- Encoder should close its 2 missing regions because the new probe reaches the
  uncovered fixed-buffer odd-padding success arm.
- Buffer may close its 2 missing regions if they are from public iterator/access
  edge states in the integration-test monomorphizations. If the miss is from
  private malformed-buffer branches that cannot be reached publicly, revert any
  no-op matrix additions and classify the remaining regions instead.

Batch 4 retention gate:

- Lines must stay at 100%.
- Functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 411, or the new Batch 4 probes are no-op
  candidates for removal.

Batch 4 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `630ae01f-7fe1-4617-8b58-d6d269765624`.
- Coverage MCP snapshot: `b6a137fa-8724-4128-b1f7-b4cc29878751`.
- Result: `5 passed / 0 failed`.
- Lines: `26597 / 26597` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42456 / 42864` (408 missing), improved from 411 missing.

Retained outcome:

- `src/types/buffer.rs`: improved from 2 missing regions to 0.
- `src/codecs/webp/native/encoder.rs`: improved from 2 missing regions to 1.
- Total missing regions improved by 3. The total region denominator increased
  by 18 because the generic matrix now records additional public edge states,
  but missing regions still dropped, so the batch is retained.

## Batch 5 plan

Goal: close the last `encoder.rs` region and start isolating the final
aggregate branch in `lossless.rs`.

Current retained gaps at snapshot `b6a137fa-8724-4128-b1f7-b4cc29878751`:

- `src/codecs/webp/native/encoder.rs`: 1 region at `write_chunk()` line 1129.
- `src/codecs/webp/native/lossless.rs`: 39 regions and 1 aggregate branch.
- `src/codecs/webp/native/vp8.rs`: 12 regions, 0 branches.
- `src/codecs/webp/native/decoder.rs`: 47 regions.
- `src/codecs/jpeg/encode/mod.rs`: 54 regions.
- `src/codecs/tiff/decode.rs`: 63 regions.
- `src/codecs/compression/zlib_ng.rs`: 192 regions.

Planned edits/investigation:

1. Add an odd-length `write_chunk(&mut Cursor<&mut [u8]>, ...)` success case.
   Batch 4 covered `Cursor<&mut [u8]>` by value; `WebPEncoder::encode()` calls
   `write_chunk(&mut self.writer, ...)`, which monomorphizes `write_chunk` for
   `&mut Cursor<&mut [u8]>`. The remaining encoder gap is still line 1129, so
   this is the exact remaining writer shape.
2. Reverse-map the single `lossless.rs` aggregate branch using raw branch
   records grouped by function instantiation before patching.

Implemented before measurement:

- `src/codecs/webp/native/encoder.rs`: added the exact
  `write_chunk(&mut odd_fixed_cursor, b"ODD!", &[1, 2, 3])` success probe.
- `lossless.rs`: no code change in this batch. Raw one-sided branch records are
  widespread per instantiation, while aggregation by source line/column did not
  identify a single globally one-sided branch. Keep this as a separate
  reverse-mapping problem instead of adding broad probes.

Batch 5 retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 408 unless Batch 5 closes the final
  branch.

Batch 5 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `c882e135-3323-4d79-a225-ab2cb7130f1d`.
- Coverage MCP snapshot: `e2c4afd8-d600-4740-97c4-abc7bc2c2ca7`.
- Result: `5 passed / 0 failed`.
- Lines: `26600 / 26600` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42466 / 42874` (408 missing), unchanged missing count.

Rejection:

- The `write_chunk(&mut Cursor<&mut [u8]>, ...)` probe increased covered and
  total regions together but did not reduce missing regions or branches.
- Removed the Batch 5 encoder probe. The latest retained state remains Batch 4
  at snapshot `b6a137fa-8724-4128-b1f7-b4cc29878751`.

## Batch 6 plan

Goal: close the last encoder region with the exact owning monomorphization.

Reverse mapping:

- MCP still reports the only encoder gap at `write_chunk()` line 1129.
- Raw LLVM branch records show one zero-hit record for the symbol ending in
  `write_chunkQQINt...Vec...`, which is the nested mutable Vec writer shape
  produced by `WebPEncoder::new(&mut out)` followed by
  `write_chunk(&mut self.writer, ...)`.
- Batch 5 tried `&mut Cursor<&mut [u8]>`, which was the wrong nested writer
  type and was rejected.

Implemented before measurement:

- `src/codecs/webp/native/encoder.rs`: add an odd-length
  `write_chunk(&mut &mut Vec<u8>, b"ODD!", &[1, 2, 3])` success probe using the
  same nested mutable Vec writer shape as the public WebP wrapper.

Batch 6 retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 408.

Batch 6 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `05733dc8-7627-4a09-8d37-d786e4d2ee4c`.
- Coverage MCP snapshot: `bee1c611-a266-4b18-b18d-f519af652326`.
- Result: `5 passed / 0 failed`.
- Lines: `26600 / 26600` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42465 / 42873` (408 missing), unchanged missing count.

Rejection:

- The nested mutable Vec writer probe did not reduce missing regions or
  branches, so it was removed.
- The remaining `encoder.rs` line 1129 partial-branch region should be treated
  as low-priority unless a future raw-record pass identifies a different
  owning state. Two exact writer-shape probes have now been no-ops.

## Batch 7 plan

Goal: reduce VP8 region debt without new reader monomorphizations.

Reverse mapping:

- Current retained VP8 gaps are 12 region-only misses.
- Raw zero-hit regions for the existing `Vp8Decoder<Cursor<Vec<u8>>>`
  `read_frame_header()` instantiation include:
  - line 1130: invalid keyframe magic return,
  - line 1167: first-partition `read_exact` failure after a valid header.
- Existing frame-header probes cover short keyframe read errors and several
  valid keyframe/interframe states, but not these two concrete
  `Cursor<Vec<u8>>` malformed states.

Implemented before measurement:

- Add `read_frame_header()` case `[0, 0, 0, 0, 0, 0]`: keyframe with complete
  three-byte magic field that is not `9d 01 2a`.
- Add `read_frame_header()` case
  `[0x20, 0, 0, 0x9d, 0x01, 0x2a, 8, 0, 8, 0]`: keyframe declares
  `first_partition_size == 1` but provides no partition byte after dimensions.

Batch 7 retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 408.

Batch 7 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `dbd6ab5f-15eb-4ec9-be5d-04eca2a2f23e`.
- Coverage MCP snapshot: `0240ab72-20e1-427f-8ada-b807cf6ffd7d`.
- Result: `5 passed / 0 failed`.
- Lines: `26602 / 26602` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42460 / 42868` (408 missing), unchanged missing count.

Rejection:

- The two additional `Cursor<Vec<u8>>` malformed frame-header cases covered
  added code but did not reduce missing regions or branches.
- Removed the Batch 7 VP8 probes. Remaining VP8 gaps appear to be lower-level
  region accounting in existing private decode states; do not add more
  frame-header cases unless raw records identify an improving owner.

## Batch 8 plan

Goal: reduce `lossless.rs` region debt through existing low-level reader
instantiations before touching full VP8L decode states.

Reverse mapping:

- `lossless.rs` still has 39 missing regions and the final aggregate branch.
- Raw region records show zero-hit regions in
  `BitReader<Cursor<Vec<u8>>>::read_bits::<u16>()` and
  `BitReader<Cursor<Vec<u8>>>::read_bits::<usize>()`.
- Existing hook coverage exercises `BitReader` mainly through fixed-array
  readers and full decoder paths. Direct `Cursor<Vec<u8>>` `read_bits` calls can
  cover those generic instantiations without introducing a new reader type.

Implemented before measurement:

- Add direct `BitReader<Cursor<Vec<u8>>>` calls:
  - enough buffered bits for `read_bits::<u16>(14)`,
  - enough buffered bits for `read_bits::<usize>(4)`,
  - empty reader for `read_bits::<usize>(4)` fill-then-consume-error path.

Batch 8 retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 408, or the probes are removed.

Batch 8 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `1e7b0053-4523-4037-803f-f1eb9c795dab`.
- Coverage MCP snapshot: `ef540fbd-c45e-4b5d-845e-5a447aedcca7`.
- Result: `5 passed / 0 failed`.
- Lines: `26611 / 26611` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42470 / 42878` (408 missing), unchanged missing count.

Rejection:

- Direct `BitReader<Cursor<Vec<u8>>>` `read_bits::<u16>()` and
  `read_bits::<usize>()` probes increased covered and total regions together
  without reducing missing branches or regions.
- Removed the Batch 8 low-level probes. The lossless branch/region misses are
  not solved by direct low-level reader calls; future attempts need to target a
  full decoder state or refactor a non-generic policy helper.

## Cleanup retained-state measurement

After removing rejected Batch 5 through Batch 8 probes, reran the approved MCP
coverage command on the final retained worktree state.

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `e81885eb-db68-4e70-a7b1-b66ccc2f38d5`.
- Coverage MCP snapshot: `1a8a1a77-d805-4021-b7a0-e12430850e75`.
- Result: `5 passed / 0 failed`.
- Lines: `26597 / 26597` (0 missing).
- Branches: `3465 / 3466` (1 missing).
- Functions: `1610 / 1610` (0 missing).
- Regions: `42456 / 42864` (408 missing).

Retained final outcome of this sweep:

- Lines: improved from 3 missing at the initial sweep baseline to 0 missing.
- Branches: improved from 4 missing at the initial sweep baseline to 1 missing.
- Regions: improved from 422 missing at the initial sweep baseline to 408
  missing.
- `src/types/buffer.rs`: now 100% regions.
- `src/codecs/webp/native/vp8.rs`: now 100% lines and branches; 12 regions
  remain.
- `src/codecs/webp/native/encoder.rs`: improved from 2 to 1 missing region.

Remaining retained gap map:

| File | Missing regions | Missing branches | Notes |
| --- | ---: | ---: | --- |
| `src/codecs/compression/zlib_ng.rs` | 192 | 0 | Largest remaining bucket; needs algorithm-state sweep, not generic probes. |
| `src/codecs/tiff/decode.rs` | 63 | 0 | Parser/decoder region-only states. |
| `src/codecs/jpeg/encode/mod.rs` | 54 | 0 | Encoder scan/coefficient region-only states. |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 | Container/animation region-only states. |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 | Final aggregate branch plus VP8L decoder region states; direct low-level reader probes were no-ops. |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 | Region-only private decode states; extra frame-header malformed cases were no-ops. |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 | Remaining `write_chunk()` line 1129 region behaved like a low-priority LLVM/generic artifact after exact writer probes. |

## Batch 9 plan

Goal: perform one region-first sweep across all retained incomplete files, then
apply the smallest high-confidence cleanup before the next Coverage MCP run.

Current retained MCP baseline:

- Coverage MCP run: `e81885eb-db68-4e70-a7b1-b66ccc2f38d5`.
- Coverage MCP snapshot: `1a8a1a77-d805-4021-b7a0-e12430850e75`.
- Lines: `26597 / 26597` (0 missing).
- Branches: `3465 / 3466` (1 missing).
- Functions: `1610 / 1610` (0 missing).
- Regions: `42456 / 42864` (408 missing).

Cross-file reverse mapping:

- `src/codecs/compression/zlib_ng.rs`: region gaps are distributed across
  level-specific matchers (`SlowMatcher`, `Level3Matcher`, `Level6Matcher`,
  `Level9Matcher`) and Huffman/tree emission (`build_tree`, `write_block`,
  `send_tree`, `send_trees`, `emit_tokens`, `emit_fixed_block`). Existing
  boundary PNG compression fixtures already cover levels 1-9. Next useful work
  is algorithm-state construction or invariant cleanup, not more compression
  boundary rows.
- `src/codecs/tiff/decode.rs`: raw zero-hit regions are concentrated in
  `decode()`, `Directory::parse()`, `unpack_indices()`, `decode_packbits()`,
  and `MsbBits::read()`. Existing public malformed TIFF fixtures and
  `#[cfg(coverage)]` private probes already call the helper states. The
  remaining gaps are mainly conversion and checked-arithmetic regions after
  classic TIFF has constrained directory values to 32-bit fields and after tile
  indexes are bounded by `tiles_across * tiles_down`.
- `src/codecs/jpeg/encode/mod.rs`: gaps cluster in
  `baseline_frequencies()`, progressive DC/AC event construction,
  `flush_progressive_eob()`, and the main `encode()` scan path. Fixture-backed
  work requires exact Pillow oracle JPEGs that force coefficient/EOB/refinement
  patterns; earlier progressive odd-size byte-parity attempts were rejected.
- `src/codecs/webp/native/decoder.rs`: gaps are mostly existing public reader
  monomorphizations (`Cursor<Vec<u8>>` and `Cursor<&[u8]>`) in VP8X/container
  frame parsing. New synthetic readers should be avoided because they increase
  generic region debt.
- `src/codecs/webp/native/lossless.rs`: the final branch and region gaps are
  dominated by `LosslessDecoder<R>` and `BitReader<R>` instantiations. Direct
  low-level reader probes were no-ops, so the next work must either generate
  real VP8L fixture states or extract non-generic policy helpers.
- `src/codecs/webp/native/vp8.rs`: remaining 12 gaps are region-only private
  decode states; extra malformed frame-header cases did not move aggregate
  coverage.
- `src/codecs/webp/native/encoder.rs`: the remaining single region stayed after
  two exact writer-shape attempts and is currently low priority.

Batch 9 implementation target:

- Start with `src/codecs/tiff/decode.rs` because it has closed branches,
  public-parser invariants, and small localized checked regions.
- Convert `Directory` value APIs from `u64` to `u32`. Classic TIFF field
  decoding in this implementation only returns BYTE/SHORT/LONG values, all of
  which fit in `u32`; this removes unreachable `u32::try_from(...)` and
  `usize::try_from(...)` failure regions in the decoder setup.
- Store TIFF offsets and byte counts as `usize` in the decode loops after
  parsing. On supported Rust targets, classic TIFF LONG values fit in `usize`;
  this removes unreachable conversion arms while preserving all bounds checks on
  actual slice access.
- Replace tile placement checked arithmetic whose operands are already bounded
  by `offsets.len() == tiles_across * tiles_down` and by `copied_width /
  copied_height` with ordinary arithmetic, leaving slice `get()` / `get_mut()`
  checks as the final safety boundary.

Batch 9 retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 408.
- If TIFF regions do not improve, revert the code changes and keep only this
  classification as the retained result.

Implemented before measurement:

- `src/codecs/tiff/decode.rs`: changed classic TIFF compression constants and
  `Directory` value APIs from `u64` to `usize`.
- Removed unreachable `u32::try_from(...)` / `usize::try_from(...)` decoder
  setup failures after `Directory::values()` has already decoded BYTE, SHORT,
  and LONG values.
- Stored TIFF offsets and byte counts as `usize` in decode loops, preserving
  all slice-bound checks through `data.get(...)`, `pixels.get_mut(...)`, and
  `decoded.get(...)`.
- Replaced tile placement checked arithmetic with direct arithmetic where
  `offsets.len() == tiles_across * tiles_down` and the copied tile rectangle
  already bounds `tile_x`, `tile_y`, `copied_width`, and `copied_height`.

Batch 9 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `ea43daa6-bcba-44a3-9869-24be754ad232`.
- Coverage MCP snapshot: `fd558f0e-a8ba-48e1-b7fc-ad4101bc8d3a`.
- Result: `5 passed / 0 failed`.
- Lines: `26590 / 26590` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42389 / 42770` (381 missing), improved by 27.
- `src/codecs/tiff/decode.rs`: improved from `1929 / 1992` regions
  (63 missing) to `1862 / 1898` regions (36 missing).

Retention:

- Retain Batch 9. It reduced unreachable TIFF region debt without regressing
  line, branch, or function coverage.

Remaining retained gap map after Batch 9:

| File | Missing regions | Missing branches | Notes |
| --- | ---: | ---: | --- |
| `src/codecs/compression/zlib_ng.rs` | 192 | 0 | Largest remaining bucket; algorithm-state / Huffman tree sweep. |
| `src/codecs/jpeg/encode/mod.rs` | 54 | 0 | Progressive and baseline coefficient-event states. |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 | VP8X/container frame parsing across existing public reader shapes. |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 | Final branch plus generic VP8L decoder/BitReader states. |
| `src/codecs/tiff/decode.rs` | 36 | 0 | Remaining checked arithmetic and parse-helper overflow protections. |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 | Region-only private decode states. |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 | Low-priority writer region artifact. |

## Batch 10 plan

Goal: close public malformed-TIFF region states with manifest-driven oracle
fixtures before adding any more private probes.

Reverse mapping from snapshot `fd558f0e-a8ba-48e1-b7fc-ad4101bc8d3a`:

- `decode()` line 29: `directory.one(257)?` missing-height / unsupported
  height value path.
- `decode()` line 34: `directory.values_or(258, &[1])?` unsupported
  BitsPerSample value path.
- `decode()` lines 80-81: missing/unsupported `TileWidth` and `TileLength`.
- `decode()` lines 149-150: missing/unsupported `StripOffsets` and
  `StripByteCounts`.
- `decode()` line 182: compressed missing-byte-count inference where a next
  strip offset is lower than the current offset.

Planned fixtures:

- `ascii_height.tiff`
- `ascii_bits.tiff`
- `ascii_strip_offsets.tiff`
- `ascii_strip_byte_counts.tiff`
- `ascii_tile_width.tiff`
- `ascii_tile_height.tiff`
- `compressed_descending_strip_offsets.tiff`

Implementation path:

1. Extend `scripts/generate_test_assets.py` TIFF generation with deterministic
   mutations for those assets.
2. Add the assets to the TIFF `error_bad_ifd` manifest row.
3. Regenerate TIFF assets and refs with the pinned Pillow oracle.
4. Run local checks and the approved Coverage MCP command.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 381.
- If the fixture rows do not improve aggregate regions, remove them from the
  manifest/generated files and record the rejection.

Implemented before measurement:

- Added deterministic TIFF malformed assets:
  - `ascii_height.tiff`
  - `ascii_bits.tiff`
  - `ascii_strip_offsets.tiff`
  - `ascii_compressed_strip_byte_counts.tiff`
  - `ascii_tile_width.tiff`
  - `ascii_tile_height.tiff`
  - `compressed_descending_strip_offsets.tiff`
- Rejected and removed attempted `ascii_strip_byte_counts.tiff` because Pillow
  accepts the uncompressed form, so it cannot live under an `expect_error`
  oracle row.
- Updated the TIFF `error_bad_ifd` manifest row and regenerated TIFF assets,
  decode refs, and coverage matrix rows with the pinned Pillow oracle.

Batch 10 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `ba168d79-bbd6-4b66-9fb5-77d132651306`.
- Coverage MCP snapshot: `96f84dfa-3470-4cc9-b3ad-3d7bbdd347f9`.
- Result: `5 passed / 0 failed`.
- Lines: `26590 / 26590` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42395 / 42770` (375 missing), improved by 6.
- `src/codecs/tiff/decode.rs`: improved from `1862 / 1898` regions
  (36 missing) to `1868 / 1898` regions (30 missing).

Retention:

- Retain Batch 10. The added cases are manifest-driven Pillow oracle fixtures
  and reduce aggregate region debt without local or coverage regression.

Remaining retained gap map after Batch 10:

| File | Missing regions | Missing branches | Notes |
| --- | ---: | ---: | --- |
| `src/codecs/compression/zlib_ng.rs` | 192 | 0 | Largest remaining bucket; algorithm-state / Huffman tree sweep. |
| `src/codecs/jpeg/encode/mod.rs` | 54 | 0 | Progressive and baseline coefficient-event states. |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 | VP8X/container frame parsing across existing public reader shapes. |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 | Final branch plus generic VP8L decoder/BitReader states. |
| `src/codecs/tiff/decode.rs` | 30 | 0 | Remaining checked arithmetic and parse-helper overflow protections. |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 | Region-only private decode states. |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 | Low-priority writer region artifact. |

## Batch 12 plan

Goal: close the final aggregate branch if it is a real existing reader-shape
gap, without adding a new synthetic monomorphization.

Reverse mapping from snapshot `c0c900e9-781f-4d40-8c11-f2a2cf13e55a`:

- The only aggregate branch debt is in `src/codecs/webp/native/lossless.rs`.
- Raw records repeatedly point at `BitReader::read_bits()` line 1320:
  `if self.nbits < num`.
- Previous direct `Cursor<Vec<u8>>` `read_bits::<u16>()` /
  `read_bits::<usize>()` probes were rejected as no-ops.
- Existing raw records include `BitReader<Take<Cursor<Vec<u8>>>>`, so a direct
  probe for that already-existing public reader shape should not introduce a
  new reader type.

Planned edit:

- Add direct coverage-hook calls for
  `BitReader<Take<Cursor<Vec<u8>>>>::read_bits::<usize>()` and
  `read_bits::<u16>()`, including both an initial fill path and a subsequent
  no-fill path.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must drop below 1, or missing regions must improve below
  371.
- If the probe does not improve branch or region debt, remove it.

Implemented before measurement:

- Added direct `BitReader<Take<Cursor<Vec<u8>>>>` calls for
  `read_bits::<usize>(4)` on fill and no-fill paths and
  `read_bits::<u16>(14)` on the fill path.

Batch 12 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `eebd1ad5-4fd2-4cdc-a4f0-0b153ca780fe`.
- Coverage MCP snapshot: `7b5e856c-30ff-49fd-86cb-f83b08c63a15`.
- Result: `5 passed / 0 failed`.
- Lines: `26593 / 26593` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42412 / 42783` (371 missing), unchanged missing count while total
  regions increased.

Rejection:

- Removed the Batch 12 probe. The final lossless branch is not closed by direct
  existing-reader-shape `read_bits()` probes; it likely needs full VP8L fixture
  construction or non-generic helper extraction.
- Batch 12 is not retained.

## Final cleanup measurement after Batch 12 rejection

After removing the rejected Batch 12 probe, reran the approved Coverage MCP
command to validate the current worktree state.

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `12347c57-ce3f-459d-bff1-2f6475d432f1`.
- Coverage MCP snapshot: `48ce7feb-ce1d-406d-9c46-4d76e0e1433f`.
- Result: `5 passed / 0 failed`.
- Lines: `26588 / 26588` (0 missing).
- Branches: `3465 / 3466` (1 missing).
- Functions: `1610 / 1610` (0 missing).
- Regions: `42396 / 42767` (371 missing).

Current retained gap map:

| File | Missing regions | Missing branches | Notes |
| --- | ---: | ---: | --- |
| `src/codecs/compression/zlib_ng.rs` | 188 | 0 | Largest remaining bucket; matcher/tree states and private helper defensive arms. |
| `src/codecs/jpeg/encode/mod.rs` | 54 | 0 | Progressive and baseline coefficient-event states. |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 | VP8X/container frame parsing across existing public reader shapes. |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 | Final branch plus generic VP8L decoder/BitReader states. |
| `src/codecs/tiff/decode.rs` | 30 | 0 | Remaining checked arithmetic and parse-helper overflow protections. |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 | Region-only private decode states. |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 | Low-priority writer region artifact. |

## Batch 11 plan

Goal: make a minimal zlib region cleanup where invariants are local and
mechanically checkable.

Reverse mapping from snapshot `96f84dfa-3470-4cc9-b3ad-3d7bbdd347f9`:

- `quick_insert_level1()` has a remaining region at the `head.get_mut(hash)?`
  update. The hash is derived from a 32-bit word by `>> 16`, so it is always in
  `0..HASH_SIZE` (`65_536`). Production and coverage callers allocate
  `head = vec![0usize; HASH_SIZE]`.
- `level1_window_tail_distance_one()` has checked-arithmetic gaps around
  `WINDOW_SIZE.checked_mul(2)?.checked_sub(MIN_LOOKAHEAD)?` and
  `position.checked_sub(1)?`. The first expression is constant arithmetic, and
  the guard `position < first_slide_guard_start || ...` short-circuits before
  any `position - 1` use; since `first_slide_guard_start > 0`, later uses have
  `position > 0`.

Planned edits:

- Replace the impossible hash-table `get()` / `get_mut()` fallible lookups with
  direct indexing in `quick_insert_level1()`.
- Replace constant checked arithmetic in `level1_window_tail_distance_one()`
  with direct constant arithmetic.
- Split the tail-byte comparison after the guard and use `position - 1`
  directly.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 375.
- If aggregate regions do not improve, revert Batch 11.

Implemented before measurement:

- Tried direct indexing in `quick_insert_level1()` for the fixed hash-table
  invariant. This was rejected because the coverage hook intentionally calls
  `quick_insert_level1(b"abcd", 0, &mut empty_head)` to prove the private helper
  returns `None` for an invalid head slice. Keeping fallible `head.get()` /
  `head.get_mut()` is therefore correct for the helper signature.
- Retained the `level1_window_tail_distance_one()` cleanup:
  - replaced constant checked arithmetic with `WINDOW_SIZE * 2 - MIN_LOOKAHEAD`;
  - split the byte comparison after the short-circuit guard;
  - used `position - 1` only after `position >= first_slide_guard_start`, which
    proves `position > 0`.

Rejected run:

- Coverage MCP run: `baf79ff5-e8e1-4e6f-afa1-6ef5023daccf`.
- Result: failed `test_internal_coverage_hooks`.
- Root cause: direct indexing panicked on the intentional empty-head probe:
  `index out of bounds: the len is 0 but the index is 25357`.

Batch 11 measurement:

- Local checks passed after reverting the direct-index change:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `60e5085a-f68e-4027-ba73-d068535c5012`.
- Coverage MCP snapshot: `c0c900e9-781f-4d40-8c11-f2a2cf13e55a`.
- Result: `5 passed / 0 failed`.
- Lines: `26588 / 26588` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42396 / 42767` (371 missing), improved by 4.
- `src/codecs/compression/zlib_ng.rs`: improved from `3476 / 3668` regions
  (192 missing) to `3477 / 3665` regions (188 missing).

Retention:

- Retain Batch 11 tail arithmetic cleanup only. Do not retry direct indexing
  unless `quick_insert_level1()` is refactored to accept a fixed-size head array
  and the invalid-head coverage probe is deliberately removed.

Remaining retained gap map after Batch 11:

| File | Missing regions | Missing branches | Notes |
| --- | ---: | ---: | --- |
| `src/codecs/compression/zlib_ng.rs` | 188 | 0 | Largest remaining bucket; matcher/tree states and private helper defensive arms. |
| `src/codecs/jpeg/encode/mod.rs` | 54 | 0 | Progressive and baseline coefficient-event states. |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 | VP8X/container frame parsing across existing public reader shapes. |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 | Final branch plus generic VP8L decoder/BitReader states. |
| `src/codecs/tiff/decode.rs` | 30 | 0 | Remaining checked arithmetic and parse-helper overflow protections. |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 | Region-only private decode states. |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 | Low-priority writer region artifact. |

## Batch 13 plan

Goal: close the smallest retained bucket, the single WebP encoder region,
without adding another synthetic writer monomorphization.

Reverse mapping from snapshot `48ce7feb-ce1d-406d-9c46-4d76e0e1433f`:

- `src/codecs/webp/native/encoder.rs` has `1810 / 1811` covered regions and
  full line/branch/function coverage.
- The only reported gap is line 1129 in `write_chunk()`:
  `if data.len() % 2 == 1`.
- Existing coverage already calls `write_chunk()` with odd and even payloads
  for `Vec<u8>` and fixed-buffer writers, and successful public WebP encoding
  for fixed buffers. The remaining gap is therefore a generic writer-shape
  artifact, not a missing image fixture.

Planned edit:

- Replace the branchy odd-padding write with an equivalent unconditional
  zero-or-one-byte write:
  `w.write_all(&[0][..data.len() % 2])?`.
- This preserves WebP RIFF semantics: odd payloads receive one zero padding
  byte; even payloads write an empty slice, which `write_all` treats as a
  no-op.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 371.
- If the refactor does not improve aggregate regions, revert it and keep
  `encoder.rs` on the remaining-gap list.

Batch 13 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `f057f2d3-db85-4645-b4fd-63c742bf0656`.
- Coverage MCP snapshot: `c3ac4eda-7664-4564-9033-a483fd5f4673`.
- Result: `5 passed / 0 failed`.
- Lines: `26586 / 26586` (0 missing), unchanged missing count.
- Branches: `3463 / 3464` (1 missing), unchanged missing count.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42395 / 42766` (371 missing), unchanged missing count.

Rejection:

- Removed the Batch 13 code refactor. It reduced total counted branches/lines
  by removing the explicit `if`, but the missing region count did not improve
  and `src/codecs/webp/native/encoder.rs` still reported one missing region.
- Keep the encoder bucket open. A later fix should either find the exact
  remaining writer monomorphization with raw region records or accept this as
  an LLVM coverage artifact after higher-value buckets are closed.

## Batch 14 plan

Goal: remove VP8 region debt caused by redundant synthetic
`read_frame_header()` monomorphizations.

Reverse mapping from snapshot `48ce7feb-ce1d-406d-9c46-4d76e0e1433f`:

- `src/codecs/webp/native/vp8.rs` has full line/branch/function coverage and
  12 missing regions.
- Raw LLVM records show a 12-region zero bucket in
  `Vp8Decoder<Cursor<Vec<u8>>>::read_frame_header()`.
- The same malformed/keyframe/interframe byte sequences are already exercised
  later through the `with_take_decoder!` helper, which builds the
  production-shaped `Vp8Decoder<Take<Cursor<&[u8]>>>`.
- Production VP8 decode reaches `read_frame_header()` through
  `Vp8Decoder::decode_frame(reader)` where the caller passes a bounded
  `take(...)` reader from the WebP container. The direct `Cursor<Vec<u8>>`
  `read_frame_header()` calls are therefore coverage-only duplicates.

Planned edit:

- Remove only the direct `Vp8Decoder<Cursor<Vec<u8>>>::read_frame_header()`
  probe block.
- Keep direct `Cursor<Vec<u8>>` probes for other private methods that are not
  duplicated in the production-shaped helper.
- Keep the `take_frame_cases` block as the canonical frame-header private-state
  coverage.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 371.
- If branch debt regresses or VP8 regions do not improve, restore the removed
  probes and classify the remaining VP8 debt as real byte-pattern debt.

Batch 14 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `f2701673-2e73-4107-a002-d703ac3f79f4`.
- Coverage MCP snapshot: `85264340-b0d8-4fc9-96d1-fe0f24dac9c4`.
- Result: `5 passed / 0 failed`.
- Lines: `26566 / 26566` (0 missing), unchanged missing count.
- Branches: `3465 / 3466` (1 missing), unchanged missing count.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42347 / 42719` (372 missing), regressed by 1.
- `src/codecs/webp/native/vp8.rs`: changed from `2742 / 2754`
  (12 missing) to `2693 / 2706` (13 missing).

Rejection:

- Restored the removed `Cursor<Vec<u8>>::read_frame_header()` probes. Although
  raw region records showed a 12-region zero bucket in that monomorphization,
  removing it also removed covered regions that were contributing to the
  aggregate source mapping and worsened VP8 by one missing region.
- The remaining VP8 debt is not safely fixed by removing this duplicate block;
  next VP8 work must generate concrete first-partition byte patterns that drive
  the missing private states instead.

## Batch 15 plan

Goal: attack the single remaining branch miss in `lossless.rs` by removing
array-specific bit-reader monomorphizations from coverage-only probes.

Reverse mapping from snapshot `48ce7feb-ce1d-406d-9c46-4d76e0e1433f`:

- `src/codecs/webp/native/lossless.rs` has full line/function coverage, one
  missing branch, and 39 missing regions.
- The aggregate branch miss is reported at `BitReader::read_bits()` line 1320:
  `if self.nbits < num`.
- Raw LLVM records show several coverage-only `BitReader<Cursor<[u8; N]>>`
  instantiations with one-sided `fill()`/`consume()`/`read_bits()` branches.
- Public decode paths and most retained private probes use `Cursor<Vec<u8>>` or
  bounded `Take<Cursor<...>>`; fixed-size array cursors are not production-like
  for this crate.

Planned edit:

- Convert the remaining direct `BitReader::__coverage_new(Cursor::new([..]))`
  probes to `Cursor::new(vec![..])`.
- Do not add new direct `read_bits()` probes in this batch; prior Batch 12
  showed direct `Take<Cursor<Vec<u8>>>` `read_bits()` probes did not close the
  branch.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must drop below 1, or missing regions must improve below
  371.
- If aggregate debt does not improve, revert the conversions and keep the
  branch classified as real VP8L bitstream-pattern debt.

Batch 15 measurement:

- Local checks passed after correcting the `get_copy_distance()` generic type:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `1dfb44ac-a17f-4c49-8ad9-19a3dae3bd0b`.
- Coverage MCP snapshot: `5f42382c-2649-48bc-996c-c78946c31d28`.
- Result: `5 passed / 0 failed`.
- Lines: `26588 / 26588` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42396 / 42767` (371 missing), unchanged.

Rejection:

- Reverted the array-cursor-to-Vec conversions. Normalizing the remaining
  direct `BitReader` probes did not change aggregate branch or region debt.
- The final lossless branch is not caused solely by fixed-array bit-reader
  probes. Treat it as VP8L bitstream-pattern debt unless a later raw mapping
  identifies a removable generic instantiation.

## Batch 16 plan

Goal: reduce TIFF region debt through local invariant cleanups, not new
fixtures.

Reverse mapping from snapshot `48ce7feb-ce1d-406d-9c46-4d76e0e1433f`:

- `src/codecs/tiff/decode.rs` has full line/branch/function coverage and 30
  missing regions.
- Raw zero regions are concentrated in:
  - `decode()` checked arithmetic around strip/tile sizes;
  - `decode_packbits()` checked arithmetic after loop guards;
  - `Directory::parse()` checked arithmetic after entry-count and slice guards;
  - `MsbBits::read()` per-bit access after a full-range precheck.

Planned edit:

- `decode_packbits()`: replace `position.checked_add(count)?` and
  `expected.checked_sub(output.len())?` where the loop guard and PackBits
  header range prove the arithmetic.
- `decode()`: replace strip `row_bytes.checked_mul(strip_rows)?`; the earlier
  `expected_total = row_bytes.checked_mul(height_usize)?` proves all per-strip
  row products are bounded.
- `Directory::parse()`: replace `index.checked_mul(12)?`; `count <= 4096`
  proves this multiplication is bounded. Replace `start.checked_add(8)?` for
  inline values because `data.get(start..start + 12)` has already succeeded.
- `MsbBits::read()`: after the full bit-range precheck succeeds, read the bit
  directly instead of calling `data_bit(...)?` for each bit.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 371.
- If the cleanup does not reduce aggregate regions, revert it.

Batch 16 measurement:

- Local checks passed after marking `data_bit()` as coverage-only:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `7d9efbf0-de0a-49c0-a6aa-3d3f20ff8146`.
- Coverage MCP snapshot: `5900ee15-2055-4fa5-9488-84859b691029`.
- Result: `5 passed / 0 failed`.
- Lines: `26592 / 26592` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1611 / 1611` (0 missing), unchanged missing count.
- Regions: `42388 / 42754` (366 missing), improved by 5.
- `src/codecs/tiff/decode.rs`: improved from `1868 / 1898`
  (30 missing) to `1860 / 1885` (25 missing).

Retention:

- Retain Batch 16. The edits remove impossible checked-arithmetic/data-access
  states after explicit local guards; no fixture behavior changed.

## Batch 17 plan

Goal: continue the TIFF invariant cleanup from the new Batch 16 snapshot.

Reverse mapping from snapshot `5900ee15-2055-4fa5-9488-84859b691029`:

- `src/codecs/tiff/decode.rs` has 25 missing regions remaining.
- Raw zero regions now include:
  - one coverage-only `data_bit()` region left behind after `MsbBits::read()`
    moved to prechecked direct bit access;
  - one `decode_packbits()` region at the literal header load after
    `position < data.len()`;
  - several `Directory::parse()` regions around repeated offset arithmetic
    after the entry count and entry-slice guards.

Planned edit:

- Remove `data_bit()` and its coverage-only invalid-index probes; production
  now uses `data_bit_unchecked()` only after the full bit-range check.
- In `decode_packbits()`, replace `*data.get(position)?` with direct indexing
  under the loop guard.
- In `Directory::parse()`, compute `entries_start = offset + 2` after
  `data.get(offset..offset + 2)` succeeds, and use direct bounded arithmetic
  for per-entry starts and the fixed 12-byte entry range.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 366.
- If aggregate debt does not improve, revert Batch 17.

Batch 17 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `f4186196-d457-442c-a751-464b7e24a35d`.
- Coverage MCP snapshot: `ff20653c-b885-4ab8-a7d6-93cc8f1c1e4e`.
- Result: `5 passed / 0 failed`.
- Lines: `26588 / 26588` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42375 / 42736` (361 missing), improved by 5.
- `src/codecs/tiff/decode.rs`: improved from `1860 / 1885`
  (25 missing) to `1847 / 1867` (20 missing).

Retention:

- Retain Batch 17. Removed coverage-only `data_bit()` debt and more impossible
  checked/data access states under local guards.

## Batch 18 plan

Goal: remove a tiny zlib level-one region bucket using already-proved indices.

Reverse mapping from snapshot `ff20653c-b885-4ab8-a7d6-93cc8f1c1e4e`:

- `src/codecs/compression/zlib_ng.rs` remains the largest bucket.
- The smallest local bucket is `level1_window_tail_distance_one()` line 144
  where `data.get(position - 1)?` and `data.get(position)?` remain after the
  Batch 11 guard cleanup.
- `tokenize_level1_position()` also has a one-region literal path at
  `data.get(*position)?`.

Planned edit:

- In `level1_window_tail_distance_one()`, use direct indexing after:
  - `position >= WINDOW_SIZE * 2 - MIN_LOOKAHEAD + 1`, proving
    `position > 0`;
  - the production caller passes `lookahead = available - position`, proving
    `position < available <= data.len()`;
  - coverage hook calls either return before indexing or pass valid positions.
- In `tokenize_level1_position()`, use direct indexing for the fallback
  literal after `available.checked_sub(*position)?` has succeeded and all
  direct overflow probes exit before the literal path.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 361.
- If aggregate debt does not improve, revert Batch 18.

Batch 18 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `f20d1c56-01f2-4328-b098-7d6a4031fe9a`.
- Coverage MCP snapshot: `f63ab28f-279c-4ce5-8d4d-79bb89a12f90`.
- Result: `5 passed / 0 failed`.
- Lines: `26588 / 26588` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42368 / 42726` (358 missing), improved by 3.
- `src/codecs/compression/zlib_ng.rs`: improved from `3477 / 3665`
  (188 missing) at the retained pre-TIFF map to `3470 / 3655`
  (185 missing).

Retention:

- Retain Batch 18. The direct indexing is under local guards already exercised
  by the existing coverage hook, and aggregate regions improved.

## Batch 19 plan

Goal: reduce JPEG encoder region debt in `baseline_frequencies()` by removing
impossible checked counter/index arithmetic from encoder-controlled state.

Reverse mapping from snapshot `f63ab28f-279c-4ce5-8d4d-79bb89a12f90`:

- `src/codecs/jpeg/encode/mod.rs` has full line/branch/function coverage and
  54 missing regions.
- The largest JPEG bucket is `baseline_frequencies()` with 15 missing regions.
- The zero regions are mostly:
  - MCU dimension arithmetic from validated sampling factors;
  - per-component block row/column arithmetic bounded by MCU loops;
  - frequency counter increments and AC zero-run increments that cannot reach
    `usize`/`u64` overflow for an in-memory image.

Planned edit:

- Replace checked MCU/block arithmetic with direct arithmetic inside
  `baseline_frequencies()`.
- Replace frequency-counter `checked_add(1)?` and AC `run.checked_add(1)?`
  with direct increments.
- Do not touch progressive event helpers in this batch.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 358.
- If aggregate debt does not improve, revert Batch 19.

Batch 19 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `17fb9dff-40fe-4f5f-9867-1734170d87d7`.
- Coverage MCP snapshot: `e0a08407-412d-429b-ae3f-986a309255f6`.
- Result: `5 passed / 0 failed`.
- Lines: `26585 / 26585` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42340 / 42683` (343 missing), improved by 15.
- `src/codecs/jpeg/encode/mod.rs`: improved from `1444 / 1498`
  (54 missing) to `1416 / 1455` (39 missing).

Retention:

- Retain Batch 19. The removed checks were on encoder-controlled MCU and
  frequency-counter state; the baseline optimized-Huffman behavior remains the
  same.

## Batch 20 plan

Goal: reduce JPEG progressive-event region debt by applying the same
encoder-controlled-state cleanup pattern as Batch 19.

Reverse mapping from snapshot `e0a08407-412d-429b-ae3f-986a309255f6`:

- JPEG encode has 39 missing regions remaining.
- Remaining raw buckets are in `encode()`, `encode_progressive_scans_exact()`,
  `dc_progressive_events()`, `ac_progressive_events()`,
  `append_ac_first_events()`, `append_ac_refine_events()`, and
  `flush_progressive_eob()`.
- Most of the event-helper gaps are impossible checked arithmetic/casts:
  frequency increments, MCU index arithmetic, AC zero-run increments,
  EOB-run increments, and EOB width-to-symbol casts.

Planned edit:

- Use direct counter increments in progressive scan frequency gathering and AC
  event helpers.
- Use direct MCU/block arithmetic in `dc_progressive_events()`.
- Replace `coefficients.len().checked_sub(1)?` with `coefficients.len() - 1`
  because a progressive AC scan range is non-empty.
- Replace `flush_progressive_eob()` checked width conversions with direct casts;
  `eob_run` flushes at the JPEG progressive limit, so `ilog2(eob_run) <= 14`.
- Do not change public option parsing or marker writing in this batch.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 343.
- If aggregate debt does not improve, revert Batch 20.

Batch 20 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `0319f7cd-9795-4135-9f0b-91c3ac164fca`.
- Coverage MCP snapshot: `0f6caa68-2044-4dc5-b2cf-4606cd926118`.
- Result: `5 passed / 0 failed`.
- Lines: `26577 / 26577` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42308 / 42633` (325 missing), improved by 18.
- `src/codecs/jpeg/encode/mod.rs`: improved from `1416 / 1455`
  (39 missing) to `1384 / 1405` (21 missing).

Retention:

- Retain Batch 20. The edits remove impossible progressive-event arithmetic and
  cast failures for encoder-controlled state.

## Batch 21 plan

Goal: remove remaining JPEG progressive helper `Option` regions after Batch 20
made their internals infallible.

Reverse mapping from snapshot `0f6caa68-2044-4dc5-b2cf-4606cd926118`:

- JPEG encode has 21 missing regions remaining.
- Several remaining regions are `?`/`expect` sites around
  `flush_progressive_eob()`, `append_ac_first_events()`, and
  `append_ac_refine_events()`.
- After Batch 20, `flush_progressive_eob()` has no fallible operation left:
  it only emits events, clears `eob_run`, and drains correction bits.
- The AC append helpers are also encoder-controlled and no longer need to
  propagate failure from `flush_progressive_eob()`.

Planned edit:

- Change `flush_progressive_eob()` to return `()`.
- Change `append_ac_first_events()` and `append_ac_refine_events()` to return
  `()`.
- Remove the corresponding `?` and `.expect(...)` calls.
- Leave `ac_progressive_events()` as `Option<Vec<_>>` because it still validates
  the scan component list.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 325.
- If aggregate debt does not improve, revert Batch 21.

Batch 21 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `ee32db2c-6fe9-4261-97de-1d01108fa44e`.
- Coverage MCP snapshot: `c7dd53fe-55cc-4f8e-a342-4ebbc880a61c`.
- Result: `5 passed / 0 failed`.
- Lines: `26571 / 26571` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42295 / 42616` (321 missing), improved by 4.
- `src/codecs/jpeg/encode/mod.rs`: improved from `1384 / 1405`
  (21 missing) to `1371 / 1388` (17 missing).

Retention:

- Retain Batch 21. The AC progressive append helpers are now correctly
  infallible internal event builders, matching their encoder-controlled inputs.

## Batch 22 plan

Goal: reduce remaining JPEG top-level `encode()` regions with public option
states plus safe sampling arithmetic cleanup.

Reverse mapping from snapshot `c7dd53fe-55cc-4f8e-a342-4ebbc880a61c`:

- JPEG encode has 17 missing regions remaining.
- The largest remaining JPEG bucket is top-level `encode()` with 11 regions.
- Zero regions include:
  - chroma-width and MCU-column checked arithmetic for fixed sampling factors;
  - restart interval conversion to the JPEG u16 field;
  - EXIF hex parsing/writing error propagation;
  - progressive scan exact helper propagation.

Planned edit:

- Replace checked chroma-width and MCU-column arithmetic with direct arithmetic;
  dimensions and sampling factors are encoder-controlled and already used to
  allocate coefficient buffers.
- Add coverage-hook calls for two public option states:
  - invalid `extra["exif_hex"]`, which should make JPEG encode return `None`;
  - oversized `extra["restart_interval"]`, which should fail u16 conversion.
- Do not change the progressive helper call in this batch.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 321.
- If aggregate debt does not improve, revert Batch 22.

Batch 22 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `204bc908-0f47-410a-aa53-5b4453ebc7b6`.
- Coverage MCP snapshot: `14e5db93-4b6a-469a-a54a-5cebc345f6ca`.
- Result: `5 passed / 0 failed`.
- Lines: `26574 / 26574` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42313 / 42629` (316 missing), improved by 5.
- `src/codecs/jpeg/encode/mod.rs`: improved from `1371 / 1388`
  (17 missing) to `1389 / 1401` (12 missing).

Retention:

- Retain Batch 22. The new option-state probes are public encoder option
  states, and the arithmetic cleanup is on fixed sampling-grid state.

## Batch 23 plan

Goal: remove remaining JPEG private progressive-helper region debt after the
previous batches made the event path encoder-controlled and internally
infallible.

Reverse mapping from snapshot `14e5db93-4b6a-469a-a54a-5cebc345f6ca`:

- `src/codecs/jpeg/encode/mod.rs` has 12 missing regions, 0 missing branches,
  0 missing lines, and 0 missing functions.
- Exact zero-region loci:
  - top-level public/error propagation: lines `168`, `178`, `181`, `187`,
    `218`, and `308`;
  - private hex/parser helper: line `458`;
  - private progressive helper invariant sites: lines `874`, `885`, `916`
    twice, and `1021`.

Planned edit:

- Change `baseline_frequencies()` to return the frequency tables directly. Its
  previous `Option` wrapper no longer has a `None` path after Batches 19 and 20
  removed the checked arithmetic.
- Change `huffman::optimal_table()` to return `OptimalTable` directly. The
  implementation always appends a sentinel frequency and always returns
  `Some(...)`; keeping an `Option` creates dead propagation regions at every
  optimized-Huffman call site.
- Change `encode_progressive_scans_exact()` and its private event builders to
  return infallible values. The default progressive scan script and component
  tables are created by this encoder, not user input.
- Replace `tables.get(table)?.as_ref()?` with direct indexing plus an invariant
  `expect`, because `ProgressiveEvent::Symbol` is emitted only for tables whose
  frequency was collected and whose DHT table was built in the same function.
- Replace `scan.comps.first()?` with direct indexing in `ac_progressive_events()`;
  all default AC scans contain exactly one component.
- Replace `decode_hex()`'s `index.checked_add(2)?` with `index + 2`; the
  iterator bounds `index < value.len()` and `step_by(2)` make overflow
  impossible for an in-memory string.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 316.
- If aggregate debt does not improve, revert Batch 23.

Batch 23 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
  - `git diff --check`
- Coverage MCP run: `c9f9be4c-7156-415a-8e5b-260e69774668`.
- Coverage MCP snapshot: `73df3836-f545-4df9-a5a9-74c0c9d89dc8`.
- Result: `5 passed / 0 failed`.
- Lines: `26568 / 26568` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42298 / 42604` (306 missing), improved by 10.
- `src/codecs/jpeg/encode/mod.rs`: improved from `1389 / 1401`
  (12 missing) to `1376 / 1378` (2 missing).

Retention:

- Retain Batch 23. `baseline_frequencies()`, `optimal_table()`, and the
  private progressive event path now express their actual infallible contract.

## Batch 24 plan

Goal: close the final two JPEG encode regions with public option-state probes,
without changing production behavior.

Reverse mapping from snapshot `73df3836-f545-4df9-a5a9-74c0c9d89dc8`:

- `src/codecs/jpeg/encode/mod.rs` has 2 missing regions:
  - line `168`: `restart_rows.checked_mul(mcu_columns)?`;
  - line `214`: `marker::write_exif_app1(&mut out, &exif)?`.

Planned edit:

- Add a coverage-hook encode call with `restart_interval = usize::MAX` and a
  width that creates more than one MCU column, forcing the checked multiplication
  overflow path.
- Add a coverage-hook encode call with an oversized but valid hex EXIF payload,
  forcing `write_exif_app1()` to reject the APP1 length through the public
  `encode()` option path.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 306.
- If aggregate debt does not improve, revert Batch 24.

Batch 24 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
  - `git diff --check`
- Coverage MCP run: `7cf945a5-e462-4f09-88d0-f84a6e81ee78`.
- Coverage MCP snapshot: `df9801a9-f7eb-41e2-88b2-124b587a08d7`.
- Result: `5 passed / 0 failed`.
- Lines: `26579 / 26579` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42327 / 42631` (304 missing), improved by 2.
- `src/codecs/jpeg/encode/mod.rs`: improved from `1376 / 1378`
  (2 missing) to `1405 / 1405` (0 missing).

Retention:

- Retain Batch 24. JPEG encode is now at 100% line, branch, function, and
  region coverage. The two probes are public option failure states and live
  under `#[cfg(coverage)]`.

## Current retained gap map after Batch 24

| File | Missing regions | Missing branches | Current read |
| --- | ---: | ---: | --- |
| `src/codecs/compression/zlib_ng.rs` | 185 | 0 | Largest remaining region bucket; mostly closed-branch DEFLATE helper arithmetic and tree/matcher state. |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 | WebP container/animation generic-region artifacts and private parser states. |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 | Only aggregate branch miss left; raw MCP line view shows many monomorphization partials, so reverse-map by helper shape before adding probes. |
| `src/codecs/tiff/decode.rs` | 20 | 0 | Closed-branch TIFF parser arithmetic and malformed tag/value states. |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 | Closed-branch VP8 private parser/decoder region artifacts. |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 | Single aggregate/expansion region; no zero source segment in the LLVM JSON. |

## Batch 25 plan

Goal: reduce the remaining lossless branch/region debt without creating new
generic decoder monomorphizations.

Reverse mapping from snapshot `df9801a9-f7eb-41e2-88b2-124b587a08d7`:

- `src/codecs/webp/native/lossless.rs` has 39 missing regions and the only
  aggregate branch miss in the project.
- MCP file view reports many raw partial branch lines because LLVM records each
  generic instantiation separately. This means line numbers alone are not a safe
  target; the fix should prefer private helper cleanup or probes in the same
  reader shape already used by production decoding.

Planned investigation/edit:

- Inspect the remaining zero-region clusters around lines `206-271`,
  `315-389`, `421-474`, `500-655`, and `1277-1320`.
- Look for helper functions that still return `Option`/`Result` for
  encoder-controlled or bitstream-prevalidated states, or coverage-only readers
  that instantiate broad `LosslessDecoder<R>` methods.
- Apply only a narrow lossless cleanup/probe batch that improves aggregate
  regions or the single branch.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must improve to 0 or missing regions must improve below 304.
- If aggregate debt does not improve, revert Batch 25.

Batch 25 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
  - `git diff --check`
- Coverage MCP run: `21c5e34d-a60c-4e94-b395-03ec9899cee5`.
- Coverage MCP snapshot: `f2c73a83-2581-4a53-bbae-7914b1c4e30f`.
- Result: `5 passed / 0 failed`.
- Lines: `26598 / 26598` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1611 / 1611` (0 missing), unchanged.
- Regions: `42353 / 42657` (304 missing), unchanged missing count.
- `src/codecs/webp/native/lossless.rs`: remained at 39 missing regions and
  1 missing branch.

Retention:

- Revert Batch 25. The direct `LosslessDecoder<Cursor<Vec<u8>>>` probes covered
  their own new coverage-hook regions but did not close any retained missing
  region or the aggregate branch miss.

## Batch 26 plan

Goal: reduce TIFF closed-branch region debt by normalizing geometry arithmetic
that is impossible to overflow on this 64-bit coverage host but still needs a
portable 32-bit failure path.

Reverse mapping from snapshot `df9801a9-f7eb-41e2-88b2-124b587a08d7`:

- `src/codecs/tiff/decode.rs` has 20 missing regions and 0 missing branches.
- Remaining zero-region loci include:
  - row/tile geometry checked arithmetic at lines `60-64`, `87`, `90-92`,
    `132-133`;
  - `unpack_indices()` checked stride/capacity at lines `354-355`;
  - `MsbBits::read()` defensive bit bound at line `558`;
  - `Directory::parse()` value length/range checks at lines `649` and `655`.

Planned edit:

- Compute TIFF geometry products in `u64`, then convert once through
  `usize::try_from(...).ok()?`. This preserves defensive failure on 32-bit
  targets and for values that exceed `usize`, while removing multiple
  impossible checked-`usize` regions on the 64-bit coverage host.
- Leave public malformed-input checks and real `data.get(...)?` bounds intact.
- Do not touch `MsbBits::read()` in this batch because its overflow guard is
  tied to arbitrary slice length, not TIFF field-width geometry.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 304.
- If aggregate debt does not improve, revert Batch 26.

Batch 26 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
  - `git diff --check`
- Coverage MCP run: `3bf84bde-72d4-4dae-b9b0-b6727a58b61e`.
- Coverage MCP snapshot: `10f2fb65-9374-4019-8e55-20eee3754bd7`.
- Result: `5 passed / 0 failed`.
- Lines: `26584 / 26584` (0 missing), unchanged.
- Branches: `3465 / 3466` (1 missing), unchanged.
- Functions: `1610 / 1610` (0 missing), unchanged.
- Regions: `42340 / 42645` (305 missing), regressed by 1.
- `src/codecs/tiff/decode.rs`: regressed from 20 missing regions to 21.

Retention:

- Revert Batch 26. The `u64` normalization preserved behavior, but LLVM region
  accounting added more region debt than it removed.

## Final retained validation for this sweep

- Coverage MCP run: `d03278a2-576f-42fb-9014-a8743389ff21`.
- Coverage MCP snapshot: `27b758c5-b9ea-4c3f-8266-a5b6bcfc31de`.
- Result: `5 passed / 0 failed`.
- Lines: `26579 / 26579` (0 missing).
- Branches: `3465 / 3466` (1 missing).
- Functions: `1610 / 1610` (0 missing).
- Regions: `42327 / 42631` (304 missing).

Retained impact in this sweep:

- JPEG encode is now 100% line, branch, function, and region coverage.
- Overall missing regions improved from 316 at snapshot
  `14e5db93-4b6a-469a-a54a-5cebc345f6ca` to 304 at snapshot
  `27b758c5-b9ea-4c3f-8266-a5b6bcfc31de`.
- Batch 25 and Batch 26 were intentionally reverted because they did not
  improve the retained aggregate metrics.

Remaining retained gap map:

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | 185 | 0 |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/tiff/decode.rs` | 20 | 0 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |

## Batch 27 all-file region inventory and plan

Goal: start a new bulk sweep from retained snapshot
`27b758c5-b9ea-4c3f-8266-a5b6bcfc31de`, document exact uncovered region loci
across every remaining file, then fix the smallest defensible cluster before
running Coverage MCP again.

Current retained snapshot:

- Lines: `26579 / 26579` (0 missing).
- Branches: `3465 / 3466` (1 missing).
- Functions: `1610 / 1610` (0 missing).
- Regions: `42327 / 42631` (304 missing).

Exact remaining file map:

| File | Missing regions | Missing branches | Read |
| --- | ---: | ---: | --- |
| `src/codecs/compression/zlib_ng.rs` | 185 | 0 | Mostly `Option`/checked-arithmetic regions inside internal DEFLATE tokenizers, matchers, tree builders, and emitters. Many are unreachable after `compress_zlib_chunked()` validates input chunk totals. |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 | WebP RIFF/VP8X/ANMF read/seek error states and animation-frame parser paths. These are mostly public malformed-container states, but several require precise chunk cursor placement. |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 | VP8L transform/Huffman bitstream states. The remaining aggregate branch is still reported through generic `BitReader`/`LosslessDecoder` instantiations, so line-only probes are unsafe. |
| `src/codecs/tiff/decode.rs` | 20 | 0 | TIFF tag-value, tile/strip geometry, bit-unpack, and directory-offset regions. Several malformed fixtures are already wired; the rest are mostly checked arithmetic or out-of-range value payloads. |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 | VP8 parser/decoder `?` propagation regions around partition init, prediction-mode errors, coefficient reads, and macroblock decode. |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 | One writer-generic aggregate region at `write_chunk` line `1129`; no zero-hit source segment in the LLVM JSON. |

Exact zero-source loci from the retained LLVM JSON:

- `zlib_ng.rs`: `52`, `54`, `55`, `79`, `81`, `82`, `88`, `102`,
  `103`, `106`, `993`, `1007`, `1018`, `1029`, `1040`, `1051`, `1085`,
  `1110`, `1113`, `1175`, `1178`, `1193`, `1194`, `1197`, `1203`,
  `1208`, `1224`, `1238`, `1282`, `1285`, `1305`, `1309`, `1312`,
  `1360`, `1369`, `1401`, `1404`, `1407`, `1409`, `1410`, `1425`,
  `1428`, `1446`, `1448`, `1470`, `1484`, `1491`, `1523`, `1542`,
  `1553`, `1557`, `1560`, `1597`, `1598`, `1619`, `1622`, `1637`,
  `1638`, `1641`, `1647`, `1652`, `1668`, `1680`, `1696`, `1697`,
  `1699`, `1702`, `1707`, `1713`, `1714`, `1715`, `1724`, `1725`,
  `1728`, `1729`, `1731`, `1742`, `1743`, `1744`, `1745`, `1746`,
  `1747`, `1753`, `1758`, `1762`, `1783`, `1785`, `1796`, `1798`,
  `1800`, `1855`, `1858`, `1908`, `1918`, `1920`, `1942`, `1965`,
  `1966`, `2015`, `2017`, `2030`, `2033`, `2036`, `2037`, `2103`,
  `2118`, `2127`, `2129`, `2135`, `2139`, `2142`, `2148`, `2149`,
  `2154`, `2158`, `2165`, `2169`, `2171`, `2177`, `2181`, `2182`,
  `2183`, `2191`, `2199`, `2200`, `2202`, `2207`, `2259`, `2279`,
  `2318`, `2326`, `2330`, `2332`, `2340`, `2348`, `2349`, `2350`,
  `2353`, `2354`, `2355`, `2374`, `2378`, `2379`, `2380`, `2392`,
  `2399`, `2401`, `2429`, `2462`, `2463`, `2464`, `2468`, `2497`,
  `2501`, `2504`, `2505`, `2507`, `2508`, `2510`, `2511`, `2540`,
  `2546`, `2564`, `2571`.
- `webp/native/decoder.rs`: `169`, `177`, `181`, `186`, `203`, `204`,
  `216`, `221`, `240`, `253`, `254`, `255`, `256`, `273`, `276`,
  `299`, `303`, `305`, `308`, `315`, `328`, `331`, `421`, `431`,
  `434`, `446`, `447`, `459`, `462`, `512`, `515`, `523`, `524`,
  `525`, `526`, `527`, `528`, `539`, `573`, `579`, `580`, `1017`.
- `webp/native/lossless.rs`: `207`, `220`, `229`, `239`, `248`, `262`,
  `265`, `315`, `317`, `324`, `377`, `380`, `382`, `383`, `392`,
  `400`, `403`, `421`, `422`, `423`, `442`, `443`, `467`, `501`, `555`.
- `tiff/decode.rs`: `34`, `60`, `61`, `62`, `64`, `87`, `90`, `91`,
  `92`, `105`, `106`, `132`, `133`, `192`, `202`, `354`, `355`, `558`,
  `649`, `655`.
- `webp/native/vp8.rs`: `999`, `1193`, `1200`, `1202`, `1220`, `1243`,
  `1257`, `1275`, `1491`, `1546`, `1859`.
- `webp/native/encoder.rs`: no zero-source segment; one aggregate writer
  region remains at the `write_chunk` generic instantiation.

Batch 27 planned edit:

1. Start with the smallest zlib cluster: level-one tokenization lines `52-106`
   and `993`.
2. Keep `compress_level1()` returning `None` when internal chunk totals are
   invalid, matching the public `compress_zlib_chunked()` contract.
3. Make only the validated level-one tokenizer path infallible:
   - `tokenize_level1()` returns `(Vec<Token>, Vec<Token>)`;
   - `tokenize_level1_position()` returns `()`;
   - `quick_insert_level1()` indexes the fixed-size hash table directly after
     computing a bounded hash;
   - slice comparisons use already-proven bounds from `lookahead`.
4. Remove coverage-hook calls whose only purpose was to exercise invalid
   private level-one inputs that production cannot send after chunk validation.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must not increase above 1.
- Missing regions must improve below 304.
- If the batch does not improve retained aggregate coverage, revert the code
  edits and keep this inventory for the next target.

Batch 27 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
  - `git diff --check`
- First Coverage MCP run: `14800c5f-a6e8-4699-a930-1157b2143bc2`.
- First snapshot: `d18495d3-bb47-41fe-b957-1e77f7d854b3`.
- First result reduced regions but introduced one uncovered line and one extra
  branch at the newly explicit `input_len != data.len()` guard.
- Added a narrow coverage-hook call for the real invalid-chunk-total guard.
- Corrected Coverage MCP run: `10a2b6a4-7292-4c3e-b4be-06e5847ecb5d`.
- Corrected snapshot: `6ce316b2-4191-48ef-973b-b8c597b7caa3`.
- Result: `5 passed / 0 failed`.
- Lines: `26573 / 26573` (0 missing), retained.
- Branches: `3467 / 3468` (1 missing), retained.
- Functions: `1610 / 1610` (0 missing), retained.
- Regions: `42283 / 42575` (292 missing), improved by 12.
- `src/codecs/compression/zlib_ng.rs`: improved from 185 missing regions to
  173 and stayed at 100% lines/branches/functions.

Retention:

- Retain Batch 27. The level-one zlib tokenizer is now infallible after the
  chunk-total validation gate, and the public invalid-chunk-total behavior is
  still covered.

## Batch 28 plan

Goal: reduce the smallest TIFF region-only cluster by removing checked
arithmetic that classic TIFF field widths prove cannot overflow on the 64-bit
coverage host, while preserving checked behavior on 32-bit targets.

Reverse mapping from snapshot `6ce316b2-4191-48ef-973b-b8c597b7caa3`:

- `src/codecs/tiff/decode.rs` still has 20 missing regions and 0 missing
  branches.
- Safe 64-bit-only cleanup targets:
  - `Directory::parse()` line `649`: `value_count * type_size`; classic TIFF
    count is `u32` and `type_size <= 8`, so the product fits 64-bit `usize`.
  - `Directory::parse()` line `655`: `value_position + byte_len`; both are
    derived from classic TIFF 32-bit offsets/counts, so the sum fits 64-bit
    `usize`.
  - `unpack_indices()` lines `354-355`: `width * bits` and `width * height`;
    both use `u32` image dimensions and `bits in {1,2,4}`, so they fit 64-bit
    `usize`.
  - Tile-count line `87` and declared strip/tile range additions at `105` and
    `192` are also candidates, but this batch will keep scope to the directory
    and low-bit unpack helpers first.

Planned edit:

- Add tiny `#[cfg(target_pointer_width = "32")]` / `#[cfg(not(...))]` helper
  functions for the proven 64-bit operations instead of broad direct
  arithmetic.
- Use the helpers only at the TIFF loci above.
- Do not alter row/tile byte-size products where `u32 * u32 * bits` can exceed
  64-bit.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must stay at 1.
- Missing regions must improve below 292.
- If aggregate debt does not improve, revert Batch 28.

Batch 28 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
  - `git diff --check`
- Coverage MCP run: `735596c6-fa03-4a11-9e40-c9e5f7457e2e`.
- Coverage MCP snapshot: `ef930ff8-ea3b-44f0-a43b-aee50395322f`.
- Result: `5 passed / 0 failed`.
- Lines: `26574 / 26574` (0 missing), retained.
- Branches: `3467 / 3468` (1 missing), retained.
- Functions: `1610 / 1610` (0 missing), retained.
- Regions: `42277 / 42565` (288 missing), improved by 4.
- `src/codecs/tiff/decode.rs`: improved from 20 missing regions to 16.

Retention:

- Retain Batch 28. The 64-bit direct arithmetic removed the directory byte
  length/range and low-bit output-capacity regions while keeping 32-bit checked
  behavior compiled for 32-bit targets.

## Batch 29 plan

Goal: continue the TIFF 64-bit-proven cleanup, but only for operations whose
inputs are still bounded by classic TIFF `u32` fields.

Reverse mapping from snapshot `ef930ff8-ea3b-44f0-a43b-aee50395322f`:

- `src/codecs/tiff/decode.rs` has 16 missing regions and 0 missing branches.
- Remaining zero loci:
  - `34`: missing/malformed `BitsPerSample` value lookup.
  - `60-64`: row-byte and total-buffer checked arithmetic; not safe to remove
    wholesale because `width * samples_per_pixel * bits` can exceed 64-bit.
  - `87`: `tiles_across * tiles_down`; safe on 64-bit because both are derived
    from `u32` dimensions.
  - `90`: `samples_per_pixel * bytes_per_sample`; safe on 64-bit because
    `samples_per_pixel` is a classic TIFF `u32` value and bytes-per-sample is
    at most 32 here.
  - `91-92`: tile row/tile byte products; not safe because `tile_width *
    samples_per_pixel * bits` can exceed 64-bit.
  - `105`, `192`: encoded byte range additions; keep checked for now because
    uncompressed/inferred byte counts can be derived from buffer-size math.
  - `106`: compressed decode-block failure path.
  - `132-133`: tile copy bounds; keep as a malformed-layout guard.
  - `202`: compressed predictor unsupported-bit-width condition.
  - `565`: `MsbBits::read()` bit-bound arithmetic; keep because arbitrary
    slice length can still overflow `len * 8`.

Planned edit:

- Use 64-bit direct arithmetic at line `87` and `90` only.
- Preserve the existing checked expressions on 32-bit targets.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must stay at 1.
- Missing regions must improve below 288.
- If the two-line cleanup is neutral or regresses, revert Batch 29.

Batch 29 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
  - `git diff --check`
- Coverage MCP run: `0c3b757a-9bb8-435d-810b-0c77c92c6559`.
- Coverage MCP snapshot: `93876e5e-c73a-428d-8f66-e72e5dd1803f`.
- Result: `5 passed / 0 failed`.
- Lines: `26575 / 26575` (0 missing), retained.
- Branches: `3467 / 3468` (1 missing), retained.
- Functions: `1610 / 1610` (0 missing), retained.
- Regions: `42275 / 42561` (286 missing), improved by 2.
- `src/codecs/tiff/decode.rs`: improved from 16 missing regions to 14.

Retention:

- Retain Batch 29. TIFF tile count and bytes-per-pixel arithmetic now uses
  direct arithmetic only on 64-bit targets where classic TIFF field sizes prove
  those operations cannot overflow.

## Current retained gap map after Batch 29

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | 173 | 0 |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/tiff/decode.rs` | 14 | 0 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |

## Batch 37 plan

Goal: continue reducing zlib post-validation region debt in `write_block()` and
tree scanning without touching malformed-token helper exits.

Reverse mapping from snapshot `38a02c5b-37bc-4562-8e73-7c4466f54225`:

- `src/codecs/compression/zlib_ng.rs` has 112 missing regions and 0 missing
  branches.
- The remaining post-validation zero-source loci include dynamic/static cost
  checked additions and conversions, `scan_tree()` repeat-count conversion, and
  `send_trees()` propagation from generated tree emission.
- The same pass still shows many matcher-internal defensive exits; this batch
  does not touch those.

Planned edit:

- Convert `write_block()` dynamic/static cost arithmetic to direct arithmetic
  after generated tree construction, because all values are bounded by the
  DEFLATE block sizes and tree-code limits already enforced by the tokenizer and
  tree builder.
- Convert `scan_tree()` repeat-count conversion to a direct cast because `count`
  is bounded by `max_count <= 138`.
- Convert `send_trees()` propagation from generated literal/distance tree
  emission to `expect(...)`; direct malformed `send_tree()` hook coverage stays
  in `send_tree()`.
- Do not change `frequencies()`, `emit_tokens()`, `emit_fixed_block()`,
  `send_code()`, or malformed `build_tree()` exits.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must stay at 1 or improve.
- Missing regions must improve below 225.
- If coverage fails or aggregate missing regions do not improve, revert Batch
  37.

Batch 32 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
  - `git diff --check`
- Coverage MCP run: `b419bcd2-2bba-4848-ad9d-5184d843615c`.
- Coverage MCP snapshot: `faad6806-f8d5-462d-bb63-829b61a42af2`.
- Result: `5 passed / 0 failed`.
- Lines: `26599 / 26599` (0 missing), retained.
- Branches: `3467 / 3468` (1 missing), retained.
- Functions: `1610 / 1610` (0 missing), retained.
- Regions: `42305 / 42575` (270 missing), improved by 8.
- `src/codecs/compression/zlib_ng.rs`: improved from 165 missing regions to
  157.

Retention:

- Retain Batch 32. Fresh level-six and level-nine tokenizer wrappers now keep
  chunk-total validation as the fallible boundary and treat lifecycle matcher
  calls as internal invariants.

## Current retained gap map after Batch 32

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | 157 | 0 |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/tiff/decode.rs` | 14 | 0 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |

## Batch 33 plan

Goal: continue the narrow zlib wrapper-invariant cleanup for the level-three
early matcher.

Reverse mapping from snapshot `faad6806-f8d5-462d-bb63-829b61a42af2`:

- `src/codecs/compression/zlib_ng.rs` has 157 missing regions and 0 missing
  branches.
- The next safe zero-source loci include `tokenize_early_matcher()` process
  propagation at lines `1868` and `1871`.
- The wrapper constructs a fresh `Level3Matcher` and validates only the input
  chunk total with `available.checked_add(chunk_length)?`. The remaining
  `matcher.process(...) ?` paths represent malformed private matcher state, not
  public image input bytes.

Planned edit:

- Keep chunk-total overflow fallible.
- Convert only the per-chunk and final `matcher.process(...)` calls in
  `tokenize_early_matcher()` to `expect(...)`.
- Leave `Level3Matcher::process()` and its helper methods fallible for direct
  malformed-state coverage hooks.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must stay at 1.
- Missing regions must improve below 270.
- If coverage fails or aggregate missing regions do not improve, revert Batch
  33.

Batch 33 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
  - `git diff --check`
- Coverage MCP run: `3e461ab3-ee4b-4ecb-9a75-9a68b6e4dc49`.
- Coverage MCP snapshot: `bbbbc617-3f9c-44e7-91f0-d3b6450a4116`.
- Result: `5 passed / 0 failed`.
- Lines: `26603 / 26603` (0 missing), retained.
- Branches: `3467 / 3468` (1 missing), retained.
- Functions: `1610 / 1610` (0 missing), retained.
- Regions: `42309 / 42577` (268 missing), improved by 2.
- `src/codecs/compression/zlib_ng.rs`: improved from 157 missing regions to
  155.

Retention:

- Retain Batch 33. The level-three wrapper now matches the level-six and
  level-nine wrapper invariant: input chunk-total validation remains fallible,
  while fresh matcher lifecycle processing is treated as internal state.

## Current retained gap map after Batch 33

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | 155 | 0 |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/tiff/decode.rs` | 14 | 0 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |

## Batch 34 plan

Goal: remove zlib matcher expression-region debt where earlier predicates
prove arithmetic safety, without weakening malformed private-state hooks that
still intentionally return `None`.

Reverse mapping from snapshot `bbbbc617-3f9c-44e7-91f0-d3b6450a4116`:

- `src/codecs/compression/zlib_ng.rs` has 155 missing regions and 0 missing
  branches.
- The next repeated zero-source loci are guarded matcher arithmetic in
  `SlowMatcher::process()`, `Level9Matcher::process()`, and
  `Level6Matcher::find_match()`:
  - candidate-distance checks after `candidate < position`;
  - match-emission insert-window arithmetic after `previous_length >= 3`;
  - loop-range endpoints bounded by `available - 3`.
- Existing direct malformed-state hooks still cover invalid distance, truncated
  windows, empty hash tables, and explicit helper failures. This batch must not
  remove those hooks.

Planned edit:

- Replace only arithmetic already proven by immediately preceding predicates:
  - `position.checked_sub(candidate)?` after `candidate < position`;
  - `position + lookahead - 3` with `available - 3`;
  - `previous_length.checked_sub(2)?` after `previous_length >= 3`;
  - insert-loop endpoints bounded by `available - 3`.
- Keep fallible reads, hash inserts, malformed-state distance checks, and
  helper return types unchanged.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must stay at 1.
- Missing regions must improve below 268.
- If coverage fails or aggregate missing regions do not improve, revert Batch
  34.

Batch 34 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
  - `git diff --check`
- Coverage MCP run: `0ce853a6-f08c-4efd-ae63-da8d436e2c7b`.
- Coverage MCP snapshot: `202fdbf1-d72b-4d19-9f1e-10423f8741c5`.
- Result: `5 passed / 0 failed`.
- Lines: `26595 / 26595` (0 missing), retained.
- Branches: `3467 / 3468` (1 missing), retained.
- Functions: `1610 / 1610` (0 missing), retained.
- Regions: `42283 / 42534` (251 missing), improved by 17.
- `src/codecs/compression/zlib_ng.rs`: improved from 155 missing regions to
  138.

Retention:

- Retain Batch 34. The direct arithmetic was limited to values already guarded
  by matcher predicates or by the current `available` window.

## Current retained gap map after Batch 34

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | 138 | 0 |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/tiff/decode.rs` | 14 | 0 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |

Branch-gap note:

- `src/codecs/webp/native/lossless.rs` still reports `113 / 114` branches.
  Coverage MCP localizes many partial lines because LLVM emits duplicated
  branch records for generic instantiations. Aggregating by source line/column
  shows both sides covered for every unique source branch, so the remaining
  branch is currently treated as per-instantiation/generic debt until a more
  precise source-level reverse map identifies a public fixture or hook input.

## Batch 35 plan

Goal: reduce zlib tree-bookkeeping region debt without repeating the rejected
Batch 30 broad rewrite.

Reverse mapping from snapshot `202fdbf1-d72b-4d19-9f1e-10423f8741c5`:

- `src/codecs/compression/zlib_ng.rs` has 138 missing regions and 0 missing
  branches.
- Remaining zlib zero-source loci include `build_tree()` structural arithmetic
  around singleton fallback, heap-tail movement, node-depth increment,
  `next_node` increment, canonical-code length conversion, and `send_trees()`
  header field conversions.
- Rejected Batch 30 failed because it removed fallibility from malformed
  `static_lengths.get(index)?` and overflow-repair bit-count arithmetic that
  existing coverage hooks intentionally exercise. Those points stay fallible.

Planned edit:

- Convert only structural arithmetic bounded by DEFLATE constants and local
  loop invariants:
  - singleton fallback `current_max + 1` for `0..=1`;
  - heap-tail `heap_max -= 1`;
  - node `depth + 1`;
  - `next_node += 1`;
  - parent length `+ 1`;
  - canonical-code length cast in `generate_codes()`;
  - `send_trees()` header casts for bounded code counts.
- Do not change:
  - frequency addition;
  - `static_lengths.get(index)?`;
  - bit-count overflow repair;
  - malformed token/spec hook behavior.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must stay at 1.
- Missing regions must improve below 251.
- If coverage fails or aggregate missing regions do not improve, revert Batch
  35.

Batch 35 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
  - `git diff --check`
- Coverage MCP run: `03056eae-b68e-48aa-85fa-097f8f292064`.
- Coverage MCP snapshot: `ace9c214-ae44-4f13-8ec2-5853a648607e`.
- Result: `5 passed / 0 failed`.
- Lines: `26594 / 26594` (0 missing), retained.
- Branches: `3467 / 3468` (1 missing), retained.
- Functions: `1610 / 1610` (0 missing), retained.
- Regions: `42261 / 42498` (237 missing), improved by 14.
- `src/codecs/compression/zlib_ng.rs`: improved from 138 missing regions to
  124.

Retention:

- Retain Batch 35. The failed Batch 30 surfaces stayed fallible, while
  structural tree counters and bounded code-header conversions were made direct.

## Current retained gap map after Batch 35

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | 124 | 0 |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/tiff/decode.rs` | 14 | 0 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |

Next reverse-map priorities:

1. `src/codecs/compression/zlib_ng.rs`: remaining 124 regions are now mostly
   true defensive helper exits in slow/medium/level-nine matchers, cost
   selection in `write_block()`, RLE tree emission in `send_tree()`, token
   emission for malformed tokens, and fixed-block helper conversions. Continue
   only with narrow invariant edits or hook probes that do not change public
   bytes.
2. `src/codecs/webp/native/decoder.rs`: 47 region-only misses. Need a WebP
   container/parser sweep using exact RIFF/VP8X/ANIM/ANMF byte inputs first;
   defer code changes until the source-level map separates fixture gaps from
   defensive parser invariants.
3. `src/codecs/webp/native/lossless.rs`: 39 regions and 1 branch. The branch is
   currently generic-instantiation debt: source-line branch aggregation shows
   both sides covered, but LLVM summary still reports one uncovered branch.
   Next pass should trace this through a focused bit-reader/lossless hook input
   before touching production logic.
4. `src/codecs/tiff/decode.rs`: 14 region-only misses. Current zero-source
   loci are malformed metadata/offset arithmetic and compressed predictor/tile
   paths; attack with minimal TIFF byte fixtures or same-module hooks depending
   on whether Pillow accepts the fixture.
5. `src/codecs/webp/native/vp8.rs`: 12 region-only misses. Needs VP8 bitstream
   state probes or a tiny generated VP8 frame fixture; do not add random tests.
6. `src/codecs/webp/native/encoder.rs`: 1 region-only miss, likely writer
   padding/generic artifact after odd/even payload paths are already covered.

## Batch 36 plan

Goal: reduce the zlib `write_block()` and tree-emission region cluster without
changing malformed token/spec behavior.

Reverse mapping from snapshot `ace9c214-ae44-4f13-8ec2-5853a648607e`:

- `src/codecs/compression/zlib_ng.rs` has 124 missing regions and 0 missing
  branches.
- Remaining zero-source loci in the post-frequency block writer include
  `build_tree(...) ?`, `scan_tree(...) ?`, dynamic/static cost checked
  additions, `send_trees(...) ?`, `emit_tokens(...) ?`, and small RLE
  repeat-count conversions in `send_tree()`.
- `frequencies(tokens)?` is still the real malformed-token boundary for
  `write_block()`. Existing coverage hooks call malformed tokens directly, so
  this batch must not remove frequency validation, `length_index()` failure, or
  `distance_index()` failure.

Planned edit:

- Keep `frequencies(tokens)?` fallible.
- Convert only `write_block()` steps that operate on internally built, valid
  Huffman trees to `expect(...)`:
  - literal/distance/bit-length tree construction;
  - code-length tree scanning;
  - dynamic tree header emission;
  - token emission after valid frequencies;
  - end-of-block send after valid literal tree construction.
- Convert `send_tree()` repeat-count `u32::try_from(...)` wrappers to direct
  casts where branch guards already constrain the count to DEFLATE's small
  repeat ranges.
- Do not change `build_tree()` malformed spec paths, `static_lengths.get`,
  overflow repair, `emit_tokens()` malformed-token paths, or `emit_fixed_block()`
  malformed-token paths.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must stay at 1 or improve.
- Missing regions must improve below 237.
- If coverage fails or aggregate missing regions do not improve, revert Batch
  36.

Batch 36 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
  - `git diff --check`
- Coverage MCP run: `5bd2fe7e-ec23-4b38-9088-3b9df3eaebdb`.
- Coverage MCP snapshot: `38a02c5b-37bc-4562-8e73-7c4466f54225`.
- Result: `5 passed / 0 failed`.
- Lines: `26604 / 26604` (0 missing), retained.
- Branches: `3467 / 3468` (1 missing), retained.
- Functions: `1610 / 1610` (0 missing), retained.
- Regions: `42271 / 42496` (225 missing), improved by 12.
- `src/codecs/compression/zlib_ng.rs`: improved from 124 missing regions to
  112.

Retention:

- Retain Batch 36. The remaining fallible boundary in `write_block()` is
  malformed token frequency validation; generated trees and post-validation
  token emission are internal invariants.

## Current retained gap map after Batch 36

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | 112 | 0 |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/tiff/decode.rs` | 14 | 0 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |

## Batch 32 plan

Goal: continue the same zlib wrapper-invariant pattern for tokenizer lifecycle
calls, without touching direct malformed matcher hooks.

Reverse mapping from snapshot `69cedb7c-a440-4121-9df1-16a3092917ff`:

- `src/codecs/compression/zlib_ng.rs` has 165 missing regions and 0 missing
  branches.
- The first remaining zlib zero-source loci include wrapper-owned matcher calls:
  - `slow()`: `matcher.process(available, false)?` and final
    `matcher.process(available, true)?`;
  - `tokenize_lookahead_medium()`: boundary `refill_boundary()?`, per-chunk
    `process(..., false)?`, final `process(..., true)?`;
  - `tokenize_level9()`: same boundary/process/final pattern.
- Existing coverage hooks still directly exercise malformed shortened matchers
  and `refill_boundary()` failure paths. This batch must not remove or weaken
  those private checks.

Planned edit:

- Keep `available.checked_add(chunk_length)?` fallible. Invalid chunk totals
  still return `None`.
- Convert only lifecycle calls on freshly constructed wrapper-owned matchers to
  `expect(...)`.
- Leave matcher methods themselves returning `Option` for private malformed
  state tests.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must stay at 1.
- Missing regions must improve below 278.
- If coverage fails or aggregate missing regions do not improve, revert Batch
  32.

Remaining TIFF zero-source loci after Batch 29:

- `34`: missing/malformed `BitsPerSample` value lookup.
- `60-64`: row-byte and total-buffer checked arithmetic.
- `98-99`: tile row/tile-size checked arithmetic.
- `112-113`: encoded tile range and block decode failure.
- `139-140`: tile copy destination/source bounds.
- `199`: encoded strip range.
- `209`: compressed predictor unsupported-bit-width condition.
- `572`: `MsbBits::read()` bit-bound arithmetic.

## Batch 30 plan

Goal: reduce the zlib Huffman-tree bookkeeping region debt without changing
token validation, frequency overflow checks, or output bytes.

Reverse mapping from snapshot `93876e5e-c73a-428d-8f66-e72e5dd1803f`:

- `src/codecs/compression/zlib_ng.rs` still has 173 missing regions and 0
  missing branches.
- `build_tree()` has a dense cluster of zero regions around fixed DEFLATE
  heap/bookkeeping operations:
  - singleton-tree fallback: `current_max.checked_add(1)`;
  - heap tail bookkeeping: repeated `heap_max.checked_sub(1)`;
  - node depth and `next_node` increments;
  - parent-length-to-child-length conversion;
  - bit-count and bit-cost arithmetic for code lengths;
  - overflow-repair bit-count reshuffling.

Planned edit:

- Keep real data-dependent overflow checks:
  - frequency addition between Huffman nodes;
  - frequency counters in `frequencies()`;
  - token length/distance validation.
- Replace only fixed-size structural checks in `build_tree()` with direct
  arithmetic/indexing where the surrounding loop invariants and DEFLATE
  constants prove the operation cannot fail:
  - `current_max + 1` for `0..=1`;
  - `heap_max -= 1`;
  - `nodes[first].depth.max(nodes[second].depth) + 1`;
  - `next_node += 1`;
  - direct `heap[heap_max + 1..heap_size]`;
  - `bits + extra` conversions where both operands are bounded by DEFLATE
    tables;
  - bit-count repair increments/decrements.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must stay at 1.
- Missing regions must improve below 286.
- If zlib regions do not improve or any branch/line regresses, revert Batch 30.

Batch 30 measurement:

- Local checks passed before coverage:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
  - `git diff --check`
- First Coverage MCP run: `592bd76f-1d59-4feb-b438-52aaf687167e`.
- Result: failed, `4 passed / 1 failed`.
- Failure: `test_internal_coverage_hooks` panicked in `build_tree()` after
  replacing `static_lengths.get(index)?` with direct indexing. The coverage hook
  deliberately exercises a malformed private `TreeSpec`, so that bounds check
  is not dead code.
- Corrected Coverage MCP run: `b9784a26-5cdc-4046-9b29-00e88b0f3f5e`.
- Result: failed, `4 passed / 1 failed`.
- Failure: `test_internal_coverage_hooks` panicked in the overflow-repair
  bit-count path after direct subtraction. That path is also deliberately
  covered by malformed private tree inputs.

Retention:

- Revert Batch 30. `build_tree()` has private malformed-spec defensive paths
  that are intentionally tested by the coverage hook. The safe way to reduce
  this zlib cluster is not broad direct arithmetic; it needs either narrower
  helper extraction or fixture data that reaches valid tree shapes.

## Final retained validation after Batch 30 revert

- Coverage MCP run: `c63d7aeb-7dff-437a-9399-a93694f254d2`.
- Coverage MCP snapshot: `045ea808-4ba7-4807-99b7-2b1e7733f74e`.
- Result: `5 passed / 0 failed`.
- Lines: `26575 / 26575` (0 missing).
- Branches: `3467 / 3468` (1 missing).
- Functions: `1610 / 1610` (0 missing).
- Regions: `42275 / 42561` (286 missing).

Retained impact in this sweep:

- Missing regions improved from 304 at snapshot
  `27b758c5-b9ea-4c3f-8266-a5b6bcfc31de` to 286 at snapshot
  `045ea808-4ba7-4807-99b7-2b1e7733f74e`.
- Net improvement: 18 regions.
- Lines and functions stayed at 100%.
- Branch debt stayed at 1 missing branch, isolated to
  `src/codecs/webp/native/lossless.rs`.

Final retained gap map:

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | 173 | 0 |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/tiff/decode.rs` | 14 | 0 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |

## Batch 31 plan

Goal: reduce the zlib region cluster with the narrow pattern that worked for
level one: after tokenizer validation, top-level compressor functions should
not expose `emit_blocks()` failure as a public `None`.

Reverse mapping from snapshot `045ea808-4ba7-4807-99b7-2b1e7733f74e`:

- `src/codecs/compression/zlib_ng.rs` has 173 missing regions and 0 missing
  branches.
- The current first zlib zero-source loci include top-level `emit_blocks(...) ?`
  calls at lines `996`, `1007`, `1018`, `1029`, `1040`, `1074`, `1274`, and
  `1531`.
- These are different from the rejected Batch 30 tree-builder checks. They are
  not malformed private `TreeSpec` defensive paths; they are public compressor
  wrappers passing tokens produced by the matching internal tokenizer.

Planned edit:

- Keep tokenizer `?` propagation unchanged. Invalid chunk totals or tokenizer
  failures still return `None`.
- Convert only the top-level compressor `emit_blocks()` calls to
  `expect(...)` with invariant-specific messages.
- Do not modify `emit_blocks()`, `write_block()`, `build_tree()`, or malformed
  token/spec coverage-hook paths.

Retention gate:

- Lines/functions must stay at 100%.
- Missing branches must stay at 1.
- Missing regions must improve below 286.
- If the batch fails coverage or does not improve aggregate missing regions,
  revert Batch 31.

Batch 31 measurement:

- Local checks passed:
  - `cargo fmt --all --check`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
  - `git diff --check`
- Coverage MCP run: `e52525a2-cb21-4db3-bbe3-d14299d3dde7`.
- Coverage MCP snapshot: `69cedb7c-a440-4121-9df1-16a3092917ff`.
- Result: `5 passed / 0 failed`.
- Lines: `26583 / 26583` (0 missing), retained.
- Branches: `3467 / 3468` (1 missing), retained.
- Functions: `1610 / 1610` (0 missing), retained.
- Regions: `42291 / 42569` (278 missing), improved by 8.
- `src/codecs/compression/zlib_ng.rs`: improved from 173 missing regions to
  165.

Retention:

- Retain Batch 31. Public compressor wrappers now keep tokenizer failure as the
  fallible boundary, while expressing the invariant that internally produced
  tokens emit as valid DEFLATE.

## Current retained gap map after Batch 31

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | 165 | 0 |
| `src/codecs/webp/native/decoder.rs` | 47 | 0 |
| `src/codecs/webp/native/lossless.rs` | 39 | 1 |
| `src/codecs/tiff/decode.rs` | 14 | 0 |
| `src/codecs/webp/native/vp8.rs` | 12 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |
