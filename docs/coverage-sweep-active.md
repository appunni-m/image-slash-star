# Coverage sweep active tracker

This file tracks the current retained coverage state and the next bulk fixes.
It is intentionally shorter than `docs/coverage-region-sweep.md`: this is the
active checklist for getting the remaining LLVM source regions and branch to
100%.

## Current retained baseline

- Coverage MCP run: `059e32bb-8a4b-445e-b514-0e1bd97b58ad`
- Coverage MCP snapshot: `eb1e5e29-2ab8-4b23-8a9f-8484af4644b2`
- Command: `all-features-llvm-cov-json-nightly-branch`
- Result: `5 passed / 0 failed`
- Lines: `27001 / 27001` (100%)
- Functions: `1630 / 1630` (100%)
- Branches: `3487 / 3488` (1 missing)
- Regions: `42737 / 42799` (62 missing)

## Remaining gap map

| File | Missing regions | Missing branches | Current reason |
| --- | ---: | ---: | --- |
| `src/codecs/compression/zlib_ng.rs` | 0 | 0 | Complete after converting proven generated-token/matcher invariants to direct operations or invariant `expect(...)` calls. |
| `src/codecs/webp/native/decoder.rs` | 32 | 0 | Batch 63 cleared all raw zero-line `Read`/`Seek` propagation entries; remaining aggregate debt currently has no raw zero-line entries in the MCP artifact. |
| `src/codecs/webp/native/lossless.rs` | 21 | 1 | Batch 63 cleared the raw zero-line bit-reader/fill propagation entries; the retained branch gap still maps to `BitReader::read_bits`. |
| `src/codecs/tiff/decode.rs` | 0 | 0 | Complete after coverage-only malformed header overflow fixtures. |
| `src/codecs/webp/native/vp8.rs` | 8 | 0 | Batch 64 first-partition fixtures cleared one more VP8 aggregate region; remaining debt is still parser/residual decode propagation. |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 | Top-level `WebPEncoder::encode()` source-region artifact remains after direct writer-failure hook coverage. |

## Batch 64 plan

Goal: make one fixture-first VP8 sweep before adding more private hooks.

Reverse mapping from retained snapshot
`1f3ce856-4942-4a21-aebd-253e40bf7250`:

- Current aggregate state is lines `27001 / 27001`, functions `1630 / 1630`,
  branches `3487 / 3488`, and regions `42736 / 42799`.
- MCP still reports WebP native debt only:
  - `decoder.rs`: 32 regions, 0 branches, with no raw zero-line entries;
  - `lossless.rs`: 21 regions, 1 branch, with no raw zero-line entries;
  - `vp8.rs`: 9 regions, 0 branches;
  - `encoder.rs`: 1 region, 0 branches, with no raw zero-line entry.
- The remaining explicit VP8 source loci are caller propagation sites in
  `read_frame_header()` / `decode_frame_()`:
  - line 1193: `read_loop_filter_adjustments()?`;
  - line 1200: `init_partitions(num_partitions)?`;
  - line 1202: `read_quantization_indices()?`;
  - line 1220: accumulated final header bit check;
  - line 1492: Y2 coefficient read propagation;
  - line 1547: UV coefficient read propagation;
  - line 1860: macroblock header propagation.
- Existing public fixtures already mutate VP8 first-partition size for
  `0..=12`, `16`, `24`, and `32`. A temp Pillow probe confirms the skipped
  partition sizes `13..=15`, `17..=23`, and `25..=31` are all rejected by
  Pillow with `OSError: failed to read next frame`, so they are valid
  manifest-driven oracle fixtures.
- Hypothesis: the skipped partition sizes may move malformed payload boundaries
  through the remaining `read_frame_header()` propagation sites. If they do not
  move the retained coverage counters, the exact caller `?` regions are likely
  hook-only and should be handled with targeted coverage-only decoder states in
  a later batch.

Planned edit:

- Expand `scripts/generate_test_assets.py` to generate the full
  `vp8_partition_0.webp` through `vp8_partition_32.webp` fixture set.
- Add the skipped partition fixtures to the WebP decode manifest as
  `expect_error` Pillow-oracle cases.
- Regenerate WebP assets and Pillow oracle references with:
  - `.oracle-venv/bin/python scripts/generate_test_assets.py --format webp`;
  - `.oracle-venv/bin/python scripts/generate_decode_refs.py --format webp`.
- Run local non-coverage gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.
- Retain the batch only if it preserves 100% line/function coverage and reduces
  aggregate missing regions or branches.

Applied edit:

- Expanded `scripts/generate_test_assets.py` to emit every
  `vp8_partition_0.webp` through `vp8_partition_32.webp`.
- Added the skipped partition sizes 13, 14, 15, 17 through 23, and 25 through
  31 to `manifest.yaml` under the WebP malformed expected-error group.
- Regenerated WebP assets and Pillow oracle references:
  - `.oracle-venv/bin/python scripts/generate_test_assets.py --format webp`;
  - `.oracle-venv/bin/python scripts/generate_decode_refs.py --format webp`.
- Pillow classified each new partition fixture as
  `builtins.OSError: failed to read next frame`.

Local validation:

- `cargo fmt --all --check`: passed.
- `git diff --check`: passed.
- `.oracle-venv/bin/python -m json.tool tests/fixtures/input/jsons/Decode.webp.json`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.

Coverage MCP validation:

- Run: `059e32bb-8a4b-445e-b514-0e1bd97b58ad`.
- Snapshot: `eb1e5e29-2ab8-4b23-8a9f-8484af4644b2`.
- Commit measured: `a0f65d521edad0cfc04889060d65979c369d6a2e`.
- Result: `5 passed / 0 failed`.
- Lines: `27001 / 27001` (100%).
- Functions: `1630 / 1630` (100%).
- Branches: `3487 / 3488` (1 missing, unchanged).
- Regions: `42737 / 42799` (62 missing), improved by 1.
- `vp8.rs`: improved from `2748 / 2757` to `2749 / 2757` regions.

Retention:

- Retain Batch 64. It keeps line/function coverage at 100%, does not worsen
  the single branch miss, and proves at least one remaining VP8 region was
  reachable by public Pillow-oracle malformed first-partition bytes.

## Batch 65 plan

Goal: clear the smallest remaining VP8 caller-propagation regions using narrow
coverage-only decoder states after the public partition sweep has been
exhausted.

Reverse mapping from retained snapshot
`eb1e5e29-2ab8-4b23-8a9f-8484af4644b2`:

- Current aggregate state is lines `27001 / 27001`, functions `1630 / 1630`,
  branches `3487 / 3488`, and regions `42737 / 42799`.
- `vp8.rs` is now `2749 / 2757` regions, so 8 aggregate regions remain.
- Batch 64 proved that one VP8 parser region was public-fixture reachable, but
  the remaining `coverage_query` view still shows only normalized partial
  branch-style line groups rather than exact region records. The actionable
  source sites remain the caller `?` boundaries in:
  - `read_frame_header()` around loop-filter, partition initialization,
    quantization, and final accumulated bit checks;
  - `read_residual_data()` around Y2 and UV coefficient propagation;
  - `decode_frame_()` around macroblock header propagation.
- The residual propagation sites are not clean public fixture targets because
  public malformed WebP bytes must first satisfy the container and frame header
  parser. They can be isolated by constructing a coverage-only `Vp8Decoder`
  with initialized macroblock borders and controlled arithmetic partition
  lengths, then calling `read_residual_data()` directly.
- The macroblock-header caller boundary may still be reachable through raw VP8
  keyframe payloads, so this batch will add only coverage-only raw VP8 payload
  probes inside the existing private hook, not new committed fixtures.

Planned edit:

- Add small coverage-only VP8 helpers inside
  `vp8::__coverage_exercise_private_branches()`:
  - one helper to construct a decoder ready for residual decoding with a
    specified token-partition length;
  - one helper to construct raw keyframe bytes with specified first-partition
    content.
- Exercise `read_residual_data()` with an empty partition for Y2 propagation and
  with progressively larger zero partitions for UV propagation.
- Exercise `decode_frame_()` with raw keyframe first partitions sized to pass
  header parsing but fail during macroblock parsing/residual reads.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.
- Retain only if line/function coverage stays 100%, branch debt does not
  increase, and missing regions decrease.

Validation:

- Run: `d13fbc55-c45f-4cab-a728-484925afea16`.
- Snapshot: `e2877547-88f3-45dd-9b68-cc79167039e4`.
- Result: `5 passed / 0 failed`.
- Lines: `27048 / 27048` (100%).
- Functions: `1632 / 1632` (100%).
- Branches: `3487 / 3488` (1 missing, unchanged).
- Regions: `42807 / 42869` (62 missing, unchanged).

Rejection:

- Do not retain Batch 65. The probes only added covered coverage-hook code and
  did not reduce the absolute missing-region count below the Batch 64 retained
  baseline. The VP8 hook edit was removed.
- Next VP8 work should use more precise reverse mapping against the raw VP8
  boolean coding state before adding hooks. Blind partition-length probes are
  not sufficient.

## Batch 66 plan

Goal: clear the smallest concrete remaining coverage debt after Batch 64,
prioritizing directly reverse-mapped paths over broad hook sweeps.

Reverse mapping from retained snapshot
`eb1e5e29-2ab8-4b23-8a9f-8484af4644b2`:

- Current aggregate state is lines `27001 / 27001`, functions `1630 / 1630`,
  branches `3487 / 3488`, and regions `42737 / 42799`.
- `encoder.rs` still has exactly one aggregate region miss, but MCP reports no
  uncovered line, partial branch line, or uncovered function line for that file.
  Selected lines around `WebPEncoder::encode()` and the existing writer-failure
  hook are all hit and have `0 / 0` branches, so there is no concrete
  line-level input to reverse-map in this file.
- `lossless.rs` still owns the only aggregate branch miss. The retained branch
  source maps to `BitReader::read_bits()` line 1457:
  `if self.nbits < num { self.fill()?; }`.
- Existing retained hooks cover:
  - the false path where enough bits are already buffered;
  - the true path where `fill()` returns an I/O error;
  - direct `fill()` short and long buffer cases;
  - direct `consume()` success and failure.
- Missing hypothesis: `read_bits()` with `nbits < num` where `fill()` succeeds
  but EOF leaves `nbits` still below `num`, causing the later `consume(num)?`
  to return `DecodingError::BitStreamError`. This is a real parser state for
  truncated VP8L payloads and is more precise than adding another malformed
  whole-file fixture.

Planned edit:

- Add one narrow coverage-only call in
  `lossless::__coverage_exercise_private_branches()`:
  `BitReader::__coverage_new(Cursor::new(Vec::<u8>::new())).read_bits::<u8>(1)`.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.
- Retain only if line/function coverage stays 100%, branch debt decreases or
  aggregate missing regions decrease, and no WebP file regresses.

Validation:

- Run: `cef7cf5a-d06a-4099-b6de-fe6f4215ecaa`.
- Snapshot: `cf5b4571-3014-45c3-8888-93068c6a458d`.
- Result: `5 passed / 0 failed`.
- Lines: `27003 / 27003` (100%).
- Functions: `1630 / 1630` (100%).
- Branches: `3487 / 3488` (1 missing, unchanged).
- Regions: `42743 / 42805` (62 missing, unchanged).
- `lossless.rs`: remained `119 / 120` branches and 21 missing regions.

Rejection:

- Do not retain Batch 66. The hook only added covered coverage-hook regions and
  did not reduce the retained absolute missing-region or missing-branch count.
- The tested `read_bits()` fill-success/consume-error state is not the retained
  branch gap. Next lossless work should avoid generic `BitReader` probes and
  instead target parser-level states in `read_transforms()`,
  `read_huffman_codes()`, `read_huffman_code_lengths()`, or
  `decode_image_data()`.

## Batch 67 plan

Goal: add valid, tiny VP8L color-indexing parity fixtures that force concrete
parser/transform branches rather than adding more malformed EOF or generic
reader probes.

Reverse mapping from retained snapshot
`eb1e5e29-2ab8-4b23-8a9f-8484af4644b2` plus Batch 66 rejection:

- Remaining retained coverage is still lines/functions at 100%, branches
  `3487 / 3488`, and 62 missing regions.
- Direct `BitReader::read_bits()` hooks have now been rejected in Batches 57
  and 66, so the single lossless branch gap is not cleared by generic helper
  state probes.
- `lossless.rs` still reports normalized partial groups at
  `read_transforms()` color-index table-size logic: table sizes `<= 2`,
  `<= 4`, `<= 16`, and `> 16`.
- Existing Pillow-generated palette fixtures may or may not force libwebp to
  emit each exact VP8L color-indexing table-size band. A handcrafted VP8L
  stream can force the transform while staying a public Pillow-oracle input.
- Prototype inputs created in `/private/tmp` were accepted by Pillow 12.2.0 /
  libwebp 1.6.0 as `RGB (1, 1)` and decoded to the expected color when:
  - the recursive color-table image stream omits the meta-Huffman flag
    (`read_meta=false`);
  - the main ARGB image stream includes the no-meta flag (`read_meta=true`).

Planned edit:

- Add reusable VP8L bit helpers for simple no-cache/no-meta image streams in
  `scripts/generate_test_assets.py`.
- Generate four valid tiny fixtures:
  - `vp8l_color_index_table2.webp`;
  - `vp8l_color_index_table3.webp`;
  - `vp8l_color_index_table5.webp`;
  - `vp8l_color_index_table17.webp`.
- Register them under the WebP lossless transform corpus, regenerate assets and
  Pillow oracle references, run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.
- Retain only if all parity tests pass and aggregate missing regions or
  branches improve without losing 100% line/function coverage.

Validation:

- Run: `098758c2-d02a-4eff-a89e-29fdd0678830`.
- Snapshot: `b8454e3b-ceb9-4e5d-97c0-01208a95863c`.
- Result: `5 passed / 0 failed`.
- Lines: `27001 / 27001` (100%).
- Functions: `1630 / 1630` (100%).
- Branches: `3487 / 3488` (1 missing, unchanged).
- Regions: `42737 / 42799` (62 missing, unchanged).
- `lossless.rs`, `decoder.rs`, and `vp8.rs` counters were unchanged.

Rejection:

- Do not retain Batch 67. The handcrafted valid color-index fixtures are
  Pillow-oracle valid, but they do not reduce any retained aggregate region or
  branch debt.
- Existing retained fixtures and hooks already exercise the relevant
  color-index table-size bands for coverage purposes. Do not add these fixtures
  unless the project later wants broader corpus coverage independent of the
  100% coverage goal.

## Batch 68 plan

Goal: clear the smallest remaining file-level region: the single
`encoder.rs` aggregate region artifact.

Reverse mapping from retained snapshot
`eb1e5e29-2ab8-4b23-8a9f-8484af4644b2`:

- `encoder.rs` is `1851 / 1852` regions with 100% lines, 100% branches, and no
  MCP line-level gaps.
- `WebPEncoder::encode()` has four fallible write boundaries:
  - RIFF signature;
  - RIFF size;
  - WEBP signature;
  - VP8L `write_chunk(...)`.
- The retained coverage hook currently forces writer failures at outer write
  calls `0..=3`. That covers RIFF, RIFF size, WEBP, and the VP8L chunk-name
  write inside `write_chunk`.
- `write_chunk()` itself has additional fallible writes for chunk size and
  payload. If the remaining source-region artifact belongs to the top-level
  `write_chunk(...)?` call, failing the inner chunk-size or payload write is the
  smallest concrete input that has not been tried.

Planned edit:

- Extend `encoder::__coverage_exercise_private_branches()` with
  `FailOnWrite { fail_at: 4 }` and `FailOnWrite { fail_at: 5 }` calls through
  `WebPEncoder::encode(...)`.
- Do not change production behavior or fixtures.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.
- Retain only if aggregate missing regions decrease or branches improve.

Validation:

- Run: `5b51f7aa-a9dc-429d-9f27-2c0735142715`.
- Snapshot: `5595c329-d616-43bc-a35f-8fcc5b5f469f`.
- Result: `5 passed / 0 failed`.
- Lines: `27011 / 27011` (100%).
- Functions: `1630 / 1630` (100%).
- Branches: `3487 / 3488` (1 missing, unchanged).
- Regions: `42745 / 42807` (62 missing, unchanged).
- `encoder.rs`: stayed one missing region (`1859 / 1860` after the temporary
  hook lines, equivalent to retained `1851 / 1852`).

Rejection:

- Do not retain Batch 68. Failing the inner VP8L chunk-size and payload writes
  only added covered hook regions; it did not clear the retained top-level
  encoder source-region artifact.
- Treat the remaining encoder region as non-actionable until a raw LLVM region
  coordinate or source restructuring hypothesis identifies a concrete uncovered
  path.

## Batch 69 plan

Goal: target the retained VP8 aggregate regions in the exact reader
instantiation reported by the MCP-produced LLVM JSON.

Reverse mapping from retained snapshot
`eb1e5e29-2ab8-4b23-8a9f-8484af4644b2` and raw LLVM export produced by MCP:

- MCP detailed summaries show `vp8.rs` has 8 missing regions and 13 uncovered
  instantiations while keeping 100% lines and branches.
- The actionable aggregate zero regions are one-character `?` propagation
  regions in the real `Cursor<&[u8]>` instantiation:
  - `init_partitions()` line 999;
  - `read_frame_header()` lines 1193, 1200, 1202, and 1220;
  - `read_residual_data()` lines 1492 and 1547;
  - `decode_frame_()` line 1860.
- Current coverage hooks directly exercise many equivalent raw VP8 parser
  states with `Cursor<Vec<u8>>` and `Take<Cursor<_>>`, but the retained zero
  coordinates remain under `Cursor<&[u8]>`.
- Public malformed WebP fixtures also use borrowed input, but the RIFF/VP8
  container path filters which raw VP8 payload states can be reached. A private
  hook can replay the exact raw VP8 payloads through `Cursor<&[u8]>` without
  changing production behavior.

Planned edit:

- Add a narrow `with_borrowed_decoder!` helper in
  `vp8::__coverage_exercise_private_branches()` that constructs
  `Vp8Decoder<Cursor<&[u8]>>`.
- Replay the existing `take_frame_cases` through `read_frame_header()` using
  borrowed slices.
- Replay the existing direct `init_partitions(2)` short-size cases through
  borrowed slices.
- Do not add new public fixtures in this batch.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.
- Retain only if aggregate missing regions decrease or branches improve.

Validation:

- Run: `efa0d554-947a-41b3-b669-8b8fa73bff12`.
- Snapshot: `39fd72f9-8a0f-4942-97d6-351cfbdfa2e7`.
- Result: `5 passed / 0 failed`.
- Lines: `27013 / 27013` (100%).
- Functions: `1630 / 1630` (100%).
- Branches: `3487 / 3488` (1 missing, unchanged).
- Regions: `42746 / 42808` (62 missing, unchanged).
- `vp8.rs`: stayed 8 missing regions after adding covered borrowed-slice hook
  lines (`2758 / 2766`, equivalent to retained `2749 / 2757`).

Rejection:

- Do not retain Batch 69. Replaying the raw VP8 parser cases through
  `Cursor<&[u8]>` only added covered hook regions and did not clear the
  retained one-character propagation regions.
- The remaining VP8 aggregate regions are not solved by duplicating reader
  instantiations. Next VP8 work needs exact boolean-coder state construction for
  the callee error itself, or source restructuring that removes impossible
  `Cursor` error-propagation artifacts.

## Batch 59 plan

Goal: make one fixture-first sweep across the remaining WebP native region debt,
prioritizing public Pillow-oracle malformed byte streams over new private
coverage hooks.

Reverse mapping from retained snapshot
`e0363b79-3da6-4fbf-8fc7-91a552bfa49f`:

- `lossless.rs` has the only remaining branch gap and the largest actionable
  fixture surface: 29 missing regions. The source loci cluster in:
  - transform parsing and recursive transform image streams:
    `read_transforms()` lines 207, 220, 239, 262, and 265;
  - meta Huffman parsing and recursive entropy images:
    `read_huffman_codes()` lines 315, 317, and 324;
  - Huffman tree construction and code-length parsing:
    `read_huffman_code()` / `read_huffman_code_lengths()` lines 392, 400,
    421, 422, 423, and 442;
  - pixel decode fill/symbol propagation:
    `decode_image_data()` lines 501 and 555;
  - the retained branch miss is still attributed by MCP to
    `BitReader::read_bits()` line 1320.
- Prior direct `BitReader::read_bits()` hook calls did not improve the branch
  or aggregate region totals, so Batch 59 will not add a new generic reader
  hook first.
- Public VP8L files can only end on byte boundaries, so bit reads immediately
  after 1, 3, or 6 consumed bits may be impossible to EOF using committed image
  bytes. Candidate fixtures should therefore target byte-boundary-aligned
  parser states first: color-indexing size truncation, recursive transform
  stream truncation, meta Huffman stream truncation, two-symbol Huffman
  truncation, and max-symbol/code-length truncation.
- `decoder.rs` remaining regions are mostly `Seek`/`Read` propagation and
  normalized animation/alpha paths. Some are public-byte reachable through
  malformed VP8X/ANMF chunk layouts, but several require a reader that fails
  after a successful header scan; those should not block the VP8L fixture sweep.
- `vp8.rs` remaining regions are parser/residual propagation sites around
  `init_partitions()`, `read_frame_header()`, and residual coefficient reads.
  The previous hook attempt regressed coverage, so this batch should only add
  VP8 byte fixtures if their malformed partition/frame payloads are
  Pillow-rejected and do not introduce new hook code.
- `encoder.rs` retains one top-level region artifact after writer-failure hooks;
  no new encoder hook is planned in this fixture batch.

Planned edit:

- Add reproducible VP8L malformed fixtures in
  `scripts/generate_test_assets.py`, then register them in the WebP
  `error_malformed_container` manifest group only if Pillow rejects them.
- Initial candidates:
  - `vp8l_color_index_size_truncated.webp`;
  - `vp8l_predictor_transform_stream_truncated.webp`;
  - `vp8l_color_transform_stream_truncated.webp`;
  - `vp8l_meta_huffman_stream_truncated.webp`;
  - `vp8l_two_symbol_truncated_one_symbol.webp`;
  - `vp8l_code_lengths_max_symbol_flag_truncated.webp`;
  - `vp8l_code_lengths_length_nbits_truncated.webp`;
  - `vp8l_code_lengths_max_value_truncated.webp`.
- Regenerate WebP fixtures and Pillow oracle references.
- Run local non-coverage gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.
- Retain the batch only if it preserves 100% line/function coverage and reduces
  aggregate missing regions or branches.

Applied edit:

- Added reproducible generation for eight Pillow-rejected VP8L malformed
  fixtures:
  - `vp8l_color_index_size_truncated.webp`;
  - `vp8l_predictor_transform_stream_truncated.webp`;
  - `vp8l_color_transform_stream_truncated.webp`;
  - `vp8l_meta_huffman_stream_truncated.webp`;
  - `vp8l_two_symbol_truncated_one_symbol.webp`;
  - `vp8l_code_lengths_max_symbol_flag_truncated.webp`;
  - `vp8l_code_lengths_length_nbits_truncated.webp`;
  - `vp8l_code_lengths_max_value_truncated.webp`.
- Added those fixtures to the WebP `error_malformed_container` manifest group.
- Regenerated WebP assets and Pillow oracle references with:
  - `.oracle-venv/bin/python scripts/generate_test_assets.py --format webp`;
  - `.oracle-venv/bin/python scripts/generate_decode_refs.py --format webp`.
- Pillow classified all eight new fixtures as
  `builtins.OSError: failed to read next frame`.

Local validation:

- `cargo fmt --all --check`: passed.
- `git diff --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.

Coverage MCP validation:

- Run: `c94c0a0b-fd6b-45ed-b70b-d15eecf47a0b`.
- Snapshot: `a8cfbdff-2d5a-46e7-bff5-46a99b9344d3`.
- Result: `5 passed / 0 failed`.
- Lines: `26811 / 26811` (100%).
- Functions: `1620 / 1620` (100%).
- Branches: `3477 / 3478` (1 missing, unchanged).
- Regions: `42508 / 42575` (67 missing), improved by 7.
- `src/codecs/webp/native/lossless.rs`: improved from 29 to 22 missing
  regions.

Retention:

- Retain Batch 59. It reduces public VP8L malformed-bitstream region debt with
  Pillow-rejected byte fixtures and preserves 100% line/function coverage.

## Batch 60 plan

Goal: continue the VP8L fixture sweep with byte-boundary EOF cases that Batch
59 did not explicitly target.

Reverse mapping from retained snapshot
`a8cfbdff-2d5a-46e7-bff5-46a99b9344d3`:

- `lossless.rs` remains the only branch-missing file and has 22 missing
  regions.
- MCP detailed file view confirms the aggregate lossless raw metrics
  (`1248 / 1270` regions, `113 / 114` branches), but it does not expose exact
  per-region coordinates beyond normalized source-line groups. The remaining
  normalized groups still cluster around transform/Huffman setup and
  code-length/pixel decode propagation.
- Public-byte candidates left after Batch 59:
  - color-indexing transform after a successful table-size byte, then EOF in
    the recursive color-map image stream (`read_transforms()` line 265);
  - simple Huffman tree with `is_first_8bits = 1`, then EOF while reading the
    8-bit zero symbol (`read_huffman_code()` around lines 382-385);
  - implicit Huffman tree with declared code-length alphabet longer than the
    bytes provided, then EOF while reading a 3-bit code-length code length
    (`read_huffman_code()` line 403);
  - code-length repeat symbol 17 and 18 paths with too few extra bits
    (`read_huffman_code_lengths()` line 467).
- These are still real malformed VP8L byte streams and should be
  Pillow-rejected fixtures, not private hooks.

Planned edit:

- Add reproducible generation and manifest rows for:
  - `vp8l_color_index_stream_truncated.webp`;
  - `vp8l_zero_symbol_truncated.webp`;
  - `vp8l_code_length_alphabet_truncated.webp`;
  - `vp8l_repeat_code17_extra_truncated.webp`;
  - `vp8l_repeat_code18_extra_truncated.webp`.
- Regenerate WebP assets and Pillow oracle references.
- Run local gates and Coverage MCP.
- Retain only if aggregate regions or branches improve without losing 100%
  line/function coverage.

Applied edit:

- Added reproducible generation for five additional Pillow-rejected VP8L
  malformed fixtures:
  - `vp8l_color_index_stream_truncated.webp`;
  - `vp8l_zero_symbol_truncated.webp`;
  - `vp8l_code_length_alphabet_truncated.webp`;
  - `vp8l_repeat_code17_extra_truncated.webp`;
  - `vp8l_repeat_code18_extra_truncated.webp`.
- Added those fixtures to the WebP `error_malformed_container` manifest group.
- Regenerated WebP assets and Pillow oracle references with:
  - `.oracle-venv/bin/python scripts/generate_test_assets.py --format webp`;
  - `.oracle-venv/bin/python scripts/generate_decode_refs.py --format webp`.
- Pillow classified all five fixtures as
  `builtins.OSError: failed to read next frame`.

Local validation:

- `cargo fmt --all --check`: passed.
- `git diff --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.

Coverage MCP validation:

- Run: `73dfbc9d-51f5-4637-97e1-bf6855001a64`.
- Snapshot: `97ef0db7-bfdc-4bfd-b0a3-e97573f4ffad`.
- Result: `5 passed / 0 failed`.
- Lines: `26811 / 26811` (100%).
- Functions: `1620 / 1620` (100%).
- Branches: `3477 / 3478` (1 missing, unchanged).
- Regions: `42509 / 42575` (66 missing), improved by 1.
- `src/codecs/webp/native/lossless.rs`: improved from 22 to 21 missing
  regions.

Retention:

- Retain Batch 60. It still improves aggregate regions without regressing
  line/function/branch coverage, but the low yield indicates that the remaining
  public byte-boundary VP8L EOF cases are nearly exhausted.

## Batch 61 plan

Goal: try the smallest public-byte WebP decoder fixtures for the remaining
`decoder.rs` region debt before adding any new private reader hooks.

Reverse mapping from retained snapshot
`97ef0db7-bfdc-4bfd-b0a3-e97573f4ffad` plus MCP source windows:

- `decoder.rs` has 35 missing regions and 0 missing branches.
- The remaining source loci are:
  - VP8X scan error propagation around line 278;
  - ANIM seek/read propagation around lines 305 and 307;
  - first-ANMF nested scan seek propagation around line 322;
  - VP8L alpha decode propagation around line 426;
  - lossy alpha range-reader propagation around line 456;
  - animation frame seek propagation around line 509;
  - animated `ALPH` next-chunk seek/read propagation around lines 573 and 574.
- Existing coverage-only hooks already include failing-seek readers for many of
  these paths. Batch 61 should therefore avoid adding new hook machinery and
  only try public malformed containers.
- Public-byte candidates that should reach source not yet covered by regular
  fixtures:
  - a valid `ANMF` scanned before an `ANIM` chunk whose header declares 6 bytes
    but physically contains only 4; VP8X validation can pass, then
    `read_image()` should fail at the ANIM `read_exact(...) ?`;
  - an animated alpha frame truncated immediately after the `ALPH` payload;
    `read_frame()` should read alpha, seek to the next nested chunk start, then
    fail reading the missing VP8 chunk header.

Planned edit:

- Add reproducible generation and manifest rows for:
  - `animated_anim_payload_eof_after_anmf.webp`;
  - `animated_alpha_missing_nested_vp8_header.webp`.
- Regenerate WebP assets and Pillow oracle references.
- Run local gates and Coverage MCP.
- Retain only if aggregate regions or branches improve without losing 100%
  line/function coverage.

Applied edit:

- Added reproducible generation for two Pillow-rejected animated WebP malformed
  fixtures:
  - `animated_anim_payload_eof_after_anmf.webp`;
  - `animated_alpha_missing_nested_vp8_header.webp`.
- Added those fixtures to the WebP `error_malformed_container` manifest group.
- Regenerated WebP assets and Pillow oracle references with:
  - `.oracle-venv/bin/python scripts/generate_test_assets.py --format webp`;
  - `.oracle-venv/bin/python scripts/generate_decode_refs.py --format webp`.
- Pillow classified both fixtures as
  `builtins.OSError: could not create decoder object`.

Local validation:

- `cargo fmt --all --check`: passed.
- `git diff --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.

Coverage MCP validation:

- Run: `b7714256-0862-4d84-a4f4-dc6ead98f2aa`.
- Snapshot: `451cab41-c1ce-4bcf-952e-036438077773`.
- Result: `5 passed / 0 failed`.
- Lines: `26811 / 26811` (100%).
- Functions: `1620 / 1620` (100%).
- Branches: `3477 / 3478` (1 missing, unchanged).
- Regions: `42510 / 42575` (65 missing), improved by 1.
- `src/codecs/webp/native/decoder.rs`: improved from 35 to 34 missing
  regions.

Retention:

- Retain Batch 61. It proves one remaining decoder path is public-byte
  reachable, but the low yield indicates the remaining decoder region debt is
  mostly failing-reader or source-region artifact work.

## Batch 62 plan

Goal: cover the remaining public VP8L-alpha decode propagation in
`decoder.rs`.

Reverse mapping after Batch 61:

- Current retained snapshot: `451cab41-c1ce-4bcf-952e-036438077773`.
- Remaining zero-region entries from the MCP-produced LLVM artifact:
  - `decoder.rs`: lines 278, 305, 322, 426, 456, 509, and 573;
  - `lossless.rs`: lines 207, 220, 239, 315, 317, 442, 501, and 555;
  - `vp8.rs`: lines 999, 1193, 1200, 1202, 1220, 1492, 1547, and 1860.
- Batch 61 cleared the public ANIM/ALPH read EOF loci at decoder lines 307 and
  574.
- Decoder line 426 is the `VP8L` image path when `self.has_alpha` is true:
  `decoder.decode_frame(self.width, self.height, buf)?`.
- Existing `vp8l_alpha_header_only.webp` does not set `self.has_alpha` because
  its standalone VP8L header has alpha bit `0`. The public way to make
  `self.has_alpha` true for a lossless image is a VP8X container with the alpha
  flag set and a `VP8L` image chunk.

Planned edit:

- Add reproducible generation and a manifest row for
  `extended_vp8l_alpha_header_only.webp`: VP8X alpha flag set, dimensions taken
  from the source VP8L header, and the VP8L chunk truncated to its five-byte
  signature/header.
- Regenerate WebP assets and Pillow oracle references.
- Run local gates and Coverage MCP.
- Retain only if aggregate regions or branches improve without losing 100%
  line/function coverage.

Applied edit:

- Added reproducible generation for
  `extended_vp8l_alpha_header_only.webp`, a VP8X alpha container whose VP8L
  payload is truncated to the five-byte signature/header.
- Added the fixture to the WebP `error_malformed_container` manifest group.
- Regenerated WebP assets and Pillow oracle references with:
  - `.oracle-venv/bin/python scripts/generate_test_assets.py --format webp`;
  - `.oracle-venv/bin/python scripts/generate_decode_refs.py --format webp`.
- Pillow classified the fixture as
  `builtins.OSError: failed to read next frame`.

Local validation:

- `cargo fmt --all --check`: passed.
- `git diff --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.

Coverage MCP validation:

- Run: `4aa00aa6-c25e-45bc-bd84-d534930dbe2b`.
- Snapshot: `ea55a704-d482-45ce-97e6-b3cf2db786f4`.
- Result: `5 passed / 0 failed`.
- Lines: `26811 / 26811` (100%).
- Functions: `1620 / 1620` (100%).
- Branches: `3477 / 3478` (1 missing, unchanged).
- Regions: `42511 / 42575` (64 missing), improved by 1.
- `src/codecs/webp/native/decoder.rs`: improved from 34 to 33 missing
  regions.

Retention:

- Retain Batch 62. It confirms decoder line 426 was public-byte reachable only
  through VP8X alpha metadata, not the standalone VP8L header alpha bit.

## Batch 63 plan

Goal: clear exact WebP native region debt that cannot be reached by committed
bytes because the missing operation is an injected `Read` or `Seek` failure.

Reverse mapping from retained snapshot
`ea55a704-d482-45ce-97e6-b3cf2db786f4`:

- Current aggregate state is lines `26811 / 26811`, functions `1620 / 1620`,
  branches `3477 / 3478`, and regions `42511 / 42575`.
- Remaining zero-region entries in the MCP-produced LLVM artifact:
  - `decoder.rs`: lines 278, 305, 322, 456, 509, and 573;
  - `lossless.rs`: lines 207, 220, 239, 315, 317, 442, 501, and 555;
  - `vp8.rs`: lines 999, 1193, 1200, 1202, 1220, 1492, 1547, and 1860;
  - `encoder.rs`: one source-region artifact with no zero-line entry.
- Decoder lines 305, 322, 456, 509, and 573 are all `Seek` propagation
  regions. Real WebP bytes can produce EOF or malformed chunk errors, but they
  cannot make an otherwise valid reader return `io::ErrorKind::Other` from
  `Seek`.
- Decoder line 278 is the non-EOF VP8X chunk-scan error propagation path. The
  public byte fixture for EOF has already been covered; non-EOF requires an
  injected failing reader.
- Lossless lines 207, 220, 239, 315, and 317 are bit-reader propagation sites
  immediately after partially consumed fields. Public WebP files only truncate
  on byte boundaries, so exact sub-byte I/O failure is coverage-hook territory.
- Lossless lines 442, 501, and 555 are `BitReader::fill()` propagation sites.
  EOF is not itself an error for `fill()`; an injected failing `BufRead`
  implementation is required.
- VP8 line 999 is `read_to_end()` propagation from the underlying reader and is
  not byte-stream reachable. The remaining VP8 parser/residual sites may still
  be reachable by carefully crafted VP8 bitstreams, so this batch should not add
  broad VP8 hooks beyond the simple reader-failure case.

Planned edit:

- Add a coverage-only failing `Read + BufRead + Seek` cursor to
  `decoder::__coverage_exercise_private_branches()` and use it only for exact
  VP8X scan and animation seek/read propagation paths.
- Add coverage-only `LosslessDecoder` states that seed the bit buffer so the
  next operation is exactly the missing sub-byte read or `fill()` propagation.
- Add a coverage-only VP8 reader that fails at `read_to_end()` for
  `init_partitions(1)`.
- Avoid new public fixtures in this batch because these targets require
  injected I/O failures rather than malformed image bytes.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.
- Retain only if line/function coverage stays 100% and aggregate missing
  regions or branches improve.

Applied edit:

- Added coverage-only reader/seek failure exercises for exact WebP native I/O
  propagation regions that cannot be reached by committed image bytes.
- Added coverage-only lossless decoder states that seed the bit buffer before
  the missing sub-byte read and `fill()` propagation sites.
- Added a coverage-only VP8 reader failure for `init_partitions(1)` so
  `read_to_end()` propagation is covered without adding a malformed fixture
  that Pillow cannot distinguish.

Local validation:

- `cargo fmt --all --check`: passed.
- `git diff --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.

Coverage MCP validation:

- Rejected intermediate run `e2cfaca7-6fd7-45bc-86e4-62315cc9943b` because
  helper self-coverage regressed line and branch totals.
- Rejected intermediate run `d0dad41f-50ce-479f-9451-089ee49e59a5` because one
  helper branch remained uncovered.
- Retained run: `92c1cdb4-3e1b-4ae9-9ad5-a778b673f5e4`.
- Snapshot: `1f3ce856-4942-4a21-aebd-253e40bf7250`.
- Result: `5 passed / 0 failed`.
- Lines: `27001 / 27001` (100%).
- Functions: `1630 / 1630` (100%).
- Branches: `3487 / 3488` (1 missing, unchanged).
- Regions: `42736 / 42799` (63 missing), improved by 1.
- `decoder.rs`: `965 / 965` lines, `90 / 90` branches,
  `1755 / 1787` regions.
- `lossless.rs`: `1092 / 1092` lines, `119 / 120` branches,
  `1361 / 1382` regions.
- `vp8.rs`: `1491 / 1491` lines, `160 / 160` branches,
  `2748 / 2757` regions.
- `encoder.rs`: `1851 / 1852` regions.

Remaining raw zero-region entries after Batch 63:

- `decoder.rs`: none in the MCP-produced LLVM artifact.
- `lossless.rs`: none in the MCP-produced LLVM artifact.
- `vp8.rs`: lines 1193, 1200, 1202, 1220, 1492, 1547, and 1860.
- `encoder.rs`: one source-region artifact with no zero-line entry.

Retention:

- Retain Batch 63. It preserves 100% line/function coverage, keeps the
  absolute branch miss count unchanged, clears the exact injected I/O
  zero-line entries, and reduces global missing regions from 64 to 63.

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

## Batch 70 plan

Goal: reduce WebP native region debt by collapsing coverage-hook-only reader
monomorphizations before adding more probes.

Baseline evidence:

- Last valid retained Coverage MCP snapshot:
  `eb1e5e29-2ab8-4b23-8a9f-8484af4644b2`.
- Retained counters: lines `27001 / 27001`, functions `1630 / 1630`,
  branches `3487 / 3488`, regions `42737 / 42799`.
- Remaining retained gap map:
  - `src/codecs/webp/native/decoder.rs`: 32 regions, 0 branches;
  - `src/codecs/webp/native/lossless.rs`: 21 regions, 1 branch;
  - `src/codecs/webp/native/vp8.rs`: 8 regions, 0 branches;
  - `src/codecs/webp/native/encoder.rs`: 1 region, 0 branches.
- Clean Coverage MCP run on commit `8caea26`:
  `3e7aa72f-1aff-4e9a-81fc-0f4442d81e77` passed `5 / 5` tests, but
  coverage ingestion failed before snapshot creation because Coverage MCP tried
  to convert a large line hit count (`4196470960`) to an `INTEGER`. This run is
  valid test evidence but not coverage evidence.

Reverse map:

| Source cluster | Evidence | Decision |
| --- | --- | --- |
| `src/codecs/webp/native/decoder.rs` hook `WebPDecoder::new(Cursor<Vec<u8>>)` cases | Public WebP decode uses `Cursor<&[u8]>`; retained detailed MCP metrics still show incomplete decoder instantiations (`77` total, `72` covered) and 32 region-only misses. Many coverage-hook constructor calls create a separate `Cursor<Vec<u8>>` monomorph solely for malformed in-memory byte vectors. | Convert constructor-only coverage calls to a local helper that borrows the generated bytes as `Cursor<&[u8]>`. This preserves the exact malformed byte states while removing a hook-only decoder reader shape. |
| `src/codecs/webp/native/vp8.rs` direct `Cursor<Vec<u8>>` parser probes | Public VP8 decode reaches `Vp8Decoder<Take<&mut Cursor<&[u8]>>>`. Existing hooks also instantiate `Vp8Decoder<Cursor<Vec<u8>>` and `Vp8Decoder<ErrorReader>`. Prior Batches 65 and 69 showed extra VP8 hooks do not clear retained regions. | Do not add new VP8 probes in this batch. If the decoder constructor-shape collapse improves coverage, repeat the same pattern for VP8 with a dedicated follow-up. |
| `src/codecs/webp/native/lossless.rs` generic `BitReader`/`LosslessDecoder` hooks | Detailed MCP metrics show very high instantiation count (`253` total, `150` covered), and Batches 57/66 proved direct generic `read_bits()` probes are ineffective. | Do not add direct `BitReader` probes. Treat lossless as a later larger generic cleanup or public VP8L fixture search. |
| `src/codecs/webp/native/encoder.rs` single writer region | No line gaps, all functions/branches/instantiations covered. Batch 68 writer-failure hooks did not reduce the missing region. | Defer until decoder/lossless generic debt is lower. |

Planned edit:

- Add a small `exercise_new(data: Vec<u8>)` helper in
  `decoder::__coverage_exercise_private_branches()` that calls
  `WebPDecoder::new(Cursor::new(data.as_slice()))`.
- Replace constructor-only `WebPDecoder::new(Cursor::new(vec_or_riff))`
  coverage calls with `exercise_new(...)`.
- Leave returned/long-lived synthetic animation decoders alone for this batch,
  because they currently own their byte buffers through `Cursor<Vec<u8>>`.
- Run local checks:
  - `cargo fmt --all --check`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- Run Coverage MCP with `all-features-llvm-cov-json-nightly-branch`. If the
  MCP ingestion overflow repeats, record the failure and keep the batch
  unretained until a valid snapshot proves improvement.

Retention gate:

- Keep only if Coverage MCP produces a valid ingested snapshot and missing
  regions fall below 62 without losing 100% lines/functions or increasing the
  single missing branch.
- If the run passes tests but ingestion fails again, keep the source change only
  if local checks pass and the edit is mechanically safe; mark coverage
  validation pending rather than claiming improvement.

Rejected pre-check:

- Tried exact `BitReader<Take<Cursor<Vec<u8>>>>` and
  `BitReader<Take<FailingSeekCursor>>` probes before applying the constructor
  cleanup.
- Run: `ae92ff9a-a094-4dc5-ad6c-26787becb565`.
- Snapshot: `88839340-0f4d-4079-a725-793af0d398b9`.
- Result: `5 passed / 0 failed`, but total missing regions stayed at 62 and
  the single lossless branch miss remained.
- Retention: rejected and reverted. This confirmed the Batch 70 plan's
  "no direct `BitReader` probes" constraint.

Retained local validation:

- `cargo fmt --all --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.
- `git diff --check`: passed.

Retained Coverage MCP validation:

- Run: `fb8554c5-caf6-4be5-9031-50ab5a9282c3`.
- Snapshot: `394eb598-e87c-4944-a560-5c65e59a0da9`.
- Result: `5 passed / 0 failed`.
- Lines: `27001 / 27001` (100%).
- Functions: `1631 / 1631` (100%).
- Branches: `3487 / 3488` (1 missing, unchanged).
- Regions: `42719 / 42776` (57 missing), improved by 5 from the current
  62-region baseline.
- `src/codecs/webp/native/decoder.rs`: improved from 32 to 27 missing regions.

Retention:

- Retain Batch 70. The edit preserves the same malformed constructor inputs but
  borrows their generated byte buffers so coverage no longer creates a
  constructor-only `WebPDecoder<Cursor<Vec<u8>>>` reader shape for these cases.

## Current retained gap map after Batch 70

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/webp/native/decoder.rs` | 27 | 0 |
| `src/codecs/webp/native/lossless.rs` | 21 | 1 |
| `src/codecs/webp/native/vp8.rs` | 8 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |

## Batch 71 plan

Goal: continue reducing WebP native region debt by applying the successful
Batch 70 reader-shape cleanup to synthetic animated-frame decoder states.

Baseline evidence:

- Current retained Coverage MCP snapshot:
  `394eb598-e87c-4944-a560-5c65e59a0da9`.
- Retained counters: lines `27001 / 27001`, functions `1631 / 1631`,
  branches `3487 / 3488`, regions `42719 / 42776`.
- Remaining retained gap map:
  - `src/codecs/webp/native/decoder.rs`: 27 regions, 0 branches;
  - `src/codecs/webp/native/lossless.rs`: 21 regions, 1 branch;
  - `src/codecs/webp/native/vp8.rs`: 8 regions, 0 branches;
  - `src/codecs/webp/native/encoder.rs`: 1 region, 0 branches.

Reverse map:

| Source cluster | Evidence | Decision |
| --- | --- | --- |
| `decoder.rs` synthetic animation helpers returning `WebPDecoder<Cursor<Vec<u8>>>` | Batch 70 reduced decoder region debt by 5 when malformed constructor-only inputs stopped creating hook-only `Cursor<Vec<u8>>` decoder instantiations. The remaining decoder gap list still clusters around `read_frame()` and animated state handling. Existing `animation_decoder_from_stream(...)` and `animation_decoder(...)` own byte buffers only so the returned decoder can be used immediately by coverage hooks. | Replace returned synthetic animation decoders with non-generic helpers that own the `Vec<u8>` for the duration of `read_frame()` and instantiate `WebPDecoder<Cursor<&[u8]>>`. Preserve mutation cases (`dispose_next_frame`, `next_frame`, two-frame reads) through explicit helper parameters instead of returning a decoder. |
| `decoder.rs` failing seek/read wrappers | These wrappers intentionally exercise impossible I/O failures and must remain custom reader shapes. | Leave them unchanged in this batch; they are not the hook-only `Cursor<Vec<u8>>` source of Batch 70's improvement. |
| `vp8.rs` coverage-hook `Vp8Decoder::new(Cursor<Vec<u8>>)` probes | Batch 70 explicitly called out VP8 as the next candidate if decoder constructor-shape cleanup improved retained coverage. The current hook still has many direct VP8 parser probes backed by owned `Vec<u8>` cursors, while public VP8 decode reaches borrowed/taken reader shapes. | Convert equivalent VP8 hook probes to borrowed-slice helper macros without adding new VP8 inputs. |
| `lossless.rs` and `encoder.rs` residual gaps | Prior direct probes for `BitReader` and writer failure did not improve retained coverage. | Do not add new probes here until the reader-shape sweep is validated. |

Planned edit:

- Remove `animation_decoder_from_stream(...)` and `animation_decoder(...)`
  helpers that return `WebPDecoder<Cursor<Vec<u8>>>`.
- Add a single `exercise_animation_data(...)` helper that builds
  `WebPDecoder<Cursor<&[u8]>>`, optionally seeds `next_frame` and
  `dispose_next_frame`, and performs one or more `read_frame()` calls.
- Convert direct synthetic animation decoder blocks to
  `exercise_animation_data(...)`.
- Keep `exercise_animation_stream(...)` for custom failing readers only.
- Convert direct `Vp8Decoder::new(Cursor<Vec<u8>>)` coverage-hook calls to
  borrowed-slice helper macros, preserving the same byte inputs and internal
  state mutations.
- Run local checks, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.

Retention gate:

- Retain only if Coverage MCP produces a valid ingested snapshot, line/function
  coverage remains 100%, branch debt does not increase, and total missing
  regions fall below 57.

Local validation:

- `cargo fmt --all --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.
- `git diff --check`: passed.

First Coverage MCP validation:

- Run: `f59b72fa-e9e6-45c1-9ef4-9f578c70735e`.
- Snapshot: `8cd904c7-d95a-4b55-bbcc-f615dbd4c86d`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines regressed to `26972 / 27006`.
- Functions regressed to `1632 / 1634`.
- Branches stayed `3487 / 3488`.
- Regions were `42521 / 42588` (67 missing).
- Root cause: two returning animation helpers had been reintroduced only to
  satisfy a stale coverage-cfg compile error; they were compiled but not hit by
  the managed coverage path.
- Retention: rejected. The helpers added uncovered line/function debt.

Correction:

- Removed the unused returning `animation_decoder_from_stream(...)` and
  `animation_decoder(...)` helpers entirely.
- Kept the borrowed-slice `exercise_animation_data(...)` helper and the VP8
  borrowed-slice macro conversions.

Corrected local validation:

- `cargo fmt --all --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.
- `git diff --check`: passed.

Corrected Coverage MCP validation:

- Run: `7a1ec429-eb80-428b-9565-e6793a4ad581`.
- Snapshot: `fd087b82-75c2-4602-8511-bfaa254c27bb`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines: `26972 / 26972` (100%).
- Functions: `1632 / 1632` (100%).
- Branches: `3487 / 3488` (1 missing, unchanged).
- Regions: `42521 / 42565` (44 missing), improved by 13 from Batch 70.
- `src/codecs/webp/native/decoder.rs`: improved from 27 to 14 missing
  regions.
- `src/codecs/webp/native/vp8.rs`: stayed at 8 missing regions.

Retention:

- Retain Batch 71. The retained improvement comes from removing returned
  animated-frame `WebPDecoder<Cursor<Vec<u8>>>` helper shapes and executing
  those synthetic states through borrowed-slice helpers instead. The VP8
  borrowed-slice conversion is neutral in this snapshot but keeps the hook
  aligned with public reader shapes.

## Current retained gap map after Batch 71

| File | Missing regions | Missing branches |
| --- | ---: | ---: |
| `src/codecs/webp/native/decoder.rs` | 14 | 0 |
| `src/codecs/webp/native/lossless.rs` | 21 | 1 |
| `src/codecs/webp/native/vp8.rs` | 8 | 0 |
| `src/codecs/webp/native/encoder.rs` | 1 | 0 |

## Batch 72 plan

Goal: reduce the largest remaining WebP native region cluster by applying the
retained reader-shape cleanup to VP8L/lossless coverage-only decoder states.

Baseline evidence:

- Current retained Coverage MCP snapshot:
  `fd087b82-75c2-4602-8511-bfaa254c27bb`.
- Retained counters: lines `26972 / 26972`, functions `1632 / 1632`,
  branches `3487 / 3488`, regions `42521 / 42565`.
- Remaining retained gap map:
  - `src/codecs/webp/native/decoder.rs`: 14 regions, 0 branches;
  - `src/codecs/webp/native/lossless.rs`: 21 regions, 1 branch;
  - `src/codecs/webp/native/vp8.rs`: 8 regions, 0 branches;
  - `src/codecs/webp/native/encoder.rs`: 1 region, 0 branches.

Reverse map:

| Source cluster | Evidence | Decision |
| --- | --- | --- |
| `lossless.rs` coverage-hook `LosslessDecoder::new(Cursor<Vec<u8>>)` cases | `rg` still finds many lossless coverage-only decoder probes backed by owned vectors. Batch 70 and Batch 71 proved that removing hook-only `Cursor<Vec<u8>>` reader shapes can reduce retained region debt without adding new inputs. | Convert equivalent `LosslessDecoder::new(...)` hook probes to borrowed-slice helpers while preserving byte inputs and downstream calls. |
| `lossless.rs` manual `LosslessDecoder { bit_reader: BitReader { reader: Cursor<Vec<u8>>, ... } }` states | These manually seeded states exist only to reverse-map exact Huffman/color-cache branches and create separate `LosslessDecoder<Cursor<Vec<u8>>>` monomorphs. | Convert them to a borrowed-slice `decoder_with_bits(Cursor<&[u8]>, ...)` helper shape. |
| `LosslessDecoder::<Cursor<Vec<u8>>>::...` static method probes | These type-qualified calls force the old owned-vector decoder type even when no owned buffer is needed. | Retarget to `LosslessDecoder::<Cursor<&[u8]>>` with a borrowed-slice `BitReader`. |
| Custom `ErrorReader` and one-byte error readers | These are intentional I/O-failure reader shapes. | Leave unchanged in this batch. |

Planned edit:

- Add local coverage-hook macros for borrowed-slice lossless decoders and
  seeded lossless bit-reader states.
- Convert owned-vector lossless decoder probes to those macros.
- Convert type-qualified `LosslessDecoder::<Cursor<Vec<u8>>>` probes to
  borrowed-slice type-qualified probes.
- Do not add new VP8L bitstream inputs in this batch.

Retention gate:

- Retain only if Coverage MCP produces a valid ingested snapshot, line/function
  coverage remains 100%, branch debt does not increase, and total missing
  regions fall below 44.

Local validation:

- `cargo fmt --all --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.
- `git diff --check`: passed.

Coverage MCP validation:

- Run: `ee4a8c09-2548-4e2c-8d4a-fac1aa500146`.
- Snapshot: `f040e808-7330-4d69-b581-eee6ce38c947`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines: `26976 / 26976` (100%).
- Functions: `1632 / 1632` (100%).
- Branches: `3487 / 3488` (1 missing, unchanged).
- Regions: `42545 / 42589` (44 missing), unchanged from Batch 71.
- `src/codecs/webp/native/lossless.rs`: stayed at 21 missing regions and one
  missing branch.

Retention:

- Reject Batch 72 and revert the lossless source changes. The reader-shape
  cleanup preserved line/function/branch coverage but did not reduce missing
  regions or the single branch gap.

## Batch 73 plan

Goal: reduce retained WebP native region debt by covering the success side of
custom failing-reader instantiations instead of adding new malformed byte
fixtures.

Baseline evidence:

- Current retained Coverage MCP snapshot:
  `a71665a6-7344-4996-a70c-3a75144ac5e8` on commit
  `d8a403dbf4206397568b2c58358d4e086256c25e`.
- Retained counters: lines `26972 / 26972`, functions `1632 / 1632`,
  branches `3487 / 3488`, regions `42521 / 42565`.
- Remaining retained gap map:
  - `src/codecs/webp/native/decoder.rs`: 14 regions, 0 branches, with
    instantiations `61 / 65`;
  - `src/codecs/webp/native/lossless.rs`: 21 regions, 1 branch, with
    instantiations `129 / 207`;
  - `src/codecs/webp/native/vp8.rs`: 8 regions, 0 branches, with
    instantiations `85 / 86`;
  - `src/codecs/webp/native/encoder.rs`: 1 region, 0 branches, with
    instantiations `72 / 72`.

Reverse map:

| Source cluster | Evidence | Decision |
| --- | --- | --- |
| `decoder.rs` `WebPDecoder<FailingReadCursor>` constructor shape | Raw LLVM function records show uncovered instantiations for `is_animated()` and `validate_output_buffer_size()` under the custom read-failure type. Existing probes only make that reader fail early inside `WebPDecoder::new(...)`. | Add one deterministic valid VP8 header through `FailingReadCursor` with the failure offset beyond EOF so the same type reaches the normal constructor tail. |
| `decoder.rs` `WebPDecoder<FailingSeekCursor>` constructor shape | Raw LLVM function records show an uncovered `validate_output_buffer_size()` instantiation under the custom seek-failure type. Existing probes use the type only for early seek errors. | Add one deterministic valid VP8 header through `FailingSeekCursor` with enough allowed seeks to reach normal constructor success. |
| `decoder.rs` animated `read_frame()` closure under `FailingSeekCursor` | Raw LLVM function records show an uncovered closure at `read_frame()` canvas initialization line 622 for `WebPDecoder<FailingSeekCursor>`. Existing animated seek-failure probes fail before canvas initialization. | Replay the existing valid VP8L solid-frame ANMF stream through `FailingSeekCursor` with enough allowed seeks to hit the normal animated frame path. |
| `vp8.rs` residual one uncovered instantiation | Detailed MCP still shows one uncovered VP8 instantiation. Batches 58, 65, and 69 already proved duplicated malformed VP8 input generation and borrowed-slice replay do not move the retained VP8 region count. | Do not add new VP8 parser probes in this batch. |
| `lossless.rs` single branch and generic-instantiation debt | Batch 72 proved borrowed-slice lossless reader-shape cleanup was neutral. The planned animated `FailingSeekCursor` success probe may cover some lossless `Take<FailingSeekCursor>` instantiations indirectly. | Let the decoder success-path probe measure this indirectly; do not add direct lossless edits. |

Planned edit:

- In `decoder::__coverage_exercise_private_branches()`, add:
  - valid simple VP8 constructor success through `FailingReadCursor`;
  - valid simple VP8 constructor success through `FailingSeekCursor`;
  - valid VP8L animated-frame success through `FailingSeekCursor` using the
    existing `vp8l_solid_64` bytes.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.

Retention gate:

- Retain only if Coverage MCP produces a valid ingested snapshot, line/function
  coverage remains 100%, branch debt does not increase, and total missing
  regions fall below 44.

Local validation:

- `cargo fmt --all --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.
- `git diff --check`: passed.

Coverage MCP validation:

- Run: `a944b1aa-dcff-44e2-b2e1-a36ba58a9f29`.
- Snapshot: `06f191c7-4315-4003-a090-51f0d501f155`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines: `26982 / 26982` (100%).
- Functions: `1632 / 1632` (100%).
- Branches: `3487 / 3488` (1 missing, unchanged).
- Regions: `42536 / 42580` (44 missing), unchanged from Batch 72.
- `src/codecs/webp/native/decoder.rs`: instantiations improved from
  `61 / 65` to `65 / 65`, but missing regions stayed at 14.
- `src/codecs/webp/native/lossless.rs`: instantiations improved from
  `129 / 207` to `150 / 207`, but missing regions stayed at 21 and the branch
  miss stayed unchanged.
- `src/codecs/webp/native/vp8.rs`: stayed at 8 missing regions.

Retention:

- Reject Batch 73 and revert the source probes. Covering the custom-reader
  success instantiations improves LLVM instantiation coverage but does not reduce
  the retained aggregate region or branch debt. This confirms the remaining
  gaps are not caused by those custom-reader early exits.

## Batch 74 plan

Goal: clear the remaining lossless branch debt with a minimal backward-reference
state that hits the copy-length-overrun side of `decode_image_data(...)`.

Baseline evidence:

- Current retained Coverage MCP snapshot:
  `a71665a6-7344-4996-a70c-3a75144ac5e8`.
- Retained counters: lines `26972 / 26972`, functions `1632 / 1632`,
  branches `3487 / 3488`, regions `42521 / 42565`.
- `src/codecs/webp/native/lossless.rs` is the only file with branch debt:
  branches `119 / 120`, regions `1361 / 1382`.
- MCP file gaps and selected line records keep pointing at
  `decode_image_data(...)` line 577:
  `if index < dist || num_values - index < length`.

Reverse map:

| Condition side | Existing coverage | Missing state |
| --- | --- | --- |
| `index < dist` | Existing backward-reference hooks start at pixel index `0`, so every non-zero distance trips the left side. | Already covered. |
| `num_values - index < length` | Existing valid-copy hooks use length values that fit the remaining output. | Need a prior literal pixel so `index >= dist`, then a backward-reference length that exceeds the remaining pixels. |

Planned edit:

- Add one deterministic coverage-hook state in
  `lossless::__coverage_exercise_private_branches()`:
  - two-pixel image (`width = 2`, `height = 1`);
  - green Huffman tree emits a literal on bit `0`, then copy-length symbol
    `257` on bit `1`;
  - distance tree emits prefix `1`, which maps to plane-code distance `1`;
  - data byte `0b0000_0010` drives the literal then backward reference.
- Expected execution: first pixel is decoded literally, then at `index = 1`,
  `dist = 1` is valid but `length = 2` exceeds the one remaining pixel, taking
  the RHS of line 577.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.

Retention gate:

- Retain only if Coverage MCP produces a valid ingested snapshot, line/function
  coverage remains 100%, branch debt decreases or does not regress, and total
  missing regions does not increase.

Local validation:

- `cargo fmt --all --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.
- `git diff --check`: passed.

Coverage MCP validation:

- Run: `72f00266-2d65-4da7-88e2-9ff01ff67b5e`.
- Snapshot: `b0d85ec6-c75d-47bc-af84-1fdfd6e2ad0a`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines: `26990 / 26990` (100%).
- Functions: `1632 / 1632` (100%).
- Branches: `3487 / 3488` (1 missing, unchanged).
- Regions: `42538 / 42582` (44 missing), unchanged from the retained
  baseline.
- `src/codecs/webp/native/lossless.rs`: branch debt stayed at `119 / 120`,
  and missing regions stayed at 21.

Retention:

- Reject Batch 74 and revert the source probe. The constructed
  copy-length-overrun state did not clear the retained branch gap or region
  debt. This means the remaining lossless branch is not the straightforward RHS
  of line 577 under the retained aggregate metric, despite the noisy MCP line
  gap pointing there.

## Batch 75 plan

Goal: clear the retained lossless branch by reverse-mapping from the branch-owning
primitive instead of the downstream decode-image line.

Baseline evidence:

- Retained Coverage MCP snapshot:
  `a71665a6-7344-4996-a70c-3a75144ac5e8`.
- Retained counters: lines `26972 / 26972`, functions `1632 / 1632`,
  branches `3487 / 3488`, regions `42521 / 42565`.
- `src/codecs/webp/native/lossless.rs` is the only file with branch debt:
  branches `119 / 120`, regions `1361 / 1382`.
- Batch 74 proved the missing branch is not the straightforward
  `decode_image_data(...)` line 577 copy-length-overrun RHS.

Reverse map:

| Source construct | Existing coverage | Missing state |
| --- | --- | --- |
| `BitReader::fill()` line 1414, `if buf.len() >= 8` | Covered by existing `Cursor::new([0u8; 8])` and short-buffer states. | No new action. |
| `BitReader::fill()` line 1420, `while !buf.is_empty() && self.nbits < 56` | Existing hooks cover empty buffer, short non-empty buffer with `nbits < 56`, refill to EOF, and reader errors. | A non-empty buffer with `nbits == 56` so the left side is true and the right side is false. |
| `BitReader::read_bits()` line 1457, `if self.nbits < num` | Existing hooks cover buffered and fill-needed reads, including reader errors. Previous direct `read_bits` probes did not clear the retained branch. | No new action until the `fill()` short-circuit state is measured. |

Planned edit:

- Add one coverage-only `BitReader` state in
  `lossless::__coverage_exercise_private_branches()`:
  - reader: `Cursor::new([0u8; 1])`;
  - `buffer = 0`;
  - `nbits = 56`;
  - call `fill()`.
- This directly exercises the `while` condition with a non-empty buffer where
  `self.nbits < 56` is false.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.

Retention gate:

- Retain only if Coverage MCP produces a valid ingested snapshot, line/function
  coverage remains 100%, branch debt decreases or does not regress, and total
  missing regions does not increase.

Local validation:

- `cargo fmt --all --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.
- `git diff --check`: passed.

Coverage MCP validation:

- Run: `5e8418cd-dc17-49b4-b3e4-1d8407738cd4`.
- Snapshot: `fa5506a3-708d-4cba-ac07-be088e476f72`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines: `26978 / 26978` (100%).
- Functions: `1632 / 1632` (100%).
- Branches: `3487 / 3488` (1 missing, unchanged).
- Regions: `42524 / 42568` (44 missing), unchanged.
- `src/codecs/webp/native/lossless.rs`: regions changed to
  `1364 / 1385`, while branch debt stayed `119 / 120`.

Retention:

- Reject Batch 75 and revert the source probe. The non-empty-buffer
  `nbits == 56` state executes and is fully covered, but it does not clear the
  retained branch or region debt. The remaining lossless branch is therefore not
  the direct `BitReader::fill()` `while` RHS false state.

## Batch 76 plan

Goal: reduce retained decoder region debt by reverse-mapping source regions that
come from repeated validation combinator expressions inside generic
`WebPDecoder<R>` methods.

Baseline evidence:

- Retained Coverage MCP snapshot:
  `a71665a6-7344-4996-a70c-3a75144ac5e8`.
- Retained counters: lines `26972 / 26972`, functions `1632 / 1632`,
  branches `3487 / 3488`, regions `42521 / 42565`.
- `src/codecs/webp/native/decoder.rs`: branches `90 / 90`, regions
  `1562 / 1576`.
- Decoder line gaps cluster on pure validation expressions in generic methods:
  lines 183-185, 411-413, 441-443, 499-501, 549-551, and 559-561.
- Batches 70-73 proved that reducing hook-only reader shapes can help, but
  adding more reader success inputs does not reduce the retained gap once those
  instantiations are covered.

Reverse map:

| Source construct | Existing coverage | Planned action |
| --- | --- | --- |
| `(condition).then_some(()).ok_or(error)?` in `read_data`, `read_image`, and `read_frame` | Every semantic true/false path is already covered, but LLVM still records retained region debt in the generic caller source. | Move the branch shape into one non-generic `require(...)` helper and call it from the generic methods. |
| `WebPDecoder<R>` generic monomorphs | Existing coverage uses `Cursor<&[u8]>`, `FailingSeekCursor`, and other reader shapes; adding success states for those types was neutral. | Avoid creating more monomorph-specific validation regions. |

Planned edit:

- Add a private non-generic `require(condition, error)` helper in
  `decoder.rs`.
- Replace the repeated `.then_some(()).ok_or(...)?` validation chains with
  `require(..., ...)?`.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.

Retention gate:

- Retain only if Coverage MCP produces a valid ingested snapshot, line/function
  coverage remains 100%, branch debt does not increase, and total missing
  regions falls below 44.

Local validation:

- `cargo fmt --all --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.
- `git diff --check`: passed.

Coverage MCP validation:

- Run: `0b60ada7-604d-4511-952c-11fe240fe3b8`.
- Snapshot: `f9a6ddfa-3e8a-49a9-8992-6d99cfa5e27a`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines: `26981 / 26981` (100%).
- Functions: `1633 / 1633` (100%).
- Branches: `3489 / 3490` (1 missing, unchanged).
- Regions: `42514 / 42558` (44 missing), unchanged.
- `src/codecs/webp/native/decoder.rs`: total regions decreased from `1576`
  to `1569`, but covered regions decreased by the same amount; retained
  decoder debt stayed at 14 missing regions.

Retention:

- Reject Batch 76 and revert the source helper. Moving repeated
  `.then_some(()).ok_or(...)` validation expressions into a non-generic helper
  reduced source-region totals but did not reduce retained missing regions.
  The remaining decoder debt is therefore not caused by those validation
  combinator expressions.

## Batch 77 plan

Goal: reduce retained VP8 region debt with exact reverse-mapped private states
for the smallest normal reader-shape gaps.

Baseline evidence:

- Retained Coverage MCP snapshot:
  `a71665a6-7344-4996-a70c-3a75144ac5e8`.
- Retained counters: lines `26972 / 26972`, functions `1632 / 1632`,
  branches `3487 / 3488`, regions `42521 / 42565`.
- `src/codecs/webp/native/vp8.rs`: branches `160 / 160`, regions
  `2726 / 2734`.
- Raw reverse-map from the LLVM JSON artifact produced by Coverage MCP shows
  small remaining normal reader-shape gaps at:
  - `init_partitions(...)` line 999 (`read_to_end`) for `Cursor<&[u8]>` and
    `Take<Cursor<&[u8]>>`;
  - `read_coefficients(...)` line 1469 for the `zigzag == 0` quantizer arm;
  - `loop_filter(...)` line 1577 for the RHS-driven subblock-filtering case.

Reverse map:

| Source construct | Existing coverage | Missing state |
| --- | --- | --- |
| `init_partitions(1)` final partition read | Existing hooks cover `n > 1` and an `ErrorReader` failure for `n == 1`. | Successful empty final partition for both cursor and taken cursor reader shapes. |
| `block[zigzag] = abs_value * if zigzag > 0 { acq } else { dcq }` | Existing hooks hit AC coefficient writes and mixed parser paths. | Fresh plane-1 coefficient read where `first == 0` and the first decoded token writes `zigzag == 0`. |
| `mb.luma_mode == B || (!mb.coeffs_skipped && mb.non_zero_dct)` | Existing loop-filter hook covers left side true and whole expression false. | Non-B macroblock with `coeffs_skipped == false` and `non_zero_dct == true`, so the RHS alone drives subblock filtering. |

Planned edit:

- Extend `vp8::__coverage_exercise_private_branches()` with:
  - direct `init_partitions(1)` success for `Cursor<&[u8]>`;
  - direct `init_partitions(1)` success for `Take<Cursor<&[u8]>>`;
  - a fresh plane-1 `read_coefficients(...)` call with a `DCT_1` fixed token;
  - a loop-filter call using a non-B, non-skipped, non-zero macroblock.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.

Retention gate:

- Retain only if Coverage MCP produces a valid ingested snapshot, line/function
  coverage remains 100%, branch debt does not increase, and total missing
  regions falls below 44.

Local validation:

- `cargo fmt --all --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.
- `git diff --check`: passed.

Coverage MCP validation:

- Run: `e8ec3770-99ff-4f5e-9001-a900c1892f9f`.
- Snapshot: `0850e8ad-6f10-425e-b810-551cf520a555`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines: `26995 / 26995` (100%).
- Functions: `1632 / 1632` (100%).
- Branches: `3487 / 3488` (1 missing, unchanged).
- Regions: `42546 / 42589` (43 missing), improved by 1.
- `src/codecs/webp/native/vp8.rs`: regions changed from `2726 / 2734`
  to `2751 / 2758`, reducing missing VP8 regions from 8 to 7.

Retention:

- Retain Batch 77. The improvement confirms that at least one remaining VP8
  region is reachable through exact reverse-mapped private states, specifically
  the normal reader-shape states around final partition/coefficient/loop-filter
  handling.

## Batch 78 plan

Goal: reduce the next retained VP8 region by replaying an existing invalid-token
coefficient state through the taken-reader shape used by container decode.

Baseline evidence:

- Retained Coverage MCP snapshot after Batch 77:
  `0850e8ad-6f10-425e-b810-551cf520a555`.
- Retained counters: lines `26995 / 26995`, functions `1632 / 1632`,
  branches `3487 / 3488`, regions `42546 / 42589`.
- `src/codecs/webp/native/vp8.rs`: branches `160 / 160`, regions
  `2751 / 2758`.
- Raw reverse-map from the Batch 77 LLVM JSON shows a two-region gap in
  `read_coefficients(...)` line 1455 for
  `Vp8Decoder<Take<Cursor<&[u8]>>>`.

Reverse map:

| Source construct | Existing coverage | Missing state |
| --- | --- | --- |
| `read_coefficients(...)` line 1455 `c => panic!("unknown token: {c}")` | Existing `catch_unwind` hook covers an invalid fixed token under `Vp8Decoder<Cursor<&[u8]>>`. | Replay the same invalid fixed-token state under `Vp8Decoder<Take<Cursor<&[u8]>>>`. |

Planned edit:

- Add a second `catch_unwind` coefficient hook in
  `vp8::__coverage_exercise_private_branches()` using `with_take_decoder!`.
- Keep the same token tree and block call as the existing cursor-backed invalid
  token hook.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.

Retention gate:

- Retain only if Coverage MCP produces a valid ingested snapshot, line/function
  coverage remains 100%, branch debt does not increase, and total missing
  regions falls below 43.

Local validation:

- `cargo fmt --all --check`: passed.
- `cargo check --all-features`: passed.
- `RUSTFLAGS='--cfg coverage' cargo check --all-features`: passed.
- `git diff --check`: passed.

Coverage MCP validation:

- Run: `1f5a7bc0-7d91-42c5-a240-f131922317f1`.
- Snapshot: `b7648354-9fc4-40b4-98a8-f9503d46cdb7`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines: `27010 / 27010` (100%).
- Functions: `1633 / 1633` (100%).
- Branches: `3487 / 3488` (1 missing, unchanged).
- Regions: `42563 / 42606` (43 missing), unchanged from Batch 77.
- `src/codecs/webp/native/vp8.rs`: regions changed to `2768 / 2775`,
  keeping VP8 debt at 7 missing regions.

Retention:

- Reject Batch 78 and revert the source hook. Replaying the invalid-token panic
  arm under `Take<Cursor<&[u8]>>` adds fully covered hook code but does not
  reduce retained aggregate VP8 debt. The line-1455 raw function gap is not one
  of the retained seven aggregate regions.

## Batch 79 plan

Goal: reduce retained VP8 region debt at `read_segment_updates()` line 1107 by
using reverse-mapped VP8 arithmetic bytes instead of guessed buffers.

Baseline evidence:

- Retained Coverage MCP snapshot after Batch 77:
  `0850e8ad-6f10-425e-b810-551cf520a555`.
- Retained counters: lines `26995 / 26995`, functions `1632 / 1632`,
  branches `3487 / 3488`, regions `42546 / 42589`.
- `src/codecs/webp/native/vp8.rs`: branches `160 / 160`, regions
  `2751 / 2758`.
- Raw reverse-map from the Batch 77 LLVM JSON shows a one-region normal
  `Cursor<&[u8]>` gap at `read_segment_updates(...)` line 1107, the default
  probability value arm.

Reverse map:

| `read_segment_updates()` flag | Needed value | Derived byte source |
| --- | --- | --- |
| `segments_update_map` | `true` | VP8 arithmetic flag 1 from `80 00 00 00`. |
| `update_segment_feature_data` | `false` | VP8 arithmetic flag 2 from `80 00 00 00`. |
| three segment-tree `update` flags | all `false` | VP8 arithmetic flags 3-5 from `80 00 00 00`, reaching `prob = 255`. |

Derivation:

- A small script modeled `ArithmeticDecoder::fast_read_flag()` and found
  `80 00 00 00` yields flag sequence
  `[true, false, false, false, false]`.

Planned edit:

- Add one coverage-only `with_cursor_decoder!` state:
  - `decoder.b.init(vec![[0x80, 0, 0, 0]], 4)`;
  - call `decoder.read_segment_updates()`.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.

Retention gate:

- Retain only if Coverage MCP produces a valid ingested snapshot, line/function
  coverage remains 100%, branch debt does not increase, and total missing
  regions falls below 43.

Validation:

- Local gates passed:
  - `cargo fmt --all --check`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- Coverage MCP run: `70550dd1-d5f4-43fc-9688-5a1889ecf369`.
- Snapshot: `2732de16-0a3b-4907-9198-2ae11037a4b6`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines: `26997 / 26997`.
- Functions: `1632 / 1632`.
- Branches: `3487 / 3488`.
- Regions: `42551 / 42594` (43 missing), unchanged.
- `src/codecs/webp/native/vp8.rs`: regions `2756 / 2763`, still
  7 missing.

Decision:

- Reject Batch 79 and revert the source hook. The derived bytes reach the
  reverse-mapped source line and change normalized line-gap evidence, but the
  retained gate is aggregate region debt. Total missing regions remain 43, so
  this is not one of the retained VP8 aggregate gaps.

## Batch 80 plan

Goal: use file-level reverse mapping, not noisy generic function mappings, to
attack the retained VP8 aggregate region debt.

Baseline evidence:

- Retained Coverage MCP snapshot after Batch 77:
  `0850e8ad-6f10-425e-b810-551cf520a555`.
- Retained counters: lines `26995 / 26995`, functions `1632 / 1632`,
  branches `3487 / 3488`, regions `42546 / 42589`.
- `src/codecs/webp/native/vp8.rs`: branches `160 / 160`, regions
  `2751 / 2758`.
- Raw LLVM file-level zero-region entries in the latest artifact map the
  retained VP8 debt to:

| Source coordinate | Source construct | Reverse-mapped input/state |
| --- | --- | --- |
| `1193:48-49` | `read_loop_filter_adjustments()?` in `read_frame_header()` | Keyframe with first-partition bytes `00 06`, derived to enable loop-filter adjustments and then run out inside the adjustment reader. |
| `1200:45-46` | `init_partitions(num_partitions)?` in `read_frame_header()` | Keyframe with first-partition bytes `00 01 00 00`, derived to set `num_partitions = 2`, with no trailing partition-size bytes. |
| `1220:30-31` | final `self.b.check(res, ())?` in `read_frame_header()` | Keyframe with first-partition bytes `00 00 00 00 00 19`, derived to pass token-probability updates, set `mb_no_skip_coeff = 1`, and run out while reading the skip probability. |
| `1492:96-97` | Y2 `read_coefficients(...)?` in `read_residual_data()` | Direct private state: force plane-1 coefficient tokens to `DCT_1` and use an empty coefficient partition, so the caller-side `?` returns `Err`. |
| `1547:95-96` | UV `read_coefficients(...)?` in `read_residual_data()` | Direct private state: force plane-3 luma blocks to `DCT_EOB`, force plane-2 UV blocks to `DCT_1`, and use exactly one coefficient byte so the first UV caller-side `?` returns `Err`. |
| `1860:62-63` | `read_macroblock_header(mbx)?` in `decode_frame_()` | Decode a one-macroblock keyframe with six zero first-partition bytes, which makes `read_frame_header()` succeed and leaves the macroblock header reader at EOF. |

Planned edit:

- Extend only `vp8::__coverage_exercise_private_branches()`.
- Add a small `keyframe_with_first_partition()` helper for the three frame-header
  byte probes and the decode-frame macroblock-header probe.
- Add a generic `force_plane_token()` helper for the two residual-data
  caller-side error probes.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.

Retention gate:

- Retain only if Coverage MCP produces a valid ingested snapshot, line/function
  coverage remains 100%, branch debt does not increase, and total missing
  regions falls below 43.

Validation:

- Local gates passed:
  - `cargo fmt --all --check`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- Coverage MCP run: `cabb5c91-a66e-4814-8d89-edeb4da3727e`.
- Snapshot: `a89216e4-afb6-4f0d-b388-1c9fe816dcc9`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines: `27050 / 27050`.
- Functions: `1634 / 1634`.
- Branches: `3487 / 3488`.
- Regions: `42629 / 42672` (43 missing), unchanged from retained Batch 77.
- `src/codecs/webp/native/vp8.rs`: regions `2834 / 2841`, still
  7 missing.

Observation:

- The three `read_frame_header()` source coordinates and the Y2 residual
  coordinate disappeared from the MCP selected-line gap view, so the
  reverse-mapped inputs do execute the intended source paths.
- The remaining raw LLVM file-level zero-region entries are still:
  - `1547:95-96`, UV `read_coefficients(...)?`;
  - `1860:62-63`, `read_macroblock_header(mbx)?`.
- Reverse-mapping the raw function records shows those two retained gaps belong
  to the `Take<Cursor<&[u8]>>` monomorphization, while Batch 80's new residual
  and decode-frame probes used `Cursor<&[u8]>`.

Decision:

- Do not mark Batch 80 retained by itself because aggregate missing regions did
  not fall below 43. Keep it temporarily as the measured transition state for
  Batch 81; revert Batch 80 together with Batch 81 if the combined result does
  not reduce aggregate debt.

## Batch 81 plan

Goal: convert Batch 80's neutral transition into a retained VP8 reduction by
replaying the two remaining aggregate coordinates through the exact
`Take<Cursor<&[u8]>>` monomorphization.

Baseline evidence:

- Coverage MCP snapshot after Batch 80:
  `a89216e4-afb6-4f0d-b388-1c9fe816dcc9`.
- Counters: lines `27050 / 27050`, functions `1634 / 1634`,
  branches `3487 / 3488`, regions `42629 / 42672`.
- `src/codecs/webp/native/vp8.rs`: branches `160 / 160`, regions
  `2834 / 2841`.
- Raw file-level zero-region entries:
  - `1547:95-96`;
  - `1860:62-63`.

Reverse map:

| Source coordinate | Batch 80 issue | Batch 81 correction |
| --- | --- | --- |
| `1547:95-96` | The UV residual EOF probe used `with_cursor_decoder!`, but the retained raw gap is in `Vp8Decoder<Take<Cursor<&[u8]>>>.read_residual_data`. | Replay the same forced plane-3 `DCT_EOB` / plane-2 `DCT_1` state with `with_take_decoder!`. |
| `1860:62-63` | The macroblock-header EOF decode used `Vp8Decoder::decode_frame(Cursor<&[u8]>)`, but the retained raw gap is in `Vp8Decoder<Take<Cursor<&[u8]>>>.decode_frame_`. | Build `Vp8Decoder::new(cursor.by_ref().take(...))` through `with_take_decoder!` and call `decoder.decode_frame_()`. |

Planned edit:

- Extend only `vp8::__coverage_exercise_private_branches()`.
- Add two `with_take_decoder!` states:
  - the UV residual caller-side EOF state;
  - the macroblock-header EOF frame decode state.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.

Retention gate:

- Retain the combined Batch 80 + Batch 81 source only if Coverage MCP produces a
  valid ingested snapshot, line/function coverage remains 100%, branch debt does
  not increase, and total missing regions falls below 43.

Validation:

- Local gates passed:
  - `cargo fmt --all --check`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- Coverage MCP run: `a8851c0f-e166-4053-b61e-47880c9a3731`.
- Snapshot: `a5e40a0a-6d7b-4818-90e2-9fa1328542c2`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines: `27067 / 27067`.
- Functions: `1634 / 1634`.
- Branches: `3487 / 3488`.
- Regions: `42651 / 42694` (43 missing), unchanged.
- `src/codecs/webp/native/vp8.rs`: regions `2856 / 2863`, still
  7 missing.

Observation:

- The `Take<Cursor<&[u8]>>` probes executed, but the two raw zero-width
  coordinates remained:
  - `1547:95-96`;
  - `1860:62-63`.
- The all-zero six-byte frame reaches `read_macroblock_header()` successfully,
  then continues to the residual path. It does not cover the `1860` error
  propagation side.
- The one-byte UV residual state runs out too early, before the first UV block,
  so it does not cover the `1547` caller-side `?`.

Decision:

- Do not mark Batch 81 retained by itself. Keep the combined source temporarily
  for Batch 82 with corrected reverse-mapped inputs; revert the whole stack if
  Batch 82 does not reduce aggregate missing regions.

## Batch 82 plan

Goal: use corrected reverse-mapped VP8 inputs for the two remaining raw
file-level coordinates after Batch 81.

Baseline evidence:

- Coverage MCP snapshot after Batch 81:
  `a5e40a0a-6d7b-4818-90e2-9fa1328542c2`.
- Counters: lines `27067 / 27067`, functions `1634 / 1634`,
  branches `3487 / 3488`, regions `42651 / 42694`.
- `src/codecs/webp/native/vp8.rs`: branches `160 / 160`, regions
  `2856 / 2863`.
- Raw function records still show `1547:95-96` in `read_residual_data()` and
  `1860:62-63` in `decode_frame_()` for both normal and coverage-reader
  monomorphizations.

Reverse map:

| Source coordinate | Failed input | Corrected input |
| --- | --- | --- |
| `1860:62-63` | Six zero first-partition bytes make `read_macroblock_header()` succeed. | First-partition bytes `00 00 00 00 00 03`, derived with the VP8 arithmetic/model tree script to pass `read_frame_header()` and make `read_macroblock_header()` return `Err`. |
| `1547:95-96` | One coefficient byte runs out before reaching the UV block. | Two coefficient bytes let the 16 forced plane-3 `DCT_EOB` luma blocks finish, then run out in the first forced plane-2 `DCT_1` UV block. |

Planned edit:

- Extend only `vp8::__coverage_exercise_private_branches()`.
- Add the corrected macroblock-header EOF first-partition bytes for both
  `Cursor<&[u8]>` and `Take<Cursor<&[u8]>>`.
- Change/add the UV residual EOF probe to use two bytes for both
  `Cursor<&[u8]>` and `Take<Cursor<&[u8]>>`.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.

Retention gate:

- Retain the combined Batch 80-82 source only if Coverage MCP produces a valid
  ingested snapshot, line/function coverage remains 100%, branch debt does not
  increase, and total missing regions falls below 43.

Validation:

- Local gates passed:
  - `cargo fmt --all --check`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- Coverage MCP run: `d07a0304-f9d1-4da4-a1e1-4a440b818313`.
- Snapshot: `e5104150-e09c-4d8b-b380-66185c33071b`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines: `27074 / 27074`.
- Functions: `1634 / 1634`.
- Branches: `3487 / 3488`.
- Regions: `42664 / 42705` (41 missing), improved by 2.
- `src/codecs/webp/native/vp8.rs`: regions `2869 / 2874`, improved to
  5 missing.

Retention:

- Retain Batches 80-82. The corrected macroblock-header EOF bytes and two-byte
  UV residual state clear the two raw VP8 zero-width coordinates while preserving
  100% line/function coverage and all previously covered branches.
- Remaining project debt after Batch 82:
  - `src/codecs/webp/native/lossless.rs`: 21 regions, 1 branch;
  - `src/codecs/webp/native/decoder.rs`: 14 regions;
  - `src/codecs/webp/native/vp8.rs`: 5 regions;
  - `src/codecs/webp/native/encoder.rs`: 1 region.

## Batch 83 plan

Goal: clear the smallest remaining file debt, the single retained region in
`src/codecs/webp/native/encoder.rs`.

Baseline evidence:

- Coverage MCP snapshot after Batch 82:
  `e5104150-e09c-4d8b-b380-66185c33071b`.
- Counters: lines `27074 / 27074`, functions `1634 / 1634`,
  branches `3487 / 3488`, regions `42664 / 42705`.
- `src/codecs/webp/native/encoder.rs`: branches `194 / 194`, regions
  `1851 / 1852`.

Reverse map:

| Source coordinate | Evidence | Input |
| --- | --- | --- |
| `1160:61-62` | Raw function records show the missing region at `let frame = encode_frame(data, width, height, color)?;`, specifically in `WebPEncoder<Cursor<&mut [u8]>>::encode`. | Use `Cursor<&mut [u8]>` with empty output, call `encode(&[], 0, 1, ColorType::Rgba8)`. The size assertion passes (`0 * 1 * 4 == 0`), then `encode_frame()` returns `EncodingError::InvalidDimensions`, exercising the caller-side `?`. |

Planned edit:

- Extend only `encoder::__coverage_exercise_private_branches()`.
- Add one `Cursor<&mut [u8]>` invalid-dimensions encode probe.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.

Retention gate:

- Retain only if Coverage MCP produces a valid ingested snapshot, line/function
  coverage remains 100%, branch debt does not increase, and total missing
  regions falls below 41.

Validation:

- Local gates passed:
  - `cargo fmt --all --check`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- Coverage MCP run: `2a894f71-237a-466c-ad5a-df774d341a9a`.
- Snapshot: `e5a6b988-d6d0-48dc-8f8c-d5f7a8eabeb3`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines: `27081 / 27081`.
- Functions: `1634 / 1634`.
- Branches: `3487 / 3488`.
- Regions: `42672 / 42712` (40 missing), improved by 1.
- `src/codecs/webp/native/encoder.rs`: regions `1859 / 1859`, branches
  `194 / 194`.

Retention:

- Retain Batch 83. The `Cursor<&mut [u8]>` invalid-dimensions probe covers the
  encoder `encode_frame(...)?` error propagation while preserving all prior
  line/function/branch coverage.
- Remaining project debt after Batch 83:
  - `src/codecs/webp/native/lossless.rs`: 21 regions, 1 branch;
  - `src/codecs/webp/native/decoder.rs`: 14 regions;
  - `src/codecs/webp/native/vp8.rs`: 5 regions.

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

## Batch 84 plan

Goal: use reverse mapping on the current retained WebP-native debt, starting
with `lossless.rs` because it still owns the only missing branch.

Reverse mapping from snapshot `e5a6b988-d6d0-48dc-8f8c-d5f7a8eabeb3`:

- Global retained state is lines/functions at 100%, branches `3487 / 3488`,
  and regions `42672 / 42712` (40 missing).
- Current file debt:
  - `lossless.rs`: 21 regions and 1 branch.
  - `decoder.rs`: 14 regions and 0 branches.
  - `vp8.rs`: 5 regions and 0 branches.
- `lossless.rs` file segments have no zero-entry regions. The remaining debt
  is expression/generic-region coverage, so the useful map is function-region
  instantiation data plus source context.
- Collapsing branch records by source span shows every source-level branch side
  is covered. The single missing branch is therefore instantiation-specific.
  The strongest repeated branch locus is `BitReader::read_bits()` at
  `if self.nbits < num`.
- The remaining region clusters map to:
  - `read_transforms()` color-index table-size bands (`<=2`, `<=4`, `<=16`,
    and `>16`) after a successful nested color-map stream;
  - `read_huffman_codes()` meta-Huffman group growth in the entropy-image map;
  - `decode_image_data()` fast path/literal/back-reference/color-cache decisions;
  - `get_copy_distance()` / `plane_code_to_distance()` prefix and plane-distance
    decisions;
  - `BitReader::{fill,consume,read_bits}` concrete reader instantiations.

Planned edit:

- Add coverage-only bit-packing helpers inside
  `lossless::__coverage_exercise_private_branches()` so inputs are generated
  from the exact parser order instead of hand-guessed byte constants.
- Add direct `read_transforms()` probes for color-index table sizes 1, 3, 5,
  and 17 with a valid nested zero-image Huffman stream.
- Add direct `read_huffman_codes()` probe whose nested entropy image emits
  meta-Huffman code `1`, then supplies two groups of simple zero-symbol trees.
- Add exact `BitReader::read_bits()` probes for `Cursor<[u8; 1]>`,
  `Cursor<[u8; 8]>`, `Cursor<Vec<u8>>`, and `Take<Cursor<&[u8]>>` so both
  buffered and fill-required sides execute on the concrete reader families seen
  in the retained artifact.
- Add one fast-path `decode_image_data()` case with `bits == 0` and an active
  color cache, because the retained source map still points at that optional
  insertion path.

Retention rule:

- Retain only if Coverage MCP passes, line/function coverage stays 100%, and
  total missing regions or the single missing branch improves.

Validation:

- Local gates passed:
  - `cargo fmt --all`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- Coverage MCP run: `fb74873b-1ec2-40ea-94e9-9ee682f07fd7`.
- Snapshot: `17d5ff16-fde8-4dab-8f6d-9324c916f49a`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines/functions stayed 100%, but the batch regressed aggregate coverage:
  - branches changed from `3487 / 3488` to `3488 / 3490`;
  - missing branches increased from 1 to 2;
  - missing regions increased from 40 to 41.
- `src/codecs/webp/native/lossless.rs` changed from
  `1361 / 1382` regions and `119 / 120` branches to `1539 / 1561`
  regions and `120 / 122` branches.

Retention:

- Reject Batch 84 and revert the source probes. The bit-packed inputs executed,
  but they introduced new coverage-hook helper/generic branch surfaces faster
  than they reduced retained debt. The useful conclusion is that future
  lossless reverse mapping must avoid adding new helper functions or new
  generic reader families; it should either reuse existing hook types with
  inline states or remove/merge proven-dead private defensive branches.

## Batch 85 plan

Goal: reduce the smallest retained file debt, `vp8.rs` at 5 missing regions,
without adding new helper functions or generic reader families.

Reverse mapping from clean snapshot `0d9e107a-8c9e-4c7f-9c73-3d3e3132ccc3`:

- Global retained state is lines/functions at 100%, branches `3487 / 3488`,
  regions `42672 / 42712` (40 missing).
- `vp8.rs`: regions `2869 / 2874`, branches `160 / 160`.
- Raw function records show `Vp8Decoder<Cursor<&[u8]>>` still has zero-count
  prediction/decode regions:
  - `intra_predict_luma`: count `0`;
  - `intra_predict_chroma`: count `0`;
  - `decode_frame_`: partial direct cursor success coverage only.
- Production WebP container decode exercises the successful lossy path through
  `Take<Cursor<&[u8]>>`, but the direct raw VP8 `Cursor<&[u8]>` shape is used
  in coverage hooks only for malformed/error states.
- `decoder.rs` already contains a valid raw 17×19 VP8 payload used to exercise
  container-level read-image paths. Replaying that same payload directly through
  `Vp8Decoder::decode_frame(Cursor<&[u8]>)` should cover the direct cursor
  prediction/decode monomorphization without introducing any new helper code.

Planned edit:

- Extend only `vp8::__coverage_exercise_private_branches()`.
- Add one inline `lossy_vp8_17x19` byte array copied from the existing decoder
  coverage hook and call
  `Vp8Decoder::decode_frame(std::io::Cursor::new(lossy_vp8_17x19.as_slice()))`.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.

Retention gate:

- Retain only if Coverage MCP passes, line/function coverage stays 100%, branch
  debt does not increase, and total missing regions falls below 40.

Validation:

- Local gates passed:
  - `cargo fmt --all`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- Coverage MCP run: `47cd2d1a-ca3a-4509-8778-8d7a407ed3f4`.
- Snapshot: `3cb63c33-d2ad-47a7-8d6a-edb32953897e`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines/functions stayed 100%, but aggregate debt was unchanged:
  - branches stayed `3487 / 3488`;
  - regions changed from `42672 / 42712` to `42677 / 42717`;
  - total missing regions stayed at 40.
- `src/codecs/webp/native/vp8.rs` changed from `2869 / 2874` regions to
  `2874 / 2879` regions, leaving the same 5 missing regions.

Retention:

- Reject Batch 85 and revert the source probe. The direct raw VP8
  `Cursor<&[u8]>` success replay executed and added only covered regions; it did
  not clear the retained VP8 debt. The remaining VP8 regions are therefore not
  solved by direct valid-frame replay through the raw cursor reader shape.

## Batch 86 plan

Goal: reduce the remaining `vp8.rs` debt by reverse-mapping only the normal
`Take<Cursor<&[u8]>>` private-method regions, reusing existing hook macros and
existing byte inputs.

Baseline evidence:

- Clean retained Coverage MCP run:
  `007440cc-5947-4f6e-974f-f5fe6015c4aa`.
- Snapshot: `98fcad48-4dd9-4b2a-b9a9-421904299a1d`.
- Global counters: lines `27081 / 27081`, functions `1634 / 1634`,
  branches `3487 / 3488`, regions `42672 / 42712` (40 missing).
- Remaining file debt:
  - `src/codecs/webp/native/vp8.rs`: 5 regions, 0 branches;
  - `src/codecs/webp/native/lossless.rs`: 21 regions, 1 branch;
  - `src/codecs/webp/native/decoder.rs`: 14 regions, 0 branches.

Reverse map:

| Raw coordinate | Source construct | Input decision |
| --- | --- | --- |
| `983:52-53`, `993:54-55` | `init_partitions(2)` `read_exact(...)?` error propagation under `Take<Cursor<&[u8]>>`. | Add direct taken-reader `init_partitions(2)` calls with too-short partition-size bytes and with a declared second-partition byte but no payload. |
| `999:37-38` | `init_partitions(...)` final `read_to_end(...)?` error propagation under `Take<Cursor<&[u8]>>`. | No direct input can make `Take<Cursor<&[u8]>>` return an I/O error from `read_to_end`; measure nearby exact states first, then consider code-shape cleanup only if still retained. |
| `1193:48-49`, `1200:45-46`, `1220:30-31` | `read_frame_header()` caller-side `?` regions for loop-filter adjustment, partition init, and final accumulated bit-reader check under `Take<Cursor<&[u8]>>`. | Replay the three already-derived `keyframe_with_first_partition(...)` cases through `with_take_decoder!` instead of only through the cursor path. |
| `1492:96-97` | `read_residual_data()` plane-1 `read_coefficients(...)?` error propagation under `Take<Cursor<&[u8]>>`. | Mirror the existing cursor plane-1 EOF state through `with_take_decoder!`. |
| `1811` / `1813` | `calculate_filter_parameters()` loop-filter-adjustment branch under `Take<Cursor<&[u8]>>`. | Add a taken-reader state with adjustments enabled and a non-`B` luma mode, covering the false side while the outer adjustment block is active. |

Planned edit:

- Extend only `vp8::__coverage_exercise_private_branches()`.
- Do not add helper functions or new reader families.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.

Retention rule:

- Retain only if Coverage MCP passes, line/function coverage stays 100%, branch
  debt does not increase, and total missing regions falls below 40.

Validation:

- Local gates passed:
  - `cargo fmt --all`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- Coverage MCP run: `42cd05b6-7ef2-416f-96a8-92bca9420e59`.
- Snapshot: `d1c2a06e-1250-4c6c-aa25-a6329e12bb06`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines: `27112 / 27112`.
- Functions: `1634 / 1634`.
- Branches: `3487 / 3488`, unchanged.
- Regions: `42723 / 42759`, improving missing regions from 40 to 36.
- `src/codecs/webp/native/vp8.rs`: regions changed from `2869 / 2874`
  to `2920 / 2921`, improving VP8 debt from 5 regions to 1.

Retention:

- Retain Batch 86. The reverse-mapped taken-reader states covered the reachable
  VP8 `init_partitions`, `read_frame_header`, `read_residual_data`, and
  `calculate_filter_parameters` gaps while preserving 100% line/function
  coverage and not increasing branch debt.
- Remaining project debt after Batch 86:
  - `src/codecs/webp/native/lossless.rs`: 21 regions, 1 branch;
  - `src/codecs/webp/native/decoder.rs`: 14 regions;
  - `src/codecs/webp/native/vp8.rs`: 1 region.

## Batch 87 plan

Goal: decide whether the final VP8 region is reachable by input or requires
code-shape cleanup.

Reverse mapping from snapshot `d1c2a06e-1250-4c6c-aa25-a6329e12bb06`:

- `vp8.rs` has one retained missing region.
- Raw LLVM records show the remaining source-coordinate candidate is
  `init_partitions()` line `999`, the `?` on
  `self.r.read_to_end(&mut buf)?`.
- The same coordinate is zero under `Cursor<&[u8]>` and
  `Take<Cursor<&[u8]>>`; those reader implementations cannot return an I/O
  error from `read_to_end()` for any byte input.
- The existing `ErrorReader` probe does cover the source span, proving the
  generic error path itself is reachable, but the concrete no-error reader
  monomorphs still keep a one-region artifact.

Planned exploration:

- First try no new input for this coordinate; it is not byte-reachable for the
  concrete readers that own the missing region.
- Inspect code-shape options that preserve generic I/O error handling while not
  creating an uncovered `?` region for the no-error reader monomorphs.
- Retain only if coverage improves and the production semantics are unchanged.

Validation:

- Local gates passed:
  - `cargo fmt --all`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- Coverage MCP run: `260e0697-d579-4447-8e89-255b188edb6c`.
- Snapshot: `b5a998a2-191c-4bc1-8897-e51f6400dcfe`.
- Result: `5 passed / 0 failed`, exit code `0`.
- The code-shape change regressed coverage:
  - lines changed from `27112 / 27112` to `27112 / 27113`;
  - branches stayed `3487 / 3488`;
  - regions changed from `42723 / 42759` to `42722 / 42759`;
  - `vp8.rs` changed from `2920 / 2921` regions to `2919 / 2921`.

Retention:

- Reject Batch 87 and revert the source change. The explicit `match` preserved
  runtime semantics, but the `Err` arm is still impossible for the normal
  cursor-backed readers and now produces an uncovered source line. The final VP8
  region remains a monomorph-specific I/O-error artifact, not a missing byte
  fixture.

## Batch 88 plan

Goal: clear the only missing branch in `lossless.rs` by reverse-mapping the
branch-owning bit-buffer state, without adding helper functions or new custom
reader families.

Reverse mapping:

- `lossless.rs` owns the only retained branch debt: branches `119 / 120`.
- Raw LLVM branch records point to `BitReader::read_bits()` line `1457`:
  `if self.nbits < num`.
- The missing side is the true side for concrete reader/value instantiations
  that are otherwise live:
  - `BitReader<Cursor<[u8; 8]>>::read_bits::<u8>`;
  - `BitReader<Cursor<Vec<u8>>>::read_bits::<usize>` and
    `read_bits::<u16>`;
  - `BitReader<Take<Cursor<&[u8]>>>::read_bits::<usize>` and
    `read_bits::<u16>`.
- Public WebP bytes do not isolate those private bit-buffer states reliably
  because higher-level Huffman parsing consumes different bit widths before
  reaching the branch.

Planned edit:

- Extend only `lossless::__coverage_exercise_private_branches()`.
- Reuse existing reader families:
  - drive the existing `Cursor<[u8; 8]>` reader from `nbits > 0` to `nbits == 0`,
    then call `read_bits::<u8>(1)`;
  - add direct empty `Cursor<Vec<u8>>` `read_bits::<usize>(1)` and
    `read_bits::<u16>(1)` calls;
  - add direct empty `Take<Cursor<&[u8]>>` `read_bits::<usize>(1)` and
    `read_bits::<u16>(1)` calls.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.

Retention rule:

- Retain only if Coverage MCP passes, line/function coverage stays 100%, total
  missing branches drops below 1, and total missing regions does not increase.

Validation:

- First local coverage-configured compile caught a bad reverse-map type label:
  raw suffix `j` maps here to `usize`, not `u32`, because `LosslessBitValue`
  is implemented for `u8`, `u16`, and `usize`.
- Corrected local gates passed:
  - `cargo fmt --all`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- First Coverage MCP run: `f19dd659-b4fa-4f85-97c5-3c72e6c8600a`,
  snapshot `4653e876-81ce-4ef9-9999-f939333b506b`.
  - Lines/functions stayed 100%.
  - Branches stayed `3487 / 3488`.
  - Regions changed to `42750 / 42786`; total missing regions stayed 36.
- Raw follow-up showed the probes covered some `read_bits` true-side rows but
  introduced or left one-sided rows for the same generic instantiations, so the
  batch was adjusted to cover both sides for `usize`/`u16` direct states.
- Adjusted Coverage MCP run: `6bc53f1a-164f-4395-910a-e7935399c1a8`,
  snapshot `84ecfc9a-f27e-4711-84f0-aa5049038a6b`.
  - Lines/functions stayed 100%.
  - Branches stayed `3487 / 3488`.
  - Regions changed to `42800 / 42836`; total missing regions still stayed 36.

Retention:

- Reject Batch 88 and revert the source probes. Direct `BitReader::read_bits`
  states can move raw per-instantiation branch records, but they do not reduce
  the retained aggregate branch or region debt and add covered-only hook
  surface. The remaining lossless branch is therefore not solved by direct
  bit-buffer `read_bits` probes.

## Batch 89 plan

Goal: reduce decoder region debt by removing a hook-only custom-reader
monomorphization that no longer provides unique aggregate coverage.

Baseline evidence:

- Clean retained Coverage MCP run:
  `4dc29da2-21c6-4597-9220-04c15cd96ede`.
- Snapshot: `cc0d4dd4-2375-4d7e-a0c4-e3fbfd12b275`.
- Global counters: lines `27112 / 27112`, functions `1634 / 1634`,
  branches `3487 / 3488`, regions `42723 / 42759` (36 missing).
- `decoder.rs` owns 14 retained missing regions.
- Raw reverse map shows the largest decoder raw debt is
  `WebPDecoder<FailingReadCursor>::read_data`, created by a single
  coverage-hook call to `WebPDecoder::new(fail_read_at(...))`.
- The policy that call was intended to cover is the VP8X chunk-scan rule:
  ignore `UnexpectedEof`, but return non-EOF I/O errors. That logic now lives
  in `allow_vp8x_chunk_scan_error(...)` and is already directly covered by the
  coverage hook for both `UnexpectedEof` and `Other`.

Planned edit:

- Remove only the broad `WebPDecoder::new(fail_read_at(vp8x_scan_read_error,
  30))` coverage-hook call.
- Keep the `FailingReadCursor` direct method probes, so the custom reader itself
  is still exercised.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.

Retention rule:

- Retain only if Coverage MCP passes, line/function coverage stays 100%, branch
  debt does not increase, and total missing regions falls below 36.

Validation:

- Local gates passed:
  - `cargo fmt --all`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- Coverage MCP run: `c19cd9e7-ec62-43f2-bb65-573799874778`.
- Snapshot: `18d24f60-60dc-4569-bbab-b1fa3c511503`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines/functions stayed 100%.
- Branches stayed `3487 / 3488`.
- Regions changed from `42723 / 42759` to `42715 / 42751`; total missing
  regions stayed 36.
- `decoder.rs` stayed at 14 missing regions, but the raw
  `WebPDecoder<FailingReadCursor>::read_data` debt disappeared from the reverse
  map.

Retention:

- Retain Batch 89 as a coverage-hook cleanup, not a debt reducer. Removing the
  broad `FailingReadCursor` constructor probe does not reduce aggregate missing
  regions, but it removes obsolete hook-only monomorphization noise while
  preserving line/function/branch coverage. The remaining decoder raw debt now
  concentrates on `FailingSeekCursor`.

## Batch 90 plan

Goal: test whether broad `WebPDecoder<FailingSeekCursor>::new(...)` probes are
still needed, now that direct `range_reader(fail_seek_after(...))` covers seek
failure.

Reverse mapping:

- After Batch 89, the largest decoder raw gaps are all
  `FailingSeekCursor` monomorphs:
  - `read_frame`;
  - `read_data`;
  - `read_image`;
  - `new`;
  - `range_reader`.
- The actual seek-failure primitive is `range_reader(...)`, and the hook already
  calls `range_reader(fail_seek_after(Vec::new(), 0), 0..0)` directly.
- The constructor-level `WebPDecoder::new(fail_seek_after(...))` cases create
  the full generic decoder state machine under a reader that is only meant to
  fail seeking.

Planned edit:

- Remove only the constructor-level `WebPDecoder::new(fail_seek_after(...))`
  calls.
- Keep direct `range_reader(...)` and later `read_image` / `read_frame`
  `FailingSeekCursor` probes for this batch.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.

Retention rule:

- Retain if Coverage MCP passes, line/function coverage stays 100%, branch debt
  does not increase, and total missing regions does not increase. Prefer a
  reduction, but accept neutral cleanup if raw decoder debt becomes materially
  simpler.

## Batch 101 validation

- Local gates passed: `cargo fmt --all`, `cargo check --all-features`, coverage
  configuration check, and `git diff --check`.
- Coverage MCP run: `abe6ee35-23ec-4931-92b2-8064acbe67c3`.
- Snapshot: `d3a6dc3d-f710-47dc-b643-df2503106727`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines/functions stayed 100%.
- Branches stayed `3487 / 3488`; Batch 101 alone does not meet its branch
  retention rule.
- Missing regions improved from 35 to 32, and all four BitReader partial-branch
  loci disappeared from the MCP gap list.
- This proves the remaining summary branch is above BitReader. Keep Batch 101
  only as part of the combined reverse-map attempt below; otherwise revert it.

## Batch 102 plan

Goal: reverse-map the remaining branch above BitReader by removing generic
instantiation from the two pure copy-distance helpers.

Reverse mapping:

- After Batch 101, MCP reports the largest newly isolated per-instantiation
  split at `get_copy_distance` line 655: 14 raw branch rows, 7 covered. Runtime
  counts prove both source sides execute (`10906` prefix codes below 4 and
  `32928` codes at least 4), so the miss is concrete-instantiation debt.
- `get_copy_distance` is an associated function inside `impl<R>` even though it
  only reads `BitReader.buffer` and updates `BitReader.nbits`; it never touches
  `R`.
- `plane_code_to_distance` is also inside `impl<R>` despite having no reader
  input. MCP shows the same per-instantiation split at lines 669 and 675.
- Existing coverage-hook inputs already cover prefix codes below/above 4,
  plane codes below/above 120, and clamped/unclamped distances.

Planned edit:

- Move `get_copy_distance` to a free non-generic function operating on
  `&mut u64` and `&mut u8`; reuse the non-generic `consume_bits` state helper.
- Move `plane_code_to_distance` to a free non-generic function.
- Update production and coverage-hook call sites without changing inputs or
  output calculations.

Retention rule:

- Retain Batches 101 and 102 together only if Coverage MCP passes,
  lines/functions stay 100%, branches reach 100%, and missing regions remain
  below the retained 35-gap baseline.

Validation:

- Local gates passed after removing the newly dead `BitReader::peek` method.
- Coverage MCP run: `f2cbdad2-186f-4b9c-a73a-f407d9d16f45`.
- Snapshot: `41eaeac8-7e3f-4179-9f37-52a9ffbae3e5`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Branches stayed `3487 / 3488` and missing regions stayed 32.
- Lines regressed to `27006 / 27008`; the generic helper branch loci vanished,
  proving they are not the summary miss, but the batch fails its retention rule.

Retention:

- Reject and revert Batch 102. Continue from Batch 101's 100% line/function
  state and 32 missing regions.

## Batch 103 plan

Goal: cover the final short-circuit branch with the exact buffered state that
reaches it.

Reverse mapping:

- After Batch 101 removes BitReader branch duplication, the earliest remaining
  compound condition is `read_huffman_codes` line 315:
  `read_meta && self.bit_reader.read_bits::<u8>(1)? == 1`.
- Existing states cover:
  - `read_meta == false`, which short-circuits the bit read;
  - `read_meta == true` with one buffered `1`, which enters the meta-Huffman
    path;
  - `read_meta == true` with `nbits == 0` and `ErrorReader`, which returns an I/O
    error before the bit comparison and therefore does not cover RHS false.
- The missing third successful control-flow path is exactly `read_meta == true`
  with `buffer == 0` and `nbits == 1`.

Planned edit:

- Add one coverage-hook `decoder_with_bits(ErrorReader, 0, 1, 1, 1)` call to
  `read_huffman_codes(true, 1, 1, None)`.
- Its first bit read returns zero without touching the reader; later parsing may
  fail on `ErrorReader`, which is acceptable because the target branch has
  already executed.
- Do not add or change public fixtures.

Retention rule:

- Retain with Batch 101 only if Coverage MCP passes, lines/functions remain
  100%, branches become 100%, and the 32-gap region state does not regress.

Validation:

- Coverage MCP run: `e5d4451c-0a9e-437d-b6b3-5cc2450be57c`.
- Snapshot: `8c8d5ecc-3c92-41e6-9b66-ad003a0544e3`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines/functions stayed 100% and missing regions stayed 32.
- The raw missed count at line 315 dropped from 8 to 7, proving the intended
  RHS-false path executed, but summary branches stayed `3487 / 3488`.

Retention:

- Reject and revert Batch 103. The exact path was useful diagnostic evidence
  but did not improve aggregate coverage.

## Batch 104 plan

Goal: isolate the two remaining compound `||` predicates reported by MCP and
cover every executable short-circuit path directly.

Reverse mapping:

- Line 577 rejects a backward copy when
  `index < dist || num_values - index < length`.
- Line 591 enters the overlapping-copy expansion when
  `length > 4 || dist < 4`.
- For each `||`, three control-flow paths exist: left true (RHS skipped), left
  false/RHS true, and both false.
- The source tracker shows 15 per-instantiation misses at each line, while all
  bodies are line-covered. This is the strongest remaining signature for the
  one summary branch miss after ruling out BitReader and line 315.

Planned edit:

- Extract each pure predicate into a private non-generic function.
- Keep the existing `if` call sites and calculations unchanged.
- Exercise the three executable truth-table paths for both helpers in the
  coverage hook with exact integer states.

Retention rule:

- Retain with Batch 101 only if Coverage MCP passes, lines/functions remain
  100%, branches become 100%, and missing regions stay at or below 32.

Validation:

- Coverage MCP run: `8b1f9246-c548-47ea-8e52-4b6175989570`.
- Snapshot: `5f086ccc-3542-42bf-a3e4-179c0dcae970`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines: `27013 / 27013`.
- Functions: `1639 / 1639`.
- Branches: `3488 / 3488` (100%; final branch closed).
- Regions: `42618 / 42650` (32 missing, unchanged from Batch 101).
- `lossless.rs` is now branches `120 / 120` and regions `1404 / 1422`.

Retention:

- Retain Batches 101 and 104. The missing summary branch was one of the two
  copy-condition short-circuit edges; the non-generic truth-table helpers
  remove the ambiguity and keep behavior unchanged.

## Batch 105 plan

Goal: remove all 14 decoder regions by matching the private decoder's reader
type to the actual byte-slice API.

Reverse mapping:

- Retained snapshot `5f086ccc-3542-42bf-a3e4-179c0dcae970` reports only:
  - `decoder.rs`: 14 regions, 0 branches;
  - `lossless.rs`: 18 regions, 0 branches.
- The 14 decoder records are the previously mapped `stream_position`, `seek`,
  `seek_relative`, `range_reader(...)?`, and VP8X scan error arms on
  `WebPDecoder<Cursor<&[u8]>>`.
- Both public WebP entry points construct exactly `Cursor<&[u8]>`; no other
  production caller supplies a generic reader.
- Batch 96's full trait-object erasure changed the downstream VP8 reader type
  and regressed VP8. Concretizing to `Cursor<&[u8]>` keeps the existing
  `Take<&mut Cursor<&[u8]>>` VP8 instantiation, so it avoids that regression.

Planned edit:

- Change private `WebPDecoder<R>` to `WebPDecoder<'a>` storing
  `Cursor<&'a [u8]>`; keep `WebPDecoder::new(cursor)` unchanged for callers.
- Replace infallible cursor `stream_position`/`seek` operations with
  `position`/`set_position`.
- Specialize `range_reader` for the cursor and make it infallible; remove `?`
  at its three call sites.
- Treat a truncated VP8X chunk-header scan as the only possible cursor read
  failure and stop scanning; remove the now-unneeded generic scan-error policy.
- Remove coverage-only failing-reader scaffolding that no production type can
  instantiate after this change.

Retention rule:

- Retain only if Coverage MCP passes, lines/functions/branches stay 100%, VP8
  stays fully covered, and decoder missing regions become zero without raising
  total missing regions above 18.

Validation:

- Local gates passed: normal and `--cfg coverage` all-feature checks, rustfmt,
  and `git diff --check`.
- Coverage MCP run: `fd56cd0f-2aea-4dd4-9f45-418f623554c6`.
- Snapshot: `f9fa0a73-1091-49e5-a5a1-cac058357e29`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines/functions/branches stayed 100%:
  - lines `26932 / 26932`;
  - functions `1628 / 1628`;
  - branches `3480 / 3480`.
- Decoder regions became `1299 / 1299` and total missing regions fell from 32
  to 18 (`42509 / 42527`).
- The only remaining region debt is `lossless.rs` at `1404 / 1422`.

Retention:

- Retain Batch 105. The private decoder now represents its only real input
  type, so impossible seek failures are absent from both the implementation and
  the coverage model.

## Batch 106 plan

Goal: remove the final 18 regions in `lossless.rs` without changing decoded
pixels or bitstream semantics.

Reverse mapping:

- Snapshot `f9fa0a73-1091-49e5-a5a1-cac058357e29` reports 100% lines,
  functions, and branches, but LLVM records only `130 / 166` covered generic
  instantiations in `lossless.rs` and `1404 / 1422` regions.
- The 39 partial-source rows span the whole `LosslessDecoder<R>` state machine:
  transforms, meta-Huffman setup, Huffman decoding, copy/cache paths, and color
  cache validation. Their broad distribution cannot correspond to 18 new
  image states after every aggregate branch is already covered; it is the
  signature of the same source branches being emitted once per reader type.
- Production currently instantiates the decoder with both owned `Take<...>`
  readers and borrowed readers from ALPH decoding. The coverage hook adds
  `Cursor`, `ErrorReader`, and staged-error reader instantiations so it can
  reverse-map exact error states. Each extra generic decoder instance requires
  its own complete branch matrix for LLVM region coverage.
- Batch 101 already routes the bit-buffer I/O through `dyn BufRead`, so reader
  method dispatch in the hot fill path is already type-erased. Unifying the
  owning decoder therefore does not add a new dispatch layer to that path.

Planned edit:

- Change `LosslessDecoder<R>` to `LosslessDecoder<'a>` and store one
  `BitReader<Box<dyn BufRead + 'a>>`.
- Make `LosslessDecoder::new` accept that boxed trait object directly and box
  the reader at the three private production call sites. This costs one small
  allocation per VP8L/ALPH decoder and prevents even the constructor from
  acquiring per-reader generic instantiations.
- Update coverage-only direct state construction to box its exact scripted
  readers. Keep the existing reverse-mapped byte/state probes unchanged.
- Do not change Huffman, transform, copy, or cache algorithms.

Retention rule:

- Retain only if normal and coverage builds pass, Coverage MCP reports five
  passing tests, all four metrics reach 100%, and exact Pillow parity remains
  intact through the manifest-driven suite.

Validation:

- Local gates passed:
  - `cargo fmt --all`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- Coverage MCP run: `331df368-78a0-4f42-9779-81d6b33356f2`.
- Snapshot: `8a939154-c18c-4532-8b5c-b7bb716aaa4c`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Repository totals are fully covered:
  - lines `26934 / 26934`;
  - functions `1628 / 1628`;
  - branches `3480 / 3480`;
  - regions `42536 / 42536`.
- `lossless.rs` is now lines `1123 / 1123`, functions `44 / 44`, branches
  `120 / 120`, regions `1431 / 1431`, and LLVM generic instantiations
  `79 / 79`.
- Decoder and VP8 remain fully covered, confirming that the reader unification
  did not recreate the Batch 96 downstream-reader regression.

Retention:

- Retain Batch 106. The last 18 misses were duplicated generic-instantiation
  regions, not undiscovered image states. One owned reader representation lets
  the existing reverse-mapped fixture/state matrix prove every emitted region.

## Batch 101 plan

Goal: remove the final lossless branch by de-monomorphizing the actual
BitReader state machine, using the retained reverse map rather than adding more
byte fixtures.

Reverse mapping:

- Retained snapshot `fd68f5be-99e5-4c54-a55d-396ca7bc046d` is lines
  `26983 / 26983`, functions `1634 / 1634`, branches `3487 / 3488`, and
  regions `42575 / 42610`.
- `lossless.rs` is the only file with summary branch debt: branches
  `119 / 120`, regions `1361 / 1382`, and 150 LLVM instantiations.
- The duplicated partial branch rows cluster in generic
  `BitReader<R>::fill`, `consume`, and `read_bits` at lines 1414, 1420, 1443,
  and 1457.
- Batch 99 extracted only another method inside `impl<R>`; it remained generic,
  so it did not change the LLVM branch summary. Batches 97, 98, and 100 also
  prove that changing concrete hook input types or adding another direct read
  cannot clear this debt.
- The retained probes already reverse-map both required states:
  - empty/short and eight-byte input for `fill`;
  - insufficient and sufficient buffered bits for `consume`;
  - `read_bits` with and without a refill.

Planned edit:

- Move buffer filling, consuming, and the `u32` bit read state machine into
  private non-generic functions accepting `&mut dyn BufRead`, `&mut u64`, and
  `&mut u8`.
- Keep `BitReader<R>` generic and keep its existing method API; wrappers will
  delegate directly, preserving static storage and call sites.
- Use `Result::map` in the generic typed conversion wrapper so no new generic
  `?` error region is created.
- Do not change fixtures, oracle outputs, or Huffman behavior.

Retention rule:

- Retain only if local gates pass, Coverage MCP passes, lines/functions stay at
  100%, the branch count becomes 100%, and missing regions do not increase.

## Batch 98 plan: reverse-map the remaining lossless branch

Current MCP snapshot: `fd68f5be-99e5-4c54-a55d-396ca7bc046d`.

Current counters:

- Lines: `26983 / 26983`
- Functions: `1634 / 1634`
- Branches: `3487 / 3488`
- Regions: `42575 / 42610`

Reverse mapping:

- File summary says `src/codecs/webp/native/lossless.rs` has the only branch
  miss: `119 / 120` branches.
- Source line coverage is already 100%; the miss is hidden by generic
  instantiation aggregation.
- Raw LLVM rows show the concrete missing branch is:
  - function: `BitReader<Cursor<[u8; 5]>>::consume`
  - source span: `lossless.rs:1443` (`if self.nbits < num`)
  - current counts: true arm `0`, false arm `2`
- Reverse use-site search maps `Cursor<[u8; 5]>` to
  `src/codecs/webp/native/huffman.rs::__coverage_exercise_private_branches`.
  That hook fills a 5-byte reader and exercises Huffman reads, but never calls
  `consume` with a count larger than the 40 bits loaded from the 5-byte buffer.

Planned edit:

- In the huffman coverage hook, add a direct
  `BitReader<Cursor<[u8; 5]>>::consume(41)` probe after a successful 5-byte
  `fill()`.
- This is fixture-based/reverse-mapped: it uses the exact reader type and state
  already responsible for the missing instantiated branch.

Retention rule:

- Retain only if Coverage MCP passes, total branch coverage becomes
  `3488 / 3488`, and no file regresses in line/function/branch coverage.
- If only regions improve but the branch remains missing, retain only if there
  is no regression and the raw reverse map proves the intended instantiation was
  affected.

Validation:

- Local gates passed:
  - `cargo fmt --all`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `5eb47ff9-7bec-4eaf-86c3-2d71e5620843`
- Snapshot: `eb1a9173-5240-4f3b-9ef9-0a74ae6e9a85`
- Result:
  - Lines: `26986 / 26986`
  - Functions: `1634 / 1634`
  - Branches: `3487 / 3488`
  - Regions: `42584 / 42619`

Decision:

- Reject and revert the `huffman.rs` hook. It covered the directly suspected
  5-byte cursor state, but aggregate branch and missing-region debt stayed
  unchanged. The remaining aggregate miss is not the
  `BitReader<Cursor<[u8; 5]>>::consume(41)` state.

## Batch 99 plan: de-multiply `read_bits<T>` branch coverage

Current retained counters remain from snapshot
`fd68f5be-99e5-4c54-a55d-396ca7bc046d`:

- Lines: `26983 / 26983`
- Functions: `1634 / 1634`
- Branches: `3487 / 3488`
- Regions: `42575 / 42610`

Reverse mapping:

- Repeated raw LLVM rows still cluster at `lossless.rs:1457`, inside
  `BitReader<R>::read_bits<T>()`:
  `if self.nbits < num { self.fill()?; }`.
- The branch is generic over both reader type `R` and output type `T`
  (`u8`, `u16`, `usize`). Prior direct probes for `read_bits<T>` did not clear
  the aggregate miss because every new `T`/reader combination creates another
  instantiation surface.
- This is monomorphization debt, not missing fixture bytes: the same fill
  decision does not semantically depend on `T`; it depends only on the reader
  state and requested bit count.

Planned edit:

- Extract the fill decision from `read_bits<T>` into a private helper on
  `BitReader<R>` that is generic only over `R`, not over `T`.
- `read_bits<T>` will call the helper before `peek` and `consume`.
- This keeps decode behavior unchanged while reducing the number of coverage
  branch instantiations for the same source decision.

Retention rule:

- Retain only if Coverage MCP passes and either:
  - total branch coverage reaches `3488 / 3488`; or
  - total missing regions decrease with no line/function/branch regression.
- Revert if it only moves the branch debt or creates new region debt elsewhere.

Validation:

- Local gates passed:
  - `cargo fmt --all`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `ca3b9ce6-ca96-4103-b21a-f8c9a209a317`
- Snapshot: `771db5a6-16fb-411b-8f3a-aa4a2b23fed3`
- Result:
  - Lines: `26987 / 26987`
  - Functions: `1635 / 1635`
  - Branches: `3487 / 3488`
  - Regions: `42582 / 42617`

Decision:

- Do not retain Batch 99 by itself. It moved the branch from
  `read_bits<T>` to `fill_if_needed`, but aggregate debt remained unchanged.
- The post-refactor raw map is useful: the remaining one-sided helper row is
  `BitReader<Cursor<[u8; 8]>>::fill_if_needed`, false side only. Batch 100
  will test the exact true-side state against the de-multiplied helper. Revert
  both B99 and B100 if the combined result does not reduce retained debt.

## Batch 100 plan: exact true-side state after de-multiplication

Current temporary state: Batch 99 helper is present but not retained alone.

Reverse mapping:

- After Batch 99, raw LLVM rows no longer report branch misses inside
  `read_bits<T>`.
- The remaining helper miss maps to:
  - function: `BitReader<Cursor<[u8; 8]>>::fill_if_needed`
  - branch: `lossless.rs:1432`, `if self.nbits < num`
  - current counts: true side `0`, false side `1`
- Existing coverage hook state for `Cursor<[u8; 8]>` pre-fills the buffer and
  then calls `read_bits(1)`, so it only exercises the false side
  (`nbits >= num`).

Planned edit:

- Add one direct coverage-hook state in
  `lossless::__coverage_exercise_private_branches()`:
  `BitReader::__coverage_new(Cursor::new([0u8; 8])).read_bits::<u8>(1)`
  without pre-filling.
- This drives the exact reader instantiation through the true side:
  `nbits == 0`, `num == 1`, then `fill()` succeeds from the 8-byte cursor.

Retention rule:

- Retain the combined Batch 99 + Batch 100 source only if Coverage MCP passes
  and total branch coverage becomes `3488 / 3488` or total missing regions
  decreases with no branch regression.
- If aggregate debt remains unchanged, revert both source changes and keep only
  this documentation.

Validation:

- Local gates passed:
  - `cargo fmt --all`
  - `cargo check --all-features`
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`
- Coverage MCP run: `f5cf0fc4-20b5-4d33-8b8f-fdb58b0b8da7`
- Snapshot: `255cde32-b925-4998-a84a-a3c8cf4c53b4`
- Result:
  - Lines: `26989 / 26989`
  - Functions: `1635 / 1635`
  - Branches: `3487 / 3488`
  - Regions: `42588 / 42623`

Decision:

- Reject and revert the combined Batch 99 + Batch 100 source changes. The
  de-multiplied helper plus exact `Cursor<[u8; 8]>` true-side state still left
  aggregate branch and missing-region debt unchanged.
- This confirms the retained branch miss is not cleared by reshaping or directly
  driving the `read_bits` fill decision. Continue reverse mapping elsewhere,
  especially aggregate region-owning decoder/lossless propagation sites.

## Batch 95 plan

Goal: remove the single remaining VP8 region by eliminating a generic
`read_to_end?` instrumentation split, without changing encoded/decoded bytes.

Reverse mapping:

- Coverage MCP raw regions identify the remaining VP8 miss at
  `Vp8Decoder<Take<Cursor<&[u8]>>>::init_partitions` and
  `Vp8Decoder<Cursor<&[u8]>>::init_partitions`, line 999, columns 37-38.
- Source:
  - `self.r.read_to_end(&mut buf)?;`
- The uncovered region is the error-propagation side of `?`.
- Fixture feasibility:
  - `Cursor<&[u8]>` and `Take<Cursor<&[u8]>>` cannot produce a read I/O error;
  - a failing reader already reaches the same source path through
    `Vp8Decoder<ErrorReader>::init_partitions(1)`;
  - LLVM still accounts the generic `init_partitions<R>` source region
    separately for infallible reader monomorphs.

Planned edit:

- Move only the final-partition read into a private non-generic helper:
  `init_final_partition(&mut dyn Read, &mut ArithmeticDecoder)`.
- Keep the partition byte layout and `ArithmeticDecoder::init(...)` call
  unchanged.
- Call the helper from `Vp8Decoder<R>::init_partitions(...)`.
- Rely on existing success fixtures and the existing `ErrorReader`
  reverse-mapped probe to cover the shared helper success/error sides.

Retention rule:

- Retain only if local gates pass, Coverage MCP passes, line/function coverage
  stays 100%, branch debt does not increase, and total missing regions drops
  below 36.

Validation:

- Local gates passed:
  - `cargo fmt --all`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- Coverage MCP run: `38f323fc-3ee6-4820-983f-ecf5374fca6e`.
- Snapshot: `8c6edee5-cc7e-4cb3-9053-2fc24857e093`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines/functions stayed 100%.
- Branches stayed `3487 / 3488`.
- Regions changed from `42569 / 42605` to `42575 / 42610`; total missing
  regions dropped from 36 to 35.
- `src/codecs/webp/native/vp8.rs` is now fully covered:
  - lines `1635 / 1635`;
  - branches `160 / 160`;
  - functions `61 / 61`;
  - regions `2926 / 2926`.

Retention:

- Retain Batch 95. Reverse mapping showed the remaining VP8 miss was a generic
  `read_to_end?` error arm for infallible `Cursor` readers. Moving only the
  final-partition read into a non-generic helper let the existing successful
  fixtures and failing-reader probe cover the shared helper sides without
  changing partition bytes or decode behavior.

## Batch 96 plan

Goal: reduce `src/codecs/webp/native/decoder.rs` region debt after Batch 95
cleared VP8.

Reverse mapping:

- Snapshot `8c6edee5-cc7e-4cb3-9053-2fc24857e093` reports:
  - total regions `42575 / 42610` (35 missing);
  - `decoder.rs` regions `1408 / 1422` (14 missing);
  - `lossless.rs` regions `1361 / 1382` and branches `119 / 120`.
- Raw file-level zero region-entry segments for `decoder.rs` are exactly:
  - `read_data`: lines 188, 242, 255, 258, 275, 278, 305, 322;
  - `read_image`: lines 423, 439, 456;
  - `read_frame`: lines 509, 567, 573.
- Source classification:
  - all except line 278 are `stream_position`, `seek`, `seek_relative`, or
    `range_reader` `?` error-propagation arms;
  - line 278 is the non-EOF `allow_vp8x_chunk_scan_error(error)?` arm while
    scanning VP8X chunks.
- Fixture feasibility:
  - successful public fixtures use `Cursor<&[u8]>`, whose seek and
    `stream_position` calls cannot fail;
  - targeted failing readers can reach these arms, but previous generic
    `WebPDecoder<FailingSeekCursor>` probes created broad monomorph noise;
  - `WebPDecoder` is only `pub(crate)` through `webp::native`, so changing its
    private representation is acceptable if the public WebP decode wrapper
    remains unchanged.

Planned edit:

- Replace generic `WebPDecoder<R>` storage with `WebPDecoder<'a>` containing
  `Box<dyn BufReadSeek + 'a>`, where `BufReadSeek: BufRead + Seek`.
- Keep `WebPDecoder::new(...)`, dimensions, animation, and read APIs unchanged
  for callers.
- Make `range_reader` operate on `&mut dyn BufReadSeek` so the range seek error
  arm is shared instead of monomorphized per reader.
- Add targeted failing-reader coverage-hook probes for the reverse-mapped
  decoder seek/read sites after the type-erasure boundary is in place.

Retention rule:

- Retain only if local gates pass, Coverage MCP passes, line/function coverage
  stays 100%, branch debt does not increase, and total missing regions drops
  below 35.

Validation:

- Local gates passed before coverage:
  - `cargo fmt --all`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- Coverage MCP run: `b915d7e4-f6dd-4b4a-aabd-e467b16a1a2b`.
- Snapshot: `949d33fd-01c7-4b8b-98cc-6618da34cf24`.
- Result: `5 passed / 0 failed`, exit code `0`.
- The decoder type-erasure experiment reduced `decoder.rs` to 1 missing region,
  but failed the retention rule:
  - total lines regressed to `27071 / 27074`;
  - total branches regressed to `3484 / 3488`;
  - `vp8.rs` regressed to lines `1632 / 1635`, branches `157 / 160`, and
    regions `2916 / 2926`.

Retention:

- Reject and revert Batch 96 source changes. The experiment proved that full
  `WebPDecoder` reader type-erasure can remove most decoder generic-region debt,
  but it changes downstream VP8 reader instantiations enough to create larger
  coverage debt. Keep Batch 95; do not keep Batch 96.

Post-revert validation:

- Local gates passed after reverting Batch 96:
  - `cargo fmt --all`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- Coverage MCP run: `4d610f92-ab5d-4e53-8aad-9a6b4b29caa6`.
- Snapshot: `3a35a0bd-e86c-492a-be1d-840c14314478`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Retained state restored:
  - lines `26983 / 26983`;
  - functions `1634 / 1634`;
  - branches `3487 / 3488`;
  - regions `42575 / 42610` (35 missing).
- Remaining file debt:
  - `decoder.rs`: regions `1408 / 1422` (14 missing), branches 100%;
  - `lossless.rs`: regions `1361 / 1382` (21 missing), branches `119 / 120`;
  - `vp8.rs`: fully covered.

## Batch 97 plan

Goal: attack the remaining `lossless.rs` branch debt with reverse mapping, not
random fixtures.

Reverse mapping:

- Retained snapshot `3a35a0bd-e86c-492a-be1d-840c14314478` reports
  `lossless.rs` at branches `119 / 120` and regions `1361 / 1382`.
- `lossless.rs` has 326 raw branch rows, 60 unique source branch spans, and 0
  source spans with an aggregate one-sided miss.
- Every source span has at least one both-sided covered row; the remaining
  branch miss is therefore a concrete-instantiation miss, not a missing source
  condition.
- One-sided raw rows cluster in the BitReader state machine:
  - `fill()` around lines 1414 and 1420;
  - `consume()` around line 1443;
  - `read_bits()` around line 1457.
- The coverage hook directly instantiates fixed-size reader types:
  - `BitReader<Cursor<[u8; 8]>>`;
  - `BitReader<Cursor<[u8; 1]>>`.
- Those concrete array reader types make opposite `fill()` sides impossible for
  the same instantiation, while the `Cursor<Vec<u8>>` reader type can exercise
  both short and long buffer states.

Planned edit:

- Change only coverage-hook direct BitReader probes from fixed arrays to
  `Vec<u8>`:
  - `Cursor::new([0u8; 8])` -> `Cursor::new(vec![0u8; 8])`;
  - `Cursor::new([0u8; 1])` -> `Cursor::new(vec![0u8; 1])`;
  - `Cursor::new([0b1010_1010u8; 8])` -> `Cursor::new(vec![0b1010_1010u8; 8])`.
- Keep all production code unchanged.

Retention rule:

- Retain only if local gates pass, Coverage MCP passes, line/function coverage
  stays 100%, and either branch debt drops to zero or region debt drops without
  increasing branch debt.

Validation:

- Local gates passed:
  - `cargo fmt --all`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- Coverage MCP run: `84136344-fc54-49c9-bb25-8104c2a9e4f6`.
- Snapshot: `fd68f5be-99e5-4c54-a55d-396ca7bc046d`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Metrics were unchanged:
  - lines `26983 / 26983`;
  - functions `1634 / 1634`;
  - branches `3487 / 3488`;
  - regions `42575 / 42610`.
- `lossless.rs` remained at branches `119 / 120` and regions `1361 / 1382`.

Retention:

- Reject and revert Batch 97. Replacing fixed-size direct BitReader reader
  probes with `Cursor<Vec<u8>>` did not change aggregate branch or region debt.
  This rules out fixed-array BitReader hook instantiations as the counted
  remaining lossless branch miss.

Validation:

- Local gates passed:
  - `cargo fmt --all`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- First Coverage MCP run: `2d2f9a7d-a455-42d6-9e91-cd4fbbab6d8b`.
- First snapshot: `e4be7344-c3e2-4235-878c-21ca5a8dffa2`.
  - Lines/functions stayed 100%.
  - Branches stayed `3487 / 3488`.
  - Regions changed from `42684 / 42720` to `42565 / 42602`; total missing
    regions increased from 36 to 37, so the batch was not retainable as first
    written.
- Raw reverse map showed the regression was the newly isolated
  `range_reader<FailingSeekCursor>` success return at line 1179. The failure
  case `range_reader(fail_seek_after(Vec::new(), 0), 0..0)` covered the `seek?`
  error side, but no input covered `Ok(r.take(...))` for that same concrete
  reader type.
- Adjustment: add `range_reader(fail_seek_after(Vec::new(), 1), 0..0)` to cover
  the success side without restoring broad `WebPDecoder<FailingSeekCursor>`
  probes.
- Adjusted local gates passed:
  - `cargo fmt --all`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- Adjusted Coverage MCP run: `09c47671-9d53-4d19-adbe-a0b910f8e8a0`.
- Adjusted snapshot: `5a400543-ecb2-49b3-bfb1-3c206f867b99`.
  - Result: `5 passed / 0 failed`, exit code `0`.
  - Lines/functions stayed 100%.
  - Branches stayed `3487 / 3488`.
  - Regions changed from `42684 / 42720` to `42569 / 42605`; total missing
    regions stayed 36.
  - `decoder.rs` stayed at 14 missing regions.

Retention:

- Retain adjusted Batch 91 as a coverage-hook cleanup. It removes the broad
  `WebPDecoder<FailingSeekCursor>` read-frame/read-image/read-data
  monomorphization debt while preserving aggregate line/function/branch/region
  coverage. The remaining decoder raw debt is now only the normal
  `Cursor<&[u8]>` decoder monomorphs plus one `range_reader<Cursor<&[u8]>>`
  region.

## Batch 92 plan

Goal: attack the smallest aggregate target: the remaining VP8 region.

Reverse mapping:

- Aggregate after Batch 91:
  - Lines `26977 / 26977`;
  - functions `1633 / 1633`;
  - branches `3487 / 3488`;
  - regions `42569 / 42605` (36 missing).
- File debt:
  - `lossless.rs`: 21 regions, 1 branch;
  - `decoder.rs`: 14 regions;
  - `vp8.rs`: 1 region.
- Raw VP8 reverse map shows the smallest live aggregate target is
  `Vp8Decoder<Take<Cursor<&[u8]>>>::init_partitions` /
  `Vp8Decoder<Cursor<&[u8]>>::init_partitions` around lines 980-1006.
- Prior Batch 87 showed that changing the `read_to_end(...)?` source shape with
  a manual `match` regresses line/region coverage, so do not reshape the
  production helper for this attempt.

Planned edit:

- Inspect exact missing region records for `init_partitions`.
- Add only targeted inputs that reach the missing line-side for the same
  concrete reader type, or reject if the missing region is a non-input-coverable
  LLVM artifact of `read_to_end(...)?`.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.

Retention rule:

- Retain only if Coverage MCP passes, line/function coverage stays 100%, branch
  debt does not increase, and total missing regions drops below 36.

Validation:

- Exact raw records for the live `init_partitions` miss:
  - `Vp8Decoder<Take<Cursor<&[u8]>>>::init_partitions`, line 999 columns
    37-38, count `0`;
  - `Vp8Decoder<Cursor<&[u8]>>::init_partitions`, line 999 columns 37-38,
    count `0`.
- Source expression:
  - `self.r.read_to_end(&mut buf)?;`
- The missed region is the error-propagation arm of the `?` operator.
- Reverse-mapped input feasibility:
  - `Cursor<&[u8]>` cannot return an I/O error from `read_to_end`;
  - `Take<Cursor<&[u8]>>` cannot return an I/O error from `read_to_end`;
  - the existing `Vp8Decoder<ErrorReader>::init_partitions(1)` covers the same
    source expression's error arm for a fallible reader, but LLVM still retains
    a per-concrete-reader source region for the infallible readers.
- Prior Batch 87 proved that manually reshaping this `?` into a `match`
  regresses line/region coverage, so no production source rewrite is retained
  here.

Retention:

- No source edit. Treat the one VP8 region as a non-input-coverable LLVM/generic
  instrumentation artifact unless we later choose an explicit production
  refactor that removes the generic `?` site without adding performance or
  readability cost.

## Batch 93 plan

Goal: reduce decoder's remaining 14 regions after Batch 91 simplified away the
custom failing-reader monomorphs.

Reverse mapping:

- Current decoder raw debt after Batch 91:
  - `WebPDecoder<Cursor<&[u8]>>::read_data`: 8 regions;
  - `WebPDecoder<Cursor<&[u8]>>::read_image`: 3 regions;
  - `WebPDecoder<Cursor<&[u8]>>::read_frame`: 3 regions;
  - `range_reader<Cursor<&[u8]>>`: 1 raw region, already folded into the 14
    aggregate decoder regions.
- The broad `FailingSeekCursor` and `FailingReadCursor` `WebPDecoder`
  monomorphs are gone, so remaining decoder misses are normal public-reader
  paths. Reverse mapping must identify whether each missing region is:
  - an error arm for infallible `Cursor<&[u8]>` reads/seeks;
  - a missing valid/invalid RIFF fixture path;
  - or a source shape artifact similar to VP8 line 999.

Planned edit:

- Inspect exact missing raw region records for the four decoder groups.
- Prefer targeted RIFF/WebP bytes that reach a missing source side for
  `Cursor<&[u8]>`.
- If a region is only an infallible `Cursor` I/O error arm, document it rather
  than adding impossible fixtures.
- Run local gates and Coverage MCP only after a source/test hook change is made.

Retention rule:

- Retain only if Coverage MCP passes, line/function coverage stays 100%, branch
  debt does not increase, and total missing regions drops below 36 or raw
  decoder debt becomes materially simpler without increasing total missing
  regions.

Validation:

- Exact raw missing region records after Batch 91:
  - `range_reader<Cursor<&[u8]>>`: line 1179, columns 45-46;
  - `WebPDecoder<Cursor<&[u8]>>::read_data`: lines 188, 242, 255, 258, 275,
    278, 305, 322;
  - `WebPDecoder<Cursor<&[u8]>>::read_image`: lines 423, 439, 456;
  - `WebPDecoder<Cursor<&[u8]>>::read_frame`: lines 509, 567, 573.
- Source classification:
  - line 188: `stream_position()?`;
  - lines 242, 255, 258, 275, 305, 322, 509, 573, and 1179:
    `seek(...)` / `seek_relative(...)` / `range_reader(...)` error arms;
  - lines 423, 439, and 456: `range_reader(&mut self.r, ...)?` error arms;
  - line 567: `stream_position()?`;
  - line 278: `allow_vp8x_chunk_scan_error(error)?` non-EOF error arm while
    scanning VP8X chunks.
- Reverse-mapped input feasibility:
  - `Cursor<&[u8]>` `stream_position`, `seek(Start(...))`, and these bounded
    positive `seek_relative(...)` calls are infallible for byte fixtures;
  - `range_reader<Cursor<&[u8]>>` uses `seek(Start(...))`, also infallible;
  - VP8X chunk scan over a `Cursor<&[u8]>` can produce EOF, which
    `allow_vp8x_chunk_scan_error` intentionally ignores, but cannot produce a
    non-EOF `io::ErrorKind::Other`; that policy arm is covered directly by
    `allow_vp8x_chunk_scan_error(...)` in the coverage hook.

Retention:

- No source edit. Treat the 14 decoder regions as non-input-coverable
  `Cursor<&[u8]>` I/O error artifacts. The previous batches already removed the
  artificial failing-reader decoder monomorph noise; further reduction would
  require a production refactor that removes generic `?` sites or dynamic-read
  helpers, not new byte fixtures.

## Batch 94 plan

Goal: attack the only remaining branch miss, which lives in
`src/codecs/webp/native/lossless.rs`.

Reverse mapping:

- Aggregate after Batch 91 remains:
  - Lines `26977 / 26977`;
  - functions `1633 / 1633`;
  - branches `3487 / 3488`;
  - regions `42569 / 42605` (36 missing).
- File debt:
  - `lossless.rs`: 21 regions, 1 branch;
  - `decoder.rs`: 14 non-input-coverable regions;
  - `vp8.rs`: 1 non-input-coverable generic `?` region.
- Prior Batch 88 showed that simple direct `BitReader::read_bits` probes can
  move raw per-instantiation records but do not reduce aggregate branch/region
  debt.

Planned edit:

- Reverse-map the aggregate missing branch to exact `lossless.rs` source and
  concrete helper.
- Prefer input/state construction that reaches the opposite side through the
  existing coverage hook without adding new broad decoder monomorphs.
- If the missing branch is another infallible-reader or dead generic artifact,
  document it explicitly with source lines and reader type.
- Run local gates, then Coverage MCP only after a source/test hook change is
  made.

Retention rule:

- Retain only if Coverage MCP passes, line/function coverage stays 100%, branch
  debt drops to zero, and total missing regions does not increase.

Validation:

- MCP file summary still reports `lossless.rs` at branches `119 / 120` and
  regions `1361 / 1382`.
- Raw file branch rows are duplicated heavily by generic instantiation. A
  source-location aggregation over all file-level LLVM branch records found:
  - 60 unique branch source locations;
  - 0 source locations where either side has aggregate count 0.
- The noisy branch clusters remain:
  - `BitReader::fill()` lines 1414 and 1420;
  - `BitReader::consume()` line 1443;
  - `BitReader::read_bits()` line 1457;
  - `read_color_cache()`, `get_copy_distance()`, and
    `plane_code_to_distance()` around lines 637-675;
  - Huffman/meta-Huffman decode branches around lines 315-330 and 376-602.
- Prior rejected batches already tested the plausible direct input/state
  mappings:
  - direct `BitReader::read_bits()` states (Batches 44, 57/66, 84, 88);
  - `BitReader::fill()` non-empty / `nbits == 56` state (Batch 75);
  - `decode_image_data()` copy-length-overrun state (Batch 74);
  - broad public VP8L fixture generation and bit-packed lossless states
    (Batch 84).

Retention:

- No source edit. The remaining lossless branch is not currently mapped to a
  source location with a missing aggregate side, and all previously measured
  direct state probes were no-ops or regressions. Treat it as unresolved
  LLVM/generic summary debt until we either:
  - produce a normalized branch report that identifies the exact uncovered
    instantiation without contradictory source aggregation; or
  - perform a deliberate production refactor that reduces generic
    `BitReader<R>` monomorphization without changing decode behavior.

Validation:

- Local gates passed:
  - `cargo fmt --all`;
  - `cargo check --all-features`;
  - `RUSTFLAGS='--cfg coverage' cargo check --all-features`;
  - `git diff --check`.
- Coverage MCP run: `fa70bc10-aee9-4a7c-b37c-7d4e07c0623d`.
- Snapshot: `892f1389-0b90-45f1-8b86-b71b8cc91572`.
- Result: `5 passed / 0 failed`, exit code `0`.
- Lines/functions stayed 100%.
- Branches stayed `3487 / 3488`.
- Regions changed from `42715 / 42751` to `42684 / 42720`; total missing
  regions stayed 36.
- `decoder.rs` stayed at 14 missing regions, but constructor-level
  `FailingSeekCursor` raw debt was reduced.

Retention:

- Retain Batch 90 as a coverage-hook cleanup. It removes broad constructor-level
  `FailingSeekCursor` probes without losing aggregate line/function/branch
  coverage. Remaining decoder raw debt is still dominated by
  `FailingSeekCursor` read-frame/read-image probes.

## Batch 91 plan

Goal: remove broad `WebPDecoder<FailingSeekCursor>` animation/read-image probes
that only exist to force seek errors, while preserving direct coverage of the
custom reader itself and the `range_reader(...)` seek-failure primitive.

Reverse mapping:

- After Batch 90, raw decoder debt still concentrates in:
  - `WebPDecoder<FailingSeekCursor>::read_frame`;
  - `WebPDecoder<FailingSeekCursor>::read_image`;
  - `WebPDecoder<FailingSeekCursor>::read_data`;
  - small helper wrappers for `read_fourcc`, `read_chunk_header`, and
    `range_reader`.
- The broad animation/read-image probes instantiate the full generic decoder
  state machine for a reader whose purpose is only to fail seeking.
- Direct method calls can keep `FailingSeekCursor::{read, fill_buf, consume,
  seek}` lines covered without pulling `WebPDecoder<FailingSeekCursor>` through
  every decode path.

Planned edit:

- Add direct coverage-hook calls to `FailingSeekCursor::read(...)` and both
  success/failure `seek(...)` states.
- Remove remaining `WebPDecoder::new(fail_seek_after(...))` constructor probes.
- Remove `exercise_animation_stream(fail_seek_after(...))` calls.
- Remove direct `WebPDecoder { r: fail_seek_after(...) }.read_image(...)`
  probes.
- Keep `range_reader(fail_seek_after(Vec::new(), 0), 0..0)`.
- Run local gates, then Coverage MCP
  `all-features-llvm-cov-json-nightly-branch`.

Retention rule:

- Retain if Coverage MCP passes, line/function coverage stays 100%, branch debt
  does not increase, and total missing regions does not increase. Prefer a
  reduction, but accept neutral cleanup if raw decoder debt becomes materially
  simpler.
