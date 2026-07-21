# 100% branch coverage attack plan

This document is the required plan before changing more implementation or fixture
code. It was originally based on Coverage MCP snapshot
`ed33587b-768e-4436-95b0-a5297ae5a2e1`, measured on pushed `main` commit
`818b3cf0e0f76a6bf3c7f67aa0cc91b21e2b9255` with suite
`all-features-lines-branches-nightly`. The current counters below are refreshed
after the committed TIFF LZW byte-alignment batch.

## Current state

- Test command: `all-features-llvm-cov-json-nightly-branch`
- Command: `cargo +nightly llvm-cov --all-features --branch --json --output-path .coverage-mcp/pillow-rs-image-llvm-nightly-branch.json --no-fail-fast`
- Result: 5 passed, 0 failed
- Current snapshot: `dc567aaf-3808-46f4-8d86-5296b6b74e7e`
- Current commit: `f565a23813f9f90b482ea9500b1d4122eab17daa`
- Lines: 21708 / 21709
- Branches: 3313 / 3484
- Functions: 1518 / 1518
- Remaining target: 1 line and 171 branches.

Coverage MCP reports this warning for LLVM JSON:

> LLVM JSON segments are normalized to segment start lines; aggregate region
> coverage is preserved from summaries.

So the file summary counters are the source of truth for branch counts. The
line-range view is used to identify where to inspect, not to sum branch counts.

## Exploration findings

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
| 23 | `src/codecs/ico/encode.rs` | 2 | 46-47 | Add ICO encode rows for size-list parsing: empty, duplicate, malformed tuple, and mixed valid/invalid requested sizes. |
| 24 | `src/codecs/jpeg/decode/huffman.rs` | 2 | 100, 181 | Add JPEG Huffman fixtures with deep code lengths, invalid/unused symbols, and entropy streams that force table boundary decisions. |
| 25 | `src/codecs/webp/encode/vp8/encoder.rs` | 1 | 441 | Add WebP lossy encode source that triggers the remaining VP8 encoder branch; inspect line 441 first to decide if fixture or cleanup. |
| 26 | `src/codecs/jpeg/encode/huffman.rs` | 1 | 141 | Add JPEG encode source producing the remaining Huffman code-length/count branch; use high-entropy and solid sources. |
| 27 | `src/codecs/webp/encode/vp8/bool_enc.rs` | 1 | 98 | Add WebP lossy encode data that drives the bool encoder across the remaining carry/range branch, likely high-entropy or threshold-sized image. |

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
