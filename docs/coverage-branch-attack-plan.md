# 100% branch coverage attack plan

This document is the required plan before changing more implementation or fixture
code. It was originally based on Coverage MCP snapshot
`ed33587b-768e-4436-95b0-a5297ae5a2e1`, measured on pushed `main` commit
`818b3cf0e0f76a6bf3c7f67aa0cc91b21e2b9255` with suite
`all-features-lines-branches-nightly`. The current counters below are refreshed
after the zlib-ng compressor private-branch batch.

## Current state

- Test command: `all-features-llvm-cov-json-nightly-branch`
- Command: `cargo +nightly llvm-cov --all-features --branch --json --output-path .coverage-mcp/pillow-rs-image-llvm-nightly-branch.json --no-fail-fast`
- Result: 5 passed, 0 failed
- Current snapshot: `1a0052ae-3f48-45e2-bfe2-1566218dd68d`
- Current measured commit metadata: `d8363ac3e86426044cd99478891a9c0419e79c86`
- Lines: 22066 / 22067
- Branches: 3337 / 3466
- Functions: 1528 / 1528
- Remaining target: 1 line and 129 branches.

## Planned zlib-ng compressor private-branch batch

Coverage MCP snapshot `55cdc496-f51a-4e02-a4fc-c288d62b658a` reports
`src/codecs/compression/zlib_ng.rs` at 1483 / 1484 lines, 371 / 380
branches, and 82 / 82 functions. Its remaining reported branch lines are:

- line 74: level-one chunk-boundary reinsertion uses
  `position >= 1 && available - position >= 3`. Existing parity fixtures hit
  normal true and short-lookahead cases, but one short-circuit edge is still
  missing. Add deterministic private calls to `tokenize_level1()` with chunk
  shapes that end at exactly `position == 0`, `position > 0` with too little
  lookahead, and `position > 0` with enough lookahead.
- line 450: `SlowMatcher::process()` final pending-literal flush uses
  `finishing && position == available && match_available`. Existing image
  compression drives the final true path and at least one false path. Add
  direct matcher calls where `finishing` is false and where finishing reaches
  the boundary with no pending literal.
- line 483: `SlowMatcher::longest_match()` loops while the hash-chain
  candidate remains before the current position. Directly seed the matcher
  with a self/current candidate to drive the complementary loop-exit edge.
- line 681: `LookaheadMediumMatcher::find_match()` accepts a found match only
  when `length >= MIN_MATCH && match_start < position`. Add a direct matcher
  state that finds enough length but with a non-prior/self candidate so the
  second predicate side is exercised, or simplify if that side is unreachable
  after the enclosing candidate guard.
- line 896: `Level9Matcher::process()` has the same final pending-literal
  flush shape as `SlowMatcher`. Add direct false-side calls mirroring the slow
  matcher cases.
- line 941: `Level9Matcher::longest_match()` loops while
  `candidate >= match_offset && candidate < position + match_offset`. Existing
  fixtures miss two loop-condition edges. Add direct seeded states for
  `candidate < match_offset` and `candidate >= position + match_offset`.
- line 1051: `fizzle_matches()` loop condition has a missing
  `adjusted_next.start > limit` edge. With the current constants, that false
  side requires `next.start == 0` while passing earlier guards. Add that direct
  no-change probe or document and simplify if the earlier quick-match indexing
  makes it unreachable.
- line 1224: `EarlyMatcher::longest_match()` breaks when
  `best_length >= nice_match || best_length >= lookahead`. Existing fixtures
  drive the break but miss one short-circuit side. Add direct seeded matcher
  states where `nice_match` is lower than the found length and where lookahead
  is the limiting condition.

No Pillow manifest row is the narrowest oracle for this batch: these branches
are private compressor state-machine predicates after the public PNG/TIFF/WebP
fixtures have already produced deterministic input bytes. The acceptable test
shape is a coverage-only private hook that checks local invariants and then a
Coverage MCP run using the approved line+branch command.

Completed evidence:

- First Coverage MCP run: `0c179a19-df50-49b1-86e4-98073e47b899`, snapshot
  `7baa59f5-166a-4e16-88bb-047748de80a3`; passed and improved `zlib_ng.rs`
  from 371 / 380 to 371 / 376 branches. This proved the matcher-state probes
  were valid but left line 74, line 501, line 947, and line 992.
- Second Coverage MCP run: `41fc08bf-7d6a-4438-b66a-9047e9074d5c`, snapshot
  `c76388ae-b056-44d9-8244-dbe03182d090`; passed and improved `zlib_ng.rs`
  to 368 / 370 branches by removing final-flush and match-start invariant
  branches. This left line 74 and line 994.
- The line-74 search found no reachable chunk/data shape for
  `position >= 1 && available - position < 3`. The reason is structural:
  level-one processing advances by at most `MAX_MATCH` (258) while the loop
  requires `MIN_LOOKAHEAD` (262), so after any processing step at least four
  bytes remain. The second predicate was removed as an invariant.
- Final Coverage MCP run: `c621bdc8-2e2e-4a64-ad94-7dd12419bb80`
- Final Coverage MCP snapshot: `ebb6e723-d062-49f1-ba4f-b87fbb1651a6`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 22013 / 22014 lines, 3334 / 3466 branches, 1527 / 1527 functions.
- Target file: `src/codecs/compression/zlib_ng.rs` is 1536 / 1537 lines,
  368 / 368 branches, and 82 / 82 functions. No branch gaps remain in this
  file.
- Pushed-head verification run:
  `2739934c-34a8-43aa-b563-b7a2eb5022a9`, snapshot
  `edcb6ce0-6419-4caa-9960-8eefe8054c07`, commit
  `06624697151612ff8f0028eb18b7fc4066af89dd`; passed with 5 passed, 0 failed.
  Pushed-head overall coverage is 22015 / 22016 lines, 3334 / 3466 branches,
  and 1527 / 1527 functions. `zlib_ng.rs` remains 1538 / 1539 lines,
  368 / 368 branches, and 82 / 82 functions.

## Planned WebP backward-reference private-branch batch

Coverage MCP snapshot `20ba7c26-5b40-40cc-b4d2-de7742cc8e34` reports
`src/codecs/webp/native/encoder/backward_refs.rs` at 755 / 755 lines,
205 / 210 branches, and 43 / 43 functions. The remaining branch lines are:

- line 115: the hash-chain candidate-search loop condition. Add deterministic
  candidate pixel sequences through the existing private hook to exercise the
  remaining loop predicate side.
- line 141: the MAX_LENGTH/best-distance propagation guard in the hash-chain
  backwards fill loop. Use a long repeated-row style input so the max-length
  propagation condition is evaluated on the complementary side. Retry evidence
  showed the alternating and long constant width-2 inputs did not close this
  line, so do not keep the no-op width-2 probe.
- line 583: `CostManager::insert_min_interval()` finding an existing interval
  that already covers a candidate window. Add a candidate whose cost is not
  better than an existing interval so the `is_none_or` false side is covered.
  First retry evidence showed this still left a branch on the `.find()`
  predicate. Add a candidate spanning two separated existing intervals so the
  closure sees covered and uncovered sub-windows.
- line 631: interval merge after insertion. Add adjacent intervals with the same
  cost and source position so the merge predicate succeeds. First retry
  evidence showed this still left a branch on the chained merge predicate. Add
  adjacent intervals with different costs and with the same cost but different
  source positions to drive the complementary sides. Second retry evidence
  closed line 583 but still left line 631, so add a non-adjacent interval to
  drive the `last.end == interval.start` false side.
- line 732: `trace_backwards()` split handling for repeated offsets. Add a
  manually constructed chain with repeated copy offsets and a later changed
  offset to drive the `next_offset != distance` side.

No Pillow manifest fixture is appropriate for this batch. These are internal
lossless WebP encoder data-structure predicates after ARGB pixels have already
been converted into hash-chain, token, and cost-model state. The narrowest
oracle is a deterministic private probe plus a Coverage MCP line+branch run.

Completed evidence:

- First Coverage MCP run: `63bef4c7-806c-4e23-8000-e9697c3e575a`, snapshot
  `126f6b8a-d2f7-4630-964e-8a1aebebc897`; passed and improved
  `backward_refs.rs` from 205 / 210 to 208 / 212 branches, closing the
  `trace_backwards()` split gap but leaving line 583, 631, 115, and 141.
- Second Coverage MCP run: `0f8217a4-45b9-4e99-9195-6eba4161357c`, snapshot
  `7a5ea347-3831-4e25-a48f-72042f80baa7`; passed and improved
  `backward_refs.rs` to 209 / 212 branches, closing line 583.
- Third Coverage MCP run: `a723086b-19a1-4e4a-b40c-e5aace859e70`, snapshot
  `8dab4023-26c6-4815-9d02-9469b881896e`; passed and improved
  `backward_refs.rs` to 210 / 212 branches, closing line 631.
- Width-2 max-length retry: `95a3d48f-6d1a-4f2e-90eb-009e24ba1b11`, snapshot
  `d9f624c8-e8b2-4550-a381-172b4e10a6b4`; passed but did not improve overall
  or target-file coverage, so the no-op probe was removed.
- Final Coverage MCP run: `cdfd0f6e-7162-44ab-b81b-89e621424907`
- Final Coverage MCP snapshot: `ca87cbef-cf1b-47b8-87c8-1e35510c00fe`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 21960 / 21961 lines, 3337 / 3478 branches, 1527 / 1527 functions.
- Target file: `src/codecs/webp/native/encoder/backward_refs.rs` is
  857 / 857 lines, 210 / 212 branches, and 45 / 45 functions. Remaining target
  gaps in this file are line 115 and line 141.

## Planned WebP histogram private-branch batch

Coverage MCP pushed-head snapshot `edcb6ce0-6419-4caa-9960-8eefe8054c07`
reports `src/codecs/webp/native/encoder/histogram.rs` at 474 / 474 lines,
106 / 112 branches, and 29 / 29 functions. The remaining branch lines are:

- line 367: `entropy_bin_combine()` skips a same-bin candidate when
  `add_eval()` cannot combine below the computed threshold. Add a direct
  same-bin histogram pair that makes `add_eval()` return `None` inside the
  entropy-bin loop.
- line 410: `stochastic_combine()` breaks early when its bounded random pair
  queue reaches nine entries. Add enough mutually mergeable histograms to fill
  the queue.
- line 413: `stochastic_combine()` continues when no candidate pair survived
  the update threshold. Add enough distinct/no-merge histograms to keep the
  queue empty for an iteration.
- line 435: after merging one stochastic pair, queued pairs touching the moved
  cluster are recomputed and dropped if `update_pair()` fails. Add a crafted
  queue/match set where this recomputation fails.
- line 517: `cluster()` invokes entropy-bin pre-combine only when the used
  cluster count is greater than `2 * BIN_SIZE` and quality is below 100. Add a
  deterministic private cluster call with more than 128 used one-pixel
  histograms and `quality < 100`.
- line 524: `cluster()` runs greedy cleanup only if stochastic combine reaches
  the quality threshold. Add complementary private cluster calls where
  stochastic combine returns true and false.

No manifest fixture is the right first move for this batch because these
branches are private lossless-WebP histogram clustering heuristics after
tokens have already been produced. The safe oracle is deterministic token and
histogram state in the existing coverage-only hook, then the approved Coverage
MCP line+branch run.

Completed evidence:

- First Coverage MCP run: `044e9aa9-74fa-43fc-9d09-2b663917c659`, snapshot
  `275d6b6c-eeee-4ba5-840c-271098f8dde6`; passed and improved one branch but
  introduced uncovered source lines because `rustfmt` split previously
  one-line uncovered branch bodies. That shape was not acceptable.
- Corrected Coverage MCP run: `937638c9-2f85-4771-8f54-458cf0ce04e9`
- Corrected Coverage MCP snapshot: `b1aae7b6-1cf6-4e47-af58-ef0cc33d5d55`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 22044 / 22045 lines, 3335 / 3466 branches, 1528 / 1528 functions.
- Target file: `src/codecs/webp/native/encoder/histogram.rs` is
  503 / 503 lines, 107 / 112 branches, and 30 / 30 functions. Remaining target
  gaps in this file are line 367, line 410, line 435, line 517, and line 524.
- Pushed-head verification run:
  `cb76751a-eebd-4423-9f56-9cd558a16f64`, snapshot
  `2110fc56-3565-476c-8eca-0ce50c1db4e8`, commit
  `2a940d27c5dea8e084f9a4df9f3954a1cb867c9e`; passed with 5 passed, 0 failed.
  Pushed-head overall coverage is 22044 / 22045 lines, 3335 / 3466 branches,
  and 1528 / 1528 functions. Histogram remains 503 / 503 lines,
  107 / 112 branches, and 30 / 30 functions.

## Planned WebP VP8 filter-parameter private-branch batch

Coverage MCP pushed-head snapshot `2110fc56-3565-476c-8eca-0ce50c1db4e8`
reports `src/codecs/webp/native/vp8.rs` at 1285 / 1285 lines, 152 / 162
branches, and 57 / 57 functions. LLVM line normalization reports many partial
header/decode lines, but the most contained branch cluster is
`calculate_filter_parameters()`:

- line 1732: segment-adjustment branch. Existing probes cover enabled
  segments; add a disabled-segments case.
- line 1742 and line 1744: loop-filter-adjustment and B-luma mode branches.
  Existing probes cover adjustment-enabled B and non-B cases; add a
  no-adjustment case and keep both B/non-B direct probes.
- line 1753: sharpness shift amount branch. Existing probes cover
  `sharpness_level > 4`; add `sharpness_level` in `1..=4`.
- line 1755: sharpness cap branch. Add a case where the shifted interior limit
  is already below `9 - sharpness_level`.
- line 1760: interior-limit floor branch. Add a low filter level with
  sharpness so shifting produces zero.
- line 1770: high-edge-variance threshold branch. Add filter-level cases below
  15, between 15 and 39, and at least 40 through direct `MacroBlock` inputs.
- line 1795: skipped-coefficients complexity reset only applies to non-B luma
  macroblocks. If direct frame decode setup is too broad, leave this for a
  later VP8 bitstream fixture batch.

No manifest fixture is the narrowest oracle for the filter-parameter sites:
they are deterministic arithmetic over parsed VP8 frame/macroblock state.
Use direct coverage-hook `Vp8Decoder` states, then the approved Coverage MCP
line+branch run. Do not edit the public decoder unless a branch is proven to be
an invariant.

Completed evidence:

- Coverage MCP run: `adba0487-bd71-4287-aa52-c60031a99208`
- Coverage MCP snapshot: `d9e11fd8-f21f-4015-8cf5-fee073f5ae3d`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 22066 / 22067 lines, 3337 / 3466 branches, 1528 / 1528 functions.
- Target file: `src/codecs/webp/native/vp8.rs` improved from 152 / 162 to
  154 / 162 branches, with 1307 / 1307 lines and 57 / 57 functions.
- Closed normalized filter-parameter gaps include the disabled-segments,
  disabled-loop-adjustment, low-sharpness shift, interior-limit floor, and
  low/mid high-edge-variance threshold cases. Remaining normalized VP8 branch
  lines are concentrated in VP8 header parsing, token probability updates,
  coefficient decoding, B-luma loop-filter adjustment, and skipped-coefficients
  complexity reset.
- Pushed-head verification run:
  `3ca96469-d35e-45a9-972f-bf73a43e6ce9`, snapshot
  `1a0052ae-3f48-45e2-bfe2-1566218dd68d`, commit
  `d8363ac3e86426044cd99478891a9c0419e79c86`; passed with 5 passed, 0 failed.
  Pushed-head overall coverage is 22066 / 22067 lines, 3337 / 3466 branches,
  and 1528 / 1528 functions. VP8 remains 1307 / 1307 lines,
  154 / 162 branches, and 57 / 57 functions.

## Planned DEFLATE malformed-zlib private-probe batch

Coverage MCP snapshot `7fcb0487-8ba7-4614-a707-be711570c3c4` reports
`src/codecs/compression/deflate.rs` at 294 / 294 lines, 46 / 50 branches, and
21 / 21 functions. The remaining branch lines are:

- lines 53-56: zlib header validation. Existing image fixtures already cover
  valid zlib streams and at least one invalid-header side, but not all
  short-circuit conditions in the internal wrapper check. Add deterministic
  private probes for:
  - invalid compression method: `00 00 00 00 00 00`;
  - invalid window size: `88 00 00 00 00 00`;
  - invalid FCHECK checksum: `78 00 00 00 00 00`;
  - preset dictionary flag with otherwise valid header check: `78 20 00 00 00 00`.
- line 282: invalid back-reference validation. Existing fixtures cover a
  back-reference before output; add a fixed-Huffman zlib stream that first
  emits one literal byte and then a distance-one back-reference:
  `78 01 73 04 02 00 00 00 00 01`. The intentionally wrong Adler trailer is
  acceptable because this probe only needs to execute the distance-validation
  branch before final checksum validation rejects the full stream. First retry
  evidence showed this probe improved coverage but still left one branch at the
  distance-validation line. That remaining branch is the impossible
  `backwards == 0` side: `DISTANCE_BASE` starts at 1 and all decoded distance
  extras are non-negative. Remove that unreachable half of the predicate and
  keep the `backwards > output.len()` validation.

No Pillow manifest row is useful for the header cases because Pillow only sees
the enclosing PNG/TIFF stream result, not each short-circuit condition in this
internal zlib wrapper. A private byte-level probe is the narrowest oracle for
the implementation branch behavior. If the fixed-Huffman stream does not
improve line 282, revert or revise that single probe before moving to a broader
PNG/TIFF malformed fixture.

Completed evidence:

- First Coverage MCP run: `642b0511-d5c6-4314-9d09-2531a2f5f9e5`, snapshot
  `dfa2a5c8-240d-41f8-bf9e-8143f74f53ed`; passed and improved `deflate.rs`
  from 46 / 50 to 49 / 50 branches. This proved the malformed zlib-header
  probes were correct and that the fixed-Huffman back-reference probe reached
  the distance-validation predicate.
- Corrected Coverage MCP run: `c0b62992-79b7-469a-86d9-1495d4f9e55e`
- Corrected Coverage MCP snapshot: `c8f1e2da-fbcd-4a83-ab56-1cd337d430ae`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 21858 / 21859 lines, 3332 / 3476 branches, 1525 / 1525 functions.
- Target file: `src/codecs/compression/deflate.rs` is 304 / 304 lines,
  48 / 48 branches, and 22 / 22 functions.

## Planned WebP predictor private-branch batch

Coverage MCP snapshot `5346a218-89b1-46e9-aea4-68d4a83522ee` reports
`src/codecs/webp/native/encoder/predictor.rs` at 238 / 238 lines, 37 / 40
branches, and 22 / 22 functions. The remaining branch lines are:

- line 118: the private `tile_histogram` preload path has covered
  `start_y > 0` with `start_y < height`, but not the bottom-boundary case where
  `start_y == height`. Add a direct private hook call with `height == start_y`
  so this boundary branch is covered without inventing a public WebP fixture
  for an internal tile query.
- line 139: inside `tile_histogram`, a transparent pixel currently covers the
  `x == 0 && y != 0` true side. Add a transparent non-left-column pixel so the
  false side of that predicate is covered while still exercising the alpha-zero
  residual logic. First retry evidence showed that this only covers the
  short-circuited `x == 0` false side; the remaining branch is the `y != 0`
  false side. Add a transparent top-left pixel (`x == 0`, `y == 0`) as the
  corrected probe.
- line 200: inside `apply_modes`, the same transparent-pixel neighbor-update
  predicate is missing the complementary side. Add an `apply_fixed` hook image
  with a transparent non-left-column pixel. As with `tile_histogram`, the
  corrected probe also needs a transparent top-left pixel to drive `x == 0`
  true and `y != 0` false.

No new Pillow manifest fixture is the right tool for this batch because all
three gaps are private encoder helper predicates after the ARGB source pixels
and predictor mode inputs have already been constructed. The coverage oracle is
therefore a deterministic private hook, followed by a single Coverage MCP
line+branch run.

Completed evidence:

- First retry Coverage MCP run: `8a671618-2a41-4469-b2c6-2374b84150e4`,
  snapshot `cccbf409-205d-488e-b255-2f128073820f`; passed and improved
  `predictor.rs` from 37 / 40 to 38 / 40 branches. This proved the
  `start_y == height` tile-boundary probe was correct and showed that the
  transparent non-left-column probe did not drive the remaining `y != 0` false
  side.
- Corrected Coverage MCP run: `837e98fd-512e-4d6b-8fca-c591ff2d708c`
- Corrected Coverage MCP snapshot: `010a4828-7a2b-446d-982b-7ebc4c5568ca`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 21847 / 21848 lines, 3330 / 3478 branches, 1524 / 1524 functions.
- Target file: `src/codecs/webp/native/encoder/predictor.rs` is 250 / 250
  lines, 40 / 40 branches, and 22 / 22 functions.

## Planned WebP lossless-transform invariant cleanup batch

Coverage MCP snapshot `e539f08a-7f14-4c0a-8adf-b5bb444f6f75` reports
`src/codecs/webp/native/lossless_transform.rs` at 462 / 462 lines, 30 / 32
branches, and 27 / 27 functions. The remaining branch lines are:

- line 523: `if width == 0 || height == 0`, where the existing private coverage
  hook only drives the `width == 0` early return. Add a deterministic hook call
  with `width > 0` and `height == 0` so the short-circuit predicate's second
  true side is exercised.
- line 570: `if packed_image_width_in_blocks > 0`, inside the same helper after
  the `width == 0 || height == 0` return. Once execution reaches that point,
  `width > 0`, and `width.div_ceil(pixels_per_packed_byte_u8.into())` is
  therefore strictly positive. This branch is an internal invariant, not a
  public WebP byte-stream behavior. Remove the guard and execute the final-block
  copy unconditionally.

No Pillow manifest fixture is useful for this batch because both sites are
inside the small-table color-indexing helper after decode has already built the
transform inputs. The branch plan is therefore a minimal private hook plus
invariant cleanup, then a single Coverage MCP run with line and branch coverage.

Completed evidence:

- Coverage MCP run: `abd3d27a-e929-49b2-a49c-1d57b1a84987`
- Coverage MCP snapshot: `cb3f057a-1633-4b00-9232-3962046beec2`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 21835 / 21836 lines, 3327 / 3478 branches, 1524 / 1524 functions.
- Target file: `src/codecs/webp/native/lossless_transform.rs` is 468 / 468
  lines, 30 / 30 branches, and 27 / 27 functions.

Coverage MCP reports this warning for LLVM JSON:

> LLVM JSON segments are normalized to segment start lines; aggregate region
> coverage is preserved from summaries.

So the file summary counters are the source of truth for branch counts. The
line-range view is used to identify where to inspect, not to sum branch counts.

## Exploration findings

The latest measured pushed `main` state before the current WebP retry is:

- Commit: `287793718391493e0e7fd636d3b9899227f52c39`
- Coverage MCP run: `3ae04e9f-1faf-4fd0-9ab5-4ebf1b3d0f9b`
- Coverage MCP snapshot: `523d37a9-9b05-4cd3-bb95-938f26dc2904`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Lines: 21703 / 21704
- Branches: 3316 / 3484
- Functions: 1518 / 1518

The current dirty WebP experiment was also measured:

- Coverage MCP run: `efd4c97c-664b-4bb0-931f-fa3bdf7bae05`
- Coverage MCP snapshot: `9c799eb1-e0c0-47e6-ac35-4416b3549044`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Lines: 21720 / 21721
- Branches: 3316 / 3484
- Functions: 1519 / 1519

That experiment did not reduce the remaining branch count. The first WebP hook
attempt covered the bool-encoder carry path with an existing previous byte and
the transparent-cleanup bottom-right corner path. The successful retry targeted
the complementary sides:

- `src/codecs/webp/encode/vp8/bool_enc.rs:98`: drive a carry with no previous
  output byte, exercising the `if let Some(previous)` false side.
- `src/codecs/webp/encode/vp8/encoder.rs:447`: call transparent cleanup with
  `height > full_height` but `width == full_width`, exercising the false side
  of the bottom-right partial-block check.

Coverage MCP proved this retry improved branch coverage by two branches. If a
future WebP hook does not improve branch coverage, revert it and move to the
next manifest-driven malformed decode fixture batch.

The repo already has the correct oracle workflow:

- `manifest.yaml` declares the source assets, encode/decode params, planned
  rows, and the pinned Pillow 12.2.0 oracle contract.
- `scripts/generate_test_assets.py` creates deterministic source fixtures for
  JPEG, PNG, GIF, BMP, TIFF, WebP, and ICO.
- `scripts/generate_decode_refs.py` opens assets through the pinned Pillow
  oracle, writes exact encoded bytes and decoded raw pixel references, and
  updates `tests/fixtures/coverage_matrix.json`.
- `tests/coverage_matrix_tests.rs` is the only integration parity suite; it
  checks exact decoded pixels and exact encoded bytes.
- `scripts/libwebp_fixture_encoder.c` is available for WebP fixtures that need
  libwebp bitstream shapes Pillow cannot easily produce from `Image.save()`.

The right default strategy is therefore:

1. Add or mutate a deterministic source asset in `scripts/generate_test_assets.py`.
2. Add a manifest row in the target format section.
3. Regenerate references with `.oracle-venv/bin/python scripts/generate_decode_refs.py --format <format>`.
4. Keep the row active only if it has exact Pillow parity.
5. Use code cleanup only when a branch is proven unreachable, redundant, or a
   defensive overflow branch that cannot be reached through public image bytes.
6. Run Coverage MCP after a completed batch, not after every individual guess.

## Dataset families to add

These fixture datasets cover most remaining branch gaps with real image inputs:

1. JPEG malformed and progressive scan corpus
   - Add more generated JPEG variants in `gen_jpeg()`.
   - Focus on progressive spectral-successive paths, restart interval edges,
     scan padding, stuffed markers, zero-length/short entropy, grayscale/CMYK
     progressive cases, and invalid marker lengths.
   - Oracle source: Pillow/libjpeg-turbo 3.1.4.1.

2. TIFF tag and storage corpus
   - Add targeted TIFF byte-structure fixtures in `gen_tiff()`.
   - Focus on unusual tag types/counts, missing/derived strip/tile byte counts,
     predictor combinations, photometric/color map variants, endian variants,
     and short/empty IFD cases tolerated or rejected by Pillow.
   - Oracle source: Pillow/libtiff 4.7.1.

3. BMP/DIB malformed header corpus
   - Add hand-built BMP rows in `gen_bmp()`.
   - Focus on header sizes, palette sizes, top-down/height sign variants,
     truncated masks, RLE escape modes, odd row padding, and unsupported bpp
     rejection matching Pillow.
   - Oracle source: Pillow BMP decoder behavior.

4. ICO directory and embedded-image corpus
   - Add hand-built ICO/CUR variants in `gen_ico()`.
   - Focus on directory count/entry bounds, duplicate sizes, PNG vs BMP entry
     selection, malformed DIB headers, cursor hotspot fields, and oversized or
     zero dimensions.
   - Oracle source: Pillow ICO/CUR behavior.

5. GIF animation and palette encoder corpus
   - Add PNG/GIF source assets in `gen_png()`/`gen_gif()` and encode rows in
     the GIF manifest section.
   - Focus on disposal/background/previous branches, local-vs-global palettes,
     transparent index placement, palette sorting, and high-color quantization.
   - Oracle source: Pillow GIF encoder/decoder behavior.

6. WebP lossy VP8 corpus
   - Use Pillow-generated lossy WebP assets for normal paths.
   - Use `scripts/libwebp_fixture_encoder.c` when a specific VP8 partition,
     coefficient, segmentation, filter, or header state cannot be reached
     through Pillow save options.
   - Focus on native VP8 decode branches and lossy encoder partition/bool
     encoder branches.
   - Oracle source: Pillow/libwebp 1.6.0 for public behavior.

7. WebP lossless VP8L corpus
   - Extend existing predictor/palette/noise fixtures in `gen_webp()`.
   - Focus on transform combinations, cache hits/misses, palette sizes around
     thresholds, histogram clustering thresholds, and Huffman code edge cases.
   - Use Pillow-generated assets first, then `libwebp_fixture_encoder.c` for
     specific bitstream shapes.

8. PNG/zlib/deflate corpus
   - Existing `zlib_boundary_source.png` added useful coverage and exposed a
     real zlib level-1 bug.
   - Add narrower source images for stored, fixed, dynamic, short-window, exact
     minimum-match, and row-boundary cases.
   - For decode-only DEFLATE paths, add malformed PNG zlib streams with exact
     Pillow accept/reject behavior.

## File-by-file pending plan

| Priority | File | Missing branches | MCP target lines | Attack plan |
|---:|---|---:|---|---|
| 1 | `src/codecs/compression/zlib_ng.rs` | 14 | 74, 321, 366, 399, 597, 767, 812, 857, 952, 967, 969, 978, 1140 | Add PNG encode fixtures for exact min-match, row-boundary, repeated/noisy windows, and compression levels 1/3/6/9. If branches stay unreachable, compare against zlib-ng control flow and remove impossible Rust-only defensive branches. |
| 2 | `src/codecs/jpeg/decode/progressive.rs` | 14 | 84, 103, 133, 155, 158, 171, 179, 182, 205, 522, 574, 673 | Generate progressive JPEG variants: grayscale, CMYK, multi-scan spectral selection, successive approximation, early EOB runs, restart intervals, and truncated scan tails. Add only rows where Pillow gives deterministic status/pixels. |
| 3 | `src/codecs/tiff/decode.rs` | 14 | 32, 43, 51, 74, 79, 113, 253, 294, 416, 467 | Add TIFF fixtures with alternate endian, malformed/short IFD entries, omitted byte counts, tiled/stripped variants, predictor variants, and unusual sample formats. Use hand-built TIFF bytes when Pillow cannot save the shape. |
| 4 | `src/codecs/bmp/decode.rs` | 13 | 222, 247, 263, 294, 493, 503 | Add BMP fixtures for palette-length edges, DIB header sizes, bitfield masks, top-down RLE rejection, short pixel data, and RLE4/RLE8 escape variants. |
| 5 | `src/codecs/ico/decode.rs` | 13 | 43, 69, 100, 109, 136, 192, 293, 344, 380, 382, 398 | Add ICO/CUR files with malformed directory entries, zero/256 dimensions, PNG/BMP mixed entries, invalid offsets/sizes, cursor hotspot variants, and embedded BMP bit-depth variants. |
| 6 | `src/codecs/gif/encode.rs` | 11 | 118, 132, 175, 409, 444, 914, 930, 937, 1053 | Add GIF encode rows for single-frame vs animation, no background vs explicit background, disposal previous/background/none, transparent index conflicts, local/global palette forcing, and high-color RGBA quantization. |
| 7 | `src/codecs/webp/native/vp8.rs` | 10 | 970, 1030, 1044, 1053, 1067, 1081, 1085, 1125, 1129, 1132, 1139, 1149, 1156, 1166, 1346, 1395, 1402, 1732, 1742, 1744, 1753, 1755, 1760, 1770, 1795 | Add WebP lossy bitstreams covering VP8 header/partition/filter/macroblock branches. Start with Pillow lossy quality/method/filter-like images; use `libwebp_fixture_encoder.c` for specific partition and coefficient forms. |
| 8 | `src/codecs/webp/native/decoder.rs` | 10 | 196, 264, 274-277, 427, 533, 543, 595 | Add WebP container fixtures: extended chunks, animation flags, ICC/EXIF/XMP combinations, invalid chunk sizes, missing VP8/VP8L chunks, and alpha chunk variants. |
| 9 | `src/codecs/webp/native/encoder.rs` | 8 | 247, 379, 392, 395, 877, 1027, 1121 | Add WebP encode rows for tiny, odd, transparent, opaque, high-entropy, palette-like, CMYK-converted, and exact threshold dimensions. Verify encoded bytes against Pillow. |
| 10 | `src/codecs/jpeg/encode/mod.rs` | 7 | 48, 373, 397, 875, 1033 | Add JPEG encode rows for metadata/no metadata, grayscale/RGB/CMYK, restart intervals, progressive odd dimensions, unsupported params, and quality/subsampling boundaries. |
| 11 | `src/codecs/jpeg/decode/parser.rs` | 7 | 99, 164, 278, 382, 407 | Add parser-focused JPEG fixtures: duplicate tables, unknown APP/COM markers, short segment lengths, marker padding, restart markers outside entropy, and trailing data. |
| 12 | `src/codecs/webp/native/encoder/histogram.rs` | 6 | 367, 410, 413, 435, 517, 524 | Add WebP lossless encode sources around histogram clustering thresholds: solid, checker, sparse, noise, palette sizes 15/16/17/255/256, and random-walk gradients. |
| 13 | `src/codecs/webp/native/encoder/backward_refs.rs` | 5 | 115, 141, 583, 631, 732 | Add WebP lossless sources targeting LZ77/cache decisions: repeated rows, long-distance repeats, short exact repeats, cache hits, and no-match noisy data. |
| 14 | `src/codecs/jpeg/decode/decode.rs` | 5 | 161, 290, 394, 405, 410 | Add JPEG decode fixtures for component count/color transform branches, grayscale/CMYK/RGB scan handling, restart recovery, and partial MCU edges. |
| 15 | `src/codecs/png/decode.rs` | 5 | 32, 44-45, 53, 407 | Add PNG decode fixtures for signature/header errors, optional chunks, palette/transparency boundaries, and zlib stream shape variants. |
| 16 | `src/codecs/gif/decode.rs` | 5 | 60, 164, 246, 317 | Add GIF decode fixtures for no global palette, local palette only, GCE transparency/disposal combinations, interlace, loop/comment/application extensions, and truncated extension blocks. |
| 17 | `src/codecs/compression/deflate.rs` | 4 | 54-56, 282 | Add PNG malformed zlib streams for fixed/dynamic Huffman errors, stored block complement errors, short header, and exact backreference boundary cases. |
| 18 | `src/codecs/jpeg/decode/bit_reader.rs` | 4 | 119, 126 | Add JPEG entropy fixtures with byte stuffing, marker-like bytes, EOI inside entropy, and truncated bit tails. |
| 19 | `src/codecs/webp/native/encoder/predictor.rs` | 3 | 118, 139, 200 | Add WebP lossless source images that select each predictor threshold: horizontal, vertical, radial/quadrant, sparse, and sharp edges. |
| 20 | `src/codecs/webp/native/lossless.rs` | 2 | summary says 2 branches; line view has many normalized partial sites | Treat as LLVM-normalized. Use VP8L fixtures for transforms, color cache, palette, meta-prefix, and Huffman edge cases, then re-query exact remaining ranges. |
| 21 | `src/codecs/webp/native/lossless_transform.rs` | 2 | 523, 570 | Add VP8L fixtures for inverse transform edge rows/columns and predictor/color-transform boundaries. |
| 22 | `src/codecs/webp/native/extended.rs` | 2 | 46-47 | Add WebP extended-header fixtures for feature flags and invalid/missing extended chunks. |
| 23 | `src/codecs/jpeg/decode/huffman.rs` | 2 | 100, 181 | Add JPEG Huffman fixtures with deep code lengths, invalid/unused symbols, and entropy streams that force table boundary decisions. |
| 24 | `src/codecs/webp/encode/vp8/encoder.rs` | 1 | 441 | Add WebP lossy encode source that triggers the remaining VP8 encoder branch; inspect line 441 first to decide if fixture or cleanup. |
| 25 | `src/codecs/webp/encode/vp8/bool_enc.rs` | 1 | 98 | Add WebP lossy encode data that drives the bool encoder across the remaining carry/range branch, likely high-entropy or threshold-sized image. |

## Completed while executing this plan

### PNG/BMP encode error parity batch

- Commit: `e4a25aa00f4a0f6cad0834dd39c7be7bac0cc12a`
- Coverage MCP run: `eed24a4b-6fcc-4281-8181-35360cbccaa9`
- Coverage MCP snapshot: `a3e804e1-a564-4ca3-9cfb-e9a14261a0a9`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Coverage movement: branches improved from 3310 / 3484 to 3312 / 3484.
- `src/codecs/png/encode.rs`: now 236 / 236 lines and 36 / 36 branches.
- `src/codecs/bmp/encode.rs`: now 182 / 182 lines and 8 / 8 branches.
- Added manifest-driven oracle rows:
  - `png.enc_error_zero_height`
  - `bmp.enc_error_oversized_height`
  - `bmp.enc_error_file_size_overflow`

### TIFF LZW byte-aligned parity batch

- Commit: `f565a23813f9f90b482ea9500b1d4122eab17daa`
- Coverage MCP run: `4ff5ca4f-7666-470c-90e6-a8ae314e4d03`
- Coverage MCP snapshot: `dc567aaf-3808-46f4-8d86-5296b6b74e7e`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Coverage movement: branches improved from 3312 / 3484 to 3313 / 3484.
- `src/codecs/tiff/encode.rs`: now 344 / 344 lines and 86 / 86 branches.
- Added manifest-driven oracle row:
  - `tiff.enc_lzw_byte_aligned`
- Exploration note: a deterministic 16-pixel grayscale PNG source drives the
  TIFF LZW writer to finish with zero pending bits, covering the `used == 0`
  side of `MsbWriter::finish`.

### ICO zero-entry size-filter parity batch

- Commit: `61af0e202a12d69f9638a8b5a181a2bea85b0091`
- Coverage MCP run: `73489388-4f76-439c-a9fa-7bbdbfa1fd6a`
- Coverage MCP snapshot: `7df3a4f3-a4af-4ac5-9382-0c8a7a11039d`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Coverage movement: branches improved from 3313 / 3484 to 3315 / 3484.
- `src/codecs/ico/encode.rs`: now 305 / 305 lines and 50 / 50 branches.
- Added manifest-driven oracle rows:
  - `ico.enc_empty_sizes`
  - `ico.enc_width_filtered_sizes`
  - `ico.enc_height_filtered_sizes`
  - `ico.enc_width_cap_filtered_sizes`
  - `ico.enc_height_cap_filtered_sizes`
- Exploration note: Pillow emits a valid six-byte zero-entry ICO header when
  requested sizes are empty or fully filtered. These rows are marked
  `encoded_only` because Pillow cannot re-open that zero-entry ICO for pixel
  roundtrip evidence, so parity is proven by exact encoded bytes.

### JPEG Huffman prefix-gap coverage-hook batch

- Commit: `218a0f6a3d7bf07cc878e24eb548879cdc3f0009`
- Coverage MCP run: `9ed703ab-8c6e-4cf1-9028-5339d2ae53b1`
- Coverage MCP snapshot: `bbf441d1-abaf-4f7a-a677-72573cc2b5ff`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Coverage movement: branches improved from 3315 / 3484 to 3316 / 3484.
- `src/codecs/jpeg/encode/huffman.rs`: now 130 / 130 lines and 24 / 24 branches.
- Exploration note: a deterministic private frequency vector with 18 nonzero
  symbols at powers-of-two frequencies drives the JPEG optimal-Huffman
  length-limiting pass through the empty-prefix-bucket backup branch at line
  141. This is exercised through the existing `#[cfg(coverage)]` private hook;
  production JPEG encoding behavior is unchanged.

### WebP VP8 complementary-branch coverage-hook batch

- Coverage MCP run: `5acc578d-90b2-413d-9b40-cd6bf291fe38`
- Coverage MCP snapshot: `897bfef9-b1b3-42f9-9f0b-448435e74ee3`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Coverage movement: branches improved from 3316 / 3484 to 3318 / 3484.
- `src/codecs/webp/encode/vp8/bool_enc.rs`: now 73 / 73 lines and 16 / 16
  branches.
- `src/codecs/webp/encode/vp8/encoder.rs`: now 480 / 480 lines and 32 / 32
  branches.
- Exploration note: the first WebP hook attempt did not improve branch counts
  because it exercised already-covered sides. The successful retry drives the
  bool-encoder carry path with no prior output byte and calls transparent-area
  cleanup with an exact full-width but partial-height image. Both are private
  `#[cfg(coverage)]` hooks; production WebP encoding behavior is unchanged.

### Zlib-ng `fizzle_matches` private-branch batch

- Starting evidence: pushed `main` commit
  `ac0645c25f989d35ac7ea86387e55e998339752a`, Coverage MCP snapshot
  `1c67e9a6-d1c2-41da-807d-af0420f7de29`.
- Current `src/codecs/compression/zlib_ng.rs`: 1405 / 1406 lines and
  366 / 380 branches.
- Target lines: 952, 967, 969, and 978 inside private `fizzle_matches`.
- Implemented:
  - Exercise the early return where `current.length > next.start + 1`.
  - Exercise the loop guard where `next.match_start > 1` is false.
  - Exercise the one-step adjustment where `changed` is true but
    `adjusted_next.length == 2`, so the final assignment is intentionally
    skipped.
  - Exercise the multi-step adjustment where the current match is fully
    consumed, so the final assignment is taken.
- Coverage MCP run: `8bfa16a5-be20-4947-9658-e9b7f1694b9f`
- Coverage MCP snapshot: `388a08ab-3594-4c24-b84c-187f32cd1470`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Coverage movement: branches improved from 3318 / 3484 to 3321 / 3484.
- `src/codecs/compression/zlib_ng.rs`: improved from 366 / 380 to
  369 / 380 branches.
- Rationale: these are private zlib-ng lazy-match states. Direct construction
  of `MediumMatch` values gives deterministic evidence for branch behavior
  without changing public PNG/zlib byte output. If Coverage MCP does not reduce
  the zlib branch gap, revert the hook additions and move to the next
  manifest-driven decode batch.

### Zlib-ng self-candidate matcher batch

- Starting evidence after the `fizzle_matches` hook run:
  Coverage MCP snapshot `388a08ab-3594-4c24-b84c-187f32cd1470`.
- Current movement from the first zlib hook: branches improved from
  3318 / 3484 to 3321 / 3484; `zlib_ng.rs` improved from 366 / 380 to
  369 / 380 branches.
- Target lines after hook insertion: 385 in `SlowMatcher::process` and 831 in
  the level-nine matcher process loop.
- Implemented:
  - Pre-insert the current position into each matcher hash table.
  - Run one process step with `lookahead >= MIN_MATCH`.
  - This makes `quick_insert(position)` return `position`, covering
    `candidate != 0` true with `candidate < position` false.
- Failed hook attempt: Coverage MCP run
  `5b7ad024-7667-450c-912c-1bb5ca285d12` failed because the hook asserted
  `slow.match_available`, which is not stable after the matcher advances
  through finalization. No coverage snapshot was ingested for that failed run.
- Successful retry: Coverage MCP run
  `14e86b8e-bf6e-46d3-a2b2-b449d4fbb828`, snapshot
  `7dad0fef-e1e7-4b40-a6ef-a2336dad942c`.
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Coverage movement: branches improved from 3321 / 3484 to 3323 / 3484.
- `src/codecs/compression/zlib_ng.rs`: improved from 369 / 380 to
  371 / 380 branches.
- Rationale: this is a private hash-table collision/self-candidate state and is
  independent of public compressed byte parity. Keep it behind `#[cfg(coverage)]`.

### JPEG bit-reader debug-assert branch batch

- Starting evidence: pushed `main` commit
  `9ff6f3ff85c39a4cff33226601b1c362a9873605`, Coverage MCP snapshot
  `e0741451-3f88-476f-9056-a9d3cbfbb163`.
- Current `src/codecs/jpeg/decode/bit_reader.rs`: 90 / 90 lines and
  22 / 26 branches.
- Target lines: 119 and 126.
- Implemented:
  - Use the existing `#[cfg(coverage)]` private hook.
  - Exercise `peek_bits(0)` and `peek_bits(bits + 1)` under
    `std::panic::catch_unwind` to cover both invalid debug-assert sides.
  - Exercise `get_bits(0)` and `get_bits(bits + 1)` under
    `std::panic::catch_unwind` to cover both invalid debug-assert sides.
- Coverage MCP run: `6f0e74a7-ea7a-4c2e-938f-097228bcbd95`
- Coverage MCP snapshot: `baa84e21-a8cc-4a6b-9f4e-2bd05f291b1f`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Coverage movement: branches improved from 3323 / 3484 to 3327 / 3484.
- `src/codecs/jpeg/decode/bit_reader.rs`: now 103 / 103 lines and
  26 / 26 branches.
- Rationale: these are debug-only contract assertions, not production JPEG
  decode behavior. Catching the expected debug assertion panic keeps the branch
  evidence coverage-only and does not alter public byte/pixel parity.

### JPEG Huffman invariant-branch cleanup batch

- Starting evidence: pushed `main` commit
  `81a5bac306ff1d2760fdeca64aa50948bae7cd1d`, Coverage MCP snapshot
  `0edf31c2-a638-459d-a5df-e4f703ab17ec`.
- Current `src/codecs/jpeg/decode/huffman.rs`: 118 / 118 lines and
  20 / 22 branches.
- Target lines: 100 and 181.
- Implemented:
  - Remove the defensive `idx < 256` guard in the lookahead fill loop. For
    validated canonical JPEG Huffman codes with `l <= HUFF_LOOKAHEAD`,
    `lookbits + ctr` is always in `0..256`; overfull code tables return
    `HuffTable::empty()` before the lookahead fill.
  - Replace the slow-loop `if !br.ensure(1) { return None; }` with an
    unconditional `br.ensure(1);`. After the initial `ensure(min)` succeeds,
    the bit reader either has a bit available or pads with IJG-compatible zero
    bits for `ensure(1)`, so the false side is unreachable inside the loop.
- Rationale: these are invariant/defensive branches, not observable Pillow
  behavior. Simplifying them removes unreachable branch obligations while
  preserving JPEG decode semantics.
- Coverage MCP run: `84414075-ff10-443e-8ccc-79ae5907bc07`
- Coverage MCP snapshot: `0d998623-a8b5-4797-b6f7-afb1cb42be65`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Coverage movement: branch obligations changed from 3327 / 3484 to
  3325 / 3480, reducing remaining missing branches from 157 to 155.
- `src/codecs/jpeg/decode/huffman.rs`: now 120 / 120 lines and
  18 / 18 branches.

### WebP extended frame predicate coverage-hook batch

- Starting evidence: pushed `main` commit
  `f460bfc61821a81ca70055a2e8c9b03ebe93a5c3`, Coverage MCP snapshot
  `2f5a6603-8f87-43ea-9924-4441048e39b1`.
- Current `src/codecs/webp/native/extended.rs`: 216 / 216 lines and
  34 / 36 branches.
- Target lines: 46-47 in `composite_frame`.
- Implemented:
  - Add `extended::__coverage_exercise_private_branches()` and call it from
    the native WebP coverage hook.
  - Exercise the `frame_offset_y == 0` false side with a 1x1 alpha frame placed
    at `y = 1`.
  - Exercise the `frame_width == canvas_width` false side with a 1x1 alpha
    frame placed at origin on a 2x2 canvas.
- Rationale: these are private frame-composition predicate branches. Tiny
  deterministic RGBA buffers prove the branch behavior without adding WebP
  fixture bytes or changing public WebP decode behavior.
- Coverage MCP run: `4b19afe0-ee77-4572-beb0-34459ba3cad9`
- Coverage MCP snapshot: `caf819ae-8d8c-4409-a726-3d4db8b9eb85`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Coverage movement: branches improved from 3325 / 3480 to 3327 / 3480.
- `src/codecs/webp/native/extended.rs`: now 231 / 231 lines and
  36 / 36 branches.

## Execution order

1. Small cleanup pass
   - Inspect single-branch lines first: PNG encode, BMP encode, TIFF encode,
     WebP VP8 bool encoder, JPEG Huffman encode.
   - Remove redundant branches only when behavior is provably equivalent and
     covered by existing manifest rows.

2. Decode malformed-input pass
   - BMP, ICO, PNG, GIF, JPEG parser/bit-reader.
   - These are high value because hand-built malformed files usually cover many
     defensive branches without changing encoder algorithms.

3. JPEG progressive pass
   - Use one input at a time and trace first divergence if any new row fails.
   - Keep new rows active only when exact Pillow pixel parity is achieved.

4. TIFF pass
   - Generate compact byte-level TIFFs for tag/storage variants.
   - Avoid broad random fixtures; every fixture must document the specific tag
     or storage edge it targets.

5. WebP VP8/VP8L pass
   - Start with Pillow-generated images.
   - Switch to `scripts/libwebp_fixture_encoder.c` only when Pillow cannot
     produce the bitstream branch shape.

6. zlib/deflate pass
   - Add PNG encode/decode fixtures for compressor and inflater branch lines.
   - If only overflow/checked arithmetic branches remain, document why public
     image bytes cannot reach them and simplify the implementation if safe.

## Batch templates

Use these templates before editing each batch. They keep the work fixture-first
and prevent broad random asset generation.

### Single-branch cleanup batch

- Inspect the MCP target line with `source_context` or a bounded local source
  read.
- Decide one of:
  - add a manifest row if a public Pillow input can reach the branch;
  - simplify the branch if both sides are behaviorally identical;
  - leave it and move to fixture work if the branch is defensive and still
    needs evidence.
- Best initial files:
  - `src/codecs/png/encode.rs`
  - `src/codecs/bmp/encode.rs`
  - `src/codecs/tiff/encode.rs`
  - `src/codecs/jpeg/encode/huffman.rs`
  - `src/codecs/webp/encode/vp8/bool_enc.rs`

### Malformed decode fixture batch

- Add byte-level fixture builders in the matching `gen_<format>()` function.
- Add manifest decode rows with clear `expect_error` or Pillow-tolerated
  descriptions.
- Regenerate only that format:
  `.oracle-venv/bin/python scripts/generate_test_assets.py --format <format>`
  `.oracle-venv/bin/python scripts/generate_decode_refs.py --format <format>`
- If a new row fails parity, isolate that single input and find the first
  implementation/Pillow divergence before adding more rows.
- Best initial formats:
  - BMP: malformed DIB/RLE/palette variants.
  - ICO: malformed directory and embedded BMP/PNG variants.
  - PNG: malformed zlib/chunk variants.
  - GIF: extension/disposal/palette variants.
  - JPEG: marker, Huffman, entropy, and progressive scan variants.

### Encoder corpus batch

- Add deterministic source images in `gen_png()`, `gen_webp()`, or the target
  source-format generator.
- Add encode rows in the target format section of `manifest.yaml`.
- Regenerate the target format references.
- Compare failures at exact encoded bytes first; only inspect decoded pixel
  roundtrip after encoded byte parity is either achieved or explicitly not
  supported by the row.
- Best initial encoders:
  - GIF palette/disposal/background rows.
  - JPEG quality/subsampling/progressive/restart rows.
  - WebP lossy/lossless threshold rows.
  - TIFF predictor/compression/sample-format rows.

### WebP special corpus batch

- Try Pillow-generated WebP assets first.
- If the missing branch requires a bitstream shape that Pillow cannot emit,
  use or extend `scripts/libwebp_fixture_encoder.c`.
- Keep any C-generated fixture deterministic and record the libwebp option or
  bitstream purpose in the manifest row description.
- Validate through Pillow references before treating the fixture as useful.

## Verification rule

After implementing a complete batch from this plan:

1. Regenerate affected assets/references with the pinned Pillow oracle.
2. Run only the approved Coverage MCP command:
   `all-features-llvm-cov-json-nightly-branch`.
3. Query summary, insights, files, and the files changed in the batch.
4. Commit only if:
   - all matrix tests pass;
   - coverage artifact is ingested;
   - no generated local-only files are staged;
   - new rows are fixture-based and exact-byte/pixel parity rows.
