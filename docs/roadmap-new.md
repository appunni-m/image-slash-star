# Public capability roadmap

This is a compact rendering of the [machine ledger](../roadmap.json).
Its claim baseline is `93ec80ec99c42671dce6cf70694bce27ad8a2ef4`. Counts and finding statuses belong to that
review; subsequent release and documentation improvements are recorded in
[Evidence](EVIDENCE.md) and [Releasing](../RELEASING.md). A historical finding
is not evidence that a later fix failed. No entry is silently removed here.

## Current pure-Rust AVIF cutover

The ledger's AVIF runtime is safe Rust with no native runtime fallback.
Its recorded baseline test counts are 45/45 matrix tests and 66/66 feature-gate tests.
Historical LLVM coverage: 100,389/110,015 lines (91.2503%), 12,861/14,246 branches (90.2780%), 5,125/5,794 functions (88.4536%), and 150,221/166,375 regions (90.2906%).
The coverage-origin verifier passes for 540 exact `cfg(coverage)` guards across 89 files.

The bounded raster contract's eleven Rust tests prove alignment, checked extents,
overlap rejection, no partial mutation, and complete-canvas enforcement.
`place_cells` validates the complete tile/grid batch before copying samples.
The earlier next-leaf regression identifies an 8×16 coded block at block coordinates `(6, 0)`;
its rectangular transform is a bounded implementation witness, not a claim that
all AVIF partitions or high-bit-depth reconstruction are complete.

## Latest API-038 implementation candidate

The retained API finding history is available in Git. Current API use and limits
are described in [Usage](USAGE.md); the ledger below retains its reviewed scope.

## AVIF planned-gap ledger (current tree)

- AVIF decode/inspect/verify: 343 rows total, 340 active, 3 explicit planned gaps.
- AVIF encode: 32 rows total, all 32 explicit planned gaps.
- Whole matrix: 1,567 rows total, 1167 active decode rows, 365 active encode rows, 3 planned decode rows, and 32 planned encode rows.

### Former native-only cases: explicit Rust work, never hidden fallback

The `former_native_only` field preserves provenance; it does not authorize a
native runtime path or count a planned row as parity. The tracked work is:

| Input | Work item |
| --- | --- |
| high_bitdepth | AVF-SAMPLE-001 |
| hdr | AVF-COLOR-001 |
| animated | AVF-SEQUENCE-001 |
| All planned encoder rows | AVF-ENCODE-001 |

## AVIF sequence implementation in progress — 2026-09-17

The single-reference temporal candidate path now projects one vector instead
of rejecting the unused second-reference sentinel. The independent
[oracle record](../tests/fixtures/outputs/av1_temporal/animated/index.json)
contains 177 lookups from the existing complete `animated.avif`, including
32 valid single-reference candidates: two insertions, 30 weight updates, and
two global-MV context updates. Unmodified and instrumented pinned dav1d output
matches byte for byte, and the trace repeats exactly.

The fixture-driven Rust regression is implemented and compile-checked;
behavioral execution remains deferred until implementation is complete. This
is private transition evidence, not proof of complete public sequence parity.
`AVF-SEQUENCE-001`, the animated matrix row, and all finding counts retain their
existing open status. Regeneration and limitations are documented in the
[temporal oracle notes](../tests/fixtures/outputs/av1_temporal/README.md).

## HDR color implementation in progress — 2026-09-17

The RGB converter now includes the exact 10-bit, full-range I444, no-alpha
CICP 9/16/9 declaration from the existing `hdr.avif`. Independent unmodified
dav1d and libavif produce identical visible planes; pinned scalar libyuv,
libavif and Pillow produce identical RGB bytes. The conversion preserves
PQ-encoded values, matching Pillow without an extra tone-mapping step.

The [color oracle record](../tests/fixtures/outputs/avif_hdr_color/hdr/index.json)
and [regeneration notes](../tests/fixtures/outputs/avif_hdr_color/README.md)
separate this color boundary from AV1 reconstruction. The Rust regression
compares the complete native planes and 48 real-pixel slices; behavioral
execution remains deferred. HDR, high-depth sequences, the encoder rows and
all finding counts retain their open statuses.

## High-depth color implementation in progress — 2026-09-17

The converter now includes the exact 12-bit limited-range I422 CICP 2/2/2
declaration with auxiliary alpha from `10bit.avif`. Independent dav1d and
libavif agree on all five frames' color and alpha planes; scalar libyuv,
libavif and Pillow agree on every RGBA byte. Both color and alpha downshift
before the 8-bit conversion. The fixture witnesses partial transparency and
RGB clipping, but contains no zero-alpha pixels.

The [native record](../tests/fixtures/outputs/avif_sequence_color/high_bitdepth/index.json)
and [regeneration notes](../tests/fixtures/outputs/avif_sequence_color/README.md)
bound this evidence to color conversion. The Rust regressions compare five
complete frames and 25 row slices; execution and managed coverage remain
deferred. Sequence admission and all open matrix/finding statuses are retained.

## Sequence presentation implementation in progress — 2026-09-17

The 2026-09-17 sequence presentation candidate now processes matching color
and alpha samples together, retains AV1 references between samples and converts
each completed display before advancing. Output byte budgets are reserved
before reconstruction, with frame-count/mode agreement and actual header
geometry checks. Ordinary first-image and sequence blanket rejections are
removed; missing surfaces remain typed capability gaps. Malformed tile
envelopes and empty reference slots retain precedence over those gaps.

The [sequence oracle](../tests/fixtures/outputs/av1_sequence/animated/index.json)
records five native displays, including hidden-reference and show-existing
state, exact 1/30-second timing and independent libavif repetition metadata.
Public regressions cover all frames of both `animated.avif` and `10bit.avif`,
but Rust behavioral execution and managed coverage remain deferred. The
[evidence notes](../tests/fixtures/outputs/av1_sequence/README.md) retain the
geometry, resource, metadata and frame-ID limitations. No row or finding is
promoted by this implementation candidate.

## Frame-ID sequence implementation in progress — 2026-09-17

Pinned native traces exposed incorrect reference syntax ordering in both Rust
and the Python inspector. The implementation now reads each index/delta pair
together, validates the expected reference ID with checked modulo arithmetic,
and presents frame-ID sequences through the common transactional path.
Repeated current IDs retain their existing structural diagnostic.

The [native bundle](../tests/fixtures/outputs/av1_sequence/error_resilient/index.json)
records both complete error-resilient frames, all seven native reference reads,
an accepted full-file ID-wraparound variant, and a rejected full-file delta
mismatch that preserves the independent first image. Native and inspector
positions agree after accounting for the OBU header. Rust behavioral execution
and managed coverage remain deferred. Short-signaling fallback, stale-reference
policy, broader reconstruction and matrix loop-origin reconciliation remain
open; no row or finding is promoted by this candidate.

## Short-reference selection implementation in progress — 2026-09-17

The short-signaling candidate now retains an unset future reference until the
native past/earliest fallback resolves it. Both the Rust helper and Python
inspector preserve native tie rules and allow duplicate LAST/GOLDEN anchors
and repeated reuse of the earliest slot. All eight reference headers remain
required before selection.

The [native corpus](../tests/fixtures/outputs/av1_short_references/index.json)
contains six complete AVIF mutations, repeated native selected-index traces,
4,608 YUV bytes and 9,216 RGB bytes. The tile and primary-image bytes are
unchanged; OBU, sample and container lengths are repaired together. Deferred
public tests compare every frame and exact timing, while a coverage-only
adapter compares native indices through the actual production helper.
All 12 strict compile-check lanes pass. Rust execution and managed coverage
remain deferred, so no row or finding is promoted. Mixed reference histories,
broader reconstruction, resource accounting and final parity remain open.

## Sequence repetition implementation in progress — 2026-09-17

The edit-list parser now matches pinned native handling of absent/nonrepeating
lists, ignored fields, rounded repeats and signed repetition overflow. Exactly
2,147,483,648 total plays remain finite; larger counts normalize to infinite.
Malformed repeating-list counts, versions and missing fields are container
errors. Color-track repetition takes precedence over alpha metadata.

The [loop corpus](../tests/fixtures/outputs/avif_loops/index.json) contains
28 complete files: 20 accepted cases with unchanged pixels and eight native
parse/Pillow-open rejections. The matrix generator now binds AVIF loop values
to native case/input/index hashes and records an independent origin. Pillow's
missing loop key remains separately recorded. Deferred public regressions cover
pixels, timing, loop values and malformed lifecycle operations. No row or
finding is promoted; Rust execution and managed coverage remain deferred.

## Display retention implementation in progress — 2026-09-17

AV1 state now retains one selected display candidate per color/alpha track.
Superseded and lower-priority candidates release their surface references and
diagnostic buffers immediately. The selection order remains temporal unit,
spatial layer, temporal layer, then latest exact tie. Reference refresh and CDF
updates remain independent of whether a display wins selection.

Deferred ownership regressions check candidate release, independent reference
ownership, hidden-sample filtering and failed flushes. They are internal model
assertions; the unchanged complete native sequence fixtures remain the pixel
regression authority. This removes candidate-history growth without establishing
a total reference/scratch memory limit or measured peak-memory improvement.
Rust behavioral tests and managed coverage remain deferred; no row is promoted.

## Complete open-task inventory

The retained ledger contains **244 active finding rows**.

| Area | Count | IDs |
| --- | ---: | --- |
| Common API | 24 | `API-008`, `API-014`, `API-017`, `API-018`, `API-019`, `API-020`, `API-023`, `API-026`, `API-027`, `API-030`, `API-033`, `API-034`, `API-036`, `API-041`, `API-043`, `API-044`, `API-045`, `API-046`, `API-047`, `API-048`, `API-051`, `API-052`, `API-053`, `API-054` |
| JPEG | 19 | `JPG-002`, `JPG-003`, `JPG-004`, `JPG-006`, `JPG-007`, `JPG-008`, `JPG-009`, `JPG-010`, `JPG-011`, `JPG-012`, `JPG-013`, `JPG-014`, `JPG-015`, `JPG-016`, `JPG-017`, `JPG-018`, `JPG-019`, `JPG-020`, `JPG-021` |
| PNG | 15 | `PNG-003`, `PNG-004`, `PNG-005`, `PNG-006`, `PNG-008`, `PNG-010`, `PNG-011`, `PNG-012`, `PNG-013`, `PNG-015`, `PNG-016`, `PNG-017`, `PNG-018`, `PNG-019`, `PNG-020` |
| GIF | 16 | `GIF-002`, `GIF-005`, `GIF-006`, `GIF-007`, `GIF-009`, `GIF-010`, `GIF-011`, `GIF-012`, `GIF-014`, `GIF-015`, `GIF-016`, `GIF-017`, `GIF-018`, `GIF-019`, `GIF-020`, `GIF-021` |
| BMP | 20 | `BMP-001`, `BMP-002`, `BMP-003`, `BMP-004`, `BMP-005`, `BMP-006`, `BMP-007`, `BMP-008`, `BMP-009`, `BMP-010`, `BMP-011`, `BMP-012`, `BMP-013`, `BMP-014`, `BMP-015`, `BMP-016`, `BMP-017`, `BMP-018`, `BMP-019`, `BMP-020` |
| ICO/CUR | 20 | `ICO-001`, `ICO-002`, `ICO-004`, `ICO-005`, `ICO-006`, `ICO-007`, `ICO-008`, `ICO-009`, `ICO-010`, `ICO-011`, `ICO-012`, `ICO-013`, `ICO-014`, `ICO-015`, `ICO-016`, `ICO-017`, `ICO-018`, `ICO-019`, `ICO-020`, `ICO-021` |
| TIFF | 26 | `TIF-002`, `TIF-003`, `TIF-005`, `TIF-006`, `TIF-007`, `TIF-008`, `TIF-009`, `TIF-010`, `TIF-011`, `TIF-012`, `TIF-013`, `TIF-014`, `TIF-016`, `TIF-017`, `TIF-018`, `TIF-020`, `TIF-021`, `TIF-022`, `TIF-023`, `TIF-024`, `TIF-025`, `TIF-026`, `TIF-027`, `TIF-028`, `TIF-029`, `TIF-030` |
| WEBP | 20 | `WEP-001`, `WEP-003`, `WEP-004`, `WEP-005`, `WEP-007`, `WEP-008`, `WEP-009`, `WEP-010`, `WEP-011`, `WEP-012`, `WEP-013`, `WEP-014`, `WEP-015`, `WEP-016`, `WEP-017`, `WEP-018`, `WEP-019`, `WEP-020`, `WEP-021`, `WEP-022` |
| AVIF | 30 | `AVF-001`, `AVF-003`, `AVF-004`, `AVF-005`, `AVF-006`, `AVF-008`, `AVF-009`, `AVF-011`, `AVF-012`, `AVF-013`, `AVF-014`, `AVF-015`, `AVF-016`, `AVF-018`, `AVF-019`, `AVF-020`, `AVF-022`, `AVF-023`, `AVF-024`, `AVF-025`, `AVF-026`, `AVF-027`, `AVF-028`, `AVF-029`, `AVF-030`, `AVF-031`, `AVF-032`, `AVF-033`, `AVF-034`, `AVF-035` |
| Features/package | 23 | `FTR-001`, `FTR-002`, `FTR-006`, `FTR-009`, `FTR-010`, `FTR-011`, `FTR-013`, `FTR-014`, `FTR-015`, `FTR-016`, `FTR-017`, `FTR-018`, `FTR-020`, `FTR-021`, `FTR-022`, `FTR-023`, `FTR-024`, `FTR-027`, `FTR-029`, `FTR-034`, `FTR-035`, `FTR-037`, `FTR-038` |
| Assurance | 29 | `QA-001`, `QA-002`, `QA-003`, `QA-006`, `QA-009`, `QA-010`, `QA-011`, `QA-012`, `QA-016`, `QA-019`, `QA-020`, `QA-021`, `QA-022`, `QA-023`, `QA-024`, `QA-026`, `QA-027`, `QA-028`, `QA-030`, `QA-031`, `QA-033`, `QA-034`, `QA-035`, `QA-036`, `QA-037`, `QA-039`, `QA-040`, `QA-041`, `QA-042` |
| Documentation | 2 | `DOC-007`, `DOC-008` |

## Parked, not pending

Future format proposals are parked. General image editing remains outside scope.

## Finding details

### API-008

Status at ledger review: **open**.

No YCbCr/YCCK/BGR transfer mode exists, constraining otherwise codec-native JPEG/TIFF/WebP/AVIF input contracts.

Next recorded action: Add a mode only when at least one decode or encode fixture needs byte-preserving transfer. Avoid adding modes merely to mirror another library.

### API-014

Status at ledger review: **open**.

Lazy materialization retains the complete encoded snapshot and each successful still/sequence decoded payload forever; clones share both, and the retained-payload model documents the independent caches and borrowed-view distinction. Owned sources and each borrowed view now retain only an immutable verification result after the first check; borrowed-view clones share that result. Source-bound policy paths reuse retained `ImageInfo` after per-operation metadata checks, so they avoid a second generic header inspection without retaining parsed codec state. There is no eviction, parsed codec-state cache, or allocator measurement.

Next recorded action: Keep the retained-payload accounting current; benchmark allocator/peak behavior before adding optional cache release or parsed-state reuse.

### API-017

Status at ledger review: **open**.

Encoders still produce complete `Vec<u8>` output for their return APIs, and codec working state can remain whole-buffer until a validated sink segment is ready. The dependency-free `OutputSink` contract normalizes write and post-delivery flush rejection to `ImageError::OutputWrite`; every current JPEG, PNG, GIF, BMP, TIFF, WebP, ICO, and native AVIF sink path now emits validated structural segments after exact-length preflight. Every sink path calls `OutputSink::flush` once after complete delivery. JPEG delivery splits marker/scan spans, GIF delivery splits signature/logical-screen, color-table, extension/image sub-block, and trailer segments, WebP delivery splits RIFF/chunk spans, ICO delivery splits its directory from embedded payload, TIFF delivery splits its header, page strip/padding, and IFD/value spans, and native AVIF delivery splits validated ISO-BMFF top-level boxes. These are structural delivery boundaries, not universal streaming; PNG filtered/compressed buffers, BMP row/palette segments, GIF working state, WebP encoded RIFF state, ICO embedded payload, TIFF page/compressed-pixel state, and native AVIF's complete encoded buffer remain bounded working state. The original cross-codec Rust-only contract exercises a genuine partial second structural write across every still encoder and supported multi-frame GIF/TIFF/WebP/native-AVIF sequence writer available in each feature/target lane; test revision `d5f7e416b30862819dbddb38f8b6027cc4219076` additionally drives the exact-byte and TIFF option/sequence sink contracts through a partial second write, preserving the delivered prefix and reporting the selected encode stage. Broader interrupted-write, rollback, and cleanup behavior remain open.

Next recorded action: The current structural sink layer now tracks whether delivery was attempted and restores opted-in checkpoints after failed writes, cancellation, or flush; next reduce transient working buffers one independently enforceable boundary at a time and define any future byte-counting writer without claiming universal streaming.

### API-018

Status at ledger review: **open**.

The incremental input contract now covers detection, basic inspection, still decode, and sequence decode (`decode_prefix`/`decode_sequence_prefix`, COR-059) with exact or progress-aware `NeedMoreData { minimum }`; streaming decompression that produces partial pixels before the container completes remains future work.

Next recorded action: Keep the same status semantics for any future streaming iterator/reader surface.

### API-019

Status at ledger review: **open**.

PNG known metadata chunks, GIF extensions, JPEG APPn/COM marker payloads, WebP ICCP/EXIF/XMP chunks, TIFF metadata tags, and AVIF top-level unknown/free/skip boxes are retained as raw opaque records. Recognized AVIF `Exif` items and `mime` items with content type `application/rdf+xml` are retained as ordered raw `OpaqueMetadata` records on still and sequence decode; primary AVIF CICP/`clli`/`mdcv` color properties, `prof`/`rICC` ICC profiles, primary `av1C` chroma sample position, and `irot`/`imir`/`pasp`/`clap` item properties remain typed source descriptors. Direct alpha `auxl` provenance is represented by `SourceAlpha::Auxiliary`, the scalar and bounded plural auxiliary-relationship getters, the ordered `SourceDescriptor::avif_grid_item_ids()` list and validated primary-grid payload topology through `SourceDescriptor::avif_grid_properties()`, bounded `dimg`/other `iref` edges through `SourceDescriptor::avif_item_relationships()`, filtered `prem` edges through `SourceDescriptor::avif_premultiplied_relationships()`, typed non-primary `colr`/`nclx` CICP declarations through `SourceDescriptor::avif_item_color_properties()`, raw non-primary `prof`/`rICC` profiles through `SourceDescriptor::avif_item_icc_profiles()`, unknown and known associated non-primary properties through `SourceDescriptor::avif_item_properties()` as source-local item ID, kind, and exact payload records, known non-primary/auxiliary `ispe`/`pixi` plane declarations through `SourceDescriptor::avif_item_plane_properties()`, and non-primary/auxiliary `av1C` declarations through `SourceDescriptor::avif_item_codec_properties()` as source-local item ID, exact payload, bit depth, and chroma sample position. Grid tile placement/composition, plane range/quality semantics, other non-primary/auxiliary color forms, and other item metadata remain open.

Next recorded action: Extend the opaque model to the remaining AVIF item/property graph and exact color fields; parsed semantics are optional and format-specific.

### API-020

Status at ledger review: **open**.

Source format is retained, but encoding always asks for an explicit target.

Next recorded action: Keep explicit target selection. Add a same-source convenience only if metadata, sequences, and unsupported modes cannot make it silently lossy.

### API-023

Status at ledger review: **open**.

Remaining gaps are transient encoded-output allocation/peak accounting (the public policy deliberately makes no recoverable-OOM promise), interior work beyond the current checkpoint set, and complete short-write/rollback semantics. The implemented decode, output-admission, cooperative work checkpoints, WebP L1/P8/L8/La8/CMYK source-mode preparation and RGBA alpha/RGB extraction after each 1,024 source pixels, PNG stored-block boundaries and 1,024-byte stored-block-copy intervals, all-level Deflate matcher/expansion/Huffman/bitstream/checksum stages, BMP row-conversion subsegments, JPEG RGB-to-YCbCr conversion and chroma-downsample output after each 1,024 pixels, baseline entropy traversal after each 1,024 MCUs, JPEG forward-DCT/quantization after each completed 8x8 block, optimized baseline Huffman frequency gathering after each 1,024 AC coefficients, progressive scan block-slot generation after each 1,024 blocks, progressive scan-event frequency gathering after each 1,024 events, progressive scan coefficient traversal after each 1,024 coefficients, JPEG baseline/progressive entropy-output after each 1,024 emitted bytes, high-color GIF nearest-palette candidate ordering and bounded scans after each 1,024 work items, lossy WebP RGBA transparent-area cleanup and alpha-palette source collection and index packing plus lossy WebP VP8 required padded Y/U/V edge-replication, 64-value segment-clustering chunks, analysis segment assignment, intra4 mode-selection candidate-trial stages, forward- and inverse-transform row/column subpasses, non-trellis quantization coefficients, method-6 trellis-quantization coefficient candidates and path-reconstruction nodes, squared-error pixels, spectral-distortion weighted-transform row/column passes, residual-cost coefficients, candidates and each completed luma 4×4 block with an outer 64-macroblock checkpoint, filter-edge adjustment, coefficient-statistics collection, and the first-partition segment-probability prepass after each 1,024 selected macroblocks as applicable and lossless WebP VP8L hidden-RGB cleanup, image-palette source scans, ordered unique-color palette drains after each 1,024 pixels or colors, and palette-mode index packing plus sampled meta-pixel materialization after each 1,024 retained histogram symbols, entropy-mode pixel histogram scans after each completed 1,024-pixel chunk on rows wider than 1,024 pixels, lossy WebP VP8 first-partition coding after each 8, 16, 32, 64, 128, 256, 512, 1,024, 2,048, 4,096, 8,192, 32,768, 65,536, 131,072, and 262,144 logical coded bits and coefficient coding through 2,097,152 logical coded bits, lossless WebP VP8L entropy-mode histogram-cost scans after each 64 symbols, palette-index lookup candidate scans after each 64 palette entries, palette sign collection and nearest-delta candidate scans after each 64 palette entries or candidate values, entropy-bin histogram-clustering populated-tile collection, min/max, and bin-assignment pre-passes after each 64 tile histograms, copy-token cache-population scans after each 256 pixels, Huffman RLE preparation and in-run code-length scans after each 64 symbols, canonical-code assignment scans after each 64 code-length symbols, Huffman-tree ordering comparisons and insertion scans after each 64 comparisons or candidate nodes, Huffman-tree code-length-token frequency and trailing zero-repeat-token trim scans after each 16 compressed token entries, Huffman code-length emission after each 16 compressed token entries, and lossless WebP VP8L bitstream coding after each 8, 16, 32, 64, 128, 256, 512, 1,024, 2,048, 4,096, 8,192, 16,384, 32,768, 65,536, 131,072, 262,144, 524,288, and 2,097,152 logical coded bits are current behavior documented in the architecture/testing contracts, not active roadmap items.

Next recorded action: Add one independently enforceable allocation or work dimension at a time; preserve unlimited wrappers, reject before future bounded allocation/work begins, and fixture each inclusive boundary and error-precedence rule.

### API-026

Status at ledger review: **open**.

Decoded samples and palettes are always owned mutable vectors. Callers cannot borrow immutable output, reuse an allocation, or transfer shared backing storage without a copy.

Next recorded action: Let the destination-buffer work solve reuse first. Add borrowed/shared public representations only if native and WASM measurements show a material copy cost.

### API-027

Status at ledger review: **open**.

The source-bound `decode_frame` contract is complete with stable per-frame errors, and TIFF has a genuine per-page decode path. GIF, APNG, WebP, and AVIF still decode the full sequence for one frame, and there is no iterator. The owned source now retains a separate lazy sequence cache, but it still materializes every frame.

Next recorded action: Extend the per-frame path to GIF/APNG/WebP/AVIF, then add iteration. Keep eager `decode_sequence` as a convenience collector and retain the shared lazy cache as the repeated-call path.

### API-030

Status at ledger review: **open**.

Codec-dispatched failures now retain a stable operation `stage`, the encoded-input byte `offset`, and a container-structure `identity` through the corresponding accessors. Caller-owned sink rejection has the separate `OutputWrite` category with selected output format, encode stage, and diagnostic message; `EncodePolicy` failures carry the selected format, encode operation, typed `EncodedOutputBytes` or `EncodeWorkUnits` resource, maximum, and observed result/checkpoint value. `Unsupported` additionally exposes `unsupported_reason()` for target-unavailable and not-implemented capability failures. BMP header, palette, pixel-span, bitfield, and RLE parse failures now retain stable context, ICO header, directory, entry-range, and embedded PNG/DIB/CUR failures now retain stable ICO context, TIFF compressed strip/tile payload failures now retain `tiff_strip`/`tiff_tile` context, and WebP inspection/container-chunk failures now retain stable WebP context. WebP still and sequence payload-decoder failures now retain `webp_bitstream` at the validated VP8/VP8L payload start, or the current ANMF container offset for animation; finer decoder-internal cursors remain intentionally limited.

Next recorded action: Extend structured fields without promising unstable prose. Every newly represented field needs malformed, boundary, capability, and output-destination fixtures.

### API-033

Status at ledger review: **open**.

Callers cannot choose source-preserving versus normalized samples, byte order, alpha association, or a codec-native output colorspace.

Next recorded action: Define explicit output policy only for byte-preserving codec needs. The default remains Pillow-observable normalized transfer bytes.

### API-034

Status at ledger review: **open**.

PNG source color fields (sRGB intent, gamma, chromaticities, raw ICC profile), primary AVIF CICP/`clli` fields (primaries, transfer, matrix, range, maxCLL, maxPALL), primary AVIF `mdcv` mastering-display fields, primary AVIF `prof`/`rICC` ICC profile bytes, primary `av1C` chroma sample position, and primary AVIF `irot`/`imir`/`pasp`/`clap` declarations are retained. Recognized AVIF EXIF/XMP item payloads are retained raw, without semantic parsing or pixel transforms; direct alpha provenance is represented by `SourceAlpha::Auxiliary` plus scalar and bounded plural source-local relationships, the supported primary grid retains its ordered derived item IDs and validated version/flags/row/column/output-canvas topology through `SourceDescriptor::avif_grid_properties()`, bounded `iref` edges—including `prem`—are retained as source descriptors, typed non-primary/auxiliary `colr`/`nclx` CICP declarations retain their source-local item IDs through `SourceDescriptor::avif_item_color_properties()`, non-primary/auxiliary `prof`/`rICC` profiles retain their exact item IDs and raw profile bytes through `SourceDescriptor::avif_item_icc_profiles()`, unknown and known associated non-primary `clli`/`mdcv`/`irot`/`imir`/`pasp`/`clap` properties retain their source-local item ID, four-byte kind, and exact payload through `SourceDescriptor::avif_item_properties()`, known non-primary/auxiliary `ispe`/`pixi` declarations retain source-local item IDs, dimensions, and uniform channel depth through `SourceDescriptor::avif_item_plane_properties()`, and non-primary/auxiliary `av1C` declarations retain source-local item IDs, exact payload, declared bit depth, and chroma sample position through `SourceDescriptor::avif_item_codec_properties()` without interpretation or pixel transforms. Grid tile placement/composition, plane range/quality semantics, other non-primary/auxiliary color forms, JPEG Adobe/JFIF color interpretation, TIFF colorimetric tags, and WebP color metadata are not yet retained.

Next recorded action: Preserve the remaining opaque profiles and exact container fields per format. Never imply that retaining color, metadata, or transform fields means pixel conversion was applied.

### API-036

Status at ledger review: **open**.

Token-aware PNG still and APNG decode now poll before each filtered-row reconstruction and sample-unpack row, including Adam7 passes; the no-token traversal remains direct. The lossy WebP VP8 required padded Y/U/V edge-replication pass, 64-value segment-clustering chunks, analysis segment-assignment pass, intra4 mode-selection candidate-trial stages, forward- and inverse-transform row/column subpasses, non-trellis quantization coefficients, method-6 trellis-quantization coefficient candidates and path-reconstruction nodes, squared-error pixels, spectral-distortion weighted-transform row/column passes, residual-cost coefficients, candidates and each completed luma 4×4 block with an outer 64-macroblock checkpoint, filter-edge adjustment, coefficient-statistics collection, and first-partition segment-probability prepass now poll after each 1,024 padded item or selected macroblock as applicable; aligned planes use a direct clone because no replication is needed. Token-aware lossless VP8L entropy-mode pixel histogram analysis also polls after each completed 1,024-pixel chunk on rows wider than 1,024 pixels; narrower rows remain bounded by existing row-start polls and the no-token traversal is direct. Remaining gaps are progress semantics, CPU/instruction interruption inside codec work beyond the documented checkpoints, work inside one documented pixel, weighted-transform row/column pass, residual-cost coefficient, or other codec unit, finer WebP stages beyond the current logical-bit, output-byte, and documented codec-internal intervals, JPEG interior work beyond its current 1,024-pixel RGB-to-YCbCr, 1,024-pixel chroma-downsample output, 1,024-MCU baseline entropy traversal, completed 8x8 forward-DCT/quantization-block, optimized baseline Huffman frequency gathering after each 1,024 AC coefficients, progressive scan block-slot generation after each 1,024 blocks, progressive scan-event frequency gathering after each 1,024 events, progressive scan coefficient traversal after each 1,024 coefficients, and 1,024-byte entropy-output intervals, and short-write/rollback cleanup. Current cancellation and sink-boundary behavior belongs in the architecture/testing contracts.

Next recorded action: Define progress and rollback semantics without claiming universal interior interruption; add checkpoints only for a real long-running operation and retain a separate Rust-only feature-gate contract when Pillow has no equivalent result.

### API-041

Status at ledger review: **open**.

Rust enums, structured errors, byte ownership, and 64-bit sizes have no stable JavaScript transfer schema.

Next recorded action: Design a versioned binding contract after native API semantics settle; preserve precise error kinds and avoid string-only JS failures.

### API-043

Status at ledger review: **open**.

The non-terminal `NeedMoreData { minimum }` state now exists for detection, basic inspection, still decode, and sequence decode, with exact minimum-byte or progress semantics; terminal results must never be retried.

Next recorded action: Keep the status stable for any future streaming surface and document per-operation progress.

### API-044

Status at ledger review: **open**.

Current resource limits are per-call eligibility checks before cache access, are never cached, and cannot be bypassed by cached success. Future output mode, strictness, metadata, or color/alpha policies would still make the single permanent still-decode cache key ambiguous.

Next recorded action: Keep resource eligibility outside the cache key. Before API-033 or another result-shaping policy lands, choose separately keyed materialization or explicitly disallow that policy on cached sources.

### API-045

Status at ledger review: **open**.

`EncodedImage::new` detects and inspects once; owned and borrowed source-bound still/sequence decode reuse that validated format instead of repeating signature detection. Policy-aware source paths repeat encoded-input, format, and metadata checks but reuse retained `ImageInfo` for dimension, decoded-byte, frame-count, and primary sequence-byte limits, avoiding a second generic header inspection. Owned clones share an immutable verification result, and borrowed-view clones share their view's verification result, but each codec still parses the container for materialization and no parsed header/index is retained.

Next recorded action: Measure the remaining duplicate codec work, then retain an immutable parsed header/index only when every codec can prove that reuse cannot make later validation weaker.

### API-046

Status at ledger review: **open**.

Callers can preflight exact row bytes, packed-row status, total allocation, and alignment through `TransferLayout` (API-025), but per-plane sizes, byte order, and future codec-native layouts are still absent. `ColorType::bits_per_pixel` is insufficient for source-endian TIFF numeric bytes and future planar data.

Next recorded action: Add a checked transfer-layout result for each new layout as it lands. Keep it about byte transport, not image processing.

### API-047

Status at ledger review: **open**.

`ImageInfo.frame_count: Option<u32>` collapses not-applicable, not-yet-scanned, scan-limited, malformed-later, and genuinely unknown states. A partial demux cannot report “N complete frames seen, another is partial.”

Next recorded action: Replace the optional count with a small completeness/result model before incremental inspect or frame enumeration becomes public.

### API-048

Status at ledger review: **open**.

`Decoded<T>` retains only the eight-format enum. It cannot identify APNG versus PNG, classic TIFF versus BigTIFF, ICO versus CUR without inspecting a selected hotspot, VP8/VP8L/VP8X, AVIF item versus sequence source, or the source precision/profile class.

Next recorded action: Add codec-specific inspected descriptors behind the format feature. Keep `ImageFormat` as the stable dispatch identity.

### API-051

Status at ledger review: **open**.

Exact numerator/denominator retention raises two distinct equality questions: `1/2` and `2/4` are the same duration but different source fields. Unbounded LCM conversion can also overflow when choosing one sequence timescale.

Next recorded action: Preserve raw source fields separately from a reduced semantic duration, use checked arithmetic, and make every encoder's quantization/overflow result explicit.

### API-052

Status at ledger review: **open**.

A format-neutral `Reserved(u8)` disposal or blend value does not identify the governing format or whether round-trip replay is legal. The same numeric code has no universal meaning across GIF, APNG, and WebP.

Next recorded action: Retain a format-qualified raw code beside normalized known semantics; unknown values must not be silently replayed into another target.

### API-053

Status at ledger review: **open**.

`RenderedCanvas` says the pixel extent but not whether the returned canvas is before blend, after blend, or after disposal, nor which prior frame state was used. That distinction affects frame extraction, seeking, cache reuse, and re-encoding.

Next recorded action: Define the exact presentation instant in rustdoc and fixtures. Expose raw source rectangles separately when exact container reconstruction needs them.

### API-054

Status at ledger review: **open**.

`DecodedSequence` has no canvas sample mode or palette namespace. Frames may carry different modes and local palettes, while a GIF background index refers to a global table rather than an arbitrary frame palette.

Next recorded action: Define allowed mixed-mode sequences and give palette-index backgrounds an explicit palette owner before generic sequence encoding expands.

### JPG-002

Status at ledger review: **open**.

Pillow accepts bilevel and YCbCr source modes; Rust has neither private bilevel normalization nor a YCbCr input mode.

Next recorded action: Add one mode at a time with exact marker, component, and pixel references.

### JPG-004

Status at ledger review: **open**.

Public options omit Pillow 12.2.0 surfaces including quantization tables, ICC, DPI, comments, `keep_rgb`, separate restart-block/row controls, and stream type.

Next recorded action: Inventory pinned `JpegImagePlugin._save`; accept only independently fixture-backed options in a typed JPEG options struct.

### JPG-006

Status at ledger review: **open**.

Legal JPEG classes beyond the manifest—lossless processes, arithmetic coding, uncommon sampling/component layouts, and 12-bit data—have no support statement.

Next recorded action: Classify each as supported, Pillow-rejected, or explicit `Unsupported` using pinned libjpeg/Pillow and upstream corpora.

### JPG-007

Status at ledger review: **open**.

The README example now rejects every non-`Rgb8` source explicitly, but a complete target-mode table is still absent from rustdoc.

Next recorded action: Add a fixture-derived direct-mode table and link it from `encode` documentation.

### JPG-003

Status at ledger review: **open**.

Source color interpretation is implicit: JFIF, Adobe APP14 transforms, CMYK/YCCK, component IDs, and source/output colorspace are not one retained contract.

Next recorded action: Reverse-map Pillow and libjpeg-turbo cases, then preserve source interpretation separately from normalized output mode.

### JPG-008

Status at ledger review: **open**.

Decode cannot select luma, RGB, BGR, CMYK, YCbCr, or YCCK output even when avoiding a conversion would help the caller or encoder.

Next recorded action: Add only codec-native output layouts justified by exact fixtures; keep RGB/luma defaults Pillow-compatible.

### JPG-009

Status at ledger review: **open**.

Progressive JPEG is decoded as one completed raster; scans and rows cannot be consumed incrementally.

Next recorded action: Add scan/row incremental decoding only after API-023/024 limits and destination buffers exist.

### JPG-010

Status at ledger review: **open**.

Restart recovery, DNL, truncated final scans, fill bytes, and bytes after EOI lack an explicit strictness matrix.

Next recorded action: Generate one minimized file per parser decision and classify Pillow-compatible acceptance, warning, or structured failure.

### JPG-011

Status at ledger review: **open**.

MPO and other multi-picture JPEG containers are neither detected as a sequence nor explicitly rejected as a distinct capability.

Next recorded action: Determine Pillow's selected-frame and iteration behavior, then classify still-first and sequence operations separately.

### JPG-012

Status at ledger review: **open**.

Marker fragmentation and ordering are retained in raw stream order through the APPn/COM metadata records, but exhaustive multi-segment ICC, EXIF, XMP, comments, density, and Adobe collision fixtures are still missing.

Next recorded action: Add exact ordered-marker fixtures and collision rules before metadata round-tripping.

### JPG-013

Status at ledger review: **open**.

Gain-map/UltraHDR JPEG is currently ordinary JPEG with unrepresented auxiliary semantics.

Next recorded action: Keep it a P3 container-extension candidate until the common auxiliary-image and color metadata model exists.

### JPG-014

Status at ledger review: **open**.

Typed custom quantization/Huffman tables, restart units, progressive scan scripts, and arbitrary application segments are absent.

Next recorded action: Add each control only when Pillow or a pinned primary encoder supplies a deterministic oracle and validation rules.

### JPG-015

Status at ledger review: **open**.

Original sample precision, quantization-table precision, component sampling factors, table selectors, and scan structure are not returned by inspection. Two files that decode to the same `Rgb8` buffer can have materially different re-encode requirements.

Next recorded action: Add a source JPEG descriptor and exact marker/table fixtures without exposing a generic image-processing model.

### JPG-016

Status at ledger review: **open**.

Abbreviated JPEG datastreams that rely on externally supplied quantization or Huffman tables are neither recognized as a separate class nor rejected with a specific capability reason.

Next recorded action: Decide whether they are explicit-format-only input or always `Unsupported`; never inherit tables from ambient/global state.

### JPG-017

Status at ledger review: **open**.

4:4:0, 4:1:1, 4:1:0, asymmetric sampling, nonstandard component order/IDs, and more-than-three color components lack a decoded-output and support matrix.

Next recorded action: Generate minimal SOF/SOS cases through libjpeg-turbo and preserve the exact Pillow outcome plus source sampling descriptor.

### JPG-018

Status at ledger review: **open**.

The compatibility key `restart_interval` does not state whether its unit is MCUs, MCU rows, or restart blocks; Pillow exposes separate restart-block and restart-row controls.

Next recorded action: Replace it with typed units and reject simultaneous/conflicting settings before adding more fixtures.

### JPG-019

Status at ledger review: **open**.

Decoder output parity can change with IDCT method, chroma upsampling, SIMD path, compiler floating-point behavior, and unusual 4:4:0 handling even when the JPEG is legal.

Next recorded action: Name the exact reconstruction policy and compare scalar/native/WASM outputs on boundary coefficient and subsampling fixtures.

### JPG-020

Status at ledger review: **open**.

Decoded coefficient blocks and lossless JPEG-to-JPEG marker/table rewrites are unavailable. A caller must fully reconstruct pixels and incur another lossy generation.

Next recorded action: Keep coefficient-domain access P3, but classify it as codec work rather than promising lossless same-format output from ordinary `encode`.

### JPG-021

Status at ledger review: **open**.

Memory/work limits do not distinguish progressive scan count, coefficient storage, marker bytes, MCU count, or restart recovery work. Width/height limits alone would not bound these paths.

Next recorded action: Add JPEG sublimits and one minimized boundary fixture per independent work dimension.

### PNG-003

Status at ledger review: **open**.

Non-indexed `tRNS`, ICC, EXIF, gamma/chromaticity, text variants, physical dimensions, time, and newer color/HDR chunks are not represented on decode.

Next recorded action: Route raw chunks into the metadata model; do not add color management or orientation application.

### PNG-004

Status at ledger review: **open**.

Pillow 12.2.0 still accepts `I` source save, with a deprecation warning for Pillow 13.

Next recorded action: Freeze the pinned behavior in a fixture and decide whether compatibility outweighs a soon-removed oracle path.

### PNG-005

Status at ledger review: **open**.

Encoder always emits non-Adam7 PNG because pinned Pillow ignores the tested interlace option.

Next recorded action: Keep behavior, but document this as oracle compatibility rather than general PNG encoder capability.

### PNG-006

Status at ledger review: **open**.

Inspection scans through pre-IDAT chunks and validates selected CRCs; it is not a fixed 33-byte metadata read.

Next recorded action: Document complexity and add limits before advertising cheap inspection on arbitrary inputs.

### PNG-008

Status at ledger review: **open**.

Text and ICC payloads are not decompressed or parsed, and chunk count and per-chunk size limits are still absent. The total ancillary extent, including retained opaque blocks, is bounded by `max_metadata_bytes` when a caller sets it.

Next recorded action: Add explicit compressed-metadata and chunk-count limits under API-023 before semantic parsing.

### PNG-010

Status at ledger review: **open**.

`sBIT`, `cICP`, `mDCV`, `cLLI`, `iCCP`, `sRGB`, `gAMA`, and `cHRM` precedence is not represented.

Next recorded action: Preserve exact fields first and publish a precedence statement without performing color conversion.

### PNG-011

Status at ledger review: **open**.

Direct 16-bit LA/RGB/RGBA encode modes are absent even though the decoder observes those source depths.

Next recorded action: Add one mode at a time with big-endian sample fixtures and exact Pillow/reference output.

### PNG-012

Status at ledger review: **open**.

Decode remains whole-buffer, and PNG still encoding still prepares filtered rows and compressed output before delivery. The PNG still and one-frame sequence sink paths now stream the validated signature and chunk structures through multiple `OutputSink` writes, but row/pass APIs, Adam7, and compressed-output generation remain non-streaming.

Next recorded action: Layer row/pass and compressed-output APIs over shared codec state after limits and transfer layouts settle; extend structural delivery to the remaining PNG sequence/codec cases only with explicit partial-output and cancellation semantics.

### PNG-013

Status at ledger review: **open**.

Extra compressed streams, split IDAT edge cases, and zlib trailing data lack a named policy; bytes after IEND are covered by the resolved trailing-input contract.

Next recorded action: Add consumed/trailing-byte fixtures for the remaining edge cases under the documented trailing policy.

### PNG-015

Status at ledger review: **open**.

`ImagePalette` cannot distinguish an indexed PLTE from the optional suggested PLTE allowed for truecolor PNG, and it cannot represent `sPLT` palettes with more than 256 entries.

Next recorded action: Keep decoded index palettes in the pixel model and retain suggested palettes only as typed/opaque metadata.

### PNG-016

Status at ledger review: **open**.

The metadata backlog does not enumerate `bKGD`, `hIST`, `sPLT`, `oFFs`, `pCAL`, `sCAL`, `tIME`, text language/translated-keyword fields, and the exact placement rules for `eXIf`.

Next recorded action: Add a chunk-property ledger and preserve raw ordered bytes before interpreting any of these values.

### PNG-017

Status at ledger review: **open**.

Decode transformation policy is implicit. Packing expansion, `tRNS` expansion, alpha stripping/addition, 16-bit stripping/scaling, byte swapping, and `sBIT` normalization cannot be selected or discovered.

Next recorded action: Define only transfer transformations needed by byte-preserving codec use; default output remains pinned to Pillow.

### PNG-018

Status at ledger review: **open**.

Filter method/type validation and Adam7 pass reconstruction are covered by aggregate pixel results, but there is no property map for every filter on first/middle/last rows at each bytes-per-pixel class or for every empty Adam7 pass.

Next recorded action: Add generated minimal witnesses and record the filter/pass property, not merely another PNG row.

### PNG-019

Status at ledger review: **open**.

Encoder controls do not type adaptive versus fixed filters, DEFLATE strategy/window choices, IDAT chunk sizing, or the interaction of `optimize`, level, type, and dictionary.

Next recorded action: Inventory Pillow's exact behavior; reject a preset dictionary if it would produce a nonconforming standard PNG stream.

### PNG-020

Status at ledger review: **open**.

A stream can end after enough bytes to display a frame but before trailing chunks and IEND are validated. The API has no separate “frame available” versus “datastream finished” state.

Next recorded action: Make incremental frame success provisional until an explicit finish operation validates the remainder.

### GIF-002

Status at ledger review: **open**.

Pillow accepts `1`, `LA`, `I`, `F`, and `I;16` source saves through private conversion; Rust rejects them.

Next recorded action: Add codec-local conversions only where exact output is required.

### GIF-005

Status at ledger review: **open**.

Exact rational storage is implemented, but the public encode contract for a valid duration that is not an exact centisecond has only defensive-model coverage.

Next recorded action: Add a caller-built manifest transform proving the structured `Unsupported` result and document that GIF encoding never rounds timing.

### GIF-006

Status at ledger review: **open**.

Quantization behavior is exact only for retained images, not the full source-mode/color distribution space.

Next recorded action: Add small reverse-mapped palette boundary fixtures before optimizing quantizer performance.

### GIF-007

Status at ledger review: **open**.

The GIF user-input flag has no field in `DecodedFrame`, so a valid control-extension bit is silently lost.

Next recorded action: Add a frame presentation flag with exact Pillow/spec reference evidence.

### GIF-009

Status at ledger review: **open**.

Frame iteration is eager and allocates one owned raster per frame; callers cannot reuse one output buffer.

Next recorded action: Add a bounded streaming iterator after API-027 and prove disposal/compositing state across partial iteration.

### GIF-010

Status at ledger review: **open**.

Extension order, multiple comments/application blocks, sub-block boundaries, and unknown extension payloads cannot round-trip.

Next recorded action: Preserve ordered raw extensions with exact limits and collision rules.

### GIF-011

Status at ledger review: **open**.

There is no frame-count, cumulative pixel, extension-byte, LZW-work, or total sequence-memory limit.

Next recorded action: Add one fixture per limit and distinguish limit exhaustion from malformed LZW.

### GIF-012

Status at ledger review: **open**.

Quantizer, dither, palette reuse, transparency index, disposal optimization, and interlace choices are not typed controls.

Next recorded action: Mirror only deterministic Pillow 12.2.0 choices that can be isolated into exact fixtures.

### GIF-014

Status at ledger review: **open**.

Header version (`87a`/`89a`), logical-screen color resolution, sort flag, pixel aspect ratio, and global-table size bits are parsed only as needed or discarded.

Next recorded action: Add a source stream descriptor and prove whether each field is retained, normalized, or intentionally ignored.

### GIF-015

Status at ledger review: **open**.

Plain Text Extensions are rendering blocks and can be associated with a Graphic Control Extension, but the decoder treats extension data only as ignorable/opaque structure.

Next recorded action: Classify plain-text presentation as unsupported retained metadata; do not rasterize text in this crate.

### GIF-016

Status at ledger review: **open**.

NETSCAPE2.0, ANIMEXTS1.0, repeated loop extensions, malformed sub-blocks, and conflicts between application extensions lack a precedence/round-trip policy.

Next recorded action: Preserve ordered application blocks and separately derive one normalized loop value with an explicit conflict diagnostic.

### GIF-017

Status at ledger review: **open**.

Global versus local palette scope is not explicit in the common sequence. A background index names the logical-screen global table even when frames use unrelated local tables.

Next recorded action: Add palette ownership identifiers before generic background or per-frame palette re-encoding.

### GIF-018

Status at ledger review: **open**.

LZW evidence needs a property matrix for minimum-code-size anomalies, clear at every code width, early/late width growth, dictionary saturation, repeated clear, missing EOI, sub-block termination, and pixels beyond the frame rectangle.

Next recorded action: Keep one minimized Pillow-observed case per state transition and one defensive case where Pillow cannot expose the branch.

### GIF-019

Status at ledger review: **open**.

Pillow's frame optimizer may crop deltas, coalesce identical frames, accumulate their duration, select transparency fills, and change local/global palette use. The public options do not say whether exact source frames or optimized presentation is requested.

Next recorded action: Add a typed frame-optimization policy and assert frame count, rectangles, duration, palette scope, and exact bytes.

### GIF-020

Status at ledger review: **open**.

A second GIF stream, multiple trailers, and an extension after the trailer have no consumed-input policy; a single trailing payload is covered by the resolved trailing-input contract.

Next recorded action: Add concatenated-stream fixtures under the documented trailing policy.

### GIF-021

Status at ledger review: **open**.

Layout-specific evidence now distinguishes GIF source rectangles from Pillow's rendered presentation, but exact raw source-rectangle sample bytes are not independently asserted.

Next recorded action: Add a format-structural raw-frame oracle before claiming byte-exact source-frame reconstruction.

### BMP-001

Status at ledger review: **open**.

`ImageFormat::Bmp` detects only `BM` files; Pillow also exposes headerless DIB as a related format and `.dib` alias.

Next recorded action: Decide whether DIB belongs under BMP or remains explicitly out of scope; do not silently treat extension support as byte detection.

### BMP-002

Status at ledger review: **open**.

DPI, color masks/profile data, palette metadata, and header variant identity are not exposed.

Next recorded action: Preserve fields only when a caller can round-trip them without image processing.

### BMP-003

Status at ledger review: **open**.

Alpha interpretation varies across BMP headers and Pillow's `USE_RAW_ALPHA` behavior; current evidence covers only the retained manifest cases.

Next recorded action: Add explicit alpha-policy fixtures for 32-bit BI_RGB and bitfield variants.

### BMP-004

Status at ledger review: **open**.

Encoder deliberately ignores compression/top-down/header requests to match Pillow cases.

Next recorded action: Keep the exact behavior, but make ignored options discoverable and target-specific rather than accepting arbitrary keys.

### BMP-005

Status at ledger review: **open**.

No independent upstream corpus is tracked for OS/2, embedded profiles, and rare bitfield layouts.

Next recorded action: Import only small licensed cases with checksums and Pillow outcomes.

### BMP-006

Status at ledger review: **open**.

`BI_JPEG` and `BI_PNG` embedded payloads are not classified, recursively limited, or exposed as delegated container data.

Next recorded action: Decide explicit `Unsupported` versus bounded delegation; never recurse without depth and cumulative-byte limits.

### BMP-007

Status at ledger review: **open**.

Decode always creates tightly packed output and cannot target a caller row stride, despite BMP's padded and bottom-up row organization.

Next recorded action: Use the common transfer-layout/destination API; do not add cropping or other processing.

### BMP-008

Status at ledger review: **open**.

V4/V5 endpoints, gamma, color-space type, profile offset/size, and rendering intent are discarded.

Next recorded action: Retain exact header/profile data under API-034/040 before semantic interpretation.

### BMP-009

Status at ledger review: **open**.

RLE delta moves, absolute-run padding, early EOL/EOB, top-down restrictions, and trailing compressed bytes need a strictness matrix.

Next recorded action: Reverse-map each branch to a minimal Pillow fixture and retain defensive cases separately.

### BMP-010

Status at ledger review: **open**.

ICO DIB XOR alpha, AND masks, 32-bit source alpha, and palette transparency interact in under-specified ways.

Next recorded action: Add cross-product fixtures shared by BMP and ICO before changing either decoder.

### BMP-011

Status at ledger review: **open**.

Headerless DIB has no explicit input/output type even though it is a distinct useful byte contract.

Next recorded action: Keep it out of `detect_format` and the signature-validated common explicit-format API unless an unambiguous DIB contract is added; consider a separate DIB API.

### BMP-012

Status at ledger review: **open**.

Related OS/2 bitmap-array/icon/pointer signatures (`BA`, `CI`, `CP`, `IC`, `PT`) and Windows `BM` are not one interchangeable format, but the support boundary is not documented.

Next recorded action: Keep automatic BMP detection at `BM`; explicitly classify the related signatures and require a separate container decision before adding any.

### BMP-013

Status at ledger review: **open**.

Header sizes 12, 16, 40, 52, 56, 64, 108, and 124 have overlapping but non-identical field layouts. Header identity and unsupported intermediate variants are not exposed.

Next recorded action: Build a header-generation matrix and return a typed variant from inspection.

### BMP-014

Status at ledger review: **open**.

Bitfield masks lack an explicit validity contract for zero, overlap, non-contiguous bits, bits outside depth, alpha overlap, and default masks when absent.

Next recorded action: Reverse-map Pillow acceptance and retain normalized channel extraction separately from exact source masks.

### BMP-015

Status at ledger review: **open**.

File size, pixel offset, DIB size, palette/mask/profile ranges, `SizeImage`, and actual payload length can disagree or overlap. Current cases do not form one precedence and trailing-byte policy.

Next recorded action: Generate structural combinations without large rasters and attach byte-range context to failures.

### BMP-016

Status at ledger review: **open**.

Palette entries are RGB triples for core headers and RGB quads for later headers; `ClrUsed`, `ClrImportant`, implicit table size, and gap bytes can disagree.

Next recorded action: Preserve table encoding and declared counts separately from the decoded RGB palette.

### BMP-017

Status at ledger review: **open**.

OS/2 Huffman 1D, RLE24, embedded JPEG, and embedded PNG compression codes are not individually classified.

Next recorded action: Give each a stable `Unsupported` capability code; bounded delegated decode requires an explicit recursion budget.

### BMP-018

Status at ledger review: **open**.

A V5 linked profile contains a Windows-encoded filename, while an embedded profile contains bytes. Treating both as one ICC blob would be wrong and could invite unintended I/O.

Next recorded action: Retain linked profile names as opaque metadata only; this crate must never open them.

### BMP-019

Status at ledger review: **open**.

Negative height is legal only for selected uncompressed/bitfield layouts, while core headers use unsigned dimensions. Negative width, minimum signed values, and top-down RLE need exact rejection rules.

Next recorded action: Add checked structural fixtures around every signed boundary.

### BMP-020

Status at ledger review: **open**.

Encoder output fixes one header generation and cannot state whether source alpha, masks, color-space fields, or profile data are preserved versus normalized away.

Next recorded action: Publish one exact output-profile descriptor per BMP encode path before adding alternative headers.

### TIF-002

Status at ledger review: **open**.

BigTIFF signatures are detected, but complete decode/encode capability is not provided.

Next recorded action: Add explicit BigTIFF success/error fixtures and capability reporting.

### TIF-003

Status at ledger review: **open**.

`P8` and YCbCr inputs accepted by Pillow cannot be encoded from the current model.

Next recorded action: Add palette TIFF first; add a YCbCr transfer mode only with exact byte requirements.

### TIF-005

Status at ledger review: **open**.

Compressed output writes dimensions as TIFF SHORT and therefore imposes an artificial 65,535 ceiling where LONG is legal.

Next recorded action: Reverse-map Pillow output for larger dimensions and select field types without allocating impossible fixtures.

### TIF-006

Status at ledger review: **open**.

Each encoded page is single-strip, chunky, classic TIFF only; there is no tiled, planar, BigTIFF, palette, JPEG/Fax/Zstd/LZMA/WebP compression output.

Next recorded action: Add only formats Pillow 12.2.0 actually exercises and the dependency-free implementation can support on WASM.

### TIF-007

Status at ledger review: **open**.

Orientation, resolution, ICC/EXIF/GPS, arbitrary tags, extra-sample association, and sub-IFDs are not retained.

Next recorded action: Design opaque IFD/tag retention with collision rules; never apply orientation in this crate.

### TIF-008

Status at ledger review: **open**.

Horizontal predictor handling for signed/float samples and unusual bit depths lacks an explicit support matrix.

Next recorded action: Use libtiff/Pillow fixtures for predictor × sample-format × endianness before generalizing.

### TIF-009

Status at ledger review: **open**.

Frame counting scans IFD chains and can return `is_animated=true` with unknown `frame_count`; ordinary still decode returns page one while sequence decode attempts the later IFD and returns its structured failure.

Next recorded action: Document the incomplete-count state and bind it to limits/capabilities.

### TIF-010

Status at ledger review: **open**.

Arbitrary sample counts, mixed bit depths, and non-RGB extra channels cannot be represented by `ImageMode`.

Next recorded action: Define a TIFF-native transfer layout only if opaque extra samples must survive; do not force them into RGBA.

### TIF-011

Status at ledger review: **open**.

Associated and unassociated alpha (`ExtraSamples`) are not distinguished in decoded output.

Next recorded action: Add source alpha semantics and prove whether Pillow unpremultiplies, preserves, or drops each case.

### TIF-012

Status at ledger review: **open**.

Fill order, photometric inversion, sample format, and per-channel depth are only partially visible after normalization; source byte order is now retained.

Next recorded action: Expand `SourceDescriptor` one independently proved field at a time while leaving decoded transfer bytes unchanged.

### TIF-013

Status at ledger review: **open**.

There is no bounded page iterator, page selection, or random-access IFD handle.

Next recorded action: Implement API-027 with IFD-cycle detection and preserve per-page dimensions/mode/metadata.

### TIF-014

Status at ledger review: **open**.

SubIFDs, pyramids, thumbnails, masks, and directory graphs are flattened or ignored.

Next recorded action: Model relationships only when a fixture needs them; distinguish primary pages from auxiliary directories.

### TIF-016

Status at ledger review: **open**.

Strip/tile decode cannot stream into a caller buffer, and encoding cannot incrementally write strips or tiles.

Next recorded action: Add bounded chunk APIs after API-024/025; never require full multipage materialization.

### TIF-017

Status at ledger review: **open**.

IFD cycles/depth, tag counts, strip/tile counts, offset arrays, decompressed bytes, and predictor work have no caller policy.

Next recorded action: Add typed TIFF sublimits and minimized cycle/overflow/exhaustion fixtures.

### TIF-018

Status at ledger review: **open**.

Sparse 64-bit offsets, BigTIFF count/offset boundaries, and host `usize` conversion are not exercised across 32-bit/WASM targets.

Next recorded action: Use generated sparse/structural inputs and target-specific checked arithmetic tests without committing huge files.

### TIF-020

Status at ledger review: **open**.

Photometric support is not catalogued for WhiteIsZero, BlackIsZero, RGB, Palette, Transparency Mask, Separated, YCbCr, CIELAB/ICCLAB/ITULAB, LogL/LogLuv, and CFA classes.

Next recorded action: Generate a source photometric capability table; do not coerce unknown extra channels into RGBA.

### TIF-021

Status at ledger review: **open**.

Compression identity is broader than “compressed”: CCITT variants/options, old/new JPEG, LZW, Deflate/Adobe Deflate, PackBits, PixarLog, SGILog, LZMA, Zstd, LERC, WebP, and vendor values need separate decode/encode capabilities.

Next recorded action: Give every observed compression a stable capability code and prevent feature-gated delegated codecs from being inferred accidentally.

### TIF-022

Status at ledger review: **open**.

The common metadata model cannot retain TIFF field type and count. BYTE/ASCII/SHORT/LONG/RATIONAL, signed variants, FLOAT/DOUBLE, IFD, LONG8/SLONG8/IFD8, inline values, and offset values can carry byte-distinct but numerically similar data.

Next recorded action: Add a typed raw tag record with source byte order, exact count, and exact bytes.

### TIF-023

Status at ledger review: **open**.

YCbCr decode behavior also depends on coefficients, subsampling, positioning, and ReferenceBlackWhite, not only `PhotometricInterpretation`.

Next recorded action: Add a complete YCbCr tag cross-product before exposing a YCbCr transfer mode or claiming exact RGB reconstruction.

### TIF-024

Status at ledger review: **open**.

Old-style JPEG-in-TIFF, new JPEG compression, shared `JPEGTables`, per-strip tables, restart boundaries, and abbreviated JPEG streams are not one path.

Next recorded action: Treat embedded JPEG state as a bounded TIFF-owned codec contract with cumulative limits and independent fixtures.

### TIF-025

Status at ledger review: **open**.

Strips/tiles can overlap, alias, leave gaps, appear out of order, extend past input, or disagree with dimensions and byte counts. The acceptance policy is not explicit.

Next recorded action: Add structural offset/count fixtures and define whether identical aliased ranges are accepted, copied once, or rejected.

### TIF-026

Status at ledger review: **open**.

Missing/zero `StripByteCounts` or `TileByteCounts`, one-strip inference, and last-strip truncation have implementation-specific recovery rules.

Next recorded action: Reverse-map Pillow/libtiff behavior and emit a diagnostic when inference preserves a usable image.

### TIF-027

Status at ledger review: **open**.

Floating-point predictor 3 has byte-plane shuffling semantics distinct from horizontal predictor 2 and depends on sample width and byte order.

Next recorded action: Add exact 16/24/32/64-bit float predictor vectors before advertising float-predictor support.

### TIF-028

Status at ledger review: **open**.

Primary pages, reduced images, masks, SubIFDs, EXIF/GPS IFDs, thumbnails, pyramids, and arbitrary directory links need relationship types, not one flat frame list.

Next recorded action: Introduce directory roles and bounded graph traversal before exposing more than the primary IFD chain.

### TIF-029

Status at ledger review: **open**.

Multipage decode retains each page's dimensions, mode, palette, exact bytes, and source byte order, but not its photometric interpretation, sample type, metadata, compression, or page-indexed error stage.

Next recorded action: Extend the per-page source descriptor and add a stable page index/stage to later-page failures.

### TIF-030

Status at ledger review: **open**.

Edge tiles and final strips have stored padding versus visible dimensions; a caller-buffer API needs to say whether padding bytes are consumed, returned, zeroed, or ignored.

Next recorded action: Make visible and storage extents explicit in the chunk-layout contract.

### WEP-003

Status at ledger review: **open**.

Animation decode now returns full rendered canvases while separately retaining exact ANMF rectangles, blend, and disposal. It still does not expose the raw nested frame bitstream needed for exact container reconstruction.

Next recorded action: Add the bounded demux/source-frame view described by WEP-011 without changing rendered decode semantics.

### WEP-004

Status at ledger review: **open**.

Pillow mode normalization is much broader than the current encoder.

Next recorded action: Add `LA` first, then integer/float/16-bit/YCbCr only as fixture-backed private conversions.

### WEP-007

Status at ledger review: **open**.

Encoder and decoder are among the largest source areas. The schema-`@3` fixture benchmark protocol records revision/hash-bound parity and Rust-only suite timings with a fixed four-worker test budget, native/WASM compile artifact sizes, and direct-child POSIX CPU/peak-RSS observations; the parity harness shares immutable source-fixture decodes across its partitioned test functions. It still does not measure codec-specific output-size, allocator count, retained-cache size, or runtime-WASM behavior.

Next recorded action: Extend the fixed lossy/lossless/alpha/animation workload set with output-size, allocator/cache, and runtime-WASM measurements; compare repeated same-host revisions before making a universal performance claim.

### WEP-001

Status at ledger review: **open**.

The API does not retain whether a source was VP8, VP8L, or extended WebP, nor expose intrinsic alpha/animation/container flags separately from normalized mode.

Next recorded action: Add source encoding properties to inspection without leaking internal decoder state.

### WEP-005

Status at ledger review: **open**.

Transparent RGB and straight-versus-premultiplied alpha behavior is not a named contract across lossy, lossless, and animation paths.

Next recorded action: Add exact invisible-RGB and alpha-edge fixtures before any optimization changes.

### WEP-008

Status at ledger review: **open**.

Near-lossless, alpha quality/filter, exact transparent RGB, presets, target size/PSNR, SNS, filtering, partitions, and sharp-YUV controls are untyped or absent.

Next recorded action: Compare Pillow 12.2.0 and pinned libwebp 1.6.0; expose only deterministic options that the in-tree encoder implements.

### WEP-009

Status at ledger review: **open**.

Decoder cannot output BGR(A), premultiplied layouts, or caller-provided YUV/RGB planes/buffers.

Next recorded action: Add transfer layouts only when they avoid measured copies or enable exact codec-native handoff.

### WEP-010

Status at ledger review: **open**.

No incremental VP8/VP8L decode accepts partial input or emits completed rows.

Next recorded action: Add after common input limits and destination contracts; preserve the whole-slice wrapper.

### WEP-011

Status at ledger review: **open**.

Callers cannot inspect raw ANMF rectangles/bitstreams or enumerate RIFF chunks without full compositing.

Next recorded action: Define a bounded demux view separate from rendered sequence decode.

### WEP-012

Status at ledger review: **open**.

Unknown RIFF chunks, duplicate metadata chunks, and chunk order are retained raw (padding normalized away), but declared-size mismatch (truncated chunks) has no strictness policy beyond skipping retention.

Next recorded action: Build ordered-container fixtures and align malformed-size outcomes with API-040.

### WEP-013

Status at ledger review: **open**.

Per-frame dimensions are bounded, but cumulative animation pixels, duration, frame count, metadata, and decode work have no caller limits.

Next recorded action: Add WebP-specific sequence limits and overflow fixtures.

### WEP-014

Status at ledger review: **open**.

The initial keyframe encoder supports exact per-frame durations, RGBA background, loop, and forced `kmax=1`, but rejects `minimize_size`, `kmin`, general `kmax`, `allow_mixed`, and alpha-quality/optimization controls.

Next recorded action: Add each optimization only with exact sequence bytes, frame metadata, and invalid interaction fixtures.

### WEP-015

Status at ledger review: **open**.

VP8X feature flags, reserved bits, canvas size, and actual ICCP/ALPH/EXIF/XMP/ANIM chunks can disagree. Current inspection does not expose a normalized-versus-declared consistency report.

Next recorded action: Add one fixture per mismatch and preserve both declared flags and observed chunks.

### WEP-016

Status at ledger review: **open**.

RIFF's 32-bit size, WebP's approximately 4-GiB container ceiling, 24-bit VP8X canvas fields, and the distinct VP8/VP8L dimension bounds are not preflighted through one public limit/capability contract.

Next recorded action: Add checked per-subtype size preflight before allocating or encoding.

### WEP-017

Status at ledger review: **open**.

ANMF offsets are stored in half-pixel units, rectangles must fit the canvas, duration is 24-bit milliseconds, reserved bits must be zero, and the payload must contain a legal ALPH+VP8 or VP8L frame form.

Next recorded action: Build exact boundary fixtures and retain raw frame sub-bitstream identity in the demux view.

### WEP-018

Status at ledger review: **open**.

The ALPH chunk has compression, filtering, preprocessing, and reserved fields; raw and VP8L-compressed alpha are only two outcomes of a broader header contract.

Next recorded action: Add all legal filter/preprocessing values and illegal reserved combinations before claiming complete alpha decode.

### WEP-019

Status at ledger review: **open**.

A partial WebP demux can know the canvas and N frames while the last frame remains incomplete. Whole-slice APIs collapse this into success or malformed without progress.

Next recorded action: Reuse API-043/047 and expose a partial-frame state only in the future streaming demux.

### WEP-020

Status at ledger review: **open**.

libwebp animation decode reports cumulative timestamps, while the common model stores individual durations. Rounding, zero-duration frames, overflow, and final timestamp must be proved separately.

Next recorded action: Assert both exact source duration and checked cumulative presentation time.

### WEP-021

Status at ledger review: **open**.

Lossy pixel output depends on fancy upsampling, filtering, dithering, cropping/scaling options, and premultiplied/output layout choices in common decoders. Only one implicit reconstruction policy is tested.

Next recorded action: Freeze the Pillow-compatible reconstruction path, compare scalar/WASM behavior, and expose alternatives only when they are codec transfer choices rather than processing.

### WEP-022

Status at ledger review: **open**.

VP8L aggregate fixtures do not yet prove each transform combination, meta-prefix group, color-cache boundary, simple/full Huffman tree form, distance mapping, and entropy-image dimension. The revision-pinned property map names 14 properties (13 witnessed properties—frame headers, color-indexing bands, subtract-green, color transforms, meta-Huffman groups, entropy-image dimensions, successful cache boundaries, simple/full Huffman trees, distance mappings, and three malformed-form scopes—and one broad predictor-mode candidate) and 68 active Pillow witnesses, with 46 successful structural facts (including predictor modes 0–13) and 40 malformed parser code/phase/bit-offset witnesses independently checked across 79 distinct active WebP rows; the Pillow rows still claim only outer results.

Next recorded action: Expand the successful structural inspector to every claimed combination, then promote candidates one property at a time without changing their Pillow-origin boundary.

### ICO-001

Status at ledger review: **open**.

Decoder exposes only the selected largest entry; callers cannot inspect/select/enumerate every stored size.

Next recorded action: Add an entry-oriented decode/inspect API whose entries are already-sized images, not an image-processing resize request.

### ICO-002

Status at ledger review: **open**.

Encoder cannot accept multiple already-sized caller entries. Pillow can save multiple entries and replacement images.

Next recorded action: Add an ICO directory model and exact ordering/selection fixtures without generating resized pixels.

### ICO-004

Status at ledger review: **open**.

Default PNG-backed path inherits PNG's accepted-mode limits; BMP-backed path accepts only RGB/RGBA.

Next recorded action: Publish separate entry-backend mode capabilities and fixture every accepted combination.

### ICO-005

Status at ledger review: **open**.

`ico` transitively enables full `png` and `bmp` features, so it is not an isolated compiled slice.

Next recorded action: Retain correctness first; measure binary impact and consider private embedded-entry capability features only if Cargo's additive rules remain clear.

### ICO-006

Status at ledger review: **open**.

Directory color count, planes, bit depth, duplicate sizes, and tie-breaking are only manifest-bounded.

Next recorded action: Add entry-directory edge cases before claiming complete ICO container support.

### ICO-007

Status at ledger review: **open**.

CUR hotspot retention covers only the selected entry; every unselected directory entry's hotspot remains inaccessible.

Next recorded action: Store hotspot per enumerated entry under the entry-oriented model.

### ICO-008

Status at ledger review: **open**.

Callers cannot inspect each entry's embedded format, stored dimensions, color count, planes/hotspot, bit depth, byte range, and source mode.

Next recorded action: Add bounded directory metadata without decoding every payload.

### ICO-009

Status at ledger review: **open**.

Entry selection is implicit. There is no exact index/size/bit-depth selection or way to distinguish a fallback from an exact match.

Next recorded action: Define deterministic selection queries and retain the current default as documented convenience behavior.

### ICO-010

Status at ledger review: **open**.

Duplicate sizes, malformed high-ranked entries, tie ordering, and fallback to a lower-ranked valid entry need explicit rules.

Next recorded action: Use minimized multi-entry fixtures and assert both selected index and error behavior.

### ICO-011

Status at ledger review: **open**.

Encoder cannot emit a mixed PNG/DIB multi-entry file from already-sized caller images.

Next recorded action: Add a directory encoder with per-entry backend/options; never resize caller images.

### ICO-012

Status at ledger review: **open**.

Directory width/height byte zero means 256, while embedded headers can disagree; zero/overflow and payload-range edges are not exhaustively asserted.

Next recorded action: Add structural fixtures before exposing entry enumeration as stable API.

### ICO-013

Status at ledger review: **open**.

A set-of-sizes view loses directory order, duplicate sizes, color-depth variants, selected index, and exact tie-break information.

Next recorded action: Expose an ordered entry list; derive a convenience set only as a lossy query.

### ICO-014

Status at ledger review: **open**.

Entry payload ranges can overlap, alias exactly, point into the directory, repeat one payload, or run past input. No policy states whether shared exact ranges are legal.

Next recorded action: Add byte-range validation and fixtures for every overlap class before zero-copy entry views.

### ICO-015

Status at ledger review: **open**.

Reserved word, resource type, count, per-entry reserved byte, planes/color-count, and CUR hotspot fields need independent validation. One invalid entry should not silently redefine the whole container.

Next recorded action: Return directory-level versus entry-indexed errors and define whether valid lower-ranked entries remain selectable.

### ICO-016

Status at ledger review: **open**.

Width/height byte zero, color-count zero, planes zero, bit-depth zero, and DIB/PNG-derived values are sentinels rather than ordinary numeric zero.

Next recorded action: Retain declared and derived values separately and test every sentinel.

### ICO-017

Status at ledger review: **open**.

DIB entry height represents XOR plus AND planes, mask rows have independent padding, and 32-bit alpha may suppress or combine with the AND mask. Truncated/mismatched halves need exact rules.

Next recorded action: Add XOR-depth × source-alpha × AND-mask × row-padding fixtures shared with BMP-010.

### ICO-018

Status at ledger review: **open**.

Directory dimensions/bit depth can disagree with embedded PNG IHDR or DIB headers. Current best-entry logic needs a documented trust/validation order.

Next recorded action: Retain both declarations, reject impossible ranges, and assert the selected effective dimensions.

### ICO-019

Status at ledger review: **open**.

The same two directory words mean planes/bit depth for ICO and x/y hotspot for CUR. A shared entry type must not expose both interpretations simultaneously.

Next recorded action: Use a tagged ICO/CUR entry descriptor with exact raw words.

### ICO-020

Status at ledger review: **open**.

Pillow sorts/deduplicates requested output sizes and can choose among appended images with equal size but different depth. A future multi-entry encoder must state ordering and duplicate policy rather than inheriting set semantics accidentally.

Next recorded action: Preserve caller order by default; add an explicit compatibility policy only if exact Pillow bytes require sorting/deduplication.

### ICO-021

Status at ledger review: **open**.

Selected-entry decode is all-or-nothing; callers cannot inspect a valid directory, skip one malformed entry, and decode another exact index with an entry-scoped error.

Next recorded action: Separate bounded directory parse from entry payload decode and attach index/range/backend to errors.

### AVF-001

Status at ledger review: **open**.

Portable AV1 still decode is a closed subset; sequence decode and all encode are unavailable on WASM.

Next recorded action: Complete the existing AVIF plan before treating the feature as target-invariant.

### AVF-003

Status at ledger review: **open**.

Native encode supports fewer Pillow source modes.

Next recorded action: Add exact private normalization only after portable encode architecture is selected, so work is not duplicated around FFI.

### AVF-004

Status at ledger review: **open**.

AVIF primary `prof`/`rICC` ICC profiles and recognized `Exif`/XMP item payloads are now retained on decode; EXIF remains raw and includes the stored AVIF TIFF-offset prefix. Direct alpha and supported grid-derived alpha items report `SourceAlpha::Auxiliary` plus source-local `auxl` relationships, bounded `prem` relationships retain the source's premultiplication declaration without changing normalized decoded samples, typed non-primary `colr`/`nclx` CICP declarations retain source-local item identity, non-primary `prof`/`rICC` profiles retain exact raw bytes through `SourceDescriptor::avif_item_icc_profiles()`, unknown and known associated non-primary `clli`/`mdcv`/`irot`/`imir`/`pasp`/`clap` properties retain exact source-local item ID/kind/payload through `SourceDescriptor::avif_item_properties()`, known non-primary/auxiliary `ispe`/`pixi` declarations retain source-local item ID, dimensions, and uniform channel depth through `SourceDescriptor::avif_item_plane_properties()`, and non-primary/auxiliary `av1C` declarations retain source-local item ID, exact payload, declared bit depth, and chroma sample position through `SourceDescriptor::avif_item_codec_properties()` without interpretation. Exact straight-versus-premultiplied sample semantics, plane range/quality, and composition beyond that normalized boundary remain open.

Next recorded action: Fold the remaining known non-alpha/auxiliary metadata into API-019 and define any additional straight-versus-premultiplied decoded-sample semantics.

### AVF-005

Status at ledger review: **open**.

Sequence encoder rejects offsets and requires every frame to match one canvas; loop semantics are not represented in AVIF output.

Next recorded action: Compare pinned Pillow/libavif sequence behavior and state unsupported properties explicitly.

### AVF-006

Status at ledger review: **open**.

AV1 support is validated by many narrow reverse-mapped fixtures, not the AV1 bitstream specification or conformance suite.

Next recorded action: Continue slice-by-slice first-divergence work, then add licensed libavif/AOM corpus classes with independent references.

### AVF-008

Status at ledger review: **open**.

Portable transfer is 8-bit normalized output; 10/12-bit samples, monochrome, planar YUV, and high-depth alpha cannot be retained directly.

Next recorded action: Add exact source descriptors and one transfer layout at a time after portable AV1 correctness.

### AVF-009

Status at ledger review: **open**.

Primary-item CICP primaries/transfer/matrix and range, primary-item `av1C` chroma sample position, primary-item `clli` maxCLL/maxPALL, primary-item `mdcv` mastering-display fields, and primary-item `prof`/`rICC` ICC profile bytes now retain through `SourceColor`; typed non-primary/auxiliary `colr`/`nclx` CICP declarations retain source-local item IDs through `SourceDescriptor::avif_item_color_properties()` without merging into the primary color result, non-primary/auxiliary `prof`/`rICC` profiles retain exact raw profile data through `SourceDescriptor::avif_item_icc_profiles()`, unknown and known associated non-primary `clli`/`mdcv`/`irot`/`imir`/`pasp`/`clap` properties retain exact raw source records through `SourceDescriptor::avif_item_properties()`, known non-primary/auxiliary `ispe`/`pixi` declarations retain source-local dimensions and uniform channel depth through `SourceDescriptor::avif_item_plane_properties()`, and non-primary/auxiliary `av1C` declarations retain source-local item IDs, exact payloads, declared bit depths, and chroma sample positions through `SourceDescriptor::avif_item_codec_properties()`. Other known non-primary/auxiliary item-color forms, plane range/quality, and color transforms remain absent.

Next recorded action: Preserve the remaining exact item/property fields without applying tone or color transforms.

### AVF-011

Status at ledger review: **open**.

Grid payload version/flags, row and column counts, and declared output canvas are now retained for the supported primary `grid` item through `SourceDescriptor::avif_grid_properties()`, alongside the bounded `dimg` child list, generic `iref` edges including filtered `prem` relationships, alpha relationships from `grid.avif` to its derived color items, and typed non-primary `colr`/`nclx` declarations. Tile placement/composition, layered/progressive images, sample transforms, and most alternative item relationships still have no composable image model.

Next recorded action: Classify each remaining graph or composition feature as decoded still, auxiliary structure, or explicit `Unsupported` with libavif fixtures and bounded graph traversal.

### AVF-012

Status at ledger review: **open**.

Gain maps, auxiliary depth, thumbnails, and supplementary images cannot be enumerated or associated with the primary image. Direct alpha and the supported grid-derived alpha links are exposed as source-local item IDs through the scalar and plural `SourceDescriptor` getters, the supported primary grid exposes its ordered derived item IDs, bounded `iref` edges including `prem` are retained, typed non-primary CICP declarations retain their item IDs, non-primary/auxiliary `ispe`/`pixi` dimensions/depth are retained through `SourceDescriptor::avif_item_plane_properties()`, and non-primary/auxiliary `av1C` payload/depth/chroma declarations are retained through `SourceDescriptor::avif_item_codec_properties()`; payload selection and broader auxiliary graphs remain inaccessible.

Next recorded action: Extend the relationship model for non-alpha, derived, grid, and supplementary content only after fixture-backed use cases; never flatten it silently into RGBA.

### AVF-013

Status at ledger review: **open**.

Sequence timing uses integer milliseconds and cannot retain exact timescale/duration, repetition, edit lists, or sample timing.

Next recorded action: Replace timing through API-009 before claiming exact animated AVIF container parity.

### AVF-014

Status at ledger review: **open**.

Item/property/reference counts, box depth/size, grid dimensions, sample count, cumulative decoded bytes, and AV1 work have no caller limits.

Next recorded action: Add BMFF and AV1 sublimits with independently identified failure context.

### AVF-015

Status at ledger review: **open**.

Portable encode lacks typed codec, speed, thread, tile, quantizer, chroma, range, tune, and lossless controls matching the native oracle bridge.

Next recorded action: Freeze required Pillow/libavif behaviors, then implement only dependency-free, deterministic controls.

### AVF-016

Status at ledger review: **open**.

Unknown top-level boxes and free/skip padding are retained raw in scan order, recognized EXIF/XMP item payloads are retained in item order, unknown and known associated non-primary `clli`/`mdcv`/`irot`/`imir`/`pasp`/`clap` properties retain exact source-local kind/payload records through `SourceDescriptor::avif_item_properties()`, known non-primary/auxiliary `ispe`/`pixi` declarations retain typed source-local dimensions/depth through `SourceDescriptor::avif_item_plane_properties()`, and non-primary/auxiliary `av1C` declarations retain exact source-local payloads plus typed depth/chroma fields through `SourceDescriptor::avif_item_codec_properties()`. The portable and inspection parsers independently reject an `ipco` table after 2,048 property entries and a FileTypeBox after 1,024 compatible-brand entries. Compatible-brand declaration retention is now governed by `SourceDescriptor::avif_file_type()`; remaining known item properties and broader item/property graph limits are not.

Next recorded action: Keep the property-table and compatible-brand ceilings separate from box, association, and graph budgets; use ordered opaque preservation under API-040 for remaining item-level boxes/properties and add the other graph limits independently.

### AVF-018

Status at ledger review: **open**.

AV1 film grain, operating points, spatial layers, scalability, and still-picture/profile constraints have no capability statement.

Next recorded action: Inventory the portable syntax subset against the pinned AOM/libavif corpus before adding syntax.

### AVF-019

Status at ledger review: **open**.

AVIF input/output cannot use caller-owned YUV/alpha planes or report precise plane allocation requirements.

Next recorded action: Add checked plane preflight and ownership only after API-024/025.

### AVF-020

Status at ledger review: **open**.

Portable encoded output has no independent compatibility lane through libavif and at least one browser decoder.

Next recorded action: Require both before replacing the native encoder bridge; Pillow parity alone may reuse the same native stack.

### AVF-022

Status at ledger review: **open**.

A file can contain both a primary image item and an image-sequence track. The common API does not expose source selection or report which source auto mode chose.

Next recorded action: Add item/track capability and explicit source selection, retaining auto as a documented policy.

### AVF-023

Status at ledger review: **open**.

Progressive/layered AVIF has layer count and partial-detail state, while incremental libavif can expose completed row count. Current decode returns only one terminal raster.

Next recorded action: Classify progressive source versus timed sequence and define provisional output/finish semantics before portable support expands.

### AVF-024

Status at ledger review: **open**.

Sequence random access depends on keyframes and a frame's maximal dependent byte extent. Eager full decode hides this and cannot support bounded range input.

Next recorded action: Retain keyframe/dependency information in a source-bound sequence decoder and prove Nth-frame equivalence with sequential decode.

### AVF-025

Status at ledger review: **open**.

Repetition can be finite additional repeats, infinite, or unknown; libavif defines finite `n` as `n + 1` total plays. The common loop field cannot preserve all states.

Next recorded action: Implement API-050 with exact native/Pillow sequence fixtures.

### AVF-026

Status at ledger review: **open**.

libavif metadata is valid after parse, but outer container properties may change after inner AV1 headers are decoded. The current inspect/decode contract has no “declared versus confirmed” distinction.

Next recorded action: Retain both observations when they differ and fail only according to an explicit strictness rule.

### AVF-027

Status at ledger review: **open**.

Main color, alpha, and gain-map content can be selected independently. A single normalized RGBA result cannot state which auxiliary content was decoded, ignored, or unavailable.

Next recorded action: Add content-selection capability flags and auxiliary relationships before gain-map support; applying a gain map remains image processing and out of scope.

### AVF-028

Status at ledger review: **open**.

Opaque and UUID item properties now have association order, essential bits, transform order, and an independent 2,048-entry `ipco` unique-property ceiling. Collision rules with properties generated by the encoder remain open.

Next recorded action: Preserve ordered raw properties, enforce the finite property-table ceiling, and reject unsafe encoder collisions; never replay unknown essential properties as if understood.

### AVF-029

Status at ledger review: **open**.

EXIF storage carries a TIFF-header offset, and libavif may derive `irot`/`imir` from EXIF orientation. The model now retains raw EXIF and independent container transform provenance, but does not parse orientation or detect contradictions.

Next recorded action: Preserve the independent fields and define contradiction/auxiliary-item policy; document that neither is applied to pixels.

### AVF-030

Status at ledger review: **open**.

Decoder strict flags, diagnostic text, I/O byte statistics, ignored EXIF/XMP policy, image count/dimension/pixel limits, and waiting-on-I/O state are available in the native reference but have no portable/common mapping.

Next recorded action: Define stable stage/limit/progress fields and use the same fixtures against native and portable implementations.

### AVF-031

Status at ledger review: **open**.

AVIF auxiliary alpha now has `SourceAlpha::Auxiliary` plus bounded source-local auxiliary-item relationships on inspection, still decode, and sequence-frame decode, backed by both direct `alpha.avif` and grid-derived `grid.avif` fixtures. The primary grid retains its ordered derived item IDs, validated version/flags/row/column/output-canvas topology through `SourceDescriptor::avif_grid_properties()`, and bounded `dimg` references through `SourceDescriptor::avif_item_relationships()`. Source-local `prem` edges are retained through `SourceDescriptor::avif_premultiplied_relationships()`, typed non-primary `colr`/`nclx` CICP declarations through `SourceDescriptor::avif_item_color_properties()`, non-primary `prof`/`rICC` profiles through `SourceDescriptor::avif_item_icc_profiles()`, unknown and known `clli`/`mdcv`/`irot`/`imir`/`pasp`/`clap` properties through `SourceDescriptor::avif_item_properties()`, non-primary/auxiliary `ispe`/`pixi` declarations through `SourceDescriptor::avif_item_plane_properties()`, and non-primary/auxiliary `av1C` declarations through `SourceDescriptor::avif_item_codec_properties()`; the existing alpha and grid fixtures assert the typed records, while duplicate declarations are mutated only in memory and decoded normalized bytes remain identical. Tile placement/composition, plane range/quality, non-alpha auxiliary payloads, other known forms beyond typed CICP/ICC/codec, and exact invisible RGB remain unrepresented.

Next recorded action: Add exact plane/relationship fixtures for the remaining auxiliary classes before high-depth alpha; keep source provenance separate from decoded transfer bytes.

### AVF-032

Status at ledger review: **open**.

`iloc` construction methods, multiple extents, data references, idat/mdat placement, non-sequential extents, and 64-bit range arithmetic are not a named corpus class.

Next recorded action: Add structural extent fixtures with a cumulative byte/range limit and precise box/item context.

### AVF-033

Status at ledger review: **open**.

Grid-derived images and gain maps may use different grids/dimensions from the primary image. The current grid fixture proves and retains the ordered derived-item list, validated grid version/flags/row/column/output-canvas topology, and alpha-to-derived-item relationships, but flattening the grid into one canvas still loses tile placement/composition and partial-failure context.

Next recorded action: Retain per-tile placement and validate every tile independently; composition stays private to decode until a composable public model is justified.

### AVF-034

Status at ledger review: **open**.

Fragmented sequence files, edit lists, sample groups, sample-description changes, sync-sample tables, and timestamp offsets have no capability statement.

Next recorded action: Classify each BMFF track feature before claiming general animated AVIF support.

### AVF-035

Status at ledger review: **open**.

`SourceDescriptor::avif_file_type()` now retains the FileTypeBox major brand, minor version, and ordered compatible brands with an independent 1,024-entry ceiling on inspection, still decode, and sequence-frame decode. The complete whole-file/item AV1 codec-capability view is still not exposed, so detection, inspection, and actual decoder capability can disagree even though bounded non-primary `av1C` declarations are retained.

Next recorded action: Generate the capability decision from FileTypeBox declarations, item codec declarations, and target restrictions; retain declared-versus-confirmed capability when they differ.

### FTR-001

Status at ledger review: **open**.

One format feature includes inspect, decode, sequence code, and encode. A decode-only consumer still compiles that format's encoder.

Next recorded action: Measure native/WASM binary sections first. If material, keep the existing format umbrella and add additive internal `*-decode`/`*-encode` features only with a simple supported migration.

### FTR-002

Status at ledger review: **open**.

`ico = ["bmp", "png"]` enables complete BMP and PNG public codecs, not just embedded-entry internals.

Next recorded action: Keep until measurements justify refactoring; any split must still let `ico` work by itself and remain additive.

### FTR-006

Status at ledger review: **open**.

The build script falls back to host `cc`/`ar` unless target-specific environment variables exist.

Next recorded action: Add documented target-tool lookup tests for representative cross targets; fail with the exact missing tool variable.

### FTR-009

Status at ledger review: **open**.

CI has no macOS, Windows, 32-bit runtime, or big-endian runtime lane; the WASM runtime lane covers `wasm32-wasip1` only.

Next recorded action: Add the smallest target matrix that catches build-script, endian, pointer-width, and runtime differences.

### FTR-010

Status at ledger review: **open**.

There is no JS binding, package manifest, core/extra artifact, loader, or runtime test yet.

Next recorded action: Keep this after native semantic parity; define copy behavior, errors, feature mapping, bundler targets, and reproducible compressed/uncompressed sizes.

### FTR-011

Status at ledger review: **open**.

Cargo feature selection does not shrink the downloaded `.crate`; it ships source for every codec and required legal texts. The locally produced `6f9c002` archive is 466,477 bytes compressed and 2,540 KiB unpacked.

Next recorded action: Document download size separately from linked native/WASM size; optimize source packaging only after legal and reproducibility requirements.

### FTR-013

Status at ledger review: **open**.

`bytemuck` is retained as the only Cargo dependency but current production code references it only as an approved placeholder (`use bytemuck as _`).

Next recorded action: Do not remove it contrary to the accepted constraint; either use it for a justified checked byte-layout boundary later or document why it remains intentionally unused.

### FTR-014

Status at ledger review: **open**.

README says the crate is unpublished/pre-release, but `Cargo.toml` does not restrict `publish`.

Next recorded action: Add an explicit release checklist and decide whether `publish = false` remains until every release blocker closes.

### FTR-015

Status at ledger review: **open**.

No `[package.metadata.docs.rs]` policy records desired features/targets. docs.rs defaults do not build all features, so the target-sensitive AVIF surface may be underrepresented.

Next recorded action: Set an intentional docs.rs feature/target policy that does not require the native AVIF stack and clearly labels restricted behavior.

### FTR-016

Status at ledger review: **open**.

Default enables all seven Rust codecs. This is convenient for applications but costly for downstream libraries that forget `default-features = false`.

Next recorded action: Keep the current default for 0.1 unless measurements/user feedback justify change; make minimal feature selection the primary library-integration example.

### FTR-017

Status at ledger review: **open**.

`wasm32-unknown-unknown` supplies `std`, but filesystem calls fail and thread spawning is unavailable; no audit proves codec paths avoid unsupported host facilities. Runtime tests now execute the feature-gate and capability-table lanes on `wasm32-wasip1`; the full semantic matrix and target-call inventory remain.

Next recorded action: Add a target-call inventory and full runtime tests. `no_std + alloc` is an optional P3 goal, not a substitute for the required browser/JS WASM support.

### FTR-018

Status at ledger review: **open**.

There is no measured `simd128` build lane, scalar equivalence gate, or target-feature policy.

Next recorded action: Benchmark per codec, require exact semantic parity, and keep scalar WASM functional before enabling optional SIMD artifacts.

### FTR-020

Status at ledger review: **open**.

No binding contract states when encoded input and decoded output are copied across the JS/WASM boundary. Common `wasm-bindgen` boxed-slice conversions copy.

Next recorded action: Measure `Uint8Array` input/output and document ownership, detachment, memory growth, and reuse before promising zero-copy.

### FTR-021

Status at ledger review: **open**.

Browser ESM, bundler ESM, Node, Deno, workers, and CommonJS packaging are not selected or tested.

Next recorded action: Choose the smallest supported target set and test actual imports in each published environment; avoid one ambiguous package.

### FTR-022

Status at ledger review: **open**.

Peak memory, memory growth, allocation failure, maximum pages, and large `Vec` transfer behavior are undefined.

Next recorded action: Bind decode/output limits to measured WASM memory and return typed failures rather than relying on trap/OOM behavior.

### FTR-023

Status at ledger review: **open**.

Cooperative cancellation now exists for decode and the partial encode boundary (API-036/COR-060/COR-070/COR-072) and never publishes a successful partial result; there is still no worker-safe binding or transferable-buffer policy.

Next recorded action: Design the JS/WASM binding after the native API settles; the token's single-threaded `Rc<Cell>` state must be replaced or wrapped for worker transfer.

### FTR-024

Status at ledger review: **open**.

The intended core/extra JS split has no checked membership manifest, feature mapping, loader behavior, or per-codec native/WASM size budget.

Next recorded action: Define exact artifact inputs and measure raw, gzip, and Brotli sizes for each revision. Do not infer size from `.crate` source size.

### FTR-027

Status at ledger review: **open**.

No pinned binding tool version, generated-glue checksum, deterministic package archive, or clean-consumer install test exists.

Next recorded action: Pin the release toolchain and compare produced artifact hashes in a clean CI environment.

### FTR-029

Status at ledger review: **open**.

There is no per-format attribution for Rust code, data tables, generated bindings, native shims, or compression after link-time optimization.

Next recorded action: Produce additive singleton/default/all artifacts with identical compiler flags and report deltas without claiming they sum linearly.

### FTR-034

Status at ledger review: **open**.

The crate builds as the default Rust library type only; it has no `cdylib`/binding wrapper, exported C/JS ABI, or generated package. A successful `wasm32` rlib build is not a consumable JavaScript codec.

Next recorded action: Choose a thin binding crate or deliberate crate-type strategy after the Rust API settles, keeping codec features forwarded explicitly.

### FTR-035

Status at ledger review: **open**.

The portable AVIF decoder branch is shared, but browser `wasm32-unknown-unknown`, WASI, and future component targets have different I/O/thread/runtime contracts. `CapabilityTarget` now distinguishes the supported WASI and non-WASI `wasm32` classes, while runtime evidence remains keyed by the full `wasm32-wasip1` triple and `wasm32-unknown-unknown` remains compile/rustdoc-only.

Next recorded action: Keep the full-triple capability fixture and publish only triples with runtime tests; add further target classes only with their own runtime contract.

### FTR-037

Status at ledger review: **open**.

One Cargo source package can produce many feature builds, but it cannot by itself define two independently versioned JS archives, loader fallback, cache keys, or shared types.

Next recorded action: Specify core/extra as reproducible release artifacts generated from one revision and one API schema; do not imply that Cargo feature names alone solve package splitting.

### FTR-038

Status at ledger review: **open**.

Format features are public Cargo API. Adding operation subfeatures, changing default membership, or making `ico` stop forwarding `png`/`bmp` can break downstream feature assumptions even if Rust symbols remain.

Next recorded action: Include feature-set diffs in the release compatibility gate and publish the umbrella/subfeature rule from FTR-026.

### QA-001

Status at ledger review: **open**.

Current feature CI runs no-feature, each singleton, default, and all, but not relevant pairwise combinations or the full powerset.

Next recorded action: Static cfg inventory plus targeted pairs for shared compression and ICO; a powerset only if runtime cost stays reasonable.

### QA-002

Status at ledger review: **open**.

The all-feature semantic manifest runs natively only. The feature-gate and capability-table suites now execute on `wasm32-wasip1`, but the full semantic matrix is still not executed in a WASM runtime.

Next recorded action: Execute default, singleton, and all supported semantic rows in a real WASM runtime.

### QA-003

Status at ledger review: **open**.

Coverage is all-feature native coverage. Disabled-feature arms and target-only behavior are partly reached by separate tests or coverage hooks, not one semantic snapshot.

Next recorded action: State coverage provenance per lane and compare native/WASM snapshots only where source mappings are compatible.

### QA-006

Status at ledger review: **open**.

The encode manifest samples many options but is not a Cartesian source-mode × target-format matrix.

Next recorded action: Add one row per Pillow-accepted/rejected mode boundary and one cross-format decode→encode row for every claimed transcode.

### QA-009

Status at ledger review: **open**.

No fuzzing, mutation corpus, or differential randomized test runs in CI.

Next recorded action: Add format-aware fuzzing after limits; preserve minimized failures as fixtures.

### QA-010

Status at ledger review: **open**.

`scripts/benchmark_fixture_workloads.py` schema-`@3` provides a clean-revision protocol with a fixed four-worker budget, separate Pillow-parity and Rust feature-gate wall/user/sys timings, direct-child POSIX peak-RSS observations, manifest/matrix hashes, native release-library size, and WASM compile-artifact size. At test/runtime revision `646ed73413a574368bfd01172fcd46c60622046f`, all four workloads passed: a clean run measured 1.327542 s / 3.260114 user s / 0.223734 sys s / 289,112,064-byte peak RSS for Pillow parity; 2.409336 s / 2.985661 user s / 0.192187 sys s / 245,104,640-byte peak RSS for the separate Rust-only feature-gate suite; 11.894242 s wall and a 7,993,312-byte native `rlib`; and 5.589502 s wall and a 25,083,139-byte WASM artifact. These observations are host/cache/toolchain-specific and do not establish a universal speed or memory improvement for suffix allocation recycling or Huffman-RLE checkpointing. Stack depth, allocator counts, retained-cache size, caller-buffer reuse, and WASM runtime measurements remain uncollected.

Next recorded action: Run the protocol on fixed lossy/lossless/alpha/animation workloads, add stack/allocator/cache/buffer-reuse and WASM-runtime collectors, and compare repeated same-host revisions before any "fast", "small", or "lightweight" claim.

### QA-011

Status at ledger review: **open**.

No semver/public API diff runs before release.

Next recorded action: Add a public API snapshot once enum/type decisions settle.

### QA-012

Status at ledger review: **open**.

Test fixtures prove Pillow 12.2.0 behavior, not every legal file accepted by the format specification.

Next recorded action: Maintain a separate format-completeness corpus and classify divergences rather than relabeling them Pillow parity.

### QA-016

Status at ledger review: **open**.

A dependency-free `OutputSink` contract exists with deterministic `OutputWrite` cause coverage for every enabled still codec and supported sequence path. JPEG, PNG, BMP, GIF still and sequence, WebP still and multi-frame sequence delivery, ICO still delivery, native AVIF still and sequence delivery, and the one-frame JPEG/BMP/WebP/ICO plus multi-page TIFF sequence deliveries now exercise multiple structural writes, policy preflight, sink-triggered cancellation where implemented, and one post-delivery flush with typed flush-failure coverage; JPEG still and one-frame sequence delivery additionally cover marker/scan boundaries, GIF delivery additionally covers signature/logical-screen, color-table, extension/image sub-block, and trailer segments, TIFF still and multi-page sequence delivery additionally cover the header, strip/padding, and IFD/value segments, and AVIF delivery additionally covers top-level ISO-BMFF box boundaries. The Rust-only cross-codec contract at test/runtime revision `163520b4ab06b9f4b15c2a6e8bdc12e9a29c4d39` proves a genuine partial second structural write and post-delivery flush rejection for every available still writer and each supported multi-frame GIF/TIFF/WebP/native-AVIF sequence writer, with the selected `OutputWrite` cause, exactly one flush attempt, exact delivered-byte preservation, and no flush after a failed structural write. Short-write recovery beyond the tested prefix, rollback, and partial-container cleanup remain open.

Next recorded action: The current structural boundaries now prove preflight preservation, checkpoint rollback, final-segment cancellation, flush ordering, still/sequence identity, and rollback-failure precedence; continue auditing future short-write and cleanup paths before claiming a universal incremental writer.

### QA-019

Status at ledger review: **open**.

Exact encoded-byte determinism is now proven between the ARM64 native host and `wasm32-wasip1` for a fixed encoder/decoder subset; x86-64, 32-bit, and big-endian lanes are still missing.

Next recorded action: Run deterministic fixture subsets across the remaining targets and classify unavoidable native-oracle differences explicitly.

### QA-020

Status at ledger review: **open**.

Peak stack use and recursion depth are not measured for nested containers, TIFF directory graphs, DEFLATE/Huffman paths, or AV1 syntax.

Next recorded action: Add bounded deep-structure fixtures and stack instrumentation before browser/embedded recommendations.

### QA-021

Status at ledger review: **open**.

Reverse-mapped/generated fixtures do not all retain generator version, parameters, first-divergence purpose, and minimized-input hash in the manifest.

Next recorded action: Extend TST-009 with reproducible generation provenance and a regeneration check.

### QA-022

Status at ledger review: **open**.

WASM compile success provides no browser evidence for boundary copies, memory growth, exceptions, worker use, or real artifact size.

Next recorded action: Run a small Playwright/WebDriver-free JS harness in a pinned browser runtime and Node for every published artifact target.

### QA-023

Status at ledger review: **open**.

Emitted bytes are primarily re-opened through Pillow, which can share libjpeg/libwebp/libtiff/libavif implementations with the oracle path.

Next recorded action: Decode representative outputs with an independent implementation or browser and record that evidence separately from Pillow parity.

### QA-024

Status at ledger review: **open**.

Round-trip tests do not publish a uniform rule separating lossless exact samples, lossy decoded tolerances, and deterministic encoded bytes.

Next recorded action: Add an assertion policy per format/mode/option row and reject ambiguous generic “round trip passed” claims.

### QA-026

Status at ledger review: **open**.

Decode/output policy boundaries, cache/retry behavior, sink preflight, structural cancellation, and the currently implemented Rust-only work-budget checkpoints—including PNG stored-block boundaries and 1,024-byte stored-block-copy intervals, all-level Deflate matcher/expansion/Huffman/bitstream/checksum stages, BMP row-conversion subsegments, JPEG RGB-to-YCbCr conversion and chroma-downsample output after each 1,024 pixels, baseline entropy traversal after each 1,024 MCUs, optimized baseline Huffman frequency gathering after each 1,024 AC coefficients, progressive scan block-slot generation after each 1,024 blocks, progressive scan-event frequency gathering after each 1,024 events, progressive scan coefficient traversal after each 1,024 coefficients, entropy output after each 1,024 emitted bytes, high-color GIF nearest-palette candidate ordering and bounded scans after each 1,024 work items, lossy WebP RGBA transparent-area cleanup and alpha-palette source collection and index packing plus lossy WebP VP8 required padded Y/U/V edge-replication, analysis segment assignment, filter-edge adjustment, coefficient-statistics collection, and first-partition segment-probability prepass after each 1,024 padded item or selected macroblock as applicable plus lossless VP8L hidden-RGB cleanup, image-palette source scans, ordered unique-color palette drains after each 1,024 pixels or colors, and palette-mode index packing plus sampled meta-pixel materialization after each 1,024 retained histogram symbols, VP8 first-partition intervals through 262,144 logical bits and coefficient intervals through 2,097,152 logical bits, VP8 boolean boundaries, 1,024-byte boolean output, lossless VP8L 256-pixel copy-token cache-population scans, palette-index lookup candidate scans after each 64 palette entries, palette sign collection and nearest-delta candidate scans after each 64 palette entries or candidate values, Huffman RLE preparation and in-run code-length scans after each 64 symbols, canonical-code assignment scans after each 64 code-length symbols, Huffman-tree ordering comparisons after each 64 comparisons, Huffman-tree insertion scans after each 64 candidate nodes, Huffman code-length-token frequency and trailing zero-repeat-token trim scans after each 16 compressed token entries, Huffman code-length emission after each 16 compressed token entries, and lossless VP8L logical bitstream intervals through 2,097,152 bits—are accepted in the feature-gated integration contract. The Pillow manifest remains the source of Pillow-observable success/error/byte evidence; it does not own caller budgets, cancellation, sink prefixes, or rollback.

Next recorded action: Add only real remaining codec/interior/allocation/short-write boundaries, keep them in the existing feature-gate contract, and record parity as unchanged regression evidence rather than adding synthetic Pillow rows.

### QA-027

Status at ledger review: **open**.

Encoder option determinism can be affected by unordered `HashMap` extras and target-native libraries, but cross-process output stability is not checked.

Next recorded action: Replace public catch-all options, sort any retained opaque options, and compare independent process runs.

### QA-028

Status at ledger review: **open**.

Corpus growth is counted in rows, not unique parser states/properties; many rows may exercise the same structural class. The WebP VP8L map now records 98 named Pillow witnesses, 84 successful structural checks, 40 malformed parser checks, 16 witnessed properties, 79 distinct active WebP rows, and the exact boundary of what Pillow proves.

Next recorded action: Extend the same map discipline to each codec and replace candidate-only VP8L entries with independently checked structural witnesses before claiming state coverage.

### QA-030

Status at ledger review: **open**.

The schema-`@3` fixture benchmark protocol separates parity and Rust-only suite time with a fixed four-worker budget, records compiled artifact sizes, and now records direct-child POSIX user/sys time and peak RSS; the parity harness shares immutable repeated source decodes across its partitioned test functions. It still does not measure output allocation count, retained encoded+decoded cache memory, sequence amplification, caller-buffer reuse, stack depth, or runtime WASM.

Next recorded action: Add allocator/cache/buffer-reuse, stack, and runtime-WASM measurements alongside time, peak RSS, and artifact size; never optimize from source line count.

### QA-031

Status at ledger review: **open**.

Legal-but-unsupported format classes are not a uniform fixture lane. Some are absent entirely, while malformed inputs dominate error coverage.

Next recorded action: Add active negative-capability rows for every named legal class and require `Unsupported` rather than incidental `Malformed`.

### QA-033

Status at ledger review: **open**.

Generator reproducibility is checked through hashes inside generated data, but a clean regeneration/no-diff run is not a mandatory CI gate for every script and asset.

Next recorded action: Run generators in a clean checkout, fail on any diff, and record pinned Python/native tool identities.

### QA-034

Status at ledger review: **open**.

Debug and optimized builds are not compared for exact results. Overflow checks, floating-point/codegen choices, and `cfg(debug_assertions)` can expose behavior that line coverage in one profile misses.

Next recorded action: Run a compact deterministic parity subset in both profiles and compare structured errors and artifacts.

### QA-035

Status at ledger review: **open**.

`EncodedImage` clone/cache concurrency is described but not stress-tested for one initialization, shared success, shared failure, panic recovery, and deterministic observations across threads.

Next recorded action: Add bounded concurrent tests without timing assertions; if WASM threads are later supported, repeat under that artifact.

### QA-036

Status at ledger review: **open**.

Future streaming decoders need lifecycle tests for every prefix boundary, repeated empty append, finish before complete, append after finish, reset, cancellation, and retained partial output.

Next recorded action: Derive prefix fixtures from existing files and compare terminal output with one-shot decode.

### QA-037

Status at ledger review: **open**.

Container-equivalent metamorphic variants are sparse: resegmented PNG IDAT/fdAT, GIF sub-block splits, reordered legal TIFF strips, WebP padding, and AVIF extent partitioning should preserve the same observable result.

Next recorded action: Add generated equivalence families while retaining exact original container metadata where the model exposes it.

### QA-039

Status at ledger review: **open**.

Oracle identity records Pillow 12.2.0 but not one generated fingerprint of every compiled Pillow feature and linked codec version used to create fixtures.

Next recorded action: Store and validate `PIL.features` plus libjpeg/zlib/libtiff/libwebp/libavif/backend versions with the manifest provenance tuple.

### QA-040

Status at ledger review: **open**.

Exact output rows do not all state whether determinism is expected across process, architecture, native backend, compiler, and optimization profile, or only within the pinned oracle build.

Next recorded action: Add a determinism scope field and test only the dimensions it promises.

### QA-041

Status at ledger review: **open**.

No retained test compares `inspect` facts before decode with source facts confirmed during/after decode when inner payload headers can disagree with outer containers.

Next recorded action: Add declared-versus-confirmed fixtures for AVIF first, then any JPEG/TIFF/WebP path with late-discovered source properties.

### QA-042

Status at ledger review: **open**.

Fixture selection has no explicit coverage for public object mutation after construction: changing dimensions, mode, palette, frame metadata, or pixels between successful validation and encode.

Next recorded action: Generate post-construction mutation cases and require every public encoder entry to revalidate without panic.

### DOC-007

Status at ledger review: **open**.

The changelog contains detailed unreleased implementation claims but no released-version link/reference structure yet.

Next recorded action: Add comparison links and release entries only at the first tag; until then, keep every claim consistent with the current branch.

### DOC-008

Status at ledger review: **open**.

Support, security, contribution, conduct, issue forms and CODEOWNERS exist, but maintainer succession/governance and release recovery are only roadmap statements.

Next recorded action: Keep this P2 for a single-maintainer pre-release; define ownership and recovery before inviting production reliance.

### FMT-000

Status at ledger review: **parked**.

AV1 is already codec work required inside AVIF.

Next recorded action: It is not itself a general image container. Keep AV1 internal unless a real caller needs raw OBU/Annex-B still input/output with its own color/dimension contract.

### FMT-001

Status at ledger review: **parked**.

Small specification, lossless RGB/RGBA, deterministic bytes, natural Rust/WASM fit.

Next recorded action: Pillow 12.2.0 has no built-in oracle; require a pinned specification/reference exception and exhaustive malformed/overflow cases before eligibility.

### FMT-002

Status at ledger review: **parked**.

Simple uncompressed family, Pillow coverage, useful high-depth and parser-limit fixtures.

Next recorded action: Decide whether one feature/format enum covers six magic values, ASCII/binary variants, tuple types, comments, maxval scaling, and exact whitespace output.

### FMT-003

Status at ledger review: **parked**.

Pillow-readable/writable, palette/RLE/direct-color variants, bounded container complexity.

Next recorded action: Origin bits, color-map ranges, 15/16-bit alpha, RLE crossing rows, extensions, developer area, and footer identity all need exact policy.

### FMT-004

Status at ledger review: **parked**.

Pillow-readable/writable and a contained palette/RLE implementation.

Next recorded action: Plane layouts, bytes-per-line padding, EGA/VGA palettes, truncated RLE, version/header variants, and multi-plane output need fixtures.

### FMT-005

Status at ledger review: **parked**.

Multi-entry image container fits the existing no-resize ICO direction.

Next recorded action: Embedded PNG/JPEG 2000/legacy pixel and mask blocks make capability transitive; entry enumeration must exist first.

### FMT-006

Status at ledger review: **parked**.

Tiny lossless 16-bit RGBA specification and deterministic stream.

Next recorded action: No built-in Pillow oracle; low ecosystem value means it follows QOI and current 16-bit transfer work.

### FMT-007

Status at ledger review: **parked**.

Image codec work with float/high-range semantics and Pillow decode evidence.

Next recorded action: Confirm a deterministic encode oracle, orientation/exposure/gamma contract, RLE variants, float transfer mode, and non-processing color policy.

### FMT-008

Status at ledger review: **parked**.

Common container with raw and block-compressed texture payloads.

Next recorded action: Many variants are GPU texture formats rather than ordinary raster codecs; mipmaps, arrays, cubemaps, BCn encode scope and oracle choice are unresolved.

### FMT-009

Status at ledger review: **parked**.

Pillow can use OpenJPEG and the format is relevant to ICNS/TIFF ecosystems.

Next recorded action: Wavelet codec size, precinct/tile/progression/color-box complexity, native-oracle licensing, zero-dependency and WASM cost make it parked.

### FMT-010

Status at ledger review: **parked**.

Modern still/animation, high depth, color and metadata capabilities.

Next recorded action: No pinned Pillow 12.2.0 built-in oracle and substantial transform/modular/VarDCT scope; parked until an acceptable fixed oracle and independent vectors exist.

### FMT-011

Status at ledger review: **parked**.

ISO-BMFF work could share bounded container concepts with AVIF.

Next recorded action: HEVC implementation, licensing/patent review, grids/auxiliary images and missing pinned Pillow oracle make it parked. AVIF completion does not imply HEIC.

### FMT-012

Status at ledger review: **parked**.

Important high-depth/multipart image interchange.

Next recorded action: Arbitrary channels, deep data, tiled levels, compression families, metadata and float layouts require a much broader transfer model; parked.

### FMT-013

Status at ledger review: **parked**.

Frequently accepted by image tools at a higher layer.

Next recorded action: Explicitly ineligible: rendering/vector/page/video processing is not raster image codec implementation for this repository.

## Required acceptance commands

```sh
make verify
make lint
make test
make coverage
make package-verify
```

A complete source-coverage claim additionally requires `make coverage-complete`.
The current alpha floors do not remove the full denominator or permit failed
tests. Changes to codec behavior need the matching public oracle fixture.
