# 100% branch coverage attack plan

This document is the required plan before changing more implementation or fixture
code. It was originally based on Coverage MCP snapshot
`ed33587b-768e-4436-95b0-a5297ae5a2e1`, measured on pushed `main` commit
`818b3cf0e0f76a6bf3c7f67aa0cc91b21e2b9255` with suite
`all-features-lines-branches-nightly`. The current counters below are refreshed
from Coverage MCP before each implementation sweep.

## Current state

- Test command: `all-features-llvm-cov-json-nightly-branch`
- Command: `cargo +nightly llvm-cov --all-features --branch --json --output-path .coverage-mcp/pillow-rs-image-llvm-nightly-branch.json --no-fail-fast`
- Result: 5 passed, 0 failed
- Current snapshot: `41e480a1-67fd-4289-9c4a-5f02d7968531`
- Current measured commit metadata: `7f1e33a55ecbe00c439f62ddc3040d18b559f309`
- Lines: 24072 / 24078
- Branches: 3424 / 3438
- Functions: 1576 / 1576
- Regions: 39635 / 40705
- Remaining target: 6 lines, 14 branches, and 1070 regions.
- Remaining branch map from this snapshot:
  - `src/codecs/webp/native/decoder.rs`: 82 / 88 branches, 6 missing.
  - `src/codecs/webp/native/vp8.rs`: 154 / 160 branches, 6 missing.
  - `src/codecs/webp/native/lossless.rs`: 108 / 110 branches, 2 missing.
- Remaining line gaps from this snapshot:
  - `src/codecs/webp/native/vp8.rs`: 1413 / 1417 lines, 6 branch missing.
  - `src/codecs/webp/native/lossless.rs`: 543 / 544 lines,
    2 branch missing.
  - `src/types/dynamic.rs`: 826 / 827 lines, 0 branch missing.
- Files now at 100% branch coverage from this sweep:
  - `src/codecs/tiff/decode.rs`: 120 / 120 branches.
  - `src/codecs/jpeg/decode/progressive.rs`: 112 / 112 branches.
  - `src/codecs/bmp/decode.rs`: 112 / 112 branches and now
    759 / 759 regions.
- Note: LLVM JSON line segments are lossy. File aggregate branch totals are the
  source of truth; normalized partial-line lists can show many more synthetic
  branch misses than the aggregate file summary.

## Dedicated generator/probe pass from snapshot `415ab006`

User direction for this pass: do one broad execution sweep instead of
single-branch iterations. Failing tests/probes are acceptable only as evidence,
but coverage claims still require a fresh Coverage MCP snapshot with an ingested
artifact.

Reverse-mapped targets before editing:

| File | Aggregate gap | Branch hypothesis | Action |
| --- | ---: | --- | --- |
| `src/codecs/tiff/decode.rs` | 1 branch | Tile predictor line 113 is missing the path `predictor == 2`, compressed tile, and unsupported sample width. The earlier uncompressed 1-bit tile returns before the predictor predicate because the tile path rejects `bits_per_sample % 8 != 0`. | Add a compressed tiled TIFF probe with 24-bit samples. Use a tiny 9-bit LZW stream with literal codes `[65, 66, 67]` for one RGB pixel so the decoder reaches the predictor predicate without requiring a DEFLATE oracle. This is a real metadata/input state, not dead code. |
| `src/codecs/webp/native/decoder.rs` | 10 branches | Remaining misses are a mix of real RIFF/container parser sides and coverage-hook helper branches. Helper `chunk()` still has an odd-payload padding branch; parser line 165 lacks non-RIFF, lines 181/187/205 need invalid VP8/VP8L public container inputs, and extended animation predicates need valid ANIM/ANMF chunk combinations. Full successful `read_image()`/`read_frame()` still needs a valid VP8/VP8L bitstream generator. | First remove hook-introduced branch noise by making probe chunks always write a pad byte and use even payloads where possible. Add explicit parser-error RIFF inputs and valid metadata-only ANIM/ANMF combinations. Leave full composition success for the VP8/VP8L generator if still missing. |
| `src/codecs/jpeg/decode/progressive.rs` | 4 branches | Lines 160 and 186 are coefficient-refinement predicates; earlier probes likely consumed opposite bits through Huffman decoding before reaching those predicates. Lines 509/531/544/546/595/597 are normalized line/`?` gaps around scan orchestration and IDCT buffer bounds. | Add bitstream probes that force `bit == 0` with an existing non-zero coefficient and `bit != 0` with the refinement mask already set. Do not add standalone helper predicates. If still missing, document as progressive entropy-generator debt. |
| `src/codecs/webp/native/lossless.rs` | 2 branches | MCP line map is normalized across many bit-reader and lossless parser lines. Existing direct `BitReader` probes covered `fill()` large/small and `consume()` success/error; likely remaining real branches are `plane_code_to_distance()` and `HuffmanInfo::get_huff_index()` private state sides, not public VP8L fixtures. | Add same-module private probes for `get_copy_distance()`, `plane_code_to_distance()` above/below the distance table, and `HuffmanInfo::get_huff_index()` with both zero and non-zero meta bits. Keep probes linear and branch-light. |
| `src/codecs/webp/native/vp8.rs` | 6 branches | Remaining aggregate misses are inside arithmetic-bitstream-dependent parser paths after prior direct states. Some line-map gaps may also be hook-introduced loops. | Add only branch-light private state probes already reachable without a full VP8 frame generator: partition count with `n == 1`, segmentation enabled/disabled quantization sides, and coefficient value signs. Full residual/parser coverage remains generator debt if aggregate does not move. |

Result from Coverage MCP run `5c7aa4fe-0446-4884-8d11-0f4ea1d979dc`,
snapshot `e59efa94-9c6b-4d6a-8e7e-d67f8b89cfe2`:

- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 23348 / 23359 lines, 3416 / 3438 branches,
  1562 / 1562 functions.
- Net branch gap: 23 missing branches down to 22 missing branches.
- Effective improvement:
  - `src/codecs/tiff/decode.rs` is now 120 / 120 branches.
- No aggregate movement:
  - `src/codecs/webp/native/decoder.rs` stayed 76 / 86 branches.
  - `src/codecs/webp/native/vp8.rs` stayed 154 / 160 branches.
  - `src/codecs/jpeg/decode/progressive.rs` stayed 114 / 118 branches.
  - `src/codecs/webp/native/lossless.rs` stayed 108 / 110 branches.

Post-result refinement before the next run:

| File | Finding | Action |
| --- | --- | --- |
| `src/codecs/jpeg/decode/progressive.rs` | The IDCT store guard `if bi < comp_buffers[comp_idx].len()` is unreachable. `block_idx` iterates only over `comp_num_blocks`, and `comp_buffers` is allocated exactly as `comp_buf_width * comp_buf_height`, where both dimensions are padded to full 8x8 blocks. | Remove the defensive guard and assign directly. This is an invariant simplification, not a fixture. |
| `src/codecs/webp/native/decoder.rs` | The parser still has reachable container predicate sides: VP8X loop with no trailing chunks when RIFF size is below the extended payload threshold; animation with only ANIM or only ANMF; EXIF present but XMP missing; VP8-only extended still image; VP8L/ALPH frame dimensions where height, not width, trips the limit; ALPH frame with valid size but invalid alpha payload. | Add metadata-only RIFF probes. These do not require successful VP8/VP8L image decoding. |
| `src/codecs/jpeg/decode/progressive.rs` | The scan call-site gaps at DC/AC `?` lines likely require block decoders returning `None`. | Add one invalid empty Huffman table and scan variants that route through DC-first, AC-first, and AC-refine call sites. If aggregate does not move, leave as progressive entropy-generator debt. |

Result from Coverage MCP run `e8528516-141a-4692-a7aa-fa0e1fbb2f8f`,
snapshot `71403b70-5a1d-4531-853e-c5d6a7e8c36c`:

- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 23409 / 23419 lines, 3419 / 3436 branches,
  1563 / 1563 functions.
- Net branch gap: 22 missing branches down to 17 missing branches.
- Effective improvements:
  - `src/codecs/webp/native/decoder.rs`: 80 / 86 branches, 6 missing.
  - `src/codecs/jpeg/decode/progressive.rs`: total branch denominator dropped
    from 118 to 116 after removing the unreachable IDCT buffer guard; it is now
    113 / 116 branches.

Third refinement before next run:

| File | Finding | Action |
| --- | --- | --- |
| `src/codecs/webp/native/decoder.rs` | A sequential rewrite of the VP8 zero-dimension and VP8X chunk-missing predicates was measured in Coverage MCP run `eb351f67-f763-428b-9ed5-81032a857ce3`, snapshot `4ff1ab92-eae9-4aa3-aeb2-6d0d2c664a3c`. It regressed the aggregate to 3416 / 3434 branches because it changed metadata-check reachability in the probe set and added uncovered split-condition lines. | Reject that refactor and restore the original compound predicates. Keep only the defensible generic-reader probe that returns `io::ErrorKind::Other` during extended chunk scanning to cover the non-EOF IO error branch. |
| `src/codecs/webp/native/decoder.rs` | Frame-success branches (`FrameOutsideImage`, canvas initialization, final `has_alpha`) require successful VP8/VP8L subframe decoding. | Keep as generator debt unless a minimal valid VP8/VP8L frame generator is implemented. Do not fake by bypassing decode. |
| `src/codecs/jpeg/decode/progressive.rs` | Lines 160 and 186 still miss one predicate side despite direct bitstream probes. The scan-call `?` line gaps had a separate hook-routing issue: the combined failing scan list returned at the DC-first scan before reaching AC-first or AC-refine call-sites. | Treat lines 160 and 186 as entropy-generator/LLVM expression mapping debt for now; avoid standalone predicate extraction. Split the failing progressive scan probes so DC-first, AC-first, and AC-refine are each run as independent inputs. |
| `src/codecs/webp/native/vp8.rs` and `lossless.rs` | Aggregates did not move after direct private state probes. | Defer to dedicated bitstream generators or invariant simplification based on a smaller proof. |

Intermediate measurement:

- Coverage MCP run: `32673773-679f-47c4-823a-e231051c959e`
- Coverage MCP snapshot: `91ef7e81-dcd4-41a9-970e-bff4feb51f09`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 23418 / 23432 lines, 3421 / 3438 branches,
  1566 / 1568 functions.
- Finding: the WebP non-EOF reader probe covered the intended scanner branch
  but introduced two uncovered hook-local `BufRead` methods. Correction before
  the next run: explicitly exercise `fill_buf()` and `consume()` on that reader
  so the hook does not regress function or line coverage.

Fourth refinement before next run:

| File | Finding | Action |
| --- | --- | --- |
| `src/codecs/jpeg/decode/progressive.rs` | The scan dispatch booleans are exhaustive: DC-first, DC-refine, AC-first, then AC-refine. Therefore the false side of the final `else if is_ac_refine` is unreachable once the preceding three cases are false. | Replace the final `else if is_ac_refine` with `else` and remove the now-unused `is_ac_refine` binding. This is invariant simplification, not a fabricated fixture. |
| `src/codecs/jpeg/decode/progressive.rs` | Coefficient refinement uses two short-circuit predicates, `bit != 0 && (coeffs[k] & p1) == 0`, at lines 160 and 186. Existing direct probes already exercise bit-false, mask-false, and update-true states, but LLVM still reports one missing branch per compound line. | Split each predicate into nested `if` statements in place. This preserves short-circuit semantics while making the actual branch points explicit to coverage. |

Result from Coverage MCP run `86496ccc-6561-4802-9e40-533f0aa50a38`,
snapshot `f63c6cdf-2451-461b-a8e6-f7dc05b28603`:

- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 23431 / 23440 lines, 3418 / 3434 branches,
  1569 / 1569 functions.
- Net branch gap: 17 missing branches down to 16 missing branches.
- JPEG progressive improved from 113 / 116 branches to 110 / 112 branches.
  The final AC-refine dispatch branch was removed as unreachable.
- New exact JPEG finding: the earlier "all ones" entropy probe used `0xFF`,
  but JPEG entropy treats `0xFF` as marker/padding, not literal one bits. This
  explains why the mask-false refinement side was not actually covered.

Fifth refinement before next run:

| File | Finding | Action |
| --- | --- | --- |
| `src/codecs/jpeg/decode/progressive.rs` | The remaining line gaps are the mask-false exits of the two nested coefficient-refinement predicates. Existing `0xFF` probes are invalid for literal one bits because JPEG entropy parsing treats `0xFF` specially. | Add non-marker entropy bytes: `0xE0` for phase-1 `one_new_coeff` plus the new-coefficient sign bit and existing-coefficient refine bit `1`, and `0x80` for phase-2 refine bit `1`. Use fresh coefficient arrays with coefficient bit already set so `(coeffs[k] & p1) != 0`. |

Result from Coverage MCP run `d71e871b-13d6-4593-9313-9ab565cd78c9`,
snapshot `5825cd9b-6830-48c8-9af9-b7aefa8df155`:

- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 23466 / 23474 lines, 3419 / 3434 branches,
  1569 / 1569 functions.
- Net branch gap: 16 missing branches down to 15 missing branches.
- JPEG progressive improved to 111 / 112 branches.
- Remaining JPEG finding: phase-2 mask-false is covered; phase-1 still misses
  because the first non-marker probe used only two leading one bits (`0xC0`).
  Phase-1 consumes three relevant bits: Huffman symbol, new-coefficient sign,
  and existing-coefficient refine.

Sixth refinement before next run:

| File | Finding | Action |
| --- | --- | --- |
| `src/codecs/jpeg/decode/progressive.rs` | Phase-1 AC-refine with `one_new_coeff` needs bit pattern `111...` to decode symbol `0x01`, consume the new-coefficient sign bit, then set the existing-coefficient refine bit. | Change the phase-1 non-marker mask-false probe from `0xC0` to `0xE0`. |

Result from Coverage MCP run `f839cf10-c30b-48f9-a8b6-1991c0338d58`,
snapshot `9273acc9-c703-4bbc-a666-46c0eba1b0a8`:

- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 23467 / 23474 lines, 3420 / 3434 branches,
  1569 / 1569 functions.
- Net branch gap: 15 missing branches down to 14 missing branches.
- `src/codecs/jpeg/decode/progressive.rs` is now 858 / 858 lines,
  112 / 112 branches, and 22 / 22 functions.
- Remaining branch-bearing files:
  - `src/codecs/webp/native/decoder.rs`: 82 / 88 branches, 6 missing.
  - `src/codecs/webp/native/vp8.rs`: 154 / 160 branches, 6 missing.
  - `src/codecs/webp/native/lossless.rs`: 108 / 110 branches, 2 missing.

## One-sweep plan for remaining 48 branches

User direction for this sweep: maximize execution in one session, accept failing
tests when they expose pending parity work, and do not spend hours doing tiny
single-file checkpoints. The implementation rule for this pass is:

1. Use manifest/Pillow fixtures when the missing branch is a public codec input
   or output behavior and exact-byte parity is practical.
2. Use the existing `#[cfg(coverage)]` internal hook when the missing branch is
   a private state-machine edge, bit-reader edge, generic `Read`/`Write`
   behavior, or a public fixture would be brittle and unrelated to Pillow
   parity.
3. Remove or simplify branches proven dead by reverse mapping. Do not add fake
   tests for unreachable code.
4. Run one MCP lines+branches coverage command after the broad batch. If tests
   fail and MCP still ingests a valid snapshot, report coverage from that
   snapshot. If MCP skips ingestion, keep the failure as pending parity evidence
   but do not claim coverage from it.

Sweep target table from snapshot `88045439-f646-49ab-838b-5c3e8b9bcbbb`:

| File | Aggregate gap | Reverse-mapped inputs/states | Action in this sweep |
| --- | ---: | --- | --- |
| `src/codecs/webp/native/lossless.rs` | 1 line, 2 branches | `BitReader::fill()` with `>=8` buffered bytes, `consume()` success after fill, and `read_bits()` when enough bits are already buffered. MCP lists 50 normalized partial lines, but aggregate says only two real branches remain. | Extend existing lossless `cfg(coverage)` hook with direct `BitReader` states; no public fixture. |
| `src/codecs/webp/native/vp8.rs` | 5 lines, 8 branches | Header/parser booleans, coefficient token decoding, filter parameter variants, skipped-block complexity reset. Reverse mapping found one dead branch: after `DCT_0` the code `continue`s, so later `abs_value == 0` cannot execute. | Add direct VP8 private states and remove the dead `abs_value == 0` complexity arm. Do not create an artificial helper solely for coverage. |
| `src/codecs/webp/native/decoder.rs` | 0 lines, 10 branches | Container-level WebP paths: zero VP8 dimensions, truncated extended-chunk EOF handling, VP8/VP8L exclusivity checks, animated frame oversized/out-of-canvas/unknown-subchunk paths, and alpha/no-alpha image read paths. | Add a native decoder `cfg(coverage)` hook and wire it through `native::mod`; use hand-built same-module decoder states for animation paths. |
| `src/codecs/jpeg/decode/progressive.rs` | 7 lines, 14 branches | AC first/refine block loop exits, EOBRUN paths, coefficient bounds, smooth DC-only assembly, CMYK inversion branch. These are scan-state edges below the public parser. | Extend existing progressive private hook with targeted Huffman tables, coefficient arrays, and tiny `JpegInfo` states. Public malformed JPEG fixtures are deferred. |
| `src/codecs/tiff/decode.rs` | 1 line, 14 branches | Header/tag validation, tile path predicates, miniswhite packed tail mask, photometric inversion, PackBits loop termination/no-op/overrun, LZW first-code invalid and exact-expected early return. | Add a TIFF decode `cfg(coverage)` hook and wire it through `tiff::mod` and `codecs::mod`; use small synthetic TIFF byte arrays and direct compression helper calls. |
| `src/types/dynamic.rs` | line-only normalization | No grouped line gaps returned by MCP file query. | Treat as LLVM normalization artifact unless later aggregate changes identify a concrete line. |
| `src/codecs/compression/zlib_ng.rs` | line-only normalization | No grouped line gaps returned by MCP file query. | Treat as LLVM normalization artifact unless later aggregate changes identify a concrete line. |

Public parity debt explicitly not solved by this sweep:

- The rejected WebP alpha fixture (`enc_lossy_alpha_17_values`) reached internal
  alpha serialization logic but failed exact Pillow encoded-byte parity
  (`154` actual bytes vs `138` expected bytes). Keep this as pending encoder
  parity work, not as a coverage blocker.
- Any new failing manifest row added after this point must identify the exact
  Pillow-oracle mismatch and must not be used to claim coverage unless Coverage
  MCP ingests a fresh snapshot from the failing run.

First sweep result:

- Coverage MCP run: `d69d4340-a620-4fe4-94e7-1c812503adc0`
- Coverage MCP snapshot: `5fe8a6f8-80d1-4f5d-a82b-460164ccdfee`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 23055 / 23066 lines, 3398 / 3436 branches,
  1561 / 1561 functions.
- Net branch gap: 48 missing branches down to 38 missing branches.
- Effective improvements:
  - `src/codecs/tiff/decode.rs`: 100 / 114 branches to 109 / 118 branches.
    The branch denominator increased because a TIFF hook was added; one
    hook-local LZW packer branch is noise and must be removed in the next pass.
  - `src/codecs/jpeg/decode/progressive.rs`: 104 / 118 branches to
    107 / 118 branches.
  - `src/codecs/webp/native/vp8.rs`: removed the proven-dead
    `abs_value == 0` branch after `DCT_0` already `continue`s.
  - `src/codecs/webp/native/decoder.rs`: added container probes, but aggregate
    gap stayed at 10 missing branches because the new probes also introduced
    coverage-only helper branches. Keep subsequent hooks branch-light.

Second sweep plan before patching:

| File | Current gap after first sweep | Planned second-pass action |
| --- | ---: | --- |
| `src/codecs/tiff/decode.rs` | 9 branches | Remove the hook-local `if used != 0` branch in the LZW test packer; add `decode_packbits(&[0], 0)` for `position < data.len()` true and `output.len() < expected` false; add YCbCr false-side probes for the stored-sample predicate. |
| `src/codecs/jpeg/decode/progressive.rs` | 11 branches | Add block-level probes for the exact short-circuit sides not hit by the first pass: `k < 64` false in AC first/refine, `k >= 64` after `k > se` is false, coefficient-refinement mask false, phase-2 loop false, and `smooth_pred` clamp false. |
| `src/codecs/webp/native/vp8.rs` | 6 branches | Defer broad bitstream generation; remaining lines are largely normalized partials around arithmetic-coded parser states. Keep the dead-branch simplification and revisit with a dedicated VP8 bitstream builder. |
| `src/codecs/webp/native/decoder.rs` | 10 branches | Defer full frame-success branches unless a minimal valid VP8/VP8L generator is added. Current probes cover public parser errors but do not reach successful animated composition branches. |
| `src/codecs/webp/native/lossless.rs` | 2 branches | Treat current line map as normalized noise until aggregate-guided reverse mapping identifies the two real branches; the direct `BitReader` probes did not change the aggregate count. |

Second sweep result:

- Coverage MCP run: `10398f8b-87fc-4e5f-9124-ff0dbd139392`
- Coverage MCP snapshot: `b0cf8dda-3669-44fb-a1c9-602bfa007b5c`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 23105 / 23116 lines, 3407 / 3434 branches,
  1561 / 1561 functions.
- Net branch gap: 38 missing branches down to 27 missing branches.
- `src/codecs/jpeg/decode/progressive.rs` improved to 114 / 118 branches.
- `src/codecs/tiff/decode.rs` improved to 111 / 116 branches and removed the
  hook-local LZW packer branch noise.

Third mini-sweep plan before patching:

| File | Current gap | Planned action |
| --- | ---: | --- |
| `src/codecs/jpeg/decode/progressive.rs` | 4 branches | Add explicit bit-false probes for the coefficient-refinement predicates at lines 160 and 186. The remaining uncovered scan-call lines are likely multiline `?`/scan orchestration states and should not be forced without a valid entropy-segment generator. |
| `src/codecs/tiff/decode.rs` | 5 branches | Add tiny tiled TIFF headers for `TileByteCounts` without `TileOffsets`, zero tile height, and non-byte-aligned tile bits. Leave predictor/compressed-tile sides for fixture-backed compressed tile generation. |

Third mini-sweep and cleanup result:

- Coverage MCP run: `c9f70f3d-54c2-4a9b-a29a-8182075cc27f`
- Coverage MCP snapshot: `b1e76eba-5e61-4576-8ae6-3c268b00d42e`
- Cleanup Coverage MCP run: `942e04e0-bcf5-4849-a415-2253c8c64ebf`
- Cleanup Coverage MCP snapshot: `415ab006-4c55-4bc3-bdc9-f1310c107563`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Final overall for this sweep: 23183 / 23194 lines, 3415 / 3438 branches,
  1562 / 1562 functions.
- Net branch gap: 48 missing branches down to 23 missing branches.

Remaining exact aggregate gaps after snapshot
`415ab006-4c55-4bc3-bdc9-f1310c107563`:

| File | Remaining branches | Next attack |
| --- | ---: | --- |
| `src/codecs/webp/native/decoder.rs` | 10 | Needs a minimal valid VP8/VP8L frame generator so `read_image()` and animated `read_frame()` reach successful composition, alpha, out-of-canvas, and final `has_alpha()` branches. Parser-error-only RIFF probes are insufficient. |
| `src/codecs/webp/native/vp8.rs` | 6 | Needs a dedicated VP8 arithmetic-bitstream builder for frame-header, segmentation, residual, skipped-block, and loop-filter states. The simple all-zero/all-one payloads did not reach the remaining parser states. |
| `src/codecs/jpeg/decode/progressive.rs` | 4 | Remaining gaps are scan-orchestration/multiline `?` states and IDCT buffer-bound false side. Do not add more isolated block probes; build a valid progressive entropy-segment fixture/harness if this remains a priority. |
| `src/codecs/webp/native/lossless.rs` | 2 | Aggregate still reports two branches even after direct `BitReader` probes. The MCP line map is normalized/noisy; use branch-level reverse mapping or refactor only after identifying the exact real branch. |
| `src/codecs/tiff/decode.rs` | 1 | Remaining branch is line 113: predictor 2 with compressed tile and unsupported sample width. Needs a compressed tile fixture/harness, not another uncompressed synthetic header. |
| `src/types/dynamic.rs` | line-only | MCP file query returns no grouped gaps; treat as LLVM normalization until a concrete line appears. |
| `src/codecs/compression/zlib_ng.rs` | line-only | MCP file query returns no grouped gaps; treat as LLVM normalization until a concrete line appears. |

## Planned WebP native Huffman invariant cleanup

Coverage MCP snapshot `96124852-5e77-4ae1-9978-a9fe5106c6c7`, measured at
commit `471d47a0035a15793fe5ca203fac744c5c1224f3`, reports
`src/codecs/webp/native/huffman.rs` at 221 / 223 lines, 24 / 24 branches, and
9 / 9 functions. This is the selected next target because it has the smallest
clean line deficit: the smaller aggregate branch file, `lossless.rs`, still
reports 2 missing branches across 50 normalized partial-branch lines and no
exact actionable branch from the MCP line map.

Target lines and reverse-mapping plan:

- line 143: `HuffmanTreeNode::Leaf(_)` while descending the overflow tree in
  `HuffmanTree::build_implicit()`.
- line 154: non-empty final tree slot after descending to the target leaf
  position in `HuffmanTree::build_implicit()`.

Reverse mapping with a local search over 140,805 canonical code-length
candidate layouts found no input that reaches either branch after the preceding
`curr_code == 2 << max_code_length` validation. This matches canonical Huffman
construction: a complete prefix-code length histogram cannot descend through an
already assigned leaf or assign into a non-empty final slot. Fix type:
invariant simplification inside the private builder, not a fabricated WebP
fixture. The public malformed WebP rows continue to cover invalid Huffman
tables at the decoder boundary; this cleanup removes defensive-unreachable
lines from the internal construction algorithm.

Completed evidence:

- Reverse-mapping probe: 140,805 canonical code-length layouts tried; none
  reached the two defensive construction conflicts after the canonical
  `curr_code` validation.
- Coverage MCP run: `9518e986-c94c-471f-8e35-b65cc89988a8`
- Coverage MCP snapshot: `e8165588-496a-422f-a242-a97b9763403e`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 22721 / 22740 lines, 3385 / 3438 branches, and
  1549 / 1549 functions.
- Target file: `src/codecs/webp/native/huffman.rs` improved from
  221 / 223 lines, 24 / 24 branches, and 9 / 9 functions to
  220 / 220 lines, 26 / 26 branches, and 9 / 9 functions. Aggregate line and
  branch coverage are now 100% for the file. MCP still lists two normalized
  partial-region lines, but the LLVM summary for the file reports no missing
  lines or branches.
- Code change: `build_implicit()` now treats the leaf-descent and non-empty
  final-slot cases as canonical-construction invariants. It keeps debug
  assertions at the invariant points and removes the unreachable error-return
  lines from the hot builder.

## Planned BMP decoder malformed-fixture batch

Coverage MCP snapshot `a83eedac-447c-4931-b9fe-2b6e5e6993dd`, measured at
commit `e5cd10c234a7519b3c84eec10ef95b23e8ecd8e6`, reports
`src/codecs/bmp/decode.rs` at 346 / 346 lines, 109 / 122 branches, and
11 / 11 functions. This is the selected target after deferring the smaller
WebP files: `lossless.rs` reports 2 aggregate missing branches across 50
normalized partial-branch lines, `encoder.rs` has prior no-op retries on the
same hidden/tree-shape gaps, and `vp8.rs` reports 8 aggregate missing branches
across 25 normalized gap lines. BMP has six concrete public decoder lines and
no uncovered-line noise.

Target branch lines and reverse-mapping plan:

- line 222: `w == 0 || h == 0 || w > 16_384 || h > 16_384`.
  Existing `invalid_height.bmp` covers `h == 0`, and `invalid_width.bmp`
  returns earlier at `width <= 0`. Reverse mapping proves `w == 0` is
  unreachable after the preceding `width <= 0` guard and successful
  `u32::try_from(width)`. Add oversized-width and oversized-height malformed
  fixtures for the remaining public reject sides, and simplify the unreachable
  `w == 0` predicate. Fix type: malformed manifest fixtures plus
  unreachable-branch simplification.
- line 247: `bit_depth == 32 && header_size >= 56` when reading V4/V5
  BI_BITFIELDS masks. Existing 32-bit V4/V5 fixtures cover the alpha-mask side,
  and `v4header16.bmp` covers the non-32-bit side. Reverse-map the remaining
  side with a valid 32-bit BITMAPV2-style `header_size == 52` fixture that has
  red/green/blue masks but no alpha mask. Fix type: Pillow-oracle manifest
  fixture if Pillow accepts it; malformed error fixture only if Pillow rejects.
- line 263: `compression == 3 && (rm == 0 || gm == 0 || bm == 0)`.
  Existing `bitfields_zero_mask.bmp` zeros the green mask at file offset 58.
  Add separate zero-red-mask and zero-blue-mask fixtures so short-circuiting
  reaches the remaining mask predicates. Fix type: malformed manifest fixtures.
- line 294: palette grayscale detection reaches the final channel comparison.
  Existing noncanonical palettes fail before the third comparison. Add a small
  paletted BMP whose palette entry has blue and green equal to the grayscale
  expectation but red different, so only `entry[2] == expected` is false. Fix
  type: Pillow-oracle manifest fixture.
- line 493: output color selection includes `compression == 1 ||
  compression == 2` after `bit_depth <= 8`. Valid BMP RLE8 and RLE4 already
  have bit depths 8 and 4, so the compression predicates short-circuit behind
  `bit_depth <= 8`. Reverse-map with invalid RLE/depth combinations and the
  Pillow oracle. Probe evidence shows Pillow accepts RLE4/RLE8 when the
  bit-depth field is 1, 4, or 8, and rejects 2/3 and truecolor depths.
  Therefore add decoder validation for unsupported/RLE-truecolor depths and
  simplify the color predicate to `bit_depth <= 8`; do not require RLE4 to be
  exactly 4 or RLE8 exactly 8. Fix type: malformed fixture plus
  unreachable-branch simplification.
- line 503: mode selection already lists 2-bit BMPs (`1 | 2 | 4 | 8`) but
  pixel decoding lacks a 2bpp branch. Preflight proved Pillow rejects 2-bit
  BMPs with `Unsupported BMP pixel depth (2)`, so the correct parity behavior
  is not to implement 2bpp. Treat generated 2-bit fixtures as malformed
  Pillow-error rows and remove 2 from the successful mode-selection match. Fix
  type: malformed manifest fixtures plus unreachable-branch simplification.

Second-pass evidence from Coverage MCP run `b5f34f3a-023d-4e27-b10d-dac3e26a13bb`
and snapshot `408e297f-297c-40ea-87d3-71ce6c0a325c`:

- The run passed with 5 passed / 0 failed and ingested the coverage artifact.
  Overall coverage moved to 22699 / 22721 lines and 3386 / 3442 branches.
- `src/codecs/bmp/decode.rs` moved to 351 / 352 lines and 115 / 118
  branches. Remaining gaps are line 300, line 494, and line 509.
- line 300 still misses one palette comparison side. Add explicit blue- and
  green-channel mismatch palettes in addition to the current red-channel
  mismatch, so the chained grayscale predicate is proven from each short-circuit
  position.
- line 494 is the `_ => return None` arm of the pixel-depth match. It became
  unreachable after explicit Pillow-compatible depth validation. Replace the raw
  `u16` depth with a small internal enum after validation so later matches are
  exhaustive without a defensive catch-all line.
- line 509 still needs a successful grayscale indexed fixture. `gray.bmp` is
  already generated and Pillow reports mode `L`, but it was not active in the
  manifest. Add it to the 8-bit depth rows.

Third-pass evidence from Coverage MCP run `f8f5f8f9-57d8-47e2-8649-be7a9fec3fd9`
and snapshot `3073d0a7-b717-4c89-8693-25396f78885f`:

- The run passed with 5 passed / 0 failed and ingested the coverage artifact.
  Overall coverage moved to 22720 / 22741 lines and 3385 / 3440 branches.
- `src/codecs/bmp/decode.rs` moved to 372 / 372 lines and 114 / 116
  branches. The only remaining target gap is line 545, the guarded
  multi-pattern `1 | 4 | 8 if !palette_is_grayscale` match arm.
- A probe-generated 4-bit grayscale BMP is accepted by Pillow as mode `L`, but
  Pillow expands the values rather than returning raw palette indices. Do not
  add that fixture casually in this batch. Instead, simplify mode selection into
  per-depth branches: 1-bit chooses `L1` vs `P8`, 4/8-bit choose `L8` vs `P8`,
  and truecolor uses `color.into()`. Existing active fixtures cover each
  resulting public side.

Completed evidence:

- Coverage MCP run: `a6a6d0d7-75b5-492a-a059-8c277af8d547`
- Coverage MCP snapshot: `36699d47-211b-42e8-8621-012e46f2f979`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 22722 / 22743 lines, 3383 / 3436 branches, and
  1549 / 1549 functions.
- Target file: `src/codecs/bmp/decode.rs` improved from 346 / 346 lines,
  109 / 122 branches, and 11 / 11 functions to 374 / 374 lines,
  112 / 112 branches, and 14 / 14 functions. No line or branch gaps remain in
  this file.
- Reverse-mapped inputs and simplifications used:
  - `oversized_width.bmp` and `oversized_height.bmp` cover the maximum
    dimension rejects after proving `w == 0` unreachable behind `width <= 0`.
  - `bitfields_v2_32_no_alpha.bmp` covers a valid 32-bit V2-style BITFIELDS
    header with RGB masks and no alpha mask.
  - `bitfields_zero_red_mask.bmp`, `bitfields_zero_mask.bmp`, and
    `bitfields_zero_blue_mask.bmp` cover all three zero-mask reject positions.
  - `palette_blue_mismatch.bmp`, `palette_green_mismatch.bmp`, and
    `palette_red_mismatch.bmp` cover each palette-grayscale short-circuit
    position.
  - `rle8_invalid_depth.bmp` and `rle4_invalid_depth.bmp` cover Pillow-rejected
    RLE truecolor-depth inputs; RLE indexed depths remain accepted to match
    Pillow's behavior.
  - `2bit.bmp` and `2bit_gray.bmp` are explicit Pillow-error rows because
    Pillow 12.2.0 rejects BMP pixel depth 2.
  - `gray.bmp` activates a generated 8-bit grayscale BMP whose Pillow output is
    mode `L`.
  - The decoder now validates BMP bit depth once into an internal enum, removing
    a defensive match catch-all that became unreachable after Pillow-compatible
    validation.
  - Mode selection is split by depth (`1`, `4/8`, and truecolor) to preserve
    observable behavior while avoiding a guarded multi-pattern branch artifact.

## Planned JPEG parser malformed-fixture batch

Coverage MCP snapshot `f6faccb2-65f5-4a1f-87ef-266fcd02c2a3`, measured at
commit `adf51eeb2d6cb299e3fdc4dd465881ce283d6183`, reports
`src/codecs/jpeg/decode/parser.rs` at 315 / 315 lines, 91 / 98 branches, and
12 / 12 functions. This is the selected target because the smaller
`src/codecs/webp/native/lossless.rs` entry is still noisy: MCP reports only
2 aggregate missing branches but 50 partial-branch lines after LLVM
normalization. `src/codecs/webp/native/encoder.rs` is smaller by aggregate
branch count, but it mixes public encoder line gaps and late internal writer
branches; the JPEG parser gaps are cleaner malformed-public-input predicates.

Target branch/line gaps and reverse-mapping plan:

- line 99: `marker_byte == 0x00 || marker_byte == 0xFF` in
  `find_next_marker()`. Existing `prefixed_stuffed_marker.jpg` covers the
  stuffed-byte `0x00` side. Reverse-map the missing fill-byte side with a
  minimal malformed JPEG body `SOI, FF, FF, EOI`, so the first predicate is
  false and the second predicate is true before marker scanning continues to
  `EOI`. Fix type: malformed manifest fixture with `expect_error`.
- line 164: `h_samp < 1 || h_samp > 4 || v_samp < 1 || v_samp > 4`.
  Existing `sof_zero_sampling.jpg` sets sampling to `0x00`, which short-circuits
  on `h_samp < 1`. Reverse-map the missing sides with three SOF mutations:
  `0x51` for `h_samp > 4`, `0x10` for `v_samp < 1` after a valid horizontal
  sample, and `0x15` for `v_samp > 4`. Fix type: malformed manifest fixtures
  with `expect_error`.
- line 278: `dc_tbl > 3 || ac_tbl > 3` in `parse_sos()`. Existing
  `sos_bad_dc_table.jpg` covers the DC-table high side. Reverse-map the missing
  AC-table side with an SOS table selector of `0x04`, leaving the DC table valid
  and making only `ac_tbl > 3` true. Fix type: malformed manifest fixture with
  `expect_error`.
- lines 382 and 385: baseline `scan_components.is_empty()` after parsing an
  `SOS`. Reverse mapping shows the false side is unreachable for baseline JPEGs:
  this parser breaks immediately after the first non-progressive scan once
  `find_eoi()` succeeds, and `scan_components` can only be populated by that
  first `SOS` path. Do not fabricate private parser state. Fix type:
  unreachable-branch simplification to assign `scan_components` and
  `entropy_start` directly in the baseline branch.
- line 410: `data.get(pos..pos + 5) == Some(b"Adobe") && length >= 14` for
  APP14 parsing. Existing CMYK/APP14 fixtures cover the full Adobe marker and
  `app14_non_adobe.jpg` covers the non-Adobe payload. Reverse-map the missing
  side with a short APP14 marker whose payload starts with `Adobe` but whose
  segment length is only `7` (`2` length bytes plus `5` payload bytes), making
  the first predicate true and the transform-length predicate false. Fix type:
  malformed error fixture because Pillow rejects the optional short APP14
  marker.

Completed evidence:

- Coverage MCP run: `8482652a-4ae9-490e-bb1d-1b34a7f139fe`
- Coverage MCP snapshot: `9e996753-3e60-4a9a-b4c8-b8ffb75c34de`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 22694 / 22715 lines, 3380 / 3446 branches, and
  1546 / 1546 functions.
- Target file: `src/codecs/jpeg/decode/parser.rs` improved from
  315 / 315 lines, 91 / 98 branches, and 12 / 12 functions to
  313 / 313 lines, 96 / 96 branches, and 12 / 12 functions. No branch gaps
  remain in this file.
- Reverse-mapped inputs and simplification used:
  - `fill_marker_only.jpg` covers the repeated `0xFF` fill-marker scanner side.
  - `sof_high_h_sampling.jpg`, `sof_zero_v_sampling.jpg`, and
    `sof_high_v_sampling.jpg` cover the remaining SOF sampling reject sides.
  - `sos_bad_ac_table.jpg` covers the SOS AC-table high reject side.
  - `app14_adobe_short_transform.jpg` covers an APP14 payload that starts with
    `Adobe` but is too short to carry the transform byte; Pillow rejects the
    minimal malformed file, so the manifest row is an exact error-parity row.
  - The baseline `scan_components.is_empty()` false side was removed as an
    invariant after reverse mapping the non-progressive parser flow: the parser
    requires EOI and breaks immediately after the first baseline SOS.

## Planned JPEG baseline decoder batch

Coverage MCP snapshot `a0e8c283-b3b1-46a2-90ec-3efba03ea16f`, measured at
commit `0267a0c2ca84c53b7db9e9c1ff788de5e068c1c0`, reports
`src/codecs/jpeg/decode/decode.rs` at 383 / 384 lines, 65 / 70 branches, and
8 / 8 functions. This is the selected target after PNG because it is the next
small non-noisy branch deficit; `webp/native/lossless.rs` remains smaller but
is deferred due the noisy LLVM-normalized map already called out in this plan.

Target branch/line gaps and reverse-mapping plan:

- line 61: `br.read_bits(size)` returns `None` while decoding an AC literal.
  Reverse mapping shows JPEG AC coefficient size is the low four bits of the
  Huffman symbol (`1..=15` on this path), and `BitReader::fill()` zero-pads to
  `MIN_GET_BITS` on exhausted entropy data. Therefore this `None` side cannot
  be reached by valid or malformed JPEG entropy using this decoder state. Fix
  type: unreachable-branch simplification to explicit zero-fill/invariant
  handling, not a fabricated private state.
- lines 164 and 166: `bi < comp_buffers[scan_comp.comp_index].len()` around
  baseline block writes. Reverse mapping proves `block_x`, `block_y`, row and
  column are bounded by the same MCU dimensions used to allocate the component
  buffer: `num_mcus_x * h_samp * 8` by `num_mcus_y * v_samp * 8`. The false
  side cannot be reached from parsed JPEG dimensions. Fix type:
  unreachable-branch simplification to direct indexed assignment.
- line 295: CMYK output inversion uses `if inverted { 255 - sample } else {
  sample }`. Existing `baseline_cmyk.jpg` covers the Adobe APP14 inverted side.
  Reverse-map the missing side with a four-component CMYK JPEG whose APP14
  Adobe marker is removed or made non-Adobe while Pillow still decodes it.
  First retry evidence showed Pillow still returns the inverted CMYK byte
  convention for this no-APP14 fixture: Rust's false side produced every byte as
  `255 - expected`. Fix type: Pillow-oracle manifest fixture plus
  unreachable-branch simplification to always emit Pillow's inverted CMYK
  convention in the baseline four-component output path.
- line 399: quant-table validation misses the `is_none()` side after the table
  vector is long enough. Reverse-map with a malformed grayscale JPEG where the
  DQT table id is moved to 3 while SOF references quant table 2, leaving
  `quant_tables[2] == None`. Fix type: malformed manifest fixture.
- line 410: DC Huffman validation misses the `is_none()` side after the table
  vector is long enough. Reverse-map with a malformed grayscale JPEG where the
  DC DHT table id is moved to 3 while SOS references DC table 2. Fix type:
  malformed manifest fixture.
- line 415: AC Huffman validation misses the `is_none()` side after the table
  vector is long enough. Reverse-map with a malformed grayscale JPEG where the
  AC DHT table id is moved to 3 while SOS references AC table 2. Fix type:
  malformed manifest fixture.

Completed evidence:

- First Coverage MCP run: `8f03a051-eee6-421e-920b-f0a0661da27b`; failed
  with 509 passed, 2 failed, and no ingested snapshot. The only decode matrix
  failure was `color_cmyk_raw`: every byte was inverted relative to Pillow,
  proving the no-APP14 CMYK false side is not Pillow-parity.
- Corrected Coverage MCP run: `e48f44f0-2f00-46aa-8b43-78b5f9a3c052`
- Corrected Coverage MCP snapshot: `97751138-7d88-42fe-9efa-042212b82598`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 22696 / 22717 lines, 3375 / 3448 branches, and
  1546 / 1546 functions.
- Target file: `src/codecs/jpeg/decode/decode.rs` improved from
  383 / 384 lines, 65 / 70 branches, and 8 / 8 functions to
  383 / 383 lines, 66 / 66 branches, and 8 / 8 functions. No branch gaps
  remain in this file.
- Reverse-mapped inputs and simplifications used:
  - `cmyk_no_adobe_app14.jpg` proves Pillow still exposes no-APP14
    four-component JPEGs through the inverted CMYK byte convention.
  - `sof_sparse_quant_table.jpg` covers the quant-table `is_none()` guard side.
  - `sos_sparse_dc_table.jpg` covers the DC Huffman table `is_none()` guard side.
  - `sos_sparse_ac_table.jpg` covers the AC Huffman table `is_none()` guard side.
  - The AC bit-read `None` side and component-buffer bounds false side were
    removed as invariants after reverse mapping the bit-reader zero-padding and
    MCU buffer allocation math.

## Planned PNG decoder malformed-fixture batch

Coverage MCP snapshot `ac42d317-4668-45ab-965b-d1cc56eb799e`, measured at
commit `55dbe9a2ab297dab77ef4573d9bf73c2c2f8004a`, reports
`src/codecs/png/decode.rs` at 347 / 347 lines, 81 / 86 branches, and
21 / 21 functions. This is the selected target because it is a clean
five-branch file with no uncovered-line noise, and each branch is either public
PNG decoder behavior or chunk-iterator terminal behavior.

Target branch lines and reverse-mapping plan:

- line 32: `width == 0 || height == 0 || filter != 0 || interlace > 1`.
  Existing fixtures cover zero width, invalid filter method, and invalid
  interlace. Reverse-map the missing side by creating a minimal PNG with
  `height == 0`.
  Fix type: malformed manifest fixtures, because these are public IHDR reject
  paths.
- line 44: `PLTE` is accepted only when `palette_rgb.is_none()`. Existing
  indexed fixtures cover the first `PLTE`. Reverse-map the missing side with a
  valid indexed PNG containing a duplicate `PLTE` after the first palette. Fix
  type: Pillow-oracle manifest fixture if Pillow tolerates it; otherwise
  malformed error fixture if Pillow rejects it.
- line 45: `tRNS` is accepted only when `palette_alpha.is_empty()`. Existing
  indexed-alpha fixtures cover the first `tRNS`. Reverse-map the missing side
  with a valid indexed PNG containing a duplicate `tRNS` after the first alpha
  chunk. Fix type: Pillow-oracle manifest fixture if Pillow tolerates it;
  otherwise malformed error fixture if Pillow rejects it.
- line 53: `compressed.is_empty() || (!saw_end && chunks.failed)`. Existing
  malformed fixtures cover empty `IDAT` and the no-`IEND` / no-parser-failure
  path. Reverse-map the missing side by building a PNG with non-empty `IDAT`,
  no `IEND`, and a truncated following chunk so `!saw_end` and `chunks.failed`
  are both true. Fix type: malformed error fixture because Pillow reports
  `Truncated File Read`.
- line 407: `Chunks::next()` stops on `self.failed || self.position ==
  self.data.len()`. Public decoding naturally covers normal end-of-data after a
  missing `IEND` and parse-failed state after malformed chunks. Reverse-map the
  missing short-circuit side by proving which side remains from selected-line
  hits. If no public PNG can isolate the side because decoding stops as soon as
  `chunks.failed` is set, use a coverage-only private probe that advances a
  local `Chunks` iterator to failure and calls `next()` again. Fix type:
  private coverage probe only for the chunk iterator state-machine terminal
  predicate.

Completed evidence:

- Coverage MCP run: `c5eebe5e-3e29-425f-b2a8-62a1813fd605`
- Coverage MCP snapshot: `aff1b610-2b7a-4fbc-b744-25e15431709c`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 22696 / 22718 lines, 3374 / 3452 branches, and
  1546 / 1546 functions.
- Target file: `src/codecs/png/decode.rs` improved from 347 / 347 lines,
  81 / 86 branches, and 21 / 21 functions to 354 / 354 lines,
  86 / 86 branches, and 22 / 22 functions. No branch gaps remain in this
  file.
- Reverse-mapped inputs used:
  - `zero_height.png` covers the missing IHDR height reject side.
  - `duplicate_plte.png` covers the duplicate `PLTE` guard; Pillow tolerates
    the input and the manifest stores its one-byte palette output.
  - `duplicate_trns.png` covers the duplicate `tRNS` guard; Pillow tolerates
    the input and the manifest stores its one-byte palette output.
  - `idat_truncated_chunk_no_iend.png` covers the non-empty-IDAT plus
    failed-chunk/no-IEND reject side; Pillow reports `Truncated File Read`.
  - The `cfg(coverage)` `Chunks` probe covers the second `next()` call after a
    parser failure, which public `decode()` cannot isolate because iteration
    stops at the first `None`.

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

Next retry plan from pushed-head snapshot
`3823fd5b-233b-4439-8b84-96ace0497d55`: the earlier alternating retry used
only `MAX_LENGTH + 260` pixels, which was too short to push the backward-fill
distance past `MAX_LENGTH`. Add a deterministic period-2 input of at least
`3 * MAX_LENGTH` pixels with `width = 2` and high quality. The intended state
is a max-length distance-2 match: adjacent distance-1 matching stays low
because the pixels alternate, while period-2 matching can reach `MAX_LENGTH`.
Expected effect is to evaluate line 115 with `best_length < MAX_LENGTH` false
while a candidate is otherwise valid, and line 141 after the propagation loop
has moved more than `MAX_LENGTH` positions from `maximum_base`.

Completed retry evidence:

- Coverage MCP run: `85bd4a00-060c-4dc7-a817-a0accd6eca7c`
- Coverage MCP snapshot: `0efc7f89-18f1-4947-b192-ef685546347f`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 22164 / 22165 lines, 3352 / 3466 branches, 1532 / 1532 functions.
- Target file: `src/codecs/webp/native/encoder/backward_refs.rs` improved from
  857 / 857 lines, 210 / 212 branches, and 45 / 45 functions to
  866 / 866 lines, 214 / 214 branches, and 46 / 46 functions. No branch gaps
  remain in this file.

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

Current retry plan from pushed-head snapshot
`c43f858f-ada8-4b38-b342-9a0c275c8609`: histogram remains 503 / 503 lines,
107 / 112 branches, and 30 / 30 functions. Current selected-line evidence
shows:

- line 367 has the `add_eval()` `Some` side covered; add a same-bin pair whose
  combined cost exceeds the threshold so the `None`/continue side is reached.
- line 410 has the queue-not-full side covered; add a larger mergeable
  histogram population intended to fill the stochastic pair queue to nine and
  break.
- line 435 has the `update_pair()` true side covered after queue remapping;
  add a remapped pair whose recomputed combined cost exceeds zero so the
  swap-remove false-pair side is reached.
- line 517 is missing one side of `quality < 100` after a large cluster count;
  add a `> 2 * BIN_SIZE` cluster call with `quality == 100`.
- line 524 needs both `cluster()` outcomes from `stochastic_combine()`; add a
  larger high-quality/distinct-token cluster to try the non-greedy side.

Keep these as coverage-only private probes. If a probe does not improve MCP
coverage, remove it before committing.

First retry evidence:

- Coverage MCP run `8f526e7e-1db6-4bb4-be73-5d8cf2318035`, snapshot
  `0b4f2f60-f09f-49b8-b464-5e9f4b52176b`, passed with 5 passed, 0 failed.
  Overall improved to 22205 / 22206 lines, 3357 / 3466 branches,
  1534 / 1534 functions. `histogram.rs` improved from 107 / 112 to
  109 / 112 branches by closing line 410 queue-full break and line 517
  large-cluster quality side. Remaining lines are 367, 435, and 524.
- Next retry: add an artificial zero-cost same-bin pair in the private hook so
  `entropy_bin_combine()` reaches line 367 with `add_eval()` returning `None`
  via the `combined_costs()` non-positive limit guard.
- Second retry evidence: Coverage MCP run
  `306ea09d-4561-4bac-b31f-07893f3653c5`, snapshot
  `c230d738-35bc-46cd-8285-851ecfa34452`, passed with 5 passed, 0 failed.
  Overall improved to 22213 / 22214 lines, 3358 / 3466 branches,
  1534 / 1534 functions. `histogram.rs` improved to 110 / 112 branches,
  leaving line 435 and line 524.
- Next retry: call `cluster()` with the larger distinct-token corpus at
  `quality == 0`, which makes the stochastic threshold one cluster and should
  be the hardest public path for `stochastic_combine()` to return true. This
  may also force remapped queued pairs to fail `update_pair()` at line 435.
- Third retry evidence: Coverage MCP run
  `af98ad20-576a-4cdd-b808-e429523b43a1`, snapshot
  `ad5406ab-9f65-430b-9fdf-233f3d7cfa4d`, passed but did not improve overall
  or target-file branch coverage. The quality-0 distinct-token probe was
  removed. Keep the accepted histogram batch at 110 / 112 branches.
- Final accepted Coverage MCP run
  `a093b41e-690f-4427-81e1-e136a59928cb`, snapshot
  `45d0cf57-072b-4d3a-a50e-8bd48be7ce5f`, passed with 5 passed, 0 failed.
  Overall improved to 22213 / 22214 lines, 3358 / 3466 branches,
  1534 / 1534 functions. `histogram.rs` is now 530 / 530 lines,
  110 / 112 branches, and 31 / 31 functions. Remaining histogram gaps are
  line 435 and line 524.
- Pushed-head verification run
  `a38eefde-529e-4368-8d72-1f070691d1e3`, snapshot
  `cadf1be3-57a2-4afa-b3fc-1bb7b09bf8de`, commit
  `863040deb907c5c8c7334b801aadfdbdc6f91ad1`; passed with 5 passed,
  0 failed. Pushed-head overall coverage is 22213 / 22214 lines,
  3358 / 3466 branches, and 1534 / 1534 functions. Histogram remains
  530 / 530 lines, 110 / 112 branches, and 31 / 31 functions.

## Planned WebP native encoder reverse-mapping batch

Coverage MCP snapshot `2a1434f6-1ae6-41bc-8c3d-f87c13b91ef3`, measured at
commit `966673dc47636ad813d6d96240140143cfafec5f`, reports
`src/codecs/webp/native/encoder.rs` at 1085 / 1090 lines,
193 / 198 branches, and 63 / 63 functions.

Current exact gap lines:

- line 247: zero-length Huffman-code repeat loop.
- line 379: Huffman tree trailing-zero trim loop guard.
- line 392: `trimmed_length > 1 && trailing_zero_bits > 12`.
- lines 395 and 396: compact tree-length encoding for `trimmed_length == 2`.
- lines 412, 516, and 596: separately counted continuation lines in
  multi-line `write_bits()` / `write_huffman_tree()` calls.
- line 795: `EntropyMode::SubtractGreen` histogram selection.
- line 1043: `encode_alpha()` palette sorting guard
  `sortable_len > 17 && palette_values[0] == 0`.
- line 1137: RIFF chunk odd-padding write and generic `Write` error path.

Reverse-mapping evidence and attack plan:

- line 247 is a structural artifact, not a real input state. A zero run always
  starts with `repetitions >= 1`; after the only subtracting branch
  (`repetitions >= 139`) the value remains at least 1, and all following
  branches `break` before `repetitions` can become zero. A reverse probe over
  run lengths 1..999 found no post-body false edge. Fix type: refactor this
  private helper loop to `loop` so the unreachable guard branch disappears.
- line 379's false side is also structural after the simple-tree fast path:
  normal-tree token streams always contain at least one non-trimmable token
  (`1..=15` or `16`) before any trailing zero tokens. A reverse search over
  13,488 patterned/random frequency vectors found no `trimmed_length == 0` or
  `trimmed_length <= 1` state. Fix type: keep a debug assertion for the
  invariant and trim with a guard that assumes the token stream is non-empty.
- line 392 is the same short-circuit shape: the normal path should have
  `trimmed_length > 1`; the meaningful decision is `trailing_zero_bits > 12`.
  Fix type: assert the invariant and remove the redundant runtime predicate.
- lines 395 and 396 are real Huffman serialization behavior. Reverse mapping
  found a concrete code-length shape `[2, 2, 2, 2, 0...]`, produced by four
  equal symbols followed by a long zero tail. It compresses to tokens
  `[2, 16, 18, 18]`, leaves `trimmed_length == 2`, computes
  `trailing_zero_bits == 16`, and must write the special five zero bits.
  Fix type: add a narrow private coverage probe using this frequency vector;
  do not delete the branch.
- lines 412, 516, and 596 are real writes, but the uncovered lines are only
  rustfmt-created continuation/closing lines around already-hit calls. Fix
  type: split the arguments into named locals and keep the fallible calls on
  semantic lines. If coverage still reports an I/O error branch, use a standard
  `Cursor<&mut [u8]>` short-buffer probe rather than a custom helper writer.
- line 795 is real public encoder behavior. A 17x17 RGB input with random
  green values and red/blue as small offsets from green produces these
  approximate entropy costs: `Direct` 52,433,119,679; `Spatial`
  52,788,514,929; `SubtractGreen` 22,205,802,114;
  `SpatialSubtractGreen` 25,075,785,173; `Palette` 33,494,755,664.
  `SubtractGreen` wins. Fix type: add a PNG source fixture and a WebP lossless
  manifest encode row only if exact Pillow encoded-byte parity passes.
- line 1043 is real public lossy-alpha behavior. A 17-value alpha palette
  `[0, 1, 2, 3, 4, 5, 6, 7, 8, 248, 249, 250, 251, 252, 253, 254, 255]`
  has `signs == 3`, `palette_values[0] == 0`, and `sortable_len > 17` false.
  Fix type: add an RGBA PNG source and WebP lossy manifest encode row if
  byte-exact Pillow parity passes; otherwise keep this as a focused
  `encode_alpha()` coverage probe because the branch belongs to the alpha
  chunk helper.
- line 1137 is not image-algorithm behavior; it exists because any generic
  `Write` implementation may fail while emitting RIFF padding. Fix type: use a
  standard short `Cursor<&mut [u8]>` probe to cover padding-write failure
  without adding a custom helper writer.

Do not keep a fixture or private probe unless the approved Coverage MCP
line+branch run proves it reduces the gap without introducing new uncovered
code.

Completed evidence:

- First adjusted Coverage MCP run:
  `448973ed-4885-4624-8e27-13cfe8eb3a0a`, failed with one manifest parity
  mismatch. The rejected public alpha row
  `enc_lossy_alpha_17_values` produced 154 bytes from the Rust encoder while
  Pillow's encoded-byte oracle produced 138 bytes, so that row and its
  generated assets were removed.
- Second Coverage MCP run:
  `e9deab7e-2ecb-4ff3-a618-84d104cef994`, snapshot
  `d127e0c6-8c89-4a0c-b405-96e438a70c4e`, passed with 5 passed and 0 failed.
  It improved `encoder.rs` to 1110 / 1110 lines and 191 / 192 branches,
  leaving only normalized partial lines 1043 and 1137.
- Final Coverage MCP run:
  `cc6a42c4-073e-4258-a46b-0be9119a90a5`, snapshot
  `974e7c51-55e2-4764-9ad2-3868d0cb68df`, passed with 5 passed and 0 failed.
  Overall coverage is now 22759 / 22773 lines, 3384 / 3432 branches, and
  1549 / 1549 functions. `src/codecs/webp/native/encoder.rs` is now
  1123 / 1123 lines, 192 / 192 branches, and 63 / 63 functions. MCP still
  lists line 1137 as a normalized partial line, but the file aggregate reports
  no missing lines or branches.
- Accepted public fixture: `enc_lossless_subtract_green` with PNG source
  `webp_subtract_green.png`; it uses Pillow encoded-byte and roundtrip-byte
  references generated through the manifest.
- Accepted private probes: the four-symbol Huffman tree trim case, 17-value and
  nonzero-leading alpha helper cases, and standard short-buffer `Cursor` RIFF
  writer-error cases. These are kept because reverse mapping showed they
  represent real encoder states or generic `Write` behavior and the final MCP
  run closed the target file's aggregate branch gap.

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

## Planned ICO decoder private-branch batch

Coverage MCP pushed-head snapshot `e515e082-a166-4458-8e1f-3667e834342a`
reports `src/codecs/ico/decode.rs` at 299 / 299 lines, 51 / 64 branches, and
11 / 11 functions. The remaining branch lines are:

- line 43: zero-count and too-many-entry header validation. Existing fixtures
  cover zero entries; add a byte-level too-many count header.
- line 69: best-entry selection. Add a directory with a later smaller/tied
  entry so the `score > best_score` false side is reached.
- line 100: zero size vs zero offset entry validation. Existing malformed
  fixture covers both zero; add separate size-zero and offset-zero directory
  entries.
- line 109: PNG-entry detection. Existing fixtures cover valid PNG and DIB;
  add a short non-PNG payload that is shorter than eight bytes.
- line 136: CUR DIB header validation. Add separate header-size-less-than-40
  and header-size-at-least-40-but-too-short byte-level CUR entries.
- line 192: ICO DIB dimension validation. Add byte-level DIBs for zero width,
  zero actual height, too-wide, and too-tall values.
- lines 293, 344, and 398: default palette-size selection for 8-bit, 4-bit,
  and 1-bit DIBs. Existing fixtures cover explicit palette paths; add direct
  helper calls for both explicit `colors_used` values and zero/default
  `colors_used` values.
- lines 380 and 382: odd-width 4-bit high/low-nibble guards. Add a direct
  odd-width 4-bit DIB helper input to drive the skipped low-nibble side and an
  even-width input to keep the low-nibble body covered after formatting. The
  high-nibble false side appears structurally unreachable because row iteration
  stops at the logical 4bpp row byte count, not padded bytes.

No Pillow manifest fixture is needed for these branches in this batch because
the public malformed ICO matrix already exists and these are narrow container
or DIB helper predicates. Use a `cfg(coverage)` decoder hook that feeds exact
byte slices into the private helpers, then verify with the approved MCP
line+branch command.

Additional pre-run adjustment after snapshot
`3567e72d-058c-4b53-86a1-f70fa46ecdc0`: the first ICO probe restored the
overall one-line gap and improved `ico/decode.rs` from 51 / 64 to 59 / 64
branches. The next probe adds direct coverage for the CUR short-data second
side and zero/default palette branches for 8bpp, 4bpp, and 1bpp indexed DIBs.

Completed evidence:

- Final Coverage MCP run:
  `596f46f2-5ff7-45b2-b5dc-257c574015c5`
- Final Coverage MCP snapshot:
  `d9fda7bd-820f-4723-bffa-1bcd9419a1d1`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 22155 / 22156 lines, 3348 / 3464 branches, 1531 / 1531 functions.
- Target file: `src/codecs/ico/decode.rs` improved from 299 / 299 lines,
  51 / 64 branches, and 11 / 11 functions to 384 / 384 lines,
  63 / 64 branches, and 13 / 13 functions.
- The only remaining ICO decoder branch is the false side of the 4bpp
  high-nibble guard at line 383. It appears structurally unreachable because
  row iteration stops at the logical row byte count, so every visited byte
  starts with `col < width`.
- Pushed-head verification run:
  `5afecdd0-276f-4842-9fe5-259c05a906ef`, snapshot
  `2c9ade72-029d-49d1-8592-eaabd98705d5`, commit
  `8b482ca6fee835db5445def8341c2d6d2e702740`; passed with 5 passed,
  0 failed. Pushed-head overall coverage is 22155 / 22156 lines,
  3348 / 3464 branches, and 1531 / 1531 functions. `ico/decode.rs` remains
  384 / 384 lines, 63 / 64 branches, and 13 / 13 functions.

## Planned GIF encoder private-branch batch

Coverage MCP snapshot `1a0052ae-3f48-45e2-bfe2-1566218dd68d` reports
`src/codecs/gif/encode.rs` at 1147 / 1147 lines, 209 / 220 branches, and
104 / 104 functions. The remaining normalized branch lines are:

- lines 118 and 132: animated-frame coalescing and transparent-palette alpha
  update. Prefer manifest animated-GIF fixtures later; do not invent broad
  sequence fixtures in this private-helper batch.
- line 175: `rgba_difference_bounds()` has debug assertion branches. Do not
  call impossible no-difference input because debug assertions are active in
  coverage builds.
- line 409: `global_table` is a local constant set to true; remove the runtime
  branch and write the global color table directly.
- line 444: transparent unchanged-pixel masking after coalescing. Defer to an
  animated GIF fixture batch unless a minimal private `PreparedImage` state is
  clearly safer.
- lines 914, 930, and 937: median-cut split loop/debug assertion edges. Add
  deterministic private `MedianBox` calls for split-boundary shapes; remove
  only branches proven to be invariants.
- line 1053: RGBA palette optimization compacts holes or shrinks half-used
  palettes. Add deterministic `quantize_rgba()` inputs for both compact and
  no-compact sides.

The safe scope for this batch is internal helper coverage plus a constant
branch cleanup. Public animation behavior should stay fixture-driven in a
later manifest batch.

Completed evidence:

- First Coverage MCP run with a newly wired GIF private hook:
  `cfd0e34d-fb33-4664-8b70-df250aa83d88`; failed in
  `test_internal_coverage_hooks` due an incorrect hook assertion about the
  opaque RGBA palette size, so no coverage snapshot was ingested.
- Corrected hook run: `5b2ea3b5-92cd-43d6-a7f1-5bed9cf28996`, snapshot
  `fa014085-fbb2-4838-8fce-35c4234c32c1`; passed, but the helper probes did
  not close their intended GIF branches. They were removed rather than keeping
  noisy coverage-only code.
- Final Coverage MCP run with only the constant `global_table` branch cleanup:
  `75c1f8ed-4156-4fc0-ae51-892f59ca069a`
- Final Coverage MCP snapshot: `48599839-c1aa-4c44-b18a-243dcb0e8aa3`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall: 22066 / 22067 lines, 3336 / 3464 branches, 1528 / 1528 functions.
- Target file: `src/codecs/gif/encode.rs` is 1147 / 1147 lines,
  208 / 218 branches, and 104 / 104 functions. The constant global-table
  branch is closed. Remaining GIF encoder gaps need fixture-driven animation,
  transparent masking, median-cut, and RGBA palette-optimization work.
- Pushed-head verification run:
  `7139eab7-e226-4122-b41c-dfcb656b20ca`, snapshot
  `e515e082-a166-4458-8e1f-3667e834342a`, commit
  `97f275ea807a5432d2f1ed8bfdb096320a6093f8`; passed with 5 passed, 0 failed.
  Pushed-head overall coverage is 22066 / 22067 lines, 3336 / 3464 branches,
  and 1528 / 1528 functions. GIF encode remains 1147 / 1147 lines,
  208 / 218 branches, and 104 / 104 functions.

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

### JPEG encoder progressive/private-branch batch

- Clean pushed baseline snapshot for source decisions:
  `5b789fe0-a42e-4905-9cd2-33893a50684a`.
- Current pushed commit: `32dc417cd6cfd1b67542882cde4711460f083578`.
- Baseline overall coverage: 22213 / 22214 lines, 3358 / 3466 branches,
  1534 / 1534 functions.
- Current `src/codecs/jpeg/encode/mod.rs`: 808 / 808 lines and
  137 / 144 branches.
- Remaining JPEG encoder gap lines:
  - line 48: `debug_assert!(w > 0 && h > 0)` lacks zero-width and/or
    zero-height assertion-failure sides. These are private invariant checks,
    not valid Pillow encode cases.
  - line 373: `downsample()` full-size fast path lacks the complementary
    sampling-ratio side in direct helper coverage.
  - line 397: `debug_assert!(vr == 1 || vr == 2)` lacks the invalid vertical
    ratio assertion-failure side. This is a private invariant check.
  - line 875: progressive interleaved DC event generation has an unexercised
    padded-MCU guard side. Add a public manifest-driven progressive odd-size
    RGB JPEG encode row first, because Pillow can express that image shape.
  - line 1033: progressive AC refinement EOB flush threshold lacks one
    threshold side. If public fixtures still cannot force the threshold, use a
    private direct `append_ac_refine_events()` probe because this is internal
    scan-script state after coefficients are already generated.

Planned actions:

1. Try `enc_progressive_odd` in `manifest.yaml` using `17x17.jpg` with
   `progressive: true`; regenerate JPEG oracle references through the existing
   pinned Pillow script. Reject the row unless it matches exact encoded bytes,
   not just decoded pixel bytes.
2. Extend the existing `#[cfg(coverage)]` JPEG encode hook with deterministic
   private probes:
   - `catch_unwind()` zero-width and zero-height `encode()` calls for line 48;
   - direct `downsample()` calls for 1x1 and 2x1/2x2 ratio sides, plus a
     caught invalid `vr = 3` call for line 397;
   - direct progressive AC-refine state if the manifest row does not close
     line 1033.
3. Run the approved Coverage MCP line+branch command once, query the JPEG file,
   and keep only probes/fixture rows that reduce the real remaining deficit.

Rejected public fixture evidence:

- Added `enc_progressive_odd` with `17x17.jpg` and `progressive: true`, then
  regenerated JPEG oracle refs with the pinned Pillow script.
- Coverage MCP run `d6652d51-e279-454a-93d8-caeb867fd1d0` failed
  `test_encode_matrix`: `enc_progressive_odd` encoded byte length was 745
  actual vs 748 expected. The row was removed because encode parity rows must
  match exact bytes.
- Regenerated JPEG refs after removal; only the doc and private JPEG encode
  hook remain dirty for the retry.

First private-hook retry evidence:

- Coverage MCP run: `6630fd9f-797f-47cc-b4e4-c28803405aa6`.
- Coverage MCP snapshot: `df001545-a993-4573-bce6-d7509ad41fb3`.
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall coverage improved from 3358 / 3466 branches to 3362 / 3466
  branches.
- `src/codecs/jpeg/encode/mod.rs` improved from 137 / 144 branches to
  141 / 144 branches.
- Remaining JPEG encoder gaps:
  - shifted line 415: `if hr == 1 && vr == 1` still lacks the
    `hr == 1` / `vr != 1` side.
  - shifted line 917: progressive interleaved padded-MCU guard still lacks
    two false sides. The rejected public fixture showed this is not currently
    byte-perfect as a manifest row.

Next private-hook retry:

- Add a caught `downsample(..., hr = 1, vr = 2)` call for line 415.
- Add a direct `dc_progressive_events()` call with a Y component sized 3x3
  blocks at 2x2 sampling and chroma components sized 1x1 blocks at 1x1
  sampling. This hand-built state forces valid in-bounds Y blocks plus
  out-of-bounds chroma row/column guards in the progressive interleaved DC
  loop without adding a non-byte-perfect public fixture.

Second private-hook retry evidence:

- Coverage MCP run: `d240941a-3a80-4ad0-92af-6085268b5b9f`.
- Coverage MCP snapshot: `51ef9e74-5223-409e-884a-cf3e42209291`.
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall coverage improved from 3362 / 3466 branches to 3365 / 3466
  branches.
- `src/codecs/jpeg/encode/mod.rs` improved from 141 / 144 branches to
  144 / 144 branches. No branch gaps remain in the JPEG encoder.
- Total progress from the clean pushed baseline for this batch:
  `src/codecs/jpeg/encode/mod.rs` moved from 137 / 144 branches to 144 / 144
  branches, and overall coverage moved from 3358 / 3466 branches to
  3365 / 3466 branches.

### GIF encoder private-branch batch

- Pushed-head snapshot before batch:
  `2091e19c-44c4-4d2f-9e32-db2cde447f15`.
- Current pushed commit: `f4cca87e7ac707f176199390f38e163cee11d003`.
- Baseline overall coverage: 22314 / 22315 lines, 3365 / 3466 branches,
  1538 / 1538 functions.
- Current `src/codecs/gif/encode.rs`: 1147 / 1147 lines and
  208 / 218 branches.
- Remaining GIF encoder gap lines:
  - lines 118, 132, and 444: animated-frame coalescing and transparent-index
    predicates.
  - line 175: private `rgba_difference_bounds()` debug invariant.
  - lines 914, 930, and 937: private median-cut split edge predicates.
  - line 1053: RGBA palette hole/half-size compaction predicate.

Planned first GIF retry:

- Add a `#[cfg(coverage)]` GIF encode private hook and wire it through
  `gif::mod.rs` and the central codec hook.
- Start with deterministic internal data only:
  - call `rgba_difference_bounds()` on identical buffers inside `catch_unwind`
    to cover the assertion-failure side of line 175;
  - call `split_median_box()` with hand-built two-color boxes and deliberately
    large `pixel_count` to force the `while split < sorted.len()` exit side and
    the second `while` false side at line 930;
  - call `split_median_box()` with equal colors inside `catch_unwind` to cover
    the invalid split assertion side where possible;
  - call `quantize_rgba()` on a small all-opaque palette with every color used
    so line 1053 takes the no-compaction side.
- Leave lines 118, 132, and 444 for a second pass unless this hook naturally
  closes them. Those branches are tied to public animated GIF byte behavior and
  should be handled separately with exact-byte fixture evidence or a carefully
  scoped coalescing hook.

First GIF retry evidence:

- Coverage MCP run: `049e0a9e-9d00-44c9-8e7c-958f29c5d8c3`.
- Coverage MCP snapshot: `514701b1-986d-4276-9d03-47fd5047b72a`.
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall coverage improved from 3365 / 3466 branches to 3369 / 3466
  branches.
- `src/codecs/gif/encode.rs` improved from 208 / 218 branches to
  212 / 218 branches.
- Remaining GIF gaps shifted to lines 150, 164, 207, 476, 969, and 1085.

Second GIF retry plan:

- Use a deterministic two-frame 16x16 RGB sequence in the private hook:
  first frame all black, second frame with 256 unique RGB colors. This should
  exercise the coalescing branch where `prepared.transparent` is `None` but the
  palette is full, covering line 150 and line 164 false sides without adding a
  non-byte-verified public fixture.
- Feed the same coalesced frames into `write_gif()` with default settings to
  exercise line 476 where a later frame has no transparent index.
- Extract the RGBA palette compaction predicate into a private helper and call
  it directly with a compact all-used palette, covering line 1085's
  no-compaction side. This keeps public `quantize_rgba()` behavior unchanged
  while making the predicate independently testable.
- Keep line 969's `split < sorted.len()` false side under observation. If the
  branch remains structurally unreachable after direct split probes, record it
  as a candidate for simplification rather than adding noisy hook code.

Second GIF retry evidence:

- Coverage MCP run `80fccd35-a4db-48e9-be60-55ef3a25d6d6`, snapshot
  `1a2f359d-384a-4f92-ab53-7c92ac97417f`, passed and improved branches, but
  introduced one uncovered hook line and two extra branch obligations through
  an unnecessary `if let` guard around deterministic coalescing. The guard was
  removed before commit.
- Cleaned Coverage MCP run: `3324172a-9f69-4d2e-b87d-1a299b8997fd`.
- Cleaned Coverage MCP snapshot: `edbcff2f-4f2e-4a12-9c1d-4909ee8a6d74`.
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall coverage improved from the pushed-head baseline 3365 / 3466
  branches to 3372 / 3466 branches.
- `src/codecs/gif/encode.rs` improved from 208 / 218 branches to
  215 / 218 branches. Remaining GIF encoder gaps are shifted lines 261, 1023,
  and 1152.

### Smallest remaining branch cleanup batch

- Pushed-head snapshot before batch:
  `620c8442-e7e5-49f2-aaf5-0883a35c2bca`.
- Current pushed commit: `953d2212c91934c7bc8f3b93b5b06f4732ad7bcc`.
- Baseline overall coverage: 22409 / 22410 lines, 3372 / 3466 branches,
  1543 / 1543 functions.
- Smallest branch deficits:
  - `src/codecs/ico/decode.rs`: 1 missing branch at line 383.
  - `src/codecs/webp/native/encoder/histogram.rs`: 2 missing branches at
    lines 435 and 524.
  - `src/codecs/gif/encode.rs`: 3 missing branches at shifted lines 265,
    1027, and 1156.

Planned actions:

1. Fix ICO first. The line-383 4bpp high-nibble guard is structurally
   unreachable on the false side for a valid nonzero-width row: every byte read
   contains a high nibble pixel; only the low nibble needs an odd-width guard.
   Remove the redundant high-nibble `if` and leave the low-nibble guard intact.
2. Add one GIF private hook call for RGBA palette compaction with a real hole
   (`used_indices = [0, 2]`) to exercise the `has_holes` side of line 1156.
3. Do not force GIF line 265 or 1027 unless a clean direct probe closes them:
   both are debug assertions whose remaining false side appears structurally
   coupled to an earlier predicate.
4. Leave WebP histogram for the next small-file pass unless the first Coverage
   MCP run shows this batch regresses or adds noisy obligations.

First small-cleanup evidence:

- Coverage MCP run: `33c897f0-6ffe-4737-b72c-a50fe44f532a`.
- Coverage MCP snapshot: `94bf894d-4240-4a05-9fdd-f9d45e526fac`.
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall branch deficit improved from 94 missing branches to 92 missing
  branches: 3372 / 3466 became 3372 / 3464.
- `src/codecs/ico/decode.rs` improved from 63 / 64 branches to 62 / 62
  branches by removing the unreachable 4bpp high-nibble guard.
- `src/codecs/gif/encode.rs` improved from 215 / 218 branches to
  216 / 218 branches by directly covering RGBA palette-hole compaction.

Next smallest-file plan:

- `src/codecs/webp/native/encoder/histogram.rs` remains 110 / 112 branches
  with gaps at lines 435 and 524.
- Try direct private histogram queue states first:
  - line 435: create a stochastic queue entry that touches a merged histogram
    but becomes invalid after recomputing pair costs, so `update_pair()` returns
    false and the stale pair is removed.
  - line 524: call `cluster()` with a token shape where
    `stochastic_combine()` returns false, covering the no-greedy-combine side.
- Remove any histogram probe that does not reduce the real branch deficit.

Rejected histogram formatting evidence:

- Coverage MCP run: `ac02543d-e57a-4ba5-aaa4-ddb31b4b2136`.
- Coverage MCP snapshot: `85b49568-09b3-47b4-ae87-5c2461052fe5`.
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Splitting the two one-line histogram branches did not reduce the branch
  deficit and introduced uncovered line noise in `histogram.rs`. The histogram
  edit was reverted; keep the next histogram attempt to actual state probes or
  a behavior-preserving simplification that reduces measured deficit.

Reverse-mapping conclusion for GIF:

- A small predicate search over `rgba_difference_bounds()` showed that every
  non-empty difference set produces `(left < right, top < bottom) == (true,
  true)`. The false side is only reachable by violating the caller contract
  that invokes this helper after a rendered-frame difference is known.
- Reverse-mapping `split_median_box()` gives the same result for valid
  median-cut candidates: the caller only splits boxes with RGB volume greater
  than one, and the selected-axis tie adjustment normalizes the split to a
  nonempty left and right partition. The debug assertion false side requires an
  invalid all-equal box, not a valid image-derived median-cut state.
- Remove both debug-only assertions rather than fabricate impossible inputs.
  This is behavior-preserving for release builds and better represents the
  reachable branch surface.

Reverse-mapped cleanup evidence:

- Coverage MCP run: `b6a72657-2579-400a-b1a3-8d0671fcfb99`.
- Coverage MCP snapshot: `5283e9d4-219c-4ab5-9a32-85f0048f3b7f`.
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Overall branch deficit improved from 94 missing branches to 90 missing
  branches: 3372 / 3466 became 3366 / 3456.
- `src/codecs/gif/encode.rs` improved from 215 / 218 branches at the pushed
  baseline to 210 / 210 branches. No GIF encoder branch gaps remain.
- `src/codecs/ico/decode.rs` improved from 63 / 64 branches at the pushed
  baseline to 62 / 62 branches. No ICO decoder branch gaps remain.

## Refreshed missing branch map after rustfmt

Coverage MCP snapshot `c3129258-5bc6-4b61-b276-de541e53b26c`, measured at
commit `8bc46868de477a12eaf6d01094d38bb95c9636fd`, reports 3368 / 3456
branches. The remaining 88 branch gaps are:

| File | Branches | Deficit | Current gap lines |
|---|---:|---:|---|
| `src/codecs/gif/decode.rs` | 71 / 76 | 5 | 60, 164, 246, 317 |
| `src/codecs/jpeg/decode/decode.rs` | 65 / 70 | 5 | 164, 295, 399, 410, 415 |
| `src/codecs/png/decode.rs` | 81 / 86 | 5 | 32, 44-45, 53, 407 |
| `src/codecs/webp/native/encoder.rs` | 193 / 198 | 5 | 247, 379, 392, 395, 1043, 1137 |
| `src/codecs/jpeg/decode/parser.rs` | 91 / 98 | 7 | 99, 164, 278, 382, 410 |
| `src/codecs/webp/native/vp8.rs` | 154 / 162 | 8 | 969, 1033, 1056, 1067, 1104, 1125, 1129, 1173, 1177, 1183, 1192, 1204, 1215, 1229, 1409, 1459, 1472, 1814, 1825, 1867 |
| `src/codecs/webp/native/decoder.rs` | 74 / 84 | 10 | 196, 264, 274-277, 427, 533, 543, 595 |
| `src/codecs/bmp/decode.rs` | 109 / 122 | 13 | 222, 247, 263, 294, 493, 503 |
| `src/codecs/jpeg/decode/progressive.rs` | 104 / 118 | 14 | 84, 103, 133, 155, 160, 173, 183, 186, 209, 533, 595, 696 |
| `src/codecs/tiff/decode.rs` | 100 / 114 | 14 | 32, 43, 51, 74, 79, 113, 253, 296, 418, 469 |
| `src/codecs/webp/native/lossless.rs` | 108 / 110 | 2 | LLVM-normalized projection lists many partial lines; defer until a tighter branch-direction signal is available. |

Execution order for this pass: fix one file at a time, starting with clean
five-branch decode files. Each file gets a local reverse-mapping step before
any fixture or private probe is added.

## Planned GIF decode reverse-mapped batch

Coverage MCP snapshot `c3129258-5bc6-4b61-b276-de541e53b26c` reports
`src/codecs/gif/decode.rs` at 313 / 313 lines, 71 / 76 branches, and
20 / 20 functions. This is the first target because the gap map is small and
specific, unlike the LLVM-normalized `lossless.rs` map.

Reverse-mapping targets before the next Coverage MCP run:

- line 60: application extension loop parsing:
  `is_loop_extension && payload.first() == Some(&1)`. Generate or directly
  decode application extensions covering non-loop identifiers, loop identifiers
  with first payload byte not `1`, and loop identifiers with short payloads.
  Use Pillow-backed fixture rows only if Pillow accepts the resulting file;
  otherwise use a coverage-only parser probe for malformed extension variants.
- line 164: image descriptor rejects `width == 0 || height == 0`. Build tiny
  image descriptors with zero width and zero height. These should be malformed
  decode rows or private decode probes, not valid Pillow parity rows unless
  Pillow accepts them consistently.
- line 246: first LZW data code guard:
  `code >= clear_code || output.len() >= expected_len`. Reverse-map direct LZW
  byte streams for both failure sides: first code at/above clear code and an
  already-full output case. This is an internal decoder-state predicate; prefer
  direct `decode_lzw()` probes if public GIF bytes cannot naturally isolate it.
- line 317: `append_code()` debug assertion while following dictionary
  prefixes. Reverse-map whether valid `decode_lzw()` dictionary construction can
  ever make `usize::from(code) >= MAX_LZW_CODE` or `len >= MAX_LZW_CODE`. If
  unreachable for valid tables, simplify the debug assertion rather than
  fabricate invalid private state.

The first debugging action is a small code-level LZW packer/search that derives
the shortest byte streams reaching line 246 and the valid/invalid dictionary
shape behind line 317. Do not run coverage again until the candidate input or
unreachable-branch simplification is documented here.

Debug result:

- Existing fixture `lzw_invalid_first.gif` maps to the first line-246 failure
  side: first data code `6`, clear code `4`, `code >= clear_code == true`, and
  `output.len() >= expected_len == false`.
- A direct LZW stream with first data code `0`, minimum code size `2`, and
  `expected_len = 0` maps to the other line-246 side:
  `code >= clear_code == false` and `output.len() >= expected_len == true`.
  Public GIF image descriptors cannot reach this because `decode_image()` first
  rejects zero width/height. Add this as a coverage-only `decode_lzw()` probe.
- Existing fixture `zero_frame_width.gif` maps to the line-164 `width == 0`
  side. Add `zero_frame_height.gif` as a manifest malformed decode fixture for
  the `height == 0` side.
- Existing fixture `animext_loop.gif` maps to line 60 with a recognized
  application identifier and payload byte `1`; `unknown_application.gif` maps
  to an unrecognized identifier. Add `animext_bad_payload.gif`, a recognized
  `ANIMEXTS1.0` extension with first payload byte `0`, for the short-circuited
  payload side.
- Line 317 is a debug-only invariant over `append_code()` prefix traversal.
  Valid `decode_lzw()` construction only calls `append_code()` with codes below
  `next_code`, inserts dictionary entries while `next_code < MAX_LZW_CODE`,
  and stores prefixes from the same valid code domain. The false side would
  require an invalid private prefix table, so remove the debug assertion rather
  than fabricate impossible state.

Completed evidence:

- Regenerated only GIF assets/references with:
  `.oracle-venv/bin/python scripts/generate_test_assets.py --format gif` and
  `.oracle-venv/bin/python scripts/generate_decode_refs.py --format gif`.
- Coverage MCP run `e8114f56-7728-4412-921d-7112169285d5`, snapshot
  `6a210c57-879e-431f-a8df-fedcf37b9c68`, passed with 5 passed and 0 failed;
  coverage artifact was ingested.
- Overall coverage improved from 3368 / 3456 branches at snapshot
  `c3129258-5bc6-4b61-b276-de541e53b26c` to 3369 / 3452 branches, reducing
  missing branches from 88 to 83.
- `src/codecs/gif/decode.rs` improved from 71 / 76 branches to 72 / 72
  branches, 316 / 316 lines, and 21 / 21 functions. No GIF decoder branch gaps
  remain.
- Committed-head verification run
  `73399bbd-6105-4832-9d66-f33a93533619`, snapshot
  `ca7bd301-b9d3-40a4-8c8b-11d249c80184`, passed with 5 passed and 0 failed
  at commit `79c75eeee9c332c7d8287396cdb675f6e144647a`; coverage artifact was
  ingested and retained the same 3369 / 3452 branch result.

## Execution order

## Planned WebP histogram reverse-mapped cleanup

Coverage MCP pushed-head snapshot `4ddc5168-3d14-413e-92c6-6faed2fb1097`
reports `src/codecs/webp/native/encoder/histogram.rs` at 530 / 530 lines,
110 / 112 branches, and 31 / 31 functions. This is the smallest clean
remaining branch target: exactly two single-branch gaps.

Reverse-mapping targets before the next Coverage MCP run:

- line 435: inside `stochastic_combine()`, after a chosen pair is merged,
  queued pairs that touched either merged histogram are remapped to the new
  cluster. The uncovered side is expected to be the drop path where
  `update_pair(histograms, &mut pair, 0)` returns `false`. A valid input must
  create a stochastic queue entry that is useful before the merge but no longer
  gives negative savings after remapping. Search by code over deterministic
  small histogram populations, not random image files, then add the minimal
  private coverage probe only if it matches the real queue/update state.
- line 524: `cluster()` invokes `greedy_combine()` only when
  `stochastic_combine(&mut clusters, threshold)` returns `true`. Existing
  probes already cover the cleanup path. The missing side is expected to be a
  token stream where stochastic clustering does not reach the quality-derived
  threshold. Search by code over generated one-pixel literal token streams with
  varied quality and `histogram_bits`, then add the smallest deterministic
  token stream that keeps `stochastic_combine()` above threshold.

Validation rule for this batch: first run a local reverse-mapping script or
temporary instrumentation that computes the exact predicate state. Then update
the coverage-only private hook with those concrete inputs. Do not add public
fixture rows for this batch because these branches are internal VP8L histogram
clustering heuristics after tokenization; Pillow byte/pixel rows are too broad
to act as the narrow oracle.

Attempt 1:

- A broad Python reverse search for the line-435 post-merge pair-drop state was
  stopped after it was too slow. Do not place that search loop inside the
  coverage hook.
- For line 524, the direct reverse mapping is simple from the source:
  `threshold = 1 + div_round(quality^3 * 99, 1_000_000)`, so `quality = 0`
  gives threshold `1`. A stream of `4 * BIN_SIZE` distinct one-pixel literal
  tokens creates many used clusters. The intended predicate state is
  `stochastic_combine(&mut clusters, 1) == false`, covering the side where
  `greedy_combine()` is skipped.
- Coverage MCP run `5cdf776a-885a-4b6d-b2ae-d47abd6f7823`, snapshot
  `6c90fff5-4e8e-4a0f-b651-fbe334cc605c`, passed but did not improve:
  overall remained 3366 / 3456 branches and `histogram.rs` remained
  110 / 112 branches with gaps at lines 435 and 524. The quality-0
  `many_distinct` probe was reverted as a no-op. Inference: either the
  generated clusters still reached the threshold, or the missing side on line
  524 is not exposed by this one-line source mapping without branch-direction
  detail from LLVM.

Debug result:

- A local reverse-mapping probe with the Rust `fast_slog()` math reproduced the
  failing assumption: both `small_tokens` and the `many_distinct` quality-0
  stream reduce to one cluster, so `stochastic_combine(..., 1)` returns `true`.
- The same probe found a deterministic internal state that exercises both
  remaining predicates: 24 histograms with 64 generated literals each, using
  pixel seed `i * 1000 + j * 37` and channels
  `r = seed * 73`, `g = seed * 151`, `b = seed * 199` modulo 256. That state
  leaves two clusters when `minimum = 1`, so line 524 should cover the
  false/no-greedy side. During the same run, after stochastic merge 7, remapped
  queued pairs fail `update_pair(..., 0)`, so line 435 should cover the
  queue-drop side.
- Coverage MCP run `108cb305-862c-4611-80b2-387812a92755`, snapshot
  `693015b1-5183-4a26-9e95-b10091dca632`, passed and improved
  `histogram.rs` from 110 / 112 to 111 / 112 branches. This confirms the
  direct stochastic probe hit line 435. It did not close line 524 because it
  bypassed `cluster()`, and rustfmt's split `if` introduced an uncovered
  closing-brace line. The formatting-only split was reverted.
- Next correction: feed the same 24 × 64 generated literals through
  `cluster()` with `histogram_bits = 6`, `width = token_count`, and
  `quality = 0`. This reconstructs the same 24 high-entropy histograms inside
  `cluster()` before line 524, so the branch at line 524 itself should observe
  `stochastic_combine(..., 1) == false`.
- Coverage MCP run `7d87df83-a7eb-4af3-a102-ced572d39151`, snapshot
  `e80f2f58-f6ba-4550-9428-8c97d96d7616`, passed with 5 passed and 0 failed;
  coverage artifact was ingested. Overall coverage improved from
  3366 / 3456 branches at pushed-head snapshot
  `4ddc5168-3d14-413e-92c6-6faed2fb1097` to 3368 / 3456 branches, leaving
  88 missing branches. `src/codecs/webp/native/encoder/histogram.rs` improved
  from 110 / 112 branches to 112 / 112 branches, 548 / 548 lines, and
  31 / 31 functions. No histogram branch gaps remain.

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

## Region-first sweep — 2026-07-21

Baseline from approved Coverage MCP command
`all-features-llvm-cov-json-nightly-branch`:

- Run: `ff36e0e5-b681-4edb-afb4-b14be7213644`
- Snapshot: `c6ad8d41-3d69-469f-8468-6948188a584c`
- Commit: `abebb1d43a5b8dcad8f0a7764a8aa202764284d3`
- Lines: `23467 / 23474`
- Branches: `3420 / 3434`
- Functions: `1569 / 1569`
- Regions: `39046 / 40408` (`1362` missing)

Coverage MCP exposes authoritative per-file region totals. LLVM region entries
were locally mapped back to source lines only to choose inputs; MCP remains the
source of truth for totals and final validation.

### Region gap table

| File | Regions | Missing regions | Branches |
| --- | ---: | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | `2701 / 3038` | `337` | `368 / 368` |
| `src/codecs/gif/encode.rs` | `2326 / 2450` | `124` | `210 / 210` |
| `src/codecs/tiff/decode.rs` | `1440 / 1551` | `111` | `120 / 120` |
| `src/codecs/ico/encode.rs` | `694 / 768` | `74` | `50 / 50` |
| `src/codecs/webp/native/encoder.rs` | `1739 / 1812` | `73` | `192 / 192` |
| `src/codecs/jpeg/encode/mod.rs` | `1430 / 1496` | `66` | `144 / 144` |
| `src/codecs/bmp/decode.rs` | `769 / 829` | `60` | `112 / 112` |
| `src/codecs/gif/decode.rs` | `573 / 633` | `60` | `72 / 72` |
| `src/codecs/compression/deflate.rs` | `554 / 608` | `54` | `48 / 48` |
| `src/codecs/webp/native/decoder.rs` | `1174 / 1225` | `51` | `82 / 88` |
| `src/codecs/webp/native/lossless.rs` | `853 / 903` | `50` | `108 / 110` |
| `src/codecs/ico/decode.rs` | `985 / 1030` | `45` | `62 / 62` |
| `src/codecs/png/decode.rs` | `655 / 699` | `44` | `86 / 86` |
| `src/codecs/tiff/encode.rs` | `610 / 643` | `33` | `86 / 86` |
| `src/codecs/jpeg/decode/parser.rs` | `526 / 558` | `32` | `96 / 96` |
| `src/codecs/webp/native/vp8.rs` | `2639 / 2667` | `28` | `154 / 160` |
| `src/codecs/png/encode.rs` | `442 / 466` | `24` | `36 / 36` |
| `src/codecs/jpeg/encode/huffman.rs` | `247 / 268` | `21` | `24 / 24` |
| `src/codecs/webp/encode/mod.rs` | `327 / 339` | `12` | `30 / 30` |
| `src/codecs/jpeg/decode/progressive.rs` | `1288 / 1300` | `12` | `112 / 112` |
| `src/types/dynamic.rs` | `1417 / 1428` | `11` | `4 / 4` |
| `src/codecs/webp/native/extended.rs` | `336 / 344` | `8` | `36 / 36` |
| `src/types/mod.rs` | `252 / 259` | `7` | `34 / 34` |
| `src/codecs/webp/decode.rs` | `105 / 111` | `6` | `6 / 6` |
| `src/codecs/mod.rs` | `99 / 103` | `4` | `6 / 6` |
| `src/types/buffer.rs` | `638 / 642` | `4` | `24 / 24` |
| `src/codecs/jpeg/decode/decode.rs` | `537 / 540` | `3` | `66 / 66` |
| `src/codecs/webp/native/byteorder_lite.rs` | `47 / 50` | `3` | `0 / 0` |
| `src/codecs/jpeg/encode/marker.rs` | `165 / 167` | `2` | `0 / 0` |
| `src/codecs/webp/native/huffman.rs` | `314 / 316` | `2` | `26 / 26` |
| `src/lib.rs` | `75 / 76` | `1` | `24 / 24` |

### First sweep plan

The first region pass targets encoder modules that already have public manifest
coverage but are not wired into the coverage-only private hook path:

| Target | Why this is first | Matching input strategy |
| --- | --- | --- |
| `ico::encode` | Missing regions are encode-side `?` failures, size parsing, resizing, BMP/PNG entry selection, and 256-dimension directory encodings. `ico::mod` currently only calls the decode hook. | Add a coverage-only hook with deterministic `DecodedImage` inputs: invalid zero-size image, L8 unsupported resize, RGB/RGBA BMP entries, explicit `sizes`, malformed `sizes`, and direct directory/helper probes for 256 and empty cases. |
| `png::encode` | Missing regions are encode-side mode selection, ancillary chunk requests, invalid image dimensions, palette/tRNS, filter arms, and checked capacity failures. `png::mod` currently only calls the decode hook. | Add a coverage-only hook using public `DecodedImage` values and direct private helper calls for every filter arm and ancillary option; no fixture row because these are internal encoder expression regions already byte-tested through manifest rows. |
| `tiff::encode` | Missing regions are encode-side mode/compression/predictor choices, PackBits state transitions, LZW empty input, and unsupported options. `tiff::mod` currently only calls the decode hook. | Add a coverage-only hook using small valid source images for L1/La8/L16/F32/I32/RGB/RGBA/CMYK, compression options, bad options, and direct PackBits/LZW helper probes. |
| `jpeg::encode` | `jpeg::encode::mod` currently only calls `huffman` hook. Missing regions include grayscale/RGB, subsampling/progressive/optimized/restart, and marker/bit-writer expression regions. | Extend the coverage-only hook to call representative grayscale/RGB encodes with standard options; keep deeper entropy reverse mapping for a later pass if branch totals do not move. |

Validation for this sweep:

1. Make only coverage-only hook/wiring changes and doc updates.
2. Run `cargo fmt`.
3. Run the approved Coverage MCP command
   `all-features-llvm-cov-json-nightly-branch`.
4. Query MCP summary/files for lines, branches, functions, and regions.
5. Record the new snapshot and per-file region deltas here.

### Attempt 1 result

Coverage MCP run `51b35891-9da0-44b8-b182-5753f365e7fb`, snapshot
`5be0aa8f-e24b-41d4-abdb-e00ee54eec10`, passed and ingested.

- Lines: `23668 / 23675`
- Branches: `3420 / 3434`
- Functions: `1575 / 1575`
- Regions: `39426 / 40782`
- Missing regions changed from `1362` to `1356`.

This closed only six pre-existing region gaps. The reason is now clear from
the reverse mapping: the broad encoder hooks covered the hook bodies, but most
remaining zero-region spans in `ico::encode`, `png::encode`, and
`tiff::encode` are unreachable conversion or checked-arithmetic failure
regions after `DecodedImage::validate()` or after ICO size filtering.

Correction before the next run:

- Stop adding broad hook code for these files.
- Simplify provably bounded regions:
  - `u32 -> usize` image dimensions are infallible on supported targets.
  - TIFF row-byte and IFD entry-count arithmetic is bounded after
    `DecodedImage::validate()`.
  - TIFF LZW encode is reached with non-empty validated pixels; keep the empty
    helper case as a direct defensive helper branch, not as an encoder `?`.
  - ICO resize dimensions are produced by ICO size filtering, and premultiply /
    unpremultiply byte-channel arithmetic is bounded by `u8` inputs.
- Re-run the same approved Coverage MCP command and record the real delta.

### Attempt 2 result

Coverage MCP run `87fc3bb8-4a9e-4729-8651-af492039c940`, snapshot
`427cd36b-90fa-47d5-ad7b-eeaffe0cb9cf`, passed and ingested.

- Lines: `23670 / 23677`
- Branches: `3422 / 3436`
- Functions: `1575 / 1575`
- Regions: `39375 / 40699`
- Missing regions: `1324`

Net from the region-first baseline: missing regions improved from `1362` to
`1324` (`38` fewer). File-level impact:

- `src/codecs/tiff/encode.rs`: missing regions improved from `33` to `13`.
- `src/codecs/ico/encode.rs`: missing regions improved from `74` to `60`.
- `src/codecs/png/encode.rs`: missing regions improved from `24` to `21`.

### Small-region cleanup batch

After the encoder invariant cleanup, the smallest remaining region files are
cheap to reverse-map and do not need fixtures:

| Target | Missing region cause | Fix type |
| --- | --- | --- |
| `src/lib.rs` | `decode_sequence()` format autodetect `None` side. | Add one coverage-only call with non-image bytes through the existing root hook. |
| `src/codecs/webp/native/byteorder_lite.rs` | `ReadBytesExt` read-short error regions for `read_exact`. | Add a coverage-only byteorder hook and wire it through `webp::native`. |
| `src/codecs/jpeg/encode/marker.rs` | `write_exif_app1()` oversized EXIF length failure. | Add exact oversized EXIF probe through the existing JPEG encoder hook. |
| `src/codecs/webp/native/huffman.rs` | `BitReader::consume()` error from slow and fast Huffman lookup paths. | Add exact empty-reader probes inside the existing Huffman hook. |

Run the same approved Coverage MCP command after this batch and record the
delta. Do not add broad hooks if the file already has an exact hook path.

### Attempt 3 result

Coverage MCP run `6fd70126-1593-45a3-98f2-f9054cd5fe0c`, snapshot
`26ae5d29-dab5-4ac5-9c9f-03a1dbe7cb51`, passed and ingested.

- Lines: `23699 / 23706`
- Branches: `3422 / 3436`
- Functions: `1576 / 1576`
- Regions: `39429 / 40747`
- Missing regions: `1318`

Net from the region-first baseline:

- Missing regions improved from `1362` to `1318` (`44` fewer).
- Region rate improved from `96.629%` to `96.765%`.
- Branch rate stayed effectively stable: `3420 / 3434` to `3422 / 3436`.

Small-file outcomes:

- `src/lib.rs`: `75 / 76` to `77 / 77` regions.
- `src/codecs/webp/native/byteorder_lite.rs`: `47 / 50` to `72 / 72`
  regions after adding exact read-short probes.
- `src/codecs/jpeg/encode/marker.rs`: `165 / 167` to `166 / 167`
  regions; one oversized EXIF region remains unmapped or merged by LLVM.
- `src/codecs/webp/native/huffman.rs`: `314 / 316` to `330 / 331`
  regions after adding fast/slow consume-error probes.

Remaining largest region gaps after attempt 3:

| File | Regions | Missing regions | Branches |
| --- | ---: | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | `2701 / 3038` | `337` | `368 / 368` |
| `src/codecs/gif/encode.rs` | `2326 / 2450` | `124` | `210 / 210` |
| `src/codecs/tiff/decode.rs` | `1440 / 1551` | `111` | `120 / 120` |
| `src/codecs/webp/native/encoder.rs` | `1739 / 1812` | `73` | `192 / 192` |
| `src/codecs/jpeg/encode/mod.rs` | `1474 / 1539` | `65` | `144 / 144` |
| `src/codecs/ico/encode.rs` | `748 / 808` | `60` | `50 / 50` |
| `src/codecs/bmp/decode.rs` | `769 / 829` | `60` | `112 / 112` |
| `src/codecs/gif/decode.rs` | `573 / 633` | `60` | `72 / 72` |
| `src/codecs/compression/deflate.rs` | `554 / 608` | `54` | `48 / 48` |
| `src/codecs/webp/native/decoder.rs` | `1174 / 1225` | `51` | `82 / 88` |

Next attack order:

1. `zlib_ng.rs`: do not add random PNG rows. Reverse-map checked-failure
   regions into compressor helper invariants. Simplify only when public
   validated image bytes make the failure impossible; otherwise add exact
   private helper states.
2. `gif/encode.rs`: use animated manifest inputs only for visible frame
   coalescing/palette behavior. Use private probes for quantizer and crop
   arithmetic states.
3. `tiff/decode.rs`: public malformed TIFF fixtures are appropriate for tag and
   storage rejects; private probes are appropriate only for compression helper
   internals.
4. WebP native decoder/VP8/VP8L branch-bearing files remain branch-priority
   after region-only cleanup because they still own the remaining branch gaps.

### Attempt 4 plan

Current Coverage MCP baseline before editing:

- Snapshot: `da32b2c0-1291-434d-b20b-32297d826718`
- Commit metadata: `33114d9b2994e8cdd2187f0ed0e6c3826aff1c3f`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Lines: `23699 / 23706`
- Branches: `3422 / 3436`
- Functions: `1576 / 1576`
- Regions: `39429 / 40747`
- Missing regions: `1318`

The top remaining region files are fully line-covered, so MCP's file-gap view
has no line ranges to report. The retained LLVM JSON was used only to map
zero-count region starts to functions; MCP remains the source of truth for
totals and validation.

Selected broad sweep:

| Target | Reverse-mapped gap | Action |
| --- | --- | --- |
| `src/codecs/tiff/decode.rs` | Region gaps at `data.get(...)?`, IFD count/entry bounds, and value-offset reads are public malformed-input states. | Add manifest-backed malformed TIFF assets for truncated signature/magic/IFD-offset bytes, truncated IFD counts/entries, and out-of-bounds tag value data. These are exact Pillow-error rows, not private hooks. |
| `src/codecs/gif/encode.rs` | The largest clusters are private encode expression regions in `coalesce_identical_frames`, `clear_frame_rect`, `composite_frame`, `prepare_image`, option parsing, and `OctreeCube::new`. Existing manifest rows already cover real GIF encode outputs. | Extend the `cfg(coverage)` hook with targeted invalid/private states: requested still-frame coalescing, background disposal clearing, transparent P-frame compositing, invalid option parsing, oversized GIF dimensions, invalid indexed palette lookup, and octree checked-shift/size edges. Do not add random GIF rows because the observable GIF encode matrix is already broad. |
| `src/codecs/compression/zlib_ng.rs` | Largest remaining clusters are checked arithmetic and slice-bound regions inside private matchers and Huffman writers. Public PNG compression rows already exercise the real level-specific byte oracle. | Add narrow `cfg(coverage)` probes for helper failure sides that are not public image behavior: short candidate comparisons, out-of-range quick inserts, and impossible chunk-length overflow states. Leave larger invariant simplifications for a separate zlib-only proof pass. |

Validation after this batch:

1. Regenerate deterministic fixture assets and the manifest-driven matrix.
2. Run `cargo fmt`.
3. Run only the approved Coverage MCP command
   `all-features-llvm-cov-json-nightly-branch`.
4. Query MCP summary and files, including lines, branches, functions, and
   regions.
5. Record the new snapshot and per-file movement here before continuing to the
   remaining WebP branch gaps.

### Attempt 4 result

Coverage MCP run `4e89ee2f-aea4-437f-bd41-7153a8d5f608`, snapshot
`e27f1a74-6d96-43c1-baa4-88429f54852d`, passed and ingested.

- Lines: `23900 / 23907`
- Branches: `3422 / 3436`
- Functions: `1576 / 1576`
- Regions: `39664 / 40944`
- Missing regions: `1280`

Net from attempt 3/current baseline:

- Missing regions improved from `1318` to `1280` (`38` fewer).
- Region rate improved from `96.765%` to `96.874%`.
- Branches stayed at `3422 / 3436`; the remaining branch gaps are still the
  WebP native decoder/VP8/VP8L files.

Target file movement:

| File | Before | After | Missing-region delta |
| --- | ---: | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | `2701 / 3038` | `2783 / 3100` | `337 -> 317` |
| `src/codecs/gif/encode.rs` | `2326 / 2450` | `2473 / 2585` | `124 -> 112` |
| `src/codecs/tiff/decode.rs` | `1440 / 1551` | `1446 / 1551` | `111 -> 105` |

Fixture rows added in this sweep:

- `truncated_signature.tiff`
- `truncated_magic.tiff`
- `truncated_ifd_offset.tiff`
- `truncated_ifd_count.tiff`
- `truncated_ifd_entry.tiff`
- `oob_tag_value_offset.tiff`

These rows are active manifest-driven Pillow error-oracle inputs under
`error_bad_ifd`, and were regenerated into `coverage_matrix.json` plus the TIFF
decode input/output JSON indexes. The GIF and zlib changes stayed behind
`cfg(coverage)` because their mapped gaps are private checked-arithmetic and
state-machine edges already covered for public output behavior by existing
manifest rows.

Next attack order from this result:

1. Continue region cleanup on `zlib_ng.rs`, but switch from adding probes to a
   dedicated invariant simplification pass. The broad helper probes improved
   only 20 missing regions while increasing total regions, so the next useful
   zlib work is proving/removing unreachable checked arithmetic.
2. Continue fixture-backed TIFF malformed coverage only where the mapped line
   corresponds to a public parser/storage boundary. The short-read fixtures
   covered the intended parser regions.
3. For GIF encode, prefer invariant simplification in validated frame geometry
   and quantizer internals. More hook volume has diminishing returns.
4. After region-only cleanup, return to the remaining branch gaps:
   `webp/native/decoder.rs` (6), `webp/native/vp8.rs` (6), and
   `webp/native/lossless.rs` (2).

### Attempt 5 plan: public malformed decoder sweep

Current Coverage MCP baseline before editing:

- Snapshot: `e27f1a74-6d96-43c1-baa4-88429f54852d`
- Commit metadata: `33114d9b2994e8cdd2187f0ed0e6c3826aff1c3f`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Lines: `23900 / 23907`
- Branches: `3422 / 3436`
- Functions: `1576 / 1576`
- Regions: `39664 / 40944`
- Missing regions: `1280`

The next region-first pass should avoid more broad private hooks. The raw LLVM
JSON was used only to locate zero-count region starts; MCP remains the source of
truth for totals. The best fixture-backed targets are public decoder boundary
states where Pillow can provide a deterministic oracle.

Selected sweep:

| Target | Reverse-mapped gap | Action |
| --- | --- | --- |
| `src/codecs/gif/decode.rs` | Missing regions cluster at header short reads, global/local color-table reads, extension payload reads, image descriptor reads, sub-block reads, GCE reads, and malformed LZW/image-data boundaries. | Add manifest-backed malformed GIF assets for truncated signature/logical screen/global palette/GCE/application/comment/image descriptor/local palette/image data/sub-block payloads. These are Pillow-error rows. |
| `src/codecs/gif/decode.rs` | Pillow accepts a NETSCAPE loop extension whose payload contains only the loop sub-block introducer byte, while Rust currently rejects it by requiring two loop-count bytes whenever the first payload byte is `1`. | Add `short_loop_payload.gif` as a valid Pillow-oracle row and change loop parsing to ignore a short loop count instead of rejecting the whole GIF. This is a parity fix, not a coverage-only bypass. |
| `src/codecs/bmp/decode.rs` | Remaining region starts include RLE delta/absolute-mode byte fetches, absolute-mode padding, and error exits below already-covered public lines. | Add manifest-backed RLE8/RLE4 malformed byte-stream assets for truncated delta and absolute modes, plus valid RLE8 delta and odd absolute-mode streams accepted by Pillow. |

Validation after this batch:

1. Regenerate deterministic fixture assets and the manifest-driven Pillow
   matrix.
2. Run `cargo fmt`.
3. Run `cargo check --all-features` and
   `RUSTFLAGS='--cfg coverage' cargo check --all-features`.
4. Run only the approved Coverage MCP command
   `all-features-llvm-cov-json-nightly-branch`.
5. Query MCP summary/files for lines, branches, functions, and regions, then
   record the snapshot and per-file movement here before continuing.

### Attempt 5 result

Coverage MCP run `ca011390-80d4-427b-8f5c-ec674a04e270`, snapshot
`c38aa6c9-97ff-438b-8bd3-4a7698deddd1`, passed and ingested.

- Lines: `23902 / 23909`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39679 / 40941`
- Missing regions: `1262`

Net from attempt 4:

- Missing regions improved from `1280` to `1262` (`18` fewer).
- Region rate improved from `96.874%` to `96.918%`.
- Branch gap stayed at `14` missing. The apparent branch movement
  (`3422 / 3436` to `3424 / 3438`) is from the new GIF short-loop branch; both
  sides are covered by existing normal-loop rows and `short_loop_payload.gif`.

Target file movement:

| File | Before | After | Missing-region delta |
| --- | ---: | ---: | ---: |
| `src/codecs/gif/decode.rs` | `573 / 633` | `586 / 630` | `60 -> 44` |
| `src/codecs/bmp/decode.rs` | `769 / 829` | `771 / 829` | `60 -> 58` |

Fixture rows and parity fix added in this sweep:

- GIF valid row: `short_loop_payload.gif`. Pillow accepts a NETSCAPE loop
  extension whose payload starts with `1` but lacks the optional two-byte loop
  count. Rust now matches Pillow by ignoring the missing loop count instead of
  rejecting the whole GIF.
- GIF Pillow-error rows:
  - `truncated_signature.gif`
  - `truncated_logical_screen.gif`
  - `truncated_global_palette.gif`
  - `truncated_application_identifier.gif`
  - `truncated_application_subblock.gif`
  - `truncated_comment_subblock.gif`
  - `truncated_image_descriptor.gif`
  - `truncated_local_palette.gif`
  - `truncated_image_data.gif`
  - `truncated_sub_block.gif`
  - `truncated_gce.gif`
- BMP valid rows:
  - `rle8_delta.bmp`
  - `rle8_absolute_odd.bmp`
- BMP Pillow-error rows:
  - `rle8_delta_truncated.bmp`
  - `rle8_absolute_truncated.bmp`
  - `rle4_delta_truncated.bmp`
  - `rle4_absolute_truncated.bmp`

Post-run raw-region map:

- `src/codecs/gif/decode.rs`: 44 zero regions remain. The public short-read
  cluster responded well to fixtures; remaining starts are mostly validated
  frame fallback arithmetic, LZW table setup/invariants, and deinterlace/input
  helper checked arithmetic.
- `src/codecs/bmp/decode.rs`: 58 zero regions remain. More RLE fixtures have
  low yield; remaining starts are mostly read-helper, dimension/header parsing,
  bitfield extraction, orientation, and palette construction regions that are
  already line/branch covered.

Next attack order from this result:

1. For region-only progress, switch back to invariant simplification in
   `src/codecs/compression/zlib_ng.rs` and `src/codecs/gif/encode.rs`. They own
   the largest remaining region deficits and public fixtures have diminishing
   returns there.
2. For fixture-backed decoder work, continue GIF only if the target line maps to
   a concrete public parsing boundary. The broad truncation batch already
   covered the obvious boundary states.
3. For branch progress, the remaining aggregate branch gaps are unchanged in
   substance:
   - `src/codecs/webp/native/decoder.rs`: `82 / 88`
   - `src/codecs/webp/native/vp8.rs`: `154 / 160`
   - `src/codecs/webp/native/lossless.rs`: `108 / 110`

### Attempt 6 plan: GIF decoder invariant simplification

Current Coverage MCP baseline before editing:

- Snapshot: `c38aa6c9-97ff-438b-8bd3-4a7698deddd1`
- Commit metadata: `2c2704c19997517155eb27af231d54cb349bd25b`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Lines: `23902 / 23909`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39679 / 40941`
- Missing regions: `1262`

Reverse-mapped target: `src/codecs/gif/decode.rs`, which now has 44 zero
regions after the public malformed fixture batch. The remaining easy wins are
not new fixtures; they are checked arithmetic regions that are impossible after
GIF field-width parsing and LZW length checks.

Planned simplifications:

| Line cluster | Invariant | Action |
| --- | --- | --- |
| Frame duration | `delay_cs` is `u16`, so centiseconds times 10 is at most `655_350`, well below `u32::MAX`. | Replace `checked_mul(10)?` with direct multiplication. |
| Logical fallback dimensions | Frame offsets and decoded GIF image dimensions originate from `u16` image-descriptor fields, so `left + width` and `top + height` fit in `u32`. | Replace `checked_add` fallback calculations with direct addition. |
| Color-table length | GIF packed color-table exponent is `(packed & 7) + 1`, always `1..=8`, so shift and `* 3` cannot overflow. | Replace checked shift/multiply with direct arithmetic. |
| LZW setup and dictionary increment | `minimum_code_size` is validated as `2..=8`, and `next_code` increments only while `< 4096`. | Replace setup `checked_*` and dictionary `checked_add(1)?` with direct arithmetic. |
| Deinterlace row copies | `decode_lzw()` returns exactly `width * height` indices before deinterlace is called; the four GIF interlace passes enumerate exactly `height` destination rows. | Replace checked row-start calculations and optional slice gets with direct slices guarded by the existing debug assertion. |
| Input byte increment | `read_u8()` only increments after `data.get(position)` succeeds, so `position + 1` cannot overflow for an in-memory slice. | Replace `checked_add(1)?` with `+= 1`. |

Validation after this batch:

1. Run `cargo fmt`.
2. Run `cargo check --all-features` and
   `RUSTFLAGS='--cfg coverage' cargo check --all-features`.
3. Run only the approved Coverage MCP command
   `all-features-llvm-cov-json-nightly-branch`.
4. Record the new summary and GIF decode movement here.

### Attempt 6 result

Coverage MCP run `ed98b88c-51f2-4c11-83fa-1dc351bf1660`, snapshot
`7a7d1b54-9113-4f74-bc32-384a7453f78a`, passed and ingested.

- Lines: `23902 / 23909`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39655 / 40905`
- Missing regions: `1250`

Net from attempt 5:

- Missing regions improved from `1262` to `1250` (`12` fewer).
- Region rate improved from `96.918%` to `96.944%`.
- Lines, branches, and functions were unchanged.

Target file movement:

| File | Before | After | Missing-region delta |
| --- | ---: | ---: | ---: |
| `src/codecs/gif/decode.rs` | `586 / 630` | `562 / 594` | `44 -> 32` |

The denominator reduction is expected: this pass removed checked-arithmetic and
optional-slice regions that were proven unreachable after GIF field-width
parsing and LZW output-length validation. No new fixtures were needed, and the
MCP run confirmed the manifest-driven parity tests still pass.

Next attack order from this result:

1. Continue region-first work in `src/codecs/compression/zlib_ng.rs` and
   `src/codecs/gif/encode.rs`; they remain the largest region deficits.
2. Keep fixture-backed decoder work for concrete public parsing boundaries only.
3. Return to branch progress with WebP native generators when region cleanup
   stops yielding clean invariant removals.

### Attempt 7 plan: zlib dynamic-block invariant simplification

Current Coverage MCP baseline before editing:

- Snapshot: `7a7d1b54-9113-4f74-bc32-384a7453f78a`
- Commit metadata: `2c2704c19997517155eb27af231d54cb349bd25b`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Lines: `23902 / 23909`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39655 / 40905`
- Missing regions: `1250`

Reverse-mapped target: `src/codecs/compression/zlib_ng.rs`, the largest
remaining region file at `2783 / 3100` regions. Its branch and function
coverage are already 100%, so this pass must remove only provably impossible
checked-arithmetic regions rather than adding hook volume.

Selected sub-batch:

| Line cluster | Invariant | Action |
| --- | --- | --- |
| `build_tree()` heap allocation | `TreeSpec::elements` is one of the fixed DEFLATE table sizes: 286 literals, 30 distances, or 19 bit-length codes. `elements * 2 + 1` cannot overflow. | Replace checked heap-size arithmetic with direct arithmetic. |
| `generate_codes()` canonical counters | `bit_counts` is produced by the bounded Huffman builder with maximum code length 15; the canonical next-code sequence fits in `u16`, and shifting left by one is always a valid one-bit shift. | Replace `checked_add`/`checked_shl` on canonical counters with direct arithmetic. |
| `send_trees()` header fields | Dynamic blocks are emitted only after `frequencies()` inserts EOB literal 256 and `max_bit_length_index` is selected from `3..BIT_LENGTH_CODES`. Therefore HLIT and HCLEN header deltas cannot underflow. | Replace checked header delta arithmetic with direct subtraction. |
| `emit_tokens()` / `emit_fixed_block()` extra bits | `length_index(length)` and `distance_index(distance)` are selected by searching base tables from high to low, so `length >= LENGTH_BASE[index]` and `distance >= DISTANCE_BASE[index]` by construction. | Replace checked extra-bit deltas with direct subtraction. |
| `scan_tree()` frequency counts | The tree scan iterates at most 286 symbols, and repeat-code frequency counters are `u32`; these increments cannot overflow. | Replace checked counter increments with direct additions. |

Explicitly deferred:

- Matchers (`process`, `longest_match`, `insert_match`) still have many
  checked regions, but those are deeper algorithmic paths. Do not rewrite them
  in this pass without a separate proof for each state machine.
- `send_tree()` repeat-code count subtraction is not included here; the zlib
  repeat-code state machine needs separate reverse mapping before removing
  checked subtraction there.

Validation after this batch:

1. Run `cargo fmt`.
2. Run `cargo check --all-features` and
   `RUSTFLAGS='--cfg coverage' cargo check --all-features`.
3. Run only the approved Coverage MCP command
   `all-features-llvm-cov-json-nightly-branch`.
4. Record the new summary and zlib movement here.

### Attempt 13 plan: small source-mapped region cleanup

Current Coverage MCP baseline before editing:

- Snapshot: `6b3e36fc-3725-4ce7-b124-e00e115c097f`
- Commit metadata: `09967bc37baa6b6aa4afde4b1c96a452016b0d7d`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Lines: `23943 / 23949`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39535 / 40697`
- Missing regions: `1162`

MCP is the source of truth for aggregate line, branch, function, and region
totals. The MCP file view does not expose exact region-start records for
region-only gaps, so the MCP-produced LLVM JSON artifact was used only to map
zero-count region entries back to source starts.

Selected sub-batch:

| File | Source-mapped regions | Reverse-mapped invariant/input | Action |
| --- | ---: | --- | --- |
| `src/types/buffer.rs` | 4 | Empty double-ended row iterators are real iterator states; the checked pixel accessors also have real overflow-protected arithmetic states when malformed buffers carry huge dimensions. | Extend the existing coverage hook with empty `Rows`/`RowsMut::next_back()` calls and huge-dimension `get_pixel_checked` / `get_pixel_mut_checked` calls. |
| `src/types/mod.rs` | 7 | `u32` width/height always fit in `usize` on supported 32-bit and 64-bit Rust targets, so the `try_from(u32)` failure side is impossible. Overflow and frame-validation failures remain real validation states. | Replace `usize::try_from(u32)` with direct casts; add coverage-hook inputs for oversized RGB byte count, invalid frame image validation, and right/bottom frame offset overflow. |
| `src/codecs/jpeg/encode/marker.rs` | 1 | `exif.len().checked_add(2)` overflow is not constructible from a real slice allocation; the real JPEG APP1 boundary is still the `u16` segment-length conversion, already exercised with oversized EXIF. | Replace the impossible checked add with direct `exif.len() + 2`, keeping the `u16::try_from` failure path. |

Explicitly deferred:

- `src/codecs/webp/decode.rs` has six wrapper `?` regions around native WebP
  decode calls. Reverse mapping shows those require real VP8/VP8L frame
  generator states or successful animated frame composition, so keep them with
  the WebP branch/generator work instead of adding fake wrapper probes.

Validation after this batch:

1. Run `cargo fmt`.
2. Run `cargo check --all-features` and
   `RUSTFLAGS='--cfg coverage' cargo check --all-features`.
3. Run only the approved Coverage MCP command
   `all-features-llvm-cov-json-nightly-branch`.
4. Record the new summary and per-file region movement here.

Refinement before recording the result:

- The first run left one source-mapped `src/types/mod.rs` region at
  `width.checked_mul(height)?`. On 64-bit targets the product of two `u32`
  dimensions is at most `(2^32 - 1)^2`, which is less than `usize::MAX`.
  On narrower targets the overflow check remains required. Split this with
  `target_pointer_width = "64"` so the current supported coverage target uses
  direct multiplication while non-64-bit builds retain `checked_mul`.

### Attempt 13 result

Coverage MCP run `5e2898b1-1e2a-4be1-83b2-32a53732ef36`, snapshot
`1378d124-9841-40b1-9168-f22ca90f3797`, passed and ingested.

- Commit metadata: `e23eadda0c7d6f28b9642ff9c2d3c78ddd6def1d`
- Lines: `24013 / 24019`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39582 / 40734`
- Missing regions: `1152`

Net from attempt 12:

- Missing regions improved from `1162` to `1152` (`10` fewer).
- Region rate improved from `97.145%` to `97.172%`.
- Branches and functions were unchanged.

Target file movement:

| File | Before | After | Missing-region delta |
| --- | ---: | ---: | ---: |
| `src/types/buffer.rs` | `638 / 642` | `664 / 666` | `4 -> 2` |
| `src/types/mod.rs` | `252 / 259` | `275 / 275` | `7 -> 0` |
| `src/codecs/jpeg/encode/marker.rs` | `166 / 167` | `164 / 164` | `1 -> 0` |

What moved:

- Exercised empty double-ended row iterators and huge malformed checked-pixel
  accessor states in `src/types/buffer.rs`.
- Removed impossible `u32` to `usize` conversion failures and split
  `expected_bytes()` pixel-count multiplication by target width: 64-bit builds
  use direct multiplication, non-64-bit builds retain the overflow guard.
- Exercised real decoded image/sequence validation failures for oversized byte
  counts, invalid frame images, and right/bottom frame offset overflow.
- Removed the unconstructible EXIF slice-length `checked_add(2)` region while
  preserving the real JPEG APP1 `u16` segment-length failure.

Remaining from this batch:

- `src/types/buffer.rs` still reports two aggregate missing regions, but the
  MCP-produced LLVM JSON artifact has no zero-count source-region entries for
  that file. Treat this as an aggregate/source-map artifact until a future
  llvm-cov report exposes an actionable source start.
- `src/codecs/webp/decode.rs` remains deferred with six wrapper `?` regions
  that need real VP8/VP8L frame generator states.

### Attempt 14 plan: JPEG baseline table-validation invariant

Current Coverage MCP baseline before editing:

- Snapshot: `1378d124-9841-40b1-9168-f22ca90f3797`
- Commit metadata: `e23eadda0c7d6f28b9642ff9c2d3c78ddd6def1d`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Lines: `24013 / 24019`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39582 / 40734`
- Missing regions: `1152`

Selected target: `src/codecs/jpeg/decode/decode.rs`, currently
`537 / 540` regions with three zero-count region starts:

- line 135: `info.dc_huff_tables[scan_comp.dc_tbl as usize].as_ref()?`
- line 136: `info.ac_huff_tables[scan_comp.ac_tbl as usize].as_ref()?`
- line 137: `info.quant_tables[comp.quant_tbl as usize].as_ref()?`

Reverse mapping:

- `decode()` validates every component quantization table before dispatching
  baseline reconstruction.
- `decode()` validates every baseline scan component's DC and AC Huffman table
  before calling `reconstruct_image()`.
- Existing malformed JPEG fixtures already cover the public sparse quant/DC/AC
  table rejection paths at the validation boundary.

Action:

- Replace the three loop-internal `as_ref()?` fallbacks with invariant
  `expect(...)` lookups tied to the earlier validation. This removes dead
  internal Option-return regions without changing public malformed JPEG
  behavior.

Validation after this batch:

1. Run `cargo fmt`.
2. Run `cargo check --all-features` and
   `RUSTFLAGS='--cfg coverage' cargo check --all-features`.
3. Run only the approved Coverage MCP command
   `all-features-llvm-cov-json-nightly-branch`.
4. Record the new summary and JPEG baseline movement here.

### Attempt 14 result

Coverage MCP run `9675a5e8-a552-4059-8f60-f224ee1ead4c`, snapshot
`5f3d481f-98ce-492c-ae0b-2412eb82c96b`, passed and ingested.

- Commit metadata: `e23eadda0c7d6f28b9642ff9c2d3c78ddd6def1d`
- Lines: `24019 / 24025`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39588 / 40737`
- Missing regions: `1149`

Net from attempt 13:

- Missing regions improved from `1152` to `1149` (`3` fewer).
- Region rate improved from `97.172%` to `97.179%`.
- Branches and functions were unchanged.

Target file movement:

| File | Before | After | Missing-region delta |
| --- | ---: | ---: | ---: |
| `src/codecs/jpeg/decode/decode.rs` | `537 / 540` | `543 / 543` | `3 -> 0` |

What moved:

- Replaced loop-internal baseline DC Huffman, AC Huffman, and quantization table
  `as_ref()?` fallbacks with invariant `expect(...)` lookups after public
  `decode()` validation.
- Public malformed sparse-table behavior remains covered at the validation
  boundary by the existing fixtures; this only removes dead internal Option
  regions after validation has already succeeded.

### Attempt 15 plan: WebP extended helper read-short states

Current Coverage MCP baseline before editing:

- Snapshot: `5f3d481f-98ce-492c-ae0b-2412eb82c96b`
- Commit metadata: `e23eadda0c7d6f28b9642ff9c2d3c78ddd6def1d`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Lines: `24019 / 24025`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39588 / 40737`
- Missing regions: `1149`

Selected target: `src/codecs/webp/native/extended.rs`, currently
`336 / 344` regions with eight zero-count region starts:

- line 258: initial VP8X flags byte short-read.
- line 266: VP8X reserved bytes short-read.
- lines 268 and 269: VP8X canvas width/height short-reads.
- line 292: `read_3_bytes()` direct short-read.
- line 347: lossless-compressed ALPH payload decode failure.
- line 356: uncompressed ALPH payload short-read.

Reverse mapping:

- These are private container/ALPH helper boundaries. Public WebP fixtures
  would have to manufacture partial RIFF chunk payloads only to reach identical
  `Read` failures, which is less direct and not tied to Pillow byte parity.
- `read_alpha_chunk()` has real invalid states for lossless-compressed alpha
  with no VP8L payload and for uncompressed alpha whose payload is shorter than
  `width * height`.

Action:

- Extend the existing `#[cfg(coverage)]` hook in `extended.rs` with direct
  `Cursor` inputs for each short-read and invalid-alpha state. Keep the hook
  branch-light and do not add public manifest rows for private read plumbing.

Validation after this batch:

1. Run `cargo fmt`.
2. Run `cargo check --all-features` and
   `RUSTFLAGS='--cfg coverage' cargo check --all-features`.
3. Run only the approved Coverage MCP command
   `all-features-llvm-cov-json-nightly-branch`.
4. Record the new summary and WebP extended movement here.

Refinement before recording the result:

- The first run reduced only one net region and left MCP partial-branch lines at
  the VP8X canvas-size overflow check and the ALPH compression split. Add a
  full 10-byte VP8X header with both dimensions at `0x00ff_ffff + 1` to cover
  `ImageTooLarge`, and add an uncompressed one-byte ALPH payload to cover the
  non-lossless success path. Do not attempt lossless-alpha success without a
  valid VP8L alpha bitstream.

### Attempt 15 result

Coverage MCP run `466ee05a-fd87-41a4-843c-5e4798dfaf89`, snapshot
`56b7bb5c-2060-4743-a851-47944168c034`, passed and ingested.

- Commit metadata: `e23eadda0c7d6f28b9642ff9c2d3c78ddd6def1d`
- Lines: `24030 / 24036`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39607 / 40755`
- Missing regions: `1148`

Net from attempt 14:

- Missing regions improved from `1149` to `1148` (`1` fewer).
- Region rate improved from `97.179%` to `97.183%`.
- Branches and functions were unchanged.

Target file movement:

| File | Before | After | Missing-region delta |
| --- | ---: | ---: | ---: |
| `src/codecs/webp/native/extended.rs` | `336 / 344` | `355 / 362` | `8 -> 7` |

What moved:

- Added direct private read-short inputs for VP8X flags/reserved/canvas reads,
  direct `read_3_bytes()` short-read, invalid lossless-compressed ALPH payload,
  uncompressed ALPH short-read, VP8X canvas-size overflow, and uncompressed
  ALPH success.

Remaining in this file:

- The refined run covered more helper paths but added the same amount of new
  hook-region accounting, so net movement stayed at one fewer missing region.
- The remaining seven aggregate regions have no zero-count source-region starts
  in the MCP-produced LLVM JSON artifact. MCP's file view points only at
  partial-branch expression lines around the canvas-size check and ALPH
  compression split. Stop adding hook volume here until a valid VP8L alpha
  bitstream generator or a clearer source-region map exists.

### Attempt 16 plan: DynamicImage malformed decoded conversions

Current Coverage MCP baseline before editing:

- Snapshot: `56b7bb5c-2060-4743-a851-47944168c034`
- Commit metadata: `e23eadda0c7d6f28b9642ff9c2d3c78ddd6def1d`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Lines: `24030 / 24036`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39607 / 40755`
- Missing regions: `1148`

Selected target: `src/types/dynamic.rs`, currently `1417 / 1428` regions.
MCP's file view has no grouped gaps, but the MCP-produced LLVM JSON artifact
maps eight zero-count source-region starts to `DynamicImage::from_decoded()`:

- L8, Rgba8, L16, La16, Rgb16, Rgba16, Rgb32F, and Rgba32F `from_raw(...)?`
  conversion failures.

Reverse mapping:

- `DynamicImage::from_decoded()` does not call `DecodedImage::validate()`.
  It intentionally accepts a borrowed decoded image and returns `None` when
  the buffer cannot instantiate the requested concrete image buffer.
- These are real malformed decoded-image states at a public conversion boundary,
  not post-validation invariants.

Action:

- Extend the existing `#[cfg(coverage)]` hook with malformed one-pixel decoded
  images whose pixel buffers are empty for each source-mapped mode. This covers
  the concrete `ImageBuffer::from_raw(...)?` failure paths without adding codec
  fixtures unrelated to Pillow byte parity.

Validation after this batch:

1. Run `cargo fmt`.
2. Run `cargo check --all-features` and
   `RUSTFLAGS='--cfg coverage' cargo check --all-features`.
3. Run only the approved Coverage MCP command
   `all-features-llvm-cov-json-nightly-branch`.
4. Record the new summary and dynamic movement here.

### Attempt 16 result

Coverage MCP run `ed37072c-54a9-4fee-bc69-46f8446d1712`, snapshot
`f3b30c5d-501b-414d-9aa4-b43e69e7c9b5`, passed and ingested.

- Commit metadata: `e23eadda0c7d6f28b9642ff9c2d3c78ddd6def1d`
- Lines: `24043 / 24049`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39628 / 40768`
- Missing regions: `1140`

Net from attempt 15:

- Missing regions improved from `1148` to `1140` (`8` fewer).
- Region rate improved from `97.183%` to `97.204%`.
- Branches and functions were unchanged.

Target file movement:

| File | Before | After | Missing-region delta |
| --- | ---: | ---: | ---: |
| `src/types/dynamic.rs` | `1417 / 1428` | `1438 / 1441` | `11 -> 3` |

What moved:

- Added malformed decoded-image inputs for L8, Rgba8, L16, La16, Rgb16,
  Rgba16, Rgb32F, and Rgba32F conversion failures in
  `DynamicImage::from_decoded()`.
- This keeps the failure behavior at the public conversion boundary and avoids
  unrelated codec fixtures.

Remaining in this file:

- `src/types/dynamic.rs` still reports three aggregate missing regions, but
  after this pass the MCP-produced LLVM JSON artifact has no zero-count
  source-region entries for the file. Treat the remaining count as
  aggregate/source-map artifact until a later report exposes an actionable
  source start.

### Attempt 17 plan: WebP encode metadata and wrapper invariants

Current Coverage MCP baseline before editing:

- Snapshot: `f3b30c5d-501b-414d-9aa4-b43e69e7c9b5`
- Commit metadata: `e23eadda0c7d6f28b9642ff9c2d3c78ddd6def1d`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Lines: `24043 / 24049`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39628 / 40768`
- Missing regions: `1140`

Selected target: `src/codecs/webp/encode/mod.rs`, currently `327 / 339`
regions. Source-mapped zero-count region starts are:

- invalid `icc_hex` / `exif_hex` option decoding at metadata attachment.
- private RIFF chunk-name/length extraction and checked arithmetic while
  rewrapping internally generated WebP bytes with metadata.
- `riff_size` checked subtraction after writing a new RIFF header.
- lossless encoder failure propagation.
- lossy RGBA alpha encoder `io::Result` propagation.

Reverse mapping:

- Invalid metadata hex values are real public encode option states and should
  return `None`.
- `attach_metadata()` only reparses bytes produced by the internal WebP
  encoders in the same `encode()` call. Those chunk headers are internally
  bounded: `offset + 8 <= encoded.len()` proves the name/length slices, and
  generated chunk lengths are inside the encoded buffer.
- The new metadata output always starts with `RIFF` + length placeholder +
  `WEBP`, so `output.len() - 8` cannot underflow. The `u32` conversion remains
  because RIFF length is a real format boundary.
- `encode_alpha()` writes to an in-memory `Vec`, so its `io::Result` cannot
  fail once the alpha data length invariant has passed. A malformed alpha
  length still panics before returning `Err`; this pass does not change that.
- Lossless zero dimensions are real public malformed image states and return
  `EncodingError::InvalidDimensions`.

Action:

- Extend the WebP encode coverage hook with invalid `icc_hex` / `exif_hex`
  options and a zero-width lossless image.
- Replace private metadata parser `try_into().ok()?`, checked chunk arithmetic,
  and RIFF subtraction with direct indexing/arithmetic under the internal
  generated-RIFF invariant.
- Replace lossy alpha `.ok()?` with an invariant `expect(...)` for the
  Vec-backed alpha encoder.

Validation after this batch:

1. Run `cargo fmt`.
2. Run `cargo check --all-features` and
   `RUSTFLAGS='--cfg coverage' cargo check --all-features`.
3. Run only the approved Coverage MCP command
   `all-features-llvm-cov-json-nightly-branch`.
4. Record the new summary and WebP encode movement here.

### Attempt 17 result

Coverage MCP run `6151cae5-ebd1-4661-a20d-bf2a1338baeb`, snapshot
`ba5a5ef7-9f4a-4463-9d40-148ca7ce5c65`, passed and ingested.

- Commit metadata: `e23eadda0c7d6f28b9642ff9c2d3c78ddd6def1d`
- Lines: `24066 / 24072`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39647 / 40775`
- Missing regions: `1128`

Net from attempt 16:

- Missing regions improved from `1140` to `1128` (`12` fewer).
- Region rate improved from `97.204%` to `97.234%`.
- Branches and functions were unchanged.

Target file movement:

| File | Before | After | Missing-region delta |
| --- | ---: | ---: | ---: |
| `src/codecs/webp/encode/mod.rs` | `327 / 339` | `345 / 346` | `12 -> 1` |

What moved:

- Exercised invalid public `icc_hex` and `exif_hex` WebP encode options.
- Exercised zero-width lossless WebP encode as a real invalid-dimensions state.
- Removed dead private metadata reparse checks that only handled malformed
  internally-generated RIFF chunk headers.
- Replaced the Vec-backed lossy alpha encoder `io::Result` propagation with an
  invariant `expect(...)` after alpha-length validation.

Remaining in this file:

- `src/codecs/webp/encode/mod.rs` is line-, branch-, and function-complete.
  One aggregate region-only gap remains, but MCP exposes no source-line gap for
  it. Treat it as a region/source-map artifact until a later LLVM report gives
  an actionable source start.

### Attempt 18 plan: BMP decode public truncation fixtures and private invariants

Current Coverage MCP baseline before editing:

- Snapshot: `ba5a5ef7-9f4a-4463-9d40-148ca7ce5c65`
- Commit metadata: `e23eadda0c7d6f28b9642ff9c2d3c78ddd6def1d`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Lines: `24066 / 24072`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39647 / 40775`
- Missing regions: `1128`

Selected target: `src/codecs/bmp/decode.rs`, currently `771 / 829` regions
with zero line/branch/function gaps.

Source-mapped zero-count starts split into two groups:

| Group | Source starts | Reverse mapping | Action |
| --- | ---: | --- | --- |
| Public short reads | BMP file header, OS/2 core header, BITMAPINFOHEADER fields, and BI_BITFIELDS mask reads. | These are real malformed BMP byte prefixes. The decoder should reject them the same way Pillow does. | Add deterministic malformed BMP assets in `scripts/generate_test_assets.py`, list them in `manifest.yaml`, and regenerate the BMP oracle matrix with Pillow. |
| Private RLE arithmetic | RLE output length, cursor position, repeat/delta/absolute counters, and absolute-mode pad accounting. | Public `decode()` validates BMP dimensions to at most `16_384 x 16_384`; RLE count/delta bytes are `u8`; byte-count increments are bounded by the input stream. The remaining checked overflows are not constructible from public BMP inputs. | Replace checked arithmetic with direct arithmetic/slicing while preserving `data.get(...)` EOF rejection. |
| Cursor operations | `Cursor::seek` / `read_to_end` on in-memory BMP bytes. | `Cursor` seeks used here are deterministic position changes. Seeking beyond the byte slice is allowed and later reads return EOF, matching the current behavior. `read_to_end` on `Cursor<&[u8]>` does not fail. | Use `set_position()` and direct remaining-slice extraction. |
| Validated conversions | `u32::try_from(width)` after `width > 0`, `usize::try_from(data_offset)` for a `u32`, 32-bit bitfield pixel slices inside an allocated scanline buffer, and palette construction from RGB triples. | These are already proven by prior validation/allocation. The real malformed cases remain at file parsing and pixel read boundaries. | Use direct casts/indexing and invariant `expect(...)` for the palette constructor. |

Fixture assets to add:

- `truncated_magic.bmp`
- `truncated_file_size.bmp`
- `truncated_data_offset.bmp`
- `truncated_dib_header_size.bmp`
- `core_header_truncated_width.bmp`
- `core_header_truncated_height.bmp`
- `core_header_truncated_planes.bmp`
- `core_header_truncated_depth.bmp`
- `info_header_truncated_height.bmp`
- `info_header_truncated_planes.bmp`
- `info_header_truncated_depth.bmp`
- `info_header_truncated_compression.bmp`
- `info_header_truncated_image_size.bmp`
- `info_header_truncated_x_pels.bmp`
- `info_header_truncated_y_pels.bmp`
- `info_header_truncated_colors_used.bmp`
- `info_header_truncated_colors_important.bmp`
- `bitfields_truncated_masks.bmp`
- `v4_bitfields_truncated_masks.bmp`

Validation after this batch:

1. Regenerate BMP fixtures and Pillow oracle rows.
2. Run `cargo fmt`.
3. Run `cargo check --all-features` and
   `RUSTFLAGS='--cfg coverage' cargo check --all-features`.
4. Run only the approved Coverage MCP command
   `all-features-llvm-cov-json-nightly-branch`.
5. Record the new summary and BMP movement here.

Refinement after the first Attempt 18 measurement:

- Coverage MCP run `56e5ce9c-af58-4062-9931-bee541e94c4d`, snapshot
  `99da374e-31a0-4b14-9b08-446367261426`, passed and ingested.
- Overall missing regions improved from `1128` to `1078`.
- `src/codecs/bmp/decode.rs` moved from `771 / 829` to `751 / 759`, so BMP
  missing regions dropped from `58` to `8`.

Remaining BMP source starts:

- RLE stream reads at count, value, and delta-right fields.
- BITMAPINFO `bit_depth` short read.
- BI_BITFIELDS green/blue mask short reads for V3 and V4+ headers.

Refinement action before recording final Attempt 18 result:

- Fix the BMP fixture generator's BITMAPINFO truncation sequence so
  `info_header_truncated_depth.bmp` actually truncates before the 16-bit
  bit-depth field rather than carrying two extra bytes from an incorrectly
  encoded planes field.
- Add public malformed BMP fixtures for empty RLE stream, one-byte RLE pair,
  delta opcode without payload, and one-/two-mask BI_BITFIELDS headers in both
  V3 and V4+ layouts.
- Regenerate BMP assets and Pillow oracle rows, then rerun the same Coverage
  MCP command.

### Attempt 18 result

Final Coverage MCP run `98666230-106e-4fe4-8967-491b8b6af1f7`, snapshot
`41e480a1-67fd-4289-9c4a-5f02d7968531`, passed and ingested.

- Commit metadata: `7f1e33a55ecbe00c439f62ddc3040d18b559f309`
- Lines: `24072 / 24078`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39635 / 40705`
- Missing regions: `1070`

Net from attempt 17:

- Missing regions improved from `1128` to `1070` (`58` fewer).
- Region rate improved from `97.234%` to `97.371%`.
- Branches and functions were unchanged.

Target file movement:

| File | Before | After | Missing-region delta |
| --- | ---: | ---: | ---: |
| `src/codecs/bmp/decode.rs` | `771 / 829` | `759 / 759` | `58 -> 0` |

What moved:

- Added 26 public malformed BMP assets under the existing manifest-driven
  `error_malformed` case. These cover short BMP file-header reads, OS/2 core
  header truncations, BITMAPINFO field truncations, RLE stream short reads, and
  truncated BI_BITFIELDS masks.
- Regenerated the authoritative BMP Pillow oracle matrix. The matrix now has
  `873` total rows; BMP decode has `88` rows and `49` expected-error rows.
- Simplified BMP decoder private invariants where public validation already
  bounds dimensions, RLE counters, `Cursor` positioning, 32-bit bitfield pixel
  slices, and palette construction.

Remaining from this batch:

- `src/codecs/bmp/decode.rs` is complete for lines, branches, functions, and
  regions. No BMP decode region debt remains.

### Attempt 9 plan: GIF encode validated-invariant region sweep

Current Coverage MCP baseline before editing:

- Snapshot: `8166d21c-ed6f-441f-b24c-95b315363185`
- Previous run: `2be535da-cc20-47ab-a190-db9de4405713`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Lines: `23904 / 23910`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39616 / 40837`
- Missing regions: `1221`

Important baseline caveat:

- This snapshot was produced from the dirty worktree that was later committed as
  `09967bc`, but MCP retained the previous commit metadata
  `2c2704c19997517155eb27af231d54cb349bd25b`.
- The next MCP run must be treated as the fresh HEAD-aligned snapshot for this
  batch.

Reverse-mapped target: `src/codecs/gif/encode.rs`

- Current file movement target from MCP: `2473 / 2585` regions.
- Missing region entries cluster around already-executed fallible `Option`
  checks in `coalesce_identical_frames()`, `clear_frame_rect()`,
  `composite_frame()`, `prepare_image()`, quantizer accounting, and
  FASTOCTREE setup.
- Public GIF encode fixture coverage is already broad: static, animated,
  looped animation, disposal modes, global/local color tables, interlace,
  transparency override, RGB/RGBA/L sources, high-color quantization, and
  Pillow error cases.
- The remaining holes in this selected batch are not ordinary Pillow-oracle
  input gaps; they are defensive continuation/failure regions after
  `DecodedSequence::validate()` and `DecodedImage::validate()` have already
  proven dimensions, frame bounds, pixel lengths, and palette shape.

Selected sub-batch:

| Line cluster | Reverse-mapped invariant | Action |
| --- | --- | --- |
| `coalesce_identical_frames()` crop geometry | `encode_sequence()` validates the sequence before coalescing. The current and previous renders are full-canvas buffers sized from the checked canvas allocation. `rgba_difference_bounds()` is only used when the render differs, so `right >= left`, `bottom >= top`, and the crop lies inside the prepared full-canvas index buffer. | Reuse the already-converted `height`, replace checked crop subtraction/arithmetic/slices/conversions with direct operations. Keep duration addition checked because two valid frames can still overflow `u32`. |
| `clear_frame_rect()` | Frame rectangles are sequence-validated and bounded by the canvas before this private helper is called from production encode. | Replace checked coordinate conversion and row arithmetic with direct operations/slices. |
| `composite_frame()` | Validated frames prove per-mode pixel lengths and canvas bounds. The private coverage hook deliberately exercises an invalid P8 palette index, so palette range lookup must stay fallible. | Replace checked source/destination arithmetic and per-mode byte offsets with direct arithmetic while preserving fallible palette/pixel slices needed by the hook. |
| `prepare_image()` L8 palette remap | The loop enumerates a fixed `[bool; 256]`, and a compact grayscale palette can contain at most 256 entries. | Replace unreachable `u8::try_from` conversions for palette index and grayscale value with direct casts. |
| `indexed_rgb()` palette offset | GIF indices are bytes; multiplying an index by 3 is bounded by `255 * 3`. The palette slice lookup remains fallible for the existing private invalid-palette hook. | Replace checked offset multiplication with direct multiplication. |
| Public fixture gaps | Potential real oracle rows still exist for explicit `animated:false` on animated input and additional loop spelling variants. These are valid parity rows, but they do not map to the current region-only defensive gaps. | Defer to a later oracle-generation pass so this pass can verify the invariant cleanup with one MCP run. |

Explicitly deferred:

- GIF writer `u16::try_from` checks for canvas/frame dimensions and offsets stay
  in place; GIF file fields are 16-bit and the existing coverage hook verifies
  the oversized failure path.
- Quantizer count accumulation and median-cut split accounting stay checked
  until there is a separate input-size/count proof. These regions are
  proportional to source pixel count, not just local geometry.
- FASTOCTREE cube construction stays checked because the coverage hook
  intentionally exercises invalid cube bit widths and oversized cube sizes.

Validation after this batch:

1. Run `cargo fmt`.
2. Run `cargo check --all-features` and
   `RUSTFLAGS='--cfg coverage' cargo check --all-features`.
3. Run only the approved Coverage MCP command
   `all-features-llvm-cov-json-nightly-branch`.
4. Record the new summary and GIF encode movement here.

### Attempt 9 result

Coverage MCP run `2a889cde-9b51-4c0c-aed6-53bfba73c81e`, snapshot
`13b4a60c-878d-4e28-b565-44f02412a45d`, passed and ingested.

- Commit metadata: `09967bc37baa6b6aa4afde4b1c96a452016b0d7d`
- Lines: `23901 / 23907`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39532 / 40706`
- Missing regions: `1174`

Net from attempt 8:

- Missing regions improved from `1221` to `1174` (`47` fewer).
- Region rate improved from `97.010%` to `97.116%`.
- Branches and functions were unchanged.
- Line totals changed from `23904 / 23910` to `23901 / 23907` because the GIF
  encode invariant cleanup removed counted defensive lines; the line rate stayed
  effectively unchanged and the new snapshot is aligned to current HEAD.

Target file movement:

| File | Before | After | Missing-region delta |
| --- | ---: | ---: | ---: |
| `src/codecs/gif/encode.rs` | `2473 / 2585` | `2389 / 2454` | `112 -> 65` |

What moved:

- Removed region-only defensive checks from GIF crop geometry and validated
  frame compositing.
- Preserved real error boundaries: GIF 16-bit dimensions/offsets, invalid
  palette lookup in the private hook, duration overflow, quantizer count
  arithmetic, and FASTOCTREE construction.

Next attack order from this result:

1. Continue region work on files with high missing-region count:
   `src/codecs/compression/zlib_ng.rs`, `src/codecs/webp/native/encoder.rs`,
   `src/codecs/jpeg/encode/mod.rs`, `src/codecs/tiff/decode.rs`,
   `src/codecs/bmp/decode.rs`, `src/codecs/ico/encode.rs`, and
   `src/codecs/png/decode.rs`.
2. If staying in GIF encode, the remaining `65` regions are mostly quantizer
   count/median-cut/FASTOCTREE accounting and should not be simplified without
   a separate pixel-count proof.
3. Branch work remains unchanged and should start with WebP native:
   `src/codecs/webp/native/decoder.rs`, `src/codecs/webp/native/vp8.rs`, and
   `src/codecs/webp/native/lossless.rs`.

### Attempt 10 plan: JPEG encode Huffman bounded-table cleanup

Current Coverage MCP baseline before editing:

- Snapshot: `13b4a60c-878d-4e28-b565-44f02412a45d`
- Previous run: `2a889cde-9b51-4c0c-aed6-53bfba73c81e`
- Commit metadata: `09967bc37baa6b6aa4afde4b1c96a452016b0d7d`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Lines: `23901 / 23907`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39532 / 40706`
- Missing regions: `1174`

Reverse-mapped target: `src/codecs/jpeg/encode/huffman.rs`

- Current file movement target from MCP: `247 / 268` regions.
- Branches and lines are already complete in this file.
- Missing regions map to fallible arithmetic in `optimal_table()`, the
  libjpeg-derived optimal Huffman table builder.

Selected sub-batch:

| Line cluster | Reverse-mapped invariant | Action |
| --- | --- | --- |
| `code_size[...] += 1` in tree merge | `code_size` contains one entry per non-zero source symbol plus the sentinel, so at most `257` entries. Each increment is bounded by the number of merges and cannot overflow `usize`. | Replace checked `usize` increments with direct additions. |
| `positions` accumulation | `length_counts` counts at most the `257` working symbols; prefix positions are bounded by the number of emitted JPEG symbols. | Replace checked position addition with direct addition. |
| `count - 1` output loop | The sentinel at source symbol `256` is always inserted with non-zero frequency, so `count >= 1`. The output loop intentionally excludes the sentinel. | Replace checked subtraction with direct `count - 1`. |
| `values[target]` / symbol cast / position increment | `positions` are derived from the same `length_counts` used to allocate `values`; the loop excludes sentinel symbol `256`, so every emitted symbol is `0..=255`. | Replace optional vector write, `u8::try_from`, and checked position increment with direct operations. |

Explicitly deferred:

- `working[first].checked_add(working[second])` remains checked because public
  optimized encoding frequency totals are proportional to image size.
- `length_counts.get_mut(length)` and length-limiting arithmetic remain checked
  until a separate proof confirms libjpeg's maximum pre-limiting code depth for
  every possible 257-symbol frequency table.
- `u8::try_from(value)` for final `BITS` counts remains checked because JPEG DHT
  count bytes are an actual format boundary.

Validation after this batch:

1. Run `cargo fmt`.
2. Run `cargo check --all-features` and
   `RUSTFLAGS='--cfg coverage' cargo check --all-features`.
3. Run only the approved Coverage MCP command
   `all-features-llvm-cov-json-nightly-branch`.
4. Record the new summary and JPEG Huffman movement here.

### Attempt 10 result

Coverage MCP run `4ec2211a-9e64-4a29-a2b2-7beccf6dd62a`, snapshot
`cc32bd8a-5142-4efd-b171-72f448b09c7f`, passed and ingested.

- Commit metadata: `09967bc37baa6b6aa4afde4b1c96a452016b0d7d`
- Lines: `23905 / 23911`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39515 / 40680`
- Missing regions: `1165`

Net from attempt 9:

- Missing regions improved from `1174` to `1165` (`9` fewer).
- Region rate improved from `97.116%` to `97.136%`.
- Branches and functions were unchanged.

Target file movement:

| File | Before | After | Missing-region delta |
| --- | ---: | ---: | ---: |
| `src/codecs/jpeg/encode/huffman.rs` | `247 / 268` | `230 / 242` | `21 -> 12` |

What moved:

- Removed region-only checked arithmetic around bounded `code_size` increments,
  prefix position accumulation, sentinel-excluding output loop, and output table
  writes.
- Preserved image-size-dependent frequency addition, length-count table bounds,
  length-limiting arithmetic, and JPEG DHT byte-count conversion.

Next attack order from this result:

1. Continue region sweep with either:
   - `src/codecs/webp/native/huffman.rs` (`330 / 331`) if the goal is to clear
     the smallest one-region file first, or
   - `src/codecs/webp/decode.rs`, `src/codecs/mod.rs`, and `src/types/mod.rs`
     for small bounded wrapper gaps.
2. For higher impact, target `src/codecs/webp/native/encoder.rs`,
   `src/codecs/tiff/decode.rs`, `src/codecs/bmp/decode.rs`, or
   `src/codecs/png/decode.rs`, but those need more reverse mapping.
3. Branch work remains WebP native only after the region sweep checkpoint.

### Attempt 11 skip: WebP native Huffman single-region artifact

Current Coverage MCP baseline before editing:

- Snapshot: `cc32bd8a-5142-4efd-b171-72f448b09c7f`
- Previous run: `4ec2211a-9e64-4a29-a2b2-7beccf6dd62a`
- Lines: `23905 / 23911`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39515 / 40680`
- Missing regions: `1165`

Reverse-mapped target checked: `src/codecs/webp/native/huffman.rs`

- MCP file metrics: `330 / 331` regions, `229 / 229` lines,
  `26 / 26` branches, `9 / 9` functions.
- Local parse of the MCP-produced LLVM JSON found:
  - zero `has_count && count == 0` source segments,
  - zero missing branch entries,
  - zero macro/expansion records.

Decision:

- Skip this file for now. There is no source-mapped missing region to reverse
  map to an input or a local invariant.
- Do not add a coverage hook or fake fixture for this file until Coverage MCP or
  llvm-cov exposes an actionable source line/region.

Next action:

- Move to a small file with source-mapped missing regions rather than trying to
  force the analyzer artifact.

### Attempt 12 plan: codec dispatcher sequence invariants

Current Coverage MCP baseline before editing:

- Snapshot: `cc32bd8a-5142-4efd-b171-72f448b09c7f`
- Previous run: `4ec2211a-9e64-4a29-a2b2-7beccf6dd62a`
- Lines: `23905 / 23911`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39515 / 40680`
- Missing regions: `1165`

Reverse-mapped target: `src/codecs/mod.rs`

- Current file movement target from MCP: `99 / 103` regions.
- Missing source-mapped regions:
  - `decode_format()` image validation failure,
  - `encode_sequence_format()` sequence validation failure,
  - `encode_sequence_format()` first-frame lookup after validation,
  - `encode_sequence_format()` non-single-frame still-format rejection.

Selected sub-batch:

| Line cluster | Reverse-mapped invariant/input | Action |
| --- | --- | --- |
| `sequence.validate().ok()?` | A malformed `DecodedSequence` is a real public API input. | Exercise the failure in the coverage hook. |
| `sequence.first()?` | Dead after `sequence.validate()` because validation rejects empty frame lists. | Replace with direct first-frame access after validation. |
| `(sequence.frames.len() == 1).then(...)` | A valid multi-frame sequence encoded as a non-animation still format is real public dispatcher behavior and should return `None`. | Exercise the rejection in the coverage hook with a two-frame L8 sequence and PNG format. |

Explicitly deferred:

- `decode_format()` validation failure stays as a defensive decoder boundary.
  There is no source-mapped public input that makes an enabled decoder return an
  invalid `DecodedImage`; do not remove the guard or fabricate a decoder result.

Validation after this batch:

1. Run `cargo fmt`.
2. Run `cargo check --all-features` and
   `RUSTFLAGS='--cfg coverage' cargo check --all-features`.
3. Run only the approved Coverage MCP command
   `all-features-llvm-cov-json-nightly-branch`.
4. Record the new summary and dispatcher movement here.

### Attempt 12 result

Coverage MCP run `f3ebc240-ad40-47ce-b5b2-b850e29d6c6a`, snapshot
`6b3e36fc-3725-4ce7-b124-e00e115c097f`, passed and ingested.

- Commit metadata: `09967bc37baa6b6aa4afde4b1c96a452016b0d7d`
- Lines: `23943 / 23949`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39535 / 40697`
- Missing regions: `1162`

Net from attempt 10:

- Missing regions improved from `1165` to `1162` (`3` fewer).
- Region rate improved from `97.136%` to `97.145%`.
- Branches and functions were unchanged.

Target file movement:

| File | Before | After | Missing-region delta |
| --- | ---: | ---: | ---: |
| `src/codecs/mod.rs` | `99 / 103` | `119 / 120` | `4 -> 1` |

What moved:

- Added coverage-hook inputs for invalid sequence validation and valid
  multi-frame sequence rejection for still-image formats.
- Removed the dead `sequence.first()?` region after
  `DecodedSequence::validate()`, which already proves the frame list is
  non-empty.

Remaining in this file:

- `decode_format()` still has one defensive region for a decoder returning an
  invalid `DecodedImage`. No public fixture can reach it without making a
  decoder violate its own contract, so keep it as a defensive boundary.

### Attempt 8 result

Coverage MCP run `2be535da-cc20-47ab-a190-db9de4405713`, snapshot
`8166d21c-ed6f-441f-b24c-95b315363185`, passed and ingested.

- Lines: `23904 / 23910`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39616 / 40837`
- Missing regions: `1221`

Net from attempt 7:

- Missing regions improved from `1232` to `1221` (`11` fewer).
- Region rate improved from `96.985%` to `97.010%`.
- Lines improved from `23902 / 23909` to `23904 / 23910`; zlib is now
  line-complete.
- Branches and functions were unchanged.

Target file movement:

| File | Before | After | Missing-region delta |
| --- | ---: | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | `2758 / 3057` | `2744 / 3032` | `299 -> 288` |

This pass removed only local block/tree scanner checks. The remaining zlib
region debt is now mostly matcher state-machine and Huffman builder accounting,
which needs more granular proofs before simplifying further.

Next attack order from this result:

1. Switch to `src/codecs/gif/encode.rs` for the next region sweep unless a
   dedicated zlib matcher proof is written first.
2. Keep branch work focused on WebP native decoder/VP8/VP8L generators.
3. Preserve the new GIF/BMP fixture rows as the public-oracle portion of this
   region sweep.

### Attempt 7 result

Coverage MCP run `6594b8d8-3734-455b-b645-24e654f75827`, snapshot
`3346c3a8-7222-45d8-a6da-d8fccd32704e`, passed and ingested.

- Lines: `23902 / 23909`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39630 / 40862`
- Missing regions: `1232`

Net from attempt 6:

- Missing regions improved from `1250` to `1232` (`18` fewer).
- Region rate improved from `96.944%` to `96.985%`.
- Lines, branches, and functions were unchanged.

Target file movement:

| File | Before | After | Missing-region delta |
| --- | ---: | ---: | ---: |
| `src/codecs/compression/zlib_ng.rs` | `2783 / 3100` | `2758 / 3057` | `317 -> 299` |

This pass removed only arithmetic regions bounded by DEFLATE constants or by
the already-selected length/distance table indices. The matcher state machines
were deliberately not touched.

Next attack order from this result:

1. Continue zlib only with similarly local invariants. Good candidates are
   token block accounting and bounded Huffman frequency increments; avoid
   `longest_match()` and `process()` without a dedicated proof.
2. Alternatively switch to `src/codecs/gif/encode.rs`, where the remaining
   region gaps are mostly validated geometry and quantizer invariants.
3. Branch work remains WebP-native specific and unchanged.

### Attempt 8 plan: zlib block/tree scanner local invariants

Current Coverage MCP baseline before editing:

- Snapshot: `3346c3a8-7222-45d8-a6da-d8fccd32704e`
- Commit metadata: `2c2704c19997517155eb27af231d54cb349bd25b`
- Result: 5 passed, 0 failed; coverage artifact ingested.
- Lines: `23902 / 23909`
- Branches: `3424 / 3438`
- Functions: `1576 / 1576`
- Regions: `39630 / 40862`
- Missing regions: `1232`

Reverse-mapped target remains `src/codecs/compression/zlib_ng.rs`, now
`2758 / 3057` regions after Attempt 7.

Selected sub-batch:

| Line cluster | Invariant | Action |
| --- | --- | --- |
| `emit_blocks()` stored-length accounting | Each block contains at most `block_tokens` compressor-generated tokens. Match lengths are bounded by DEFLATE's `MAX_MATCH` and literals add exactly one byte. The already-expanded token stream is built from the same token list in order, so each per-block byte span is inside the expanded buffer. | Replace checked per-block byte accumulation and optional block slicing with direct arithmetic/slicing. |
| `write_block()` stored-cost branch | The branch only executes when `uncompressed.len() <= u16::MAX`, so adding the stored-block length/checksum overhead of `4` bytes cannot overflow. | Replace `checked_add(4)` with direct addition. |
| `scan_tree()` next-node lookup | `max_code` is produced by `build_tree()` and is always within `nodes`; the loop uses `index + 1` only when `index < max_code`. | Replace optional `nodes.get(index + 1)` with direct indexing. |
| `send_trees()` code-length order lookup | `max_bit_length_index` is chosen from `3..BIT_LENGTH_CODES`, and every entry in `CODE_LENGTH_ORDER` is a valid bit-length symbol. | Replace optional order/node lookups with direct indexing. |
| `send_tree()` next-node lookup | Same `max_code` invariant as `scan_tree()`. | Replace optional `tree.nodes.get(index + 1)` with direct indexing. |
| `send_tree()` repeat-code extra counts | Reverse mapping of zlib's run-length state: for a non-zero run whose length differs from the previous run, `min_count` is `4`, so after sending one explicit code, `count - 3` is valid. For a continuing same-length run, no explicit-code decrement happens and `count >= 3`. Zero runs select repeat code 17 only for `3..=10` and repeat code 18 only for `>=11`. | Replace checked repeat-count subtractions with direct subtraction. |

Explicitly deferred:

- Huffman frequency counters in `frequencies()` are still left checked because
  they are proportional to input size and need an input-size proof before
  replacing `u32` checked increments.
- Matcher state machines remain deferred.

Validation after this batch:

1. Run `cargo fmt`.
2. Run `cargo check --all-features` and
   `RUSTFLAGS='--cfg coverage' cargo check --all-features`.
3. Run only the approved Coverage MCP command
   `all-features-llvm-cov-json-nightly-branch`.
4. Record the new summary and zlib movement here.
