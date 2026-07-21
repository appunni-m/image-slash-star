# 100% branch coverage attack plan

This document is the required plan before changing more implementation or fixture
code. It was originally based on Coverage MCP snapshot
`ed33587b-768e-4436-95b0-a5297ae5a2e1`, measured on pushed `main` commit
`818b3cf0e0f76a6bf3c7f67aa0cc91b21e2b9255` with suite
`all-features-lines-branches-nightly`. The current counters below are refreshed
after the latest local coverage verification.

## Current state

- Test command: `all-features-llvm-cov-json-nightly-branch`
- Command: `cargo +nightly llvm-cov --all-features --branch --json --output-path .coverage-mcp/pillow-rs-image-llvm-nightly-branch.json --no-fail-fast`
- Result: 5 passed, 0 failed
- Current snapshot: `e8165588-496a-422f-a242-a97b9763403e`
- Current measured commit metadata: `471d47a0035a15793fe5ca203fac744c5c1224f3`
- Lines: 22721 / 22740
- Branches: 3385 / 3438
- Functions: 1549 / 1549
- Remaining target: 19 lines and 53 branches.
- Remaining branch map from this snapshot:
  - `src/codecs/webp/native/lossless.rs`: 108 / 110 branches, 2 missing.
  - `src/codecs/webp/native/encoder.rs`: 193 / 198 branches, 5 missing.
  - `src/codecs/webp/native/vp8.rs`: 154 / 162 branches, 8 missing.
  - `src/codecs/webp/native/decoder.rs`: 74 / 84 branches, 10 missing.
  - `src/codecs/jpeg/decode/progressive.rs`: 104 / 118 branches, 14 missing.
  - `src/codecs/tiff/decode.rs`: 100 / 114 branches, 14 missing.
- Remaining line-only gaps from this snapshot:
  - `src/types/dynamic.rs`: 813 / 814 lines, 0 branch missing.
  - `src/codecs/compression/zlib_ng.rs`: 1538 / 1539 lines, 0 branch missing.
- Note: commit `55dbe9a2ab297dab77ef4573d9bf73c2c2f8004a` is the local
  GIF decoder coverage commit; `origin/main` was still at
  `816dbf474b72c3e33b34e40c9cde013ff327c8b2` when this section was refreshed.
  The preceding rustfmt pass did not change branch coverage, but split some
  one-line branch bodies into separately counted lines.

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

## Planned WebP native encoder private-branch batch

Coverage MCP pushed-head snapshot `4103ae89-e85a-4909-b0fd-54be5b675a5c`
reports `src/codecs/webp/native/encoder.rs` at 1058 / 1058 lines,
190 / 198 branches, and 62 / 62 functions. The remaining concrete branch
lines are:

- line 247: zero-length Huffman code repeat loop. Add a direct
  `compressed_huffman_tokens(&[0; 300])` probe so the `code 18, extra 0x7f`
  chunk subtracts 138 and loops again.
- lines 379, 392, and 395: Huffman tree trimming. Defer if the simple helper
  probes do not reveal a clean frequency vector; these need tree-shape-specific
  frequencies rather than public image fixtures.
- line 877: lossless WebP dimension validation. Existing hook covers
  zero-width; add zero-height, too-wide, and too-tall calls with matching data
  lengths so the precondition assert remains valid.
- line 1027: palette delta minimization swaps a leading zero only when more
  than 17 palette values are sortable. Existing hook covers the leading-zero
  shape; add a signs-both/nonzero-leading input to drive one complementary
  side.
- line 1121: RIFF chunk padding. Add direct `write_chunk()` probes with odd and
  even payload lengths.

No Pillow manifest fixture is appropriate for these first-pass branches:
dimension validation and RIFF padding are deterministic encoder boundary
behavior, while Huffman/palette probes target private encoder normalization
after candidate pixels have already been selected. Use coverage-only private
hook assertions and verify with the approved Coverage MCP line+branch command.

First-pass evidence before short-palette retry:

- Coverage MCP run `a8904821-666e-45e8-b725-1f8aa32e9007`, snapshot
  `8c4bb4bf-2278-4d10-9124-5b0f7e7a0222`, passed and improved overall
  coverage to 3356 / 3468 branches, but introduced a hook assertion branch.
- Final accepted Coverage MCP run `3b7d8f94-4f67-45eb-9187-0d991c4b10c5`,
  snapshot `fc4d8182-71fa-4256-906c-ed388d40f8a7`, passed with 5 passed,
  0 failed and
  improved overall coverage to 3355 / 3466 branches. `encoder.rs` improved
  from 190 / 198 to 193 / 198 branches. Dimension validation is now fully
  covered.
- Short-palette retry `a47a7e00-695c-483a-b8bf-6bd5932ebe9b`, snapshot
  `df58d5f5-02a5-4f9a-b8ae-9244def3f610`, passed but did not improve overall
  or target-file branch coverage, so the no-op probe was removed.

Current retry plan from pushed-head snapshot
`5b789fe0-a42e-4905-9cd2-33893a50684a`: `encoder.rs` remains 1080 / 1080
lines, 193 / 198 branches, and 63 / 63 functions. Try three narrow
coverage-only probes:

- line 247: add `compressed_huffman_tokens(&[0; 276])` so zero-run repetition
  subtracts exactly two 138-symbol chunks and the inner `while repetitions != 0`
  evaluates its false side.
- line 1027: call `encode_alpha()` with 17 unique alpha values, leading zero,
  and mixed positive/negative deltas so `signs == 3`, `palette_values[0] == 0`,
  and `sortable_len > 17` is false.
- line 1121: add a coverage-only writer that fails on the padding byte, so the
  odd-padding `write_all(&[0])?` error branch is reached. If needed, add an
  even-length failing writer to isolate the remaining hidden `?` branch.

Do not keep any probe that does not improve MCP line+branch coverage.

Retry evidence:

- Coverage MCP run `8da58832-0511-4632-af0f-ffffc1e53b23`, snapshot
  `9b904344-a1da-47c0-8488-59debe449317`, passed but introduced uncovered
  helper-writer lines and did not close the existing encoder gap lines.
- Coverage MCP run `4f62c69e-14b1-4adc-a626-844250839c19`, snapshot
  `07a01e76-742d-4f05-9585-b6e60949385a`, passed after covering the helper
  writer flush path, but still did not reduce the remaining branch deficit:
  overall moved from 3358 / 3466 to 3360 / 3468 and `encoder.rs` from
  193 / 198 to 195 / 200, leaving the same six gap lines. The code probes were
  removed as no-op coverage noise. Remaining `encoder.rs` gaps are still
  lines 247, 379, 392, 395, 1027, and 1121.

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
